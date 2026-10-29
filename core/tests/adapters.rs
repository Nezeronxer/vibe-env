//! Adapter seam: parse/finish on saved engine output (core/tests/fixtures/agents), command lines,
//! availability, limit detection, own command. Files named `*-from-docs*` were assembled from the
//! engine's documentation, the others were captured from real runs on the dev machine.
use chrono::{Duration, Local, Timelike};
use nightshift_core::agent::adapters::*;
use nightshift_core::agent::actions;
use nightshift_core::agent::*;
use nightshift_core::model::{CustomAgent, Settings};
use std::sync::atomic::AtomicBool;

fn fixture(name: &str) -> String {
    let p = format!("{}/tests/fixtures/agents/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

/// An adapter whose process prints a saved fixture (and exits with `exit_ok`): the real `run_blocking` path.
struct Replay<'a> {
    inner: &'a dyn Adapter,
    path: String,
    exit_ok: bool,
}

impl Adapter for Replay<'_> {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn available(&self) -> Result<(), String> {
        Ok(())
    }
    fn command(&self, _run: &Run) -> std::process::Command {
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg(format!("cat \"$1\"; exit {}", if self.exit_ok { 0 } else { 1 })).arg("sh").arg(&self.path);
        c
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        self.inner.parse(line)
    }
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
        self.inner.finish(exit_ok, all_text)
    }
}

/// What `run_blocking` makes of a process that printed the fixture `name`.
fn replay(a: &dyn Adapter, name: &str, exit_ok: bool) -> Vec<AgentEvent> {
    let path = format!("{}/tests/fixtures/agents/{name}", env!("CARGO_MANIFEST_DIR"));
    replay_path(a, &path, exit_ok, vec![])
}

fn replay_path(a: &dyn Adapter, path: &str, exit_ok: bool, env: Vec<(String, String)>) -> Vec<AgentEvent> {
    let d = tempfile::tempdir().unwrap();
    let mut run = Run::new(d.path().to_path_buf(), "p".into(), String::new(), Role::Worker);
    run.extra_env = env;
    let r = Replay { inner: a, path: path.to_string(), exit_ok };
    run_blocking(&r, &run, &mut |_| {}, &AtomicBool::new(false)).unwrap()
}

/// An action at its start, as the fixture has it.
fn start(id: &str, name: &str, input: ActionInput) -> AgentEvent {
    AgentEvent::Action(ToolAction { id: id.into(), name: name.into(), input, output: None, ok: true, done: false })
}
/// The same action with its result.
fn end(id: &str, name: &str, input: ActionInput, output: Option<&str>, ok: bool) -> AgentEvent {
    AgentEvent::Action(ToolAction { id: id.into(), name: name.into(), input, output: output.map(String::from), ok, done: true })
}
fn bash(c: &str) -> ActionInput {
    ActionInput::Bash { command: c.into() }
}
/// v6 (G70): the tokens of a run are checked in `chat_shell.rs`; these tests read the talk of a run.
fn no_tokens(v: Vec<AgentEvent>) -> Vec<AgentEvent> {
    v.into_iter().filter(|e| !matches!(e, AgentEvent::Tokens { .. })).collect()
}
fn text(t: &str) -> AgentEvent {
    AgentEvent::Text(t.into())
}
fn delta(t: &str) -> AgentEvent {
    AgentEvent::TextDelta(t.into())
}
fn done(session: &str, t: &str) -> AgentEvent {
    AgentEvent::Done { session: Some(session.into()), text: t.into(), is_error: false }
}

#[test]
fn claude_real_run_text_tool_and_done_with_session() {
    // fixture captured from `claude -p "run echo hi, then say ok" --output-format stream-json --verbose`
    let evs = replay(&Claude::new(), "claude.jsonl", true);
    let plain: Vec<AgentEvent> =
        evs.into_iter().filter(|e| !matches!(e, AgentEvent::Usage(_) | AgentEvent::Cost(_) | AgentEvent::Context { .. } | AgentEvent::Tokens { .. })).collect();
    assert_eq!(
        plain,
        vec![
            start("toolu_01MELK46bqyQtgAZJQ5CpVwy", "Bash", bash("echo hi")),
            end("toolu_01MELK46bqyQtgAZJQ5CpVwy", "Bash", bash("echo hi"), Some("hi"), true),
            text("ok"),
            done("fe525380-9073-47bf-9375-54c8ca3c6d08", "ok")
        ]
    );
}

#[test]
fn opencode_real_run_text_tool_and_done_with_session() {
    let evs = no_tokens(replay(&OpenCode::new(), "opencode.jsonl", true));
    let id = "call_0b407c29ddb54297b98f91e2";
    assert_eq!(
        evs,
        vec![start(id, "bash", bash("echo hi")), end(id, "bash", bash("echo hi"), Some("hi\n"), true), text("ok"), done("ses_ea2aa5d4affeYzoOqNuIEz6nKL", "ok")]
    );
}

#[test]
fn opencode_real_error_becomes_failed_done() {
    let evs = replay(&OpenCode::new(), "opencode-error.jsonl", false);
    assert_eq!(
        evs,
        vec![AgentEvent::Done {
            session: Some("ses_ea2adb15bffemjRtBYASjZqSRW".into()),
            text: "Unexpected server error. Check server logs for details.".into(),
            is_error: true
        }]
    );
}

#[test]
fn qwen_stream_text_tool_done_and_real_auth_error() {
    let evs = no_tokens(replay(&Qwen::new(), "qwen-from-docs.jsonl", true));
    assert_eq!(
        evs,
        vec![start("t1", "run_shell_command", bash("ls")), end("t1", "run_shell_command", bash("ls"), Some("a b"), true), text("ok"), done("qwen-sess-1", "ok")]
    );
    // captured from the installed `qwen` without credentials
    let evs = replay(&Qwen::new(), "qwen-error.jsonl", false);
    assert_eq!(
        evs,
        vec![AgentEvent::AuthRequired(
            "No auth type is selected. Please configure an auth type (e.g. via settings or `--auth-type`) before running in non-interactive mode.".into()
        )]
    );
}

#[test]
fn codex_events_text_tool_done_and_error() {
    let evs = no_tokens(replay(&Codex::new(), "codex-from-docs.jsonl", true));
    assert_eq!(
        evs,
        vec![
            start("item_1", "shell", bash("bash -lc ls")),
            end("item_1", "shell", bash("bash -lc ls"), Some("docs\nsrc\n"), true),
            text("Repo contains docs and src. ok"),
            done("0199a213-81c0-7800-8aa1-bbab2a035a53", "Repo contains docs and src. ok")
        ]
    );
    // a usage-limit failure is a Limit, with the reset time taken from "in 2 hours 30 minutes"
    let before = Local::now();
    let evs = replay(&Codex::new(), "codex-error-from-docs.jsonl", false);
    let after = Local::now();
    let [AgentEvent::Limit { resets_at: Some(t) }] = evs.as_slice() else { panic!("{evs:?}") };
    assert!(*t >= before + Duration::minutes(150) && *t <= after + Duration::minutes(150));
}

#[test]
fn gemini_events_join_deltas_and_report_errors() {
    let evs = replay(&Gemini::new(), "gemini-from-docs.jsonl", true);
    assert_eq!(
        evs,
        // the pieces of the answer (`delta: true`) are deltas: the chat streams them, the log gets the whole answer once in Done
        vec![start("t1", "run_shell_command", bash("ls")), end("t1", "run_shell_command", bash("ls"), Some("a b"), true), delta("o"), delta("k"), done("gem-sess-1", "ok")]
    );
    let evs = replay(&Gemini::new(), "gemini-error-from-docs.jsonl", false);
    assert!(matches!(evs.as_slice(), [AgentEvent::Limit { resets_at: None }]), "{evs:?}");
}

