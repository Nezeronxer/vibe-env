//! Window, tray, hotkey and system notifications around the core engine.
//! Commands are thin wrappers over `Engine` / `Store` / `git`.
use nightshift_core::agent;
use nightshift_core::engine::{Engine, EngineEvent, NewTask};
use nightshift_core::git;
use nightshift_core::model::{Project, Settings, Status, Task};
use nightshift_core::notify::{Notice, Notifier};
use nightshift_core::store::Store;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Emitter, Manager, State, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_notification::NotificationExt;

struct Tray {
    icon: TrayIcon,
    pause: MenuItem<Wry>,
    /// Disabled first menu item: the only place the counter is visible on Linux.
    status: MenuItem<Wry>,
}

struct App {
    engine: RwLock<Arc<Engine>>,
    dir: PathBuf,
    tray: Mutex<Option<Tray>>,
    /// Serializes `set_settings`, so only one hotkey change / adapter swap is in flight.
    settings_lock: Mutex<()>,
    /// UI language, recomputed at start and when settings change (not on every event).
    ru: AtomicBool,
}

type Res<T> = Result<T, String>;

fn lk<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}
/// Runs `f` with the engine. Do not nest it or call `eng` inside.
fn with_engine<T>(s: &State<App>, f: impl FnOnce(&Engine) -> T) -> T {
    let g = s.engine.read().unwrap_or_else(|e| e.into_inner());
    f(&g)
}
fn eng(s: &State<App>) -> Arc<Engine> {
    s.engine.read().unwrap_or_else(|e| e.into_inner()).clone()
}

// ---------- tray counter, window title ----------

/// `2 ▶ 1 ?`: only non-zero parts, empty when idle.
fn tray_label(running: usize, waiting: usize) -> String {
    let mut parts = vec![];
    if running > 0 {
        parts.push(format!("{running} ▶"));
    }
    if waiting > 0 {
        parts.push(format!("{waiting} ?"));
    }
    parts.join(" ")
}

fn counts(tasks: &[Task]) -> (usize, usize) {
    let running = tasks.iter().filter(|t| nightshift_core::engine::is_active(&t.status)).count();
    let waiting = tasks.iter().filter(|t| matches!(t.status, Status::NeedsHuman | Status::AwaitingOutward)).count();
    (running, waiting)
}

