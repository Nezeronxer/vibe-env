//! Window, tray, keys, notifications and the OS around the core: the commands are thin wrappers over `Hub`
//! (see `hub.rs` and its neighbours, where the logic and its tests live).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window next to the app on Windows
mod agentcmds;
mod auto;
mod claudetrust;
mod config;
mod custom;
mod diag;
mod envs;
mod gitcmds;
mod hub;
mod kitcmds;
mod market;
mod notifier;
mod open;
mod ops;
mod pathfix;
mod presence;
mod preview;
mod proj;
mod ptycmds;
mod sessions;
mod settings_check;
mod shellcmds;
mod startup;
mod term;
#[cfg(test)]
mod testkit;
mod tray;
mod updates;
mod windows;

use hub::{e2s, lk, Hub, Res};
use kitcmds::KRes;
use nightshift_core::agent;
use nightshift_core::chat::{ChatEvent, ChatTarget};
use nightshift_core::engine::EngineEvent;
use nightshift_core::kits::{Choices, ExportDest, HeldRef, Installed, KitSource, Kits, Scope, Selection};
use nightshift_core::model::{ModelChoice, OutwardChoice, Project, Settings};
use nightshift_core::notify::Notifier;
use notifier::{AppNotifier, Shown, Sink};
use presence::KeepAwake;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Emitter, Manager, State, Wry};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_notification::NotificationExt;

#[derive(Clone)]
struct Tray {
    icon: TrayIcon,
    freeze: MenuItem<Wry>,
    /// Disabled first menu item: the only place the counter is visible on Linux.
    status: MenuItem<Wry>,
    /// Disabled item "Сессии: N": the PTY sessions that live without a window (story 301).
    sessions: MenuItem<Wry>,
}

/// Lines the window must get after it has loaded (it may load later than they happen): name -> payload.
type Startup = Arc<Mutex<Vec<(String, Value)>>>;

struct App {
    hub: Arc<Hub>,
    tray: Mutex<Option<Tray>>,
    /// Serializes `set_settings`, so only one key change / adapter swap is in flight.
    settings_lock: Mutex<()>,
    /// UI language, recomputed at start and when settings change (not on every event).
    ru: Arc<AtomicBool>,
    /// The notification settings the notifier reads (it sits below the engine, before the store is reachable).
    notify_cfg: Arc<Mutex<nightshift_core::model::NotifySettings>>,
    keep: KeepAwake,
    /// 0 unknown, 1 there is a tray host, 2 there is none.
    host: AtomicU8,
    startup: Startup,
    /// `--new-chat` at the first start: the window gets `focus-input` once it is up.
    first_new_chat: AtomicBool,
    /// git answered `--version` once: asked again only while it did not (the «Нужен git» screen checks again)
    git_ok: AtomicBool,
    /// The data folder and the copy of the unreadable state file, when the file could not be read at start (G16.4).
    unreadable: Option<(PathBuf, String)>,
    fresh: startup::FreshGate,
}

fn hub(s: &State<App>) -> Arc<Hub> {
    s.hub.clone()
}

fn note_startup(list: &Startup, name: &str, payload: Value) {
    let mut v = lk(list);
    v.retain(|(n, p)| !(n == name && (name != "kits-recovered" || *p == payload)));
    v.push((name.to_string(), payload));
}

// ---------- language ----------

