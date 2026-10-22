//! One run of one role (spec 16): the adapter is the task's env and nothing else (G22), the model is
//! the role's from the task's snapshot, the prompt is rendered for the vendor of the model that really
//! starts (G14.3), read-only roles read through the env's flag or in a throwaway copy, and everything
//! the run reports is acted on here: limit windows, cost, a refused model, a missing login, a limit hit.
use super::state::Pin;
use super::*;
use crate::agent::{self, AgentEvent, Run};
use crate::models;
use crate::prompt::{self, Vendor};
use chrono::Duration as CDuration;

/// Keys of the questions the engine asks itself; the UI translates them.
pub(crate) const AUTH_REQUIRED: &str = "auth_required";
pub(crate) const ENV_UNAVAILABLE: &str = "env_unavailable";
pub(super) const WAITING_LIMIT: &str = "waiting_limit_reset";
pub(super) const BUDGET_EXCEEDED: &str = "budget_exceeded";
pub(super) const DAY_BUDGET_EXCEEDED: &str = "day_budget_exceeded";
pub(super) const SPECIAL_BUDGET: &str = "budget";
pub(super) const SPECIAL_DAY_BUDGET: &str = "day_budget";
pub(super) const SPECIAL_AUTH: &str = "auth";
/// "Add $5" on a budget question.
pub(super) const BUDGET_STEP_USD: f64 = 5.0;
/// A limit with no known reset time is waited out this long.
pub(crate) const LIMIT_WAIT_MINS: i64 = 30;
/// Log line of a finished run: `cost_usd: <amount> (<role>)`. `summary_since` adds these up.
pub(super) const COST_LOG: &str = "cost_usd: ";

/// How a call to a role ended.
pub(super) enum RoleEnd {
    Done { text: String, session: Option<String> },
    /// The run failed (the text is what the env said).
    Failed(String),
    /// The limit is used up and there was no other model to go on with.
    Limit(Option<DateTime<Local>>),
    /// The task now waits for a login; it has been moved.
    Auth,
    Cancelled,
    /// The task was moved aside (budget question, env gone); the caller just stops.
    Stop,
}

/// What a run came to.
pub(super) enum Ran {
    Events(Vec<AgentEvent>),
    Cancelled,
    /// Killed after the silence limit.
    Stalled,
}

pub(super) struct RoleCall<'a> {
    pub role: RoleKind,
    pub resume: Option<String>,
    /// The prompt for the vendor of the model that starts; the flag says the model was switched
    /// in the middle of the work (limit fallback), so the text must tell what was done so far.
    pub prompt: &'a dyn Fn(Vendor, bool) -> String,
    pub schema: Option<String>,
    /// Where the task returns after a question or a wait.
    pub stage: Status,
    /// Set by `prompt` to (text, is a whole prompt): a whole one goes to `Task.rendered_prompt`
    /// as the run starts (story 147).
    pub shown: Option<&'a std::cell::RefCell<Option<(String, bool)>>>,
}

/// Roles that only read.
pub(super) fn is_read_only(role: RoleKind) -> bool {
    !matches!(role, RoleKind::Worker | RoleKind::TestAuthor | RoleKind::Integrator)
}

pub(super) fn vendor_name(v: Vendor) -> &'static str {
    match v {
        Vendor::Anthropic => "anthropic",
        Vendor::OpenAi => "openai",
        Vendor::Google => "google",
        Vendor::Alibaba => "qwen",
        Vendor::Generic => "generic",
    }
}

pub(crate) struct Picked {
    pub cli_arg: Option<String>,
    pub label: String,
    pub vendor: Vendor,
    pub note: Option<ModelNote>,
}

