//! Engine + FakeAdapter + temporary git repo (seam 1 of the spec): the state machine of v1, which v2 keeps.
mod common;
use common::*;
use nightshift_core::agent::{Adapter, FakeAdapter, FakeStep};
use nightshift_core::engine::Engine;
use nightshift_core::model::*;
use nightshift_core::notify::{Notice, NoticeKind, Notifier, RecordingNotifier};
use nightshift_core::store::Store;
use std::fs;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn task_reaches_done_alone_with_exactly_one_commit_on_main() {
    let env = Env::new();
    let p = env.project(|_| {});
    let n = Arc::new(RecordingNotifier::default());
    let w = fake(vec![vec![write("feature.txt", "hello\n"), done("Added feature.txt")]]);
    let r = reviewer(1);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), r.clone()], n.clone(), None);
    let before = env.main_count();

    let id = submit(&e, &p, "Add a feature file\nwith details");
    let t = wait_status(&e, &id, Status::Done);

    assert_eq!(env.main_count(), before + 1);
    assert_eq!(env.main_file("feature.txt"), "hello");
    assert_eq!(g(&env.repo, &["log", "-1", "--format=%s", "main"]), "Add a feature file");
    assert_eq!(t.outcome.commit.as_deref(), Some(g(&env.repo, &["rev-parse", "main"]).as_str()));
    assert!(t.branch.starts_with(&format!("ns/{}-", &id[..8])), "branch {}", t.branch);
    assert_eq!(t.outcome.files_changed, 1);
    assert!(t.outcome.review.as_ref().unwrap().approved);
    // story 6: no check_cmd -> tests None and the human is told so
    assert!(t.outcome.tests.is_none());
    let notices = notices(&n, 1);
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].kind, NoticeKind::Done);
    assert!(notices[0].body.contains("no tests"), "body: {}", notices[0].body);
    assert!(notices[0].body.contains("Added feature.txt"), "body: {}", notices[0].body);
    // worktree removed, branch deleted (Local finish); reviewer got task + diff, fresh session
    assert!(t.worktree.is_none());
    assert!(!env.d.path().join("wts").join(&id).exists());
    assert_eq!(g(&env.repo, &["branch", "--list", "ns/*"]), "");
    let rc = r.calls();
    assert_eq!(rc.len(), 1);
    assert!(rc[0].resume.is_none());
    assert!(rc[0].prompt.contains("Add a feature file") && rc[0].prompt.contains("+hello"));
    assert_eq!(w.calls()[0].prompt, "Add a feature file\nwith details");
}

#[test]
fn red_checks_three_times_fail_the_task_and_leave_main_alone() {
    let env = Env::new();
    let p = env.project(|p| p.check_cmd = Some("echo 'assertion failed: x == 1'; exit 1".into()));
    let n = Arc::new(RecordingNotifier::default());
    let w = fake(vec![vec![write("b.txt", "1"), done("try 1")], vec![write("b.txt", "2"), done("try 2")], vec![write("b.txt", "3"), done("try 3")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(3)], n.clone(), None);
    let main_before = g(&env.repo, &["rev-parse", "main"]);

    let id = submit(&e, &p, "Make x equal 1");
    let t = wait_status(&e, &id, Status::Failed);

    assert_eq!(g(&env.repo, &["rev-parse", "main"]), main_before);
    assert_eq!(t.attempts, 3);
    assert_eq!(w.calls().len(), 3);
    // the worker got the failing output to fix, in its own session
    assert!(w.calls()[1].prompt.contains("assertion failed: x == 1"));
    assert_eq!(w.calls()[1].resume.as_deref(), Some("fake-session-0"));
    assert!(!t.outcome.tests.as_ref().unwrap().passed);
    assert_eq!(kinds(&n), vec![NoticeKind::Failed]);
    // a failed task keeps its worktree for inspection
    assert!(t.worktree.as_ref().unwrap().exists());
}

#[test]
fn red_check_is_fixed_by_the_worker_then_done_with_one_attempt() {
    let env = Env::new();
    let p = env.project(|p| p.check_cmd = Some("test -f fixed.txt || { echo 'missing fixed.txt'; exit 1; }".into()));
    let n = Arc::new(RecordingNotifier::default());
    let w = fake(vec![vec![write("b.txt", "b"), done("first")], vec![write("fixed.txt", "ok"), done("Fixed the check")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(1)], n.clone(), None);
    let before = env.main_count();

    let id = submit(&e, &p, "Add b and pass the checks");
    let t = wait_status(&e, &id, Status::Done);

    assert_eq!(t.attempts, 1);
    assert!(t.outcome.tests.as_ref().unwrap().passed);
    assert_eq!(env.main_count(), before + 1);
    assert_eq!(env.main_file("fixed.txt"), "ok");
    assert_eq!(env.main_file("b.txt"), "b");
    assert!(w.calls()[1].prompt.contains("missing fixed.txt"));
    let body = &notices(&n, 1)[0].body;
    assert!(body.contains("tests passed") && body.contains("Fixed the check"), "{body}");
}

#[test]
fn needs_human_question_then_answer_resumes_the_same_session() {
    let env = Env::new();
    let p = env.project(|_| {});
    let n = Arc::new(RecordingNotifier::default());
    let w = fake(vec![
        vec![write("db.txt", "?"), done("Set up storage.\nNEEDS_HUMAN: Which database?\nOPTIONS: sqlite | postgres")],
        vec![write("db.txt", "sqlite"), done("Used sqlite")],
    ]);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(1)], n.clone(), None);

    let id = submit(&e, &p, "Add storage");
    let t = wait_status(&e, &id, Status::NeedsHuman);
    let q = t.question.unwrap();
    assert_eq!(q.text, "Which database?");
    assert_eq!(q.options, vec!["sqlite", "postgres"]);
    assert_eq!(q.kind, QuestionKind::Agent);
    {
        let notices = notices(&n, 1);
        assert_eq!(notices[0].kind, NoticeKind::Question);
        assert_eq!(notices[0].body, "Which database?");
        assert_eq!(notices[0].options, vec!["sqlite", "postgres"]);
    }

    // the answer cannot be stored: an error, and the task keeps waiting
    let pend = env.d.path().join("wts").join(".pending");
    let _ = fs::remove_dir_all(&pend);
    fs::write(&pend, "").unwrap();
    assert!(e.answer(&id, "sqlite").is_err());
    assert_eq!(task(&e, &id).status, Status::NeedsHuman);
    fs::remove_file(&pend).unwrap();

    // two clicks at once: one answer gets through, the other finds the task no longer waiting
    let (e1, e2, i1, i2) = (e.clone(), e.clone(), id.clone(), id.clone());
    let a = std::thread::spawn(move || e1.answer(&i1, "sqlite").map(|_| "sqlite"));
    let b = std::thread::spawn(move || e2.answer(&i2, "postgres").map(|_| "postgres"));
    let rs = [a.join().unwrap(), b.join().unwrap()];
    let won: Vec<&str> = rs.iter().filter_map(|r| r.clone().ok()).collect();
    assert_eq!(won.len(), 1, "{rs:?}");
    assert!(rs.iter().any(|r| r.as_ref().err().map(String::as_str) == Some("not_waiting")), "{rs:?}");
    let t = wait_status(&e, &id, Status::Done);

    let calls = w.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1].resume.as_deref(), Some("fake-session-0"));
    assert_eq!(calls[1].prompt, won[0]);
    assert_eq!(env.main_file("db.txt"), "sqlite");
    assert!(t.question.is_none());
}

