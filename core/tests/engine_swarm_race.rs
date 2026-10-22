//! The race (Соревнование, G11.1): one preparation, N executors in copies of their own with models of
//! the env in turn, the gates of each, the reviewer's marks, the pick of the person or of the timer.
mod common;
use common::*;
use nightshift_core::agent::{FakeAdapter, FakeStep};
use nightshift_core::engine::{Engine, NewTask};
use nightshift_core::model::*;
use nightshift_core::notify::{NoticeKind, RecordingNotifier};
use std::sync::Arc;
use std::time::{Duration, Instant};


/// An env that does one small task end to end, for a task that only tells that the scheduler went round.
pub fn barrier_env() -> Arc<FakeAdapter> {
    Arc::new(
        FakeAdapter::new(vec![])
            .with_id("fb")
            .with_role(RoleKind::Planner, vec![vec![done(&brief_json("Barrier", &["write barrier.txt"], false, false, None))]])
            .with_role(RoleKind::Critic, vec![vec![done(CRITIC_OK)]])
            .with_role(RoleKind::TestAuthor, vec![vec![write("b_test.txt", "x\n"), done("tests")]])
            .with_role(RoleKind::Worker, vec![vec![write("barrier.txt", "b\n"), done("done")]])
            .with_role(RoleKind::Reviewer, vec![approve()]),
    )
}

pub fn race_spec(n: u8) -> Option<SwarmSpec> {
    Some(SwarmSpec { mode: SwarmMode::Race, n })
}

pub fn submit_swarm(e: &Engine, p: &Project, text: &str, spec: Option<SwarmSpec>) -> String {
    e.submit(NewTask { project_id: p.id.clone(), prompt: text.into(), swarm: spec, ..NewTask::default() }).unwrap()
}

/// `compare` JSON of the reviewer: marks of A, B, C and the recommendation.
pub fn compare(rec: &str, marks: &[(&str, [u8; 4])]) -> String {
    let scores: Vec<_> = marks
        .iter()
        .map(|(v, m)| serde_json::json!({"variant": v, "correctness": m[0], "coverage": m[1], "simplicity": m[2], "risk": m[3], "why": format!("why {v}")}))
        .collect();
    serde_json::json!({"scores": scores, "recommend": rec, "advice": format!("{rec} is the better one")}).to_string()
}

/// The env of the tests of the race: three models of its own, the pipeline of the preparation, a worker per model.
pub fn race_fake(id: &str, worker: impl Fn(&str) -> Vec<Vec<FakeStep>>, reviewer: Vec<Vec<FakeStep>>) -> FakeAdapter {
    let mut f = FakeAdapter::new(vec![])
        .with_id(id)
        .with_role(RoleKind::Planner, vec![vec![done(&brief_json("Add the feature file", &["write feature.txt"], false, false, None))]])
        .with_role(RoleKind::Critic, vec![vec![done(CRITIC_OK)]])
        .with_role(RoleKind::TestAuthor, vec![vec![write("t_test.txt", "expect feature\n"), done("tests written")]])
        .with_role(RoleKind::Reviewer, reviewer);
    for m in ["m1", "m2", "m3"] {
        f = f.with_variant(RoleKind::Worker, m, worker(m));
    }
    f
}
const CRITIC_OK: &str = r#"{"ok":true,"issues":[],"plan":[]}"#;

fn settings_for(env: &str, max_parallel: u32) -> impl FnOnce(&mut Settings) + '_ {
    move |s: &mut Settings| {
        env_models(s, env, &["m1", "m2", "m3"], "m1", &[], None);
        s.max_parallel = max_parallel;
    }
}

const CHECK: &str = r#"grep -q ok feature.txt && echo "3 passed""#;

