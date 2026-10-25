//! Chat with the model of an env (G20): the main surface of an env. A chat = env + project + CLI
//! session. Every message of the person is handled in one of three ways, in this order:
//!  1. a task of the chat is going on: the message is a note to it (or the answer to its question), G05;
//!  2. it starts with a verb of command (or the model ends its answer with `NS_TASK:`): a task is made
//!     through `Engine::submit`, the same way as from the task field;
//!  3. otherwise a run of the `Chat` role answers it, read-only, in the chat's own copy of the project.
//!
//! Works on top of the public API of `Engine`. One env per chat and per task: the adapter of a chat is the
//! adapter of its env and nothing else (G22). Lines of the feed are keys `key: details`, the UI translates.
mod attach;
mod reply;
mod term;
mod verbs;
pub use attach::{from_base64, image_ext, Attached, MAX_ATTACHMENTS, MAX_ATTACHMENT_BYTES};
pub use term::TermLaunch;

use crate::engine::{Engine, EngineEvent, NewTask, NoteResult};
use crate::git;
use crate::model::*;
use crate::prompt;
use chrono::{DateTime, Duration as CDuration, Local};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

/// Keys of the system lines of the feed (details after `: `).
pub mod keys {
    /// A task was made of the message (`kind` = Task, `task_id` set); the details are its title.
    pub const THIS_IS_TASK: &str = "this_is_task";
    /// The task could not be made; the details are the engine's reason.
    pub const TASK_NOT_CREATED: &str = "task_not_created";
    /// The model did not answer; the details are the reason. `retry` repeats.
    pub const NOT_ANSWERED: &str = "not_answered";
    /// The run was stopped (by the person); not an error, the feed says "stopped" in a neutral tone. `retry` repeats.
    pub const STOPPED: &str = "stopped";
    /// The env's limit is used up; the details are the clock time (HH:MM) the answer comes after.
    pub const WILL_ANSWER_AFTER: &str = "will_answer_after";
    /// The CLI could not continue the session; a new one started with a recap of the last 20 messages.
    pub const NEW_SESSION_STARTED: &str = "new_session_started";
    /// The message went to a running task as a note; the details are the task's title.
    pub const NOTE_QUEUED: &str = "note_queued";
    /// The note reached the executor; the details are the clock time (HH:MM).
    pub const NOTE_DELIVERED: &str = "note_delivered";
    /// The message answered the question of a task; the details are the task's title.
    pub const ANSWER_PASSED: &str = "answer_passed";
    /// The task could not take the note; the details are the reason key (`task_already_merged`, `task_cancelled`).
    pub const NOTE_NOT_PASSED: &str = "note_not_passed";
    /// "This was a question": the task was cancelled and the message goes back to the model.
    pub const BACK_TO_QUESTION: &str = "back_to_question";
    /// The chat was opened in the person's terminal: edits made there in the read-only copy are not kept (A07.2).
    pub const TERMINAL_EDITS_LOST: &str = "terminal_edits_lost";
    /// v6 (G83): the chat is open in a terminal tab (the same session in the real CLI); edits there are not kept.
    pub const OPENED_IN_TERMINAL: &str = "opened_in_terminal";
    /// v6 (G85): back from the terminal of an env whose session the app cannot read: the talk is there.
    pub const CONTINUED_IN_TERMINAL: &str = "continued_in_terminal";
    /// v6 (G83.1): back from the terminal; the details are how many messages of it were read into the feed (Claude Code).
    pub const BACK_FROM_TERMINAL: &str = "back_from_terminal";
}

/// Reasons after `not_answered: ` - keys, details after `: `; the UI translates the key.
pub mod reason {
    pub const AUTH_REQUIRED: &str = crate::engine::AUTH_REQUIRED;
    pub const ENV_UNAVAILABLE: &str = crate::engine::ENV_UNAVAILABLE;
    pub const AGENT_NOT_FOUND: &str = "agent_not_found";
    pub const PROJECT_NOT_FOUND: &str = "project_not_found";
    pub const COPY_FAILED: &str = "copy_failed";
    pub const MODEL_REJECTED: &str = "model_rejected";
    /// The CLI ended with an error; the details are the end of what it said.
    pub const CLI_ERROR: &str = "cli_error";
    /// The CLI could not be run at all (the system's error).
    pub const RUN_FAILED: &str = "run_failed";
    /// The CLI ended without a result.
    pub const AGENT_NO_RESULT: &str = "agent_no_result";
    /// The run was stopped; shown as the line `keys::STOPPED`.
    pub const STOPPED: &str = "stopped";
    /// v6 (G62.4): the app ended while the answer was being written; `retry` repeats, nothing repeats by itself.
    pub const INTERRUPTED: &str = "interrupted";
}

/// Where a message goes: an existing chat or a new one for an env and a project.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatTarget {
    Existing(ChatId),
    New { env: EnvId, project_id: ProjectId },
}

/// The state shown in the list, most urgent first (the derived order is the order of urgency).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatState {
    WaitsYou,
    Failed,
    Working,
    Queued,
    Done,
    /// No tasks: just a conversation.
    Talk,
}

/// One item of the list of chats.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatSummary {
    pub id: ChatId,
    pub env: EnvId,
    pub project_id: ProjectId,
    pub title: String,
    pub state: ChatState,
    /// The most urgent task of the chat (the one `state` comes from).
    pub task_id: Option<TaskId>,
    /// Start of the last answer of the model (about 80 characters); None = nothing answered yet.
    pub last: Option<String>,
    pub updated_at: DateTime<Local>,
    pub archived: bool,
    /// A chat of a convoy of tasks.
    pub convoy: bool,
    /// The chat of a swarm task.
    pub swarm: bool,
    /// v6 (G83): the chat is open in a terminal now.
    #[serde(default)]
    pub in_terminal: bool,
}

