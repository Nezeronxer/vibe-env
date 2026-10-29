//! GitHub publishing and the history page (v4 bounds, task 24): thin over `core::git`. `path` must be the folder of
//! a project the app knows: the window never makes the app push or rewrite some other folder.
use crate::hub::{e2s, lk, Res};
use crate::App;
use nightshift_core::git;
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::State;

fn check_sha(sha: &str) -> Res<()> {
    (sha.chars().all(|c| c.is_ascii_hexdigit()) && (4..=64).contains(&sha.len())).then_some(()).ok_or_else(|| "bad_sha".to_string())
}

fn project_path(s: &State<'_, App>, path: &str) -> Res<PathBuf> {
    let p = PathBuf::from(path);
    let known = lk(s.hub.engine.store()).projects().into_iter().any(|x| x.path == p);
    known.then_some(p).ok_or_else(|| "project_not_found".to_string())
}

/// v8: the open PRs of a known project for the GitHub panel (read-only `gh pr list`); Err = the gh hint (not installed, not logged in).
#[tauri::command]
pub async fn gh_prs(s: State<'_, App>, path: String) -> Res<Value> {
    let p = project_path(&s, &path)?;
    tauri::async_runtime::spawn_blocking(move || git::pr_list(&p)).await.map_err(e2s)?.map_err(e2s)
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
    check_sha(&sha)?;
    let pushed = git::is_pushed(&p, &sha);
    let new = tauri::async_runtime::spawn_blocking(move || git::revert_commit(&p, &sha)).await.map_err(e2s)?.map_err(e2s)?;
    Ok(json!({ "sha": new, "pushed": pushed }))
}

/// Story 306: `StartupTax` of an env - what CLAUDE.md/AGENTS.md, skill and agent descriptions and the MCP list cost
/// before the first word. `project` = project id (its folder adds the project's rules); unknown id → `project_not_found`.
#[tauri::command]
pub async fn startup_tax(s: State<'_, App>, env: String, project: Option<String>) -> Res<Value> {
    let dir = match project {
        Some(id) => Some(lk(s.hub.engine.store()).project(&id).map(|p| p.path).ok_or_else(|| "project_not_found".to_string())?),
        None => None,
    };
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let tax = tauri::async_runtime::spawn_blocking(move || nightshift_core::agent::tax::startup_tax(&env, &home, dir.as_deref())).await.map_err(e2s)?;
    serde_json::to_value(tax).map_err(e2s)
}

/// Story 305: "Сжать" - the agent writes a hand-off note, then `/compact <keep>` in its session (`Engine::compact`).
/// Errors: `task_not_found`, `nothing_to_compact`.
#[tauri::command]
pub fn compact(s: State<'_, App>, task_id: String, keep: Option<String>) -> Res<()> {
    s.hub.engine.compact(&task_id, keep.as_deref())
}

/// Files of one commit of the history page that belongs to no task: `[{path, insertions, deletions}]`.
#[tauri::command]
pub async fn commit_stat(s: State<'_, App>, path: String, sha: String) -> Res<Value> {
    let p = project_path(&s, &path)?;
    check_sha(&sha)?;
    let rows = tauri::async_runtime::spawn_blocking(move || git::commit_stat(&p, &sha)).await.map_err(e2s)?;
    Ok(Value::Array(rows.into_iter().map(|f| json!({ "path": f.path, "insertions": f.insertions, "deletions": f.deletions })).collect()))
}

/// The diff of one file of such a commit, in the form of `diff_file`: `{lines: [{kind, text}], binary, size_kb, truncated}`.
#[tauri::command]
pub async fn commit_file_diff(s: State<'_, App>, path: String, sha: String, file: String) -> Res<Value> {
    let p = project_path(&s, &path)?;
    check_sha(&sha)?;
    let d = tauri::async_runtime::spawn_blocking(move || git::commit_file_diff(&p, &sha, &file, 2000)).await.map_err(e2s)?;
    Ok(json!({ "lines": crate::proj::diff_lines(&d.text), "binary": d.binary, "size_kb": d.size.div_ceil(1024), "truncated": d.truncated }))
}

#[cfg(test)]
mod tests {
    use super::check_sha;

    #[test]
    fn sha_must_be_hex_of_4_to_64() {
        for ok in ["abcd", "0123456789abcdef0123456789abcdef01234567", &"f".repeat(64)] {
            assert_eq!(check_sha(ok), Ok(()), "{ok}");
        }
        for bad in ["", "abc", "zzzz", "--all", "HEAD", "abcd;rm", &"a".repeat(65)] {
            assert_eq!(check_sha(bad), Err("bad_sha".to_string()), "{bad}");
        }
    }
}
