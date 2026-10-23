//! F15: "Проверить как пользователь" (off by default, never fails the task, only local addresses) and the address
//! check the preview window shares with it.
mod common;
use common::*;
use nightshift_core::model::*;
use nightshift_core::notify::RecordingNotifier;
use nightshift_core::preview::{self, Capture, NodeCapture};
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Writes three fake PNGs and records each call; `missing` plays a machine without the tool.
#[derive(Default)]
struct FakeCapture {
    missing: bool,
    calls: Mutex<Vec<String>>,
}
impl Capture for FakeCapture {
    fn available(&self, _cwd: &Path) -> Result<(), String> {
        if self.missing { Err(preview::NO_TOOL.into()) } else { Ok(()) }
    }
    fn shoot(&self, url: &str, _cwd: &Path, out: &Path) -> Result<Vec<PathBuf>, String> {
        self.calls.lock().unwrap().push(url.to_string());
        Ok((1..=3).map(|n| {
            let f = out.join(format!("{n}.png"));
            fs::write(&f, b"\x89PNG fake").unwrap();
            f
        }).collect())
    }
}

/// One task through the short path with the given project and capture; the finished task and the capture.
fn run(env: &Env, setup: impl FnOnce(&mut Project), capture: Option<Arc<FakeCapture>>) -> (Task, Option<Arc<FakeCapture>>, PathBuf) {
    let (t, c, d, _) = run_e(env, setup, capture);
    (t, c, d)
}
fn run_e(env: &Env, setup: impl FnOnce(&mut Project), capture: Option<Arc<FakeCapture>>) -> (Task, Option<Arc<FakeCapture>>, PathBuf, Arc<nightshift_core::engine::Engine>) {
    let p = env.project(setup);
    let e = env.engine(env.store(&p, |_| {}), vec![fake(vec![vec![write("f.txt", "1"), done("done")]]), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let dir = env.d.path().join("proof");
    if let Some(c) = &capture {
        e.set_proof(c.clone(), dir.clone());
    }
    let id = submit(&e, &p, "Add f");
    (wait_status(&e, &id, Status::Done), capture, dir, e)
}

#[test]
fn the_step_is_off_by_default_and_nothing_is_started_or_shot() {
    let env = Env::new();
    assert!(!Project::new("x", env.repo.clone(), "main").proof, "off in a new project");
    let old: Project = serde_json::from_str(r#"{"id":"p","name":"n","path":"/x","base_branch":"main","check_cmd":null,"setup_cmd":null}"#).unwrap();
    assert!(!old.proof && old.app_url.is_none(), "a project saved before F15 reads as off");
    let marker = env.d.path().join("started");
    let (t, cap, _) = run(&env, |p| p.run_cmd = Some(format!("touch '{}'", marker.display())), Some(Arc::new(FakeCapture::default())));
    assert!(t.outcome.proof.is_none());
    assert!(cap.unwrap().calls.lock().unwrap().is_empty());
    assert!(!marker.exists(), "run_cmd was not started");
}

#[test]
fn without_a_capture_tool_the_step_is_skipped_with_a_note_and_the_task_still_lands() {
    let env = Env::new();
    let on = |p: &mut Project| {
        p.proof = true;
        p.run_cmd = Some("sleep 30".into());
        p.app_url = Some("http://localhost:9".into());
    };
    // nothing bound at all
    let (t, _, _) = run(&env, on, None);
    assert_eq!(t.outcome.proof, Some(Proof { shots: vec![], skipped: Some("no_capture_tool".into()) }));
    assert!(t.outcome.commit.is_some(), "merged as usual");
    // a tool that is not on this machine
    let env = Env::new();
    let (t, cap, _) = run(&env, on, Some(Arc::new(FakeCapture { missing: true, ..Default::default() })));
    assert_eq!(t.outcome.proof.unwrap().skipped.as_deref(), Some("no_capture_tool"));
    assert!(cap.unwrap().calls.lock().unwrap().is_empty());
}

#[test]
fn an_address_that_is_not_local_is_refused_and_nothing_is_shot() {
    let env = Env::new();
    let (t, cap, _) = run(&env, |p| {
        p.proof = true;
        p.run_cmd = Some("sleep 30".into());
        p.app_url = Some("http://example.com/".into());
    }, Some(Arc::new(FakeCapture::default())));
    assert_eq!(t.outcome.proof.unwrap().skipped.as_deref(), Some("not_local_url"));
    assert!(cap.unwrap().calls.lock().unwrap().is_empty());
}

#[test]
fn an_app_that_does_not_come_up_is_a_note_not_a_failure() {
    let env = Env::new();
    let free = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port(); // dropped: nothing listens there
    let (t, cap, _) = run(&env, |p| {
        p.proof = true;
        p.run_cmd = Some("exit 1".into());
        p.app_url = Some(format!("http://127.0.0.1:{free}/"));
    }, Some(Arc::new(FakeCapture::default())));
    assert_eq!(t.outcome.proof.unwrap().skipped.as_deref(), Some("app_not_started"));
    assert!(cap.unwrap().calls.lock().unwrap().is_empty());
}

#[test]
fn with_the_app_up_the_shots_go_onto_the_task_and_the_app_is_stopped() {
    let env = Env::new();
    // the "app": the test listens on the port; run_cmd stays up and says its pid
    let srv = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = srv.local_addr().unwrap().port();
    let pid_file = env.d.path().join("pid");
    let ready = env.d.path().join("go");
    // the port must look free before run_cmd starts: the listener answers only once run_cmd has written the pid
    let addr = srv.local_addr().unwrap();
    drop(srv);
    let (go, pf) = (ready.clone(), pid_file.clone());
    let server = std::thread::spawn(move || {
        until("run_cmd started", || pf.exists());
        let l = TcpListener::bind(addr).unwrap();
        fs::write(&go, "").unwrap();
        let _ = l.accept();
        std::thread::sleep(std::time::Duration::from_secs(3));
    });
    let (t, cap, dir, e) = run_e(&env, |p| {
        p.proof = true;
        p.run_cmd = Some(format!("echo $$ > '{}'; exec sleep 30", pid_file.display()));
        p.app_url = Some(format!("http://127.0.0.1:{port}"));
    }, Some(Arc::new(FakeCapture::default())));
    let proof = t.outcome.proof.clone().unwrap();
    assert_eq!(proof, Proof { shots: vec!["1.png".into(), "2.png".into(), "3.png".into()], skipped: None });
    assert_eq!(cap.unwrap().calls.lock().unwrap().clone(), vec![format!("http://127.0.0.1:{port}/")]);
    for f in &proof.shots {
        assert!(dir.join(&t.id).join(f).is_file(), "{f} kept in the data folder");
    }
    assert!(!g(&env.repo, &["show", "--stat", "main"]).contains(".png"), "no shot lands in the commit");
    let pid: u32 = fs::read_to_string(&pid_file).unwrap().trim().parse().unwrap();
    until("the app is stopped", || !alive(pid));
    server.join().unwrap();
    e.remove(&t.id).unwrap();
    assert!(!dir.join(&t.id).exists(), "removing the task removes its shots");
}

#[test]
fn only_http_localhost_and_127_0_0_1_pass_the_address_check() {
    for (input, want) in [
        ("http://localhost:3000", "http://localhost:3000/"),
        ("http://LOCALHOST:5173/app?x=1#y", "http://localhost:5173/app?x=1#y"),
        ("  http://127.0.0.1/  ", "http://127.0.0.1/"),
        ("HTTP://127.0.0.1:8080/a", "http://127.0.0.1:8080/a"),
    ] {
        assert_eq!(preview::local_url(input).as_deref(), Ok(want), "{input}");
    }
    for bad in [
        "http://example.com", "https://localhost:3000", "http://localhost.evil.com/", "http://localhost@evil.com/",
        "http://evil.com@localhost/", "http://localhost:3000@evil.com/", "http://127.0.0.2/", "http://[::1]:3000/",
        "file:///etc/passwd", "javascript:alert(1)", "http://localhost:99999/", "http://localhost:/", "http://localhost:3x/",
        "http://localhost\\@evil.com/", "http://local host/", "localhost:3000", "", "http://0.0.0.0:3000/",
    ] {
        assert_eq!(preview::local_url(bad), Err(preview::NOT_LOCAL), "{bad}");
    }
    assert_eq!(preview::host_port("http://127.0.0.1:8080/a"), ("127.0.0.1".to_string(), 8080));
    assert_eq!(preview::host_port("http://localhost/"), ("localhost".to_string(), 80));
}

#[test]
fn a_pick_from_the_page_is_capped_and_cleaned() {
    let p = preview::parse_pick(r#"{"selector":"main > button.cta","text":"  Купить\n  сейчас  ","w":120.4,"h":40}"#).unwrap();
    assert_eq!(p, preview::Pick { selector: "main > button.cta".into(), text: "Купить сейчас".into(), w: 120, h: 40 });
    let long = preview::parse_pick(&format!(r#"{{"selector":"{}","text":"{}\u0007","w":-5,"h":1e12}}"#, "a".repeat(1000), "b".repeat(1000))).unwrap();
    assert!(long.selector.chars().count() <= 300 && long.text.chars().count() <= 160 && !long.text.contains('\u{7}'));
    assert_eq!((long.w, long.h), (0, 100_000));
    for bad in ["", "null", "[]", r#"{"selector":""}"#, r#"{"selector":5}"#, "not json"] {
        assert!(preview::parse_pick(bad).is_none(), "{bad}");
    }
}

#[test]
fn a_machine_without_node_has_no_capture_tool() {
    let c = NodeCapture { node: "/nonexistent/node".into() };
    assert_eq!(c.available(Path::new("/")), Err(preview::NO_TOOL.to_string()));
}
