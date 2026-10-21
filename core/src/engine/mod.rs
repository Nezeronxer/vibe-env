//! Task lifecycle engine: queue with seats per env, the task state machine, the roles of the
//! pipeline, retries, limits, one merge at a time per project. Everything runs on plain threads.
//!
//! Ownership rule: a task's state is written only by whoever holds its claim in `running`
//! (the drive thread, or cancel / retry / undo / approve / answer while they work), checked
//! against the status inside the store's critical section. `push_base` takes each claim too.
//!
//! One env per task (G22): every run of every role takes its adapter from `Task.agent` and nowhere else.
//! The v1 project fields `reviewer` and `fallback_agent` are not read; seeing them is only noted in the log.
mod flow;
mod ops;
mod roles;
mod state;
mod steps;
mod util;

pub use ops::{is_snoozed, HandOff, NoteResult, Summary, SummaryItem};

use crate::agent::Adapter;
use crate::git;
use crate::model::*;
use crate::notify::{Notice, NoticeKind, Notifier};
use crate::store::Store;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Wall clock, replaceable in tests (limit pauses, the once-a-minute merge retry, the night window).
pub type Clock = Arc<dyn Fn() -> DateTime<Local> + Send + Sync>;

/// Variables for the runs of an env (kit secrets): env id -> pairs. Bound to `Kits::env_vars` by the app.
pub type EnvVars = Arc<dyn Fn(&str) -> Vec<(String, String)> + Send + Sync>;

#[derive(Clone, Debug)]
pub enum EngineEvent {
    TaskChanged(Task),
    Log(TaskId, LogEvent),
    ProjectChanged(ProjectId),
    /// New limit windows of the env were stored.
    LimitsChanged(EnvId),
    CatalogChanged(EnvId),
    /// `freeze` was switched.
    Frozen(bool),
}

/// A task to put in the queue. Everything but the project and the words is optional.
#[derive(Clone, Debug, Default)]
pub struct NewTask {
    pub project_id: ProjectId,
    pub prompt: String,
    /// The env; the project's by default.
    pub agent: Option<String>,
    pub chat_id: Option<ChatId>,
    /// "Quality" for this task; the settings' by default.
    pub quality: Option<QualityPlan>,
    pub swarm: Option<SwarmSpec>,
    pub start: Option<StartWhen>,
    /// The executor's model; the env's default by default.
    pub model: Option<ModelChoice>,
}

/// The longest task text `submit` takes (R04.1), in characters.
pub const MAX_PROMPT_CHARS: usize = 100_000;

const TICK: Duration = Duration::from_millis(40);
const WAIT_LIMIT: Duration = Duration::from_secs(15);
/// How often a task waiting for a login or for its env to come back tries again.
const ENV_RETRY_SECS: i64 = 60;

/// Stages that are worked on right now.
pub fn is_active(s: &Status) -> bool {
    matches!(s, Status::Preparing | Status::Working | Status::Checking | Status::Reviewing | Status::Merging)
}

fn cancellable(s: &Status) -> bool {
    is_active(s) || matches!(s, Status::Queued | Status::NeedsHuman | Status::PausedLimit | Status::MergeBlocked)
}

/// What the worker gets next and where a limit pause resumes. Kept on disk next to the
/// worktrees, so an answer or a fix request survives a restart.
#[derive(Serialize, Deserialize, Default, Clone)]
struct Pending {
    stage: Option<Status>,
    prompt: Option<String>,
    /// Agent runs in a row that ended in an error (separate from the check rounds in `attempts`).
    #[serde(default)]
    agent_errors: u32,
}

#[derive(Default)]
struct Running {
    stopping: bool,
    /// task id -> cancel flag of whoever holds the claim
    tasks: HashMap<TaskId, Arc<AtomicBool>>,
    /// Tasks somebody waits to take the claim of (`hold`): the scheduler leaves them alone, else it would
    /// restart a task at the very moment a command stopped it.
    wanted: HashMap<TaskId, usize>,
}

