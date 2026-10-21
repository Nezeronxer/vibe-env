//! The preparatory steps of "Quality" (spec 139): the Planner's breakdown, the spec for vague tasks,
//! the Critic of the plan, the tests written first. None of them can fail a task: one that does not
//! work after one repeat is skipped with the reason in the feed (G09). The gates and the review never are.
use super::flow::{Flow, Res};
use super::roles::{is_read_only, RoleCall, RoleEnd};
use super::*;
use crate::prompt::{self, Brief, RoleInputs, Vendor};

/// The result of an attempt at a structured answer.
pub(super) enum Structured<T> {
    Parsed(T),
    /// Not usable after one repeat; why.
    Unusable(String),
    /// The task waits (question, limit, login, env) or was cancelled: the caller returns `Flow::Stop`.
    Stopped,
}

pub(super) fn first_pending(t: &Task, among: &[Step]) -> Option<Step> {
    t.steps.iter().find(|(s, st)| among.contains(s) && matches!(st, StepState::Pending | StepState::Running)).map(|(s, _)| *s)
}

pub(super) fn step_is(t: &Task, step: Step, pred: impl Fn(&StepState) -> bool) -> bool {
    t.steps.iter().any(|(s, st)| *s == step && pred(st))
}

/// The Brief of a task whose breakdown is not usable: the person's words only.
pub(super) fn plain_brief(t: &Task) -> Brief {
    Brief { goal: t.prompt.clone(), original: t.prompt.clone(), ..Brief::default() }
}

impl Engine {
    pub(super) fn set_step(&self, t: &mut Task, step: Step, st: StepState) -> Result<(), String> {
        match t.steps.iter_mut().find(|(s, _)| *s == step) {
            Some(slot) => slot.1 = st,
            None => t.steps.push((step, st)),
        }
        self.save(t)
    }

    /// A task that arrives without steps (saved by v1): the steps its quality plan asks for.
    pub(super) fn ensure_steps(&self, t: &mut Task, p: &Project) {
        if t.steps.is_empty() {
            t.steps = flow::initial_steps(&t.quality, p);
        }
    }

    pub(super) fn brief_of(&self, t: &Task) -> Brief {
        self.state(&t.id).brief.unwrap_or_else(|| plain_brief(t))
    }

    /// What an answer of the person gives back to the role that asked: its session, and the answer as the prompt.
    fn take_answer(&self, t: &Task, role: RoleKind) -> (Option<String>, Option<String>) {
        let st = self.state(&t.id);
        if st.asked != Some(role) {
            return (None, None);
        }
        let answer = self.pending(&t.id).prompt;
        self.clear_pending(&t.id);
        let _ = self.update_state(&t.id, |s| {
            s.asked = None;
            s.asked_session = None;
        });
        (st.asked_session, answer)
    }

    /// A read-only role whose answer is JSON: one repeat if it cannot be used. Questions of the role are passed on.
    pub(super) fn structured<T>(
        &self,
        t: &mut Task,
        p: &Project,
        role: RoleKind,
        stage: Status,
        schema: String,
        base: &dyn Fn(Vendor, bool) -> String,
        parse: &dyn Fn(&str) -> Result<T, String>,
        cancel: &AtomicBool,
    ) -> Result<Structured<T>, String> {
        debug_assert!(is_read_only(role));
        let (mut resume, answer) = self.take_answer(t, role);
        let mut err = String::new();
        for round in 0..2 {
            let resume_now = resume.take();
            let has_resume = resume_now.is_some();
            let prompt = |v: Vendor, s: bool| match (&answer, has_resume, round) {
                (Some(a), true, 0) => a.clone(),
                (Some(a), false, 0) => format!("{}\n\n{a}", base(v, s)),
                (None, _, 0) => base(v, s),
                _ => prompt::format_retry(&base(v, s), &err),
            };
            let call = RoleCall { role, resume: resume_now, prompt: &prompt, schema: Some(schema.clone()), stage: stage.clone(), shown: None };
            match self.run_role(t, p, call, cancel)? {
                RoleEnd::Done { text, session } => {
                    let sig = prompt::parse_signals(&text);
                    if let Some(q) = sig.needs_human {
                        self.ask_role(t, p, role, stage, q, session)?;
                        return Ok(Structured::Stopped);
                    }
                    match parse(&text) {
                        Ok(v) => return Ok(Structured::Parsed(v)),
                        Err(e) => {
                            self.log(&t.id, LogKind::Stage, &format!("{role:?} answered in wrong format: {e}"));
                            err = e;
                        }
                    }
                }
                RoleEnd::Failed(text) => {
                    err = util::tail(&text, 3);
                    self.log(&t.id, LogKind::Stage, &format!("{role:?} run failed: {err}"));
                }
                RoleEnd::Limit(resets) => {
                    self.on_limit(t, p, resets, stage)?;
                    return Ok(Structured::Stopped);
                }
                RoleEnd::Auth | RoleEnd::Stop | RoleEnd::Cancelled => return Ok(Structured::Stopped),
            }
        }
        Ok(Structured::Unusable(err))
    }