/// What the window listens to.
#[allow(clippy::large_enum_variant)] // a message travels whole to the window, like a task in `EngineEvent`
#[derive(Clone, Debug, PartialEq)]
pub enum ChatEvent {
    /// A message was added to a feed. One with an `action` is the stored result of that action.
    Msg(ChatMsg),
    /// v3: an action of the model or of a role of the chat's task, at its start (not stored) and again with its
    /// result (then also as `Msg`); `msg_id` is the id of the message that stores it.
    Action { chat: ChatId, msg_id: String, action: ToolAction },
    /// v3: a piece of the text being written; `msg_id` is the id of the message the whole text comes in.
    Delta { chat: ChatId, msg_id: String, text: String },
    /// v6 (G80.1): a piece of what the model thinks before the answer `msg_id` (not stored).
    Thinking { chat: ChatId, msg_id: String, text: String },
    /// v6 (G62): the queue or the limit wait of the chat changed; the whole new state.
    Queue { chat: ChatId, queue: Vec<QueuedMsg>, wait: Option<ChatWait> },
    /// The chat's list item may have changed (a task of it moved, the chat was created or archived).
    Changed(ChatId),
    Removed(ChatId),
}

/// What the app needs to open the chat in the person's terminal (A07.2).
#[derive(Debug)]
pub struct Terminal {
    pub env: EnvId,
    /// The chat's copy of the project: the terminal opens here.
    pub copy: PathBuf,
    pub session: Option<String>,
    /// The env's command to continue the session. None = it cannot (or no session yet): the app opens a new
    /// session of the env's CLI in `copy`.
    pub command: Option<Command>,
}

/// A run of the model answers one message; `no_task` = do not look for `NS_TASK:` in the answer
/// (the person said "this was a question").
#[derive(Clone, Debug)]
pub(crate) struct Job {
    pub msg: ChatMsg,
    pub no_task: bool,
}

/// An answer held back by a used-up limit (its copy on disk is `Chat.wait`).
struct Held {
    chat: ChatId,
    job: Job,
    until: DateTime<Local>,
}

/// A note sent to a task, waiting for the line "note delivered" of the task's log (matched by the note's id).
#[derive(Clone)]
struct PendingNote {
    task: TaskId,
    msg: ChatMsg,
}

/// Notes sent and not yet reported delivered; a delivery that came before the note was registered waits in `early`.
#[derive(Default)]
struct NoteBook {
    pending: HashMap<String, PendingNote>,
    early: HashMap<String, DateTime<Local>>,
}

/// What of the task's log goes into the chat's feed (G05, decision of the person): the prose of the roles
/// that write and the actions of every role. The roles that answer in JSON (planner, critic, reviewer ...) stay out.
fn speaks(role: RoleKind) -> bool {
    matches!(role, RoleKind::Worker | RoleKind::TestAuthor | RoleKind::Integrator | RoleKind::Watcher)
}

/// How far back one read of a feed goes.
const PAGE: usize = 50;

/// Chats of the app over one `Engine`.
pub struct Chats {
    pub(crate) engine: Arc<Engine>,
    subs: Mutex<Vec<Sender<ChatEvent>>>,
    /// One `send` at a time: the id check and the creation of the chat are one step.
    send_lock: Mutex<()>,
    /// msg id -> chat made for it (a New target sent twice).
    first_msgs: Mutex<HashMap<String, ChatId>>,
    queues: Mutex<HashMap<ChatId, VecDeque<Job>>>,
    waiting: Mutex<Vec<Held>>,
    /// chat -> the stop switch of its answer being written (`stop_answer`).
    stops: Mutex<HashMap<ChatId, Arc<AtomicBool>>>,
    notes: Mutex<NoteBook>,
    push_lock: Mutex<()>,
    /// task -> its chat (None = not a chat's task), for the log lines mirrored into feeds.
    task_chat: Mutex<HashMap<TaskId, Option<ChatId>>>,
    /// (task, role) -> the last prose mirrored, so the final text of a run is not shown twice.
    last_said: Mutex<HashMap<(TaskId, RoleKind), String>>,
    /// (task, action id) -> the id of the message its result is stored in, from its start on.
    act_msgs: Mutex<HashMap<(TaskId, String), String>>,
    /// (task, role) -> the id of the message the text being streamed will be stored in.
    delta_msgs: Mutex<HashMap<(TaskId, RoleKind), String>>,
    /// Where Claude Code keeps its sessions; None = `~/.claude/projects` (`sessions::claude_dir`).
    claude_projects: Mutex<Option<PathBuf>>,
    pub(crate) stop: AtomicBool,
}

pub(crate) fn lk<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Tasks that are still going on or wait: a message to the chat is a note to them, the chat cannot be removed.
fn is_open(s: &Status) -> bool {
    !matches!(s, Status::Done | Status::DoneNotPushed | Status::Failed | Status::Cancelled | Status::Reverted)
}

fn state_of(s: &Status) -> Option<ChatState> {
    Some(match s {
        Status::NeedsHuman | Status::AwaitingOutward | Status::MergeBlocked => ChatState::WaitsYou,
        Status::Failed | Status::DoneNotPushed => ChatState::Failed,
        Status::Preparing | Status::Working | Status::Checking | Status::Reviewing | Status::Merging | Status::PausedLimit | Status::HandedOff => {
            ChatState::Working
        }
        Status::Queued => ChatState::Queued,
        Status::Done | Status::Reverted => ChatState::Done,
        Status::Cancelled => return None,
    })
}

fn short(text: &str, n: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= n {
        flat
    } else {
        format!("{}...", flat.chars().take(n).collect::<String>())
    }
}

impl Chats {
    /// Chats over an engine: the adapters, the clock, the secrets of the env and the place for the chats' copies
    /// of projects (`<worktrees dir>/.chats`) are the engine's.
    pub fn new(engine: Arc<Engine>) -> Arc<Chats> {
        let me = Arc::new(Chats {
            engine,
            subs: Mutex::new(vec![]),
            send_lock: Mutex::new(()),
            first_msgs: Mutex::new(HashMap::new()),
            queues: Mutex::new(HashMap::new()),
            waiting: Mutex::new(vec![]),
            stops: Mutex::new(HashMap::new()),
            notes: Mutex::new(NoteBook::default()),
            push_lock: Mutex::new(()),
            task_chat: Mutex::new(HashMap::new()),
            last_said: Mutex::new(HashMap::new()),
            act_msgs: Mutex::new(HashMap::new()),
            delta_msgs: Mutex::new(HashMap::new()),
            claude_projects: Mutex::new(None),
            stop: AtomicBool::new(false),
        });
        me.restore();
        Chats::spawn_watchers(&me);
        me
    }

