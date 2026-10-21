//! Commands of the engine added in v2 (spec "Границы v2"): notes, the outward question with its
//! four answers, lifting a failed task, switching the model, freeze, hand-off to a terminal, the
//! night queue, the digest of what happened while the person was away.
use super::state::Note;
use super::*;
use crate::models;
use chrono::Duration as CDuration;
use std::process::Command;

/// First words of the line in a project's log saying that the v1 settings do nothing now (G22.1).
pub(super) const V1_SETTING_NOTE: &str = "v1_setting_ignored";

/// What `note` did with the message.
#[derive(Clone, Debug, PartialEq)]
pub enum NoteResult {
    /// Kept for the next run of its addressee.
    Queued,
    /// The task was waiting for an answer: the note is the answer.
    Answered,
    /// The task cannot take it; the chat handles the message as an ordinary one. The reason is a key
    /// (`task_already_merged`, `task_cancelled`).
    NotDelivered(String),
}

/// What the app needs to open the person's terminal on a task (A07).
#[derive(Debug)]
pub struct HandOff {
    pub task_id: TaskId,
    pub env: EnvId,
    /// The task's copy: the terminal opens here.
    pub worktree: PathBuf,
    pub session: Option<String>,
    /// The env's command to continue the session, arguments as a list. None = it cannot, or the task has no session.
    pub command: Option<Command>,
    /// False when `command` is None: the app opens a new session of the env's CLI in `worktree`
    /// and the feed says "the session cannot be continued - opened a new one".
    pub resumed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SummaryItem {
    pub task_id: TaskId,
    pub project: String,
    pub env: EnvId,
    pub title: String,
    /// The one-line result, the reason, or the question.
    pub text: String,
    pub commit: Option<String>,
}

/// "While you were away" (A06): built from the logs and the tasks on disk, not from the window's memory.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub since: DateTime<Local>,
    pub done: Vec<SummaryItem>,
    pub failed: Vec<SummaryItem>,
    pub waiting: Vec<SummaryItem>,
    pub spent_usd: f64,
    pub limits: Vec<(EnvId, EnvLimits)>,
}

impl Summary {
    /// Nothing happened: no digest is shown.
    pub fn is_empty(&self) -> bool {
        self.done.is_empty() && self.failed.is_empty() && self.waiting.is_empty() && self.spent_usd == 0.0
    }
}

/// A card the person put off ("Отложить"): it is not among those waiting for them until `resume_at`.
/// A login wait uses `resume_at` as its retry time, that is not a snooze.
pub fn is_snoozed(t: &Task, now: DateTime<Local>) -> bool {
    let call = matches!(t.status, Status::NeedsHuman | Status::Failed | Status::AwaitingOutward);
    let auth = t.question.as_ref().is_some_and(|q| q.text.starts_with(roles::AUTH_REQUIRED));
    call && !auth && t.resume_at.is_some_and(|r| r > now)
}

fn role_of(name: &str) -> Option<RoleKind> {
    serde_json::from_value(serde_json::Value::String(name.trim().to_ascii_lowercase())).ok()
}

impl Engine {
    // ---- notes and answers ----

