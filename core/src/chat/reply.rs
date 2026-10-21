//! One answer of the model: a run of the `Chat` role, read-only, in the chat's copy of the project.
//! Split by what changes it: which model (`choose_model`), what the run came to (`read_outcome`), what the
//! model gets when its session is gone (`recap`). Choosing a model, folding a run, recording limits and costs
//! are the engine's own functions, so a chat and a task of one env are treated alike.
use super::*;
use crate::agent::{self, Adapter, Run};
use crate::engine::{fold, pick, tail, Picked};
use crate::models;
use regex::Regex;
use std::sync::OnceLock;

/// What the chat role adds to the system prompt of the role (the rest - read-only, data blocks - is there already).
const CHAT_RULE: &str = "You are in a chat with the person about their project. Answer questions about the code and the project. \
Never change files. When the person's message asks for a change of the code, answer briefly and end your reply with \
the last line `NS_TASK: <short title of the task>`; never add that line to the answer of a question.";

/// The CLI did not find the session to continue.
fn session_lost(text: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| {
        Regex::new(r"(?i)no conversation found|(?:session|conversation).{0,60}(?:not found|no such|does not exist|expired|invalid|unknown|could not be (?:found|resumed))|could not (?:find|resume).{0,40}(?:session|conversation)")
            .expect("static pattern")
    });
    r.is_match(text)
}

/// What one run of the model came to.
enum Outcome {
    Reply { session: Option<String>, text: String },
    /// Not answered; the reason is a key with details (`reason::*`).
    Failed(String),
    /// The limit is used up until then.
    Limit(DateTime<Local>),
    /// The CLI does not know the session it was asked to continue.
    SessionLost,
}

impl Chats {
    // ---- step 1: which model ----

    /// The model of the chat role: the env's own choice for it, else the env's default, else the CLI's default
    /// as the catalog names it. Same function as for the roles of a task, so the vendor and the caption agree.
    fn choose_model(&self, env: &str, a: &dyn Adapter, env_set: &EnvSettings) -> Picked {
        let cat = models::catalog(env, a, &lk(self.engine.store()));
        let choice = env_set.roles.get(&RoleKind::Chat).or(env_set.default_model.as_ref()).cloned();
        let choice = choice.unwrap_or(ModelChoice { family: models::DEFAULT_FAMILY.into(), version: None });
        pick(&cat, env, Some(&choice))
    }

    // ---- step 2: what the run came to ----

    /// Records what the env reported (windows, cost, a used-up limit) the way the engine does for a task,
    /// and says what the run came to.
    fn read_outcome(&self, env: &str, had_session: bool, events: &[agent::AgentEvent]) -> Outcome {
        let f = fold(events);
        if let Some(w) = &f.windows {
            self.engine.record_usage(env, w.clone());
        }
        if let Some(c) = f.cost {
            self.engine.add_day_cost(env, c);
        }
        if let Some(m) = &f.auth {
            return Outcome::Failed(format!("{}: {m}", reason::AUTH_REQUIRED));
        }
        if let Some(m) = &f.rejected {
            return Outcome::Failed(format!("{}: {m}", reason::MODEL_REJECTED));
        }
        if let Some(resets) = f.limit {
            let known = self.engine.limit_reset(env, resets, f.windows.as_ref());
            return Outcome::Limit(self.engine.record_limit_hit(env, known));
        }
        match f.done {
            Some((session, text, false)) => Outcome::Reply { session, text },
            Some((_, text, true)) if had_session && session_lost(&text) => Outcome::SessionLost,
            Some((_, text, true)) => Outcome::Failed(format!("{}: {}", reason::CLI_ERROR, tail(&text, 3).replace('\n', " "))),
            None => Outcome::Failed(reason::AGENT_NO_RESULT.to_string()),
        }
    }

    // ---- step 3: a new session after a lost one ----

    /// The last 20 messages of the person and of the model before `msg`, for a session that starts from nothing (G20.6).
    fn recap(&self, chat: &str, msg: &ChatMsg) -> String {
        let lines: Vec<String> = self
            .recent_talk(chat, msg.at, 20)
            .iter()
            .map(|m| format!("{}: {}", if m.who == Who::Human { "person" } else { "assistant" }, m.text))
            .collect();
        lines.join("\n")
    }

    // ---- the answer ----

