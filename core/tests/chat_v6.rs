//! v6 chat (task 27): the queue and the limit wait on disk, no double send after a restart, "Повторить" after an
//! answer cut by a restart, the thoughts of Claude, the words of the spinner from the settings of Claude Code.
mod common;
use common::*;
use nightshift_core::agent::adapters::Claude;
use nightshift_core::agent::{self, Adapter, AgentEvent, FakeAdapter, FakeStep, Run};
use nightshift_core::chat::{ChatEvent, ChatTarget, Chats};
use nightshift_core::engine::{Clock, Engine};
use nightshift_core::model::*;
use nightshift_core::notify::RecordingNotifier;
use nightshift_core::store::Store;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// One app "process": an engine over the store in `<env>/data` and the chats over it. A restart = a new `App`
/// on the same folder with the same adapter (its scripts go on where they were).
struct App {
    e: Arc<Engine>,
    chats: Arc<Chats>,
}

fn start(env: &Env, p: &Project, a: Arc<dyn Adapter>, clock: Clock) -> App {
    let mut st = Store::open(&env.d.path().join("data")).unwrap();
    st.upsert_project(p.clone()).unwrap();
    let mut s = st.settings();
    s.quality.on = false;
    s.envs.entry(a.id().to_string()).or_default();
    st.set_settings(s).unwrap();
    let e = Engine::new_with_clock(st, vec![a], Arc::new(RecordingNotifier::default()), env.d.path().join("wts"), clock);
    let chats = Chats::new(e.clone());
    App { e, chats }
}

