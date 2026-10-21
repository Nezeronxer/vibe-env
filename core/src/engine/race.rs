//! A swarm on one task (G11): the dispatch into the three modes, and the race (Соревнование): one
//! preparation, N executors in copies of their own, the gates of each, the reviewer's marks, the
//! pick of the person or of the timer, then the usual merge queue. Also `pick_variant`, `stop_swarm`
//! and the view of the window.
use super::flow::{gerr, Flow, Res};
use super::lanes::{Lane, LaneEnd};
use super::roles::pick;
use super::state::{Advice, Asked, LaneState, Variant};
use super::steps::Structured;
use super::*;
use crate::models;
use crate::prompt::{self, Vendor};
use crate::swarm::view::*;
use crate::swarm::{self, parse, prompts};
use chrono::Duration as CDuration;

/// Key of the question of a stopped swarm that waits for a pick.
pub(super) const SPECIAL_SWARM_STOPPED: &str = "swarm_stopped";
const DIFF_FOR_COMPARE: usize = 30_000;

fn default_choice() -> ModelChoice {
    ModelChoice { family: models::DEFAULT_FAMILY.into(), version: None }
}

impl Engine {
    /// The "Code" step of a task with a swarm. None = not a swarm's business (the single executor goes).
    pub(super) fn swarm_code(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Option<Res> {
        let spec = t.swarm.clone()?;
        if self.state(&t.id).swarm.collapsed {
            return None;
        }
        Some(match spec.mode {
            SwarmMode::Race => self.race_code(t, p, spec.n, cancel),
            SwarmMode::Crew => self.crew_code(t, p, spec.n, cancel),
            SwarmMode::Council => match self.council_propose(t, p, spec.n, cancel) {
                Ok(None) => return None,
                Ok(Some(flow)) => Ok(flow),
                Err(e) => Err(e),
            },
        })
    }

    /// The "Review" step of a task with a swarm. None = the ordinary reviewer.
    pub(super) fn swarm_review(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Option<Res> {
        let spec = t.swarm.clone()?;
        let st = self.state(&t.id).swarm;
        if st.collapsed {
            return None;
        }
        match spec.mode {
            SwarmMode::Race if !st.variants.is_empty() => Some(self.race_choose(t, p, cancel)),
            SwarmMode::Council if st.council.decided => Some(self.council_review(t, p, spec.n, cancel)),
            _ => None,
        }
    }

    // ---- state of the variants ----

    pub(super) fn variant(&self, id: &str, letter: &str) -> Option<Variant> {
        self.state(id).swarm.variants.into_iter().find(|v| v.id == letter)
    }

    pub(super) fn set_variant(&self, id: &str, letter: &str, f: impl FnOnce(&mut Variant)) {
        let _ = self.update_state(id, |s| {
            if let Some(v) = s.swarm.variants.iter_mut().find(|v| v.id == letter) {
                f(v);
            }
        });
    }

    fn results_of(&self, variants: &[Variant], advice: Option<&str>) -> Vec<swarm::VariantResult> {
        variants
            .iter()
            .map(|v| swarm::VariantResult {
                id: v.id.clone(),
                passed: v.state == LaneState::Passed,
                green: v.green,
                total: v.total,
                scores: v.scores.map(|s| swarm::Scores { correctness: s[0], coverage: s[1], simplicity: s[2], risk: s[3] }),
                recommended: advice == Some(v.id.as_str()),
            })
            .collect()
    }

    // ---- the race ----

    fn race_code(&self, t: &mut Task, p: &Project, n: u8, cancel: &AtomicBool) -> Res {
        let id = t.id.clone();
        let wt = t.worktree.clone().ok_or("no_worktree")?;
        self.set_step(t, Step::Code, StepState::Running)?;
        if self.state(&id).swarm.variants.is_empty() {
            // the common preparation is committed: every variant starts from it
            self.g_commit(&wt, &t.title).map_err(gerr)?;
            let a = self.adapter(&t.agent).ok_or_else(|| format!("agent_not_found: {}", t.agent))?;
            let cat = models::catalog(&t.agent, &*a, &lk(&self.store));
            let worker = Self::choice_for(t, RoleKind::Worker).unwrap_or_else(default_choice);
            let dir = self.worktrees_dir.join(format!("{id}-swarm"));
            let mut variants = vec![];
            for (letter, model) in swarm::letters(n as usize).into_iter().zip(swarm::assign_models(&cat, &worker, n)) {
                let branch = format!("{}-{}", t.branch, letter.to_lowercase());
                let worktree = self.add_wt(&p.path, &dir, &letter, &branch, &t.branch).map_err(gerr)?;
                variants.push(Variant {
                    id: letter,
                    model,
                    branch,
                    worktree,
                    session: None,
                    pin: None,
                    state: LaneState::Waiting,
                    attempts: 0,
                    next_prompt: None,
                    gates: vec![],
                    green: 0,
                    total: 0,
                    summary: None,
                    scores: None,
                    why: None,
                    why_failed: None,
                    started_at: None,
                    finished_at: None,
                    files: vec![],
                });
            }
            self.update_state(&id, |s| s.swarm.variants = variants)?;
            self.stage(t, &format!("race: {n} variants"));
        }
        // the person answered a variant's question: it goes back to that variant, in its session
        if let Some((letter, answer)) = self.take_lane_answer(&id) {
            self.set_variant(&id, &letter, |v| {
                v.next_prompt = Some(answer);
                v.state = LaneState::Waiting;
            });
        }
        let todo: Vec<String> =
            self.state(&id).swarm.variants.iter().filter(|v| matches!(v.state, LaneState::Waiting | LaneState::Running | LaneState::Interrupted)).map(|v| v.id.clone()).collect();
        {
            let _mode = self.lanes_mode(&id);
            std::thread::scope(|sc| {
                for letter in &todo {
                    let (id, letter) = (&id, letter);
                    sc.spawn(move || self.variant_lane(id, p, letter, cancel));
                }
            });
        }
        // the lanes wrote run records and cost: take the task as it is now
        *t = self.task(&id).ok_or("task_not_found")?;
        if cancel.load(Ordering::SeqCst) {
            return Ok(Flow::Stop);
        }
        let sw = self.state(&id).swarm;
        if let Some(v) = sw.variants.iter().find(|v| v.state == LaneState::Interrupted) {
            let why = v.why_failed.clone().unwrap_or_default();
            return self.pause_for(t, p, &why);
        }
        if sw.variants.iter().any(|v| v.state == LaneState::Asking) {
            self.ask_next_lane(t, p, Status::Working)?;
            return Ok(Flow::Stop);
        }
        if sw.variants.iter().any(|v| v.state == LaneState::Passed) {
            // the gates of the variants are the gates of the task; the reviewer compares, a person or the timer picks
            for s in [Step::Code, Step::Gates] {
                self.set_step(t, s, StepState::Done)?;
            }
            self.set_step(t, Step::Review, StepState::Running)?;
            self.set(t, Status::Reviewing)?;
            self.stage(t, "race: comparing the variants");
            return self.race_compare(t, p, cancel);
        }
        // nobody passed: the best attempt stays in the task's copy, the person lifts it (G15.2)
        let results = self.results_of(&sw.variants, None);
        let best = swarm::best_failed(&results).ok_or("swarm_failed: no variant ran")?;
        let v = sw.variants.iter().find(|v| v.id == best).cloned().ok_or("swarm_failed")?;
        self.g_ff(&p.path, &t.branch, &v.branch).map_err(gerr)?;
        t.session = v.session.clone();
        t.models.insert(RoleKind::Worker, v.model.clone());
        t.outcome.gates = v.gates.clone();
        t.outcome.tests = v.gates.iter().find(|g| g.name == "tests").map(|g| TestResult { passed: g.passed, tail: g.tail.clone() });
        t.outcome.summary = v.summary.clone();
        self.update_state(&id, |s| s.swarm.collapsed = true)?;
        Err(format!("swarm_failed: best attempt {best}, {} of {} tests green", v.green, v.total))
    }

    /// One variant, start to end: a seat, a copy, the executor, the gates, again until green or out of attempts.
    fn variant_lane(&self, id: &str, p: &Project, letter: &str, cancel: &AtomicBool) {
        let Some(t) = self.task(id) else { return };
        let Some(v0) = self.variant(id, letter) else { return };
        let interrupted = |why: String| {
            self.set_variant(id, letter, |v| {
                v.state = LaneState::Interrupted;
                v.why_failed = Some(why);
            })
        };
        self.lane_waiting(id, RoleKind::Worker, letter, &v0.model);
        let Some(_seat) = self.take_seat(&t.agent, cancel) else { return interrupted("cancelled".into()) };
        self.set_variant(id, letter, |v| {
            v.state = LaneState::Running;
            v.started_at.get_or_insert_with(|| self.now());
            v.why_failed = None;
        });
        let fail = |why: String, gates: Option<Vec<GateResult>>| {
            self.log_variant(id, letter, LogKind::Stage, &format!("variant_failed: {letter}: {why}"));
            self.finish_variant(id, p, letter, LaneState::Failed, Some(why), gates);
        };
        if v0.started_at.is_none() {
            // a fresh copy gets the files outside git and the project's setup, as the task's own copy did
            match self.setup_copy(p, &v0.worktree, cancel) {
                Ok(true) => {}
                Ok(false) => return interrupted("cancelled".into()),
                Err(e) => return fail(e, None),
            }
        }
        let cap = lk(&self.store).settings().attempts.max(1);
        loop {
            let Some(v) = self.variant(id, letter) else { return };
            let next = v.next_prompt.clone();
            let ptext = |vendor: Vendor, resumed: bool| match (&next, resumed) {
                (Some(n), true) => n.clone(),
                (Some(n), false) => format!("{}\n\n{n}", self.worker_prompt(&t, p, vendor)),
                (None, true) => prompt::CONTINUE.to_string(),
                (None, false) => self.worker_prompt(&t, p, vendor),
            };
            let lane = Lane {
                task_id: id,
                project: p,
                role: RoleKind::Worker,
                label: letter,
                cwd: Some(v.worktree.clone()),
                model: v.model.clone(),
                session: v.session.clone(),
                pin: v.pin.clone(),
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
                LaneEnd::Failed(e) => return fail(e, None),
                LaneEnd::Restart => {
                    // silent again after the nudge: a new session in the same copy, and it counts as an attempt
                    let att = v.attempts + 1;
                    if att >= cap {
                        return fail("stalled: no output from the agent".into(), None);
                    }
                    self.set_variant(id, letter, |v| {
                        v.attempts = att;
                        v.session = None;
                        v.pin = None;
                    });
                }
                LaneEnd::Done { text, session, pin, model } => {
                    let sig = prompt::parse_signals(&text);
                    self.set_variant(id, letter, |v| {
                        v.session = session;
                        v.pin = Some(pin);
                        v.model = model;
                        v.next_prompt = None;
                        if !sig.body.is_empty() {
                            v.summary = Some(sig.body.chars().take(500).collect());
                        }
                    });
                    if let Some((q, options)) = sig.needs_human.clone() {
                        // the question goes to the person with the variant named; the answer comes back here (G15)
                        let s = self.variant(id, letter).and_then(|v| v.session);
                        self.lane_asks(id, Asked { label: letter.to_string(), kind: "Variant".into(), question: q, options, session: s });
                        return self.set_variant(id, letter, |v| v.state = LaneState::Asking);
                    }
                    if let Err(e) = self.g_commit(&v.worktree, &t.title) {
                        return fail(gerr(e), None);
                    }
                    let touched = match self.protect_tests(&t, &v.worktree) {
                        Ok(x) => x,
                        Err(e) => return fail(e, None),
                    };
                    if !touched.is_empty() {
                        let att = v.attempts + 1;
                        self.log_variant(id, letter, LogKind::Stage, &format!("tests_read_only: {}", touched.join(", ")));
                        if att >= cap {
                            return fail("tests_modified".into(), None);
                        }
                        self.set_variant(id, letter, |v| {
                            v.attempts = att;
                            v.next_prompt = Some(prompt::tests_readonly_prompt(&touched));
                        });
                        continue;
                    }
                    let Some(gates) = self.gate_results(id, p, &v.worktree, cancel) else { return interrupted("cancelled".into()) };
                    if gates.iter().all(|g| g.passed) {
                        self.log_variant(id, letter, LogKind::Stage, &format!("variant_passed: {letter}"));
                        return self.finish_variant(id, p, letter, LaneState::Passed, None, Some(gates));
                    }
                    let att = v.attempts + 1;
                    if att >= cap {
                        self.set_variant(id, letter, |v| v.attempts = att);
                        return fail("tests_failed".into(), Some(gates));
                    }
                    let out: Vec<String> = gates.iter().filter(|g| !g.passed).map(|g| format!("{}:\n{}", g.name, g.tail)).collect();
                    self.set_variant(id, letter, |v| {
                        v.attempts = att;
                        v.next_prompt = Some(prompt::tests_failed_prompt(&out.join("\n\n")));
                    });
                }
            }
        }
    }

    /// A fresh copy of a variant or a part gets what the task's own copy got: the files outside git and
    /// the project's setup. Ok(false) = cancelled.
    pub(super) fn setup_copy(&self, p: &Project, wt: &std::path::Path, cancel: &AtomicBool) -> Result<bool, String> {
        let copied = util::copy_files(&p.path, wt, &p.copy_files);
        if !copied.is_empty() {
            git::hide_untracked(wt, &copied).map_err(gerr)?;
        }
        if let Some(cmd) = p.setup_cmd.as_deref().filter(|c| !c.trim().is_empty()) {
            let Some((ok, out)) = util::sh(wt, cmd, cancel) else { return Ok(false) };
            if !ok {
                return Err(format!("setup_failed: {cmd}\n{}", util::tail(&out, 20)));
            }
        }
        Ok(true)
    }

    fn log_variant(&self, id: &str, letter: &str, kind: LogKind, text: &str) {
        let mut ev = LogEvent::new(self.now(), kind, text);
        ev.role = Some(RoleKind::Worker);
        ev.variant = Some(letter.to_string());
        self.log_ev(id, ev);
    }

    /// Writes the end of a variant: its state, the gates with their counts, the files it changed.
    fn finish_variant(&self, id: &str, p: &Project, letter: &str, state: LaneState, why: Option<String>, gates: Option<Vec<GateResult>>) {
        let Some(t) = self.task(id) else { return };
        let Some(v) = self.variant(id, letter) else { return };
        let stats = git::diffstat(&v.worktree, &t.branch);
        let _ = p;
        let counts = gates.as_ref().map(|g| {
            let all: String = g.iter().map(|x| x.tail.clone()).collect::<Vec<_>>().join("\n");
            match parse::count_tests(&all) {
                Some((pa, fa)) => (pa, pa + fa),
                None => (g.iter().filter(|x| x.passed).count() as u32, g.len() as u32),
            }
        });
        self.set_variant(id, letter, |v| {
            v.state = state;
            v.why_failed = why;
            v.finished_at = Some(self.now());
            if let Some(g) = gates {
                v.gates = g;
            }
            if let Some((green, total)) = counts {
                v.green = green;
                v.total = total;
            }
            v.files = stats.iter().map(|f| (f.path.clone(), f.insertions, f.deletions)).collect();
        });
    }

    /// The reviewer compares the variants that passed: marks, a recommendation, a word for the person.
    /// Then the task waits (status Reviewing) for the person's pick, or for the timer. No sound (G11.1).
    fn race_compare(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Res {
        let id = t.id.clone();
        let sw = self.state(&id).swarm;
        let passed: Vec<&Variant> = sw.variants.iter().filter(|v| v.state == LaneState::Passed).collect();
        let ids: Vec<String> = passed.iter().map(|v| v.id.clone()).collect();
        let shown: Vec<(String, String, String)> = passed
            .iter()
            .map(|v| (v.id.clone(), v.summary.clone().unwrap_or_default(), git::diff(&v.worktree, &t.branch, DIFF_FOR_COMPARE)))
            .collect();
        let request = prompts::compare_request(&t.prompt, &shown);
        let base = |_: Vendor, _: bool| request.clone();
        let parse_it = |s: &str| parse::parse_comparison(s, &ids);
        let r = self.structured(t, p, RoleKind::Reviewer, Status::Reviewing, prompts::compare_schema(), &base, &parse_it, cancel)?;
        let (marks, recommend, advice_text) = match r {
            Structured::Stopped => return Ok(Flow::Stop),
            Structured::Unusable(_) => {
                self.log(&id, LogKind::Review, prompt::BAD_FORMAT);
                (vec![], None, prompt::BAD_FORMAT.to_string())
            }
            Structured::Parsed(c) => (c.scores, Some(c.recommend), c.advice),
        };
        self.update_state(&id, |s| {
            for (v, m, why) in &marks {
                if let Some(x) = s.swarm.variants.iter_mut().find(|x| &x.id == v) {
                    x.scores = Some(*m);
                    x.why = why.clone();
                }
            }
        })?;
        let sw = self.state(&id).swarm;
        let ranked = swarm::rank(&self.results_of(&sw.variants, recommend.as_deref()));
        let best = ranked.first().cloned().ok_or("swarm_failed: nothing to pick")?;
        let settings = lk(&self.store).settings();
        let at = self.now() + CDuration::minutes(settings.autopick_min as i64);
        let text = if advice_text.is_empty() { format!("variant {best}") } else { advice_text };
        self.update_state(&id, |s| s.swarm.advice = Some(Advice { variant: best.clone(), text: text.clone(), autopick_at: at }))?;
        let mut ev = LogEvent::new(self.now(), LogKind::Review, &format!("race_advice: {best}: {text}"));
        ev.role = Some(RoleKind::Reviewer);
        self.log_ev(&id, ev);
        t.resume_at = Some(at); // the timer: the scheduler comes back at this time and takes the recommended one
        self.save(t)?;
        Ok(Flow::Stop)
    }

    /// The task woke up in "Reviewing" of a race: the timer is due (else it waits on), or it has no advice yet.
    fn race_choose(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Res {
        let sw = self.state(&t.id).swarm;
        if sw.stopped {
            return Ok(Flow::Stop);
        }
        let Some(advice) = sw.advice else { return self.race_compare(t, p, cancel) };
        if self.now() < advice.autopick_at {
            t.resume_at = Some(advice.autopick_at);
            self.save(t)?;
            return Ok(Flow::Stop);
        }
        self.adopt_variant(t, p, &advice.variant, "timer")?;
        Ok(Flow::Next)
    }

    /// The task's copy takes the work of the variant, and the task goes to the merge queue.
    fn adopt_variant(&self, t: &mut Task, p: &Project, letter: &str, by: &str) -> Result<(), String> {
        let v = self.variant(&t.id, letter).filter(|v| v.state == LaneState::Passed).ok_or("invalid_variant")?;
        self.g_ff(&p.path, &t.branch, &v.branch).map_err(gerr)?;
        t.session = v.session.clone();
        t.models.insert(RoleKind::Worker, v.model.clone());
        t.outcome.gates = v.gates.clone();
        t.outcome.tests = v.gates.iter().find(|g| g.name == "tests").map(|g| TestResult { passed: g.passed, tail: g.tail.clone() });
        t.outcome.summary = v.summary.clone();
        t.outcome.review = Some(Review { approved: true, issues: vec![] });
        t.question = None;
        t.resume_at = None;
        for (s, st) in t.steps.iter_mut() {
            if matches!(s, Step::Code | Step::Gates | Step::E2e | Step::Review) && !matches!(st, StepState::Skipped(_)) {
                *st = StepState::Done;
            }
        }
        let letter = letter.to_string();
        let by = by.to_string();
        self.update_state(&t.id, |s| {
            s.swarm.picked = Some(letter.clone());
            s.swarm.picked_by = Some(by.clone());
            if s.special.as_deref() == Some(SPECIAL_SWARM_STOPPED) {
                s.special = None;
            }
        })?;
        t.status = Status::Merging;
        self.save(t)?;
        self.stage(t, &format!("picked: {letter} ({by})"));
        Ok(())
    }

    /// "Взять A" (G11.1): the copy of the variant becomes the task's work and goes to the merge queue.
    pub fn pick_variant(&self, id: &str, variant: &str) -> Result<(), String> {
        let _h = self.hold(id, false)?;
        self.pick_locked(id, variant, "person")
    }

    /// `pick_variant` for whoever holds the claim already.
    pub(super) fn pick_locked(&self, id: &str, variant: &str, by: &str) -> Result<(), String> {
        let mut t = self.task(id).ok_or("task_not_found")?;
        let p = self.project(&t.project_id).ok_or("project_not_found")?;
        if t.swarm.as_ref().is_none_or(|s| s.mode != SwarmMode::Race) {
            return Err("not_a_race".into());
        }
        let st = self.state(id);
        let waiting = t.status == Status::Reviewing && st.swarm.advice.is_some();
        let stopped = t.status == Status::NeedsHuman && st.special.as_deref() == Some(SPECIAL_SWARM_STOPPED);
        if !(waiting || stopped) {
            return Err("not_waiting".into());
        }
        self.adopt_variant(&mut t, &p, variant, by)
    }

    /// "Остановить рой": every run of the swarm stops; what passed the gates stays and can be taken,
    /// the copies stay until the task is removed (G11.5).
    pub fn stop_swarm(&self, id: &str) -> Result<(), String> {
        let _h = self.hold(id, true)?;
        let mut t = self.task(id).ok_or("task_not_found")?;
        let Some(spec) = t.swarm.clone() else { return Err("not_a_swarm".into()) };
        if !matches!(t.status, Status::Working | Status::Checking | Status::Reviewing | Status::NeedsHuman | Status::PausedLimit) {
            return Err("not_stoppable".into());
        }
        for r in t.runs.iter_mut().filter(|r| matches!(r.state, StepState::Running | StepState::Pending)) {
            r.state = StepState::Failed("stopped".into());
        }
        let mut passed: Vec<String> = vec![];
        self.update_state(id, |s| {
            s.swarm.stopped = true;
            for v in s.swarm.variants.iter_mut() {
                match v.state {
                    LaneState::Passed => passed.push(v.id.clone()),
                    LaneState::Failed => {}
                    _ => {
                        v.state = LaneState::Failed;
                        v.why_failed = Some("stopped".into());
                    }
                }
            }
            for part in s.swarm.parts.iter_mut().filter(|x| !matches!(x.state, LaneState::Passed | LaneState::Failed)) {
                part.state = LaneState::Failed;
                part.why_failed = Some("stopped".into());
            }
            if !passed.is_empty() && spec.mode == SwarmMode::Race {
                s.special = Some(SPECIAL_SWARM_STOPPED.into());
            }
            s.swarm.collapsed = passed.is_empty();
        })?;
        t.resume_at = None;
        if passed.is_empty() {
            // nothing to take: "Не вышло", without a call (the person pressed stop)
            t.status = Status::Failed;
            t.outcome.error = Some("swarm_stopped".into());
            t.finished_at = Some(self.now());
        } else {
            t.status = Status::NeedsHuman;
            t.question = Some(Question { text: SPECIAL_SWARM_STOPPED.into(), options: passed.clone(), kind: QuestionKind::Agent });
        }
        self.save(&t)?;
        self.log(id, LogKind::Human, &format!("swarm_stopped: {} passed", passed.len()));
        Ok(())
    }

    /// Removes the copies of variants and parts (with their branches); the task's own copy is the caller's.
    pub(super) fn drop_swarm_copies(&self, id: &str, p: &Project) {
        let sw = self.state(id).swarm;
        let dirs = sw.variants.iter().map(|v| v.worktree.clone()).chain(sw.parts.iter().map(|x| x.worktree.clone()));
        for d in dirs {
            if let Err(e) = self.rm_wt(&p.path, &d, true) {
                self.log(id, LogKind::Error, &format!("copy_not_removed: {}: {e}", d.display()));
            }
        }
        let root = self.worktrees_dir.join(format!("{id}-swarm"));
        if let Err(e) = std::fs::remove_dir_all(&root) {
            if root.exists() {
                self.log(id, LogKind::Error, &format!("copy_not_removed: {}: {e}", root.display()));
            }
        }
    }

    /// The copy of a variant or a part (the diff of the window is read there).
    pub fn swarm_copy(&self, id: &str, variant: &str) -> Option<PathBuf> {
        let sw = self.state(id).swarm;
        sw.variants.iter().find(|v| v.id == variant).map(|v| v.worktree.clone()).or_else(|| sw.parts.iter().find(|x| x.id == variant).map(|x| x.worktree.clone()))
    }

    /// What the swarm screen shows (`Task.swarm_view` of the window). None = the task has no swarm.
    pub fn swarm_view(&self, id: &str) -> Option<SwarmView> {
        let t = self.task(id)?;
        let spec = t.swarm.clone()?;
        let st = self.state(id).swarm;
        let a = self.adapter(&t.agent);
        let cat = a.as_ref().map(|a| models::catalog(&t.agent, &**a, &lk(&self.store)));
        let label = |m: &ModelChoice| cat.as_ref().map(|c| pick(c, &t.agent, Some(m)).label).unwrap_or_else(|| m.family.clone());
        let ran_label = |letter: &str, role: RoleKind, m: &ModelChoice| {
            t.runs.iter().rev().find(|r| r.role == role && r.variant.as_deref() == Some(letter)).map(|r| r.model.clone()).unwrap_or_else(|| label(m))
        };
        let mut view = SwarmView { stopped: st.stopped, picked: st.picked.clone(), picked_by: st.picked_by.clone(), ..Default::default() };
        view.variants = st
            .variants
            .iter()
            .map(|v| {
                let cost: f64 = t.runs.iter().filter(|r| r.variant.as_deref() == Some(v.id.as_str())).filter_map(|r| r.cost_usd).sum();
                let minutes = v.started_at.zip(v.finished_at).map(|(a, b)| (b - a).num_minutes().max(0) as u32);
                VariantView {
                    id: v.id.clone(),
                    model: ran_label(&v.id, RoleKind::Worker, &v.model),
                    state: match v.state {
                        LaneState::Waiting => "queued",
                        LaneState::Running | LaneState::Interrupted | LaneState::Asking => "running",
                        LaneState::Passed => "passed",
                        LaneState::Failed => "failed",
                    }
                    .into(),
                    gate: (v.total > 0 || !v.gates.is_empty()).then_some(Gate { passed: v.green, total: v.total }),
                    score: v.scores.map(|s| s.iter().map(|x| *x as f32).sum::<f32>() / 4.0),
                    summary: v.summary.clone(),
                    criteria: v
                        .scores
                        .map(|s| ["correctness", "coverage", "simplicity", "risk"].iter().zip(s).map(|(k, x)| Criterion { key: k.to_string(), value: x }).collect())
                        .unwrap_or_default(),
                    why: v.why.clone(),
                    stats: Some(Stats { ins: v.files.iter().map(|f| f.1).sum(), del: v.files.iter().map(|f| f.2).sum(), minutes }).filter(|_| !v.files.is_empty()),
                    cost_usd: (cost > 0.0).then_some(cost),
                    why_failed: v.why_failed.clone().filter(|_| v.state == LaneState::Failed),
                    files: v.files.iter().map(|f| FileView { path: f.0.clone(), insertions: f.1, deletions: f.2 }).collect(),
                    worktree: v.worktree.to_string_lossy().to_string(),
                }
            })
            .collect();
        view.advice = st.advice.as_ref().map(|a| AdviceView {
            variant: a.variant.clone(),
            model: st.variants.iter().find(|v| v.id == a.variant).map(|v| ran_label(&v.id, RoleKind::Worker, &v.model)),
            text: a.text.clone(),
            autopick_at: a.autopick_at.to_rfc3339(),
        });
        view.parts = st
            .parts
            .iter()
            .map(|x| PartView {
                id: x.id.clone(),
                title: x.title.clone(),
                files: x.files.clone(),
                worker: format!("Worker {}", x.id),
                model: ran_label(&x.id, RoleKind::Worker, &x.model),
                state: match x.state {
                    LaneState::Waiting => "waiting",
                    LaneState::Running | LaneState::Interrupted | LaneState::Asking => "running",
                    LaneState::Passed => "done",
                    LaneState::Failed => "failed",
                }
                .into(),
            })
            .collect();
        if spec.mode == SwarmMode::Council {
            let c = &st.council;
            let v = |x: &super::state::VoteRec| VoteView { voter: x.voter.clone(), for_: x.choice.clone(), why: x.why.clone() };
            view.council = Some(CouncilView {
                proposals: c.proposals.iter().map(|x| ProposalView { id: x.id.clone(), model: x.model.clone(), text: x.text.clone() }).collect(),
                votes: c.votes.iter().map(v).collect(),
                winner: c.winner.clone(),
                decided_by: c.decided_by.clone(),
                review_votes: c.review_votes.iter().map(v).collect(),
            });
        }
        Some(view)
    }
}
