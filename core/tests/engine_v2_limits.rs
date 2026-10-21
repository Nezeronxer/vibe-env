//! Engine v2: limits and fallback model, refused models, budget, seats per env, the night queue,
//! an env that is gone or asks to log in. Main seam: Engine + FakeAdapter + temp repo + a clock the test moves.
mod common;
use common::*;
use nightshift_core::agent::{FakeAdapter, FakeStep};
use nightshift_core::engine::EngineEvent;
use nightshift_core::model::*;
use nightshift_core::notify::{NoticeKind, RecordingNotifier};
use std::sync::Arc;
use std::time::Duration;

fn quiet_pipeline(id: &str) -> FakeAdapter {
    FakeAdapter::new(vec![]).with_id(id).with_role(RoleKind::Reviewer, (0..6).map(|_| approve()).collect())
}

#[test]
fn a_limit_with_the_spare_model_also_used_up_pauses_until_the_reset_the_run_reported_and_then_goes_on() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let (clock, now) = manual_clock();
    let t0 = *now.lock().unwrap();
    let resets = t0 + chrono::Duration::hours(2);
    let fa = Arc::new(quiet_pipeline("fa").with_role(
        RoleKind::Worker,
        vec![
            vec![FakeStep::Usage(vec![window("five_hour", resets, true, t0)]), FakeStep::Limit], // the main model
            vec![FakeStep::Limit],                                                              // the spare is used up too
            vec![write("f.txt", "f"), done("finished")],                                         // after the reset
        ],
    ));
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store(&p, |s| env_models(s, "fa", &["big", "spare"], "big", &[], Some("spare"))), vec![fa.clone()], n.clone(), Some(clock));

    let id = submit(&e, &p, "Add f");
    let t = wait_status(&e, &id, Status::PausedLimit);
    assert_eq!(t.resume_at, Some(resets), "the time the env reported");
    assert_eq!(calls_of(&fa, RoleKind::Worker).len(), 2, "main model, then the spare, then the wait");
    assert!(has_log(&e, &id, "switched_to: spare"));
    assert_eq!(kinds_of(&n), vec![NoticeKind::Limit], "a quiet notice, no call");

    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(calls_of(&fa, RoleKind::Worker).len(), 2, "nothing before the reset");
    advance(&now, 2 * 3600 + 1);
    wait_status(&e, &id, Status::Done);
    assert_eq!(env.main_file("f.txt"), "f");
}

#[test]
fn without_a_spare_model_the_wait_is_half_an_hour_when_the_time_is_not_known_and_the_limit_is_stored() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let (clock, now) = manual_clock();
    let t0 = *now.lock().unwrap();
    let fa = Arc::new(quiet_pipeline("fa").with_role(RoleKind::Worker, vec![vec![FakeStep::Limit], vec![write("f.txt", "f"), done("ok")]]));
    let e = env.engine(env.store(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), Some(clock));

    let id = submit(&e, &p, "Add f");
    let t = wait_status(&e, &id, Status::PausedLimit);
    assert_eq!(t.resume_at, Some(t0 + chrono::Duration::minutes(30)));
    // the env's limit is known as used up until then (the night queue and new tasks read it)
    let EnvLimits::Known(w) = e.limits("fa") else { panic!("{:?}", e.limits("fa")) };
    assert!(w.iter().any(|x| x.exhausted && x.resets_at == t.resume_at), "{w:?}");
    advance(&now, 31 * 60);
    wait_status(&e, &id, Status::Done);
}

