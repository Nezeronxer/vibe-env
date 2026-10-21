use nightshift_core::agent::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

fn run_in(cwd: &std::path::Path) -> Run {
    Run::new(cwd.to_path_buf(), "p".into(), "s".into(), Role::Worker)
}

#[test]
fn scripted_steps_write_files_and_emit_events() {
    let d = tempfile::tempdir().unwrap();
    let fake = FakeAdapter::new(vec![vec![
        FakeStep::Say("thinking".into()),
        FakeStep::Write("src/new it's.txt".into(), "hello\nit's".into()),
        FakeStep::Done("all done".into()),
    ]]);
    let mut seen = vec![];
    let evs = run_blocking(&fake, &run_in(d.path()), &mut |e| seen.push(e.clone()), &AtomicBool::new(false)).unwrap();
    assert_eq!(evs, seen);
    assert_eq!(std::fs::read_to_string(d.path().join("src/new it's.txt")).unwrap(), "hello\nit's");
    assert_eq!(evs[0], AgentEvent::Text("thinking".into()));
    assert_eq!(evs[1], AgentEvent::Tool { name: "Write".into(), summary: "src/new it's.txt".into() });
    assert_eq!(evs[2], AgentEvent::Done { session: Some("fake-session-0".into()), text: "all done".into(), is_error: false });
    assert_eq!(fake.calls().len(), 1);
}

#[test]
fn each_invocation_takes_the_next_script_and_resume_keeps_session() {
    let d = tempfile::tempdir().unwrap();
    let fake = FakeAdapter::new(vec![vec![FakeStep::Limit], vec![FakeStep::Done("second".into())]]);
    let c = AtomicBool::new(false);
    let first = run_blocking(&fake, &run_in(d.path()), &mut |_| {}, &c).unwrap();
    assert_eq!(first, vec![AgentEvent::Limit { resets_at: None }]);
    let mut r = run_in(d.path());
    r.resume = Some("sess-9".into());
    let second = run_blocking(&fake, &r, &mut |_| {}, &c).unwrap();
    assert_eq!(second, vec![AgentEvent::Done { session: Some("sess-9".into()), text: "second".into(), is_error: false }]);
}

#[test]
fn failing_script_ends_with_an_error_done() {
    let d = tempfile::tempdir().unwrap();
    let fake = FakeAdapter::new(vec![vec![FakeStep::Say("x".into()), FakeStep::Fail]]);
    let evs = run_blocking(&fake, &run_in(d.path()), &mut |_| {}, &AtomicBool::new(false)).unwrap();
    assert!(matches!(evs.last(), Some(AgentEvent::Done { is_error: true, .. })));
}

#[test]
fn cancel_kills_the_run_before_the_script_finishes() {
    let d = tempfile::tempdir().unwrap();
    let fake = FakeAdapter::new(vec![vec![
        FakeStep::Say("started".into()),
        FakeStep::Sleep(20_000),
        FakeStep::Write("late.txt".into(), "x".into()),
        FakeStep::Done("never".into()),
    ]]);
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let c2 = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        c2.store(true, Ordering::SeqCst);
    });
    let t = Instant::now();
    let evs = run_blocking(&fake, &run_in(d.path()), &mut |_| {}, &cancel).unwrap();
    assert!(t.elapsed() < Duration::from_secs(5), "cancel took {:?}", t.elapsed());
    assert_eq!(evs, vec![AgentEvent::Text("started".into())]);
    assert!(!d.path().join("late.txt").exists());
}

struct Sh(String);
impl Adapter for Sh {
    fn id(&self) -> &str { "sh" }
    fn name(&self) -> &str { "sh" }
    fn available(&self) -> Result<(), String> { Ok(()) }
    fn command(&self, run: &Run) -> std::process::Command {
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg(&self.0).current_dir(&run.cwd);
        c
    }
    fn parse(&self, _line: &str) -> Vec<AgentEvent> { vec![] }
}

#[test]
fn cancel_also_kills_grandchildren() {
    let d = tempfile::tempdir().unwrap();
    let a = Sh("sleep 30 & echo $! > grandchild.pid; echo started; wait".into());
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let c2 = cancel.clone();
    let dir = d.path().to_path_buf();
    std::thread::spawn(move || {
        while !dir.join("grandchild.pid").exists() {
            std::thread::sleep(Duration::from_millis(20));
        }
        std::thread::sleep(Duration::from_millis(100));
        c2.store(true, Ordering::SeqCst);
    });
    run_blocking(&a, &run_in(d.path()), &mut |_| {}, &cancel).unwrap();
    let pid = std::fs::read_to_string(d.path().join("grandchild.pid")).unwrap().trim().to_string();
    let alive = || std::process::Command::new("kill").args(["-0", &pid]).output().unwrap().status.success();
    let t = Instant::now();
    while alive() && t.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!alive(), "grandchild {pid} survived cancel");
}

#[test]
fn leftover_background_process_does_not_hold_run_blocking() {
    let d = tempfile::tempdir().unwrap();
    // exits at once, leaving `sleep 30` with the inherited stdout/stderr
    let a = Sh("sleep 30 & echo $! > bg.pid; echo hi".into());
    let t = Instant::now();
    run_blocking(&a, &run_in(d.path()), &mut |_| {}, &AtomicBool::new(false)).unwrap();
    assert!(t.elapsed() < Duration::from_secs(5), "took {:?}", t.elapsed());
    let pid = std::fs::read_to_string(d.path().join("bg.pid")).unwrap().trim().to_string();
    std::thread::sleep(Duration::from_millis(200));
    let alive = std::process::Command::new("kill").args(["-0", &pid]).output().unwrap().status.success();
    assert!(!alive, "background process {pid} survived");
}

