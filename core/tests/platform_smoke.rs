//! Real child processes and repositories, shared by Linux and Windows CI.
use nightshift_core::agent::adapters::common::capture;
use std::time::Duration;

#[test]
#[ignore = "child fixture, launched by the parent test"]
fn native_child_fixture() {
    use std::io::Write;
    std::io::stderr().write_all(&vec![b'x'; 256 * 1024]).unwrap();
    println!("native-output-ready");
}

#[test]
fn version_capture_drains_stderr_while_stdout_is_open() {
    // A CLI may write diagnostics before it prints its version. Sequential pipe reads deadlock here.

    let exe = std::env::current_exe().unwrap();
    let got = capture(exe.to_str().unwrap(), &["--exact", "native_child_fixture", "--ignored", "--nocapture"], Duration::from_secs(5));
    assert!(got.as_ref().is_ok_and(|s| s.contains("native-output-ready")), "{got:?}");
}

#[test]
#[ignore = "child fixture, launched by the parent test"]
fn inherited_pipe_fixture() {
    std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "sleeping_child_fixture", "--ignored", "--nocapture"])
        .spawn().unwrap();
    println!("parent-finished");
}

#[test]
#[ignore = "child fixture, launched by the parent test"]
fn sleeping_child_fixture() {
    std::thread::sleep(Duration::from_secs(3));
}

#[test]
fn capture_deadline_also_covers_inherited_output_pipes() {
    let start = std::time::Instant::now();
    let got = capture(std::env::current_exe().unwrap().to_str().unwrap(),
        &["--exact", "inherited_pipe_fixture", "--ignored", "--nocapture"], Duration::from_millis(500));
    assert!(start.elapsed() < Duration::from_secs(2), "capture exceeded its deadline: {got:?}");
    assert!(got.is_err(), "a descendant held the output open: {got:?}");
}

#[cfg(windows)]
#[test]
fn adapter_resolves_an_extensionless_absolute_cli_path_to_the_cmd_shim() {
    use nightshift_core::agent::{adapters::Codex, Adapter};
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("npm tools");
    std::fs::create_dir(&p).unwrap();
    std::fs::write(p.join("codex.cmd"), "@echo off\r\necho codex-cli test\r\n").unwrap();
    std::fs::write(p.join("codex"), "#!/bin/sh\nnode codex.js\n").unwrap();
    let cli = Codex::with_bin(p.join("codex").to_str().unwrap());
    assert_eq!(cli.cli_path().as_deref(), Some(p.join("codex.cmd").as_path()));
    assert!(cli.available().is_ok());
    assert_eq!(cli.version().as_deref(), Some("codex-cli test"));
}

