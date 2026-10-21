//! The per-task state machine, run on the thread holding the task's claim:
//! queued → preparing → working ⇄ checking → reviewing → merging → done / awaiting_outward / ...
use super::util::{self, *};
use super::{Engine, Pending};
use crate::agent::{self, AgentEvent, Role, Run};
use crate::git::{self, GitError};
use crate::model::*;
use crate::notify::NoticeKind;
use chrono::{DateTime, Duration as CDuration, Local};
use std::sync::atomic::{AtomicBool, Ordering};

const MAX_ATTEMPTS: u32 = 3;
const MAX_REVIEW_ROUNDS: u32 = 2;
const DIFF_FOR_REVIEW: usize = 60_000;
const MERGE_RETRY_SECS: i64 = 60;
const LIMIT_WAIT_MINS: i64 = 30;
/// Outward question keys (UI translates).
pub(super) const PUSH_TO_ORIGIN: &str = "push_to_origin";
pub(super) const OPEN_PR: &str = "open_pr";
pub(super) const PUSH_REVERT: &str = "push_revert";

/// After a step: keep driving, or the task now waits (human, timer) / is finished / was cancelled.
enum Flow {
    Next,
    Stop,
}

type Step = Result<Flow, String>;

fn gerr(e: GitError) -> String {
    e.to_string()
}

