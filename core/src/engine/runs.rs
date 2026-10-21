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
pub(super) struct Folded {
    pub windows: Option<Vec<LimitWindow>>,
    pub cost: Option<f64>,
    /// `Some(resets)` = the env said "limit".
    pub limit: Option<Option<DateTime<Local>>>,
    pub rejected: Option<String>,
    pub auth: Option<String>,
    /// session, text, is_error
    pub done: Option<(Option<String>, String, bool)>,
}

pub(super) fn fold(events: &[AgentEvent]) -> Folded {
    let mut f = Folded::default();
    for e in events {
        match e {
            AgentEvent::Usage(w) => f.windows = Some(w.clone()),
            AgentEvent::Cost(c) => f.cost = Some(f.cost.unwrap_or(0.0) + c),
            AgentEvent::Limit { resets_at } => f.limit = Some(*resets_at),
            AgentEvent::ModelRejected(m) => f.rejected = Some(m.clone()),
            AgentEvent::AuthRequired(m) => f.auth = Some(m.clone()),
            AgentEvent::Done { session, text, is_error } => f.done = Some((session.clone(), text.clone(), *is_error)),
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
    /// Adds a cost to the task and to the day of its env: the one way a cost is written.
    pub(super) fn add_cost(&self, task_id: &str, env: &str, role: RoleKind, cost: f64) {
        let t = {
            let mut st = lk(&self.store);
            let Some(mut t) = st.task(task_id) else { return };
            t.cost_usd = Some(t.cost_usd.unwrap_or(0.0) + cost);
            let _ = st.upsert_task(&t);
            let _ = st.add_day_spend(env, self.now().date_naive(), cost, 0.0);
            t
        };
        self.emit(EngineEvent::TaskChanged(t));
        self.log(task_id, LogKind::Stage, &format!("{COST_LOG}{cost:.4} ({role:?})"));
    }

    /// The decision after a run: records what the env reported (windows, cost), then says what is next.
    /// `label` is the model the run had, for the feed.
    pub(super) fn after_run(&self, cx: &mut RunCx, label: &str, f: &Folded) -> Next {
        if let Some(w) = &f.windows {
            self.record_usage(cx.env, w.clone());
        }
        if let Some(c) = f.cost {
            self.add_cost(cx.task_id, cx.env, cx.role, c);
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
            let resets = resets
                .or_else(|| f.windows.as_ref().and_then(|w| w.iter().filter(|x| x.exhausted).filter_map(|x| x.resets_at).max()))
                .or_else(|| self.limit_wait(cx.env, self.now()));
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
