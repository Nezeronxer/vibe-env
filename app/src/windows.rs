//! More windows (story 260): the same `index.html?window=<id>`, which asks for the tab it was given; the processes
//! stay in the core, so moving a tab never stops them.
use crate::hub::{e2s, lk, Res};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, Window};

/// Labels of the extra windows start with this (the capability `windows.json` names them by it).
pub const PREFIX: &str = "w-";

#[derive(Default)]
pub struct Windows {
    /// window id -> the tab it shows (what the UI passed: the kind of panel, the PTY session or chat id)
    tabs: Mutex<HashMap<String, Value>>,
}

impl Windows {
    pub fn gone(&self, label: &str) {
        if let Some(id) = label.strip_prefix(PREFIX) {
            lk(&self.tabs).remove(id);
        }
    }
}

#[tauri::command]
pub async fn window_detach(app: AppHandle, w: State<'_, Windows>, tab_spec: Value) -> Res<String> {
    let id = uuid_short();
    lk(&w.tabs).insert(id.clone(), tab_spec);
    WebviewWindowBuilder::new(&app, format!("{PREFIX}{id}"), WebviewUrl::App(format!("index.html?window={id}").into()))
        .title("Nightshift")
        .inner_size(820.0, 560.0)
        .build()
        .map_err(e2s)?;
    let _ = app.emit("windows-changed", ());
    Ok(id)
}

/// The tab a new window was opened with.
#[tauri::command]
pub async fn window_tab(w: State<'_, Windows>, id: String) -> Res<Value> {
    lk(&w.tabs).get(&id).cloned().ok_or_else(|| "window_not_found".into())
}

/// "Вернуть в главное окно": the main window gets the tab back (`window-return`), this window closes.
#[tauri::command]
pub async fn window_return(app: AppHandle, window: Window, tab_spec: Value) -> Res<()> {
    let _ = app.emit_to("main", "window-return", json!({ "tabSpec": tab_spec }));
    crate::show_window(&app);
    if window.label() != "main" {
        if let Some(ww) = app.get_webview_window(window.label()) {
            ww.close().map_err(e2s)?;
        }
    }
    Ok(())
}

/// Window ids only have to differ within one run.
fn uuid_short() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(1);
    N.fetch_add(1, Ordering::Relaxed).to_string()
}
