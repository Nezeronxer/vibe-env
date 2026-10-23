//! F15 (#10) the app preview and (#3) the proof shots. The preview is a window of its own (label `preview`) on an
//! address `core::preview::local_url` lets through: no capability names it, so the page cannot call the app (remote
//! origin, Tauri refuses its IPC); it cannot navigate, open windows or download outside localhost. "Указать элемент"
//! puts a script into that window only after the Rust side saw a local address, and reads the pick back by polling.
use crate::hub::{e2s, lk, Res};
use crate::App;
use nightshift_core::preview;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, State, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const LABEL: &str = "preview";
const PICK_FOR: Duration = Duration::from_secs(120);

/// The press of "Указать элемент" that is waited for: a newer press ends the wait of the one before.
#[derive(Default)]
pub struct Picker(AtomicU64);

/// The page side of "Указать элемент": an outline under the pointer, the first click (or Esc) ends it.
const PICK_JS: &str = r#"(() => {
  if (window.__nsPickOff) window.__nsPickOff();
  window.__nsPick = null;
  const box = document.createElement('div');
  box.style.cssText = 'position:fixed;pointer-events:none;z-index:2147483647;border:2px solid #f2a33a;background:rgba(242,163,58,.14);border-radius:3px;display:none';
  document.documentElement.appendChild(box);
  const esc = (s) => (window.CSS && CSS.escape ? CSS.escape(s) : s);
  const sel = (el) => {
    const parts = [];
    for (let e = el; e && e.nodeType === 1 && parts.length < 5; e = e.parentElement) {
      if (e.id) { parts.unshift('#' + esc(e.id)); break; }
      let s = e.localName + [...e.classList].slice(0, 2).map((c) => '.' + esc(c)).join('');
      const same = e.parentElement ? [...e.parentElement.children].filter((x) => x.localName === e.localName) : [];
      if (same.length > 1) s += ':nth-of-type(' + (same.indexOf(e) + 1) + ')';
      parts.unshift(s);
    }
    return parts.join(' > ');
  };
  const stop = (e) => { e.preventDefault(); e.stopPropagation(); };
  const over = (e) => { const r = e.target.getBoundingClientRect(); Object.assign(box.style, { display: 'block', left: r.left + 'px', top: r.top + 'px', width: r.width + 'px', height: r.height + 'px' }); };
  const click = (e) => {
    stop(e);
    const el = e.target, r = el.getBoundingClientRect();
    window.__nsPick = JSON.stringify({ selector: sel(el), text: String(el.innerText || el.value || el.getAttribute('aria-label') || el.getAttribute('alt') || '').slice(0, 400), w: r.width, h: r.height });
    off();
  };
  const key = (e) => { if (e.key === 'Escape') { stop(e); window.__nsPick = ''; off(); } };
  const evs = [['mouseover', over], ['pointerdown', stop], ['mousedown', stop], ['mouseup', stop], ['click', click], ['keydown', key]];
  const off = () => { for (const [n, f] of evs) removeEventListener(n, f, true); box.remove(); document.documentElement.style.cursor = ''; window.__nsPickOff = null; };
  for (const [n, f] of evs) addEventListener(n, f, true);
  document.documentElement.style.cursor = 'crosshair';
  window.__nsPickOff = off;
})();"#;
/// Takes the pick once: null = not yet, "" = Esc, otherwise the JSON the page script wrote.
const TAKE_JS: &str = "(() => { const p = window.__nsPick; if (p === null || p === undefined) return null; window.__nsPick = undefined; return String(p); })()";

/// The origin of the app's own pages (in `tauri dev` it can be an http://localhost one): never a preview.
fn app_origin(app: &AppHandle) -> String {
    app.get_webview_window("main").and_then(|w| w.url().ok()).map(|u| u.origin().ascii_serialization()).unwrap_or_default()
}

/// What the preview window may show: a local address that is not the app itself, or a blank page.
fn allowed(u: &Url, app_origin: &str) -> bool {
    u.as_str() == "about:blank" || (preview::local_url(u.as_str()).is_ok() && u.origin().ascii_serialization() != app_origin)
}

/// The preview window, only while it shows a local page.
fn window(app: &AppHandle) -> Res<WebviewWindow> {
    let w = app.get_webview_window(LABEL).ok_or("preview_closed")?;
    let u = w.url().map_err(e2s)?;
    allowed(&u, &app_origin(app)).then_some(w).ok_or_else(|| preview::NOT_LOCAL.to_string())
}

/// Opens (or points) the preview window at `url`; the normalized address. Err `not_local_url` for anything else.
#[tauri::command]
pub async fn preview_open(app: AppHandle, url: String) -> Res<String> {
    let url = preview::local_url(&url).map_err(str::to_string)?;
    let u: Url = url.parse().map_err(|_| preview::NOT_LOCAL.to_string())?;
    let own = app_origin(&app);
    if !allowed(&u, &own) {
        return Err(preview::NOT_LOCAL.into());
    }
    if let Some(w) = app.get_webview_window(LABEL) {
        w.navigate(u).map_err(e2s)?;
        let _ = w.set_focus();
        return Ok(url);
    }
    WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::External(u))
        .title("Nightshift - Preview")
        .inner_size(1280.0, 800.0)
        .on_navigation(move |u| allowed(u, &own))
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_download(|_, _| false)
        .build()
        .map_err(e2s)?;
    Ok(url)
}

