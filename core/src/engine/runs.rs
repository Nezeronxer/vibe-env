//! What every run of every role comes to, whoever started it (the role of a task or a lane of a swarm):
//! folding what the run reported, the cost, the limit windows, a refused model, a missing login, a
//! limit hit with the other model to go on with, the silence of the Watcher. One function decides, so
//! the same failure of the env leads a lane and a task to the same end.
use super::roles::{pick, BUDGET_EXCEEDED, COST_LOG, DAY_BUDGET_EXCEEDED, SPECIAL_BUDGET, SPECIAL_DAY_BUDGET};
use super::*;
use crate::agent::AgentEvent;
use crate::models;
use chrono::Duration as CDuration;

/// The most runs one call of a role makes (a refused model, a fallback, a nudge, a restart).
pub(super) const MAX_RUN_TRIES: usize = 16;

/// What a run reported, folded.
#[derive(Default)]
pub(crate) struct Folded {
    pub windows: Option<Vec<LimitWindow>>,
    pub cost: Option<f64>,
    /// `Some(resets)` = the env said "limit".
    pub limit: Option<Option<DateTime<Local>>>,
    pub rejected: Option<String>,
    pub auth: Option<String>,
    /// session, text, is_error
    pub done: Option<(Option<String>, String, bool)>,
    /// The last context in use and window the run reported (story 305); None = it reported neither.
    pub context: Option<(Option<u64>, Option<u64>)>,
    /// G70: the tokens of the run (input, output), summed; None = the CLI said nothing of them.
    pub tokens: Option<(u64, u64)>,
}

pub(crate) fn fold(events: &[AgentEvent]) -> Folded {
    let mut f = Folded::default();
    for e in events {
        match e {
            AgentEvent::Usage(w) => f.windows = Some(w.clone()),
            AgentEvent::Cost(c) => f.cost = Some(f.cost.unwrap_or(0.0) + c),
            AgentEvent::Tokens { input, output } => {
                let t = f.tokens.get_or_insert((0, 0));
                t.0 += input;
                t.1 += output;
            }
            AgentEvent::Limit { resets_at } => f.limit = Some(*resets_at),
            AgentEvent::ModelRejected(m) => f.rejected = Some(m.clone()),
            AgentEvent::AuthRequired(m) => f.auth = Some(m.clone()),
            AgentEvent::Done { session, text, is_error } => f.done = Some((session.clone(), text.clone(), *is_error)),
            AgentEvent::Context { used, window } => {
                let c = f.context.get_or_insert((None, None));
                c.0 = used.or(c.0);
                c.1 = window.or(c.1);
            }
            _ => {}
        }
    }
    f
}

/// Why the call goes on with another run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Again {
    /// The CLI refused the model; the next of the chain (G18.4). Not an attempt.
    Rejected,
    /// The limit is used up and the env has another model (A05): same branch, fresh session.
    Fallback,
}

/// What comes after a run.
pub(super) enum Next {
    Done { text: String },
    /// The run failed (what the env said).
    Failed(String),
    /// Every model of the chain was refused.
    Refused(String),
    Auth(String),
    /// The limit is used up and there is nothing to go on with; when it resets, if known.
    Limit(Option<DateTime<Local>>),
    Again(Again),
}

/// The state of one call of a role that the decision needs and changes.
pub(super) struct RunCx<'a> {
    pub task_id: &'a str,
    pub env: &'a str,
    pub role: RoleKind,
    pub cat: &'a Catalog,
    pub fallback: Option<ModelChoice>,
    pub choice: Option<ModelChoice>,
    pub tried_fallback: bool,
}

/// What the Watcher does with a run that went silent.
#[derive(Debug, PartialEq)]
pub(super) enum Stall {
    /// First silence and a session to go on with: diagnose, then continue it.
    Nudge,
    /// Second silence, or no session: a new session, and it counts as an attempt.
    Restart,
}

pub(super) fn stall_decision(nudged: bool, session: &Option<String>) -> Stall {
    if !nudged && session.is_some() { Stall::Nudge } else { Stall::Restart }
}