#[test]
fn a_secret_cut_between_two_gemini_pieces_is_held_until_it_is_whole() {
    let secret = "tok-ABCDEFGH12345678";
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("gemini.jsonl");
    let piece = |t: &str| format!(r#"{{"type":"message","role":"assistant","content":"{t}","delta":true}}"#);
    std::fs::write(&f, format!("{}\n{}\n{}\n", piece("the key is tok-ABCD"), piece("EFGH12345678 ok"), r#"{"type":"result","status":"success","stats":{"total_tokens":10}}"#)).unwrap();
    let evs = replay_path(&Gemini::new(), f.to_str().unwrap(), true, vec![("API_TOKEN".into(), secret.into())]);
    let shown: String = evs.iter().filter_map(|e| if let AgentEvent::TextDelta(t) | AgentEvent::Text(t) = e { Some(t.as_str()) } else { None }).collect();
    assert!(shown.starts_with("the key is ") && shown.ends_with(" ok") && !shown.contains("tok-"), "{shown:?}");
    assert!(matches!(evs.last(), Some(AgentEvent::Done { text, .. }) if !text.contains("tok-")), "{evs:?}");
}

#[test]
fn cursor_events_tool_text_done() {
    let evs = replay(&Cursor::new(), "cursor-from-docs.jsonl", true);
    let w = ActionInput::Write { path: "notes.txt".into(), head: "hi".into(), lines: 1 };
    // a write that worked has no output to show: the new file's head is the view
    assert_eq!(evs, vec![start("c1", "write", w.clone()), end("c1", "write", w, None, true), text("ok"), done("cur-sess-1", "ok")]);
}

#[test]
fn cursor_crash_without_result_is_a_failed_done() {
    let a = Cursor::new();
    let evs = a.finish(false, "{\"type\":\"system\"}\nerror: boom\n");
    assert_eq!(evs, vec![AgentEvent::Done { session: None, text: "error: boom".into(), is_error: true }]);
}

// ---- v8 envs: outputs assembled from each vendor's docs or source (research-clis §7-14), not from runs ----

#[test]
fn amp_grok_and_droid_speak_the_claude_result_shape() {
    let evs = no_tokens(replay(&Amp::new(), "amp-from-docs.jsonl", true));
    let id = "toolu_019cyniPYrSgaJitUSMyxyNV";
    let sess = "T-d2fc4acc-dd1d-497f-9609-ed0da22a7c95";
    assert_eq!(
        evs,
        vec![
            start(id, "Bash", bash("ls")),
            end(id, "Bash", bash("ls"), Some("index.js README.md"), true),
            text("Two files: index.js and README.md"),
            done(sess, "Two files: index.js and README.md")
        ]
    );
    let read = ActionInput::Read { path: "src/main.rs".into() };
    let evs = no_tokens(replay(&Grok::new(), "grok-from-docs.jsonl", true));
    assert_eq!(
        evs,
        vec![
            text("Let me read the file."),
            start("call_1", "read_file", read.clone()),
            end("call_1", "read_file", read, None, true), // a read keeps no output (the Tracker)
            text("Here's a summary..."),
            done("abc123", "Here's a summary...")
        ]
    );
    // droid exec -o json: one result line, with its tokens (cache read + written count as input)
    let evs = replay(&Droid::new(), "droid-from-docs.jsonl", true);
    assert_eq!(
        evs,
        vec![
            AgentEvent::Tokens { input: 4 + 13528 + 17913, output: 18 },
            done("8af22e0a-d222-42c6-8c7e-7a059e391b0b", "A TypeScript monorepo with an API and a UI package.")
        ]
    );
    // a crash without a result line is a failed Done
    assert!(matches!(Droid::new().finish(false, "boom\n").as_slice(), [AgentEvent::Done { is_error: true, .. }]));
}

#[test]
fn kimi_copilot_and_goose_streams_have_no_result_line_and_done_is_built_at_the_end() {
    let evs = no_tokens(replay(&Kimi::new(), "kimi-from-docs.jsonl", true));
    assert_eq!(
        evs,
        vec![text("Let me look."), start("tc_1", "Bash", bash("ls")), end("tc_1", "Bash", bash("ls"), Some("a b"), true), text("ok"), done("kimi-sess-1", "ok")]
    );
    let evs = replay(&Copilot::new(), "copilot-from-docs.jsonl", true);
    assert_eq!(
        evs,
        vec![
            text("Let me look."),
            start("tc_1", "bash", bash("ls")),
            end("tc_1", "bash", bash("ls"), Some("a b"), true),
            delta("ok"),
            text("ok"),
            AgentEvent::Tokens { input: 120, output: 7 },
            done("cop-sess-1", "ok")
        ]
    );
    let shell = ActionInput::Other { summary: "ls".into() };
    let evs = replay(&Goose::new(), "goose-from-docs.jsonl", true);
    assert_eq!(
        evs,
        vec![
            text("Let me look."),
            start("tr_1", "developer__shell", shell.clone()),
            end("tr_1", "developer__shell", shell, Some("a b"), true),
            text("ok"),
            AgentEvent::Tokens { input: 120, output: 10 },
            AgentEvent::Done { session: None, text: "ok".into(), is_error: false }
        ]
    );
}

#[test]
fn crush_and_aider_print_text_and_their_login_complaints_are_auth_required() {
    for a in [&Crush::new() as &dyn Adapter, &Aider::new()] {
        assert_eq!(a.parse("\x1b[1mHello\x1b[0m"), vec![text("Hello")], "{}", a.id());
        assert!(a.parse("   ").is_empty());
        assert_eq!(a.finish(true, "line 1\nline 2\n"), vec![AgentEvent::Done { session: None, text: "line 1\nline 2".into(), is_error: false }]);
        assert!(matches!(a.finish(false, "boom\n").as_slice(), [AgentEvent::Done { is_error: true, .. }]), "{}", a.id());
    }
    // the texts are from the sources (internal/cmd/run.go, aider/onboarding.py): // unverified as CLI output
    let crush = "Error: no providers configured - please run 'crush' to set up a provider interactively";
    assert_eq!(Crush::new().finish(false, crush), auth(crush));
    let aider = "No LLM model was specified and no API keys were provided.";
    assert_eq!(Aider::new().finish(false, aider), auth(aider));
}

#[test]
fn kimi_copilot_and_goose_login_and_limit_errors_from_docs() {
    // // unverified: shapes from the vendors' docs / source, this checks OUR reading of them
    assert_eq!(replay(&Copilot::new(), "copilot-auth-from-docs.jsonl", false), auth("authentication required: 401 Bad credentials"));
    let quota = r#"{"type":"session.error","data":{"errorType":"quota","message":"monthly premium requests used up"}}"#;
    assert!(matches!(Copilot::new().finish(false, quota).as_slice(), [AgentEvent::Limit { .. }]));
    assert_eq!(replay(&Goose::new(), "goose-auth-from-docs.jsonl", false), auth("No api key passed in"));
    assert_eq!(Kimi::new().finish(false, "Error: OAuth login expired, run /login\n"), auth("Error: OAuth login expired, run /login"));
}

#[test]
fn v8_commands_put_each_option_where_the_cli_takes_it() {
    let mut r = run(Role::Worker);
    r.resume = Some("s-1".into());
    let v = args(&Kimi::new(), &r);
    assert_eq!((arg_after(&v, "--output-format").as_str(), arg_after(&v, "--session").as_str()), ("stream-json", "s-1"));
    assert_eq!(v.last().unwrap(), "--prompt=no push\n\ndo it");
    let v = args(&Copilot::new(), &r);
    assert!(v.contains(&"--output-format=json".into()) && v.contains(&"--no-ask-user".into()) && v.contains(&"--resume=s-1".into()), "{v:?}");
    assert_eq!(v.last().unwrap(), "--prompt=no push\n\ndo it");
    let v = args(&Crush::new(), &r);
    assert_eq!(v[..2], ["run", "--quiet"]);
    assert_eq!(arg_after(&v, "--session"), "s-1");
    let v = args(&Amp::new(), &r);
    assert_eq!(v[..3], ["threads", "continue", "s-1"]);
    assert_eq!(v[v.len() - 2..], ["--execute=no push\n\ndo it", "--stream-json"]);
    let mut m = run(Role::Worker);
    m.model = Some("smart".into());
    assert!(!args(&Amp::new(), &m).iter().any(|x| x == "smart" || x.contains("model")), "amp has no model flag");
    let v = args(&Goose::new(), &r);
    assert_eq!(v[..3], ["run", "--output-format", "stream-json"]);
    assert!(!v.contains(&"s-1".into()), "goose runs are never resumed: {v:?}");
    let v = args(&Droid::new(), &r);
    assert_eq!((v[0].as_str(), arg_after(&v, "-o").as_str(), arg_after(&v, "--session-id").as_str()), ("exec", "json", "s-1"));
    assert!(!v.contains(&"high".into()), "never --auto high (it allows git push)");
    let v = args(&Aider::new(), &r);
    assert_eq!(v[..3], ["--yes-always", "--no-pretty", "--no-stream"]);
    assert!(!v.contains(&"s-1".into()), "aider has no sessions: {v:?}");
    let v = args(&Grok::new(), &r);
    assert_eq!((arg_after(&v, "--output-format").as_str(), arg_after(&v, "--resume").as_str()), ("streaming-messages-json", "s-1"));
    assert_eq!(v.last().unwrap(), "--single=no push\n\ndo it");
}

fn claude_error_result(msg: &str) -> Vec<AgentEvent> {
    let line = serde_json::json!({"type":"result","subtype":"success","is_error":true,"session_id":"s1","result":msg}).to_string();
    Claude::new().parse(&line)
}

fn limit_time(msg: &str) -> chrono::DateTime<Local> {
    match claude_error_result(msg).as_slice() {
        [AgentEvent::Limit { resets_at: Some(t) }] => *t,
        other => panic!("{msg:?} -> {other:?}"),
    }
}

#[test]
fn limit_with_epoch_clock_and_relative_times() {
    // legacy Claude Code form: epoch seconds after a pipe
    assert_eq!(limit_time("Claude AI usage limit reached|1792576800").timestamp(), 1792576800);

    // "resets 3pm (Europe/Moscow)" -> the next 15:00 Moscow time (UTC+3, no DST) = 12:00 UTC, within a day
    let now = Local::now();
    let t = limit_time("You've hit your limit \u{b7} resets 3pm (Europe/Moscow)");
    let utc = t.with_timezone(&chrono::Utc);
    // the zone needs the system tz database (a minimal Linux image has none: then it is local time, checked below)
    if std::path::Path::new("/usr/share/zoneinfo/Europe/Moscow").is_file() {
        assert_eq!((utc.hour(), utc.minute()), (12, 0));
    }
    assert!(t > now && t <= now + Duration::hours(24));

    // a zone the database does not know is local time, not UTC (`date` says +0000 for it)
    let t = limit_time("limit reached, resets 3pm (Nowhere/Fake)");
    assert_eq!((t.hour(), t.minute()), (15, 0));

    // no zone in the text -> local time
    let t = limit_time("limit reached, resets 3pm");
    assert_eq!((t.hour(), t.minute()), (15, 0));

    // 24-hour clock with minutes
    let t = limit_time("5-hour limit reached. Resets at 14:30");
    assert_eq!((t.hour(), t.minute()), (14, 30));

    // relative
    let now = Local::now();
    let t = limit_time("rate limit exceeded, try again in 45 minutes");
    assert!((t - now - Duration::minutes(45)).num_seconds().abs() < 5);

    // 12 AM is midnight, not noon
    assert_eq!(limit_time("usage limit reached, resets at 12am").hour(), 0);
}

#[test]
fn limit_without_time_and_ordinary_errors() {
    for m in ["Usage limit reached", "You have reached your weekly limit", "429 Too Many Requests", "Rate limit hit"] {
        assert_eq!(claude_error_result(m), vec![AgentEvent::Limit { resets_at: None }], "{m}");
    }
    // not a limit: stays a failed Done and keeps the session
    assert_eq!(
        claude_error_result("API Error: 500 internal server error"),
        vec![AgentEvent::Done { session: Some("s1".into()), text: "API Error: 500 internal server error".into(), is_error: true }]
    );
    // a successful result that merely talks about limits is not a limit
    let ok = r#"{"type":"result","subtype":"success","is_error":false,"session_id":"s2","result":"I added a rate limit to the API"}"#;
    assert_eq!(Claude::new().parse(ok), vec![done("s2", "I added a rate limit to the API")]);
}

fn run(role: Role) -> Run {
    Run::new("/tmp".into(), "do it".into(), "no push".into(), role)
}

fn args(a: &dyn Adapter, r: &Run) -> Vec<String> {
    a.command(r).get_args().map(|s| s.to_string_lossy().into_owned()).collect()
}

fn arg_after(v: &[String], flag: &str) -> String {
    let i = v.iter().position(|a| a == flag).unwrap_or_else(|| panic!("{flag} missing in {v:?}"));
    v[i + 1].clone()
}

#[test]
fn claude_worker_bans_push_remote_sudo_and_reads_of_ssh_only() {
    let v = args(&Claude::new(), &run(Role::Worker));
    assert_eq!(arg_after(&v, "--permission-mode"), "bypassPermissions");
    let settings: serde_json::Value = serde_json::from_str(&arg_after(&v, "--settings")).unwrap();
    let deny: Vec<&str> = settings["permissions"]["deny"].as_array().unwrap().iter().map(|d| d.as_str().unwrap()).collect();
    for d in ["Bash(git push:*)", "Bash(git remote:*)", "Bash(sudo:*)"] {
        assert!(deny.contains(&d), "{d} missing in {deny:?}");
    }
    // R17i.2: ~/.ssh is not read; no other Read(...) deny: they become OS-level bans in the sandbox and a `*.pem` one
    // breaks TLS for all subprocesses
    assert!(deny.contains(&"Read(~/.ssh/**)"), "{deny:?}");
    assert_eq!(deny.iter().filter(|d| d.starts_with("Read")).count(), 1, "{deny:?}");
    assert_eq!(settings["sandbox"]["enabled"], true);
    assert_eq!(arg_after(&v, "--append-system-prompt"), "no push");
    assert_eq!(v.last().unwrap(), "do it");
}

#[test]
fn claude_reviewer_is_read_only_and_resume_continues_the_session() {
    let mut r = run(Role::Reviewer);
    r.resume = Some("abc-123".into());
    let v = args(&Claude::new(), &r);
    assert_eq!(arg_after(&v, "--permission-mode"), "plan");
    assert_eq!(arg_after(&v, "--resume"), "abc-123");
    assert!(!v.contains(&"bypassPermissions".to_string()));
}

#[test]
fn prompt_with_a_leading_dash_reaches_every_builtin_cli_as_text() {
    let mut r = run(Role::Worker);
    r.system = String::new();
    r.prompt = "--help me".into();
    for a in adapters(&Settings::default()) {
        let v = args(a.as_ref(), &r);
        // `--<flag>=<prompt>` (one word) or `-- <prompt>`
        let ok = v.iter().any(|x| x.starts_with("--") && x.split_once('=').is_some_and(|(_, p)| p == "--help me"))
            || v.iter().position(|x| x == "--").is_some_and(|i| v.get(i + 1).is_some_and(|p| p == "--help me"));
        assert!(ok, "{}: {v:?}", a.id());
    }
}

#[test]
fn opencode_uses_session_and_dir_and_has_no_read_only_flag() {
    let mut r = run(Role::Worker);
    r.resume = Some("ses_1".into());
    let v = args(&OpenCode::new(), &r);
    assert_eq!(arg_after(&v, "--session"), "ses_1");
    assert_eq!(arg_after(&v, "--dir"), "/tmp");
    assert_eq!(arg_after(&v, "--format"), "json");
    // no verified read-only flag: the engine runs such roles in a throwaway copy
    let ro = args(&OpenCode::new(), &run(Role::Reviewer));
    assert!(!ro.contains(&"--agent".to_string()) && !OpenCode::new().read_only_native());
}

#[test]
fn codex_worker_writes_only_in_workspace_and_reviewer_reads_only() {
    assert_eq!(arg_after(&args(&Codex::new(), &run(Role::Worker)), "--sandbox"), "workspace-write");
    assert_eq!(arg_after(&args(&Codex::new(), &run(Role::Reviewer)), "--sandbox"), "read-only");
    let mut r = run(Role::Worker);
    r.resume = Some("thread-1".into());
    // unverified: argument order of `exec ... resume <id>` comes from the docs only; this pins our choice, not engine behaviour
    let v = args(&Codex::new(), &r);
    let i = v.iter().position(|a| a == "resume").unwrap();
    assert_eq!(v[i + 1], "thread-1");
}

#[test]
fn every_engine_hands_the_system_rules_to_the_model() {
    // engines without a system-prompt flag get the rules in front of the prompt
    for a in adapters(&Settings::default()).iter().filter(|a| a.id() != "claude") {
        let v = args(a.as_ref(), &run(Role::Worker));
        assert!(v.iter().any(|x| x == "no push\n\ndo it" || x.split_once('=').is_some_and(|(f, p)| f.starts_with("--") && p == "no push\n\ndo it")), "{} -> {v:?}", a.id());
    }
}

#[test]
fn missing_engine_reports_an_install_hint() {
    let cases: Vec<(Box<dyn Adapter>, &str)> = vec![
        (Box::new(Claude::with_bin("ns-no-such-bin")), "npm i -g @anthropic-ai/claude-code"),
        (Box::new(OpenCode::with_bin("ns-no-such-bin")), "npm i -g opencode-ai"),
        (Box::new(Codex::with_bin("ns-no-such-bin")), "npm i -g @openai/codex"),
        (Box::new(Gemini::with_bin("ns-no-such-bin")), "npm i -g @google/gemini-cli"),
        (Box::new(Cursor::with_bin("ns-no-such-bin")), "cursor.com/install"),
        (Box::new(Qwen::with_bin("ns-no-such-bin")), "npm i -g @qwen-code/qwen-code"),
        (Box::new(Kimi::with_bin("ns-no-such-bin")), "code.kimi.com/kimi-code/install.sh"),
        (Box::new(Copilot::with_bin("ns-no-such-bin")), "gh.io/copilot-install"),
        (Box::new(Crush::with_bin("ns-no-such-bin")), "npm install -g @charmland/crush"),
        (Box::new(Amp::with_bin("ns-no-such-bin")), "ampcode.com/install.sh"),
        (Box::new(Goose::with_bin("ns-no-such-bin")), "aaif-goose/goose/releases/download/stable/download_cli.sh"),
        (Box::new(Droid::with_bin("ns-no-such-bin")), "app.factory.ai/cli"),
        (Box::new(Aider::with_bin("ns-no-such-bin")), "aider.chat/install.sh"),
        (Box::new(Grok::with_bin("ns-no-such-bin")), "x.ai/cli/install.sh"),
    ];
    for (a, hint) in cases {
        let e = a.available().unwrap_err();
        assert!(e.starts_with("not installed: ") && e.contains(hint), "{e}");
    }
    assert!(Claude::with_bin("sh").available().is_ok());
}

#[test]
fn adapters_lists_the_builtin_engines_then_the_custom_ones() {
    let s = Settings { custom_agents: vec![CustomAgent { id: "mine".into(), name: "Mine".into(), run: "echo {prompt}".into(), resume: None }], ..Settings::default() };
    let ids: Vec<String> = adapters(&s).iter().map(|a| a.id().to_string()).collect();
    assert_eq!(ids, ["claude", "codex", "gemini", "opencode", "cursor", "qwen", "kimi", "copilot", "crush", "amp", "goose", "droid", "aider", "grok", "mine"]);
    assert_eq!(adapters(&Settings::default()).len(), 14);
}

#[test]
fn custom_command_cannot_shadow_a_builtin_engine() {
    let mut s = Settings::default();
    let c = |id: &str| CustomAgent { id: id.into(), name: "X".into(), run: "true".into(), resume: None };
    s.custom_agents = vec![c("claude"), c("mine"), c("mine")];
    let list = adapters(&s);
    let ids: Vec<&str> = list.iter().map(|a| a.id()).collect();
    assert_eq!(ids, ["claude", "codex", "gemini", "opencode", "cursor", "qwen", "kimi", "copilot", "crush", "amp", "goose", "droid", "aider", "grok", "mine"]);
    assert_eq!(list[0].name(), "Claude Code");
}

#[test]
fn only_claude_is_guarded() {
    let s = Settings { custom_agents: vec![CustomAgent { id: "mine".into(), name: "Mine".into(), run: "true".into(), resume: None }], ..Settings::default() };
    let guarded: Vec<String> = adapters(&s).iter().filter(|a| a.guarded()).map(|a| a.id().to_string()).collect();
    assert_eq!(guarded, ["claude"]);
}

#[cfg(unix)]
#[test]
fn network_allow_rule_is_emitted_only_for_claude_code_2_1_186_or_newer() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let fake = |name: &str, version: &str| {
        let p = d.path().join(name);
        std::fs::write(&p, format!("#!/bin/sh\necho '{version} (Claude Code)'\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p.to_string_lossy().into_owned()
    };
    let allow = |bin: &str| {
        let v = args(&Claude::with_bin(bin), &run(Role::Worker));
        let s: serde_json::Value = serde_json::from_str(&arg_after(&v, "--settings")).unwrap();
        s["permissions"]["allow"].clone()
    };
    assert_eq!(allow(&fake("old", "2.1.185")), serde_json::Value::Null);
    assert_eq!(allow(&fake("edge", "2.1.186")), serde_json::json!(["WebFetch(domain:*)"]));
    assert_eq!(allow(&fake("new", "2.2.0")), serde_json::json!(["WebFetch(domain:*)"]));
}

fn custom(run: &str, resume: Option<&str>) -> Custom {
    Custom(CustomAgent { id: "mine".into(), name: "Mine".into(), run: run.into(), resume: resume.map(String::from) })
}

fn run_custom(c: &Custom, r: &Run) -> Vec<AgentEvent> {
    run_blocking(c, r, &mut |_| {}, &AtomicBool::new(false)).unwrap()
}

#[test]
fn custom_command_never_pastes_values_into_shell_text() {
    let nasty = "it's \"q\" $(touch PWNED) `touch PWNED` ; touch PWNED\nsecond line";
    // bare, double-quoted and single-quoted placeholders must all deliver the text literally
    for template in [
        "printf '[%s]' {prompt} {cwd} > out.txt && echo fine",
        "printf '[%s]' \"{prompt}\" \"{cwd}\" > out.txt && echo fine",
        "printf '[%s]' '{prompt}' '{cwd}' > out.txt && echo fine",
    ] {
        let d = tempfile::tempdir().unwrap();
        let mut r = run(Role::Worker);
        r.cwd = d.path().to_path_buf();
        r.system = String::new();
        r.prompt = nasty.into();
        let evs = run_custom(&custom(template, None), &r);
        assert_eq!(evs.last().unwrap(), &AgentEvent::Done { session: None, text: "fine".into(), is_error: false }, "{template}");
        let out = std::fs::read_to_string(d.path().join("out.txt")).unwrap();
        assert_eq!(out, format!("[{nasty}][{}]", d.path().to_string_lossy()), "{template}");
        assert!(!d.path().join("PWNED").exists(), "{template}: injection ran");
    }
}

#[test]
fn custom_command_nonzero_exit_is_a_failed_done() {
    let d = tempfile::tempdir().unwrap();
    let mut r = run(Role::Worker);
    r.cwd = d.path().to_path_buf();
    let evs = run_custom(&custom("echo boom >&2; exit 3", None), &r);
    assert_eq!(evs, vec![AgentEvent::Done { session: None, text: "boom".into(), is_error: true }]);
    // a failed command that merely mentioned "rate limit" in its output is a failure, not a limit
    let evs = run_custom(&custom("echo 'I added a rate limit to the API'; exit 1", None), &r);
    assert_eq!(evs.last().unwrap(), &AgentEvent::Done { session: None, text: "I added a rate limit to the API".into(), is_error: true });
    assert!(!evs.iter().any(|e| matches!(e, AgentEvent::Limit { .. })));
    // an old limit line far above the failure tail is not looked at
    let evs = run_custom(&custom("echo 'usage limit reached'; for i in 1 2 3 4 5 6; do echo \"error: step $i failed\" >&2; done; exit 1", None), &r);
    assert!(matches!(evs.last(), Some(AgentEvent::Done { is_error: true, .. })), "{evs:?}");
    assert!(!evs.iter().any(|e| matches!(e, AgentEvent::Limit { .. })));
    // a limit message from the command becomes a Limit
    let evs = run_custom(&custom("echo 'usage limit reached, try again in 10 minutes' >&2; exit 1", None), &r);
    assert!(matches!(evs.last(), Some(AgentEvent::Limit { resets_at: Some(_) })), "{evs:?}");
}

#[test]
fn custom_resume_template_gets_the_session() {
    let d = tempfile::tempdir().unwrap();
    let mut r = run(Role::Worker);
    r.cwd = d.path().to_path_buf();
    r.system = String::new();
    r.resume = Some("S 1".into());
    let c = custom("echo fresh", Some("echo resumed {session}"));
    assert_eq!(run_custom(&c, &r).last().unwrap(), &AgentEvent::Done { session: None, text: "resumed S 1".into(), is_error: false });
    r.resume = None;
    assert_eq!(run_custom(&c, &r).last().unwrap(), &AgentEvent::Done { session: None, text: "fresh".into(), is_error: false });
}

#[test]
fn custom_placeholder_inside_a_longer_quoted_string_is_one_literal_word() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("a.txt"), "").unwrap(); // a glob `*` would expand to this
    let mut r = run(Role::Worker);
    r.cwd = d.path().to_path_buf();
    r.system = String::new();
    for prompt in ["*", "two  words *"] {
        r.prompt = prompt.into();
        for template in [
            "printf '<%s>' \"say {prompt} now\" > out.txt",
            "printf '<%s>' 'say {prompt} now' > out.txt",
            "printf '<%s>' say-{prompt}-now > out.txt",
        ] {
            run_custom(&custom(template, None), &r);
            let out = std::fs::read_to_string(d.path().join("out.txt")).unwrap();
            let want = template.contains("say-").then(|| format!("<say-{prompt}-now>")).unwrap_or(format!("<say {prompt} now>"));
            assert_eq!(out, want, "{template} / {prompt}");
        }
    }
}

#[test]
fn claude_network_note_explains_why_the_network_is_closed() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let mk = |name: &str, body: &str| {
        let p = d.path().join(name);
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p.to_string_lossy().into_owned()
    };
    let old = Claude::with_bin(&mk("old", "echo '2.1.100 (Claude Code)'")).network_note().unwrap();
    assert!(old.contains("2.1.100") && old.contains("2.1.186"), "{old}");
    let broken = Claude::with_bin(&mk("broken", "echo nonsense")).network_note().unwrap();
    assert!(broken.contains("could not be read"), "{broken}");
    assert_eq!(Claude::with_bin(&mk("new", "echo '2.1.186 (Claude Code)'")).network_note(), None);
}

// ---- v2: usage and cost, model flag, read-only flag, sign-in and model refusals, model lists ----

use chrono::TimeZone;
use nightshift_core::model::{LimitWindow, ModelFamily};

fn events_of(a: &dyn Adapter, name: &str) -> Vec<AgentEvent> {
    fixture(name).lines().flat_map(|l| a.parse(l)).collect()
}

fn windows(evs: &[AgentEvent]) -> Vec<Vec<LimitWindow>> {
    evs.iter().filter_map(|e| if let AgentEvent::Usage(w) = e { Some(w.clone()) } else { None }).collect()
}

#[test]
fn claude_rate_limit_event_gives_the_five_hour_and_weekly_windows() {
    let evs = events_of(&Claude::new(), "claude.jsonl");
    let all = windows(&evs);
    assert_eq!(all.len(), 2, "the live run printed two rate_limit_event lines");
    // numbers read by hand from the saved live run: 5 h 13 %, week 11 %, resets 1792576800 / 1792918800
    let first = &all[0];
    assert_eq!(first.len(), 2);
    assert_eq!((first[0].name.as_str(), first[0].used_pct), ("five_hour", Some(13.0)));
    assert_eq!((first[1].name.as_str(), first[1].used_pct), ("seven_day", Some(11.0)));
    assert_eq!(first[0].resets_at, Some(Local.timestamp_opt(1792576800, 0).unwrap()));
    assert_eq!(first[1].resets_at, Some(Local.timestamp_opt(1792918800, 0).unwrap()));
    assert!(first.iter().all(|w| !w.exhausted && w.source == "rate_limit_event" && w.model.is_none()));
    assert_eq!(all[1][0].used_pct, Some(14.0), "the later event carries the later number");
    // an allowed status is not a limit
    assert!(!evs.iter().any(|e| matches!(e, AgentEvent::Limit { .. })));
}

#[test]
fn claude_blocked_status_is_a_limit_with_the_reset_time() {
    let evs = events_of(&Claude::new(), "claude-rate-limit-rejected.jsonl");
    assert!(evs.contains(&AgentEvent::Limit { resets_at: Some(Local.timestamp_opt(1792576800, 0).unwrap()) }), "{evs:?}");
    let w = &windows(&evs)[0];
    assert!(w[0].exhausted && w[0].used_pct == Some(100.0), "{w:?}");
    assert!(!w[1].exhausted);
}

#[test]
fn claude_rejected_status_with_the_extra_usage_allowed_is_no_limit() {
    let d = tempfile::tempdir().unwrap();
    let line = |overage: &str, using: bool| {
        serde_json::json!({"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1792576800,"rateLimitType":"five_hour","overageStatus":overage,"isUsingOverage":using,
            "unifiedWindows":{"five_hour":{"utilization":1.0,"resetsAt":1792576800},"seven_day":{"utilization":0.11,"resetsAt":1792918800}}}})
    };
    let result = serde_json::json!({"type":"result","subtype":"success","is_error":false,"result":"done","session_id":"s1"});
    // paid by the extra usage: the answer stands, the env is not held until the reset
    for (overage, using) in [("allowed", true), ("allowed", false), ("allowed_warning", true)] {
        let evs = replay_lines(d.path(), &[line(overage, using), result.clone()], false);
        assert!(!evs.iter().any(|e| matches!(e, AgentEvent::Limit { .. })), "{overage} {using}: {evs:?}");
        assert!(windows(&evs).iter().flatten().all(|w| !w.exhausted), "{overage} {using}: {evs:?}");
        assert!(matches!(evs.last(), Some(AgentEvent::Done { is_error: false, .. })), "{evs:?}");
    }
    // the fixture of a real stop: no extra usage
    let evs = replay_lines(d.path(), &[line("rejected", false), result], false);
    assert!(evs.iter().any(|e| matches!(e, AgentEvent::Limit { .. })), "{evs:?}");
}

