//! The error report (G71) and the app log (G71.2): the zip holds only technical facts, whatever the data folder
//! holds; the log rotates and keeps no message text.
mod common;
use common::*;
use nightshift_core::diag::{self, AppLog};
use nightshift_core::model::*;
use nightshift_core::notify::RecordingNotifier;
use std::io::Read;
use std::sync::Arc;

/// A token of a real shape, put together at run time (no secret-looking literal in the repository).
fn token() -> String {
    format!("ghp_{}", "Q7".repeat(18))
}

const PROMPT: &str = "Add the PLANTEDPROMPT file for the orchard";
const ANSWER: &str = "PLANTEDANSWER the orchard file is in place";
const CHAT_TEXT: &str = "PLANTEDCHATTEXT what about the orchard";

fn unzip(path: &std::path::Path) -> Vec<(String, String)> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    (0..z.len())
        .map(|i| {
            let mut f = z.by_index(i).unwrap();
            let mut s = String::new();
            f.read_to_string(&mut s).unwrap();
            (f.name().unwrap().to_string(), s)
        })
        .collect()
}

#[test]
fn the_report_holds_versions_settings_states_and_the_log_but_no_token_and_no_text() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = fake(vec![vec![write("orchard.txt", "apples\n"), done(ANSWER)]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, PROMPT);
    wait_status(&e, &id, Status::Done);
    e.shutdown();
    let data = env.d.path().join("data");

    // the person's data with a token and texts in it: settings, a chat, the app log
    let tok = token();
    let state_path = data.join("state.json");
    let mut state: serde_json::Value = serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
    state["settings"]["custom_agents"] = serde_json::json!([{ "id": "mine", "name": "Mine", "run": format!("mycli --token {tok} {{prompt}}") }]);
    state["settings"]["envs"]["fake"]["extra_args"] = serde_json::json!(["--api-key", tok]);
    state["settings"]["release_feed"] = serde_json::json!(format!("https://example.com/feed?token={tok}"));
    state["chats"] = serde_json::json!([{ "id": "c1", "env": "fake", "project_id": p.id, "title": CHAT_TEXT, "created_at": "2026-10-23T10:00:00+03:00", "updated_at": "2026-10-23T10:00:00+03:00" }]);
    std::fs::write(&state_path, serde_json::to_vec(&state).unwrap()).unwrap();
    std::fs::create_dir_all(data.join("chats")).unwrap();
    std::fs::write(data.join("chats").join("c1.jsonl"), format!("{{\"text\":\"{CHAT_TEXT} {tok}\"}}\n")).unwrap();
    let log = AppLog::new(&data);
    log.write("info", "app_started");
    log.write("warn", &format!("cli_update_failed: fake GITHUB_TOKEN={tok}"));

    let out = env.d.path().join("out");
    let zip = diag::collect_with(&data, &out, &diag::Extra { envs: vec![("fake".into(), Some("1.2.3".into()))] }).unwrap();
    assert!(zip.starts_with(&out) && zip.extension().is_some_and(|x| x == "zip"));
    let files = unzip(&zip);
    let names: Vec<&str> = files.iter().map(|f| f.0.as_str()).collect();
    for n in ["README.txt", "versions.json", "settings.json", "tasks.json", "chats.json", "app.log"] {
        assert!(names.contains(&n), "{n} in {names:?}");
    }
    let all: String = files.iter().map(|f| f.1.as_str()).collect::<Vec<_>>().join("\n");
    assert!(!all.contains(&tok), "a token got into the report");
    for text in ["PLANTEDPROMPT", "PLANTEDANSWER", "PLANTEDCHATTEXT", "orchard", "apples"] {
        assert!(!all.contains(text), "{text} got into the report");
    }
    // what is there: the version, the env's CLI, the task's state and the log's events
    let get = |n: &str| files.iter().find(|f| f.0 == n).unwrap().1.clone();
    assert!(get("versions.json").contains(env!("CARGO_PKG_VERSION")) && get("versions.json").contains("1.2.3"));
    let tasks: serde_json::Value = serde_json::from_str(&get("tasks.json")).unwrap();
    assert_eq!(tasks[0]["id"], id.as_str());
    assert_eq!(tasks[0]["status"], "done");
    assert!(tasks[0].get("title").is_none() && tasks[0].get("prompt").is_none());
    assert!(get("app.log").contains("app_started") && get("app.log").contains("cli_update_failed"));
    assert!(get("README.txt").contains("NOT inside"));
}

#[test]
fn the_app_log_rotates_at_its_size_and_keeps_five_files() {
    let d = tempfile::tempdir().unwrap();
    let log = AppLog::with_limits(d.path(), 400, 5);
    for i in 0..200 {
        log.write("info", &format!("event_{i:03}\nsecond line"));
    }
    let mut files: Vec<String> = std::fs::read_dir(d.path().join("logs")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    files.sort();
    assert_eq!(files, ["app.log", "app.log.1", "app.log.2", "app.log.3", "app.log.4"]);
    for f in &files {
        assert!(std::fs::metadata(d.path().join("logs").join(f)).unwrap().len() <= 400, "{f} is over the limit");
    }
    let tail = log.tail(3);
    assert_eq!(tail.len(), 3);
    assert!(tail[2].ends_with("event_199 second line"), "{tail:?}");
    assert!(log.tail(1000).iter().all(|l| !l.contains('\n')));
    // the real limits of the app
    assert_eq!((diag::LOG_MAX_BYTES, diag::LOG_KEEP), (1024 * 1024, 5));
}