#[test]
fn a_race_prepares_once_runs_three_variants_in_copies_of_their_own_and_the_picked_one_lands_in_main() {
    let env = Env::new();
    let p = env.project(|p| {
        p.check_cmd = Some(CHECK.into());
        p.agent = "fa".into();
    });
    let w = Arc::new(race_fake(
        "fa",
        |m| vec![vec![write("feature.txt", &format!("ok from {m}\n")), done(&format!("Built it with {m}"))]],
        vec![vec![done(&compare("B", &[("A", [6, 6, 6, 6]), ("B", [9, 8, 8, 9]), ("C", [5, 5, 5, 5])]))]],
    ));
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store_v2(&p, settings_for("fa", 3)), vec![w.clone()], n.clone(), None);

    let id = submit_swarm(&e, &p, "Add the feature file", race_spec(3));
    until("the advice of the reviewer", || e.swarm_view(&id).is_some_and(|v| v.advice.is_some()));

    // the preparation is one: planner, critic and test author ran once each
    for role in [RoleKind::Planner, RoleKind::Critic, RoleKind::TestAuthor] {
        assert_eq!(calls_of(&w, role).len(), 1, "{role:?}");
    }
    // three executors, each on its own model, each in a copy of its own
    let workers = calls_of(&w, RoleKind::Worker);
    let mut models: Vec<_> = workers.iter().map(|r| r.model.clone().unwrap()).collect();
    models.sort();
    assert_eq!(models, ["m1", "m2", "m3"]);
    let mut dirs: Vec<_> = workers.iter().map(|r| r.cwd.clone()).collect();
    dirs.sort();
    dirs.dedup();
    assert_eq!(dirs.len(), 3);
    assert!(workers.iter().all(|r| !r.read_only));
    // the gates ran in each copy: 3 of 3 tests green, all passed
    let view = e.swarm_view(&id).unwrap();
    assert_eq!(view.variants.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["A", "B", "C"]);
    for v in &view.variants {
        assert_eq!(v.state, "passed");
        assert_eq!(v.gate.as_ref().map(|g| (g.passed, g.total)), Some((3, 3)));
        assert!(v.files.iter().any(|f| f.path == "feature.txt"), "{:?}", v.files);
    }
    // the reviewer marked the four criteria and advised B
    let b = view.variants.iter().find(|v| v.id == "B").unwrap();
    assert_eq!(b.criteria.iter().map(|c| (c.key.as_str(), c.value)).collect::<Vec<_>>(), [("correctness", 9), ("coverage", 8), ("simplicity", 8), ("risk", 9)]);
    assert_eq!(view.advice.as_ref().unwrap().variant, "B");
    assert_eq!(task(&e, &id).status, Status::Reviewing, "waits for the pick");
    assert_eq!(env.main_count(), 1, "nothing landed yet");

    // the person takes A, which the reviewer did not advise
    e.pick_variant(&id, "A").unwrap();
    let t = wait_status(&e, &id, Status::Done);
    assert_eq!(env.main_file("feature.txt").trim(), format!("ok from {}", view.variants[0].model_id(&workers)));
    assert_eq!(env.main_count(), 2, "one commit for the task");
    assert!(env.main_file("t_test.txt").contains("expect feature"), "the common preparation is in it");
    assert_eq!(e.swarm_view(&id).unwrap().picked.as_deref(), Some("A"));
    assert_eq!(t.outcome.review.as_ref().map(|r| r.approved), Some(true));
    // the advice was silent: no call of the person at all
    assert!(!kinds_of(&n).contains(&NoticeKind::Question), "{:?}", kinds_of(&n));
}

trait ModelId {
    fn model_id(&self, workers: &[nightshift_core::agent::Run]) -> String;
}
impl ModelId for nightshift_core::swarm::view::VariantView {
    /// The model the variant ran on, found by the copy it ran in (the copy of A is named A).
    fn model_id(&self, workers: &[nightshift_core::agent::Run]) -> String {
        workers.iter().find(|r| r.cwd.ends_with(&self.id)).and_then(|r| r.model.clone()).unwrap()
    }
}