/// A held claim; released on drop.
struct Held<'a> {
    e: &'a Engine,
    id: String,
    /// Taken by `hold`: also ends the wish to take it.
    wished: bool,
}
impl Drop for Held<'_> {
    fn drop(&mut self) {
        let mut r = lk(&self.e.running);
        r.tasks.remove(&self.id);
        if self.wished {
            self.e.unwish(&mut r, &self.id);
        }
    }
}

/// A panic elsewhere must not take the whole engine down with poisoned locks.
fn lk<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// An env whose login was asked for: one call per env, then silence until it works again.
struct AuthWait {
    notified: bool,
}

pub struct Engine {
    store: Mutex<Store>,
    adapters: Mutex<Vec<Arc<dyn Adapter>>>,
    notifier: Arc<dyn Notifier>,
    worktrees_dir: PathBuf,
    clock: Clock,
    subs: Mutex<Vec<Sender<EngineEvent>>>,
    running: Mutex<Running>,
    merge_locks: Mutex<HashMap<ProjectId, Arc<Mutex<()>>>>,
    scheduler: Mutex<Option<JoinHandle<()>>>,
    /// `freeze`: no new runs, running ones are stopped, states stay.
    frozen: AtomicBool,
    /// The person is away (the app's idle / lock detection): night tasks may start.
    away: AtomicBool,
    state_lock: Mutex<()>,
    env_vars: Mutex<Option<EnvVars>>,
    auth: Mutex<HashMap<EnvId, AuthWait>>,
    /// env -> (when checked, answer of `Adapter::available`)
    avail: Mutex<HashMap<EnvId, (Instant, Result<(), String>)>>,
}

impl Engine {
    pub fn new(store: Store, adapters: Vec<Arc<dyn Adapter>>, notifier: Arc<dyn Notifier>, worktrees_dir: PathBuf) -> Arc<Engine> {
        Engine::new_with_clock(store, adapters, notifier, worktrees_dir, Arc::new(Local::now))
    }

    /// Same as `new` with an injected clock.
    pub fn new_with_clock(
        store: Store,
        adapters: Vec<Arc<dyn Adapter>>,
        notifier: Arc<dyn Notifier>,
        worktrees_dir: PathBuf,
        clock: Clock,
    ) -> Arc<Engine> {
        for a in &adapters {
            a.set_private_dir(&worktrees_dir);
        }
        Arc::new(Engine {
            store: Mutex::new(store),
            adapters: Mutex::new(adapters),
            notifier,
            worktrees_dir,
            clock,
            subs: Mutex::new(vec![]),
            running: Mutex::new(Running::default()),
            merge_locks: Mutex::new(HashMap::new()),
            scheduler: Mutex::new(None),
            frozen: AtomicBool::new(false),
            away: AtomicBool::new(false),
            state_lock: Mutex::new(()),
            env_vars: Mutex::new(None),
            auth: Mutex::new(HashMap::new()),
            avail: Mutex::new(HashMap::new()),
        })
    }

    /// Replaces the adapters (the person changed their own commands in the settings). Runs that are
    /// going keep the adapter they started with; every run that starts afterwards sees the new list.
    pub fn set_adapters(&self, adapters: Vec<Arc<dyn Adapter>>) {
        for a in &adapters {
            a.set_private_dir(&self.worktrees_dir);
        }
        *lk(&self.adapters) = adapters;
        lk(&self.avail).clear();
    }

    /// The provider of `Run.extra_env` per env (task 13 binds it to `Kits::env_vars`).
    pub fn set_env_vars(&self, f: EnvVars) {
        *lk(&self.env_vars) = Some(f);
    }

    /// Starts the scheduler. Tasks left in an active stage by a previous run are picked up again:
    /// the worker resumes its session, checks / review / merge simply run again.
    pub fn start(self: &Arc<Self>) {
        let mut s = lk(&self.scheduler);
        if s.is_some() || lk(&self.running).stopping {
            return;
        }
        self.sweep_ro_copies();
        let me = self.clone();
        *s = Some(std::thread::spawn(move || {
            while !lk(&me.running).stopping {
                me.tick();
                std::thread::sleep(TICK);
            }
        }));
    }