#[test]
fn claude_reports_the_cost_of_the_run_before_done_and_only_claude_does() {
    let evs = events_of(&Claude::new(), "claude.jsonl");
    let i = evs.iter().position(|e| matches!(e, AgentEvent::Cost(_))).expect("cost");
    assert_eq!(evs[i], AgentEvent::Cost(0.2655434)); // total_cost_usd of the saved run
    // then the tokens of the run (G70), then Done
    assert!(matches!(evs[i + 1], AgentEvent::Tokens { .. }) && matches!(evs[i + 2], AgentEvent::Done { .. }), "{evs:?}");
    let costly: Vec<String> = adapters(&Settings::default()).iter().filter(|a| a.reports_cost()).map(|a| a.id().to_string()).collect();
    assert_eq!(costly, ["claude"]);
}

#[test]
fn claude_budget_schema_and_model_reach_the_command() {
    let mut r = run(Role::Worker);
    r.model = Some("opus".into());
    r.max_budget_usd = Some(1.5);
    r.json_schema = Some("{\"type\":\"object\"}".into());
    let v = args(&Claude::new(), &r);
    assert_eq!(arg_after(&v, "--model"), "opus");
    assert_eq!(arg_after(&v, "--max-budget-usd"), "1.5");
    assert_eq!(arg_after(&v, "--json-schema"), "{\"type\":\"object\"}");
    let plain = args(&Claude::new(), &run(Role::Worker));
    for f in ["--model", "--max-budget-usd", "--json-schema"] {
        assert!(!plain.contains(&f.to_string()), "{f} without a value");
    }
}