fn is_ru(s: &Settings) -> bool {
    match s.language.as_str() {
        "ru" => true,
        "auto" => {
            let env = std::env::var("LC_ALL").or_else(|_| std::env::var("LANG")).unwrap_or_default();
            let mac = if cfg!(target_os = "macos") {
                Command::new("defaults").args(["read", "-g", "AppleLanguages"]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default()
            } else {
                String::new()
            };
            env.starts_with("ru") || mac.contains("\"ru")
        }
        _ => false,
    }
}

fn refresh_tray(app: &AppHandle) {
    // events can arrive before setup has registered the state
    let Some(st) = app.try_state::<App>() else { return };
    let e = st.engine.read().unwrap_or_else(|e| e.into_inner()).clone();
    let (tasks, settings) = {
        let s = lk(e.store());
        (s.tasks(), s.settings())
    };
    let (running, waiting) = counts(&tasks);
    let label = tray_label(running, waiting);
    let ru = st.ru.load(Ordering::Relaxed);
    if let Some(t) = lk(&st.tray).as_ref() {
        if cfg!(target_os = "macos") {
            let _ = t.icon.set_title(if label.is_empty() { None } else { Some(label.as_str()) });
        }
        let tip = if label.is_empty() { "Nightshift".to_string() } else { format!("Nightshift: {label}") };
        let _ = t.icon.set_tooltip(Some(tip));
        let idle = if ru { "Ничего не идёт" } else { "Idle" };
        let _ = t.status.set_text(if label.is_empty() { idle } else { label.as_str() });
        let text = match (settings.paused, ru) {
            (true, true) => "Снять паузу",
            (true, false) => "Resume all",
            (false, true) => "Пауза всего",
            (false, false) => "Pause all",
        };
        let _ = t.pause.set_text(text);
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_title(&if waiting > 0 { format!("Nightshift ({waiting})") } else { "Nightshift".into() });
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

fn quit(app: &AppHandle) {
    let app = app.clone();
    // shutdown waits for agent processes, so keep it off the UI thread
    std::thread::spawn(move || {
        let e = app.state::<App>().engine.read().unwrap_or_else(|e| e.into_inner()).clone();
        e.shutdown();
        app.exit(0);
    });
}

// ---------- notifications ----------

/// notification id -> task, so a click can open the task (the last 200 are kept)
static NOTES: Mutex<Vec<(i32, String)>> = Mutex::new(Vec::new());
static NOTE_ID: AtomicI32 = AtomicI32::new(1);

struct SystemNotifier(AppHandle);
impl Notifier for SystemNotifier {
    fn send(&self, n: &Notice) {
        let id = NOTE_ID.fetch_add(1, Ordering::Relaxed);
        {
            let mut v = lk(&NOTES);
            v.push((id, n.task_id.clone()));
            if v.len() > 200 {
                v.remove(0);
            }
        }
        let mut b = self.0.notification().builder().id(id).title(format!("{} · {}", n.project, n.title)).body(&n.body);
        if n.kind.is_loud() {
            b = b.sound("default");
        }
        if let Err(e) = b.show() {
            eprintln!("notification failed: {e}");
        }
    }
}

/// Click on a system notification: show the window and, if the platform says which one, open its task.
fn on_notification_click(app: &AppHandle, a: &tauri_plugin_notification::ActionPerformed) {
    show_window(app);
    let task = a.notification().and_then(|n| lk(&NOTES).iter().find(|(id, _)| *id == n.id()).map(|(_, t)| t.clone()));
    if let Some(t) = task {
        let _ = app.emit("open-task", t);
    }
}

// ---------- engine wiring ----------

fn build_engine(app: &AppHandle, dir: &Path) -> Res<Arc<Engine>> {
    let store = Store::open(dir).map_err(e2s)?;
    let adapters = agent::adapters(&store.settings());
    let e = Engine::new(store, adapters, Arc::new(SystemNotifier(app.clone())), dir.join("worktrees"));
    let rx = e.subscribe();
    let h = app.clone();
    std::thread::spawn(move || {
        for ev in rx {
            let changed = match ev {
                EngineEvent::TaskChanged(t) => h.emit("task-changed", &t).is_ok(),
                EngineEvent::Log(id, ev) => {
                    let _ = h.emit("task-log", json!({ "id": id, "event": ev }));
                    false // log lines do not change the counter
                }
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
                    false
                }
            };
            if changed {
                refresh_tray(&h);
            }
        }
    });
    e.start();
    Ok(e)
}

/// Registers `new` first and drops `old` only afterwards, so a failure leaves the old key working.
fn register_hotkey(app: &AppHandle, new: &str, old: Option<&str>) -> Res<()> {
    let sc: Shortcut = new.parse().map_err(|e| format!("bad hotkey '{new}': {e}"))?;
    let old_sc = old.and_then(|o| o.parse::<Shortcut>().ok());
    if old_sc == Some(sc) {
        return Ok(());
    }
    let gs = app.global_shortcut();
    gs.register(sc).map_err(|e| format!("hotkey '{new}': {e}"))?;
    if let Some(o) = old_sc {
        let _ = gs.unregister(o);
    }
    Ok(())
}

/// GUI apps start with a minimal PATH; agents, gh and node live elsewhere.
fn fix_path() {
    if !cfg!(unix) {
        return;
    }
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let out = Command::new(shell).args(["-ilc", "printf '\\nNSPATH=%s\\n' \"$PATH\""]).stdin(std::process::Stdio::null()).output();
        let _ = tx.send(out);
    });
    let from_shell = rx
        .recv_timeout(std::time::Duration::from_secs(4))
        .ok()
        .and_then(|r| r.ok())
        .and_then(|o| String::from_utf8_lossy(&o.stdout).lines().find_map(|l| l.strip_prefix("NSPATH=").map(str::to_string)))
        .unwrap_or_default();
    let home = std::env::var("HOME").unwrap_or_default();
    let extra = ["/opt/homebrew/bin", "/usr/local/bin"].map(String::from).into_iter().chain([format!("{home}/.local/bin"), format!("{home}/.cargo/bin")]);
    let cur = std::env::var("PATH").unwrap_or_default();
    let mut seen = vec![];
    for p in from_shell.split(':').map(String::from).chain(extra).chain(cur.split(':').map(String::from)) {
        if !p.is_empty() && !seen.contains(&p) {
            seen.push(p);
        }
    }
    std::env::set_var("PATH", seen.join(":"));
}

