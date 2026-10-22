//! The crew (Бригада, G11.2): the Coordinator splits the task into parts with files of their own, the
//! app checks the split itself, the workers run in copies of their own from the task's branch, and the
//! finished parts land in the task's branch one by one through the same merge queue as main (rebase,
//! gates, fast-forward). Then the whole task goes through the gates, the review and the merge as usual.
use super::flow::{gerr, Flow, Res};
use super::lanes::{Lane, LaneEnd};
use super::state::{Asked, LaneState, PartRt};
use super::steps::Structured;
use super::*;
use crate::models;
use crate::prompt::{self, Vendor};
use crate::swarm::{self, parse, prompts};
use std::path::Path;

/// Puts the files of a part (relative paths git reported) from its copy into the task's copy; a file
/// the part deleted is deleted there. Only regular files, and only inside the task's copy.
fn apply_files(from: &Path, to: &Path, paths: &[String]) -> Result<(), String> {
    let root = to.canonicalize().map_err(|e| format!("{}: {e}", to.display()))?;
    for rel in paths {
        let (src, dst) = (from.join(rel), to.join(rel));
        if rel.split('/').any(|c| c == ".." || c.is_empty()) || Path::new(rel).is_absolute() {
            return Err(format!("part_unsafe_path: {rel}"));
        }
        let parent = dst.parent().ok_or_else(|| format!("part_unsafe_path: {rel}"))?;
        std::fs::create_dir_all(parent).map_err(|e| format!("{rel}: {e}"))?;
        if !parent.canonicalize().is_ok_and(|p| p.starts_with(&root)) {
            return Err(format!("part_unsafe_path: {rel}"));
        }
        match src.symlink_metadata() {
            Ok(m) if m.file_type().is_symlink() => return Err(format!("part_symlink: {rel}")),
            Ok(m) if m.is_file() => {
                if dst.symlink_metadata().is_ok_and(|d| d.file_type().is_symlink()) {
                    return Err(format!("part_symlink: {rel}"));
                }
                std::fs::copy(&src, &dst).map_err(|e| format!("{rel}: {e}"))?;
            }
            Ok(_) => {} // a folder: its files are listed one by one
            Err(_) => {
                if dst.is_file() || dst.symlink_metadata().is_ok() {
                    std::fs::remove_file(&dst).map_err(|e| format!("{rel}: {e}"))?;
                }
            }
        }
    }
    Ok(())
}

impl Engine {
    pub(super) fn crew_code(&self, t: &mut Task, p: &Project, n: u8, cancel: &AtomicBool) -> Res {
        let id = t.id.clone();
        let wt = t.worktree.clone().ok_or("no_worktree")?;
        self.set_step(t, Step::Code, StepState::Running)?;
        if self.state(&id).swarm.parts.is_empty() {
            self.g_commit(t, &wt, &t.title).map_err(gerr)?; // the preparation is part of the task's branch
            match self.crew_split(t, p, n, cancel)? {
                Some(parts) => {
                    self.stage(t, &format!("crew: {} parts", parts.len()));
                    self.update_state(&id, |s| s.swarm.parts = parts)?;
                }
                None => return Ok(Flow::Stop),
            }
            if self.state(&id).swarm.collapsed {
                return Ok(Flow::Next); // no split to work with: the single executor does it all
            }
        }
        // the person answered a part's question: it goes back to that part, in its session
        if let Some((letter, answer)) = self.take_lane_answer(&id) {
            self.set_part(&id, &letter, |x| {
                x.next_prompt = Some(answer);
                x.state = LaneState::Waiting;
            });
        }
        let sw = self.state(&id).swarm;
        let todo: Vec<String> =
            sw.parts.iter().filter(|x| !matches!(x.state, LaneState::Passed | LaneState::Failed | LaneState::Asking)).map(|x| x.id.clone()).collect();
        {
            let _mode = self.lanes_mode(&id);
            if sw.sequential {
                for part in &todo {
                    if cancel.load(Ordering::SeqCst) {
                        break;
                    }
                    self.part_lane(&id, p, part, cancel);
                }
            } else {
                std::thread::scope(|sc| {
                    for part in &todo {
                        let (id, part) = (&id, part);
                        sc.spawn(move || self.part_lane(id, p, part, cancel));
                    }
                });
            }
        }
        *t = self.task(&id).ok_or("task_not_found")?;
        if cancel.load(Ordering::SeqCst) {
            return Ok(Flow::Stop);
        }
        let sw = self.state(&id).swarm;
        if let Some(x) = sw.parts.iter().find(|x| x.state == LaneState::Interrupted) {
            let why = x.why_failed.clone().unwrap_or_default();
            return self.pause_for(t, p, &why);
        }
        if sw.parts.iter().any(|x| x.state == LaneState::Asking) {
            self.ask_next_lane(t, p, Status::Working)?;
            return Ok(Flow::Stop);
        }
        if let Some(x) = sw.parts.iter().find(|x| x.state == LaneState::Failed) {
            // what landed stays in the task's copy; the person lifts it as a single executor
            self.update_state(&id, |s| s.swarm.collapsed = true)?;
            return Err(format!("crew_part_failed: {}: {}", x.title, x.why_failed.clone().unwrap_or_default()));
        }
        self.update_state(&id, |s| s.swarm.landed_all = true)?;
        self.set_step(t, Step::Code, StepState::Done)?;
        self.set(t, Status::Checking)?;
        self.clear_pending(&id);
        Ok(Flow::Next)
    }

