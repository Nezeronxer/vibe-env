//! Adapter seam: parse/finish on saved engine output (core/tests/fixtures/agents), command lines,
//! availability, limit detection, own command. Files named `*-from-docs*` were assembled from the
//! engine's documentation, the others were captured from real runs on the dev machine.
use chrono::{Duration, Local, Timelike};
use nightshift_core::agent::adapters::*;
use nightshift_core::agent::*;
use nightshift_core::model::{CustomAgent, Settings};
use std::sync::atomic::AtomicBool;

fn fixture(name: &str) -> String {
    let p = format!("{}/tests/fixtures/agents/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

/// What run_blocking does with a process that printed `name`: parse every line, then finish().
fn replay(a: &dyn Adapter, name: &str, exit_ok: bool) -> Vec<AgentEvent> {
    let text = fixture(name);
    let mut evs: Vec<AgentEvent> = text.lines().flat_map(|l| a.parse(l)).collect();
    evs.extend(a.finish(exit_ok, &text));
    evs
}

fn tool(name: &str, summary: &str) -> AgentEvent {
    AgentEvent::Tool { name: name.into(), summary: summary.into() }
}
fn text(t: &str) -> AgentEvent {
    AgentEvent::Text(t.into())
}
fn done(session: &str, t: &str) -> AgentEvent {
    AgentEvent::Done { session: Some(session.into()), text: t.into(), is_error: false }
}

#[test]
fn claude_real_run_text_tool_and_done_with_session() {
    // fixture captured from `claude -p "run echo hi, then say ok" --output-format stream-json --verbose`
    let evs = replay(&Claude::new(), "claude.jsonl", true);
    assert_eq!(
        evs,
        vec![tool("Bash", "echo hi"), text("ok"), done("fe525380-9073-47bf-9375-54c8ca3c6d08", "ok")]
    );
}

#[test]
fn opencode_real_run_text_tool_and_done_with_session() {
    let evs = replay(&OpenCode::new(), "opencode.jsonl", true);
    assert_eq!(evs, vec![tool("bash", "echo hi"), text("ok"), done("ses_ea2aa5d4affeYzoOqNuIEz6nKL", "ok")]);
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
    let evs = replay(&Qwen::new(), "qwen-from-docs.jsonl", true);
    assert_eq!(evs, vec![tool("run_shell_command", "ls"), text("ok"), done("qwen-sess-1", "ok")]);
    // captured from the installed `qwen` without credentials
    let evs = replay(&Qwen::new(), "qwen-error.jsonl", false);
    assert_eq!(
        evs,
        vec![AgentEvent::Done {
            session: Some("c70e4c45-86b0-4f12-bdd9-426303ebf043".into()),
            text: "No auth type is selected. Please configure an auth type (e.g. via settings or `--auth-type`) before running in non-interactive mode.".into(),
            is_error: true
        }]
    );
}

#[test]
fn codex_events_text_tool_done_and_error() {
    let evs = replay(&Codex::new(), "codex-from-docs.jsonl", true);
    assert_eq!(
        evs,
        vec![tool("shell", "bash -lc ls"), text("Repo contains docs and src. ok"), done("0199a213-81c0-7800-8aa1-bbab2a035a53", "Repo contains docs and src. ok")]
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
    assert_eq!(evs, vec![tool("run_shell_command", "ls"), text("o"), text("k"), done("gem-sess-1", "ok")]);
    let evs = replay(&Gemini::new(), "gemini-error-from-docs.jsonl", false);
    assert!(matches!(evs.as_slice(), [AgentEvent::Limit { resets_at: None }]), "{evs:?}");
}

#[test]
fn cursor_events_tool_text_done() {
    let evs = replay(&Cursor::new(), "cursor-from-docs.jsonl", true);
    assert_eq!(evs, vec![tool("write", "notes.txt"), text("ok"), done("cur-sess-1", "ok")]);
}

#[test]
fn cursor_crash_without_result_is_a_failed_done() {
    let a = Cursor::new();
    let evs = a.finish(false, "{\"type\":\"system\"}\nerror: not logged in\n");
    assert_eq!(evs, vec![AgentEvent::Done { session: None, text: "error: not logged in".into(), is_error: true }]);
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
    assert_eq!((utc.hour(), utc.minute()), (12, 0));
    assert!(t > now && t <= now + Duration::hours(24));

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
    Run { cwd: "/tmp".into(), prompt: "do it".into(), system: "no push".into(), resume: None, role }
}

fn args(a: &dyn Adapter, r: &Run) -> Vec<String> {
    a.command(r).get_args().map(|s| s.to_string_lossy().into_owned()).collect()
}

fn arg_after(v: &[String], flag: &str) -> String {
    let i = v.iter().position(|a| a == flag).unwrap_or_else(|| panic!("{flag} missing in {v:?}"));
    v[i + 1].clone()
}

#[test]
fn claude_worker_bans_push_remote_sudo_and_never_denies_read() {
    let v = args(&Claude::new(), &run(Role::Worker));
    assert_eq!(arg_after(&v, "--permission-mode"), "bypassPermissions");
    let settings: serde_json::Value = serde_json::from_str(&arg_after(&v, "--settings")).unwrap();
    let deny: Vec<&str> = settings["permissions"]["deny"].as_array().unwrap().iter().map(|d| d.as_str().unwrap()).collect();
    for d in ["Bash(git push:*)", "Bash(git remote:*)", "Bash(sudo:*)"] {
        assert!(deny.contains(&d), "{d} missing in {deny:?}");
    }
    // Read(...) denies become OS-level bans in the sandbox and break TLS (.pem) for all subprocesses
    assert!(deny.iter().all(|d| !d.starts_with("Read")), "{deny:?}");
    assert!(!arg_after(&v, "--settings").contains("Read("));
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
        let ok = v.iter().any(|x| x == "--prompt=--help me")
            || v.iter().position(|x| x == "--").is_some_and(|i| v.get(i + 1).is_some_and(|p| p == "--help me"));
        assert!(ok, "{}: {v:?}", a.id());
    }
}

#[test]
fn opencode_uses_session_dir_and_reviewer_agent_plan() {
    let mut r = run(Role::Worker);
    r.resume = Some("ses_1".into());
    let v = args(&OpenCode::new(), &r);
    assert_eq!(arg_after(&v, "--session"), "ses_1");
    assert_eq!(arg_after(&v, "--dir"), "/tmp");
    assert_eq!(arg_after(&v, "--format"), "json");
    assert_eq!(arg_after(&args(&OpenCode::new(), &run(Role::Reviewer)), "--agent"), "plan");
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
    for a in [&OpenCode::new() as &dyn Adapter, &Codex::new(), &Gemini::new(), &Cursor::new(), &Qwen::new()] {
        let v = args(a, &run(Role::Worker));
        assert!(v.iter().any(|x| x == "no push\n\ndo it" || x == "--prompt=no push\n\ndo it"), "{} -> {v:?}", a.id());
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
    ];
    for (a, hint) in cases {
        let e = a.available().unwrap_err();
        assert!(e.starts_with("not installed: ") && e.contains(hint), "{e}");
    }
    assert!(Claude::with_bin("sh").available().is_ok());
}

#[test]
fn adapters_lists_six_builtin_engines_then_the_custom_ones() {
    let mut s = Settings::default();
    s.custom_agents = vec![CustomAgent { id: "mine".into(), name: "Mine".into(), run: "echo {prompt}".into(), resume: None }];
    let ids: Vec<String> = adapters(&s).iter().map(|a| a.id().to_string()).collect();
    assert_eq!(ids, ["claude", "codex", "gemini", "opencode", "cursor", "qwen", "mine"]);
    assert_eq!(adapters(&Settings::default()).len(), 6);
}

#[test]
fn custom_command_cannot_shadow_a_builtin_engine() {
    let mut s = Settings::default();
    let c = |id: &str| CustomAgent { id: id.into(), name: "X".into(), run: "true".into(), resume: None };
    s.custom_agents = vec![c("claude"), c("mine"), c("mine")];
    let list = adapters(&s);
    let ids: Vec<&str> = list.iter().map(|a| a.id()).collect();
    assert_eq!(ids, ["claude", "codex", "gemini", "opencode", "cursor", "qwen", "mine"]);
    assert_eq!(list[0].name(), "Claude Code");
}

#[test]
fn only_claude_is_guarded() {
    let mut s = Settings::default();
    s.custom_agents = vec![CustomAgent { id: "mine".into(), name: "Mine".into(), run: "true".into(), resume: None }];
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