// ---------- opening things ----------

/// Files that may be opened: plain text, code, images, PDF. Everything else (no extension,
/// .terminal, .webloc, .command, .app, scripts, ...) is refused; folders are only revealed.
const OPENABLE: [&str; 37] = [
    "md", "txt", "json", "toml", "yaml", "yml", "rs", "ts", "tsx", "js", "jsx", "mjs", "py", "go", "java", "c", "h", "cpp", "css", "html", "log",
    "diff", "patch", "csv", "xml", "ini", "lock", "png", "jpg", "jpeg", "gif", "svg", "webp", "pdf", "rb", "php", "sql",
];

#[derive(Debug, PartialEq)]
enum Target {
    Url(String),
    File(PathBuf),
    /// shown in the file manager, never launched
    Dir(PathBuf),
}

/// open_path: http(s), or a plain file / folder inside a registered project or the worktrees dir.
/// The returned path is the canonical one that passed the check; open exactly that.
fn check_openable(target: &str, roots: &[PathBuf]) -> Res<Target> {
    if target.starts_with("https://") || target.starts_with("http://") {
        return Ok(Target::Url(target.to_string()));
    }
    let bad = || format!("cannot open: {target}");
    let p = std::fs::canonicalize(target).map_err(|_| bad())?;
    if !roots.iter().filter_map(|r| std::fs::canonicalize(r).ok()).any(|r| p.starts_with(r)) {
        return Err(bad());
    }
    if p.is_dir() {
        return Ok(Target::Dir(p));
    }
    use std::os::unix::fs::PermissionsExt;
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let executable = p.metadata().map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(true);
    if p.is_file() && OPENABLE.contains(&ext.as_str()) && !executable {
        Ok(Target::File(p))
    } else {
        Err(bad())
    }
}

/// open_terminal / open_editor: an existing local folder inside the same roots (canonical path returned).
fn check_dir(target: &str, roots: &[PathBuf]) -> Res<PathBuf> {
    match check_openable(target, roots)? {
        Target::Dir(p) => Ok(p),
        _ => Err(format!("not a folder: {target}")),
    }
}

fn open_roots(s: &State<App>) -> Vec<PathBuf> {
    let e = eng(s);
    let mut v: Vec<PathBuf> = lk(e.store()).projects().into_iter().map(|p| p.path).collect();
    v.push(s.dir.join("worktrees"));
    v
}

fn opener(target: &Target) -> Res<()> {
    let bin = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let mut c = Command::new(bin);
    match target {
        Target::Url(u) => c.arg(u),
        Target::File(p) => c.arg(p),
        Target::Dir(p) if cfg!(target_os = "macos") => c.arg("-R").arg(p), // reveal, never launch a bundle
        Target::Dir(p) => c.arg(p),
    };
    c.spawn().map(|_| ()).map_err(|e| format!("{bin}: {e}"))
}

fn exists_in_path(bin: &str) -> bool {
    std::env::var("PATH").unwrap_or_default().split(':').any(|d| Path::new(d).join(bin).is_file())
}

/// `$VISUAL` / `$EDITOR` only when it is not a terminal editor.
fn gui_editor() -> Option<Vec<String>> {
    const TUI: [&str; 11] = ["vi", "vim", "nvim", "nano", "emacs", "micro", "hx", "helix", "pico", "ed", "joe"];
    ["VISUAL", "EDITOR"].iter().filter_map(|k| std::env::var(k).ok()).find_map(|v| {
        let parts: Vec<String> = v.split_whitespace().map(String::from).collect();
        let name = Path::new(parts.first()?).file_name()?.to_string_lossy().to_string();
        (!TUI.contains(&name.as_str()) && !parts.contains(&"-nw".to_string())).then_some(parts)
    })
}

// ---------- commands ----------

#[tauri::command]
async fn list_projects(s: State<'_, App>) -> Res<Vec<Project>> {
    Ok(lk(eng(&s).store()).projects())
}

#[tauri::command]
async fn add_project(app: AppHandle, s: State<'_, App>, path: String) -> Res<Project> {
    let p = std::fs::canonicalize(&path).map_err(|e| format!("{path}: {e}"))?;
    let info = git::repo_info(&p);
    if !info.is_repo {
        return Err("not a git repo".into());
    }
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "project".into());
    let mut proj = Project::new(&name, p.clone(), info.base_branch.as_deref().unwrap_or("main"));
    let (check, setup) = git::detect_commands(&p);
    proj.check_cmd = check;
    proj.setup_cmd = setup;
    with_engine(&s, |e| {
        let mut st = lk(e.store());
        if st.projects().iter().any(|x| x.path == p) {
            return Err("already added".to_string());
        }
        st.upsert_project(proj.clone()).map_err(e2s)
    })?;
    let _ = app.emit("project-changed", &proj.id);
    Ok(proj)
}

