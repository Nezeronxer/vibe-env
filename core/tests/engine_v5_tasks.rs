//! F13: snapshots before each run + rollback, the "До мержа" list that stops the merge, the folder handoff.
mod common;
use common::*;
use nightshift_core::agent::FakeAdapter;
use nightshift_core::model::*;
use nightshift_core::notify::RecordingNotifier;
use std::fs;
use std::sync::Arc;

const CHANGES: &str = r#"{"verdict":"changes","issues":["handle empty input"]}"#;

/// A task that ends Failed with its copy kept: the check never goes green.
fn failed_task(env: &Env) -> (Arc<nightshift_core::engine::Engine>, String) {
    let p = env.project(|p| p.check_cmd = Some("false".into()));
    let w = fake(vec![vec![write("x.txt", "1"), done("v1")], vec![write("x.txt", "2"), done("v2")], vec![write("x.txt", "3"), done("v3")]]);
    let e = env.engine(env.store(&p, |s| s.attempts = 2), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Make x");
    wait_status(&e, &id, Status::Failed);
    (e, id)
}

#[test]
fn a_snapshot_is_taken_before_each_run_and_the_rollback_returns_the_code_and_can_be_undone() {
    let env = Env::new();
    let (e, id) = failed_task(&env);
    let snaps = e.snapshots(&id).unwrap();
    assert!(snaps.len() >= 2, "one per worker run, got {}", snaps.len());
    assert!(snaps.iter().all(|s| s.label == "worker"));
    let wt = task(&e, &id).worktree.unwrap();
    assert_eq!(fs::read_to_string(wt.join("x.txt")).unwrap(), "2");
    // snapshot 1 was taken before the first run: no x.txt yet
    let saved = e.restore_snapshot(&id, 1).unwrap();
    assert!(!wt.join("x.txt").exists());
    // ... and the state before the rollback can be had again
    e.restore_snapshot(&id, saved).unwrap();
    assert_eq!(fs::read_to_string(wt.join("x.txt")).unwrap(), "2");
    // the refs are no branches
    assert!(!g(&env.repo, &["branch", "--all"]).contains("nightshift"));
    // removing the task removes its snapshots
    e.remove(&id).unwrap();
    assert!(g(&env.repo, &["for-each-ref", "refs/nightshift/"]).is_empty());
}

#[test]
fn the_merge_waits_for_an_open_item_and_goes_on_when_it_is_ticked() {
    let env = Env::new();
    let p = env.project(|_| {});
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    let w = Gated::new("fake", vec![vec![write("f.txt", "ok"), done("done")]], 0, &gd);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let before = env.main_count();
    let id = submit(&e, &p, "Add f");
    w.wait_started();
    e.todo_add(&id, "update the changelog", "human").unwrap();
    w.open();
    let t = wait_status(&e, &id, Status::NeedsHuman);
    assert_eq!(t.question.as_ref().unwrap().text, "todos_open: 1");
    assert_eq!(env.main_count(), before, "nothing merged");
    // pressing "Continue" with the item open does not pass
    assert!(e.answer(&id, "Continue").is_err());
    assert_eq!(task(&e, &id).status, Status::NeedsHuman);
    let item = task(&e, &id).todos[0].id.clone();
    e.todo_set(&id, &item, true).unwrap();
    wait_status(&e, &id, Status::Done);
    assert_eq!(env.main_count(), before + 1);
}

#[test]
fn a_finding_of_the_reviewer_becomes_an_item_and_the_approval_closes_it() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = fake(vec![vec![write("f.txt", "1"), done("v1")], vec![write("f.txt", "2"), done("v2")]]);
    let rev = Arc::new(FakeAdapter::new(vec![vec![done(CHANGES)], approve()]).with_id("rev"));
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), rev], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add f");
    wait_status(&e, &id, Status::Done);
    let t = task(&e, &id);
    assert_eq!(t.todos.len(), 1);
    assert_eq!((t.todos[0].text.as_str(), t.todos[0].by.as_str(), t.todos[0].done), ("handle empty input", "reviewer", true));
    // the second run of the executor was told about it
    let prompts: Vec<String> = w.calls().iter().map(|c| c.prompt.clone()).collect();
    assert!(prompts[1].contains("handle empty input"), "{:?}", prompts[1]);
}

#[test]
fn take_refuses_a_dirty_folder_switches_a_clean_one_and_give_returns_the_branch() {
    let env = Env::new();
    let (e, id) = failed_task(&env);
    let t = task(&e, &id);
    fs::write(env.repo.join("a.txt"), "mine\n").unwrap();
    let err = e.take_to_main(&id).unwrap_err();
    assert!(err.contains("main_dirty"), "{err}");
    assert_eq!(g(&env.repo, &["branch", "--show-current"]), "main");
    assert_eq!(fs::read_to_string(env.repo.join("a.txt")).unwrap(), "mine\n");
    g(&env.repo, &["checkout", "-q", "--", "a.txt"]);

    e.take_to_main(&id).unwrap();
    assert_eq!(g(&env.repo, &["branch", "--show-current"]), t.branch);
    assert_eq!(task(&e, &id).taken_from.as_deref(), Some("main"));
    // nothing starts work in the detached copy meanwhile
    assert_eq!(e.retry(&id).unwrap_err(), "taken_to_main");
    assert_eq!(e.remove(&id).unwrap_err(), "taken_to_main");
    assert!(e.restore_snapshot(&id, 1).is_err());

    e.give_to_agent(&id).unwrap();
    assert_eq!(g(&env.repo, &["branch", "--show-current"]), "main");
    assert_eq!(g(&t.worktree.clone().unwrap(), &["branch", "--show-current"]), t.branch);
    assert_eq!(task(&e, &id).taken_from, None);
    e.retry(&id).unwrap(); // works again
}
