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
pub(super) const AUTH_REQUIRED: &str = "auth_required";
pub(super) const ENV_UNAVAILABLE: &str = "env_unavailable";
pub(super) const WAITING_LIMIT: &str = "waiting_limit_reset";
pub(super) const BUDGET_EXCEEDED: &str = "budget_exceeded";
pub(super) const DAY_BUDGET_EXCEEDED: &str = "day_budget_exceeded";
pub(super) const SPECIAL_BUDGET: &str = "budget";
pub(super) const SPECIAL_DAY_BUDGET: &str = "day_budget";
pub(super) const SPECIAL_AUTH: &str = "auth";
/// "Add $5" on a budget question.
pub(super) const BUDGET_STEP_USD: f64 = 5.0;
/// A limit with no known reset time is waited out this long.
pub(super) const LIMIT_WAIT_MINS: i64 = 30;
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

fn vendor_name(v: Vendor) -> &'static str {
    match v {
        Vendor::Anthropic => "anthropic",
        Vendor::OpenAi => "openai",
        Vendor::Google => "google",
        Vendor::Alibaba => "qwen",
        Vendor::Generic => "generic",
    }
}

struct Picked {
    cli_arg: Option<String>,
    label: String,
    vendor: Vendor,
    note: Option<ModelNote>,
}