#[tauri::command]
async fn update_project(app: AppHandle, s: State<'_, App>, project: Project) -> Res<()> {
    let id = project.id.clone();
    with_engine(&s, |e| lk(e.store()).upsert_project(project).map_err(e2s))?;
    let _ = app.emit("project-changed", id);
    Ok(())
}

#[tauri::command]
async fn remove_project(app: AppHandle, s: State<'_, App>, id: String) -> Res<()> {
    with_engine(&s, |e| lk(e.store()).remove_project(&id).map_err(e2s))?;
    let _ = app.emit("project-changed", id);
    Ok(())
}

#[tauri::command]
async fn init_repo(path: String) -> Res<()> {
    git::init_repo(Path::new(&path)).map_err(e2s)
}

fn project_of(s: &State<App>, id: &str) -> Res<Project> {
    lk(eng(s).store()).project(&id.to_string()).ok_or_else(|| format!("project not found: {id}"))
}

#[tauri::command]
async fn repo_info(s: State<'_, App>, id: String) -> Res<Value> {
    let i = git::repo_info(&project_of(&s, &id)?.path);
    Ok(json!({ "is_repo": i.is_repo, "base_branch": i.base_branch, "current_branch": i.current_branch, "dirty": i.dirty,
        "origin_url": i.origin_url, "web_url": i.web_url, "ahead": i.ahead, "behind": i.behind }))
}

#[tauri::command]
async fn list_tasks(s: State<'_, App>) -> Res<Vec<Task>> {
    Ok(lk(eng(&s).store()).tasks())
}

#[tauri::command]
async fn get_log(s: State<'_, App>, id: String, limit: usize) -> Res<Value> {
    Ok(json!(lk(eng(&s).store()).read_log(&id, limit)))
}

#[tauri::command]
async fn submit_task(s: State<'_, App>, project_id: String, prompt: String, agent: Option<String>) -> Res<String> {
    with_engine(&s, |e| e.submit(NewTask { project_id, prompt, agent, ..NewTask::default() }))
}

#[tauri::command]
async fn answer(s: State<'_, App>, id: String, text: String) -> Res<()> {
    with_engine(&s, |e| e.answer(&id, &text))
}

#[tauri::command]
async fn approve_outward(s: State<'_, App>, id: String, yes: bool) -> Res<()> {
    with_engine(&s, |e| e.approve_outward(&id, yes))
}

#[tauri::command]
async fn cancel(s: State<'_, App>, id: String) -> Res<()> {
    with_engine(&s, |e| e.cancel(&id))
}

#[tauri::command]
async fn retry(s: State<'_, App>, id: String) -> Res<()> {
    with_engine(&s, |e| e.retry(&id))
}

#[tauri::command]
async fn remove_task(app: AppHandle, s: State<'_, App>, id: String) -> Res<()> {
    with_engine(&s, |e| e.remove(&id))?;
    let _ = app.emit("task-removed", &id);
    refresh_tray(&app);
    Ok(())
}

#[tauri::command]
async fn undo(s: State<'_, App>, id: String) -> Res<()> {
    with_engine(&s, |e| e.undo(&id))
}

#[tauri::command]
async fn push_base(s: State<'_, App>, project_id: String) -> Res<()> {
    with_engine(&s, |e| e.push_base(&project_id))
}

#[tauri::command]
async fn pull_base(s: State<'_, App>, project_id: String) -> Res<()> {
    with_engine(&s, |e| e.pull_base(&project_id))
}

#[tauri::command]
async fn pause_all(app: AppHandle, s: State<'_, App>, paused: bool) -> Res<()> {
    with_engine(&s, |e| e.pause_all(paused));
    refresh_tray(&app);
    Ok(())
}

#[tauri::command]
async fn list_agents(s: State<'_, App>) -> Res<Value> {
    let settings = lk(eng(&s).store()).settings();
    Ok(json!(agent::adapters(&settings)
        .iter()
        .map(|a| {
            let r = a.available();
            json!({ "id": a.id(), "name": a.name(), "available": r.is_ok(), "hint": r.err().unwrap_or_default(), "guarded": a.guarded() })
        })
        .collect::<Vec<_>>()))
}

