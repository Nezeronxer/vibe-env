//! The app's state without a window: the engine, the chats and the kits, and the commands as plain
//! functions over them (seam 1 of the spec). The `#[tauri::command]` wrappers in `main.rs` only call these.
use nightshift_core::agent::{self, Adapter};
use nightshift_core::chat::{ChatTarget, Chats};
use nightshift_core::engine::{Clock, Engine};
use nightshift_core::kits::{ImportPlan, Kits};
use nightshift_core::model::{ChatKind, ChatMsg, Project, Task, ToolAction};
use nightshift_core::notify::Notifier;
use nightshift_core::store::Store;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

pub type Res<T> = Result<T, String>;

pub fn lk<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
pub fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// What the app does after settings were saved.
pub(crate) type SettingsHook = Box<dyn Fn(&nightshift_core::model::Settings) + Send + Sync>;

pub struct Hub {
    pub engine: Arc<Engine>,
    pub chats: Arc<Chats>,
    pub kits: Arc<Kits>,
    /// The app's data folder (`app_data_dir()`); copies of tasks and chats live in `<dir>/worktrees`.
    pub dir: PathBuf,
    /// The same list the engine got: `list_envs` reads the envs from here.
    pub(crate) adapters: Mutex<Vec<Arc<dyn Adapter>>>,
    /// Previews of kits waiting for "Install": the window sends back only the id, never a plan we would trust.
    pub(crate) plans: Mutex<Vec<ImportPlan>>,
    pub(crate) envs: Mutex<Vec<crate::envs::EnvInfo>>,
    /// Ids of the convoy cards the window already has.
    pub(crate) cards: Mutex<std::collections::HashSet<String>>,
    /// What the app does after settings were saved (the notifier's copy, the language, the tray): set by `main`.
    pub(crate) settings_hooks: Mutex<Vec<SettingsHook>>,
}

impl Hub {
    /// Opens the engine over `store`, the chats over the engine, and binds the kit secrets to the runs (R13).
    pub fn new(store: Store, adapters: Vec<Arc<dyn Adapter>>, notifier: Arc<dyn Notifier>, kits: Kits, dir: PathBuf, clock: Clock) -> Arc<Hub> {
        let engine = Engine::new_with_clock(store, adapters.clone(), notifier, dir.join("worktrees"), clock);
        let kits = Arc::new(kits);
        let k = kits.clone();
        engine.set_env_vars(Arc::new(move |env| k.env_vars(env)));
        // F15: "Проверить как пользователь" shoots with node + playwright when the machine has them
        engine.set_proof(Arc::new(nightshift_core::preview::NodeCapture::default()), dir.join("proof"));
        let chats = Chats::new(engine.clone());
        Arc::new(Hub { engine, chats, kits, dir, adapters: Mutex::new(adapters), plans: Mutex::new(vec![]), envs: Mutex::new(vec![]), cards: Mutex::new(Default::default()), settings_hooks: Mutex::new(vec![]) })
    }

    /// Starts the scheduler. The cards of convoys that are already on disk count as delivered.
    pub fn on_settings(&self, f: Box<dyn Fn(&nightshift_core::model::Settings) + Send + Sync>) {
        lk(&self.settings_hooks).push(f);
    }

    /// F15: the proof shots of a task are in `<this>/<task id>/`.
    pub fn proof_dir(&self) -> PathBuf {
        self.dir.join("proof")
    }

    pub fn start(&self) {
        self.deliver_cards(&mut |_| {});
        self.engine.start();
    }

    /// The convoy's task cards are written into the chat without an event (task 10): each new one goes to the window once.
    pub fn deliver_cards(&self, emit: &mut dyn FnMut(&ChatMsg)) {
        let envs: Vec<String> = lk(&self.adapters).iter().map(|a| a.id().to_string()).collect();
        let st = lk(self.engine.store());
        let mut seen = lk(&self.cards);
        for chat in envs.iter().flat_map(|e| st.chats(e)).filter(|c| c.convoy) {
            for m in st.read_chat(&chat.id, None, 500).iter().filter(|m| m.kind == Some(ChatKind::Task)) {
                if seen.insert(m.id.clone()) {
                    emit(m);
                }
            }
        }
    }

    pub fn settings(&self) -> nightshift_core::model::Settings {
        lk(self.engine.store()).settings()
    }

    pub fn project(&self, id: &str) -> Res<Project> {
        lk(self.engine.store()).project(&id.to_string()).ok_or_else(|| format!("project_not_found: {id}"))
    }

