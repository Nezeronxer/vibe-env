//! Lanes: the runs of a swarm that go side by side (a variant of a race, a part of a crew, a member of
//! the council). A lane takes a seat of the env (a swarm takes N seats, G11.7), runs in a copy of its
//! own, and writes to the task only its own run records and the cost: the rest of the task belongs to
//! the thread that drives it. Every lane is a run of the task's env, never of another one (G22).
use super::flow::{Flow, Res};
use super::roles::{pick, vendor_name, Ran};
use super::runs::{fold, stall_decision, Again, Next, RunCx, Stall, MAX_RUN_TRIES};
use super::state::{Asked, Pin};
use super::*;
use crate::agent::Run;
use crate::models;
use crate::prompt::{self, RoleInputs, Vendor};

/// A seat of an env held by a lane; given back on drop.
pub(super) struct Seat<'a> {
    e: &'a Engine,
    env: EnvId,
}
impl Drop for Seat<'_> {
    fn drop(&mut self) {
        let mut r = lk(&self.e.running);
        if let Some(n) = r.lanes.get_mut(&self.env) {
            *n = n.saturating_sub(1);
        }
        drop(r);
        self.e.seat_cv.notify_all();
    }
}

/// While alive, the task's own seat is the lanes' (see `Running.swarming`).
pub(super) struct LanesMode<'a> {
    e: &'a Engine,
    id: String,
}
impl Drop for LanesMode<'_> {
    fn drop(&mut self) {
        lk(&self.e.running).swarming.remove(&self.id);
        self.e.seat_cv.notify_all();
    }
}

/// One run of a lane.
pub(super) struct Lane<'a> {
    pub task_id: &'a str,
    pub project: &'a Project,
    pub role: RoleKind,
    /// "A", "B" ... (variant, part or voter): tags the run records and the feed.
    pub label: &'a str,
    /// The lane's own copy; None for a reader (it gets a throwaway copy).
    pub cwd: Option<PathBuf>,
    pub model: ModelChoice,
    pub session: Option<String>,
    pub pin: Option<Pin>,
    /// The prompt for the vendor that starts; the flag says the session is resumed.
    pub prompt: &'a dyn Fn(Vendor, bool) -> String,
    pub schema: Option<String>,
    /// A reader looks at the task's branch (the work so far) instead of base.
    pub at_task: bool,
}

pub(super) enum LaneEnd {
    Done { text: String, session: Option<String>, pin: Pin, model: ModelChoice },
    Failed(String),
    /// The env's limit is used up (and the env has no other model to go on with).
    Limit(Option<DateTime<Local>>),
    Auth(String),
    /// The CLI of the env is gone: the task waits for it, as it does for any role.
    Unavailable(String),
    /// The budget of the task or of the day is spent: the task asks the person, as for any role.
    Budget,
    /// The Watcher: silence again after the nudge. A new session is needed; the caller counts the attempt.
    Restart,
    Cancelled,
}

impl LaneEnd {
    /// What interrupted the lane, for the task to wait for (see `pause_for`). None = the lane ended on its own.
    pub(super) fn interrupt(&self) -> Option<String> {
        match self {
            LaneEnd::Cancelled => Some("cancelled".into()),
            LaneEnd::Auth(m) => Some(format!("auth:{m}")),
            LaneEnd::Limit(r) => Some(format!("limit:{}", r.map(|d| d.to_rfc3339()).unwrap_or_default())),
            LaneEnd::Unavailable(w) => Some(format!("env:{w}")),
            LaneEnd::Budget => Some("budget".into()),
            _ => None,
        }
    }
}

impl Engine {
    /// The git dir of the project that `path` (the project, or one of its copies) belongs to.
    fn git_dir_of(path: &std::path::Path) -> PathBuf {
        let dot = path.join(".git");
        if dot.is_dir() {
            return dot;
        }
        // a linked worktree: `.git` is a file "gitdir: <project>/.git/worktrees/<name>"
        std::fs::read_to_string(&dot)
            .ok()
            .and_then(|s| s.trim().strip_prefix("gitdir:").map(|p| PathBuf::from(p.trim())))
            .and_then(|p| p.parent().and_then(|x| x.parent()).map(|x| x.to_path_buf()))
            .unwrap_or_else(|| path.to_path_buf())
    }