#[tauri::command]
async fn get_settings(s: State<'_, App>) -> Res<Settings> {
    Ok(lk(eng(&s).store()).settings())
}

#[tauri::command]
async fn set_settings(app: AppHandle, s: State<'_, App>, settings: Settings) -> Res<()> {
    let _one_at_a_time = lk(&s.settings_lock);
    let old = with_engine(&s, |e| -> Res<Settings> {
        let old = lk(e.store()).settings();
        // new key first: if it cannot be registered nothing is saved and the old key keeps working
        let hotkey_changed = settings.hotkey != old.hotkey;
        if hotkey_changed {
            register_hotkey(&app, &settings.hotkey, Some(&old.hotkey))?;
        }
        if let Err(x) = lk(e.store()).set_settings(settings.clone()) {
            if hotkey_changed {
                let _ = register_hotkey(&app, &old.hotkey, Some(&settings.hotkey));
            }
            return Err(e2s(x));
        }
        Ok(old)
    })?;
    // running tasks keep the adapter they started with; the next runs see the new commands
    if json!(settings.custom_agents) != json!(old.custom_agents) || json!(settings.envs) != json!(old.envs) {
        with_engine(&s, |e| e.set_adapters(agent::adapters(&settings)));
    }
    s.ru.store(is_ru(&settings), Ordering::Relaxed);
    refresh_tray(&app);
    Ok(())
}

#[tauri::command]
async fn github_info(s: State<'_, App>, project_id: String) -> Res<Value> {
    let p = project_of(&s, &project_id)?;
    let gh = match git::gh_status(&p.path) {
        Ok(u) => json!({ "ghUser": u }),
        Err(e) => json!({ "ghHint": e.to_string() }),
    };
    let branches: Vec<Value> = git::list_ns_branches(&p.path).into_iter().map(|(name, merged)| json!({ "name": name, "merged": merged })).collect();
    let mut out = gh;
    out["repoUrl"] = json!(git::repo_info(&p.path).web_url);
    out["branches"] = json!(branches);
    Ok(out)
}

#[tauri::command]
async fn delete_merged_branches(s: State<'_, App>, project_id: String) -> Res<u32> {
    git::delete_merged_ns_branches(&project_of(&s, &project_id)?.path).map_err(e2s)
}

#[tauri::command]
async fn pr_state(s: State<'_, App>, task_id: String) -> Res<String> {
    let e = eng(&s);
    let t = lk(e.store()).task(&task_id).ok_or("task not found")?;
    let url = t.outcome.pr_url.ok_or("no PR for this task")?;
    git::pr_state(&project_of(&s, &t.project_id)?.path, &url).map_err(e2s)
}

/// `git --numstat` lines: `<ins>\t<del>\t<path>`; binary files show `-`.
fn parse_numstat(out: &str) -> Vec<Value> {
    out.lines()
        .filter_map(|l| {
            let mut p = l.splitn(3, '\t');
            let (a, d, path) = (p.next()?, p.next()?, p.next()?);
            Some(json!({ "path": path, "insertions": a.parse::<u32>().unwrap_or(0), "deletions": d.parse::<u32>().unwrap_or(0) }))
        })
        .collect()
}

#[tauri::command]
async fn get_diffstat(s: State<'_, App>, task_id: String) -> Res<Vec<Value>> {
    let t = lk(eng(&s).store()).task(&task_id).ok_or("task not found")?;
    let p = project_of(&s, &t.project_id)?;
    if let Some(wt) = t.worktree.as_ref().filter(|w| w.exists()) {
        return Ok(git::diffstat(wt, &p.base_branch)
            .into_iter()
            .map(|f| json!({ "path": f.path, "insertions": f.insertions, "deletions": f.deletions }))
            .collect());
    }
    // merged task: its worktree is gone, the squash commit on base has the changes
    let Some(sha) = t.outcome.commit.as_deref().filter(|c| c.chars().all(|ch| ch.is_ascii_hexdigit())) else { return Ok(vec![]) };
    let out = Command::new("git").current_dir(&p.path).args(["show", "--numstat", "--format=", "--no-renames", sha]).output().map_err(e2s)?;
    Ok(parse_numstat(&String::from_utf8_lossy(&out.stdout)))
}

