//! v3 seam: the actions of a task's runs in its log and in the engine's events (stories 225, 226, 230, 231).
mod common;
use common::*;
use nightshift_core::agent::{ActionInput, FakeAdapter, FakeStep, ToolAction};
use nightshift_core::engine::{Engine, EngineEvent};
use nightshift_core::model::*;
use nightshift_core::notify::RecordingNotifier;
use nightshift_core::store::Store;
use std::sync::Arc;

fn edits(log: &[LogEvent]) -> Vec<(String, String)> {
    log.iter()
        .filter_map(|e| match e.action.as_ref().map(|a| &a.input) {
            Some(ActionInput::Edit { path, diff }) => Some((path.clone(), diff.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn a_cli_without_the_text_of_its_edits_gets_an_edit_per_changed_file_from_the_copy_after_the_step() {
    let env = Env::new();
    let p = env.project(|_| {});
    // the fake CLI says only "Write <path>": no text of the change
    let w = fake(vec![vec![write("a.txt", "two\n"), write("src/new.rs", "fn main() {}\n"), done("changed")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Change a.txt and add a file");
    wait_status(&e, &id, Status::Done);
    let mut got = edits(&e.task_log(&id, None, 500));
    got.sort();
    assert_eq!(
        got,
        [
            ("a.txt".to_string(), "--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-one\n+two\n".to_string()),
            ("src/new.rs".to_string(), "--- /dev/null\n+++ b/src/new.rs\n@@ -0,0 +1 @@\n+fn main() {}\n".to_string()),
        ]
    );
    // the reviewer reads, it changes nothing: no edits of its own
    let by_reviewer = e.task_log(&id, None, 500).into_iter().filter(|l| l.action.is_some() && l.role == Some(RoleKind::Reviewer)).count();
    assert_eq!(by_reviewer, 0);
}

fn act(id: &str, done: bool, output: Option<&str>) -> FakeStep {
    FakeStep::Action(ToolAction {
        id: id.into(),
        name: "Bash".into(),
        input: ActionInput::Bash { command: "cat .secret".into() },
        output: output.map(String::from),
        ok: true,
        done,
    })
}

#[test]
fn a_secret_in_an_actions_output_is_replaced_by_its_name_in_the_log_and_in_the_event() {
    let secret = "s3cr3t-value-123";
    let env = Env::new();
    let p = env.project(|_| {});
    let w = fake(vec![vec![act("t1", false, None), act("t1", true, Some(&format!("token: {secret}\n"))), done("read it")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    e.set_env_vars(Arc::new(move |_| vec![("API_TOKEN".to_string(), secret.to_string())]));
    let rx = e.subscribe();
    let id = submit(&e, &p, "Read the token");
    wait_status(&e, &id, Status::Done);
    let sent: Vec<ToolAction> = rx.try_iter().filter_map(|ev| if let EngineEvent::Log(t, l) = ev { l.action.filter(|_| t == id) } else { None }).collect();
    assert_eq!(sent.iter().map(|a| a.done).collect::<Vec<_>>(), [false, true], "the start is sent, then the result");
    assert_eq!(sent[1].output.as_deref(), Some("token: ${API_TOKEN}\n"));
    let stored: Vec<ToolAction> = e.task_log(&id, None, 500).into_iter().filter_map(|l| l.action).collect();
    assert_eq!(stored.len(), 1, "only the result is stored");
    assert_eq!(stored[0].output.as_deref(), Some("token: ${API_TOKEN}\n"));
    let raw = std::fs::read_to_string(env.d.path().join("data/logs").join(format!("{id}.jsonl"))).unwrap();
    assert!(!raw.contains(secret));
}

#[test]
fn after_a_restart_the_log_gives_the_actions_with_output_and_diff_and_old_lines_still_read() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = fake(vec![vec![act("t1", false, None), act("t1", true, Some("hello\n")), write("a.txt", "two\n"), done("ok")]]);
    let store = env.store(&p, |_| {});
    let e = env.engine(store, vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Say hello");
    wait_status(&e, &id, Status::Done);
    e.shutdown();
    drop(e);
    // a line as v2 wrote it, without `action`
    let path = env.d.path().join("data").join("logs").join(format!("{id}.jsonl"));
    let mut raw = std::fs::read_to_string(&path).unwrap();
    raw.push_str("{\"at\":\"2099-01-01T00:00:00+00:00\",\"kind\":\"tool\",\"text\":\"Bash: ls\",\"role\":\"worker\"}\n");
    std::fs::write(&path, raw).unwrap();

    let again = Engine::new(Store::open(&env.d.path().join("data")).unwrap(), vec![Arc::new(FakeAdapter::new(vec![]))], Arc::new(RecordingNotifier::default()), env.d.path().join("wts"));
    let log = again.task_log(&id, None, 500);
    let out: Vec<Option<String>> = log.iter().filter_map(|l| l.action.as_ref()).filter(|a| a.id == "t1").map(|a| a.output.clone()).collect();
    assert_eq!(out, [Some("hello\n".to_string())]);
    assert_eq!(edits(&log), [("a.txt".to_string(), "--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-one\n+two\n".to_string())]);
    let old = log.last().unwrap();
    assert_eq!((old.text.as_str(), old.action.is_none()), ("Bash: ls", true));
    // a page before a time: only what came earlier
    let first = &log[0];
    assert!(again.task_log(&id, Some(first.at), 500).is_empty());
    assert_eq!(again.task_log(&id, None, 2).len(), 2);
}