    /// The project, but only when its folder is still there: "папка не найдена: <путь>" (R11.10).
    pub fn project_ready(&self, id: &str) -> Res<Project> {
        let p = self.project(id)?;
        if p.path.is_dir() {
            Ok(p)
        } else {
            Err(format!("folder_not_found: {}", p.path.display()))
        }
    }

    pub fn set_adapters(&self, list: Vec<Arc<dyn Adapter>>) {
        *lk(&self.adapters) = list.clone();
        self.engine.set_adapters(list);
    }

    /// The adapters of the settings (own commands, CLI paths), for after `set_settings`.
    pub fn reload_adapters(&self) {
        self.set_adapters(agent::adapters(&self.settings()));
    }

    pub fn adapter(&self, env: &str) -> Option<Arc<dyn Adapter>> {
        lk(&self.adapters).iter().find(|a| a.id() == env).cloned()
    }

    /// A task as the window reads it: the core's own form plus `swarm_view` for a swarm (task 10).
    pub fn task_view(&self, t: &Task) -> Value {
        let mut v = serde_json::to_value(t).unwrap_or(Value::Null);
        if t.swarm.is_some() {
            if let (Some(view), Some(o)) = (self.engine.swarm_view(&t.id), v.as_object_mut()) {
                o.insert("swarm_view".into(), json!(view));
            }
        }
        v
    }

    pub fn list_tasks(&self) -> Vec<Value> {
        let tasks = lk(self.engine.store()).tasks();
        tasks.iter().map(|t| self.task_view(t)).collect()
    }

    // ---- chats ----

    /// A message of the person: to a chat, or the first one of a new chat (that needs the project's folder).
    pub fn send_chat(&self, target: ChatTarget, msg_id: &str, text: &str) -> Res<String> {
        if let ChatTarget::New { project_id, .. } = &target {
            self.project_ready(project_id)?;
        }
        self.chats.send(target, msg_id, text)
    }

    /// G84: what the window takes from the env's settings for a project (Claude Code: `spinnerVerbs`); the person's
    /// own settings only when there is no project.
    pub fn ui_hints(&self, env: &str, project_id: Option<&str>) -> Value {
        let cwd = project_id.and_then(|id| self.project(id).ok()).map(|p| p.path).unwrap_or_default();
        json!(self.adapter(env).map(|a| a.ui_hints(&cwd)).unwrap_or_default())
    }

    /// A page of a task's log (`Engine::task_log`); `before` is a time as the window got it (RFC 3339).
    pub fn task_log(&self, id: &str, before: Option<&str>, limit: usize) -> Res<Vec<nightshift_core::model::LogEvent>> {
        let before = match before {
            Some(b) => Some(serde_json::from_value(Value::String(b.to_string())).map_err(|_| format!("bad_time: {b}"))?),
            None => None,
        };
        Ok(self.engine.task_log(id, before, limit))
    }

    pub fn submit_task(&self, project_id: &str, prompt: &str, agent: Option<String>) -> Res<String> {
        self.project_ready(project_id)?;
        self.engine.submit(nightshift_core::engine::NewTask { project_id: project_id.into(), prompt: prompt.into(), agent, ..Default::default() })
    }

    pub fn submit_convoy(&self, project_id: &str, env: &str, text: &str) -> Res<String> {
        self.project_ready(project_id)?;
        self.engine.submit_convoy(project_id, env, text)
    }
}

/// The window gets an agent's actions and streamed text not more often than once per `GAP` for each task and chat
/// (story 227: a fast command must not stall the window). What comes in between waits and goes out merged:
/// the newest state of each action, the pieces of text of one message joined.
/// Sends one event to the window: its name and payload.
pub type Emit = Arc<dyn Fn(&str, Value) + Send + Sync>;

pub struct Coalescer {
    slots: Arc<Mutex<std::collections::HashMap<String, Slot>>>,
    emit: Emit,
}

pub const GAP: std::time::Duration = std::time::Duration::from_millis(50);

#[derive(Default)]
struct Slot {
    last: Option<std::time::Instant>,
    items: Vec<WinItem>,
}

/// One event for the window: `task-action {id, action}`, `chat-action {id, msgId, action}`, `chat-delta {id, msgId, text}`,
/// v6 `chat-thinking {id, msgId, text}`.
#[derive(Clone, Debug)]
pub enum WinItem {
    TaskAction { task: String, action: ToolAction },
    ChatAction { chat: String, msg_id: String, action: ToolAction },
    ChatDelta { chat: String, msg_id: String, text: String },
    ChatThinking { chat: String, msg_id: String, text: String },
}