#[tauri::command]
async fn open_path(s: State<'_, App>, path: String) -> Res<()> {
    let t = check_openable(&path, &open_roots(&s))?;
    opener(&t)
}

#[tauri::command]
async fn open_editor(s: State<'_, App>, path: String) -> Res<()> {
    let dir = check_dir(&path, &open_roots(&s))?;
    let path = dir.to_string_lossy().to_string();
    if let Some(mut parts) = gui_editor() {
        let bin = parts.remove(0);
        return Command::new(bin).args(parts).arg(&path).spawn().map(|_| ()).map_err(e2s);
    }
    if exists_in_path("code") {
        return Command::new("code").arg(&path).spawn().map(|_| ()).map_err(e2s);
    }
    if cfg!(target_os = "macos") && Command::new("open").args(["-a", "Visual Studio Code", &path]).status().map(|s| s.success()).unwrap_or(false) {
        return Ok(());
    }
    opener(&Target::Dir(dir))
}

#[tauri::command]
async fn open_terminal(s: State<'_, App>, path: String) -> Res<()> {
    let path = check_dir(&path, &open_roots(&s))?.to_string_lossy().to_string();
    if cfg!(target_os = "macos") {
        return Command::new("open").args(["-a", "Terminal", &path]).spawn().map(|_| ()).map_err(e2s);
    }
    let env_term = std::env::var("TERMINAL").ok().filter(|t| !t.is_empty());
    for bin in env_term.iter().map(String::as_str).chain(["x-terminal-emulator", "kitty", "gnome-terminal", "konsole"]) {
        let mut c = Command::new(bin);
        c.current_dir(&path);
        match bin.rsplit('/').next().unwrap_or(bin) {
            "kitty" => c.args(["--directory", &path]),
            "gnome-terminal" => c.arg(format!("--working-directory={path}")),
            "konsole" => c.args(["--workdir", &path]),
            _ => &mut c,
        };
        if c.spawn().is_ok() {
            return Ok(());
        }
    }
    Err("no terminal found (set $TERMINAL)".into())
}