/// The model of a choice in a catalog: the CLI argument, the caption, and whose template it gets.
pub(crate) fn pick(cat: &Catalog, env: &str, choice: Option<&ModelChoice>) -> Picked {
    let Some(c) = choice else {
        return Picked { cli_arg: None, label: "default".into(), vendor: Vendor::from_name(env), note: None };
    };
    let r = models::resolve(cat, c);
    let family_vendor = cat.families.iter().find(|f| f.id == c.family).map(|f| f.vendor.as_str());
    let vendor = match family_vendor {
        Some(models::VENDOR_CLI) => Vendor::from_name(env),
        Some(models::VENDOR_CUSTOM) | None => Vendor::from_model_id(r.cli_arg.as_deref().unwrap_or(&c.family)),
        Some(v) => match Vendor::from_name(v) {
            Vendor::Generic => Vendor::from_model_id(r.cli_arg.as_deref().unwrap_or(&c.family)),
            known => known,
        },
    };
    Picked { cli_arg: r.cli_arg, label: r.label, vendor, note: r.note }
}

impl Engine {
    /// The model choice of a role: its own in the task's snapshot, else the executor's ("as the executor").
    pub(super) fn choice_for(t: &Task, role: RoleKind) -> Option<ModelChoice> {
        t.models.get(&role).or_else(|| t.models.get(&RoleKind::Worker)).cloned()
    }

