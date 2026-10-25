//! The council (Совет, G11.3): proposals, one round of votes counted by the app, one executor, the
//! council on the review; a tie goes to the Reviewer.
mod common;
use common::*;
use nightshift_core::agent::FakeAdapter;
use nightshift_core::engine::NewTask;
use nightshift_core::model::*;
use nightshift_core::notify::RecordingNotifier;
use std::sync::Arc;

fn propose(text: &str) -> Vec<nightshift_core::agent::FakeStep> {
    vec![done(&serde_json::json!({ "approach": text }).to_string())]
}
fn vote(for_: &str) -> Vec<nightshift_core::agent::FakeStep> {
    vec![done(&serde_json::json!({"vote": for_, "why": format!("{for_} is simpler")}).to_string())]
}
fn verdict(ok: bool) -> Vec<nightshift_core::agent::FakeStep> {
    vec![done(&serde_json::json!({"verdict": if ok { "approve" } else { "changes" }, "issues": if ok { vec![] } else { vec!["rename it"] }}).to_string())]
}

fn run(e: &nightshift_core::engine::Engine, p: &Project, n: u8) -> String {
    e.submit(NewTask { project_id: p.id.clone(), prompt: "Add the feature file".into(), swarm: Some(SwarmSpec { mode: SwarmMode::Council, n }), ..NewTask::default() }).unwrap()
}

fn env_for(n: usize) -> impl FnOnce(&mut Settings) {
    move |s: &mut Settings| {
        env_models(s, "fa", &["m1", "m2", "m3"][..n], "m1", &[], None);
        s.max_parallel = 3;
    }
}

