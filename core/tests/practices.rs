//! Practices from the videos (stories 305-312) and the commit body (292). Main seam: Engine + FakeAdapter + temp repo;
//! the Claude parser on the saved real run; the startup tax on a temp home.
mod common;
use common::*;
use nightshift_core::agent::adapters::Claude;
use nightshift_core::agent::{tax, Adapter, AgentEvent, FakeAdapter, FakeStep, Run};
use nightshift_core::model::*;
use nightshift_core::notify::RecordingNotifier;
use std::fs;
use std::sync::Arc;

fn quiet(id: &str) -> FakeAdapter {
    FakeAdapter::new(vec![]).with_id(id).with_role(RoleKind::Reviewer, (0..6).map(|_| approve()).collect())
}

#[test]
fn a_switch_to_the_spare_model_leaves_a_handoff_note_in_the_copy_and_the_next_prompt_starts_with_it() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(quiet("fa").with_role(
        RoleKind::Worker,
        vec![
            vec![write("half.txt", "half"), FakeStep::Say("Wrote half of it, chose plain text".into()), FakeStep::Limit],
            vec![FakeStep::Limit], // the spare is used up too: the task waits with its copy
        ],
    ));
    let e = env.engine(env.store(&p, |s| env_models(s, "fa", &["big", "spare"], "big", &[], Some("spare"))), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Add the feature");
    let t = wait_status(&e, &id, Status::PausedLimit);
    let note = t.handoff.clone().expect("a note on the task");
    assert!(note.starts_with("HANDOFF NOTE (limit_fallback)"), "{note}");
    assert!(note.contains("Goal: Add the feature") && note.contains("half.txt") && note.contains("Wrote half of it"), "{note}");
    let file = t.worktree.clone().unwrap().join(".nightshift/handoff.md");
    assert_eq!(fs::read_to_string(file).unwrap(), note);
    let calls = calls_of(&fa, RoleKind::Worker);
    assert_eq!(calls.len(), 2);
    assert!(!calls[0].prompt.starts_with("HANDOFF"));
    assert!(calls[1].prompt.starts_with(&note), "{}", calls[1].prompt);
    assert!(has_log(&e, &id, "handoff: limit_fallback"));
    // the note never goes into a commit
    assert!(!g(&t.worktree.unwrap(), &["log", "--name-only", "--format="]).contains(".nightshift"));
}