#[test]
fn model_flag_is_the_envs_own_and_absent_without_a_model() {
    let flags: [(Box<dyn Adapter>, &str); 13] = [
        (Box::new(Claude::new()), "--model"),
        (Box::new(Codex::new()), "-m"),
        (Box::new(Gemini::new()), "-m"),
        (Box::new(OpenCode::new()), "-m"),
        (Box::new(Cursor::new()), "--model"),
        (Box::new(Qwen::new()), "-m"),
        (Box::new(Kimi::new()), "-m"),
        (Box::new(Copilot::new()), "--model"),
        (Box::new(Crush::new()), "-m"),
        (Box::new(Goose::new()), "--model"),
        (Box::new(Droid::new()), "-m"),
        (Box::new(Aider::new()), "--model"),
        (Box::new(Grok::new()), "-m"),
    ];
    for (a, flag) in flags {
        let mut r = run(Role::Worker);
        assert!(!args(a.as_ref(), &r).contains(&flag.to_string()), "{}: flag without a model", a.id());
        r.model = Some("provider/some-model".into());
        assert_eq!(arg_after(&args(a.as_ref(), &r), flag), "provider/some-model", "{}", a.id());
    }
}

#[test]
fn read_only_flag_is_set_where_the_env_has_one() {
    // (adapter, flag, value after it, has a native read-only mode)
    let cases: [(Box<dyn Adapter>, &str, &str, bool); 5] = [
        (Box::new(Claude::new()), "--permission-mode", "plan", true),
        (Box::new(Codex::new()), "--sandbox", "read-only", true),
        (Box::new(Gemini::new()), "--approval-mode", "plan", true),
        (Box::new(Cursor::new()), "--mode", "ask", true),
        (Box::new(Grok::new()), "--permission-mode", "dontAsk", true),
    ];
    for (a, flag, value, native) in cases {
        let mut r = run(Role::Worker);
        r.read_only = true;
        let v = args(a.as_ref(), &r);
        let found = v.iter().any(|x| x == &format!("{flag}={value}")) || (v.iter().position(|x| x == flag).is_some_and(|i| v[i + 1] == value));
        assert!(found, "{}: {v:?}", a.id());
        assert_eq!(a.read_only_native(), native, "{}", a.id());
        // a writing run does not get it
        let w = args(a.as_ref(), &run(Role::Worker));
        assert!(!w.iter().any(|x| x == &format!("{flag}={value}")) && !w.windows(2).any(|p| p[0] == flag && p[1] == value), "{}: {w:?}", a.id());
    }
    // opencode, Qwen and the own command have no flag: the engine uses a throwaway copy
    let mut r = run(Role::Worker);
    r.read_only = true;
    let q = args(&Qwen::new(), &r);
    assert!(!q.contains(&"--yolo".to_string()), "a read-only Qwen run is not auto-approved: {q:?}");
    for a in [&OpenCode::new() as &dyn Adapter, &Qwen::new(), &custom("true", None), &Kimi::new(), &Copilot::new(), &Crush::new(), &Amp::new(), &Goose::new()] {
        assert!(!a.read_only_native(), "{}", a.id());
    }
    // Copilot like Qwen: a reader is not auto-approved
    assert!(!args(&Copilot::new(), &r).contains(&"--allow-all-tools".to_string()));
    assert!(args(&Copilot::new(), &run(Role::Worker)).contains(&"--allow-all-tools".to_string()));
    // Droid: `droid exec` reads only by itself, a writer gets `--auto medium` (never `high`: that allows git push)
    assert!(Droid::new().read_only_native() && !args(&Droid::new(), &r).contains(&"--auto".to_string()));
    assert_eq!(arg_after(&args(&Droid::new(), &run(Role::Worker)), "--auto"), "medium");
    // Aider: --dry-run for a reader only
    assert!(Aider::new().read_only_native() && args(&Aider::new(), &r).contains(&"--dry-run".to_string()));
    assert!(!args(&Aider::new(), &run(Role::Worker)).contains(&"--dry-run".to_string()));
    // Grok: a writer runs with --yolo, a reader never
    assert!(args(&Grok::new(), &run(Role::Worker)).contains(&"--yolo".to_string()) && !args(&Grok::new(), &r).contains(&"--yolo".to_string()));
}

#[test]
fn json_schema_goes_only_to_claude_and_codex() {
    let mut r = run(Role::Worker);
    r.json_schema = Some("{\"type\":\"object\"}".into());
    assert_eq!(arg_after(&args(&Claude::new(), &r), "--json-schema"), "{\"type\":\"object\"}");
    let priv_dir = tempfile::tempdir().unwrap();
    let codex = Codex::new().with_schema_dir(priv_dir.path());
    let schema_path = arg_after(&args(&codex, &r), "--output-schema");
    assert_eq!(std::fs::read_to_string(&schema_path).unwrap(), "{\"type\":\"object\"}", "Codex takes a file");
    assert!(std::path::Path::new(&schema_path).starts_with(priv_dir.path()), "in the app's private folder");
    assert_ne!(schema_path, arg_after(&args(&codex, &r), "--output-schema"), "a file of its own per run");
    for a in adapters(&Settings::default()).iter().filter(|a| a.id() != "claude" && a.id() != "codex") {
        let v = args(a.as_ref(), &r);
        assert!(!v.iter().any(|x| x.contains("schema")), "{}: {v:?}", a.id());
    }
}