impl App {
    /// The app goes away the hard way: nothing more is written by it.
    fn stop(self) {
        self.chats.shutdown();
        self.e.shutdown();
        std::thread::sleep(Duration::from_millis(150));
    }
    fn feed(&self, chat: &str) -> Vec<ChatMsg> {
        self.chats.messages(chat, None, 200)
    }
    fn wait_feed(&self, chat: &str, what: &str, pred: impl Fn(&[ChatMsg]) -> bool) -> Vec<ChatMsg> {
        let start = Instant::now();
        loop {
            let m = self.feed(chat);
            if pred(&m) {
                return m;
            }
            assert!(start.elapsed() < Duration::from_secs(30), "timeout waiting for {what}; feed: {:#?}", m.iter().map(|x| (&x.who, &x.text, &x.reply_to)).collect::<Vec<_>>());
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn new_target(p: &Project) -> ChatTarget {
    ChatTarget::New { env: "fake".into(), project_id: p.id.clone() }
}

fn chat_fake(scripts: Vec<Vec<FakeStep>>) -> Arc<FakeAdapter> {
    Arc::new(FakeAdapter::new(vec![]).with_role(RoleKind::Chat, scripts))
}

fn texts(m: &[ChatMsg]) -> Vec<String> {
    m.iter().map(|x| x.text.clone()).collect()
}

fn model_text<'a>(m: &'a [ChatMsg], reply_to: &str) -> Option<&'a str> {
    m.iter().find(|x| x.who == Who::Model && x.reply_to.as_deref() == Some(reply_to)).map(|x| x.text.as_str())
}

#[test]
fn a_queued_message_and_a_limit_wait_come_back_after_a_restart_and_go_by_themselves_after_the_reset() {
    let env = Env::new();
    let p = env.project(|_| {});
    let fa = chat_fake(vec![vec![FakeStep::Limit], vec![done("Tests run with cargo.")], vec![done("And the docs are in docs/.")]]);
    let (clock, now) = clock_at(12, 0);
    let app = start(&env, &p, fa.clone(), clock.clone());
    let id = app.chats.send(new_target(&p), "m1", "How do the tests run?").unwrap();
    app.wait_feed(&id, "the limit line", |m| m.iter().any(|x| x.text == "will_answer_after: 12:30"));
    // the chat waits: the next message stands in the queue, not in the feed
    assert_eq!(app.chats.send(ChatTarget::Existing(id.clone()), "m2", "Where are the docs?").unwrap(), id);
    let (queue, wait) = app.chats.queue(&id);
    assert_eq!(queue.iter().map(|q| (q.id.as_str(), q.text.as_str())).collect::<Vec<_>>(), [("m2", "Where are the docs?")]);
    assert_eq!(wait.as_ref().map(|w| w.msg_id.as_str()), Some("m1"));
    assert!(app.feed(&id).iter().all(|m| m.id != "m2"), "a queued message is not in the feed yet");
    app.stop();

    let app = start(&env, &p, fa.clone(), clock);
    let (queue, wait) = app.chats.queue(&id);
    assert_eq!(queue.iter().map(|q| q.id.as_str()).collect::<Vec<_>>(), ["m2"], "the queue is on disk");
    let wait = wait.expect("the wait is on disk");
    assert_eq!((wait.msg_id.as_str(), wait.resets_at.format("%H:%M").to_string().as_str()), ("m1", "12:30"));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(fa.calls().len(), 1, "nothing runs before the reset");

    advance(&now, 31 * 60);
    let m = app.wait_feed(&id, "both answers", |m| model_text(m, "m2").is_some());
    assert_eq!(model_text(&m, "m1"), Some("Tests run with cargo."));
    assert_eq!(model_text(&m, "m2"), Some("And the docs are in docs/."));
    let pos = |pred: &dyn Fn(&ChatMsg) -> bool| m.iter().position(pred).unwrap();
    assert!(pos(&|x| x.id == "m2") > pos(&|x| x.who == Who::Model && x.reply_to.as_deref() == Some("m1")), "the queued message goes after the first answer: {:?}", texts(&m));
    assert_eq!(fa.calls().len(), 3);
    let (queue, wait) = app.chats.queue(&id);
    assert!(queue.is_empty() && wait.is_none());
}

#[test]
fn a_queued_message_is_edited_or_dropped_by_its_id_and_the_edit_survives_a_restart() {
    let env = Env::new();
    let p = env.project(|_| {});
    let fa = chat_fake(vec![vec![FakeStep::Limit], vec![done("one")], vec![done("two")]]);
    let (clock, now) = clock_at(12, 0);
    let app = start(&env, &p, fa.clone(), clock.clone());
    let id = app.chats.send(new_target(&p), "m1", "first").unwrap();
    app.wait_feed(&id, "the limit line", |m| m.iter().any(|x| x.text.starts_with("will_answer_after")));
    app.chats.queue_add(&id, "q1", "second, typo", false).unwrap();
    app.chats.queue_add(&id, "q2", "third", false).unwrap();
    app.chats.queue_add(&id, "q0", "zero, first of all", true).unwrap();
    app.chats.queue_add(&id, "q0", "zero again", true).unwrap(); // the same id again: nothing
    assert_eq!(app.chats.queue(&id).0.iter().map(|q| q.id.as_str()).collect::<Vec<_>>(), ["q0", "q1", "q2"]);
    app.chats.queue_edit(&id, "q1", "second").unwrap();
    app.chats.queue_drop(&id, "q0").unwrap();
    assert_eq!(app.chats.queue_edit(&id, "nope", "x").unwrap_err(), "queued_not_found");
    assert_eq!(app.chats.queue_drop(&id, "nope").unwrap_err(), "queued_not_found");
    assert!(app.chats.queue_edit(&id, "q2", "   ").is_err(), "an empty text is refused");
    app.stop();

    let app = start(&env, &p, fa.clone(), clock);
    assert_eq!(app.chats.queue(&id).0.iter().map(|q| (q.id.as_str(), q.text.as_str())).collect::<Vec<_>>(), [("q1", "second"), ("q2", "third")]);
    advance(&now, 31 * 60);
    let m = app.wait_feed(&id, "the queue is answered", |m| m.iter().filter(|x| x.who == Who::Model).count() == 3 || m.iter().any(|x| x.kind == Some(ChatKind::Error)));
    let humans: Vec<&str> = m.iter().filter(|x| x.who == Who::Human).map(|x| x.text.as_str()).collect();
    assert_eq!(humans, ["first", "second", "third"], "the edited text goes, the dropped one does not; feed: {:?}", m.iter().map(|x| (&x.who, &x.text)).collect::<Vec<_>>());
}

#[test]
fn the_same_message_id_again_after_a_restart_makes_no_second_chat_and_no_second_answer() {
    let env = Env::new();
    let p = env.project(|_| {});
    let fa = chat_fake(vec![vec![done("Here.")], vec![done("Twice?")]]);
    let (clock, _now) = clock_at(12, 0);
    let app = start(&env, &p, fa.clone(), clock.clone());
    let id = app.chats.send(new_target(&p), "n1", "Where is main?").unwrap();
    app.wait_feed(&id, "the answer", |m| model_text(m, "n1").is_some());
    app.stop();

    let app = start(&env, &p, fa.clone(), clock);
    assert_eq!(app.chats.send(new_target(&p), "n1", "Where is main?").unwrap(), id, "the window sends the same id again");
    assert_eq!(app.chats.send(ChatTarget::Existing(id.clone()), "n1", "Where is main?").unwrap(), id);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(app.chats.list("fake").len(), 1, "no second chat");
    assert_eq!(app.feed(&id).iter().filter(|m| m.who == Who::Human).count(), 1);
    assert_eq!(fa.calls().len(), 1, "no second answer");
}

#[test]
fn an_answer_cut_by_a_restart_gets_a_retry_line_and_is_not_run_again_by_itself() {
    let env = Env::new();
    let p = env.project(|_| {});
    let fa = chat_fake(vec![vec![FakeStep::Delta("Let me ".into()), FakeStep::Sleep(20_000), done("never")], vec![done("Now complete.")]]);
    let (clock, _now) = clock_at(12, 0);
    let app = start(&env, &p, fa.clone(), clock.clone());
    let rx = app.chats.subscribe();
    let id = app.chats.send(new_target(&p), "c1", "Explain the build").unwrap();
    let start_t = Instant::now();
    while !rx.try_iter().any(|e| matches!(e, ChatEvent::Delta { .. })) {
        assert!(start_t.elapsed() < Duration::from_secs(30), "the answer never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    app.stop();

    let app = start(&env, &p, fa.clone(), clock);
    let m = app.wait_feed(&id, "the retry line", |m| m.iter().any(|x| x.reply_to.as_deref() == Some("c1") && x.who == Who::System));
    let line = m.iter().find(|x| x.reply_to.as_deref() == Some("c1") && x.who == Who::System).unwrap();
    assert_eq!((line.text.as_str(), line.kind), ("not_answered: interrupted", Some(ChatKind::Error)));
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(fa.calls().len(), 1, "an answer that had started is not run again without asking");
    app.chats.retry(&id, "c1").unwrap();
    let m = app.wait_feed(&id, "the answer after retry", |m| model_text(m, "c1").is_some());
    assert_eq!(model_text(&m, "c1"), Some("Now complete."));
}

#[test]
fn a_message_sent_while_the_model_answers_waits_in_the_queue_and_stop_ends_the_answer_calmly() {
    let env = Env::new();
    let p = env.project(|_| {});
    let fa = chat_fake(vec![vec![FakeStep::Sleep(20_000), done("never")], vec![done("Second answer.")]]);
    let (clock, _now) = clock_at(12, 0);
    let app = start(&env, &p, fa.clone(), clock);
    let id = app.chats.send(new_target(&p), "s1", "slow question").unwrap();
    until("the first run", || fa.calls().len() == 1);
    app.chats.send(ChatTarget::Existing(id.clone()), "s2", "next question").unwrap();
    assert_eq!(app.chats.queue(&id).0.iter().map(|q| q.id.as_str()).collect::<Vec<_>>(), ["s2"]);
    app.chats.stop_answer(&id).unwrap();
    let m = app.wait_feed(&id, "the second answer", |m| model_text(m, "s2").is_some());
    let line = m.iter().find(|x| x.reply_to.as_deref() == Some("s1")).unwrap();
    assert_eq!(line.text, "stopped", "a stop is calm, with «Повторить»");
    assert!(app.chats.queue(&id).0.is_empty());
    assert_eq!(app.chats.stop_answer("no-chat").unwrap_err(), "chat_not_found");
}

// ---- the thoughts of the model ----

/// The fake with thoughts: a line `say` whose text starts with `think:` is a piece of thinking.
struct Thinker(FakeAdapter);

impl Adapter for Thinker {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn name(&self) -> &str {
        self.0.name()
    }
    fn available(&self) -> Result<(), String> {
        Ok(())
    }
    fn command(&self, run: &Run) -> Command {
        self.0.command(run)
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        self.0
            .parse(line)
            .into_iter()
            .map(|e| match e {
                AgentEvent::Text(t) if t.starts_with("think:") => AgentEvent::Thinking(t.trim_start_matches("think:").to_string()),
                other => other,
            })
            .collect()
    }
    fn finish(&self, exit_ok: bool, all: &str) -> Vec<AgentEvent> {
        self.0.finish(exit_ok, all)
    }
}

#[test]
fn the_thoughts_of_the_model_come_to_the_window_under_the_id_of_the_answer_once() {
    let env = Env::new();
    let p = env.project(|_| {});
    let say = |t: &str| FakeStep::Say(t.into());
    // the pieces, then the whole block repeated (a CLI that sends both): the block is not shown twice
    let fa = Arc::new(Thinker(FakeAdapter::new(vec![]).with_role(RoleKind::Chat, vec![vec![say("think:The user asks "), say("think:about tests."), say("think:The user asks about tests."), done("cargo test")]])));
    let (clock, _now) = clock_at(12, 0);
    let app = start(&env, &p, fa, clock);
    let rx = app.chats.subscribe();
    let id = app.chats.send(new_target(&p), "t1", "How to test?").unwrap();
    let m = app.wait_feed(&id, "the answer", |m| model_text(m, "t1").is_some());
    let answer_id = m.iter().find(|x| x.who == Who::Model).unwrap().id.clone();
    std::thread::sleep(Duration::from_millis(100));
    let thoughts: Vec<(String, String, String)> = rx
        .try_iter()
        .filter_map(|e| match e {
            ChatEvent::Thinking { chat, msg_id, text } => Some((chat, msg_id, text)),
            _ => None,
        })
        .collect();
    assert_eq!(thoughts.iter().map(|t| t.2.as_str()).collect::<String>(), "The user asks about tests.");
    assert!(thoughts.iter().all(|t| t.0 == id && t.1 == answer_id), "{thoughts:?}");
}

#[test]
fn claude_thinking_blocks_and_deltas_become_thinking_events_and_the_text_stays_text() {
    let path = format!("{}/tests/fixtures/claude_stream_thinking.jsonl", env!("CARGO_MANIFEST_DIR"));
    let lines = std::fs::read_to_string(path).unwrap();
    let a = Claude::new();
    let events: Vec<AgentEvent> = lines.lines().flat_map(|l| a.parse(l)).collect();
    let deltas: String = events.iter().filter_map(|e| if let AgentEvent::Thinking(t) = e { Some(t.as_str()) } else { None }).take(2).collect();
    assert_eq!(deltas, "The person wants the test command. It is in the Makefile.");
    let blocks: Vec<&String> = events.iter().filter_map(|e| if let AgentEvent::Thinking(t) = e { Some(t) } else { None }).collect();
    assert_eq!(blocks.len(), 3, "two deltas and the whole block of the assistant message: {blocks:?}");
    assert_eq!(blocks[2], "The person wants the test command. It is in the Makefile.");
    assert!(events.iter().any(|e| matches!(e, AgentEvent::TextDelta(t) if t == "Run `make test`.")));
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Text(t) if t == "Run `make test`.")));
    assert!(!events.iter().any(|e| matches!(e, AgentEvent::Text(t) if t.contains("Makefile"))), "a thought is never the answer");
}