// ---- v2 fake: scripts per role and variant, usage / cost / refusals, model in calls() ----

use chrono::Local;
use nightshift_core::model::{LimitWindow, ModelFamily, RoleKind};

fn run_as(cwd: &std::path::Path, role: Role, model: Option<&str>, read_only: bool) -> Run {
    let mut r = Run::new(cwd.to_path_buf(), "p".into(), "s".into(), role);
    r.model = model.map(String::from);
    r.read_only = read_only;
    r
}

fn go(fake: &FakeAdapter, r: &Run) -> Vec<AgentEvent> {
    run_blocking(fake, r, &mut |_| {}, &AtomicBool::new(false)).unwrap()
}

fn say_of(evs: &[AgentEvent]) -> String {
    evs.iter().find_map(|e| if let AgentEvent::Done { text, .. } = e { Some(text.clone()) } else { None }).unwrap()
}

#[test]
fn scripts_are_picked_by_role_and_swarm_variant_not_by_order() {
    let d = tempfile::tempdir().unwrap();
    let fake = FakeAdapter::new(vec![vec![FakeStep::Done("plain".into())]])
        .with_role(RoleKind::Reviewer, vec![vec![FakeStep::Done("review 1".into())], vec![FakeStep::Done("review 2".into())]])
        .with_variant(RoleKind::Worker, "model-a", vec![vec![FakeStep::Done("variant a".into())]])
        .with_variant(RoleKind::Worker, "model-b", vec![vec![FakeStep::Done("variant b".into())]])
        .with_variant(RoleKind::Worker, "slot-c", vec![vec![FakeStep::Done("variant c".into())]]);
    // swarm runs in any order: each variant gets its own script
    let order = ["model-b", "model-a"];
    for m in order {
        let evs = go(&fake, &run_as(d.path(), Role::Worker, Some(m), false));
        assert_eq!(say_of(&evs), format!("variant {}", &m[m.len() - 1..]));
    }
    // a variant can also be told by a folder of the copy
    std::fs::create_dir_all(d.path().join("slot-c")).unwrap();
    let evs = go(&fake, &run_as(&d.path().join("slot-c"), Role::Worker, None, false));
    assert_eq!(say_of(&evs), "variant c");
    // a variant whose queue is used up answers "ok"
    assert_eq!(say_of(&go(&fake, &run_as(d.path(), Role::Worker, Some("model-b"), false))), "ok");
    // roles keep their own queues
    assert_eq!(say_of(&go(&fake, &run_as(d.path(), Role::Reviewer, None, true))), "review 1");
    assert_eq!(say_of(&go(&fake, &run_as(d.path(), Role::Reviewer, None, true))), "review 2");
    // a run of no special role uses the plain list
    assert_eq!(say_of(&go(&fake, &run_as(d.path(), Role::Planner, None, true))), "plain");
}

#[test]
fn calls_show_the_model_and_the_read_only_flag_of_each_run() {
    let d = tempfile::tempdir().unwrap();
    let fake = FakeAdapter::new(vec![]);
    go(&fake, &run_as(d.path(), Role::Worker, Some("opus"), false));
    go(&fake, &run_as(d.path(), Role::Chat, None, true));
    let calls = fake.calls();
    assert_eq!((calls[0].model.as_deref(), calls[0].read_only, calls[0].role), (Some("opus"), false, RoleKind::Worker));
    assert_eq!((calls[1].model.as_deref(), calls[1].read_only, calls[1].role), (None, true, RoleKind::Chat));
}

#[test]
fn usage_cost_refusals_and_sign_in_are_scripted_events() {
    let d = tempfile::tempdir().unwrap();
    let w = LimitWindow {
        name: "five_hour".into(), model: None, used_pct: Some(40.0), remaining: None, resets_at: None,
        exhausted: false, source: "fake".into(), seen_at: Local::now(),
    };
    let fake = FakeAdapter::new(vec![
        vec![FakeStep::Usage(vec![w.clone()]), FakeStep::Cost(0.5), FakeStep::Done("ok".into())],
        vec![FakeStep::RejectModel("no such model".into())],
        vec![FakeStep::Auth("not logged in".into())],
    ]);
    let r = run_as(d.path(), Role::Worker, None, false);
    let evs = go(&fake, &r);
    assert_eq!(evs[0], AgentEvent::Usage(vec![w]));
    assert_eq!(evs[1], AgentEvent::Cost(0.5));
    assert_eq!(go(&fake, &r), vec![AgentEvent::ModelRejected("no such model".into())]);
    assert_eq!(go(&fake, &r), vec![AgentEvent::AuthRequired("not logged in".into())]);
}

#[test]
fn fake_models_default_to_an_error_like_envs_without_a_list_command() {
    let fam = ModelFamily { id: "x".into(), label: "X".into(), vendor: "v".into(), blurb: None, versions: vec![], default: false, exact: Default::default() };
    assert!(FakeAdapter::new(vec![]).models().is_err());
    assert_eq!(FakeAdapter::new(vec![]).with_models(vec![fam.clone()]).models().unwrap(), vec![fam]);
    assert!(!FakeAdapter::new(vec![]).read_only_native());
    assert!(FakeAdapter::new(vec![]).with_read_only_native(true).read_only_native());
}