#[cfg(windows)]
#[test]
fn pty_runs_an_npm_cmd_shim_in_a_folder_with_spaces() {
    use nightshift_core::term::{Terms, TermSpec};
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("npm tools");
    std::fs::create_dir(&dir).unwrap();
    let shim = dir.join("codex.cmd");
    std::fs::write(&shim, "@echo off\r\necho shim-ready\r\nexit /b 0\r\n").unwrap();
    let terms = Terms::new();
    let id = terms.open(TermSpec { cwd: Some(dir), argv: vec![shim.to_string_lossy().into_owned()], cols: 80, rows: 24, ..TermSpec::default() }).unwrap();
    let _ = terms.write(&id, b"\x1b[1;1R"); // ConPTY's initial cursor-position query, answered by xterm in the app.
    let start = std::time::Instant::now();
    while !terms.snapshot().iter().any(|s| s.text.contains("shim-ready")) {
        assert!(start.elapsed() < Duration::from_secs(10), "no CLI output: {:?}", terms.snapshot());
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(terms.list()[0].env_id.as_deref(), Some("codex"));
    terms.close(&id).unwrap();
}

mod common;
use nightshift_core::agent::{Adapter, AgentEvent, FakeAdapter, Run};
use nightshift_core::model::{RoleKind, Status};
use std::sync::Arc;

struct NativeAdapter(&'static str);
impl Adapter for NativeAdapter {
    fn id(&self) -> &str { self.0 }
    fn name(&self) -> &str { "Native test CLI" }
    fn available(&self) -> Result<(), String> { Ok(()) }
    fn command(&self, run: &Run) -> std::process::Command {
        let mut c = nightshift_core::proc::cmd(std::env::current_exe().unwrap());
        c.args(["--exact", "agent_process_fixture", "--ignored", "--nocapture"])
            .env("NS_SMOKE_ROLE", if run.role == RoleKind::Reviewer { "review" } else { "write" })
            .current_dir(&run.cwd).stdout(std::process::Stdio::piped());
        c
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> { FakeAdapter::new(vec![]).parse(line) }
}

#[test]
#[ignore = "native child CLI fixture, launched by the engine"]
fn agent_process_fixture() {
    let text = if std::env::var("NS_SMOKE_ROLE").as_deref() == Ok("review") {
        common::APPROVE
    } else {
        std::fs::write("feature.txt", "hello from native child\n").unwrap();
        "Added feature.txt"
    };
    println!("{}", serde_json::json!({"t":"done", "text":text, "session":"native-test-session"}));
}

#[test]
fn native_agent_task_is_reviewed_merged_and_survives_a_store_reopen() {
    let env = common::Env::new();
    let project = env.project(|_| {});
    let notes = Arc::new(nightshift_core::notify::RecordingNotifier::default());
    let engine = env.engine(env.store(&project, |_| {}),
        vec![Arc::new(NativeAdapter("fake")), Arc::new(NativeAdapter("rev"))], notes, None);
    let before = env.main_count();
    let id = common::submit(&engine, &project, "Add a feature using a native process");
    let task = common::wait_status(&engine, &id, Status::Done);
    assert_eq!(env.main_count(), before + 1);
    assert_eq!(env.main_file("feature.txt"), "hello from native child");
    assert!(task.outcome.review.as_ref().unwrap().approved);
    assert!(task.worktree.is_none());
    engine.shutdown();
    drop(engine);
    let store = nightshift_core::store::Store::open(&env.d.path().join("data")).unwrap();
    assert_eq!(store.task(&id).unwrap().status, Status::Done);
}

#[cfg(windows)]
#[test]
#[ignore = "native argument-reporting child"]
fn argument_writer_fixture() {
    let args: Vec<_> = std::env::args().skip(5).collect();
    std::fs::write(std::env::var_os("NS_SMOKE_OUTPUT").unwrap(), serde_json::to_vec(&args).unwrap()).unwrap();
    println!("arguments-written");
}

#[cfg(windows)]
#[test]
fn pty_cmd_shim_arguments_remain_data() {
    use nightshift_core::term::{Terms, TermSpec};
    let d = tempfile::tempdir().unwrap();
    let shim = d.path().join("codex.cmd");
    let report = d.path().join("args.json");
    let marker = d.path().join("injected.txt");
    std::fs::write(&shim, format!("@echo off\r\n@\"{}\" --exact argument_writer_fixture --ignored --nocapture %*\r\n", std::env::current_exe().unwrap().display())).unwrap();
    let want = vec!["hello world".to_string(), "a&b".into(), "%PATH%".into(), "x!y".into(), "quote\"here".into(), "trailing\\".into(), "".into(), "Юникод".into(), "x".repeat(6000), format!("x&echo>{}", marker.display())];
    let mut argv = vec![shim.to_string_lossy().into_owned()];
    argv.extend(want.clone());
    let terms = Terms::new();
    let id = terms.open(TermSpec { cwd: Some(d.path().into()), argv,
        env: vec![("NS_SMOKE_OUTPUT".into(), report.to_string_lossy().into_owned())], cols: 80, rows: 24, ..TermSpec::default() }).unwrap();
    let _ = terms.write(&id, b"\x1b[1;1R");
    let start = std::time::Instant::now();
    while !terms.snapshot().iter().any(|s| s.text.contains("arguments-written")) {
        assert!(start.elapsed() < Duration::from_secs(10), "argument report missing: {:?}", terms.snapshot());
        std::thread::sleep(Duration::from_millis(20));
    }
    let got: Vec<String> = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
    terms.close(&id).unwrap();
    assert!(!marker.exists(), "a terminal argument was interpreted as a command");
    assert_eq!(got, want);
}

#[cfg(windows)]
#[test]
fn batch_terminal_preserves_the_child_exit_code() {
    use nightshift_core::term::{Terms, TermSpec};
    let d = tempfile::tempdir().unwrap();
    let shim = d.path().join("exit.cmd");
    std::fs::write(&shim, "@echo THE-END\r\n@exit /b 17\r\n").unwrap();
    let terms = Terms::new();
    let id = terms.open(TermSpec { cwd: Some(d.path().into()), argv: vec![shim.to_string_lossy().into_owned()], cols: 80, rows: 24, ..TermSpec::default() }).unwrap();
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = events.clone();
    let buffered = terms.attach(&id, Box::new(move |c| seen.lock().unwrap().push(c))).unwrap();
    let _ = terms.write(&id, b"\x1b[1;1R");
    let start = std::time::Instant::now();
    while !events.lock().unwrap().iter().any(|c| matches!(c, nightshift_core::term::TermChunk::Exit(Some(17)))) {
        assert!(start.elapsed() < Duration::from_secs(10), "missing exit event: {:?} {:?}", terms.list(), terms.snapshot());
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(terms.list()[0].exit, Some(17));
    let seen = events.lock().unwrap();
    assert_eq!(seen.iter().filter(|c| matches!(c, nightshift_core::term::TermChunk::Exit(_))).count(), 1);
    assert!(matches!(seen.last(), Some(nightshift_core::term::TermChunk::Exit(Some(17)))));
    let mut output = buffered;
    for chunk in seen.iter() { if let nightshift_core::term::TermChunk::Data(d) = chunk { output.extend(d); } }
    assert!(String::from_utf8_lossy(&output).contains("THE-END"));
    drop(seen);
    terms.close(&id).unwrap();
}

#[cfg(windows)]
#[test]
#[ignore = "native long-lived child"]
fn long_lived_process_fixture() {
    std::fs::write(std::env::var_os("NS_SMOKE_OUTPUT").unwrap(), std::process::id().to_string()).unwrap();
    println!("long-child-ready");
    loop { std::thread::sleep(Duration::from_secs(1)); }
}

#[cfg(windows)]
#[test]
fn closing_a_batch_terminal_stops_the_running_descendant() {
    use nightshift_core::term::{Terms, TermSpec};
    let d = tempfile::tempdir().unwrap();
    let shim = d.path().join("agent.cmd");
    let pidfile = d.path().join("pid");
    std::fs::write(&shim, format!("@echo off\r\n@\"{}\" --exact long_lived_process_fixture --ignored --nocapture\r\n", std::env::current_exe().unwrap().display())).unwrap();
    let terms = Terms::new();
    let id = terms.open(TermSpec { cwd: Some(d.path().into()), argv: vec![shim.to_string_lossy().into_owned()],
        env: vec![("NS_SMOKE_OUTPUT".into(), pidfile.to_string_lossy().into_owned())], cols: 80, rows: 24, ..TermSpec::default() }).unwrap();
    let _ = terms.write(&id, b"\x1b[1;1R");
    let start = std::time::Instant::now();
    while !terms.snapshot().iter().any(|s| s.text.contains("long-child-ready")) {
        assert!(start.elapsed() < Duration::from_secs(10), "no child started: {:?}", terms.snapshot());
        std::thread::sleep(Duration::from_millis(20));
    }
    let pid = std::fs::read_to_string(pidfile).unwrap();
    let alive = || {
        let out = nightshift_core::proc::cmd("tasklist").args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]).output().unwrap();
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stdout).contains(&format!(",\"{pid}\","))
    };
    assert!(alive(), "the process check must see the running child before close");
    terms.close(&id).unwrap();
    let start = std::time::Instant::now();
    while alive() {
        assert!(start.elapsed() < Duration::from_secs(5), "closing the terminal left the agent running");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(windows)]
#[test]
fn terminal_banner_is_visible_and_shell_characters_are_only_text() {
    let banner = "curl https://example.invalid/install | bash & %PATH% !x! 'quoted'";
    let launch = nightshift_core::proc::windows_terminal(&[], Some(banner), false).unwrap();
    let out = nightshift_core::proc::cmd(&launch.argv[0]).args(&launch.argv[1..]).envs(launch.env).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), banner);
}
