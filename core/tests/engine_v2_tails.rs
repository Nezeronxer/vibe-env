//! Engine v2: the tails of the review of task 03 (output that is not UTF-8, a task landed once and
//! retried, a list of hidden files that cannot be read), what the codex adapter is given, old data.
mod common;
use common::*;
use nightshift_core::agent::{Adapter, Role, Run};
use nightshift_core::model::*;
use nightshift_core::notify::RecordingNotifier;
use std::fs;
use std::sync::Arc;

#[test]
fn check_output_in_cp1251_and_with_binary_bytes_reaches_the_tail_whole_and_nothing_crashes() {
    let env = Env::new();
    // 0xE9 (cp1251 / latin-1 letter), 0xFF 0xFE and a NUL: none of it is UTF-8
    let p = env.project(|p| p.check_cmd = Some(r"printf 'first line\n\351 caf\351 ok\n\377\376\000 bin\nlast line\n'; exit 1".into()));
    let w = fake(vec![vec![done("1")], vec![done("2")], vec![done("3")]]);
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(1)], n.clone(), None);

    let id = submit(&e, &p, "Make it pass");
    let t = wait_status(&e, &id, Status::Failed);

    let tail = &t.outcome.tests.as_ref().unwrap().tail;
    let lines: Vec<&str> = tail.lines().collect();
    assert_eq!(lines.len(), 4, "every line is there: {tail:?}");
    assert_eq!(lines[0], "first line");
    assert!(lines[1].contains('\u{FFFD}') && lines[1].contains("ok"), "undecodable bytes become the replacement character: {:?}", lines[1]);
    assert!(lines[2].contains('\u{FFFD}') && lines[2].contains("bin"), "{:?}", lines[2]);
    assert_eq!(lines[3], "last line");
    // the worker was told, with the same text
    assert!(w.calls()[1].prompt.contains("last line") && w.calls()[1].prompt.contains('\u{FFFD}'));
    // and the stored task reads back after a restart
    e.shutdown();
    let st = nightshift_core::store::Store::open(&env.d.path().join("data")).unwrap();
    assert_eq!(st.task(&id).unwrap().outcome.tests.unwrap().tail, *tail);
}

