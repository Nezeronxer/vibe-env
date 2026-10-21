//! The convoy (Поручение, G08.1): one phrase, tasks with dependencies, a chat with their cards.
mod common;
use common::*;
use nightshift_core::agent::FakeAdapter;
use nightshift_core::model::*;
use nightshift_core::notify::{NoticeKind, RecordingNotifier};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn until(what: &str, mut f: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while !f() {
        assert!(t0.elapsed() < Duration::from_secs(60), "timeout: {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn tasks_json(tasks: &[(&str, &[&str])]) -> String {
    let ts: Vec<_> = tasks.iter().map(|(id, deps)| serde_json::json!({"id": id, "title": format!("Task {id}"), "text": format!("Do {id}"), "depends_on": deps})).collect();
    serde_json::json!({ "tasks": ts }).to_string()
}

fn workers(n: usize) -> Vec<Vec<nightshift_core::agent::FakeStep>> {
    (0..n).map(|k| vec![write(&format!("f{k}.txt"), "x\n"), done("done")]).collect()
}

#[test]
fn a_convoy_of_four_makes_a_chat_with_cards_and_each_task_starts_after_the_ones_it_waits_for_have_landed() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let w = Arc::new(
        FakeAdapter::new(vec![])
            .with_id("fa")
            .with_role(RoleKind::Coordinator, vec![vec![done(&tasks_json(&[("t1", &[]), ("t2", &["t1"]), ("t3", &["t1"]), ("t4", &["t2", "t3"])]))]])
            .with_role(RoleKind::Worker, workers(4))
            .with_role(RoleKind::Reviewer, (0..4).map(|_| approve()).collect()),
    );
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store(&p, |s| s.max_parallel = 3), vec![w.clone()], n.clone(), None);
    let chat_id = e.submit_convoy(&p.id, "fa", "Prepare the release 2.4").unwrap();

    // the chat is there at once, with the person's phrase
    let chat = e.store().lock().unwrap().chat(&chat_id).unwrap();
    assert!(chat.convoy);
    assert_eq!(chat.title, "Поручение: Prepare the release 2.4");
    until("four tasks done", || {
        let ts = e.store().lock().unwrap().tasks();
        ts.len() == 4 && ts.iter().all(|t| t.status == Status::Done)
    });

    let ts = e.store().lock().unwrap().tasks();
    let by = |title: &str| ts.iter().find(|t| t.title == title).unwrap().clone();
    assert!(ts.iter().all(|t| t.chat_id.as_deref() == Some(chat_id.as_str()) && t.convoy_id == ts[0].convoy_id && t.convoy_id.is_some()));
    // one card per task, in the chat, behind the phrase
    let msgs = e.store().lock().unwrap().read_chat(&chat_id, None, 50);
    assert_eq!(msgs.len(), 5);
    assert_eq!(msgs[0].who, Who::Human);
    let cards: Vec<_> = msgs.iter().filter_map(|m| m.task_id.clone()).collect();
    assert_eq!(cards.len(), 4);
    for t in &ts {
        assert!(cards.contains(&t.id));
    }
    // dependencies are real ids, and nothing started before what it waits for had landed
    let (t1, t2, t3, t4) = (by("Task t1"), by("Task t2"), by("Task t3"), by("Task t4"));
    assert_eq!(t2.depends_on, vec![t1.id.clone()]);
    assert_eq!(t4.depends_on, vec![t2.id.clone(), t3.id.clone()]);
    for (child, parents) in [(&t2, vec![&t1]), (&t3, vec![&t1]), (&t4, vec![&t2, &t3])] {
        for parent in parents {
            assert!(child.started_at.unwrap() >= parent.finished_at.unwrap(), "{} started before {} landed", child.title, parent.title);
        }
    }
    // the history of main reads in the same order: Task t1 first, Task t4 last
    let log = g(&env.repo, &["log", "--format=%s", "main"]);
    let order: Vec<&str> = log.lines().filter(|l| l.starts_with("Task")).collect();
    assert_eq!((order.first().copied(), order.last().copied()), (Some("Task t4"), Some("Task t1")), "{order:?}");
    // the Coordinator ran once, the holder of the split left no card behind
    assert_eq!(calls_of(&w, RoleKind::Coordinator).len(), 1);
    assert_eq!(calls_of(&w, RoleKind::Worker).len(), 4);
    assert!(!kinds_of(&n).contains(&NoticeKind::Question));
}

#[test]
fn a_convoy_of_thirteen_tasks_is_not_accepted_and_a_vague_request_becomes_a_question() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let ids: Vec<String> = (0..13).map(|i| format!("t{i}")).collect();
    let many = tasks_json(&ids.iter().map(|i| (i.as_str(), &[][..])).collect::<Vec<_>>());
    let w = Arc::new(FakeAdapter::new(vec![]).with_id("fa").with_role(RoleKind::Coordinator, vec![vec![done(&many)], vec![done(&many)]]));
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone()], n.clone(), None);
    e.submit_convoy(&p.id, "fa", "Do everything").unwrap();
    until("the split failed", || e.store().lock().unwrap().tasks().iter().any(|t| t.status == Status::Failed));
    let tasks = e.store().lock().unwrap().tasks();
    assert_eq!(tasks.len(), 1, "no task of the convoy was made");
    assert!(tasks[0].outcome.error.as_ref().unwrap().starts_with("convoy_split_failed: more than 12 tasks"), "{:?}", tasks[0].outcome.error);
    assert_eq!(calls_of(&w, RoleKind::Coordinator).len(), 2, "one repeat, as for every structured answer");
    assert_eq!(kinds(&n), vec![NoticeKind::Failed]);

    // a vague request: the Coordinator asks, the answer goes back to it, then it splits
    let env = Env::new();
    let p = env.project(|p| p.agent = "fb".into());
    let w = Arc::new(FakeAdapter::new(vec![]).with_id("fb").with_role(
        RoleKind::Coordinator,
        vec![vec![done("Which release?\nNEEDS_HUMAN: Which release do you mean?\nOPTIONS: 2.4 | 2.5")], vec![done(&tasks_json(&[("t1", &[]), ("t2", &["t1"])]))]],
    ).with_role(RoleKind::Worker, workers(2)).with_role(RoleKind::Reviewer, vec![approve(), approve()]));
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone()], n.clone(), None);
    let chat = e.submit_convoy(&p.id, "fb", "Prepare the release").unwrap();
    until("the question", || e.store().lock().unwrap().tasks().iter().any(|t| t.status == Status::NeedsHuman));
    let asked = e.store().lock().unwrap().tasks().into_iter().find(|t| t.status == Status::NeedsHuman).unwrap();
    assert_eq!(asked.question.as_ref().unwrap().text, "Which release do you mean?");
    assert_eq!(asked.question.as_ref().unwrap().options, ["2.4", "2.5"]);
    assert_eq!(notices(&n, 1)[0].kind, NoticeKind::Question);
    e.answer(&asked.id, "2.4").unwrap();
    until("both tasks done", || {
        let ts = e.store().lock().unwrap().tasks();
        ts.len() == 2 && ts.iter().all(|t| t.status == Status::Done)
    });
    let calls = calls_of(&w, RoleKind::Coordinator);
    assert_eq!(calls[1].resume.as_deref(), Some("fake-session-0"), "the answer goes to the same Coordinator");
    assert!(calls[1].prompt.contains("2.4"));
    assert_eq!(e.store().lock().unwrap().read_chat(&chat, None, 50).iter().filter(|m| m.task_id.is_some()).count(), 2);
}
