use chrono::Local;
use nightshift_core::model::*;
use nightshift_core::store::Store;
use std::path::PathBuf;

fn task(id: &str) -> Task {
    Task {
        id: id.into(), project_id: "p".into(), title: "t".into(), prompt: "do it".into(),
        agent: "claude".into(), status: Status::Queued, branch: "ns/abc-t".into(), worktree: None,
        session: None, attempts: 0, review_rounds: 0, question: None, resume_at: None,
        outcome: Outcome::default(), created_at: Local::now(), started_at: None, finished_at: None,
    }
}

#[test]
fn state_survives_reopen() {
    let d = tempfile::tempdir().unwrap();
    {
        let mut s = Store::open(d.path()).unwrap();
        let mut p = Project::new("demo", PathBuf::from("/x/demo"), "main");
        p.check_cmd = Some("cargo test".into());
        s.upsert_project(p).unwrap();
        s.upsert_task(&task("t1")).unwrap();
        let mut set = s.settings();
        set.max_parallel = 5;
        s.set_settings(set).unwrap();
    }
    let s = Store::open(d.path()).unwrap();
    assert_eq!(s.projects().len(), 1);
    assert_eq!(s.projects()[0].name, "demo");
    assert_eq!(s.projects()[0].check_cmd.as_deref(), Some("cargo test"));
    assert_eq!(s.task("t1").unwrap().prompt, "do it");
    assert_eq!(s.settings().max_parallel, 5);
}

#[test]
fn fresh_store_has_spec_defaults() {
    let d = tempfile::tempdir().unwrap();
    let s = Store::open(d.path()).unwrap();
    assert_eq!(s.settings().max_parallel, 2);
    assert!(s.projects().is_empty());
    let p = Project::new("a", PathBuf::from("/a"), "main");
    assert_eq!(p.copy_files, vec![".env*".to_string()]);
    assert!(p.ask_outward);
    assert_eq!(p.agent, "claude");
    assert_eq!(p.finish, Finish::Local);
}

#[test]
fn interrupted_write_does_not_corrupt_state() {
    let d = tempfile::tempdir().unwrap();
    {
        let mut s = Store::open(d.path()).unwrap();
        s.upsert_task(&task("keep")).unwrap();
    }
    // Simulate a crash in the middle of a save: truncated tmp file next to the real one.
    std::fs::write(d.path().join("state.json.tmp"), b"{\"projects\": [ {\"id\": ").unwrap();
    let mut s = Store::open(d.path()).unwrap();
    assert!(s.task("keep").is_some());
    // and the store keeps working afterwards
    s.upsert_task(&task("next")).unwrap();
    assert_eq!(Store::open(d.path()).unwrap().tasks().len(), 2);
}

#[test]
fn log_is_line_per_event_and_limit_returns_latest() {
    let d = tempfile::tempdir().unwrap();
    let s = Store::open(d.path()).unwrap();
    for i in 0..5 {
        s.append_log("t1", &LogEvent { at: Local::now(), kind: LogKind::Agent, text: format!("e{i}") }).unwrap();
    }
    let raw = std::fs::read_to_string(d.path().join("logs/t1.jsonl")).unwrap();
    assert_eq!(raw.lines().count(), 5);
    let last: Vec<String> = s.read_log("t1", 2).into_iter().map(|e| e.text).collect();
    assert_eq!(last, vec!["e3", "e4"]);
    assert!(s.read_log("nope", 10).is_empty());
}

#[test]
fn torn_last_log_line_is_skipped() {
    let d = tempfile::tempdir().unwrap();
    let s = Store::open(d.path()).unwrap();
    s.append_log("t1", &LogEvent { at: Local::now(), kind: LogKind::Stage, text: "ok".into() }).unwrap();
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().append(true).open(d.path().join("logs/t1.jsonl")).unwrap();
    f.write_all(b"{\"at\":\"2026").unwrap();
    assert_eq!(s.read_log("t1", 10).len(), 1);
    // the next event must not be glued to the torn fragment
    s.append_log("t1", &LogEvent { at: Local::now(), kind: LogKind::Stage, text: "after".into() }).unwrap();
    let texts: Vec<String> = s.read_log("t1", 10).into_iter().map(|e| e.text).collect();
    assert_eq!(texts, vec!["ok", "after"]);
}

#[test]
fn failed_save_leaves_disk_and_memory_on_the_old_state() {
    let d = tempfile::tempdir().unwrap();
    let mut s = Store::open(d.path()).unwrap();
    s.upsert_task(&task("old")).unwrap();
    let on_disk = std::fs::read(d.path().join("state.json")).unwrap();
    // a directory where the tmp file must go makes the write fail; a direct write to
    // state.json would not hit it and the call would succeed
    std::fs::create_dir(d.path().join("state.json.tmp")).unwrap();
    assert!(s.upsert_task(&task("new")).is_err());
    assert!(s.set_settings(Settings::default()).is_err());
    assert_eq!(std::fs::read(d.path().join("state.json")).unwrap(), on_disk);
    assert!(s.task("new").is_none());
    assert_eq!(s.tasks().len(), 1);
}