    /// Runs a git operation that writes into the `.git` of the project of `path`, alone.
    pub(crate) fn with_git<R>(&self, path: &std::path::Path, f: impl FnOnce() -> R) -> R {
        let key = Self::git_dir_of(path);
        let key = key.canonicalize().unwrap_or(key); // /var and /private/var are one place
        let lock = lk(&self.repo_locks).entry(key).or_default().clone();
        let _one = lock.lock().unwrap_or_else(|e| e.into_inner());
        f()
    }

    pub(super) fn add_wt(&self, repo: &std::path::Path, dir: &std::path::Path, id: &str, branch: &str, base: &str) -> Result<PathBuf, git::GitError> {
        self.with_git(repo, || git::create_worktree(repo, dir, id, branch, base))
    }
    pub(crate) fn rm_wt(&self, repo: &std::path::Path, wt: &std::path::Path, delete_branch: bool) -> Result<(), git::GitError> {
        self.with_git(repo, || git::remove_worktree(repo, wt, delete_branch))
    }
    pub(crate) fn add_ro(&self, repo: &std::path::Path, dir: &std::path::Path, base: &str) -> Result<PathBuf, git::GitError> {
        self.with_git(repo, || git::create_readonly_copy(repo, dir, base))
    }
    /// "Защищённые пути" of the task's env (R17i.2): read from the settings on every call, never from the copy, where
    /// the agent can write.
    pub(super) fn protects(&self, t: &Task) -> bool {
        lk(&self.store).settings().envs.get(&t.agent).is_none_or(|e| e.protected_paths)
    }
    pub(super) fn g_commit(&self, t: &Task, wt: &std::path::Path, msg: &str) -> Result<Option<String>, git::GitError> {
        let protect = self.protects(t);
        self.with_git(wt, || git::commit_all(wt, msg, protect))
    }
    pub(super) fn g_uncommit(&self, t: &Task, wt: &std::path::Path, base: &str) -> Result<bool, git::GitError> {
        let protect = self.protects(t);
        self.with_git(wt, || git::uncommit_hidden(wt, base, protect))
    }
    pub(super) fn g_rebase(&self, wt: &std::path::Path, base: &str) -> Result<(), git::GitError> {
        self.with_git(wt, || git::rebase_onto(wt, base))
    }
    pub(super) fn g_rebase_continue(&self, t: &Task, wt: &std::path::Path) -> Result<(), git::GitError> {
        let protect = self.protects(t);
        self.with_git(wt, || git::rebase_continue(wt, protect))
    }
    pub(super) fn g_squash(&self, repo: &std::path::Path, wt: &std::path::Path, base: &str, msg: &str) -> Result<String, git::GitError> {
        self.with_git(repo, || git::squash_to_base(repo, wt, base, msg))
    }
    pub(super) fn g_ff(&self, repo: &std::path::Path, branch: &str, to: &str) -> Result<(), git::GitError> {
        self.with_git(repo, || git::ff_branch(repo, branch, to))
    }
    pub(super) fn g_revert(&self, repo: &std::path::Path, base: &str, sha: &str) -> Result<String, git::GitError> {
        self.with_git(repo, || git::revert(repo, base, sha))
    }
    pub(super) fn g_fetch_ff(&self, repo: &std::path::Path, base: &str) -> Result<(), git::GitError> {
        self.with_git(repo, || git::fetch_ff(repo, base))
    }
    pub(super) fn g_push(&self, repo: &std::path::Path, branch: &str) -> Result<(), git::GitError> {
        git::push(repo, branch) // over the network: not under the lock of the project
    }
    pub(super) fn g_forget(&self, repo: &std::path::Path, branch: &str) {
        self.with_git(repo, || git::forget_merged(repo, branch))
    }
    pub(super) fn g_open_pr(&self, repo: &std::path::Path, branch: &str, base: &str, title: &str, body: &str) -> Result<String, git::GitError> {
        git::open_pr(repo, branch, base, title, body) // pushes over the network: not under the lock
    }

