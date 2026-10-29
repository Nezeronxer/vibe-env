//! Catalogs of skills, MCP servers and mods (G40-G42, research-catalogs.md). The catalogs give metadata only; an item
//! is downloaded (HTTPS, size limits, safe paths), put together as a kit in memory and goes through `core::kits`:
//! the same "what will change" plan, snapshot, held executables and secrets by name only.
//! Everything from the network is data: it is shown and written as files, never run here.

mod build;
pub(crate) use build::{render_server, Remote, Server};
mod sources;

use crate::kits::{ImportPlan, KitError, Kits, ScriptText, SnapshotInfo, SnapshotState};
use crate::net::{Fetch, NetError};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use crate::kits::Scope;

/// A catalog index or answer is used for this long before it is asked again (G41.3).
pub const CACHE_TTL: Duration = Duration::from_secs(24 * 3600);
/// One item downloaded (an archive): 20 MB (G41.4).
pub const MAX_ITEM: usize = 20 * 1024 * 1024;
/// Unpacked: 100 MB (G41.4).
pub const MAX_UNPACKED: u64 = 100 * 1024 * 1024;
/// A catalog answer (the community marketplace is ~1.5 MB).
const MAX_INDEX: usize = 8 * 1024 * 1024;
/// Items per page of a source that has no paging of its own.
const PAGE: usize = 20;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MarketError {
    #[error("{0}")]
    Net(NetError),
    #[error("http_{status}: {url}")]
    Http { url: String, status: u16 },
    /// The source answered something that is not what its format says (broken JSON...).
    #[error("bad_answer: {0}")]
    BadAnswer(String),
    #[error("not_found: {0}")]
    NotFound(String),
    /// npm / archive / command sources of a marketplace, a plugin with nothing to install, a header form...
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// "Мод - только для Claude Code", a skill for an environment whose skills folder is not confirmed.
    #[error("env_unsupported: {kind:?} in {env}")]
    EnvUnsupported { kind: Kind, env: String },
    #[error("bad_source: {0}")]
    BadSource(String),
    /// Paths, symlinks, sizes, the plan, the snapshot: what `core::kits` says.
    #[error("{0}")]
    Kit(KitError),
    #[error("io: {0}")]
    Io(String),
}

impl MarketError {
    /// The key the window translates.
    pub fn key(&self) -> &'static str {
        match self {
            MarketError::Net(e) => e.key(),
            MarketError::Http { .. } => "http_error",
            MarketError::BadAnswer(_) => "bad_answer",
            MarketError::NotFound(_) => "not_found",
            MarketError::Unsupported(_) => "unsupported",
            MarketError::EnvUnsupported { .. } => "env_unsupported",
            MarketError::BadSource(_) => "bad_source",
            MarketError::Kit(e) => e.kind(),
            MarketError::Io(_) => "io",
        }
    }
}

impl Serialize for MarketError {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            MarketError::Kit(e) => e.serialize(ser),
            e => serde_json::json!({ "kind": e.key(), "detail": e.to_string() }).serialize(ser),
        }
    }
}

impl From<KitError> for MarketError {
    fn from(e: KitError) -> Self {
        MarketError::Kit(e)
    }
}
impl From<NetError> for MarketError {
    fn from(e: NetError) -> Self {
        MarketError::Net(e)
    }
}

fn io(e: impl std::fmt::Display) -> MarketError {
    MarketError::Io(e.to_string())
}

pub type Result<T> = std::result::Result<T, MarketError>;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Skill,
    Mcp,
    Mod,
}