#[test]
fn usage_events_are_stored_in_limits_json_announced_and_an_env_without_data_says_why() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let now = chrono::Local::now();
    let w = window("five_hour", now + chrono::Duration::hours(3), false, now);
    let fa = Arc::new(quiet_pipeline("fa").with_role(RoleKind::Worker, vec![vec![FakeStep::Usage(vec![w.clone()]), write("f.txt", "f"), done("ok")]]));
    let fb = Arc::new(quiet_pipeline("fb"));
    let e = env.engine(env.store(&p, |_| {}), vec![fa, fb], Arc::new(RecordingNotifier::default()), None);
    let events = e.subscribe();
    assert_eq!(e.limits("fa"), EnvLimits::NoData("after_first_run".into()), "before the first run");

    let id = submit(&e, &p, "Add f");
    wait_status(&e, &id, Status::Done);

    assert_eq!(e.limits("fa"), EnvLimits::Known(vec![w.clone()]));
    let file = std::fs::read_to_string(env.d.path().join("data").join("limits.json")).unwrap();
    assert!(file.contains("five_hour") && file.contains("rate_limit_event"), "{file}");
    let got: Vec<String> = events.try_iter().filter_map(|ev| if let EngineEvent::LimitsChanged(e) = ev { Some(e) } else { None }).collect();
    assert_eq!(got, vec!["fa".to_string()], "announced once");
    // an env that ran but reported nothing: no made-up numbers, and the reason
    let id2 = submit_with(&e, &p, "Add g", "fb");
    wait_status(&e, &id2, Status::Done);
    assert_eq!(e.limits("fb"), EnvLimits::NoData("env_does_not_report".into()));
}

#[test]
fn a_model_the_cli_refuses_is_not_an_attempt_the_next_model_of_the_chain_runs_and_the_feed_says_which() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(quiet_pipeline("fa").with_role(
        RoleKind::Worker,
        vec![vec![FakeStep::RejectModel("no such model: opus 4.8".into())], vec![write("f.txt", "f"), done("ok")]],
    ));
    let cat = Catalog {
        env: "fa".into(),
        families: vec![ModelFamily {
            id: "opus".into(),
            label: "Opus".into(),
            vendor: "anthropic".into(),
            blurb: None,
            versions: vec!["5.5".into(), "4.8".into()],
            default: false,
            exact: [("5.5".to_string(), "claude-opus-5-5".to_string()), ("4.8".to_string(), "claude-opus-4-8".to_string())].into(),
        }],
        source: CatalogSource::CliList,
        at: chrono::Local::now(),
    };
    let store = env.store(&p, |s| {
        env_models(s, "fa", &[], "opus", &[], None);
        s.envs.get_mut("fa").unwrap().default_model = Some(ModelChoice { family: "opus".into(), version: Some("4.8".into()) });
    });
    store.set_catalog(cat).unwrap();
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(store, vec![fa.clone()], n.clone(), None);

    let id = submit(&e, &p, "Add f");
    let t = wait_status(&e, &id, Status::Done);

    let w = calls_of(&fa, RoleKind::Worker);
    assert_eq!((w[0].model.as_deref(), w[1].model.as_deref()), (Some("claude-opus-4-8"), Some("claude-opus-5-5")));
    assert_eq!(t.attempts, 0, "not an attempt");
    assert!(has_log(&e, &id, "model_rejected: Opus 4.8 -> Opus 5.5"));
    // the run record says what really worked
    assert!(t.runs.iter().any(|r| r.role == RoleKind::Worker && r.model == "Opus 5.5" && matches!(r.state, StepState::Done)));
    assert!(!kinds_of(&n).iter().any(|k| k.is_loud()), "no call for a refused model: {:?}", kinds_of(&n));
}

#[test]
fn a_pinned_version_that_left_the_list_runs_on_the_nearest_and_the_feed_says_so() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(quiet_pipeline("fa").with_role(RoleKind::Worker, vec![vec![write("f.txt", "f"), done("ok")]]));
    let n = Arc::new(RecordingNotifier::default());
    let cat = Catalog {
        env: "fa".into(),
        families: vec![ModelFamily {
            id: "sonnet".into(),
            label: "Sonnet".into(),
            vendor: "anthropic".into(),
            blurb: None,
            versions: vec!["5.5".into(), "4.8".into()],
            default: false,
            exact: [("5.5".to_string(), "claude-sonnet-5-5".to_string()), ("4.8".to_string(), "claude-sonnet-4-8".to_string())].into(),
        }],
        source: CatalogSource::CliList,
        at: chrono::Local::now(),
    };
    let store = env.store(&p, |s| {
        env_models(s, "fa", &[], "sonnet", &[], None);
        s.envs.get_mut("fa").unwrap().default_model = Some(ModelChoice { family: "sonnet".into(), version: Some("4.5".into()) });
    });
    store.set_catalog(cat).unwrap();
    let e = env.engine(store, vec![fa.clone()], n.clone(), None);

    let id = submit(&e, &p, "Add f");
    wait_status(&e, &id, Status::Done);

    assert_eq!(calls_of(&fa, RoleKind::Worker)[0].model.as_deref(), Some("claude-sonnet-4-8"), "the next newer of the same family");
    assert!(has_log(&e, &id, "model_replaced: 4.5 -> 4.8"));
    assert!(!kinds_of(&n).contains(&NoticeKind::Question), "no call for that");
}

