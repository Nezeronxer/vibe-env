//! v6 task 29: the chat as a shell of the CLI. The menu "/" (what the env has), "В терминале" for every env (the
//! words that continue a session, the chat marked, the talk of the terminal read back), images attached to a
//! message (kept in the app's data, 0600, gone with the chat) and the tokens of a run in the spending history.
mod common;
use common::*;
use nightshift_core::agent::adapters::{Claude, Codex, Cursor, Custom, Gemini, OpenCode, Qwen};
use nightshift_core::agent::{inventory, Adapter, AgentEvent, FakeAdapter, FakeStep, Run};
use nightshift_core::chat::{ChatTarget, Chats, MAX_ATTACHMENTS, MAX_ATTACHMENT_BYTES};
use nightshift_core::engine::Engine;
use nightshift_core::model::*;
use nightshift_core::notify::RecordingNotifier;
use nightshift_core::store::Store;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---------------- "В терминале": the words of each env (spec v6, table «Среды для «/» и «В терминале»») ----------------

fn last3(v: Option<Vec<String>>) -> Option<Vec<String>> {
    v.map(|v| v[1..].to_vec())
}

#[test]
fn every_env_continues_a_session_with_the_words_of_the_table_and_a_bad_id_gets_none() {
    let cwd = Path::new("/tmp");
    let s = "a1b2-c3";
    let words = |a: &dyn Adapter| last3(a.resume_argv(s, cwd));
    assert_eq!(words(&Claude::with_bin("claude-x")), Some(vec!["--resume".into(), s.into()]));
    assert_eq!(words(&Codex::with_bin("codex-x")), Some(vec!["resume".into(), s.into()]));
    assert_eq!(words(&Gemini::with_bin("gemini-x")), Some(vec!["--resume".into(), s.into()]));
    assert_eq!(words(&Qwen::with_bin("qwen-x")), Some(vec!["--resume".into(), s.into()]));
    assert_eq!(words(&OpenCode::with_bin("opencode-x")), Some(vec!["--session".into(), s.into()]));
    assert_eq!(words(&Cursor::with_bin("cursor-agent-x")), Some(vec!["--resume".into(), s.into()]));
    // the program comes first, as configured (not found on PATH: the name itself)
    assert_eq!(Claude::with_bin("claude-x").resume_argv(s, cwd).unwrap()[0], "claude-x");
    let own = Custom(CustomAgent { id: "mine".into(), name: "Mine".into(), run: "my-cli {prompt}".into(), resume: None });
    assert_eq!(own.resume_argv(s, cwd), None, "an own command has no session to continue");
    for bad in ["--help", "a b", "x;rm -rf ~", ""] {
        assert_eq!(Claude::new().resume_argv(bad, cwd), None, "{bad:?} must never reach a terminal");
        assert_eq!(OpenCode::new().resume_argv(bad, cwd), None);
    }
    // checked on this machine: claude, qwen, opencode; the others are from the docs
    assert!(Claude::new().resume_checked() && Qwen::new().resume_checked() && OpenCode::new().resume_checked());
    assert!(!Codex::new().resume_checked() && !Gemini::new().resume_checked() && !Cursor::new().resume_checked());
}

// ---------------- images: who reads them, how they are passed ----------------

#[test]
fn claude_codex_and_opencode_take_images_the_others_do_not() {
    assert!(Claude::new().supports_images());
    assert!(Codex::new().supports_images());
    assert!(OpenCode::new().supports_images());
    assert!(!Gemini::new().supports_images());
    assert!(!Qwen::new().supports_images());
    assert!(!Cursor::new().supports_images());
}

fn args_of(c: &std::process::Command) -> Vec<String> {
    c.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
}