impl WinItem {
    /// The task or chat the item belongs to: one slot each.
    pub fn key(&self) -> String {
        match self {
            WinItem::TaskAction { task, .. } => format!("task:{task}"),
            WinItem::ChatAction { chat, .. } | WinItem::ChatDelta { chat, .. } | WinItem::ChatThinking { chat, .. } => format!("chat:{chat}"),
        }
    }
    fn send(self, emit: &dyn Fn(&str, Value)) {
        match self {
            WinItem::TaskAction { task, action } => emit("task-action", json!({ "id": task, "action": action })),
            WinItem::ChatAction { chat, msg_id, action } => emit("chat-action", json!({ "id": chat, "msgId": msg_id, "action": action })),
            WinItem::ChatDelta { chat, msg_id, text } => emit("chat-delta", json!({ "id": chat, "msgId": msg_id, "text": text })),
            WinItem::ChatThinking { chat, msg_id, text } => emit("chat-thinking", json!({ "id": chat, "msgId": msg_id, "text": text })),
        }
    }
}

/// Puts `it` into the waiting items: the newer state of the same action replaces the older; a piece of text joins the
/// waiting text of the same message and the same kind (answer or thought, v6), wherever it is in the list.
fn merge(items: &mut Vec<WinItem>, it: WinItem) {
    use WinItem::*;
    for old in items.iter_mut() {
        match (old, &it) {
            (TaskAction { action: a, .. }, TaskAction { action: b, .. }) if a.id == b.id => {
                *a = b.clone();
                return;
            }
            (ChatAction { msg_id: m, action: a, .. }, ChatAction { msg_id: n, action: b, .. }) if m == n && a.id == b.id => {
                *a = b.clone();
                return;
            }
            (ChatDelta { msg_id: m, text, .. }, ChatDelta { msg_id: n, text: more, .. }) | (ChatThinking { msg_id: m, text, .. }, ChatThinking { msg_id: n, text: more, .. })
                if m == n =>
            {
                text.push_str(more);
                return;
            }
            _ => {}
        }
    }
    items.push(it);
}