#[test]
fn the_budget_of_a_task_asks_add_five_or_stop_when_reached() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(quiet_pipeline("fa").with_role(RoleKind::Worker, vec![vec![write("f.txt", "f"), FakeStep::Cost(5.0), done("ok")]]));
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store(&p, |_| {}), vec![fa.clone()], n.clone(), None);

    let id = submit(&e, &p, "Add f");
    let t = wait_status(&e, &id, Status::NeedsHuman);

    // the first run was handed the whole budget, and it spent it
    assert_eq!(calls_of(&fa, RoleKind::Worker)[0].max_budget_usd, Some(5.0));
    assert_eq!(t.cost_usd, Some(5.0));
    let q = t.question.unwrap();
    assert_eq!(q.text, "budget_exceeded: 5.00 of 5.00");
    assert_eq!(q.options, vec!["Add $5", "Stop"]);
    let got = notices(&n, 1); // the notice follows the status
    assert_eq!(got.iter().filter(|x| x.kind == NoticeKind::Question).count(), 1);
    assert_eq!(got[0].options, vec!["Add $5", "Stop"]);
    assert!(t.runs.iter().all(|r| r.role != RoleKind::Reviewer), "the reviewer waits for the decision");

    e.answer(&id, "Add $5").unwrap();
    let t = wait_status(&e, &id, Status::Done);
    assert_eq!(t.budget_usd, Some(10.0));
    assert_eq!(calls_of(&fa, RoleKind::Reviewer)[0].max_budget_usd, Some(5.0), "the rest of the budget goes to the next run");
    assert_eq!(t.runs.iter().find(|r| r.role == RoleKind::Worker).unwrap().cost_usd, Some(5.0));

    // "Stop" ends the task for good, the copy is gone
    let fb = Arc::new(quiet_pipeline("fa").with_role(RoleKind::Worker, vec![vec![write("g.txt", "g"), FakeStep::Cost(5.0), done("ok")]]));
    let env2 = Env::new();
    let p2 = env2.project(|p| p.agent = "fa".into());
    let e2 = env2.engine(env2.store(&p2, |_| {}), vec![fb], Arc::new(RecordingNotifier::default()), None);
    let id2 = submit(&e2, &p2, "Add g");
    wait_status(&e2, &id2, Status::NeedsHuman);
    e2.answer(&id2, "Stop").unwrap();
    let t2 = task(&e2, &id2);
    assert_eq!(t2.status, Status::Cancelled);
    assert!(t2.worktree.is_none());
}

#[test]
fn an_env_that_does_not_report_cost_gets_no_budget_and_says_so_once() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(quiet_pipeline("fa").with_role(RoleKind::Worker, vec![vec![write("f.txt", "f"), FakeStep::Cost(50.0), done("ok")]]));
    let e = env.engine(env.store(&p, |_| {}), vec![Arc::new(Knobs::new(fa.clone()).no_cost())], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Add f");
    wait_status(&e, &id, Status::Done);

    assert!(calls_of(&fa, RoleKind::Worker)[0].max_budget_usd.is_none());
    assert_eq!(log_texts(&e, &id).iter().filter(|l| l.as_str() == "cost: env does not report it").count(), 1);
}

#[test]
fn seats_are_counted_per_env_the_second_task_of_a_full_env_waits_while_another_env_starts_at_once() {
    let env = Env::new();
    let p = env.project(|_| {});
    let gd = env.d.path().join("gate");
    std::fs::create_dir(&gd).unwrap();
    let a = Gated::new("fa", vec![vec![write("a1.txt", "1"), done("1")], vec![write("a2.txt", "2"), done("2")]], 0, &gd);
    let b = fake_as("fb", vec![vec![write("b1.txt", "b"), done("b")]]);
    let e = env.engine(env.store(&p, |s| s.max_parallel = 1), vec![a.clone(), b.clone(), reviewer(3)], Arc::new(RecordingNotifier::default()), None);

    let a1 = submit_with(&e, &p, "A one", "fa");
    let a2 = submit_with(&e, &p, "A two", "fa");
    a.wait_started();
    let b1 = submit_with(&e, &p, "B one", "fb");
    // B does not wait for A: it finishes while A holds its only seat
    wait_status(&e, &b1, Status::Done);
    assert_eq!(task(&e, &a2).status, Status::Queued);
    assert_eq!(a.calls(), 1);
    a.open();
    wait_status(&e, &a1, Status::Done);
    wait_status(&e, &a2, Status::Done);
}

