//! Seam 1 for the chat (spec 104-119): `Chats` over `Engine` + `FakeAdapter` + a temp repo + a clock the test moves.
mod common;
use chrono::{DateTime, Local};
use common::*;
use nightshift_core::agent::{Adapter, AgentEvent, FakeAdapter, FakeStep, Run};
use nightshift_core::chat::{ChatEvent, ChatState, ChatTarget, Chats};
use nightshift_core::agent::{ActionInput, ToolAction};
use nightshift_core::engine::Engine;
use nightshift_core::model::*;
use nightshift_core::notify::RecordingNotifier;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Rig {
    env: Env,
    p: Project,
    e: Arc<Engine>,
    chats: Arc<Chats>,
    fa: Arc<FakeAdapter>,
    now: Arc<Mutex<DateTime<Local>>>,
}

fn new_target(p: &Project) -> ChatTarget {
    ChatTarget::New { env: "fake".into(), project_id: p.id.clone() }
}

/// The fake answers the chat role with `scripts` in order; the engine is set up with `settings`.
fn rig_with(scripts: Vec<Vec<FakeStep>>, settings: impl FnOnce(&mut Settings)) -> Rig {
    rig_fake(FakeAdapter::new(vec![]).with_role(RoleKind::Chat, scripts), settings)
}

type Parts = (Env, Project, Arc<Engine>, Arc<Chats>, Arc<Mutex<DateTime<Local>>>);

fn build(adapter: Arc<dyn Adapter>, settings: impl FnOnce(&mut Settings)) -> Parts {
    let env = Env::new();
    let p = env.project(|_| {});
    let (clock, now) = clock_at(12, 0);
    let e = env.engine(env.store(&p, settings), vec![adapter], Arc::new(RecordingNotifier::default()), Some(clock));
    let chats = Chats::new(e.clone());
    (env, p, e, chats, now)
}

fn rig_fake(fake: FakeAdapter, settings: impl FnOnce(&mut Settings)) -> Rig {
    let fa = Arc::new(fake);
    let (env, p, e, chats, now) = build(fa.clone(), settings);
    Rig { env, p, e, chats, fa, now }
}

fn rig(scripts: Vec<Vec<FakeStep>>) -> Rig {
    rig_with(scripts, |_| {})
}

fn feed(r: &Rig, chat: &str) -> Vec<ChatMsg> {
    r.chats.messages(chat, None, 200)
}

fn wait_feed(r: &Rig, chat: &str, what: &str, pred: impl Fn(&[ChatMsg]) -> bool) -> Vec<ChatMsg> {
    let start = Instant::now();
    loop {
        let m = feed(r, chat);
        if pred(&m) {
            return m;
        }
        assert!(start.elapsed() < Duration::from_secs(30), "timeout waiting for {what}; feed: {:#?}", m.iter().map(|x| (&x.who, &x.text)).collect::<Vec<_>>());
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn tasks(r: &Rig) -> Vec<Task> {
    r.e.store().lock().unwrap().tasks()
}

#[test]
fn a_question_is_answered_in_the_feed_and_changes_nothing() {
    // a pinned model of the env's catalog: the signature is "<Env> · <Model version>"
    let env_models = |s: &mut Settings| {
        s.envs.insert(
            "fake".into(),
            EnvSettings { default_model: Some(ModelChoice { family: "opus".into(), version: Some("5.5".into()) }), ask_push: false, ask_pr: false, ..EnvSettings::default() },
        );
    };
    let r = rig_with(vec![vec![done("Billing sums usage per month in billing.rs.")]], env_models);
    r.e.store()
        .lock()
        .unwrap()
        .set_catalog(Catalog {
            env: "fake".into(),
            families: vec![ModelFamily {
                id: "opus".into(),
                label: "Opus".into(),
                vendor: "anthropic".into(),
                blurb: None,
                versions: vec!["5.5".into()],
                default: false,
                exact: [("5.5".to_string(), "claude-opus-5-5".to_string())].into(),
                source_note: None,
            }],
            source: CatalogSource::Snapshot,
            at: Local::now(),
        })
        .unwrap();
    let main_before = g(&r.env.repo, &["rev-parse", "main"]);

    let id = r.chats.send(new_target(&r.p), "m1", "Как устроен биллинг?").unwrap();
    let m = wait_feed(&r, &id, "the answer", |m| m.iter().any(|x| x.who == Who::Model));

    let answer = m.iter().find(|x| x.who == Who::Model).unwrap();
    assert_eq!(answer.text, "Billing sums usage per month in billing.rs.");
    assert_eq!(answer.model.as_deref(), Some("Fake · Opus 5.5"));
    assert_eq!(answer.reply_to.as_deref(), Some("m1"));
    assert!(tasks(&r).is_empty(), "a question makes no task");
    assert_eq!(g(&r.env.repo, &["rev-parse", "main"]), main_before);
    assert_eq!(g(&r.env.repo, &["status", "--porcelain"]), "", "the person's folder is untouched");
    // the run: read-only, the chat role, in a copy (not in the person's folder), the text in a DATA block
    let run = &r.fa.calls()[0];
    assert!(run.read_only && run.role == RoleKind::Chat);
    assert_ne!(run.cwd, r.env.repo);
    assert_eq!(run.model.as_deref(), Some("claude-opus-5-5"));
    assert!(run.prompt.contains("<<<DATA MESSAGE") && run.prompt.contains("Как устроен биллинг?"));
    // the list: a conversation, with the start of the last answer
    let list = r.chats.list("fake");
    assert_eq!(list.len(), 1);
    assert_eq!((list[0].state, list[0].title.as_str()), (ChatState::Talk, "Как устроен биллинг?"));
    assert_eq!(list[0].last.as_deref(), Some("Billing sums usage per month in billing.rs."));
}

#[test]
fn what_the_model_writes_while_answering_is_thrown_away_before_the_next_message() {
    let r = rig(vec![vec![write("leak.txt", "oops"), done("Done reading.")], vec![done("Second answer.")]]);
    let id = r.chats.send(new_target(&r.p), "m1", "What does a.txt hold?").unwrap();
    wait_feed(&r, &id, "the first answer", |m| m.iter().any(|x| x.text == "Done reading."));

    // the person's folder and the repository never saw the file
    assert!(!r.env.repo.join("leak.txt").exists());
    assert_eq!(g(&r.env.repo, &["status", "--porcelain"]), "");
    assert_eq!(g(&r.env.repo, &["ls-tree", "-r", "--name-only", "main"]), "a.txt");
    // it sits in the chat's copy (where the run happened) until the next message
    let copy = r.fa.calls()[0].cwd.clone();
    assert_eq!(std::fs::read_to_string(copy.join("leak.txt")).unwrap(), "oops");
    r.chats.send(ChatTarget::Existing(id.clone()), "m2", "And now?").unwrap();
    wait_feed(&r, &id, "the second answer", |m| m.iter().any(|x| x.text == "Second answer."));
    assert!(!copy.join("leak.txt").exists());
    assert_eq!(g(&r.env.repo, &["status", "--porcelain"]), "");
}

/// Tasks of the chat wait for the night, so the test sees them as made and not run.
fn hold_tasks(s: &mut Settings) {
    s.envs.insert(
        "fake".into(),
        EnvSettings {
            last_start: StartWhen::Night,
            last_quality: Some(QualityPlan { on: false, ..QualityPlan::default() }),
            ask_push: false,
            ask_pr: false,
            ..EnvSettings::default()
        },
    );
}

fn task_line(m: &[ChatMsg]) -> Option<&ChatMsg> {
    m.iter().find(|x| x.kind == Some(ChatKind::Task))
}

#[test]
fn a_command_in_either_language_becomes_a_task_at_once_with_the_last_choice_of_the_env() {
    let r = rig_with(vec![], hold_tasks);
    for (i, text) in ["Сделай кнопку выхода", "Add a logout button"].iter().enumerate() {
        let id = r.chats.send(new_target(&r.p), &format!("c{i}"), text).unwrap();
        let line = task_line(&feed(&r, &id)).cloned().expect("the line 'this is a task' is there at once");
        assert!(line.text.starts_with("this_is_task: "), "{}", line.text);
        let t = r.e.store().lock().unwrap().task(line.task_id.as_deref().unwrap()).unwrap();
        assert_eq!((t.prompt.as_str(), t.chat_id.as_deref(), t.agent.as_str()), (*text, Some(id.as_str()), "fake"));
        // parameters: the person's last choice for this env
        assert_eq!(t.start, StartWhen::Night);
        assert!(!t.quality.on);
    }
    assert_eq!(tasks(&r).len(), 2);
    assert!(r.fa.calls().is_empty(), "no model run for a command");
}

#[test]
fn ns_task_at_the_end_of_the_answer_makes_a_task_and_stays_out_of_the_feed_but_in_the_middle_it_does_not() {
    let r = rig_with(
        vec![
            vec![done("The list loads in one query per row.\nNS_TASK: Make the list load faster")],
            vec![done("NS_TASK: quoted in the middle\nNothing to change here.")],
        ],
        hold_tasks,
    );
    let id = r.chats.send(new_target(&r.p), "c1", "хорошо бы, чтобы список грузился быстрее").unwrap();
    let m = wait_feed(&r, &id, "task line", |m| task_line(m).is_some());
    let answer = m.iter().find(|x| x.who == Who::Model).unwrap();
    assert_eq!(answer.text, "The list loads in one query per row.");
    assert!(m.iter().all(|x| !x.text.contains("NS_TASK")), "the sign is not shown");
    let t = tasks(&r);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].prompt, "хорошо бы, чтобы список грузился быстрее", "the task is made of the person's message");

    let id2 = r.chats.send(new_target(&r.p), "c2", "what about the footer?").unwrap();
    let m2 = wait_feed(&r, &id2, "answer", |m| m.iter().any(|x| x.who == Who::Model));
    assert_eq!(m2.iter().find(|x| x.who == Who::Model).unwrap().text, "NS_TASK: quoted in the middle\nNothing to change here.");
    assert_eq!(tasks(&r).len(), 1, "a sign in the middle is text");
}

