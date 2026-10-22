//! Model catalog of an env: where the list comes from, how a choice becomes a CLI argument,
//! what to try when the CLI refuses a model (stories 84-88, 171; spec 21).
//!
//! Sources, in order: the env's own list command (`Adapter::models`), the last good snapshot in the
//! `Store`, the adapter's built-in hint list. Every catalog also carries "CLI default" (no flag)
//! and the person's own models (`EnvSettings.custom_models`).
use crate::agent::Adapter;
use crate::model::{Catalog, CatalogSource, ModelChoice, ModelFamily, ModelNote, ResolvedModel};
use crate::store::Store;
use chrono::Local;

/// Family id of "CLI default": resolves to no flag.
pub const DEFAULT_FAMILY: &str = "default";
/// `ModelFamily.vendor` of the two pseudo entries; they are never "stronger" or "newer".
pub const VENDOR_CLI: &str = "cli";
pub const VENDOR_CUSTOM: &str = "custom";

/// `1.10` > `1.9`: numeric compare of dotted versions.
pub fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let n = |s: &str| s.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    n(a).cmp(&n(b))
}

fn is_pseudo(f: &ModelFamily) -> bool {
    f.vendor == VENDOR_CLI || f.vendor == VENDOR_CUSTOM
}

/// Adds "CLI default" in front and the custom models at the end (replacing earlier copies).
fn with_extras(mut families: Vec<ModelFamily>, custom: &[String]) -> Vec<ModelFamily> {
    families.retain(|f| !is_pseudo(f));
    let mut out = vec![ModelFamily {
        id: DEFAULT_FAMILY.into(),
        label: "CLI default".into(),
        vendor: VENDOR_CLI.into(),
        blurb: None,
        versions: vec![],
        default: true,
        exact: Default::default(),
    }];
    for f in &mut families {
        f.default = false; // exactly one entry is the default, and it is the one with no flag
    }
    out.extend(families.iter().cloned());
    for m in custom {
        if !out.iter().any(|f| &f.id == m) {
            out.push(ModelFamily {
                id: m.clone(),
                label: m.clone(),
                vendor: VENDOR_CUSTOM.into(),
                blurb: Some("custom model".into()),
                versions: vec![],
                default: false,
                exact: Default::default(),
            });
        }
    }
    out
}

fn build(env: &str, families: Vec<ModelFamily>, source: CatalogSource, at: chrono::DateTime<Local>, store: &Store) -> Catalog {
    let custom = store.settings().envs.get(env).map(|e| e.custom_models.clone()).unwrap_or_default();
    // only the pseudo entries: the person has the env's list empty, so everything is typed by hand
    let source = if families.is_empty() { CatalogSource::Manual } else { source };
    Catalog { env: env.to_string(), families: with_extras(families, &custom), source, at }
}

/// Runs the env's list command. Ok -> the list is saved as the snapshot. A failed command is not an
/// error: the last snapshot is used, and without one the built-in hint list (G18.3).
pub fn refresh(env: &str, a: &dyn Adapter, store: &Store) -> Catalog {
    if let Ok(families) = a.models() {
        if !families.is_empty() {
            let now = Local::now();
            let _ = store.set_catalog(Catalog { env: env.to_string(), families: families.clone(), source: CatalogSource::CliList, at: now });
            return build(env, families, CatalogSource::CliList, now, store);
        }
    }
    catalog(env, a, store)
}

/// What is known without running the CLI: the last snapshot, else the built-in hint list.
pub fn catalog(env: &str, a: &dyn Adapter, store: &Store) -> Catalog {
    match store.catalog(env) {
        Some(snap) => build(env, snap.families, CatalogSource::Snapshot, snap.at, store),
        None => build(env, a.builtin_models(), CatalogSource::BuiltIn, Local::now(), store),
    }
}

