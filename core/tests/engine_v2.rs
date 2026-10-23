//! Engine v2 through the main seam: Engine + FakeAdapter + a temporary git repo (spec "Швы" 1).
//! The pipeline of roles, "Quality", the prompt per vendor, one env per task.
mod common;
use common::*;
use nightshift_core::agent::{FakeAdapter, FakeStep};
use nightshift_core::model::*;
use nightshift_core::engine::NewTask;
use nightshift_core::notify::RecordingNotifier;
use std::sync::Arc;

#[test]
fn a_task_of_env_a_never_calls_env_b_each_role_has_its_own_session_and_the_v1_settings_only_leave_a_line() {
    let env = Env::new();
    let p = env.project(|p| {
        p.agent = "fa".into();
        p.check_cmd = Some("test -f feature.txt".into());
        p.reviewer = Some("fb".into()); // v1 settings: nobody reads them any more
        p.fallback_agent = Some("fb".into());
    });
    let fa = Arc::new(pipeline_fake_n("fa", 2));
    let fb = Arc::new(pipeline_fake("fb"));
    let e = env.engine(env.store_v2(&p, |_| {}), vec![fa.clone(), fb.clone()], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Add the feature file");
    let t = wait_status(&e, &id, Status::Done);

    assert!(fb.calls().is_empty(), "the other env was called: {:?}", roles_in_order(&fb));
    assert_eq!(
        roles_in_order(&fa),
        vec![RoleKind::Planner, RoleKind::Critic, RoleKind::TestAuthor, RoleKind::Worker, RoleKind::Reviewer]
    );
    // a role is a run of its own: its own session, none shared
    let sessions: Vec<String> = t.runs.iter().map(|r| r.session.clone().expect("every run has a session")).collect();
    assert_eq!(sessions.len(), 5);
    assert_eq!(fa.calls().len(), 5, "taken before the second task");
    let mut uniq = sessions.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(uniq.len(), 5, "{sessions:?}");
    assert!(fa.calls().iter().all(|c| c.resume.is_none()));
    // the one line about the v1 settings is in the project's log, once, even after more tasks
    let id2 = submit(&e, &p, "Add the feature file again");
    wait_status(&e, &id2, Status::Done);
    let project_log = log_texts(&e, &p.id);
    assert_eq!(project_log.iter().filter(|l| l.starts_with("v1_setting_ignored")).count(), 1, "{project_log:?}");
}

#[test]
fn full_pipeline_records_steps_and_each_role_gets_the_flag_of_its_own_model() {
    let env = Env::new();
    let p = env.project(|p| {
        p.agent = "fa".into();
        p.check_cmd = Some("test -f feature.txt".into());
    });
    let fa = Arc::new(pipeline_fake("fa"));
    let settings = |s: &mut Settings| {
        // planner and critic on one model, the executor and the test author on another, the reviewer on a third
        env_models(s, "fa", &["m-plan", "m-code", "m-review"], "m-code", &[(RoleKind::Planner, "m-plan"), (RoleKind::Critic, "m-plan"), (RoleKind::Reviewer, "m-review")], None);
    };
    let e = env.engine(env.store_v2(&p, settings), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Add the feature file");
    let t = wait_status(&e, &id, Status::Done);

    let models: Vec<(RoleKind, Option<String>)> = fa.calls().into_iter().map(|c| (c.role, c.model)).collect();
    assert_eq!(
        models,
        vec![
            (RoleKind::Planner, Some("m-plan".into())),
            (RoleKind::Critic, Some("m-plan".into())),
            (RoleKind::TestAuthor, Some("m-code".into())),
            (RoleKind::Worker, Some("m-code".into())),
            (RoleKind::Reviewer, Some("m-review".into())),
        ]
    );
    // the snapshot of the task and the runs it records
    assert_eq!(t.models.get(&RoleKind::Planner).map(|m| m.family.as_str()), Some("m-plan"));
    use StepState::{Done, Skipped};
    assert_eq!(
        t.steps,
        vec![
            (Step::Plan, Done),
            (Step::Spec, Skipped("not_vague".into())),
            (Step::Critique, Done),
            (Step::TestsFirst, Done),
            (Step::Code, Done),
            (Step::Gates, Done),
            (Step::E2e, Skipped("e2e_off".into())),
            (Step::Review, Done),
            (Step::Merge, Done),
        ]
    );
    let ran: Vec<(RoleKind, String)> = t.runs.iter().map(|r| (r.role, r.model.clone())).collect();
    assert_eq!(ran[0], (RoleKind::Planner, "m-plan".to_string()));
    assert_eq!(ran[4], (RoleKind::Reviewer, "m-review".to_string()));
    assert!(t.runs.iter().all(|r| matches!(r.state, StepState::Done) && r.started_at.is_some() && r.finished_at.is_some()));
    // the feed knows who spoke and on what model
    let log = e.store().lock().unwrap().read_log(&id, 500);
    assert!(log.iter().any(|l| l.role == Some(RoleKind::Planner) && l.model.as_deref() == Some("m-plan")));
    assert_eq!(env.main_file("feature.txt"), "hello");
}

#[test]
fn a_one_phrase_edit_skips_the_plan_and_the_critic_and_says_why() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(
        pipeline_fake("fa").with_role(RoleKind::Planner, vec![vec![done(&brief_json("Fix the typo in a.txt", &[], true, false, None))]]),
    );
    let e = env.engine(env.store_v2(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "fix the typo in a.txt");
    let t = wait_status(&e, &id, Status::Done);

    use StepState::Skipped;
    let state = |s: Step| t.steps.iter().find(|(x, _)| *x == s).unwrap().1.clone();
    assert_eq!(state(Step::Plan), Skipped("one_phrase_edit".into()));
    assert_eq!(state(Step::Critique), Skipped("one_phrase_edit".into()));
    assert_eq!(state(Step::Spec), Skipped("one_phrase_edit".into()));
    assert!(has_log(&e, &id, "plan_skipped: one_phrase_edit"));
    // only the planner thought about it; no critic ran, the executor still got the person's words
    assert_eq!(roles_in_order(&fa), vec![RoleKind::Planner, RoleKind::Worker, RoleKind::Reviewer]);
    assert!(calls_of(&fa, RoleKind::Worker)[0].prompt.contains("fix the typo in a.txt"));
}

#[test]
fn a_project_without_a_test_runner_skips_the_tests_step_with_the_reason_and_no_test_author_runs() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into()); // no check_cmd
    let fa = Arc::new(pipeline_fake("fa"));
    let e = env.engine(env.store_v2(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Add the feature file");
    let t = wait_status(&e, &id, Status::Done);

    let tests = t.steps.iter().find(|(s, _)| *s == Step::TestsFirst).unwrap();
    assert_eq!(tests.1, StepState::Skipped("no_test_runner".into()));
    assert!(calls_of(&fa, RoleKind::TestAuthor).is_empty());
    assert!(t.outcome.tests.is_none(), "no tests in project");
}

#[test]
fn preparatory_steps_that_fail_twice_are_skipped_and_the_task_goes_on_with_gates_and_review_intact() {
    let env = Env::new();
    let p = env.project(|p| {
        p.agent = "fa".into();
        p.check_cmd = Some("test -f feature.txt".into());
    });
    // the planner crashes twice, the test author crashes twice; the critic is never reached (no plan)
    let fa = Arc::new(
        pipeline_fake("fa")
            .with_role(RoleKind::Planner, vec![vec![FakeStep::Fail], vec![FakeStep::Fail]])
            .with_role(RoleKind::TestAuthor, vec![vec![FakeStep::Fail], vec![FakeStep::Fail]]),
    );
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store_v2(&p, |_| {}), vec![fa.clone()], n.clone(), None);

    let id = submit(&e, &p, "Add the feature file");
    let t = wait_status(&e, &id, Status::Done);

    let state = |s: Step| t.steps.iter().find(|(x, _)| *x == s).unwrap().1.clone();
    assert!(matches!(state(Step::Plan), StepState::Skipped(ref w) if w.starts_with("failed:")), "{:?}", state(Step::Plan));
    assert_eq!(state(Step::Critique), StepState::Skipped("no_plan".into()));
    assert!(matches!(state(Step::TestsFirst), StepState::Skipped(ref w) if w.starts_with("failed:")));
    // gates and review are never skipped
    assert_eq!(state(Step::Gates), StepState::Done);
    assert_eq!(state(Step::Review), StepState::Done);
    assert_eq!(calls_of(&fa, RoleKind::Planner).len(), 2, "one repeat, then on");
    assert_eq!(calls_of(&fa, RoleKind::Reviewer).len(), 1);
    // nobody was called for a skipped step
    assert!(!kinds_of(&n).iter().any(|k| matches!(k, nightshift_core::notify::NoticeKind::Failed | nightshift_core::notify::NoticeKind::Question)));
    // the task still ran with the person's words, and says the prompt was not rewritten
    let w = &calls_of(&fa, RoleKind::Worker)[0];
    assert!(w.prompt.contains("Add the feature file") && w.prompt.contains("prompt not rewritten:"), "{}", w.prompt);
    assert!(has_log(&e, &id, "prompt_not_rewritten:"));
}

#[test]
fn the_executor_cannot_rewrite_the_test_authors_files_the_edit_is_undone_counted_and_said() {
    let env = Env::new();
    let p = env.project(|p| {
        p.agent = "fa".into();
        p.check_cmd = Some("test -f feature.txt".into());
    });
    let fa = Arc::new(pipeline_fake("fa").with_role(
        RoleKind::Worker,
        vec![
            vec![write("t0_test.txt", "hacked\n"), write("feature.txt", "hello\n"), done("made the test pass by rewriting it")],
            vec![done("left the tests alone")],
        ],
    ));
    let e = env.engine(env.store_v2(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Add the feature file");
    let t = wait_status(&e, &id, Status::Done);

    // the test author's version is what landed
    assert_eq!(env.main_file("t0_test.txt"), "expect feature");
    assert_eq!(t.attempts, 1, "the attempt is counted");
    assert!(has_log(&e, &id, "tests_read_only: t0_test.txt"));
    let w = calls_of(&fa, RoleKind::Worker);
    assert_eq!(w.len(), 2);
    assert!(w[1].prompt.contains("Tests are read-only: t0_test.txt"), "{}", w[1].prompt);
    assert_eq!(w[1].resume.as_deref(), Some("fake-session-3"), "the same executor session, told what happened");
    // from the start the executor is told which files are not its own
    assert!(w[0].prompt.contains("Tests are read-only: t0_test.txt"), "{}", w[0].prompt);
}

#[test]
fn the_reviewer_sees_the_task_the_diff_and_the_gate_output_only_and_cannot_change_anything() {
    let env = Env::new();
    let p = env.project(|p| {
        p.agent = "fa".into();
        p.check_cmd = Some("echo GATE-OUTPUT-LINE; test -f feature.txt".into());
    });
    let fa = Arc::new(pipeline_fake("fa").with_role(RoleKind::Reviewer, vec![vec![write("evil.txt", "changed by the reviewer\n"), done(APPROVE)]]));
    let e = env.engine(env.store_v2(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Add the feature file");
    wait_status(&e, &id, Status::Done);

    let r = &calls_of(&fa, RoleKind::Reviewer)[0];
    assert!(r.resume.is_none(), "a fresh session");
    assert!(r.is_read_only());
    // it looks at a throwaway copy, not at the task's copy: the env has no read-only flag of its own
    let task_copy = env.d.path().join("wts").join(&id);
    assert_ne!(r.cwd, task_copy);
    assert!(r.cwd.components().any(|c| c.as_os_str() == ".ro"), "{:?}", r.cwd);
    assert!(!r.cwd.exists(), "the copy is gone after the run");
    let files = g(&env.repo, &["ls-tree", "-r", "--name-only", "main"]);
    assert!(!files.lines().any(|f| f == "evil.txt"), "{files}");
    // what it was given: the task, the diff, the output of the checks - not the plan, not the executor's words
    assert!(r.prompt.contains("Add the feature file"));
    assert!(r.prompt.contains("+hello"), "the diff: {}", r.prompt);
    assert!(r.prompt.contains("GATE-OUTPUT-LINE"));
    for hidden in ["write feature.txt", "Added the feature", "keep the public API"] {
        assert!(!r.prompt.contains(hidden), "`{hidden}` reached the reviewer:\n{}", r.prompt);
    }
}

#[test]
fn an_env_with_a_read_only_flag_runs_its_reading_roles_in_the_task_copy_with_the_flag() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(pipeline_fake("fa").with_read_only_native(true));
    let e = env.engine(env.store_v2(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Add the feature file");
    wait_status(&e, &id, Status::Done);

    let task_copy = env.d.path().join("wts").join(&id);
    for role in [RoleKind::Planner, RoleKind::Critic, RoleKind::Reviewer] {
        let c = &calls_of(&fa, role)[0];
        assert!(c.read_only, "{role:?}");
        assert_eq!(c.cwd, task_copy, "{role:?}");
    }
    let w = &calls_of(&fa, RoleKind::Worker)[0];
    assert!(!w.read_only && w.cwd == task_copy);
}

/// Task B forks main and waits on a question until task A has landed an edit of the same line, then B goes on.
fn conflict_setup(resolution: Vec<FakeStep>) -> (Env, Arc<nightshift_core::engine::Engine>, String, Arc<FakeAdapter>, std::path::PathBuf) {
    let env = Env::new();
    let log = env.d.path().join("gate-runs");
    let p = env.project(|p| p.check_cmd = Some(format!("cat a.txt >> '{}'; echo --- >> '{}'", log.display(), log.display())));
    let wb = Arc::new(FakeAdapter::new(vec![]).with_id("fb").with_role(RoleKind::Worker, vec![vec![write("a.txt", "B\n"), done("B\nNEEDS_HUMAN: go on?")], vec![done("ok")]]).with_role(RoleKind::Integrator, vec![resolution]).with_role(RoleKind::Reviewer, vec![approve(), approve()]));
    let wa = Arc::new(FakeAdapter::new(vec![]).with_id("fa").with_role(RoleKind::Worker, vec![vec![write("a.txt", "A\n"), done("A")]]).with_role(RoleKind::Reviewer, vec![approve()]));
    let e = env.engine(env.store(&p, |_| {}), vec![wa, wb.clone()], Arc::new(RecordingNotifier::default()), None);
    let b = submit_with(&e, &p, "Write B", "fb");
    wait_status(&e, &b, Status::NeedsHuman);
    let a = submit_with(&e, &p, "Write A", "fa");
    wait_status(&e, &a, Status::Done);
    e.answer(&b, "yes").unwrap();
    (env, e, b, wb, log)
}

#[test]
fn a_conflict_goes_to_the_integrator_a_fresh_run_with_both_sides_and_the_gates_run_again() {
    let (env, e, b, wb, log) = conflict_setup(vec![write("a.txt", "A\nB\n"), done("merged both")]);
    let t = wait_status(&e, &b, Status::Done);

    let ints = calls_of(&wb, RoleKind::Integrator);
    assert_eq!(ints.len(), 1);
    assert!(ints[0].resume.is_none(), "a fresh session");
    assert!(!ints[0].is_read_only());
    // it sees the task and both sides of the conflict
    assert!(ints[0].prompt.contains("Write B"), "{}", ints[0].prompt);
    assert!(ints[0].prompt.contains("<<<<<<<") && ints[0].prompt.contains(">>>>>>>"), "{}", ints[0].prompt);
    assert!(ints[0].prompt.contains("+A") || ints[0].prompt.contains("A\n"), "{}", ints[0].prompt);
    // its session is not the executor's
    let sessions: Vec<_> = t.runs.iter().filter(|r| r.role == RoleKind::Worker || r.role == RoleKind::Integrator).map(|r| (r.role, r.session.clone())).collect();
    let worker_s = sessions.iter().find(|(r, _)| *r == RoleKind::Worker).unwrap().1.clone();
    let int_s = sessions.iter().find(|(r, _)| *r == RoleKind::Integrator).unwrap().1.clone();
    assert!(int_s.is_some() && int_s != worker_s);
    // the gates ran again on the resolved tree before the merge
    let runs = std::fs::read_to_string(&log).unwrap();
    assert!(runs.contains("A\nB\n---"), "the resolved tree was checked: {runs:?}");
    assert_eq!(env.main_file("a.txt"), "A\nB");
}

#[test]
fn a_conflict_the_integrator_leaves_in_the_files_fails_the_task_as_conflict_with_main() {
    let (env, e, b, _wb, _log) = conflict_setup(vec![done("did nothing")]);
    let t = wait_status(&e, &b, Status::Failed);
    assert_eq!(t.outcome.error.as_deref(), Some("conflict_with_main"));
    assert_eq!(env.main_file("a.txt"), "A");
}

#[test]
fn a_breakdown_in_the_wrong_format_after_one_repeat_sends_the_task_on_in_the_plain_template() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(pipeline_fake("fa").with_role(RoleKind::Planner, vec![vec![done("Sure, I will look into it.")], vec![done("Still thinking {not json")]]));
    let e = env.engine(env.store_v2(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Add the feature file");
    let t = wait_status(&e, &id, Status::Done);

    let planner = calls_of(&fa, RoleKind::Planner);
    assert_eq!(planner.len(), 2, "one repeat");
    assert!(planner[1].prompt.contains("Your previous answer could not be used"), "{}", planner[1].prompt);
    let w = &calls_of(&fa, RoleKind::Worker)[0];
    // the plain template: goal / constraints / done when, the person's words in their block, and the line saying why
    for h in ["Goal:", "Constraints:", "Done when:"] {
        assert!(w.prompt.contains(h), "{h} in {}", w.prompt);
    }
    assert!(w.prompt.contains("<<<DATA TASK ") && w.prompt.contains("Add the feature file"));
    assert!(w.prompt.trim_end().ends_with("prompt not rewritten: no JSON object in the answer"), "{}", w.prompt);
    assert!(has_log(&e, &id, "prompt_not_rewritten: no JSON object in the answer"));
    assert!(matches!(t.steps.iter().find(|(s, _)| *s == Step::Plan).unwrap().1, StepState::Skipped(_)));
}

#[test]
fn when_the_spare_model_takes_over_the_prompt_is_made_by_the_template_of_its_vendor() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(
        pipeline_fake("fa").with_role(RoleKind::Worker, vec![vec![write("half.txt", "half"), FakeStep::Limit], vec![write("feature.txt", "hello\n"), done("done on the spare")]]),
    );
    let settings = |s: &mut Settings| {
        env_models(s, "fa", &["anthropic/claude-big", "openai/gpt-spare"], "anthropic/claude-big", &[], Some("openai/gpt-spare"));
    };
    let e = env.engine(env.store_v2(&p, settings), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Add the feature file");
    let t = wait_status(&e, &id, Status::Done);

    let w = calls_of(&fa, RoleKind::Worker);
    assert_eq!(w.len(), 2);
    assert_eq!((w[0].model.as_deref(), w[1].model.as_deref()), (Some("anthropic/claude-big"), Some("openai/gpt-spare")));
    // the first run: the template of Anthropic (XML tags, the role in the system prompt)
    assert!(w[0].prompt.contains("<request>") && !w[0].prompt.contains("# Instructions"), "{}", w[0].prompt);
    assert!(w[0].system.starts_with("You are a software engineer"));
    // the second: the template of OpenAI (Markdown headings, Identity / Instructions), a new session, the work so far fenced
    // after the hand-off note, which opens the prompt of the model that takes over (story 307)
    let body = w[1].prompt.strip_prefix(t.handoff.as_deref().unwrap()).unwrap().trim_start();
    assert!(body.starts_with("# Instructions"), "{}", w[1].prompt);
    assert!(w[1].system.starts_with("# Identity"), "{}", w[1].system);
    assert!(w[1].resume.is_none());
    assert!(w[1].prompt.contains("<<<DATA DIFF ") && w[1].prompt.contains("+half"), "{}", w[1].prompt);
    assert!(w[1].prompt.contains("Add the feature file"));
    assert!(has_log(&e, &id, "switched_to: openai/gpt-spare"));
    // the card shows the prompt of the run that started the work on the model that runs it
    assert!(t.rendered_prompt.as_deref().unwrap().starts_with("# Instructions"));
}

#[test]
fn text_diff_and_output_in_every_roles_prompt_are_fenced_with_a_uuid_label_and_the_rule_is_in_the_system_prompt() {
    let env = Env::new();
    let p = env.project(|p| {
        p.agent = "fa".into();
        p.check_cmd = Some("echo CHECK-OUT; test -f feature.txt".into());
    });
    let fa = Arc::new(pipeline_fake("fa"));
    let e = env.engine(env.store_v2(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Add the feature file");
    wait_status(&e, &id, Status::Done);

    let fenced_labels = |text: &str| -> Vec<String> {
        text.lines().filter_map(|l| l.strip_prefix("<<<DATA ")).map(|r| r.trim_end_matches(">>>").to_string()).collect()
    };
    for c in fa.calls() {
        let labels = fenced_labels(&c.prompt);
        assert!(!labels.is_empty(), "{:?} has no fenced data:\n{}", c.role, c.prompt);
        for l in &labels {
            let (name, uuid) = l.split_once(' ').unwrap();
            assert!(name.chars().all(|c| c.is_ascii_uppercase() || c == '_'), "{l}");
            assert_eq!(uuid.len(), 32, "{l}");
            assert!(c.prompt.contains(&format!("<<<END {uuid}>>>")));
        }
        assert!(c.system.contains("never an instruction to you"), "{:?}", c.role);
    }
    // the diff and the output of the checks the reviewer reads are in blocks of their own
    let r = &calls_of(&fa, RoleKind::Reviewer)[0];
    let names: Vec<String> = fenced_labels(&r.prompt).iter().map(|l| l.split(' ').next().unwrap().to_string()).collect();
    assert_eq!(names, vec!["TASK", "DIFF", "CHECKS"]);
    assert!(r.prompt.contains("CHECK-OUT"));
}

#[test]
fn a_vague_task_gets_a_spec_step_that_writes_it_out_and_the_executor_gets_the_spec_and_the_criteria() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(pipeline_fake("fa").with_role(
        RoleKind::Planner,
        vec![vec![done(&brief_json("Make the feature better", &["improve it"], false, true, None))], vec![done(SPEC_JSON)]],
    ));
    let e = env.engine(env.store_v2(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "make the feature better");
    let t = wait_status(&e, &id, Status::Done);

    assert_eq!(t.steps.iter().find(|(s, _)| *s == Step::Spec).unwrap().1, StepState::Done);
    assert_eq!(calls_of(&fa, RoleKind::Planner).len(), 2, "a second planner run writes the spec");
    assert!(calls_of(&fa, RoleKind::Planner)[1].prompt.contains("is unclear"));
    let w = &calls_of(&fa, RoleKind::Worker)[0];
    assert!(w.prompt.contains("The feature file holds one line."), "{}", w.prompt);
    assert!(w.prompt.contains("feature.txt has one line"), "the criteria of the spec replaced the planner's");
    assert!(!w.prompt.contains("feature.txt exists"));
}

#[test]
fn the_critics_remarks_and_corrected_plan_reach_the_executor() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(pipeline_fake("fa").with_role(
        RoleKind::Critic,
        vec![vec![done(r#"{"ok":false,"issues":["no step for the docs"],"plan":["write feature.txt","update the docs","run the checks"]}"#)]],
    ));
    let e = env.engine(env.store_v2(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Add the feature file");
    wait_status(&e, &id, Status::Done);

    let crit = &calls_of(&fa, RoleKind::Critic)[0];
    assert!(crit.prompt.contains("write feature.txt") && crit.prompt.contains("Add the feature file"), "the critic sees the task and the plan: {}", crit.prompt);
    assert!(crit.is_read_only());
    let w = &calls_of(&fa, RoleKind::Worker)[0];
    assert!(w.prompt.contains("update the docs"), "{}", w.prompt);
    assert!(w.prompt.contains("Plan review: no step for the docs"), "{}", w.prompt);
    assert!(has_log(&e, &id, "critique: 1 remarks"));
}

#[test]
fn a_risky_task_is_marked_in_the_feed_and_still_runs_with_one_executor_for_now() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(pipeline_fake("fa").with_role(
        RoleKind::Planner,
        vec![vec![done(&brief_json("Change the payment rounding", &["edit pay.rs"], false, false, Some("touches payments")))]],
    ));
    let e = env.engine(env.store_v2(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "change how payments are rounded");
    wait_status(&e, &id, Status::Done);
    assert!(has_log(&e, &id, "risky: touches payments - 2 variants"));
    assert_eq!(calls_of(&fa, RoleKind::Worker).len(), 1);
}

#[test]
fn any_role_can_ask_the_person_and_its_own_session_goes_on_with_the_answer() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(pipeline_fake("fa").with_role(
        RoleKind::Critic,
        vec![vec![done("NEEDS_HUMAN: Is auth in scope?\nOPTIONS: yes | no")], vec![done(CRITIQUE_OK)]],
    ));
    let n = Arc::new(RecordingNotifier::default());
    let e = env.engine(env.store_v2(&p, |_| {}), vec![fa.clone()], n.clone(), None);

    let id = submit(&e, &p, "Add the feature file");
    let t = wait_status(&e, &id, Status::NeedsHuman);
    assert_eq!(t.question.as_ref().unwrap().text, "Is auth in scope?");
    assert_eq!(t.question.as_ref().unwrap().options, vec!["yes", "no"]);
    assert_eq!(notices(&n, 1)[0].kind, nightshift_core::notify::NoticeKind::Question);

    e.answer(&id, "no").unwrap();
    wait_status(&e, &id, Status::Done);
    let c = calls_of(&fa, RoleKind::Critic);
    assert_eq!(c.len(), 2);
    assert_eq!(c[1].prompt, "no");
    assert_eq!(c[1].resume, Some(t.runs.iter().find(|r| r.role == RoleKind::Critic).unwrap().session.clone().unwrap()));
}

#[test]
fn the_e2e_step_runs_the_verifier_read_only_and_a_failed_check_sends_the_executor_back() {
    let env = Env::new();
    let p = env.project(|p| {
        p.agent = "fa".into();
        p.run_cmd = Some("echo start-the-app".into());
    });
    let fa = Arc::new(
        pipeline_fake("fa")
            .with_role(RoleKind::Worker, vec![vec![write("feature.txt", "hello\n"), done("v1")], vec![done("fixed the page")]])
            .with_role(RoleKind::Verifier, vec![vec![done(r#"{"pass":false,"issues":["the page is blank"]}"#)], vec![done(r#"{"pass":true,"issues":[]}"#)]])
            .with_role(RoleKind::Reviewer, vec![approve(), approve()]),
    );
    let e = env.engine(env.store_v2(&p, |s| s.quality.e2e = true), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Add the feature file");
    let t = wait_status(&e, &id, Status::Done);

    let v = calls_of(&fa, RoleKind::Verifier);
    assert_eq!(v.len(), 2);
    assert!(v[0].is_read_only() && v[0].prompt.contains("start-the-app"), "{}", v[0].prompt);
    assert!(v[0].prompt.contains("feature.txt exists"), "the criteria: {}", v[0].prompt);
    let w = calls_of(&fa, RoleKind::Worker);
    assert!(w[1].prompt.contains("the page is blank"), "{}", w[1].prompt);
    assert_eq!(t.attempts, 1, "a failed e2e is a failed round, like red gates");
    assert_eq!(t.steps.iter().find(|(s, _)| *s == Step::E2e).unwrap().1, StepState::Done);
}

#[test]
fn a_verifier_that_answers_in_the_wrong_format_is_a_failure_not_a_pass() {
    let env = Env::new();
    let p = env.project(|p| {
        p.agent = "fa".into();
        p.run_cmd = Some("echo start".into());
    });
    let fa = Arc::new(
        pipeline_fake("fa")
            .with_role(RoleKind::Worker, vec![vec![write("feature.txt", "hello\n"), done("v1")], vec![done("again")]])
            .with_role(RoleKind::Verifier, vec![vec![done("looks fine to me")], vec![done("really, fine")], vec![done(r#"{"pass":true,"issues":[]}"#)]])
            .with_role(RoleKind::Reviewer, vec![approve(), approve()]),
    );
    let e = env.engine(env.store_v2(&p, |s| s.quality.e2e = true), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add the feature file");
    let t = wait_status(&e, &id, Status::Done);
    assert_eq!(t.attempts, 1);
    assert!(calls_of(&fa, RoleKind::Worker)[1].prompt.contains("verifier answered in wrong format"));
}

#[test]
fn a_task_text_is_taken_up_to_a_hundred_thousand_characters_the_title_is_eighty_of_the_first_line() {
    let env = Env::new();
    let p = env.project(|_| {});
    let e = env.engine(env.store(&p, |s| s.paused = true), vec![fake(vec![]), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let first = "x".repeat(120);
    let long = format!("{first}\nsecond line {}", "y".repeat(99_000));
    let id = submit(&e, &p, &long);
    let t = task(&e, &id);
    assert_eq!(t.title, "x".repeat(80));
    assert_eq!(t.prompt.chars().count(), long.chars().count());
    let exactly = "z".repeat(100_000);
    assert!(e.submit(NewTask { project_id: p.id.clone(), prompt: exactly, ..Default::default() }).is_ok());
    let too_long = "z".repeat(100_001);
    assert_eq!(e.submit(NewTask { project_id: p.id.clone(), prompt: too_long, ..Default::default() }).unwrap_err(), "prompt_too_long");
    assert_eq!(e.submit(NewTask { project_id: p.id.clone(), prompt: "   ".into(), ..Default::default() }).unwrap_err(), "empty_prompt");
    // the cap counts characters, not bytes
    let cyr = "я".repeat(100_000);
    assert!(e.submit(NewTask { project_id: p.id.clone(), prompt: cyr, ..Default::default() }).is_ok());
}

#[test]
fn quality_off_is_the_short_path_code_gates_review_merge_with_no_planner_and_the_raw_text() {
    let env = Env::new();
    let p = env.project(|p| {
        p.agent = "fa".into();
        p.check_cmd = Some("test -f feature.txt".into());
    });
    let fa = Arc::new(pipeline_fake("fa"));
    let e = env.engine(env.store_v2(&p, |s| s.quality.on = false), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "  Add the feature file\nwith details ");
    let t = wait_status(&e, &id, Status::Done);

    assert_eq!(roles_in_order(&fa), vec![RoleKind::Worker, RoleKind::Reviewer]);
    assert_eq!(calls_of(&fa, RoleKind::Worker)[0].prompt, "Add the feature file\nwith details", "the words as they are");
    assert_eq!(t.steps.iter().map(|(s, _)| *s).collect::<Vec<_>>(), vec![Step::Code, Step::Gates, Step::Review, Step::Merge]);
    assert!(t.steps.iter().all(|(_, st)| *st == StepState::Done));
}

#[test]
fn structured_roles_are_given_a_schema_and_the_executor_is_not_and_the_gates_are_recorded_check_by_check() {
    let env = Env::new();
    let p = env.project(|p| {
        p.agent = "fa".into();
        p.check_cmd = Some("test -f feature.txt".into());
        p.extra_checks = vec![("lint".into(), "echo lint-clean".into())];
    });
    let fa = Arc::new(pipeline_fake("fa"));
    let e = env.engine(env.store_v2(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add the feature file");
    let t = wait_status(&e, &id, Status::Done);

    for role in [RoleKind::Planner, RoleKind::Critic, RoleKind::Reviewer] {
        let s = calls_of(&fa, role)[0].json_schema.clone().unwrap_or_else(|| panic!("{role:?} has no schema"));
        assert!(serde_json::from_str::<serde_json::Value>(&s).is_ok(), "{s}");
    }
    assert!(calls_of(&fa, RoleKind::Worker)[0].json_schema.is_none());
    assert!(calls_of(&fa, RoleKind::TestAuthor)[0].json_schema.is_none());
    // what the gates ran and how it went
    assert_eq!(
        t.outcome.gates,
        vec![
            GateResult { name: "tests".into(), passed: true, tail: String::new() },
            GateResult { name: "lint".into(), passed: true, tail: "lint-clean".into() },
        ]
    );
    // the reviewer saw both
    let r = &calls_of(&fa, RoleKind::Reviewer)[0];
    assert!(r.prompt.contains("lint: passed") && r.prompt.contains("lint-clean") && r.prompt.contains("tests: passed"), "{}", r.prompt);
}

#[test]
fn a_red_extra_check_keeps_the_task_from_going_on_like_red_tests() {
    let env = Env::new();
    let p = env.project(|p| {
        p.agent = "fa".into();
        p.extra_checks = vec![("types".into(), "test -f typed.txt || { echo 'missing typed.txt'; exit 1; }".into())];
    });
    let fa = Arc::new(
        pipeline_fake("fa")
            .with_role(RoleKind::Worker, vec![vec![write("feature.txt", "x"), done("v1")], vec![write("typed.txt", "t"), done("v2")]])
            .with_role(RoleKind::Reviewer, vec![approve()]),
    );
    let e = env.engine(env.store_v2(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add the feature file");
    let t = wait_status(&e, &id, Status::Done);
    let w = calls_of(&fa, RoleKind::Worker);
    assert_eq!(w.len(), 2);
    assert!(w[1].prompt.contains("types:") && w[1].prompt.contains("missing typed.txt"), "{}", w[1].prompt);
    assert_eq!(t.attempts, 1);
    assert_eq!(calls_of(&fa, RoleKind::Reviewer).len(), 1, "the review came only after the gates were green");
}

#[test]
fn a_model_that_is_not_in_the_catalog_runs_without_a_flag_and_the_feed_says_so() {
    let env = Env::new();
    let p = env.project(|p| p.agent = "fa".into());
    let fa = Arc::new(pipeline_fake("fa"));
    let store = env.store_v2(&p, |s| env_models(s, "fa", &[], "vanished", &[], None));
    let e = env.engine(store, vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add the feature file");
    wait_status(&e, &id, Status::Done);
    assert!(calls_of(&fa, RoleKind::Worker)[0].model.is_none());
    assert!(has_log(&e, &id, "model_not_in_catalog: vanished"));
}

#[test]
fn latest_becomes_a_number_when_a_run_starts_and_a_resumed_session_stays_on_its_model() {
    let env = Env::new();
    let p = env.project(|p| {
        p.agent = "fa".into();
        p.check_cmd = Some("test -f fixed.txt".into());
    });
    let gd = env.d.path().join("gate");
    std::fs::create_dir(&gd).unwrap();
    let held = Gated::wrap(
        FakeAdapter::new(vec![]).with_id("fa").with_role(RoleKind::Worker, vec![vec![done("v1")], vec![write("fixed.txt", "ok"), done("v2")]]).with_role(RoleKind::Reviewer, vec![approve()]),
        0,
        &gd,
    );
    let family = |versions: &[&str]| Catalog {
        env: "fa".into(),
        families: vec![ModelFamily {
            id: "opus".into(),
            label: "Opus".into(),
            vendor: "anthropic".into(),
            blurb: None,
            versions: versions.iter().map(|v| v.to_string()).collect(),
            default: false,
            exact: versions.iter().map(|v| (v.to_string(), format!("claude-opus-{}", v.replace('.', "-")))).collect(),
            source_note: None,
        }],
        source: CatalogSource::CliList,
        at: chrono::Local::now(),
    };
    let store = env.store(&p, |s| {
        env_models(s, "fa", &[], "opus", &[], None);
        s.envs.get_mut("fa").unwrap().default_model = Some(ModelChoice { family: "opus".into(), version: None }); // "latest"
    });
    store.set_catalog(family(&["5.5", "4.8"])).unwrap();
    let e = env.engine(store, vec![held.clone()], Arc::new(RecordingNotifier::default()), None);

    let id = submit(&e, &p, "Fix it");
    held.wait_started(); // run 0 has started on 5.5
    e.store().lock().unwrap().set_catalog(family(&["5.6", "5.5", "4.8"])).unwrap(); // a newer version appears
    held.open();
    wait_status(&e, &id, Status::Done);

    let c = held.inner.calls();
    let w: Vec<_> = c.iter().filter(|r| r.role == RoleKind::Worker).collect();
    assert_eq!(w[0].model.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(w[1].model.as_deref(), Some("claude-opus-5-5"), "the session goes on with the model it started on");
    assert_eq!(w[1].resume.as_deref(), Some("fake-session-0"));
    let r = c.iter().find(|r| r.role == RoleKind::Reviewer).unwrap();
    assert_eq!(r.model.as_deref(), Some("claude-opus-5-6"), "a run that starts now takes the number of now");
}

#[test]
fn a_broken_git_step_fails_the_task_with_the_command_and_what_git_said() {
    let env = Env::new();
    let p = env.project(|p| p.base_branch = "no-such-branch".into());
    let e = env.engine(env.store(&p, |_| {}), vec![fake(vec![]), reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add f");
    let t = wait_status(&e, &id, Status::Failed);
    let why = t.outcome.error.unwrap();
    assert!(why.contains("git worktree add"), "the command: {why}");
    assert!(why.to_lowercase().contains("no-such-branch") || why.contains("invalid reference"), "what git said: {why}");
}

#[test]
fn a_kit_variable_with_a_value_out_of_its_shape_is_not_put_on_the_run_and_the_feed_names_it_without_the_value() {
    let env = Env::new();
    let p = env.project(|p| {
        p.agent = "fa".into();
        p.check_cmd = Some("test -f feature.txt".into());
    });
    let fa = Arc::new(pipeline_fake("fa"));
    let e = env.engine(env.store_v2(&p, |_| {}), vec![fa.clone()], Arc::new(RecordingNotifier::default()), None);
    e.set_env_vars(Arc::new(|_| {
        vec![
            ("PATH".into(), "./evil-bin:/usr/bin".into()),
            ("HOME".into(), "/tmp/x\nEVIL=1".into()),
            ("LANG".into(), "C;evil".into()),
            ("API_TOKEN".into(), "tok-kept".into()),
        ]
    }));

    let id = submit(&e, &p, "Add the feature file");
    wait_status(&e, &id, Status::Done);

    for run in fa.calls() {
        assert_eq!(run.extra_env, vec![("API_TOKEN".to_string(), "tok-kept".to_string())], "{:?}", run.role);
    }
    let log = log_texts(&e, &id).join("\n");
    for name in ["PATH", "HOME", "LANG"] {
        assert!(log.contains(&format!("env_value_rejected: {name}")), "{log}");
    }
    assert!(!log.contains("evil"), "the rejected value is not written: {log}");
}