fn is_ru(s: &Settings) -> bool {
    match s.language.as_str() {
        "ru" => true,
        "auto" => {
            let env = std::env::var("LC_ALL").or_else(|_| std::env::var("LANG")).unwrap_or_default();
            let mac = if cfg!(target_os = "macos") {
                nightshift_core::proc::cmd("defaults").args(["read", "-g", "AppleLanguages"]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default()
            } else {
                String::new()
            };
            env.starts_with("ru") || mac.contains("\"ru")
        }
        _ => false,
    }
}

// ---------- tray counter, window title ----------

fn refresh_tray(app: &AppHandle) {
    // events can arrive before setup has registered the state
    let Some(st) = app.try_state::<App>() else { return };
    let tasks = lk(st.hub.engine.store()).tasks();
    let (running, waiting) = tray::counts(&tasks);
    let label = tray::label(running, waiting);
    let ru = st.ru.load(Ordering::Relaxed);
    let frozen = st.hub.engine.is_frozen();
    // The setters wait for the main thread: the lock is let go first, or a refresh on the main thread (the tray's
    // "Freeze", the global key) would wait for the lock a refresh on another thread holds while it waits for the main thread.
    let tray = lk(&st.tray).clone();
    if let Some(t) = tray.as_ref() {
        if cfg!(target_os = "macos") {
            let _ = t.icon.set_title(if label.is_empty() { None } else { Some(label.as_str()) });
        }
        let tip = if label.is_empty() { "Nightshift".to_string() } else { format!("Nightshift: {label}") };
        let _ = t.icon.set_tooltip(Some(tip));
        let idle = if ru { "Ничего не идёт" } else { "Idle" };
        let _ = t.status.set_text(if label.is_empty() { idle } else { label.as_str() });
        let text = match (frozen, ru) {
            (true, true) => "Разморозить",
            (true, false) => "Unfreeze",
            (false, true) => "Заморозить всё",
            (false, false) => "Freeze all",
        };
        let _ = t.freeze.set_text(text);
        let live = app.try_state::<ptycmds::Pty>().map(|p| p.live()).unwrap_or(0);
        let _ = t.sessions.set_text(format!("{} {live}", if ru { "Сессии:" } else { "Sessions:" }));
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_title(&if waiting > 0 { format!("Nightshift ({waiting})") } else { "Nightshift".into() });
    }
}

fn sync_keep_awake(app: &AppHandle) {
    if let Some(st) = app.try_state::<App>() {
        st.keep.update(st.hub.keep_awake_wanted());
    }
}

/// What the window learns about the tray and the global key (R10.1, R10.2).
fn tray_state(st: &App) -> Value {
    json!({ "available": st.host.load(Ordering::Relaxed) == 1, "hotkey": tray::hotkey_available(), "wayland": !tray::hotkey_available() })
}

fn check_tray_host(app: &AppHandle) {
    let Some(st) = app.try_state::<App>() else { return };
    let now = match tray::tray_host() {
        Some(true) => 1,
        _ => 2,
    };
    if st.host.swap(now, Ordering::Relaxed) != now {
        let _ = app.emit("tray-state", tray_state(&st));
    }
}

// ---------- window ----------

fn show_window(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn new_chat(app: &AppHandle) {
    show_window(app);
    let _ = app.emit("focus-input", ());
}

/// The terminals of the last run: folders and text, restored by the window (story 253).
const TERMS_FILE: &str = "terms.json";

/// The terminals saved at the last quit, to reopen; empty when the data folder could not be read. The snapshot holds
/// terminal output, so it is removed in either case.
fn restored_terms(dir: &std::path::Path, readable: bool) -> Vec<nightshift_core::term::TermSaved> {
    let saved = ptycmds::load_saved(&dir.join(TERMS_FILE));
    if readable { saved } else { vec![] }
}

/// The quit question is shown once: ⌘Q or "Выйти" again while it is open adds no second dialog (story 303).
struct QuitGate(AtomicBool);

impl QuitGate {
    const fn new() -> Self {
        QuitGate(AtomicBool::new(false))
    }

    /// Runs `ask` unless a question is already open (`None`). After "Stay" the gate opens again; after "Quit" it
    /// stays shut, the app is on its way out.
    fn ask(&self, ask: impl FnOnce() -> bool) -> Option<bool> {
        if self.0.swap(true, Ordering::SeqCst) {
            return None;
        }
        let yes = ask();
        if !yes {
            self.0.store(false, Ordering::SeqCst);
        }
        Some(yes)
    }
}

static QUIT_GATE: QuitGate = QuitGate::new();

/// "Выход": with work going on the person is asked first; then the runs are stopped, the state stays on disk (R08.2).
fn quit(app: &AppHandle) {
    let app = app.clone();
    // the dialog and the shutdown wait, so they stay off the UI thread
    std::thread::spawn(move || {
        let st = app.state::<App>();
        let tasks = lk(st.hub.engine.store()).tasks();
        let ru = st.ru.load(Ordering::Relaxed);
        let pty = app.state::<ptycmds::Pty>();
        if let Some(text) = tray::quit_prompt(tray::counts(&tasks).0, pty.live(), ru) {
            let (yes, no) = if ru { ("Выйти", "Остаться") } else { ("Quit", "Stay") };
            let shown = QUIT_GATE.ask(|| app.dialog().message(text).title("Nightshift").buttons(MessageDialogButtons::OkCancelCustom(yes.into(), no.into())).blocking_show());
            if shown != Some(true) {
                return;
            }
        }
        shut_down(&app);
        app.exit(0);
    });
}

static SHUT_DOWN: AtomicBool = AtomicBool::new(false);

/// The terminals are saved and the runs stopped, once. `quit` comes here, and so does the end of the process by another
/// way (Dock "Quit", log-out, shutdown on macOS), which never passes `quit`: otherwise the tabs would not come back and
/// the agents' processes would be left running.
fn shut_down(app: &AppHandle) {
    if SHUT_DOWN.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some(st) = app.try_state::<App>() else { return };
    app.state::<ptycmds::Pty>().save_and_end(&st.hub.dir.join(TERMS_FILE));
    st.hub.chats.shutdown();
    st.hub.engine.shutdown();
    st.keep.update(false);
}

// ---------- notifications ----------

/// notification id -> task, so a click can open the task (the last 200 are kept)
static NOTES: Mutex<Vec<(i32, String)>> = Mutex::new(Vec::new());

struct SystemSink(AppHandle);
impl Sink for SystemSink {
    fn show(&self, s: &Shown) -> Result<(), String> {
        {
            let mut v = lk(&NOTES);
            v.retain(|(id, _)| *id != s.id);
            v.push((s.id, s.task_id.clone()));
            if v.len() > 200 {
                v.remove(0);
            }
        }
        // `show` returns Ok whatever the system does with it (the plugin sends from another thread): without a server to
        // take it the notification is lost, and the window must be told how to switch it on
        if tray::notification_server() == Some(false) {
            return Err("no_notification_server".into());
        }
        let mut b = self.0.notification().builder().id(s.id).title(&s.title).body(&s.body);
        if let Some(sound) = &s.sound {
            b = b.sound(sound);
        }
        b.show().map_err(e2s)
    }
}

/// Click on a system notification: show the window and, if the platform says which one, open its task.
fn on_notification_click(app: &AppHandle, a: &tauri_plugin_notification::ActionPerformed) {
    show_window(app);
    // a terminal agent's notification names its pane; a task's names its task
    if let Some(pane) = a.notification().and_then(|n| agentcmds::pane_of(n.id())) {
        let _ = app.emit("agent-notify-click", json!({ "pane": pane }));
        return;
    }
    let task = a.notification().and_then(|n| lk(&NOTES).iter().find(|(id, _)| *id == n.id()).map(|(_, t)| t.clone()));
    if let Some(t) = task {
        let _ = app.emit("open-task", t);
    }
}

// ---------- events from the core to the window ----------

fn forward_events(app: &AppHandle, hub: &Arc<Hub>, notifier: &Arc<AppNotifier>) {
    let rx = hub.engine.subscribe();
    let (h, hub2, n) = (app.clone(), hub.clone(), notifier.clone());
    // actions and streamed text go out merged, at most once per 50 ms per task / chat (story 227)
    let he = app.clone();
    let win = Arc::new(hub::Coalescer::new(Arc::new(move |name: &str, payload: Value| {
        let _ = he.emit(name, payload);
    })));
    let w = win.clone();
    std::thread::spawn(move || {
        for ev in rx {
            let changed = match ev {
                EngineEvent::TaskChanged(t) => {
                    let _ = h.emit("task-changed", hub2.task_view(&t));
                    if !envs::waits_for_human(&t) {
                        n.forget(&t.id);
                    }
                    if t.convoy_id.is_some() {
                        hub2.deliver_cards(&mut |m| {
                            let _ = h.emit("chat-msg", m);
                        });
                    }
                    true
                }
                EngineEvent::Log(id, ev) => {
                    match ev.action {
                        Some(action) => w.push(hub::WinItem::TaskAction { task: id, action }),
                        None => {
                            w.flush(&format!("task:{id}"));
                            let _ = h.emit("task-log", json!({ "id": id, "event": ev }));
                        }
                    }
                    false // log lines do not change the counter
                }
                // a task's text is streamed to its chat (`chat-delta`), the task's own feed gets the whole line
                EngineEvent::TextDelta { .. } => false,
                EngineEvent::ProjectChanged(id) => h.emit("project-changed", id).is_ok(),
                EngineEvent::LimitsChanged(env) => {
                    let _ = h.emit("limits-changed", env);
                    false
                }
                EngineEvent::CatalogChanged(env) => {
                    let _ = h.emit("catalog-changed", env);
                    false
                }
                EngineEvent::Frozen(on) => {
                    let _ = h.emit("frozen", on);
                    true
                }
            };
            if changed {
                refresh_tray(&h);
                sync_keep_awake(&h);
            }
        }
    });
    let (h, rx) = (app.clone(), hub.chats.subscribe());
    std::thread::spawn(move || {
        for ev in rx {
            match ev {
                // the stored result of an action: the window has its start as `chat-action` already
                ChatEvent::Msg(m) if m.action.is_some() => {
                    let action = m.action.clone().unwrap_or_else(|| unreachable!());
                    win.push(hub::WinItem::ChatAction { chat: m.chat_id.clone(), msg_id: m.id.clone(), action });
                }
                // the list of chats is read again by the window when a message of an unknown chat arrives
                ChatEvent::Msg(m) => {
                    win.flush(&format!("chat:{}", m.chat_id));
                    let _ = h.emit("chat-msg", &m);
                }
                ChatEvent::Action { chat, msg_id, action } => win.push(hub::WinItem::ChatAction { chat, msg_id, action }),
                ChatEvent::Delta { chat, msg_id, text } => win.push(hub::WinItem::ChatDelta { chat, msg_id, text }),
                // v6 (task 27): thoughts go merged like text; the queue of a chat goes whole at once
                ChatEvent::Thinking { chat, msg_id, text } => win.push(hub::WinItem::ChatThinking { chat, msg_id, text }),
                ChatEvent::Queue { chat, queue, wait } => {
                    let _ = h.emit("chat-queue", json!({ "id": chat, "queue": queue, "wait": wait }));
                }
                ChatEvent::Changed(_) | ChatEvent::Removed(_) => {}
            }
        }
    });
}

/// Everything that runs on a timer: envs again every 5 minutes (G17.3), the models once a day (G18.1), the chat
/// answers that waited for a limit, the cards of a convoy, the person's presence and the hold on sleep.
fn start_timers(app: &AppHandle, hub: &Arc<Hub>) {
    let (h, hb) = (app.clone(), hub.clone());
    std::thread::spawn(move || {
        let mut tick = 0u64;
        loop {
            if tick.is_multiple_of(10) {
                // every 5 minutes (tick = 30 s)
                hb.recheck_envs();
                check_tray_host(&h);
                if tick == 0 {
                    // the envs are known now: what one CLI has goes into the others (Settings.spread_kits)
                    hb.spread_at_start();
                }
            }
            if tick.is_multiple_of(2880) {
                // at start and then once a day; the CLIs answer slowly, so not on this thread
                let hc = hb.clone();
                std::thread::spawn(move || {
                    for e in hc.list_envs().iter().filter(|e| e["installed"] == true).filter_map(|e| e["id"].as_str().map(String::from)) {
                        let _ = hc.engine.refresh_catalog(&e);
                    }
                });
            }
            hb.chats.release_due();
            hb.deliver_cards(&mut |m| {
                let _ = h.emit("chat-msg", m);
            });
            tick += 1;
            std::thread::sleep(Duration::from_secs(30));
        }
    });
    let (h, hb) = (app.clone(), hub.clone());
    std::thread::spawn(move || loop {
        let (idle, locked) = presence::probe(hb.settings().limits.away_min);
        hb.set_presence(presence::decide(idle, locked, hb.settings().limits.away_min));
        sync_keep_awake(&h);
        std::thread::sleep(Duration::from_secs(15));
    });
}

// ---------- global keys ----------

/// Puts the keys of `new` in place (new ones first, so a refusal leaves the old ones working) and drops the ones `old`
/// had that `new` does not. Nothing to do where the system has no global keys (Wayland, R10.1).
fn register_keys(app: &AppHandle, old: Option<&Settings>, new: &Settings) -> Res<()> {
    if !tray::hotkey_available() {
        return Ok(());
    }
    let parse = |s: &Settings| -> Vec<Shortcut> { [&s.keys.new_chat, &s.keys.freeze].iter().filter_map(|k| k.parse().ok()).collect() };
    let old_set = old.map(parse).unwrap_or_default();
    let gs = app.global_shortcut();
    let mut added: Vec<Shortcut> = vec![];
    for (field, text) in [("new_chat", &new.keys.new_chat), ("freeze", &new.keys.freeze)] {
        let sc: Shortcut = text.parse().map_err(|_| format!("bad_key: keys.{field}"))?;
        if old_set.contains(&sc) || added.contains(&sc) {
            continue;
        }
        if gs.register(sc).is_err() {
            for a in &added {
                let _ = gs.unregister(*a);
            }
            return Err(format!("key_busy: keys.{field}"));
        }
        added.push(sc);
    }
    let keep = parse(new);
    for o in old_set.into_iter().filter(|o| !keep.contains(o)) {
        let _ = gs.unregister(o);
    }
    Ok(())
}

// ---------- commands ----------

/// look8: the blur under a see-through window (window transparency in Settings -> «Окно и материалы»). The window is created
/// transparent (tauri.conf.json); the page decides how much of it to fill. macOS: Tauri's own `set_effects` puts an
/// NSVisualEffectView (material UnderWindowBackground, always active) under the web view, or takes it away. Elsewhere there is
/// nothing to add: a compositor (Hyprland, KWin, ...) shows what is behind as it is. Returns whether the blur is on.
#[tauri::command]
fn window_glass(window: tauri::WebviewWindow, blur: bool) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        use tauri::window::{Effect, EffectState, EffectsBuilder};
        let fx = blur.then(|| EffectsBuilder::new().effect(Effect::UnderWindowBackground).state(EffectState::Active).build());
        window.set_effects(fx).map_err(|e| e.to_string())?;
        Ok(blur)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, blur);
        Ok(false)
    }
}