#[tauri::command]
pub async fn preview_reload(app: AppHandle) -> Res<()> {
    window(&app)?.reload().map_err(e2s)
}

/// Width of the page: 390 (phone), 768 (tablet), anything else = desktop 1280.
#[tauri::command]
pub async fn preview_width(app: AppHandle, px: Option<u32>) -> Res<()> {
    let (w, h) = match px {
        Some(390) => (390.0, 844.0),
        Some(768) => (768.0, 1024.0),
        _ => (1280.0, 800.0),
    };
    let win = window(&app)?;
    win.set_size(LogicalSize::new(w, h)).map_err(e2s)?;
    let _ = win.set_focus();
    Ok(())
}

#[tauri::command]
pub async fn preview_close(app: AppHandle) -> Res<()> {
    if let Some(w) = app.get_webview_window(LABEL) {
        w.close().map_err(e2s)?;
    }
    Ok(())
}

/// "Указать элемент": the next click in the preview comes to the main window as `preview-pick {selector, text, w, h}`.
#[tauri::command]
pub async fn preview_pick(app: AppHandle, picker: State<'_, Picker>) -> Res<()> {
    let w = window(&app)?;
    w.eval(PICK_JS).map_err(e2s)?;
    let gen = picker.0.fetch_add(1, Ordering::SeqCst) + 1;
    let _ = w.set_focus();
    std::thread::spawn(move || wait_pick(app, gen));
    Ok(())
}

fn wait_pick(app: AppHandle, gen: u64) {
    let start = Instant::now();
    while start.elapsed() < PICK_FOR {
        std::thread::sleep(Duration::from_millis(300));
        if app.state::<Picker>().0.load(Ordering::SeqCst) != gen {
            return;
        }
        // closed, or gone to a page that is not local: the pick is over
        let Ok(w) = window(&app) else { return };
        let (tx, rx) = mpsc::channel();
        if w.eval_with_callback(TAKE_JS, move |r| drop(tx.send(r))).is_err() {
            return;
        }
        let Ok(r) = rx.recv_timeout(Duration::from_secs(2)) else { continue };
        match serde_json::from_str::<Option<String>>(&r) {
            Ok(Some(s)) if s.is_empty() => return,
            Ok(Some(s)) => {
                if let Some(p) = preview::parse_pick(&s) {
                    let _ = app.emit_to("main", "preview-pick", p);
                }
                return;
            }
            _ => {}
        }
    }
}

/// One proof screenshot of a task as a `data:` URL (the window's CSP takes `img-src data:`, no file access).
/// Only a file the task lists in `outcome.proof.shots`.
#[tauri::command]
pub async fn proof_shot(s: State<'_, App>, task_id: String, file: String) -> Res<String> {
    let t = lk(s.hub.engine.store()).task(&task_id).ok_or("task_not_found")?;
    let listed = t.outcome.proof.as_ref().is_some_and(|p| p.shots.contains(&file));
    if !listed || file.contains(['/', '\\']) || file.starts_with('.') {
        return Err("shot_not_found".into());
    }
    let bytes = std::fs::read(s.hub.proof_dir().join(&t.id).join(&file)).map_err(|_| "shot_not_found".to_string())?;
    Ok(format!("data:image/png;base64,{}", b64(&bytes)))
}

fn b64(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            s.push(if i <= c.len() { A[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_rfc_examples() {
        for (i, o) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")] {
            assert_eq!(b64(i.as_bytes()), o, "{i}");
        }
    }

    #[test]
    fn the_preview_may_show_only_local_pages_that_are_not_the_app() {
        let ok = |s: &str, own: &str| allowed(&s.parse().unwrap(), own);
        assert!(ok("http://localhost:3000/a", "tauri://localhost") && ok("http://127.0.0.1:5173/", "null") && ok("about:blank", ""));
        for bad in ["https://example.com/", "http://localhost.evil.com/", "file:///etc/passwd", "tauri://localhost/index.html", "http://ipc.localhost/x", "data:text/html,hi"] {
            assert!(!ok(bad, "tauri://localhost"), "{bad}");
        }
        // `tauri dev` serves the app on a local port: that origin is the app, not a preview
        assert!(!ok("http://localhost:1430/index.html", "http://localhost:1430"));
    }

    /// No capability names the preview window, and none opens the IPC to remote pages: the page there calls nothing.
    #[test]
    fn no_capability_reaches_the_preview_window() {
        for f in [include_str!("../capabilities/default.json"), include_str!("../capabilities/windows.json")] {
            let c: serde_json::Value = serde_json::from_str(f).unwrap();
            assert!(c.get("remote").is_none(), "{c}");
            for w in c["windows"].as_array().unwrap().iter().filter_map(|w| w.as_str()) {
                let hit = w == LABEL || w == "*" || w.strip_suffix('*').is_some_and(|p| LABEL.starts_with(p));
                assert!(!hit, "capability window pattern {w} would match {LABEL}");
            }
        }
    }
}
