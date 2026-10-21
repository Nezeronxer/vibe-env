//! The Watcher (G08.2), "Отложить" (G08.3), the merge queue (G08.4), freeze for the new runs (G08.7).
mod common;
use common::*;
use nightshift_core::agent::FakeAdapter;
use nightshift_core::model::*;
use nightshift_core::notify::RecordingNotifier;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn until(what: &str, f: impl Fn() -> bool) {
    let t0 = Instant::now();
    while !f() {
        assert!(t0.elapsed() < Duration::from_secs(60), "timeout: {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

const CHANGES: &str = r#"{"verdict":"changes","issues":["fix the file"]}"#;

/// Run 0 of the executor is done, the review wants changes, run 1 (the same session) goes silent.
fn silent_worker(extra: Vec<Vec<FakeStep>>) -> FakeAdapter {
    let mut w = vec![vec![write("a1.txt", "1"), done("v1")], vec![FakeStep::Sleep(60_000)]];
    w.extend(extra);
    FakeAdapter::new(vec![])
        .with_id("fake")
        .with_role(RoleKind::Worker, w)
        .with_role(RoleKind::Reviewer, vec![vec![done(CHANGES)], approve(), approve()])
        .with_role(RoleKind::Watcher, vec![vec![done("It waits for an answer that never comes.")], vec![done("Still stuck.")]])
}
use nightshift_core::agent::FakeStep;

#[test]
fn silence_of_15_minutes_stops_the_run_and_goes_on_in_the_same_session_with_a_diagnosis() {
    let env = Env::new();
    let p = env.project(|_| {});
    let (clock, now) = manual_clock();
    let w = Arc::new(silent_worker(vec![vec![write("a2.txt", "2"), done("v2")]]));
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone()], Arc::new(RecordingNotifier::default()), Some(clock));
    let t0 = *now.lock().unwrap();
    let id = submit(&e, &p, "Make the files");

    until("run 1 started", || calls_of(&w, RoleKind::Worker).len() == 2);
    // not silent yet: 14 minutes change nothing (what the clock says the Watcher saw is checked below)
    advance(&now, 14 * 60);
    advance(&now, 61);
    wait_status(&e, &id, Status::Done);
    // the silence was judged by the clock of the engine: the nudge is dated after 15 whole minutes
    let nudge = e.store().lock().unwrap().read_log(&id, 5000).into_iter().find(|l| l.text.starts_with("watcher: silent for 15 min")).unwrap();
    assert!(nudge.at - t0 >= chrono::Duration::minutes(15), "{:?}", nudge.at - t0);

    let workers = calls_of(&w, RoleKind::Worker);
    assert_eq!(workers.len(), 3);
    assert_eq!(workers[1].resume.as_deref(), Some("fake-session-0"));
    assert_eq!(workers[2].resume.as_deref(), Some("fake-session-0"), "the nudge goes to the same session");
    assert!(workers[2].prompt.contains("what is blocking you"));
    assert_eq!(calls_of(&w, RoleKind::Watcher).len(), 1, "one diagnosis, by a run of its own");
    assert!(has_log(&e, &id, "watcher: It waits for an answer"));
    let ev = e.store().lock().unwrap().read_log(&id, 5000).into_iter().find(|l| l.text.starts_with("watcher: It waits")).unwrap();
    assert_eq!(ev.role, Some(RoleKind::Watcher));
}

#[test]
fn five_more_minutes_restart_the_role_in_a_new_session_and_count_an_attempt_until_none_is_left() {
    let env = Env::new();
    let p = env.project(|_| {});
    let (clock, now) = manual_clock();
    // run 1 and the nudged run 2 stay silent, run 3 (a new session) works
    let w = Arc::new(silent_worker(vec![vec![FakeStep::Sleep(60_000)], vec![write("a3.txt", "3"), done("v3")]]));
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone()], Arc::new(RecordingNotifier::default()), Some(clock));
    let id = submit(&e, &p, "Make the files");
    until("run 1", || calls_of(&w, RoleKind::Worker).len() == 2);
    advance(&now, 15 * 60 + 1);
    until("the nudged run 2", || calls_of(&w, RoleKind::Worker).len() == 3);
    advance(&now, 5 * 60 + 1);
    let t = wait_status(&e, &id, Status::Done);
    let workers = calls_of(&w, RoleKind::Worker);
    assert_eq!(workers.len(), 4);
    assert_eq!(workers[3].resume, None, "a new session");
    assert_eq!(t.attempts, 1, "the restart is an attempt");
    assert_eq!(calls_of(&w, RoleKind::Watcher).len(), 1, "the diagnosis is made once, at the first silence");

    // attempts used up: "Не вышло"
    let env = Env::new();
    let p = env.project(|_| {});
    let (clock, now) = manual_clock();
    let w = Arc::new(silent_worker(vec![vec![FakeStep::Sleep(60_000)]]));
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store(&p, |s| s.attempts = 1), vec![w.clone()], n.clone(), Some(clock));
    let id = submit(&e, &p, "Make the files");
    until("run 1", || calls_of(&w, RoleKind::Worker).len() == 2);
    advance(&now, 15 * 60 + 1);
    until("the nudged run", || calls_of(&w, RoleKind::Worker).len() == 3);
    advance(&now, 5 * 60 + 1);
    let t = wait_status(&e, &id, Status::Failed);
    assert!(t.outcome.error.unwrap().starts_with("stalled"));
}

