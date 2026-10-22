//! Config file commands (v4 bounds). Skeleton of task 18: every command answers `not_built` until its task fills it.
use crate::hub::Res;
use serde_json::Value;

#[tauri::command]
pub async fn config_open() -> Res<Value> {
    Err("not_built".into())
}

#[tauri::command]
pub async fn config_errors() -> Res<Value> {
    Err("not_built".into())
}