#[test]
fn five_hundred_same_lines_reach_the_agent_as_a_counter_and_a_failed_check_gives_the_raw_output_path() {
    let env = Env::new();
    let p = env.project(|p| p.check_cmd = Some("i=0; while [ $i -lt 500 ]; do echo 'same line'; i=$((i+1)); done; exit 1".into()));
    let w = fake(vec![vec![write("b.txt", "b"), done("did b")], vec![write("c.txt", "c"), done("did c")]]);
    let e = env.engine(env.store(&p, |s| s.attempts = 2), vec![w.clone(), reviewer(2)], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Add b");
    let t = wait_status(&e, &id, Status::Failed);
    let second = &calls_of(&w, RoleKind::Worker)[1].prompt;
    assert_eq!(second.matches("same line").count(), 1, "{second}");
    assert!(second.contains("same line  (×500)"), "{second}");
    let raw = t.worktree.unwrap().join(".nightshift/out/tests.log");
    assert!(second.contains(&raw.display().to_string()), "{second}");
    assert_eq!(fs::read_to_string(&raw).unwrap().lines().filter(|l| *l == "same line").count(), 500, "the raw output is whole");
}

#[test]
fn without_squeezing_the_agent_gets_the_tail_as_it_was_and_still_the_path() {
    let env = Env::new();
    let p = env.project(|p| p.check_cmd = Some("i=0; while [ $i -lt 500 ]; do echo 'same line'; i=$((i+1)); done; exit 1".into()));
    let w = fake(vec![vec![write("b.txt", "b"), done("did b")], vec![done("did c")]]);
    let e = env.engine(
        env.store(&p, |s| {
            s.attempts = 2;
            s.compress_output = false;
        }),
        vec![w.clone(), reviewer(2)],
        Arc::new(RecordingNotifier::default()),
        None,
    );
    let id = submit(&e, &p, "Add b");
    wait_status(&e, &id, Status::Failed);
    let second = &calls_of(&w, RoleKind::Worker)[1].prompt;
    assert_eq!(second.matches("same line").count(), 60, "the last 60 lines, as before");
    assert!(second.contains(".nightshift/out/tests.log"));
}

#[test]
fn two_tasks_of_a_project_with_isolated_ports_get_different_ports_and_without_the_flag_none() {
    let env = Env::new();
    let p = env.project(|p| {
        p.agent = "fa".into();
        p.isolate_ports = true;
        p.check_cmd = Some("test -n \"$PORT\" && test \"$PORT\" = \"$NIGHTSHIFT_PORT_BASE\"".into());
    });
    let slow = |f: &str| vec![FakeStep::Sleep(1500), write(f, "x"), done("ok")];
    let fa = Arc::new(quiet("fa").with_role(RoleKind::Worker, vec![slow("a1.txt"), slow("a2.txt"), slow("a3.txt")]));
    let e = env.engine(env.store(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);
    let a = submit(&e, &p, "Task one");
    let b = submit(&e, &p, "Task two");
    let (ta, tb) = (wait_status(&e, &a, Status::Done), wait_status(&e, &b, Status::Done));
    let port = |r: &Run| r.extra_env.iter().find(|(k, _)| k == "PORT").map(|(_, v)| v.clone());
    let ports: Vec<Option<String>> = calls_of(&fa, RoleKind::Worker).iter().map(port).collect();
    assert_eq!(ports.len(), 2);
    assert!(ports.iter().all(Option::is_some), "{ports:?}");
    assert_ne!(ports[0], ports[1], "the gates saw the same PORT too (the check passed)");
    let mut got = vec![ta.port.unwrap(), tb.port.unwrap()];
    got.sort();
    assert_eq!(got, vec![4000, 4010]);
    assert!(ta.db_name.unwrap().starts_with("demo_"));

    // the same project without the flag: no variables
    let mut q = p.clone();
    q.isolate_ports = false;
    e.store().lock().unwrap().upsert_project(q.clone()).unwrap();
    let c = submit(&e, &q, "Task three");
    let tc = wait(&e, &c, "checked", |t| matches!(t.status, Status::Done | Status::Failed));
    let last = calls_of(&fa, RoleKind::Worker).pop().unwrap();
    assert_eq!(port(&last), None);
    assert_eq!((tc.port, tc.status), (None, Status::Failed), "the check needs PORT, so it is red without it");
}

#[test]
fn the_commit_body_is_the_first_paragraph_cut_at_a_sentence_never_mid_word() {
    let env = Env::new();
    let p = env.project(|_| {});
    let sentence = "Rewrote the parser so that every token keeps its position in the source file. ";
    let report = format!("{}\n\nSecond paragraph that must not be in the body.", sentence.repeat(26)); // ~2000 chars
    assert!(report.chars().count() > 2000);
    let w = fake(vec![vec![write("p.txt", "p"), done(&report)]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Rewrite the parser");
    wait_status(&e, &id, Status::Done);

    let msg = g(&env.repo, &["log", "-1", "--format=%B", "main"]);
    let mut parts = msg.split("\n\n");
    assert_eq!(parts.next(), Some("Rewrite the parser"));
    let body = parts.next().unwrap();
    assert!(body.chars().count() <= 500, "{}", body.chars().count());
    assert!(body.ends_with("file.…"), "ends after a whole sentence: {body}");
    // whole sentences only: 6 of them fit in 500 characters with the ellipsis (6 * 78 = 468, a 7th would be 546)
    assert_eq!(body, format!("{}…", sentence.repeat(6).trim_end()));
    assert_eq!(parts.next(), Some(format!("Nightshift-Task: {id}").as_str()), "the trailer the history reads");
}

#[test]
fn the_worker_session_context_share_comes_from_the_run() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = fake(vec![vec![FakeStep::Context(50_000, 200_000), write("x.txt", "x"), done("ok")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add x");
    assert_eq!(wait_status(&e, &id, Status::Done).context_pct, Some(25.0));
}

#[test]
fn claude_reports_the_context_in_use_and_the_window_of_a_real_run() {
    let text = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/agents/claude.jsonl")).unwrap();
    let c = Claude::new();
    let ctx: Vec<(Option<u64>, Option<u64>)> = text
        .lines()
        .flat_map(|l| c.parse(l))
        .filter_map(|e| match e {
            AgentEvent::Context { used, window } => Some((used, window)),
            _ => None,
        })
        .collect();
    // the last turn: input 2 + cache creation 2342 + cache read 40110; the window from `modelUsage`
    assert_eq!(ctx.iter().rev().find_map(|c| c.0), Some(2 + 2342 + 40110));
    assert_eq!(ctx.iter().rev().find_map(|c| c.1), Some(1_000_000));
}

#[test]
fn roles_get_their_tools_allowed_and_claude_passes_them_one_value_each() {
    let env = Env::new();
    let p = env.project(|p| p.check_cmd = Some("true".into()));
    let w = fake(vec![vec![write("x.txt", "x"), done("ok")]]);
    let e = env.engine(
        env.store(&p, |s| {
            s.role_tools.insert(RoleKind::Reviewer, vec!["Read".into()]);
        }),
        vec![w.clone(), reviewer(1)],
        Arc::new(RecordingNotifier::default()),
        None,
    );
    let id = submit(&e, &p, "Add x");
    wait_status(&e, &id, Status::Done);
    assert_eq!(calls_of(&w, RoleKind::Worker)[0].allowed_tools, vec!["Read", "Grep", "Glob", "Edit", "Write", "Bash(true)"]);

    let mut run = Run::new(std::env::temp_dir(), "p".into(), String::new(), RoleKind::Reviewer);
    run.allowed_tools = vec!["Read".into(), "Bash(cargo test)".into()];
    let cmd = Claude::with_bin("claude").command(&run);
    let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    let at = args.iter().position(|a| a == "--allowedTools").unwrap();
    assert_eq!(args[at + 1..at + 3].to_vec(), vec!["Read", "Bash(cargo test)"]);
    assert!(args[at + 3].starts_with("--"), "the list ends before the next flag: {args:?}");
}

#[test]
fn compact_runs_the_compact_command_in_the_session_before_the_next_turn() {
    let env = Env::new();
    let p = env.project(|_| {});
    // no separate reviewer adapter: the env itself answers `compact_prompt` (FakeAdapter has `/compact`)
    let w = Arc::new(quiet("fake").with_role(
        RoleKind::Worker,
        vec![vec![done("NEEDS_HUMAN: Which DB?")], vec![done("compacted")], vec![write("db.txt", "sqlite"), done("Used sqlite")]],
    ));
    let e = env.engine(env.store(&p, |_| {}), vec![w.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add storage");
    wait_status(&e, &id, Status::NeedsHuman);
    e.compact(&id, Some("the schema decisions")).unwrap();
    e.answer(&id, "sqlite").unwrap();
    let t = wait_status(&e, &id, Status::Done);
    let calls = calls_of(&w, RoleKind::Worker);
    assert_eq!(calls[1].prompt, "/compact the schema decisions");
    assert_eq!(calls[1].resume, calls[2].resume, "the same session goes on");
    assert!(calls[1].resume.is_some());
    assert!(calls[2].prompt.starts_with("HANDOFF NOTE (compact)"), "{}", calls[2].prompt);
    assert!(t.handoff.is_some());
}

#[test]
fn the_startup_tax_counts_rules_skill_descriptions_and_mcp_servers_and_flags_long_rules() {
    let d = tempfile::tempdir().unwrap();
    let (home, proj) = (d.path().join("home"), d.path().join("proj"));
    fs::create_dir_all(home.join(".claude/skills/pdf")).unwrap();
    fs::create_dir_all(proj.join(".claude/agents")).unwrap();
    fs::write(home.join(".claude/CLAUDE.md"), "rule line\n".repeat(250)).unwrap();
    fs::write(proj.join("CLAUDE.md"), "short\n").unwrap();
    fs::write(home.join(".claude/skills/pdf/SKILL.md"), "---\nname: pdf\ndescription: Read PDFs\n---\nA very long body that is not loaded until called.\n").unwrap();
    fs::write(proj.join(".claude/agents/rev.md"), "---\nname: rev\ndescription: Reviews\n---\nbody\n").unwrap();
    fs::write(home.join(".claude.json"), r#"{"mcpServers":{"github":{},"fs":{}}}"#).unwrap();
    fs::write(proj.join(".mcp.json"), r#"{"mcpServers":{"fs":{},"db":{}}}"#).unwrap();

    let t = tax::startup_tax("claude", &home, Some(&proj));
    assert_eq!(t.rules.len(), 2);
    let long = &t.rules[0];
    assert_eq!((long.lines, long.long, long.tokens), (250, true, 2500 / 4));
    assert!(!t.rules[1].long);
    // only `name:` and `description:` count: "name: pdf" (9) + "description: Read PDFs" (22) = 31 bytes -> 8 tokens
    assert_eq!((t.skills, t.skill_tokens), (1, 8));
    assert_eq!(t.agents, 1);
    assert_eq!(t.mcp_servers, vec!["github", "fs", "db"], "in the order of the config file");
    assert_eq!(t.total_tokens, 625 + 2 + 8 + t.agent_tokens);
}

#[test]
fn the_startup_tax_counts_the_text_of_a_block_description_too() {
    let d = tempfile::tempdir().unwrap();
    let home = d.path().join("home");
    fs::create_dir_all(home.join(".claude/skills/big")).unwrap();
    // `description: >-` and the two lines of its text count, the next key does not
    fs::write(home.join(".claude/skills/big/SKILL.md"), "---\nname: big\ndescription: >-\n  This skill reads\n  and writes things\nallowed-tools: x\n---\nbody\n").unwrap();
    let t = tax::startup_tax("claude", &home, None);
    // "name: big" 9 + "description: >-" 15 + "  This skill reads" 18 + "  and writes things" 20 = 62 bytes -> 16 tokens
    assert_eq!((t.skills, t.skill_tokens), (1, 16));
}