/// How a source is asked.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Api {
    /// `.claude-plugin/marketplace.json` (one file, searched here).
    #[default]
    Marketplace,
    /// `skills.sh/api/search?q=&limit=` (undocumented, no key).
    SkillsSh,
    /// `skillsmp.com/api/v1/skills/search?q=&limit=&page=`.
    SkillsMp,
    /// MCP registry API `v0.1/servers` (official, GitHub's).
    McpRegistry,
    /// `registry.smithery.ai/servers?q=&pageSize=&page=`.
    Smithery,
    /// GitHub contents API of a folder whose subfolders are mods.
    GithubDir,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Source {
    pub id: String,
    pub kind: Kind,
    pub url: String,
    pub enabled: bool,
    #[serde(default)]
    pub api: Api,
    #[serde(default)]
    pub name: String,
    /// Built in: its address and kind are fixed, only `enabled` is the human's.
    #[serde(default)]
    pub builtin: bool,
    /// Why it is off by default (`terms_not_checked`, `fallback`).
    #[serde(default)]
    pub note: Option<String>,
}

/// A source that did not answer: its last saved answer is shown with its date (G41.1), or nothing.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Offline {
    pub source: String,
    /// `no_connection`, `timeout`, `bad_answer`, `http_error`...
    pub reason: String,
    /// The saved list shown instead, if there is one.
    pub cached_at: Option<DateTime<Local>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Page<T> {
    pub items: Vec<T>,
    /// Pass back for the next portion; None = the end.
    pub cursor: Option<String>,
    #[serde(default)]
    pub offline: Vec<Offline>,
}