#[test]
fn without_an_answer_the_recommended_variant_is_taken_after_autopick_minutes_and_nobody_is_called() {
    let env = Env::new();
    let p = env.project(|p| {
        p.check_cmd = Some(CHECK.into());
        p.agent = "fa".into();
    });
    let (clock, now) = manual_clock();
    let w = Arc::new(race_fake(
        "fa",
        |m| vec![vec![write("feature.txt", &format!("ok from {m}\n")), done("built")]],
        vec![vec![done(&compare("B", &[("A", [6, 6, 6, 6]), ("B", [9, 8, 8, 9]), ("C", [5, 5, 5, 5])]))]],
    ));
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store_v2(&p, settings_for("fa", 3)), vec![w, barrier_env()], n.clone(), Some(clock));
    // another project, whose checks are none: the task that tells the scheduler went round
    let pb = env.project(|p| p.agent = "fb".into());
    e.store().lock().unwrap().upsert_project(pb.clone()).unwrap();
    let id = submit_swarm(&e, &p, "Add the feature file", race_spec(3));
    until("the advice", || e.swarm_view(&id).is_some_and(|v| v.advice.is_some()));

    // the timer says when: autopick_min (10) after the advice
    let at = chrono::DateTime::parse_from_rfc3339(&e.swarm_view(&id).unwrap().advice.unwrap().autopick_at).unwrap();
    assert_eq!((at.with_timezone(&chrono::Local) - *now.lock().unwrap()).num_minutes(), 10);
    advance(&now, 9 * 60);
    // the scheduler has looked at the clock once more when a task that came after the race is done:
    // it goes through the tasks in order in each round, the due race would have been taken first
    let barrier = submit_with(&e, &pb, "A task in another env, to know the scheduler went round", "fb");
    wait_status(&e, &barrier, Status::Done);
    assert_eq!(task(&e, &id).status, Status::Reviewing, "9 minutes: still waiting");
    advance(&now, 61);
    wait_status(&e, &id, Status::Done);

    let v = e.swarm_view(&id).unwrap();
    assert_eq!((v.picked.as_deref(), v.picked_by.as_deref()), (Some("B"), Some("timer")));
    assert_eq!(env.main_file("feature.txt").trim(), "ok from m2", "B ran on the second model of the env");
    assert!(!kinds_of(&n).contains(&NoticeKind::Question), "{:?}", kinds_of(&n));
}

/// A project whose check is green only when feature.txt says `ok`; it prints what results.txt holds.
const COUNTING_CHECK: &str = "cat results.txt; grep -q ok feature.txt";