/// A choice -> the CLI argument and a caption, with the note when the choice is out of date.
/// "Latest" is the env's alias for the family where the CLI has one (Claude), else the family's
/// newest listed version. A pinned version is the exact identifier the catalog holds for it; one that
/// left the list moves to the nearest: the next newer of the same family, else the newest. The caller
/// keeps the pin; moving it is a person's act. Nothing here builds a model name: a choice the catalog
/// cannot name starts with no flag (`Rejected`) instead of a made-up identifier.
pub fn resolve(cat: &Catalog, c: &ModelChoice) -> ResolvedModel {
    let Some(f) = cat.families.iter().find(|f| f.id == c.family) else {
        let tried = c.version.as_deref().map_or_else(|| c.family.clone(), |v| format!("{} {v}", c.family));
        return ResolvedModel { cli_arg: None, label: tried.clone(), note: Some(ModelNote::Rejected { tried }) };
    };
    if f.id == DEFAULT_FAMILY && f.vendor == VENDOR_CLI {
        return ResolvedModel { cli_arg: None, label: f.label.clone(), note: None };
    }
    if f.vendor == VENDOR_CUSTOM {
        return ResolvedModel { cli_arg: Some(f.id.clone()), label: f.label.clone(), note: None };
    }
    let latest = || match f.versions.first().and_then(|v| f.exact.get(v)) {
        Some(id) if cat.env != "claude" => ResolvedModel { cli_arg: Some(id.clone()), label: format!("{} {}", f.label, f.versions[0]), note: None },
        _ => ResolvedModel { cli_arg: Some(f.id.clone()), label: f.label.clone(), note: None },
    };
    let pinned = |v: &str, note: Option<ModelNote>| {
        f.exact.get(v).map(|id| ResolvedModel { cli_arg: Some(id.clone()), label: format!("{} {v}", f.label), note })
    };
    let Some(v) = c.version.as_deref() else { return latest() };
    if f.versions.iter().any(|x| x == v) {
        let newest = &f.versions[0];
        let note = version_cmp(newest, v).is_gt().then(|| ModelNote::Newer(newest.clone()));
        if let Some(r) = pinned(v, note) {
            return r;
        }
    }
    // gone from the list (or listed without an identifier): the nearest version that has one
    let to = f
        .versions
        .iter()
        .filter(|x| f.exact.contains_key(*x) && version_cmp(x, v).is_gt())
        .min_by(|a, b| version_cmp(a, b))
        .or_else(|| f.versions.iter().find(|x| f.exact.contains_key(*x)));
    match to {
        Some(to) => pinned(to, Some(ModelNote::Replaced { from: v.to_string(), to: to.clone() })).expect("checked above"),
        None => {
            let mut r = latest(); // the family has no versions to move to: its own id
            r.note = Some(ModelNote::Replaced { from: v.to_string(), to: "latest".into() });
            r
        }
    }
}

/// What to try after the CLI refused `c`: the family's latest, then no flag at all. None = nothing left.
pub fn next_on_reject(cat: &Catalog, c: &ModelChoice) -> Option<ModelChoice> {
    let default = || Some(ModelChoice { family: DEFAULT_FAMILY.into(), version: None });
    if c.family == DEFAULT_FAMILY {
        return None;
    }
    let fam = cat.families.iter().find(|f| f.id == c.family);
    let latest_differs = match (&c.version, fam) {
        (None, _) => false,
        (Some(v), Some(f)) => f.versions.first().is_none_or(|l| l != v),
        (Some(_), None) => true,
    };
    if latest_differs && fam.is_none_or(|f| f.vendor != VENDOR_CUSTOM) {
        return Some(ModelChoice { family: c.family.clone(), version: None });
    }
    default()
}

/// The first model of the env's list (strongest first) that is not `c`: where a retry can go up.
pub fn stronger_than(cat: &Catalog, c: &ModelChoice) -> Option<ModelChoice> {
    cat.families
        .iter()
        .filter(|f| !is_pseudo(f))
        .map(|f| ModelChoice { family: f.id.clone(), version: None })
        .find(|m| m != c)
}