#[test]
fn this_was_a_question_cancels_the_task_before_the_code_step_and_later_only_cancel_works() {
    // before "Code": the task is cancelled and the message goes back to the model as a question
    let r = rig_with(vec![vec![done("It prints the total.\nNS_TASK: must not become a task again")]], hold_tasks);
    let id = r.chats.send(new_target(&r.p), "c1", "Сделай отчёт про биллинг").unwrap();
    let task_id = task_line(&feed(&r, &id)).unwrap().task_id.clone().unwrap();
    r.chats.not_a_task(&id, &task_id).unwrap();
    assert_eq!(task(&r.e, &task_id).status, Status::Cancelled);
    let m = wait_feed(&r, &id, "the answer", |m| m.iter().any(|x| x.who == Who::Model));
    assert_eq!(m.iter().find(|x| x.who == Who::Model).unwrap().text, "It prints the total.\nNS_TASK: must not become a task again");
    assert!(m.iter().any(|x| x.text == "back_to_question"));
    assert!(r.fa.calls()[0].prompt.contains("Сделай отчёт про биллинг"), "the model sees the same message");
    assert_eq!(tasks(&r).len(), 1, "no second task");

    // the worker is already writing: too late, the usual cancel applies
    let r = rig_fake(FakeAdapter::new(vec![]).with_role(RoleKind::Worker, vec![vec![FakeStep::Sleep(20_000), done("late")]]), |_| {});
    let id = r.chats.send(new_target(&r.p), "c1", "Fix the footer").unwrap();
    let task_id = task_line(&feed(&r, &id)).unwrap().task_id.clone().unwrap();
    wait(&r.e, &task_id, "the worker running", |t| t.runs.iter().any(|x| x.role == RoleKind::Worker));
    let err = r.chats.not_a_task(&id, &task_id).unwrap_err();
    assert!(err.starts_with("already_coding"), "{err}");
    r.e.cancel(&task_id).unwrap();
    assert_eq!(task(&r.e, &task_id).status, Status::Cancelled);
}

#[test]
fn the_same_message_id_twice_is_one_message_and_one_task_and_an_empty_message_is_refused() {
    let r = rig_with(vec![], hold_tasks);
    let first = r.chats.send(new_target(&r.p), "same", "Add dark mode").unwrap();
    // Enter twice before the window knows the chat; then a click plus Enter in the chat itself
    assert_eq!(r.chats.send(new_target(&r.p), "same", "Add dark mode").unwrap(), first);
    assert_eq!(r.chats.send(ChatTarget::Existing(first.clone()), "same", "Add dark mode").unwrap(), first);
    let humans = feed(&r, &first).into_iter().filter(|m| m.who == Who::Human).count();
    assert_eq!(humans, 1);
    assert_eq!(tasks(&r).len(), 1);
    assert_eq!(r.chats.list("fake").len(), 1);

    for blank in ["", "  \n\t "] {
        assert!(r.chats.send(new_target(&r.p), "e1", blank).is_err());
        assert!(r.chats.send(ChatTarget::Existing(first.clone()), "e2", blank).is_err());
    }
    assert_eq!(r.chats.list("fake").len(), 1, "a refused message makes no chat");
    assert_eq!(feed(&r, &first).into_iter().filter(|m| m.who == Who::Human).count(), 1);
}