fn night_task(e: &nightshift_core::engine::Engine, p: &Project, text: &str) -> String {
    e.submit(nightshift_core::engine::NewTask { project_id: p.id.clone(), prompt: text.into(), agent: Some("fa".into()), start: Some(StartWhen::Night), ..Default::default() }).unwrap()
}

fn two_plain_tasks(id: &str) -> FakeAdapter {
    quiet_pipeline(id).with_role(RoleKind::Worker, vec![vec![write("n.txt", "n"), done("n")], vec![write("a.txt", "A"), done("a")]])
}

#[test]
fn a_night_task_takes_no_seat_waits_outside_the_window_and_starts_when_the_person_is_away() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let (clock, _now) = clock_at(12, 0);
    // the first worker script is for the task that runs first, whichever it is: both write their own file
    let fa = Arc::new(
        quiet_pipeline("fa").with_role(RoleKind::Worker, vec![vec![write("day.txt", "d"), done("day")], vec![write("night.txt", "n"), done("night")]]),
    );
    let e = env.engine(env.store(&p, |s| s.max_parallel = 1), vec![fa.clone()], Arc::new(RecordingNotifier::default()), Some(clock));

    let night = night_task(&e, &p, "Night job");
    let day = submit_with(&e, &p, "Day job", "fa");
    // submitted second, yet it starts first: the waiting night task holds no seat
    wait_status(&e, &day, Status::Done);
    std::thread::sleep(Duration::from_millis(300));
    let t = task(&e, &night);
    assert_eq!(t.status, Status::Queued);
    assert_eq!(t.start, StartWhen::Night, "marked on the card");
    assert_eq!(env.main_file("day.txt"), "d");

    e.set_presence(true); // nobody at the computer
    wait_status(&e, &night, Status::Done);
    assert_eq!(env.main_file("night.txt"), "n");
}

#[test]
fn a_night_task_starts_inside_the_night_window_by_itself_and_start_now_starts_it_at_once() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let (clock, now) = clock_at(22, 59);
    let fa = Arc::new(quiet_pipeline("fa").with_role(RoleKind::Worker, vec![vec![write("a.txt", "A"), done("a")], vec![write("b.txt", "B"), done("b")]]));
    let e = env.engine(env.store(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), Some(clock));

    let a = night_task(&e, &p, "Night a");
    let b = night_task(&e, &p, "Night b");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!((task(&e, &a).status, task(&e, &b).status), (Status::Queued, Status::Queued), "22:59 is before the window");
    e.start_now(&b).unwrap();
    wait_status(&e, &b, Status::Done);
    assert_eq!(task(&e, &a).status, Status::Queued);
    advance(&now, 90); // 23:00:30
    wait_status(&e, &a, Status::Done);
    assert!(e.start_now(&a).is_err(), "nothing to start now");
}

#[test]
fn a_used_up_limit_holds_the_night_queue_and_new_tasks_say_what_they_wait_for_until_the_reset() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let (clock, now) = clock_at(23, 30);
    let t0 = *now.lock().unwrap();
    let resets = t0 + chrono::Duration::hours(1);
    let store = env.store(&p, |_| {});
    store.set_limits("fa", EnvLimits::Known(vec![window("five_hour", resets, true, t0)])).unwrap();
    let fa = Arc::new(two_plain_tasks("fa"));
    let e = env.engine(store, vec![fa.clone()], Arc::new(RecordingNotifier::default()), Some(clock));

    let night = night_task(&e, &p, "Night job");
    let day = submit_with(&e, &p, "Day job", "fa");
    std::thread::sleep(Duration::from_millis(400));
    // both are in the queue, and each card says what it waits for and until when
    for id in [&night, &day] {
        let t = task(&e, id);
        assert_eq!(t.status, Status::Queued, "{id}");
        assert_eq!(t.outcome.error.as_deref(), Some("waiting_limit_reset"));
        assert_eq!(t.resume_at, Some(resets));
    }
    assert_eq!(fa.calls().len(), 0);
    advance(&now, 3601);
    wait_status(&e, &night, Status::Done);
    wait_status(&e, &day, Status::Done);
    assert_eq!(task(&e, &day).outcome.error, None);
}

