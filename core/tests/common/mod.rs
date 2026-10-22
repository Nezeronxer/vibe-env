//! Harness of the engine tests: a temp repo, a store, the fake adapters, waiting helpers.
#![allow(dead_code)]
use chrono::{DateTime, Local};
use nightshift_core::agent::{Adapter, FakeAdapter, FakeStep};
use nightshift_core::engine::{Clock, Engine, NewTask};
use nightshift_core::model::*;
use nightshift_core::notify::{Notice, NoticeKind, RecordingNotifier};
use nightshift_core::store::Store;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tempfile::TempDir;

pub fn g(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").current_dir(dir).args(args).output().unwrap();
    assert!(o.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

pub const APPROVE: &str = r#"{"verdict":"approve","issues":[]}"#;

pub fn approve() -> Vec<FakeStep> {
    vec![FakeStep::Done(APPROVE.into())]
}
pub fn done(text: &str) -> FakeStep {
    FakeStep::Done(text.into())
}
pub fn write(path: &str, body: &str) -> FakeStep {
    FakeStep::Write(path.into(), body.into())
}

/// A temp dir with a repo (one commit on main, `a.txt`), the store and the worktrees dir.
pub struct Env {
    pub d: TempDir,
    pub repo: PathBuf,
}

impl Env {
    pub fn new() -> Env {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        fs::create_dir(&repo).unwrap();
        g(&repo, &["init", "-q", "-b", "main"]);
        g(&repo, &["config", "user.email", "t@example.com"]);
        g(&repo, &["config", "user.name", "T"]);
        fs::write(repo.join("a.txt"), "one\n").unwrap();
        g(&repo, &["add", "-A"]);
        g(&repo, &["commit", "-q", "-m", "init"]);
        Env { d, repo }
    }

    pub fn project(&self, f: impl FnOnce(&mut Project)) -> Project {
        let mut p = Project::new("demo", self.repo.clone(), "main");
        p.agent = "fake".into();
        p.reviewer = Some("rev".into());
        p.finish = Finish::Local;
        f(&mut p);
        p
    }

    /// The store of a v1-style test: "Quality" is off, so the short path Code -> Gates -> Review -> Merge runs.
    pub fn store(&self, p: &Project, settings: impl FnOnce(&mut Settings)) -> Store {
        self.store_v2(p, |s| {
            s.quality.on = false;
            settings(s);
        })
    }

    /// The store with the defaults of v2 (Quality on) and whatever the test changes.
    pub fn store_v2(&self, p: &Project, settings: impl FnOnce(&mut Settings)) -> Store {
        let mut st = Store::open(&self.d.path().join("data")).unwrap();
        st.upsert_project(p.clone()).unwrap();
        let mut s = st.settings();
        settings(&mut s);
        st.set_settings(s).unwrap();
        st
    }

    pub fn engine(&self, store: Store, adapters: Vec<Arc<dyn Adapter>>, n: Arc<RecordingNotifier>, clock: Option<Clock>) -> Arc<Engine> {
        // the envs of the test do not ask before push / PR unless the test says so in the settings
        let mut store = store;
        let mut s = store.settings();
        for a in &adapters {
            s.envs.entry(a.id().to_string()).or_insert_with(|| EnvSettings { ask_push: false, ask_pr: false, ..EnvSettings::default() });
        }
        store.set_settings(s).unwrap();
        let adapters = route_reviewers(adapters);
        let wts = self.d.path().join("wts");
        let e = match clock {
            Some(c) => Engine::new_with_clock(store, adapters, n, wts, c),
            None => Engine::new(store, adapters, n, wts),
        };
        e.start();
        e
    }

    pub fn main_count(&self) -> u32 {
        g(&self.repo, &["rev-list", "--count", "main"]).parse().unwrap()
    }
    pub fn main_file(&self, f: &str) -> String {
        g(&self.repo, &["show", &format!("main:{f}")])
    }
}

pub fn task(e: &Engine, id: &str) -> Task {
    e.store().lock().unwrap().task(id).unwrap()
}


pub fn wait(e: &Engine, id: &str, what: &str, pred: impl Fn(&Task) -> bool) -> Task {
    let start = Instant::now();
    loop {
        let t = task(e, id);
        if pred(&t) {
            return t;
        }
        assert!(start.elapsed() < Duration::from_secs(90), "timeout waiting for {what}; task is {:?}: {:?}", t.status, t.outcome);
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Waits for `f` (the bound the tests' own copies of it had).
pub fn until(what: &str, mut f: impl FnMut() -> bool) {
    let start = Instant::now();
    while !f() {
        assert!(start.elapsed() < Duration::from_secs(60), "timeout: {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn wait_status(e: &Engine, id: &str, s: Status) -> Task {
    wait(e, id, &format!("{s:?}"), |t| t.status == s)
}

pub fn submit(e: &Engine, p: &Project, prompt: &str) -> String {
    e.submit(NewTask { project_id: p.id.clone(), prompt: prompt.into(), ..NewTask::default() }).unwrap()
}

/// The notice follows the status change, so wait until `count` notices have arrived.
pub fn notices(n: &RecordingNotifier, count: usize) -> Vec<Notice> {
    let start = Instant::now();
    while n.0.lock().unwrap().len() < count && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
    }
    n.0.lock().unwrap().clone()
}

pub fn kinds(n: &RecordingNotifier) -> Vec<NoticeKind> {
    notices(n, 1).iter().map(|x| x.kind.clone()).collect()
}

pub fn fake(scripts: Vec<Vec<FakeStep>>) -> Arc<FakeAdapter> {
    Arc::new(FakeAdapter::new(scripts))
}
pub fn reviewer(n: usize) -> Arc<FakeAdapter> {
    Arc::new(FakeAdapter::new((0..n).map(|_| approve()).collect()).with_id("rev"))
}

/// FakeAdapter whose invocation number `gated` first writes its shell pid to `dir/pid` and then
/// waits until the test creates `dir/gate`: the test knows the agent is running and holds it there.
pub struct Gated {
    pub inner: FakeAdapter,
    gated: usize,
    dir: PathBuf,
}

impl Gated {
    pub fn new(id: &str, scripts: Vec<Vec<FakeStep>>, gated: usize, dir: &Path) -> Arc<Gated> {
        Arc::new(Gated { inner: FakeAdapter::new(scripts).with_id(id), gated, dir: dir.to_path_buf() })
    }
    /// Holds invocation number `gated` (counted over all roles) of a fake that has scripts per role.
    pub fn wrap(inner: FakeAdapter, gated: usize, dir: &Path) -> Arc<Gated> {
        Arc::new(Gated { inner, gated, dir: dir.to_path_buf() })
    }
    pub fn calls(&self) -> usize {
        self.inner.calls().len()
    }
    /// Pid of the gated agent once it is running.
    pub fn wait_started(&self) -> u32 {
        let f = self.dir.join("pid");
        let start = Instant::now();
        loop {
            if let Some(pid) = fs::read_to_string(&f).ok().and_then(|s| s.trim().parse().ok()) {
                return pid;
            }
            assert!(start.elapsed() < Duration::from_secs(30), "gated agent never started");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    pub fn open(&self) {
        fs::write(self.dir.join("gate"), "").unwrap();
    }
}

impl Adapter for Gated {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn name(&self) -> &str {
        "Gated"
    }
    fn available(&self) -> Result<(), String> {
        Ok(())
    }
    fn command(&self, run: &nightshift_core::agent::Run) -> Command {
        let n = self.calls();
        let inner = self.inner.command(run);
        let script = inner.get_args().nth(1).unwrap().to_string_lossy().to_string();
        let script = if n == self.gated {
            let (pid, gate) = (self.dir.join("pid"), self.dir.join("gate"));
            format!("echo $$ > '{}'\nwhile [ ! -e '{}' ]; do sleep 0.02; done\n{script}", pid.display(), gate.display())
        } else {
            script
        };
        let mut c = Command::new("sh");
        c.arg("-c").arg(script).current_dir(&run.cwd).stdout(std::process::Stdio::piped());
        c
    }
    fn parse(&self, line: &str) -> Vec<nightshift_core::agent::AgentEvent> {
        self.inner.parse(line)
    }
    fn finish(&self, ok: bool, all: &str) -> Vec<nightshift_core::agent::AgentEvent> {
        self.inner.finish(ok, all)
    }
}

/// Wraps a fake: every run of `role` waits until the test calls `open()`. A state the test looks at (two runs going
/// at once, a third waiting for a seat) then holds still until the test has seen it, instead of lasting only as long
/// as a sleep of the fake, which a machine under load may not give the test a turn to see.
pub struct HoldRole {
    pub inner: FakeAdapter,
    role: RoleKind,
    gate: PathBuf,
}

impl HoldRole {
    pub fn wrap(inner: FakeAdapter, role: RoleKind, dir: &Path) -> Arc<HoldRole> {
        Arc::new(HoldRole { inner, role, gate: dir.join("hold-role-gate") })
    }
    pub fn open(&self) {
        fs::write(&self.gate, "").unwrap();
    }
}

impl Adapter for HoldRole {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn available(&self) -> Result<(), String> {
        self.inner.available()
    }
    fn command(&self, run: &nightshift_core::agent::Run) -> Command {
        let inner = self.inner.command(run);
        if run.role != self.role {
            return inner;
        }
        let script = inner.get_args().nth(1).unwrap().to_string_lossy().to_string();
        let mut c = Command::new("sh");
        c.arg("-c").arg(format!("while [ ! -e '{}' ]; do sleep 0.02; done\n{script}", self.gate.display())).current_dir(&run.cwd).stdout(std::process::Stdio::piped());
        c
    }
    fn parse(&self, line: &str) -> Vec<nightshift_core::agent::AgentEvent> {
        self.inner.parse(line)
    }
    fn finish(&self, ok: bool, all: &str) -> Vec<nightshift_core::agent::AgentEvent> {
        self.inner.finish(ok, all)
    }
    fn models(&self) -> Result<Vec<nightshift_core::model::ModelFamily>, String> {
        self.inner.models()
    }
    fn read_only_native(&self) -> bool {
        self.inner.read_only_native()
    }
    fn reports_cost(&self) -> bool {
        self.inner.reports_cost()
    }
}

pub fn alive(pid: u32) -> bool {
    Command::new("kill").args(["-0", &pid.to_string()]).stderr(std::process::Stdio::null()).status().unwrap().success()
}

/// One fake `gh` for the whole test binary (NIGHTSHIFT_GH is process-wide): logs its arguments.
pub fn fake_gh() -> PathBuf {
    static GH: std::sync::OnceLock<TempDir> = std::sync::OnceLock::new();
    let d = GH.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("gh");
        let log = d.path().join("calls");
        fs::write(&p, format!("#!/bin/sh\necho \"$@\" >> '{}'\necho https://github.com/o/r/pull/9\n", log.display())).unwrap();
        Command::new("chmod").arg("755").arg(&p).status().unwrap();
        std::env::set_var("NIGHTSHIFT_GH", &p);
        d
    });
    d.path().join("calls")
}

pub fn gh_calls_for(log: &Path, branch: &str) -> usize {
    fs::read_to_string(log).unwrap_or_default().lines().filter(|l| l.contains(branch)).count()
}

pub fn fake_as(id: &str, scripts: Vec<Vec<FakeStep>>) -> Arc<FakeAdapter> {
    Arc::new(FakeAdapter::new(scripts).with_id(id))
}

pub fn submit_with(e: &Engine, p: &Project, prompt: &str, agent: &str) -> String {
    e.submit(NewTask { project_id: p.id.clone(), prompt: prompt.into(), agent: Some(agent.into()), ..NewTask::default() }).unwrap()
}

/// A clock that only moves when the test says so.
pub fn manual_clock() -> (Clock, Arc<Mutex<DateTime<Local>>>) {
    let now = Arc::new(Mutex::new(Local::now()));
    let n = now.clone();
    (Arc::new(move || *n.lock().unwrap()), now)
}

pub fn with_origin(env: &Env) -> PathBuf {
    let bare = env.d.path().join("origin.git");
    g(env.d.path(), &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()]);
    g(&env.repo, &["remote", "add", "origin", bare.to_str().unwrap()]);
    g(&env.repo, &["push", "-q", "-u", "origin", "main"]);
    bare
}


/// The v1 tests script the reviewer in an adapter of its own ("rev"). The engine knows one adapter per
/// task now, so each other adapter is wrapped: runs of the Reviewer role are served by the scripts of
/// "rev", everything else by the adapter itself. The engine sees a single env; this only routes scripts.
pub struct Routed {
    inner: Arc<dyn Adapter>,
    reviewer: Arc<dyn Adapter>,
}

impl Adapter for Routed {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn available(&self) -> Result<(), String> {
        self.inner.available()
    }
    fn command(&self, run: &nightshift_core::agent::Run) -> Command {
        if run.role == RoleKind::Reviewer {
            self.reviewer.command(run)
        } else {
            self.inner.command(run)
        }
    }
    fn parse(&self, line: &str) -> Vec<nightshift_core::agent::AgentEvent> {
        self.inner.parse(line)
    }
    fn finish(&self, ok: bool, all: &str) -> Vec<nightshift_core::agent::AgentEvent> {
        self.inner.finish(ok, all)
    }
    fn read_only_native(&self) -> bool {
        self.inner.read_only_native()
    }
    fn reports_cost(&self) -> bool {
        self.inner.reports_cost()
    }
    fn interactive_resume(&self, session: &str, cwd: &Path) -> Option<Command> {
        self.inner.interactive_resume(session, cwd)
    }
    fn models(&self) -> Result<Vec<ModelFamily>, String> {
        self.inner.models()
    }
    fn builtin_models(&self) -> Vec<ModelFamily> {
        self.inner.builtin_models()
    }
}

pub fn route_reviewers(adapters: Vec<Arc<dyn Adapter>>) -> Vec<Arc<dyn Adapter>> {
    let Some(rev) = adapters.iter().find(|a| a.id() == "rev").cloned() else { return adapters };
    adapters
        .into_iter()
        .map(|a| if a.id() == "rev" { a } else { Arc::new(Routed { inner: a, reviewer: rev.clone() }) as Arc<dyn Adapter> })
        .collect()
}

// ---- scripts of the roles of the pipeline ----

pub fn brief_json(goal: &str, plan: &[&str], trivial: bool, vague: bool, risky: Option<&str>) -> String {
    serde_json::json!({
        "goal": goal, "constraints": ["keep the public API"], "done_when": ["feature.txt exists"], "files": ["a.txt"],
        "vague": vague, "trivial": trivial, "risky": risky, "plan": plan,
    })
    .to_string()
}

pub const CRITIQUE_OK: &str = r#"{"ok":true,"issues":[],"plan":[]}"#;
pub const SPEC_JSON: &str = r#"{"spec":"The feature file holds one line.","done_when":["feature.txt has one line"]}"#;

/// Fake env with scripts per role for `n` tasks through the whole pipeline: plan, critique, tests, code, review.
/// The first task's code writes `feature.txt`, the k-th writes `feature{k}.txt`.
pub fn pipeline_fake_n(id: &str, n: usize) -> FakeAdapter {
    let rep = |s: Vec<FakeStep>| (0..n).map(|_| s.clone()).collect::<Vec<_>>();
    FakeAdapter::new(vec![])
        .with_id(id)
        .with_role(RoleKind::Planner, rep(vec![done(&brief_json("Add the feature file", &["write feature.txt", "run the checks"], false, false, None))]))
        .with_role(RoleKind::Critic, rep(vec![done(CRITIQUE_OK)]))
        .with_role(RoleKind::TestAuthor, (0..n).map(|k| vec![write(&format!("t{k}_test.txt"), "expect feature\n"), done("tests written")]).collect())
        .with_role(
            RoleKind::Worker,
            (0..n).map(|k| vec![write(&if k == 0 { "feature.txt".to_string() } else { format!("feature{k}.txt") }, "hello\n"), done("Added the feature")]).collect(),
        )
        .with_role(RoleKind::Reviewer, rep(approve()))
}

pub fn pipeline_fake(id: &str) -> FakeAdapter {
    pipeline_fake_n(id, 1)
}

/// Calls of an adapter for one role.
pub fn calls_of(a: &FakeAdapter, role: RoleKind) -> Vec<nightshift_core::agent::Run> {
    a.calls().into_iter().filter(|r| r.role == role).collect()
}

pub fn roles_in_order(a: &FakeAdapter) -> Vec<RoleKind> {
    a.calls().iter().map(|r| r.role).collect()
}

/// Settings of one env with models of its own (custom models need no catalog).
pub fn env_models(s: &mut Settings, env: &str, models: &[&str], default: &str, roles: &[(RoleKind, &str)], fallback: Option<&str>) {
    let m = |f: &str| ModelChoice { family: f.into(), version: None };
    s.envs.insert(
        env.into(),
        EnvSettings {
            custom_models: models.iter().map(|x| x.to_string()).collect(),
            default_model: Some(m(default)),
            roles: roles.iter().map(|(r, f)| (*r, m(f))).collect(),
            fallback: fallback.map(m),
            ask_push: false,
            ask_pr: false,
            ..EnvSettings::default()
        },
    );
}

pub fn log_texts(e: &Engine, id: &str) -> Vec<String> {
    e.store().lock().unwrap().read_log(id, 5000).into_iter().map(|l| l.text).collect()
}

pub fn has_log(e: &Engine, id: &str, needle: &str) -> bool {
    log_texts(e, id).iter().any(|l| l.contains(needle))
}

pub fn kinds_of(n: &RecordingNotifier) -> Vec<NoticeKind> {
    n.0.lock().unwrap().iter().map(|x| x.kind.clone()).collect()
}

/// A fake env with switches: whether it reports cost, whether the CLI can continue a session in a
/// terminal, whether it is installed at all.
pub struct Knobs {
    pub inner: Arc<dyn Adapter>,
    pub cost: bool,
    pub resumable: bool,
    pub installed: Arc<std::sync::atomic::AtomicBool>,
}

impl Knobs {
    pub fn new(inner: Arc<dyn Adapter>) -> Knobs {
        Knobs { inner, cost: true, resumable: false, installed: Arc::new(std::sync::atomic::AtomicBool::new(true)) }
    }
    pub fn no_cost(mut self) -> Knobs {
        self.cost = false;
        self
    }
    pub fn resumable(mut self) -> Knobs {
        self.resumable = true;
        self
    }
}

impl Adapter for Knobs {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn name(&self) -> &str {
        "Knobs"
    }
    fn available(&self) -> Result<(), String> {
        if self.installed.load(std::sync::atomic::Ordering::SeqCst) {
            Ok(())
        } else {
            Err("the CLI was removed".into())
        }
    }
    fn command(&self, run: &nightshift_core::agent::Run) -> Command {
        self.inner.command(run)
    }
    fn parse(&self, line: &str) -> Vec<nightshift_core::agent::AgentEvent> {
        self.inner.parse(line)
    }
    fn finish(&self, ok: bool, all: &str) -> Vec<nightshift_core::agent::AgentEvent> {
        self.inner.finish(ok, all)
    }
    fn reports_cost(&self) -> bool {
        self.cost
    }
    fn read_only_native(&self) -> bool {
        self.inner.read_only_native()
    }
    fn interactive_resume(&self, session: &str, cwd: &Path) -> Option<Command> {
        self.resumable.then(|| {
            let mut c = Command::new("echo");
            c.arg("resume").arg(session).current_dir(cwd);
            c
        })
    }
}

/// A clock that stands at `h:m` today and moves only when the test says so.
pub fn clock_at(h: u32, m: u32) -> (Clock, Arc<Mutex<DateTime<Local>>>) {
    use chrono::TimeZone;
    let d = Local::now().date_naive();
    let t = Local.from_local_datetime(&d.and_hms_opt(h, m, 0).unwrap()).unwrap();
    let now = Arc::new(Mutex::new(t));
    let n = now.clone();
    (Arc::new(move || *n.lock().unwrap()), now)
}

pub fn advance(now: &Arc<Mutex<DateTime<Local>>>, secs: i64) {
    let mut g = now.lock().unwrap();
    *g += chrono::Duration::seconds(secs);
}

pub fn window(name: &str, resets: DateTime<Local>, exhausted: bool, now: DateTime<Local>) -> LimitWindow {
    LimitWindow {
        name: name.into(),
        model: None,
        used_pct: Some(if exhausted { 100.0 } else { 54.0 }),
        remaining: None,
        resets_at: Some(resets),
        exhausted,
        source: "rate_limit_event".into(),
        seen_at: now,
    }
}