const CHANGES: &str = r#"{"verdict":"changes","issues":["handle empty input","add a test"]}"#;

#[test]
fn reviewer_changes_go_to_the_worker_then_approve_is_done() {
    let env = Env::new();
    let p = env.project(|_| {});
    let n = Arc::new(RecordingNotifier::default());
    let w = fake(vec![vec![write("f.txt", "v1"), done("v1")], vec![write("f.txt", "v2"), done("v2 with tests")]]);
    let r = Arc::new(FakeAdapter::new(vec![vec![done(CHANGES)], approve()]).with_id("rev"));
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), r.clone()], n, None);

    let id = submit(&e, &p, "Parse input");
    let t = wait_status(&e, &id, Status::Done);

    let fix = &w.calls()[1];
    assert!(fix.prompt.contains("handle empty input") && fix.prompt.contains("add a test"), "{}", fix.prompt);
    assert_eq!(fix.resume.as_deref(), Some("fake-session-0"));
    assert_eq!(t.review_rounds, 1);
    assert_eq!(env.main_file("f.txt"), "v2");
    // every review is a fresh session that sees the current diff
    assert!(r.calls().iter().all(|c| c.resume.is_none()));
    assert!(r.calls()[1].prompt.contains("+v2"));
}

#[test]
fn two_reviewer_rejections_fail_with_the_issues_on_the_card() {
    let env = Env::new();
    let p = env.project(|_| {});
    let n = Arc::new(RecordingNotifier::default());
    let w = fake(vec![vec![write("f.txt", "v1"), done("v1")], vec![write("f.txt", "v2"), done("v2")]]);
    let r = Arc::new(FakeAdapter::new(vec![vec![done(CHANGES)], vec![done(CHANGES)]]).with_id("rev"));
    let e = env.engine(env.store(&p, |s| s.review_rounds = 2), vec![w, r], n.clone(), None);
    let main_before = g(&env.repo, &["rev-parse", "main"]);

    let id = submit(&e, &p, "Parse input");
    let t = wait_status(&e, &id, Status::Failed);

    assert_eq!(t.review_rounds, 2);
    assert_eq!(t.outcome.review.as_ref().unwrap().issues, vec!["handle empty input", "add a test"]);
    assert_eq!(g(&env.repo, &["rev-parse", "main"]), main_before);
    assert_eq!(kinds(&n), vec![NoticeKind::Failed]);
}

#[test]
fn reviewer_bad_json_is_retried_once_then_counts_as_changes() {
    let env = Env::new();
    let p = env.project(|_| {});
    let n = Arc::new(RecordingNotifier::default());
    let w = fake(vec![vec![write("f.txt", "v1"), done("v1")], vec![done("nothing to change")]]);
    let r = Arc::new(FakeAdapter::new(vec![vec![done("LGTM!")], vec![done("{\"verdict\":\"approve\"}")], approve()]).with_id("rev"));
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), r.clone()], n, None);
    let id = submit(&e, &p, "Parse input");
    wait_status(&e, &id, Status::Done);
    // retry succeeded: no extra worker round
    assert_eq!(w.calls().len(), 1);
    assert_eq!(r.calls().len(), 2);

    let w2 = fake(vec![vec![write("g.txt", "v1"), done("v1")], vec![done("fixed")]]);
    let r2 = Arc::new(FakeAdapter::new(vec![vec![done("LGTM!")], vec![done("still prose")], approve()]).with_id("rev"));
    let env2 = Env::new();
    let p2 = env2.project(|_| {});
    let e2 = env2.engine(env2.store(&p2, |_| {}), vec![w2.clone(), r2], Arc::new(RecordingNotifier::default()), None);
    let id2 = submit(&e2, &p2, "Parse input");
    let t2 = wait_status(&e2, &id2, Status::Done);
    assert_eq!(t2.review_rounds, 1);
    assert!(w2.calls()[1].prompt.contains("reviewer answered in wrong format"));
}

#[test]
fn two_parallel_tasks_on_different_files_both_land_with_a_commit_each() {
    let env = Env::new();
    // every check run logs which of the two files its tree has
    let log = env.d.path().join("checks.log");
    let cmd = format!("ls x.txt y.txt 2>/dev/null | tr '\\n' ' ' >> '{l}'; echo >> '{l}'", l = log.display());
    let p = env.project(|p| p.check_cmd = Some(cmd));
    let n = Arc::new(RecordingNotifier::default());
    let wa = fake_as("fa", vec![vec![write("x.txt", "x"), FakeStep::Sleep(400), done("x")]]);
    let wb = fake_as("fb", vec![vec![write("y.txt", "y"), FakeStep::Sleep(400), done("y")]]);
    let e = env.engine(env.store(&p, |s| s.max_parallel = 2), vec![wa, wb, reviewer(2)], n, None);
    let before = env.main_count();

    let a = submit_with(&e, &p, "Add x", "fa");
    let b = submit_with(&e, &p, "Add y", "fb");
    let ta = wait_status(&e, &a, Status::Done);
    let tb = wait_status(&e, &b, Status::Done);

    assert_eq!(env.main_count(), before + 2);
    assert_eq!(env.main_file("x.txt"), "x");
    assert_eq!(env.main_file("y.txt"), "y");
    let subjects = g(&env.repo, &["log", "-2", "--format=%s", "main"]);
    assert!(subjects.contains("Add x") && subjects.contains("Add y"), "{subjects}");
    // they really overlapped
    assert!(tb.started_at.unwrap() < ta.finished_at.unwrap() && ta.started_at.unwrap() < tb.finished_at.unwrap());
    // the human's checkout follows main (rule 1: clean main is fast-forwarded)
    assert_eq!(fs::read_to_string(env.repo.join("x.txt")).unwrap(), "x");
    // the second merge checked the tree rebased onto the first one's commit
    let runs = fs::read_to_string(&log).unwrap();
    assert!(runs.lines().any(|l| l.trim() == "x.txt y.txt"), "{runs}");
}