    fn project_hint(&self, t: &Task, p: &Project) -> String {
        let mut names: Vec<String> = t
            .worktree
            .as_ref()
            .and_then(|w| std::fs::read_dir(w).ok())
            .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n != ".git").collect())
            .unwrap_or_default();
        names.sort();
        names.truncate(60);
        format!(
            "Project: {}\nChecks: {}\nTop-level entries: {}",
            p.name,
            p.check_cmd.as_deref().unwrap_or("none"),
            names.join(", ")
        )
    }

    /// Plan step: the Planner breaks the task down (and the app writes the executor's prompt from it).
    pub(super) fn step_plan(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Res {
        self.set_step(t, Step::Plan, StepState::Running)?;
        self.stage(t, "planning");
        let request = prompt::brief_request(&t.prompt, &self.project_hint(t, p));
        let base = |_: Vendor, _: bool| request.clone();
        let r = self.structured(t, p, RoleKind::Planner, Status::Working, prompt::brief_schema(), &base, &prompt::parse_brief, cancel)?;
        match r {
            Structured::Stopped => Ok(Flow::Stop),
            Structured::Unusable(why) => {
                // G14.2: the task goes on with the person's words in the plain template
                self.update_state(&t.id, |s| {
                    s.brief = None;
                    s.brief_error = Some(why.clone());
                })?;
                self.stage(t, &format!("prompt_not_rewritten: {why}"));
                self.set_step(t, Step::Plan, StepState::Skipped(format!("failed: {why}")))?;
                for s in [Step::Spec, Step::Critique] {
                    if step_is(t, s, |st| matches!(st, StepState::Pending)) {
                        self.set_step(t, s, StepState::Skipped("no_plan".into()))?;
                    }
                }
                Ok(Flow::Next)
            }
            Structured::Parsed(mut b) => {
                b.original = t.prompt.clone();
                if !t.quality.plan {
                    b.plan.clear();
                }
                if let Some(why) = &b.risky {
                    // the variants themselves are the swarm's work (task 10): until then one executor, marked
                    let n = t.quality.best_of_n.unwrap_or(2);
                    self.stage(t, &format!("risky: {why} - {n} variants (one executor for now)"));
                }
                let (trivial, vague) = (b.trivial, b.vague);
                self.update_state(&t.id, |s| {
                    s.brief = Some(b);
                    s.brief_error = None;
                })?;
                if trivial {
                    self.stage(t, "plan_skipped: one_phrase_edit");
                    self.set_step(t, Step::Plan, StepState::Skipped("one_phrase_edit".into()))?;
                    if step_is(t, Step::Critique, |st| matches!(st, StepState::Pending)) {
                        self.set_step(t, Step::Critique, StepState::Skipped("one_phrase_edit".into()))?;
                    }
                } else {
                    self.set_step(t, Step::Plan, StepState::Done)?;
                }
                if step_is(t, Step::Spec, |st| matches!(st, StepState::Pending)) && !(vague && !trivial) {
                    self.set_step(t, Step::Spec, StepState::Skipped(if trivial { "one_phrase_edit" } else { "not_vague" }.into()))?;
                }
                Ok(Flow::Next)
            }
        }
    }

    /// Spec step (vague tasks only): the Planner writes the task out and its acceptance criteria.
    pub(super) fn step_spec(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Res {
        let Some(brief) = self.state(&t.id).brief else {
            self.set_step(t, Step::Spec, StepState::Skipped("no_plan".into()))?;
            return Ok(Flow::Next);
        };
        self.set_step(t, Step::Spec, StepState::Running)?;
        self.stage(t, "spec");
        let request = prompt::spec_request(&brief, &t.prompt);
        let base = |_: Vendor, _: bool| request.clone();
        match self.structured(t, p, RoleKind::Planner, Status::Working, prompt::spec_schema(), &base, &prompt::parse_spec, cancel)? {
            Structured::Stopped => Ok(Flow::Stop),
            Structured::Unusable(why) => {
                self.stage(t, &format!("spec skipped: {why}"));
                self.set_step(t, Step::Spec, StepState::Skipped(format!("failed: {why}")))?;
                Ok(Flow::Next)
            }
            Structured::Parsed((spec, done_when)) => {
                self.update_state(&t.id, |s| {
                    if let Some(b) = s.brief.as_mut() {
                        b.spec = Some(spec);
                        if !done_when.is_empty() {
                            b.done_when = done_when;
                        }
                    }
                })?;
                self.set_step(t, Step::Spec, StepState::Done)?;
                Ok(Flow::Next)
            }
        }
    }

    /// Critique step: the Critic reads the task and the plan.
    pub(super) fn step_critique(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Res {
        let brief = self.state(&t.id).brief;
        let Some(brief) = brief.filter(|b| !b.plan.is_empty()) else {
            self.set_step(t, Step::Critique, StepState::Skipped("no_plan".into()))?;
            return Ok(Flow::Next);
        };
        self.set_step(t, Step::Critique, StepState::Running)?;
        self.stage(t, "critique");
        let b2 = brief.clone();
        let base = move |v: Vendor, _: bool| prompt::render(v, RoleKind::Critic, &b2, &RoleInputs::default());
        match self.structured(t, p, RoleKind::Critic, Status::Working, prompt::critique_schema(), &base, &prompt::parse_critique, cancel)? {
            Structured::Stopped => Ok(Flow::Stop),
            Structured::Unusable(why) => {
                self.stage(t, &format!("critique skipped: {why}"));
                self.set_step(t, Step::Critique, StepState::Skipped(format!("failed: {why}")))?;
                Ok(Flow::Next)
            }
            Structured::Parsed(c) => {
                self.update_state(&t.id, |s| {
                    if let Some(b) = s.brief.as_mut() {
                        if !c.plan.is_empty() {
                            b.plan = c.plan.clone();
                        }
                        b.constraints.extend(c.issues.iter().map(|i| format!("Plan review: {i}")));
                    }
                })?;
                if !c.ok || !c.issues.is_empty() {
                    self.stage(t, &format!("critique: {} remarks", c.issues.len()));
                }
                self.set_step(t, Step::Critique, StepState::Done)?;
                Ok(Flow::Next)
            }
        }
    }

    /// Tests-first step: the test author writes the tests, the app commits them and remembers which
    /// files are theirs, so the executor cannot rewrite them (G09.1).
    pub(super) fn step_tests_first(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Res {
        let wt = t.worktree.clone().ok_or("no_worktree")?;
        self.set_step(t, Step::TestsFirst, StepState::Running)?;
        self.stage(t, "writing tests first");
        let brief = self.brief_of(t);
        let (mut resume, answer) = self.take_answer(t, RoleKind::TestAuthor);
        let mut err = String::new();
        for round in 0..2 {
            let b = brief.clone();
            let resume_now = resume.take();
            let has_resume = resume_now.is_some();
            let ans = answer.clone().filter(|_| round == 0);
            let prompt = move |v: Vendor, _: bool| {
                let base = prompt::render(v, RoleKind::TestAuthor, &b, &RoleInputs::default());
                match (&ans, has_resume) {
                    (Some(a), true) => a.clone(),
                    (Some(a), false) => format!("{base}\n\n{a}"),
                    (None, _) => base,
                }
            };
            let call = RoleCall { role: RoleKind::TestAuthor, resume: resume_now, prompt: &prompt, schema: None, stage: Status::Working, shown: None };
            match self.run_role(t, p, call, cancel)? {
                RoleEnd::Done { text, session } => {
                    let sig = prompt::parse_signals(&text);
                    if let Some(q) = sig.needs_human {
                        self.ask_role(t, p, RoleKind::TestAuthor, Status::Working, q, session)?;
                        return Ok(Flow::Stop);
                    }
                    let sha = git::commit_all(&wt, "tests").map_err(|e| e.to_string())?;
                    let files: Vec<String> = git::diffstat(&wt, &p.base_branch).into_iter().map(|f| f.path).collect();
                    let Some(sha) = sha.filter(|_| !files.is_empty()) else {
                        self.stage(t, "tests skipped: no_tests_written");
                        self.set_step(t, Step::TestsFirst, StepState::Skipped("no_tests_written".into()))?;
                        return Ok(Flow::Next);
                    };
                    self.log(&t.id, LogKind::Git, &format!("tests first: {}", files.join(", ")));
                    self.update_state(&t.id, |s| {
                        s.test_files = files;
                        s.tests_commit = Some(sha);
                    })?;
                    self.set_step(t, Step::TestsFirst, StepState::Done)?;
                    return Ok(Flow::Next);
                }
                RoleEnd::Failed(text) => {
                    err = util::tail(&text, 3);
                    self.log(&t.id, LogKind::Stage, &format!("TestAuthor run failed: {err}"));
                }
                RoleEnd::Limit(resets) => {
                    self.on_limit(t, p, resets, Status::Working)?;
                    return Ok(Flow::Stop);
                }
                RoleEnd::Auth | RoleEnd::Stop | RoleEnd::Cancelled => return Ok(Flow::Stop),
            }
        }
        self.stage(t, &format!("tests skipped: {err}"));
        self.set_step(t, Step::TestsFirst, StepState::Skipped(format!("failed: {err}")))?;
        Ok(Flow::Next)
    }
}