#[tauri::command]
async fn list_projects(s: State<'_, App>) -> Res<Vec<Project>> {
    Ok(lk(s.hub.engine.store()).projects())
}

#[tauri::command]
async fn add_project(app: AppHandle, s: State<'_, App>, path: String) -> Res<Project> {
    let proj = s.hub.add_project(&path)?;
    let _ = app.emit("project-changed", &proj.id);
    Ok(proj)
}

#[tauri::command]
async fn create_project(app: AppHandle, s: State<'_, App>, parent: String, name: String) -> Res<Project> {
    let proj = s.hub.create_project(&parent, &name)?;
    let _ = app.emit("project-changed", &proj.id);
    Ok(proj)
}

#[tauri::command]
async fn update_project(app: AppHandle, s: State<'_, App>, project: Project) -> Res<()> {
    let id = project.id.clone();
    hub(&s).update_project(project)?;
    let _ = app.emit("project-changed", id);
    Ok(())
}

#[tauri::command]
async fn remove_project(app: AppHandle, s: State<'_, App>, id: String) -> Res<()> {
    s.hub.remove_project(&id)?;
    let _ = app.emit("project-changed", id);
    Ok(())
}

#[tauri::command]
async fn init_repo(path: String) -> Res<()> {
    nightshift_core::git::init_repo(Path::new(&path)).map_err(e2s)
}

#[tauri::command]
async fn repo_info(s: State<'_, App>, id: String) -> Res<Value> {
    hub(&s).repo_info(&id)
}

#[tauri::command]
async fn list_tasks(s: State<'_, App>) -> Res<Vec<Value>> {
    Ok(s.hub.list_tasks())
}

#[tauri::command]
async fn get_log(s: State<'_, App>, id: String, limit: usize) -> Res<Value> {
    Ok(json!(lk(s.hub.engine.store()).read_log(&id, limit)))
}

/// v3: a page of the task's log before `before` (an RFC 3339 time; None = the newest), with the actions'
/// output and diffs (story 230).
#[tauri::command]
async fn get_task_log(s: State<'_, App>, id: String, before: Option<String>, limit: usize) -> Res<Value> {
    Ok(json!(s.hub.task_log(&id, before.as_deref(), limit)?))
}

#[tauri::command]
async fn submit_task(s: State<'_, App>, project_id: String, prompt: String, agent: Option<String>) -> Res<String> {
    hub(&s).submit_task(&project_id, &prompt, agent)
}

#[tauri::command]
async fn answer(s: State<'_, App>, id: String, text: String) -> Res<()> {
    s.hub.engine.answer(&id, &text)
}

#[tauri::command]
async fn approve_outward(s: State<'_, App>, id: String, yes: bool) -> Res<()> {
    s.hub.engine.approve_outward(&id, yes)
}

#[tauri::command]
async fn cancel(s: State<'_, App>, id: String) -> Res<()> {
    s.hub.engine.cancel(&id)
}

#[tauri::command]
async fn retry(s: State<'_, App>, id: String) -> Res<()> {
    s.hub.retry(&id)
}

#[tauri::command]
async fn remove_task(app: AppHandle, s: State<'_, App>, id: String) -> Res<()> {
    s.hub.engine.remove(&id)?;
    let _ = app.emit("task-removed", &id);
    refresh_tray(&app);
    Ok(())
}

#[tauri::command]
async fn undo(s: State<'_, App>, id: String) -> Res<()> {
    s.hub.engine.undo(&id)
}

#[tauri::command]
async fn push_base(s: State<'_, App>, project_id: String) -> Res<()> {
    s.hub.push_base(&project_id)
}

#[tauri::command]
async fn pull_base(s: State<'_, App>, project_id: String) -> Res<()> {
    hub(&s).project_ready(&project_id)?;
    s.hub.engine.pull_base(&project_id)
}

#[tauri::command]
async fn pause_all(app: AppHandle, s: State<'_, App>, paused: bool) -> Res<()> {
    s.hub.engine.pause_all(paused);
    refresh_tray(&app);
    Ok(())
}

#[tauri::command]
async fn list_agents(s: State<'_, App>) -> Res<Value> {
    let h = hub(&s);
    let list = h.list_envs();
    Ok(json!(list
        .iter()
        .map(|e| json!({ "id": e["id"], "name": e["name"], "available": e["installed"], "hint": e["error"].as_str().unwrap_or(""), "guarded": e["sandbox"]["push_blocked"] }))
        .collect::<Vec<_>>()))
}

#[tauri::command]
async fn get_settings(s: State<'_, App>) -> Res<Settings> {
    Ok(s.hub.settings())
}

#[tauri::command]
async fn set_settings(app: AppHandle, s: State<'_, App>, settings: Value) -> Res<()> {
    let _one_at_a_time = lk(&s.settings_lock);
    s.hub.set_settings(settings, &|from, to| register_keys(&app, Some(from), to))?;
    Ok(())
}

#[tauri::command]
async fn github_info(s: State<'_, App>, project_id: String) -> Res<Value> {
    hub(&s).github_info(&project_id)
}

#[tauri::command]
async fn delete_merged_branches(s: State<'_, App>, project_id: String) -> Res<u32> {
    s.hub.delete_merged_branches(&project_id)
}

#[tauri::command]
async fn pr_state(s: State<'_, App>, task_id: String) -> Res<String> {
    hub(&s).pr_state(&task_id)
}