    /// The Coordinator's split, checked by the app. A split that overlaps is asked for again once; if it
    /// overlaps again the parts go one after the other. None = the task waits (question, limit, login).
    /// Ok(Some(vec![])) with `collapsed` set = the split could not be used at all.
    fn crew_split(&self, t: &mut Task, p: &Project, n: u8, cancel: &AtomicBool) -> Result<Option<Vec<PartRt>>, String> {
        let id = t.id.clone();
        let mut fix: Option<String> = None;
        let mut chosen: Option<Vec<parse::SplitPart>> = None;
        for attempt in 0..2 {
            let brief = self.brief_of(t);
            let request = prompts::split_request(&t.prompt, &brief, n as usize, fix.as_deref());
            let base = |_: Vendor, _: bool| request.clone();
            let parse_it = |s: &str| parse::parse_split(s, n as usize);
            match self.structured(t, p, RoleKind::Coordinator, Status::Working, prompts::split_schema(), &base, &parse_it, cancel)? {
                Structured::Stopped => return Ok(None),
                Structured::Unusable(why) => {
                    self.stage(t, &format!("crew_split_failed: {why}"));
                    self.update_state(&id, |s| s.swarm.collapsed = true)?;
                    return Ok(Some(vec![]));
                }
                Structured::Parsed(parts) => {
                    let ps: Vec<swarm::Part> = parts.iter().map(|x| swarm::Part { id: x.id.clone(), files: x.files.clone() }).collect();
                    match swarm::check_split(&ps) {
                        Ok(()) => {
                            chosen = Some(parts);
                            break;
                        }
                        Err(o) => {
                            self.stage(t, &format!("split_overlap: {} and {} both touch {} / {}", o.a, o.b, o.path_a, o.path_b));
                            self.update_state(&id, |s| s.swarm.split_tries += 1)?;
                            fix = Some(format!("parts {} and {} both touch {} / {}", o.a, o.b, o.path_a, o.path_b));
                            if attempt == 1 {
                                // twice: they go one after the other, each from what the earlier ones left
                                self.update_state(&id, |s| s.swarm.sequential = true)?;
                                self.stage(t, "split_sequential: the parts overlap, they run one by one");
                                chosen = Some(parts);
                            }
                        }
                    }
                }
            }
        }
        let parts = chosen.ok_or("crew_split_failed")?;
        let a = self.adapter(&t.agent).ok_or_else(|| format!("agent_not_found: {}", t.agent))?;
        let cat = models::catalog(&t.agent, &*a, &lk(&self.store));
        let worker = Self::choice_for(t, RoleKind::Worker).unwrap_or(ModelChoice { family: models::DEFAULT_FAMILY.into(), version: None });
        let ms = swarm::assign_models(&cat, &worker, parts.len() as u8);
        Ok(Some(
            parts
                .into_iter()
                .zip(swarm::letters(n as usize))
                .zip(ms)
                .map(|((x, letter), model)| PartRt {
                    branch: format!("{}-{}", t.branch, letter.to_lowercase()),
                    worktree: self.worktrees_dir.join(format!("{id}-swarm")).join(&letter),
                    id: letter,
                    title: x.title,
                    files: x.files,
                    brief: x.brief,
                    model,
                    session: None,
                    pin: None,
                    state: LaneState::Waiting,
                    attempts: 0,
                    next_prompt: None,
                    why_failed: None,
                })
                .collect(),
        ))
    }