#[test]
fn extra_args_are_separate_arguments_and_extra_env_reaches_the_process() {
    let mut r = run(Role::Worker);
    r.extra_args = vec!["--flag".into(), "a value; touch PWNED".into()];
    r.extra_env = vec![("NS_TEST_SECRET".into(), "s3".into())];
    for a in adapters(&Settings::default()) {
        let c = a.command(&r);
        let v: Vec<String> = c.get_args().map(|s| s.to_string_lossy().into_owned()).collect();
        let i = v.iter().position(|x| x == "a value; touch PWNED");
        let i = i.unwrap_or_else(|| panic!("{}: {v:?}", a.id()));
        assert_eq!(v[i - 1], "--flag", "{}: two arguments, not one string", a.id());
        let env: Vec<_> = c.get_envs().filter(|(k, _)| *k == "NS_TEST_SECRET").collect();
        assert_eq!(env.len(), 1, "{}", a.id());
        assert_eq!(env[0].1.unwrap(), "s3");
    }
    // the own command gets the environment too
    let d = tempfile::tempdir().unwrap();
    let mut r = run(Role::Worker);
    r.cwd = d.path().to_path_buf();
    r.extra_env = vec![("NS_TEST_SECRET".into(), "s3".into())];
    let evs = run_custom(&custom("echo \"$NS_TEST_SECRET\"", None), &r);
    assert_eq!(evs.last().unwrap(), &AgentEvent::Done { session: None, text: "s3".into(), is_error: false });
}

#[test]
fn cli_path_from_the_env_settings_replaces_the_binary() {
    let mut s = Settings::default();
    let e = nightshift_core::model::EnvSettings { cli_path: Some("/opt/special/codex".into()), ..Default::default() };
    s.envs.insert("codex".into(), e);
    let list = adapters(&s);
    let codex = list.iter().find(|a| a.id() == "codex").unwrap();
    assert_eq!(codex.command(&run(Role::Worker)).get_program(), "/opt/special/codex");
    let claude = list.iter().find(|a| a.id() == "claude").unwrap();
    assert_ne!(claude.command(&run(Role::Worker)).get_program(), "/opt/special/codex");
}

fn auth(msg: &str) -> Vec<AgentEvent> {
    vec![AgentEvent::AuthRequired(msg.into())]
}

#[test]
fn qwen_real_output_without_login_is_auth_required_even_with_a_bad_model() {
    let q = "No auth type is selected. Please configure an auth type (e.g. via settings or `--auth-type`) before running in non-interactive mode.";
    assert_eq!(replay(&Qwen::new(), "qwen-error.jsonl", false), auth(q));
    // `qwen -m nonexistent-model-x` (real): the login check comes first, the model is never looked at
    assert_eq!(replay(&Qwen::new(), "qwen-bad-model-no-auth.jsonl", false), auth(q));
}

#[test]
fn login_rule_matches_the_messages_we_know_from_docs_not_cli_output() {
    // texts assembled from docs / memory of the CLIs, // unverified: this checks OUR rule, not what the CLIs print
    assert_eq!(replay(&Codex::new(), "codex-auth-from-docs.jsonl", false), auth("Not logged in. Please run `codex login`."));
    assert_eq!(
        replay(&Gemini::new(), "gemini-auth-from-docs.jsonl", false),
        auth("Please set an Auth method in your .gemini/settings.json or specify GEMINI_API_KEY.")
    );
    assert_eq!(
        replay(&Cursor::new(), "cursor-auth-from-docs.jsonl", false),
        auth("Authentication required. Please run 'agent login' first, or set CURSOR_API_KEY.")
    );
    let claude = r#"{"type":"result","subtype":"success","is_error":true,"session_id":"s","result":"Not logged in \u00b7 Please run /login"}"#;
    assert!(matches!(Claude::new().parse(claude).as_slice(), [AgentEvent::AuthRequired(_)]));
}

#[test]
fn real_cli_output_for_a_refused_model_is_model_rejected() {
    // opencode: real output of `opencode run -m bogus/model --print-logs` (the JSON stream alone says only
    // "Unexpected server error")
    let evs = OpenCode::new().finish(false, &fixture("opencode-model-rejected.log"));
    let [AgentEvent::ModelRejected(m)] = evs.as_slice() else { panic!("{evs:?}") };
    assert!(m.contains("ProviderModelNotFoundError: Model not found: bogus/model."), "{m}");
    // Claude: real `claude -p --model nonexistent-model-x --output-format stream-json` (no request reaches the model)
    let evs = replay(&Claude::new(), "claude-model-rejected.jsonl", false);
    let m = evs.iter().find_map(|e| if let AgentEvent::ModelRejected(m) = e { Some(m.clone()) } else { None }).expect("ModelRejected");
    assert!(m.contains("issue with the selected model (nonexistent-model-x)"), "{m}");
    assert!(!evs.iter().any(|e| matches!(e, AgentEvent::Done { .. })), "a refusal replaces Done");
}

#[test]
fn model_refusal_rule_matches_the_messages_we_know_from_docs_not_cli_output() {
    // // unverified texts from docs / memory: this checks OUR rule, not what these CLIs print
    assert_eq!(
        replay(&Codex::new(), "codex-model-rejected-from-docs.jsonl", false),
        vec![AgentEvent::ModelRejected("The 'gpt-nope' model is not supported when using Codex with a ChatGPT account.".into())]
    );
    assert!(matches!(replay(&Gemini::new(), "gemini-model-rejected-from-docs.jsonl", false).as_slice(), [AgentEvent::ModelRejected(_)]));
    assert!(matches!(replay(&Cursor::new(), "cursor-model-rejected-from-docs.jsonl", false).as_slice(), [AgentEvent::ModelRejected(_)]));
    assert!(matches!(replay(&Qwen::new(), "qwen-model-rejected-from-docs.jsonl", false).as_slice(), [AgentEvent::ModelRejected(_)]));
}

#[test]
fn ordinary_failures_and_limits_are_not_mistaken_for_login_or_model_errors() {
    // the real opencode server error without the log line stays a plain failure
    let evs = replay(&OpenCode::new(), "opencode-error.jsonl", false);
    assert!(matches!(evs.as_slice(), [AgentEvent::Done { is_error: true, .. }]), "{evs:?}");
    // a limit text keeps being a limit, with no reset time when the text has none
    assert!(matches!(replay(&Gemini::new(), "gemini-error-from-docs.jsonl", false).as_slice(), [AgentEvent::Limit { resets_at: None }]));
}