#[tauri::command]
async fn get_diffstat(s: State<'_, App>, task_id: String) -> Res<Vec<Value>> {
    hub(&s).get_diffstat(&task_id)
}

#[tauri::command]
async fn open_path(s: State<'_, App>, path: String) -> Res<()> {
    s.hub.open_path(&path)
}

#[tauri::command]
async fn open_editor(s: State<'_, App>, path: String) -> Res<()> {
    s.hub.open_editor(&path)
}

fn terminal_dir(s: &State<App>) -> PathBuf {
    s.hub.dir.join("terminal")
}

#[tauri::command]
async fn open_terminal(s: State<'_, App>, path: String) -> Res<()> {
    let dir = s.hub.terminal_folder(&path)?;
    term::open(&term::Launch { cwd: Some(dir), argv: vec![], banner: None, env: vec![] }, &terminal_dir(&s))
}

// ---- data that cannot be read (G16.4) ----

#[tauri::command]
async fn data_state(s: State<'_, App>) -> Res<Value> {
    // showing the screen issues the token that "start clean" needs
    Ok(json!({ "unreadable": s.unreadable.as_ref().map(|(_, c)| c), "token": s.unreadable.as_ref().map(|_| s.fresh.issue()) }))
}

#[tauri::command]
async fn open_data_folder(s: State<'_, App>) -> Res<()> {
    let (dir, _) = s.unreadable.as_ref().ok_or("data_readable")?;
    open::opener(&open::Target::Dir(dir.clone()))
}

/// Only with the token the screen was given: the file is put aside (never deleted), the app restarts on an empty store.
#[tauri::command]
async fn start_fresh(app: AppHandle, s: State<'_, App>, token: String) -> Res<()> {
    let (dir, _) = s.unreadable.as_ref().ok_or("data_readable")?;
    s.fresh.start_fresh(dir, &token)?;
    app.restart()
}

// ---- v2: envs, models, limits ----

#[tauri::command]
async fn list_envs(s: State<'_, App>) -> Res<Vec<Value>> {
    // everything runs through git: without it the window shows «Нужен git» (app.js reads "git not found") instead of a
    // first run whose «Выбрать папку» fails later; before, only the browser mock ever said so
    if !s.git_ok.load(Ordering::Relaxed) {
        if !git_works() {
            return Err("git not found: install git".into());
        }
        s.git_ok.store(true, Ordering::Relaxed);
    }
    Ok(s.hub.list_envs())
}

/// `git --version` answers (the macOS /usr/bin/git stub without the developer tools fails it).
fn git_works() -> bool {
    nightshift_core::proc::cmd("git").arg("--version").stdin(std::process::Stdio::null()).output().map(|o| o.status.success()).unwrap_or(false)
}

/// The envs asked again (a CLI was just installed in the window's terminal): `list_envs` alone answers from its cache.
#[tauri::command]
async fn recheck_envs(s: State<'_, App>) -> Res<Vec<Value>> {
    s.hub.recheck_envs();
    Ok(s.hub.list_envs())
}

#[tauri::command]
async fn get_catalog(s: State<'_, App>, env: String) -> Res<Value> {
    s.hub.get_catalog(&env)
}

#[tauri::command]
async fn refresh_catalog(s: State<'_, App>, env: String) -> Res<Value> {
    s.hub.refresh_catalog(&env)
}

#[tauri::command]
async fn get_limits(s: State<'_, App>, env: String) -> Res<Value> {
    Ok(s.hub.get_limits(&env))
}

#[tauri::command]
async fn check_custom(s: State<'_, App>, id: String) -> Res<Value> {
    let r = hub(&s).check_custom(&id)?;
    match (r.code, r.timed_out) {
        (Some(0), _) => Ok(json!({ "output": r.output, "code": 0 })),
        (_, true) => Err(format!("check_timeout: {}", r.output)),
        (c, _) => Err(format!("check_failed: exit {}: {}", c.map(|c| c.to_string()).unwrap_or_else(|| "?".into()), r.output)),
    }
}

#[tauri::command]
async fn install_env(s: State<'_, App>, env: String) -> Res<Value> {
    let dir = terminal_dir(&s);
    s.hub.install_env(&env, &|l| term::open(l, &dir))
}

#[tauri::command]
async fn login_env(s: State<'_, App>, env: String) -> Res<()> {
    let dir = terminal_dir(&s);
    s.hub.login_env(&env, &|l| term::open(l, &dir))
}

// ---- v2: chats ----

#[tauri::command]
async fn list_chats(s: State<'_, App>, env: String) -> Res<Value> {
    Ok(json!(s.hub.chats.list(&env)))
}

#[tauri::command]
async fn get_chat(s: State<'_, App>, id: String, before: Option<String>, limit: usize) -> Res<Value> {
    Ok(json!(s.hub.get_chat(&id, before.as_deref(), limit)?))
}

#[tauri::command]
async fn send_chat(s: State<'_, App>, target: ChatTarget, msg_id: String, text: String, attachments: Option<Vec<PathBuf>>) -> Res<String> {
    hub(&s).send_chat(target, &msg_id, &text, &attachments.unwrap_or_default())
}

#[tauri::command]
async fn retry_chat(s: State<'_, App>, id: String, msg_id: String) -> Res<()> {
    s.hub.chats.retry(&id, &msg_id)
}

/// F12 #9: a short read-only question with the chat's context; nothing is stored in the chat.
#[tauri::command]
async fn side_ask(s: State<'_, App>, id: String, text: String) -> Res<String> {
    s.hub.chats.side_ask(&id, &text)
}

#[tauri::command]
async fn remove_chat(s: State<'_, App>, id: String) -> Res<()> {
    s.hub.chats.remove(&id)
}

#[tauri::command]
async fn open_chat_terminal(s: State<'_, App>, id: String) -> Res<()> {
    let dir = terminal_dir(&s);
    s.hub.open_chat_terminal(&id, &|l| term::open(l, &dir))
}

// ---- v6 (task 27): the queue of a chat on disk, stopping an answer, the words of the spinner ----

#[tauri::command]
async fn chat_queue(s: State<'_, App>, id: String) -> Res<Value> {
    let (queue, wait) = s.hub.chats.queue(&id);
    Ok(json!({ "queue": queue, "wait": wait }))
}

#[tauri::command]
async fn chat_queue_add(s: State<'_, App>, id: String, msg_id: String, text: String, first: Option<bool>, attachments: Option<Vec<PathBuf>>) -> Res<()> {
    s.hub.chats.queue_add_with(&id, &msg_id, &text, first.unwrap_or(false), &attachments.unwrap_or_default())
}

#[tauri::command]
async fn chat_queue_edit(s: State<'_, App>, id: String, msg_id: String, text: String) -> Res<()> {
    s.hub.chats.queue_edit(&id, &msg_id, &text)
}

#[tauri::command]
async fn chat_queue_drop(s: State<'_, App>, id: String, msg_id: String) -> Res<()> {
    s.hub.chats.queue_drop(&id, &msg_id)
}

#[tauri::command]
async fn chat_stop(s: State<'_, App>, id: String) -> Res<()> {
    s.hub.chats.stop_answer(&id)
}

#[tauri::command]
async fn ui_hints(s: State<'_, App>, env: String, project_id: Option<String>) -> Res<Value> {
    Ok(s.hub.ui_hints(&env, project_id.as_deref()))
}

// ---- v2: tasks ----

#[tauri::command]
async fn note(s: State<'_, App>, task_id: String, to: Option<String>, text: String) -> Res<Value> {
    s.hub.note(&task_id, to, &text)
}

#[tauri::command]
async fn answer_outward(s: State<'_, App>, task_id: String, choice: OutwardChoice) -> Res<()> {
    s.hub.answer_outward(&task_id, choice)
}

#[tauri::command]
async fn resume_failed(s: State<'_, App>, task_id: String, hint: Option<String>, model: Option<ModelChoice>) -> Res<()> {
    s.hub.resume_failed(&task_id, hint, model)
}