    /// A message of the person to the agents (G05). While the task waits for an answer it is the
    /// answer; otherwise it waits for the next run of `to` (the executor by default) and the feed
    /// notes when it was delivered. A task that has landed cannot take it.
    pub fn note(&self, id: &str, to: Option<String>, text: &str) -> Result<NoteResult, String> {
        let t = self.task(id).ok_or("task_not_found")?;
        let to = match to.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(n) => Some(role_of(n).ok_or_else(|| format!("unknown_role: {n}"))?),
            None => None,
        };
        match t.status {
            Status::Done | Status::DoneNotPushed | Status::Reverted => return Ok(NoteResult::NotDelivered("task_already_merged".into())),
            Status::Cancelled => return Ok(NoteResult::NotDelivered("task_cancelled".into())),
            _ => {}
        }
        let text = text.trim();
        if text.is_empty() {
            return Err("empty_note".into());
        }
        let asks = t.status == Status::NeedsHuman
            && t.question.as_ref().is_some_and(|q| q.kind == QuestionKind::Agent)
            && self.state(id).special.is_none();
        if asks {
            return self.answer(id, text).map(|_| NoteResult::Answered);
        }
        let note = Note { id: uuid::Uuid::new_v4().to_string(), to, text: text.to_string(), at: self.now(), delivered: false };
        self.update_state(id, |s| s.notes.push(note))?;
        self.log(id, LogKind::Human, &format!("note: {text}"));
        Ok(NoteResult::Queued)
    }

    /// Notes for the run of `role` that starts now: marks them delivered (and says so in the feed).
    pub(super) fn take_notes(&self, id: &str, role: RoleKind) -> Vec<String> {
        let writes = matches!(role, RoleKind::Worker | RoleKind::TestAuthor | RoleKind::Integrator);
        let mut texts = vec![];
        let _ = self.update_state(id, |s| {
            for n in s.notes.iter_mut().filter(|n| !n.delivered) {
                if n.to == Some(role) || (n.to.is_none() && writes) {
                    n.delivered = true;
                    texts.push(n.text.clone());
                }
            }
        });
        for t in &texts {
            let mut ev = LogEvent::new(self.now(), LogKind::Human, &format!("note delivered: {t}"));
            ev.role = Some(role);
            self.log_ev(id, ev);
        }
        texts
    }

    /// The person's answer to "push / PR?" (G01.8). PR project: push and PR, only push, no.
    /// Push project: push, no. `Push` in a PR project means push and PR.
    pub fn answer_outward(&self, id: &str, choice: OutwardChoice) -> Result<(), String> {
        let _h = self.hold(id, false)?;
        let mut t = self.task(id).ok_or("task_not_found")?;
        if t.status != Status::AwaitingOutward {
            return Err("not_waiting".into()); // e.g. the second of two clicks
        }
        let p = self.project(&t.project_id).ok_or("project_not_found")?;
        let revert = t.question.as_ref().is_some_and(|q| q.text == flow::PUSH_REVERT);
        let yes = choice != OutwardChoice::No;
        if revert && !matches!(choice, OutwardChoice::Push | OutwardChoice::No) {
            return Err("invalid_choice".into());
        }
        if !revert {
            let ok = match p.finish {
                Finish::Pr => true,
                Finish::Push => matches!(choice, OutwardChoice::Push | OutwardChoice::No),
                Finish::Local => false,
            };
            if !ok {
                return Err("invalid_choice".into());
            }
        }
        t.question = None;
        self.log(id, LogKind::Human, if yes { "approved: push" } else { "declined: push" });
        match (revert, choice) {
            (true, OutwardChoice::No) => {
                t.status = Status::Reverted;
                t.outcome.error = Some("push_skipped".into());
                self.save(&t)?;
            }
            (true, _) => {
                t.status = Status::Reverted;
                t.outcome.error = git::push(&p.path, &p.base_branch).err().map(|e| e.to_string());
                self.save(&t)?;
                self.emit(EngineEvent::ProjectChanged(p.id.clone()));
            }
            (false, OutwardChoice::No) => {
                t.outcome.error = Some("push_skipped".into());
                self.complete(&mut t, &p, Status::DoneNotPushed);
            }
            (false, OutwardChoice::PushOnly) => {
                // the branch goes to origin, no PR is opened: done, with a note
                match git::push(&p.path, &t.branch) {
                    Ok(()) => {
                        t.outcome.error = Some(flow::PR_NOT_OPENED.into());
                        self.log(id, LogKind::Git, &format!("pushed {}", t.branch));
                        self.complete(&mut t, &p, Status::Done);
                        self.emit(EngineEvent::ProjectChanged(p.id.clone()));
                    }
                    Err(e) => {
                        t.outcome.error = Some(e.to_string());
                        self.log(id, LogKind::Error, &e.to_string());
                        self.complete(&mut t, &p, Status::DoneNotPushed);
                    }
                }
            }
            (false, _) => self.outward(&mut t, &p),
        }
        Ok(())
    }

    /// `answer_outward` with the two old buttons.
    pub fn approve_outward(&self, id: &str, yes: bool) -> Result<(), String> {
        self.answer_outward(id, if yes { OutwardChoice::Push } else { OutwardChoice::No })
    }

    /// An answer to a question the engine asked itself: more budget / stop, or "I logged in".
    pub(super) fn answer_special(&self, t: &Task, key: &str, text: &str, stage: Status) -> Result<(), String> {
        let id = t.id.as_str();
        let stop = text.trim().to_ascii_lowercase().starts_with("stop") && key != roles::SPECIAL_AUTH;
        self.log(id, LogKind::Human, text);
        if stop {
            self.update_state(id, |s| s.special = None)?;
            return self.do_cancel(id);
        }
        match key {
            roles::SPECIAL_BUDGET => {
                let add = roles::BUDGET_STEP_USD;
                self.cas(id, "not_waiting", |x| x.status == Status::NeedsHuman, |x| x.budget_usd = Some(x.budget_usd.unwrap_or(0.0) + add))?;
            }
            roles::SPECIAL_DAY_BUDGET => {
                lk(&self.store).add_day_spend(&t.agent, self.now().date_naive(), 0.0, roles::BUDGET_STEP_USD).map_err(|e| e.to_string())?;
            }
            _ => {} // login: just try again
        }
        // a login wait ends only with a run that works (or cancel / remove): "Login" may come before the login itself
        if key != roles::SPECIAL_AUTH {
            self.update_state(id, |s| s.special = None)?;
        }
        self.cas(id, "not_waiting", |x| x.status == Status::NeedsHuman, |x| {
            x.question = None;
            x.status = stage;
            x.resume_at = None;
        })?;
        Ok(())
    }

    // ---- lifting a failed task, switching the model ----

    /// "Не вышло" -> the same copy, with the person's hint and / or a stronger model; attempts and
    /// review rounds start again (G15.2).
    pub fn resume_failed(&self, id: &str, hint: Option<String>, model: Option<ModelChoice>) -> Result<(), String> {
        let _h = self.hold(id, false)?;
        let mut t = self.task(id).ok_or("task_not_found")?;
        if t.status != Status::Failed {
            return Err("not_failed".into());
        }
        if t.worktree.as_ref().is_none_or(|w| !w.exists()) {
            return Err("no_worktree".into());
        }
        let hint = hint.map(|h| h.trim().to_string()).filter(|h| !h.is_empty());
        let switched = model.is_some();
        if let Some(m) = model {
            t.models.insert(RoleKind::Worker, m); // a new model starts a new session
            t.session = None;
        }
        let at = self.now();
        if let Some(h) = &hint {
            self.update_state(id, |s| s.notes.push(Note { id: uuid::Uuid::new_v4().to_string(), to: None, text: h.clone(), at, delivered: false }))?;
        }
        self.update_state(id, |s| {
            if switched {
                s.pinned.remove(&RoleKind::Worker);
                s.continue_next = true;
            }
        })?;
        self.set_pending(id, &Pending { stage: Some(Status::Working), ..Default::default() })?;
        t.attempts = 0;
        t.review_rounds = 0;
        t.question = None;
        t.resume_at = None;
        t.outcome.error = None;
        t.finished_at = None;
        for (s, st) in t.steps.iter_mut() {
            if matches!(s, Step::Code | Step::Gates | Step::E2e | Step::Review | Step::Merge) && !matches!(st, StepState::Skipped(_)) {
                *st = StepState::Pending;
            }
        }
        t.status = Status::Working;
        self.save(&t)?;
        self.log(id, LogKind::Human, &format!("resume_failed: {}", hint.as_deref().unwrap_or("no hint")));
        Ok(())
    }

    /// Restarts the current role on another model of the same env (the run in progress is stopped).
    pub fn switch_model(&self, id: &str, choice: ModelChoice) -> Result<(), String> {
        // the role is read before the claim: stopping the run marks it "interrupted"
        let running = |t: &Task| t.runs.iter().rev().find(|r| matches!(r.state, StepState::Running)).map(|r| r.role);
        let before = self.task(id).and_then(|t| running(&t));
        let _h = self.hold(id, true)?;
        let mut t = self.task(id).ok_or("task_not_found")?;
        if !matches!(t.status, Status::Working | Status::Checking | Status::Reviewing | Status::NeedsHuman | Status::PausedLimit) {
            return Err("not_switchable".into());
        }
        // a role's question: the role that asked (its run is saved as Done, so it is not the last open one)
        let asked = self.state(id).asked.filter(|_| t.status == Status::NeedsHuman);
        // nothing was running (a paused limit, a login, the budget): the role of the last unfinished run
        let last_open = t.runs.last().filter(|r| !matches!(r.state, StepState::Done)).map(|r| r.role);
        let role = running(&t).or(before).or(asked).or(last_open).unwrap_or(RoleKind::Worker);
        t.models.insert(role, choice.clone());
        if role == RoleKind::Worker {
            t.session = None;
        }
        self.update_state(id, |s| {
            s.pinned.remove(&role);
            if s.asked == Some(role) {
                s.asked_session = None; // another model is another session: the answer goes to a fresh run
            }
            if role == RoleKind::Worker {
                s.continue_next = true;
            }
        })?;
        for r in t.runs.iter_mut().filter(|r| matches!(r.state, StepState::Running)) {
            r.state = StepState::Failed("switched".into());
        }
        self.save(&t)?;
        self.log(id, LogKind::Human, &format!("switch_model: {role:?} -> {}", choice.family));
        Ok(())
    }

    /// "Отложить": the card leaves the person's list until `until`; the copy is kept (G15.2).
    pub fn snooze(&self, id: &str, until: DateTime<Local>) -> Result<(), String> {
        let _h = self.hold(id, false)?;
        self.cas(id, "not_snoozable", |t| matches!(t.status, Status::NeedsHuman | Status::Failed | Status::AwaitingOutward), |t| {
            t.resume_at = Some(until);
        })?;
        self.log(id, LogKind::Human, &format!("snoozed until {}", until.format("%Y-%m-%d %H:%M")));
        Ok(())
    }

    // ---- freeze, hand-off ----

    /// Freeze (R18i): agent processes stop at once, statuses and sessions stay, nothing new starts.
    /// The git part of a merge that began goes to its end, its Integrator stops; hand-offs are not touched.
    pub fn freeze(&self, on: bool) {
        let was = self.frozen.swap(on, Ordering::SeqCst);
        if on && !was {
            let integrating = |t: &Task| t.runs.last().is_some_and(|r| r.role == RoleKind::Integrator && matches!(r.state, StepState::Running));
            let ids: Vec<(TaskId, Arc<AtomicBool>)> = lk(&self.running).tasks.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            for (id, flag) in ids {
                if self.task(&id).is_some_and(|t| t.status != Status::Merging || integrating(&t)) {
                    flag.store(true, Ordering::SeqCst);
                }
            }
        }
        if was != on {
            self.emit(EngineEvent::Frozen(on));
        }
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen.load(Ordering::SeqCst)
    }

    /// The person takes the task to a terminal (A07): its run stops, the engine leaves it alone
    /// (also after a restart) until `take_back`.
    pub fn hand_off(&self, id: &str) -> Result<HandOff, String> {
        let _h = self.hold(id, true)?;
        let mut t = self.task(id).ok_or("task_not_found")?;
        if !(is_active(&t.status) || matches!(t.status, Status::NeedsHuman | Status::Failed | Status::PausedLimit | Status::MergeBlocked)) {
            return Err("not_handoffable".into());
        }
        let wt = t.worktree.clone().filter(|w| w.exists()).ok_or("no_worktree")?;
        let a = self.adapter(&t.agent).ok_or_else(|| format!("agent_not_found: {}", t.agent))?;
        let command = t.session.as_deref().and_then(|s| a.interactive_resume(s, &wt));
        for r in t.runs.iter_mut().filter(|r| matches!(r.state, StepState::Running)) {
            r.state = StepState::Failed("handed_off".into());
        }
        t.status = Status::HandedOff;
        t.question = None;
        t.resume_at = None;
        self.save(&t)?;
        let resumed = command.is_some();
        if resumed {
            self.log(id, LogKind::Human, "handed off to the terminal");
        } else {
            self.log(id, LogKind::Human, "handed off to the terminal: session cannot be continued, a new one is opened");
        }
        Ok(HandOff { task_id: t.id.clone(), env: t.agent.clone(), worktree: wt, session: t.session.clone(), command, resumed })
    }

    /// "Вернуть агентам": the person's edits are committed and the task goes through the gates and the review (A07.3).
    pub fn take_back(&self, id: &str) -> Result<(), String> {
        let _h = self.hold(id, false)?;
        let mut t = self.task(id).ok_or("task_not_found")?;
        if t.status != Status::HandedOff {
            return Err("not_handed_off".into());
        }
        let wt = t.worktree.clone().filter(|w| w.exists()).ok_or("no_worktree")?;
        if !git::rebase_in_progress(&wt) {
            git::commit_all(&wt, &t.title).map_err(|e| e.to_string())?;
        }
        for (s, st) in t.steps.iter_mut() {
            if matches!(s, Step::Gates | Step::E2e | Step::Review | Step::Merge) && !matches!(st, StepState::Skipped(_)) {
                *st = StepState::Pending;
            }
            if *s == Step::Code {
                *st = StepState::Done;
            }
        }
        self.clear_pending(id);
        t.status = Status::Checking;
        self.save(&t)?;
        self.log(id, LogKind::Human, "taken back from the terminal");
        Ok(())
    }

    // ---- the night queue ----

    /// The app's presence detector: `away` = nobody at the computer (idle for `away_min` minutes or the
    /// screen is locked). Night tasks may start while it is true.
    pub fn set_presence(&self, away: bool) {
        self.away.store(away, Ordering::SeqCst);
    }

    /// A night or deferred task starts now (A08.3).
    pub fn start_now(&self, id: &str) -> Result<(), String> {
        let _h = self.hold(id, false)?;
        self.cas(id, "not_waiting_to_start", |t| t.status == Status::Queued && t.start != StartWhen::Now, |t| {
            t.start = StartWhen::Now;
            t.resume_at = None;
        })?;
        self.log(id, LogKind::Human, "start now");
        Ok(())
    }

    /// "Это был вопрос": cancels the task while it has not reached the "Code" step and gives back
    /// the message text for the chat (G20.3).
    pub fn not_a_task(&self, id: &str) -> Result<String, String> {
        let _h = self.hold(id, true)?;
        let t = self.task(id).ok_or("task_not_found")?;
        let coding = t.runs.iter().any(|r| r.role == RoleKind::Worker) || t.steps.iter().any(|(s, st)| *s == Step::Code && !matches!(st, StepState::Pending));
        if coding || !matches!(t.status, Status::Queued | Status::Preparing | Status::Working | Status::NeedsHuman) {
            return Err("already_coding".into());
        }
        let text = t.prompt.clone();
        self.do_cancel(id)?;
        Ok(text)
    }

    // ---- what the env reports ----

    /// The windows the env last reported, or why there are none. No estimates (spec 22).
    pub fn limits(&self, env: &str) -> EnvLimits {
        let stored = lk(&self.store).limits(env);
        match stored {
            Some(EnvLimits::Known(w)) if !w.is_empty() => EnvLimits::Known(w),
            Some(EnvLimits::NoData(why)) => EnvLimits::NoData(why),
            _ => {
                let ran = lk(&self.store).tasks().iter().any(|t| t.agent == env && !t.runs.is_empty());
                EnvLimits::NoData(if ran { "env_does_not_report" } else { "after_first_run" }.into())
            }
        }
    }

    /// The model catalog of the env: runs its list command (outside the store's lock), keeps the
    /// result as the snapshot, and says so.
    pub fn refresh_catalog(&self, env: &str) -> Result<Catalog, String> {
        let a = self.adapter(env).ok_or_else(|| format!("agent_not_found: {env}"))?;
        let listed = a.models(); // may take seconds: no lock
        let cat = {
            let st = lk(&self.store);
            if let Ok(f) = listed.as_ref().map(|f| f.as_slice()) {
                if !f.is_empty() {
                    let _ = st.set_catalog(Catalog { env: env.to_string(), families: f.to_vec(), source: CatalogSource::CliList, at: self.now() });
                }
            }
            models::catalog(env, &*a, &st)
        };
        self.emit(EngineEvent::CatalogChanged(env.to_string()));
        Ok(cat)
    }

    /// "Пока тебя не было": what finished, failed, waits for the person, and what it cost since `at`.
    pub fn summary_since(&self, at: DateTime<Local>) -> Summary {
        let now = self.now();
        let (tasks, projects, settings) = {
            let st = lk(&self.store);
            (st.tasks(), st.projects(), st.settings())
        };
        let mut s = Summary { since: at, done: vec![], failed: vec![], waiting: vec![], spent_usd: 0.0, limits: vec![] };
        let mut envs: Vec<EnvId> = settings.envs.keys().cloned().collect();
        for t in &tasks {
            let log = lk(&self.store).read_log(&t.id, 20_000);
            let mut touched = t.finished_at.is_some_and(|f| f >= at);
            for e in log.iter().filter(|e| e.at >= at) {
                touched = true;
                if let Some(c) = e.text.strip_prefix(roles::COST_LOG).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse::<f64>().ok()) {
                    s.spent_usd += c;
                }
            }
            if !envs.contains(&t.agent) {
                envs.push(t.agent.clone());
            }
            let project = projects.iter().find(|p| p.id == t.project_id).map(|p| p.name.clone()).unwrap_or_default();
            let item = |text: String| SummaryItem {
                task_id: t.id.clone(),
                project: project.clone(),
                env: t.agent.clone(),
                title: t.title.clone(),
                text,
                commit: t.outcome.commit.clone(),
            };
            match t.status {
                Status::Done | Status::DoneNotPushed if t.finished_at.is_some_and(|f| f >= at) => s.done.push(item(flow::summary_line(t))),
                Status::Failed if touched && !is_snoozed(t, now) => s.failed.push(item(t.outcome.error.clone().unwrap_or_default())),
                Status::NeedsHuman | Status::AwaitingOutward if !is_snoozed(t, now) => {
                    s.waiting.push(item(t.question.as_ref().map(|q| q.text.clone()).unwrap_or_default()))
                }
                _ => {}
            }
        }
        for env in envs {
            if let Some(l) = lk(&self.store).limits(&env) {
                s.limits.push((env, l));
            }
        }
        s
    }

    // ---- when a queued task may start ----

    /// Everything that keeps a queued task from starting besides a free seat. A reason is written on the
    /// card (`outcome.error`, `resume_at`) so the person sees what it waits for (G26.4).
    pub(super) fn may_start(&self, t: &Task, tasks: &[Task], settings: &Settings, now: DateTime<Local>) -> bool {
        let env = &t.agent;
        if lk(&self.auth).contains_key(env) {
            // known by its state, not its status: a retry flips the status to Working for the run (G17.2)
            let asks_login = |o: &Task| {
                o.agent == *env
                    && (is_active(&o.status) || matches!(o.status, Status::NeedsHuman | Status::PausedLimit | Status::MergeBlocked))
                    && self.state(&o.id).special.as_deref() == Some(roles::SPECIAL_AUTH)
            };
            if tasks.iter().any(asks_login) {
                return false; // a login is asked for: nothing of this env starts until it works
            }
            // the tasks that asked were cancelled, removed or handed off: the queue goes on
            lk(&self.auth).remove(env);
        }
        // a task that already waits for its env asks again at its own pace (once a minute), not from the cache
        let retrying = t.outcome.error.as_deref().is_some_and(|e| e.starts_with(roles::ENV_UNAVAILABLE));
        if let Err(why) = self.env_available(env, retrying) {
            self.park(t, &format!("{}: {why}", roles::ENV_UNAVAILABLE), now + CDuration::seconds(ENV_RETRY_SECS));
            return false;
        }
        match &t.start {
            StartWhen::Now => {}
            StartWhen::Night => {
                if !(util::in_window(now, &settings.limits.night_from, &settings.limits.night_to) || self.away.load(Ordering::SeqCst)) {
                    return false;
                }
            }
            StartWhen::AfterMerge { .. } => {
                let busy = tasks.iter().any(|o| {
                    o.id != t.id && o.project_id == t.project_id && (is_active(&o.status) || (o.status == Status::Queued && o.start == StartWhen::Now))
                });
                if busy {
                    return false;
                }
            }
        }
        if let Some(until) = self.limit_wait(env, now) {
            let has_fallback = settings.envs.get(env).is_some_and(|e| e.fallback.is_some());
            if !has_fallback {
                self.park(t, roles::WAITING_LIMIT, until);
                return false;
            }
        }
        true
    }

    /// A task put in the queue while the env's limit is used up says at once what it waits for.
    pub(super) fn hold_back_if_limited(&self, t: &Task) {
        let now = self.now();
        let has_fallback = lk(&self.store).settings().envs.get(&t.agent).is_some_and(|e| e.fallback.is_some());
        if let (Some(until), false) = (self.limit_wait(&t.agent, now), has_fallback) {
            self.park(t, roles::WAITING_LIMIT, until);
        }
    }

    /// Writes "waiting for `reason` until `until`" on a queued task (once per reason).
    fn park(&self, t: &Task, reason: &str, until: DateTime<Local>) {
        let key = reason.split(':').next().unwrap_or(reason).to_string();
        let fresh = self.cas(&t.id, "changed", |x| x.status == Status::Queued, |x| {
            if !x.outcome.error.as_deref().is_some_and(|e| e.starts_with(&key)) {
                x.outcome.error = Some(reason.to_string());
            }
            x.resume_at = Some(until);
        });
        if let Ok(x) = fresh {
            if x.outcome.error.as_deref() == Some(reason) && t.outcome.error.as_deref() != Some(reason) {
                self.log(&t.id, LogKind::Stage, &format!("{reason} (until {})", until.format("%H:%M")));
            }
        }
    }

    /// When the env's used-up limit resets, if it is used up now.
    pub(super) fn limit_wait(&self, env: &str, now: DateTime<Local>) -> Option<DateTime<Local>> {
        let Some(EnvLimits::Known(ws)) = lk(&self.store).limits(env) else { return None };
        ws.iter()
            .filter(|w| w.exhausted)
            .map(|w| w.resets_at.unwrap_or(w.seen_at + CDuration::minutes(roles::LIMIT_WAIT_MINS)))
            .filter(|u| *u > now)
            .max()
    }

    /// `Adapter::available`, asked at most every few seconds per env.
    pub(super) fn env_available(&self, env: &str, fresh: bool) -> Result<(), String> {
        const TTL: Duration = Duration::from_secs(2);
        if let Some((at, r)) = lk(&self.avail).get(env).filter(|_| !fresh) {
            if at.elapsed() < TTL {
                return r.clone();
            }
        }
        let r = match self.adapter(env) {
            Some(a) => a.available(),
            None => Err(format!("agent_not_found: {env}")),
        };
        lk(&self.avail).insert(env.to_string(), (Instant::now(), r.clone()));
        r
    }
}