fn script_printing(dir: &std::path::Path, expect_args: &str, fixture_name: &str) -> String {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join(format!("fake-{}", expect_args.replace(' ', "-")));
    let body = format!(
        "#!/bin/sh\n[ \"$*\" = \"{expect_args}\" ] || {{ echo \"unexpected args: $*\" >&2; exit 2; }}\ncat '{}/tests/fixtures/agents/{fixture_name}'\n",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::write(&p, body).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p.to_string_lossy().into_owned()
}

fn ids(f: &[ModelFamily]) -> Vec<&str> {
    f.iter().map(|f| f.id.as_str()).collect()
}

#[test]
fn opencode_models_come_from_the_saved_list() {
    let d = tempfile::tempdir().unwrap();
    let a = OpenCode::with_bin(&script_printing(d.path(), "models", "opencode-models.txt"));
    let fams = a.models().unwrap();
    // ten lines of the real `opencode models`, in order, each its own family
    assert_eq!(fams.len(), 10);
    assert_eq!(fams[0].id, "opencode/beam-free");
    assert_eq!(fams[9].id, "opencode/nemotron-3.5-super-vl-free");
    assert!(fams.iter().all(|f| f.vendor == "opencode" && f.versions.is_empty()));
}

#[test]
fn codex_and_cursor_list_parsing_rule_on_samples_from_docs_not_cli_output() {
    let d = tempfile::tempdir().unwrap();
    // from docs, // unverified output shapes
    let codex = Codex::with_bin(&script_printing(d.path(), "debug models", "codex-models-from-docs.json")).models().unwrap();
    assert_eq!(ids(&codex), ["gpt-5.1-codex", "gpt", "gpt-5.1-codex-mini"]);
    let gpt = &codex[1];
    assert_eq!(gpt.versions, ["5.1", "5"], "newest first");
    assert_eq!(gpt.label, "GPT-5.1");
    assert_eq!(gpt.exact["5"], "gpt-5", "the id is the one the CLI printed");
    let cursor = Cursor::with_bin(&script_printing(d.path(), "--list-models", "cursor-models-from-docs.txt")).models().unwrap();
    assert_eq!(ids(&cursor), ["auto", "sonnet", "gpt"]);
    assert_eq!(cursor[1].versions, ["4.5", "4"]);
    assert!(cursor[0].default && !cursor[1].default, "(current) marks the default");
}

#[test]
fn envs_without_a_list_command_have_only_the_built_in_hint_list() {
    for a in [&Claude::new() as &dyn Adapter, &Gemini::new(), &Qwen::new()] {
        assert!(a.models().is_err(), "{}", a.id());
        let b = a.builtin_models();
        assert!(!b.is_empty(), "{}", a.id());
        // the description is a dictionary key of the interface (story 240); the source mark is a field of its own
        for f in &b {
            assert!(!f.default, "{} {}", a.id(), f.id);
            assert_eq!(f.blurb.as_deref(), Some(format!("model.desc.{}", f.id).as_str()), "{} {}", a.id(), f.id);
            let note = f.source_note.as_deref().unwrap_or_default();
            assert!(note.contains("2026-10-21; не сверено с CLI"), "{} {}: {note}", a.id(), f.id);
        }
        // every version has the identifier the vendor docs give for it
        assert!(b.iter().all(|f| f.versions.iter().all(|v| f.exact.contains_key(v))), "{}", a.id());
    }
    let claude = Claude::new().builtin_models();
    let opus = claude.iter().find(|f| f.id == "opus").unwrap();
    assert_eq!(opus.versions[0], "5.5");
    assert_eq!(opus.exact["5.5"], "claude-opus-5-5", "also seen live in the init event of the saved run");
    assert_eq!(opus.exact["4.6"], "claude-opus-4-6");
    assert_eq!(Gemini::new().builtin_models().iter().find(|f| f.id == "pro").unwrap().exact["2.5"], "gemini-2.5-pro");
    assert_eq!(Qwen::new().builtin_models().iter().find(|f| f.id == "qwen-plus").unwrap().exact["3.7"], "qwen3.7-plus");
    assert_eq!(ids(&Claude::new().builtin_models())[..3], ["fable", "opus", "sonnet"]);
    // a missing CLI is an Err too, not a panic
    assert!(Codex::with_bin("ns-no-such-bin").models().is_err());
}

#[test]
fn interactive_resume_is_an_argument_list_and_checks_the_session_id() {
    let cwd = std::path::Path::new("/tmp");
    let argv = |c: Option<std::process::Command>| -> Option<Vec<String>> {
        c.map(|c| c.get_args().map(|s| s.to_string_lossy().into_owned()).collect())
    };
    let want = [
        (Box::new(Claude::new()) as Box<dyn Adapter>, vec!["--resume", "abc-123"]),
        (Box::new(Gemini::new()), vec!["--resume", "abc-123"]),
        (Box::new(Qwen::new()), vec!["--resume", "abc-123"]),
        (Box::new(Cursor::new()), vec!["--resume", "abc-123"]),
        (Box::new(OpenCode::new()), vec!["-s", "abc-123"]),
        (Box::new(Kimi::new()), vec!["--session", "abc-123"]),
        (Box::new(Copilot::new()), vec!["--resume=abc-123"]),
        (Box::new(Crush::new()), vec!["--session", "abc-123"]),
        (Box::new(Amp::new()), vec!["threads", "continue", "abc-123"]),
        (Box::new(Goose::new()), vec!["session", "--resume", "--session-id", "abc-123"]),
        (Box::new(Droid::new()), vec!["--resume", "abc-123"]),
        (Box::new(Grok::new()), vec!["--resume", "abc-123"]),
    ];
    for (a, v) in want {
        let c = a.interactive_resume("abc-123", cwd);
        assert_eq!(c.as_ref().map(|c| c.get_current_dir().map(|d| d.to_path_buf())), Some(Some(cwd.to_path_buf())), "{}", a.id());
        assert_eq!(argv(c), Some(v.into_iter().map(String::from).collect()), "{}", a.id());
        // nothing that could be read as an option or a shell command gets through
        for bad in ["", "-x", "--resume", "a b", "a;touch PWNED", "$(id)", "a\nb", &"x".repeat(200)] {
            assert!(a.interactive_resume(bad, cwd).is_none(), "{} accepted {bad:?}", a.id());
        }
    }
    assert!(Codex::new().interactive_resume("abc-123", cwd).is_none());
    assert!(Aider::new().interactive_resume("abc-123", cwd).is_none(), "aider has no sessions");
    assert!(custom("echo {prompt}", Some("echo {session}")).interactive_resume("abc-123", cwd).is_none());
}

#[test]
fn sandbox_info_and_install_hints() {
    let info = Claude::with_bin("sh").sandbox_info();
    assert!(info.sandboxed && info.push_blocked && !info.writes_outside_copy);
    for a in adapters(&Settings::default()).iter().filter(|a| a.id() != "claude") {
        let i = a.sandbox_info();
        assert!(!i.sandboxed && !i.push_blocked && i.network_open && i.writes_outside_copy, "{}", a.id());
    }
    // the two commands the mockup shows, marked as not verified
    assert_eq!(Cursor::new().install_hint().unwrap(), "curl https://cursor.com/install -fsS | bash (не сверено)");
    assert_eq!(Qwen::new().install_hint().unwrap(), "npm install -g @qwen-code/qwen-code (не сверено)");
    assert!(custom("true", None).install_hint().is_none());
}

// ---- core::models: catalog sources, resolve, retry chain ----

use nightshift_core::model::{Catalog, CatalogSource, EnvSettings, ModelChoice, ModelNote};
use nightshift_core::models::{catalog, next_on_reject, refresh, resolve, stronger_than};
use nightshift_core::store::Store;

fn fam(id: &str, versions: &[&str]) -> ModelFamily {
    ModelFamily {
        id: id.into(), label: id.to_uppercase(), vendor: "v".into(), blurb: None,
        versions: versions.iter().map(|s| s.to_string()).collect(), default: false,
        // test data: the identifier of version v is spelled out here, the code under test never builds one
        exact: versions.iter().map(|v| (v.to_string(), format!("{id}-{v}"))).collect(),
        source_note: None,
    }
}

fn cat(env: &str, families: Vec<ModelFamily>) -> Catalog {
    let mut c = Catalog { env: env.into(), families, source: CatalogSource::BuiltIn, at: Local::now() };
    // the same pseudo entries every real catalog has: build it through the public path
    let d = tempfile::tempdir().unwrap();
    let store = Store::open(d.path()).unwrap();
    store.set_catalog(c.clone()).unwrap();
    let a = FakeAdapter::new(vec![]);
    c = catalog(env, &a, &store);
    c
}

fn pick(f: &str, v: Option<&str>) -> ModelChoice {
    ModelChoice { family: f.into(), version: v.map(String::from) }
}

#[test]
fn catalog_uses_the_snapshot_when_the_cli_fails_and_the_hint_list_without_one() {
    let d = tempfile::tempdir().unwrap();
    let store = Store::open(d.path()).unwrap();
    // no snapshot, CLI fails: the built-in hint list (Claude), marked as such
    let c = catalog("claude", &Claude::with_bin("ns-no-such-bin"), &store);
    assert_eq!(c.source, CatalogSource::BuiltIn);
    assert_eq!(c.families[0].id, "default", "CLI default comes first");
    assert!(c.families[0].default && c.families.iter().filter(|f| f.default).count() == 1);
    assert_eq!(ids(&c.families)[1..4], ["fable", "opus", "sonnet"]);
    assert_eq!(refresh("claude", &Claude::with_bin("ns-no-such-bin"), &store).source, CatalogSource::BuiltIn);

    // a good list is saved as the snapshot...
    let ok = FakeAdapter::new(vec![]).with_models(vec![fam("a", &["2", "1"]), fam("b", &[])]);
    let fresh = refresh("fake", &ok, &store);
    assert_eq!((fresh.source.clone(), ids(&fresh.families)), (CatalogSource::CliList, vec!["default", "a", "b"]));
    // ...and when the CLI fails later, the snapshot is what comes back, also after reopening the store
    let broken = FakeAdapter::new(vec![]);
    drop(store);
    let store = Store::open(d.path()).unwrap();
    let again = refresh("fake", &broken, &store);
    assert_eq!((again.source, ids(&again.families)), (CatalogSource::Snapshot, vec!["default", "a", "b"]));
    assert_eq!(catalog("fake", &broken, &store).families, again.families);
    // an env with neither a list nor hints has only the pseudo entries
    let none = catalog("codex", &Codex::with_bin("ns-no-such-bin"), &store);
    assert_eq!((none.source, ids(&none.families)), (CatalogSource::Manual, vec!["default"]));
}

#[test]
fn own_models_are_added_to_every_catalog() {
    let d = tempfile::tempdir().unwrap();
    let mut store = Store::open(d.path()).unwrap();
    let mut s = store.settings();
    s.envs.insert("claude".into(), EnvSettings { custom_models: vec!["my-org/tuned-1".into()], ..EnvSettings::default() });
    store.set_settings(s).unwrap();
    let c = catalog("claude", &Claude::new(), &store);
    assert_eq!(c.families.last().unwrap().id, "my-org/tuned-1");
    assert_eq!(c.families.last().unwrap().vendor, "custom");
    // an own model goes to the CLI as typed
    assert_eq!(resolve(&c, &pick("my-org/tuned-1", None)).cli_arg.as_deref(), Some("my-org/tuned-1"));
}

#[test]
fn latest_is_the_alias_at_claude_and_the_first_version_elsewhere() {
    let claude = cat("claude", vec![fam("opus", &["5.5", "4.8"])]);
    let r = resolve(&claude, &pick("opus", None));
    assert_eq!((r.cli_arg.as_deref(), r.note), (Some("opus"), None));
    let codex = cat("codex", vec![fam("gpt", &["5.1", "5"])]);
    let r = resolve(&codex, &pick("gpt", None));
    assert_eq!((r.cli_arg.as_deref(), r.label.as_str(), r.note), (Some("gpt-5.1"), "GPT 5.1", None));
    // no versions known: the family id is the name
    let r = resolve(&cat("qwen", vec![fam("qwen3-coder-plus", &[])]), &pick("qwen3-coder-plus", None));
    assert_eq!(r.cli_arg.as_deref(), Some("qwen3-coder-plus"));
    // CLI default: no flag
    assert_eq!(resolve(&claude, &pick("default", None)).cli_arg, None);
}

#[test]
fn a_pinned_older_version_says_a_newer_one_exists() {
    let c = cat("codex", vec![fam("gpt", &["5.1", "5"])]);
    let r = resolve(&c, &pick("gpt", Some("5")));
    assert_eq!((r.cli_arg.as_deref(), r.note), (Some("gpt-5"), Some(ModelNote::Newer("5.1".into()))));
    let r = resolve(&c, &pick("gpt", Some("5.1")));
    assert_eq!(r.note, None, "the newest has nothing newer");
}

#[test]
fn a_removed_version_moves_to_the_next_newer_one_else_the_newest() {
    let c = cat("codex", vec![fam("gpt", &["5.2", "5.1", "4"])]);
    // 5 is gone: the next newer of the family is 5.1 (not the newest, 5.2)
    let r = resolve(&c, &pick("gpt", Some("5")));
    assert_eq!(r.cli_arg.as_deref(), Some("gpt-5.1"));
    assert_eq!(r.note, Some(ModelNote::Replaced { from: "5".into(), to: "5.1".into() }));
    // 9 is gone and nothing is newer: the newest of the family
    let r = resolve(&c, &pick("gpt", Some("9")));
    assert_eq!(r.note, Some(ModelNote::Replaced { from: "9".into(), to: "5.2".into() }));
    // versions compare as numbers: 5.10 is newer than 5.9
    let c = cat("codex", vec![fam("gpt", &["5.10", "5.9"])]);
    assert_eq!(resolve(&c, &pick("gpt", Some("5.8"))).note, Some(ModelNote::Replaced { from: "5.8".into(), to: "5.9".into() }));
    // a family the catalog does not know is not started under a made-up name
    let r = resolve(&c, &pick("mystery", Some("1")));
    assert_eq!((r.cli_arg, r.note), (None, Some(ModelNote::Rejected { tried: "mystery 1".into() })));
    // a listed family without versions cannot name a pinned one: its own id, noted
    let flat = cat("qwen", vec![fam("qwen3-coder-plus", &[])]);
    let r = resolve(&flat, &pick("qwen3-coder-plus", Some("9")));
    assert_eq!((r.cli_arg.as_deref(), r.note), (Some("qwen3-coder-plus"), Some(ModelNote::Replaced { from: "9".into(), to: "latest".into() })));
}

#[test]
fn a_refused_model_is_retried_as_latest_of_the_family_then_without_a_flag() {
    let c = cat("codex", vec![fam("gpt", &["5.1", "5"]), fam("qwen", &[])]);
    let step1 = next_on_reject(&c, &pick("gpt", Some("5"))).unwrap();
    assert_eq!(step1, pick("gpt", None));
    let step2 = next_on_reject(&c, &step1).unwrap();
    assert_eq!(step2, pick("default", None));
    assert_eq!(resolve(&c, &step2).cli_arg, None);
    assert_eq!(next_on_reject(&c, &step2), None, "nothing left after the CLI default");
    // pinned to the newest already: "latest" would be the same model, straight to no flag
    assert_eq!(next_on_reject(&c, &pick("gpt", Some("5.1"))), Some(pick("default", None)));
    assert_eq!(next_on_reject(&c, &pick("qwen", None)), Some(pick("default", None)));
}

#[test]
fn stronger_is_the_first_listed_model_that_is_not_the_current_one() {
    let c = cat("claude", vec![fam("fable", &[]), fam("opus", &[]), fam("sonnet", &[])]);
    assert_eq!(stronger_than(&c, &pick("sonnet", None)), Some(pick("fable", None)));
    assert_eq!(stronger_than(&c, &pick("fable", None)), Some(pick("opus", None)));
    assert_eq!(stronger_than(&c, &pick("default", None)), Some(pick("fable", None)), "the CLI default is not a model of the list");
    assert_eq!(stronger_than(&cat("codex", vec![]), &pick("x", None)), None);
}

#[test]
fn codex_run_fails_loudly_when_the_schema_file_cannot_be_written() {
    let d = tempfile::tempdir().unwrap();
    let blocker = d.path().join("not-a-dir");
    std::fs::write(&blocker, "x").unwrap(); // a file where the folder must be
    let mut r = run(Role::Worker);
    r.cwd = d.path().to_path_buf();
    r.json_schema = Some("{}".into());
    let codex = Codex::with_bin("codex").with_schema_dir(&blocker.join("schemas"));
    let evs = run_blocking(&codex, &r, &mut |_| {}, &AtomicBool::new(false)).unwrap();
    let [AgentEvent::Done { is_error: true, text, .. }] = evs.as_slice() else { panic!("{evs:?}") };
    assert!(text.contains("cannot write the answer schema file"), "{text}");
}

#[test]
fn broad_words_in_the_tail_of_the_output_are_not_a_login_or_model_error() {
    // a failed run whose own error is plain, but whose log tail mentions unauthorized / invalid model (test output)
    let all = "{\"type\":\"error\",\"message\":\"turn failed\"}\ntest auth::unauthorized_is_rejected ... ok\nassert invalid model name rejected\n";
    let evs = Codex::new().finish(false, all);
    assert!(matches!(evs.as_slice(), [AgentEvent::Done { is_error: true, .. }]), "{evs:?}");
    // the same words in the CLI's own error message do count
    let own = "{\"type\":\"error\",\"message\":\"401 Unauthorized\"}\n";
    assert!(matches!(Codex::new().finish(false, own).as_slice(), [AgentEvent::AuthRequired(_)]));
}

// ---- v3: actions with input, output and diff; streamed text (stories 225-231) ----

fn actions_of(evs: &[AgentEvent]) -> Vec<ToolAction> {
    evs.iter().filter_map(|e| if let AgentEvent::Action(a) = e { Some(a.clone()) } else { None }).collect()
}

#[test]
fn claude_live_stream_gives_each_action_with_its_result_and_the_text_word_by_word() {
    // captured from claude 2.1.308 `-p --output-format stream-json --verbose --include-partial-messages`
    // (paths shortened to /repo, thinking and input deltas left out)
    let path = format!("{}/tests/fixtures/claude_stream_partial.jsonl", env!("CARGO_MANIFEST_DIR"));
    let evs = replay_path(&Claude::new(), &path, true, vec![]);
    let acts = actions_of(&evs);
    let read = |p: &str| ActionInput::Read { path: p.into() };
    let edit_diff = "--- a/repo/calc.py\n+++ b/repo/calc.py\n@@ -1,2 +1,3 @@\n def add(a, b):\n-    return a + b\n+    s = a + b\n+    return s\n";
    let notes = ActionInput::Write { path: "/repo/notes.txt".into(), head: "alpha\nbeta\ngamma".into(), lines: 3 };
    let want_done = vec![
        end("toolu_01HSTpFejLcZRnjmR1wut3bK", "Read", read("/repo/calc.py"), None, true),
        end("toolu_01JiVM761xCKLda52CQB8Hfa", "Edit", ActionInput::Edit { path: "/repo/calc.py".into(), diff: edit_diff.into() }, None, true),
        end("toolu_016hXt1MRnX5Wk5t3xPh2D4X", "Write", notes.clone(), None, true),
        end("toolu_01Ldpu8vBvCLfNgdv6m8Q3Df", "Bash", bash("echo hello; ls"), Some("hello"), true),
        end(
            "toolu_018mv9gwgjfTdxpV7wbkGHGn",
            "Read",
            read("/repo/missing_file.py"),
            Some("File does not exist. Note: your current working directory is /repo."),
            false,
        ),
    ];
    let got_done: Vec<AgentEvent> = acts.iter().filter(|a| a.done).cloned().map(AgentEvent::Action).collect();
    assert_eq!(got_done, want_done);
    // every action came first at its start, before its result, with the same id
    let starts: Vec<(&str, &str)> = acts.iter().filter(|a| !a.done).map(|a| (a.id.as_str(), a.name.as_str())).collect();
    assert_eq!(
        starts,
        [
            ("toolu_01HSTpFejLcZRnjmR1wut3bK", "Read"),
            ("toolu_01JiVM761xCKLda52CQB8Hfa", "Edit"),
            ("toolu_016hXt1MRnX5Wk5t3xPh2D4X", "Write"),
            ("toolu_01Ldpu8vBvCLfNgdv6m8Q3Df", "Bash"),
            ("toolu_018mv9gwgjfTdxpV7wbkGHGn", "Read")
        ]
    );
    assert_eq!(acts.iter().find(|a| !a.done && a.name == "Write").unwrap().input, notes);
    // the pieces of streamed text make the same text as the whole messages
    let deltas: String = evs.iter().filter_map(|e| if let AgentEvent::TextDelta(t) = e { Some(t.as_str()) } else { None }).collect();
    let texts: String = evs.iter().filter_map(|e| if let AgentEvent::Text(t) = e { Some(t.as_str()) } else { None }).collect();
    assert!(evs.iter().filter(|e| matches!(e, AgentEvent::TextDelta(_))).count() > 10);
    assert_eq!(deltas, texts);
    assert!(texts.starts_with("TodoWrite isn't among the tools") && texts.ends_with("missing_file.py was confirmed missing."), "{texts}");
}

#[test]
fn claude_runs_with_partial_messages() {
    let d = tempfile::tempdir().unwrap();
    let c = Claude::with_bin("/bin/true").command(&Run::new(d.path().to_path_buf(), "p".into(), String::new(), Role::Worker));
    let args: Vec<String> = c.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    assert!(args.iter().any(|a| a == "--include-partial-messages"), "{args:?}");
}

#[test]
fn a_plan_tool_becomes_a_checklist() {
    let todo = serde_json::json!({"todos": [
        {"content": "read", "status": "completed", "activeForm": "Reading"},
        {"content": "edit", "status": "in_progress", "activeForm": "Editing"},
        {"content": "run", "status": "pending", "activeForm": "Running"},
        {"content": "docs", "status": "cancelled"}
    ]});
    let a = actions::tool_action("t1", "TodoWrite", &todo);
    let item = |t: &str, s: PlanState| PlanItem { text: t.into(), state: s };
    assert_eq!(
        a.input,
        ActionInput::Plan { items: vec![item("read", PlanState::Done), item("edit", PlanState::Active), item("run", PlanState::Pending), item("docs", PlanState::Dropped)] }
    );
    // Codex `update_plan` (plan: [{step, status}]) and opencode `todowrite` read the same way
    let plan = actions::tool_action("t2", "update_plan", &serde_json::json!({"plan": [{"step": "a", "status": "completed"}, {"step": "b", "status": "in_progress"}]}));
    assert_eq!(plan.input, ActionInput::Plan { items: vec![item("a", PlanState::Done), item("b", PlanState::Active)] });
    let oc = actions::tool_action("t3", "todowrite", &serde_json::json!({"todos": [{"content": "x", "status": "pending", "id": "1"}]}));
    assert_eq!(oc.input, ActionInput::Plan { items: vec![item("x", PlanState::Pending)] });
}

#[test]
fn edits_and_new_files_are_unified_diffs() {
    // a change of several lines inside a file: 3 lines of context, the file's own line numbers
    let old = "a\nb\nc\nd\ne\nf\n";
    let new = "a\nb\nC\nC2\nd\ne\nf\n";
    assert_eq!(actions::unified_diff("src/x.rs", Some(old), Some(new)), "--- a/src/x.rs\n+++ b/src/x.rs\n@@ -1,6 +1,7 @@\n a\n b\n-c\n+C\n+C2\n d\n e\n f\n");
    // a new file: every line added
    assert_eq!(actions::unified_diff("n.txt", None, Some("x\ny\n")), "--- /dev/null\n+++ b/n.txt\n@@ -0,0 +1,2 @@\n+x\n+y\n");
    // MultiEdit at its start: the pieces it replaces
    let me = actions::tool_action(
        "t",
        "MultiEdit",
        &serde_json::json!({"file_path": "m.py", "edits": [{"old_string": "a = 1", "new_string": "a = 2"}, {"old_string": "b = 1\nc = 1", "new_string": "b = 2"}]}),
    );
    assert_eq!(me.input, ActionInput::Edit { path: "m.py".into(), diff: "--- a/m.py\n+++ b/m.py\n@@ -1,3 +1,2 @@\n-a = 1\n-b = 1\n-c = 1\n+a = 2\n+b = 2\n".into() });
}

#[test]
fn long_output_and_diff_are_cut_and_secrets_never_leave_the_run() {
    let secret = "s3cr3t-value-123";
    let d = tempfile::tempdir().unwrap();
    let big_out = format!("TOKEN={secret}\n{}", "x".repeat(70_000));
    let big_new = "y\n".repeat(130_000);
    let lines = [
        serde_json::json!({"type":"stream_event","parent_tool_use_id":null,"event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"key is s3cr3"}}}),
        serde_json::json!({"type":"stream_event","parent_tool_use_id":null,"event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"t-value-123 ok"}}}),
        serde_json::json!({"type":"assistant","message":{"content":[{"type":"text","text":format!("key is {secret} ok")}]}}),
        serde_json::json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":format!("echo {secret}")}}]}}),
        serde_json::json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":big_out}]}}),
        serde_json::json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"t2","name":"Write","input":{"file_path":"big.txt","content":"z"}}]}}),
        serde_json::json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t2","content":"ok"}]},"tool_use_result":{"type":"update","filePath":"big.txt","originalFile":"z\n","content":big_new}}),
        serde_json::json!({"type":"result","subtype":"success","is_error":false,"result":format!("done {secret}"),"session_id":"s1"}),
    ];
    let path = d.path().join("stream.jsonl");
    std::fs::write(&path, lines.iter().map(|l| l.to_string() + "\n").collect::<String>()).unwrap();
    let evs = replay_path(&Claude::new(), path.to_str().unwrap(), true, vec![("API_TOKEN".into(), secret.into())]);
    assert!(!format!("{evs:?}").contains(secret), "the value is nowhere in what the run says");
    let deltas: String = evs.iter().filter_map(|e| if let AgentEvent::TextDelta(t) = e { Some(t.as_str()) } else { None }).collect();
    assert_eq!(deltas, "key is ${API_TOKEN} ok", "a value split between two pieces is caught too");
    let acts = actions_of(&evs);
    let bash = acts.iter().find(|a| a.id == "t1" && a.done).unwrap();
    assert_eq!(bash.input, ActionInput::Bash { command: "echo ${API_TOKEN}".into() });
    let out = bash.output.as_deref().unwrap();
    assert!(out.starts_with("TOKEN=${API_TOKEN}\nxxx"), "{}", &out[..40]);
    assert!(out.len() <= OUTPUT_MAX + 64 && out.ends_with(&format!("[output_truncated: {} bytes]", big_out.len() - secret.len() + "${API_TOKEN}".len())), "{}", &out[out.len() - 60..]);
    let ActionInput::Edit { diff, .. } = &acts.iter().find(|a| a.id == "t2" && a.done).unwrap().input else { panic!("an overwrite is an edit") };
    assert!(diff.len() <= DIFF_MAX + 64 && diff.contains("\n[diff_truncated: "), "{}", &diff[diff.len() - 60..]);
    assert!(diff.starts_with("--- a/big.txt\n+++ b/big.txt\n@@ -1 +1,130000 @@\n-z\n+y\n"), "{}", &diff[..80]);
}