#[tauri::command]
async fn pick_variant(s: State<'_, App>, task_id: String, variant: String) -> Res<()> {
    s.hub.engine.pick_variant(&task_id, &variant)
}

#[tauri::command]
async fn stop_swarm(s: State<'_, App>, task_id: String) -> Res<()> {
    s.hub.engine.stop_swarm(&task_id)
}

#[tauri::command]
async fn nudge(s: State<'_, App>, task_id: String) -> Res<()> {
    s.hub.engine.nudge(&task_id)
}

#[tauri::command]
async fn switch_model(s: State<'_, App>, task_id: String, model: ModelChoice) -> Res<()> {
    s.hub.engine.switch_model(&task_id, model)
}

#[tauri::command]
async fn freeze(app: AppHandle, s: State<'_, App>, on: bool) -> Res<()> {
    s.hub.freeze(on);
    refresh_tray(&app);
    Ok(())
}

#[tauri::command]
async fn hand_off(s: State<'_, App>, task_id: String) -> Res<Value> {
    let dir = terminal_dir(&s);
    s.hub.hand_off(&task_id, &term::available, &|l| term::open(l, &dir))
}

#[tauri::command]
async fn take_back(s: State<'_, App>, task_id: String) -> Res<()> {
    s.hub.take_back(&task_id)
}

#[tauri::command]
async fn start_now(s: State<'_, App>, task_id: String) -> Res<()> {
    s.hub.engine.start_now(&task_id)
}

// F13: rollback to a turn, the "До мержа" list, the project folder on a task's branch
#[tauri::command]
async fn task_snapshots(s: State<'_, App>, task_id: String) -> Res<Value> {
    Ok(serde_json::to_value(s.hub.engine.snapshots(&task_id)?).unwrap_or_default())
}

#[tauri::command]
async fn restore_snapshot(s: State<'_, App>, task_id: String, n: u32) -> Res<u32> {
    s.hub.engine.restore_snapshot(&task_id, n)
}

#[tauri::command]
async fn todo_add(s: State<'_, App>, task_id: String, text: String) -> Res<()> {
    s.hub.engine.todo_add(&task_id, &text, "human")
}

#[tauri::command]
async fn todo_set(s: State<'_, App>, task_id: String, todo_id: String, done: bool) -> Res<()> {
    s.hub.engine.todo_set(&task_id, &todo_id, done)
}

#[tauri::command]
async fn todo_remove(s: State<'_, App>, task_id: String, todo_id: String) -> Res<()> {
    s.hub.engine.todo_remove(&task_id, &todo_id)
}

#[tauri::command]
async fn take_to_main(s: State<'_, App>, task_id: String) -> Res<()> {
    s.hub.engine.take_to_main(&task_id)
}

#[tauri::command]
async fn give_to_agent(s: State<'_, App>, task_id: String) -> Res<()> {
    s.hub.engine.give_to_agent(&task_id)
}

#[tauri::command]
async fn not_a_task(s: State<'_, App>, task_id: String) -> Res<()> {
    s.hub.not_a_task(&task_id)
}

#[tauri::command]
async fn submit_convoy(s: State<'_, App>, project_id: String, env: String, text: String) -> Res<String> {
    hub(&s).submit_convoy(&project_id, &env, &text)
}

#[tauri::command]
async fn summary_since(s: State<'_, App>, at: String) -> Res<Value> {
    s.hub.summary_since(&at)
}

#[tauri::command]
async fn diff_file(s: State<'_, App>, task_id: String, path: String, variant: Option<String>) -> Res<Value> {
    s.hub.diff_file(&task_id, &path, variant.as_deref())
}

// ---- v2: kits and secrets ----

#[tauri::command]
async fn kits_inventory(s: State<'_, App>, env: String, scope: Scope) -> KRes<Value> {
    s.hub.kits_inventory(&env, &scope)
}

#[tauri::command]
async fn kits_export(s: State<'_, App>, env: String, selection: Selection, dest: ExportDest) -> KRes<Value> {
    s.hub.kits_export(&env, &selection, dest)
}

#[tauri::command]
async fn kits_preview(s: State<'_, App>, env: String, source: KitSource, scope: Scope) -> KRes<Value> {
    s.hub.kits_preview(&env, source, &scope)
}

#[tauri::command]
async fn kits_install(app: AppHandle, s: State<'_, App>, plan: Value, choices: Choices) -> KRes<Value> {
    let (snapshot, project) = s.hub.kits_install(&plan, &choices)?;
    if let Some(id) = project {
        let _ = app.emit("project-changed", id);
    }
    Ok(json!(snapshot))
}

#[tauri::command]
async fn kits_enable(s: State<'_, App>, env: String, item: HeldRef) -> KRes<()> {
    s.hub.kits_enable(&env, &item)
}

#[tauri::command]
async fn kits_snapshots(s: State<'_, App>, env: String) -> KRes<Value> {
    s.hub.kits_snapshots(&env)
}

#[tauri::command]
async fn kits_rollback(s: State<'_, App>, id: String) -> KRes<Value> {
    s.hub.kits_rollback(&id)
}

#[tauri::command]
async fn kits_check_update(s: State<'_, App>, installed: Installed) -> KRes<Value> {
    s.hub.kits_check_update(&installed)
}

#[tauri::command]
async fn set_secret(s: State<'_, App>, env: String, name: String, value: String) -> KRes<Value> {
    s.hub.set_secret(&env, &name, &value)
}

// ---------- start ----------

