//! A project whose folder is gone holds its tasks (R11.10): they wait with a key on the feed, nothing runs,
//! and they go on when the folder is pointed out again.
mod common;
use common::*;
use nightshift_core::model::*;
use nightshift_core::notify::RecordingNotifier;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn tasks_of_a_project_with_a_missing_folder_wait_and_go_on_after_the_new_place_is_given() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = fake(vec![vec![write("f.txt", "f"), done("f")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let moved = env.d.path().join("moved");
    std::fs::rename(&env.repo, &moved).unwrap();

    let id = submit(&e, &p, "Add f");
    let t = wait(&e, &id, "the wait", |t| t.outcome.error.as_deref().is_some_and(|x| x.starts_with("waiting_folder")));
    assert_eq!(t.status, Status::Queued);
    assert!(t.outcome.error.unwrap().contains(&env.repo.display().to_string()));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(task(&e, &id).status, Status::Queued, "it does not run and does not fail");
    assert!(w.calls().is_empty());
    let log: Vec<String> = e.store().lock().unwrap().read_log(&id, 50).into_iter().map(|l| l.text).collect();
    assert_eq!(log.iter().filter(|l| l.starts_with("waiting_folder")).count(), 1, "said once: {log:?}");

    // "Указать заново": the same repository at its new place
    let mut np = p.clone();
    np.path = moved.clone();
    e.store().lock().unwrap().upsert_project(np).unwrap();
    let t = wait_status(&e, &id, Status::Done);
    assert!(t.outcome.error.is_none());
    assert_eq!(w.calls().len(), 1);
}