    /// Runs one role to its end. Moves the task aside itself when something other than an answer
    /// comes back (`Auth`, `Stop`); the caller handles the rest.
    pub(super) fn run_role(&self, t: &mut Task, p: &Project, call: RoleCall, cancel: &AtomicBool) -> Result<RoleEnd, String> {
        let env = t.agent.clone();
        let a = self.adapter(&env).ok_or_else(|| format!("agent_not_found: {env}"))?;
        if let Err(why) = a.available() {
            // the env is gone (CLI removed, path broken): the task waits and goes on by itself (G17.3)
            self.wait_for_env(t, &why)?;
            return Ok(RoleEnd::Stop);
        }
        if t.outcome.error.as_deref().is_some_and(|e| e.starts_with(ENV_UNAVAILABLE) || e.starts_with(WAITING_LIMIT)) {
            t.outcome.error = None;
        }
        t.resume_at = None;
        if self.budget_gate(t, p, &*a, call.stage.clone())? {
            return Ok(RoleEnd::Stop);
        }
        let role = call.role;
        let settings = lk(&self.store).settings();
        let env_set = settings.envs.get(&env).cloned().unwrap_or_default();
        let cat = models::catalog(&env, &*a, &lk(&self.store));
        let notes = self.pending_notes(&t.id, role);
        let state = self.state(&t.id);

        let mut resume = call.resume.clone();
        let mut switched = false;
        let mut choice = Self::choice_for(t, role);
        let mut pinned: Option<Pin> = resume.as_ref().and_then(|_| state.pinned.get(&role).cloned());
        // the limit is known to be used up and there is another model to go on with: start there
        if self.start_on_fallback(&t.id, &env, &cat, env_set.fallback.as_ref(), &mut choice) {
            switched = resume.is_some() || (role == RoleKind::Worker && t.session.is_some());
            t.models.insert(role, choice.clone().unwrap_or(ModelChoice { family: models::DEFAULT_FAMILY.into(), version: None }));
            resume = None;
            pinned = None;
            if role == RoleKind::Worker {
                t.session = None;
            }
        }
        let mut announced = false;
        let mut tried_fallback = false;
        // the Watcher's nudge: the next run continues the session with this text (G08.2)
        let mut nudge_text: Option<String> = None;
        let mut nudged = false;
        for _ in 0..runs::MAX_RUN_TRIES {
            let (cli_arg, label, vendor, note) = match &pinned {
                Some(p) => (p.cli_arg.clone(), p.label.clone(), Vendor::from_name(&p.vendor), None),
                None => {
                    let k = pick(&cat, &env, choice.as_ref());
                    (k.cli_arg, k.label, k.vendor, k.note)
                }
            };
            if !announced && pinned.is_none() {
                announced = true;
                match note {
                    Some(ModelNote::Replaced { from, to }) => self.log(&t.id, LogKind::Stage, &format!("model_replaced: {from} -> {to}")),
                    Some(ModelNote::Rejected { tried }) => self.log(&t.id, LogKind::Stage, &format!("model_not_in_catalog: {tried}")),
                    _ => {}
                }
            }
            let mut text = match nudge_text.take() {
                Some(n) => n,
                None => (call.prompt)(vendor, switched),
            };
            if let Some((s, true)) = call.shown.and_then(|c| c.borrow().clone()) {
                t.rendered_prompt = Some(s);
            }
            if !notes.is_empty() {
                text = format!("{text}\n\n{}", prompt::notes_block(&notes.iter().map(|n| n.1.clone()).collect::<Vec<_>>()));
            }
            let system = prompt::system_for(vendor, role);

            // where it runs: writers in the task's copy; readers through the env's flag or in a throwaway copy
            let ro = is_read_only(role);
            let copy = if ro && !a.read_only_native() { Some(self.ro_copy(t, p, role)?) } else { None };
            // a reader needs no copy of the task (the Coordinator of a convoy has none yet)
            let cwd = match (&copy, t.worktree.clone()) {
                (Some(c), _) => c.clone(),
                (None, Some(w)) => w,
                (None, None) if ro => p.path.clone(),
                (None, None) => return Err("no_worktree".into()),
            };
            let mut run = Run::new(cwd, text, system, role);
            run.resume = resume.clone();
            run.model = cli_arg.clone();
            run.read_only = ro;
            run.extra_env = self.env_vars_of(&env, Some(&t.id));
            run.json_schema = call.schema.clone();
            run.extra_args = env_set.extra_args.clone();
            if a.reports_cost() {
                // capped by what is left of the task and of the env's day, whichever is less
                let day = lk(&self.store).day_spend(&env, self.now().date_naive());
                let day_left = (settings.limits.day_usd + day.extra_usd - day.spent_usd).max(0.0);
                let task_left = t.budget_usd.map(|b| (b - t.cost_usd.unwrap_or(0.0)).max(0.0));
                run.max_budget_usd = Some(task_left.map_or(day_left, |x| x.min(day_left)));
            }

            let idx = t.runs.len();
            t.runs.push(RoleRun {
                role,
                variant: None,
                model: label.clone(),
                session: resume.clone(),
                state: StepState::Running,
                started_at: Some(self.now()),
                finished_at: None,
                cost_usd: None,
            });
            self.save(t)?;
            let silence = self.silence_limit(role, nudged);
            let ran = self.run_agent(t, &*a, &run, (&label, None), &notes, silence, cancel);
            if let Some(dir) = copy {
                self.drop_ro(&t.id, p, &dir);
            }
            let events = match ran {
                Ran::Events(e) => e,
                Ran::Cancelled => {
                    t.runs[idx].state = StepState::Failed("interrupted".into());
                    t.runs[idx].finished_at = Some(self.now());
                    let _ = self.save(t);
                    return Ok(RoleEnd::Cancelled);
                }
                Ran::Stalled => {
                    t.runs[idx].state = StepState::Failed("stalled".into());
                    t.runs[idx].finished_at = Some(self.now());
                    self.save(t)?;
                    let known = resume.clone().or_else(|| if role == RoleKind::Worker { t.session.clone() } else { None });
                    if runs::stall_decision(nudged, &known) == runs::Stall::Nudge {
                        // first silence: say what the log shows, go on with the same session
                        self.log(&t.id, LogKind::Stage, &format!("watcher: silent for {} min, nudged", settings.stall_min));
                        if self.diagnose(t, p, call.stage.clone(), cancel)? {
                            return Ok(RoleEnd::Cancelled);
                        }
                        nudged = true;
                        resume = known;
                        nudge_text = Some(watch::NUDGE.to_string());
                        continue;
                    }
                    // second silence (or no session to go on with): a new session, and it counts as an attempt
                    t.attempts += 1;
                    let cap = settings.attempts.max(1);
                    self.log(&t.id, LogKind::Stage, &format!("watcher: restarted {role:?} with a new session (attempt {})", t.attempts));
                    if t.attempts >= cap {
                        self.save(t)?;
                        return Err("stalled: no output from the agent".into());
                    }
                    nudged = false;
                    resume = None;
                    pinned = None;
                    switched = true;
                    if role == RoleKind::Worker {
                        t.session = None;
                    }
                    self.save(t)?;
                    continue;
                }
            };

            // what the run reported, and what comes next: the same decision for every run of every role
            let f = runs::fold(&events);
            let mut cx = runs::RunCx { task_id: &t.id, env: &env, role, cat: &cat, fallback: env_set.fallback.clone(), choice: choice.clone(), tried_fallback };
            let next = self.after_run(&mut cx, &label, &f);
            (choice, tried_fallback) = (cx.choice, cx.tried_fallback);
            if let Some(c) = f.cost {
                t.cost_usd = self.task(&t.id).and_then(|x| x.cost_usd).or(t.cost_usd);
                t.runs[idx].cost_usd = Some(c);
            }
            let session = f.done.as_ref().and_then(|d| d.0.clone());
            t.runs[idx].session = session.clone().or(t.runs[idx].session.take());
            t.runs[idx].finished_at = Some(self.now());
            if let Some(s) = &session {
                if pinned.is_none() {
                    let pin = Pin { cli_arg: cli_arg.clone(), label: label.clone(), vendor: vendor_name(vendor).to_string() };
                    self.update_state(&t.id, |st| {
                        st.pinned.insert(role, pin);
                    })?;
                }
                if role == RoleKind::Worker {
                    t.session = Some(s.clone());
                }
            }
            // what the writer did so far is committed, so whoever goes on sees it in the diff
            let commit_work = |t: &Task| -> Result<(), String> {
                if !is_read_only(role) {
                    if let Some(w) = t.worktree.as_ref().filter(|w| !git::rebase_in_progress(w)) {
                        self.g_commit(t, w, &t.title).map_err(|e| e.to_string())?;
                    }
                }
                Ok(())
            };
            match next {
                runs::Next::Auth(msg) => {
                    t.runs[idx].state = StepState::Failed("auth_required".into());
                    self.on_auth(t, &*a, &msg, call.stage.clone())?;
                    return Ok(RoleEnd::Auth);
                }
                runs::Next::Refused(m) => {
                    t.runs[idx].state = StepState::Failed("model_rejected".into());
                    self.save(t)?;
                    return Ok(RoleEnd::Failed(m));
                }
                runs::Next::Again(why) => {
                    let next_model = choice.clone().unwrap_or(ModelChoice { family: models::DEFAULT_FAMILY.into(), version: None });
                    t.models.insert(role, next_model);
                    pinned = None;
                    resume = None;
                    if why == runs::Again::Fallback {
                        t.runs[idx].state = StepState::Failed("limit".into());
                        commit_work(t)?;
                        switched = true;
                        if role == RoleKind::Worker {
                            t.session = None;
                        }
                    } else {
                        t.runs[idx].state = StepState::Failed("model_rejected".into()); // not an attempt (G18.4)
                    }
                    continue;
                }
                runs::Next::Limit(resets) => {
                    t.runs[idx].state = StepState::Failed("limit".into());
                    commit_work(t)?;
                    self.save(t)?;
                    return Ok(RoleEnd::Limit(resets));
                }
                runs::Next::Done { text } => {
                    t.runs[idx].state = StepState::Done;
                    self.save(t)?;
                    self.auth_ok(&env);
                    if state.special.as_deref() == Some(SPECIAL_AUTH) {
                        self.update_state(&t.id, |s| s.special = None)?; // the login worked: the task no longer waits for it
                    }
                    return Ok(RoleEnd::Done { text, session });
                }
                runs::Next::Failed(text) => {
                    t.runs[idx].state = StepState::Failed(util::tail(&text, 3));
                    self.save(t)?;
                    return Ok(RoleEnd::Failed(text));
                }
            }
        }
        Err("model_rejected: no model of the env was accepted".into())
    }