#[test]
fn with_max_parallel_one_the_second_task_of_the_env_waits_queued() {
    let env = Env::new();
    let p = env.project(|_| {});
    let n = Arc::new(RecordingNotifier::default());
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    let wa = Gated::new("fa", vec![vec![write("x.txt", "x"), done("x")], vec![write("y.txt", "y"), done("y")]], 0, &gd);
    let e = env.engine(env.store(&p, |s| s.max_parallel = 1), vec![wa.clone(), reviewer(2)], n, None);

    let a = submit_with(&e, &p, "Add x", "fa");
    let b = submit_with(&e, &p, "Add y", "fa");
    wa.wait_started(); // held at the gate: the only seat of the env is taken
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(task(&e, &b).status, Status::Queued);
    assert_eq!(wa.calls(), 1);
    wa.open();

    wait_status(&e, &a, Status::Done);
    wait_status(&e, &b, Status::Done);
    assert_eq!(env.main_file("y.txt"), "y");
}

/// Task B forks main, then waits on a question until task A has landed an edit of the same line.
fn conflicting_pair(resolution: Vec<FakeStep>) -> (Env, Arc<Engine>, String, Arc<FakeAdapter>) {
    let env = Env::new();
    let p = env.project(|_| {});
    let wa = fake_as("fa", vec![vec![write("a.txt", "A\n"), done("A")]]);
    let wb = fake_as("fb", vec![vec![write("a.txt", "B\n"), done("B\nNEEDS_HUMAN: go on?")], vec![done("ok")], resolution]);
    let e = env.engine(env.store(&p, |_| {}), vec![wa, wb.clone(), reviewer(4)], Arc::new(RecordingNotifier::default()), None);
    let b = submit_with(&e, &p, "Write B", "fb");
    wait_status(&e, &b, Status::NeedsHuman);
    let a = submit_with(&e, &p, "Write A", "fa");
    wait_status(&e, &a, Status::Done);
    e.answer(&b, "yes").unwrap();
    (env, e, b, wb)
}

#[test]
fn rebase_conflict_is_resolved_by_the_integrator_in_a_fresh_session_then_done() {
    let (env, e, b, wb) = conflicting_pair(vec![write("a.txt", "A\nB\n"), done("merged both")]);
    let t = wait_status(&e, &b, Status::Done);
    let calls = wb.calls();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[2].role, RoleKind::Integrator);
    assert!(calls[2].prompt.contains("conflict"), "{}", calls[2].prompt);
    assert!(calls[2].resume.is_none(), "the executor is not the one who resolves");
    assert_eq!(env.main_file("a.txt"), "A\nB");
    assert_eq!(g(&env.repo, &["log", "-1", "--format=%s", "main"]), "Write B");
    assert!(t.outcome.commit.is_some());
}

#[test]
fn conflict_the_integrator_leaves_unresolved_fails_as_conflict_with_main() {
    let (env, e, b, _) = conflicting_pair(vec![done("could not")]);
    let main_before = g(&env.repo, &["rev-parse", "main"]);
    let t = wait_status(&e, &b, Status::Failed);
    assert_eq!(t.outcome.error.as_deref(), Some("conflict_with_main"));
    assert_eq!(g(&env.repo, &["rev-parse", "main"]), main_before);
    assert_eq!(env.main_file("a.txt"), "A");
}