#[test]
fn a_used_up_limit_with_a_spare_model_does_not_hold_tasks_they_start_on_the_spare() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let (clock, now) = manual_clock();
    let t0 = *now.lock().unwrap();
    let store = env.store(&p, |s| env_models(s, "fa", &["big", "spare"], "big", &[], Some("spare")));
    store.set_limits("fa", EnvLimits::Known(vec![window("five_hour", t0 + chrono::Duration::hours(2), true, t0)])).unwrap();
    let fa = Arc::new(quiet_pipeline("fa").with_role(RoleKind::Worker, vec![vec![write("a.txt", "A"), done("a")]]));
    let e = env.engine(store, vec![fa.clone()], Arc::new(RecordingNotifier::default()), Some(clock));

    let id = submit(&e, &p, "Add a");
    wait_status(&e, &id, Status::Done);
    assert_eq!(calls_of(&fa, RoleKind::Worker)[0].model.as_deref(), Some("spare"));
}

#[test]
fn an_env_that_is_gone_makes_its_tasks_wait_and_go_on_when_it_is_back_never_in_another_env() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let (clock, now) = manual_clock();
    let fa_inner = Arc::new(quiet_pipeline("fa").with_role(RoleKind::Worker, vec![vec![write("a.txt", "A"), done("a")]]));
    let fa = Arc::new(Knobs::new(fa_inner.clone()));
    let fb = Arc::new(quiet_pipeline("fb").with_role(RoleKind::Worker, vec![vec![write("b.txt", "B"), done("b")]]));
    fa.installed.store(false, std::sync::atomic::Ordering::SeqCst);
    let e = env.engine(env.store(&p, |_| {}), vec![fa.clone(), fb.clone()], Arc::new(RecordingNotifier::default()), Some(clock));

    let id = submit(&e, &p, "Add a");
    std::thread::sleep(Duration::from_millis(500));
    let t = task(&e, &id);
    assert_eq!(t.status, Status::Queued);
    assert_eq!(t.outcome.error.as_deref(), Some("env_unavailable: the CLI was removed"));
    assert!(fa_inner.calls().is_empty() && fb.calls().is_empty(), "no run in any env");

    fa.installed.store(true, std::sync::atomic::Ordering::SeqCst); // the CLI is back
    advance(&now, 61);
    wait_status(&e, &id, Status::Done);
    assert!(fb.calls().is_empty(), "it never moved to another env");
    assert_eq!(task(&e, &id).outcome.error, None);
}

#[test]
fn a_login_is_asked_once_per_env_retried_every_minute_and_the_task_goes_on_after_it() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let (clock, now) = manual_clock();
    let fa = Arc::new(quiet_pipeline("fa").with_role(
        RoleKind::Worker,
        vec![vec![FakeStep::Auth("Not logged in. Please run `fa login`.".into())], vec![FakeStep::Auth("Not logged in.".into())], vec![write("a.txt", "A"), done("a")]],
    ));
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store(&p, |_| {}), vec![fa.clone()], n.clone(), Some(clock));

    let id = submit(&e, &p, "Add a");
    let t = wait_status(&e, &id, Status::NeedsHuman);
    assert_eq!(t.question.as_ref().unwrap().text, "auth_required: Fake");
    assert_eq!(notices(&n, 1).iter().filter(|x| x.kind == NoticeKind::Question).count(), 1);

    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(calls_of(&fa, RoleKind::Worker).len(), 1, "no retry before a minute has passed");
    advance(&now, 61); // the first retry: still not logged in
    wait(&e, &id, "second try", |_| calls_of(&fa, RoleKind::Worker).len() == 2);
    wait_status(&e, &id, Status::NeedsHuman);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(n.0.lock().unwrap().iter().filter(|x| x.kind == NoticeKind::Question).count(), 1, "the person is not called again");
    advance(&now, 61); // the person logged in
    wait_status(&e, &id, Status::Done);
    assert_eq!(calls_of(&fa, RoleKind::Worker).len(), 3);
    assert_eq!(env.main_file("a.txt"), "A");
}