    /// Runs an agent, logging every event tagged with its role, model and variant. With `silence` set,
    /// a run that prints nothing for that long (by the engine's clock) is killed: `Stalled`. `notes` (in its prompt) are marked
    /// delivered once the agent's process has started; a start that fails leaves them for the next run.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_agent(
        &self,
        t: &Task,
        a: &dyn agent::Adapter,
        run: &Run,
        (model, variant): (&str, Option<&str>),
        notes: &[(String, String)],
        silence: Option<CDuration>,
        cancel: &AtomicBool,
    ) -> Ran {
        let id = t.id.clone();
        let role = run.role;
        let kind = if role == RoleKind::Reviewer { LogKind::Review } else { LogKind::Agent };
        let last = Mutex::new(self.now());
        let tag = |k: LogKind, text: &str| {
            let mut ev = LogEvent::new(self.now(), k, text);
            ev.role = Some(role);
            ev.model = Some(model.to_string());
            ev.variant = variant.map(str::to_string);
            self.log_ev(&id, ev);
        };
        let mut on = |e: &AgentEvent| {
            *lk(&last) = self.now();
            match e {
                AgentEvent::Text(s) => tag(kind.clone(), s),
                AgentEvent::Tool { name, summary } => tag(LogKind::Tool, &format!("{name}: {summary}")),
                AgentEvent::Done { text, is_error, .. } => tag(if *is_error { LogKind::Error } else { kind.clone() }, text),
                AgentEvent::Limit { .. } => tag(LogKind::Stage, "usage limit reached"),
                AgentEvent::Usage(_) | AgentEvent::Cost(_) => {}
                AgentEvent::ModelRejected(m) | AgentEvent::AuthRequired(m) => tag(LogKind::Error, m),
            }
        };
        let local = AtomicBool::new(false);
        let (stalled, done) = (AtomicBool::new(false), AtomicBool::new(false));
        let res = std::thread::scope(|sc| {
            let flag: &AtomicBool = match silence {
                Some(limit) => {
                    let (done, local, stalled, last) = (&done, &local, &stalled, &last);
                    sc.spawn(move || {
                        while !done.load(Ordering::SeqCst) {
                            if cancel.load(Ordering::SeqCst) {
                                local.store(true, Ordering::SeqCst);
                            } else if self.now() - *lk(last) >= limit {
                                stalled.store(true, Ordering::SeqCst);
                                local.store(true, Ordering::SeqCst);
                            }
                            std::thread::sleep(Duration::from_millis(15));
                        }
                    });
                    local
                }
                None => cancel,
            };
            // also when the run panics: the watcher must end, or the scope never returns
            struct EndOnDrop<'a>(&'a AtomicBool);
            impl Drop for EndOnDrop<'_> {
                fn drop(&mut self) {
                    self.0.store(true, Ordering::SeqCst);
                }
            }
            let _end = EndOnDrop(&done);
            agent::run_blocking_with(a, run, &mut on, &mut || self.deliver_notes(&id, role, variant, notes), flag)
        });
        if cancel.load(Ordering::SeqCst) {
            return Ran::Cancelled;
        }
        if stalled.load(Ordering::SeqCst) {
            return Ran::Stalled;
        }
        Ran::Events(res.unwrap_or_else(|e| {
            let ev = AgentEvent::Done { session: None, text: format!("{}: {e}", a.id()), is_error: true };
            on(&ev);
            vec![ev]
        }))
    }

    // ---- read-only copies ----

    pub(super) fn ro_copy(&self, t: &Task, p: &Project, role: RoleKind) -> Result<PathBuf, String> {
        self.ro_copy_on(t, p, matches!(role, RoleKind::Reviewer | RoleKind::Verifier))
    }

    /// A throwaway copy at the task's branch (`at_task`: the work so far) or at base (the project as it is).
    pub(super) fn ro_copy_on(&self, t: &Task, p: &Project, at_task: bool) -> Result<PathBuf, String> {
        let dir = self.worktrees_dir.join(".ro").join(format!("{}-{}", &t.id[..t.id.len().min(8)], &uuid::Uuid::new_v4().simple().to_string()[..8]));
        // the reviewer and the verifier look at the task's work, the others at the project as it is on base
        let at = if at_task { t.branch.as_str() } else { p.base_branch.as_str() };
        self.add_ro(&p.path, &dir, at).map_err(|e| e.to_string())
    }

    /// Removes a throwaway copy; one that cannot be removed is a line in the feed of the task, not silence.
    pub(super) fn drop_ro(&self, task_id: &str, p: &Project, dir: &std::path::Path) {
        if let Err(e) = self.rm_wt(&p.path, dir, false) {
            self.log(task_id, LogKind::Error, &format!("copy_not_removed: {}: {e}", dir.display()));
        }
        if let Err(e) = std::fs::remove_dir_all(dir) {
            if dir.exists() {
                self.log(task_id, LogKind::Error, &format!("copy_not_removed: {}: {e}", dir.display()));
            }
        }
    }

    /// Throwaway copies left by a crash are gone at start.
    pub(super) fn sweep_ro_copies(&self) {
        let ro = self.worktrees_dir.join(".ro");
        if !ro.exists() {
            return;
        }
        if let Err(e) = std::fs::remove_dir_all(&ro) {
            for p in lk(&self.store).projects() {
                self.log(&p.id, LogKind::Error, &format!("copy_not_removed: {}: {e}", ro.display()));
            }
        }
        for p in lk(&self.store).projects() {
            if let Err(e) = self.rm_wt(&p.path, &ro.join("none"), false) {
                self.log(&p.id, LogKind::Error, &format!("copy_not_removed: {e}")); // prunes what is gone
            }
        }
    }

    // ---- limits, cost, login, env ----

    pub(crate) fn record_usage(&self, env: &str, windows: Vec<LimitWindow>) {
        let new = EnvLimits::Known(windows);
        let changed = {
            let st = lk(&self.store);
            let changed = st.limits(env).as_ref() != Some(&new);
            if changed {
                let _ = st.set_limits(env, new);
            }
            changed
        };
        if changed {
            self.emit(EngineEvent::LimitsChanged(env.to_string()));
        }
    }

    /// The env said "limit": a used-up window until the reset (or for a while when the time is unknown).
    /// Returns the time the env is waited for.
    pub(crate) fn record_limit_hit(&self, env: &str, resets: Option<DateTime<Local>>) -> DateTime<Local> {
        let now = self.now();
        let until = resets.unwrap_or(now + CDuration::minutes(LIMIT_WAIT_MINS));
        let w = LimitWindow {
            name: "limit".into(),
            model: None,
            used_pct: None,
            remaining: None,
            resets_at: Some(until),
            exhausted: true,
            source: "limit_event".into(),
            seen_at: now,
        };
        {
            let st = lk(&self.store);
            let mut ws = match st.limits(env) {
                Some(EnvLimits::Known(ws)) => ws,
                _ => vec![],
            };
            // a window the run itself reported as used up already says it
            if !ws.iter().any(|x| x.exhausted && x.resets_at.is_some_and(|r| r >= until)) {
                ws.retain(|x| x.name != "limit");
                ws.push(w);
            }
            let _ = st.set_limits(env, EnvLimits::Known(ws));
        }
        self.emit(EngineEvent::LimitsChanged(env.to_string()));
        until
    }

    /// The task waits for the env to come back; it goes on by itself (the same env, never another).
    pub(super) fn wait_for_env(&self, t: &mut Task, why: &str) -> Result<(), String> {
        let text = format!("{ENV_UNAVAILABLE}: {why}");
        if t.outcome.error.as_deref() != Some(&text) {
            self.log(&t.id, LogKind::Stage, &text);
        }
        t.outcome.error = Some(text);
        t.resume_at = Some(self.now() + CDuration::seconds(ENV_RETRY_SECS));
        self.save(t)
    }

    /// The env asks to log in: every task of it waits, the person is called once per env (G17.2).
    pub(super) fn on_auth(&self, t: &mut Task, a: &dyn agent::Adapter, msg: &str, stage: Status) -> Result<(), String> {
        let first = {
            let mut m = lk(&self.auth);
            let e = m.entry(t.agent.clone()).or_insert(AuthWait { notified: false });
            let first = !e.notified;
            e.notified = true;
            first
        };
        let text = format!("{AUTH_REQUIRED}: {}", a.name());
        let options = vec!["Login".to_string()];
        t.question = Some(Question { text: text.clone(), options: options.clone(), kind: QuestionKind::Agent });
        t.status = Status::NeedsHuman;
        // no fixed login detection: a timer asks again every minute (the engine learns of a login by trying)
        t.resume_at = Some(self.now() + CDuration::seconds(ENV_RETRY_SECS));
        // what the question rests on is written before the status that shows it: whoever sees NeedsHuman (the
        // scheduler closing the env's queue, an answer, a note) finds the stage and the login mark already there
        let mut pend = self.pending(&t.id);
        pend.stage = Some(stage);
        self.set_pending(&t.id, &pend)?;
        self.update_state(&t.id, |s| s.special = Some(SPECIAL_AUTH.to_string()))?;
        self.save(t)?;
        self.log(&t.id, LogKind::Stage, &format!("{text} ({msg})"));
        if first {
            if let Some(p) = self.project(&t.project_id) {
                self.notify(NoticeKind::Question, t, &p, text, options);
            }
        }
        Ok(())
    }

    /// A run worked: the env is logged in.
    fn auth_ok(&self, env: &str) {
        lk(&self.auth).remove(env);
    }

    /// True = the task now waits for the person's decision on the budget (the caller stops).
    pub(super) fn budget_gate(&self, t: &mut Task, p: &Project, a: &dyn agent::Adapter, stage: Status) -> Result<bool, String> {
        if !a.reports_cost() {
            // no bill from this env: only the caps on attempts and rounds act (G15.3)
            if !self.state(&t.id).cost_noted {
                self.update_state(&t.id, |s| s.cost_noted = true)?;
                self.log(&t.id, LogKind::Stage, "cost: env does not report it");
            }
            return Ok(false);
        }
        if let Some((key, text)) = self.budget_breach(t) {
            self.ask_special(t, p, stage, key, text)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// A question the engine asks (budget): buttons, one call.
    fn ask_special(&self, t: &mut Task, p: &Project, stage: Status, key: &str, text: String) -> Result<(), String> {
        let options = vec![format!("Add ${BUDGET_STEP_USD:.0}"), "Stop".to_string()];
        t.question = Some(Question { text: text.clone(), options: options.clone(), kind: QuestionKind::Agent });
        t.status = Status::NeedsHuman;
        let mut pend = self.pending(&t.id);
        pend.stage = Some(stage);
        self.set_pending(&t.id, &pend)?;
        let key = key.to_string();
        self.update_state(&t.id, |s| s.special = Some(key))?;
        self.save(t)?; // after what the answer needs (see `on_auth`)
        self.log(&t.id, LogKind::Stage, &text);
        self.notify(NoticeKind::Question, t, p, text, options);
        Ok(())
    }

    /// A role ended its reply with `NEEDS_HUMAN:`: the task waits, the same role resumes on the answer.
    pub(super) fn ask_role(&self, t: &mut Task, p: &Project, role: RoleKind, stage: Status, q: (String, Vec<String>), session: Option<String>) -> Result<(), String> {
        let (body, options) = (q.0.clone(), q.1.clone());
        t.question = Some(Question { text: q.0, options: q.1, kind: QuestionKind::Agent });
        t.status = Status::NeedsHuman;
        let mut pend = self.pending(&t.id);
        pend.stage = Some(stage);
        pend.prompt = None;
        self.set_pending(&t.id, &pend)?;
        self.update_state(&t.id, |s| {
            s.asked = Some(role);
            s.asked_session = session;
            s.special = None;
        })?;
        self.save(t)?; // after what the answer needs (see `on_auth`)
        self.log(&t.id, LogKind::Stage, "needs human");
        self.notify(NoticeKind::Question, t, p, body, options);
        Ok(())
    }
}