#[test]
fn one_failed_variant_does_not_stop_the_others_and_says_why() {
    let env = Env::new();
    let p = env.project(|p| {
        p.check_cmd = Some(CHECK.into());
        p.agent = "fa".into();
    });
    let w = Arc::new(race_fake(
        "fa",
        |m| {
            if m == "m3" {
                // C never gets the file right, in both of its two attempts
                vec![vec![write("feature.txt", "nope\n"), done("tried")], vec![write("feature.txt", "nope\n"), done("tried again")]]
            } else {
                vec![vec![write("feature.txt", &format!("ok from {m}\n")), done("built")]]
            }
        },
        vec![vec![done(&compare("A", &[("A", [9, 9, 9, 9]), ("B", [5, 5, 5, 5])]))]],
    ));
    let e = env.engine(env.store_v2(&p, |s| { settings_for("fa", 3)(s); s.attempts = 2; }), vec![w.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = submit_swarm(&e, &p, "Add the feature file", race_spec(3));
    until("the advice", || e.swarm_view(&id).is_some_and(|v| v.advice.is_some()));

    let v = e.swarm_view(&id).unwrap();
    let state = |l: &str| v.variants.iter().find(|x| x.id == l).unwrap().clone();
    assert_eq!((state("A").state.as_str(), state("B").state.as_str(), state("C").state.as_str()), ("passed", "passed", "failed"));
    assert_eq!(state("C").why_failed.as_deref(), Some("tests_failed"));
    assert_eq!(state("C").gate.as_ref().map(|g| g.passed), Some(0));
    assert!(has_log(&e, &id, "variant_failed: C: tests_failed"));
    // the reviewer was shown the two that passed, not the failed one
    let shown = &calls_of(&w, RoleKind::Reviewer)[0].prompt;
    assert!(shown.contains("VARIANT_A") && shown.contains("VARIANT_B") && !shown.contains("VARIANT_C"));
    // a variant that failed cannot be taken
    assert_eq!(e.pick_variant(&id, "C").unwrap_err(), "invalid_variant");
    e.pick_variant(&id, "B").unwrap();
    wait_status(&e, &id, Status::Done);
    assert_eq!(env.main_file("feature.txt").trim(), "ok from m2");
}

#[test]
fn when_every_variant_fails_the_task_fails_with_the_best_attempt_and_resume_goes_on_from_it() {
    let env = Env::new();
    let p = env.project(|p| {
        p.check_cmd = Some(COUNTING_CHECK.into());
        p.agent = "fa".into();
    });
    let bad = |m: &str, counts: &str| {
        vec![vec![write("feature.txt", "nope\n"), write("results.txt", counts), done(&format!("{m} tried"))]]
    };
    let mut f = FakeAdapter::new(vec![])
        .with_id("fa")
        .with_role(RoleKind::Planner, vec![vec![done(&brief_json("Add it", &["write"], false, false, None))]])
        .with_role(RoleKind::Critic, vec![vec![done(CRITIC_OK)]])
        .with_role(RoleKind::TestAuthor, vec![vec![write("t_test.txt", "x\n"), done("tests")]])
        .with_role(RoleKind::Reviewer, vec![approve()]);
    // by hand: A 1 green, B 4 green, C 2 green: B is the best attempt
    f = f.with_variant(RoleKind::Worker, "m1", bad("A", "1 passed, 4 failed\n"));
    f = f.with_variant(RoleKind::Worker, "m2", bad("B", "4 passed, 1 failed\n"));
    // C's queue also holds the run after the lift: it is the model the person chooses
    let mut c = bad("C", "2 passed, 3 failed\n");
    c.push(vec![write("feature.txt", "ok from the lift\n"), done("fixed it")]);
    f = f.with_variant(RoleKind::Worker, "m3", c);
    let w = Arc::new(f);
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store_v2(&p, |s| { settings_for("fa", 3)(s); s.attempts = 1; }), vec![w.clone()], n.clone(), None);
    let id = submit_swarm(&e, &p, "Add the feature file", race_spec(3));

    let t = wait_status(&e, &id, Status::Failed);
    assert_eq!(t.outcome.error.as_deref(), Some("swarm_failed: best attempt B, 4 of 5 tests green"));
    assert_eq!(kinds(&n), vec![NoticeKind::Failed], "one call: «Не вышло»");
    assert_eq!(calls_of(&w, RoleKind::Reviewer).len(), 0, "nothing to compare");
    // the copy of the task holds the best attempt
    assert_eq!(std::fs::read_to_string(t.worktree.as_ref().unwrap().join("results.txt")).unwrap(), "4 passed, 1 failed\n");

    // the lift works with that attempt: a stronger model, the same copy
    e.resume_failed(&id, Some("the file must say ok".into()), Some(ModelChoice { family: "m3".into(), version: None })).unwrap();
    wait_status(&e, &id, Status::Done);
    assert_eq!(env.main_file("feature.txt").trim(), "ok from the lift");
    assert_eq!(env.main_file("results.txt"), "4 passed, 1 failed", "it went on from the best attempt, not from nothing");
}

#[test]
fn variants_wait_for_seats_when_the_env_has_fewer_than_n_and_a_swarm_of_one_or_six_is_refused() {
    let env = Env::new();
    let p = env.project(|p| {
        p.check_cmd = Some(CHECK.into());
        p.agent = "fa".into();
    });
    // the executors wait at a gate: two running and the third waiting for a seat holds until the test has seen it
    let w = HoldRole::wrap(
        race_fake(
            "fa",
            |m| vec![vec![write("feature.txt", &format!("ok from {m}\n")), done("built")]],
            vec![vec![done(&compare("A", &[("A", [9, 9, 9, 9]), ("B", [5, 5, 5, 5]), ("C", [4, 4, 4, 4])]))]],
        ),
        RoleKind::Worker,
        env.d.path(),
    );
    // two seats for a swarm of three
    let e = env.engine(env.store_v2(&p, settings_for("fa", 2)), vec![w.clone()], Arc::new(RecordingNotifier::default()), None);
    for n in [1, 6] {
        let err = e.submit(NewTask { project_id: p.id.clone(), prompt: "x".into(), swarm: race_spec(n), ..NewTask::default() }).unwrap_err();
        assert!(err.starts_with("swarm_size"), "{n}: {err}");
    }
    let id = submit_swarm(&e, &p, "Add the feature file", race_spec(3));
    let most_running = std::cell::Cell::new(0);
    let running_now = |t: &Task| {
        let n = t.runs.iter().filter(|r| r.role == RoleKind::Worker && r.variant.is_some() && r.state == StepState::Running).count();
        most_running.set(most_running.get().max(n));
        n
    };
    until("two variants running and the third waiting for a seat", || {
        let t = task(&e, &id);
        running_now(&t) == 2 && t.runs.iter().any(|r| r.role == RoleKind::Worker && r.variant.is_some() && r.state == StepState::Pending)
    });
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert_eq!(running_now(&task(&e, &id)), 2, "the third does not start while both seats are taken");
    w.open();
    until("the advice", || {
        let t = task(&e, &id);
        running_now(&t);
        t.status == Status::Reviewing && e.swarm_view(&id).is_some_and(|v| v.advice.is_some())
    });
    assert_eq!(most_running.get(), 2, "never more runs than seats");
    let v = e.swarm_view(&id).unwrap();
    assert!(v.variants.iter().all(|x| x.state == "passed"), "and it ran when a seat freed up");
}

#[test]
fn stopping_the_swarm_stops_every_run_keeps_what_passed_and_the_copies_until_the_task_is_removed() {
    let env = Env::new();
    let p = env.project(|p| {
        p.check_cmd = Some(CHECK.into());
        p.agent = "fa".into();
    });
    let w = Arc::new(race_fake(
        "fa",
        |m| {
            if m == "m1" {
                vec![vec![write("feature.txt", "ok from m1\n"), done("built")]]
            } else {
                vec![vec![FakeStep::Sleep(60_000), write("feature.txt", "ok\n"), done("late")]]
            }
        },
        vec![vec![done(&compare("A", &[("A", [9, 9, 9, 9])]))]],
    ));
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store_v2(&p, settings_for("fa", 3)), vec![w.clone()], n.clone(), None);
    let id = submit_swarm(&e, &p, "Add the feature file", race_spec(3));
    until("A passed while B and C still run", || {
        e.swarm_view(&id).is_some_and(|v| v.variants.len() == 3 && v.variants[0].state == "passed" && v.variants[1].state == "running" && v.variants[2].state == "running")
    });
    let t0 = Instant::now();
    e.stop_swarm(&id).unwrap();
    assert!(t0.elapsed() < Duration::from_secs(10), "the runs were killed, not waited for");

    let t = task(&e, &id);
    assert_eq!(t.status, Status::NeedsHuman);
    assert!(t.runs.iter().all(|r| !matches!(r.state, StepState::Running | StepState::Pending)), "{:?}", t.runs);
    let v = e.swarm_view(&id).unwrap();
    assert!(v.stopped);
    assert_eq!(v.variants.iter().map(|x| x.state.as_str()).collect::<Vec<_>>(), ["passed", "failed", "failed"]);
    assert_eq!(v.variants[1].why_failed.as_deref(), Some("stopped"));
    assert!(!kinds_of(&n).contains(&NoticeKind::Question), "stopping is the person's own act: no call");
    // the copies are all there
    assert!(v.variants.iter().all(|x| std::path::Path::new(&x.worktree).exists()));

    // the state «рой остановлен» is not a question of an agent: a plain reply is not an answer to it
    let special = e.answer(&id, "A").unwrap_err();
    assert_eq!(special, "swarm_stopped");
    assert_eq!(task(&e, &id).status, Status::NeedsHuman, "the reply changed nothing");
    assert_eq!(e.note(&id, None, "just chatting").unwrap(), nightshift_core::engine::NoteResult::Queued, "it is a note to the agents, not an answer");

    // what passed can still be taken
    e.pick_variant(&id, "A").unwrap();
    wait_status(&e, &id, Status::Done);
    assert_eq!(env.main_file("feature.txt").trim(), "ok from m1");
    let v = e.swarm_view(&id).unwrap();
    assert!(v.variants.iter().all(|x| std::path::Path::new(&x.worktree).exists()), "the copies live after the task is done");
    e.remove(&id).unwrap();
    assert!(v.variants.iter().all(|x| !std::path::Path::new(&x.worktree).exists()), "and are gone with «Удалить»");
}