    fn set_part(&self, id: &str, letter: &str, f: impl FnOnce(&mut PartRt)) {
        let _ = self.update_state(id, |s| {
            if let Some(x) = s.swarm.parts.iter_mut().find(|x| x.id == letter) {
                f(x);
            }
        });
    }

    fn part(&self, id: &str, letter: &str) -> Option<PartRt> {
        self.state(id).swarm.parts.into_iter().find(|x| x.id == letter)
    }

    /// One part: a seat, a copy from the task's branch, the worker, then the landing (rebase, gates,
    /// fast-forward of the task's branch) under the project's merge lock; red gates go back to the worker.
    fn part_lane(&self, id: &str, p: &Project, letter: &str, cancel: &AtomicBool) {
        let Some(t) = self.task(id) else { return };
        let Some(x0) = self.part(id, letter) else { return };
        let interrupted = |why: String| {
            self.set_part(id, letter, |x| {
                x.state = LaneState::Interrupted;
                x.why_failed = Some(why);
            })
        };
        let fail = |why: String| {
            let mut ev = LogEvent::new(self.now(), LogKind::Stage, &format!("part_failed: {letter}: {why}"));
            ev.role = Some(RoleKind::Worker);
            ev.variant = Some(letter.to_string());
            self.log_ev(id, ev);
            self.set_part(id, letter, |x| {
                x.state = LaneState::Failed;
                x.why_failed = Some(why);
            })
        };
        self.lane_waiting(id, RoleKind::Worker, letter, &x0.model);
        let Some(_seat) = self.take_seat(&t.agent, cancel) else { return interrupted("cancelled".into()) };
        self.set_part(id, letter, |x| {
            x.state = LaneState::Running;
            x.why_failed = None;
        });
        if !x0.worktree.exists() {
            let dir = self.worktrees_dir.join(format!("{id}-swarm"));
            // from the task's branch as it is now: parts that landed before are in it
            if let Err(e) = self.add_wt(&p.path, &dir, letter, &x0.branch, &t.branch) {
                return fail(gerr(e));
            }
            match self.setup_copy(p, &x0.worktree, cancel) {
                Ok(true) => {}
                Ok(false) => return interrupted("cancelled".into()),
                Err(e) => return fail(e),
            }
        }
        let cap = lk(&self.store).settings().attempts.max(1);
        let others: Vec<(String, Vec<String>)> =
            self.state(id).swarm.parts.iter().filter(|o| o.id != letter).map(|o| (o.title.clone(), o.files.clone())).collect();
        loop {
            let Some(x) = self.part(id, letter) else { return };
            let next = x.next_prompt.clone();
            let ptext = |_: Vendor, resumed: bool| match (&next, resumed) {
                (Some(n), true) => n.clone(),
                (Some(n), false) => format!("{}\n\n{n}", prompts::part_prompt(&t.prompt, &x.title, &x.brief, &x.files, &others)),
                (None, true) => prompt::CONTINUE.to_string(),
                (None, false) => prompts::part_prompt(&t.prompt, &x.title, &x.brief, &x.files, &others),
            };
            let lane = Lane {
                task_id: id,
                project: p,
                role: RoleKind::Worker,
                label: letter,
                cwd: Some(x.worktree.clone()),
                model: x.model.clone(),
                session: x.session.clone(),
                pin: x.pin.clone(),
                prompt: &ptext,
                schema: None,
                at_task: false,
            };
            let end = self.run_lane(lane, cancel);
            if let Some(why) = end.interrupt() {
                return interrupted(why);
            }
            match end {
                LaneEnd::Limit(_) | LaneEnd::Auth(_) | LaneEnd::Unavailable(_) | LaneEnd::Budget | LaneEnd::Cancelled => return, // taken by `interrupt` above
                LaneEnd::Failed(e) => return fail(e),
                LaneEnd::Restart => {
                    // silent again after the nudge: a new session in the same copy, and it counts as an attempt
                    let att = x.attempts + 1;
                    if att >= cap {
                        return fail("stalled: no output from the agent".into());
                    }
                    self.set_part(id, letter, |x| {
                        x.attempts = att;
                        x.session = None;
                        x.pin = None;
                    });
                }
                LaneEnd::Done { text, session, pin, model } => {
                    self.set_part(id, letter, |x| {
                        x.session = session;
                        x.pin = Some(pin);
                        x.model = model;
                        x.next_prompt = None;
                    });
                    let sig = prompt::parse_signals(&text);
                    if let Some((q, options)) = sig.needs_human {
                        // the question goes to the person with the part named; the answer comes back here (G15)
                        let s = self.part(id, letter).and_then(|x| x.session);
                        self.lane_asks(id, Asked { label: letter.to_string(), kind: "Part".into(), question: q, options, session: s });
                        return self.set_part(id, letter, |x| x.state = LaneState::Asking);
                    }
                    if let Err(e) = self.g_commit(&t, &x.worktree, &t.title) {
                        return fail(gerr(e));
                    }
                    // the app does not take the split on its word: of what the part changed, only the files it
                    // owns are applied, the rest is left behind
                    let changed: Vec<String> = git::diffstat(&x.worktree, &t.branch).into_iter().map(|f| f.path).collect();
                    let (own, outside): (Vec<String>, Vec<String>) = changed.into_iter().partition(|f| swarm::covers(&x.files, f));
                    if !outside.is_empty() {
                        self.log_part(id, letter, &format!("part_outside_files: {}", outside.join(", ")));
                    }
                    // landing: one at a time, in the same queue as main
                    let lock = self.merge_lock(&p.id);
                    let _one = loop {
                        match lock.try_lock() {
                            Ok(g) => break g,
                            Err(std::sync::TryLockError::Poisoned(e)) => break e.into_inner(),
                            Err(std::sync::TryLockError::WouldBlock) => {}
                        }
                        if cancel.load(Ordering::SeqCst) {
                            return interrupted("cancelled".into());
                        }
                        std::thread::sleep(Duration::from_millis(15));
                    };
                    // the part is put on the branch as it stands now (what landed before is in it) and the gates
                    // look at the two together: the same thing a rebase and a run of the gates would show
                    let Some(task_wt) = self.task(id).and_then(|t| t.worktree) else { return fail("no_worktree".into()) };
                    if let Err(e) = apply_files(&x.worktree, &task_wt, &own) {
                        let _ = git::restore_paths(&task_wt, "HEAD", &own);
                        return fail(e);
                    }
                    let Some(gates) = self.gate_results(id, p, &task_wt, cancel) else {
                        let _ = git::restore_paths(&task_wt, "HEAD", &own);
                        return interrupted("cancelled".into());
                    };
                    if gates.iter().all(|g| g.passed) {
                        if let Err(e) = self.g_commit(&t, &task_wt, &format!("{}: {}", t.title, x.title)) {
                            return fail(format!("part_not_landed: {}", gerr(e)));
                        }
                        self.log_part(id, letter, &format!("part_landed: {letter}"));
                        self.set_part(id, letter, |x| x.state = LaneState::Passed);
                        return;
                    }
                    // red: the part comes off the branch again and goes back to its worker with the output
                    let _ = git::restore_paths(&task_wt, "HEAD", &own);
                    drop(_one);
                    let att = x.attempts + 1;
                    if att >= cap {
                        return fail("tests_failed".into());
                    }
                    let out: Vec<String> = gates.iter().filter(|g| !g.passed).map(|g| format!("{}:\n{}", g.name, g.tail)).collect();
                    self.set_part(id, letter, |x| {
                        x.attempts = att;
                        x.next_prompt = Some(prompt::tests_failed_prompt(&out.join("\n\n")));
                    });
                }
            }
        }
    }

    fn log_part(&self, id: &str, letter: &str, text: &str) {
        let mut ev = LogEvent::new(self.now(), LogKind::Git, text);
        ev.role = Some(RoleKind::Worker);
        ev.variant = Some(letter.to_string());
        self.log_ev(id, ev);
    }
}
