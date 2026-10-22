//! GitHub publishing and the history page (v4 bounds, task 24). Skeleton of task 18: every command answers `not_built` until its task fills it.
use crate::hub::Res;
use serde_json::Value;

#[tauri::command]
pub async fn gh_publish(_path: String, _name: String, _private: bool, _desc: String) -> Res<Value> {
    Err("not_built".into())
}

#[tauri::command]
pub async fn git_log_page(_path: String, _skip: usize, _limit: usize) -> Res<Value> {
    Err("not_built".into())
}

#[tauri::command]
pub async fn git_revert_commit(_path: String, _sha: String) -> Res<Value> {
    Err("not_built".into())
}