#[test]
fn notes_reach_the_next_run_of_their_addressee_only_and_the_feed_of_the_swarm_names_role_model_and_variant() {
    let env = Env::new();
    let p = env.project(|p| {
        p.check_cmd = Some(CHECK.into());
        p.agent = "fa".into();
    });
    let w = Arc::new(race_fake(
        "fa",
        |m| vec![vec![write("feature.txt", &format!("ok from {m}\n")), done(&format!("{m} built it"))]],
        vec![vec![done(&compare("A", &[("A", [9, 9, 9, 9]), ("B", [5, 5, 5, 5]), ("C", [4, 4, 4, 4])]))]],
    ));
    // midday: on the wall clock a run at night (23:00-08:00) starts the night task before start_now
    let (clock, _now) = clock_at(12, 0);
    let e = env.engine(env.store_v2(&p, settings_for("fa", 3)), vec![w.clone()], Arc::new(RecordingNotifier::default()), Some(clock));
    // queued first, so the notes are there before any run of the swarm starts
    let id = e.submit(NewTask { project_id: p.id.clone(), prompt: "Add the feature file".into(), swarm: race_spec(3), start: Some(StartWhen::Night), ..NewTask::default() }).unwrap();
    for (to, text) in [(Some("A"), "ONLY-A-NOTE"), (None, "EVERYONE-NOTE"), (Some("reviewer"), "REVIEWER-NOTE")] {
        assert_eq!(e.note(&id, to.map(str::to_string), text).unwrap(), nightshift_core::engine::NoteResult::Queued);
    }
    e.start_now(&id).unwrap();
    until("the advice", || e.swarm_view(&id).is_some_and(|v| v.advice.is_some()));

    let by_label = |l: &str| calls_of(&w, RoleKind::Worker).into_iter().find(|r| r.cwd.ends_with(l)).unwrap().prompt;
    let (pa, pb, pc) = (by_label("A"), by_label("B"), by_label("C"));
    assert!(pa.contains("ONLY-A-NOTE") && !pb.contains("ONLY-A-NOTE") && !pc.contains("ONLY-A-NOTE"), "only A gets what is for A: {:?}", (pa.contains("ONLY-A-NOTE"), pb.contains("ONLY-A-NOTE"), pc.contains("ONLY-A-NOTE")));
    for (name, p) in [("A", &pa), ("B", &pb), ("C", &pc)] {
        assert!(p.contains("EVERYONE-NOTE"), "everybody gets «Всем», {name} did not");
        assert!(!p.contains("REVIEWER-NOTE"), "{name}");
    }
    let review = &calls_of(&w, RoleKind::Reviewer)[0].prompt;
    assert!(review.contains("REVIEWER-NOTE") && review.contains("EVERYONE-NOTE") && !review.contains("ONLY-A-NOTE"));
    // the feed says where each note went
    let log = e.store().lock().unwrap().read_log(&id, 5000);
    let delivered = |text: &str, variant: Option<&str>, role: RoleKind| {
        log.iter().any(|l| l.text == format!("note delivered: {text}") && l.variant.as_deref() == variant && l.role == Some(role))
    };
    assert!(delivered("ONLY-A-NOTE", Some("A"), RoleKind::Worker));
    assert!(delivered("EVERYONE-NOTE", Some("C"), RoleKind::Worker));
    assert!(delivered("REVIEWER-NOTE", None, RoleKind::Reviewer));
    // every line of a variant's agent carries the role, the model and the variant
    let said = log.iter().find(|l| l.text == "m2 built it").expect("B's last words are in the feed");
    assert_eq!((said.role, said.model.is_some(), said.variant.as_deref()), (Some(RoleKind::Worker), true, Some("B")));
}