    /// G62: what the chats had on disk when the app ended: the limit waits go back to the clock, an answer that
    /// was being written gets "not answered: interrupted" with «Повторить» (it is not run again by itself, it may
    /// have cost money already), the queues go on as soon as their chat is free.
    fn restore(self: &Arc<Self>) {
        let chats = lk(self.engine.store()).all_chats();
        for c in chats {
            if let Some(id) = c.answering.clone() {
                self.update_chat(&c.id, |x| x.answering = None);
                if self.find_msg(&c.id, |m| m.id == id && m.who == Who::Human).is_some() {
                    self.line(&c.id, keys::NOT_ANSWERED, reason::INTERRUPTED, Some(ChatKind::Error), Some(&id), None);
                }
            }
            if let Some(w) = c.wait.clone() {
                match self.find_msg(&c.id, |m| m.id == w.msg_id && m.who == Who::Human) {
                    Some(msg) => lk(&self.waiting).push(Held { chat: c.id.clone(), job: Job { msg, no_task: false }, until: w.resets_at }),
                    None => self.update_chat(&c.id, |x| x.wait = None),
                }
            }
            if !c.queue.is_empty() {
                self.kick(&c.id);
            }
            // G83: the terminals ended with the app: the chat is back, with what was said there
            if c.terminal.is_some() {
                let _ = self.terminal_closed(&c.id);
            }
        }
        self.sweep_pending();
    }

