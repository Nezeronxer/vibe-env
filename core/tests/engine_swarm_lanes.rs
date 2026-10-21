//! A lane of a swarm meets what any role of a task meets, and ends the same way: a limit of the env,
//! a CLI that is gone, the budget, silence (the Watcher), a question for the person; and the git of
//! the project is written by one at a time.
mod common;
use common::*;
use nightshift_core::agent::{Adapter, FakeAdapter, FakeStep};
use nightshift_core::engine::{Engine, NewTask};
use nightshift_core::model::*;
use nightshift_core::notify::{NoticeKind, RecordingNotifier};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn until(what: &str, mut f: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while !f() {
        assert!(t0.elapsed() < Duration::from_secs(60), "timeout: {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

const CHECK: &str = r#"grep -q ok feature.txt && echo "3 passed""#;
const CRITIC_OK: &str = r#"{"ok":true,"issues":[],"plan":[]}"#;

fn spec(mode: SwarmMode, n: u8) -> Option<SwarmSpec> {
    Some(SwarmSpec { mode, n })
}

fn go(e: &Engine, p: &Project, text: &str, mode: SwarmMode, n: u8) -> String {
    e.submit(NewTask { project_id: p.id.clone(), prompt: text.into(), swarm: spec(mode, n), ..NewTask::default() }).unwrap()
}

fn compare(rec: &str, ids: &[&str]) -> String {
    let scores: Vec<_> = ids
        .iter()
        .map(|v| serde_json::json!({"variant": v, "correctness": 7, "coverage": 7, "simplicity": 7, "risk": 7, "why": "ok"}))
        .collect();
    serde_json::json!({"scores": scores, "recommend": rec, "advice": "take it"}).to_string()
}

fn project(env: &Env) -> Project {
    env.project(|p| {
        p.check_cmd = Some(CHECK.into());
        p.agent = "fa".into();
    })
}

fn models3(s: &mut Settings) {
    env_models(s, "fa", &["m1", "m2", "m3"], "m1", &[], None);
    s.max_parallel = 3;
}

fn ok_run(m: &str) -> Vec<FakeStep> {
    vec![write("feature.txt", &format!("ok from {m}\n")), done(&format!("{m} built it"))]
}

/// A race env: the preparation (planner, critic, test author), a worker script per model, the reviewer.
fn fake(workers: Vec<(&str, Vec<Vec<FakeStep>>)>, ids: &[&str]) -> FakeAdapter {
    let mut f = FakeAdapter::new(vec![])
        .with_id("fa")
        .with_role(RoleKind::Planner, vec![vec![done(&brief_json("Add it", &["write feature.txt"], false, false, None))]])
        .with_role(RoleKind::Critic, vec![vec![done(CRITIC_OK)]])
        .with_role(RoleKind::TestAuthor, vec![vec![write("t_test.txt", "x\n"), done("tests")]])
        .with_role(RoleKind::Reviewer, vec![vec![done(&compare(ids[0], ids))]]);
    for (m, scripts) in workers {
        f = f.with_variant(RoleKind::Worker, m, scripts);
    }
    f
}

fn has_advice(e: &Engine, id: &str) -> bool {
    e.swarm_view(id).is_some_and(|v| v.advice.is_some())
}

fn worker_models(w: &FakeAdapter) -> Vec<String> {
    calls_of(w, RoleKind::Worker).into_iter().map(|r| r.model.unwrap_or_default()).collect()
}

#[test]
fn a_limit_of_the_env_in_the_middle_of_a_race_pauses_the_whole_task_and_only_the_stopped_variant_goes_again() {
    let env = Env::new();
    let p = project(&env);
    let (clock, now) = manual_clock();
    let w = Arc::new(fake(vec![("m1", vec![vec![FakeStep::Limit], ok_run("m1")]), ("m2", vec![ok_run("m2")]), ("m3", vec![ok_run("m3")])], &["B", "A", "C"]));
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store_v2(&p, models3), vec![w.clone()], n.clone(), Some(clock));
    let t0 = *now.lock().unwrap();
    let id = go(&e, &p, "Add the feature file", SwarmMode::Race, 3);

    let t = wait_status(&e, &id, Status::PausedLimit);
    assert_eq!(t.resume_at.map(|r| (r - t0).num_minutes()), Some(30), "the time of a limit without a reset is half an hour");
    let v = e.swarm_view(&id).unwrap();
    assert_eq!(v.variants.iter().map(|x| x.state.as_str()).collect::<Vec<_>>(), ["running", "passed", "passed"], "B and C finished, A waits for the limit");
    assert_eq!(kinds(&n), vec![NoticeKind::Limit]);
    assert!(matches!(e.limits("fa"), EnvLimits::Known(ws) if ws.iter().any(|x| x.exhausted)));

    advance(&now, 31 * 60);
    until("the advice", || has_advice(&e, &id));
    assert_eq!(worker_models(&w).iter().filter(|m| *m == "m1").count(), 2, "A ran again, nobody else did");
    assert_eq!(worker_models(&w).len(), 4);
    assert!(e.swarm_view(&id).unwrap().variants.iter().all(|x| x.state == "passed"));
}

#[test]
fn when_the_limit_is_known_and_the_env_has_another_model_every_lane_starts_on_it() {
    let env = Env::new();
    let p = project(&env);
    let (clock, now) = manual_clock();
    let at = *now.lock().unwrap();
    let exhausted = vec![window("five_hour", at + chrono::Duration::hours(2), true, at)];
    let mut f = fake(vec![("m3", vec![ok_run("m3"), ok_run("m3")])], &["A", "B"]);
    // the planner is the first to learn that the limit is used up
    f = f.with_role(RoleKind::Planner, vec![vec![FakeStep::Usage(exhausted), done(&brief_json("Add it", &["write feature.txt"], false, false, None))]]);
    let w = Arc::new(f);
    let e = env.engine(env.store_v2(&p, |s| { env_models(s, "fa", &["m1", "m2", "m3"], "m1", &[], Some("m3")); s.max_parallel = 3; }), vec![w.clone()], Arc::new(RecordingNotifier::default()), Some(clock));
    let id = go(&e, &p, "Add the feature file", SwarmMode::Race, 2);
    until("the advice", || has_advice(&e, &id));
    // the variants were meant for m1 and m2, and both went to m3 at once, as a role of a task does
    assert_eq!(worker_models(&w), ["m3", "m3"]);
    assert!(log_texts(&e, &id).iter().filter(|l| l.starts_with("switched_to:")).count() >= 2);
}

#[test]
fn a_cli_that_goes_away_in_the_middle_of_a_race_makes_the_task_wait_for_it_as_for_any_role() {
    let env = Env::new();
    let p = project(&env);
    let (clock, now) = manual_clock();
    let inner = Arc::new(
        fake(vec![("m1", vec![ok_run("m1")]), ("m2", vec![ok_run("m2")])], &["A", "B"])
            // the planner takes its time: the CLI is gone by the time the variants start
            .with_role(RoleKind::Planner, vec![vec![FakeStep::Sleep(500), done(&brief_json("Add it", &["write feature.txt"], false, false, None))]]),
    );
    let knobs = Arc::new(Knobs::new(inner.clone()));
    let e = env.engine(
        env.store_v2(&p, |s| {
            models3(s);
            s.quality = QualityPlan { on: true, spec: false, plan: false, tests_first: false, e2e: false, best_of_n: None };
        }),
        vec![knobs.clone() as Arc<dyn Adapter>],
        Arc::new(RecordingNotifier::default()),
        Some(clock),
    );
    let id = go(&e, &p, "Add the feature file", SwarmMode::Race, 2);
    until("the planner runs", || calls_of(&inner, RoleKind::Planner).len() == 1);
    knobs.installed.store(false, Ordering::SeqCst);
    until("the task waits for its env", || task(&e, &id).outcome.error.as_deref().is_some_and(|x| x.starts_with("env_unavailable")));
    let t = task(&e, &id);
    assert_eq!(t.status, Status::Working, "not failed: it waits, as a task whose planner would find the CLI gone");
    assert!(e.swarm_view(&id).unwrap().variants.iter().all(|v| v.state != "failed"), "{:?}", e.swarm_view(&id).unwrap().variants);
    assert!(calls_of(&inner, RoleKind::Worker).is_empty());

    knobs.installed.store(true, Ordering::SeqCst);
    advance(&now, 61); // it asks again once a minute
    until("the advice", || has_advice(&e, &id));
    assert_eq!(calls_of(&inner, RoleKind::Worker).len(), 2);
}

#[test]
fn the_budget_spent_in_a_lane_is_asked_of_the_person_as_for_a_role_and_the_answer_lets_the_lane_go_on() {
    let env = Env::new();
    let p = project(&env);
    let n = Arc::new(RecordingNotifier::default());
    // A's first run costs more than the task's $5 and leaves the gates red; its second run needs more budget
    let w = Arc::new(fake(
        vec![
            ("m1", vec![vec![FakeStep::Cost(6.0), write("feature.txt", "nope\n"), done("v1")], ok_run("m1")]),
            ("m2", vec![vec![FakeStep::Cost(0.5)].into_iter().chain(ok_run("m2")).collect()]),
        ],
        &["A", "B"],
    ));
    let e = env.engine(env.store_v2(&p, models3), vec![w.clone()], n.clone(), None);
    let id = go(&e, &p, "Add the feature file", SwarmMode::Race, 2);
    let t = wait_status(&e, &id, Status::NeedsHuman);
    assert!(t.question.as_ref().unwrap().text.starts_with("budget_exceeded"), "{:?}", t.question);
    assert_eq!(notices(&n, 1)[0].kind, NoticeKind::Question);
    assert_eq!(worker_models(&w).iter().filter(|m| *m == "m1").count(), 1, "A did not start its second run on money that is not there");

    e.answer(&id, "Add $5").unwrap();
    until("the advice", || has_advice(&e, &id));
    assert_eq!(worker_models(&w).iter().filter(|m| *m == "m1").count(), 2);
    assert_eq!(kinds(&n).iter().filter(|k| **k == NoticeKind::Question).count(), 1, "one call, not one per lane");
}

/// Variant A: a red run, then a run in the same session that stays silent, then what `last` says.
fn silent_a(last: Vec<FakeStep>) -> Vec<Vec<FakeStep>> {
    vec![vec![write("feature.txt", "nope\n"), done("v1")], vec![FakeStep::Sleep(60_000)], last]
}

#[test]
fn a_lane_that_goes_silent_is_nudged_in_its_session_after_a_diagnosis_by_a_run_of_the_watcher() {
    let env = Env::new();
    let p = project(&env);
    let (clock, now) = manual_clock();
    let t0 = *now.lock().unwrap();
    let w = Arc::new(
        fake(vec![("m1", silent_a(ok_run("m1"))), ("m2", vec![ok_run("m2")])], &["A", "B"])
            .with_role(RoleKind::Watcher, vec![vec![done("A is waiting for an answer nobody gives.")]]),
    );
    let e = env.engine(env.store_v2(&p, models3), vec![w.clone()], Arc::new(RecordingNotifier::default()), Some(clock));
    let id = go(&e, &p, "Add the feature file", SwarmMode::Race, 2);
    let m1 = || calls_of(&w, RoleKind::Worker).into_iter().filter(|r| r.model.as_deref() == Some("m1")).collect::<Vec<_>>();
    until("A is silent in its second run", || m1().len() == 2);
    advance(&now, 15 * 60 + 1);
    until("the advice", || has_advice(&e, &id));

    let runs = m1();
    assert_eq!(runs.len(), 3);
    assert!(runs[1].resume.is_some() && runs[2].resume == runs[1].resume, "the nudge goes to the same session: {:?}", runs.iter().map(|r| r.resume.clone()).collect::<Vec<_>>());
    assert!(runs[2].prompt.contains("what is blocking you"));
    assert_eq!(calls_of(&w, RoleKind::Watcher).len(), 1, "one diagnosis, by a run of its own");
    let log = e.store().lock().unwrap().read_log(&id, 5000);
    let line = log.iter().find(|l| l.text.starts_with("watcher: A is waiting")).expect("the diagnosis is in the feed");
    assert_eq!(line.role, Some(RoleKind::Watcher));
    assert!(line.at - t0 >= chrono::Duration::minutes(15), "not before the silence was 15 minutes long");
    assert!(e.swarm_view(&id).unwrap().variants.iter().all(|v| v.state == "passed"));
}

#[test]
fn five_more_minutes_of_silence_start_a_new_session_in_the_lane_and_count_an_attempt() {
    for (attempts, ends_failed) in [(3, false), (2, true)] {
        let (clock, now) = manual_clock();
        let w = Arc::new(
            fake(vec![("m1", silent_a(vec![FakeStep::Sleep(60_000)]).into_iter().chain([ok_run("m1")]).collect()), ("m2", vec![ok_run("m2")])], &["B", "A"])
                .with_role(RoleKind::Watcher, vec![vec![done("Stuck.")]]),
        );
        let env2 = Env::new();
        let p2 = project(&env2);
        let e = env2.engine(env2.store_v2(&p2, |s| { models3(s); s.attempts = attempts; }), vec![w.clone()], Arc::new(RecordingNotifier::default()), Some(clock));
        let id = go(&e, &p2, "Add the feature file", SwarmMode::Race, 2);
        let m1 = || calls_of(&w, RoleKind::Worker).into_iter().filter(|r| r.model.as_deref() == Some("m1")).collect::<Vec<_>>();
        until("silent", || m1().len() == 2);
        advance(&now, 15 * 60 + 1);
        until("the nudged run", || m1().len() == 3);
        advance(&now, 5 * 60 + 1);
        until("the advice", || has_advice(&e, &id));
        let v = e.swarm_view(&id).unwrap();
        let a = v.variants.iter().find(|x| x.id == "A").unwrap();
        if ends_failed {
            // the red run took one attempt, the restart took the second: none left
            assert_eq!((a.state.as_str(), a.why_failed.as_deref().map(|w| w.starts_with("stalled"))), ("failed", Some(true)));
            assert_eq!(m1().len(), 3);
        } else {
            assert_eq!(a.state, "passed");
            assert_eq!(m1().len(), 4);
            assert_eq!(m1()[3].resume, None, "a new session");
        }
    }
}

const ASK: &str = "Which logo?\nNEEDS_HUMAN: Which logo?\nOPTIONS: big | small";

#[test]
fn a_question_of_a_variant_reaches_the_person_once_naming_the_variant_and_the_answer_goes_back_to_it() {
    let env = Env::new();
    let p = project(&env);
    let n = Arc::new(RecordingNotifier::default());
    let w = Arc::new(fake(vec![("m1", vec![vec![done(ASK)], ok_run("m1")]), ("m2", vec![ok_run("m2")])], &["A", "B"]));
    let e = env.engine(env.store_v2(&p, models3), vec![w.clone()], n.clone(), None);
    let id = go(&e, &p, "Add the feature file", SwarmMode::Race, 2);

    let t = wait_status(&e, &id, Status::NeedsHuman);
    let q = t.question.unwrap();
    assert_eq!((q.text.as_str(), q.options.clone(), q.kind), ("Variant A: Which logo?", vec!["big".to_string(), "small".to_string()], QuestionKind::Agent));
    let ns = notices(&n, 1);
    assert_eq!((ns[0].kind.clone(), ns[0].body.as_str()), (NoticeKind::Question, "Variant A: Which logo?"));
    let v = e.swarm_view(&id).unwrap();
    assert_eq!((v.variants[0].state.as_str(), v.variants[1].state.as_str()), ("running", "passed"), "B went on while A waits for the person");

    e.answer(&id, "small").unwrap();
    until("the advice", || has_advice(&e, &id));
    let a: Vec<_> = calls_of(&w, RoleKind::Worker).into_iter().filter(|r| r.model.as_deref() == Some("m1")).collect();
    assert_eq!(a.len(), 2);
    assert!(a[1].resume.is_some() && a[1].resume.as_deref().is_some_and(|s| s.starts_with("fake-session-")), "{:?}", a[1].resume);
    assert_eq!(a[1].prompt, "small", "the answer itself, in the session of A");
    assert_eq!(kinds(&n).iter().filter(|k| **k == NoticeKind::Question).count(), 1, "one call for one question");
    assert!(e.swarm_view(&id).unwrap().variants.iter().all(|x| x.state == "passed"));
}

#[test]
fn a_question_of_a_part_of_a_crew_names_the_part_and_the_answer_goes_back_to_it() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let split = serde_json::json!({"parts":[{"id":"p1","title":"one","files":["one.txt"]},{"id":"p2","title":"two","files":["two.txt"]}]}).to_string();
    let n = Arc::new(RecordingNotifier::default());
    let w = Arc::new(
        FakeAdapter::new(vec![])
            .with_id("fa")
            .with_role(RoleKind::Coordinator, vec![vec![done(&split)]])
            .with_role(RoleKind::Reviewer, vec![approve()])
            .with_variant(RoleKind::Worker, "A", vec![vec![done(ASK)], vec![write("one.txt", "1\n"), done("did one")]])
            .with_variant(RoleKind::Worker, "B", vec![vec![write("two.txt", "2\n"), done("did two")]]),
    );
    let e = env.engine(env.store(&p, models3), vec![w.clone()], n.clone(), None);
    let id = go(&e, &p, "Build both", SwarmMode::Crew, 2);
    let t = wait_status(&e, &id, Status::NeedsHuman);
    assert_eq!(t.question.unwrap().text, "Part A: Which logo?");
    assert_eq!(notices(&n, 1)[0].kind, NoticeKind::Question);
    e.answer(&id, "small").unwrap();
    wait_status(&e, &id, Status::Done);
    let a: Vec<_> = calls_of(&w, RoleKind::Worker).into_iter().filter(|r| r.cwd.ends_with("A")).collect();
    assert_eq!(a.len(), 2);
    assert!(a[1].resume.is_some() && a[1].prompt == "small");
    assert_eq!((env.main_file("one.txt").as_str(), env.main_file("two.txt").as_str()), ("1", "2"));
    assert_eq!(kinds(&n).iter().filter(|k| **k == NoticeKind::Question).count(), 1);
}

#[test]
fn a_question_of_a_member_of_the_council_names_the_member_and_the_answer_goes_back_to_it() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let propose = |t: &str| vec![done(&serde_json::json!({ "approach": t }).to_string())];
    let vote = vec![done(r#"{"vote":"A","why":"fine"}"#)];
    let verdict = vec![done(r#"{"verdict":"approve","issues":[]}"#)];
    let n = Arc::new(RecordingNotifier::default());
    let w = Arc::new(
        FakeAdapter::new(vec![])
            .with_id("fa")
            .with_variant(RoleKind::Council, "m1", vec![propose("Approach of A"), vote.clone(), verdict.clone()])
            .with_variant(RoleKind::Council, "m2", vec![vec![done(ASK)], propose("Approach of B"), vote, verdict])
            .with_role(RoleKind::Worker, vec![vec![write("feature.txt", "built\n"), done("Built it")]]),
    );
    let e = env.engine(env.store(&p, models3), vec![w.clone()], n.clone(), None);
    let id = go(&e, &p, "Add the feature file", SwarmMode::Council, 2);
    let t = wait_status(&e, &id, Status::NeedsHuman);
    assert_eq!(t.question.unwrap().text, "Member B: Which logo?");
    assert_eq!(notices(&n, 1)[0].kind, NoticeKind::Question);
    e.answer(&id, "small").unwrap();
    wait_status(&e, &id, Status::Done);
    let b: Vec<_> = calls_of(&w, RoleKind::Council).into_iter().filter(|r| r.model.as_deref() == Some("m2")).collect();
    assert!(b[1].resume.is_some() && b[1].prompt == "small", "{:?} {:?}", b[1].resume, b[1].prompt);
    let c = e.swarm_view(&id).unwrap().council.unwrap();
    assert_eq!(c.proposals.len(), 2, "B proposed after the answer");
    assert_eq!(kinds(&n).iter().filter(|k| **k == NoticeKind::Question).count(), 1);
}

/// A hook that stands in the way of every change of a ref of the project: it says when a transaction
/// opens and closes (by the pid of the git that runs it) and holds it open for a moment. Two of them
/// open at once are two git commands writing into the same `.git` at once.
fn watch_refs(env: &Env) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let log = env.d.path().join("ref-transactions");
    let hook = env.repo.join(".git").join("hooks").join("reference-transaction");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(
        &hook,
        format!("#!/bin/sh\ncase \"$1\" in\n  prepared) echo \"s $PPID $(ps -o command= -p $PPID)\" >> '{0}'; sleep 0.04;;\n  committed|aborted) echo \"e $PPID\" >> '{0}';;\nesac\nexit 0\n", log.display()),
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    log
}

/// Panics when two transactions were open at once; returns how many there were.
fn assert_never_overlapped(log: &std::path::Path) -> usize {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let mut open: Vec<String> = vec![];
    let mut seen = 0;
    for l in text.lines() {
        let (kind, rest) = l.split_once(' ').unwrap();
        let pid = rest.split(' ').next().unwrap();
        if kind == "s" {
            assert!(open.iter().all(|o| o == pid), "git {rest} began changing refs while git {open:?} was still at it; the log: {text}");
            open.push(pid.to_string());
            seen += 1;
        } else {
            open.retain(|o| o != pid);
        }
    }
    seen
}

#[test]
fn copies_of_a_swarm_made_and_merged_at_once_with_other_tasks_never_write_into_the_git_of_the_project_together() {
    let env = Env::new();
    let refs = watch_refs(&env);
    let p = env.project(|p| p.agent = "fa".into());
    let split = serde_json::json!({"parts": (1..=5).map(|i| serde_json::json!({"id": format!("p{i}"), "title": format!("part {i}"), "files": [format!("part{i}.txt")]})).collect::<Vec<_>>()}).to_string();
    let mut fa = FakeAdapter::new(vec![])
        .with_id("fa")
        .with_role(RoleKind::Coordinator, vec![vec![done(&split)]])
        .with_role(RoleKind::Reviewer, vec![approve()]);
    for (i, l) in ["A", "B", "C", "D", "E"].iter().enumerate() {
        fa = fa.with_variant(RoleKind::Worker, l, vec![vec![write(&format!("part{}.txt", i + 1), "x\n"), done("built")]]);
    }
    let e = env.engine(
        env.store(&p, |s| {
            env_models(s, "fa", &["m1"], "m1", &[], None);
            s.max_parallel = 12;
        }),
        vec![Arc::new(fa), Arc::new(pipeline_fake_n("fb", 4))],
        Arc::new(RecordingNotifier::default()),
        None,
    );
    // five parts are made side by side while four plain tasks make their copies and merge
    let crew = go(&e, &p, "Build five parts", SwarmMode::Crew, 5);
    let plain: Vec<String> = (0..4).map(|k| submit_with(&e, &p, &format!("Plain task {k}"), "fb")).collect();
    for id in plain.iter().chain([&crew]) {
        let t = wait_status(&e, id, Status::Done);
        assert!(t.outcome.error.is_none());
    }
    let seen = assert_never_overlapped(&refs);
    assert!(seen > 30, "the hook saw the work of the whole run: {seen} transactions");
    assert_eq!(env.main_count(), 1 + 5, "every task landed, one commit each");
}

#[test]
fn a_copy_that_cannot_be_removed_is_a_line_in_the_feed_not_silence() {
    use std::os::unix::fs::PermissionsExt;
    let env = Env::new();
    let p = project(&env);
    let w = Arc::new(fake(vec![("m1", vec![ok_run("m1")]), ("m2", vec![ok_run("m2")])], &["A", "B"]));
    let e = env.engine(env.store_v2(&p, models3), vec![w], Arc::new(RecordingNotifier::default()), None);
    let id = go(&e, &p, "Add the feature file", SwarmMode::Race, 2);
    until("the advice", || has_advice(&e, &id));
    // the folder that holds the copies cannot be written to: nothing in it can be taken away
    let root = env.d.path().join("wts").join(format!("{id}-swarm"));
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o555)).unwrap();
    e.cancel(&id).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
    let log = log_texts(&e, &id);
    assert!(log.iter().any(|l| l.starts_with("copy_not_removed: ")), "{log:?}");
}