#[test]
fn freeze_stops_every_run_of_the_swarm_and_the_watcher_does_not_tick_while_frozen() {
    let env = Env::new();
    let p = env.project(|p| {
        p.check_cmd = Some(CHECK.into());
        p.agent = "fa".into();
    });
    let (clock, now) = manual_clock();
    // every variant: the first run is silent (and is frozen), the second one works
    let w = Arc::new(race_fake(
        "fa",
        |m| vec![vec![FakeStep::Sleep(60_000)], vec![write("feature.txt", &format!("ok from {m}\n")), done("built")]],
        vec![vec![done(&compare("A", &[("A", [9, 9, 9, 9]), ("B", [5, 5, 5, 5]), ("C", [4, 4, 4, 4])]))]],
    ));
    let e = env.engine(env.store_v2(&p, settings_for("fa", 3)), vec![w.clone()], Arc::new(RecordingNotifier::default()), Some(clock));
    let id = submit_swarm(&e, &p, "Add the feature file", race_spec(3));
    until("three runs going", || calls_of(&w, RoleKind::Worker).len() == 3 && task(&e, &id).runs.iter().filter(|r| r.role == RoleKind::Worker && r.state == StepState::Running).count() == 3);
    e.freeze(true);
    until("every run stopped", || task(&e, &id).runs.iter().all(|r| !matches!(r.state, StepState::Running)));
    assert_eq!(task(&e, &id).status, Status::Working, "the status stays");
    // an hour passes while frozen: nobody is "silent" now
    advance(&now, 3600);
    assert_eq!(calls_of(&w, RoleKind::Worker).len(), 3, "nothing started");
    assert!(calls_of(&w, RoleKind::Watcher).is_empty(), "and the Watcher did not look at the silence of the frozen runs");

    e.freeze(false);
    until("the advice", || e.swarm_view(&id).is_some_and(|v| v.advice.is_some()));
    assert_eq!(calls_of(&w, RoleKind::Worker).len(), 6, "each variant started again from where it was");
    assert!(calls_of(&w, RoleKind::Watcher).is_empty());
    assert!(e.swarm_view(&id).unwrap().variants.iter().all(|v| v.state == "passed"));
}
