//! The per-task state machine, run on the thread holding the task's claim:
//! queued → preparing → working (plan, spec, critique, tests, code) ⇄ checking (gates, e2e) →
//! reviewing → merging → done / awaiting_outward / ...
//! `Task.status` stays the v1 set (31 e2e tests stand on it); where the task is inside a stage is `Task.steps`.
use super::roles::{RoleCall, RoleEnd};
use super::steps::{first_pending, step_is};
use super::util::{self, *};
use super::{Engine, Pending};
use crate::git::{self, GitError};
use crate::model::*;
use crate::notify::NoticeKind;
use crate::prompt::{self, Conflict, RoleInputs, Vendor};
use chrono::{DateTime, Duration as CDuration, Local};
use std::cell::RefCell;

/// The most of the worker's report kept on the task (the commit takes its first paragraph, story 292).
pub(super) const SUMMARY_MAX: usize = 4000;
use std::sync::atomic::{AtomicBool, Ordering};

const MAX_AGENT_ERRORS: u32 = 3;
const DIFF_FOR_REVIEW: usize = 60_000;
const MERGE_RETRY_SECS: i64 = 60;
const INTEGRATOR_ROUNDS: u32 = 5;
/// Outward question keys (UI translates).
pub(super) const PUSH_TO_ORIGIN: &str = "push_to_origin";
pub(super) const OPEN_PR: &str = "open_pr";
pub(super) const PUSH_REVERT: &str = "push_revert";
/// `Outcome.error` of a PR task that was pushed but whose PR was not opened ("Только push").
pub(super) const PR_NOT_OPENED: &str = "pr_not_opened";

/// After a step: keep driving, or the task now waits (human, timer) / is finished / was cancelled.
pub(super) enum Flow {
    Next,
    Stop,
}

pub(super) type Res = Result<Flow, String>;

pub(super) fn gerr(e: GitError) -> String {
    let s = e.to_string();
    // "git: hidden_list_unreadable: ..." is a key for the person, not a git message
    s.strip_prefix("git: hidden_list_unreadable").map(|r| format!("hidden_list_unreadable{r}")).unwrap_or(s)
}

/// The steps a task goes through. With "Quality" on, the preparatory ones are there too (G10);
/// a step that cannot apply is skipped from the start, with its reason (G24.5).
pub(super) fn initial_steps(q: &QualityPlan, p: &Project) -> Vec<(Step, StepState)> {
    use StepState::{Pending, Skipped};
    if !q.on {
        return vec![(Step::Code, Pending), (Step::Gates, Pending), (Step::Review, Pending), (Step::Merge, Pending)];
    }
    let off = |on: bool, why: &str| if on { Pending } else { Skipped(why.to_string()) };
    let tests = if !q.tests_first {
        Skipped("disabled".into())
    } else if p.check_cmd.as_deref().is_none_or(|c| c.trim().is_empty()) {
        Skipped("no_test_runner".into())
    } else {
        Pending
    };
    let e2e = if !q.e2e {
        Skipped("e2e_off".into())
    } else if p.run_cmd.as_deref().is_none_or(|c| c.trim().is_empty()) {
        Skipped("no_run_cmd".into())
    } else {
        Pending
    };
    vec![
        (Step::Plan, Pending),
        (Step::Spec, off(q.spec, "disabled")),
        (Step::Critique, off(q.plan, "disabled")),
        (Step::TestsFirst, tests),
        (Step::Code, Pending),
        (Step::Gates, Pending),
        (Step::E2e, e2e),
        (Step::Review, Pending),
        (Step::Merge, Pending),
    ]
}

/// One line: what was done, files, tests, review, commit.
pub(super) fn summary_line(t: &Task) -> String {
    let o = &t.outcome;
    let mut parts = vec![o.summary.as_deref().and_then(|s| s.lines().next()).unwrap_or(&t.title).to_string()];
    parts.push(format!("{} files +{} -{}", o.files_changed, o.insertions, o.deletions));
    parts.push(match &o.tests {
        None => "no tests (reviewer only)".into(),
        Some(r) if r.passed => "tests passed".into(),
        Some(_) => "tests failed".into(),
    });
    if let Some(r) = &o.review {
        parts.push(if r.approved { "review approved".into() } else { format!("review: {} issues", r.issues.len()) });
    }
    if let Some(c) = &o.commit {
        parts.push(c.chars().take(7).collect());
    }
    if let Some(u) = &o.pr_url {
        parts.push(u.clone());
    }
    if t.status == Status::DoneNotPushed {
        parts.push(format!("not pushed: {}", o.error.as_deref().unwrap_or("")));
    } else if o.error.as_deref() == Some(PR_NOT_OPENED) {
        parts.push("PR not opened".into());
    }
    parts.join(" · ")
}