    fn fail(&self, chat: &str, msg_id: &str, why: &str) {
        self.line(chat, keys::NOT_ANSWERED, why, Some(ChatKind::Error), Some(msg_id), None);
    }

    fn hold_for_limit(&self, c: &Chat, job: &Job, until: DateTime<Local>) {
        let at = until.format("%H:%M").to_string();
        self.line(&c.id, keys::WILL_ANSWER_AFTER, &at, Some(ChatKind::Warn), Some(&job.msg.id), None);
        self.wait_until(&c.id, job.clone(), until);
    }

    /// Answers one message. Everything that goes wrong ends as a line "not answered" in the feed.
    pub(super) fn answer(self: &Arc<Self>, chat: &str, job: &Job) {
        let msg = &job.msg;
        let Some(c) = self.chat(chat) else { return };
        let Some(p) = lk(self.engine.store()).project(&c.project_id) else { return self.fail(chat, &msg.id, reason::PROJECT_NOT_FOUND) };
        let Some(a) = self.engine.adapter(&c.env) else { return self.fail(chat, &msg.id, &format!("{}: {}", reason::AGENT_NOT_FOUND, c.env)) };
        if let Err(why) = a.available() {
            return self.fail(chat, &msg.id, &format!("{}: {why}", reason::ENV_UNAVAILABLE));
        }
        if let Some(until) = self.engine.limit_wait(&c.env, self.now()) {
            return self.hold_for_limit(&c, job, until);
        }
        let copy = match self.ensure_copy(&c, &p) {
            Ok(d) => d,
            Err(e) => return self.fail(chat, &msg.id, &format!("{}: {e}", reason::COPY_FAILED)),
        };
        let env_set = self.settings().envs.get(&c.env).cloned().unwrap_or_default();
        let model = self.choose_model(&c.env, &*a, &env_set);
        let signature = format!("{} · {}", a.name(), model.label);

        let (mut session, mut recap) = (c.session.clone(), None);
        loop {
            let mut run = Run::new(
                copy.clone(),
                message_prompt(&msg.text, recap.as_deref()),
                format!("{}\n\n{CHAT_RULE}", prompt::system_for(model.vendor, RoleKind::Chat)),
                RoleKind::Chat,
            );
            run.resume = session.clone();
            run.model = model.cli_arg.clone();
            run.read_only = true;
            run.extra_args = env_set.extra_args.clone();
            run.extra_env = self.engine.env_vars_of(&c.env);
            let events = match agent::run_blocking(&*a, &run, &mut |_| {}, &self.stop) {
                Ok(_) if self.stop.load(Ordering::SeqCst) => return,
                Ok(e) => e,
                Err(e) => return self.fail(chat, &msg.id, &format!("{}: {e}", reason::RUN_FAILED)),
            };
            match self.read_outcome(&c.env, session.is_some(), &events) {
                Outcome::Reply { session, text } => return self.reply(&c, job, &signature, session, &text),
                Outcome::Failed(why) => return self.fail(chat, &msg.id, &why),
                Outcome::Limit(until) => return self.hold_for_limit(&c, job, until),
                Outcome::SessionLost => {
                    self.update_chat(chat, |x| x.session = None);
                    self.line(chat, keys::NEW_SESSION_STARTED, "", Some(ChatKind::Warn), Some(&msg.id), None);
                    recap = Some(self.recap(chat, msg));
                    session = None;
                }
            }
        }
    }

    /// The answer goes to the feed; a closing `NS_TASK:` line makes the message a task and does not show.
    fn reply(&self, c: &Chat, job: &Job, signature: &str, session: Option<String>, text: &str) {
        if session.is_some() {
            self.update_chat(&c.id, |x| x.session = session.clone());
        }
        let sig = prompt::parse_signals(text);
        let is_task = !job.no_task && sig.ns_task.is_some();
        let shown = if is_task { sig.body.clone() } else { text.trim().to_string() };
        if !shown.is_empty() {
            self.push(ChatMsg {
                id: uuid::Uuid::new_v4().to_string(),
                chat_id: c.id.clone(),
                at: self.now(),
                who: Who::Model,
                text: shown,
                task_id: None,
                delivered_at: None,
                kind: None,
                tool: None,
                model: Some(signature.to_string()),
                reply_to: Some(job.msg.id.clone()),
            });
        }
        if is_task {
            self.make_task(c, &job.msg);
        }
    }
}
