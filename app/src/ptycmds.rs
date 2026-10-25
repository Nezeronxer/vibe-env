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

    /// `label` attaches to `id`. The window that had it before, if another one, comes back: it must be told it lost the
    /// session (`term-taken`), or its pane shows a frozen terminal that it still thinks is attached.
    fn take(&self, id: &str, label: &str) -> Option<String> {
        lk(&self.owners).insert(id.to_string(), label.to_string()).filter(|old| old != label)
    }

    /// `label` lets go of `id`. False = the session is another window's now, and stays attached to it.
    fn release(&self, id: &str, label: &str) -> bool {
        let mut o = lk(&self.owners);
        if o.get(id).is_some_and(|l| l != label) {
            return false;
        }
        o.remove(id);
        true
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
            let _ = write_private(file, text.as_bytes());
        }
        for t in self.terms.list() {
            let _ = self.terms.close(&t.id);
        }
    }
}

/// Terminal output can hold anything typed or printed: only the owner reads the file (0600, like kit secrets).
/// The old file is removed first, since the mode applies only to a file being created.
fn write_private(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;
    let _ = std::fs::remove_file(file);
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    o.mode(0o600);
    o.open(file)?.write_all(bytes)
}

/// What the last run left (empty when nothing or unreadable); the file is removed once read, so the text does not stay on disk.
pub fn load_saved(file: &Path) -> Vec<TermSaved> {
    let text = std::fs::read_to_string(file).ok();
    let _ = std::fs::remove_file(file);
    text.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
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

/// What the window typed: `data` is text (written as UTF-8), `bytes` are raw bytes (xterm's `onBinary`: a mouse report of
/// the old X10 form is one byte per column, and from the 95th column on that byte is above 127, which UTF-8 would make two).
/// When both come, `bytes` win.
fn typed(data: Option<String>, bytes: Option<Vec<u8>>) -> Vec<u8> {
    bytes.unwrap_or_else(|| data.unwrap_or_default().into_bytes())
}

#[tauri::command]
pub async fn term_write(p: State<'_, Pty>, id: String, data: Option<String>, bytes: Option<Vec<u8>>) -> Res<()> {
    p.terms.write(&id, &typed(data, bytes)).map_err(e2s)
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
    if let Some(old) = p.take(&id, window.label()) {
        let _ = app.emit_to(old.as_str(), "term-taken", json!({ "id": id }));
    }
    let mut h = lk(&held);
    let _ = channel.send(InvokeResponseBody::Raw(buf));
    for c in h.take().unwrap_or_default() {
        send(&app, &channel, &id, c);
    }
    Ok(())
}

#[tauri::command]
pub async fn term_detach(window: Window, p: State<'_, Pty>, id: String) -> Res<()> {
    // a window that lost the session to another one (or hid after that) must not cut the new window off
    if !p.release(&id, window.label()) {
        return Ok(());
    }
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn raw_bytes_go_to_the_terminal_as_they_are_and_text_as_utf8() {
        assert_eq!(typed(Some("é".into()), None), vec![0xC3, 0xA9], "text is UTF-8");
        // an X10 mouse report at column 100: ESC [ M, button, column + 33, row + 33; 133 is above 127
        let report = vec![0x1B, b'[', b'M', 32, 133, 34];
        assert_eq!(typed(None, Some(report.clone())), report);
        assert_eq!(typed(Some("ignored".into()), Some(report.clone())), report, "bytes win over text");
        assert!(typed(None, None).is_empty());
    }

    #[test]
    fn a_session_taken_by_another_window_tells_the_old_one_and_is_not_detached_by_it() {
        let p = Pty::default();
        assert_eq!(p.take("t1", "main"), None, "nobody had it");
        assert_eq!(p.take("t1", "main"), None, "the same window again is no news");
        assert_eq!(p.take("t1", "w-1").as_deref(), Some("main"), "the old window is told");
        assert!(!p.release("t1", "main"), "the old window's detach changes nothing");
        assert!(p.release("t1", "w-1"));
        assert!(p.release("t1", "main"), "nobody has it: nothing to keep");
    }

    /// Audit v4: the snapshot holds terminal output, so only the owner reads it, and it does not outlive the start.
    #[test]
    fn the_terminal_snapshot_is_private_and_gone_after_the_start() {
        let t = crate::testkit::Scratch::new("terms");
        let f = t.0.join("terms.json");
        std::fs::write(&f, "[]").unwrap();
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o644)).unwrap();
        Pty::default().save_and_end(&f);
        assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600);
        assert!(load_saved(&f).is_empty());
        assert!(!f.exists(), "loaded at start, then removed");
    }
}
