//! Task lifecycle engine: queue with a parallelism limit, the task state machine, retries,
//! one merge at a time per project. Everything runs on plain threads.
//!
//! Ownership rule: a task's state is written only by whoever holds its claim in `running`
//! (the drive thread, or cancel / retry / undo / approve / answer while they work), checked
//! against the status inside the store's critical section. `push_base` takes each claim too.
mod flow;
mod util;

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

/// Wall clock, replaceable in tests (limit pauses, the once-a-minute merge retry).
pub type Clock = Arc<dyn Fn() -> DateTime<Local> + Send + Sync>;

#[derive(Clone, Debug)]
pub enum EngineEvent {
    TaskChanged(Task),
    Log(TaskId, LogEvent),
    ProjectChanged(ProjectId),
}

#[derive(Clone, Debug)]
pub struct NewTask {
    pub project_id: ProjectId,
    pub prompt: String,
    pub agent: Option<String>,
}

const TICK: Duration = Duration::from_millis(40);
const WAIT_LIMIT: Duration = Duration::from_secs(15);

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
}

/// A held claim; released on drop.
struct Held<'a> {
    e: &'a Engine,
    id: String,
}
impl Drop for Held<'_> {
    fn drop(&mut self) {
        lk(&self.e.running).tasks.remove(&self.id);
    }
}

/// A panic elsewhere must not take the whole engine down with poisoned locks.
fn lk<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct Engine {
    store: Mutex<Store>,
    adapters: Vec<Arc<dyn Adapter>>,
    notifier: Arc<dyn Notifier>,
    worktrees_dir: PathBuf,
    clock: Clock,
    subs: Mutex<Vec<Sender<EngineEvent>>>,
    running: Mutex<Running>,
    merge_locks: Mutex<HashMap<ProjectId, Arc<Mutex<()>>>>,
    scheduler: Mutex<Option<JoinHandle<()>>>,
}

fn is_active(s: &Status) -> bool {
    matches!(s, Status::Preparing | Status::Working | Status::Checking | Status::Reviewing | Status::Merging)
}

