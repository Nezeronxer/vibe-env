use chrono::Local;
use nightshift_core::model::*;
use nightshift_core::store::Store;
use std::path::PathBuf;

fn task(id: &str) -> Task {
    Task { id: id.into(), project_id: "p".into(), title: "t".into(), prompt: "do it".into(), branch: "ns/abc-t".into(), ..Task::default() }
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
    assert_eq!(s.settings().max_parallel, 3);
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
        s.append_log("t1", &LogEvent::new(Local::now(), LogKind::Agent, &format!("e{i}"))).unwrap();
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
    s.append_log("t1", &LogEvent::new(Local::now(), LogKind::Stage, "ok")).unwrap();
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().append(true).open(d.path().join("logs/t1.jsonl")).unwrap();
    f.write_all(b"{\"at\":\"2026").unwrap();
    assert_eq!(s.read_log("t1", 10).len(), 1);
    // the next event must not be glued to the torn fragment
    s.append_log("t1", &LogEvent::new(Local::now(), LogKind::Stage, "after")).unwrap();
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

fn open_fixture(name: &str) -> (tempfile::TempDir, Store) {
    let d = tempfile::tempdir().unwrap();
    // read + write, not `fs::copy`: copy takes the file's extended attributes too, and a Dropbox folder's ones may not be set elsewhere
    std::fs::write(d.path().join("state.json"), std::fs::read(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap();
    let s = Store::open(d.path()).unwrap();
    (d, s)
}

#[test]
fn v1_state_file_opens_with_spec_defaults_for_new_fields() {
    let (_d, s) = open_fixture("state-v1.json");
    // what v1 stored is unchanged
    let t = s.task("t-1").unwrap();
    assert_eq!((t.status, t.agent.as_str(), t.session.as_deref(), t.review_rounds), (Status::NeedsHuman, "codex", Some("thread-1"), 2));
    assert_eq!(t.question.unwrap().options, vec!["pg", "sqlite"]);
    let p = s.projects().remove(0);
    assert_eq!((p.finish, p.ask_outward, p.check_cmd.as_deref()), (Finish::Push, false, Some("cargo test")));
    // the dead v1 fields stay in the file's model
    assert_eq!((p.reviewer.as_deref(), p.fallback_agent.as_deref()), (Some("gemini"), Some("qwen")));
    let st = s.settings();
    assert_eq!((st.max_parallel, st.paused, st.language.as_str()), (4, true, "ru"));
    assert_eq!(st.telegram.chat_id, Some(42));
    // new fields: spec 29 values, written down by hand from the table
    assert!(t.models.is_empty() && t.steps.is_empty() && t.runs.is_empty() && t.depends_on.is_empty());
    assert_eq!((t.chat_id, t.swarm, t.cost_usd, t.budget_usd), (None, None, None, None));
    assert_eq!(t.start, StartWhen::Now);
    assert!(p.extra_checks.is_empty() && p.run_cmd.is_none());
    assert_eq!((st.attempts, st.review_rounds, st.stall_min, st.autopick_min, st.keep_awake), (3, 3, 15, 10, true));
    assert_eq!((st.limits.task_usd, st.limits.day_usd, st.limits.warn_pct), (5.0, 40.0, 80));
    assert_eq!((st.limits.night_from.as_str(), st.limits.night_to.as_str(), st.limits.away_min), ("23:00", "08:00", 20));
    assert_eq!((st.keep.archive_days, st.keep.log_events, st.keep.chat_records, st.keep.summary_after_min), (7, 500, 200, 30));
    assert_eq!((st.look.theme.as_str(), st.look.font_px, st.look.role_labels), ("system", 13, true));
    assert_eq!((st.swarm_default.n, st.swarm_default.mode), (3, SwarmMode::Race));
    let q = st.quality;
    assert_eq!((q.on, q.spec, q.plan, q.tests_first, q.e2e, q.best_of_n), (true, true, true, true, false, Some(2)));
    assert!(st.envs.is_empty());
    assert!(s.settings_recovered().is_none());
}

#[test]
fn a_saved_v1_file_keeps_telegram_reviewer_and_fallback() {
    let (d, mut s) = open_fixture("state-v1.json");
    s.set_settings(s.settings()).unwrap(); // any write rewrites the file
    let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(d.path().join("state.json")).unwrap()).unwrap();
    assert_eq!(raw["settings"]["telegram"]["chat_id"], 42);
    assert_eq!(raw["projects"][0]["reviewer"], "gemini");
    assert_eq!(raw["projects"][0]["fallback_agent"], "qwen");
}

fn msg(id: &str, chat: &str, mins: i64) -> ChatMsg {
    ChatMsg {
        id: id.into(), chat_id: chat.into(), at: Local::now() + chrono::Duration::minutes(mins),
        who: Who::Human, text: format!("text {id}"), task_id: None, delivered_at: None,
        kind: None, tool: None, model: None, reply_to: None, action: None, attachments: vec![],
    }
}

#[test]
fn chat_message_with_a_known_id_is_not_written_twice() {
    let d = tempfile::tempdir().unwrap();
    let s = Store::open(d.path()).unwrap();
    assert!(s.append_chat_msg(&msg("m1", "c1", 0)).unwrap());
    assert!(!s.append_chat_msg(&msg("m1", "c1", 5)).unwrap());
    assert!(s.append_chat_msg(&msg("m1", "c2", 0)).unwrap(), "the id is unique per chat");
    assert_eq!(s.read_chat("c1", None, 10).len(), 1);
    assert_eq!(std::fs::read_to_string(d.path().join("chats/c1.jsonl")).unwrap().lines().count(), 1);
}

#[test]
fn chat_is_read_in_pages_from_the_end() {
    let d = tempfile::tempdir().unwrap();
    let s = Store::open(d.path()).unwrap();
    for i in 0..7 {
        s.append_chat_msg(&msg(&format!("m{i}"), "c1", i)).unwrap();
    }
    let ids = |v: Vec<ChatMsg>| v.into_iter().map(|m| m.id).collect::<Vec<_>>();
    let last = s.read_chat("c1", None, 3);
    assert_eq!(ids(last.clone()), ["m4", "m5", "m6"]);
    let prev = s.read_chat("c1", Some(last[0].at), 3);
    assert_eq!(ids(prev.clone()), ["m1", "m2", "m3"]);
    assert_eq!(ids(s.read_chat("c1", Some(prev[0].at), 3)), ["m0"]);
    assert!(s.read_chat("nope", None, 3).is_empty());
}

#[test]
fn chats_are_listed_per_env_and_removed_with_their_feed() {
    let d = tempfile::tempdir().unwrap();
    let mut s = Store::open(d.path()).unwrap();
    let chat = |id: &str, env: &str| Chat {
        id: id.into(), env: env.into(), project_id: "p".into(), title: id.into(), session: None,
        convoy: false, created_at: Local::now(), updated_at: Local::now(), archived: false,
        queue: vec![], wait: None, answering: None, terminal: None,
    };
    s.upsert_chat(&chat("c1", "claude")).unwrap();
    s.upsert_chat(&chat("c2", "codex")).unwrap();
    s.append_chat_msg(&msg("m1", "c1", 0)).unwrap();
    let s = {
        drop(s);
        Store::open(d.path()).unwrap()
    };
    assert_eq!(s.chats("claude").len(), 1);
    assert_eq!(s.chat("c2").unwrap().env, "codex");
    let mut s = s;
    s.remove_chat("c1").unwrap();
    assert!(s.chat("c1").is_none() && s.chats("claude").is_empty());
    assert!(s.read_chat("c1", None, 10).is_empty());
}

#[test]
fn catalog_and_limits_survive_reopen_per_env() {
    use nightshift_core::model::{Catalog, CatalogSource, EnvLimits, LimitWindow};
    let d = tempfile::tempdir().unwrap();
    let cat = Catalog { env: "codex".into(), families: vec![], source: CatalogSource::CliList, at: Local::now() };
    let win = LimitWindow {
        name: "5h".into(), model: None, used_pct: Some(54.0), remaining: None, resets_at: None,
        exhausted: false, source: "rate_limit_event".into(), seen_at: Local::now(),
    };
    {
        let s = Store::open(d.path()).unwrap();
        s.set_catalog(cat.clone()).unwrap();
        s.set_limits("claude", EnvLimits::Known(vec![win.clone()])).unwrap();
        s.set_limits("qwen", EnvLimits::NoData("not reported".into())).unwrap();
    }
    let s = Store::open(d.path()).unwrap();
    assert_eq!(s.catalog("codex"), Some(cat));
    assert_eq!(s.catalog("claude"), None);
    assert_eq!(s.limits("claude"), Some(EnvLimits::Known(vec![win])));
    assert_eq!(s.limits("qwen"), Some(EnvLimits::NoData("not reported".into())));
}

#[test]
fn unreadable_settings_fall_back_to_defaults_and_unreadable_state_is_never_replaced() {
    let d = tempfile::tempdir().unwrap();
    {
        let mut s = Store::open(d.path()).unwrap();
        s.upsert_task(&task("t1")).unwrap();
    }
    // settings block of the wrong type: the tasks next to it are intact and must stay
    let path = d.path().join("state.json");
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    v["settings"] = serde_json::json!({ "max_parallel": "many" });
    let bad = serde_json::to_vec(&v).unwrap();
    std::fs::write(&path, &bad).unwrap();
    let s = Store::open(d.path()).unwrap();
    assert_eq!(s.settings().max_parallel, 3);
    assert!(s.task("t1").is_some());
    let copy = s.settings_recovered().expect("the caller learns about the copy").to_path_buf();
    assert_eq!(std::fs::read(&copy).unwrap(), bad);
    assert_ne!(copy, path);

    // a file that is not JSON at all: open refuses, keeps a copy, and the file is left as it was
    std::fs::write(&path, b"{ not json").unwrap();
    let Err(nightshift_core::store::StoreError::Unreadable { copy }) = Store::open(d.path()) else { panic!("must not open") };
    assert_eq!(std::fs::read(&copy).unwrap(), b"{ not json");
    assert_eq!(std::fs::read(&path).unwrap(), b"{ not json", "nothing was overwritten with an empty state");
}

#[test]
fn tab_state_and_tab_keys_from_the_ui_survive_a_save_and_a_reopen_and_old_files_get_defaults() {
    // old settings without them: the defaults the UI has (ui/logic-v3.js tabKeyDefaults)
    let (_d, s) = open_fixture("state-v1.json");
    let st = serde_json::to_value(s.settings()).unwrap();
    let (m, sh) = if cfg!(target_os = "macos") { ("Cmd", "Shift+Cmd") } else { ("Ctrl", "Ctrl+Shift") };
    for (k, v) in [("tab_new", format!("{m}+T")), ("tab_close", format!("{m}+W")), ("tab_split", format!("{m}+D")), ("tab_unsplit", format!("{sh}+D"))] {
        assert_eq!(st["keys"][k], v.as_str(), "{k}");
    }
    assert!(st["ui"]["tabs"].is_null());

    // what the UI sends to set_settings (tabs.js persist, settings.js setK)
    let d = tempfile::tempdir().unwrap();
    let tabs = serde_json::json!({ "v": 1, "tabs": [{ "env": "claude", "chat": "c1" }], "active": 0, "split": null });
    let mut v = serde_json::to_value(Settings::default()).unwrap();
    v["ui"] = serde_json::json!({ "tabs": tabs });
    v["keys"]["tab_new"] = "Alt+Cmd+T".into();
    v["keys"]["tab_unsplit"] = "Ctrl+Alt+U".into();
    Store::open(d.path()).unwrap().set_settings(serde_json::from_value(v).unwrap()).unwrap();
    let back = serde_json::to_value(Store::open(d.path()).unwrap().settings()).unwrap();
    assert_eq!(back["ui"]["tabs"], tabs);
    assert_eq!((back["keys"]["tab_new"].as_str(), back["keys"]["tab_unsplit"].as_str()), (Some("Alt+Cmd+T"), Some("Ctrl+Alt+U")));
}
