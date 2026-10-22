//! CLI update commands (v4 bounds, task 25). Skeleton of task 18: every command answers `not_built` until its task fills it.
use crate::hub::Res;
use serde_json::Value;

#[tauri::command]
pub async fn cli_status(_env: String) -> Res<Value> {
    Err("not_built".into())
}

#[tauri::command]
pub async fn cli_update(_env: String) -> Res<Value> {
    Err("not_built".into())
}

#[tauri::command]
pub async fn updates_history(_env: String) -> Res<Value> {
    Err("not_built".into())
}
