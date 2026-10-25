//! v6 (task 29): the chat as a shell of the CLI. Commands: `chat_shell{env, projectId?}` (what the menu "/" shows and
//! what the env can do: images, continuing a session), `chat_attach{id?, data}` (a pasted image, base64),
//! `chat_attach_path{id?, path}` (a file dropped on the field), `chat_detach{path}` (×), `chat_terminal{id, fresh?,
//! cols?, rows?}` (the chat's session in a terminal tab: a PTY of `core::term` opened here, so the app knows when it
//! ends and gives the chat back with what was said there).
use crate::hub::Res;
use crate::ptycmds::Pty;
use crate::App;
use nightshift_core::chat::{from_base64, Attached};
use nightshift_core::term::TermSpec;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

/// How often the app looks whether a chat's terminal has ended.
const WATCH_EVERY: Duration = Duration::from_millis(700);

/// The menu "/" of an env for a project (the person's folders only, when there is no project), and what the env can do.
#[tauri::command]
pub async fn chat_shell(s: State<'_, App>, env: String, project_id: Option<String>) -> Res<Value> {
    let a = s.hub.adapter(&env).ok_or_else(|| format!("agent_not_found: {env}"))?;
    let cwd = project_id.and_then(|id| s.hub.project(&id).ok()).map(|p| p.path).unwrap_or_default();
    let inv = if cwd.as_os_str().is_empty() { nightshift_core::agent::Inventory::default() } else { a.inventory(&cwd) };
    let resume = a.resume_argv("probe-session", Path::new("/")).is_some();
    Ok(json!({ "inventory": inv, "images": a.supports_images(), "resume": resume, "checked": a.resume_checked() }))
}

/// A pasted image (the window sends its bytes as base64); `id` None = a chat not made yet.
#[tauri::command]
pub async fn chat_attach(s: State<'_, App>, id: Option<String>, data: String) -> Res<Attached> {
    if data.len() > nightshift_core::chat::MAX_ATTACHMENT_BYTES / 3 * 4 + 1024 {
        return Err("image_too_big".into());
    }
    let bytes = from_base64(&data)?;
    s.hub.chats.attach(id.as_deref(), &bytes)
}

/// A file dropped on the input field: read by the app (the person's file stays where it is). The window gets the kept
/// path and a `data:` URL of the picture for its thumbnail (the CSP allows `img-src data:`, not the file).
#[tauri::command]
pub async fn chat_attach_path(s: State<'_, App>, id: Option<String>, path: PathBuf) -> Res<Value> {
    let a = s.hub.chats.attach_file(id.as_deref(), &path)?;
    let bytes = std::fs::read(&a.path).map_err(|e| e.to_string())?;
    let mime = match nightshift_core::chat::image_ext(&bytes) {
        Some("jpg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "image/png",
    };
    Ok(json!({ "path": a.path, "size": a.size, "preview": format!("data:{mime};base64,{}", to_base64(&bytes)) }))
}

/// Standard base64 with padding (the thumbnail of a dropped picture).
fn to_base64(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                out.push(A[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_round_trips_through_the_cores_reader() {
        for s in [&b""[..], b"h", b"hi", b"hey", b"hello world"] {
            assert_eq!(nightshift_core::chat::from_base64(&super::to_base64(s)).unwrap(), s);
        }
        assert_eq!(super::to_base64(b"hello"), "aGVsbG8=");
    }
}

#[tauri::command]
pub async fn chat_detach(s: State<'_, App>, path: PathBuf) -> Res<()> {
    s.hub.chats.detach(&path)
}

/// "В терминале": the PTY runs the env's CLI with the chat's session in the chat's copy; the window shows it in a tab
/// (`termId`). When it ends (the person quits the CLI, closes the tab), the chat is told (`Chats::terminal_closed`).
#[tauri::command]
pub async fn chat_terminal(app: AppHandle, s: State<'_, App>, p: State<'_, Pty>, id: String, fresh: Option<bool>, cols: Option<u16>, rows: Option<u16>) -> Res<Value> {
    let l = s.hub.chats.terminal_open(&id, fresh.unwrap_or(false))?;
    let title = s.hub.adapter(&l.env).map(|a| a.name().to_string()).unwrap_or_else(|| l.env.clone());
    let spec = TermSpec { cwd: Some(l.cwd.clone()), argv: l.argv.clone(), env: vec![], cols: cols.unwrap_or(100).max(20), rows: rows.unwrap_or(30).max(5), title: title.clone() };
    let term_id = match p.terms.open(spec) {
        Ok(t) => t,
        Err(e) => {
            // the chat must not stay marked for a terminal that never started
            let _ = s.hub.chats.terminal_closed(&id);
            return Err(e.to_string());
        }
    };
    crate::refresh_tray(&app);
    let (a, chat, tid) = (app.clone(), id.clone(), term_id.clone());
    std::thread::spawn(move || loop {
        std::thread::sleep(WATCH_EVERY);
        let live = a.try_state::<Pty>().map(|p| p.terms.list().iter().any(|t| t.id == tid && t.running)).unwrap_or(false);
        if !live {
            if let Some(st) = a.try_state::<App>() {
                let _ = st.hub.chats.terminal_closed(&chat);
            }
            return;
        }
    });
    Ok(json!({ "termId": term_id, "env": l.env, "argv": l.argv, "cwd": l.cwd, "resumed": l.resumed, "checked": l.checked, "title": title }))
}