#[test]
fn an_attached_image_goes_to_the_cli_as_a_path_in_the_prompt_and_its_own_flag() {
    let img = PathBuf::from("/data/attachments/c1/shot.png");
    let mut run = Run::new(PathBuf::from("/tmp"), "What is wrong here?".into(), String::new(), RoleKind::Chat);
    run.attachments = vec![img.clone()];
    let claude = args_of(&Claude::new().command(&run));
    let prompt = claude.last().unwrap();
    assert!(prompt.starts_with("What is wrong here?") && prompt.contains("/data/attachments/c1/shot.png"), "{prompt}");
    assert!(claude.iter().any(|a| a == "--add-dir=/data/attachments/c1"), "Claude may read the folder: {claude:?}");
    let codex = args_of(&Codex::new().command(&run));
    assert!(codex.iter().any(|a| a == "--image=/data/attachments/c1/shot.png"), "{codex:?}");
    let oc = args_of(&OpenCode::new().command(&run));
    assert!(oc.iter().any(|a| a == "--file=/data/attachments/c1/shot.png"), "{oc:?}");
    // without images nothing changes
    run.attachments.clear();
    let plain = args_of(&Claude::new().command(&run));
    assert_eq!(plain.last().unwrap(), "What is wrong here?");
    assert!(!plain.iter().any(|a| a.starts_with("--add-dir")));
}

// ---------------- tokens of a run (G70, the request of task 28) ----------------

fn tokens(a: &dyn Adapter, line: &str) -> Vec<(u64, u64)> {
    a.parse(line).into_iter().filter_map(|e| if let AgentEvent::Tokens { input, output } = e { Some((input, output)) } else { None }).collect()
}