fn build_app(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let h = app.handle().clone();
    let dir = h.path().app_data_dir()?;
    let home = h.path().home_dir()?;
    let startup_list: Startup = Arc::new(Mutex::new(vec![]));
    let (store, replaced, unreadable) = match startup::open_store(&dir)? {
        startup::Opened::Ready { store, replaced } => (store, replaced, None),
        // keep the window quiet: an empty store somewhere else stands in while the dialog is open
        startup::Opened::Unreadable { copy } => {
            let tmp = std::env::temp_dir().join(format!("nightshift-unreadable-{}", std::process::id()));
            (nightshift_core::store::Store::open(&tmp).map_err(e2s)?, None, Some(copy))
        }
    };
    let settings = store.settings();
    let ru = Arc::new(AtomicBool::new(is_ru(&settings)));
    let notify_cfg = Arc::new(Mutex::new(settings.notify.clone()));
    let (hb_h, list) = (h.clone(), startup_list.clone());
    let notifier = Arc::new(AppNotifier::new(
        Arc::new(SystemSink(h.clone())),
        { let c = notify_cfg.clone(); Arc::new(move || lk(&c).clone()) },
        Arc::new(chrono::Local::now),
        { let r = ru.clone(); Arc::new(move || r.load(Ordering::Relaxed)) },
        Arc::new(move |how| {
            note_startup(&list, "notify-blocked", json!({ "how": how }));
            let _ = hb_h.emit("notify-blocked", json!({ "how": how }));
        }),
    ));
    let hub_dir = if unreadable.is_some() { std::env::temp_dir().join(format!("nightshift-unreadable-{}", std::process::id())) } else { dir.clone() };
    let kits = Kits::new(home, hub_dir.join("kits"));
    let hb = Hub::new(store, agent::adapters(&settings), notifier.clone() as Arc<dyn Notifier>, kits, hub_dir, Arc::new(chrono::Local::now));
    let state = App {
        hub: hb.clone(),
        tray: Mutex::new(None),
        settings_lock: Mutex::new(()),
        ru: ru.clone(),
        notify_cfg,
        keep: KeepAwake::system(),
        host: AtomicU8::new(0),
        startup: startup_list.clone(),
        first_new_chat: AtomicBool::new(tray::wants_new_chat(&std::env::args().collect::<Vec<_>>())),
        git_ok: AtomicBool::new(false),
        unreadable: unreadable.clone().map(|c| (dir.clone(), c)),
        fresh: startup::FreshGate::default(),
    };
    app.manage(state);
    {
        // what follows a saved setting: the notifier's copy, the language, the tray and the hold on sleep
        let (cfg, ru, h2) = (app.state::<App>().notify_cfg.clone(), ru.clone(), h.clone());
        hb.on_settings(Box::new(move |s| {
            *lk(&cfg) = s.notify.clone();
            ru.store(is_ru(s), Ordering::Relaxed);
            refresh_tray(&h2);
            sync_keep_awake(&h2);
        }));
    }

    let saved = restored_terms(&dir, unreadable.is_none());
    if unreadable.is_some() {
        // the window shows its own screen; the engine of the stand-in store is never started
        return Ok(());
    }
    if !saved.is_empty() {
        note_startup(&startup_list, "terms-restored", json!(saved));
    }
    if let Some(p) = replaced {
        note_startup(&startup_list, "settings-replaced", json!({ "path": p }));
    }
    for r in hb.kits_recover() {
        note_startup(&startup_list, "kits-recovered", json!({ "kit_name": r.kit_name, "env": r.env, "message": r.message, "snapshot": r.snapshot }));
    }

    // tray
    let is_ru_now = ru.load(Ordering::Relaxed);
    let t = |ru_s: &'static str, en: &'static str| if is_ru_now { ru_s } else { en };
    let open_i = MenuItem::with_id(app, "open", t("Открыть", "Open"), true, None::<&str>)?;
    let new_i = MenuItem::with_id(app, "new", t("Новый чат", "New chat"), true, None::<&str>)?;
    let freeze_i = MenuItem::with_id(app, "freeze", t("Заморозить всё", "Freeze all"), true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", t("Выход", "Quit"), true, None::<&str>)?;
    let status = MenuItem::with_id(app, "status", t("Ничего не идёт", "Idle"), false, None::<&str>)?;
    let sessions = MenuItem::with_id(app, "sessions", t("Сессии: 0", "Sessions: 0"), false, None::<&str>)?;
    let menu = Menu::new(app)?;
    if cfg!(target_os = "linux") {
        // Linux trays have no tooltip: the counter is the first (disabled) menu line
        menu.append(&status)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    menu.append_items(&[&open_i, &new_i, &freeze_i, &sessions, &PredefinedMenuItem::separator(app)?, &quit_i])?;
    let mut tb = TrayIconBuilder::with_id("main").menu(&menu).tooltip("Nightshift").on_menu_event(|app, ev| match ev.id().as_ref() {
        "open" => show_window(app),
        "new" => new_chat(app),
        "freeze" => {
            let st = app.state::<App>();
            st.hub.freeze(!st.hub.engine.is_frozen());
            refresh_tray(app);
        }
        "quit" => quit(app),
        _ => {}
    });
    if let Some(icon) = app.default_window_icon() {
        tb = tb.icon(icon.clone());
    }
    let tray_icon = tb.build(app)?;
    *lk(&app.state::<App>().tray) = Some(Tray { icon: tray_icon, freeze: freeze_i, status, sessions });
    // NS_RECORD=1: WebKit keeps drawing a covered window (timers, xterm, live output), so a window recording of a run stays live
    #[cfg(target_os = "macos")]
    if std::env::var_os("NS_RECORD").is_some() {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.with_webview(|wv| unsafe {
                let v = &*(wv.inner() as *mut objc2::runtime::AnyObject);
                let has: bool = objc2::msg_send![v, respondsToSelector: objc2::sel!(_setWindowOcclusionDetectionEnabled:)];
                if has {
                    let _: () = objc2::msg_send![v, _setWindowOcclusionDetectionEnabled: false];
                }
            });
        }
    }
    // our own menu bar: Tauri's standard one takes ⌘W for "Close Window" before the page sees it
    #[cfg(target_os = "macos")]
    {
        app.set_menu(app_menu(&h, is_ru_now)?)?;
        app.on_menu_event(|app, ev| {
            if ev.id().as_ref() == QUIT_ID {
                quit(app); // ⌘Q: one question for the tasks and the sessions (story 303)
            } else if ev.id().as_ref() == CLOSE_WINDOW_ID {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.close(); // CloseRequested: hidden or minimized as on the red button
                }
            }
        });
    }

    h.notification().on_action({
        let h = h.clone();
        move |a| on_notification_click(&h, a)
    }).ok();
    if let Err(e) = register_keys(&h, None, &settings) {
        eprintln!("global keys not registered: {e}");
    }
    forward_events(&h, &hb, &notifier);
    check_tray_host(&h);
    hb.start();
    start_timers(&h, &hb);
    config::start(&h, hb.clone(), register_keys);
    updates::start(&h, hb.clone());
    diag::start(&h, hb.clone());
    auto::start(&h, hb.clone());
    refresh_tray(&h);
    Ok(())
}

/// The window may load later than the things it must be told: send them (again) when a page has loaded. Only to the
/// window that loaded: another window opening (or the preview reloading) must not make the notices of this one double
/// or come back after they were closed. The preview is a page of somebody else's and is told nothing.
fn emit_startup(app: &AppHandle, label: &str) {
    if label == preview::LABEL {
        return;
    }
    let Some(st) = app.try_state::<App>() else { return };
    // copied out: the emits below must not run under the lock
    let lines: Vec<(String, Value)> = lk(&st.startup).clone();
    for (name, payload) in &lines {
        let _ = app.emit_to(label, name, payload);
    }
    let _ = app.emit_to(label, "tray-state", tray_state(&st));
    if label == "main" && st.first_new_chat.swap(false, Ordering::Relaxed) {
        let _ = app.emit_to(label, "focus-input", ());
    }
}

/// One item of the macOS menu bar: a standard one, or our "Close Window".
#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
enum MenuEntry {
    About,
    Services,
    Hide,
    HideOthers,
    ShowAll,
    Quit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Fullscreen,
    Minimize,
    Maximize,
    Separator,
    CloseWindow,
}

/// "Close Window" is on ⇧⌘W: ⌘W, ⌘T, ⌘D and ⇧⌘D are the page's tab keys (caught in `keydown`), and a menu
/// item with one of them would take it first. No other item of the menu uses them.
#[cfg(any(target_os = "macos", test))]
const CLOSE_WINDOW_KEY: &str = "Shift+CmdOrCtrl+W";
#[cfg(any(target_os = "macos", test))]
const CLOSE_WINDOW_ID: &str = "close_window";
#[cfg(any(target_os = "macos", test))]
const QUIT_ID: &str = "app_quit";

/// The menu bar: the app, Edit (copy / paste work in the fields), View, Window.
#[cfg(any(target_os = "macos", test))]
fn menu_layout(ru: bool) -> Vec<(&'static str, Vec<MenuEntry>)> {
    use MenuEntry::*;
    let t = |r: &'static str, e: &'static str| if ru { r } else { e };
    vec![
        ("Nightshift", vec![About, Separator, Services, Separator, Hide, HideOthers, ShowAll, Separator, Quit]),
        (t("Правка", "Edit"), vec![Undo, Redo, Separator, Cut, Copy, Paste, SelectAll]),
        (t("Вид", "View"), vec![Fullscreen]),
        (t("Окно", "Window"), vec![Minimize, Maximize, Separator, CloseWindow]),
    ]
}

/// The words of a menu item (macOS draws the standard ones in English unless told).
#[cfg(any(target_os = "macos", test))]
fn menu_label(e: MenuEntry, ru: bool) -> Option<&'static str> {
    use MenuEntry::*;
    let (r, en) = match e {
        About => ("О программе Nightshift", "About Nightshift"),
        Services => ("Службы", "Services"),
        Hide => ("Скрыть Nightshift", "Hide Nightshift"),
        HideOthers => ("Скрыть остальные", "Hide Others"),
        ShowAll => ("Показать все", "Show All"),
        Quit => ("Выйти из Nightshift", "Quit Nightshift"),
        Undo => ("Отменить", "Undo"),
        Redo => ("Повторить", "Redo"),
        Cut => ("Вырезать", "Cut"),
        Copy => ("Копировать", "Copy"),
        Paste => ("Вставить", "Paste"),
        SelectAll => ("Выбрать все", "Select All"),
        Fullscreen => ("Во весь экран", "Enter Full Screen"),
        Minimize => ("Свернуть", "Minimize"),
        Maximize => ("Изменить масштаб", "Zoom"),
        CloseWindow => ("Закрыть окно", "Close Window"),
        Separator => return None,
    };
    Some(if ru { r } else { en })
}