#[test]
fn two_tasks_of_one_env_asking_for_a_login_make_one_call_between_them() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let (clock, now) = manual_clock();
    let wait_a_bit = |f: FakeStep| vec![FakeStep::Sleep(300), f];
    let fa = Arc::new(quiet_pipeline("fa").with_role(
        RoleKind::Worker,
        vec![
            wait_a_bit(FakeStep::Auth("Not logged in".into())),
            wait_a_bit(FakeStep::Auth("Not logged in".into())),
            vec![write("one.txt", "1"), done("1")],
            vec![write("two.txt", "2"), done("2")],
        ],
    ));
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store(&p, |s| s.max_parallel = 2), vec![fa.clone()], n.clone(), Some(clock));

    let a = submit(&e, &p, "One");
    let b = submit(&e, &p, "Two");
    wait_status(&e, &a, Status::NeedsHuman);
    wait_status(&e, &b, Status::NeedsHuman);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(n.0.lock().unwrap().iter().filter(|x| x.kind == NoticeKind::Question).count(), 1);
    advance(&now, 61);
    wait_status(&e, &a, Status::Done);
    wait_status(&e, &b, Status::Done);
}

#[test]
fn the_daily_budget_of_an_env_asks_in_the_same_way_and_adding_five_raises_the_cap_for_the_day() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(quiet_pipeline("fa").with_role(RoleKind::Worker, vec![vec![write("f.txt", "f"), FakeStep::Cost(3.0), done("ok")]]));
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store(&p, |s| s.limits.day_usd = 3.0), vec![fa.clone()], n.clone(), None);

    let id = submit(&e, &p, "Add f");
    let t = wait_status(&e, &id, Status::NeedsHuman);
    let q = t.question.unwrap();
    assert_eq!(q.text, "day_budget_exceeded: 3.00 of 3.00");
    assert_eq!(q.options, vec!["Add $5", "Stop"]);
    assert_eq!(t.budget_usd, Some(5.0), "the budget of the task itself is not used up");

    e.answer(&id, "Add $5").unwrap();
    wait_status(&e, &id, Status::Done);
    let today = chrono::Local::now().date_naive();
    assert_eq!(e.store().lock().unwrap().day_spend("fa", today).extra_usd, 5.0, "on disk: a restart does not ask again");
    assert_eq!(calls_of(&fa, RoleKind::Reviewer).len(), 1);
}

#[test]
fn the_catalog_is_refreshed_from_the_envs_list_kept_as_the_snapshot_and_announced() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fam = ModelFamily { id: "m1".into(), label: "M1".into(), vendor: "openai".into(), blurb: None, versions: vec![], default: false, exact: Default::default() };
    let fa = Arc::new(quiet_pipeline("fa").with_models(vec![fam.clone()]));
    let e = env.engine(env.store(&p, |_| {}), vec![fa], Arc::new(RecordingNotifier::default()), None);
    let events = e.subscribe();

    let cat = e.refresh_catalog("fa").unwrap();
    assert_eq!(cat.source, CatalogSource::Snapshot);
    assert!(cat.families.iter().any(|f| f.id == "m1"));
    assert!(e.store().lock().unwrap().catalog("fa").unwrap().families.iter().any(|f| f.id == "m1"), "the snapshot is on disk");
    assert!(events.try_iter().any(|ev| matches!(ev, EngineEvent::CatalogChanged(env) if env == "fa")));
    assert!(e.refresh_catalog("nope").is_err());
}

#[test]
fn the_day_spend_outlives_retry_and_removal_and_caps_the_next_run_by_what_is_left_of_the_day() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(quiet_pipeline("fa").with_role(
        RoleKind::Worker,
        vec![vec![write("f.txt", "f"), FakeStep::Cost(2.0), done("ok")], vec![write("g.txt", "g"), done("ok")]],
    ));
    let e = env.engine(env.store(&p, |s| s.limits.day_usd = 3.0), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);

    let a = submit(&e, &p, "Add f");
    wait_status(&e, &a, Status::Done);
    e.remove(&a).unwrap();
    let b = submit(&e, &p, "Add g");
    wait_status(&e, &b, Status::Done);
    let w = calls_of(&fa, RoleKind::Worker);
    assert_eq!(w[1].max_budget_usd, Some(1.0), "the day has 3 - 2 left, less than the task's 5");
    assert_eq!(e.store().lock().unwrap().day_spend("fa", chrono::Local::now().date_naive()).spent_usd, 2.0);
}

