//! Engine v2: the question "push / PR?" with its four answers in both modes, when it is asked,
//! and the retry of a push that was refused.
mod common;
use common::*;
use nightshift_core::model::*;
use nightshift_core::notify::{NoticeKind, RecordingNotifier};
use std::path::PathBuf;
use std::sync::Arc;

struct Setup {
    env: Env,
    e: Arc<nightshift_core::engine::Engine>,
    p: Project,
    bare: PathBuf,
    gh: PathBuf,
    n: Arc<RecordingNotifier>,
}

fn setup(finish: Finish, project_asks: bool, env_ask: bool) -> Setup {
    let gh = fake_gh();
    let env = Env::new();
    let bare = with_origin(&env);
    let p = env.project(|p| {
        p.finish = finish.clone();
        p.ask_outward = project_asks;
    });
    let n = Arc::new(RecordingNotifier::default());
    let w = fake(vec![vec![write("f.txt", "f"), done("f")]]);
    let store = env.store(&p, |s| {
        s.envs.insert("fake".into(), EnvSettings { ask_push: env_ask, ask_pr: env_ask, ..EnvSettings::default() });
    });
    let e = env.engine(store, vec![w, reviewer(1)], n.clone(), None);
    Setup { env, e, p, bare, gh, n }
}

fn on_origin(bare: &std::path::Path, branch: &str) -> bool {
    std::process::Command::new("git").current_dir(bare).args(["rev-parse", "--verify", "-q", &format!("refs/heads/{branch}")]).status().unwrap().success()
}

#[test]
fn pr_project_push_and_pr_opens_the_pull_request() {
    let s = setup(Finish::Pr, true, true);
    let id = submit(&s.e, &s.p, "Add f");
    let t = wait_status(&s.e, &id, Status::AwaitingOutward);
    assert_eq!(t.question.as_ref().unwrap().options, vec!["Push and PR", "Push only", "Skip"]);
    assert_eq!(notices(&s.n, 1).iter().find(|x| x.kind == NoticeKind::Outward).unwrap().options.len(), 3);

    s.e.answer_outward(&id, OutwardChoice::PushAndPr).unwrap();
    let t = wait_status(&s.e, &id, Status::Done);
    assert_eq!(t.outcome.pr_url.as_deref(), Some("https://github.com/o/r/pull/9"));
    assert!(on_origin(&s.bare, &t.branch));
    assert_eq!(gh_calls_for(&s.gh, &t.branch), 1);
}

#[test]
fn pr_project_push_only_puts_the_branch_on_origin_without_a_pr_and_is_done_with_a_note() {
    let s = setup(Finish::Pr, true, true);
    let id = submit(&s.e, &s.p, "Add f");
    let t = wait_status(&s.e, &id, Status::AwaitingOutward);

    s.e.answer_outward(&id, OutwardChoice::PushOnly).unwrap();
    let t2 = wait_status(&s.e, &id, Status::Done);
    assert!(on_origin(&s.bare, &t.branch), "the branch is on origin");
    assert_eq!(gh_calls_for(&s.gh, &t.branch), 0, "no PR was opened");
    assert!(t2.outcome.pr_url.is_none());
    assert_eq!(t2.outcome.error.as_deref(), Some("pr_not_opened"));
}

#[test]
fn pr_project_no_keeps_the_work_local_done_not_pushed() {
    let s = setup(Finish::Pr, true, true);
    let id = submit(&s.e, &s.p, "Add f");
    let t = wait_status(&s.e, &id, Status::AwaitingOutward);

    s.e.answer_outward(&id, OutwardChoice::No).unwrap();
    let t2 = wait_status(&s.e, &id, Status::DoneNotPushed);
    assert!(!on_origin(&s.bare, &t.branch));
    assert_eq!(t2.outcome.error.as_deref(), Some("push_skipped"));
    assert_eq!(gh_calls_for(&s.gh, &t.branch), 0);
}