#[test]
fn the_council_proposes_votes_once_the_winner_is_built_and_the_council_approves_by_majority() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let w = Arc::new(
        FakeAdapter::new(vec![])
            .with_id("fa")
            // each member: its proposal, its vote, its verdict on the work
            .with_variant(RoleKind::Council, "m1", vec![propose("Approach of A"), vote("B"), verdict(true)])
            .with_variant(RoleKind::Council, "m2", vec![propose("Approach of B"), vote("B"), verdict(true)])
            .with_variant(RoleKind::Council, "m3", vec![propose("Approach of C"), vote("A"), verdict(false)])
            .with_role(RoleKind::Worker, vec![vec![write("feature.txt", "built\n"), done("Built it")]]),
    );
    let e = env.engine(env.store(&p, env_for(3)), vec![w.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = run(&e, &p, 3);
    wait_status(&e, &id, Status::Done);

    // 3 proposals + 3 votes + 3 verdicts; one writer; no separate Reviewer (the majority was clear)
    assert_eq!(calls_of(&w, RoleKind::Council).len(), 9);
    assert!(calls_of(&w, RoleKind::Council).iter().all(|r| r.read_only), "the council only reads");
    assert_eq!(calls_of(&w, RoleKind::Worker).len(), 1);
    assert!(calls_of(&w, RoleKind::Reviewer).is_empty());
    // the writer was told what the council chose: B won with 2 votes of 3 (A -> B, B -> B, C -> A)
    let writer = &calls_of(&w, RoleKind::Worker)[0].prompt;
    assert!(writer.contains("Approach of B") && !writer.contains("Approach of C"), "{writer}");
    // everybody saw everybody's proposals when voting
    let votes: Vec<_> = calls_of(&w, RoleKind::Council).into_iter().filter(|r| r.prompt.contains("PROPOSALS")).collect();
    assert_eq!(votes.len(), 3);
    assert!(votes.iter().all(|r| r.prompt.contains("Approach of A") && r.prompt.contains("Approach of C")));
    // what the «Обсуждение» reads from the task
    let c = e.swarm_view(&id).unwrap().council.unwrap();
    assert_eq!(c.proposals.iter().map(|x| (x.id.as_str(), x.text.as_str())).collect::<Vec<_>>(), [("A", "Approach of A"), ("B", "Approach of B"), ("C", "Approach of C")]);
    assert_eq!(c.votes.iter().map(|v| (v.voter.as_str(), v.for_.as_str())).collect::<Vec<_>>(), [("A", "B"), ("B", "B"), ("C", "A")]);
    assert_eq!((c.winner.as_deref(), c.decided_by.as_deref()), (Some("B"), Some("votes")));
    assert_eq!(c.review_votes.iter().map(|v| v.for_.as_str()).collect::<Vec<_>>(), ["approve", "approve", "changes"]);
    assert_eq!(env.main_file("feature.txt"), "built");
}

#[test]
fn a_tie_of_the_proposals_and_a_tie_on_the_review_are_decided_by_the_reviewer() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let w = Arc::new(
        FakeAdapter::new(vec![])
            .with_id("fa")
            .with_variant(RoleKind::Council, "m1", vec![propose("Approach of A"), vote("A"), verdict(true)])
            .with_variant(RoleKind::Council, "m2", vec![propose("Approach of B"), vote("B"), verdict(false)])
            .with_role(RoleKind::Worker, vec![vec![write("feature.txt", "built\n"), done("Built it")]])
            // the first call is the tie of the proposals (B), the second is the review the council could not settle
            .with_role(RoleKind::Reviewer, vec![vec![done(r#"{"choice":"B"}"#)], approve()]),
    );
    let e = env.engine(env.store(&p, env_for(2)), vec![w.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = run(&e, &p, 2);
    wait_status(&e, &id, Status::Done);

    let c = e.swarm_view(&id).unwrap().council.unwrap();
    assert_eq!((c.winner.as_deref(), c.decided_by.as_deref()), (Some("B"), Some("reviewer")));
    assert!(calls_of(&w, RoleKind::Worker)[0].prompt.contains("Approach of B"));
    assert_eq!(calls_of(&w, RoleKind::Reviewer).len(), 2);
    assert_eq!(c.review_votes.iter().map(|v| v.for_.as_str()).collect::<Vec<_>>(), ["approve", "changes"]);
    assert_eq!(env.main_file("feature.txt"), "built");
}

#[test]
fn a_member_whose_throwaway_copy_fails_does_not_read_in_the_real_folder() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let w = Arc::new(
        FakeAdapter::new(vec![])
            .with_id("fa")
            .with_variant(RoleKind::Council, "m1", vec![propose("A"), vote("A"), verdict(true)])
            .with_variant(RoleKind::Council, "m2", vec![propose("B"), vote("A"), verdict(true)])
            .with_role(RoleKind::Worker, vec![vec![write("feature.txt", "built\n"), done("Built it")]]),
    );
    let e = env.engine(env.store(&p, env_for(2)), vec![w.clone()], Arc::new(RecordingNotifier::default()), None);
    // the folder of the throwaway copies cannot be made (a file stands in its place); the task's own copy is elsewhere
    let wts = env.d.path().join("wts");
    std::fs::create_dir_all(&wts).unwrap();
    std::fs::write(wts.join(".ro"), "in the way").unwrap();
    let id = run(&e, &p, 2);
    wait(&e, &id, "the task ends", |t| !matches!(t.status, Status::Queued | Status::Preparing | Status::Working | Status::Checking | Status::Reviewing | Status::Merging));
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(w.calls().iter().all(|r| r.cwd != env.repo), "no run in the person's folder: {:?}", w.calls().iter().map(|r| (&r.role, &r.cwd)).collect::<Vec<_>>());
    assert!(has_log(&e, &id, "copy_failed") || task(&e, &id).outcome.error.as_deref().is_some_and(|x| x.contains("copy_failed")), "the reason is told: {:?}", task(&e, &id).outcome.error);
}

#[test]
fn a_throwaway_folder_that_cannot_be_removed_at_start_is_told_and_does_not_hang_the_engine() {
    let env = Env::new();
    let p = env.project(|_| {});
    std::fs::create_dir_all(env.d.path().join("wts")).unwrap();
    std::fs::write(env.d.path().join("wts").join(".ro"), "in the way").unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let store = env.store(&p, |_| {});
    let wts = env.d.path().join("wts");
    std::thread::spawn(move || {
        let e = nightshift_core::engine::Engine::new(store, vec![], Arc::new(RecordingNotifier::default()), wts);
        e.start();
        let _ = tx.send(e);
    });
    // the sweep logged the failure for each project while it held the lock of the store that the log wants itself
    let e = rx.recv_timeout(std::time::Duration::from_secs(20)).expect("the engine did not start");
    assert!(has_log(&e, &p.id, "copy_not_removed"));
}