impl Engine {
    /// Adds a cost to the day of the env (what a chat's run costs too).
    /// A chat's run in `project` (G70): its cost goes to the day of the env, and the run is the chat's own row of the
    /// history with its tokens. `usd` None = the env reports no money ("нет данных", not 0).
    pub fn add_chat_cost(&self, env: &str, project: &str, usd: Option<f64>, tokens: Option<(u64, u64)>) {
        if let Some(c) = usd {
            let _ = lk(&self.store).add_day_spend(env, self.now().date_naive(), c, 0.0);
        }
        self.record_spend(Some(project), env, true, usd, tokens);
    }

    /// One run's spending in the history (G70): `usd` None = the env reports no money; `tokens` (input, output).
    pub(crate) fn record_spend(&self, project: Option<&str>, env: &str, chat: bool, usd: Option<f64>, tokens: Option<(u64, u64)>) {
        let today = self.now().date_naive();
        let (tokens_in, tokens_out) = (tokens.map(|t| t.0), tokens.map(|t| t.1));
        let row = crate::store::SpendRow { day: today, month: false, project: project.map(String::from), env: env.to_string(), chat, usd, tokens_in, tokens_out };
        let _ = lk(&self.store).add_spend(row, today);
    }

    /// The spending history, by day, project and env (oldest first).
    pub fn spend_history(&self, from: Option<chrono::NaiveDate>, to: Option<chrono::NaiveDate>) -> Vec<crate::store::SpendRow> {
        lk(&self.store).spend_history(from, to)
    }

    /// When a limit the run hit resets: the time of the message, else a used-up window of the run, else one
    /// stored earlier (None = unknown, `LIMIT_WAIT_MINS` is waited).
    pub(crate) fn limit_reset(&self, env: &str, resets: Option<DateTime<Local>>, windows: Option<&Vec<LimitWindow>>) -> Option<DateTime<Local>> {
        resets
            .or_else(|| windows.and_then(|w| w.iter().filter(|x| x.exhausted).filter_map(|x| x.resets_at).max()))
            .or_else(|| self.limit_wait(env, self.now()))
    }

    /// Adds a cost to the task and to the day of its env: the one way a cost is written.
    pub(super) fn add_cost(&self, task_id: &str, env: &str, role: RoleKind, cost: f64, tokens: Option<(u64, u64)>) {
        let t = {
            let mut st = lk(&self.store);
            let Some(mut t) = st.task(task_id) else { return };
            t.cost_usd = Some(t.cost_usd.unwrap_or(0.0) + cost);
            let _ = st.upsert_task(&t);
            t
        };
        let _ = lk(&self.store).add_day_spend(env, self.now().date_naive(), cost, 0.0);
        self.record_spend(Some(&t.project_id), env, false, Some(cost), tokens);
        self.emit(EngineEvent::TaskChanged(t));
        self.log(task_id, LogKind::Stage, &format!("{COST_LOG}{cost:.4} ({role:?})"));
    }