fn main() {
    fix_path();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show_window(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, ev| {
                    if ev.state() == ShortcutState::Pressed {
                        show_window(app);
                        let _ = app.emit("focus-input", ());
                    }
                })
                .build(),
        )
        .setup(|app| {
            let h = app.handle().clone();
            let dir = h.path().app_data_dir()?;
            let engine = build_engine(&h, &dir)?;
            let settings = lk(engine.store()).settings();
            app.manage(App { engine: RwLock::new(engine), dir, tray: Mutex::new(None), settings_lock: Mutex::new(()), ru: AtomicBool::new(false) });

            let ru = is_ru(&settings);
            app.state::<App>().ru.store(ru, Ordering::Relaxed);
            let t = |ru_s: &'static str, en: &'static str| if ru { ru_s } else { en };
            let open = MenuItem::with_id(app, "open", t("Открыть", "Open"), true, None::<&str>)?;
            let new = MenuItem::with_id(app, "new", t("Новый чат", "New chat"), true, None::<&str>)?;
            let pause = MenuItem::with_id(app, "pause", t("Пауза всего", "Pause all"), true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", t("Выход", "Quit"), true, None::<&str>)?;
            let status = MenuItem::with_id(app, "status", t("Ничего не идёт", "Idle"), false, None::<&str>)?;
            let menu = Menu::new(app)?;
            if cfg!(target_os = "linux") {
                // Linux trays have no tooltip: the counter is the first (disabled) menu line
                menu.append(&status)?;
                menu.append(&PredefinedMenuItem::separator(app)?)?;
            }
            menu.append_items(&[&open, &new, &pause, &PredefinedMenuItem::separator(app)?, &quit_i])?;
            let mut tb = TrayIconBuilder::with_id("main").menu(&menu).tooltip("Nightshift").on_menu_event(|app, ev| match ev.id().as_ref() {
                "open" => show_window(app),
                "new" => {
                    show_window(app);
                    let _ = app.emit("focus-input", ());
                }
                "pause" => {
                    let e = app.state::<App>().engine.read().unwrap_or_else(|e| e.into_inner()).clone();
                    let paused = lk(e.store()).settings().paused;
                    e.pause_all(!paused);
                    refresh_tray(app);
                }
                "quit" => quit(app),
                _ => {}
            });
            if let Some(icon) = app.default_window_icon() {
                tb = tb.icon(icon.clone());
            }
            let tray = tb.build(app)?;
            *lk(&app.state::<App>().tray) = Some(Tray { icon: tray, pause, status });

            h.notification().on_action({
                let h = h.clone();
                move |a| on_notification_click(&h, a)
            }).ok();
            if let Err(e) = register_hotkey(&h, &settings.hotkey, None) {
                eprintln!("hotkey not registered: {e}");
            }
            refresh_tray(&h);
            Ok(())
        })
        .on_window_event(|w, ev| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = ev {
                api.prevent_close();
                let _ = w.hide();
                #[cfg(target_os = "macos")]
                let _ = w.app_handle().set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_projects, add_project, update_project, remove_project, init_repo, repo_info,
            list_tasks, get_log, submit_task, answer, approve_outward,
            cancel, retry, remove_task, undo, push_base, pull_base, pause_all,
            list_agents, get_settings, set_settings,
            github_info, delete_merged_branches, pr_state, get_diffstat,
            open_path, open_editor, open_terminal
        ])
        .build(tauri::generate_context!())
        .expect("error while building Nightshift");
    app.run(|_, ev| {
        // the window only hides; the process ends via tray "Quit"
        if let tauri::RunEvent::ExitRequested { code: None, api, .. } = ev {
            api.prevent_exit();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_label_shows_only_nonzero_parts() {
        assert_eq!(tray_label(2, 1), "2 ▶ 1 ?");
        assert_eq!(tray_label(0, 3), "3 ?");
        assert_eq!(tray_label(1, 0), "1 ▶");
        assert_eq!(tray_label(0, 0), "");
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ns-app-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.canonicalize().unwrap()
    }

    #[test]
    fn open_allows_web_links_and_listed_file_types_inside_projects_only() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch("open");
        let outside = scratch("open-outside");
        for f in ["notes.txt", "run.sh", "tool", "tool.txt", "a.terminal", "a.webloc", "a.fileloc", "a.inetloc", "noext"] {
            std::fs::write(root.join(f), "x").unwrap();
        }
        std::fs::set_permissions(root.join("tool.txt"), std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::create_dir(root.join("Thing.app")).unwrap();
        std::fs::write(outside.join("notes.txt"), "x").unwrap();
        std::os::unix::fs::symlink(outside.join("notes.txt"), root.join("link.txt")).unwrap();
        let roots = vec![root.clone()];
        let p = |n: &str| root.join(n).to_string_lossy().to_string();

        assert_eq!(check_openable("https://github.com/o/r", &roots), Ok(Target::Url("https://github.com/o/r".into())));
        assert_eq!(check_openable(&p("notes.txt"), &roots), Ok(Target::File(root.join("notes.txt"))));
        assert_eq!(check_openable(&root.to_string_lossy(), &roots), Ok(Target::Dir(root.clone())));
        // a folder named like a bundle is only revealed, not launched
        assert_eq!(check_openable(&p("Thing.app"), &roots), Ok(Target::Dir(root.join("Thing.app"))));
        for bad in [p("run.sh"), p("tool"), p("tool.txt"), p("a.terminal"), p("a.webloc"), p("a.fileloc"), p("a.inetloc"), p("noext"),
                    p("link.txt"), outside.join("notes.txt").to_string_lossy().to_string(),
                    "/".into(), "file:///etc/passwd".into(), "javascript:alert(1)".into(), "-a Calculator".into(), p("missing")] {
            assert!(check_openable(&bad, &roots).is_err(), "{bad}");
        }
    }

    #[test]
    fn terminal_and_editor_take_only_existing_folders() {
        let root = scratch("dir");
        std::fs::write(root.join("a.txt"), "x").unwrap();
        let roots = vec![root.clone()];
        assert_eq!(check_dir(&root.to_string_lossy(), &roots), Ok(root.clone()));
        assert!(check_dir(&root.join("a.txt").to_string_lossy(), &roots).is_err());
        assert!(check_dir("https://example.com", &roots).is_err());
    }

    #[test]
    fn numstat_lines_become_file_stats() {
        let s = parse_numstat("3\t1\tsrc/a.rs\n-\t-\timg/logo.png\n");
        assert_eq!(s, vec![json!({"path":"src/a.rs","insertions":3,"deletions":1}), json!({"path":"img/logo.png","insertions":0,"deletions":0})]);
    }
}