#[test]
fn dirty_main_postpones_the_merge_keeps_the_humans_edit_and_retries_each_minute() {
    let env = Env::new();
    let p = env.project(|_| {});
    let (clock, now) = manual_clock();
    let w = fake(vec![vec![write("feature.txt", "f"), done("feature")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), Some(clock));
    fs::write(env.repo.join("a.txt"), "human edit\n").unwrap();
    let before = env.main_count();

    let id = submit(&e, &p, "Add feature");
    let t = wait_status(&e, &id, Status::MergeBlocked);
    assert_eq!(t.outcome.error.as_deref(), Some("merge_blocked_dirty_main"));
    let t0 = *now.lock().unwrap();
    assert_eq!(t.resume_at, Some(t0 + chrono::Duration::seconds(60)));
    assert_eq!(fs::read_to_string(env.repo.join("a.txt")).unwrap(), "human edit\n");
    assert_eq!(env.main_count(), before);

    // the human commits their edit; nothing happens until the minute is up
    g(&env.repo, &["commit", "-qam", "human work"]);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(task(&e, &id).status, Status::MergeBlocked);
    *now.lock().unwrap() = t0 + chrono::Duration::seconds(61);
    let t = wait_status(&e, &id, Status::Done);

    assert_eq!(env.main_count(), before + 2);
    assert_eq!(env.main_file("a.txt"), "human edit");
    assert_eq!(env.main_file("feature.txt"), "f");
    assert!(t.outcome.error.is_none());
}

#[test]
fn push_finish_asks_first_then_pushes_to_origin() {
    let env = Env::new();
    let bare = with_origin(&env);
    let p = env.project(|p| {
        p.finish = Finish::Push;
        p.ask_outward = true;
    });
    let n = Arc::new(RecordingNotifier::default());
    let w = fake(vec![vec![write("f.txt", "f"), done("f")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], n.clone(), None);
    let origin_before = g(&bare, &["rev-parse", "main"]);

    let id = submit(&e, &p, "Add f");
    let t = wait_status(&e, &id, Status::AwaitingOutward);
    assert_eq!(t.question.as_ref().unwrap().kind, QuestionKind::Outward);
    assert_eq!(g(&bare, &["rev-parse", "main"]), origin_before, "nothing leaves the machine before approval");
    let outward = notices(&n, 1).iter().find(|x| x.kind == NoticeKind::Outward).cloned().unwrap();
    assert_eq!(outward.options, vec!["Push", "Skip"]);

    e.approve_outward(&id, true).unwrap();
    let t = wait_status(&e, &id, Status::Done);
    assert_eq!(g(&bare, &["rev-parse", "main"]), g(&env.repo, &["rev-parse", "main"]));
    assert_eq!(t.outcome.commit.as_deref(), Some(g(&bare, &["rev-parse", "main"]).as_str()));
}

#[test]
fn refused_push_keeps_the_commit_locally_as_done_not_pushed() {
    let env = Env::new();
    let bare = with_origin(&env);
    // someone else pushed first: a plain push is rejected (never forced)
    let other = env.d.path().join("other");
    g(env.d.path(), &["clone", "-q", bare.to_str().unwrap(), other.to_str().unwrap()]);
    g(&other, &["config", "user.email", "o@example.com"]);
    g(&other, &["config", "user.name", "O"]);
    fs::write(other.join("o.txt"), "o").unwrap();
    g(&other, &["add", "-A"]);
    g(&other, &["commit", "-qm", "theirs"]);
    g(&other, &["push", "-q", "origin", "main"]);
    let theirs = g(&bare, &["rev-parse", "main"]);

    let p = env.project(|p| {
        p.finish = Finish::Push;
        p.ask_outward = false;
    });
    let w = fake(vec![vec![write("f.txt", "f"), done("f")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add f");
    let t = wait_status(&e, &id, Status::DoneNotPushed);

    assert!(t.outcome.error.as_deref().unwrap().contains("push failed"), "{:?}", t.outcome.error);
    assert_eq!(g(&bare, &["rev-parse", "main"]), theirs);
    assert_eq!(env.main_file("f.txt"), "f");
    assert_eq!(t.outcome.commit.as_deref(), Some(g(&env.repo, &["rev-parse", "main"]).as_str()));
}

#[test]
fn usage_limit_pauses_until_reset_and_then_continues_by_itself() {
    let env = Env::new();
    let p = env.project(|_| {});
    let (clock, now) = manual_clock();
    let n = Arc::new(RecordingNotifier::default());
    let w = fake(vec![vec![write("half.txt", "half"), FakeStep::Limit], vec![write("rest.txt", "rest"), done("finished")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(1)], n.clone(), Some(clock));

    let id = submit(&e, &p, "Big job");
    let t = wait_status(&e, &id, Status::PausedLimit);
    let t0 = *now.lock().unwrap();
    // no reset time in the message: retry in 30 minutes (spec, story 34)
    assert_eq!(t.resume_at, Some(t0 + chrono::Duration::minutes(30)));
    assert_eq!(kinds(&n), vec![NoticeKind::Limit]);
    assert!(!NoticeKind::Limit.is_loud());

    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(w.calls().len(), 1, "must not continue before the reset");
    *now.lock().unwrap() = t0 + chrono::Duration::minutes(31);
    wait_status(&e, &id, Status::Done);

    assert_eq!(w.calls().len(), 2);
    assert_eq!(env.main_file("half.txt"), "half");
    assert_eq!(env.main_file("rest.txt"), "rest");
}

/// Settings of the env "fake" with two models of its own, the first the executor's, the second the spare.
fn two_models(s: &mut Settings, big: &str, spare: &str) {
    let m = |f: &str| ModelChoice { family: f.into(), version: None };
    s.envs.insert(
        "fake".into(),
        EnvSettings {
            custom_models: vec![big.into(), spare.into()],
            default_model: Some(m(big)),
            fallback: Some(m(spare)),
            ask_push: false,
            ask_pr: false,
            ..EnvSettings::default()
        },
    );
}

#[test]
fn usage_limit_with_a_fallback_model_continues_there_on_the_same_branch_in_a_new_session() {
    let env = Env::new();
    let p = env.project(|p| {
        p.check_cmd = Some("test -f rest.txt || { echo 'need rest.txt'; exit 1; }".into());
    });
    let n = Arc::new(RecordingNotifier::default());
    // red check, then the limit hits the run that was to fix it
    let w = fake(vec![
        vec![write("half.txt", "half"), done("half")],
        vec![FakeStep::Limit],
        vec![write("rest.txt", "rest"), done("finished on the spare model")],
    ]);
    let e = env.engine(env.store(&p, |s| two_models(s, "big", "spare")), vec![w.clone(), reviewer(1)], n.clone(), None);

    let id = submit(&e, &p, "Big job");
    let t = wait_status(&e, &id, Status::Done);

    let c = w.calls();
    assert_eq!(c.len(), 3);
    assert_eq!((c[0].model.as_deref(), c[1].model.as_deref(), c[2].model.as_deref()), (Some("big"), Some("big"), Some("spare")));
    assert!(c[2].resume.is_none(), "new session on the fallback");
    assert!(c[2].prompt.contains("Big job") && c[2].prompt.contains("+half"), "{}", c[2].prompt);
    assert!(c[2].prompt.contains("need rest.txt"), "the pending fix request reaches the fallback: {}", c[2].prompt);
    assert_eq!(t.attempts, 1);
    assert_eq!(t.agent, "fake", "the env never changes");
    let log = e.store().lock().unwrap().read_log(&id, 500);
    assert!(log.iter().any(|l| l.text == "switched_to: spare"));
    assert_eq!(env.main_file("half.txt"), "half");
    assert_eq!(env.main_file("rest.txt"), "rest");
    assert!(!kinds(&n).contains(&NoticeKind::Limit));
}

#[test]
fn undo_reverts_the_task_commit_on_main() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = fake(vec![vec![write("a.txt", "changed\n"), done("changed a")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Change a");
    let t = wait_status(&e, &id, Status::Done);
    let before = env.main_count();

    e.undo(&id).unwrap();

    assert_eq!(task(&e, &id).status, Status::Reverted);
    assert_eq!(env.main_count(), before + 1);
    assert_eq!(env.main_file("a.txt"), "one");
    let msg = g(&env.repo, &["log", "-1", "--format=%B", "main"]);
    assert!(msg.contains(t.outcome.commit.as_deref().unwrap()), "{msg}");
    assert!(e.undo(&id).is_err(), "already reverted");
}

#[test]
fn files_from_copy_files_and_worktreeinclude_reach_the_worktree_but_never_main() {
    let env = Env::new();
    fs::write(env.repo.join(".gitignore"), ".env*\nsecrets/\n").unwrap();
    fs::write(env.repo.join(".worktreeinclude"), "# local config\nsecrets/local.json\nnotes.txt\n").unwrap();
    g(&env.repo, &["add", "-A"]);
    g(&env.repo, &["commit", "-qm", "ignore rules"]);
    fs::write(env.repo.join(".env"), "TOKEN=x").unwrap();
    fs::create_dir(env.repo.join("secrets")).unwrap();
    fs::write(env.repo.join("secrets/local.json"), "{}").unwrap();
    fs::write(env.repo.join("notes.txt"), "untracked, not ignored").unwrap();
    let p = env.project(|p| p.check_cmd = Some("test -f .env && test -f secrets/local.json && test -f notes.txt".into()));
    let w = fake(vec![vec![write("f.txt", "f"), done("f")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Add f");
    let t = wait_status(&e, &id, Status::Done);

    assert!(t.outcome.tests.unwrap().passed);
    let files = g(&env.repo, &["ls-tree", "-r", "--name-only", "main"]);
    assert!(!files.contains(".env") && !files.contains("secrets") && !files.contains("notes.txt"), "{files}");
}

#[test]
fn retry_of_a_failed_task_starts_over_from_a_clean_branch() {
    let env = Env::new();
    let p = env.project(|p| p.check_cmd = Some("test ! -f bad.txt".into()));
    let w = fake(vec![
        vec![write("bad.txt", "1"), done("1")],
        vec![done("2")],
        vec![done("3")],
        vec![write("good.txt", "g"), done("clean")],
    ]);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Do it");
    let failed = wait_status(&e, &id, Status::Failed);
    // a landing marker left for this branch name must not make the new work count as landed
    g(&env.repo, &["update-ref", &format!("refs/ns-merged/{}", failed.branch), "main"]);
    let before = env.main_count();

    e.retry(&id).unwrap();
    let t = wait_status(&e, &id, Status::Done);

    assert_eq!(t.attempts, 0);
    assert!(w.calls()[3].resume.is_none(), "fresh session");
    assert_eq!(w.calls()[3].prompt, "Do it");
    let files = g(&env.repo, &["ls-tree", "-r", "--name-only", "main"]);
    assert!(files.contains("good.txt") && !files.contains("bad.txt"), "{files}");
    assert_eq!(env.main_count(), before + 1);
}

#[test]
fn pr_finish_pushes_the_branch_and_records_the_pr_url() {
    fake_gh();

    let env = Env::new();
    let bare = with_origin(&env);
    let p = env.project(|p| {
        p.finish = Finish::Pr;
        p.ask_outward = false;
    });
    let n = Arc::new(RecordingNotifier::default());
    let w = fake(vec![vec![write("f.txt", "f"), done("f")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], n.clone(), None);
    let main_before = g(&env.repo, &["rev-parse", "main"]);

    let id = submit(&e, &p, "Add f");
    let t = wait_status(&e, &id, Status::Done);

    assert_eq!(t.outcome.pr_url.as_deref(), Some("https://github.com/o/r/pull/9"));
    assert!(notices(&n, 1)[0].body.contains("https://github.com/o/r/pull/9"));
    assert_eq!(g(&bare, &["show", &format!("{}:f.txt", t.branch)]), "f");
    assert_eq!(g(&env.repo, &["rev-parse", "main"]), main_before, "the PR, not local main, carries the change");
    assert!(g(&env.repo, &["branch", "--list", &t.branch]).contains(&t.branch), "branch stays for the PR");
    assert!(t.worktree.is_none());
    assert_eq!(e.undo(&id).unwrap_err(), "pr_no_undo");
}

#[test]
fn cancel_while_working_kills_the_agent_and_removes_worktree_and_branch() {
    let env = Env::new();
    let p = env.project(|_| {});
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    let w = Gated::new("fake", vec![vec![write("x.txt", "x"), done("x")]], 0, &gd);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let main_before = g(&env.repo, &["rev-parse", "main"]);

    let id = submit(&e, &p, "Slow job");
    let pid = w.wait_started();
    let wt = task(&e, &id).worktree.unwrap();
    assert!(alive(pid));
    e.cancel(&id).unwrap();

    assert!(!alive(pid), "agent process survived cancel");
    let t = task(&e, &id);
    assert_eq!(t.status, Status::Cancelled);
    assert!(t.worktree.is_none() && !wt.exists());
    assert_eq!(g(&env.repo, &["branch", "--list", "ns/*"]), "");
    assert_eq!(g(&env.repo, &["rev-parse", "main"]), main_before);
}

#[test]
fn cancel_racing_the_scheduler_stays_cancelled_and_never_starts() {
    let env = Env::new();
    let p = env.project(|_| {});
    let (clock, now) = manual_clock();
    // the executors wait at a gate that never opens: a task that started is still going when its cancel comes, however
    // long the cancels before it took (each waits for its own task's git step to end)
    let w = HoldRole::wrap(FakeAdapter::new((0..6).map(|_| vec![done("late")]).collect()), RoleKind::Worker, env.d.path());
    let lim = fake_as("lim", vec![vec![FakeStep::Limit]]);
    let e = env.engine(env.store(&p, |s| s.max_parallel = 8), vec![w.clone(), lim.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), Some(clock));

    // a queued task cancelled right after submit, while ticks pick up new tasks
    let ids: Vec<String> = (0..6).map(|i| submit(&e, &p, &format!("Job {i}"))).collect();
    for id in &ids {
        e.cancel(id).unwrap();
    }
    let calls = w.inner.calls().len();
    std::thread::sleep(Duration::from_millis(300));
    for id in &ids {
        let t = task(&e, id);
        assert_eq!(t.status, Status::Cancelled);
        assert!(t.worktree.is_none());
    }
    assert_eq!(w.inner.calls().len(), calls, "nothing starts after cancel returned");

    // a paused task cancelled exactly when its reset time arrives
    let id = submit_with(&e, &p, "Limited", "lim");
    let t = wait_status(&e, &id, Status::PausedLimit);
    *now.lock().unwrap() = t.resume_at.unwrap();
    e.cancel(&id).unwrap();
    // a tick may win the claim a moment earlier and start the agent; cancel kills it, and
    // nothing starts once cancel has returned
    let before = lim.calls().len();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(task(&e, &id).status, Status::Cancelled);
    assert_eq!(lim.calls().len(), before);
}

#[test]
fn restart_in_the_middle_of_working_resends_the_pending_prompt_in_one_instance() {
    let env = Env::new();
    let p = env.project(|p| p.check_cmd = Some("test -f fixed.txt || { echo 'need fixed.txt'; exit 1; }".into()));
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    // run 1 ends with a session; the check is red; run 2 (the fix) hangs until the quit
    let w1 = Gated::new("fake", vec![vec![write("b.txt", "b"), done("first")], vec![done("never")]], 1, &gd);
    let e1 = env.engine(env.store(&p, |_| {}), vec![w1.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e1, &p, "Add b");
    let pid = w1.wait_started();
    e1.shutdown();
    assert!(!alive(pid), "quit must stop the agent");
    assert_eq!(task(&e1, &id).status, Status::Working, "quit leaves the stage on disk");
    drop(e1);

    let store = Store::open(&env.d.path().join("data")).unwrap();
    let w2 = fake(vec![vec![write("fixed.txt", "ok"), done("fixed")]]);
    let e2 = env.engine(store, vec![w2.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let before = env.main_count();
    let t = wait_status(&e2, &id, Status::Done);

    let c = w2.calls();
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].resume.as_deref(), Some("fake-session-0"));
    // the fix request survived the restart (not a bare "continue")
    assert!(c[0].prompt.contains("need fixed.txt"), "{}", c[0].prompt);
    assert_eq!(e2.store().lock().unwrap().tasks().len(), 1);
    assert_eq!(env.main_count(), before + 1);
    assert_eq!(env.main_file("b.txt"), "b");
    assert_eq!(t.attempts, 1);
}

#[test]
fn answer_given_before_a_quit_reaches_the_worker_after_restart() {
    let env = Env::new();
    let p = env.project(|_| {});
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    let w1 = Gated::new("fake", vec![vec![done("NEEDS_HUMAN: Which DB?")], vec![done("never")]], 1, &gd);
    let e1 = env.engine(env.store(&p, |_| {}), vec![w1.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e1, &p, "Add storage");
    wait_status(&e1, &id, Status::NeedsHuman);
    e1.answer(&id, "sqlite").unwrap();
    w1.wait_started();
    e1.shutdown();
    drop(e1);

    let w2 = fake(vec![vec![write("db.txt", "sqlite"), done("ok")]]);
    let e2 = env.engine(Store::open(&env.d.path().join("data")).unwrap(), vec![w2.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    wait_status(&e2, &id, Status::Done);
    assert_eq!(w2.calls()[0].prompt, "sqlite");
}

#[test]
fn shutdown_racing_the_scheduler_leaves_one_instance_after_restart() {
    for delay in [0u64, 15, 30, 45, 60] {
        let env = Env::new();
        let p = env.project(|_| {});
        let w1 = fake(vec![vec![write("x.txt", "x"), done("x")]]);
        let e1 = env.engine(env.store(&p, |_| {}), vec![w1.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
        let id = submit(&e1, &p, "Add x");
        std::thread::sleep(Duration::from_millis(delay));
        e1.shutdown();
        let calls = w1.calls().len();
        let st1 = task(&e1, &id);
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(w1.calls().len(), calls, "delay {delay}: work started after shutdown returned");
        assert_eq!(task(&e1, &id).status, st1.status, "delay {delay}: state changed after shutdown");
        drop(e1);

        let before = env.main_count();
        let w2 = fake(vec![vec![write("x.txt", "x"), done("x")]]);
        let e2 = env.engine(Store::open(&env.d.path().join("data")).unwrap(), vec![w2, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
        let t = wait(&e2, &id, "finished", |t| t.status == Status::Done);
        assert_eq!(env.main_count(), before + if st1.status == Status::Done { 0 } else { 1 }, "delay {delay}");
        assert!(t.outcome.commit.is_some());
        assert_eq!(e2.store().lock().unwrap().tasks().len(), 1);
    }
}

#[test]
fn pause_lets_the_current_agent_step_finish_then_holds_everything_until_unpaused() {
    let env = Env::new();
    let p = env.project(|_| {});
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    // one env with one seat: the second task waits behind the first
    let w = Gated::new("fa", vec![vec![write("x.txt", "x"), done("x")], vec![write("y.txt", "y"), done("y")]], 0, &gd);
    let e = env.engine(env.store(&p, |s| s.max_parallel = 1), vec![w.clone(), reviewer(2)], Arc::new(RecordingNotifier::default()), None);
    let a = submit_with(&e, &p, "Add x", "fa");
    let b = submit_with(&e, &p, "Add y", "fa");
    w.wait_started(); // the agent is running and cannot finish before the gate opens

    e.pause_all(true);
    w.open();
    let held = wait(&e, &a, "agent step finished", |t| t.status == Status::Checking);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(task(&e, &a).status, held.status, "no next step while paused");
    assert_eq!(task(&e, &b).status, Status::Queued);
    assert_eq!(w.calls(), 1, "the queued task did not start");

    e.pause_all(false);
    wait_status(&e, &a, Status::Done);
    wait_status(&e, &b, Status::Done);
}

/// Adapter that panics while parsing the agent's output (a bug inside the engine's thread).
struct Panicky(FakeAdapter);
impl Adapter for Panicky {
    fn id(&self) -> &str {
        "fake"
    }
    fn name(&self) -> &str {
        "Panicky"
    }
    fn available(&self) -> Result<(), String> {
        Ok(())
    }
    fn command(&self, run: &nightshift_core::agent::Run) -> Command {
        self.0.command(run)
    }
    fn parse(&self, _line: &str) -> Vec<nightshift_core::agent::AgentEvent> {
        panic!("parser bug")
    }
}

#[test]
fn a_panic_while_driving_fails_the_task_once_and_the_engine_keeps_working() {
    let env = Env::new();
    let p = env.project(|_| {});
    let n = Arc::new(RecordingNotifier::default());
    let bad = Arc::new(Panicky(FakeAdapter::new(vec![vec![done("x")]])));
    let good = fake_as("ok", vec![vec![write("y.txt", "y"), done("y")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![bad.clone(), good, reviewer(1)], n.clone(), None);

    let id = submit(&e, &p, "Boom");
    let t = wait_status(&e, &id, Status::Failed);
    assert!(t.outcome.error.as_deref().unwrap().contains("parser bug"), "{:?}", t.outcome.error);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(bad.0.calls().len(), 1, "no restart loop");
    assert_eq!(kinds(&n), vec![NoticeKind::Failed]);

    let id2 = submit_with(&e, &p, "Fine", "ok");
    wait_status(&e, &id2, Status::Done);
}

#[test]
fn crash_between_squash_and_save_does_not_land_the_task_twice() {
    let env = Env::new();
    let p = env.project(|_| {});
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    let w = fake(vec![vec![write("f.txt", "f"), done("f")]]);
    let rev = Gated::new("rev", vec![approve()], 0, &gd);
    let e1 = env.engine(env.store(&p, |_| {}), vec![w, rev.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e1, &p, "Add f");
    rev.wait_started();
    e1.shutdown();
    drop(e1);
    // what a crash right after the squash leaves behind: commit on main, task still merging
    let mut st = Store::open(&env.d.path().join("data")).unwrap();
    let mut t = st.task(&id).unwrap();
    let wt = t.worktree.clone().unwrap();
    let sha = nightshift_core::git::squash_to_base(&env.repo, &wt, "main", "Add f").unwrap();
    t.status = Status::Merging;
    st.upsert_task(&t).unwrap();
    let count = env.main_count();

    let e2 = env.engine(st, vec![fake(vec![]), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let t = wait_status(&e2, &id, Status::Done);
    assert_eq!(env.main_count(), count, "landed twice");
    assert_eq!(t.outcome.commit.as_deref(), Some(sha.as_str()));
}

#[test]
fn env_files_of_a_project_made_by_init_repo_reach_the_worktree_but_never_main() {
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("app");
    fs::create_dir(&repo).unwrap();
    fs::write(repo.join("main.txt"), "m").unwrap();
    fs::write(repo.join(".env"), "TOKEN=x").unwrap();
    nightshift_core::git::init_repo(&repo).unwrap(); // .env stays untracked and NOT ignored
    let mut p = Project::new("app", repo.clone(), "main");
    p.agent = "fake".into();
    p.reviewer = Some("rev".into());
    p.check_cmd = Some("test -f .env".into());
    let mut st = Store::open(&d.path().join("data")).unwrap();
    st.upsert_project(p.clone()).unwrap();
    let mut s = st.settings();
    s.quality.on = false; // the short path of v1
    s.envs.insert("fake".into(), EnvSettings { ask_push: false, ask_pr: false, ..EnvSettings::default() });
    st.set_settings(s).unwrap();
    let w = fake(vec![vec![write("f.txt", "f"), done("f")]]);
    let e = Engine::new(st, route_reviewers(vec![w, reviewer(1)]), Arc::new(RecordingNotifier::default()), d.path().join("wts"));
    e.start();

    let id = submit(&e, &p, "Add f");
    let t = wait_status(&e, &id, Status::Done);

    assert!(t.outcome.tests.unwrap().passed, "copied .env was visible to the check");
    let files = g(&repo, &["ls-tree", "-r", "--name-only", "main"]);
    assert!(files.contains("f.txt") && !files.contains(".env"), "{files}");
}

#[test]
fn undo_of_a_pushed_task_asks_before_pushing_the_revert() {
    let env = Env::new();
    let bare = with_origin(&env);
    let p = env.project(|p| {
        p.finish = Finish::Push;
        p.ask_outward = true;
    });
    let n = Arc::new(RecordingNotifier::default());
    let w = fake(vec![vec![write("f.txt", "f"), done("f")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], n.clone(), None);
    let id = submit(&e, &p, "Add f");
    wait_status(&e, &id, Status::AwaitingOutward);
    e.approve_outward(&id, true).unwrap();
    wait_status(&e, &id, Status::Done);
    let pushed = g(&bare, &["rev-parse", "main"]);

    e.undo(&id).unwrap();
    let t = task(&e, &id);
    assert_eq!(t.status, Status::AwaitingOutward);
    assert_eq!(t.question.as_ref().unwrap().kind, QuestionKind::Outward);
    assert_eq!(g(&bare, &["rev-parse", "main"]), pushed, "revert not pushed without approval");
    assert_eq!(notices(&n, 3).iter().filter(|x| x.kind == NoticeKind::Outward).count(), 2);

    e.approve_outward(&id, true).unwrap();
    assert_eq!(task(&e, &id).status, Status::Reverted);
    assert_eq!(g(&bare, &["rev-parse", "main"]), g(&env.repo, &["rev-parse", "main"]));
    assert_eq!(g(&bare, &["show", "main:a.txt"]), "one");
    assert!(g(&bare, &["ls-tree", "--name-only", "main"]).lines().all(|f| f != "f.txt"));
}

#[test]
fn double_approve_opens_one_pr() {
    let log = fake_gh();
    let env = Env::new();
    with_origin(&env);
    let p = env.project(|p| {
        p.finish = Finish::Pr;
        p.ask_outward = true;
    });
    let w = fake(vec![vec![write("f.txt", "f"), done("f")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add f");
    let t = wait_status(&e, &id, Status::AwaitingOutward);

    let (e1, e2, i1, i2) = (e.clone(), e.clone(), id.clone(), id.clone());
    let a = std::thread::spawn(move || e1.approve_outward(&i1, true));
    let b = std::thread::spawn(move || e2.approve_outward(&i2, true));
    let ok = [a.join().unwrap(), b.join().unwrap()].iter().filter(|r| r.is_ok()).count();

    assert_eq!(ok, 1);
    assert_eq!(gh_calls_for(&log, &t.branch), 1);
    assert_eq!(task(&e, &id).status, Status::Done);
}

/// FakeAdapter whose invocation number `at` first runs `sh` in the worktree (the agent's own commands).
struct Shell {
    inner: FakeAdapter,
    at: usize,
    sh: String,
}

impl Shell {
    fn new(id: &str, scripts: Vec<Vec<FakeStep>>, at: usize, sh: &str) -> Arc<Shell> {
        Arc::new(Shell { inner: FakeAdapter::new(scripts).with_id(id), at, sh: sh.into() })
    }
}

impl Adapter for Shell {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn name(&self) -> &str {
        "Shell"
    }
    fn available(&self) -> Result<(), String> {
        Ok(())
    }
    fn command(&self, run: &nightshift_core::agent::Run) -> Command {
        let n = self.inner.calls().len();
        let inner = self.inner.command(run);
        let script = inner.get_args().nth(1).unwrap().to_string_lossy().to_string();
        let script = if n == self.at { format!("{}\n{script}", self.sh) } else { script };
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

#[test]
fn copied_env_never_lands_whatever_the_worker_does_with_gitignore_or_git() {
    let env = Env::new();
    fs::write(env.repo.join(".env"), "TOKEN=x").unwrap(); // untracked, not ignored
    let p = env.project(|p| p.copy_files = vec![".env".into()]);
    // task 0 adds an ignore rule for .env, task 1 drops it again, task 2's worker commits .env itself
    let w = Shell::new(
        "fake",
        vec![
            vec![write(".gitignore", ".env\n"), write("a0.txt", "0"), done("ignore env")],
            vec![write(".gitignore", "tmp/\n"), write("a1.txt", "1"), done("drop the rule")],
            vec![write("a2.txt", "2"), done("committed myself")],
        ],
        2,
        "git add -A && git commit -qm 'agent commit'",
    );
    // the 4th review commits .env in its own copy
    let rev = Shell::new("rev", (0..4).map(|_| approve()).collect(), 3, "git add -f .env && git commit -qm 'reviewer commit'");
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), rev], Arc::new(RecordingNotifier::default()), None);

    for i in 0..3 {
        let id = submit(&e, &p, &format!("Step {i}"));
        let t = wait_status(&e, &id, Status::Done);
        let files = g(&env.repo, &["ls-tree", "-r", "--name-only", "main"]);
        assert!(!files.lines().any(|f| f == ".env"), "step {i}: {files}");
        assert_eq!(env.main_file(&format!("a{i}.txt")), i.to_string());
        assert!(t.outcome.error.is_none());
    }
    assert_eq!(env.main_file(".gitignore"), "tmp/");

    // the reviewer commits .env too, but it reads in a throwaway copy: the commit never reaches the branch
    let id = submit(&e, &p, "Step 3");
    let t = wait_status(&e, &id, Status::Done);
    let files = g(&env.repo, &["ls-tree", "-r", "--name-only", "main"]);
    assert!(!files.lines().any(|f| f == ".env"), "{files}");
    assert!(t.outcome.error.is_none());
}

#[test]
fn push_base_after_a_refused_push_marks_the_waiting_tasks_done() {
    let env = Env::new();
    let bare = with_origin(&env);
    g(&env.repo, &["remote", "set-url", "origin", env.d.path().join("missing.git").to_str().unwrap()]);
    let p = env.project(|p| {
        p.finish = Finish::Push;
        p.ask_outward = false;
    });
    let w = fake(vec![vec![write("f.txt", "f"), done("f")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add f");
    wait_status(&e, &id, Status::DoneNotPushed);

    g(&env.repo, &["remote", "set-url", "origin", bare.to_str().unwrap()]);
    e.push_base(&p.id).unwrap();

    let t = task(&e, &id);
    assert_eq!(t.status, Status::Done);
    assert!(t.outcome.error.is_none());
    assert_eq!(g(&bare, &["rev-parse", "main"]), g(&env.repo, &["rev-parse", "main"]));
}

#[test]
fn agent_errors_are_retried_in_the_same_session_and_do_not_use_up_check_rounds() {
    let env = Env::new();
    let p = env.project(|p| p.check_cmd = Some("test -f fixed.txt || { echo 'need fixed.txt'; exit 1; }".into()));
    let w = fake(vec![
        vec![write("b.txt", "b"), done("first")],
        vec![FakeStep::Fail],
        vec![FakeStep::Fail],
        vec![write("fixed.txt", "ok"), done("fixed")],
    ]);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add b");
    let t = wait_status(&e, &id, Status::Done);

    assert_eq!(t.attempts, 1, "only the red check counts");
    let calls = w.calls();
    assert_eq!(calls.len(), 4);
    for c in &calls[1..] {
        assert_eq!(c.resume.as_deref(), Some("fake-session-0"));
        assert!(c.prompt.contains("need fixed.txt"), "same prompt again: {}", c.prompt);
    }
    assert_eq!(env.main_file("fixed.txt"), "ok");

    // three errors in a row give up
    let env2 = Env::new();
    let p2 = env2.project(|_| {});
    let w2 = fake(vec![vec![FakeStep::Fail], vec![FakeStep::Fail], vec![FakeStep::Fail]]);
    let e2 = env2.engine(env2.store(&p2, |_| {}), vec![w2.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id2 = submit(&e2, &p2, "Add c");
    let t2 = wait_status(&e2, &id2, Status::Failed);
    assert!(t2.outcome.error.as_deref().unwrap().starts_with("agent_failed"), "{:?}", t2.outcome.error);
    assert_eq!(t2.attempts, 0);
    assert_eq!(w2.calls().len(), 3);
}

#[test]
fn cancel_while_waiting_for_another_tasks_merge_returns_promptly() {
    let env = Env::new();
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    let (first, held, gate) = (gd.join("first"), gd.join("held"), gd.join("gate"));
    // a tree with slow.txt passes its first check; the second one (inside the merge) waits at the gate
    let cmd = format!(
        "test ! -f slow.txt || {{ if [ -e '{f}' ]; then touch '{h}'; while [ ! -e '{g}' ]; do sleep 0.02; done; else touch '{f}'; fi; }}",
        f = first.display(),
        h = held.display(),
        g = gate.display()
    );
    let p = env.project(|p| p.check_cmd = Some(cmd));
    let wa = fake_as("fa", vec![vec![write("slow.txt", "s"), done("s")]]);
    let wb = fake_as("fb", vec![vec![write("y.txt", "y"), done("y")]]);
    let e = env.engine(env.store(&p, |s| s.max_parallel = 2), vec![wa, wb, reviewer(2)], Arc::new(RecordingNotifier::default()), None);

    let a = submit_with(&e, &p, "Slow", "fa");
    let start = Instant::now();
    while !held.exists() {
        assert!(start.elapsed() < Duration::from_secs(30), "task A never reached its merge");
        std::thread::sleep(Duration::from_millis(10));
    }
    let b = submit_with(&e, &p, "Quick", "fb");
    wait_status(&e, &b, Status::Merging);
    std::thread::sleep(Duration::from_millis(200));

    let t0 = Instant::now();
    e.cancel(&b).unwrap();
    assert!(t0.elapsed() < Duration::from_secs(5), "cancel took {:?}", t0.elapsed());
    assert_eq!(task(&e, &b).status, Status::Cancelled);

    fs::write(&gate, "").unwrap();
    wait_status(&e, &a, Status::Done);
    let files = g(&env.repo, &["ls-tree", "-r", "--name-only", "main"]);
    assert!(files.contains("slow.txt") && !files.contains("y.txt"), "{files}");
}

/// Notifier with a bug: panics on the "done" notice, after the task is already saved as done.
struct PanicOnDone;
impl Notifier for PanicOnDone {
    fn send(&self, n: &Notice) {
        if n.kind == NoticeKind::Done {
            panic!("notifier bug");
        }
    }
}

#[test]
fn a_panic_after_the_task_is_done_leaves_it_done() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = fake(vec![vec![write("f.txt", "f"), done("f")]]);
    let e = Engine::new(env.store(&p, |_| {}), route_reviewers(vec![w, reviewer(1)]), Arc::new(PanicOnDone), env.d.path().join("wts"));
    e.start();
    let id = submit(&e, &p, "Add f");
    wait_status(&e, &id, Status::Done);
    std::thread::sleep(Duration::from_millis(300));
    let t = task(&e, &id);
    assert_eq!(t.status, Status::Done);
    assert!(t.outcome.error.is_none(), "{:?}", t.outcome.error);
    assert_eq!(env.main_file("f.txt"), "f");
}
