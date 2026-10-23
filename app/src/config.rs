//! The config file (stories 293, 294): applied at start and on every change (`config-changed{errors}`); the
//! settings screen writes the keys it changed into it. No file = nothing happens until "Open config" makes one.
use crate::hub::{e2s, lk, Hub, Res};
use crate::App;
use nightshift_core::config::{Config, ConfigError};
use nightshift_core::model::Settings;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

/// How `main` registers the global keys of new settings.
pub type Keys = fn(&AppHandle, Option<&Settings>, &Settings) -> Res<()>;

static ERRORS: Mutex<Vec<ConfigError>> = Mutex::new(Vec::new());

fn config_file() -> Config {
    Config::new(Config::default_path())
}

/// The file over the current settings; saved only when something differs. The file's errors are kept.
fn apply_file(app: &AppHandle, hub: &Hub, keys: Keys) -> Vec<ConfigError> {
    let Some(st) = app.try_state::<App>() else { return vec![] };
    let _one_at_a_time = lk(&st.settings_lock);
    let cur = hub.settings();
    let (new, errors) = match config_file().apply(&cur) {
        Ok(x) => x,
        Err(e) => (cur.clone(), vec![ConfigError { line: 0, key: String::new(), msg: e.to_string() }]),
    };
    let new = serde_json::to_value(&new).unwrap_or_default();
    if new != serde_json::to_value(&cur).unwrap_or_default() {
        if let Err(e) = hub.set_settings(new, &|from, to| keys(app, Some(from), to)) {
            eprintln!("config not applied: {e}");
        }
    }
    *lk(&ERRORS) = errors.clone();
    errors
}

pub fn start(app: &AppHandle, hub: Arc<Hub>, keys: Keys) {
    apply_file(app, &hub, keys);
    hub.on_settings(Box::new(|s| {
        if let Err(e) = config_file().write_settings(s) {
            eprintln!("config not written: {e}");
        }
    }));
    let (app, h) = (app.clone(), hub.clone());
    let watched = config_file().watch(Box::new(move |_| {
        let errors = apply_file(&app, &h, keys);
        let _ = app.emit("config-changed", json!({ "errors": errors }));
    }));
    if let Err(e) = watched {
        eprintln!("config not watched: {e}");
    }
}

/// "Open config": made with commented defaults when missing, then shown in the text editor.
#[tauri::command]
pub async fn config_open() -> Res<Value> {
    let c = config_file();
    let created = c.create_default().map_err(e2s)?;
    let mut open = if cfg!(target_os = "macos") { std::process::Command::new("open") } else { std::process::Command::new("xdg-open") };
    if cfg!(target_os = "macos") {
        open.arg("-t"); // a file without an extension has no app of its own
    }
    open.arg(&c.path).spawn().map_err(e2s)?;
    Ok(json!({ "path": c.path, "created": created }))
}

#[tauri::command]
pub async fn config_errors() -> Res<Value> {
    Ok(json!({ "path": Config::default_path(), "errors": *lk(&ERRORS) }))
}
