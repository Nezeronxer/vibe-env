//! Catalog commands (G40-G42): thin over `core::market`. The window sends ids, never items or plans we would trust:
//! an item is one seen in a search (or made here from a link), a plan is one `market_plan` made. Errors are
//! `{kind, detail}` like the kits ones. Network and unpacking run off the async runtime.
use crate::App;
use nightshift_core::market::{Kind, Market, MarketError, Scope, Source};
use nightshift_core::net::CurlFetch;
use serde_json::{json, Value};
use std::sync::OnceLock;
use tauri::State;

type MRes = Result<Value, Value>;

static MARKET: OnceLock<Market> = OnceLock::new();

fn market(s: &State<'_, App>) -> &'static Market {
    MARKET.get_or_init(|| Market::new(Box::new(CurlFetch), s.hub.dir.join("market")).with_kits(s.hub.kits.clone()))
}

fn merr(e: MarketError) -> Value {
    serde_json::to_value(&e).unwrap_or_else(|_| json!({ "kind": e.key() }))
}

fn val(v: serde_json::Result<Value>) -> MRes {
    v.map_err(|e| json!({ "kind": "io", "detail": e.to_string() }))
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, Value> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| json!({ "kind": "io", "detail": e.to_string() }))
}

fn id_of(v: &Value) -> Option<String> {
    v.get("id").and_then(Value::as_str).map(String::from)
}

#[tauri::command]
pub async fn market_search(s: State<'_, App>, kind: Kind, query: String, cursor: Option<String>) -> MRes {
    let m = market(&s);
    blocking(move || m.search(kind, &query, cursor.as_deref()).map_err(merr).and_then(|x| val(serde_json::to_value(x)))).await?
}

#[tauri::command]
pub async fn market_item(s: State<'_, App>, id: String) -> MRes {
    let m = market(&s);
    blocking(move || m.item(&id).map_err(merr).and_then(|x| val(serde_json::to_value(x)))).await?
}

/// `item` is `{id}` of a searched item, or `{kind, link}` for "Установить мод / скилл по ссылке".
#[tauri::command]
pub async fn market_plan(s: State<'_, App>, item: Value, env: String, scope: Scope) -> MRes {
    let m = market(&s);
    blocking(move || {
        let it = match (id_of(&item).and_then(|id| m.known(&id)), item.get("link").and_then(Value::as_str)) {
            (Some(it), _) => it,
            (None, Some(link)) => {
                let kind: Kind = serde_json::from_value(item.get("kind").cloned().unwrap_or(json!("mod"))).map_err(|_| json!({ "kind": "bad_source", "detail": "kind" }))?;
                m.from_link(kind, link).map_err(merr)?
            }
            _ => return Err(json!({ "kind": "not_found", "detail": "item" })),
        };
        m.install(&it, &env, scope).map_err(merr).and_then(|x| val(serde_json::to_value(x)))
    })
    .await?
}

#[tauri::command]
pub async fn market_apply(s: State<'_, App>, plan: Value, enabled_exec: Vec<String>) -> MRes {
    let m = market(&s);
    blocking(move || {
        let p = id_of(&plan).and_then(|id| m.plan(&id)).ok_or_else(|| json!({ "kind": "changed" }))?;
        m.apply(&p, enabled_exec).map_err(merr).and_then(|x| val(serde_json::to_value(x)))
    })
    .await?
}

#[tauri::command]
pub async fn market_installed(s: State<'_, App>, env: String) -> MRes {
    let m = market(&s);
    blocking(move || m.installed(&env).map_err(merr).and_then(|x| val(serde_json::to_value(x)))).await?
}

#[tauri::command]
pub async fn market_remove(s: State<'_, App>, env: String, id: String) -> MRes {
    let m = market(&s);
    blocking(move || m.remove(&env, &id).map_err(merr).map(|_| Value::Null)).await?
}

#[tauri::command]
pub async fn market_sources(s: State<'_, App>) -> MRes {
    market(&s).sources().map_err(merr).and_then(|x| val(serde_json::to_value(x)))
}

#[tauri::command]
pub async fn market_set_sources(s: State<'_, App>, sources: Vec<Source>) -> MRes {
    let m = market(&s);
    m.set_sources(sources).map_err(merr)?;
    m.sources().map_err(merr).and_then(|x| val(serde_json::to_value(x)))
}