/// A CLI that forgets its sessions once `lost` is set: continuing one fails the way Claude does.
struct Amnesia {
    inner: FakeAdapter,
    lost: AtomicBool,
    runs: Mutex<Vec<Run>>,
}

impl Adapter for Amnesia {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn available(&self) -> Result<(), String> {
        Ok(())
    }
    fn command(&self, run: &Run) -> Command {
        self.runs.lock().unwrap().push(run.clone());
        if self.lost.load(Ordering::SeqCst) && run.resume.is_some() {
            let mut c = Command::new("sh");
            c.arg("-c").arg("echo 'No conversation found with session ID: gone' >&2; exit 1").current_dir(&run.cwd);
            return c;
        }
        self.inner.command(run)
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        self.inner.parse(line)
    }
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
        if exit_ok {
            vec![]
        } else {
            let said = all_text.trim();
            vec![AgentEvent::Done { session: None, text: if said.is_empty() { "fake agent failed".into() } else { said.into() }, is_error: true }]
        }
    }
}

#[test]
fn the_next_message_continues_the_session_and_a_lost_session_starts_a_new_one_with_a_recap() {
    let am = Arc::new(Amnesia {
        inner: FakeAdapter::new(vec![]).with_role(RoleKind::Chat, vec![vec![done("Billing lives in billing.rs.")], vec![done("Yes, monthly.")], vec![done("Sorry, I lost track: it is monthly.")]]),
        lost: AtomicBool::new(false),
        runs: Mutex::new(vec![]),
    });
    let (_env, p, _e, chats, _now) = build(am.clone(), |_| {});
    let wait_n = |id: &str, n: usize| {
        let start = Instant::now();
        while chats.messages(id, None, 200).iter().filter(|m| m.who == Who::Model).count() < n {
            assert!(start.elapsed() < Duration::from_secs(30), "timeout waiting for answer {n}");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let id = chats.send(new_target(&p), "m1", "Where is billing?").unwrap();
    wait_n(&id, 1);
    chats.send(ChatTarget::Existing(id.clone()), "m2", "Is it per month?").unwrap();
    wait_n(&id, 2);
    {
        let runs = am.runs.lock().unwrap();
        assert_eq!(runs[0].resume, None);
        let session = chats.terminal(&id).unwrap().session.expect("the chat keeps the session of the CLI");
        assert_eq!(runs[1].resume.as_deref(), Some(session.as_str()), "the same session goes on");
    }

    am.lost.store(true, Ordering::SeqCst);
    chats.send(ChatTarget::Existing(id.clone()), "m3", "And per year?").unwrap();
    wait_n(&id, 3);
    let runs = am.runs.lock().unwrap();
    assert_eq!(runs.len(), 4, "the lost session cost one failed run and one new run");
    assert!(runs[2].resume.is_some(), "it first tried to go on");
    assert_eq!(runs[3].resume, None, "then a new session");
    for earlier in ["Where is billing?", "Billing lives in billing.rs.", "Is it per month?", "Yes, monthly."] {
        assert!(runs[3].prompt.contains(earlier), "the recap holds {earlier:?}");
    }
    assert!(runs[3].prompt.contains("And per year?"));
    let feed = chats.messages(&id, None, 200);
    assert!(feed.iter().any(|m| m.who == Who::System && m.text == "new_session_started"));
    assert!(feed.iter().all(|m| !m.text.starts_with("not_answered")));
}

#[test]
fn a_failed_answer_shows_the_reason_and_retry_repeats_it() {
    let r = rig(vec![vec![FakeStep::Fail], vec![done("Now it works.")]]);
    let id = r.chats.send(new_target(&r.p), "m1", "Why is the build red?").unwrap();
    let m = wait_feed(&r, &id, "not answered", |m| m.iter().any(|x| x.text.starts_with("not_answered: ")));
    let line = m.iter().find(|x| x.text.starts_with("not_answered: ")).unwrap();
    assert_eq!(line.kind, Some(ChatKind::Error));
    assert_eq!(line.text, "not_answered: cli_error: fake agent failed");
    assert_eq!(line.reply_to.as_deref(), Some("m1"));
    assert!(m.iter().all(|x| x.who != Who::Model));

    r.chats.retry(&id, "m1").unwrap();
    let m = wait_feed(&r, &id, "the answer after retry", |m| m.iter().any(|x| x.who == Who::Model));
    assert_eq!(m.iter().filter(|x| x.who == Who::Human).count(), 1, "retry does not add the message again");
    assert_eq!(m.iter().find(|x| x.who == Who::Model).unwrap().text, "Now it works.");
    assert!(r.chats.retry(&id, "no-such-message").is_err());
}

#[test]
fn a_used_up_limit_holds_the_message_and_it_goes_by_itself_after_the_reset() {
    let r = rig(vec![vec![FakeStep::Limit], vec![done("Back again.")]]);
    let id = r.chats.send(new_target(&r.p), "m1", "How do the tests run?").unwrap();
    // the env gives no reset time: the clock reads 12:00, the wait is half an hour
    let m = wait_feed(&r, &id, "limit line", |m| m.iter().any(|x| x.text.starts_with("will_answer_after")));
    assert_eq!(m.iter().find(|x| x.text.starts_with("will_answer_after")).unwrap().text, "will_answer_after: 12:30");
    assert_eq!(r.fa.calls().len(), 1, "nothing is sent while the limit holds");
    advance(&r.now, 29 * 60);
    assert_eq!(r.chats.release_due(), 0, "29 minutes later it is still held");

    advance(&r.now, 2 * 60);
    let m = wait_feed(&r, &id, "the answer after the reset", |m| m.iter().any(|x| x.who == Who::Model));
    assert_eq!(m.iter().find(|x| x.who == Who::Model).unwrap().text, "Back again.");
    assert_eq!(r.fa.calls().len(), 2);
}

#[test]
fn a_message_while_a_task_goes_on_is_a_note_that_is_marked_delivered_when_the_executor_gets_it() {
    let fake = FakeAdapter::new(vec![]).with_role(RoleKind::Worker, vec![vec![write("f.txt", "x"), done("done")]]);
    let r = rig_fake(fake, hold_tasks);
    let id = r.chats.send(new_target(&r.p), "c1", "Add a footer").unwrap();
    let task_id = task_line(&feed(&r, &id)).unwrap().task_id.clone().unwrap();

    r.chats.send(ChatTarget::Existing(id.clone()), "c2", "use the small logo").unwrap();
    let m = feed(&r, &id);
    let queued = m.iter().find(|x| x.text.starts_with("note_queued")).expect("'will pass it on' under the message");
    assert_eq!((queued.reply_to.as_deref(), queued.task_id.as_deref()), (Some("c2"), Some(task_id.as_str())));
    assert!(r.fa.calls().is_empty(), "a note is not a question for the model");
    assert_eq!(tasks(&r).len(), 1, "and not a new task");

    r.e.start_now(&task_id).unwrap();
    let m = wait_feed(&r, &id, "delivery", |m| m.iter().any(|x| x.text.starts_with("note_delivered")));
    let done_line = m.iter().find(|x| x.text.starts_with("note_delivered")).unwrap();
    assert_eq!((done_line.text.as_str(), done_line.reply_to.as_deref()), ("note_delivered: 12:00", Some("c2")));
    let worker = r.fa.calls().into_iter().find(|c| c.role == RoleKind::Worker).unwrap();
    assert!(worker.prompt.contains("use the small logo"));
}

#[test]
fn a_message_on_a_question_of_the_task_is_the_answer() {
    let fake = FakeAdapter::new(vec![]).with_role(RoleKind::Worker, vec![vec![done("NEEDS_HUMAN: Which color?")], vec![write("f.txt", "blue"), done("done")]]);
    let r = rig_fake(fake, |s| {
        s.quality.on = false;
    });
    let id = r.chats.send(new_target(&r.p), "c1", "Add a button").unwrap();
    let task_id = task_line(&feed(&r, &id)).unwrap().task_id.clone().unwrap();
    wait_status(&r.e, &task_id, Status::NeedsHuman);

    r.chats.send(ChatTarget::Existing(id.clone()), "c2", "blue, please").unwrap();
    let m = feed(&r, &id);
    assert!(m.iter().any(|x| x.text.starts_with("answer_passed") && x.reply_to.as_deref() == Some("c2")));
    assert!(m.iter().all(|x| !x.text.starts_with("note_queued")));
    wait(&r.e, &task_id, "the task goes on", |t| t.status != Status::NeedsHuman);
    assert!(has_log(&r.e, &task_id, "blue, please"));
    let start = Instant::now();
    while !r.fa.calls().iter().any(|c| c.role == RoleKind::Worker && c.prompt.contains("blue, please")) {
        assert!(start.elapsed() < Duration::from_secs(30), "the worker never got the answer");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_task_that_lands_before_the_note_arrives_is_not_passed_and_the_message_is_handled_as_an_ordinary_one() {
    // the review is slow: the note comes in while the task is reviewed and the task merges without a next worker run
    let fake = FakeAdapter::new(vec![])
        .with_role(RoleKind::Worker, vec![vec![write("f.txt", "x"), done("done")]])
        .with_role(RoleKind::Reviewer, vec![vec![FakeStep::Sleep(1500), done(APPROVE)]])
        .with_role(RoleKind::Chat, vec![vec![done("I added f.txt.")]]);
    let r = rig_fake(fake, |s| {
        s.quality.on = false;
    });
    let id = r.chats.send(new_target(&r.p), "c1", "Add f.txt").unwrap();
    let task_id = task_line(&feed(&r, &id)).unwrap().task_id.clone().unwrap();
    wait(&r.e, &task_id, "the review", |t| t.runs.iter().any(|x| x.role == RoleKind::Reviewer));

    r.chats.send(ChatTarget::Existing(id.clone()), "c2", "what did you change?").unwrap();
    assert!(feed(&r, &id).iter().any(|x| x.text.starts_with("note_queued")), "first it looks like a note");
    wait_status(&r.e, &task_id, Status::Done);
    let m = wait_feed(&r, &id, "the ordinary answer", |m| m.iter().any(|x| x.who == Who::Model));
    let line = m.iter().find(|x| x.text.starts_with("note_not_passed")).expect("'not passed' under the message");
    assert_eq!((line.text.as_str(), line.kind, line.reply_to.as_deref()), ("note_not_passed: task_already_merged", Some(ChatKind::Warn), Some("c2")));
    assert_eq!(m.iter().find(|x| x.who == Who::Model).unwrap().text, "I added f.txt.");
    assert_eq!(tasks(&r).len(), 1);
}

// ---- the list, the archive, the pages of a feed, removal ----

fn seed_chat(st: &mut nightshift_core::store::Store, p: &Project, id: &str, env: &str, age_days: i64) {
    let at = Local::now() - chrono::Duration::days(age_days);
    st.upsert_chat(&Chat { id: id.into(), env: env.into(), project_id: p.id.clone(), title: format!("chat {id}"), session: None, convoy: false, created_at: at, updated_at: at, archived: false }).unwrap();
}

fn seed_task(st: &mut nightshift_core::store::Store, p: &Project, chat: Option<&str>, status: Status, n: i64) -> String {
    let id = format!("task-{n}");
    let question = (status == Status::NeedsHuman).then(|| Question { text: "Which?".into(), options: vec![], kind: QuestionKind::Agent });
    st.upsert_task(&Task {
        id: id.clone(),
        project_id: p.id.clone(),
        title: format!("task {n}"),
        agent: "fake".into(),
        status,
        question,
        chat_id: chat.map(str::to_string),
        created_at: Local::now() - chrono::Duration::minutes(100 - n),
        ..Task::default()
    })
    .unwrap();
    id
}

fn seed_msg(st: &nightshift_core::store::Store, chat: &str, id: &str, who: Who, text: &str, at: DateTime<Local>) {
    let m = ChatMsg { id: id.into(), chat_id: chat.into(), at, who, text: text.into(), task_id: None, delivered_at: None, kind: None, tool: None, model: None, reply_to: None, action: None };
    assert!(st.append_chat_msg(&m).unwrap());
}

/// An engine that is not started (tasks seeded in the store stay as they are) and chats over it.
fn quiet(seed: impl FnOnce(&mut nightshift_core::store::Store, &Project)) -> (Env, Project, Arc<Engine>, Arc<Chats>, Arc<FakeAdapter>) {
    let env = Env::new();
    let p = env.project(|_| {});
    let mut st = env.store(&p, |_| {});
    seed(&mut st, &p);
    let fa = Arc::new(FakeAdapter::new(vec![]));
    let clock: nightshift_core::engine::Clock = Arc::new(Local::now);
    let e = Engine::new_with_clock(st, vec![fa.clone()], Arc::new(RecordingNotifier::default()), env.d.path().join("wts"), clock.clone());
    let chats = Chats::new(e.clone());
    (env, p, e, chats, fa)
}

#[test]
fn the_list_shows_the_most_urgent_state_of_each_chat_and_the_swarm_and_convoy_entries_and_the_archive() {
    let (_env, _p, e, chats, _fa) = quiet(|st, p| {
        for c in ["waits", "failed", "working", "queued", "done", "talk", "old", "other-env"] {
            seed_chat(st, p, c, if c == "other-env" { "claude" } else { "fake" }, if c == "old" { 8 } else { 0 });
        }
        // one chat of a convoy
        st.upsert_chat(&Chat { convoy: true, ..st.chat("talk").unwrap() }).unwrap();
        st.upsert_chat(&Chat { id: "convoy".into(), title: "convoy".into(), ..st.chat("talk").unwrap() }).unwrap();
        // tasks of each chat in a mixed order: the list says what is most urgent
        let mut n = 0;
        for (chat, statuses) in [
            ("waits", vec![Status::Done, Status::Working, Status::NeedsHuman, Status::Queued]),
            ("failed", vec![Status::Working, Status::Failed, Status::Done]),
            ("working", vec![Status::Done, Status::Queued, Status::Checking]),
            ("queued", vec![Status::Done, Status::Queued]),
            ("done", vec![Status::Cancelled, Status::Done]),
            ("old", vec![]),
        ] {
            for s in statuses {
                n += 1;
                seed_task(st, p, Some(chat), s, n);
            }
        }
        // a swarm task: its own entry, no chat of it in the store yet
        let sw = seed_task(st, p, None, Status::Working, 90);
        let mut t = st.task(&sw).unwrap();
        t.swarm = Some(SwarmSpec::default());
        st.upsert_task(&t).unwrap();
        // the last answer of a plain conversation
        seed_msg(st, "talk", "a1", Who::Model, &format!("{} tail", "Billing sums usage. ".repeat(10)), Local::now());
    });
    let _ = &e;
    let list = chats.list("fake");
    let state = |id: &str| list.iter().find(|s| s.id == id).unwrap_or_else(|| panic!("{id} is not listed")).state;
    assert_eq!(state("waits"), ChatState::WaitsYou);
    assert_eq!(state("failed"), ChatState::Failed);
    assert_eq!(state("working"), ChatState::Working);
    assert_eq!(state("queued"), ChatState::Queued);
    assert_eq!(state("done"), ChatState::Done);
    assert_eq!(state("talk"), ChatState::Talk);
    assert_eq!(state("convoy"), ChatState::Talk);
    assert_eq!(list.iter().find(|s| s.id == "waits").unwrap().task_id.as_deref(), Some("task-3"), "the task the state comes from");
    assert!(list.iter().find(|s| s.id == "talk").unwrap().convoy);
    let last = list.iter().find(|s| s.id == "talk").unwrap().last.clone().unwrap();
    assert!(last.starts_with("Billing sums usage.") && last.chars().count() <= 83, "{last}");
    // the swarm task is an entry of its own
    let swarm = list.iter().find(|s| s.swarm).expect("the swarm task is listed");
    assert_eq!((swarm.id.as_str(), swarm.task_id.as_deref(), swarm.state), ("task-90", Some("task-90"), ChatState::Working));
    // another env's chats stay out; an empty env has an empty list
    assert!(list.iter().all(|s| s.id != "other-env"));
    assert!(chats.list("gemini").is_empty());
    // a chat idle for 7 days is in the archive, listed after the others
    assert!(list.iter().find(|s| s.id == "old").unwrap().archived);
    assert!(list.last().unwrap().archived && list.iter().filter(|s| s.archived).count() == 1);

    // the first message brings it back
    chats.send(ChatTarget::Existing("old".into()), "back", "Hello again").unwrap();
    assert!(!chats.list("fake").iter().find(|s| s.id == "old").unwrap().archived);
}

#[test]
fn a_long_feed_comes_in_pages_of_200_newest_first_page_then_older_ones() {
    let (_env, _p, _e, chats, _fa) = quiet(|st, p| {
        seed_chat(st, p, "long", "fake", 0);
        let t0 = Local::now() - chrono::Duration::hours(1);
        for i in 0..250 {
            seed_msg(st, "long", &format!("m{i}"), Who::Human, &format!("message {i}"), t0 + chrono::Duration::seconds(i));
        }
    });
    let page1 = chats.messages("long", None, 200);
    assert_eq!(page1.len(), 200);
    assert_eq!((page1[0].text.as_str(), page1[199].text.as_str()), ("message 50", "message 249"));
    let page2 = chats.messages("long", Some(page1[0].at), 200);
    assert_eq!(page2.len(), 50);
    assert_eq!((page2[0].text.as_str(), page2[49].text.as_str()), ("message 0", "message 49"));
    assert!(chats.messages("long", Some(page2[0].at), 200).is_empty());
}

#[test]
fn a_chat_with_a_going_task_cannot_be_removed_and_one_with_finished_tasks_goes_while_the_tasks_stay() {
    let r = rig_with(vec![vec![done("Billing is in billing.rs.")]], hold_tasks);
    // a chat that has been answered has a copy of the project
    let answered = r.chats.send(new_target(&r.p), "q1", "Where is billing?").unwrap();
    wait_feed(&r, &answered, "the answer", |m| m.iter().any(|x| x.who == Who::Model));
    let copy = r.fa.calls()[0].cwd.clone();
    assert!(copy.exists());
    let finished = seed_task(&mut r.e.store().lock().unwrap(), &r.p, Some(&answered), Status::Done, 1);

    // a chat with a task that waits for the night
    let busy = r.chats.send(new_target(&r.p), "t1", "Add a footer").unwrap();
    assert_eq!(r.chats.remove(&busy).unwrap_err(), "cancel_task_first");
    assert!(r.chats.list("fake").iter().any(|s| s.id == busy));
    assert_eq!(tasks(&r).len(), 2, "nothing was removed");

    r.chats.remove(&answered).unwrap();
    assert!(r.chats.list("fake").iter().all(|s| s.id != answered));
    assert!(r.chats.messages(&answered, None, 200).is_empty());
    assert!(!copy.exists(), "its copy of the project is gone");
    assert!(!g(&r.env.repo, &["worktree", "list"]).contains(".chats"));
    assert_eq!(task(&r.e, &finished).status, Status::Done, "finished tasks stay in the history of the project");
}

#[test]
fn a_chat_of_one_env_never_calls_the_adapter_of_another() {
    let env = Env::new();
    let p = env.project(|_| {});
    let a = Arc::new(FakeAdapter::new(vec![]).with_role(RoleKind::Chat, vec![vec![done("from A")]]));
    let b = Arc::new(FakeAdapter::new(vec![]).with_id("other").with_role(RoleKind::Chat, vec![vec![done("from B")]]));
    let (clock, _now) = clock_at(12, 0);
    let adapters: Vec<Arc<dyn Adapter>> = vec![a.clone(), b.clone()];
    let e = env.engine(env.store(&p, hold_tasks), adapters, Arc::new(RecordingNotifier::default()), Some(clock));
    let chats = Chats::new(e.clone());

    let in_b = chats.send(ChatTarget::New { env: "other".into(), project_id: p.id.clone() }, "b1", "What is this project?").unwrap();
    let m = (0..).map(|_| { std::thread::sleep(Duration::from_millis(20)); chats.messages(&in_b, None, 200) }).take(1500).find(|m| m.iter().any(|x| x.who == Who::Model)).expect("B answered");
    assert_eq!(m.iter().find(|x| x.who == Who::Model).unwrap().text, "from B");
    // a task made in the chat of B belongs to B, and a later question there goes to B again
    chats.send(ChatTarget::Existing(in_b.clone()), "b2", "Add a license file").unwrap();
    let t = e.store().lock().unwrap().tasks();
    assert_eq!((t.len(), t[0].agent.as_str(), t[0].chat_id.as_deref()), (1, "other", Some(in_b.as_str())));
    assert!(a.calls().is_empty(), "the adapter of env A was never touched by a chat of env B");
    assert_eq!(b.calls().len(), 1);
}

// ---- reasons, shutdown, retry guards, accounting, vendor, recap, notes by id, the roles in the feed ----

fn not_answered(m: &[ChatMsg]) -> Vec<String> {
    m.iter().filter(|x| x.text.starts_with("not_answered")).map(|x| x.text.clone()).collect()
}

#[test]
fn a_missing_login_a_refused_model_and_a_run_without_a_result_are_told_by_key_with_details() {
    let r = rig(vec![vec![FakeStep::Auth("not logged in".into())], vec![FakeStep::RejectModel("no such model".into())], vec![FakeStep::Say("hm".into())]]);
    let id = r.chats.send(new_target(&r.p), "m1", "first").unwrap();
    wait_feed(&r, &id, "auth line", |m| not_answered(m).len() == 1);
    r.chats.send(ChatTarget::Existing(id.clone()), "m2", "second").unwrap();
    wait_feed(&r, &id, "rejected line", |m| not_answered(m).len() == 2);
    r.chats.send(ChatTarget::Existing(id.clone()), "m3", "third").unwrap();
    let m = wait_feed(&r, &id, "no result line", |m| not_answered(m).len() == 3);
    assert_eq!(not_answered(&m), ["not_answered: auth_required: not logged in", "not_answered: model_rejected: no such model", "not_answered: agent_no_result"]);
    assert!(m.iter().filter(|x| x.text.starts_with("not_answered")).all(|x| x.kind == Some(ChatKind::Error)));
}

#[test]
fn a_new_session_that_fails_too_is_a_cli_error_and_the_lost_session_is_forgotten() {
    let am = Arc::new(Amnesia {
        inner: FakeAdapter::new(vec![]).with_role(RoleKind::Chat, vec![vec![done("one")], vec![FakeStep::Fail]]),
        lost: AtomicBool::new(false),
        runs: Mutex::new(vec![]),
    });
    let (_env, p, _e, chats, _now) = build(am.clone(), |_| {});
    let id = chats.send(new_target(&p), "m1", "first").unwrap();
    let wait_for = |what: &str, f: &dyn Fn(&[ChatMsg]) -> bool| {
        let start = Instant::now();
        loop {
            let m = chats.messages(&id, None, 200);
            if f(&m) {
                return m;
            }
            assert!(start.elapsed() < Duration::from_secs(30), "timeout waiting for {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    wait_for("first answer", &|m| m.iter().any(|x| x.who == Who::Model));
    assert!(chats.terminal(&id).unwrap().session.is_some());
    am.lost.store(true, Ordering::SeqCst);
    chats.send(ChatTarget::Existing(id.clone()), "m2", "second").unwrap();
    let m = wait_for("cli error", &|m| not_answered(m).len() == 1);
    assert_eq!(not_answered(&m), ["not_answered: cli_error: fake agent failed"]);
    assert!(m.iter().any(|x| x.text == "new_session_started"));
    assert_eq!(chats.terminal(&id).unwrap().session, None, "the session the CLI lost is not kept");
}

#[test]
fn after_shutdown_nothing_new_is_taken() {
    let r = rig(vec![vec![done("one")]]);
    let id = r.chats.send(new_target(&r.p), "m1", "first").unwrap();
    wait_feed(&r, &id, "answer", |m| m.iter().any(|x| x.who == Who::Model));
    r.chats.shutdown();
    assert_eq!(r.chats.send(ChatTarget::Existing(id.clone()), "m2", "second").unwrap_err(), "shutdown");
    assert_eq!(r.chats.send(new_target(&r.p), "m3", "Add a thing").unwrap_err(), "shutdown");
    assert_eq!(r.chats.retry(&id, "m1").unwrap_err(), "shutdown");
    assert_eq!(feed(&r, &id).iter().filter(|x| x.who == Who::Human).count(), 1, "the refused message was not kept");
    assert!(tasks(&r).is_empty());
    assert_eq!(r.fa.calls().len(), 1);
}

#[test]
fn retry_takes_only_a_message_of_the_person_in_that_chat() {
    let r = rig(vec![vec![FakeStep::Fail], vec![done("Fine now.")]]);
    let a = r.chats.send(new_target(&r.p), "a1", "question A").unwrap();
    let m = wait_feed(&r, &a, "failure", |m| !not_answered(m).is_empty());
    let system_id = m.iter().find(|x| x.who == Who::System).unwrap().id.clone();
    let b = r.chats.send(new_target(&r.p), "b1", "question B").unwrap();
    wait_feed(&r, &b, "answer B", |m| m.iter().any(|x| x.who == Who::Model));
    let model_id = feed(&r, &b).iter().find(|x| x.who == Who::Model).unwrap().id.clone();
    let calls = r.fa.calls().len();

    assert_eq!(r.chats.retry(&a, "b1").unwrap_err(), "message_not_found", "a message of another chat");
    assert_eq!(r.chats.retry(&b, &system_id).unwrap_err(), "message_not_found", "a line of the app");
    assert_eq!(r.chats.retry(&b, &model_id).unwrap_err(), "message_not_found", "an answer of the model");
    assert_eq!(r.chats.retry("no-chat", "a1").unwrap_err(), "chat_not_found");
    assert_eq!(r.fa.calls().len(), calls, "nothing ran");
}

#[test]
fn a_run_of_the_chat_is_accounted_like_a_run_of_a_task() {
    let at = Local::now();
    let w = window("five_hour", at + chrono::Duration::hours(3), false, at);
    let fa = Arc::new(FakeAdapter::new(vec![]).with_role(RoleKind::Chat, vec![vec![FakeStep::Usage(vec![w.clone()]), FakeStep::Cost(0.25), done("ok")]]));
    let (_env, p, e, chats, now) = build(fa.clone(), |_| {});
    let rx = e.subscribe();
    let id = chats.send(new_target(&p), "m1", "How much do you cost?").unwrap();
    let start = Instant::now();
    while chats.messages(&id, None, 200).iter().all(|x| x.who != Who::Model) {
        assert!(start.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(e.limits("fake"), EnvLimits::Known(vec![w]), "the windows the run reported");
    assert!(rx.try_iter().any(|ev| matches!(ev, nightshift_core::engine::EngineEvent::LimitsChanged(env) if env == "fake")), "and the window of the app hears of it");
    let today = now.lock().unwrap().date_naive();
    let day = e.store().lock().unwrap().day_spend("fake", today);
    assert!((day.spent_usd - 0.25).abs() < 1e-9, "{day:?}");
}

#[test]
fn a_limit_hit_in_the_chat_is_told_to_the_whole_app_like_a_task_limit_is() {
    let r = rig(vec![vec![FakeStep::Limit]]);
    let rx = r.e.subscribe();
    let id = r.chats.send(new_target(&r.p), "m1", "How do the tests run?").unwrap();
    wait_feed(&r, &id, "limit line", |m| m.iter().any(|x| x.text.starts_with("will_answer_after")));
    assert!(rx.try_iter().any(|ev| matches!(ev, nightshift_core::engine::EngineEvent::LimitsChanged(env) if env == "fake")));
    // the engine's tasks of the env wait for the same reset
    match r.e.limits("fake") {
        EnvLimits::Known(ws) => assert!(ws.iter().any(|w| w.exhausted && w.resets_at.is_some())),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_chat_and_the_tasks_of_one_env_get_the_same_vendor_and_the_same_model() {
    // a custom model the catalog cannot place: both go by its identifier, and an OpenAI one gets the OpenAI form
    let fake = FakeAdapter::new(vec![]).with_role(RoleKind::Chat, vec![vec![done("hi")]]).with_role(RoleKind::Worker, vec![vec![done("ok")]]);
    let r = rig_fake(fake, |s| {
        s.envs.insert(
            "fake".into(),
            EnvSettings {
                custom_models: vec!["gpt-5.1".into()],
                default_model: Some(ModelChoice { family: "gpt-5.1".into(), version: None }),
                ask_push: false,
                ask_pr: false,
                ..EnvSettings::default()
            },
        );
    });
    let q = r.chats.send(new_target(&r.p), "q", "What is this?").unwrap();
    wait_feed(&r, &q, "answer", |m| m.iter().any(|x| x.who == Who::Model));
    r.chats.send(new_target(&r.p), "t", "Add a line to a.txt").unwrap();
    let start = Instant::now();
    while !r.fa.calls().iter().any(|c| c.role == RoleKind::Worker) {
        assert!(start.elapsed() < Duration::from_secs(30), "no worker run");
        std::thread::sleep(Duration::from_millis(20));
    }
    let calls = r.fa.calls();
    let chat_run = calls.iter().find(|c| c.role == RoleKind::Chat).unwrap();
    let worker_run = calls.iter().find(|c| c.role == RoleKind::Worker).unwrap();
    assert_eq!((chat_run.model.as_deref(), worker_run.model.as_deref()), (Some("gpt-5.1"), Some("gpt-5.1")));
    assert!(chat_run.system.starts_with("# Identity") && worker_run.system.starts_with("# Identity"), "both are OpenAI forms:\n{}\n{}", chat_run.system, worker_run.system);
}

#[test]
fn a_lost_session_gets_a_recap_of_the_last_20_messages_of_the_person_and_the_model_only() {
    let am = Arc::new(Amnesia {
        inner: FakeAdapter::new(vec![]).with_role(RoleKind::Chat, vec![vec![done("Fine.")]]),
        lost: AtomicBool::new(true),
        runs: Mutex::new(vec![]),
    });
    let (_env, p, e, chats, _now) = build(am.clone(), |_| {});
    let t0 = Local::now() - chrono::Duration::hours(1);
    {
        let mut st = e.store().lock().unwrap();
        st.upsert_chat(&Chat { id: "talk".into(), env: "fake".into(), project_id: p.id.clone(), title: "talk".into(), session: Some("old".into()), convoy: false, created_at: t0, updated_at: t0, archived: false }).unwrap();
        for i in 0..30 {
            let who = if i % 2 == 0 { Who::Human } else { Who::Model };
            seed_msg(&st, "talk", &format!("t{i}"), who, &format!("talk-{i:02}"), t0 + chrono::Duration::seconds(2 * i));
            seed_msg(&st, "talk", &format!("s{i}"), Who::System, &format!("note_queued: line-{i:02}"), t0 + chrono::Duration::seconds(2 * i + 1));
        }
    }
    chats.send(ChatTarget::Existing("talk".into()), "new", "latest question").unwrap();
    let start = Instant::now();
    while am.runs.lock().unwrap().len() < 2 {
        assert!(start.elapsed() < Duration::from_secs(30), "no second run");
        std::thread::sleep(Duration::from_millis(20));
    }
    let runs = am.runs.lock().unwrap();
    assert_eq!(runs[0].resume.as_deref(), Some("old"));
    let recap = &runs[1].prompt;
    for i in 10..30 {
        assert!(recap.contains(&format!("talk-{i:02}")), "the recap lacks message {i}");
    }
    assert!(!recap.contains("talk-09"), "only the last 20");
    assert!(!recap.contains("line-"), "lines of the app are not a part of the conversation");
}

#[test]
fn two_notes_with_the_same_words_are_each_reported_delivered_under_their_own_message() {
    let fake = FakeAdapter::new(vec![]).with_role(RoleKind::Worker, vec![vec![write("f.txt", "x"), done("done")]]);
    let r = rig_fake(fake, hold_tasks);
    let id = r.chats.send(new_target(&r.p), "c1", "Add a footer").unwrap();
    let task_id = task_line(&feed(&r, &id)).unwrap().task_id.clone().unwrap();
    r.chats.send(ChatTarget::Existing(id.clone()), "c2", "use the small logo").unwrap();
    r.chats.send(ChatTarget::Existing(id.clone()), "c3", "use the small logo").unwrap();
    r.e.start_now(&task_id).unwrap();
    let m = wait_feed(&r, &id, "both deliveries", |m| m.iter().filter(|x| x.text.starts_with("note_delivered")).count() == 2);
    let to: Vec<&str> = m.iter().filter(|x| x.text.starts_with("note_delivered")).filter_map(|x| x.reply_to.as_deref()).collect();
    assert_eq!(to, ["c2", "c3"], "in the order the notes were given");
}

#[test]
fn the_messages_of_the_roles_and_the_actions_of_the_agents_come_into_the_feed_of_the_chat_once_and_in_order() {
    let fake = FakeAdapter::new(vec![])
        .with_role(RoleKind::Worker, vec![vec![FakeStep::Say("Looking at the footer".into()), write("f.txt", "x"), done("Footer added")]])
        .with_role(RoleKind::Reviewer, vec![approve()]);
    let r = rig_fake(fake, |s| {
        s.quality.on = false;
    });
    let id = r.chats.send(new_target(&r.p), "c1", "Add a footer").unwrap();
    let task_id = task_line(&feed(&r, &id)).unwrap().task_id.clone().unwrap();
    wait_status(&r.e, &task_id, Status::Done);
    let m = wait_feed(&r, &id, "the roles", |m| m.iter().filter(|x| matches!(x.who, Who::Role(_))).count() >= 3);
    let roles: Vec<&ChatMsg> = m.iter().filter(|x| matches!(x.who, Who::Role(_))).collect();
    let shape: Vec<(Who, &str, Option<&str>)> = roles.iter().map(|x| (x.who.clone(), x.text.as_str(), x.tool.as_ref().map(|t| t.name.as_str()))).collect();
    assert_eq!(
        shape,
        [
            (Who::Role(RoleKind::Worker), "Looking at the footer", None),
            (Who::Role(RoleKind::Worker), "", Some("Write")),
            (Who::Role(RoleKind::Worker), "Footer added", None),
            // v3: the fake CLI shows no text of its edit, so the step's diff of the copy follows it (story 226)
            (Who::Role(RoleKind::Worker), "", Some("Edit")),
        ]
    );
    assert_eq!(roles[1].tool.as_ref().unwrap().arg, "f.txt");
    let Some(ActionInput::Edit { path, diff }) = roles[3].action.as_ref().map(|a| &a.input) else { panic!("{:?}", roles[3]) };
    assert_eq!((path.as_str(), diff.contains("+x")), ("f.txt", true), "{diff}");
    assert!(roles.iter().all(|x| x.model.is_some() && x.task_id.as_deref() == Some(task_id.as_str())), "each says which role, model and task");
    assert!(m.windows(2).all(|w| w[0].at <= w[1].at), "in the order of time");
}

#[test]
fn the_answer_streams_into_its_own_message_and_its_actions_come_with_their_result_and_stay_in_the_feed() {
    let act = |done: bool, out: Option<&str>| {
        FakeStep::Action(ToolAction { id: "t1".into(), name: "Read".into(), input: ActionInput::Read { path: "a.txt".into() }, output: out.map(String::from), ok: true, done })
    };
    let r = rig(vec![vec![FakeStep::Delta("Hel".into()), FakeStep::Delta("lo".into()), act(false, None), act(true, Some("one")), done("Hello")]]);
    let rx = r.chats.subscribe();
    let id = r.chats.send(new_target(&r.p), "c1", "what is in a.txt?").unwrap();
    let m = wait_feed(&r, &id, "the answer", |m| m.iter().any(|x| x.who == Who::Model && x.text == "Hello"));
    let answer = m.iter().find(|x| x.who == Who::Model && x.action.is_none()).unwrap();
    let stored = m.iter().find(|x| x.action.is_some()).expect("the action's result is in the feed");
    let a = stored.action.as_ref().unwrap();
    assert_eq!((a.id.as_str(), a.done, a.output.as_deref()), ("t1", true, None), "a read that worked keeps no output");
    let evs: Vec<ChatEvent> = rx.try_iter().collect();
    let deltas: Vec<(&str, &str)> = evs.iter().filter_map(|e| if let ChatEvent::Delta { msg_id, text, .. } = e { Some((msg_id.as_str(), text.as_str())) } else { None }).collect();
    assert_eq!(deltas, [(answer.id.as_str(), "Hel"), (answer.id.as_str(), "lo")], "pieces of the text go to the id of the message it ends in");
    let started: Vec<&str> = evs.iter().filter_map(|e| if let ChatEvent::Action { msg_id, action, .. } = e { (!action.done).then_some(msg_id.as_str()) } else { None }).collect();
    assert_eq!(started, [stored.id.as_str()], "the start is sent under the id its result is stored with");
}

#[test]
fn a_side_question_is_answered_read_only_with_the_chat_context_and_leaves_the_feed_alone() {
    let r = rig(vec![vec![done("The build uses cargo.")], vec![done("It is src/main.rs.")]]);
    let id = r.chats.send(new_target(&r.p), "s1", "How is it built?").unwrap();
    wait_feed(&r, &id, "the first answer", |m| m.iter().any(|x| x.who == Who::Model));
    let before = feed(&r, &id);
    let session = r.chats.list("fake")[0].clone();
    assert_eq!(r.chats.side_ask(&id, "  ").unwrap_err(), "empty_or_too_long");
    assert_eq!(r.chats.side_ask(&id, "Where is main?").unwrap(), "It is src/main.rs.");
    let calls = r.fa.calls();
    let side = calls.last().unwrap();
    assert!(side.read_only && side.resume.is_none(), "read-only and not the chat's session");
    assert!(side.prompt.contains("How is it built?") && side.prompt.contains("Where is main?"), "the context holds the talk: {}", side.prompt);
    assert_eq!(feed(&r, &id), before, "nothing is written to the feed");
    assert_eq!(r.chats.list("fake")[0].last, session.last);
}
