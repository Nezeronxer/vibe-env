//! `core::term` on a real PTY: echo, resize, the buffer on attach, the exit code, the ack pause, the environment.
use nightshift_core::term::{TermChunk, TermSpec, Terms};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn spec(argv: &[&str]) -> TermSpec {
    TermSpec { cwd: None, argv: argv.iter().map(|s| s.to_string()).collect(), env: vec![], cols: 80, rows: 24, title: String::new() }
}

/// The bytes and the exit (once it came).
type Seen = (Vec<u8>, Option<Option<i32>>);

/// A sink that keeps everything it got.
#[derive(Clone, Default)]
struct Got(Arc<Mutex<Seen>>);

impl Got {
    fn sink(&self) -> Box<dyn Fn(TermChunk) + Send> {
        let g = self.0.clone();
        Box::new(move |c| {
            let mut g = g.lock().unwrap();
            match c {
                TermChunk::Data(d) => g.0.extend(d),
                TermChunk::Exit(code) => g.1 = Some(code),
            }
        })
    }
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap().0).into_owned()
    }
    fn len(&self) -> usize {
        self.0.lock().unwrap().0.len()
    }
    fn exit(&self) -> Option<Option<i32>> {
        self.0.lock().unwrap().1
    }
}

fn wait_for(what: &str, secs: u64, f: impl Fn() -> bool) {
    let end = Instant::now() + Duration::from_secs(secs);
    while !f() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn what_is_typed_comes_back_through_the_pty() {
    let t = Terms::new();
    let id = t.open(spec(&["/bin/cat"])).unwrap();
    let got = Got::default();
    t.attach(&id, got.sink()).unwrap();
    t.write(&id, b"hello-pty\n").unwrap();
    wait_for("the echo", 5, || got.text().matches("hello-pty").count() >= 2); // the tty echo and cat's copy
    t.close(&id).unwrap();
}

#[test]
fn resize_changes_what_stty_size_says() {
    let t = Terms::new();
    let id = t.open(spec(&["/bin/sh"])).unwrap();
    let got = Got::default();
    t.attach(&id, got.sink()).unwrap();
    t.resize(&id, 120, 40).unwrap();
    t.write(&id, b"stty size\n").unwrap();
    wait_for("40 120", 5, || got.text().contains("40 120"));
    t.close(&id).unwrap();
}

#[test]
fn attach_after_output_returns_the_buffer_and_the_snapshot_keeps_text_and_folder() {
    let t = Terms::new();
    let dir = tempfile::tempdir().unwrap();
    let mut s = spec(&["/bin/sh", "-c", "echo before-attach; sleep 5"]);
    s.cwd = Some(dir.path().to_path_buf());
    s.title = "probe".into();
    let id = t.open(s).unwrap();
    wait_for("the output in the buffer", 5, || t.snapshot().iter().any(|x| x.text.contains("before-attach")));
    let buf = t.attach(&id, Got::default().sink()).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("before-attach"));
    let snap = t.snapshot();
    assert_eq!(snap.len(), 1);
    assert_eq!((snap[0].title.as_str(), std::path::Path::new(&snap[0].cwd)), ("probe", dir.path()));
    t.close(&id).unwrap();
    assert!(t.list().is_empty());
}

#[test]
fn the_end_of_the_process_gives_its_code() {
    let t = Terms::new();
    let id = t.open(spec(&["/bin/sh", "-c", "exit 3"])).unwrap();
    let got = Got::default();
    t.attach(&id, got.sink()).unwrap();
    wait_for("the exit", 5, || got.exit().is_some());
    assert_eq!(got.exit(), Some(Some(3)));
    let info = t.list().into_iter().find(|i| i.id == id).unwrap();
    assert_eq!((info.running, info.exit), (false, Some(3)));
}