#[test]
fn push_project_push_and_no_and_the_answers_that_do_not_fit_are_refused() {
    let s = setup(Finish::Push, true, true);
    let id = submit(&s.e, &s.p, "Add f");
    let t = wait_status(&s.e, &id, Status::AwaitingOutward);
    assert_eq!(t.question.as_ref().unwrap().options, vec!["Push", "Skip"]);
    for wrong in [OutwardChoice::PushAndPr, OutwardChoice::PushOnly] {
        assert_eq!(s.e.answer_outward(&id, wrong).unwrap_err(), "invalid_choice");
    }
    assert_eq!(task(&s.e, &id).status, Status::AwaitingOutward, "still waiting");
    let origin_before = g(&s.bare, &["rev-parse", "main"]);

    s.e.answer_outward(&id, OutwardChoice::Push).unwrap();
    wait_status(&s.e, &id, Status::Done);
    assert_ne!(g(&s.bare, &["rev-parse", "main"]), origin_before);
    assert_eq!(g(&s.bare, &["rev-parse", "main"]), g(&s.env.repo, &["rev-parse", "main"]));

    let s2 = setup(Finish::Push, true, true);
    let id2 = submit(&s2.e, &s2.p, "Add f");
    wait_status(&s2.e, &id2, Status::AwaitingOutward);
    let before = g(&s2.bare, &["rev-parse", "main"]);
    s2.e.answer_outward(&id2, OutwardChoice::No).unwrap();
    let t = wait_status(&s2.e, &id2, Status::DoneNotPushed);
    assert_eq!(g(&s2.bare, &["rev-parse", "main"]), before, "nothing left the machine");
    assert!(t.outcome.commit.is_some(), "the commit is on local main");
}

#[test]
fn the_question_is_asked_when_the_project_asks_or_the_env_asks_and_not_when_neither_does() {
    // project says no, env says yes: asked
    let s = setup(Finish::Push, false, true);
    let id = submit(&s.e, &s.p, "Add f");
    wait_status(&s.e, &id, Status::AwaitingOutward);

    // project says yes, env says no: asked
    let s = setup(Finish::Push, true, false);
    let id = submit(&s.e, &s.p, "Add f");
    wait_status(&s.e, &id, Status::AwaitingOutward);

    // neither: it pushes by itself
    let s = setup(Finish::Push, false, false);
    let id = submit(&s.e, &s.p, "Add f");
    wait_status(&s.e, &id, Status::Done);
    assert_eq!(g(&s.bare, &["rev-parse", "main"]), g(&s.env.repo, &["rev-parse", "main"]));

    // the same for a PR: the env's "ask before a PR" alone is enough
    let s = setup(Finish::Pr, false, true);
    let id = submit(&s.e, &s.p, "Add f");
    wait_status(&s.e, &id, Status::AwaitingOutward);
    assert_eq!(gh_calls_for(&s.gh, &task(&s.e, &id).branch), 0);
}

#[test]
fn a_refused_push_of_a_pr_task_is_repeated_by_retry_which_sends_the_branch_and_opens_the_pr() {
    let gh = fake_gh();
    let env = Env::new();
    let p = env.project(|p| {
        p.finish = Finish::Pr;
        p.ask_outward = false;
    });
    let w = fake(vec![vec![write("f.txt", "f"), done("f")]]);
    // no origin yet: the branch cannot be pushed
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add f");
    let t = wait_status(&e, &id, Status::DoneNotPushed);
    assert!(t.outcome.error.is_some() && t.outcome.pr_url.is_none());

    let bare = with_origin(&env);
    e.retry(&id).unwrap();
    let t = wait_status(&e, &id, Status::Done);
    assert_eq!(t.outcome.pr_url.as_deref(), Some("https://github.com/o/r/pull/9"));
    assert!(on_origin(&bare, &t.branch));
    assert_eq!(gh_calls_for(&gh, &t.branch), 1);
}