#[cfg(target_os = "macos")]
fn app_menu(app: &AppHandle, ru: bool) -> tauri::Result<Menu<Wry>> {
    use tauri::menu::{IsMenuItem, Submenu};
    let menu = Menu::new(app)?;
    for (title, entries) in menu_layout(ru) {
        let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = vec![];
        for e in entries {
            let l = menu_label(e, ru);
            items.push(match e {
                MenuEntry::About => Box::new(PredefinedMenuItem::about(app, l, None)?),
                MenuEntry::Services => Box::new(PredefinedMenuItem::services(app, l)?),
                MenuEntry::Hide => Box::new(PredefinedMenuItem::hide(app, l)?),
                MenuEntry::HideOthers => Box::new(PredefinedMenuItem::hide_others(app, l)?),
                MenuEntry::ShowAll => Box::new(PredefinedMenuItem::show_all(app, l)?),
                MenuEntry::Quit => Box::new(MenuItem::with_id(app, QUIT_ID, l.unwrap_or_default(), true, Some("CmdOrCtrl+Q"))?),
                MenuEntry::Undo => Box::new(PredefinedMenuItem::undo(app, l)?),
                MenuEntry::Redo => Box::new(PredefinedMenuItem::redo(app, l)?),
                MenuEntry::Cut => Box::new(PredefinedMenuItem::cut(app, l)?),
                MenuEntry::Copy => Box::new(PredefinedMenuItem::copy(app, l)?),
                MenuEntry::Paste => Box::new(PredefinedMenuItem::paste(app, l)?),
                MenuEntry::SelectAll => Box::new(PredefinedMenuItem::select_all(app, l)?),
                MenuEntry::Fullscreen => Box::new(PredefinedMenuItem::fullscreen(app, l)?),
                MenuEntry::Minimize => Box::new(PredefinedMenuItem::minimize(app, l)?),
                MenuEntry::Maximize => Box::new(PredefinedMenuItem::maximize(app, l)?),
                MenuEntry::Separator => Box::new(PredefinedMenuItem::separator(app)?),
                MenuEntry::CloseWindow => Box::new(MenuItem::with_id(app, CLOSE_WINDOW_ID, l.unwrap_or_default(), true, Some(CLOSE_WINDOW_KEY))?),
            });
        }
        let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|b| b.as_ref()).collect();
        menu.append(&Submenu::with_items(app, title, true, &refs)?)?;
    }
    Ok(menu)
}