    /// Stops the runs and the watchers; messages stay in the store. `send`, `retry` and `not_a_task` refuse afterwards.
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::SeqCst);
        for f in lk(&self.stops).values() {
            f.store(true, Ordering::SeqCst);
        }
    }

    fn running(&self) -> Result<(), String> {
        if self.stop.load(Ordering::SeqCst) {
            Err("shutdown".into())
        } else {
            Ok(())
        }
    }

    pub fn subscribe(&self) -> Receiver<ChatEvent> {
        let (tx, rx) = channel();
        lk(&self.subs).push(tx);
        rx
    }

    pub(crate) fn emit(&self, e: ChatEvent) {
        lk(&self.subs).retain(|s| s.send(e.clone()).is_ok());
    }

    pub(crate) fn now(&self) -> DateTime<Local> {
        self.engine.now()
    }

    pub(crate) fn settings(&self) -> Settings {
        lk(self.engine.store()).settings()
    }

    pub(crate) fn chat(&self, id: &str) -> Option<Chat> {
        lk(self.engine.store()).chat(id)
    }

    fn update_chat(&self, id: &str, f: impl FnOnce(&mut Chat)) {
        let mut st = lk(self.engine.store());
        if let Some(mut c) = st.chat(id) {
            f(&mut c);
            let _ = st.upsert_chat(&c);
        }
    }

    /// The newest message of the chat that `pred` takes. Reads the feed back a page at a time and stops at it.
    pub(crate) fn find_msg(&self, chat: &str, pred: impl Fn(&ChatMsg) -> bool) -> Option<ChatMsg> {
        let mut before = None;
        loop {
            let page = lk(self.engine.store()).read_chat(chat, before, PAGE);
            if let Some(m) = page.iter().rev().find(|m| pred(m)) {
                return Some(m.clone());
            }
            before = Some(page.first()?.at);
        }
    }

    /// The last `n` messages of the person and of the model before `before`, oldest first (system lines do not count).
    pub(crate) fn recent_talk(&self, chat: &str, before: DateTime<Local>, n: usize) -> Vec<ChatMsg> {
        let mut out: Vec<ChatMsg> = vec![];
        let mut before = Some(before);
        while out.len() < n {
            let page = lk(self.engine.store()).read_chat(chat, before, PAGE);
            let Some(first) = page.first() else { break };
            before = Some(first.at);
            out.extend(page.iter().rev().filter(|m| matches!(m.who, Who::Human | Who::Model)).take(n - out.len()).cloned());
        }
        out.reverse();
        out
    }

    fn chat_tasks(&self, chat: &str) -> Vec<Task> {
        lk(self.engine.store()).tasks().into_iter().filter(|t| t.chat_id.as_deref() == Some(chat)).collect()
    }

    /// Adds a message to the feed. `at` is never earlier than the last message of the chat, so a page
    /// cut at `before` never splits two messages written in the same instant.
    pub(crate) fn push(&self, mut m: ChatMsg) -> Option<ChatMsg> {
        {
            let _g = lk(&self.push_lock);
            let mut st = lk(self.engine.store());
            if let Some(last) = st.read_chat(&m.chat_id, None, 1).last() {
                if m.at <= last.at {
                    m.at = last.at + CDuration::microseconds(1);
                }
            }
            if !st.append_chat_msg(&m).ok()? {
                return None;
            }
            if let Some(mut c) = st.chat(&m.chat_id) {
                c.updated_at = m.at;
                let _ = st.upsert_chat(&c);
            }
        }
        self.emit(ChatEvent::Msg(m.clone()));
        Some(m)
    }

    /// A line of the app in the feed: `key: details`.
    pub(crate) fn line(&self, chat: &str, key: &str, details: &str, kind: Option<ChatKind>, reply_to: Option<&str>, task: Option<&str>) {
        let text = if details.is_empty() { key.to_string() } else { format!("{key}: {details}") };
        self.push(ChatMsg {
            id: uuid::Uuid::new_v4().to_string(),
            chat_id: chat.to_string(),
            at: self.now(),
            who: Who::System,
            text,
            task_id: task.map(str::to_string),
            delivered_at: None,
            kind,
            tool: None,
            model: None,
            reply_to: reply_to.map(str::to_string),
            action: None,
            attachments: vec![],
        });
    }

    // ---- sending ----

    /// Takes a message of the person. `msg_id` is made by the window: the same id again does nothing
    /// (double Enter, click plus Enter). An empty message is refused. Returns the chat the message is in.
    /// The answer of the model comes later as `ChatEvent::Msg`.
    pub fn send(self: &Arc<Self>, target: ChatTarget, msg_id: &str, text: &str) -> Result<ChatId, String> {
        self.send_with(target, msg_id, text, &[])
    }

    /// `send` with images (G65): files `attach` kept, at most `MAX_ATTACHMENTS`, for an env that reads images. They
    /// move into the chat's folder and go to the model as paths. Err also `too_many_images`, `attachment_not_found`,
    /// `images_not_supported` (nothing is made then).
    pub fn send_with(self: &Arc<Self>, target: ChatTarget, msg_id: &str, text: &str, attachments: &[PathBuf]) -> Result<ChatId, String> {
        self.running()?;
        let text = text.trim();
        if text.is_empty() {
            return Err("empty_message".into());
        }
        if msg_id.trim().is_empty() {
            return Err("empty_message_id".into());
        }
        let _g = lk(&self.send_lock);
        let env_of = match &target {
            ChatTarget::Existing(id) => self.chat(id).map(|c| c.env),
            ChatTarget::New { env, .. } => Some(env.clone()),
        };
        let own = match env_of {
            Some(env) => self.check_attachments(&env, attachments)?,
            None => vec![],
        };
        let chat = match target {
            ChatTarget::Existing(id) => {
                let chat = self.chat(&id).ok_or("chat_not_found")?;
                if chat.queue.iter().any(|q| q.id == msg_id) {
                    return Ok(chat.id); // the same message again, still in the queue
                }
                chat
            }
            ChatTarget::New { env, project_id } => {
                if let Some(id) = lk(&self.first_msgs).get(msg_id) {
                    return Ok(id.clone());
                }
                // G62.3: the same first message again after a restart: the chat made for it is on disk
                if let Some(id) = self.chat_with_msg(&env, &project_id, msg_id) {
                    lk(&self.first_msgs).insert(msg_id.to_string(), id.clone());
                    return Ok(id);
                }
                lk(self.engine.store()).project(&project_id).ok_or("project_not_found")?;
                self.engine.adapter(&env).ok_or_else(|| format!("{}: {env}", reason::AGENT_NOT_FOUND))?;
                let now = self.now();
                let chat = Chat {
                    id: uuid::Uuid::new_v4().to_string(),
                    env,
                    project_id,
                    title: short(text.lines().find(|l| !l.trim().is_empty()).unwrap_or(text), 60),
                    session: None,
                    convoy: false,
                    created_at: now,
                    updated_at: now,
                    archived: false,
                    queue: vec![],
                    wait: None,
                    answering: None,
                    terminal: None,
                };
                lk(self.engine.store()).upsert_chat(&chat).map_err(|e| e.to_string())?;
                lk(&self.first_msgs).insert(msg_id.to_string(), chat.id.clone());
                self.emit(ChatEvent::Changed(chat.id.clone()));
                chat
            }
        };
        let attachments = self.move_attachments(&chat.id, own)?;
        // G62: the model is answering or waits for a limit: the message waits its turn in the queue on disk
        if self.answer_busy(&chat) && self.find_msg(&chat.id, |m| m.id == msg_id).is_none() {
            self.push_queued(&chat.id, QueuedMsg { id: msg_id.to_string(), text: text.to_string(), attachments, at: self.now() }, false);
            return Ok(chat.id);
        }
        self.take_in(&chat, msg_id, text, attachments);
        Ok(chat.id)
    }

    /// The message goes into the feed and is handled: a note or answer to a task of the chat, a task, or a question.
    /// False = the same message is in the feed already.
    fn take_in(self: &Arc<Self>, chat: &Chat, msg_id: &str, text: &str, attachments: Vec<PathBuf>) -> bool {
        let msg = ChatMsg {
            id: msg_id.to_string(),
            chat_id: chat.id.clone(),
            at: self.now(),
            who: Who::Human,
            text: text.to_string(),
            task_id: None,
            delivered_at: None,
            kind: None,
            tool: None,
            model: None,
            reply_to: None,
            action: None,
            attachments,
        };
        let Some(msg) = self.push(msg) else { return false }; // the same message again
        if chat.archived {
            self.update_chat(&chat.id, |c| c.archived = false);
            self.emit(ChatEvent::Changed(chat.id.clone()));
        }
        if !self.pass_to_task(chat, &msg) {
            self.route_ordinary(chat, &msg);
        }
        true
    }

    /// A chat of the env and the project whose last page has the message `msg_id` (a New target sent again).
    fn chat_with_msg(&self, env: &str, project: &str, msg_id: &str) -> Option<ChatId> {
        let chats: Vec<Chat> = lk(self.engine.store()).chats(env).into_iter().filter(|c| c.project_id == project).collect();
        chats.into_iter().find(|c| c.queue.iter().any(|q| q.id == msg_id) || self.messages(&c.id, None, PAGE).iter().any(|m| m.id == msg_id)).map(|c| c.id)
    }

    // ---- the queue of a chat (G62) ----

    /// An answer of the model is being written or waits in memory, or the chat waits for a limit.
    fn answer_busy(&self, chat: &Chat) -> bool {
        chat.wait.is_some() || lk(&self.queues).contains_key(&chat.id)
    }

    /// A task of the chat is on its way (not waiting for the person): a queued message waits for it to end.
    fn task_busy(&self, chat: &str) -> bool {
        self.chat_tasks(chat).iter().any(|t| matches!(state_of(&t.status), Some(ChatState::Working | ChatState::Queued)) && t.status != Status::HandedOff)
    }

    fn queue_changed(&self, chat: &str) {
        if let Some(c) = self.chat(chat) {
            self.emit(ChatEvent::Queue { chat: c.id, queue: c.queue, wait: c.wait });
        }
    }

    fn push_queued(&self, chat: &str, q: QueuedMsg, first: bool) {
        self.update_chat(chat, |c| {
            if !c.queue.iter().any(|x| x.id == q.id) {
                if first {
                    c.queue.insert(0, q);
                } else {
                    c.queue.push(q);
                }
            }
        });
        self.queue_changed(chat);
    }

    /// The queue and the limit wait of a chat, as the window shows them.
    pub fn queue(&self, chat: &str) -> (Vec<QueuedMsg>, Option<ChatWait>) {
        self.chat(chat).map(|c| (c.queue, c.wait)).unwrap_or_default()
    }

    /// Puts a message of the person into the queue of the chat (first = before the others, "Сразу"). It goes by
    /// itself when the chat is free: no answer is written, no limit is waited for and no task of the chat is on its
    /// way. The same id again does nothing.
    pub fn queue_add(self: &Arc<Self>, chat: &str, msg_id: &str, text: &str, first: bool) -> Result<(), String> {
        self.queue_add_with(chat, msg_id, text, first, &[])
    }

    /// `queue_add` with images (the checks of `send_with`).
    pub fn queue_add_with(self: &Arc<Self>, chat: &str, msg_id: &str, text: &str, first: bool, attachments: &[PathBuf]) -> Result<(), String> {
        self.running()?;
        let text = text.trim();
        if text.is_empty() {
            return Err("empty_message".into());
        }
        if msg_id.trim().is_empty() {
            return Err("empty_message_id".into());
        }
        {
            let _g = lk(&self.send_lock);
            let c = self.chat(chat).ok_or("chat_not_found")?;
            if c.queue.iter().any(|q| q.id == msg_id) || self.find_msg(chat, |m| m.id == msg_id).is_some() {
                return Ok(());
            }
            let own = self.check_attachments(&c.env, attachments)?;
            let attachments = self.move_attachments(chat, own)?;
            self.push_queued(chat, QueuedMsg { id: msg_id.to_string(), text: text.to_string(), attachments, at: self.now() }, first);
        }
        self.kick(chat);
        Ok(())
    }

    /// Changes the text of a queued message. Err `queued_not_found` = it has gone already (or never was).
    pub fn queue_edit(&self, chat: &str, msg_id: &str, text: &str) -> Result<(), String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("empty_message".into());
        }
        let _g = lk(&self.send_lock);
        let c = self.chat(chat).ok_or("chat_not_found")?;
        if !c.queue.iter().any(|q| q.id == msg_id) {
            return Err("queued_not_found".into());
        }
        self.update_chat(chat, |c| {
            if let Some(q) = c.queue.iter_mut().find(|q| q.id == msg_id) {
                q.text = text.to_string();
            }
        });
        self.queue_changed(chat);
        Ok(())
    }

    /// Takes a message out of the queue. Err `queued_not_found` = it has gone already (or never was).
    pub fn queue_drop(&self, chat: &str, msg_id: &str) -> Result<(), String> {
        let _g = lk(&self.send_lock);
        let c = self.chat(chat).ok_or("chat_not_found")?;
        if !c.queue.iter().any(|q| q.id == msg_id) {
            return Err("queued_not_found".into());
        }
        self.update_chat(chat, |c| c.queue.retain(|q| q.id != msg_id));
        self.queue_changed(chat);
        Ok(())
    }

    /// The first queued message, when the chat is free to take it: out of the queue and into the feed.
    fn next_queued(self: &Arc<Self>, chat: &str) -> bool {
        let Some(c) = self.chat(chat) else { return false };
        if c.wait.is_some() || self.task_busy(chat) {
            return false;
        }
        let Some(q) = c.queue.first().cloned() else { return false };
        self.update_chat(chat, |x| x.queue.retain(|y| y.id != q.id));
        self.queue_changed(chat);
        let Some(c) = self.chat(chat) else { return false };
        self.take_in(&c, &q.id, &q.text, q.attachments);
        true
    }

    /// Starts the chat's runner when it has queued messages and nothing else runs (it takes them one by one).
    fn kick(self: &Arc<Self>, chat: &str) {
        let has = self.chat(chat).is_some_and(|c| !c.queue.is_empty());
        if !has || self.running().is_err() {
            return;
        }
        let start = {
            let mut q = lk(&self.queues);
            if q.contains_key(chat) {
                false
            } else {
                q.insert(chat.to_string(), VecDeque::new());
                true
            }
        };
        if start {
            let me = self.clone();
            let chat = chat.to_string();
            std::thread::spawn(move || me.drain(&chat));
        }
    }

    /// Stops the answer being written in the chat; the feed says "stopped" with «Повторить».
    pub fn stop_answer(&self, chat: &str) -> Result<(), String> {
        self.chat(chat).ok_or("chat_not_found")?;
        if let Some(f) = lk(&self.stops).get(chat) {
            f.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    /// An ordinary message of the chat: a task when it starts with a verb of command, else a question for the model.
    fn route_ordinary(self: &Arc<Self>, chat: &Chat, msg: &ChatMsg) {
        if verbs::starts_with_command(&msg.text) {
            self.make_task(chat, msg);
        } else {
            self.enqueue(&chat.id, Job { msg: msg.clone(), no_task: false });
        }
    }

    /// G05: a task of the chat is going on. True = the message was dealt with (note or answer).
    /// The note goes to the last created of the open tasks; the answer to the one that waits for it.
    fn pass_to_task(&self, chat: &Chat, msg: &ChatMsg) -> bool {
        let mut open: Vec<Task> = self.chat_tasks(&chat.id).into_iter().filter(|t| is_open(&t.status)).collect();
        open.sort_by_key(|t| t.created_at);
        let asks = |t: &&Task| t.status == Status::NeedsHuman && t.question.as_ref().is_some_and(|q| q.kind == QuestionKind::Agent);
        let Some(t) = open.iter().rev().find(asks).or(open.last()).cloned() else { return false };
        let line = |key: &str, details: &str, kind: Option<ChatKind>| self.line(&chat.id, key, details, kind, Some(&msg.id), Some(&t.id));
        match self.engine.note_tracked(&t.id, None, &msg.text) {
            Ok((NoteResult::Queued, note_id)) => {
                line(keys::NOTE_QUEUED, &t.title, None);
                if let Some(nid) = note_id {
                    self.register_note(nid, PendingNote { task: t.id.clone(), msg: msg.clone() });
                }
                true
            }
            Ok((NoteResult::Answered, _)) => {
                line(keys::ANSWER_PASSED, &t.title, None);
                true
            }
            // G05.2: the task landed first; the message is an ordinary one
            Ok((NoteResult::NotDelivered(why), _)) => {
                line(keys::NOTE_NOT_PASSED, &why, Some(ChatKind::Warn));
                false
            }
            Err(e) => {
                line(keys::NOTE_NOT_PASSED, &e, Some(ChatKind::Warn));
                false
            }
        }
    }

    /// The note waits for its "delivered" line; if that came already, it is reported at once.
    fn register_note(&self, note_id: String, n: PendingNote) {
        let early = {
            let mut book = lk(&self.notes);
            let early = book.early.remove(&note_id);
            if early.is_none() {
                book.pending.insert(note_id, n.clone());
            }
            early
        };
        if let Some(at) = early {
            self.report_delivered(&n, at);
        }
    }

    fn report_delivered(&self, n: &PendingNote, at: DateTime<Local>) {
        self.line(&n.msg.chat_id, keys::NOTE_DELIVERED, &at.format("%H:%M").to_string(), None, Some(&n.msg.id), Some(&n.task));
    }

    /// The task of a message: parameters from the person's last choice for the env (spec 105).
    pub(crate) fn make_task(&self, chat: &Chat, msg: &ChatMsg) -> Option<TaskId> {
        let env = self.settings().envs.get(&chat.env).cloned().unwrap_or_default();
        let r = self.engine.submit(NewTask {
            project_id: chat.project_id.clone(),
            prompt: msg.text.clone(),
            agent: Some(chat.env.clone()),
            chat_id: Some(chat.id.clone()),
            quality: env.last_quality,
            swarm: env.last_swarm,
            start: Some(env.last_start),
            model: env.default_model,
        });
        match r {
            Ok(id) => {
                let title = lk(self.engine.store()).task(&id).map(|t| t.title).unwrap_or_default();
                self.line(&chat.id, keys::THIS_IS_TASK, &title, Some(ChatKind::Task), Some(&msg.id), Some(&id));
                Some(id)
            }
            Err(e) => {
                self.line(&chat.id, keys::TASK_NOT_CREATED, &e, Some(ChatKind::Error), Some(&msg.id), None);
                None
            }
        }
    }

    /// "This was a question" (G20.3): while the task has not reached the "Code" step it is cancelled and
    /// the message goes back to the model as a question; later it fails with `already_coding` (use `cancel`).
    pub fn not_a_task(self: &Arc<Self>, chat: &str, task_id: &str) -> Result<(), String> {
        self.running()?;
        let t = lk(self.engine.store()).task(task_id).ok_or("task_not_found")?;
        if t.chat_id.as_deref() != Some(chat) {
            return Err("task_not_in_chat".into());
        }
        self.engine.not_a_task(task_id)?;
        let card = self.find_msg(chat, |m| m.task_id.as_deref() == Some(task_id) && m.kind == Some(ChatKind::Task));
        let Some(msg_id) = card.and_then(|m| m.reply_to) else { return Ok(()) };
        let Some(msg) = self.find_msg(chat, |m| m.id == msg_id && m.who == Who::Human) else { return Ok(()) };
        self.line(chat, keys::BACK_TO_QUESTION, "", None, Some(&msg_id), Some(task_id));
        self.enqueue(chat, Job { msg, no_task: true });
        Ok(())
    }

    /// The terminal of the chat was opened: the feed says that edits made there do not stay (A07.2).
    pub fn terminal_opened(&self, chat: &str) -> Result<(), String> {
        self.chat(chat).ok_or("chat_not_found")?;
        self.line(chat, keys::TERMINAL_EDITS_LOST, "", Some(ChatKind::Warn), None, None);
        Ok(())
    }

    /// Repeats the answer to a message that got "not answered".
    pub fn retry(self: &Arc<Self>, chat: &str, msg_id: &str) -> Result<(), String> {
        self.running()?;
        self.chat(chat).ok_or("chat_not_found")?;
        let msg = self.find_msg(chat, |m| m.id == msg_id).filter(|m| m.who == Who::Human).ok_or("message_not_found")?;
        self.enqueue(chat, Job { msg, no_task: false });
        Ok(())
    }

    // ---- the list and the lives of chats ----

    /// Chats of the env: archived last, then the most recently moved first. A chat with open tasks is not
    /// archived. A swarm task without a chat in the store gets an item of its own (id = the chat's, else the task's).
    pub fn list(&self, env: &str) -> Vec<ChatSummary> {
        let now = self.now();
        let days = i64::from(self.settings().keep.archive_days);
        let (chats, tasks) = {
            let st = lk(self.engine.store());
            (st.chats(env), st.tasks().into_iter().filter(|t| t.agent == env).collect::<Vec<_>>())
        };
        let mut out = vec![];
        for mut c in chats {
            let mine: Vec<&Task> = tasks.iter().filter(|t| t.chat_id.as_deref() == Some(c.id.as_str())).collect();
            if !c.archived && days > 0 && now - c.updated_at > CDuration::days(days) && !mine.iter().any(|t| is_open(&t.status)) {
                c.archived = true;
                self.update_chat(&c.id, |x| x.archived = true);
            }
            out.push(self.summary(&c, &mine));
        }
        let known: Vec<String> = out.iter().map(|s| s.id.clone()).collect();
        for t in tasks.iter().filter(|t| t.swarm.is_some()) {
            let id = t.chat_id.clone().unwrap_or_else(|| t.id.clone());
            if known.contains(&id) {
                continue;
            }
            out.push(ChatSummary {
                id,
                env: env.to_string(),
                project_id: t.project_id.clone(),
                title: t.title.clone(),
                state: state_of(&t.status).unwrap_or(ChatState::Talk),
                task_id: Some(t.id.clone()),
                last: None,
                updated_at: t.finished_at.or(t.started_at).unwrap_or(t.created_at),
                archived: false,
                convoy: false,
                swarm: true,
                in_terminal: false,
            });
        }
        out.sort_by(|a, b| a.archived.cmp(&b.archived).then(b.updated_at.cmp(&a.updated_at)));
        out
    }

    fn summary(&self, c: &Chat, tasks: &[&Task]) -> ChatSummary {
        let top = tasks.iter().filter_map(|t| state_of(&t.status).map(|s| (s, *t))).min_by(|a, b| a.0.cmp(&b.0).then(b.1.created_at.cmp(&a.1.created_at)));
        let last = self.find_msg(&c.id, |m| m.who == Who::Model).map(|m| short(&m.text, 80));
        ChatSummary {
            id: c.id.clone(),
            env: c.env.clone(),
            project_id: c.project_id.clone(),
            title: c.title.clone(),
            state: top.map_or(ChatState::Talk, |x| x.0),
            task_id: top.map(|x| x.1.id.clone()),
            last,
            updated_at: c.updated_at,
            archived: c.archived,
            convoy: c.convoy,
            swarm: tasks.iter().any(|t| t.swarm.is_some()),
            in_terminal: c.terminal.is_some(),
        }
    }

    /// A page of the feed, oldest first: the last `limit` messages older than `before` (None = from the end).
    /// The window asks for 200; the next page is `before` = `at` of the first message it has.
    pub fn messages(&self, id: &str, before: Option<DateTime<Local>>, limit: usize) -> Vec<ChatMsg> {
        lk(self.engine.store()).read_chat(id, before, limit)
    }

    /// Deletes a chat with its feed and its copy of the project. Refused while a task of it goes on;
    /// finished tasks stay in the history of the project (G20.12).
    pub fn remove(&self, id: &str) -> Result<(), String> {
        let c = self.chat(id).ok_or("chat_not_found")?;
        if self.chat_tasks(id).iter().any(|t| is_open(&t.status)) {
            return Err("cancel_task_first".into());
        }
        let project = lk(self.engine.store()).project(&c.project_id);
        let dir = self.copy_dir(id);
        if dir.exists() {
            if let Some(p) = project {
                let _ = self.engine.rm_wt(&p.path, &dir, false);
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
        lk(self.engine.store()).remove_chat(id).map_err(|e| e.to_string())?;
        self.drop_attachments(id);
        lk(&self.waiting).retain(|w| w.chat != id);
        self.emit(ChatEvent::Removed(id.to_string()));
        Ok(())
    }

    /// The copy of the chat and the id of its session, for "open in terminal" (A07.2).
    pub fn terminal(&self, id: &str) -> Result<Terminal, String> {
        let c = self.chat(id).ok_or("chat_not_found")?;
        let p = lk(self.engine.store()).project(&c.project_id).ok_or("project_not_found")?;
        let a = self.engine.adapter(&c.env).ok_or_else(|| format!("{}: {}", reason::AGENT_NOT_FOUND, c.env))?;
        let copy = self.ensure_copy(&c, &p)?;
        let command = c.session.as_deref().and_then(|s| a.interactive_resume(s, &copy));
        Ok(Terminal { env: c.env, copy, session: c.session, command })
    }

    pub(crate) fn copy_dir(&self, chat: &str) -> PathBuf {
        let safe: String = chat.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        self.engine.worktrees_dir().join(".chats").join(safe)
    }

    /// The chat's copy of the project on a fresh base; whatever the last run left in it is thrown away.
    pub(crate) fn ensure_copy(&self, c: &Chat, p: &Project) -> Result<PathBuf, String> {
        let dir = self.copy_dir(&c.id);
        if dir.exists() {
            if self.engine.with_git(&dir, || git::refresh_readonly_copy(&dir, &p.base_branch)).is_ok() {
                return Ok(dir);
            }
            let _ = self.engine.rm_wt(&p.path, &dir, false);
            let _ = std::fs::remove_dir_all(&dir);
        }
        self.engine.add_ro(&p.path, &dir, &p.base_branch).map_err(|e| e.to_string())
    }

    // ---- model runs ----

    pub(crate) fn enqueue(self: &Arc<Self>, chat: &str, job: Job) {
        let start = {
            let mut q = lk(&self.queues);
            match q.get_mut(chat) {
                Some(v) => {
                    v.push_back(job);
                    false
                }
                None => {
                    q.insert(chat.to_string(), VecDeque::from([job]));
                    true
                }
            }
        };
        if start {
            let me = self.clone();
            let chat = chat.to_string();
            std::thread::spawn(move || me.drain(&chat));
        }
    }

    fn drain(self: &Arc<Self>, chat: &str) {
        loop {
            if self.stop.load(Ordering::SeqCst) {
                lk(&self.queues).remove(chat);
                return;
            }
            let job = lk(&self.queues).get_mut(chat).and_then(|v| v.pop_front());
            if let Some(job) = job {
                self.answer(chat, &job);
                continue;
            }
            // nothing in memory: the queue on disk; `send` holds the same lock while it decides to queue
            let _g = lk(&self.send_lock);
            if self.next_queued(chat) {
                continue;
            }
            let mut q = lk(&self.queues);
            if q.get(chat).is_none_or(|v| v.is_empty()) {
                q.remove(chat);
                return;
            }
        }
    }

    pub(crate) fn wait_until(&self, chat: &str, job: Job, until: DateTime<Local>) {
        let msg_id = job.msg.id.clone();
        lk(&self.waiting).push(Held { chat: chat.to_string(), job, until });
        self.update_chat(chat, |c| c.wait = Some(ChatWait { msg_id, resets_at: until }));
        self.queue_changed(chat);
    }

    // ---- watchers: the clock (answers held back by a limit) and the engine's events ----

    fn spawn_watchers(me: &Arc<Chats>) {
        let weak: Weak<Chats> = Arc::downgrade(me);
        let rx = me.engine.subscribe();
        let w = weak.clone();
        std::thread::spawn(move || loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(ev) => match w.upgrade() {
                    Some(c) if !c.stop.load(Ordering::SeqCst) => c.on_engine(ev),
                    _ => return,
                },
                Err(RecvTimeoutError::Timeout) => {
                    if w.upgrade().is_none_or(|c| c.stop.load(Ordering::SeqCst)) {
                        return;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => return,
            }
        });
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(40));
            let Some(c) = weak.upgrade() else { return };
            if c.stop.load(Ordering::SeqCst) {
                return;
            }
            c.release_due();
        });
    }

    /// Sends the answers that a limit held back and whose time has come by the clock of the engine.
    /// The chat's own timer calls it every few milliseconds; returns how many went.
    pub fn release_due(self: &Arc<Self>) -> usize {
        let now = self.now();
        let due: Vec<Held> = {
            let mut ws = lk(&self.waiting);
            let (due, rest): (Vec<Held>, Vec<Held>) = ws.drain(..).partition(|w| w.until <= now);
            *ws = rest;
            due
        };
        let n = due.len();
        for w in due {
            self.enqueue(&w.chat, w.job);
        }
        n
    }

    /// G05.2: the task landed (or was cancelled) before a note of it was delivered. Under each such message:
    /// "not passed: <why>", and the message is handled as an ordinary one of the chat.
    fn landed_with_notes(self: &Arc<Self>, t: &Task) {
        let why = match t.status {
            Status::Done | Status::DoneNotPushed | Status::Reverted => "task_already_merged",
            Status::Cancelled => "task_cancelled",
            _ => return,
        };
        let mine: Vec<PendingNote> = {
            let mut book = lk(&self.notes);
            let ids: Vec<String> = book.pending.iter().filter(|(_, n)| n.task == t.id).map(|(id, _)| id.clone()).collect();
            ids.iter().filter_map(|id| book.pending.remove(id)).collect()
        };
        for n in mine {
            self.line(&n.msg.chat_id, keys::NOTE_NOT_PASSED, why, Some(ChatKind::Warn), Some(&n.msg.id), Some(&t.id));
            if let Some(chat) = self.chat(&n.msg.chat_id) {
                self.route_ordinary(&chat, &n.msg);
            }
        }
    }

    fn on_engine(self: &Arc<Self>, ev: EngineEvent) {
        match ev {
            EngineEvent::TaskChanged(t) => {
                if let Some(c) = t.chat_id.clone() {
                    self.emit(ChatEvent::Changed(c.clone()));
                    self.kick(&c); // a queued message goes once the task is no longer on its way
                }
                self.landed_with_notes(&t);
            }
            EngineEvent::Log(task, e) => match e.note_id.clone() {
                Some(nid) => self.on_delivered(&nid, e.at),
                None => self.mirror(&task, &e),
            },
            EngineEvent::TextDelta { task, role, text } if speaks(role) => {
                if let Some(chat) = self.chat_of_task(&task) {
                    let msg_id = lk(&self.delta_msgs).entry((task, role)).or_insert_with(|| uuid::Uuid::new_v4().to_string()).clone();
                    self.emit(ChatEvent::Delta { chat, msg_id, text });
                }
            }
            _ => {}
        }
    }

    /// The engine says the note reached the executor (matched by the note's id).
    fn on_delivered(&self, note_id: &str, at: DateTime<Local>) {
        let n = {
            let mut book = lk(&self.notes);
            let n = book.pending.remove(note_id);
            if n.is_none() {
                book.early.insert(note_id.to_string(), at);
            }
            n
        };
        if let Some(n) = n {
            self.report_delivered(&n, at);
        }
    }

    fn chat_of_task(&self, task: &str) -> Option<ChatId> {
        if let Some(c) = lk(&self.task_chat).get(task) {
            return c.clone();
        }
        let c = lk(self.engine.store()).task(task).and_then(|t| t.chat_id);
        lk(&self.task_chat).insert(task.to_string(), c.clone());
        c
    }

    /// The roles' messages and the agents' actions of a task go into the feed of its chat, in the order they happen.
    fn mirror(&self, task: &str, e: &LogEvent) {
        let Some(role) = e.role else { return };
        let Some(chat) = self.chat_of_task(task) else { return };
        if let Some(a) = &e.action {
            return self.mirror_action(task, &chat, role, e, a);
        }
        let mut id = None;
        let (text, tool) = match e.kind {
            LogKind::Tool => {
                let (name, arg) = e.text.split_once(": ").unwrap_or((e.text.as_str(), ""));
                (String::new(), Some(ToolCall { name: name.to_string(), arg: arg.to_string(), result: String::new(), ok: true }))
            }
            LogKind::Agent if speaks(role) => {
                let text = e.text.trim().to_string();
                // the final text of a run repeats its last line
                if text.is_empty() || lk(&self.last_said).insert((task.to_string(), role), text.clone()).as_deref() == Some(text.as_str()) {
                    return;
                }
                id = lk(&self.delta_msgs).remove(&(task.to_string(), role)); // the streamed text ends in this message
                (text, None)
            }
            _ => return,
        };
        if self.chat(&chat).is_none() {
            return;
        }
        self.push(ChatMsg {
            id: id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            chat_id: chat,
            at: e.at,
            who: Who::Role(role),
            text,
            task_id: Some(task.to_string()),
            delivered_at: None,
            kind: None,
            tool,
            model: e.model.clone(),
            reply_to: None,
            action: None,
            attachments: vec![],
        });
    }

    /// An action of a role of the task: its start goes to the window only, its result is stored in the same message.
    fn mirror_action(&self, task: &str, chat: &str, role: RoleKind, e: &LogEvent, a: &ToolAction) {
        if self.chat(chat).is_none() {
            return;
        }
        let key = (task.to_string(), a.id.clone());
        if !a.done {
            let msg_id = lk(&self.act_msgs).entry(key).or_insert_with(|| uuid::Uuid::new_v4().to_string()).clone();
            return self.emit(ChatEvent::Action { chat: chat.to_string(), msg_id, action: a.clone() });
        }
        let msg_id = lk(&self.act_msgs).remove(&key).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        self.push(action_msg(msg_id, chat, e.at, Who::Role(role), Some(task), e.model.clone(), None, a));
    }
}