#[test]
fn a_cancelled_task_that_asked_for_a_login_does_not_hold_the_queue_of_its_env() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(quiet_pipeline("fa").with_role(RoleKind::Worker, vec![vec![FakeStep::Auth("Not logged in".into())], vec![write("g.txt", "g"), done("ok")]]));
    let e = env.engine(env.store(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);

    let a = submit(&e, &p, "One");
    wait_status(&e, &a, Status::NeedsHuman);
    e.cancel(&a).unwrap();
    let b = submit(&e, &p, "Two");
    wait_status(&e, &b, Status::Done);
}

#[test]
fn a_task_retrying_its_login_keeps_the_queue_of_its_env_closed_and_the_person_is_called_once() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let (clock, now) = manual_clock();
    // the retry runs a while before it hits the login again: the queue is looked at during it
    let fa = Arc::new(quiet_pipeline("fa").with_role(
        RoleKind::Worker,
        vec![
            vec![FakeStep::Auth("Not logged in".into())],
            vec![FakeStep::Sleep(600), FakeStep::Auth("Not logged in".into())],
            vec![write("one.txt", "1"), done("1")],
            vec![write("two.txt", "2"), done("2")],
        ],
    ));
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store(&p, |s| s.max_parallel = 2), vec![fa.clone()], n.clone(), Some(clock));

    let a = submit(&e, &p, "One");
    wait_status(&e, &a, Status::NeedsHuman);
    let b = submit(&e, &p, "Two");
    advance(&now, 61); // the retry of the first task, still not logged in
    wait(&e, &a, "the retry", |_| calls_of(&fa, RoleKind::Worker).len() >= 2);
    wait(&e, &a, "the retry to end", |t| t.status == Status::NeedsHuman && t.runs.len() >= 2);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(calls_of(&fa, RoleKind::Worker).len(), 2, "the queued task did not start in an env with no login");
    assert_eq!(task(&e, &b).status, Status::Queued);
    assert_eq!(n.0.lock().unwrap().iter().filter(|x| x.kind == NoticeKind::Question).count(), 1, "one call for the env");

    advance(&now, 61); // the person logged in
    wait_status(&e, &a, Status::Done);
    wait_status(&e, &b, Status::Done);
    assert_eq!(n.0.lock().unwrap().iter().filter(|x| x.kind == NoticeKind::Question).count(), 1);
}

#[test]
fn pressing_login_while_still_logged_out_keeps_the_queue_of_the_env_closed_and_calls_once() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let (clock, now) = manual_clock();
    let fa = Arc::new(quiet_pipeline("fa").with_role(
        RoleKind::Worker,
        vec![
            vec![FakeStep::Auth("Not logged in".into())],
            vec![FakeStep::Sleep(600), FakeStep::Auth("Not logged in".into())],
            vec![write("one.txt", "1"), done("1")],
            vec![write("two.txt", "2"), done("2")],
        ],
    ));
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store(&p, |s| s.max_parallel = 2), vec![fa.clone()], n.clone(), Some(clock));

    let a = submit(&e, &p, "One");
    wait_status(&e, &a, Status::NeedsHuman);
    let b = submit(&e, &p, "Two");
    e.answer(&a, "Login").unwrap(); // the person says so, but the env is still logged out
    wait(&e, &a, "the retry", |_| calls_of(&fa, RoleKind::Worker).len() >= 2);
    wait(&e, &a, "the retry to end", |t| t.status == Status::NeedsHuman && t.runs.len() >= 2);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(calls_of(&fa, RoleKind::Worker).len(), 2, "the queued task did not start in an env with no login");
    assert_eq!(task(&e, &b).status, Status::Queued);
    assert_eq!(n.0.lock().unwrap().iter().filter(|x| x.kind == NoticeKind::Question).count(), 1, "one call for the env");

    advance(&now, 61); // logged in now
    wait_status(&e, &a, Status::Done);
    wait_status(&e, &b, Status::Done);
}
