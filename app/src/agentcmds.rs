//! Commands of the terminal agents (an agent = a PTY running an env's CLI in a folder): its own git copy, what it
//! changed, its token use, and the system notification that brings its pane back. Thin over `core`; every text that
//! goes to the window or the notification centre passes `scrub`.
use crate::hub::{lk, Res};
use crate::App;
use nightshift_core::{scrub, termusage};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, State};
use tauri_plugin_notification::NotificationExt;

fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}

fn clean(e: String) -> String {
    scrub::clean(&e, &home())
}

/// A folder the window names: absolute (a relative one would mean the app's own working folder).
fn folder(p: &str) -> Res<PathBuf> {
    Path::new(p).is_absolute().then(|| PathBuf::from(p)).ok_or_else(|| "bad_path".to_string())
}

/// Runs git work off the UI thread; the error text is scrubbed on the way out.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Res<T> + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?.map_err(clean)
}

/// `{path, branch}`: a separate git copy for a terminal agent, branch `ns/term-<id>-<slug of name>` from the project's
/// base. Err: `project_not_found`, `not_a_repo`, `worktree_failed: <why>`.
#[tauri::command]
pub async fn agent_worktree(s: State<'_, App>, project_id: String, name: String) -> Res<Value> {
    // the task text names the branch: a key typed into it must not reach git
    let (hub, name) = (s.hub.clone(), clean(name));
    blocking(move || hub.engine.agent_worktree(&project_id, &name).map(|(path, branch)| json!({ "path": path.to_string_lossy(), "branch": branch }))).await
}

/// `{branch_deleted}`. Only the root of a copy made by `agent_worktree`. Err: `bad_path`, `not_a_repo`,
/// `not_own_worktree`, `uncommitted_changes` (without `force`), `git_failed: <why>`.
#[tauri::command]
pub async fn agent_worktree_remove(s: State<'_, App>, path: String, force: bool) -> Res<Value> {
    let (hub, path) = (s.hub.clone(), folder(&path)?);
    blocking(move || hub.engine.agent_worktree_remove(&path, force).map(|deleted| json!({ "branch_deleted": deleted }))).await
}

/// `{repo, branch, base, own_worktree, files: [{path, status, add, del}], truncated}`: the folder against HEAD (an agent's own copy: against
/// where its branch left the base, its own commits included).
#[tauri::command]
pub async fn agent_changes(s: State<'_, App>, cwd: String) -> Res<Value> {
    let (hub, cwd) = (s.hub.clone(), folder(&cwd)?);
    blocking(move || hub.engine.agent_changes(&cwd).map(|c| json!(c))).await
}

/// `{text, truncated}`: the unified diff of one file against the same point as `agent_changes` (an untracked file as a new one), scrubbed.
/// Err: `bad_path`, `git_failed: <why>`.
#[tauri::command]
pub async fn agent_file_diff(s: State<'_, App>, cwd: String, file: String) -> Res<Value> {
    let (hub, cwd) = (s.hub.clone(), folder(&cwd)?);
    blocking(move || hub.engine.agent_file_diff(&cwd, &file).map(|(text, truncated)| json!({ "text": text, "truncated": truncated }))).await
}

/// Puts one file back as that point has it (a file it lacks is removed). Err: `bad_path`, `not_a_repo`, `git_failed: <why>`.
#[tauri::command]
pub async fn agent_revert_file(s: State<'_, App>, cwd: String, file: String) -> Res<()> {
    let (hub, cwd) = (s.hub.clone(), folder(&cwd)?);
    blocking(move || hub.engine.agent_revert_file(&cwd, &file)).await
}

/// `{merged, sha}`: commit everything of a terminal agent's copy and fast-forward the project's base to it. Err:
/// `bad_path`, `not_a_repo`, `not_own_worktree`, `project_not_found`, `rebase_in_progress`,
/// `protected_file_committed: <paths>`, `base_dirty`, `not_fast_forward`, `base_missing`, `git_failed: <why>`.
#[tauri::command]
pub async fn agent_merge(s: State<'_, App>, cwd: String, message: String) -> Res<Value> {
    // the message is the task text: a key typed into it must not land in the base's history
    let (hub, cwd, message) = (s.hub.clone(), folder(&cwd)?, clean(message));
    blocking(move || hub.engine.agent_merge(&cwd, &message).map(|(merged, sha)| json!({ "merged": merged, "sha": sha }))).await
}