// ---- the words of the spinner (G84) ----

#[test]
fn the_spinner_words_come_from_spinner_verbs_of_the_claude_settings_of_the_project_the_local_file_last() {
    let d = tempfile::tempdir().unwrap();
    let cl = d.path().join(".claude");
    std::fs::create_dir_all(&cl).unwrap();
    std::fs::write(cl.join("settings.json"), r#"{"spinnerVerbs": {"mode": "append", "verbs": ["Варю", "Колдую"]}, "model": "opus"}"#).unwrap();
    let h = Claude::new().ui_hints(d.path());
    assert_eq!(h.spinner_verbs, Some(vec!["Варю".to_string(), "Колдую".to_string()]));
    assert!(h.spinner_append);
    std::fs::write(cl.join("settings.local.json"), r#"{"spinnerVerbs": {"mode": "replace", "verbs": ["Думаю над кодом", 7, "", "Тку"]}}"#).unwrap();
    let h = Claude::new().ui_hints(d.path());
    assert_eq!(h.spinner_verbs, Some(vec!["Думаю над кодом".to_string(), "Тку".to_string()]), "only text, the local file wins");
    assert!(!h.spinner_append);
}

#[test]
fn without_spinner_verbs_or_with_a_broken_file_there_are_no_words_and_other_envs_have_none() {
    let d = tempfile::tempdir().unwrap();
    let f = |name: &str, body: &str| {
        let p = d.path().join(name);
        std::fs::write(&p, body).unwrap();
        p
    };
    let none = f("a.json", r#"{"model": "opus"}"#);
    let broken = f("b.json", "{ not json");
    let wrong = f("c.json", r#"{"spinnerVerbs": "Thinking"}"#);
    for files in [vec![none.clone()], vec![broken.clone()], vec![wrong.clone()], vec![d.path().join("missing.json")], vec![]] {
        assert_eq!(agent::spinner_hints(&files).spinner_verbs, None, "{files:?}");
    }
    let good = f("d.json", r#"{"spinnerVerbs": {"mode": "replace", "verbs": ["Тку"]}}"#);
    assert_eq!(agent::spinner_hints(&[good, broken]).spinner_verbs, Some(vec!["Тку".to_string()]), "a broken later file does not erase the words");
    assert_eq!(FakeAdapter::new(vec![]).ui_hints(d.path()).spinner_verbs, None);
}
