//! Engine v2: freeze, the terminal hand-off, notes of the person, the answer to "push / PR?",
//! lifting a failed task, switching a model, put-offs, a swapped adapter, the digest, restarts.
mod common;
use common::*;
use nightshift_core::agent::{Adapter, FakeAdapter, FakeStep};
use nightshift_core::engine::{Engine, EngineEvent, NewTask, NoteResult};
use nightshift_core::model::*;
use nightshift_core::notify::{NoticeKind, RecordingNotifier};
use nightshift_core::store::Store;
use std::fs;
use std::sync::Arc;
use std::time::Duration;

/// A project whose first check run is red: the executor has to come back once.
fn red_once(p: &mut Project) {
    p.check_cmd = Some("test -f fixed.txt".into());
}

fn restart(env: &Env, adapters: Vec<Arc<dyn Adapter>>) -> Arc<Engine> {
    env.engine(Store::open(&env.d.path().join("data")).unwrap(), adapters, Arc::new(RecordingNotifier::default()), None)
}

#[test]
fn freeze_stops_the_runs_at_once_keeps_status_and_session_and_unfreeze_goes_on_from_the_same_place() {
    let env = Env::new();
    let p = env.project(red_once);
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    // run 0 leaves the check red, run 1 would fix it but is held at the gate
    let w = Gated::new("fake", vec![vec![done("v1")], vec![write("fixed.txt", "ok"), done("v2")], vec![write("fixed.txt", "ok"), done("v2")]], 1, &gd);
    let e = env.engine(env.store(&p, |s| s.max_parallel = 3), vec![w.clone(), reviewer(3)], Arc::new(RecordingNotifier::default()), None);
    let events = e.subscribe();

    let id = submit(&e, &p, "Fix it");
    let pid = w.wait_started();
    assert!(alive(pid));
    e.freeze(true);
    wait_until("the agent process is gone", || !alive(pid));
    let t = task(&e, &id);
    assert_eq!(t.status, Status::Working, "the status stays");
    assert_eq!(t.session.as_deref(), Some("fake-session-0"), "so does the session");
    assert_eq!(t.attempts, 1);
    assert!(e.is_frozen());

    // nothing new starts while frozen
    let second = submit(&e, &p, "Another");
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(task(&e, &second).status, Status::Queued);
    assert_eq!(w.calls(), 2, "run 1 was started and killed, no more");

    e.freeze(false);
    wait_status(&e, &id, Status::Done);
    let calls = w.inner.calls();
    assert_eq!(calls[2].resume.as_deref(), Some("fake-session-0"), "the same session, from the same place");
    let frozen: Vec<bool> = events.try_iter().filter_map(|ev| if let EngineEvent::Frozen(b) = ev { Some(b) } else { None }).collect();
    assert_eq!(frozen, vec![true, false]);
}

