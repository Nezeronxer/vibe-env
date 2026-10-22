//! The app's state without a window: the engine, the chats and the kits, and the commands as plain
//! functions over them (seam 1 of the spec). The `#[tauri::command]` wrappers in `main.rs` only call these.
use nightshift_core::agent::{self, Adapter};
use nightshift_core::chat::{ChatTarget, Chats};
use nightshift_core::engine::{Clock, Engine};
use nightshift_core::kits::{ImportPlan, Kits};
use nightshift_core::model::{ChatKind, ChatMsg, Project, Task};
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
        let chats = Chats::new(engine.clone());
        Arc::new(Hub { engine, chats, kits, dir, adapters: Mutex::new(adapters), plans: Mutex::new(vec![]), envs: Mutex::new(vec![]), cards: Mutex::new(Default::default()), settings_hooks: Mutex::new(vec![]) })
    }

    /// Starts the scheduler. The cards of convoys that are already on disk count as delivered.
    pub fn on_settings(&self, f: Box<dyn Fn(&nightshift_core::model::Settings) + Send + Sync>) {
        lk(&self.settings_hooks).push(f);
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

    pub fn submit_task(&self, project_id: &str, prompt: &str, agent: Option<String>) -> Res<String> {
        self.project_ready(project_id)?;
        self.engine.submit(nightshift_core::engine::NewTask { project_id: project_id.into(), prompt: prompt.into(), agent, ..Default::default() })
    }

    pub fn submit_convoy(&self, project_id: &str, env: &str, text: &str) -> Res<String> {
        self.project_ready(project_id)?;
        self.engine.submit_convoy(project_id, env, text)
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
}
