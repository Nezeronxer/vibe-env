//! GitHub publishing and the history page (v4 bounds, task 24): thin over `core::git`. `path` must be the folder of
//! a project the app knows: the window never makes the app push or rewrite some other folder.
use crate::hub::{e2s, lk, Res};
use crate::App;
use nightshift_core::git;
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::State;

fn project_path(s: &State<'_, App>, path: &str) -> Res<PathBuf> {
    let p = PathBuf::from(path);
    let known = lk(s.hub.engine.store()).projects().into_iter().any(|x| x.path == p);
    known.then_some(p).ok_or_else(|| "project_not_found".to_string())
}

/// Ok(url); Err = the `PublishError` JSON `{kind, ...}` (gh_missing, gh_not_logged_in, name_taken, bad_name,
/// protected_in_history{paths}, publish_failed{detail}). The window asks the person first: this goes outward.
#[tauri::command]
pub async fn gh_publish(s: State<'_, App>, path: String, name: String, private: bool, desc: String) -> Result<String, Value> {
    let p = project_path(&s, &path).map_err(|k| json!({ "kind": k }))?;
    tauri::async_runtime::spawn_blocking(move || git::publish(&p, &name, private, &desc))
        .await
        .map_err(|e| json!({ "kind": "publish_failed", "detail": e.to_string() }))?
        .map_err(|e| serde_json::to_value(&e).unwrap_or_else(|_| json!({ "kind": "publish_failed" })))
}

/// `[CommitRow]`: `{sha, parents, author, time (Unix s), subject, refs, task, lane, edges: [[from, to]]}`.
#[tauri::command]
pub async fn git_log_page(s: State<'_, App>, path: String, skip: usize, limit: usize) -> Res<Value> {
    let p = project_path(&s, &path)?;
    let rows = tauri::async_runtime::spawn_blocking(move || git::log_page(&p, skip, limit.min(1000))).await.map_err(e2s)?.map_err(e2s)?;
    serde_json::to_value(rows).map_err(e2s)
}

/// `{sha, pushed}`: the new commit, and whether the undone one was already on a remote (then pushing the
/// revert is an outward action the window asks about, D01).
#[tauri::command]
pub async fn git_revert_commit(s: State<'_, App>, path: String, sha: String) -> Res<Value> {
    let p = project_path(&s, &path)?;
    if !sha.chars().all(|c| c.is_ascii_hexdigit()) || !(4..=64).contains(&sha.len()) {
        return Err("bad_sha".into());
    }
    let pushed = git::is_pushed(&p, &sha);
    let new = tauri::async_runtime::spawn_blocking(move || git::revert_commit(&p, &sha)).await.map_err(e2s)?.map_err(e2s)?;
    Ok(json!({ "sha": new, "pushed": pushed }))
}