    pub(super) fn lanes_mode(&self, id: &str) -> LanesMode<'_> {
        lk(&self.running).swarming.insert(id.to_string());
        LanesMode { e: self, id: id.to_string() }
    }

    /// Waits for a free seat of `env` (the cap is `max_parallel`), woken when one is given back.
    /// None = cancelled (or the engine stops) while waiting.
    pub(super) fn take_seat(&self, env: &str, cancel: &AtomicBool) -> Option<Seat<'_>> {
        loop {
            let cap = lk(&self.store).settings().max_parallel.max(1) as usize;
            let mut r = lk(&self.running);
            if cancel.load(Ordering::SeqCst) || r.stopping {
                return None;
            }
            let claims = r.claim_env.iter().filter(|(id, e)| e.as_str() == env && !r.swarming.contains(*id)).count();
            let held = r.lanes.get(env).copied().unwrap_or(0);
            if claims + held < cap {
                *r.lanes.entry(env.to_string()).or_default() += 1;
                return Some(Seat { e: self, env: env.to_string() });
            }
            // a seat given back, a claim released or a cancel wakes us at once; the timeout only covers a change of the cap
            let (g, _) = self.seat_cv.wait_timeout(r, Duration::from_millis(500)).unwrap_or_else(|e| e.into_inner());
            drop(g);
        }
    }

    /// Puts the lane's run records of (role, label) in the task, replacing the ones it had.
    pub(super) fn publish_runs(&self, task_id: &str, role: RoleKind, label: &str, runs: &[RoleRun]) {
        let t = {
            let mut st = lk(&self.store);
            let Some(mut t) = st.task(task_id) else { return };
            t.runs.retain(|r| !(r.role == role && r.variant.as_deref() == Some(label)));
            t.runs.extend(runs.iter().cloned());
            let _ = st.upsert_task(&t);
            t
        };
        self.emit(EngineEvent::TaskChanged(t));
    }

    /// The lane waits for a seat: "в очереди" on its track.
    pub(super) fn lane_waiting(&self, task_id: &str, role: RoleKind, label: &str, model: &ModelChoice) {
        let Some(t) = self.task(task_id) else { return };
        let Some(a) = self.adapter(&t.agent) else { return };
        let cat = models::catalog(&t.agent, &*a, &lk(&self.store));
        let mut runs = Self::lane_runs_of(&t, role, label);
        runs.push(RoleRun {
            role,
            variant: Some(label.to_string()),
            model: pick(&cat, &t.agent, Some(model)).label,
            session: None,
            state: StepState::Pending,
            started_at: None,
            finished_at: None,
            cost_usd: None,
        });
        self.publish_runs(task_id, role, label, &runs);
    }

    /// The records the lane has already, without the "waiting" placeholders.
    fn lane_runs_of(t: &Task, role: RoleKind, label: &str) -> Vec<RoleRun> {
        t.runs.iter().filter(|r| r.role == role && r.variant.as_deref() == Some(label) && !matches!(r.state, StepState::Pending)).cloned().collect()
    }

    /// The prompt of an executor that starts fresh in its own copy: the same as the single executor's.
    pub(super) fn worker_prompt(&self, t: &Task, p: &Project, v: Vendor) -> String {
        let st = self.state(&t.id);
        let tests_exist = p.check_cmd.as_deref().is_some_and(|c| !c.trim().is_empty());
        let text = if !t.quality.on {
            t.prompt.clone()
        } else {
            match &st.brief {
                Some(b) => prompt::render(v, RoleKind::Worker, b, &RoleInputs { tests_exist, tests_readonly: st.test_files.clone(), ..Default::default() }),
                None => prompt::render_simple(&t.prompt, st.brief_error.as_deref().unwrap_or("no breakdown")),
            }
        };
        match &st.worker_hint {
            Some(h) => format!("{text}\n\n{h}"),
            None => text,
        }
    }

    /// Runs one role of a lane to its end, in the env of the task. What the env reports is read by the same
    /// functions as for the role of a task (`runs`), so the same failure of the env leads to the same end;
    /// the lane changes nothing of the task except its own run records and the cost.
    pub(super) fn run_lane(&self, l: Lane, cancel: &AtomicBool) -> LaneEnd {
        let Some(t) = self.task(l.task_id) else { return LaneEnd::Failed("task_not_found".into()) };
        let env = t.agent.clone();
        let Some(a) = self.adapter(&env) else { return LaneEnd::Failed(format!("agent_not_found: {env}")) };
        if let Err(why) = a.available() {
            return LaneEnd::Unavailable(why);
        }
        if a.reports_cost() && self.budget_breach(&t).is_some() {
            return LaneEnd::Budget;
        }
        let settings = lk(&self.store).settings();
        let env_set = settings.envs.get(&env).cloned().unwrap_or_default();
        let cat = models::catalog(&env, &*a, &lk(&self.store));
        let role = l.role;
        let notes = self.pending_lane_notes(l.task_id, role, Some(l.label));
        let mut lane_runs = Self::lane_runs_of(&t, role, l.label);
        let mut choice = Some(l.model.clone());
        let mut pinned = l.pin.clone();
        let mut resume = l.session.clone();
        if self.start_on_fallback(l.task_id, &env, &cat, env_set.fallback.as_ref(), &mut choice) {
            pinned = None;
            resume = None;
        }
        let mut cx = RunCx { task_id: l.task_id, env: &env, role, cat: &cat, fallback: env_set.fallback.clone(), choice: choice.clone(), tried_fallback: false };
        let mut announced = false;
        let mut nudge_text: Option<String> = None;
        let mut nudged = false;
        for _ in 0..MAX_RUN_TRIES {
            let (cli_arg, label, vendor, note) = match &pinned {
                Some(p) => (p.cli_arg.clone(), p.label.clone(), Vendor::from_name(&p.vendor), None),
                None => {
                    let k = pick(&cat, &env, cx.choice.as_ref());
                    (k.cli_arg, k.label, k.vendor, k.note)
                }
            };
            if !announced && pinned.is_none() {
                announced = true;
                match note {
                    Some(ModelNote::Replaced { from, to }) => self.log(l.task_id, LogKind::Stage, &format!("model_replaced: {from} -> {to}")),
                    Some(ModelNote::Rejected { tried }) => self.log(l.task_id, LogKind::Stage, &format!("model_not_in_catalog: {tried}")),
                    _ => {}
                }
            }
            let mut text = match nudge_text.take() {
                Some(n) => n,
                None => (l.prompt)(vendor, resume.is_some()),
            };
            if !notes.is_empty() {
                text = format!("{text}\n\n{}", prompt::notes_block(&notes.iter().map(|n| n.1.clone()).collect::<Vec<_>>()));
            }
            let system = prompt::system_for(vendor, role);
            let ro = super::roles::is_read_only(role);
            let copy = if ro && !a.read_only_native() { self.ro_copy_on(&t, l.project, l.at_task).ok() } else { None };
            let Some(cwd) = copy.clone().or(l.cwd.clone()).or_else(|| ro.then(|| l.project.path.clone())) else {
                return LaneEnd::Failed("no_worktree".into());
            };
            let mut run = Run::new(cwd, text, system, role);
            run.resume = resume.clone();
            run.model = cli_arg.clone();
            run.read_only = ro;
            run.extra_env = self.env_vars_of(&env, Some(l.task_id));
            run.json_schema = l.schema.clone();
            run.extra_args = env_set.extra_args.clone();
            if a.reports_cost() {
                let fresh = self.task(l.task_id).unwrap_or_else(|| t.clone());
                let day = lk(&self.store).day_spend(&env, self.now().date_naive());
                let day_left = (settings.limits.day_usd + day.extra_usd - day.spent_usd).max(0.0);
                let task_left = fresh.budget_usd.map(|b| (b - fresh.cost_usd.unwrap_or(0.0)).max(0.0));
                run.max_budget_usd = Some(task_left.map_or(day_left, |x| x.min(day_left)));
            }
            lane_runs.push(RoleRun {
                role,
                variant: Some(l.label.to_string()),
                model: label.clone(),
                session: resume.clone(),
                state: StepState::Running,
                started_at: Some(self.now()),
                finished_at: None,
                cost_usd: None,
            });
            let idx = lane_runs.len() - 1;
            self.publish_runs(l.task_id, role, l.label, &lane_runs);
            let ran = self.run_agent(&t, &*a, &run, (&label, Some(l.label)), &notes, self.silence_limit(role, nudged), cancel);
            if let Some(dir) = copy {
                self.drop_ro(&t.id, l.project, &dir);
            }
            let end = |runs: &mut Vec<RoleRun>, state: StepState| {
                runs[idx].state = state;
                runs[idx].finished_at = Some(self.now());
                self.publish_runs(l.task_id, role, l.label, runs);
            };
            let events = match ran {
                Ran::Events(e) => e,
                Ran::Cancelled => {
                    end(&mut lane_runs, StepState::Failed("interrupted".into()));
                    return LaneEnd::Cancelled;
                }
                Ran::Stalled => {
                    end(&mut lane_runs, StepState::Failed("stalled".into()));
                    // the same decision as for the role of a task: nudge the session once, then a new session
                    match stall_decision(nudged, &resume) {
                        Stall::Nudge => {
                            self.log(l.task_id, LogKind::Stage, &format!("watcher: silent for {} min, nudged {role:?} {}", settings.stall_min, l.label));
                            if self.diagnose_lane(&l, &t, cancel) {
                                return LaneEnd::Cancelled;
                            }
                            nudged = true;
                            nudge_text = Some(watch::NUDGE.to_string());
                            continue;
                        }
                        Stall::Restart => {
                            self.log(l.task_id, LogKind::Stage, &format!("watcher: restarted {role:?} {} with a new session", l.label));
                            return LaneEnd::Restart;
                        }
                    }
                }
            };
            let f = fold(&events);
            let next = self.after_run(&mut cx, &label, &f);
            if let Some(c) = f.cost {
                lane_runs[idx].cost_usd = Some(c);
            }
            let session = f.done.as_ref().and_then(|d| d.0.clone());
            if session.is_some() {
                lane_runs[idx].session = session.clone();
            }
            match next {
                Next::Auth(msg) => {
                    end(&mut lane_runs, StepState::Failed("auth_required".into()));
                    return LaneEnd::Auth(msg);
                }
                Next::Refused(m) => {
                    end(&mut lane_runs, StepState::Failed("model_rejected".into()));
                    return LaneEnd::Failed(m);
                }
                Next::Again(why) => {
                    end(&mut lane_runs, StepState::Failed(if why == Again::Fallback { "limit" } else { "model_rejected" }.into()));
                    pinned = None;
                    resume = None;
                    continue;
                }
                Next::Limit(resets) => {
                    end(&mut lane_runs, StepState::Failed("limit".into()));
                    return LaneEnd::Limit(resets);
                }
                Next::Done { text } => {
                    end(&mut lane_runs, StepState::Done);
                    let pin = Pin { cli_arg, label, vendor: vendor_name(vendor).to_string() };
                    return LaneEnd::Done { text, session, pin, model: cx.choice.clone().unwrap_or(l.model.clone()) };
                }
                Next::Failed(text) => {
                    end(&mut lane_runs, StepState::Failed(util::tail(&text, 3)));
                    return LaneEnd::Failed(format!("agent_failed: {}", util::tail(&text, 5)));
                }
            }
        }
        LaneEnd::Failed("model_rejected: no model of the env was accepted".into())
    }

    /// The Watcher's diagnosis for a lane, by a run of its own (read-only, in the lane's label). True = cancelled.
    fn diagnose_lane(&self, l: &Lane, t: &Task, cancel: &AtomicBool) -> bool {
        let request = self.diagnosis_request(l.task_id);
        let ptext = |_: Vendor, _: bool| request.clone();
        let lane = Lane {
            task_id: l.task_id,
            project: l.project,
            role: RoleKind::Watcher,
            label: l.label,
            cwd: None,
            model: Self::choice_for(t, RoleKind::Watcher).unwrap_or_else(|| l.model.clone()),
            session: None,
            pin: None,
            prompt: &ptext,
            schema: None,
            at_task: false,
        };
        match self.run_lane(lane, cancel) {
            LaneEnd::Done { text, .. } => {
                self.log_diagnosis(l.task_id, &text);
                false
            }
            LaneEnd::Cancelled => true,
            _ => false, // no diagnosis: the nudge goes on without it
        }
    }

    // ---- what happens to the task when its lanes are interrupted or ask ----

    /// The task waits for what interrupted its lanes (a limit, a login, the env gone, the budget); the
    /// lanes that were interrupted go again afterwards, from where they were.
    pub(super) fn pause_for(&self, t: &mut Task, p: &Project, why: &str) -> Res {
        let stage = Status::Working;
        if let Some(msg) = why.strip_prefix("auth:") {
            let a = self.adapter(&t.agent).ok_or("agent_not_found")?;
            self.on_auth(t, &*a, msg, stage)?;
        } else if let Some(r) = why.strip_prefix("limit:") {
            let resets = DateTime::parse_from_rfc3339(r).ok().map(|d| d.with_timezone(&Local));
            return self.on_limit(t, p, resets, if t.status == Status::Reviewing { Status::Reviewing } else { stage });
        } else if let Some(msg) = why.strip_prefix("env:") {
            self.wait_for_env(t, msg)?;
        } else if why.starts_with("budget") {
            let a = self.adapter(&t.agent).ok_or("agent_not_found")?;
            self.budget_gate(t, p, &*a, stage)?;
        }
        Ok(Flow::Stop)
    }

    /// A lane's agent ended with `NEEDS_HUMAN:`: the question waits for the person, with the lane named.
    pub(super) fn lane_asks(&self, id: &str, asked: Asked) {
        let _ = self.update_state(id, |s| s.swarm.asking.push(asked));
    }

    /// The first waiting question of a lane goes to the person (one call). True = the task now waits.
    pub(super) fn ask_next_lane(&self, t: &mut Task, p: &Project, stage: Status) -> Result<bool, String> {
        let first = self.update_state(&t.id, |s| if s.swarm.asking.is_empty() { None } else { Some(s.swarm.asking.remove(0)) })?;
        let Some(a) = first else { return Ok(false) };
        let role = if a.kind == "Member" { RoleKind::Council } else { RoleKind::Worker };
        let text = format!("{} {}: {}", a.kind, a.label, a.question);
        // the lane is named before the question shows: an answer given at once goes back to it
        let label = a.label;
        self.update_state(&t.id, |s| s.swarm.asked_lane = Some(label))?;
        self.ask_role(t, p, role, stage, (text, a.options), a.session)?;
        Ok(true)
    }

    /// The person answered a lane's question: (the lane, the answer). The answer goes back to that lane.
    pub(super) fn take_lane_answer(&self, id: &str) -> Option<(String, String)> {
        let label = self.state(id).swarm.asked_lane?;
        let answer = self.pending(id).prompt?;
        self.clear_pending(id);
        let _ = self.update_state(id, |s| {
            s.swarm.asked_lane = None;
            s.asked = None;
            s.asked_session = None;
        });
        Some((label, answer))
    }
}
