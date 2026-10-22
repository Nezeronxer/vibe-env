//! `term_*` commands: PTY sessions of `core::term`, their output to the window through a raw channel (no JSON on the
//! bytes), one window attached per session; closing a window only detaches it (story 301).
use crate::hub::{e2s, lk, Res};
use nightshift_core::term::{TermChunk, TermInfo, TermSaved, TermSpec, Terms};
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, State, Window};

#[derive(Default)]
pub struct Pty {
    pub terms: Terms,
    /// session -> label of the window it is attached to
    owners: Mutex<HashMap<String, String>>,
}

impl Pty {
    /// Sessions whose process still runs (the tray counter, the quit question).
    pub fn live(&self) -> usize {
        self.terms.list().iter().filter(|t| t.running).count()
    }

    /// A window is gone: its sessions keep running, detached.
    pub fn window_gone(&self, label: &str) {
        let mut o = lk(&self.owners);
        for (id, _) in o.iter().filter(|(_, l)| *l == label) {
            let _ = self.terms.detach(id);
        }
        o.retain(|_, l| l != label);
    }

    /// At quit: the folders and text go to `file`, the processes end (story 253, 303).
    pub fn save_and_end(&self, file: &Path) {
        if let Ok(text) = serde_json::to_string(&self.terms.snapshot()) {
            let _ = std::fs::write(file, text);
        }
        for t in self.terms.list() {
            let _ = self.terms.close(&t.id);
        }
    }
}

/// What the last run left (empty when nothing or unreadable).
pub fn load_saved(file: &Path) -> Vec<TermSaved> {
    std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn send(app: &AppHandle, ch: &Channel<InvokeResponseBody>, id: &str, c: TermChunk) {
    match c {
        TermChunk::Data(d) => {
            let _ = ch.send(InvokeResponseBody::Raw(d));
        }
        TermChunk::Exit(code) => {
            let _ = ch.send(InvokeResponseBody::Json(json!({ "exit": code }).to_string()));
            let _ = app.emit("term-exit", json!({ "id": id, "code": code }));
            crate::refresh_tray(app);
        }
    }
}

#[tauri::command]
pub async fn term_open(app: AppHandle, p: State<'_, Pty>, spec: TermSpec) -> Res<String> {
    let id = p.terms.open(spec).map_err(e2s)?;
    crate::refresh_tray(&app);
    Ok(id)
}

#[tauri::command]
pub async fn term_write(p: State<'_, Pty>, id: String, data: String) -> Res<()> {
    p.terms.write(&id, data.as_bytes()).map_err(e2s)
}

#[tauri::command]
pub async fn term_resize(p: State<'_, Pty>, id: String, cols: u16, rows: u16) -> Res<()> {
    p.terms.resize(&id, cols, rows).map_err(e2s)
}

#[tauri::command]
pub async fn term_close(app: AppHandle, p: State<'_, Pty>, id: String) -> Res<()> {
    lk(&p.owners).remove(&id);
    p.terms.close(&id).map_err(e2s)?;
    crate::refresh_tray(&app);
    Ok(())
}

/// The buffer first, then the live output, in order, all through `channel`: raw bytes, and `{"exit": code}` at the end.
#[tauri::command]
pub async fn term_attach(app: AppHandle, window: Window, p: State<'_, Pty>, id: String, channel: Channel<InvokeResponseBody>) -> Res<()> {
    // pieces that come while the buffer is being taken wait until it has gone out
    let held: Arc<Mutex<Option<Vec<TermChunk>>>> = Arc::new(Mutex::new(Some(vec![])));
    let (h, ch, a, tid) = (held.clone(), channel.clone(), app.clone(), id.clone());
    let buf = p
        .terms
        .attach(
            &id,
            Box::new(move |c| match lk(&h).as_mut() {
                Some(q) => q.push(c),
                None => send(&a, &ch, &tid, c),
            }),
        )
        .map_err(e2s)?;
    lk(&p.owners).insert(id.clone(), window.label().to_string());
    let mut h = lk(&held);
    let _ = channel.send(InvokeResponseBody::Raw(buf));
    for c in h.take().unwrap_or_default() {
        send(&app, &channel, &id, c);
    }
    Ok(())
}

#[tauri::command]
pub async fn term_detach(p: State<'_, Pty>, id: String) -> Res<()> {
    lk(&p.owners).remove(&id);
    p.terms.detach(&id).map_err(e2s)
}

#[tauri::command]
pub async fn term_ack(p: State<'_, Pty>, id: String, bytes: usize) -> Res<()> {
    p.terms.ack(&id, bytes).map_err(e2s)
}

#[tauri::command]
pub async fn term_list(p: State<'_, Pty>) -> Res<Vec<TermInfo>> {
    Ok(p.terms.list())
}