impl Engine {
    pub(super) fn drive(&self, id: &str, cancel: &AtomicBool) {
        loop {
            if cancel.load(Ordering::SeqCst) {
                return;
            }
            let Some(mut t) = self.task(id) else { return };
            let Some(p) = self.project(&t.project_id) else {
                self.fail(&mut t, "", "project_not_found".into());
                return;
            };
            if super::lk(&self.store).settings().paused {
                return; // the stage is on disk; the scheduler resumes it when unpaused
            }
            // frozen: a merge that began is carried to its end, nothing else goes on (R18i.1)
            if self.frozen.load(Ordering::SeqCst) && t.status != Status::Merging {
                return;
            }
            let r = match t.status {
                // the split of a convoy is a run of the Coordinator in a task of its own, not a coding task
                Status::Queued | Status::Preparing | Status::Working if super::convoy::is_plan(&t) => self.convoy_plan(&mut t, &p, cancel),
                Status::Queued | Status::Preparing => self.prepare(&mut t, &p, cancel),
                Status::Working => self.work(&mut t, &p, cancel),
                Status::Checking => self.check_step(&mut t, &p, cancel),
                Status::Reviewing => self.review(&mut t, &p, cancel),
                Status::Merging => self.merge(&mut t, &p, cancel),
                _ => return,
            };
            if cancel.load(Ordering::SeqCst) {
                return; // cancel / shutdown / freeze own the task state from here
            }
            match r {
                Ok(Flow::Next) => {}
                Ok(Flow::Stop) => return,
                Err(e) => {
                    self.fail(&mut t, &p.name, e);
                    return;
                }
            }
        }
    }

    pub(super) fn set(&self, t: &mut Task, s: Status) -> Result<(), String> {
        t.status = s;
        self.save(t)
    }

    pub(super) fn stage(&self, t: &Task, text: &str) {
        self.log(&t.id, LogKind::Stage, text);
    }

    /// Next worker prompt, written before the status that uses it.
    fn next_prompt(&self, t: &Task, prompt: String) -> Result<(), String> {
        self.set_pending(&t.id, &Pending { prompt: Some(prompt), stage: Some(Status::Working), ..Default::default() })
    }

    /// The executor has to do its step again (red gates, review notes): the later steps wait for it.
    fn back_to_code(&self, t: &mut Task) -> Result<(), String> {
        for (s, st) in t.steps.iter_mut() {
            if matches!(s, Step::Code | Step::Gates | Step::E2e | Step::Review | Step::Merge) && !matches!(st, StepState::Skipped(_)) {
                *st = StepState::Pending;
            }
        }
        self.set(t, Status::Working)
    }

    /// Failed: the worktree stays for inspection until remove / retry / resume.
    pub(super) fn fail(&self, t: &mut Task, project: &str, why: String) {
        t.status = Status::Failed;
        t.outcome.error = Some(why.clone());
        t.finished_at = Some(self.now());
        for (_, st) in t.steps.iter_mut() {
            if matches!(st, StepState::Running) {
                *st = StepState::Failed(why.clone());
            }
        }
        let _ = self.save(t);
        self.clear_pending(&t.id); // the breakdown and the notes stay: "resume" needs them
        self.log(&t.id, LogKind::Error, &why);
        self.notifier.send(&crate::notify::Notice {
            kind: NoticeKind::Failed,
            task_id: t.id.clone(),
            project: project.to_string(),
            title: t.title.clone(),
            body: why,
            options: vec![],
        });
    }