/// Where an item's files come from.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Origin {
    #[default]
    None,
    /// `path` inside `owner/repo` at `ref` (a commit, a branch, or HEAD when None); `skill` = find the folder of this
    /// skill by name (skills.sh gives no path).
    Github {
        repo: String,
        #[serde(default)]
        path: String,
        #[serde(default, rename = "ref")]
        git_ref: Option<String>,
        #[serde(default)]
        skill: Option<String>,
    },
    /// Any other git repository (limited shallow clone).
    Git {
        url: String,
        #[serde(default)]
        path: String,
    },
    Folder {
        path: PathBuf,
    },
    Zip {
        path: PathBuf,
    },
    /// `server.json` of an MCP registry.
    Mcp {
        server: Value,
    },
    /// A Smithery server: its address comes from the detail call.
    Smithery {
        name: String,
    },
    Unsupported {
        why: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Item {
    /// `<source id>:<id in the source>`.
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub source: String,
    /// Plain text from the catalog - untrusted, shown as text only.
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: Option<String>,
    /// None = "лицензия не указана".
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub installs: Option<u64>,
    #[serde(default)]
    pub stars: Option<u64>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub origin: Origin,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VarInfo {
    pub name: String,
    pub secret: bool,
    pub required: bool,
    #[serde(default)]
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ItemDetail {
    pub item: Item,
    /// SKILL.md (instructions for the agent) as plain text, when it could be fetched.
    pub readme: Option<String>,
    /// MCP: variables it needs (values go to the keyring, never into a file).
    pub env_vars: Vec<VarInfo>,
    /// MCP: the command it starts (`npx -y pkg@1.2.3`) or the remote address.
    pub runs: Option<String>,
    /// `node`, `uv`, `docker`.
    pub needs: Vec<String>,
}

/// What would be written and what of it is executable. `kit` is the full plan of `core::kits` ("Что изменится").
#[derive(Serialize, Clone, Debug)]
pub struct InstallPlan {
    pub id: String,
    pub item: Item,
    pub env: String,
    pub scope: Scope,
    /// Written on "Install" (`~`-style targets).
    pub writes: Vec<String>,
    /// Keys of held entries (hooks, MCP servers, scripted skills, mods): off until ticked (`apply(enabled_exec)`).
    pub executable: Vec<String>,
    /// MCP: command line or remote address shown before install.
    pub runs: Option<String>,
    pub needs: Vec<String>,
    /// `runs_code` (local MCP, mod, scripted skill), `needs_docker`, `remote`.
    pub warnings: Vec<String>,
    /// SKILL.md files in full: instructions for the agent, read before install.
    pub texts: Vec<ScriptText>,
    /// This item is already installed into this environment and scope ("установлено").
    pub installed: bool,
    pub kit: ImportPlan,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Installed {
    pub id: String,
    /// Commit (git items) or version (MCP) that is installed.
    pub version: String,
    /// A newer one, when the source says so ("есть обновление").
    pub update: Option<String>,
    pub name: String,
    pub kind: Kind,
    pub scope: Scope,
    pub snapshot: String,
    pub at: DateTime<Local>,
}

/// One install, kept in `<cache>/installed.json`.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct Record {
    env: String,
    item: Item,
    scope: Scope,
    version: String,
    snapshot: String,
    at: DateTime<Local>,
}

/// Saved answer of a GET: `<cache>/http/<sha>.json` + `.body`.
#[derive(Serialize, Deserialize)]
struct CacheMeta {
    url: String,
    at: DateTime<Local>,
    etag: Option<String>,
}

pub struct Market {
    fetch: Box<dyn Fetch>,
    dir: PathBuf,
    kits: Option<Arc<Kits>>,
    ttl: Duration,
    /// Items seen in searches and links: install and detail take items by id only.
    seen: Mutex<HashMap<String, Item>>,
    /// `<kind>/<id>` the window lists for the current search of each kind (its first portion and the next ones).
    listed: Mutex<std::collections::HashSet<String>>,
    plans: Mutex<Vec<InstallPlan>>,
}

/// Plans kept for "Install".
const KEEP_PLANS: usize = 16;

impl Market {
    pub fn new(fetch: Box<dyn Fetch>, cache_dir: PathBuf) -> Self {
        Self { fetch, dir: cache_dir, kits: None, ttl: CACHE_TTL, seen: Mutex::new(HashMap::new()), listed: Mutex::new(Default::default()), plans: Mutex::new(vec![]) }
    }

    /// The kits that write, snapshot and roll back (the app passes its own, so snapshots show in "Наборы").
    pub fn with_kits(mut self, kits: Arc<Kits>) -> Self {
        self.kits = Some(kits);
        self
    }

    /// Test seam: how long a saved answer counts as fresh.
    #[doc(hidden)]
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    fn kits(&self) -> Result<&Kits> {
        self.kits.as_deref().ok_or_else(|| MarketError::Io("kits_not_set".into()))
    }

    // ---------------- sources ----------------

    fn sources_file(&self) -> PathBuf {
        self.dir.join("sources.json")
    }

    pub fn sources(&self) -> Result<Vec<Source>> {
        let saved: Vec<Source> = std::fs::read(self.sources_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let mut out = sources::builtins();
        for b in out.iter_mut() {
            if let Some(s) = saved.iter().find(|s| s.id == b.id) {
                b.enabled = s.enabled;
            }
        }
        out.extend(saved.into_iter().filter(|s| !s.builtin && !sources::is_builtin(&s.id)));
        Ok(out)
    }

    /// Built-in sources keep their address (only on/off is taken); own sources are checked: HTTPS, a GitHub
    /// `owner/repo` becomes its `marketplace.json`.
    pub fn set_sources(&self, list: Vec<Source>) -> Result<()> {
        let mut out: Vec<Source> = vec![];
        for s in list {
            let s = if sources::is_builtin(&s.id) { Source { builtin: true, ..s } } else { sources::own(s)? };
            if out.iter().any(|o| o.id == s.id) {
                return Err(MarketError::BadSource(format!("duplicate id: {}", s.id)));
            }
            out.push(s);
        }
        let b = serde_json::to_vec_pretty(&out).map_err(io)?;
        write_atomic(&self.sources_file(), &b)
    }

    // ---------------- network with a saved copy ----------------

    /// GET through the 24 h cache: fresh copy as is; else ask (with ETag); no answer → the saved copy and its date.
    pub(crate) fn cached_get(&self, url: &str, max: usize) -> Result<(Vec<u8>, Option<DateTime<Local>>)> {
        let key = crate::kits::sha256_hex(url.as_bytes());
        let meta_p = self.dir.join("http").join(format!("{}.json", &key[..32]));
        let body_p = meta_p.with_extension("body");
        let meta: Option<CacheMeta> = std::fs::read(&meta_p).ok().and_then(|b| serde_json::from_slice(&b).ok()).filter(|m: &CacheMeta| m.url == url);
        let saved = meta.as_ref().and_then(|_| std::fs::read(&body_p).ok());
        if let (Some(m), Some(b)) = (&meta, &saved) {
            let age = Local::now().signed_duration_since(m.at).to_std().unwrap_or_default();
            if age < self.ttl {
                return Ok((b.clone(), None));
            }
        }
        let etag = meta.as_ref().and_then(|m| m.etag.clone()).filter(|_| saved.is_some());
        let res = self.fetch.get(url, max, etag.as_deref());
        let stale = |e: MarketError| match (&meta, &saved) {
            (Some(m), Some(b)) => Ok((b.clone(), Some(m.at))),
            _ => Err(e),
        };
        match res {
            Ok(o) if o.status == 304 && saved.is_some() => {
                let m = CacheMeta { url: url.into(), at: Local::now(), etag: o.etag.or(etag) };
                let _ = write_atomic(&meta_p, &serde_json::to_vec(&m).unwrap_or_default());
                Ok((saved.unwrap_or_default(), None))
            }
            Ok(o) if (200..300).contains(&o.status) => {
                let m = CacheMeta { url: url.into(), at: Local::now(), etag: o.etag };
                let _ = write_atomic(&body_p, &o.body);
                let _ = write_atomic(&meta_p, &serde_json::to_vec(&m).unwrap_or_default());
                Ok((o.body, None))
            }
            Ok(o) => stale(MarketError::Http { url: url.into(), status: o.status }),
            Err(e) => stale(e.into()),
        }
    }

    /// GET without the cache (archives): 2xx only.
    pub(crate) fn get(&self, url: &str, max: usize) -> Result<Vec<u8>> {
        let o = self.fetch.get(url, max, None)?;
        if !(200..300).contains(&o.status) {
            return Err(MarketError::Http { url: url.into(), status: o.status });
        }
        Ok(o.body)
    }

    // ---------------- search and detail ----------------

    /// One portion from every enabled source of `kind`; `cursor` carries each source's own position. A source that
    /// did not answer is listed in `offline` (with its saved list if there is one); all of them failing is an error.
    pub fn search(&self, kind: Kind, query: &str, cursor: Option<&str>) -> Result<Page<Item>> {
        let pos: BTreeMap<String, String> = match cursor {
            Some(c) => serde_json::from_str(c).map_err(|_| MarketError::BadSource("cursor".into()))?,
            None => BTreeMap::new(),
        };
        let mut page = Page { items: vec![], cursor: None, offline: vec![] };
        let mut next = BTreeMap::new();
        let mut asked = 0;
        let mut first_err = None;
        for s in self.sources()?.into_iter().filter(|s| s.enabled && s.kind == kind) {
            let at = pos.get(&s.id).map(String::as_str);
            if cursor.is_some() && at.is_none() {
                continue;
            }
            asked += 1;
            match sources::search(self, &s, query.trim(), at) {
                Ok((items, more, stale)) => {
                    page.items.extend(items);
                    if let Some(m) = more {
                        next.insert(s.id.clone(), m);
                    }
                    if let Some(at) = stale {
                        page.offline.push(Offline { source: s.id.clone(), reason: "no_connection".into(), cached_at: Some(at) });
                    }
                }
                Err(e) => {
                    page.offline.push(Offline { source: s.id.clone(), reason: e.key().into(), cached_at: None });
                    first_err.get_or_insert(e);
                }
            }
        }
        if asked > 0 && page.items.is_empty() && page.offline.len() == asked && page.offline.iter().all(|o| o.cached_at.is_none()) {
            return Err(first_err.unwrap_or_else(|| MarketError::NotFound(query.into())));
        }
        // an id met twice (two entries of one marketplace...) keeps its first item, as the window does: the card shown
        // and the item installed are one; a next portion never replaces what the window already lists
        let mut seen = self.seen.lock().unwrap();
        let mut listed = self.listed.lock().unwrap();
        let tag = format!("{kind:?}/");
        if cursor.is_none() {
            listed.retain(|x| !x.starts_with(&tag));
        }
        page.items.retain(|it| listed.insert(format!("{tag}{}", it.id)));
        for it in &page.items {
            seen.insert(it.id.clone(), it.clone());
        }
        page.cursor = (!next.is_empty()).then(|| serde_json::to_string(&next).unwrap_or_default());
        Ok(page)
    }

    /// An item seen in a search or made from a link (the window sends ids only; an item is never taken from it).
    pub fn known(&self, id: &str) -> Option<Item> {
        self.seen.lock().unwrap().get(id).cloned()
    }

    /// An item from a link the human gave: a GitHub address (`https://github.com/o/r[/tree/<ref>/<path>]`), another git
    /// repository, a `.zip` or a folder (absolute paths).
    pub fn from_link(&self, kind: Kind, link: &str) -> Result<Item> {
        let it = sources::from_link(kind, link)?;
        self.seen.lock().unwrap().insert(it.id.clone(), it.clone());
        Ok(it)
    }

    pub fn item(&self, id: &str) -> Result<ItemDetail> {
        let item = self.known(id).ok_or_else(|| MarketError::NotFound(id.into()))?;
        build::detail(self, item)
    }

    // ---------------- install ----------------

    /// Download the item, check it and say what installing it into `env` would change. Writes nothing outside the
    /// cache folder.
    pub fn install(&self, item: &Item, env: &str, scope: Scope) -> Result<InstallPlan> {
        let kits = self.kits()?;
        let built = build::build(self, item, env)?;
        // the snapshot names the catalog item, not an address: "Проверить обновление" in "Наборы" would clone it as a kit
        // (a homepage is any text from the source); the catalog checks its own updates
        let source = crate::kits::KitSource::Repo(format!("{}{}", crate::kits::CATALOG_SOURCE, item.id));
        let kit = kits.preview_built(env, &item.name, source, built.commit.clone(), &scope, built.files, built.env_vars)?;
        let mut item = item.clone();
        if let Some(v) = built.version {
            item.version = Some(v);
        }
        let installed = self.records().iter().any(|r| r.env == env && r.item.id == item.id && r.scope == scope && self.snapshot_live(env, &r.snapshot));
        let writes = kit.entries.iter().filter(|e| e.held.is_none() && e.change != crate::kits::Change::Same).map(|e| e.target.clone()).collect();
        let executable: Vec<String> = kit.entries.iter().filter(|e| e.held.is_some()).map(|e| e.key.clone()).collect();
        let mut warnings = built.warnings;
        if !executable.is_empty() && !warnings.iter().any(|w| w == "runs_code") && built.runs.as_deref().is_none_or(|r| !r.starts_with("https://")) {
            warnings.push("runs_code".into());
        }
        let plan = InstallPlan {
            id: kit.id.clone(),
            item,
            env: env.into(),
            scope,
            writes,
            executable,
            runs: built.runs,
            needs: built.needs,
            warnings,
            texts: built.texts,
            installed,
            kit,
        };
        let mut plans = self.plans.lock().unwrap();
        plans.push(plan.clone());
        if plans.len() > KEEP_PLANS {
            plans.remove(0);
        }
        Ok(plan)
    }

    /// The plan made by `install` with this id (the window sends back only the id).
    pub fn plan(&self, id: &str) -> Option<InstallPlan> {
        self.plans.lock().unwrap().iter().find(|p| p.id == id).cloned()
    }

    /// Install the plan (taken by its id from what `install` made): files, snapshot, then each held entry whose key the
    /// human ticked. Held entries not ticked stay off and can be turned on later in "Наборы".
    pub fn apply(&self, plan: &InstallPlan, enabled_exec: Vec<String>) -> Result<SnapshotInfo> {
        let kits = self.kits()?;
        let p = self.plan(&plan.id).ok_or(MarketError::Kit(KitError::Changed))?;
        let snap = kits.install(&p.kit, &Default::default())?;
        self.plans.lock().unwrap().retain(|x| x.id != p.id);
        let mut recs = self.records();
        recs.push(Record {
            env: p.env.clone(),
            item: p.item.clone(),
            scope: p.scope.clone(),
            version: p.item.version.clone().or(p.kit.commit.clone()).unwrap_or_default(),
            snapshot: snap.clone(),
            at: Local::now(),
        });
        self.save_records(&recs)?;
        for key in enabled_exec.iter().filter(|k| p.executable.contains(k)) {
            kits.enable_item(&p.env, &crate::kits::HeldRef { snapshot: snap.clone(), key: key.clone() })?;
        }
        kits.snapshots(&p.env).into_iter().find(|s| s.id == snap).ok_or(MarketError::NotFound(snap))
    }

    fn records(&self) -> Vec<Record> {
        std::fs::read(self.dir.join("installed.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn save_records(&self, r: &[Record]) -> Result<()> {
        write_atomic(&self.dir.join("installed.json"), &serde_json::to_vec_pretty(r).map_err(io)?)
    }

    fn snapshot_live(&self, env: &str, id: &str) -> bool {
        self.kits.as_ref().is_some_and(|k| k.snapshots(env).iter().any(|s| s.id == id && matches!(s.state, SnapshotState::Done | SnapshotState::NotApplied)))
    }

    /// What the catalog installed into `env` and is still there, newest install per item, with "есть обновление".
    pub fn installed(&self, env: &str) -> Result<Vec<Installed>> {
        let mut out: Vec<Installed> = vec![];
        for r in self.records().into_iter().rev().filter(|r| r.env == env && self.snapshot_live(env, &r.snapshot)) {
            if out.iter().any(|o| o.id == r.item.id && o.scope == r.scope) {
                continue;
            }
            let update = build::latest(self, &r.item).filter(|v| !v.is_empty() && *v != r.version);
            out.push(Installed { id: r.item.id.clone(), version: r.version, update, name: r.item.name.clone(), kind: r.item.kind, scope: r.scope, snapshot: r.snapshot, at: r.at });
        }
        Ok(out)
    }

    /// Remove an installed item: every snapshot of it is rolled back, newest first (as it was before).
    pub fn remove(&self, env: &str, id: &str) -> Result<()> {
        let kits = self.kits()?;
        let recs = self.records();
        let mine: Vec<&Record> = recs.iter().filter(|r| r.env == env && r.item.id == id).collect();
        if mine.is_empty() {
            return Err(MarketError::NotFound(id.into()));
        }
        for r in mine.iter().rev() {
            kits.rollback(&r.snapshot)?;
        }
        let rest: Vec<Record> = recs.iter().filter(|r| !(r.env == env && r.item.id == id)).cloned().collect();
        self.save_records(&rest)
    }
}

pub(crate) fn write_atomic(p: &Path, b: &[u8]) -> Result<()> {
    let dir = p.parent().ok_or_else(|| io("no parent"))?;
    std::fs::create_dir_all(dir).map_err(io)?;
    let tmp = dir.join(format!(".{}.tmp-{}", p.file_name().unwrap_or_default().to_string_lossy(), uuid::Uuid::new_v4().simple()));
    std::fs::write(&tmp, b).map_err(io)?;
    std::fs::rename(&tmp, p).map_err(io)
}