/// The model of a choice in a catalog: the CLI argument, the caption, and whose template it gets.
fn pick(cat: &Catalog, env: &str, choice: Option<&ModelChoice>) -> Picked {
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
    fn choice_for(t: &Task, role: RoleKind) -> Option<ModelChoice> {
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
        let notes = self.take_notes(&t.id, role);
        let state = self.state(&t.id);

        let mut resume = call.resume.clone();
        let mut switched = false;
        let mut choice = Self::choice_for(t, role);
        let mut pinned: Option<Pin> = resume.as_ref().and_then(|_| state.pinned.get(&role).cloned());
        // the limit is known to be used up and there is another model to go on with: start there
        if let (Some(fb), Some(_)) = (env_set.fallback.clone(), self.limit_wait(&env, self.now())) {
            if choice.as_ref() != Some(&fb) {
                self.log(&t.id, LogKind::Stage, &format!("switched_to: {}", pick(&cat, &env, Some(&fb)).label));
                switched = resume.is_some() || (role == RoleKind::Worker && t.session.is_some());
                choice = Some(fb.clone());
                t.models.insert(role, fb);
                resume = None;
                pinned = None;
                if role == RoleKind::Worker {
                    t.session = None;
                }
            }
        }
        let mut announced = false;
        let mut tried_fallback = false;
        for _ in 0..8 {
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
            let mut text = (call.prompt)(vendor, switched);
            if let Some((s, true)) = call.shown.and_then(|c| c.borrow().clone()) {
                t.rendered_prompt = Some(s);
            }
            if !notes.is_empty() {
                text = format!("{text}\n\n{}", prompt::notes_block(&notes));
            }
            let system = prompt::system_for(vendor, role);

            // where it runs: writers in the task's copy; readers through the env's flag or in a throwaway copy
            let ro = is_read_only(role);
            let wt = t.worktree.clone().ok_or("no_worktree")?;
            let copy = if ro && !a.read_only_native() { Some(self.ro_copy(t, p, role)?) } else { None };
            let mut run = Run::new(copy.clone().unwrap_or(wt), text, system, role);
            run.resume = resume.clone();
            run.model = cli_arg.clone();
            run.read_only = ro;
            run.extra_env = lk(&self.env_vars).as_ref().map(|f| f(&env)).unwrap_or_default();
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
            let events = self.run_agent(t, &*a, &run, &label, cancel);
            if let Some(dir) = copy {
                self.drop_ro(p, &dir);
            }
            let Some(events) = events else {
                t.runs[idx].state = StepState::Failed("interrupted".into());
                t.runs[idx].finished_at = Some(self.now());
                let _ = self.save(t);
                return Ok(RoleEnd::Cancelled);
            };

            // what the run reported
            let mut cost = 0.0;
            let mut has_cost = false;
            let mut windows = None;
            let (mut limit, mut rejected, mut auth, mut done) = (None, None, None, None);
            for e in &events {
                match e {
                    AgentEvent::Usage(w) => windows = Some(w.clone()),
                    AgentEvent::Cost(c) => {
                        cost += c;
                        has_cost = true;
                    }
                    AgentEvent::Limit { resets_at } => limit = Some(*resets_at),
                    AgentEvent::ModelRejected(m) => rejected = Some(m.clone()),
                    AgentEvent::AuthRequired(m) => auth = Some(m.clone()),
                    AgentEvent::Done { session, text, is_error } => done = Some((session.clone(), text.clone(), *is_error)),
                    _ => {}
                }
            }
            if let Some(w) = &windows {
                self.record_usage(&env, w.clone());
            }
            if has_cost {
                t.cost_usd = Some(t.cost_usd.unwrap_or(0.0) + cost);
                t.runs[idx].cost_usd = Some(cost);
                lk(&self.store).add_day_spend(&env, self.now().date_naive(), cost, 0.0).map_err(|e| e.to_string())?;
                self.log(&t.id, LogKind::Stage, &format!("{COST_LOG}{cost:.4} ({role:?})"));
            }
            let session = done.as_ref().and_then(|d| d.0.clone());
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

            if let Some(msg) = auth {
                t.runs[idx].state = StepState::Failed("auth_required".into());
                self.on_auth(t, &*a, &msg, call.stage.clone())?;
                return Ok(RoleEnd::Auth);
            }
            if let Some(msg) = rejected {
                // not an attempt: the same role again on the next model of the chain (G18.4)
                t.runs[idx].state = StepState::Failed("model_rejected".into());
                let cur = choice.clone().unwrap_or(ModelChoice { family: models::DEFAULT_FAMILY.into(), version: None });
                let Some(next) = models::next_on_reject(&cat, &cur) else {
                    self.save(t)?;
                    return Ok(RoleEnd::Failed(format!("model_rejected: {msg}")));
                };
                let to = pick(&cat, &env, Some(&next)).label;
                self.log(&t.id, LogKind::Stage, &format!("model_rejected: {label} -> {to}"));
                t.models.insert(role, next.clone());
                choice = Some(next);
                pinned = None;
                resume = None;
                continue;
            }
            if let Some(resets) = limit {
                t.runs[idx].state = StepState::Failed("limit".into());
                // the time comes from the message, else from a used-up window of this run, else from one stored earlier
                let resets = resets
                    .or_else(|| windows.as_ref().and_then(|w| w.iter().filter(|x| x.exhausted).filter_map(|x| x.resets_at).max()))
                    .or_else(|| self.limit_wait(&env, self.now()));
                self.record_limit_hit(&env, resets);
                // what the writer did so far is committed, so whoever goes on sees it in the diff
                if !is_read_only(role) {
                    if let Some(w) = t.worktree.as_ref().filter(|w| !git::rebase_in_progress(w)) {
                        git::commit_all(w, &t.title).map_err(|e| e.to_string())?;
                    }
                }
                if let Some(fb) = env_set.fallback.clone().filter(|f| choice.as_ref() != Some(f) && !tried_fallback) {
                    // A05: go on with the other model of this env, in the same branch, with a fresh session
                    tried_fallback = true;
                    let to = pick(&cat, &env, Some(&fb)).label;
                    self.log(&t.id, LogKind::Stage, &format!("switched_to: {to}"));
                    t.models.insert(role, fb.clone());
                    choice = Some(fb);
                    pinned = None;
                    resume = None;
                    switched = true;
                    if role == RoleKind::Worker {
                        t.session = None;
                    }
                    continue;
                }
                self.save(t)?;
                return Ok(RoleEnd::Limit(resets));
            }
            return match done {
                Some((_, text, false)) => {
                    t.runs[idx].state = StepState::Done;
                    self.save(t)?;
                    self.auth_ok(&env);
                    if state.special.as_deref() == Some(SPECIAL_AUTH) {
                        self.update_state(&t.id, |s| s.special = None)?; // the login worked: the task no longer waits for it
                    }
                    Ok(RoleEnd::Done { text, session })
                }
                Some((_, text, true)) => {
                    t.runs[idx].state = StepState::Failed(util::tail(&text, 3));
                    self.save(t)?;
                    Ok(RoleEnd::Failed(text))
                }
                None => {
                    t.runs[idx].state = StepState::Failed("no result".into());
                    self.save(t)?;
                    Ok(RoleEnd::Failed("agent ended without a result".into()))
                }
            };
        }
        Err("model_rejected: no model of the env was accepted".into())
    }

    /// Runs an agent, logging every event tagged with its role and model. None = cancelled.
    fn run_agent(&self, t: &Task, a: &dyn agent::Adapter, run: &Run, model: &str, cancel: &AtomicBool) -> Option<Vec<AgentEvent>> {
        let id = t.id.clone();
        let role = run.role;
        let kind = if role == RoleKind::Reviewer { LogKind::Review } else { LogKind::Agent };
        let tag = |k: LogKind, text: &str| {
            let mut ev = LogEvent::new(self.now(), k, text);
            ev.role = Some(role);
            ev.model = Some(model.to_string());
            self.log_ev(&id, ev);
        };
        let mut on = |e: &AgentEvent| match e {
            AgentEvent::Text(s) => tag(kind.clone(), s),
            AgentEvent::Tool { name, summary } => tag(LogKind::Tool, &format!("{name}: {summary}")),
            AgentEvent::Done { text, is_error, .. } => tag(if *is_error { LogKind::Error } else { kind.clone() }, text),
            AgentEvent::Limit { .. } => tag(LogKind::Stage, "usage limit reached"),
            AgentEvent::Usage(_) | AgentEvent::Cost(_) => {}
            AgentEvent::ModelRejected(m) | AgentEvent::AuthRequired(m) => tag(LogKind::Error, m),
        };
        let res = agent::run_blocking(a, run, &mut on, cancel);
        if cancel.load(Ordering::SeqCst) {
            return None;
        }
        Some(res.unwrap_or_else(|e| {
            let ev = AgentEvent::Done { session: None, text: format!("{}: {e}", a.id()), is_error: true };
            on(&ev);
            vec![ev]
        }))
    }

    // ---- read-only copies ----

    fn ro_copy(&self, t: &Task, p: &Project, role: RoleKind) -> Result<PathBuf, String> {
        let dir = self.worktrees_dir.join(".ro").join(format!("{}-{}", &t.id[..t.id.len().min(8)], &uuid::Uuid::new_v4().simple().to_string()[..8]));
        // the reviewer and the verifier look at the task's work, the others at the project as it is on base
        let at = if matches!(role, RoleKind::Reviewer | RoleKind::Verifier) { t.branch.as_str() } else { p.base_branch.as_str() };
        git::create_readonly_copy(&p.path, &dir, at).map_err(|e| e.to_string())
    }

    fn drop_ro(&self, p: &Project, dir: &std::path::Path) {
        let _ = git::remove_worktree(&p.path, dir, false);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Throwaway copies left by a crash are gone at start.
    pub(super) fn sweep_ro_copies(&self) {
        let ro = self.worktrees_dir.join(".ro");
        if !ro.exists() {
            return;
        }
        let _ = std::fs::remove_dir_all(&ro);
        for p in lk(&self.store).projects() {
            let _ = git::remove_worktree(&p.path, &ro.join("none"), false); // prunes what is gone
        }
    }

    // ---- limits, cost, login, env ----

    fn record_usage(&self, env: &str, windows: Vec<LimitWindow>) {
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
    fn record_limit_hit(&self, env: &str, resets: Option<DateTime<Local>>) {
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
    }

    /// The task waits for the env to come back; it goes on by itself (the same env, never another).
    fn wait_for_env(&self, t: &mut Task, why: &str) -> Result<(), String> {
        let text = format!("{ENV_UNAVAILABLE}: {why}");
        if t.outcome.error.as_deref() != Some(&text) {
            self.log(&t.id, LogKind::Stage, &text);
        }
        t.outcome.error = Some(text);
        t.resume_at = Some(self.now() + CDuration::seconds(ENV_RETRY_SECS));
        self.save(t)
    }

    /// The env asks to log in: every task of it waits, the person is called once per env (G17.2).
    fn on_auth(&self, t: &mut Task, a: &dyn agent::Adapter, msg: &str, stage: Status) -> Result<(), String> {
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
        self.save(t)?;
        let mut pend = self.pending(&t.id);
        pend.stage = Some(stage);
        self.set_pending(&t.id, &pend)?;
        self.update_state(&t.id, |s| s.special = Some(SPECIAL_AUTH.to_string()))?;
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
    fn budget_gate(&self, t: &mut Task, p: &Project, a: &dyn agent::Adapter, stage: Status) -> Result<bool, String> {
        if !a.reports_cost() {
            // no bill from this env: only the caps on attempts and rounds act (G15.3)
            if !self.state(&t.id).cost_noted {
                self.update_state(&t.id, |s| s.cost_noted = true)?;
                self.log(&t.id, LogKind::Stage, "cost: env does not report it");
            }
            return Ok(false);
        }
        let spent = t.cost_usd.unwrap_or(0.0);
        if let Some(b) = t.budget_usd.filter(|b| spent >= *b) {
            self.ask_special(t, p, stage, SPECIAL_BUDGET, format!("{BUDGET_EXCEEDED}: {spent:.2} of {b:.2}"))?;
            return Ok(true);
        }
        let settings = lk(&self.store).settings();
        let day = lk(&self.store).day_spend(&t.agent, self.now().date_naive());
        let cap = settings.limits.day_usd + day.extra_usd;
        let today = day.spent_usd;
        if today >= cap {
            self.ask_special(t, p, stage, SPECIAL_DAY_BUDGET, format!("{DAY_BUDGET_EXCEEDED}: {today:.2} of {cap:.2}"))?;
            return Ok(true);
        }
        Ok(false)
    }

    /// A question the engine asks (budget): buttons, one call.
    fn ask_special(&self, t: &mut Task, p: &Project, stage: Status, key: &str, text: String) -> Result<(), String> {
        let options = vec![format!("Add ${BUDGET_STEP_USD:.0}"), "Stop".to_string()];
        t.question = Some(Question { text: text.clone(), options: options.clone(), kind: QuestionKind::Agent });
        t.status = Status::NeedsHuman;
        self.save(t)?;
        let mut pend = self.pending(&t.id);
        pend.stage = Some(stage);
        self.set_pending(&t.id, &pend)?;
        let key = key.to_string();
        self.update_state(&t.id, |s| s.special = Some(key))?;
        self.log(&t.id, LogKind::Stage, &text);
        self.notify(NoticeKind::Question, t, p, text, options);
        Ok(())
    }

    /// A role ended its reply with `NEEDS_HUMAN:`: the task waits, the same role resumes on the answer.
    pub(super) fn ask_role(&self, t: &mut Task, p: &Project, role: RoleKind, stage: Status, q: (String, Vec<String>), session: Option<String>) -> Result<(), String> {
        let (body, options) = (q.0.clone(), q.1.clone());
        t.question = Some(Question { text: q.0, options: q.1, kind: QuestionKind::Agent });
        t.status = Status::NeedsHuman;
        self.save(t)?;
        let mut pend = self.pending(&t.id);
        pend.stage = Some(stage);
        pend.prompt = None;
        self.set_pending(&t.id, &pend)?;
        self.update_state(&t.id, |s| {
            s.asked = Some(role);
            s.asked_session = session;
            s.special = None;
        })?;
        self.log(&t.id, LogKind::Stage, "needs human");
        self.notify(NoticeKind::Question, t, p, body, options);
        Ok(())
    }
}