fn main() {
    // Windows has USERPROFILE, not HOME; the core and git read HOME for ~/.claude, ~/.config and the like
    #[cfg(windows)]
    if std::env::var_os("HOME").is_none() {
        if let Some(p) = std::env::var_os("USERPROFILE") {
            std::env::set_var("HOME", p);
        }
    }
    pathfix::fix_path();
    let app = tauri::Builder::default()
        // a second start: show the window; with `--new-chat` also put the cursor into a new chat (R10.1)
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if tray::wants_new_chat(&args) {
                new_chat(app);
            } else {
                show_window(app);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, sc, ev| {
                    if ev.state() != ShortcutState::Pressed {
                        return;
                    }
                    let Some(st) = app.try_state::<App>() else { return };
                    let keys = st.hub.settings().keys;
                    if keys.new_chat.parse::<Shortcut>().ok().as_ref() == Some(sc) {
                        new_chat(app);
                    } else if keys.freeze.parse::<Shortcut>().ok().as_ref() == Some(sc) {
                        st.hub.freeze(!st.hub.engine.is_frozen());
                        refresh_tray(app);
                    }
                })
                .build(),
        )
        .setup(|app| build_app(app))
        .on_page_load(|webview, payload| {
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                let (app, label) = (webview.app_handle().clone(), webview.label().to_string());
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(1200));
                    emit_startup(&app, &label);
                });
            }
        })
        .manage(ptycmds::Pty::default())
        .manage(windows::Windows::default())
        .manage(preview::Picker::default())
        .on_window_event(|w, ev| match ev {
            // an extra window closes for real; its sessions go on, detached (story 301)
            tauri::WindowEvent::Destroyed if w.label() != "main" => {
                let app = w.app_handle();
                app.state::<ptycmds::Pty>().window_gone(w.label());
                app.state::<windows::Windows>().gone(w.label());
                let _ = app.emit("windows-changed", ());
            }
            tauri::WindowEvent::CloseRequested { api, .. } if w.label() == "main" => {
                // the state file could not be read: there is no tray, no menu and no "Quit" anywhere, so closing the window ends the app
                if w.app_handle().try_state::<App>().is_some_and(|s| s.unreadable.is_some()) {
                    w.app_handle().exit(0);
                    return;
                }
                api.prevent_close();
                // a hidden page stops parsing output; detached, the sessions run on into their buffer (story 301)
                if let Some(p) = w.app_handle().try_state::<ptycmds::Pty>() {
                    p.window_gone(w.label());
                }
                let host = match w.app_handle().try_state::<App>().map(|s| s.host.load(Ordering::Relaxed)) {
                    Some(1) => Some(true),
                    Some(2) => Some(false),
                    _ => None,
                };
                match tray::on_close(host) {
                    tray::OnClose::Hide => {
                        let _ = w.hide();
                        #[cfg(target_os = "macos")]
                        let _ = w.app_handle().set_activation_policy(tauri::ActivationPolicy::Accessory);
                    }
                    // no icon to come back by: the window stays in the taskbar (R10.2)
                    tray::OnClose::Minimize => {
                        let _ = w.minimize();
                    }
                }
            }
            // back to the window: the envs are asked again (G17.3)
            tauri::WindowEvent::Focused(true) => {
                if let Some(st) = w.app_handle().try_state::<App>() {
                    let hb = st.hub.clone();
                    std::thread::spawn(move || {
                        hb.recheck_envs();
                    });
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            list_projects, add_project, create_project, update_project, remove_project, init_repo, repo_info,
            list_tasks, get_log, get_task_log, submit_task, answer, approve_outward,
            cancel, retry, remove_task, undo, push_base, pull_base, pause_all,
            list_agents, get_settings, set_settings,
            github_info, delete_merged_branches, pr_state, get_diffstat,
            open_path, open_editor, open_terminal,
            task_snapshots, restore_snapshot, todo_add, todo_set, todo_remove, take_to_main, give_to_agent,
            window_glass,
            list_envs, recheck_envs, get_catalog, refresh_catalog, get_limits, check_custom, install_env, login_env,
            list_chats, get_chat, send_chat, retry_chat, side_ask, remove_chat, open_chat_terminal,
            chat_queue, chat_queue_add, chat_queue_edit, chat_queue_drop, chat_stop, ui_hints,
            shellcmds::chat_shell, shellcmds::chat_attach, shellcmds::chat_attach_path, shellcmds::chat_detach, shellcmds::chat_terminal,
            note, answer_outward, resume_failed, pick_variant, stop_swarm, nudge, switch_model, freeze,
            hand_off, take_back, start_now, not_a_task, submit_convoy, summary_since, diff_file,
            kits_inventory, kits_export, kits_preview, kits_install, kits_enable, kits_snapshots, kits_rollback, kits_check_update, set_secret, kitcmds::kits_spread, kitcmds::kits_spread_last, kitcmds::kits_spread_undo,
            data_state, open_data_folder, start_fresh,
            ptycmds::term_open, ptycmds::term_write, ptycmds::term_resize, ptycmds::term_close, ptycmds::term_attach,
            ptycmds::term_detach, ptycmds::term_ack, ptycmds::term_list,
            agentcmds::agent_worktree, agentcmds::agent_worktree_remove, agentcmds::agent_changes, agentcmds::agent_file_diff,
            agentcmds::agent_revert_file, agentcmds::agent_merge, agentcmds::term_usage, agentcmds::notify_agent,
            windows::window_detach, windows::window_tab, windows::window_return,
            // v4 skeletons (tasks 19-25)
            market::market_search, market::market_item, market::market_plan, market::market_apply, market::market_installed,
            market::market_remove, market::market_sources, market::market_set_sources,
            updates::cli_status, updates::cli_update, updates::updates_history, updates::cli_update_mode,
            gitcmds::gh_publish, gitcmds::gh_prs, gitcmds::gh_account, gitcmds::gh_repos, gitcmds::clone_project, gitcmds::git_log_page, gitcmds::git_revert_commit, gitcmds::startup_tax, gitcmds::compact, gitcmds::commit_stat, gitcmds::commit_file_diff,
            config::config_open, config::config_errors,
            auto::auto_get, auto::auto_schedule_save, auto::auto_schedule_remove, auto::auto_schedule_toggle, auto::auto_suggest_scan,
            auto::auto_issue_answer, sessions::sessions_list, sessions::session_continue, auto::auto_suggest_do, auto::auto_suggest_dismiss, auto::auto_suggest_nightly, auto::auto_ci_set, auto::auto_ci_poll,
            diag::spend_history, diag::diag_collect, diag::app_update_state, diag::app_update_check, diag::app_update_dismiss,
            preview::preview_open, preview::preview_reload, preview::preview_width, preview::preview_close, preview::preview_pick, preview::proof_shot
        ])
        .build(tauri::generate_context!())
        .expect("error while building Nightshift");
    app.run(|app, ev| match ev {
        // the window only hides; the process ends via tray "Quit"
        tauri::RunEvent::ExitRequested { code: None, api, .. } => api.prevent_exit(),
        tauri::RunEvent::Exit => shut_down(app),
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use super::{menu_label, menu_layout, restored_terms, MenuEntry, QuitGate, CLOSE_WINDOW_KEY, TERMS_FILE};

    /// Live run v4 (303): ⌘Q pressed again while the question is open stacked a second and a third dialog.
    #[test]
    fn a_second_quit_while_the_question_is_open_shows_no_second_question() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{mpsc, Arc};
        let gate = Arc::new(QuitGate::new());
        let asked = Arc::new(AtomicUsize::new(0));
        let (open_tx, open_rx) = mpsc::channel();
        let (answer_tx, answer_rx) = mpsc::channel::<bool>();
        let (g, a) = (gate.clone(), asked.clone());
        let first = std::thread::spawn(move || {
            g.ask(|| {
                a.fetch_add(1, Ordering::SeqCst);
                open_tx.send(()).unwrap();
                answer_rx.recv().unwrap()
            })
        });
        open_rx.recv().unwrap(); // the first question is on the screen
        let second = gate.ask(|| {
            asked.fetch_add(1, Ordering::SeqCst);
            true
        });
        assert_eq!(second, None, "the second quit waits for the open question");
        assert_eq!(asked.load(Ordering::SeqCst), 1);
        answer_tx.send(false).unwrap(); // "Остаться"
        assert_eq!(first.join().unwrap(), Some(false));
        // after "Остаться" the next quit asks again
        assert_eq!(gate.ask(|| true), Some(true));
        assert_eq!(gate.ask(|| true), None, "after \"Выйти\" the app is leaving: no new question");
    }

    /// Live run v4: About / Services / Hide / Hide Others / Show All were English beside a Russian "Выйти".
    #[test]
    fn the_system_items_of_the_menu_speak_the_language_of_the_window() {
        let cyr = |s: &str| s.chars().any(|c| ('а'..='я').contains(&c.to_lowercase().next().unwrap()));
        for (_, entries) in menu_layout(true) {
            for e in entries.into_iter().filter(|e| *e != MenuEntry::Separator) {
                let label = menu_label(e, true).unwrap_or_default();
                assert!(cyr(label), "{e:?}: {label:?}");
                assert!(!cyr(menu_label(e, false).unwrap_or("x")), "{e:?} in English");
            }
        }
        assert_eq!(menu_label(MenuEntry::Hide, true), Some("Скрыть Nightshift"));
        assert_eq!(menu_label(MenuEntry::About, false), Some("About Nightshift"));
    }

    /// Repair of F4: the terminal snapshot holds terminal output, so it is removed at start even when the data folder
    /// is unreadable and nothing is restored.
    #[test]
    fn the_terminal_snapshot_is_removed_at_start_in_every_branch() {
        for readable in [true, false] {
            let t = crate::testkit::Scratch::new("start-terms");
            std::fs::write(t.0.join(TERMS_FILE), "[]").unwrap();
            assert!(restored_terms(&t.0, readable).is_empty());
            assert!(!t.0.join(TERMS_FILE).exists(), "readable={readable}");
        }
    }
    use tauri_plugin_global_shortcut::Shortcut;

    /// ⌘W / ⌘T / ⌘D / ⇧⌘D reach the page: the menu has no standard "Close Window" (⌘W) and ours is on another key.
    #[test]
    fn the_menu_bar_leaves_the_tab_keys_to_the_page() {
        for ru in [false, true] {
            let layout = menu_layout(ru);
            let titles: Vec<&str> = layout.iter().map(|(t, _)| *t).collect();
            assert_eq!(titles.len(), 4, "{titles:?}");
            assert!(layout[1].1.contains(&MenuEntry::Copy) && layout[1].1.contains(&MenuEntry::Paste), "Edit with copy and paste");
            let closes: Vec<usize> = layout.iter().enumerate().filter(|(_, (_, e))| e.contains(&MenuEntry::CloseWindow)).map(|(i, _)| i).collect();
            assert_eq!(closes, [3], "one Close Window, in Window");
        }
        let close: Shortcut = CLOSE_WINDOW_KEY.parse().unwrap();
        assert_eq!(close, "Shift+CmdOrCtrl+W".parse::<Shortcut>().unwrap());
        let k = nightshift_core::model::KeySettings::default();
        for page in [&k.tab_new, &k.tab_close, &k.tab_split, &k.tab_unsplit] {
            assert_ne!(page.parse::<Shortcut>().unwrap(), close, "{page}");
        }
    }

    /// The window: no remote sources, no inline code; the capabilities name only what the window itself calls (R13).
    #[test]
    fn the_security_config_has_no_outside_source_and_the_capabilities_stay_small() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let csp = conf["app"]["security"]["csp"].as_str().unwrap();
        assert!(!csp.contains("unsafe-eval") && !csp.contains('*'), "{csp}");
        // xterm writes its own <style> elements (WKWebView refuses them otherwise); scripts stay 'self' only
        let inline: Vec<&str> = csp.split(';').filter(|d| d.contains("unsafe-inline")).map(str::trim).collect();
        assert_eq!(inline, ["style-src 'self' 'unsafe-inline'"], "{csp}");
        assert!(csp.split(';').any(|d| d.trim() == "script-src 'self'"), "{csp}");
        let sources: Vec<&str> = csp.split([';', ' ']).filter(|w| w.contains("://") || w.contains('.')).collect();
        assert_eq!(sources, ["http://ipc.localhost"], "the only address is Tauri's own IPC");
        assert!(conf["app"]["windows"].as_array().unwrap().iter().all(|w| w.get("url").is_none()), "the window loads the bundled ui, no remote URL");
        let caps: serde_json::Value = serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        assert_eq!(caps["permissions"], serde_json::json!(["core:event:default", "dialog:allow-open"]));
        assert_eq!(caps["windows"], serde_json::json!(["main"]));
        let extra: serde_json::Value = serde_json::from_str(include_str!("../capabilities/windows.json")).unwrap();
        assert_eq!((&extra["windows"], &extra["permissions"]), (&serde_json::json!(["w-*"]), &serde_json::json!(["core:event:default"])));
        let targets: Vec<&str> = conf["bundle"]["targets"].as_array().unwrap().iter().filter_map(|t| t.as_str()).collect();
        for t in ["app", "dmg", "deb", "appimage"] {
            assert!(targets.contains(&t), "{t}");
        }
    }
}