#[test]
fn a_task_that_landed_can_be_retried_and_the_new_work_reaches_main_the_old_marker_does_not_get_in_the_way() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = fake(vec![vec![write("one.txt", "1"), done("first")], vec![write("two.txt", "2"), done("second")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(2)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add the files");
    let first = wait_status(&e, &id, Status::Done);
    let before = env.main_count();
    assert!(first.outcome.commit.is_some());

    e.retry(&id).unwrap();
    let t = wait_status(&e, &id, Status::Done);
    assert!(wait_commit_differs(&e, &id, first.outcome.commit.as_deref().unwrap()), "a new commit");
    assert_eq!(env.main_count(), before + 1, "one more commit on main");
    assert_eq!(env.main_file("two.txt"), "2");
    assert_ne!(t.outcome.commit, first.outcome.commit);
    assert_eq!(t.attempts, 0);
    assert!(w.calls()[1].resume.is_none(), "from a clean branch, a fresh session");
}

fn wait_commit_differs(e: &nightshift_core::engine::Engine, id: &str, old: &str) -> bool {
    task(e, id).outcome.commit.as_deref().is_some_and(|c| c != old)
}

#[test]
fn a_hidden_files_list_that_cannot_be_read_stops_the_task_with_no_commit() {
    let env = Env::new();
    fs::write(env.repo.join(".env"), "TOKEN=x").unwrap(); // copied into the copy and hidden from commits
    let p = env.project(|_| {});
    let gd = env.d.path().join("gate");
    fs::create_dir(&gd).unwrap();
    let w = Gated::new("fake", vec![vec![write("f.txt", "f"), done("f")]], 0, &gd);
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone(), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let main_before = g(&env.repo, &["rev-parse", "main"]);

    let id = submit(&e, &p, "Add f");
    w.wait_started();
    // the list is there (the copy was made), then something other than "no such file" goes wrong with it
    let wt = env.d.path().join("wts").join(&id);
    let list = std::path::PathBuf::from(g(&wt, &["rev-parse", "--absolute-git-dir"])).join("ns-hidden");
    assert!(list.is_file(), "{list:?}");
    fs::remove_file(&list).unwrap();
    fs::create_dir(&list).unwrap(); // reading a folder as a file fails, and it is not NotFound
    w.open();

    let t = wait_status(&e, &id, Status::Failed);
    assert!(t.outcome.error.as_deref().unwrap().starts_with("hidden_list_unreadable"), "{:?}", t.outcome.error);
    assert_eq!(g(&env.repo, &["rev-list", "--count", &format!("main..{}", t.branch)]), "0", "nothing was committed on the branch");
    assert_eq!(g(&env.repo, &["rev-parse", "main"]), main_before);
    let files = g(&env.repo, &["ls-tree", "-r", "--name-only", "main"]);
    assert!(!files.contains(".env") && !files.contains("f.txt"), "{files}");
}

#[test]
fn a_missing_hidden_files_list_just_means_nothing_is_hidden() {
    let env = Env::new(); // no .env anywhere: no list is ever written
    let p = env.project(|_| {});
    let w = fake(vec![vec![write("f.txt", "f"), done("f")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add f");
    wait_status(&e, &id, Status::Done);
    assert_eq!(env.main_file("f.txt"), "f");
}

#[test]
fn the_engine_hands_every_adapter_a_folder_of_its_data_and_codex_writes_its_schema_there_with_mode_0600() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "codex".into());
    let codex = Arc::new(nightshift_core::agent::adapters::Codex::with_bin("codex-not-installed"));
    let wts = env.d.path().join("wts");
    let _e = env.engine(env.store(&p, |_| {}), vec![codex.clone()], Arc::new(RecordingNotifier::default()), None);

    let mut run = Run::new(env.d.path().to_path_buf(), "hi".into(), String::new(), Role::Planner);
    run.json_schema = Some(r#"{"type":"object"}"#.into());
    let cmd = codex.command(&run);
    let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().to_string()).collect();
    let path = std::path::PathBuf::from(&args[args.iter().position(|a| a == "--output-schema").unwrap() + 1]);
    assert!(path.starts_with(wts.join("schemas")), "under the folder the engine was given: {path:?}");
    assert_eq!(fs::read_to_string(&path).unwrap(), r#"{"type":"object"}"#);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::metadata(path.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
    }
}

#[test]
fn a_codex_that_was_never_given_a_folder_says_so_instead_of_picking_one() {
    let codex = nightshift_core::agent::adapters::Codex::with_bin("codex-not-installed");
    let d = tempfile::tempdir().unwrap();
    let mut run = Run::new(d.path().to_path_buf(), "hi".into(), String::new(), Role::Planner);
    run.json_schema = Some("{}".into());
    let evs = nightshift_core::agent::run_blocking(&codex, &run, &mut |_| {}, &std::sync::atomic::AtomicBool::new(false)).unwrap();
    let nightshift_core::agent::AgentEvent::Done { is_error: true, text, .. } = &evs[0] else { panic!("{evs:?}") };
    assert!(text.contains("cannot write the answer schema file") && text.contains("no private folder"), "{text}");
}

#[test]
fn data_of_v1_and_of_the_first_v2_reads_with_defaults() {
    // a chat message without the new fields, a task outcome without the gates
    let m: ChatMsg = serde_json::from_str(r#"{"id":"m1","chat_id":"c1","at":"2026-10-21T10:00:00+03:00","who":"human","text":"hi"}"#).unwrap();
    assert_eq!((m.kind, m.tool, m.model, m.reply_to), (None, None, None, None));
    let m2: ChatMsg = serde_json::from_str(
        r#"{"id":"m2","chat_id":"c1","at":"2026-10-21T10:00:00+03:00","who":{"role":"worker"},"text":"x","kind":"warn","tool":{"name":"edit","arg":"a.rs","result":"ok","ok":true},"model":"Opus 5.5 · run 2","reply_to":"m1"}"#,
    )
    .unwrap();
    assert_eq!(m2.kind, Some(ChatKind::Warn));
    assert_eq!(m2.tool.unwrap().name, "edit");
    assert_eq!(m2.reply_to.as_deref(), Some("m1"));
    let o: Outcome = serde_json::from_str(r#"{"summary":null,"files_changed":0,"insertions":0,"deletions":0,"tests":null,"review":null,"commit":null,"pr_url":null,"error":null}"#).unwrap();
    assert!(o.gates.is_empty());
}