    fn prepare(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Res {
        let began = std::time::Instant::now();
        t.started_at.get_or_insert(self.now());
        self.note_v1_settings(p);
        // put off by the agent (DEFERRED) and released: the copy and the session are as they were left
        if self.state(&t.id).deferred && t.worktree.as_ref().is_some_and(|w| w.exists()) {
            self.update_state(&t.id, |s| s.deferred = false)?;
            self.stage(t, "deferred task released");
            self.set(t, Status::Working)?;
            return Ok(Flow::Next);
        }
        self.set(t, Status::Preparing)?;
        self.stage(t, "preparing");
        self.clear_pending(&t.id);
        // notes the person left while the task was queued survive the fresh start
        // one write: a note given right now from another thread is not lost between a read and a clear
        self.update_state(&t.id, |s| {
            let notes: Vec<_> = std::mem::take(&mut s.notes).into_iter().filter(|n| !n.delivered).collect();
            *s = Default::default();
            s.notes = notes;
        })?;
        if t.outcome.error.as_deref().is_some_and(|e| e.starts_with("waiting_") || e.starts_with("env_unavailable") || e.starts_with("dependency_blocked")) {
            t.outcome.error = None;
        }
        self.ensure_steps(t, p);
        // interrupted earlier: prepare again from scratch
        if let Some(wt) = t.worktree.take() {
            self.rm_wt(&p.path, &wt, true).map_err(gerr)?;
        }
        let wt = self.add_wt(&p.path, &self.worktrees_dir, &t.id, &t.branch, &p.base_branch).map_err(gerr)?;
        t.worktree = Some(wt.clone());
        self.save(t)?;
        self.log(&t.id, LogKind::Git, &format!("worktree {} on {}", wt.display(), t.branch));
        let mut copied = util::copy_files(&p.path, &wt, &p.copy_files);
        if !copied.is_empty() {
            self.stage(t, &format!("copied {}", copied.join(", ")));
        }
        // never part of a commit, tracked by git ignore rules or not; the app's own folder too (hand-off, raw output)
        copied.push(super::practices::APP_DIR.to_string());
        git::hide_untracked(&wt, &copied).map_err(gerr)?;
        // one step before the agent (story 311): files, ports and database, the setup command
        self.alloc_ports(t, p)?;
        if let (Some(port), Some(db)) = (t.port, &t.db_name) {
            self.stage(t, &format!("ports: {port}, db: {db}"));
        }
        if let Some(cmd) = p.setup_cmd.as_deref().filter(|c| !c.trim().is_empty()) {
            self.stage(t, &format!("setup: {cmd}"));
            let Some((ok, out)) = sh(&wt, cmd, &Self::copy_env(t), cancel) else { return Ok(Flow::Stop) };
            if !ok {
                return Err(format!("setup_failed: {cmd}\n{}", tail(&out, 60)));
            }
        }
        self.stage(t, &format!("prepared_in: {}s", began.elapsed().as_secs()));
        self.set(t, Status::Working)?;
        Ok(Flow::Next)
    }

    /// The first step of the stage that is not done: the preparatory ones, then the code.
    fn work(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Res {
        self.ensure_steps(t, p);
        if !t.quality.on && !self.state(&t.id).quality_off_noted {
            // G14: with "Quality" off the executor gets the person's words as they are; the feed says so, once
            self.update_state(&t.id, |s| s.quality_off_noted = true)?;
            self.stage(t, "prompt_not_rewritten: quality_off");
        }
        if t.quality.on {
            match first_pending(t, &[Step::Plan, Step::Spec, Step::Critique, Step::TestsFirst]) {
                Some(Step::Plan) => return self.step_plan(t, p, cancel),
                Some(Step::Spec) => return self.step_spec(t, p, cancel),
                Some(Step::Critique) => return self.step_critique(t, p, cancel),
                Some(Step::TestsFirst) => return self.step_tests_first(t, p, cancel),
                _ => {}
            }
        }
        if let Some(r) = self.swarm_code(t, p, cancel) {
            return r;
        }
        self.step_code(t, p, cancel)
    }

    /// The executor writes the code. Its prompt is made when its run starts, for the vendor of the model
    /// that starts (G14.3); a session that exists is resumed with what is pending for it.
    fn step_code(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Res {
        let wt = t.worktree.clone().ok_or("no_worktree")?;
        let st = self.state(&t.id);
        let pend = self.pending(&t.id);
        let settings = super::lk(&self.store).settings();
        self.set_step(t, Step::Code, StepState::Running)?;
        self.stage(t, &format!("working ({})", t.agent));
        if st.asked == Some(RoleKind::Worker) {
            self.update_state(&t.id, |s| {
                s.asked = None;
                s.asked_session = None;
            })?;
        }

        let has_session = t.session.is_some();
        let brief = if t.quality.on { st.brief.clone() } else { None };
        let quality_on = t.quality.on;
        let tests_exist = p.check_cmd.as_deref().is_some_and(|c| !c.trim().is_empty());
        let test_files = st.test_files.clone();
        let user_text = t.prompt.clone();
        let rewrite_err = st.brief_error.clone();
        let (wt2, base) = (wt.clone(), p.base_branch.clone());
        let continue_next = st.continue_next;
        let last_prompt = RefCell::new(None::<(String, bool)>);
        let initial_plain = |v: Vendor| -> String {
            if !quality_on {
                return user_text.clone();
            }
            match &brief {
                Some(b) => prompt::render(v, RoleKind::Worker, b, &RoleInputs { tests_exist, tests_readonly: test_files.clone(), ..Default::default() }),
                None => prompt::render_simple(&user_text, rewrite_err.as_deref().unwrap_or("no breakdown")),
            }
        };
        let hint = st.worker_hint.clone();
        let initial = |v: Vendor| -> String {
            let text = initial_plain(v);
            match &hint {
                Some(h) => format!("{text}\n\n{h}"),
                None => text,
            }
        };
        let prompt_fn = |v: Vendor, switched: bool| -> String {
            let fresh = switched || continue_next;
            let text = if fresh {
                // another agent (model) goes on with what was begun: the task and what is done so far
                let b = brief.clone().unwrap_or_else(|| prompt::Brief { goal: user_text.clone(), original: user_text.clone(), ..Default::default() });
                let inputs = RoleInputs {
                    diff: Some(git::diff(&wt2, &base, DIFF_FOR_REVIEW)),
                    continue_work: true,
                    tests_exist,
                    tests_readonly: test_files.clone(),
                    ..Default::default()
                };
                let mut s = prompt::render(v, RoleKind::Worker, &b, &inputs);
                if let Some(last) = pend.prompt.as_deref().filter(|l| *l != prompt::CONTINUE) {
                    s.push_str(&format!("\n\nThe last request to the previous agent, still to do:\n\n{last}"));
                }
                s
            } else if let Some(pr) = &pend.prompt {
                if has_session { pr.clone() } else { format!("{}\n\n{pr}", initial(v)) }
            } else if has_session {
                prompt::CONTINUE.to_string()
            } else {
                initial(v)
            };
            *last_prompt.borrow_mut() = Some((text.clone(), fresh || !has_session));
            text
        };
        let shown = quality_on.then_some(&last_prompt);
        let call = RoleCall { role: RoleKind::Worker, resume: t.session.clone(), prompt: &prompt_fn, schema: None, stage: Status::Working, shown };
        let end = self.run_role(t, p, call, cancel)?;
        let text = match end {
            RoleEnd::Done { text, .. } => text,
            RoleEnd::Cancelled | RoleEnd::Stop | RoleEnd::Auth => return Ok(Flow::Stop),
            RoleEnd::Limit(resets) => return self.on_limit(t, p, resets, Status::Working),
            RoleEnd::Failed(text) => {
                // same prompt again, same session; counted apart from the check rounds
                let mut pend = self.pending(&t.id);
                pend.agent_errors += 1;
                if pend.agent_errors >= MAX_AGENT_ERRORS {
                    return Err(format!("agent_failed: {}", tail(&text, 20)));
                }
                self.set_pending(&t.id, &pend)?;
                self.save(t)?;
                return Ok(Flow::Next);
            }
        };
        if continue_next {
            self.update_state(&t.id, |s| s.continue_next = false)?;
        }
        // the prompt is cleared below only once the next status is saved
        let sig = prompt::parse_signals(&text);
        if !sig.body.is_empty() {
            t.outcome.summary = Some(sig.body.chars().take(SUMMARY_MAX).collect());
        }
        if self.g_uncommit(t, &wt, &p.base_branch).map_err(gerr)? {
            self.log(&t.id, LogKind::Git, "agent committed a copied local file or a key; its commits were folded back without it");
        }
        let kept_out = if self.protects(t) { git::protected_pending(&wt) } else { vec![] };
        if !kept_out.is_empty() {
            self.log(&t.id, LogKind::Git, &format!("protected_paths_not_committed: {}", kept_out.join(", ")));
        }
        self.g_commit(t, &wt, &t.title).map_err(gerr)?;
        if let Some(reason) = sig.deferred {
            if self.defer(t, &reason)? {
                return Ok(Flow::Stop);
            }
        }
        if let Some(q) = sig.needs_human {
            let session = t.session.clone();
            self.ask_role(t, p, RoleKind::Worker, Status::Working, q, session)?;
            return Ok(Flow::Stop);
        }
        // the tests of the test author are not the executor's to change (G09.1)
        let touched = self.protect_tests(t, &wt)?;
        if !touched.is_empty() {
            t.attempts += 1;
            self.stage(t, &format!("tests_read_only: {}", touched.join(", ")));
            if t.attempts >= settings.attempts.max(1) {
                return Err("tests_modified".into());
            }
            self.next_prompt(t, prompt::tests_readonly_prompt(&touched))?;
            self.save(t)?;
            return Ok(Flow::Next);
        }
        self.set_step(t, Step::Code, StepState::Done)?;
        self.set(t, Status::Checking)?;
        self.clear_pending(&t.id);
        Ok(Flow::Next)
    }

    /// The executor's edits to the test author's files are undone and committed away. Returns the files it touched.
    pub(super) fn protect_tests(&self, t: &Task, wt: &std::path::Path) -> Result<Vec<String>, String> {
        let st = self.state(&t.id);
        let Some(from) = st.tests_commit.clone().filter(|_| !st.test_files.is_empty()) else { return Ok(vec![]) };
        let touched: Vec<String> = git::changed_paths(wt, &from, "HEAD").into_iter().filter(|p| st.test_files.contains(p)).collect();
        if touched.is_empty() {
            return Ok(touched);
        }
        git::restore_paths(wt, &from, &touched).map_err(gerr)?;
        self.g_commit(t, wt, "restore tests").map_err(gerr)?;
        Ok(touched)
    }

    /// `DEFERRED:` from the executor: the task goes back to the queue with its copy and session, up to two
    /// times (G08.3); a third time it fails with the reason.
    fn defer(&self, t: &mut Task, reason: &str) -> Result<bool, String> {
        let left = match t.start {
            StartWhen::AfterMerge { left: 0 } => return Err(format!("deferred_too_often: {reason}")),
            StartWhen::AfterMerge { left } => left - 1,
            _ => 1,
        };
        self.update_state(&t.id, |s| s.deferred = true)?;
        t.start = StartWhen::AfterMerge { left };
        self.clear_pending(&t.id);
        self.set(t, Status::Queued)?;
        self.stage(t, &format!("deferred: {reason}"));
        Ok(true)
    }

    /// Limit hit and no model to go on with: wait for the reset (or half an hour when it is not known).
    pub(super) fn on_limit(&self, t: &mut Task, p: &Project, resets: Option<DateTime<Local>>, stage: Status) -> Res {
        let mut pend = self.pending(&t.id);
        if stage == Status::Working {
            if let Some(wt) = t.worktree.clone().filter(|w| !git::rebase_in_progress(w)) {
                self.g_commit(t, &wt, &t.title).map_err(gerr)?;
            }
        }
        let until = resets.unwrap_or_else(|| self.now() + CDuration::minutes(super::roles::LIMIT_WAIT_MINS));
        t.resume_at = Some(until);
        pend.stage = Some(stage); // a worker prompt that did not get through is kept for after the reset
        self.set_pending(&t.id, &pend)?;
        self.set(t, Status::PausedLimit)?;
        let body = format!("paused until {}", until.format("%H:%M"));
        self.stage(t, &body);
        self.notify(NoticeKind::Limit, t, p, body, vec![]);
        Ok(Flow::Stop)
    }

    /// The gates (spec 142) run in `wt`: the project's tests and every extra check. None = cancelled.
    pub(super) fn gate_results(&self, id: &str, p: &Project, wt: &std::path::Path, cancel: &AtomicBool) -> Option<Vec<GateResult>> {
        let mut checks: Vec<(String, String)> = vec![];
        if let Some(c) = p.check_cmd.as_deref().filter(|c| !c.trim().is_empty()) {
            checks.push(("tests".into(), c.to_string()));
        }
        checks.extend(p.extra_checks.iter().filter(|(_, c)| !c.trim().is_empty()).cloned());
        let mut results = vec![];
        let envs = self.task(id).map(|t| Self::copy_env(&t)).unwrap_or_default();
        for (name, cmd) in checks {
            self.log(id, LogKind::Check, &format!("$ {cmd}"));
            let (passed, out) = sh(wt, &cmd, &envs, cancel)?;
            if !passed {
                // the whole output stays in the copy: what the agent gets may be squeezed (story 308)
                let f = super::practices::raw_output_path(wt, &name);
                let _ = std::fs::create_dir_all(f.parent().unwrap_or(wt)).and_then(|_| std::fs::write(&f, &out));
            }
            let tl = tail(&out, 60);
            self.log(id, LogKind::Check, &format!("{}\n{tl}", if passed { "passed" } else { "failed" }));
            results.push(GateResult { name, passed, tail: tl });
        }
        Some(results)
    }

    /// The gates in the task's copy; the result goes on the task. None = cancelled; Some(true) = all green,
    /// or nothing to run.
    fn run_gates(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Result<Option<bool>, String> {
        let wt = t.worktree.clone().ok_or("no_worktree")?;
        t.outcome.tests = None;
        t.outcome.gates.clear();
        let Some(results) = self.gate_results(&t.id, p, &wt, cancel) else { return Ok(None) };
        if results.is_empty() {
            self.log(&t.id, LogKind::Check, "no tests in project - reviewer only");
            return Ok(Some(true));
        }
        t.outcome.tests = results.iter().find(|r| r.name == "tests").map(|r| TestResult { passed: r.passed, tail: r.tail.clone() });
        let all = results.iter().all(|r| r.passed);
        t.outcome.gates = results;
        Ok(Some(all))
    }

    /// Red gates (or red e2e): back to the executor with the output, up to the cap of failed rounds.
    fn on_red(&self, t: &mut Task) -> Res {
        let cap = super::lk(&self.store).settings().attempts.max(1);
        t.attempts += 1;
        if t.attempts >= cap {
            return Err("tests_failed".into());
        }
        let squeeze = super::lk(&self.store).settings().compress_output;
        let wt = t.worktree.clone().unwrap_or_default();
        let mut raw = vec![];
        let out: Vec<String> = t
            .outcome
            .gates
            .iter()
            .filter(|g| !g.passed)
            .map(|g| {
                let f = super::practices::raw_output_path(&wt, &g.name);
                let full = std::fs::read_to_string(&f).ok();
                if full.is_some() {
                    raw.push(format!("- {}: {}", g.name, f.display()));
                }
                let text = match full.filter(|_| squeeze) {
                    Some(full) => tail(&prompt::squeeze(&full), 60),
                    None => g.tail.clone(),
                };
                format!("{}:\n{text}", g.name)
            })
            .collect();
        let mut msg = prompt::tests_failed_prompt(&out.join("\n\n"));
        if !raw.is_empty() {
            msg.push_str(&format!("\n\nThe full raw output of each failed check is in a file:\n{}", raw.join("\n")));
        }
        self.next_prompt(t, msg)?;
        self.back_to_code(t)?;
        Ok(Flow::Next)
    }

    fn check_step(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Res {
        self.ensure_steps(t, p);
        self.stage(t, "checking");
        self.set_step(t, Step::Gates, StepState::Running)?;
        match self.run_gates(t, p, cancel)? {
            None => return Ok(Flow::Stop),
            Some(false) => return self.on_red(t),
            Some(true) => {}
        }
        self.set_step(t, Step::Gates, StepState::Done)?;
        if step_is(t, Step::E2e, |s| matches!(s, StepState::Pending | StepState::Running)) {
            match self.run_e2e(t, p, cancel)? {
                None => return Ok(Flow::Stop),
                Some(false) => return self.on_red(t),
                Some(true) => {}
            }
        }
        if self.proof_step(t, p, cancel).is_none() {
            return Ok(Flow::Stop);
        }
        self.set(t, Status::Reviewing)?;
        Ok(Flow::Next)
    }

    /// The Verifier starts the project as a user would (only when e2e is on and the project can be started).
    /// An answer that is not `{pass, issues}` counts as a failure: the proof is missing.
    fn run_e2e(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Result<Option<bool>, String> {
        self.set_step(t, Step::E2e, StepState::Running)?;
        self.stage(t, "e2e");
        let mut brief = self.brief_of(t);
        brief.original = t.prompt.clone();
        let inputs = RoleInputs { run_cmd: p.run_cmd.clone(), ..Default::default() };
        let base = |v: Vendor, _: bool| prompt::render(v, RoleKind::Verifier, &brief, &inputs);
        let parse = |s: &str| prompt::parse_verify(s).ok_or_else(|| "not a {pass, issues} object".to_string());
        let r = self.structured(t, p, RoleKind::Verifier, Status::Checking, prompt::verify_schema(), &base, &parse, cancel)?;
        let (pass, issues) = match r {
            super::steps::Structured::Stopped => return Ok(None),
            super::steps::Structured::Unusable(why) => (false, vec![format!("verifier answered in wrong format: {why}")]),
            super::steps::Structured::Parsed(x) => x,
        };
        if pass {
            self.set_step(t, Step::E2e, StepState::Done)?;
            return Ok(Some(true));
        }
        t.outcome.gates.push(GateResult { name: "e2e".into(), passed: false, tail: issues.join("\n") });
        self.set_step(t, Step::E2e, StepState::Pending)?;
        Ok(Some(false))
    }

    fn review(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Res {
        if let Some(r) = self.swarm_review(t, p, cancel) {
            return r;
        }
        self.review_plain(t, p, cancel)
    }

    /// The one reviewer (a run of its own, read-only) judges the work.
    pub(super) fn review_plain(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Res {
        let wt = t.worktree.clone().ok_or("no_worktree")?;
        self.stage(t, &format!("reviewing ({})", t.agent));
        self.set_step(t, Step::Review, StepState::Running)?;
        let checks = if t.outcome.gates.is_empty() {
            "No tests in this project.".to_string()
        } else {
            t.outcome.gates.iter().map(|g| format!("{}: {}\n{}", g.name, if g.passed { "passed" } else { "failed" }, g.tail)).collect::<Vec<_>>().join("\n\n")
        };
        // the task, the diff and the output of the checks: nothing of what the executor thought (story 12)
        let brief = prompt::Brief { original: t.prompt.clone(), ..Default::default() };
        let inputs = RoleInputs { diff: Some(git::diff(&wt, &p.base_branch, DIFF_FOR_REVIEW)), gate_output: Some(checks), ..Default::default() };
        let base = |v: Vendor, _: bool| prompt::render(v, RoleKind::Reviewer, &brief, &inputs);
        let parse = |s: &str| prompt::parse_verdict(s).ok_or_else(|| "not a verdict".to_string());
        let r = self.structured(t, p, RoleKind::Reviewer, Status::Reviewing, prompt::verdict_schema(), &base, &parse, cancel)?;
        let (approved, issues) = match r {
            super::steps::Structured::Stopped => return Ok(Flow::Stop),
            super::steps::Structured::Unusable(_) => {
                self.log(&t.id, LogKind::Review, prompt::BAD_FORMAT);
                (false, vec![prompt::BAD_FORMAT.to_string()])
            }
            super::steps::Structured::Parsed(x) => x,
        };
        self.apply_verdict(t, approved, issues)
    }

    /// What a verdict does: approved goes to the merge queue, changes go back to the executor (up to the cap).
    pub(super) fn apply_verdict(&self, t: &mut Task, approved: bool, issues: Vec<String>) -> Res {
        t.outcome.review = Some(Review { approved, issues: issues.clone() });
        // a finding becomes an item of "До мержа"; the reviewer's own items close when it approves
        t.todos = self.review_todos(&t.id, approved, &issues)?;
        if approved {
            self.set_step(t, Step::Review, StepState::Done)?;
            self.set(t, Status::Merging)?;
            return Ok(Flow::Next);
        }
        let cap = super::lk(&self.store).settings().review_rounds.max(1);
        t.review_rounds += 1;
        if t.review_rounds >= cap {
            return Err("review_rejected".into());
        }
        self.next_prompt(t, prompt::changes_prompt(&issues))?;
        self.back_to_code(t)?;
        Ok(Flow::Next)
    }

    /// One merge at a time per project: rebase (a conflict goes to the Integrator) → gates again →
    /// squash onto base → finish.
    fn merge(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Res {
        // landed before a crash that came ahead of saving the commit: do not land it twice
        if t.outcome.commit.is_none() && p.finish != Finish::Pr {
            if let Some(sha) = git::merged_commit(&p.path, &p.base_branch, &t.branch) {
                self.log(&t.id, LogKind::Git, &format!("already on {}: {sha}", p.base_branch));
                t.outcome.commit = Some(sha);
                self.save(t)?;
            }
        }
        if t.outcome.commit.is_none() {
            // the "До мержа" list: an open item keeps the task out of the queue
            if self.open_todos_now(&t.id) > 0 {
                return self.park_on_todos(t, p);
            }
            let lock = self.merge_lock(&p.id);
            // the place in the queue: `Queued(since)` while another task merges, `Running` for the one that does (G08.4)
            self.set_step(t, Step::Merge, StepState::Queued(self.now()))?;
            // waiting for another task's merge must still notice cancel / shutdown
            let _one = loop {
                match lock.try_lock() {
                    Ok(g) => break g,
                    Err(std::sync::TryLockError::Poisoned(e)) => break e.into_inner(),
                    Err(std::sync::TryLockError::WouldBlock) => {}
                }
                if cancel.load(Ordering::SeqCst) {
                    return Ok(Flow::Stop);
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            };
            self.stage(t, "merging");
            self.set_step(t, Step::Merge, StepState::Running)?;
            let wt = t.worktree.clone().ok_or("no_worktree")?;
            let mut merged = false;
            for _ in 0..5 {
                // a rebase stopped by a restart: the Integrator goes on with it
                let conflict = if git::rebase_in_progress(&wt) {
                    Some(String::new())
                } else {
                    match self.g_rebase(&wt, &p.base_branch) {
                        Ok(()) => None,
                        Err(GitError::Conflict(m)) => Some(m),
                        Err(e) => return Err(gerr(e)),
                    }
                };
                if let Some(m) = conflict {
                    if self.frozen.load(Ordering::SeqCst) {
                        return Ok(Flow::Stop); // no agent starts under a freeze: the stopped rebase waits for the Integrator
                    }
                    self.log(&t.id, LogKind::Git, &format!("conflict with {}", p.base_branch));
                    match self.integrate(t, p, m, cancel)? {
                        None => return Ok(Flow::Stop),
                        Some(true) => {}
                        Some(false) => return Err("conflict_with_main".into()),
                    }
                }
                // shortcut: checks re-run even when the rebase changed nothing; skip it once git can report HEAD
                match self.run_gates(t, p, cancel)? {
                    None => return Ok(Flow::Stop),
                    Some(false) => return self.on_red(t),
                    Some(true) => {}
                }
                // copied local files (.env ...) never leave the machine, not even in branch history
                let leaked = git::try_hidden_committed(&wt, &p.base_branch).map_err(gerr)?;
                if !leaked.is_empty() {
                    return Err(format!("hidden_file_committed: {}", leaked.join(", ")));
                }
                // a key or a .env some commit of the branch added (R17i.2): nothing is merged, pushed or put in a PR
                let keys = if self.protects(t) { git::protected_committed(&wt, &p.base_branch).map_err(gerr)? } else { vec![] };
                if !keys.is_empty() {
                    return Err(format!("protected_file_committed: {}", keys.join(", ")));
                }
                // an item added while the task waited in the queue or rebased
                if self.open_todos_now(&t.id) > 0 {
                    return self.park_on_todos(t, p);
                }
                let stats = git::diffstat(&wt, &p.base_branch);
                t.outcome.files_changed = stats.len() as u32;
                t.outcome.insertions = stats.iter().map(|s| s.insertions).sum();
                t.outcome.deletions = stats.iter().map(|s| s.deletions).sum();
                if p.finish == Finish::Pr {
                    merged = true; // the PR carries the branch; base is not touched locally
                    break;
                }
                // the body: the report's first paragraph, cut at a sentence (story 292); the trailer marks a task's commit
                let msg = match t.outcome.summary.as_deref().map(prompt::commit_body).filter(|b| !b.is_empty()) {
                    Some(b) => format!("{}\n\n{b}\n\n{}: {}", t.title, git::TASK_TRAILER, t.id),
                    None => format!("{}\n\n{}: {}", t.title, git::TASK_TRAILER, t.id),
                };
                match self.g_squash(&p.path, &wt, &p.base_branch, &msg) {
                    Ok(sha) => {
                        self.log(&t.id, LogKind::Git, &format!("{} ← {sha}", p.base_branch));
                        t.outcome.commit = Some(sha);
                        t.outcome.error = None;
                        self.save(t)?;
                        merged = true;
                        break;
                    }
                    Err(GitError::BaseMoved(_)) => continue,
                    Err(GitError::AdvanceBlocked(m)) => {
                        t.resume_at = Some(self.now() + CDuration::seconds(MERGE_RETRY_SECS));
                        t.outcome.error = Some("merge_blocked_dirty_main".into());
                        self.set_step(t, Step::Merge, StepState::Pending)?;
                        self.set(t, Status::MergeBlocked)?;
                        self.log(&t.id, LogKind::Git, &format!("merge postponed: {m}"));
                        return Ok(Flow::Stop);
                    }
                    Err(e) => return Err(gerr(e)),
                }
            }
            if !merged {
                return Err(format!("{} keeps moving, merge gave up", p.base_branch));
            }
            // landed: the next in the queue is first now, so this one must not still look like it merges
            self.set_step(t, Step::Merge, StepState::Done)?;
        }
        self.finish(t, p)
    }

    /// The Integrator (a fresh run of the same env, not the executor) resolves the stopped rebase (R19i.3).
    /// Some(true) = rebased; Some(false) = it could not; None = the task waits or was stopped.
    fn integrate(&self, t: &mut Task, p: &Project, mut git_out: String, cancel: &AtomicBool) -> Result<Option<bool>, String> {
        let wt = t.worktree.clone().ok_or("no_worktree")?;
        let brief = prompt::Brief { goal: t.prompt.clone(), original: t.prompt.clone(), ..Default::default() };
        for round in 0..INTEGRATOR_ROUNDS {
            if self.frozen.load(Ordering::SeqCst) {
                return Ok(None); // the next commit's conflict waits in the stopped rebase until the freeze ends (R18i.1)
            }
            // the person's answer to the Integrator is taken only once a run will start, so a freeze keeps it
            let (mut resume, mut ans) = (None, None);
            let st = self.state(&t.id);
            if round == 0 && st.asked == Some(RoleKind::Integrator) {
                resume = st.asked_session.clone();
                ans = self.pending(&t.id).prompt;
                self.update_state(&t.id, |s| {
                    s.asked = None;
                    s.asked_session = None;
                })?;
            }
            let inputs = RoleInputs {
                diff: Some(git::diff(&wt, &p.base_branch, DIFF_FOR_REVIEW)),
                conflict: Some(Conflict { base: p.base_branch.clone(), git_output: git_out.clone() }),
                ..Default::default()
            };
            // the answer is the prompt of the resumed session, or follows a fresh one's
            let resumed = resume.is_some();
            let prompt_fn = |v: Vendor, _: bool| match &ans {
                Some(a) if resumed => a.clone(),
                Some(a) => format!("{}\n\n{a}", prompt::render(v, RoleKind::Integrator, &brief, &inputs)),
                None => prompt::render(v, RoleKind::Integrator, &brief, &inputs),
            };
            let call = RoleCall { role: RoleKind::Integrator, resume, prompt: &prompt_fn, schema: None, stage: Status::Merging, shown: None };
            match self.run_role(t, p, call, cancel)? {
                RoleEnd::Done { text, session } => {
                    let sig = prompt::parse_signals(&text);
                    if let Some(q) = sig.needs_human {
                        self.ask_role(t, p, RoleKind::Integrator, Status::Merging, q, session)?;
                        return Ok(None);
                    }
                }
                RoleEnd::Failed(_) => return Ok(Some(false)),
                RoleEnd::Limit(resets) => {
                    self.on_limit(t, p, resets, Status::Merging)?;
                    return Ok(None);
                }
                RoleEnd::Auth | RoleEnd::Stop | RoleEnd::Cancelled => return Ok(None),
            }
            self.clear_pending(&t.id);
            let left = git::diff(&wt, &p.base_branch, usize::MAX);
            if left.lines().any(|l| l.starts_with("+<<<<<<<") || l.starts_with("+>>>>>>>")) {
                return Ok(Some(false));
            }
            match self.g_rebase_continue(t, &wt) {
                Ok(()) => {
                    self.log(&t.id, LogKind::Git, "conflict resolved, rebase continued");
                    return Ok(Some(true));
                }
                // the next commit of the branch conflicts too: another look by a fresh run
                Err(GitError::Conflict(m)) => git_out = m,
                Err(e) => return Err(gerr(e)),
            }
        }
        Ok(Some(false))
    }

    fn finish(&self, t: &mut Task, p: &Project) -> Res {
        if let Some(wt) = t.worktree.take() {
            self.rm_wt(&p.path, &wt, p.finish != Finish::Pr).map_err(gerr)?;
        }
        t.outcome.error = None;
        if p.finish == Finish::Local {
            self.complete(t, p, Status::Done);
        } else if self.asks_outward(t, p) {
            let text = if p.finish == Finish::Push { PUSH_TO_ORIGIN } else { OPEN_PR };
            let options = if p.finish == Finish::Push {
                vec!["Push".to_string(), "Skip".to_string()]
            } else {
                vec!["Push and PR".to_string(), "Push only".to_string(), "Skip".to_string()]
            };
            t.question = Some(Question { text: text.into(), options: options.clone(), kind: QuestionKind::Outward });
            self.set(t, Status::AwaitingOutward)?;
            self.stage(t, "awaiting approval to push");
            self.notify(NoticeKind::Outward, t, p, summary_line(t), options);
        } else {
            self.outward(t, p);
        }
        Ok(Flow::Stop)
    }

    pub(super) fn complete(&self, t: &mut Task, p: &Project, s: Status) {
        t.status = s;
        t.question = None;
        t.finished_at = Some(self.now());
        for (step, st) in t.steps.iter_mut() {
            if *step == Step::Merge {
                *st = StepState::Done;
            }
        }
        let _ = self.save(t);
        self.clear_pending(&t.id);
        self.keep_only_swarm(&t.id);
        let line = summary_line(t);
        self.stage(t, &line);
        self.notify(NoticeKind::Done, t, p, line, vec![]);
    }

    /// Push base / open the PR. A refusal keeps the local result: done_not_pushed with the reason.
    pub(super) fn outward(&self, t: &mut Task, p: &Project) {
        let r = match p.finish {
            Finish::Pr => {
                let body = format!(
                    "{}\n\nTests: {}\nReview: {}",
                    t.outcome.summary.as_deref().unwrap_or(&t.title),
                    match &t.outcome.tests {
                        None => "no tests in project",
                        Some(r) if r.passed => "passed",
                        Some(_) => "failed",
                    },
                    match &t.outcome.review {
                        Some(r) if r.approved => "approved",
                        _ => "-",
                    }
                );
                self.g_open_pr(&p.path, &t.branch, &p.base_branch, &t.title, &body).map(|u| t.outcome.pr_url = Some(u))
            }
            _ => self.g_push(&p.path, &p.base_branch),
        };
        match r {
            Ok(()) => {
                t.outcome.error = None;
                self.log(&t.id, LogKind::Git, "pushed");
                self.complete(t, p, Status::Done);
                self.emit(super::EngineEvent::ProjectChanged(p.id.clone()));
            }
            Err(e) => {
                t.outcome.error = Some(e.to_string());
                self.log(&t.id, LogKind::Error, &e.to_string());
                self.complete(t, p, Status::DoneNotPushed);
            }
        }
    }
}