#[test]
fn each_cli_reports_the_tokens_it_says() {
    // Claude: modelUsage of the result, the cache counted as input (live 2.1.308 shape, fixture claude.jsonl)
    let fx = fs::read_to_string("tests/fixtures/agents/claude.jsonl").unwrap();
    let result = fx.lines().find(|l| l.contains("\"type\":\"result\"")).unwrap();
    assert_eq!(tokens(&Claude::new(), result), vec![(4 + 50837 + 31725, 78)]);
    // Qwen: usage of the result; a result of nothing used says nothing
    assert_eq!(tokens(&Qwen::new(), r#"{"type":"result","subtype":"success","session_id":"q","is_error":false,"result":"ok","usage":{"input_tokens":10,"output_tokens":2}}"#), vec![(10, 2)]);
    assert!(tokens(&Qwen::new(), r#"{"type":"result","subtype":"error_during_execution","session_id":"q","is_error":true,"usage":{"input_tokens":0,"output_tokens":0}}"#).is_empty());
    // Codex: turn.completed (docs shape, fixture codex-from-docs.jsonl)
    assert_eq!(tokens(&Codex::new(), r#"{"type":"turn.completed","usage":{"input_tokens":24763,"cached_input_tokens":24448,"output_tokens":122,"reasoning_output_tokens":0}}"#), vec![(24763, 122)]);
    // opencode: each step_finish (live 1.18.34 shape): input + cache, output + reasoning
    let oc = r#"{"type":"step_finish","sessionID":"s","part":{"type":"step-finish","tokens":{"total":71984,"input":65800,"output":20,"reasoning":20,"cache":{"write":0,"read":6144}},"cost":0}}"#;
    assert_eq!(tokens(&OpenCode::new(), oc), vec![(65800 + 6144, 40)]);
    // Gemini: stats with input and output only (the docs' sample gives only a total: nothing)
    assert_eq!(tokens(&Gemini::new(), r#"{"type":"result","status":"success","stats":{"total_tokens":12,"input_tokens":9,"output_tokens":3}}"#), vec![(9, 3)]);
    assert!(tokens(&Gemini::new(), r#"{"type":"result","status":"success","stats":{"total_tokens":10}}"#).is_empty());
}

#[test]
fn the_tokens_of_a_task_reach_the_spending_history() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = fake(vec![vec![write("f.txt", "x\n"), FakeStep::Cost(0.1), FakeStep::Tokens(1000, 50), FakeStep::Tokens(200, 5), done("Added f.txt")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add a file");
    wait_status(&e, &id, Status::Done);
    let rows: Vec<_> = e.spend_history(None, None).into_iter().filter(|r| r.env == "fake").collect();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!((rows[0].tokens_in, rows[0].tokens_out), (Some(1200), Some(55)));
}

// ---------------- the chat: one app over a temp folder ----------------

struct App {
    e: Arc<Engine>,
    chats: Arc<Chats>,
    fa: Arc<FakeAdapter>,
}

fn start(env: &Env, p: &Project, fa: FakeAdapter) -> App {
    let fa = Arc::new(fa);
    let mut st = Store::open(&env.d.path().join("data")).unwrap();
    st.upsert_project(p.clone()).unwrap();
    let mut s = st.settings();
    s.quality.on = false;
    s.envs.entry(fa.id().to_string()).or_default();
    st.set_settings(s).unwrap();
    let e = Engine::new(st, vec![fa.clone()], Arc::new(RecordingNotifier::default()), env.d.path().join("data").join("worktrees"));
    let chats = Chats::new(e.clone());
    App { e, chats, fa }
}

impl App {
    fn feed(&self, chat: &str) -> Vec<ChatMsg> {
        self.chats.messages(chat, None, 200)
    }
    fn wait_feed(&self, chat: &str, what: &str, pred: impl Fn(&[ChatMsg]) -> bool) -> Vec<ChatMsg> {
        let t = Instant::now();
        loop {
            let m = self.feed(chat);
            if pred(&m) {
                return m;
            }
            assert!(t.elapsed() < Duration::from_secs(30), "timeout waiting for {what}: {:#?}", m.iter().map(|x| (&x.who, &x.text)).collect::<Vec<_>>());
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn chat_runs(&self) -> Vec<Run> {
        self.fa.calls().into_iter().filter(|r| r.role == RoleKind::Chat).collect()
    }
}

fn chat_fake(scripts: Vec<Vec<FakeStep>>) -> FakeAdapter {
    FakeAdapter::new(vec![]).with_role(RoleKind::Chat, scripts)
}

/// The smallest PNG there is (1x1, from the spec of the format), and a JPEG start.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15,
    0xC4, 0x89,
];
const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46];

fn mode(p: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(p).unwrap().permissions().mode() & 0o777
}

#[test]
fn an_image_is_kept_in_the_apps_data_0600_goes_to_the_agent_as_a_path_and_is_gone_with_the_chat() {
    let env = Env::new();
    let p = env.project(|_| {});
    let app = start(&env, &p, chat_fake(vec![vec![done("A red button.")]]).with_images(true));
    // pasted before the chat exists: it waits in the app's data, never in the project
    let a = app.chats.attach(None, PNG).unwrap();
    assert!(a.path.starts_with(app.chats.attachments_dir()), "{:?}", a.path);
    assert!(a.path.starts_with(env.d.path().join("data").join("attachments")));
    assert_eq!(a.path.extension().unwrap(), "png");
    assert_eq!(mode(&a.path), 0o600);
    let chat = app.chats.send_with(ChatTarget::New { env: "fake".into(), project_id: p.id.clone() }, "m1", "What is on the screenshot?", &[a.path.clone()]).unwrap();
    app.wait_feed(&chat, "the answer", |m| m.iter().any(|x| x.who == Who::Model));
    // the run got the file of the chat's own folder
    let run = app.chat_runs().pop().unwrap();
    assert_eq!(run.attachments.len(), 1);
    let kept = &run.attachments[0];
    assert!(kept.starts_with(app.chats.attachments_dir().join(&chat)), "{kept:?}");
    assert!(kept.is_file() && mode(kept) == 0o600);
    assert!(!a.path.exists(), "moved, not copied");
    // the person's message keeps the list (the feed shows it)
    let me = app.feed(&chat).into_iter().find(|m| m.id == "m1").unwrap();
    assert_eq!(me.attachments, vec![kept.clone()]);
    // nothing of it in the project's git
    assert!(g(&env.repo, &["status", "--porcelain"]).is_empty());
    // deleting the chat deletes its images
    app.chats.remove(&chat).unwrap();
    assert!(!app.chats.attachments_dir().join(&chat).exists());
}

#[test]
fn not_an_image_too_big_too_many_a_foreign_path_or_an_env_without_images_are_refused() {
    let env = Env::new();
    let p = env.project(|_| {});
    let app = start(&env, &p, chat_fake(vec![vec![done("ok")]]).with_images(true));
    assert_eq!(app.chats.attach(None, b"just some text, not a picture").unwrap_err(), "not_an_image");
    let mut big = PNG.to_vec();
    big.resize(MAX_ATTACHMENT_BYTES + 1, 0);
    assert_eq!(app.chats.attach(None, &big).unwrap_err(), "image_too_big");
    // a dropped file is read by path, the same checks
    let txt = env.d.path().join("notes.txt");
    fs::write(&txt, "hello").unwrap();
    assert_eq!(app.chats.attach_file(None, &txt).unwrap_err(), "not_an_image");
    let jpg = env.d.path().join("shot.jpg");
    fs::write(&jpg, JPEG).unwrap();
    let j = app.chats.attach_file(None, &jpg).unwrap();
    assert_eq!(j.path.extension().unwrap(), "jpg");
    assert!(jpg.exists(), "the person's file stays where it was");
    // six at once
    let six: Vec<PathBuf> = (0..MAX_ATTACHMENTS + 1).map(|_| app.chats.attach(None, PNG).unwrap().path).collect();
    let target = ChatTarget::New { env: "fake".into(), project_id: p.id.clone() };
    assert_eq!(app.chats.send_with(target.clone(), "m1", "look", &six).unwrap_err(), "too_many_images");
    // a path outside the app's attachments is never passed on
    assert_eq!(app.chats.send_with(target.clone(), "m2", "look", &[jpg.clone()]).unwrap_err(), "attachment_not_found");
    // removed with ×
    app.chats.detach(&six[0]).unwrap();
    assert!(!six[0].exists());
    assert!(app.chats.detach(&jpg).is_err(), "only the app's own files are removed");
    assert!(jpg.exists());

    // an env that does not read images
    let env2 = Env::new();
    let p2 = env2.project(|_| {});
    let app2 = start(&env2, &p2, chat_fake(vec![vec![done("ok")]]));
    let a = app2.chats.attach(None, PNG).unwrap();
    let r = app2.chats.send_with(ChatTarget::New { env: "fake".into(), project_id: p2.id.clone() }, "m1", "look", &[a.path]);
    assert_eq!(r.unwrap_err(), "images_not_supported");
}

#[test]
fn a_queued_message_keeps_its_images() {
    let env = Env::new();
    let p = env.project(|_| {});
    let app = start(&env, &p, chat_fake(vec![vec![FakeStep::Sleep(600), done("first")], vec![done("second")]]).with_images(true));
    let chat = app.chats.send(ChatTarget::New { env: "fake".into(), project_id: p.id.clone() }, "m1", "first question").unwrap();
    let a = app.chats.attach(Some(&chat), PNG).unwrap();
    app.chats.queue_add_with(&chat, "m2", "and this screenshot?", false, &[a.path.clone()]).unwrap();
    app.wait_feed(&chat, "two answers", |m| m.iter().filter(|x| x.who == Who::Model).count() == 2);
    let runs = app.chat_runs();
    assert_eq!(runs.last().unwrap().attachments, vec![a.path], "an image of the chat's folder stays where it is");
}

// ---------------- the chat in the terminal and back ----------------

fn key(p: &Path) -> String {
    nightshift_core::sessions::key_for(p)
}

#[test]
fn the_terminal_continues_the_session_in_the_chats_copy_and_marks_the_chat() {
    let env = Env::new();
    let p = env.project(|_| {});
    let app = start(&env, &p, chat_fake(vec![vec![done("Hi.")]]));
    let chat = app.chats.send(ChatTarget::New { env: "fake".into(), project_id: p.id.clone() }, "m1", "hello").unwrap();
    app.wait_feed(&chat, "the answer", |m| m.iter().any(|x| x.who == Who::Model));
    let session = app.e.store().lock().unwrap().chat(&chat).unwrap().session.unwrap();
    let l = app.chats.terminal_open(&chat, false).unwrap();
    assert!(l.resumed);
    assert_eq!(l.argv, vec!["fake-cli".to_string(), "--resume".into(), session.clone()]);
    assert!(l.cwd.ends_with(&chat) && l.cwd.join("a.txt").exists(), "the chat's copy of the project: {:?}", l.cwd);
    assert!(app.chats.list("fake")[0].in_terminal, "the list says the chat is open in the terminal");
    let feed = app.feed(&chat);
    assert!(feed.iter().any(|m| m.text.starts_with("opened_in_terminal")), "{:?}", feed.iter().map(|m| &m.text).collect::<Vec<_>>());
    // back: another env than Claude cannot be read - one line, and the same session goes on
    let n = app.chats.terminal_closed(&chat).unwrap();
    assert_eq!(n, 0);
    assert!(app.feed(&chat).iter().any(|m| m.text == "continued_in_terminal"));
    assert!(!app.chats.list("fake")[0].in_terminal);
    assert_eq!(app.e.store().lock().unwrap().chat(&chat).unwrap().session, Some(session));
    assert_eq!(app.chats.terminal_closed(&chat).unwrap(), 0, "closing twice does nothing");
}

#[test]
fn without_a_session_or_a_cli_the_person_gets_a_reason_and_may_start_a_new_conversation() {
    let env = Env::new();
    let p = env.project(|_| {});
    let app = start(&env, &p, chat_fake(vec![vec![done("ok")]]));
    // a chat that never got an answer has no session yet
    let chat = app.chats.send(ChatTarget::New { env: "fake".into(), project_id: p.id.clone() }, "m1", "hello").unwrap();
    app.wait_feed(&chat, "the answer", |m| m.iter().any(|x| x.who == Who::Model));
    let mut c = app.e.store().lock().unwrap().chat(&chat).unwrap();
    c.session = None;
    app.e.store().lock().unwrap().upsert_chat(&c).unwrap();
    assert_eq!(app.chats.terminal_open(&chat, false).unwrap_err(), "no_session");
    let l = app.chats.terminal_open(&chat, true).unwrap();
    assert!(!l.resumed);
    assert_eq!(l.argv, Vec::<String>::new(), "the fake has no CLI on disk: a shell in the copy");
    assert_eq!(app.chats.terminal_open("nope", true).unwrap_err(), "chat_not_found");
}

#[test]
fn a_claude_session_continued_in_the_terminal_is_read_back_into_the_feed() {
    let env = Env::new();
    let p = env.project(|_| {});
    let app = start(&env, &p, chat_fake(vec![vec![done("Hi.")], vec![done("Still here.")]]).with_id("claude"));
    let projects = env.d.path().join("claude-projects");
    app.chats.set_claude_projects(&projects);
    let chat = app.chats.send(ChatTarget::New { env: "claude".into(), project_id: p.id.clone() }, "m1", "hello").unwrap();
    app.wait_feed(&chat, "the answer", |m| m.iter().any(|x| x.who == Who::Model));
    let session = app.e.store().lock().unwrap().chat(&chat).unwrap().session.unwrap();
    let l = app.chats.terminal_open(&chat, false);
    assert_eq!(l.unwrap_err(), "session_not_found", "Claude finds the session by the folder it runs in");
    let copy = app.chats.terminal_open(&chat, true).unwrap().cwd;
    app.chats.terminal_closed(&chat).unwrap();
    // the CLI keeps the session under the key of the copy
    let dir = projects.join(key(&copy));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{session}.jsonl"));
    fs::write(&file, "{\"type\":\"user\",\"timestamp\":\"2020-01-01T00:00:00Z\",\"message\":{\"role\":\"user\",\"content\":\"old\"}}\n").unwrap();
    let l = app.chats.terminal_open(&chat, false).unwrap();
    assert!(l.resumed);
    let now = chrono::Utc::now().to_rfc3339();
    let lines = [
        format!(r#"{{"type":"user","uuid":"u1","timestamp":"{now}","message":{{"role":"user","content":"Что в main.rs?"}}}}"#),
        format!(r#"{{"type":"assistant","uuid":"a1","timestamp":"{now}","message":{{"role":"assistant","content":[{{"type":"thinking","thinking":"hm"}},{{"type":"text","text":"Точка входа."}}]}}}}"#),
        format!(r#"{{"type":"user","uuid":"u2","timestamp":"{now}","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"t","content":"x"}}]}}}}"#),
        format!(r#"{{"type":"assistant","uuid":"a2","isSidechain":true,"timestamp":"{now}","message":{{"role":"assistant","content":[{{"type":"text","text":"subagent"}}]}}}}"#),
        "not json".to_string(),
    ];
    let mut text = fs::read_to_string(&file).unwrap();
    text.push_str(&lines.join("\n"));
    text.push('\n');
    fs::write(&file, text).unwrap();
    assert_eq!(app.chats.terminal_closed(&chat).unwrap(), 2);
    let feed = app.feed(&chat);
    let talk: Vec<(Who, String)> = feed.iter().filter(|m| m.who != Who::System && m.action.is_none()).map(|m| (m.who.clone(), m.text.clone())).collect();
    assert_eq!(feed.last().unwrap().text, "back_from_terminal: 2", "the feed says the chat is back and what came of the terminal");
    assert_eq!(talk[talk.len() - 2..].to_vec(), vec![(Who::Human, "Что в main.rs?".to_string()), (Who::Model, "Точка входа.".to_string())]);
    assert!(!feed.iter().any(|m| m.text == "old" || m.text == "subagent"));
    // the chat goes on with the same session
    app.chats.send(ChatTarget::Existing(chat.clone()), "m2", "and more?").unwrap();
    app.wait_feed(&chat, "the second answer", |m| m.iter().any(|x| x.who == Who::Model && x.reply_to.as_deref() == Some("m2")));
    assert_eq!(app.chat_runs().last().unwrap().resume, Some(session));
}

// ---------------- the menu "/" ----------------

fn put(p: &Path, text: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn names(v: &[inventory::InvItem]) -> Vec<String> {
    v.iter().map(|i| i.name.clone()).collect()
}

#[test]
fn the_menu_shows_the_skills_commands_mcp_and_mods_of_claude_code() {
    let d = tempfile::tempdir().unwrap();
    let (home, proj) = (d.path().join("home"), d.path().join("proj"));
    put(&home.join(".claude/skills/review/SKILL.md"), "---\nname: review\ndescription: Reviews the diff\n---\nbody");
    put(&home.join(".claude/skills/notes/SKILL.md"), "# Notes skill\nTakes notes");
    put(&home.join(".claude/skills/broken/README.md"), "no SKILL.md");
    put(&home.join(".claude/commands/deploy.md"), "---\ndescription: Ship it\n---\n");
    put(&home.join(".claude/commands/git/sync.md"), "Sync the branch");
    put(&proj.join(".claude/commands/local.md"), "Only here");
    let plug = home.join(".claude/plugins/cache/pony/1.0");
    put(&plug.join("skills/lazy/SKILL.md"), "---\ndescription: Be lazy\n---\n");
    put(&plug.join("commands/help.md"), "Help");
    let off = home.join(".claude/plugins/cache/off/1.0");
    put(&off.join("skills/hidden/SKILL.md"), "---\ndescription: x\n---\n");
    put(
        &home.join(".claude/plugins/installed_plugins.json"),
        &format!(r#"{{"version":2,"plugins":{{"pony@m":[{{"installPath":"{}"}}],"off@m":[{{"installPath":"{}"}}]}}}}"#, plug.display(), off.display()),
    );
    put(&home.join(".claude/mods/tolya-text/.claude-plugin/plugin.json"), r#"{"name":"tolya-text","description":"Edits the answer"}"#);
    put(
        &home.join(".claude/settings.json"),
        r#"{"enabledPlugins":{"pony@m":true,"off@m":false},"env":{"CLAUDE_CODE_PLUGIN_DIRS":"~/.claude/mods/tolya-text","SECRET_TOKEN":"tok-1"}}"#,
    );
    put(&home.join(".claude.json"), r#"{"mcpServers":{"github":{"command":"/usr/bin/gh-mcp","args":["--token","tok-2"]}}}"#);
    put(&proj.join(".mcp.json"), r#"{"mcpServers":{"db":{"url":"https://db.example/mcp?key=tok-3"}}}"#);

    let inv = inventory::scan("claude", &home, &proj);
    assert_eq!(names(&inv.skills), vec!["notes", "pony:lazy", "review"]);
    assert_eq!(inv.skills.iter().find(|s| s.name == "review").unwrap().description, "Reviews the diff");
    assert_eq!(inv.skills.iter().find(|s| s.name == "notes").unwrap().description, "Notes skill");
    assert_eq!(inv.skills.iter().find(|s| s.name == "pony:lazy").unwrap().source, "plugin:pony");
    assert_eq!(names(&inv.commands), vec!["deploy", "git:sync", "local", "pony:help"]);
    assert_eq!(inv.commands.iter().find(|s| s.name == "local").unwrap().source, "project");
    assert_eq!(names(&inv.mcp), vec!["db", "github"]);
    assert_eq!(names(&inv.mods), vec!["tolya-text"]);
    assert_eq!(inv.mods[0].description, "Edits the answer");
    let all = serde_json::to_string(&inv).unwrap();
    assert!(!all.contains("tok-"), "no argument, URL query or env value leaks into the menu: {all}");
    // the adapter's own inventory reads the same places (the person's HOME)
    assert_eq!(Claude::new().inventory(&proj), inventory::scan("claude", &inventory::home().unwrap(), &proj));
}

#[test]
fn the_menu_of_the_other_envs_comes_from_their_own_folders_and_an_own_command_has_none() {
    let d = tempfile::tempdir().unwrap();
    let (home, proj) = (d.path().join("home"), d.path().join("proj"));
    // Qwen (and Gemini the same way)
    put(&home.join(".qwen/skills/plan/SKILL.md"), "---\ndescription: Plans\n---\n");
    put(&home.join(".qwen/commands/fix.toml"), "description = \"Fix a bug\"\nprompt = \"...\"\n");
    put(&home.join(".qwen/commands/git/log.toml"), "description = \"Show log\"\n");
    put(&home.join(".qwen/settings.json"), r#"{"mcpServers":{"fs":{"command":"npx"}}}"#);
    put(&home.join(".qwen/extensions/conductor/qwen-extension.json"), r#"{"name":"conductor","description":"Tracks work"}"#);
    let q = inventory::scan("qwen", &home, &proj);
    assert_eq!(names(&q.skills), vec!["plan"]);
    assert_eq!(names(&q.commands), vec!["fix", "git:log"]);
    assert_eq!(q.commands[0].description, "Fix a bug");
    assert_eq!(names(&q.mcp), vec!["fs"]);
    assert_eq!(names(&q.mods), vec!["conductor"]);
    put(&home.join(".gemini/commands/test.toml"), "description = \"Run tests\"\n");
    assert_eq!(names(&inventory::scan("gemini", &home, &proj).commands), vec!["test"]);
    // opencode: commands/ and command/, skills, plugins, mcp of opencode.json
    put(&home.join(".config/opencode/commands/impeccable.md"), "---\ndescription: Polish UI\n---\n");
    put(&proj.join(".opencode/command/local.md"), "Local");
    put(&home.join(".config/opencode/skills/graphify/SKILL.md"), "---\ndescription: Graph\n---\n");
    put(&home.join(".config/opencode/plugins/agent-plugin.ts"), "export default {}");
    put(&proj.join("opencode.json"), r#"{"mcp":{"browser":{"type":"local"}}}"#);
    let o = inventory::scan("opencode", &home, &proj);
    assert_eq!(names(&o.commands), vec!["impeccable", "local"]);
    assert_eq!(names(&o.skills), vec!["graphify"]);
    assert_eq!(names(&o.mods), vec!["agent-plugin"]);
    assert_eq!(names(&o.mcp), vec!["browser"]);
    // Codex: skills of ~/.agents, prompts as /prompts:<name>, MCP tables of config.toml
    put(&home.join(".agents/skills/autopilot/SKILL.md"), "---\ndescription: Builds\n---\n");
    put(&home.join(".codex/prompts/review.md"), "Review");
    put(&home.join(".codex/config.toml"), "model = \"x\"\n[mcp_servers.github]\ncommand = \"gh\"\n[mcp_servers.github.env]\nTOKEN = \"tok\"\n");
    let c = inventory::scan("codex", &home, &proj);
    assert_eq!((names(&c.skills), names(&c.commands), names(&c.mcp)), (vec!["autopilot".to_string()], vec!["prompts:review".to_string()], vec!["github".to_string()]));
    // Cursor: rules and mcp.json
    put(&proj.join(".cursor/rules/style.mdc"), "---\ndescription: Code style\n---\n");
    put(&home.join(".cursor/mcp.json"), r#"{"mcpServers":{"linear":{"url":"https://x"}}}"#);
    let cu = inventory::scan("cursor", &home, &proj);
    assert_eq!((names(&cu.skills), names(&cu.mcp)), (vec!["style".to_string()], vec!["linear".to_string()]));
    // nothing: empty, never an error
    assert_eq!(inventory::scan("custom-x", &home, &proj), inventory::Inventory::default());
    let own = Custom(CustomAgent { id: "mine".into(), name: "Mine".into(), run: "my-cli {prompt}".into(), resume: None });
    assert_eq!(own.inventory(&proj), inventory::Inventory::default());
}