pub fn wait_until(what: &str, f: impl Fn() -> bool) {
    let start = std::time::Instant::now();
    while !f() {
        assert!(start.elapsed() < Duration::from_secs(20), "timeout waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_merge_that_began_before_the_freeze_is_carried_to_its_end() {
    let env = Env::new();
    let (counter, started, go) = (env.d.path().join("count"), env.d.path().join("started"), env.d.path().join("go"));
    // the first run of the check is the gates, the second is the merge's: it holds until the test lets it go
    let p = env.project(|p| {
        p.check_cmd = Some(format!(
            "if [ -f '{c}' ]; then touch '{s}'; while [ ! -f '{g}' ]; do sleep 0.05; done; else touch '{c}'; fi",
            c = counter.display(),
            s = started.display(),
            g = go.display()
        ))
    });
    let w = fake(vec![vec![write("f.txt", "f"), done("f")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let before = env.main_count();

    let id = submit(&e, &p, "Add f");
    wait_until("the merge to be checking", || started.exists());
    assert_eq!(task(&e, &id).status, Status::Merging);
    e.freeze(true);
    fs::write(&go, "").unwrap();
    let t = wait_status(&e, &id, Status::Done);
    assert_eq!(env.main_count(), before + 1, "the squash landed although the engine is frozen");
    assert!(t.outcome.commit.is_some());
    assert!(e.is_frozen());
}

#[test]
fn a_task_handed_to_the_terminal_stays_untouched_also_after_a_restart_until_it_is_taken_back_through_gates_and_review() {
    let env = Env::new();
    let p = env.project(red_once);
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    let w = Gated::new("fake", vec![vec![done("v1")], vec![done("never")]], 1, &gd);
    let e = env.engine(env.store(&p, |_| {}), vec![Arc::new(Knobs::new(w.clone()).resumable()), reviewer(2)], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Fix it");
    w.wait_started();
    let h = e.hand_off(&id).unwrap();
    assert!(h.resumed && h.command.is_some());
    let cmd = h.command.unwrap();
    assert!(cmd.get_args().any(|a| a == "fake-session-0"), "{cmd:?}");
    assert_eq!(h.worktree, env.d.path().join("wts").join(&id));
    assert_eq!(task(&e, &id).status, Status::HandedOff);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(w.calls(), 2, "nobody runs the session now");

    // the app is closed and opened again: still the person's
    e.shutdown();
    let w2 = fake(vec![vec![write("fixed.txt", "agent"), done("agent run")]]);
    let e2 = restart(&env, vec![w2.clone(), reviewer(2)]);
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(task(&e2, &id).status, Status::HandedOff);
    assert!(w2.calls().is_empty());

    // the person fixes it by hand and gives it back
    fs::write(h.worktree.join("fixed.txt"), "by hand").unwrap();
    e2.take_back(&id).unwrap();
    let t = wait_status(&e2, &id, Status::Done);
    assert_eq!(env.main_file("fixed.txt"), "by hand", "the person's edit reached main");
    assert!(w2.calls().is_empty(), "the agent did not run again: the way back leads through gates and review");
    assert!(t.outcome.review.as_ref().unwrap().approved, "the review ran");
    assert_eq!(t.outcome.tests.as_ref().map(|x| x.passed), Some(true), "and the gates");
}

#[test]
fn when_the_session_cannot_be_continued_the_hand_off_says_a_new_one_is_opened() {
    let env = Env::new();
    let p = env.project(|_| {});
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    let w = Gated::new("fake", vec![vec![write("f.txt", "f"), done("f")]], 0, &gd);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add f");
    w.wait_started();
    let h = e.hand_off(&id).unwrap();
    assert!(!h.resumed && h.command.is_none() && h.session.is_none());
    assert!(has_log(&e, &id, "session cannot be continued, a new one is opened"));
    assert!(e.hand_off(&id).is_err(), "it is the person's already");
}

#[test]
fn a_note_written_while_the_executor_works_reaches_its_next_run_and_the_delivery_is_marked() {
    let env = Env::new();
    let p = env.project(red_once);
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    let w = Gated::new("fake", vec![vec![done("v1")], vec![write("fixed.txt", "ok"), done("v2")]], 0, &gd);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(2)], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Fix it");
    w.wait_started();
    assert_eq!(e.note(&id, None, "also keep it fast").unwrap(), NoteResult::Queued);
    w.open();
    wait_status(&e, &id, Status::Done);

    let calls = w.inner.calls();
    assert!(!calls[0].prompt.contains("also keep it fast"), "the first run had begun before the note");
    assert!(calls[1].prompt.contains("also keep it fast"), "{}", calls[1].prompt);
    let log = e.store().lock().unwrap().read_log(&id, 500);
    let delivered = log.iter().find(|l| l.text == "note delivered: also keep it fast").expect("the delivery is in the feed");
    assert_eq!(delivered.role, Some(RoleKind::Worker));
    // it is delivered once
    assert_eq!(calls.iter().filter(|c| c.prompt.contains("also keep it fast")).count(), 1);
}

#[test]
fn a_note_to_a_task_waiting_for_an_answer_is_the_answer() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = fake(vec![vec![done("Set up.\nNEEDS_HUMAN: Which database?\nOPTIONS: sqlite | postgres")], vec![write("db.txt", "sqlite"), done("used sqlite")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add storage");
    wait_status(&e, &id, Status::NeedsHuman);

    assert_eq!(e.note(&id, None, "sqlite, please").unwrap(), NoteResult::Answered);
    wait_status(&e, &id, Status::Done);
    let calls = w.calls();
    assert_eq!(calls[1].prompt, "sqlite, please");
    assert_eq!(calls[1].resume.as_deref(), Some("fake-session-0"));
}

#[test]
fn a_note_to_a_task_that_has_landed_is_not_delivered_so_the_chat_can_treat_it_as_a_message() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = fake(vec![vec![write("f.txt", "f"), done("f")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add f");
    wait_status(&e, &id, Status::Done);

    assert_eq!(e.note(&id, None, "and one more thing").unwrap(), NoteResult::NotDelivered("task_already_merged".into()));
    assert!(e.note(&id, Some("nobody".into()), "x").is_err(), "an unknown addressee");
    assert_eq!(e.note("no-such-task", None, "x").unwrap_err(), "task_not_found");
    // nothing was queued for a later run
    assert_eq!(w.calls().len(), 1);
}

#[test]
fn a_failed_task_is_lifted_in_the_same_copy_with_a_hint_and_a_stronger_model_attempts_start_again() {
    let env = Env::new();
    let p = env.project(red_once);
    // three runs leave the check red: the task fails; the fourth, on the stronger model, fixes it
    let w = fake(vec![vec![done("try 1")], vec![done("try 2")], vec![done("try 3")], vec![write("fixed.txt", "ok"), done("fixed")]]);
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store(&p, |s| env_models(s, "fake", &["weak", "strong"], "weak", &[], None)), vec![w.clone(), reviewer(1)], n.clone(), None);

    let id = submit(&e, &p, "Make fixed.txt");
    let failed = wait_status(&e, &id, Status::Failed);
    assert_eq!(failed.outcome.error.as_deref(), Some("tests_failed"));
    let wt = failed.worktree.clone().unwrap();
    assert!(wt.exists(), "the copy is kept for the person");

    let strong = ModelChoice { family: "strong".into(), version: None };
    e.resume_failed(&id, Some("create fixed.txt in the repo root".into()), Some(strong)).unwrap();
    let t = wait_status(&e, &id, Status::Done);

    let c = w.calls();
    assert_eq!(c.len(), 4);
    assert_eq!(c[3].model.as_deref(), Some("strong"));
    assert!(c[3].resume.is_none(), "a new model starts a new session");
    assert!(c[3].prompt.contains("create fixed.txt in the repo root"), "{}", c[3].prompt);
    assert!(c[3].prompt.contains("Another agent started this task"), "{}", c[3].prompt);
    assert_eq!(t.attempts, 0, "attempts start again");
    assert_eq!(t.worktree, None);
    assert_eq!(env.main_file("fixed.txt"), "ok");
    // a task that did not fail cannot be lifted
    assert_eq!(e.resume_failed(&id, None, None).unwrap_err(), "not_failed");
}

#[test]
fn a_hint_alone_resumes_the_same_session_with_the_hint() {
    let env = Env::new();
    let p = env.project(red_once);
    let w = fake(vec![vec![done("try 1")], vec![done("try 2")], vec![done("try 3")], vec![write("fixed.txt", "ok"), done("fixed")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Make fixed.txt");
    wait_status(&e, &id, Status::Failed);

    e.resume_failed(&id, Some("the file is called fixed.txt".into()), None).unwrap();
    wait_status(&e, &id, Status::Done);
    let c = w.calls();
    assert_eq!(c[3].resume.as_deref(), Some("fake-session-0"));
    assert!(c[3].prompt.contains("the file is called fixed.txt"));
}

#[test]
fn switching_the_model_restarts_the_current_role_on_the_other_model_of_the_same_env() {
    let env = Env::new();
    let p = env.project(|_| {});
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    let w = Gated::new("fake", vec![vec![done("never seen")], vec![write("f.txt", "f"), done("f")]], 0, &gd);
    let e = env.engine(env.store(&p, |s| env_models(s, "fake", &["weak", "strong"], "weak", &[], None)), vec![w.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add f");
    let pid = w.wait_started();

    e.switch_model(&id, ModelChoice { family: "strong".into(), version: None }).unwrap();
    wait_until("the held run to stop", || !alive(pid));
    wait_status(&e, &id, Status::Done);
    let c = w.inner.calls();
    assert_eq!((c[0].model.as_deref(), c[1].model.as_deref()), (Some("weak"), Some("strong")));
    assert!(c[1].resume.is_none());
    assert!(c[1].prompt.contains("Another agent started this task"));
}

#[test]
fn a_message_that_was_no_task_cancels_it_before_the_code_step_and_not_after() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let (clock, _now) = clock_at(12, 0);
    let fa = Arc::new(FakeAdapter::new(vec![vec![write("f.txt", "f"), done("f")]]).with_id("fa").with_role(RoleKind::Reviewer, vec![approve()]));
    let e = env.engine(env.store(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), Some(clock));
    let night = e
        .submit(NewTask { project_id: p.id.clone(), prompt: "how does billing work?".into(), agent: Some("fa".into()), start: Some(StartWhen::Night), ..Default::default() })
        .unwrap();
    assert_eq!(e.not_a_task(&night).unwrap(), "how does billing work?", "the text goes back to the chat");
    assert_eq!(task(&e, &night).status, Status::Cancelled);

    let real = submit_with(&e, &p, "Add f", "fa");
    wait_status(&e, &real, Status::Done);
    assert_eq!(e.not_a_task(&real).unwrap_err(), "already_coding");
}

#[test]
fn deferred_puts_the_task_back_in_the_queue_with_its_copy_and_session_at_most_twice() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = fake(vec![
        vec![write("part1.txt", "1"), done("Did the first part.\nDEFERRED: needs the login work first")],
        vec![write("part2.txt", "2"), done("Did the second.\nDEFERRED: still not ready")],
        vec![write("part3.txt", "3"), done("All done.\nDEFERRED: once more?")], // a third wait is "Не вышло" with the reason (G08.3)
    ]);
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(1)], n.clone(), None);

    let id = submit(&e, &p, "Do it in parts");
    let t = wait_status(&e, &id, Status::Failed);

    let c = w.calls();
    assert_eq!(c.len(), 3);
    assert_eq!(c[1].resume.as_deref(), Some("fake-session-0"), "the same session after the wait");
    assert_eq!(c[2].resume.as_deref(), Some("fake-session-0"));
    assert_eq!(t.start, StartWhen::AfterMerge { left: 0 });
    assert_eq!(t.outcome.error.as_deref(), Some("deferred_too_often: once more?"));
    assert!(has_log(&e, &id, "deferred: needs the login work first"));
    // the two put-offs were silent: the only call is the failure
    assert_eq!(kinds(&n), vec![NoticeKind::Failed]);
}

#[test]
fn a_snoozed_card_leaves_the_waiting_list_until_the_time_and_stays_so_after_a_restart() {
    let env = Env::new();
    let p = env.project(red_once);
    let (clock, now) = manual_clock();
    let w = fake(vec![vec![done("1")], vec![done("2")], vec![done("3")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), Some(clock));
    let id = submit(&e, &p, "Make fixed.txt");
    wait_status(&e, &id, Status::Failed);
    let t0 = *now.lock().unwrap();
    assert!(!nightshift_core::engine::is_snoozed(&task(&e, &id), t0));
    assert_eq!(e.summary_since(t0 - chrono::Duration::hours(1)).failed.len(), 1);

    e.snooze(&id, t0 + chrono::Duration::hours(2)).unwrap();
    assert!(nightshift_core::engine::is_snoozed(&task(&e, &id), t0));
    assert_eq!(e.summary_since(t0 - chrono::Duration::hours(1)).failed.len(), 0, "put off");
    e.shutdown();
    let e2 = env.engine(Store::open(&env.d.path().join("data")).unwrap(), vec![fake(vec![]), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let t = task(&e2, &id);
    assert_eq!(t.status, Status::Failed, "the copy is kept and \"retry\" is there");
    assert!(nightshift_core::engine::is_snoozed(&t, t0 + chrono::Duration::minutes(5)));
    assert!(!nightshift_core::engine::is_snoozed(&t, t0 + chrono::Duration::hours(3)), "back in the list at the time");
    assert!(t.worktree.as_ref().unwrap().exists());
    assert!(e2.retry(&id).is_ok());
}

#[test]
fn swapped_adapters_are_seen_by_the_next_task_and_a_task_in_flight_still_finishes() {
    let env = Env::new();
    let p = env.project(|_| {});
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    let old = Gated::new("fake", vec![vec![write("one.txt", "1"), done("one")]], 0, &gd);
    let e = env.engine(env.store(&p, |_| {}), vec![old.clone(), reviewer(2)], Arc::new(RecordingNotifier::default()), None);

    let first = submit(&e, &p, "First");
    old.wait_started();
    // the person changed their own commands: the list is replaced, nothing is stopped
    let new = fake(vec![vec![write("two.txt", "2"), done("two")]]);
    e.set_adapters(route_reviewers(vec![new.clone(), reviewer(2)]));
    old.open();
    wait_status(&e, &first, Status::Done);
    let second = submit(&e, &p, "Second");
    wait_status(&e, &second, Status::Done);

    assert_eq!(old.calls(), 1, "the old adapter ran the task that began on it, and only that");
    assert_eq!(new.calls().len(), 1);
    assert!(new.calls()[0].prompt.contains("Second"));
    assert_eq!(env.main_file("two.txt"), "2");
}

#[test]
fn is_active_names_the_stages_that_are_worked_on() {
    for s in [Status::Preparing, Status::Working, Status::Checking, Status::Reviewing, Status::Merging] {
        assert!(nightshift_core::engine::is_active(&s), "{s:?}");
    }
    for s in [Status::Queued, Status::NeedsHuman, Status::PausedLimit, Status::AwaitingOutward, Status::MergeBlocked, Status::Done, Status::Failed, Status::HandedOff] {
        assert!(!nightshift_core::engine::is_active(&s), "{s:?}");
    }
}

/// What each step of the pipeline runs, in order: the roles of `pipeline_fake`.
const STEPS: [RoleKind; 5] = [RoleKind::Planner, RoleKind::Critic, RoleKind::TestAuthor, RoleKind::Worker, RoleKind::Reviewer];

#[test]
fn a_restart_in_the_middle_of_any_step_goes_on_from_that_step_once_and_the_finished_steps_are_not_run_again() {
    for (i, role) in STEPS.iter().enumerate() {
        let env = Env::new();
        let p = env.project(|p| p.check_cmd = Some("test -f feature.txt".into()));
        let gd = env.d.path().join("gate");
        fs::create_dir(&gd).unwrap();
        let held = Gated::wrap(pipeline_fake("fake"), i, &gd);
        let e1 = env.engine(env.store_v2(&p, |_| {}), vec![held.clone()], Arc::new(RecordingNotifier::default()), None);
        let id = submit(&e1, &p, "Add the feature file");
        let pid = held.wait_started();
        assert_eq!(held.inner.calls().last().unwrap().role, *role, "step {i} is held");
        e1.shutdown(); // the app is closed in the middle of that step
        wait_until("the run to be killed", || !alive(pid));

        let after = Arc::new(pipeline_fake("fake"));
        let e2 = restart(&env, vec![after.clone()]);
        wait_status(&e2, &id, Status::Done);

        assert_eq!(roles_in_order(&after), STEPS[i..].to_vec(), "after a restart in step {i} ({role:?})");
        assert_eq!(e2.store().lock().unwrap().tasks().len(), 1, "no second instance");
        assert_eq!(env.main_file("feature.txt"), "hello");
        assert_eq!(g(&env.repo, &["log", "--format=%s", "main"]).lines().filter(|l| l.contains("Add the feature file")).count(), 1, "landed once (step {i})");
    }
}

#[test]
fn a_writing_role_that_has_a_session_goes_on_with_it_after_a_restart() {
    let env = Env::new();
    let p = env.project(|p| p.check_cmd = Some("test -f fixed.txt".into()));
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    // run 0 leaves the check red and gives a session; run 1 is held when the app is closed
    let held = Gated::wrap(
        FakeAdapter::new(vec![]).with_id("fake").with_role(RoleKind::Worker, vec![vec![done("v1")], vec![write("fixed.txt", "ok"), done("v2")]]).with_role(RoleKind::Reviewer, vec![approve()]),
        1,
        &gd,
    );
    let e1 = env.engine(env.store(&p, |_| {}), vec![held.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e1, &p, "Fix it");
    held.wait_started();
    e1.shutdown();

    let after = Arc::new(FakeAdapter::new(vec![]).with_id("fake").with_role(RoleKind::Worker, vec![vec![write("fixed.txt", "ok"), done("v2")]]).with_role(RoleKind::Reviewer, vec![approve()]));
    let e2 = restart(&env, vec![after.clone()]);
    wait_status(&e2, &id, Status::Done);
    let w = calls_of(&after, RoleKind::Worker);
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].resume.as_deref(), Some("fake-session-0"), "the session the executor had");
    assert!(w[0].prompt.contains("test -f fixed.txt") || w[0].prompt.contains("The project checks failed"), "{}", w[0].prompt);
}

#[test]
fn the_digest_is_built_from_the_logs_on_disk_after_a_restart() {
    let env = Env::new();
    let p = env.project(|p| p.check_cmd = Some("test -f ok.txt".into()));
    let since = chrono::Local::now() - chrono::Duration::minutes(1);
    // one task lands (and costs), one fails after three red rounds (and costs)
    let w = fake(vec![
        vec![write("ok.txt", "1"), FakeStep::Cost(0.5), done("landed")],
        vec![FakeStep::Cost(0.25), done("red 1")],
        vec![FakeStep::Cost(0.25), done("red 2")],
        vec![FakeStep::Cost(0.25), done("red 3")],
    ]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let good = submit(&e, &p, "Land me");
    wait_status(&e, &good, Status::Done);
    let e_ok_commit = task(&e, &good).outcome.commit.clone().unwrap();
    // the second task runs from a copy where ok.txt is missing: break the check by removing the file in main first
    g(&env.repo, &["rm", "-q", "ok.txt"]);
    g(&env.repo, &["commit", "-q", "-m", "drop ok.txt"]);
    let bad = submit(&e, &p, "Fail me");
    wait_status(&e, &bad, Status::Failed);
    e.shutdown();

    let e2 = restart(&env, vec![fake(vec![]), reviewer(1)]);
    let s = e2.summary_since(since);
    assert_eq!(s.done.len(), 1);
    assert_eq!(s.done[0].task_id, good);
    assert_eq!(s.done[0].commit.as_deref(), Some(e_ok_commit.as_str()));
    assert_eq!(s.failed.len(), 1);
    assert_eq!(s.failed[0].task_id, bad);
    assert_eq!(s.failed[0].text, "tests_failed");
    assert!((s.spent_usd - 1.25).abs() < 1e-9, "{}", s.spent_usd);
    assert!(!s.is_empty());
    // nothing happened after this moment: no digest
    let later = e2.summary_since(chrono::Local::now() + chrono::Duration::minutes(5));
    assert!(later.is_empty(), "{later:?}");
}

#[test]
fn a_task_with_a_limit_and_a_conflict_reaches_done_without_calling_the_person_once() {
    let env = Env::new();
    let p = env.project(|_| {});
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    // B starts first and is held; A lands a.txt meanwhile; B then hits a limit, goes on on the spare model,
    // and its rebase conflicts with A's change: the Integrator resolves it
    let held = Gated::wrap(
        FakeAdapter::new(vec![])
            .with_id("fake")
            .with_role(
                RoleKind::Worker,
                vec![
                    vec![write("a.txt", "B\n"), FakeStep::Limit],
                    vec![write("a.txt", "A\n"), done("A")],
                    vec![write("a.txt", "B\n"), done("B on the spare")],
                ],
            )
            .with_role(RoleKind::Integrator, vec![vec![write("a.txt", "A\nB\n"), done("merged both")]])
            .with_role(RoleKind::Reviewer, vec![approve(), approve()]),
        0,
        &gd,
    );
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store(&p, |s| env_models(s, "fake", &["big", "spare"], "big", &[], Some("spare"))), vec![held.clone()], n.clone(), None);

    let b = submit(&e, &p, "Write B");
    held.wait_started();
    let a = submit(&e, &p, "Write A");
    wait_status(&e, &a, Status::Done);
    held.open();
    wait_status(&e, &b, Status::Done);

    assert_eq!(env.main_file("a.txt"), "A\nB");
    let all = n.0.lock().unwrap().clone();
    assert!(!all.iter().any(|x| matches!(x.kind, NoticeKind::Question | NoticeKind::Failed | NoticeKind::Outward)), "no call: {:?}", all.iter().map(|x| &x.kind).collect::<Vec<_>>());
    assert!(has_log(&e, &b, "switched_to: spare"));
    assert!(has_log(&e, &b, "conflict resolved, rebase continued"));
}

#[test]
fn an_unanswered_call_makes_exactly_one_notice_however_long_it_waits() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = fake(vec![vec![done("Which one?\nNEEDS_HUMAN: sqlite or postgres?\nOPTIONS: sqlite | postgres")]]);
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], n.clone(), None);
    let id = submit(&e, &p, "Add storage");
    wait_status(&e, &id, Status::NeedsHuman);
    std::thread::sleep(Duration::from_millis(800)); // many scheduler ticks
    e.pause_all(true);
    e.pause_all(false);
    e.freeze(true);
    e.freeze(false);
    std::thread::sleep(Duration::from_millis(300));
    let got = n.0.lock().unwrap().clone();
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].kind, NoticeKind::Question);
    assert_eq!(task(&e, &id).status, Status::NeedsHuman, "and the task waits");
}

#[test]
fn freeze_leaves_a_waiting_question_and_a_handed_over_task_exactly_as_they_were() {
    let env = Env::new();
    let p = env.project(red_once);
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    let w = Gated::new(
        "fake",
        vec![vec![done("Which one?\nNEEDS_HUMAN: a or b?\nOPTIONS: a | b")], vec![done("terminal"), ], vec![write("fixed.txt", "x"), done("after")]],
        1,
        &gd,
    );
    let e = env.engine(env.store(&p, |s| s.max_parallel = 3), vec![w.clone(), reviewer(3)], Arc::new(RecordingNotifier::default()), None);
    let asking = submit(&e, &p, "Ask me");
    let q_before = wait_status(&e, &asking, Status::NeedsHuman).question.unwrap();
    let held = submit(&e, &p, "Go to the terminal");
    w.wait_started();
    e.hand_off(&held).unwrap();

    e.freeze(true);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(task(&e, &held).status, Status::HandedOff);
    let t = task(&e, &asking);
    assert_eq!(t.status, Status::NeedsHuman);
    assert_eq!((t.question.as_ref().unwrap().text.clone(), t.question.unwrap().options), (q_before.text.clone(), q_before.options.clone()));
    e.freeze(false);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(task(&e, &held).status, Status::HandedOff);
    assert_eq!(task(&e, &asking).status, Status::NeedsHuman);
    // the question asked before the freeze is still answerable
    e.answer(&asking, "a").unwrap();
}

#[test]
fn an_answer_to_a_snoozed_question_goes_on_at_once_not_when_the_snooze_ends() {
    let env = Env::new();
    let p = env.project(|_| {});
    let (clock, now) = manual_clock();
    let w = fake(vec![vec![done("NEEDS_HUMAN: which file?")], vec![write("f.txt", "f"), done("f")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), Some(clock));
    let id = submit(&e, &p, "Add a file");
    wait_status(&e, &id, Status::NeedsHuman);
    let t0 = *now.lock().unwrap();
    e.snooze(&id, t0 + chrono::Duration::hours(2)).unwrap();

    e.answer(&id, "f.txt").unwrap();
    let t = wait_status(&e, &id, Status::Done); // the clock does not move: a kept snooze would hold it
    assert!(t.resume_at.is_none());
}

#[test]
fn switching_the_model_of_a_running_reviewer_restarts_the_reviewer_not_the_executor() {
    let env = Env::new();
    let p = env.project(|p| p.reviewer = None);
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    let inner = FakeAdapter::new(vec![vec![write("f.txt", "f"), done("f")]]).with_id("fake").with_role(RoleKind::Reviewer, vec![vec![done("never seen")], approve()]);
    let w = Gated::wrap(inner, 1, &gd);
    let e = env.engine(env.store(&p, |s| env_models(s, "fake", &["weak", "strong"], "weak", &[], None)), vec![w.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add f");
    let pid = w.wait_started();

    e.switch_model(&id, ModelChoice { family: "strong".into(), version: None }).unwrap();
    wait_until("the held run to stop", || !alive(pid));
    let t = wait_status(&e, &id, Status::Done);
    assert_eq!(t.models.get(&RoleKind::Reviewer).map(|m| m.family.as_str()), Some("strong"));
    let r = calls_of(&w.inner, RoleKind::Reviewer);
    assert_eq!((r[0].model.as_deref(), r[1].model.as_deref()), (Some("weak"), Some("strong")));
    assert_eq!(calls_of(&w.inner, RoleKind::Worker).len(), 1);
}

#[test]
fn switching_the_model_while_the_reviewer_waits_for_an_answer_switches_the_reviewer() {
    let env = Env::new();
    let p = env.project(|p| p.reviewer = None);
    let w = Arc::new(
        FakeAdapter::new(vec![vec![write("f.txt", "f"), done("f")]])
            .with_id("fake")
            .with_role(RoleKind::Reviewer, vec![vec![done("NEEDS_HUMAN: keep the old name?\nOPTIONS: yes | no")], approve()]),
    );
    let e = env.engine(env.store(&p, |s| env_models(s, "fake", &["weak", "strong"], "weak", &[], None)), vec![w.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add f");
    wait_status(&e, &id, Status::NeedsHuman);

    e.switch_model(&id, ModelChoice { family: "strong".into(), version: None }).unwrap();
    assert_eq!(task(&e, &id).models.get(&RoleKind::Reviewer).map(|m| m.family.as_str()), Some("strong"));
    e.answer(&id, "keep it").unwrap();
    wait_status(&e, &id, Status::Done);
    let r = calls_of(&w, RoleKind::Reviewer);
    assert_eq!((r[0].model.as_deref(), r[1].model.as_deref()), (Some("weak"), Some("strong")));
    assert!(r[1].resume.is_none(), "another model is another session");
    assert!(r[1].prompt.contains("keep it"), "the answer reaches the reviewer");
    let wk = calls_of(&w, RoleKind::Worker);
    assert_eq!(wk.len(), 1, "the executor is not run again");
    assert_eq!(wk[0].model.as_deref(), Some("weak"));
}

#[test]
fn a_freeze_between_two_conflicting_commits_holds_the_next_integrator_and_unfreeze_goes_on_with_the_rebase() {
    let env = Env::new();
    let p = env.project(|p| {
        p.reviewer = None;
        red_once(p);
    });
    // the rebase of the second task stops at its first commit, then at its second: two Integrator rounds.
    // The commit made by `rebase --continue` after the first round waits in a hook until the test lets it go.
    let (started, go) = (env.d.path().join("started"), env.d.path().join("go"));
    let hook = env.repo.join(".git/hooks/prepare-commit-msg");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\ngd=$(git rev-parse --git-dir)\nif [ -d \"$gd/rebase-merge\" ] && [ ! -f '{s}' ]; then touch '{s}'; while [ ! -f '{g}' ]; do sleep 0.05; done; fi\n",
            s = started.display(),
            g = go.display()
        ),
    )
    .unwrap();
    std::process::Command::new("chmod").arg("755").arg(&hook).status().unwrap();
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    let inner = FakeAdapter::new(vec![])
        .with_id("fake")
        .with_role(
            RoleKind::Worker,
            vec![
                vec![write("a.txt", "B1\n"), done("B1")], // second task, held until the first lands; checks red
                vec![write("a.txt", "A\n"), write("fixed.txt", "ok\n"), done("A")],
                vec![write("a.txt", "B2\n"), write("fixed.txt", "ok\n"), done("B2")],
            ],
        )
        .with_role(RoleKind::Integrator, vec![vec![write("a.txt", "A\nB1\n"), done("first")], vec![write("a.txt", "A\nB2\n"), done("second")]])
        .with_role(RoleKind::Reviewer, vec![approve(), approve()]);
    let w = Gated::wrap(inner, 0, &gd);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone()], Arc::new(RecordingNotifier::default()), None);

    let b = submit(&e, &p, "Write B");
    w.wait_started();
    let a = submit(&e, &p, "Write A");
    wait_status(&e, &a, Status::Done);
    w.open();
    wait_until("the first conflict to be resolved and the rebase to go on", || started.exists());
    e.freeze(true);
    fs::write(&go, "").unwrap();
    std::thread::sleep(Duration::from_millis(800));
    let t = task(&e, &b);
    assert_eq!(t.status, Status::Merging, "the task stays in the merge");
    assert_eq!(calls_of(&w.inner, RoleKind::Integrator).len(), 1, "no Integrator starts under a freeze");
    assert!(nightshift_core::git::rebase_in_progress(t.worktree.as_ref().unwrap()), "the rebase waits where it stopped");

    e.freeze(false);
    wait_status(&e, &b, Status::Done);
    assert_eq!(calls_of(&w.inner, RoleKind::Integrator).len(), 2);
    assert_eq!(env.main_file("a.txt"), "A\nB2");
}

/// An env whose CLI cannot be started while `broken` is on: no process, so no run of it is recorded either.
struct CannotStart {
    inner: Arc<FakeAdapter>,
    broken: std::sync::atomic::AtomicBool,
}

impl Adapter for CannotStart {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn available(&self) -> Result<(), String> {
        Ok(())
    }
    fn command(&self, run: &nightshift_core::agent::Run) -> std::process::Command {
        if self.broken.load(std::sync::atomic::Ordering::SeqCst) {
            return std::process::Command::new("/nonexistent/nightshift-cli");
        }
        self.inner.command(run)
    }
    fn parse(&self, line: &str) -> Vec<nightshift_core::agent::AgentEvent> {
        self.inner.parse(line)
    }
    fn finish(&self, ok: bool, all: &str) -> Vec<nightshift_core::agent::AgentEvent> {
        self.inner.finish(ok, all)
    }
}

#[test]
fn a_note_whose_run_never_started_is_not_marked_delivered_and_reaches_the_next_run() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = Arc::new(CannotStart { inner: fake(vec![vec![write("f.txt", "x"), done("ok")]]), broken: true.into() });
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Add f");
    assert_eq!(e.note(&id, None, "use tabs").unwrap(), NoteResult::Queued);
    wait_status(&e, &id, Status::Failed);
    assert!(!has_log(&e, &id, "note delivered: use tabs"), "no process got it");

    w.broken.store(false, std::sync::atomic::Ordering::SeqCst);
    e.resume_failed(&id, None, None).unwrap();
    wait_status(&e, &id, Status::Done);
    let worker = w.inner.calls().into_iter().find(|c| c.role == RoleKind::Worker).expect("the executor ran");
    assert!(worker.prompt.contains("use tabs"), "{}", worker.prompt);
    assert_eq!(log_texts(&e, &id).iter().filter(|l| *l == "note delivered: use tabs").count(), 1);
}