// ---- review of task 16: secrets at the edge of a cut, in secret files, in .env files below the top ----

#[test]
fn a_secret_on_the_edge_of_the_diff_cut_is_scrubbed_before_the_cut() {
    let secret = "s3cr3t-value-123";
    let prefix = "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-z\n+";
    // the secret starts 5 bytes before the cut: a cut before the scrub would leave "s3cr3" in the diff
    let line = format!("{}{secret}", "x".repeat(DIFF_MAX - 5 - prefix.len()));
    let mut a = actions::tool_action("t", "Edit", &serde_json::json!({"file_path": "f", "old_string": "z", "new_string": line}));
    actions::finish_action(&mut a, &Scrub::new([("API_TOKEN".to_string(), secret.to_string())]));
    let ActionInput::Edit { diff, .. } = &a.input else { panic!() };
    assert!(diff.contains("[diff_truncated: "), "the diff is cut");
    assert!(!diff.contains("s3cr3"), "{}", &diff[DIFF_MAX - 20..DIFF_MAX + 10]);
}

#[test]
fn the_text_of_a_secret_file_never_shows_only_its_path() {
    let scrub = Scrub::default();
    let hidden = |name: &str, input: serde_json::Value| {
        let mut a = actions::tool_action("t", name, &input);
        actions::finish_action(&mut a, &scrub);
        a.input
    };
    let e = hidden("Edit", serde_json::json!({"file_path": "/repo/backend/.env", "old_string": "A=1", "new_string": "A=new-secret-1"}));
    assert_eq!(e, ActionInput::Edit { path: "/repo/backend/.env".into(), diff: actions::SECRET_HIDDEN.into() });
    let w = hidden("Write", serde_json::json!({"file_path": "keys/server.key", "content": "-----BEGIN KEY-----\nabc\n"}));
    assert_eq!(w, ActionInput::Write { path: "keys/server.key".into(), head: actions::SECRET_HIDDEN.into(), lines: 2 });
    // an ordinary file keeps its diff
    assert!(matches!(hidden("Edit", serde_json::json!({"file_path": "a.py", "old_string": "a", "new_string": "b"})), ActionInput::Edit { diff, .. } if diff.contains("+b")));
}