/// The stored form of an action's result in a feed: the action, and the v2 `tool` line for older readers.
#[allow(clippy::too_many_arguments)]
pub(crate) fn action_msg(id: String, chat: &str, at: DateTime<Local>, who: Who, task: Option<&str>, model: Option<String>, reply_to: Option<&str>, a: &ToolAction) -> ChatMsg {
    let line = crate::agent::actions::summary_line(a);
    let arg = line.split_once(": ").map(|(_, r)| r.to_string()).unwrap_or_default();
    ChatMsg {
        id,
        chat_id: chat.to_string(),
        at,
        who,
        text: String::new(),
        task_id: task.map(str::to_string),
        delivered_at: None,
        kind: None,
        tool: Some(ToolCall { name: a.name.clone(), arg, result: a.output.clone().unwrap_or_default(), ok: a.ok }),
        model,
        reply_to: reply_to.map(str::to_string),
        action: Some(a.clone()),
        attachments: vec![],
    }
}

/// The text the model gets for the message (data in a fenced block, G14.4).
pub(crate) fn message_prompt(text: &str, recap: Option<&str>) -> String {
    let mut p = String::new();
    if let Some(r) = recap {
        p.push_str("The earlier session is gone. Here is the recent conversation, for context:\n");
        p.push_str(&prompt::fenced("HISTORY", r));
        p.push_str("\n\n");
    }
    p.push_str("Message of the person (answer it; do not change any file):\n");
    p.push_str(&prompt::fenced("MESSAGE", text));
    p
}