/// `{tokens_in, tokens_out, usd}`, each a number or null ("нет данных"): the env's own session files for this exact
/// folder, records at or after `since_ms`. Claude Code and Codex only.
#[tauri::command]
pub async fn term_usage(cwd: String, env: String, since_ms: u64) -> Res<Value> {
    let cwd = folder(&cwd)?;
    blocking(move || Ok(json!(termusage::usage(&cwd, &env, since_ms)))).await
}

// ---------- notification of an agent ----------

/// Own range, so an agent's notification never replaces a task's (those count from 1).
static NEXT_ID: AtomicI32 = AtomicI32::new(1_000_000);
/// notification id -> pane, so a click can name the pane (the last 200 are kept)
static PANES: Mutex<Vec<(i32, String)>> = Mutex::new(Vec::new());

fn remember(id: i32, pane: &str) {
    let mut v = lk(&PANES);
    v.push((id, pane.to_string()));
    if v.len() > 200 {
        v.remove(0);
    }
}

/// The pane of the notification a click was on; None = not an agent's notification.
pub fn pane_of(id: i32) -> Option<String> {
    lk(&PANES).iter().find(|(i, _)| *i == id).map(|(_, p)| p.clone())
}

/// A sound unless the person's quiet hours are on (the notification still arrives, as the engine's do) or the sound is off.
fn sound_for(cfg: &nightshift_core::model::NotifySettings, now: chrono::DateTime<chrono::Local>) -> Option<String> {
    if crate::notifier::in_quiet(now, &cfg.quiet_from, &cfg.quiet_to) { None } else { crate::notifier::sound_name(&cfg.sound) }
}

fn cut(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// A system notification for a terminal agent. A click brings the main window up and emits the window event
/// `agent-notify-click` with `{pane}`. Err: `bad_pane`, `notify_failed` (the system refuses notifications).
#[tauri::command]
pub fn notify_agent(app: AppHandle, s: State<'_, App>, title: String, body: String, pane: String) -> Res<()> {
    if pane.is_empty() || pane.chars().count() > 128 {
        return Err("bad_pane".into());
    }
    let (title, body) = (clean(cut(&title, 120)), clean(cut(&body, 400)));
    let sound = sound_for(&lk(&s.notify_cfg), chrono::Local::now());
    if crate::tray::notification_server() == Some(false) {
        return Err("notify_failed".into());
    }
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    remember(id, &pane);
    let mut b = app.notification().builder().id(id).title(title).body(body);
    if let Some(sound) = sound {
        b = b.sound(sound);
    }
    b.show().map_err(|_| "notify_failed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use nightshift_core::model::NotifySettings;

    #[test]
    fn a_click_finds_its_pane_and_forgets_the_old_ones() {
        remember(7_000_001, "pane-a");
        assert_eq!(pane_of(7_000_001).as_deref(), Some("pane-a"));
        assert_eq!(pane_of(7_000_999), None, "a task's notification is no agent's");
        for i in 0..200 {
            remember(8_000_000 + i, "x");
        }
        assert_eq!(pane_of(7_000_001), None, "only the last 200 are kept");
    }

    #[test]
    fn quiet_hours_and_a_sound_set_to_none_silence_the_notification() {
        let cfg = NotifySettings::default();
        let at = |h| chrono::Local.with_ymd_and_hms(2026, 10, 25, h, 0, 0).unwrap();
        assert!(sound_for(&cfg, at(12)).is_some());
        assert_eq!(sound_for(&cfg, at(23)), None);
        assert_eq!(sound_for(&cfg, at(3)), None);
        assert_eq!(sound_for(&NotifySettings { sound: "none".into(), ..cfg }, at(12)), None);
    }

    #[test]
    fn a_folder_must_be_absolute_and_errors_are_scrubbed() {
        assert_eq!(folder("rel/dir"), Err("bad_path".to_string()));
        assert!(folder("/tmp").is_ok());
        let secret = format!("git_failed: token ghp_{}", "a".repeat(36));
        assert!(!clean(secret).contains("ghp_aaaa"));
    }
}