#[test]
fn nothing_runs_for_the_watcher_when_nobody_is_silent() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = Arc::new(FakeAdapter::new(vec![]).with_id("fake").with_role(RoleKind::Reviewer, vec![approve()]));
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Easy one");
    wait_status(&e, &id, Status::Done);
    assert!(calls_of(&w, RoleKind::Watcher).is_empty());
    assert_eq!(w.calls().len(), 2, "the executor and the reviewer, nothing else");
}

#[test]
fn two_tasks_merge_one_at_a_time_and_their_steps_tell_who_is_first_and_who_waits() {
    let env = Env::new();
    // the checks of the merge take a while, so the second task has to wait in the queue
    let p = env.project(|p| p.check_cmd = Some("sleep 1".into()));
    let w = Arc::new(
        FakeAdapter::new(vec![])
            .with_id("fake")
            .with_role(RoleKind::Worker, vec![vec![write("one.txt", "1"), done("1")], vec![write("two.txt", "2"), done("2")]])
            .with_role(RoleKind::Reviewer, vec![approve(), approve()]),
    );
    let e = env.engine(env.store(&p, |s| s.max_parallel = 3), vec![w], Arc::new(RecordingNotifier::default()), None);
    let a = submit(&e, &p, "Add one");
    let b = submit(&e, &p, "Add two");

    let merge_of = |t: &Task| t.steps.iter().find(|(s, _)| *s == Step::Merge).map(|(_, st)| st.clone());
    let (mut both_running, mut saw_pair) = (false, false);
    let t0 = Instant::now();
    loop {
        let (ta, tb) = (task(&e, &a), task(&e, &b));
        let (ma, mb) = (merge_of(&ta), merge_of(&tb));
        both_running |= ma == Some(StepState::Running) && mb == Some(StepState::Running);
        // the first in the pipe runs, the other waits since a moment that is not after the first's
        saw_pair |= matches!((&ma, &mb), (Some(StepState::Running), Some(StepState::Queued(_))) | (Some(StepState::Queued(_)), Some(StepState::Running)));
        if ta.status == Status::Done && tb.status == Status::Done {
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(60), "timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!both_running, "two merges at the same time");
    assert!(saw_pair, "one never saw a first and a waiting one in the steps");
    assert_eq!(env.main_file("one.txt") + &env.main_file("two.txt"), "12");
}

#[test]
fn nudge_by_hand_stops_the_run_and_goes_on_in_the_same_session() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = Arc::new(silent_worker(vec![vec![write("a2.txt", "2"), done("v2")]]));
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Make the files");
    until("run 1 started", || calls_of(&w, RoleKind::Worker).len() == 2);
    e.nudge(&id).unwrap();
    wait_status(&e, &id, Status::Done);
    let workers = calls_of(&w, RoleKind::Worker);
    assert_eq!(workers.len(), 3);
    assert_eq!(workers[2].resume.as_deref(), Some("fake-session-0"));
    assert!(workers[2].prompt.contains("what is blocking you"));
    assert!(calls_of(&w, RoleKind::Watcher).is_empty(), "nobody diagnosed anything: the person pushed");
    assert!(has_log(&e, &id, "nudge"));
    assert_eq!(e.nudge(&id).unwrap_err(), "not_nudgeable", "a finished task cannot be nudged");
}

#[test]
fn with_quality_off_the_executor_gets_the_words_as_they_are_and_the_feed_says_so_once() {
    let env = Env::new();
    let p = env.project(|_| {});
    // two worker runs (a review round), so "once" is more than a coincidence
    let w = Arc::new(
        FakeAdapter::new(vec![])
            .with_id("fake")
            .with_role(RoleKind::Worker, vec![vec![write("a1.txt", "1"), done("v1")], vec![write("a2.txt", "2"), done("v2")]])
            .with_role(RoleKind::Reviewer, vec![vec![done(CHANGES)], approve()]),
    );
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Make the files exactly like this");
    wait_status(&e, &id, Status::Done);
    assert_eq!(calls_of(&w, RoleKind::Worker)[0].prompt, "Make the files exactly like this");
    let lines = log_texts(&e, &id).into_iter().filter(|l| l == "prompt_not_rewritten: quality_off").count();
    assert_eq!(lines, 1);
    let ev = e.store().lock().unwrap().read_log(&id, 5000).into_iter().find(|l| l.text == "prompt_not_rewritten: quality_off").unwrap();
    assert!(matches!(ev.kind, LogKind::Stage));
}
