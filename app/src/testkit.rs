//! Seam 1 for the app: `Hub` over `Engine` + `FakeAdapter` + a temp repository + a clock the test moves.
use crate::hub::{lk, Hub};
use chrono::{DateTime, Local, TimeZone};
use nightshift_core::agent::{Adapter, FakeAdapter};
use nightshift_core::kits::{Kits, SecretStore};
use nightshift_core::model::{EnvSettings, Project, Task};
use nightshift_core::notify::RecordingNotifier;
use nightshift_core::store::Store;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub fn g(dir: &Path, args: &[&str]) -> String {
    let o = nightshift_core::proc::cmd("git").current_dir(dir).args(args).output().unwrap();
    assert!(o.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

/// A scratch folder that goes away with the test.
pub struct Scratch(pub PathBuf);
impl Scratch {
    pub fn new(name: &str) -> Scratch {
        static N: AtomicU32 = AtomicU32::new(0);
        let d = std::env::temp_dir().join(format!("ns-app-{name}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Scratch(d.canonicalize().unwrap())
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The system keyring replaced by a map: tests never touch the real one.
#[derive(Default)]
pub struct MemKeyring(pub Mutex<HashMap<String, String>>);
impl SecretStore for MemKeyring {
    fn set(&self, service: &str, user: &str, value: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(format!("{service}|{user}"), value.into());
        Ok(())
    }
    fn get(&self, service: &str, user: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().get(&format!("{service}|{user}")).cloned())
    }
}

pub struct Rig {
    pub s: Scratch,
    pub repo: PathBuf,
    pub hub: Arc<Hub>,
    pub project: Project,
    pub fake: Arc<FakeAdapter>,
}

pub fn at(h: u32, m: u32) -> DateTime<Local> {
    Local.with_ymd_and_hms(2026, 10, 21, h, m, 0).unwrap()
}

impl Rig {
    pub fn new(fake: FakeAdapter) -> Rig {
        Rig::with(fake, |_| {})
    }

    /// `tweak` changes the settings before the engine starts.
    pub fn with(fake: FakeAdapter, tweak: impl FnOnce(&mut nightshift_core::model::Settings)) -> Rig {
        Rig::build(fake, false, tweak, |_| {})
    }

    pub fn build(fake: FakeAdapter, resumable: bool, tweak: impl FnOnce(&mut nightshift_core::model::Settings), tweak_project: impl FnOnce(&mut Project)) -> Rig {
        let s = Scratch::new("rig");
        let repo = s.0.join("repo");
        std::fs::create_dir(&repo).unwrap();
        g(&repo, &["init", "-q", "-b", "main"]);
        g(&repo, &["config", "user.email", "t@example.com"]);
        g(&repo, &["config", "user.name", "T"]);
        std::fs::write(repo.join("a.txt"), "one\n").unwrap();
        g(&repo, &["add", "-A"]);
        g(&repo, &["commit", "-q", "-m", "init"]);
        let mut project = Project::new("demo", repo.clone(), "main");
        let id = fake.id().to_string();
        project.agent = id.clone();
        tweak_project(&mut project);
        let mut store = Store::open(&s.0.join("data")).unwrap();
        store.upsert_project(project.clone()).unwrap();
        let mut st = store.settings();
        st.quality.on = false;
        st.envs.insert(id, EnvSettings { ask_push: false, ask_pr: false, ..EnvSettings::default() });
        tweak(&mut st);
        store.set_settings(st).unwrap();
        let fake = Arc::new(fake);
        let notes = Arc::new(RecordingNotifier::default());
        let noon = at(12, 0);
        let kits = Kits::new(s.0.join("home"), s.0.join("data").join("kits")).with_secret_store(Box::new(MemKeyring::default()));
        let adapters: Vec<Arc<dyn Adapter>> = vec![if resumable { Arc::new(Resumable(fake.clone())) } else { fake.clone() }];
        let hub = Hub::new(store, adapters, notes.clone(), kits, s.0.join("data"), Arc::new(move || noon));
        hub.start();
        Rig { s, repo, hub, project, fake }
    }

    pub fn task(&self, id: &str) -> Task {
        lk(self.hub.engine.store()).task(id).unwrap()
    }

    pub fn wait_task(&self, id: &str, what: &str, pred: impl Fn(&Task) -> bool) -> Task {
        until(what, || pred(&self.task(id)));
        self.task(id)
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.hub.chats.shutdown();
        self.hub.engine.shutdown();
    }
}

/// The fake with an `interactive_resume` like Claude's: `<bin> --resume <id>` in the copy.
pub struct Resumable(pub Arc<FakeAdapter>);
impl Adapter for Resumable {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn name(&self) -> &str {
        self.0.name()
    }
    fn available(&self) -> Result<(), String> {
        Ok(())
    }
    fn command(&self, run: &nightshift_core::agent::Run) -> Command {
        self.0.command(run)
    }
    fn parse(&self, line: &str) -> Vec<nightshift_core::agent::AgentEvent> {
        self.0.parse(line)
    }
    fn finish(&self, ok: bool, all: &str) -> Vec<nightshift_core::agent::AgentEvent> {
        self.0.finish(ok, all)
    }
    fn interactive_resume(&self, session: &str, cwd: &Path) -> Option<Command> {
        let mut c = nightshift_core::proc::cmd("/opt/bin/claude");
        c.current_dir(cwd).args(["--resume", session]);
        Some(c)
    }
}

impl Rig {
    /// A bare origin with main pushed, for the tests of push and PR.
    pub fn with_origin(&self) -> PathBuf {
        let bare = self.s.0.join("origin.git");
        g(&self.s.0, &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()]);
        g(&self.repo, &["remote", "add", "origin", bare.to_str().unwrap()]);
        g(&self.repo, &["push", "-q", "-u", "origin", "main"]);
        bare
    }
}

/// One fake `gh` for the whole test binary (`NIGHTSHIFT_GH` is process-wide): logs its arguments, prints a PR link.
pub fn fake_gh() -> PathBuf {
    static GH: std::sync::OnceLock<Scratch> = std::sync::OnceLock::new();
    let d = GH.get_or_init(|| {
        let d = Scratch::new("gh");
        let p = d.0.join("gh");
        std::fs::write(&p, format!("#!/bin/sh\necho \"$@\" >> '{}'\necho https://github.com/o/r/pull/9\n", d.0.join("calls").display())).unwrap();
        nightshift_core::proc::cmd("chmod").arg("755").arg(&p).status().unwrap();
        std::env::set_var("NIGHTSHIFT_GH", &p);
        d
    });
    d.0.join("calls")
}

pub fn gh_calls_for(log: &Path, branch: &str) -> usize {
    std::fs::read_to_string(log).unwrap_or_default().lines().filter(|l| l.contains(branch)).count()
}

pub fn until(what: &str, mut f: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while !f() {
        assert!(t0.elapsed() < Duration::from_secs(60), "timeout: {what}");
        std::thread::sleep(Duration::from_millis(15));
    }
}