#[test]
fn without_ack_reading_stops_and_ack_lets_it_go_on() {
    let t = Terms::new();
    let id = t.open(spec(&["/usr/bin/yes"])).unwrap();
    let got = Got::default();
    t.attach(&id, got.sink()).unwrap();
    wait_for("1 MB of output", 10, || got.len() >= 1 << 20);
    std::thread::sleep(Duration::from_millis(300));
    let first = got.len();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(got.len(), first, "nothing more without an ack");
    assert!(first <= (1 << 20) + (256 << 10), "stopped near 1 MB, got {first}");
    t.ack(&id, first).unwrap();
    wait_for("more after the ack", 5, || got.len() > first);
    t.close(&id).unwrap();
}

#[test]
fn the_pty_gets_no_secrets_of_the_app() {
    std::env::set_var("NIGHTSHIFT_TEST_INNER", "inner-value");
    let t = Terms::new();
    let mut s = spec(&["/usr/bin/env"]);
    s.env = vec![("OPENAI_API_KEY".into(), "kit-secret-value".into()), ("GH_TOKEN".into(), "kit-token-value".into()), ("CODEX_HOME".into(), "/tmp/codex-home".into())];
    let id = t.open(s).unwrap();
    let got = Got::default();
    t.attach(&id, got.sink()).unwrap();
    wait_for("the exit", 5, || got.exit().is_some());
    let text = got.text();
    assert!(text.contains("CODEX_HOME=/tmp/codex-home") && text.contains("TERM=xterm-256color"), "{text}");
    for leak in ["OPENAI_API_KEY", "kit-secret-value", "GH_TOKEN", "kit-token-value", "NIGHTSHIFT_TEST_INNER", "inner-value"] {
        assert!(!text.contains(leak), "{leak} reached the pty");
    }
}

#[test]
fn ctrl_c_gets_through_while_reading_waits_for_an_ack() {
    let t = Arc::new(Terms::new());
    let id = t.open(spec(&["/bin/sh"])).unwrap();
    let got = Got::default();
    t.attach(&id, got.sink()).unwrap();
    t.write(&id, b"yes\n").unwrap();
    wait_for("1 MB of output", 10, || got.len() >= 1 << 20);
    let (t2, id2) = (t.clone(), id.clone());
    let w = std::thread::spawn(move || t2.write(&id2, b"\x03").unwrap());
    wait_for("the write of Ctrl-C", 2, || w.is_finished());
    // the window catches up: the rest of `yes` and the prompt after ^C come
    wait_for("^C", 5, || {
        t.ack(&id, 1 << 30).unwrap();
        got.text().ends_with("$ ") || got.text().ends_with("# ")
    });
    t.close(&id).unwrap();
}

#[test]
fn a_sink_that_reads_the_list_on_exit_does_not_hang_the_session() {
    let t = Arc::new(Terms::new());
    let id = t.open(spec(&["/bin/sh", "-c", "sleep 0.3; exit 4"])).unwrap();
    let seen: Arc<Mutex<Option<usize>>> = Arc::default();
    let (t2, s2) = (t.clone(), seen.clone());
    // what the app does on exit: the tray counts the live sessions
    t.attach(&id, Box::new(move |c| if let TermChunk::Exit(_) = c { *s2.lock().unwrap() = Some(t2.list().len()) })).unwrap();
    wait_for("the sink to see the list", 5, || seen.lock().unwrap().is_some());
    let (t3, done) = (t.clone(), Arc::new(Mutex::new(false)));
    let d2 = done.clone();
    std::thread::spawn(move || { let _ = t3.list(); *d2.lock().unwrap() = true; });
    wait_for("list() after the exit", 2, || *done.lock().unwrap());
    // and an attach after the exit may also read the list from its sink
    let (t4, s4) = (t.clone(), Arc::new(Mutex::new(false)));
    let s5 = s4.clone();
    t.attach(&id, Box::new(move |_| { let _ = t4.list(); *s5.lock().unwrap() = true; })).unwrap();
    assert!(*s4.lock().unwrap());
}