fn cancellable(s: &Status) -> bool {
    is_active(s) || matches!(s, Status::Queued | Status::NeedsHuman | Status::PausedLimit | Status::MergeBlocked)
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
        Arc::new(Engine {
            store: Mutex::new(store),
            adapters,
            notifier,
            worktrees_dir,
            clock,
            subs: Mutex::new(vec![]),
            running: Mutex::new(Running::default()),
            merge_locks: Mutex::new(HashMap::new()),
            scheduler: Mutex::new(None),
        })
    }

    /// Starts the scheduler. Tasks left in an active stage by a previous run are picked up again:
    /// the worker resumes its session, checks / review / merge simply run again.
    pub fn start(self: &Arc<Self>) {
        let mut s = lk(&self.scheduler);
        if s.is_some() || lk(&self.running).stopping {
            return;
        }
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
        let p = self.project(&t.project_id).ok_or("project_not_found")?;
        let agent = t.agent.unwrap_or(p.agent);
        if self.adapter(&agent).is_none() {
            return Err(format!("agent_not_found: {agent}"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let title = util::title_of(&prompt);
        let task = Task {
            branch: format!("ns/{}-{}", &id[..8], util::slug(&title)),
            id: id.clone(),
            project_id: p.id,
            title,
            prompt,
            agent,
            status: Status::Queued,
            worktree: None,
            session: None,
            attempts: 0,
            review_rounds: 0,
            question: None,
            resume_at: None,
            outcome: Outcome::default(),
            created_at: self.now(),
            started_at: None,
            finished_at: None,
            ..Task::default()
        };
        self.save(&task)?;
        Ok(id)
    }

    /// Human answer to the worker's `NEEDS_HUMAN` question; the worker resumes the same session.
    pub fn answer(&self, id: &str, text: &str) -> Result<(), String> {
        let asks = |t: &Task| {
            t.status == Status::NeedsHuman && t.question.as_ref().is_some_and(|q| q.kind == QuestionKind::Agent)
        };
        // the claim orders two clicks (and the drive thread finishing its step); the second
        // finds the task no longer waiting
        let _h = self.hold(id, false)?;
        if !self.task(id).is_some_and(|t| asks(&t)) {
            return Err("not_waiting".into());
        }
        // persisted before the status flips, so neither the scheduler nor a restart can miss it
        self.set_pending(id, &Pending { prompt: Some(text.to_string()), ..Default::default() })?;
        if let Err(e) = self.cas(id, "not_waiting", asks, |t| {
            t.question = None;
            t.status = Status::Working;
        }) {
            self.clear_pending(id);
            return Err(e);
        }
        self.log(id, LogKind::Human, text);
        Ok(())
    }

    pub fn approve_outward(&self, id: &str, yes: bool) -> Result<(), String> {
        let _h = self.hold(id, false)?;
        let mut t = self.task(id).ok_or("task_not_found")?;
        if t.status != Status::AwaitingOutward {
            return Err("not_waiting".into()); // e.g. the second of two clicks
        }
        let p = self.project(&t.project_id).ok_or("project_not_found")?;
        let revert = t.question.as_ref().is_some_and(|q| q.text == flow::PUSH_REVERT);
        t.question = None;
        self.log(id, LogKind::Human, if yes { "approved: push" } else { "declined: push" });
        match (revert, yes) {
            (true, true) => {
                t.status = Status::Reverted;
                t.outcome.error = git::push(&p.path, &p.base_branch).err().map(|e| e.to_string());
                self.save(&t)?;
                self.emit(EngineEvent::ProjectChanged(p.id.clone()));
            }
            (true, false) => {
                t.status = Status::Reverted;
                t.outcome.error = Some("push_skipped".into());
                self.save(&t)?;
            }
            (false, true) => self.outward(&mut t, &p),
            (false, false) => {
                t.outcome.error = Some("push_skipped".into());
                self.complete(&mut t, &p, Status::DoneNotPushed);
            }
        }
        Ok(())
    }

    /// Stops the agent / check process, removes the worktree and the task branch.
    pub fn cancel(&self, id: &str) -> Result<(), String> {
        let _h = self.hold(id, true)?;
        let now = self.now();
        let mut t = self.cas(id, "not_cancellable", |t| cancellable(&t.status), |t| {
            t.status = Status::Cancelled;
            t.question = None;
            t.resume_at = None;
            t.finished_at = Some(now);
        })?;
        self.clear_pending(id);
        self.log(id, LogKind::Stage, "cancelled");
        if let (Some(wt), Some(p)) = (t.worktree.take(), self.project(&t.project_id)) {
            git::remove_worktree(&p.path, &wt, true).map_err(|e| e.to_string())?;
            self.save(&t)?;
        }
        Ok(())
    }

    /// Failed / cancelled: start over from a clean branch. Done-not-pushed: push / open the PR again.
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
            Status::Failed | Status::Cancelled => {
                // failed tasks keep their worktree; removing it with the branch gives a clean start
                if let Some(wt) = t.worktree.take() {
                    git::remove_worktree(&p.path, &wt, true).map_err(|e| e.to_string())?;
                }
                self.clear_pending(id);
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
        if was_pushed && p.ask_outward {
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
        let (settings, tasks) = {
            let st = lk(&self.store);
            (st.settings(), st.tasks())
        };
        if settings.paused {
            return;
        }
        let now = self.now();
        let mut free = (settings.max_parallel.max(1) as usize).saturating_sub(lk(&self.running).tasks.len());
        let due = |t: &Task| t.resume_at.map(|r| r <= now).unwrap_or(true);
        // continuing tasks first, then the queue in submit order
        let ready = tasks
            .iter()
            .filter(|t| is_active(&t.status) || (matches!(t.status, Status::PausedLimit | Status::MergeBlocked) && due(t)))
            .chain(tasks.iter().filter(|t| t.status == Status::Queued));
        for t in ready {
            if free == 0 {
                break;
            }
            // claim first (refused while stopping or when someone holds it), then compare-and-set
            let Some(flag) = self.claim(&t.id) else { continue };
            let seen = t.status.clone();
            let stage = self.pending(&t.id).stage;
            let r = self.cas(&t.id, "changed", |x| x.status == seen && due(x), |x| match seen {
                Status::PausedLimit => {
                    x.status = stage.unwrap_or(Status::Working);
                    x.resume_at = None;
                }
                Status::MergeBlocked => {
                    x.status = Status::Merging;
                    x.resume_at = None;
                }
                _ => {}
            });
            if r.is_err() {
                lk(&self.running).tasks.remove(&t.id);
                continue;
            }
            if seen == Status::PausedLimit {
                self.log(&t.id, LogKind::Stage, "limit reset, continuing");
            }
            free -= 1;
            let (me, id) = (self.clone(), t.id.clone());
            std::thread::spawn(move || {
                let _h = Held { e: &me, id: id.clone() };
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

    /// Takes the task's claim; None when it is held already or the engine is stopping.
    fn claim(&self, id: &str) -> Option<Arc<AtomicBool>> {
        let mut r = lk(&self.running);
        if r.stopping || r.tasks.contains_key(id) {
            return None;
        }
        let f = Arc::new(AtomicBool::new(false));
        r.tasks.insert(id.to_string(), f.clone());
        Some(f)
    }

    /// Waits for the claim (`kill` = stop whoever holds it first) and keeps it until drop.
    fn hold(&self, id: &str, kill: bool) -> Result<Held<'_>, String> {
        let t0 = Instant::now();
        loop {
            {
                let r = lk(&self.running);
                if r.stopping {
                    return Err("stopping".into());
                }
                match r.tasks.get(id) {
                    Some(f) if kill => f.store(true, Ordering::SeqCst),
                    Some(_) => {}
                    None => {}
                }
            }
            if self.claim(id).is_some() {
                return Ok(Held { e: self, id: id.to_string() });
            }
            if t0.elapsed() > WAIT_LIMIT {
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

    fn adapter(&self, id: &str) -> Option<Arc<dyn Adapter>> {
        self.adapters.iter().find(|a| a.id() == id).cloned()
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
        let ev = LogEvent::new(self.now(), kind, text);
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
}