impl Coalescer {
    /// `emit(event name, payload)` sends to the window. A thread sends what waited once its `GAP` is over;
    /// it ends with the last handle to the coalescer.
    pub fn new(emit: Emit) -> Coalescer {
        let slots: Arc<Mutex<std::collections::HashMap<String, Slot>>> = Arc::default();
        let (weak, e) = (Arc::downgrade(&slots), emit.clone());
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(5));
            let Some(slots) = weak.upgrade() else { return };
            let now = std::time::Instant::now();
            let due: Vec<WinItem> = lk(&slots)
                .values_mut()
                .filter(|s| !s.items.is_empty() && s.last.is_none_or(|l| now.duration_since(l) >= GAP))
                .flat_map(|s| {
                    s.last = Some(now);
                    std::mem::take(&mut s.items)
                })
                .collect();
            drop(slots);
            for it in due {
                it.send(&*e);
            }
        });
        Coalescer { slots, emit }
    }

    pub fn push(&self, it: WinItem) {
        let now = std::time::Instant::now();
        let send_now = {
            let mut slots = lk(&self.slots);
            let s = slots.entry(it.key()).or_default();
            if s.items.is_empty() && s.last.is_none_or(|l| now.duration_since(l) >= GAP) {
                s.last = Some(now);
                Some(it)
            } else {
                merge(&mut s.items, it);
                None
            }
        };
        if let Some(it) = send_now {
            it.send(&*self.emit);
        }
    }

    /// Sends what waits for `key` now: before another event of the same task or chat, so the window sees them in order.
    pub fn flush(&self, key: &str) {
        let items = match lk(&self.slots).get_mut(key) {
            Some(s) if !s.items.is_empty() => {
                s.last = Some(std::time::Instant::now());
                std::mem::take(&mut s.items)
            }
            _ => return,
        };
        for it in items {
            it.send(&*self.emit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::*;
    use nightshift_core::agent::{FakeAdapter, FakeStep};
    use nightshift_core::model::{RoleKind, Who};

    #[test]
    fn a_new_chat_gets_the_answer_of_the_model_and_a_moved_folder_stops_the_next_one() {
        let r = Rig::new(FakeAdapter::new(vec![]).with_role(RoleKind::Chat, vec![vec![FakeStep::Done("pong".into())]]));
        let target = ChatTarget::New { env: "fake".into(), project_id: r.project.id.clone() };
        let chat = r.hub.send_chat(target.clone(), "m1", "ping").unwrap();
        until("the answer", || r.hub.chats.messages(&chat, None, 50).iter().any(|m| m.who == Who::Model && m.text == "pong"));
        assert_eq!(r.hub.chats.list("fake").len(), 1);

        std::fs::rename(&r.repo, r.s.0.join("moved")).unwrap();
        let err = r.hub.send_chat(target, "m2", "again").unwrap_err();
        assert_eq!(err, format!("folder_not_found: {}", r.repo.display()));
        assert_eq!(r.hub.chats.list("fake").len(), 1, "no chat was made");
    }

    #[test]
    fn five_hundred_lines_of_output_in_100_ms_give_at_most_three_window_events_and_the_last_has_them_all() {
        use nightshift_core::model::ActionInput;
        let sent: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
        let s2 = sent.clone();
        let c = Coalescer::new(Arc::new(move |n: &str, v: Value| lk(&s2).push((n.to_string(), v))));
        let start = std::time::Instant::now();
        let mut out = String::new();
        for i in 0..500u32 {
            out.push_str(&format!("line {i}\n"));
            let action = ToolAction { id: "t1".into(), name: "Bash".into(), input: ActionInput::Bash { command: "make".into() }, output: Some(out.clone()), ok: true, done: false };
            c.push(WinItem::TaskAction { task: "task-1".into(), action });
            // evenly over 95 ms
            let at = start + std::time::Duration::from_micros(190 * u64::from(i + 1));
            if let Some(d) = at.checked_duration_since(std::time::Instant::now()) {
                std::thread::sleep(d);
            }
        }
        assert!(start.elapsed() < std::time::Duration::from_millis(100), "the lines took {:?}", start.elapsed());
        std::thread::sleep(GAP * 3);
        let got = lk(&sent).clone();
        assert!(!got.is_empty() && got.len() <= 3, "{} events", got.len());
        assert!(got.iter().all(|(n, v)| n == "task-action" && v["id"] == "task-1"));
        assert_eq!(got.last().unwrap().1["action"]["output"].as_str(), Some(out.as_str()), "nothing is lost: the newest state goes last");
        // pieces of one message are joined, not dropped
        lk(&sent).clear();
        for t in ["a", "b", "c"] {
            c.push(WinItem::ChatDelta { chat: "c1".into(), msg_id: "m1".into(), text: t.into() });
        }
        std::thread::sleep(GAP * 3);
        let texts: String = lk(&sent).iter().map(|(_, v)| v["text"].as_str().unwrap_or("").to_string()).collect();
        assert_eq!(texts, "abc");
        assert!(lk(&sent).iter().all(|(n, v)| n == "chat-delta" && v["msgId"] == "m1"));
    }

    /// G80.1/G81: pieces of the answer and of the thoughts of one message come in turns; each kind is joined with
    /// its own kind of the same message, so the window gets one text event and one thought event, nothing mixed.
    #[test]
    fn thoughts_and_text_of_one_message_are_joined_each_by_its_kind() {
        let sent: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
        let s2 = sent.clone();
        let c = Coalescer::new(Arc::new(move |n: &str, v: Value| lk(&s2).push((n.to_string(), v))));
        c.push(WinItem::ChatThinking { chat: "c1".into(), msg_id: "m0".into(), text: "warm ".into() }); // goes at once: the slot was idle
        for (k, t) in [("t", "I "), ("d", "Hel"), ("t", "think"), ("d", "lo"), ("t", ".")] {
            let (chat, msg_id, text) = ("c1".to_string(), "m1".to_string(), t.to_string());
            c.push(if k == "t" { WinItem::ChatThinking { chat, msg_id, text } } else { WinItem::ChatDelta { chat, msg_id, text } });
        }
        std::thread::sleep(GAP * 3);
        let got: Vec<(String, String, String)> = lk(&sent).iter().map(|(n, v)| (n.clone(), v["msgId"].as_str().unwrap_or("").into(), v["text"].as_str().unwrap_or("").into())).collect();
        assert_eq!(got, [
            ("chat-thinking".to_string(), "m0".to_string(), "warm ".to_string()),
            ("chat-thinking".into(), "m1".into(), "I think.".into()),
            ("chat-delta".into(), "m1".into(), "Hello".into()),
        ]);
    }
}