#[test]
fn values_of_env_files_below_the_top_of_the_copy_are_scrubbed_too() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("backend/config")).unwrap();
    std::fs::write(d.path().join("backend/.env"), "API_KEY=deepsecret-999\n").unwrap();
    std::fs::write(d.path().join("backend/config/.env.local"), "DB_PASS=\"nested-pass-42\"\n").unwrap();
    let run = Run::new(d.path().to_path_buf(), "p".into(), String::new(), Role::Worker);
    let sc = Scrub::for_run(&run);
    assert_eq!(sc.text("deepsecret-999 / nested-pass-42"), "${API_KEY} / ${DB_PASS}");
}

// ---- live run 23.10 (F2): symlinked .env, scrub before every cut, wide .env search, paths, our own tools ----

#[test]
fn a_symlinked_env_file_in_the_copy_is_read() {
    let (copy, outside) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    std::fs::write(outside.path().join("real.env"), "LINK_KEY=linked-secret-77\n").unwrap();
    std::os::unix::fs::symlink(outside.path().join("real.env"), copy.path().join(".env")).unwrap();
    // a symlinked folder is still not entered
    std::fs::create_dir_all(outside.path().join("other")).unwrap();
    std::fs::write(outside.path().join("other/.env"), "FAR_KEY=far-away-secret\n").unwrap();
    std::os::unix::fs::symlink(outside.path().join("other"), copy.path().join("other")).unwrap();
    let sc = Scrub::for_run(&Run::new(copy.path().to_path_buf(), "p".into(), String::new(), Role::Worker));
    assert_eq!(sc.text("linked-secret-77 far-away-secret"), "${LINK_KEY} far-away-secret");
}

#[test]
fn a_multiline_secret_across_the_head_of_a_new_file_is_scrubbed_before_the_cut() {
    let pem = "-----BEGIN KEY-----\nMIIBsecretline1\nMIIBsecretline2\nMIIBsecretline3\n-----END KEY-----";
    let content = format!("{}{pem}\ntail\n", (1..=18).map(|i| format!("line{i}\n")).collect::<String>());
    let mut a = actions::tool_action("t", "Write", &serde_json::json!({"file_path": "cfg.txt", "content": content}));
    actions::finish_action(&mut a, &Scrub::new([("PEM_KEY".to_string(), pem.to_string())]));
    let ActionInput::Write { head, lines, .. } = &a.input else { panic!() };
    assert!(!head.contains("MIIB") && !head.contains("BEGIN KEY"), "{head}");
    assert!(head.contains("${PEM_KEY}"), "{head}");
    assert_eq!(head.lines().count(), actions::HEAD_LINES, "{head}");
    assert_eq!(*lines, 24, "the count is of the real file");
}

#[test]
fn a_secret_across_the_cut_of_a_one_line_summary_is_scrubbed_before_the_cut() {
    let secret = "s3cr3t-value-123";
    let input = serde_json::json!({"description": format!("{} {secret}", "x".repeat(115))});
    let mut a = actions::tool_action("t", "Task", &input);
    actions::finish_action(&mut a, &Scrub::new([("API_TOKEN".to_string(), secret.to_string())]));
    let ActionInput::Other { summary } = &a.input else { panic!() };
    assert!(!summary.contains("s3cr"), "{summary}");
    assert!(summary.starts_with(&"x".repeat(115)) && summary.chars().count() <= 121, "{summary}");
}

#[test]
fn an_env_file_is_found_even_beside_a_folder_with_hundreds_of_folders() {
    let d = tempfile::tempdir().unwrap();
    for i in 0..600 {
        std::fs::create_dir_all(d.path().join(format!("docs/d{i}"))).unwrap();
    }
    for i in 0..600 {
        std::fs::create_dir_all(d.path().join(format!("examples/e{i}"))).unwrap();
    }
    std::fs::create_dir_all(d.path().join("server")).unwrap();
    std::fs::write(d.path().join("server/.env"), "SERVER_KEY=server-secret-5\n").unwrap();
    let sc = Scrub::for_run(&Run::new(d.path().to_path_buf(), "p".into(), String::new(), Role::Worker));
    assert_eq!(sc.text("server-secret-5"), "${SERVER_KEY}");
}

/// What `run_blocking` makes of a Claude stream of `lines` run in `cwd`, with an answer schema or not.
fn replay_lines(cwd: &std::path::Path, lines: &[serde_json::Value], schema: bool) -> Vec<AgentEvent> {
    let f = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(f.path(), lines.iter().map(|l| l.to_string() + "\n").collect::<String>()).unwrap();
    let mut run = Run::new(cwd.to_path_buf(), "p".into(), String::new(), Role::Worker);
    run.json_schema = schema.then(|| "{}".to_string());
    let claude = Claude::new();
    let r = Replay { inner: &claude, path: f.path().to_string_lossy().into_owned(), exit_ok: true };
    run_blocking(&r, &run, &mut |_| {}, &AtomicBool::new(false)).unwrap()
}

fn tool_use(id: &str, name: &str, input: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":id,"name":name,"input":input}]}})
}
fn tool_result(id: &str, rec: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":id,"content":"ok"}]},"tool_use_result":rec})
}

#[test]
fn paths_in_actions_are_relative_to_the_copy_and_outside_ones_start_with_home() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().canonicalize().unwrap();
    let home = std::env::var("HOME").unwrap();
    let (inside, real_inside) = (d.path().join("calc.py"), root.join("src/m.py"));
    let lines = [
        tool_use("r1", "Read", serde_json::json!({"file_path": inside})),
        tool_result("r1", serde_json::json!({})),
        tool_use("e1", "Edit", serde_json::json!({"file_path": real_inside, "old_string": "a", "new_string": "b"})),
        tool_result("e1", serde_json::json!({"filePath": real_inside, "structuredPatch": [{"oldStart":1,"oldLines":1,"newStart":1,"newLines":1,"lines":["-a","+b"]}]})),
        tool_use("r2", "Read", serde_json::json!({"file_path": format!("{home}/notes/x.md")})),
        tool_result("r2", serde_json::json!({})),
        tool_use("r3", "Read", serde_json::json!({"file_path": "/etc/hosts"})),
        tool_result("r3", serde_json::json!({})),
    ];
    let acts = actions_of(&replay_lines(d.path(), &lines, false));
    let path_of = |id: &str| match &acts.iter().find(|a| a.id == id && a.done).unwrap().input {
        ActionInput::Read { path } | ActionInput::Edit { path, .. } => path.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(path_of("r1"), "calc.py");
    assert_eq!(path_of("e1"), "src/m.py");
    assert_eq!(path_of("r2"), "~/notes/x.md");
    assert_eq!(path_of("r3"), "/etc/hosts");
    let start = acts.iter().find(|a| a.id == "r1" && !a.done).unwrap();
    assert_eq!(start.input, ActionInput::Read { path: "calc.py".into() }, "the start too");
    let ActionInput::Edit { diff, .. } = &acts.iter().find(|a| a.id == "e1" && a.done).unwrap().input else { panic!() };
    assert!(diff.starts_with("--- a/src/m.py\n+++ b/src/m.py\n@@ -1,1 +1,1 @@\n"), "{diff}");
}

#[test]
fn the_structured_answer_of_our_roles_is_a_log_line_not_an_action() {
    let d = tempfile::tempdir().unwrap();
    let lines = [
        tool_use("b1", "Bash", serde_json::json!({"command": "ls"})),
        serde_json::json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"b1","content":"a"}]}}),
        tool_use("so", "StructuredOutput", serde_json::json!({"verdict": "approve", "notes": "fine"})),
        serde_json::json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"so","content":"Structured output provided successfully"}]}}),
        serde_json::json!({"type":"result","subtype":"success","is_error":false,"result":"","session_id":"s1"}),
    ];
    let evs = replay_lines(d.path(), &lines, true);
    let acts = actions_of(&evs);
    assert!(acts.iter().all(|a| a.name != "StructuredOutput"), "{acts:?}");
    assert_eq!(acts.iter().filter(|a| a.name == "Bash").count(), 2, "other tools stay actions");
    let tools: Vec<_> = evs.iter().filter_map(|e| if let AgentEvent::Tool { name, summary } = e { Some((name.clone(), summary.clone())) } else { None }).collect();
    assert_eq!(tools.len(), 1, "{tools:?}");
    assert_eq!(tools[0].0, "StructuredOutput");
    assert!(tools[0].1.contains("approve"), "{tools:?}");
}

#[test]
fn a_model_that_reads_as_a_flag_never_reaches_argv() {
    let mut r = run(Role::Worker);
    r.model = Some("--yolo".into());
    for a in [&Qwen::new() as &dyn Adapter, &Gemini::new(), &Codex::new(), &OpenCode::new(), &Claude::new(), &Cursor::new()] {
        let v = args(a, &r);
        assert!(!v.windows(2).any(|p| (p[0] == "-m" || p[0] == "--model") && p[1] == "--yolo"), "{}: {v:?}", a.id());
    }
    r.model = Some("qwen3-coder-plus".into());
    assert_eq!(arg_after(&args(&Qwen::new(), &r), "-m"), "qwen3-coder-plus");
}
