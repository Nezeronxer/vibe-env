//! One env per task (G22, G11): no run of a swarm, a crew, a council or a convoy ever goes to the
//! adapter of another env (fake-A and fake-B in one engine).
mod common;
use common::*;
use nightshift_core::agent::{FakeAdapter, FakeStep};
use nightshift_core::engine::NewTask;
use nightshift_core::model::*;
use nightshift_core::notify::RecordingNotifier;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn until(what: &str, mut f: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while !f() {
        assert!(t0.elapsed() < Duration::from_secs(60), "timeout: {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn marks() -> String {
    serde_json::json!({"scores": [
        {"variant":"A","correctness":9,"coverage":9,"simplicity":9,"risk":9,"why":"a"},
        {"variant":"B","correctness":5,"coverage":5,"simplicity":5,"risk":5,"why":"b"}], "recommend":"A", "advice":"A"})
    .to_string()
}

/// An env that can do everything a swarm asks of it, whichever mode.
fn env_of(id: &str) -> FakeAdapter {
    let s = |v: Vec<FakeStep>| vec![v; 4];
    let split = serde_json::json!({"parts":[{"id":"p1","title":"one","files":["one.txt"]},{"id":"p2","title":"two","files":["two.txt"]}]}).to_string();
    let convoy = serde_json::json!({"tasks":[{"id":"t1","title":"First","text":"first","depends_on":[]},{"id":"t2","title":"Second","text":"second","depends_on":["t1"]}]}).to_string();
    FakeAdapter::new(vec![])
        .with_id(id)
        .with_role(RoleKind::Coordinator, vec![vec![done(&split)], vec![done(&convoy)], vec![done(&split)]])
        .with_role(RoleKind::Reviewer, (0..6).map(|i| if i == 0 { vec![done(&marks())] } else { approve() }).collect())
        .with_role(RoleKind::Council, s(vec![done(r#"{"approach":"the approach","vote":"A","verdict":"approve","issues":[]}"#)]))
        .with_role(RoleKind::Worker, (0..8).map(|k| vec![write(&format!("w{k}.txt"), "ok\n"), done("done")]).collect())
}

#[test]
fn a_race_a_crew_a_council_and_a_convoy_in_one_env_never_call_the_adapter_of_the_other() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let (a, b) = (Arc::new(env_of("fa")), Arc::new(env_of("fb")));
    let e = env.engine(
        env.store(&p, |s| {
            env_models(s, "fa", &["m1", "m2"], "m1", &[], None);
            env_models(s, "fb", &["m1", "m2"], "m1", &[], None);
            s.max_parallel = 3;
        }),
        vec![a.clone(), b.clone()],
        Arc::new(RecordingNotifier::default()),
        None,
    );
    let swarm = |mode| NewTask { project_id: p.id.clone(), prompt: format!("Work in {mode:?} mode"), swarm: Some(SwarmSpec { mode, n: 2 }), ..NewTask::default() };
    // the race: wait for the advice, pick
    let race = e.submit(swarm(SwarmMode::Race)).unwrap();
    until("the advice", || e.swarm_view(&race).is_some_and(|v| v.advice.is_some()));
    e.pick_variant(&race, "A").unwrap();
    wait_status(&e, &race, Status::Done);
    // the crew and the council, one after the other
    for mode in [SwarmMode::Crew, SwarmMode::Council] {
        let id = e.submit(swarm(mode)).unwrap();
        wait_status(&e, &id, Status::Done);
    }
    // the convoy
    e.submit_convoy(&p.id, "fa", "Do two things").unwrap();
    until("the convoy is done", || {
        let ts = e.store().lock().unwrap().tasks();
        ts.iter().filter(|t| t.convoy_id.is_some()).count() == 2 && ts.iter().filter(|t| t.convoy_id.is_some()).all(|t| t.status == Status::Done)
    });

    assert!(!a.calls().is_empty());
    assert_eq!(b.calls().len(), 0, "the other env was never asked for anything: {:?}", b.calls().iter().map(|r| r.role).collect::<Vec<_>>());
    assert!(e.store().lock().unwrap().tasks().iter().all(|t| t.agent == "fa"));
}

#[test]
fn two_races_at_once_in_two_envs_keep_their_runs_to_their_own_adapter() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let (a, b) = (Arc::new(env_of("fa")), Arc::new(env_of("fb")));
    let e = env.engine(
        env.store(&p, |s| {
            env_models(s, "fa", &["m1", "m2"], "m1", &[], None);
            env_models(s, "fb", &["m1", "m2"], "m1", &[], None);
            s.max_parallel = 2;
        }),
        vec![a.clone(), b.clone()],
        Arc::new(RecordingNotifier::default()),
        None,
    );
    let go = |agent: &str| {
        e.submit(NewTask { project_id: p.id.clone(), prompt: format!("Race in {agent}"), agent: Some(agent.into()), swarm: Some(SwarmSpec { mode: SwarmMode::Race, n: 2 }), ..NewTask::default() }).unwrap()
    };
    let (ia, ib) = (go("fa"), go("fb"));
    for id in [&ia, &ib] {
        until("the advice", || e.swarm_view(id).is_some_and(|v| v.advice.is_some()));
    }
    let mine = |calls: Vec<nightshift_core::agent::Run>, id: &str, other: &str| {
        assert!(!calls.is_empty());
        for r in calls {
            let c = r.cwd.to_string_lossy().to_string();
            assert!(c.contains(&id[..8]) && !c.contains(&other[..8]), "a run of the other task came here: {c}");
        }
    };
    mine(a.calls(), &ia, &ib);
    mine(b.calls(), &ib, &ia);
    assert_eq!(e.swarm_view(&ia).unwrap().variants.len(), 2);
    assert_eq!(e.swarm_view(&ib).unwrap().variants.len(), 2);
}
