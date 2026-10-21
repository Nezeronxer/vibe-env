//! The crew (Бригада, G11.2): the Coordinator splits, the app checks the split, the parts land in the
//! task's branch one by one, then the whole task goes through gates, review and merge.
mod common;
use common::*;
use nightshift_core::agent::{FakeAdapter, FakeStep};
use nightshift_core::engine::NewTask;
use nightshift_core::model::*;
use nightshift_core::notify::RecordingNotifier;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn until(what: &str, mut f: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while !f() {
        assert!(t0.elapsed() < Duration::from_secs(60), "timeout: {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn split(parts: &[(&str, &[&str])]) -> String {
    let ps: Vec<_> = parts.iter().map(|(id, files)| serde_json::json!({"id": id, "title": format!("part {id}"), "files": files, "brief": format!("do {id}")})).collect();
    serde_json::json!({ "parts": ps }).to_string()
}

fn crew(n: u8) -> Option<SwarmSpec> {
    Some(SwarmSpec { mode: SwarmMode::Crew, n })
}

fn run(e: &nightshift_core::engine::Engine, p: &Project, n: u8) -> String {
    e.submit(NewTask { project_id: p.id.clone(), prompt: "Build the three things".into(), swarm: crew(n), ..NewTask::default() }).unwrap()
}

#[test]
fn parts_without_overlap_land_in_the_task_branch_one_by_one_then_the_whole_task_goes_to_main() {
    let env = Env::new();
    let counter = env.d.path().join("gate-runs");
    // every run of the gates leaves a line: the parts, the whole task, the merge
    let p = env.project(|p| {
        p.agent = "fa".into();
        p.check_cmd = Some(format!("echo run >> '{}'", counter.display()));
    });
    let w = Arc::new(
        FakeAdapter::new(vec![])
            .with_id("fa")
            .with_role(RoleKind::Coordinator, vec![vec![done(&split(&[("p1", &["one.txt"]), ("p2", &["two.txt"]), ("p3", &["docs/"])]))]])
            .with_role(RoleKind::Reviewer, vec![approve()])
            .with_variant(RoleKind::Worker, "A", vec![vec![write("one.txt", "1\n"), done("did one")]])
            // B also writes a file that is not its: the app takes it back
            .with_variant(RoleKind::Worker, "B", vec![vec![write("two.txt", "2\n"), write("one.txt", "stolen\n"), write("stray.txt", "x\n"), done("did two")]])
            .with_variant(RoleKind::Worker, "C", vec![vec![write("docs/x.md", "3\n"), done("did three")]]),
    );
    let e = env.engine(env.store(&p, |s| s.max_parallel = 3), vec![w.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = run(&e, &p, 3);
    wait_status(&e, &id, Status::Done);

    assert_eq!(calls_of(&w, RoleKind::Coordinator).len(), 1);
    assert_eq!(calls_of(&w, RoleKind::Worker).len(), 3);
    // one commit on main, with every part, and only what the parts own
    assert_eq!(env.main_count(), 2);
    assert_eq!((env.main_file("one.txt").as_str(), env.main_file("two.txt").as_str(), env.main_file("docs/x.md").as_str()), ("1", "2", "3"));
    assert!(g(&env.repo, &["ls-tree", "-r", "--name-only", "main"]).lines().all(|f| f != "stray.txt"), "a file outside the part was undone");
    assert!(has_log(&e, &id, "part_outside_files: one.txt, stray.txt"), "{:?}", log_texts(&e, &id));
    for l in ["A", "B", "C"] {
        assert!(has_log(&e, &id, &format!("part_landed: {l}")));
    }
    // gates: 3 parts + the whole task + the merge
    assert_eq!(std::fs::read_to_string(&counter).unwrap().lines().count(), 5);
    // the parts as the window reads them
    let v = e.swarm_view(&id).unwrap();
    assert_eq!(v.parts.iter().map(|x| (x.id.as_str(), x.state.as_str())).collect::<Vec<_>>(), [("A", "done"), ("B", "done"), ("C", "done")]);
    assert_eq!(v.parts[0].files, ["one.txt"]);
    let _ = FakeStep::Fail;
}

fn crew_fake(splits: Vec<String>, sleep_ms: u64) -> Arc<FakeAdapter> {
    let part = |f: &str| vec![vec![FakeStep::Sleep(sleep_ms), write(f, "x\n"), done("did it")]];
    Arc::new(
        FakeAdapter::new(vec![])
            .with_id("fa")
            .with_role(RoleKind::Coordinator, splits.iter().map(|s| vec![done(s)]).collect())
            .with_role(RoleKind::Reviewer, vec![approve()])
            .with_variant(RoleKind::Worker, "A", part("one.txt"))
            .with_variant(RoleKind::Worker, "B", part("two.txt")),
    )
}

#[test]
fn a_split_that_overlaps_is_made_again_once_and_a_good_second_split_runs_side_by_side() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let bad = split(&[("p1", &["one.txt", "src"]), ("p2", &["two.txt", "src/a.rs"])]);
    let good = split(&[("p1", &["one.txt"]), ("p2", &["two.txt"])]);
    let w = crew_fake(vec![bad, good], 500);
    let e = env.engine(env.store(&p, |s| s.max_parallel = 3), vec![w.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = run(&e, &p, 2);
    until("both parts run at once", || {
        let t = task(&e, &id);
        t.runs.iter().filter(|r| r.role == RoleKind::Worker && r.state == StepState::Running).count() == 2
    });
    wait_status(&e, &id, Status::Done);

    let coord = calls_of(&w, RoleKind::Coordinator);
    assert_eq!(coord.len(), 2, "the split was asked for once more");
    assert!(coord[1].prompt.contains("both touch"), "and the Coordinator was told where it overlapped");
    assert!(has_log(&e, &id, "split_overlap: p1 and p2"));
    assert!(!has_log(&e, &id, "split_sequential"));
    assert_eq!((env.main_file("one.txt").as_str(), env.main_file("two.txt").as_str()), ("x", "x"));
}

#[test]
fn a_split_that_overlaps_twice_runs_its_parts_one_after_the_other() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let bad = split(&[("p1", &["one.txt", "src"]), ("p2", &["two.txt", "src/a.rs"])]);
    let w = crew_fake(vec![bad.clone(), bad], 300);
    let e = env.engine(env.store(&p, |s| s.max_parallel = 3), vec![w.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = run(&e, &p, 2);
    let t = wait_status(&e, &id, Status::Done);

    assert_eq!(calls_of(&w, RoleKind::Coordinator).len(), 2, "once more, not forever");
    assert!(has_log(&e, &id, "split_sequential"));
    let (a, b) = {
        let by = |l: &str| t.runs.iter().find(|r| r.role == RoleKind::Worker && r.variant.as_deref() == Some(l)).unwrap().clone();
        (by("A"), by("B"))
    };
    assert!(a.finished_at.unwrap() <= b.started_at.unwrap(), "B started after A was done: {a:?} {b:?}");
    assert_eq!((env.main_file("one.txt").as_str(), env.main_file("two.txt").as_str()), ("x", "x"));
}
