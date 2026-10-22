//! Catalog commands (v4 bounds, task 24). Skeleton of task 18: every command answers `not_built` until its task fills it.
use crate::hub::Res;
use serde_json::Value;

#[tauri::command]
pub async fn market_search(_kind: String, _query: String, _cursor: Option<String>) -> Res<Value> {
    Err("not_built".into())
}

#[tauri::command]
pub async fn market_item(_id: String) -> Res<Value> {
    Err("not_built".into())
}

#[tauri::command]
pub async fn market_plan(_item: Value, _env: String, _scope: Value) -> Res<Value> {
    Err("not_built".into())
}

#[tauri::command]
pub async fn market_apply(_plan: Value, _enabled_exec: Vec<String>) -> Res<Value> {
    Err("not_built".into())
}

#[tauri::command]
pub async fn market_installed(_env: String) -> Res<Value> {
    Err("not_built".into())
}

#[tauri::command]
pub async fn market_remove(_env: String, _id: String) -> Res<Value> {
    Err("not_built".into())
}

#[tauri::command]
pub async fn market_sources() -> Res<Value> {
    Err("not_built".into())
}

#[tauri::command]
pub async fn market_set_sources(_sources: Value) -> Res<Value> {
    Err("not_built".into())
}