fn limit_of(events: &[AgentEvent]) -> Option<Option<DateTime<Local>>> {
    events.iter().find_map(|e| match e {
        AgentEvent::Limit { resets_at } => Some(*resets_at),
        _ => None,
    })
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
            let r = match t.status {
                Status::Queued | Status::Preparing => self.prepare(&mut t, &p, cancel),
                Status::Working => self.work(&mut t, &p, cancel),
                Status::Checking => self.check_step(&mut t, &p, cancel),
                Status::Reviewing => self.review(&mut t, &p, cancel),
                Status::Merging => self.merge(&mut t, &p, cancel),
                _ => return,
            };
            if cancel.load(Ordering::SeqCst) {
                return; // cancel / shutdown own the task state from here
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

    fn set(&self, t: &mut Task, s: Status) -> Result<(), String> {
        t.status = s;
        self.save(t)
    }

    fn stage(&self, t: &Task, text: &str) {
        self.log(&t.id, LogKind::Stage, text);
    }

    /// Next worker prompt, written before the status that uses it.
    fn next_prompt(&self, t: &Task, prompt: String) -> Result<(), String> {
        self.set_pending(&t.id, &Pending { prompt: Some(prompt), ..Default::default() })
    }

    /// Failed: the worktree stays for inspection until remove / retry.
    pub(super) fn fail(&self, t: &mut Task, project: &str, why: String) {
        t.status = Status::Failed;
        t.outcome.error = Some(why.clone());
        t.finished_at = Some(self.now());
        let _ = self.save(t);
        self.clear_pending(&t.id);
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

    fn prepare(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Step {
        t.started_at.get_or_insert(self.now());
        self.set(t, Status::Preparing)?;
        self.stage(t, "preparing");
        self.clear_pending(&t.id);
        // interrupted earlier: prepare again from scratch
        if let Some(wt) = t.worktree.take() {
            git::remove_worktree(&p.path, &wt, true).map_err(gerr)?;
        }
        let wt = git::create_worktree(&p.path, &self.worktrees_dir, &t.id, &t.branch, &p.base_branch).map_err(gerr)?;
        t.worktree = Some(wt.clone());
        self.save(t)?;
        self.log(&t.id, LogKind::Git, &format!("worktree {} on {}", wt.display(), t.branch));
        let copied = util::copy_files(&p.path, &wt, &p.copy_files);
        if !copied.is_empty() {
            // never part of a commit, tracked by git ignore rules or not
            git::hide_untracked(&wt, &copied).map_err(gerr)?;
            self.stage(t, &format!("copied {}", copied.join(", ")));
        }
        if let Some(cmd) = p.setup_cmd.as_deref().filter(|c| !c.trim().is_empty()) {
            self.stage(t, &format!("setup: {cmd}"));
            let Some((ok, out)) = sh(&wt, cmd, cancel) else { return Ok(Flow::Stop) };
            if !ok {
                return Err(format!("setup_failed: {cmd}\n{}", tail(&out, 60)));
            }
        }
        self.set(t, Status::Working)?;
        Ok(Flow::Next)
    }

    /// Runs an agent, logging every event. None = cancelled.
    fn run_agent(&self, t: &Task, a: &dyn agent::Adapter, run: &Run, cancel: &AtomicBool) -> Option<Vec<AgentEvent>> {
        let id = t.id.clone();
        let kind = if run.role == Role::Reviewer { LogKind::Review } else { LogKind::Agent };
        let mut on = |e: &AgentEvent| match e {
            AgentEvent::Text(s) => self.log(&id, kind.clone(), s),
            AgentEvent::Tool { name, summary } => self.log(&id, LogKind::Tool, &format!("{name}: {summary}")),
            AgentEvent::Done { text, is_error, .. } => {
                self.log(&id, if *is_error { LogKind::Error } else { kind.clone() }, text)
            }
            AgentEvent::Limit { .. } => self.log(&id, LogKind::Stage, "usage limit reached"),
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

    fn work(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Step {
        let wt = t.worktree.clone().ok_or("no_worktree")?;
        let a = self.adapter(&t.agent).ok_or(format!("agent_not_found: {}", t.agent))?;
        // the pending prompt (answer, check output, review notes, ...) stays on disk until a run
        // has taken it: a crash or quit mid-run sends it again
        let mut text = self.pending(&t.id).prompt.unwrap_or_else(|| if t.session.is_some() { CONTINUE.into() } else { t.prompt.clone() });
        if t.session.is_none() && !text.starts_with(&t.prompt) {
            text = format!("{}\n\n{text}", t.prompt); // no session to resume: restate the task
        }
        self.stage(t, &format!("working ({})", t.agent));
        let run = Run { cwd: wt.clone(), prompt: text, system: worker_system(), resume: t.session.clone(), role: Role::Worker };
        let Some(events) = self.run_agent(t, &*a, &run, cancel) else { return Ok(Flow::Stop) };

        let done = events.iter().rev().find_map(|e| match e {
            AgentEvent::Done { session, text, is_error } => Some((session.clone(), text.clone(), *is_error)),
            _ => None,
        });
        if let Some(s) = done.as_ref().and_then(|d| d.0.clone()) {
            t.session = Some(s);
        }
        if let Some(resets) = limit_of(&events) {
            return self.on_limit(t, p, resets, Status::Working);
        }
        let (text, failed) = match done {
            Some((_, text, err)) => (text, err),
            None => ("agent ended without a result".into(), true),
        };
        if failed {
            // same prompt again, same session; counted apart from the check rounds
            let mut pend = self.pending(&t.id);
            pend.agent_errors += 1;
            if pend.agent_errors >= MAX_ATTEMPTS {
                return Err(format!("agent_failed: {}", tail(&text, 20)));
            }
            self.set_pending(&t.id, &pend)?;
            self.save(t)?;
            return Ok(Flow::Next);
        }
        // the prompt is cleared below only once the next status is saved

        let (summary, question) = needs_human(&text);
        if !summary.is_empty() {
            t.outcome.summary = Some(summary.chars().take(500).collect());
        }
        let rebasing = git::rebase_in_progress(&wt);
        if !rebasing {
            if git::uncommit_hidden(&wt, &p.base_branch).map_err(gerr)? {
                self.log(&t.id, LogKind::Git, "agent committed a copied local file; its commits were folded back without it");
            }
            git::commit_all(&wt, &t.title).map_err(gerr)?;
        }
        if let Some(q) = question {
            let (body, options) = (q.text.clone(), q.options.clone());
            t.question = Some(q);
            self.set(t, Status::NeedsHuman)?;
            self.clear_pending(&t.id);
            self.stage(t, "needs human");
            self.notify(NoticeKind::Question, t, p, body, options);
            return Ok(Flow::Stop);
        }
        if rebasing {
            let left = git::diff(&wt, &p.base_branch, usize::MAX);
            if left.lines().any(|l| l.starts_with("+<<<<<<<") || l.starts_with("+>>>>>>>")) {
                return Err("conflict_with_main".into());
            }
            match git::rebase_continue(&wt) {
                Ok(()) => self.log(&t.id, LogKind::Git, "conflict resolved, rebase continued"),
                Err(GitError::Conflict(m)) => {
                    self.next_prompt(t, conflict_prompt(&p.base_branch, &m))?;
                    self.save(t)?;
                    return Ok(Flow::Next);
                }
                Err(e) => return Err(gerr(e)),
            }
        }
        self.set(t, Status::Checking)?;
        self.clear_pending(&t.id);
        Ok(Flow::Next)
    }

    /// Limit hit: the worker continues on the project's fallback engine, otherwise wait for the reset.
    fn on_limit(&self, t: &mut Task, p: &Project, resets: Option<DateTime<Local>>, stage: Status) -> Step {
        let mut pend = self.pending(&t.id);
        if stage == Status::Working {
            if let Some(wt) = t.worktree.clone().filter(|w| !git::rebase_in_progress(w)) {
                git::commit_all(&wt, &t.title).map_err(gerr)?;
            }
            if let Some(fb) = p.fallback_agent.clone().filter(|f| f != &t.agent && self.adapter(f).is_some()) {
                self.stage(t, &format!("switched to {fb}"));
                let diff = t.worktree.as_ref().map(|w| git::diff(w, &p.base_branch, DIFF_FOR_REVIEW)).unwrap_or_default();
                // fresh session on the fallback: the task, the work so far and what was asked last
                self.next_prompt(t, fallback_prompt(&t.prompt, &diff, pend.prompt.as_deref()))?;
                t.agent = fb;
                t.session = None;
                self.save(t)?;
                return Ok(Flow::Next);
            }
        }
        let until = resets.unwrap_or_else(|| self.now() + CDuration::minutes(LIMIT_WAIT_MINS));
        t.resume_at = Some(until);
        pend.stage = Some(stage); // a worker prompt that did not get through is kept for after the reset
        self.set_pending(&t.id, &pend)?;
        self.set(t, Status::PausedLimit)?;
        let body = format!("paused until {}", until.format("%H:%M"));
        self.stage(t, &body);
        self.notify(NoticeKind::Limit, t, p, body, vec![]);
        Ok(Flow::Stop)
    }

    /// Runs `check_cmd`. None = cancelled; Some(true) = green or no tests in the project.
    fn run_check(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Result<Option<bool>, String> {
        let Some(cmd) = p.check_cmd.as_deref().filter(|c| !c.trim().is_empty()) else {
            t.outcome.tests = None;
            self.log(&t.id, LogKind::Check, "no tests in project - reviewer only");
            return Ok(Some(true));
        };
        let wt = t.worktree.clone().ok_or("no_worktree")?;
        self.log(&t.id, LogKind::Check, &format!("$ {cmd}"));
        let Some((passed, out)) = sh(&wt, cmd, cancel) else { return Ok(None) };
        let tl = tail(&out, 60);
        self.log(&t.id, LogKind::Check, &format!("{}\n{tl}", if passed { "passed" } else { "failed" }));
        t.outcome.tests = Some(TestResult { passed, tail: tl });
        Ok(Some(passed))
    }

    /// Red check: back to the worker with the output, up to MAX_ATTEMPTS failed rounds.
    fn on_red(&self, t: &mut Task) -> Step {
        t.attempts += 1;
        if t.attempts >= MAX_ATTEMPTS {
            return Err("tests_failed".into());
        }
        let tl = t.outcome.tests.as_ref().map(|r| r.tail.clone()).unwrap_or_default();
        self.next_prompt(t, tests_failed_prompt(&tl))?;
        self.set(t, Status::Working)?;
        Ok(Flow::Next)
    }

    fn check_step(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Step {
        self.stage(t, "checking");
        match self.run_check(t, p, cancel)? {
            None => Ok(Flow::Stop),
            Some(false) => self.on_red(t),
            Some(true) => {
                self.set(t, Status::Reviewing)?;
                Ok(Flow::Next)
            }
        }
    }

    fn review(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Step {
        let wt = t.worktree.clone().ok_or("no_worktree")?;
        let rid = p.reviewer.clone().unwrap_or_else(|| t.agent.clone());
        let a = self.adapter(&rid).ok_or(format!("agent_not_found: {rid}"))?;
        self.stage(t, &format!("reviewing ({rid})"));
        let tests = match &t.outcome.tests {
            None => "No tests in this project.".to_string(),
            Some(r) => format!("{}\n{}", if r.passed { "passed" } else { "failed" }, r.tail),
        };
        let base = review_prompt(&t.prompt, &git::diff(&wt, &p.base_branch, DIFF_FOR_REVIEW), &tests);
        let mut verdict = None;
        for round in 0..2 {
            let text = if round == 0 { base.clone() } else { review_retry_prompt(&base) };
            let run = Run { cwd: wt.clone(), prompt: text, system: reviewer_system(), resume: None, role: Role::Reviewer };
            let Some(events) = self.run_agent(t, &*a, &run, cancel) else { return Ok(Flow::Stop) };
            if let Some(resets) = limit_of(&events) {
                return self.on_limit(t, p, resets, Status::Reviewing);
            }
            let answer = events.iter().rev().find_map(|e| match e {
                AgentEvent::Done { text, is_error: false, .. } => Some(text.clone()),
                _ => None,
            });
            verdict = answer.as_deref().and_then(parse_verdict);
            if verdict.is_some() {
                break;
            }
            self.log(&t.id, LogKind::Review, BAD_FORMAT);
        }
        let review = verdict.unwrap_or(Review { approved: false, issues: vec![BAD_FORMAT.into()] });
        let approved = review.approved;
        let issues = review.issues.clone();
        t.outcome.review = Some(review);
        if approved {
            self.set(t, Status::Merging)?;
            return Ok(Flow::Next);
        }
        t.review_rounds += 1;
        if t.review_rounds >= MAX_REVIEW_ROUNDS {
            return Err("review_rejected".into());
        }
        self.next_prompt(t, changes_prompt(&issues))?;
        self.set(t, Status::Working)?;
        Ok(Flow::Next)
    }

    /// One merge at a time per project: rebase → check again → squash onto base → finish.
    fn merge(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Step {
        // landed before a crash that came ahead of saving the commit: do not land it twice
        if t.outcome.commit.is_none() && p.finish != Finish::Pr {
            if let Some(sha) = git::merged_commit(&p.path, &p.base_branch, &t.branch) {
                self.log(&t.id, LogKind::Git, &format!("already on {}: {sha}", p.base_branch));
                t.outcome.commit = Some(sha);
                self.save(t)?;
            }
        }
        if t.outcome.commit.is_none() {
            let lock = self.merge_lock(&p.id);
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
            let wt = t.worktree.clone().ok_or("no_worktree")?;
            let mut merged = false;
            for _ in 0..5 {
                match git::rebase_onto(&wt, &p.base_branch) {
                    Ok(()) => {}
                    Err(GitError::Conflict(m)) => {
                        self.log(&t.id, LogKind::Git, &format!("conflict with {}", p.base_branch));
                        self.next_prompt(t, conflict_prompt(&p.base_branch, &m))?;
                        self.set(t, Status::Working)?;
                        return Ok(Flow::Next);
                    }
                    Err(e) => return Err(gerr(e)),
                }
                // shortcut: checks re-run even when the rebase changed nothing; skip it once git can report HEAD
                match self.run_check(t, p, cancel)? {
                    None => return Ok(Flow::Stop),
                    Some(false) => return self.on_red(t),
                    Some(true) => {}
                }
                // copied local files (.env ...) never leave the machine, not even in branch history
                let leaked = git::hidden_committed(&wt, &p.base_branch);
                if !leaked.is_empty() {
                    return Err(format!("hidden_file_committed: {}", leaked.join(", ")));
                }
                let stats = git::diffstat(&wt, &p.base_branch);
                t.outcome.files_changed = stats.len() as u32;
                t.outcome.insertions = stats.iter().map(|s| s.insertions).sum();
                t.outcome.deletions = stats.iter().map(|s| s.deletions).sum();
                if p.finish == Finish::Pr {
                    merged = true; // the PR carries the branch; base is not touched locally
                    break;
                }
                let msg = match &t.outcome.summary {
                    Some(s) => format!("{}\n\n{s}", t.title),
                    None => t.title.clone(),
                };
                match git::squash_to_base(&p.path, &wt, &p.base_branch, &msg) {
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
        }
        self.finish(t, p)
    }

    fn finish(&self, t: &mut Task, p: &Project) -> Step {
        if let Some(wt) = t.worktree.take() {
            git::remove_worktree(&p.path, &wt, p.finish != Finish::Pr).map_err(gerr)?;
        }
        t.outcome.error = None;
        if p.finish == Finish::Local {
            self.complete(t, p, Status::Done);
        } else if p.ask_outward {
            let text = if p.finish == Finish::Push { PUSH_TO_ORIGIN } else { OPEN_PR };
            let options = vec!["Push".to_string(), "Skip".to_string()];
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
        let _ = self.save(t);
        self.clear_pending(&t.id);
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
                git::open_pr(&p.path, &t.branch, &p.base_branch, &t.title, &body).map(|u| t.outcome.pr_url = Some(u))
            }
            _ => git::push(&p.path, &p.base_branch),
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
