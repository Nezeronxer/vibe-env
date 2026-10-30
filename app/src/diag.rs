//! v6 (task 28): the app log, the error report, the spending history and the app's own release (G69-G71).
//! Commands: `spend_history{from?, to?}`, `diag_collect{dir?}`, `app_update_state`, `app_update_check`,
//! `app_update_dismiss{version}`. Event: `app-update {latest, url}` when a newer release is found by the daily check.
use crate::hub::{e2s, lk, Hub, Res};
use crate::open;
use crate::App;
use nightshift_core::diag;
use nightshift_core::net::release::{Releases, UpdateCheck};
use nightshift_core::net::CurlFetch;
use nightshift_core::store::SpendRow;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

/// How often the thread looks whether the daily check is due.
const LOOK_EVERY: Duration = Duration::from_secs(3600);

fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn feed(hub: &Hub) -> Option<String> {
    let f = hub.settings().release_feed.unwrap_or_else(|| nightshift_core::net::release::DEFAULT_FEED.into());
    (!f.trim().is_empty()).then_some(f)
}

/// The daily check of the release; the bar is told only about a version it should show. A failure is quiet: it
/// is kept for "О программе" and goes to the app log as a key.
fn check(app: &AppHandle, hub: &Hub) -> UpdateCheck {
    let r = Releases::new(&hub.dir);
    let v = r.check(&CurlFetch, feed(hub).as_deref(), current(), chrono::Local::now());
    match &v.error {
        Some(e) if v.feed => diag::log("warn", &format!("app_update_check_failed: {e}")),
        _ => {}
    }
    if v.show {
        let _ = app.emit("app-update", json!({ "latest": v.latest, "url": v.url }));
    }
    v
}

/// The app log opens, and the release is looked at once a day (only with a feed).
pub fn start(app: &AppHandle, hub: Arc<Hub>) {
    diag::init(&hub.dir);
    diag::log("info", &format!("app_started: version {} {} {}", current(), std::env::consts::OS, std::env::consts::ARCH));
    let app = app.clone();
    let _ = std::thread::Builder::new().name("release".into()).spawn(move || loop {
        if feed(&hub).is_some() && Releases::new(&hub.dir).due(chrono::Local::now()) {
            check(&app, &hub);
        }
        std::thread::sleep(LOOK_EVERY);
    });
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(e2s)
}

fn day(s: Option<String>) -> Res<Option<chrono::NaiveDate>> {
    s.filter(|x| !x.is_empty()).map(|x| chrono::NaiveDate::parse_from_str(&x, "%Y-%m-%d").map_err(|_| "bad_day".to_string())).transpose()
}

/// "Расходы": rows by day (or folded month), project and env, oldest first.
#[tauri::command]
pub async fn spend_history(s: State<'_, App>, from: Option<String>, to: Option<String>) -> Res<Vec<SpendRow>> {
    let (from, to) = (day(from)?, day(to)?);
    Ok(s.hub.engine.spend_history(from, to))
}

/// "Собрать отчёт об ошибке": the zip in `dir` (a folder the person chose) or in Downloads; its place is shown.
#[tauri::command]
pub async fn diag_collect(s: State<'_, App>, dir: Option<String>) -> Res<String> {
    let hub = s.hub.clone();
    let out = match dir.filter(|d| !d.trim().is_empty()) {
        Some(d) => {
            let p = PathBuf::from(d);
            if !p.is_absolute() || !p.is_dir() {
                return Err("bad_dir".into());
            }
            p
        }
        None => {
            let home = std::env::var_os("HOME").map(PathBuf::from).ok_or("no_home")?;
            let dl = home.join("Downloads");
            if dl.is_dir() { dl } else { home }
        }
    };
    blocking(move || {
        let envs = lk(&hub.adapters).iter().map(|a| (a.id().to_string(), a.version())).collect();
        let path = diag::collect_with(&hub.dir, &out, &diag::Extra { envs })?;
        diag::log("info", "diag_collected");
        let _ = open::opener(&open::Target::Reveal(path.clone()));
        Ok(path.to_string_lossy().to_string())
    })
    .await?
}

/// "О программе": the version, what the last check found, its time and error; no network.
#[tauri::command]
pub async fn app_update_state(s: State<'_, App>) -> Res<UpdateCheck> {
    Ok(Releases::new(&s.hub.dir).view(current(), feed(&s.hub).as_deref()))
}

/// "Проверить" in "О программе": a check now (nothing is fetched without a feed).
#[tauri::command]
pub async fn app_update_check(app: AppHandle, s: State<'_, App>) -> Res<UpdateCheck> {
    let hub = s.hub.clone();
    blocking(move || check(&app, &hub)).await
}

/// The bar of `version` was closed: it does not come back until a newer version.
#[tauri::command]
pub async fn app_update_dismiss(s: State<'_, App>, version: String) -> Res<()> {
    if version.len() > 40 {
        return Err("bad_version".into());
    }
    Releases::new(&s.hub.dir).dismiss(&version);
    Ok(())
}