    /// Stops the scheduler and every agent / check process, leaving task states on disk as they
    /// are, so the next `start()` continues them (app quit). Nothing starts afterwards.
    /// Extra over interfaces.md.
    pub fn shutdown(&self) {
        {
            let mut r = lk(&self.running);
            r.stopping = true;
            for f in r.tasks.values() {
                f.store(true, Ordering::SeqCst);
            }
        }
        if let Some(h) = lk(&self.scheduler).take() {
            let _ = h.join();
        }
        let t0 = Instant::now();
        while !lk(&self.running).tasks.is_empty() && t0.elapsed() < WAIT_LIMIT {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn subscribe(&self) -> Receiver<EngineEvent> {
        let (tx, rx) = channel();
        lk(&self.subs).push(tx);
        rx
    }

    pub fn store(&self) -> &Mutex<Store> {
        &self.store
    }

    pub fn submit(&self, t: NewTask) -> Result<TaskId, String> {
        let prompt = t.prompt.trim().to_string();
        if prompt.is_empty() {
            return Err("empty_prompt".into());
        }
        if prompt.chars().count() > MAX_PROMPT_CHARS {
            return Err("prompt_too_long".into());
        }
        let p = self.project(&t.project_id).ok_or("project_not_found")?;
        let agent = t.agent.clone().unwrap_or_else(|| p.agent.clone());
        if self.adapter(&agent).is_none() {
            return Err(format!("agent_not_found: {agent}"));
        }
        let settings = lk(&self.store).settings();
        let env = settings.envs.get(&agent).cloned().unwrap_or_default();
        let id = uuid::Uuid::new_v4().to_string();
        let title = util::title_of(&prompt);
        // the models of the roles are a snapshot of the env's settings now (spec 16)
        let mut models = env.roles.clone();
        if let Some(m) = t.model.clone().or(env.default_model.clone()) {
            models.insert(RoleKind::Worker, m);
        }
        let quality = t.quality.clone().unwrap_or_else(|| settings.quality.clone());
        let mut task = Task {
            branch: format!("ns/{}-{}", &id[..8], util::slug(&title)),
            id: id.clone(),
            project_id: p.id.clone(),
            title,
            prompt,
            agent,
            status: Status::Queued,
            created_at: self.now(),
            chat_id: t.chat_id,
            models,
            swarm: t.swarm,
            start: t.start.unwrap_or(StartWhen::Now),
            budget_usd: Some(settings.limits.task_usd),
            ..Task::default()
        };
        task.steps = flow::initial_steps(&quality, &p);
        task.quality = quality;
        self.note_v1_settings(&p);
        self.save(&task)?;
        self.hold_back_if_limited(&task);
        Ok(id)
    }

    /// Human answer to a role's `NEEDS_HUMAN` question; the same role resumes the same session.
    /// A question the engine asked itself (budget, login) is answered by pressing it through.
    pub fn answer(&self, id: &str, text: &str) -> Result<(), String> {
        let asks = |t: &Task| {
            t.status == Status::NeedsHuman && t.question.as_ref().is_some_and(|q| q.kind == QuestionKind::Agent)
        };
        // the claim orders two clicks (and the drive thread finishing its step); the second
        // finds the task no longer waiting
        let _h = self.hold(id, false)?;
        let Some(t) = self.task(id).filter(|t| asks(t)) else { return Err("not_waiting".into()) };
        let special = self.state(id).special;
        let stage = self.pending(id).stage.unwrap_or(Status::Working);
        if let Some(key) = special {
            return self.answer_special(&t, &key, text, stage);
        }
        // persisted before the status flips, so neither the scheduler nor a restart can miss it
        self.set_pending(id, &Pending { prompt: Some(text.to_string()), stage: Some(stage.clone()), ..Default::default() })?;
        if let Err(e) = self.cas(id, "not_waiting", asks, |t| {
            t.question = None;
            t.status = stage.clone();
            t.resume_at = None;
        }) {
            self.clear_pending(id);
            return Err(e);
        }
        self.log(id, LogKind::Human, text);
        Ok(())
    }

    /// Stops the agent / check process, removes the worktree and the task branch.
    pub fn cancel(&self, id: &str) -> Result<(), String> {
        let _h = self.hold(id, true)?;
        self.do_cancel(id)
    }

    /// `cancel` for whoever holds the claim already.
    fn do_cancel(&self, id: &str) -> Result<(), String> {
        let now = self.now();
        let mut t = self.cas(id, "not_cancellable", |t| cancellable(&t.status), |t| {
            t.status = Status::Cancelled;
            t.question = None;
            t.resume_at = None;
            t.finished_at = Some(now);
        })?;
        self.clear_pending(id);
        self.clear_state(id);
        self.log(id, LogKind::Stage, "cancelled");
        if let (Some(wt), Some(p)) = (t.worktree.take(), self.project(&t.project_id)) {
            git::remove_worktree(&p.path, &wt, true).map_err(|e| e.to_string())?;
            self.save(&t)?;
        }
        Ok(())
    }

    /// Failed / cancelled / done: start over from a clean branch (the new work reaches main even if the
    /// task landed once). Done-not-pushed: push / open the PR again.
    pub fn retry(&self, id: &str) -> Result<(), String> {
        let _h = self.hold(id, false)?;
        let mut t = self.task(id).ok_or("task_not_found")?;
        let p = self.project(&t.project_id).ok_or("project_not_found")?;
        match t.status {
            Status::DoneNotPushed => {
                self.log(id, LogKind::Human, "retry push");
                self.outward(&mut t, &p);
                Ok(())
            }
            Status::Failed | Status::Cancelled | Status::Done | Status::Reverted => {
                // failed tasks keep their worktree; removing it with the branch gives a clean start
                if let Some(wt) = t.worktree.take() {
                    git::remove_worktree(&p.path, &wt, true).map_err(|e| e.to_string())?;
                }
                self.clear_pending(id);
                self.clear_state(id);
                git::forget_merged(&p.path, &t.branch); // else the new work would count as landed
                t.status = Status::Queued;
                t.session = None;
                t.attempts = 0;
                t.review_rounds = 0;
                t.question = None;
                t.resume_at = None;
                t.outcome = Outcome::default();
                t.started_at = None;
                t.finished_at = None;
                t.runs.clear();
                let settings = lk(&self.store).settings();
                t.steps = flow::initial_steps(&t.quality, &p);
                t.budget_usd = Some(settings.limits.task_usd);
                t.cost_usd = None;
                self.save(&t)?;
                self.log(id, LogKind::Human, "retry");
                Ok(())
            }
            _ => Err("not_retryable".into()),
        }
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        if self.task(id).is_some_and(|t| cancellable(&t.status)) {
            self.cancel(id)?;
        }
        let _h = self.hold(id, true)?;
        let t = self.task(id).ok_or("task_not_found")?;
        if let (Some(wt), Some(p)) = (&t.worktree, self.project(&t.project_id)) {
            git::remove_worktree(&p.path, wt, true).map_err(|e| e.to_string())?;
        }
        self.clear_pending(id);
        self.clear_state(id);
        lk(&self.store).remove_task(id).map_err(|e| e.to_string())
    }

    /// Revert commit on base. A pushed task's revert is pushed too: right away, or after
    /// approval (awaiting_outward "push the revert?") when the project asks first.
    pub fn undo(&self, id: &str) -> Result<(), String> {
        let _h = self.hold(id, false)?;
        let mut t = self.task(id).ok_or("task_not_found")?;
        if !matches!(t.status, Status::Done | Status::DoneNotPushed) {
            return Err("not_undoable".into());
        }
        let p = self.project(&t.project_id).ok_or("project_not_found")?;
        let Some(sha) = t.outcome.commit.clone() else {
            // a PR result lives on the branch / GitHub: nothing on base to revert
            let pr = t.outcome.pr_url.is_some() || p.finish == Finish::Pr;
            return Err(if pr { "pr_no_undo" } else { "no_commit" }.into());
        };
        let new = git::revert(&p.path, &p.base_branch, &sha).map_err(|e| e.to_string())?;
        self.log(id, LogKind::Git, &format!("reverted {sha} as {new}"));
        let was_pushed = t.status == Status::Done && p.finish == Finish::Push;
        t.status = Status::Reverted;
        self.save(&t)?;
        self.emit(EngineEvent::ProjectChanged(p.id.clone()));
        if was_pushed && self.asks_outward(&t, &p) {
            let options = vec!["Push".to_string(), "Skip".to_string()];
            t.question = Some(Question { text: flow::PUSH_REVERT.into(), options: options.clone(), kind: QuestionKind::Outward });
            t.status = Status::AwaitingOutward;
            self.save(&t)?;
            self.notify(NoticeKind::Outward, &t, &p, format!("revert {}", &new[..new.len().min(7)]), options);
        } else if was_pushed {
            git::push(&p.path, &p.base_branch).map_err(|e| e.to_string())?;
            self.log(id, LogKind::Git, &format!("pushed {}", p.base_branch));
        }
        Ok(())
    }

    pub fn push_base(&self, project_id: &str) -> Result<(), String> {
        let p = self.project(&project_id.to_string()).ok_or("project_not_found")?;
        git::push(&p.path, &p.base_branch).map_err(|e| e.to_string())?;
        self.emit(EngineEvent::ProjectChanged(p.id.clone()));
        if p.finish == Finish::Push {
            // their commits are on base, which is on origin now
            let waiting: Vec<TaskId> = lk(&self.store)
                .tasks()
                .into_iter()
                .filter(|t| t.project_id == p.id && t.status == Status::DoneNotPushed)
                .map(|t| t.id)
                .collect();
            for id in waiting {
                let Ok(_h) = self.hold(&id, false) else { continue };
                let pushed = |t: &Task| t.status == Status::DoneNotPushed;
                if self.cas(&id, "changed", pushed, |t| {
                    t.status = Status::Done;
                    t.outcome.error = None;
                }).is_ok() {
                    self.log(&id, LogKind::Git, "pushed");
                }
            }
        }
        Ok(())
    }

    pub fn pull_base(&self, project_id: &str) -> Result<(), String> {
        let p = self.project(&project_id.to_string()).ok_or("project_not_found")?;
        git::fetch_ff(&p.path, &p.base_branch).map_err(|e| e.to_string())?;
        self.emit(EngineEvent::ProjectChanged(p.id));
        Ok(())
    }

    /// Paused: nothing new starts; running tasks stop after their current step and wait.
    pub fn pause_all(&self, paused: bool) {
        let mut st = lk(&self.store);
        let mut s = st.settings();
        s.paused = paused;
        let _ = st.set_settings(s);
    }

    // ---- scheduler ----

    fn tick(self: &Arc<Self>) {
        if self.frozen.load(Ordering::SeqCst) {
            return;
        }
        let (settings, tasks) = {
            let st = lk(&self.store);
            (st.settings(), st.tasks())
        };
        if settings.paused {
            return;
        }
        let now = self.now();
        // seats are per env (R19i.5): a task holding a claim uses one seat of its env
        let mut used: HashMap<String, usize> = HashMap::new();
        {
            let r = lk(&self.running);
            for t in tasks.iter().filter(|t| r.tasks.contains_key(&t.id)) {
                *used.entry(t.agent.clone()).or_default() += 1;
            }
        }
        let cap = settings.max_parallel.max(1) as usize;
        let due = |t: &Task| t.resume_at.map(|r| r <= now).unwrap_or(true);
        let continuing = |t: &Task| {
            is_active(&t.status)
                || matches!(t.status, Status::PausedLimit | Status::MergeBlocked)
                || (t.status == Status::NeedsHuman && t.question.as_ref().is_some_and(|q| q.text.starts_with(roles::AUTH_REQUIRED)))
        };
        // continuing tasks first, then the queue in submit order
        let ready = tasks.iter().filter(|t| continuing(t)).chain(tasks.iter().filter(|t| t.status == Status::Queued));
        for t in ready {
            if used.get(&t.agent).copied().unwrap_or(0) >= cap || !due(t) {
                continue;
            }
            if t.status == Status::Queued && !self.may_start(t, &tasks, &settings, now) {
                continue;
            }
            // claim first (refused while stopping or when someone holds it), then compare-and-set
            let Some(flag) = self.claim(&t.id) else { continue };
            let seen = t.status.clone();
            let stage = self.pending(&t.id).stage;
            let r = self.cas(&t.id, "changed", |x| x.status == seen && due(x), |x| match seen {
                Status::PausedLimit | Status::NeedsHuman => {
                    x.status = stage.unwrap_or(Status::Working);
                    x.resume_at = None;
                    x.question = None;
                }
                Status::MergeBlocked => {
                    x.status = Status::Merging;
                    x.resume_at = None;
                }
                _ => {
                    x.resume_at = None;
                }
            });
            if r.is_err() {
                lk(&self.running).tasks.remove(&t.id);
                continue;
            }
            if seen == Status::PausedLimit {
                self.log(&t.id, LogKind::Stage, "limit reset, continuing");
            }
            *used.entry(t.agent.clone()).or_default() += 1;
            let (me, id) = (self.clone(), t.id.clone());
            std::thread::spawn(move || {
                let _h = Held { e: &me, id: id.clone(), wished: false };
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| me.drive(&id, &flag)));
                if let Err(p) = r {
                    let msg = p
                        .downcast_ref::<&str>()
                        .map(|s| s.to_string())
                        .or_else(|| p.downcast_ref::<String>().cloned())
                        .unwrap_or_default();
                    // only a task the panic left mid-stage; a finished or waiting one keeps its state
                    if let Some(mut t) = me.task(&id).filter(|t| is_active(&t.status)) {
                        let name = me.project(&t.project_id).map(|p| p.name).unwrap_or_default();
                        me.fail(&mut t, &name, format!("internal_error: {msg}"));
                    }
                }
            });
        }
    }