    /// The decision after a run: records what the env reported (windows, cost), then says what is next.
    /// `label` is the model the run had, for the feed.
    pub(super) fn after_run(&self, cx: &mut RunCx, label: &str, f: &Folded) -> Next {
        if let Some(w) = &f.windows {
            self.record_usage(cx.env, w.clone());
        }
        match f.cost {
            Some(c) => self.add_cost(cx.task_id, cx.env, cx.role, c, f.tokens),
            // the run is in the history even when the env says nothing of money: "нет данных", not 0
            None => {
                if let Some(t) = self.task(cx.task_id) {
                    self.record_spend(Some(&t.project_id), cx.env, false, None, f.tokens);
                }
            }
        }
        if let Some(msg) = &f.auth {
            return Next::Auth(msg.clone());
        }
        if let Some(msg) = &f.rejected {
            let cur = cx.choice.clone().unwrap_or(ModelChoice { family: models::DEFAULT_FAMILY.into(), version: None });
            let Some(next) = models::next_on_reject(cx.cat, &cur) else { return Next::Refused(format!("model_rejected: {msg}")) };
            let to = pick(cx.cat, cx.env, Some(&next)).label;
            self.log(cx.task_id, LogKind::Stage, &format!("model_rejected: {label} -> {to}"));
            cx.choice = Some(next);
            return Next::Again(Again::Rejected);
        }
        if let Some(resets) = f.limit {
            // the time comes from the message, else from a used-up window of this run, else from one stored earlier
            let resets = self.limit_reset(cx.env, resets, f.windows.as_ref());
            self.record_limit_hit(cx.env, resets);
            if let Some(fb) = cx.fallback.clone().filter(|b| cx.choice.as_ref() != Some(b) && !cx.tried_fallback) {
                cx.tried_fallback = true;
                self.log(cx.task_id, LogKind::Stage, &format!("switched_to: {}", pick(cx.cat, cx.env, Some(&fb)).label));
                cx.choice = Some(fb);
                return Next::Again(Again::Fallback);
            }
            return Next::Limit(resets);
        }
        match &f.done {
            Some((_, text, false)) => Next::Done { text: text.clone() },
            Some((_, text, true)) => Next::Failed(text.clone()),
            None => Next::Failed("agent ended without a result".into()),
        }
    }

    /// The limit of the env is known to be used up and the env has another model: the run starts there.
    /// True = the choice was changed (a session of the old model cannot go on).
    pub(super) fn start_on_fallback(&self, task_id: &str, env: &str, cat: &Catalog, fallback: Option<&ModelChoice>, choice: &mut Option<ModelChoice>) -> bool {
        let (Some(fb), Some(_)) = (fallback, self.limit_wait(env, self.now())) else { return false };
        if choice.as_ref() == Some(fb) {
            return false;
        }
        self.log(task_id, LogKind::Stage, &format!("switched_to: {}", pick(cat, env, Some(fb)).label));
        *choice = Some(fb.clone());
        true
    }

    /// The budget of the task or of the day is spent: the key of the question and its text.
    pub(super) fn budget_breach(&self, t: &Task) -> Option<(&'static str, String)> {
        let spent = t.cost_usd.unwrap_or(0.0);
        if let Some(b) = t.budget_usd.filter(|b| spent >= *b) {
            return Some((SPECIAL_BUDGET, format!("{BUDGET_EXCEEDED}: {spent:.2} of {b:.2}")));
        }
        let settings = lk(&self.store).settings();
        let day = lk(&self.store).day_spend(&t.agent, self.now().date_naive());
        let cap = settings.limits.day_usd + day.extra_usd;
        (day.spent_usd >= cap).then(|| (SPECIAL_DAY_BUDGET, format!("{DAY_BUDGET_EXCEEDED}: {:.2} of {cap:.2}", day.spent_usd)))
    }

    /// The short diagnosis by the Watcher goes in the feed (the text is what its run said).
    pub(super) fn log_diagnosis(&self, task_id: &str, text: &str) {
        let short: String = text.trim().chars().take(300).collect();
        let mut ev = LogEvent::new(self.now(), LogKind::Stage, &format!("watcher: {short}"));
        ev.role = Some(RoleKind::Watcher);
        self.log_ev(task_id, ev);
    }

    /// What the Watcher is asked: the tail of the log of the task.
    pub(super) fn diagnosis_request(&self, task_id: &str) -> String {
        let tail: Vec<String> = lk(&self.store).read_log(task_id, 40).into_iter().map(|e| e.text).collect();
        format!(
            "A coding agent has printed nothing for a long time. In one or two sentences say what most likely blocks it, \
             judging by the end of its log. Do not change any file.\n\n{}",
            crate::prompt::fenced("LOG", &tail.join("\n"))
        )
    }

    /// Minutes of silence the next run may have: the first silence, or the second after a nudge.
    pub(super) fn silence_limit(&self, role: RoleKind, nudged: bool) -> Option<CDuration> {
        let stall = lk(&self.store).settings().stall_min;
        (role != RoleKind::Watcher && stall > 0).then(|| CDuration::minutes(if nudged { watch::RESTART_MIN } else { stall as i64 }))
    }
}
