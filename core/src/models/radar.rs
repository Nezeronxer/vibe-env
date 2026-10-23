//! New models from the public catalog `models.dev` (research-updates §3): which entries are models to offer,
//! and where a new one goes in an env's families. Pure functions; the fetching lives in `core::updates`.
use crate::model::ModelFamily;
use crate::models::version_cmp;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// The models.dev provider that announces new models of an env without a list command of its own.
/// Codex, opencode and Cursor have their own list (the truth for them: models.dev keeps models a plan lost).
pub fn provider_of(env: &str) -> Option<&'static str> {
    match env {
        "claude" => Some("anthropic"),
        "gemini" => Some("google"),
        "qwen" => Some("alibaba"),
        _ => None,
    }
}

fn not_a_model() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // dated copies (`-20251001`, `-2026-01-23`), API aliases and non-chat models (research-updates §3, filter)
    R.get_or_init(|| Regex::new(r"image|tts|embedding|live|realtime|audio|computer-use|customtools|latest|-\d{8}\b|-\d{4}-\d{2}-\d{2}").unwrap())
}

/// `(id, name)` of the provider's chat models with tools that are not deprecated, in the catalog's order.
pub fn chat_models(api: &Value, provider: &str) -> Vec<(String, String)> {
    let Some(models) = api[provider]["models"].as_object() else { return vec![] };
    models
        .iter()
        .filter(|(id, m)| {
            m["tool_call"] == true
                && m["modalities"]["output"] == serde_json::json!(["text"])
                && m["status"] != "deprecated"
                && !not_a_model().is_match(id)
                // untrusted catalog: an id like `--yolo` would later be read as a CLI flag
                && crate::agent::adapters::common::is_model_id(id)
        })
        .map(|(id, m)| (id.clone(), m["name"].as_str().unwrap_or(id).to_string()))
        .collect()
}

/// The id with its first number replaced by `#`, and that number as a version: `claude-opus-5-5` ->
/// (`claude-opus-#`, `5.5`), `gemini-3.8-flash` -> (`gemini-#-flash`, `3.8`).
fn shape(id: &str) -> (String, Option<String>) {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| Regex::new(r"\d+(?:[.-]\d+)*").unwrap());
    match r.find(id) {
        Some(m) => (format!("{}#{}", &id[..m.start()], &id[m.end()..]), Some(m.as_str().replace('-', "."))),
        None => (id.to_string(), None),
    }
}

fn major(v: &str) -> &str {
    v.split('.').next().unwrap_or(v)
}

/// Puts `id` into the family whose identifiers have its shape (newest first), or a family of its own.
/// `by_button`: a newer generation (`3.x` -> `4`) goes after the current latest, so "latest" does not move
/// by itself (Gemini, Qwen: the CLI may not be ready for it, research-updates §4.1). Nothing when known.
pub fn place(families: &mut Vec<ModelFamily>, id: &str, name: &str, vendor: &str, by_button: bool) {
    if families.iter().any(|f| f.id == id || f.exact.values().any(|e| e == id)) {
        return;
    }
    let (sh, version) = shape(id);
    let home = version.as_ref().and_then(|_| families.iter_mut().find(|f| f.exact.values().any(|e| shape(e).0 == sh)));
    match (home, version) {
        (Some(f), Some(v)) if !f.versions.contains(&v) => {
            let newest = f.versions.first().cloned();
            let at = match newest {
                Some(n) if by_button && version_cmp(major(&v), major(&n)).is_gt() => 1,
                _ => f.versions.iter().position(|x| version_cmp(&v, x).is_gt()).unwrap_or(f.versions.len()),
            };
            f.versions.insert(at, v.clone());
            f.exact.insert(v, id.to_string());
        }
        (Some(_), _) => {}
        (None, _) => families.push(ModelFamily {
            id: id.to_string(),
            label: name.to_string(),
            vendor: vendor.to_string(),
            blurb: None,
            versions: vec![],
            default: false,
            exact: Default::default(),
            source_note: Some("models.dev".into()),
        }),
    }
}

/// Every identifier a catalog can start: exact ids, and the ids of families without versions.
pub fn ids(families: &[ModelFamily]) -> Vec<(String, String)> {
    let mut out = vec![];
    for f in families {
        if f.versions.is_empty() {
            out.push((f.id.clone(), f.label.clone()));
        }
        for v in &f.versions {
            if let Some(e) = f.exact.get(v) {
                out.push((e.clone(), format!("{} {v}", f.label)));
            }
        }
    }
    out
}

/// "Latest" of an existing family moved to `id` by itself.
pub fn moved_latest(before: &[ModelFamily], after: &[ModelFamily], id: &str) -> bool {
    after.iter().any(|f| before.iter().any(|b| b.id == f.id) && f.versions.first().and_then(|v| f.exact.get(v)).is_some_and(|e| e == id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes() {
        assert_eq!(shape("claude-opus-5-5"), ("claude-opus-#".into(), Some("5.5".into())));
        assert_eq!(shape("qwen3.8-max"), ("qwen#-max".into(), Some("3.8".into())));
        assert_eq!(shape("gpt-sol"), ("gpt-sol".into(), None));
    }
}