    /// Takes the task's claim for the scheduler; None when it is held already, somebody waits for it, or the engine is stopping.
    fn claim(&self, id: &str) -> Option<Arc<AtomicBool>> {
        self.claim_as(id, false)
    }

    fn claim_as(&self, id: &str, wished: bool) -> Option<Arc<AtomicBool>> {
        let mut r = lk(&self.running);
        if r.stopping || r.tasks.contains_key(id) || (!wished && r.wanted.contains_key(id)) {
            return None;
        }
        let f = Arc::new(AtomicBool::new(false));
        r.tasks.insert(id.to_string(), f.clone());
        Some(f)
    }

    fn unwish(&self, r: &mut Running, id: &str) {
        if let Some(n) = r.wanted.get_mut(id) {
            *n -= 1;
            if *n == 0 {
                r.wanted.remove(id);
            }
        }
    }

    /// Waits for the claim (`kill` = stop whoever holds it first) and keeps it until drop.
    fn hold(&self, id: &str, kill: bool) -> Result<Held<'_>, String> {
        let t0 = Instant::now();
        *lk(&self.running).wanted.entry(id.to_string()).or_default() += 1;
        loop {
            {
                let mut r = lk(&self.running);
                if r.stopping {
                    self.unwish(&mut r, id);
                    return Err("stopping".into());
                }
                match r.tasks.get(id) {
                    Some(f) if kill => f.store(true, Ordering::SeqCst),
                    Some(_) => {}
                    None => {}
                }
            }
            if self.claim_as(id, true).is_some() {
                return Ok(Held { e: self, id: id.to_string(), wished: true });
            }
            if t0.elapsed() > WAIT_LIMIT {
                self.unwish(&mut lk(&self.running), id);
                return Err("busy".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Check and write in one critical section of the store.
    fn cas(&self, id: &str, err: &str, ok: impl Fn(&Task) -> bool, f: impl FnOnce(&mut Task)) -> Result<Task, String> {
        let t = {
            let mut st = lk(&self.store);
            let mut t = st.task(id).ok_or("task_not_found")?;
            if !ok(&t) {
                return Err(err.to_string());
            }
            f(&mut t);
            st.upsert_task(&t).map_err(|e| e.to_string())?;
            t
        };
        self.emit(EngineEvent::TaskChanged(t.clone()));
        Ok(t)
    }

    // ---- pending prompt (on disk) ----

    fn pending_path(&self, id: &str) -> PathBuf {
        let safe: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        self.worktrees_dir.join(".pending").join(format!("{safe}.json"))
    }

    fn pending(&self, id: &str) -> Pending {
        std::fs::read(self.pending_path(id)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn set_pending(&self, id: &str, p: &Pending) -> Result<(), String> {
        let path = self.pending_path(id);
        let tmp = path.with_extension("tmp");
        let err = |e: std::io::Error| format!("pending_write_failed: {e}");
        std::fs::create_dir_all(path.parent().unwrap_or(&self.worktrees_dir)).map_err(err)?;
        std::fs::write(&tmp, serde_json::to_vec(p).map_err(|e| e.to_string())?).map_err(err)?;
        std::fs::rename(&tmp, &path).map_err(err)
    }

    fn clear_pending(&self, id: &str) {
        let _ = std::fs::remove_file(self.pending_path(id));
    }

    // ---- helpers ----

    fn now(&self) -> DateTime<Local> {
        (self.clock)()
    }

    fn task(&self, id: &str) -> Option<Task> {
        lk(&self.store).task(id)
    }

    fn project(&self, id: &ProjectId) -> Option<Project> {
        lk(&self.store).project(id)
    }

    /// The adapter of an env. A run in progress keeps the one it started with (`set_adapters`).
    fn adapter(&self, id: &str) -> Option<Arc<dyn Adapter>> {
        lk(&self.adapters).iter().find(|a| a.id() == id).cloned()
    }

    fn merge_lock(&self, project: &ProjectId) -> Arc<Mutex<()>> {
        lk(&self.merge_locks).entry(project.clone()).or_default().clone()
    }

    fn save(&self, t: &Task) -> Result<(), String> {
        lk(&self.store).upsert_task(t).map_err(|e| e.to_string())?;
        self.emit(EngineEvent::TaskChanged(t.clone()));
        Ok(())
    }

    fn log(&self, id: &str, kind: LogKind, text: &str) {
        self.log_ev(id, LogEvent::new(self.now(), kind, text));
    }

    fn log_ev(&self, id: &str, ev: LogEvent) {
        let _ = lk(&self.store).append_log(id, &ev);
        self.emit(EngineEvent::Log(id.to_string(), ev));
    }

    fn emit(&self, e: EngineEvent) {
        lk(&self.subs).retain(|s| s.send(e.clone()).is_ok());
    }

    fn notify(&self, kind: NoticeKind, t: &Task, p: &Project, body: String, options: Vec<String>) {
        self.notifier.send(&Notice {
            kind,
            task_id: t.id.clone(),
            project: p.name.clone(),
            title: t.title.clone(),
            body,
            options,
        });
    }

    /// The project still has the v1 `reviewer` / `fallback_agent` settings: they do nothing now,
    /// and its log says so once (G22.1). This is the only place those fields are looked at.
    fn note_v1_settings(&self, p: &Project) {
        if p.reviewer.is_none() && p.fallback_agent.is_none() {
            return;
        }
        let seen = lk(&self.store).read_log(&p.id, 200).iter().any(|e| e.text.starts_with(ops::V1_SETTING_NOTE));
        if !seen {
            self.log(&p.id, LogKind::Stage, ops::V1_SETTING_NOTE);
        }
    }

    /// Ask outward when the project asks or the env's rights ask (G01.9).
    fn asks_outward(&self, t: &Task, p: &Project) -> bool {
        let env = lk(&self.store).settings().envs.get(&t.agent).cloned().unwrap_or_default();
        p.ask_outward || if p.finish == Finish::Pr { env.ask_pr } else { env.ask_push }
    }
}
