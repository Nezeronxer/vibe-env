//! CLI updates and new models (stories 274-277, 299): a check at start and every 6 h, an update in the env's mode
//! (auto by default) once the env has no running launch; models.dev every 6 h, the CLIs' model lists every 24 h
//! and right after an update. Events: `cli-updated{env, from, to}`, `cli-update-failed{env, from, to, error,
//! rolled_back, cmd}`, `cli-update-available{env, from, to, major}`, `new-model{env, id, name, auto}`, `catalog-changed`.
use crate::hub::{e2s, lk, Hub, Res};
use crate::App;
use nightshift_core::models::version_cmp;
use nightshift_core::net::CurlFetch;
use nightshift_core::updates::{Mode, UpdateError, Updates};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

const EVERY: Duration = Duration::from_secs(6 * 3600);
const LISTS_EVERY: Duration = Duration::from_secs(24 * 3600);

static UPDATES: OnceLock<Updates> = OnceLock::new();

fn updates(hub: &Arc<Hub>) -> &'static Updates {
    let u = UPDATES.get_or_init(|| {
        let h = hub.clone();
        let busy = move |env: &str| lk(h.engine.store()).tasks().iter().any(|t| t.agent == env && nightshift_core::engine::is_active(&t.status));
        Updates::new(Box::new(CurlFetch), vec![]).with_dir(hub.dir.join("updates")).with_busy(Arc::new(busy))
    });
    u.set_adapters(lk(&hub.adapters).clone());
    u
}

/// Runs the update and tells the window; true = a new version is in place.
fn run_update(app: &AppHandle, u: &Updates, env: &str) -> bool {
    match u.update(env) {
        Ok(log) if log.ok => {
            let changed = log.from != log.to;
            if changed {
                let _ = app.emit("cli-updated", json!({ "env": env, "from": log.from, "to": log.to }));
            }
            changed
        }
        Ok(log) => {
            let cmd = u.check(env).map(|s| s.cmd.join(" ")).unwrap_or_default();
            let _ = app.emit("cli-update-failed", json!({ "env": env, "from": log.from, "to": log.to, "error": "update_failed", "rolled_back": log.rolled_back, "cmd": cmd }));
            false
        }
        Err(UpdateError::MajorJump { from, to }) => {
            let _ = app.emit("cli-update-available", json!({ "env": env, "from": from, "to": to, "major": true }));
            false
        }
        Err(e) => {
            let cmd = u.check(env).map(|s| s.cmd.join(" ")).unwrap_or_default();
            let _ = app.emit("cli-update-failed", json!({ "env": env, "error": e.to_string(), "rolled_back": false, "cmd": cmd }));
            false
        }
    }
}

fn radar(app: &AppHandle, hub: &Arc<Hub>, u: &Updates, cli_lists: bool) {
    let news = u.models_radar(hub.engine.store(), cli_lists);
    let envs: BTreeSet<&str> = news.iter().map(|n| n.env.as_str()).collect();
    for n in &news {
        let _ = app.emit("new-model", n);
    }
    for env in envs {
        let _ = app.emit("catalog-changed", env);
    }
}

/// One pass: interrupted updates first, then every installed env by its mode, then the models.
fn pass(app: &AppHandle, hub: &Arc<Hub>, told: &Mutex<BTreeSet<String>>, lists: bool) {
    let u = updates(hub);
    let mut updated = false;
    for env in u.interrupted() {
        updated |= run_update(app, u, &env);
    }
    let envs: Vec<String> = lk(&hub.adapters).iter().filter(|a| a.available().is_ok()).map(|a| a.id().to_string()).collect();
    for env in envs {
        let mode = u.mode(&env);
        if mode == Mode::Off {
            continue;
        }
        let Ok(st) = u.check(&env) else { continue };
        // no public latest (Cursor) = the CLI's own auto-update does it
        let (Some(from), Some(to)) = (st.installed.clone(), st.latest.clone()) else { continue };
        if version_cmp(&to, &from).is_le() {
            continue;
        }
        if mode == Mode::Notify || st.major {
            if lk(told).insert(format!("{env}@{to}")) {
                let _ = app.emit("cli-update-available", json!({ "env": env, "from": from, "to": to, "major": st.major }));
            }
            continue;
        }
        updated |= run_update(app, u, &env);
    }
    radar(app, hub, u, lists || updated);
}

pub fn start(app: &AppHandle, hub: Arc<Hub>) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("updates".into()).spawn(move || {
        let told = Mutex::new(BTreeSet::new());
        let mut lists_at: Option<Instant> = None;
        loop {
            let lists = lists_at.is_none_or(|t| t.elapsed() >= LISTS_EVERY);
            if lists {
                lists_at = Some(Instant::now());
            }
            pass(&app, &hub, &told, lists);
            std::thread::sleep(EVERY);
        }
    });
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(e2s)
}

/// "Check now" (story 277): installed, latest, method, the command, and the env's mode.
#[tauri::command]
pub async fn cli_status(s: State<'_, App>, env: String) -> Res<Value> {
    let hub = s.hub.clone();
    blocking(move || {
        let u = updates(&hub);
        let st = u.check(&env).map_err(e2s)?;
        let mut v = serde_json::to_value(st).map_err(e2s)?;
        v["mode"] = json!(u.mode(&env));
        v["updating"] = json!(u.updating(&env));
        Ok(v)
    })
    .await?
}

#[tauri::command]
pub async fn cli_update(app: AppHandle, s: State<'_, App>, env: String) -> Res<Value> {
    let hub = s.hub.clone();
    blocking(move || {
        let u = updates(&hub);
        if run_update(&app, u, &env) {
            radar(&app, &hub, u, true);
        }
        serde_json::to_value(u.history(&env).last()).map_err(e2s)
    })
    .await?
}

#[tauri::command]
pub async fn updates_history(s: State<'_, App>, env: String) -> Res<Value> {
    serde_json::to_value(updates(&s.hub).history(&env)).map_err(e2s)
}

/// "Авто" / "Только сообщать" / "Выкл" of an env (story 274): `auto` | `notify` | `off`.
#[tauri::command]
pub async fn cli_update_mode(s: State<'_, App>, env: String, mode: Mode) -> Res<()> {
    updates(&s.hub).set_mode(&env, mode);
    Ok(())
}
