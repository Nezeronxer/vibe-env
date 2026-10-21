//! Kits (G25): what an environment already uses, export of its skills / rules / hooks / MCP /
//! subagents / settings into a kit, and a careful import - plan first, snapshot before any
//! write, hooks and MCP servers off until turned on, secrets only in the keyring.
//!
//! Works only with files: the CLI folders under `home`, a project folder, and `data_dir`.
//! No network except `git clone` of a link the human gave (R13).

mod diff;
mod layout;
mod scrub;
mod secrets;
mod snap;
mod source;
mod toml_lite;
mod units;

pub use secrets::{SecretSaved, SecretStore, SystemKeyring};
pub use snap::SnapshotState;

use chrono::{DateTime, Local};
use layout::Layout;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use snap::{Meta, Snaps};
use source::LoadedKit;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use units::{ConfOp, DiskTree, MapTree, Tree, Unit};

pub type SnapshotId = String;

pub const KIT_FORMAT: u32 = 1;
pub const KIT_KIND: &str = "nightshift-kit";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    Skills,
    Rules,
    Hooks,
    Mcp,
    Agents,
    Settings,
}

impl Group {
    pub const ALL: [Group; 6] = [Group::Skills, Group::Rules, Group::Hooks, Group::Mcp, Group::Agents, Group::Settings];
}

/// "For all projects" (the environment folder in the home) or "only this project".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Global,
    Project(PathBuf),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum KitSource {
    /// `https://`, `ssh://`, `git@…` or an absolute local repository folder.
    Repo(String),
    Folder(PathBuf),
    Zip(PathBuf),
}

#[derive(Serialize, Clone, Debug)]
pub struct GroupInfo {
    pub group: Group,
    /// The layout of this group is confirmed for the environment (spec §26).
    pub available: bool,
    /// Available and not empty: the checkbox can be ticked.
    pub active: bool,
    /// `layout_unverified` - UI: "раскладка <среды> не сверена".
    pub note: Option<String>,
    pub count: usize,
    pub items: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Inventory {
    pub env: String,
    pub env_name: String,
    pub scope: Scope,
    /// Folder the environment reads, `~`-style (`~/.claude`) or the project path.
    pub dir: String,
    /// Something was found: "Найдены твои скиллы и правила (<dir>) - подключены".
    pub found: bool,
    pub groups: Vec<GroupInfo>,
    /// Settings keys that run something or widen permissions: listed by name, held on import.
    pub dangerous: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Selection {
    pub name: String,
    pub groups: Vec<Group>,
    pub scope: Scope,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ExportDest {
    Folder(PathBuf),
    Zip(PathBuf),
    /// A new local repository, initialised and committed; never pushed.
    Git(PathBuf),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EnvVarNeed {
    pub name: String,
    /// Units that need it (`mcp:github`, `hook:PreToolUse:…`).
    pub used_by: Vec<String>,
    /// Not set through `set_secret` for this environment yet.
    #[serde(default)]
    pub missing: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Skipped {
    pub path: String,
    /// `secret_file`, `too_large`, `binary_with_secret`, `not_in_layout`, `layout_unverified`, `inline_form`.
    pub reason: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct ExportReport {
    pub dest: PathBuf,
    pub files: Vec<String>,
    pub env_vars: Vec<EnvVarNeed>,
    pub secrets_replaced: usize,
    pub skipped: Vec<Skipped>,
    /// Exported settings keys that run something or widen permissions (held on import).
    pub dangerous: Vec<String>,
    /// Commit of the local repository (`ExportDest::Git`).
    pub commit: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    Add,
    Replace,
    Same,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HeldKind {
    Hook,
    Mcp,
    /// "содержит скрипты"
    ScriptedSkill,
    /// A setting that runs a program (status line, helper, notify…).
    Command,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ScriptText {
    pub path: String,
    pub text: String,
}

/// Something that runs code: shown in full and installed off.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Held {
    pub kind: HeldKind,
    pub event: Option<String>,
    /// The whole entry (hook / server / setting) as it will be written.
    pub text: String,
    /// Full text of every script it runs that the kit carries.
    pub scripts: Vec<ScriptText>,
}

#[derive(Serialize, Clone, Debug)]
pub struct PlanEntry {
    pub key: String,
    pub group: Group,
    pub label: String,
    /// Where it goes, `~`-style.
    pub target: String,
    pub change: Change,
    /// For `Replace`: old → new, line by line.
    pub diff: Option<String>,
    /// Hooks, MCP servers, scripted skills, running settings: installed off.
    pub held: Option<Held>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ImportPlan {
    pub id: String,
    pub env: String,
    pub kit_name: String,
    pub source: KitSource,
    pub commit: Option<String>,
    pub scope: Scope,
    pub target_dir: String,
    pub entries: Vec<PlanEntry>,
    pub env_vars: Vec<EnvVarNeed>,
    pub skipped: Vec<Skipped>,
    /// Project scope: files written into the project (relative), for `Project.copy_files`.
    pub project_files: Vec<String>,
    /// "файлы набора в проекте не закоммичены - закоммить, если нужны всем"
    pub uncommitted_in_project: bool,
    /// `check_update`: commit installed before (None for a first preview).
    pub update_from: Option<String>,
    pub kit_sha: String,
}

/// What the human unticked on the import screen.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Choices {
    pub skip: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HeldItem {
    pub key: String,
    pub group: Group,
    pub kind: HeldKind,
    pub label: String,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HeldRef {
    pub snapshot: SnapshotId,
    pub key: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct SnapshotInfo {
    pub id: SnapshotId,
    pub env: String,
    pub kit_name: String,
    pub source: KitSource,
    pub commit: Option<String>,
    pub scope: Scope,
    pub at: DateTime<Local>,
    pub state: SnapshotState,
    pub held: Vec<HeldItem>,
    pub project_files: Vec<String>,
    /// Files this snapshot can put back (`~`-style).
    pub changed: Vec<String>,
}

/// A kit installed from somewhere, for `check_update`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Installed {
    pub env: String,
    pub kit_name: String,
    pub source: KitSource,
    pub commit: Option<String>,
    pub scope: Scope,
    pub snapshot: SnapshotId,
}

impl SnapshotInfo {
    pub fn installed(&self) -> Installed {
        Installed {
            env: self.env.clone(),
            kit_name: self.kit_name.clone(),
            source: self.source.clone(),
            commit: self.commit.clone(),
            scope: self.scope.clone(),
            snapshot: self.id.clone(),
        }
    }
}

/// An install found unfinished at start and rolled back.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Recovered {
    pub snapshot: SnapshotId,
    pub env: String,
    pub kit_name: String,
    /// i18n key: "установка <kit_name> прервана - откатил".
    pub message: String,
}

#[derive(thiserror::Error, Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum KitError {
    #[error("not a kit: {0}")]
    NotAKit(String),
    #[error("broken zip: {0}")]
    BadZip(String),
    #[error("path leaves the kit folder: {0}")]
    UnsafePath(String),
    #[error("symlink in kit: {0}")]
    Symlink(String),
    #[error("file over 10 MB: {path}")]
    FileTooLarge { path: String, size: u64 },
    #[error("kit over 50 MB")]
    KitTooLarge { size: u64 },
    #[error("too many files: {0}")]
    TooManyFiles(usize),
    #[error("kit is damaged: {0}")]
    Corrupt(String),
    /// "Это набор Codex. В Claude Code он не ставится…" + "Открыть в <kit_env>".
    #[error("kit for {kit_env} cannot go into {target_env}")]
    WrongEnv { kit_env: String, kit_env_name: String, target_env: String, target_env_name: String },
    #[error("clone failed: {0}")]
    CloneFailed(String),
    #[error("unknown environment: {0}")]
    UnknownEnv(String),
    #[error("destination is not empty: {0}")]
    DestNotEmpty(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("kit changed since preview")]
    Changed,
    #[error("bad name: {0}")]
    BadName(String),
    #[error("install interrupted")]
    Interrupted,
    #[error("io: {0}")]
    Io(String),
}

fn io(e: impl std::fmt::Display) -> KitError {
    KitError::Io(e.to_string())
}

pub(crate) struct Manifest {
    pub name: String,
    pub env: String,
    pub files: Vec<ManifestFile>,
    pub env_vars: Vec<EnvVarNeed>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct ManifestFile {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

/// `kit.json`.
#[derive(Serialize, Deserialize)]
struct KitJson {
    format: u32,
    kind: String,
    name: String,
    env: String,
    app_version: String,
    created_at: String,
    groups: Vec<Group>,
    files: Vec<ManifestFile>,
    env_vars: Vec<EnvVarNeed>,
}

impl Manifest {
    pub(crate) fn parse(raw: &[u8]) -> Result<Manifest, KitError> {
        let k: KitJson = serde_json::from_slice(raw).map_err(|_| KitError::NotAKit("bad_kit_json".into()))?;
        if k.kind != KIT_KIND {
            return Err(KitError::NotAKit("bad_kind".into()));
        }
        if k.format != KIT_FORMAT {
            return Err(KitError::NotAKit(format!("format_{}", k.format)));
        }
        let name: String = k.name.chars().filter(|c| !c.is_control()).take(100).collect();
        let mut seen = BTreeSet::new();
        for f in &k.files {
            let p = source::safe_rel(&f.path)?;
            if p != f.path || p == "kit.json" || !seen.insert(p) {
                return Err(KitError::UnsafePath(f.path.clone()));
            }
        }
        let env_vars = k.env_vars.into_iter().filter(|v| secrets::valid_name(&v.name)).collect();
        Ok(Manifest { name, env: k.env, files: k.files, env_vars })
    }
}

fn env_name(env: &str) -> String {
    layout::layout(env).map(|l| l.name.to_string()).unwrap_or_else(|| env.to_string())
}

pub struct Kits {
    home: PathBuf,
    data: PathBuf,
    store: Box<dyn SecretStore>,
    cache: Mutex<BTreeMap<String, Arc<LoadedKit>>>,
    crash_after: Option<usize>,
}

impl Kits {
    /// `home` - the user's home folder (CLI folders live under it); `data_dir` - the app's data.
    pub fn new(home: impl Into<PathBuf>, data_dir: impl Into<PathBuf>) -> Kits {
        Kits { home: home.into(), data: data_dir.into(), store: Box::new(SystemKeyring), cache: Mutex::new(BTreeMap::new()), crash_after: None }
    }

    /// Replace the system keyring (tests, or a host without one).
    pub fn with_secret_store(mut self, store: Box<dyn SecretStore>) -> Kits {
        self.store = store;
        self
    }

    /// Test seam: the install stops dead after `n` file writes, as if the app was killed -
    /// no clean-up, the journal stays open for `recover()`.
    #[doc(hidden)]
    pub fn crash_after_writes(mut self, n: usize) -> Kits {
        self.crash_after = Some(n);
        self
    }

    fn kits_dir(&self) -> PathBuf {
        self.data.join("kits")
    }
    fn snaps(&self) -> Snaps {
        Snaps { root: self.kits_dir().join("snapshots") }
    }
    fn secrets(&self) -> secrets::Secrets {
        secrets::Secrets { dir: self.kits_dir() }
    }

    fn layout(&self, env: &str) -> Result<&'static Layout, KitError> {
        layout::layout(env).ok_or_else(|| KitError::UnknownEnv(env.to_string()))
    }

    fn canon_home(&self) -> PathBuf {
        self.home.canonicalize().unwrap_or_else(|_| self.home.clone())
    }

    fn tree<'a>(&self, l: &'a Layout, scope: &Scope) -> DiskTree<'a> {
        match scope {
            Scope::Global => DiskTree { layout: l, base: self.home.clone(), project: false, allowed: self.canon_home() },
            Scope::Project(p) => DiskTree { layout: l, base: p.clone(), project: true, allowed: p.canonicalize().unwrap_or_else(|_| p.clone()) },
        }
    }

    fn tilde(&self, p: &Path) -> String {
        let canon = self.canon_home();
        match p.strip_prefix(&self.home).or_else(|_| p.strip_prefix(&canon)) {
            Ok(r) if r.as_os_str().is_empty() => "~".into(),
            Ok(r) => format!("~/{}", r.display()),
            Err(_) => p.display().to_string(),
        }
    }

    fn check_scope(&self, scope: &Scope) -> Result<(), KitError> {
        if let Scope::Project(p) = scope {
            if !p.is_absolute() || !p.is_dir() {
                return Err(KitError::NotFound(p.display().to_string()));
            }
        }
        Ok(())
    }

    // ---------------- inventory ----------------

    pub fn inventory(&self, env: &str, scope: &Scope) -> Inventory {
        let Some(l) = layout::layout(env) else {
            return Inventory {
                env: env.into(),
                env_name: env.into(),
                scope: scope.clone(),
                dir: String::new(),
                found: false,
                groups: Group::ALL.iter().map(|g| GroupInfo { group: *g, available: false, active: false, note: Some("layout_unverified".into()), count: 0, items: vec![] }).collect(),
                dangerous: vec![],
            };
        };
        let t = self.tree(l, scope);
        let cut = if l.home_dir.is_empty() { None } else { units::cut(l, &self.home.to_string_lossy(), &t, None, false).ok() };
        let units = cut.map(|c| c.units).unwrap_or_default();
        let groups: Vec<GroupInfo> = Group::ALL
            .iter()
            .map(|g| {
                let items: Vec<String> = units.iter().filter(|u| u.group == *g).map(|u| u.label.clone()).collect();
                let available = l.available(*g);
                GroupInfo {
                    group: *g,
                    available,
                    active: available && !items.is_empty(),
                    note: (!available).then(|| "layout_unverified".to_string()),
                    count: items.len(),
                    items,
                }
            })
            .collect();
        let dir = match scope {
            Scope::Global if !l.home_dir.is_empty() => format!("~/{}", l.home_dir),
            Scope::Global => String::new(),
            Scope::Project(p) => p.join(l.project_dir).display().to_string(),
        };
        let dangerous = dangerous(units.iter());
        Inventory { env: env.into(), env_name: l.name.into(), scope: scope.clone(), dir, found: groups.iter().any(|g| g.count > 0), groups, dangerous }
    }

    // ---------------- export ----------------

    pub fn export(&self, env: &str, sel: &Selection, dest: ExportDest) -> Result<ExportReport, KitError> {
        let l = self.layout(env)?;
        self.check_scope(&sel.scope)?;
        let name: String = sel.name.chars().filter(|c| !c.is_control()).take(100).collect();
        if name.trim().is_empty() {
            return Err(KitError::BadName(sel.name.clone()));
        }
        let mut skipped = vec![];
        for g in &sel.groups {
            if !l.available(*g) {
                skipped.push(Skipped { path: format!("{g:?}").to_lowercase(), reason: "layout_unverified".into() });
            }
        }
        let t = self.tree(l, &sel.scope);
        let cut = if l.home_dir.is_empty() { units::Cut { units: vec![], loose: vec![], notes: vec![] } } else { units::cut(l, &self.home.to_string_lossy(), &t, None, false)? };
        let chosen: Vec<&Unit> = cut.units.iter().filter(|u| sel.groups.contains(&u.group)).collect();
        if sel.groups.iter().any(|g| matches!(g, Group::Hooks | Group::Mcp)) {
            skipped.extend(cut.notes.iter().map(|(p, r)| Skipped { path: p.clone(), reason: r.clone() }));
        }

        let mut sc = scrub::Scrubber::new(&self.home.to_string_lossy());
        let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut text_files: BTreeSet<String> = BTreeSet::new();
        let mut conf_ops: BTreeMap<String, Vec<ConfOp>> = BTreeMap::new();
        for u in &chosen {
            for f in &u.files {
                if files.contains_key(f) {
                    continue;
                }
                if scrub::is_secret_file(f) {
                    skipped.push(Skipped { path: f.clone(), reason: "secret_file".into() });
                    continue;
                }
                let Some(b) = t.read(f) else { continue };
                if b.len() as u64 > source::MAX_FILE {
                    skipped.push(Skipped { path: f.clone(), reason: "too_large".into() });
                    continue;
                }
                match String::from_utf8(b) {
                    Ok(s) => {
                        files.insert(f.clone(), sc.text(&s, &u.key).into_bytes());
                        text_files.insert(f.clone());
                    }
                    Err(e) => {
                        files.insert(f.clone(), e.into_bytes());
                    }
                }
            }
            if let Some(op) = &u.conf {
                let key = u.key.clone();
                let sc_cell = std::cell::RefCell::new(&mut sc);
                let scrubbed = units::map_op(
                    op,
                    &mut |s| {
                        let mut s2 = sc_cell.borrow_mut();
                        let out = s2.toml(s, &key);
                        s2.note_refs(&out, &key);
                        out
                    },
                    &mut |v| {
                        let mut s2 = sc_cell.borrow_mut();
                        s2.json(v, &key);
                        let txt = v.to_string();
                        s2.note_refs(&txt, &key);
                    },
                );
                conf_ops.entry(op.file().to_string()).or_default().push(scrubbed);
            }
        }
        for (file, ops) in &conf_ops {
            let refs: Vec<&ConfOp> = ops.iter().collect();
            files.insert(file.clone(), units::apply_conf(None, &refs)?);
            text_files.insert(file.clone());
        }
        // last pass: a secret value found in one place is removed everywhere
        let mut total = 0u64;
        let keys: Vec<String> = files.keys().cloned().collect();
        for k in keys {
            if text_files.contains(&k) {
                let s = String::from_utf8_lossy(&files[&k]).to_string();
                files.insert(k.clone(), sc.final_pass(&s).into_bytes());
            } else if !sc.binary_is_clean(&files[&k]) {
                files.remove(&k);
                skipped.push(Skipped { path: k.clone(), reason: "binary_with_secret".into() });
                continue;
            }
            total += files[&k].len() as u64;
        }
        if total > source::MAX_TOTAL {
            return Err(KitError::KitTooLarge { size: total });
        }
        let env_vars: Vec<EnvVarNeed> =
            sc.vars.iter().map(|(n, by)| EnvVarNeed { name: n.clone(), used_by: by.iter().cloned().collect(), missing: false }).collect();
        let mut groups: Vec<Group> = chosen.iter().map(|u| u.group).collect();
        groups.sort();
        groups.dedup();
        let kj = KitJson {
            format: KIT_FORMAT,
            kind: KIT_KIND.into(),
            name,
            env: env.into(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            created_at: Local::now().to_rfc3339(),
            groups,
            files: files.iter().map(|(p, b)| ManifestFile { path: p.clone(), sha256: source::sha256_hex(b), size: b.len() as u64 }).collect(),
            env_vars: env_vars.clone(),
        };
        let mut kit_json = serde_json::to_vec_pretty(&kj).map_err(io)?;
        kit_json.push(b'\n');

        let mut commit = None;
        let dest_path = match &dest {
            ExportDest::Folder(p) | ExportDest::Git(p) => {
                if p.exists() && std::fs::read_dir(p).map_err(io)?.next().is_some() {
                    return Err(KitError::DestNotEmpty(p.display().to_string()));
                }
                for (rel, b) in &files {
                    snap::write_atomic(&p.join(rel), b, None)?;
                }
                snap::write_atomic(&p.join("kit.json"), &kit_json, None)?;
                if let ExportDest::Git(_) = dest {
                    crate::git::init_repo(p).map_err(|e| KitError::Io(e.to_string()))?;
                    commit = std::process::Command::new("git")
                        .current_dir(p)
                        .args(["rev-parse", "HEAD"])
                        .output()
                        .ok()
                        .filter(|o| o.status.success())
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
                }
                p.clone()
            }
            ExportDest::Zip(p) => {
                if p.exists() {
                    return Err(KitError::DestNotEmpty(p.display().to_string()));
                }
                let mut buf = std::io::Cursor::new(Vec::new());
                {
                    use std::io::Write;
                    let mut z = zip::ZipWriter::new(&mut buf);
                    let opt = zip::write::SimpleFileOptions::default().unix_permissions(0o644);
                    z.start_file("kit.json", opt).map_err(io)?;
                    z.write_all(&kit_json).map_err(io)?;
                    for (rel, b) in &files {
                        z.start_file(rel.as_str(), opt).map_err(io)?;
                        z.write_all(b).map_err(io)?;
                    }
                    z.finish().map_err(io)?;
                }
                snap::write_atomic(p, &buf.into_inner(), None)?;
                p.clone()
            }
        };
        Ok(ExportReport {
            dest: dest_path,
            files: files.keys().cloned().collect(),
            env_vars,
            secrets_replaced: sc.replaced,
            skipped,
            dangerous: dangerous(chosen.iter().copied()),
            commit,
        })
    }

    // ---------------- import ----------------

    /// `~/` in hook commands, MCP and settings values → this machine's home; in project scope
    /// scripts of the environment folder point into the project copy.
    fn localize(&self, l: &Layout, scope: &Scope, u: &Unit) -> Option<ConfOp> {
        let op = u.conf.as_ref()?;
        if !matches!(u.group, Group::Hooks | Group::Mcp | Group::Settings) {
            return Some(op.clone());
        }
        let home = self.home.to_string_lossy().trim_end_matches('/').to_string();
        let proj = match scope {
            Scope::Project(p) => Some(format!("{}/{}/", p.to_string_lossy().trim_end_matches('/'), l.project_dir)),
            Scope::Global => None,
        };
        let re = regex::Regex::new(r#"(^|[\s'"=:(\[,])~/"#).unwrap();
        let mut f = |s: &str| -> String {
            let mut s = s.to_string();
            if let Some(pp) = &proj {
                s = s.replace(&format!("~/{}/", l.home_dir), pp).replace(&format!("$HOME/{}/", l.home_dir), pp);
            }
            re.replace_all(&s, |c: &regex::Captures| format!("{}{home}/", &c[1])).to_string()
        };
        let mut fj = |v: &mut Value| units::map_strings(v, &mut |s| {
            let mut s = s.to_string();
            if let Some(pp) = &proj {
                s = s.replace(&format!("~/{}/", l.home_dir), pp).replace(&format!("$HOME/{}/", l.home_dir), pp);
            }
            re.replace_all(&s, |c: &regex::Captures| format!("{}{home}/", &c[1])).to_string()
        });
        Some(units::map_op(op, &mut f, &mut fj))
    }

    /// Real path to write `rel` of the environment layout, refusing anything that resolves
    /// outside the allowed folder (home for global scope, the project for project scope).
    fn target(&self, t: &DiskTree, rel: &str) -> Result<PathBuf, KitError> {
        let p = t.path(rel);
        let mut existing = p.as_path();
        let mut rest: Vec<std::ffi::OsString> = vec![];
        while std::fs::symlink_metadata(existing).is_err() {
            rest.push(existing.file_name().map(|n| n.to_os_string()).unwrap_or_default());
            existing = existing.parent().ok_or_else(|| KitError::UnsafePath(rel.into()))?;
        }
        let mut real = existing.canonicalize().map_err(|_| KitError::UnsafePath(rel.into()))?;
        for r in rest.iter().rev() {
            real.push(r);
        }
        if !real.starts_with(&t.allowed) || units::forbidden(&real) {
            return Err(KitError::UnsafePath(rel.into()));
        }
        if real.is_dir() {
            return Err(KitError::UnsafePath(rel.into()));
        }
        Ok(real)
    }

    fn entry_for(&self, l: &Layout, scope: &Scope, t: &DiskTree, kit: &MapTree, u: &Unit) -> PlanEntry {
        let mut present = 0;
        let mut same = 0;
        let mut parts = 0;
        let mut diff = String::new();
        let mut targets = vec![];
        for f in &u.files {
            parts += 1;
            let new = kit.read(f).unwrap_or_default();
            targets.push(self.tilde(&t.path(f)));
            if let Some(old) = t.read(f) {
                present += 1;
                if old == new {
                    same += 1;
                } else if !u.scripts.contains(f) || u.held.is_none() {
                    diff.push_str(&format!("@@ {f}\n"));
                    diff.push_str(&diff::diff(&String::from_utf8_lossy(&old), &String::from_utf8_lossy(&new)));
                }
            }
        }
        if let Some(op) = self.localize(l, scope, u) {
            parts += 1;
            let cur = t.read(op.file());
            let (has, eq, old, new) = units::conf_state(cur.as_deref(), &op);
            targets.push(format!("{} → {}", self.tilde(&t.path(op.file())), u.label));
            if has {
                present += 1;
            }
            if eq {
                same += 1;
            } else if has {
                diff.push_str(&diff::diff(&old, &new));
            }
        }
        let change = if parts > 0 && same == parts {
            Change::Same
        } else if present == 0 {
            Change::Add
        } else {
            Change::Replace
        };
        let held = match (u.held, change) {
            (Some(kind), Change::Add | Change::Replace) => Some(Held {
                kind,
                event: u.event.clone(),
                text: match self.localize(l, scope, u) {
                    Some(ConfOp::JsonSet { value, .. }) | Some(ConfOp::JsonPush { value, .. }) => serde_json::to_string_pretty(&value).unwrap_or_default(),
                    _ => units::unit_text(u, kit),
                },
                scripts: u.scripts.iter().map(|s| ScriptText { path: s.clone(), text: String::from_utf8_lossy(&kit.read(s).unwrap_or_default()).to_string() }).collect(),
            }),
            _ => None,
        };
        PlanEntry {
            key: u.key.clone(),
            group: u.group,
            label: u.label.clone(),
            target: targets.join(", "),
            change,
            diff: (change == Change::Replace && !diff.is_empty()).then_some(diff),
            held,
        }
    }

    fn project_rels(&self, l: &Layout, u: &Unit) -> Vec<String> {
        let mut v: Vec<String> = u.files.iter().map(|f| l.project_rel(f)).collect();
        if let Some(op) = &u.conf {
            v.push(l.project_rel(op.file()));
        }
        v
    }

    fn plan(&self, l: &'static Layout, env: &str, src: &KitSource, scope: &Scope, kit: Arc<LoadedKit>) -> Result<ImportPlan, KitError> {
        let mt = MapTree(&kit.files);
        let all: Vec<String> = kit.files.keys().cloned().collect();
        let cut = units::cut(l, &self.home.to_string_lossy(), &mt, Some(&all), true)?;
        let t = self.tree(l, scope);
        let entries: Vec<PlanEntry> = cut.units.iter().map(|u| self.entry_for(l, scope, &t, &mt, u)).collect();
        let unverified_dirs = ["skills/", "agents/", "commands/", "hooks/"];
        let mut skipped: Vec<Skipped> = cut
            .loose
            .iter()
            .map(|p| Skipped {
                path: p.clone(),
                reason: if unverified_dirs.iter().any(|d| p.starts_with(d)) || l.home_dir.is_empty() { "layout_unverified" } else { "not_in_layout" }.into(),
            })
            .collect();
        skipped.extend(cut.notes.iter().map(|(p, r)| Skipped { path: p.clone(), reason: r.clone() }));
        let set: BTreeSet<String> = self.secrets().names().get(env).cloned().unwrap_or_default().into_iter().collect();
        let env_vars = kit.manifest.env_vars.iter().map(|v| EnvVarNeed { missing: !set.contains(&v.name), ..v.clone() }).collect();
        let mut project_files = vec![];
        if let Scope::Project(_) = scope {
            for (u, e) in cut.units.iter().zip(&entries) {
                if u.held.is_none() && e.change != Change::Same {
                    for r in self.project_rels(l, u) {
                        if !project_files.contains(&r) {
                            project_files.push(r);
                        }
                    }
                }
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.cache.lock().unwrap().insert(id.clone(), kit.clone());
        Ok(ImportPlan {
            id,
            env: env.into(),
            kit_name: kit.manifest.name.clone(),
            source: src.clone(),
            commit: kit.commit.clone(),
            scope: scope.clone(),
            target_dir: match scope {
                Scope::Global => format!("~/{}", l.home_dir),
                Scope::Project(p) => p.display().to_string(),
            },
            entries,
            env_vars,
            skipped,
            uncommitted_in_project: !project_files.is_empty(),
            project_files,
            update_from: None,
            kit_sha: kit.sha.clone(),
        })
    }

    /// Read the kit and say what installing it would change. Writes nothing to the home,
    /// project or data folders (a repository is cloned into the system temp folder and removed).
    pub fn preview(&self, env: &str, src: KitSource, scope: &Scope) -> Result<ImportPlan, KitError> {
        let l = self.layout(env)?;
        self.check_scope(scope)?;
        if let KitSource::Folder(p) | KitSource::Zip(p) = &src {
            if !p.is_absolute() {
                return Err(KitError::NotFound(p.display().to_string()));
            }
        }
        let kit = source::load(&src, &source::scratch_dir())?;
        if kit.manifest.env != env {
            return Err(KitError::WrongEnv {
                kit_env: kit.manifest.env.clone(),
                kit_env_name: env_name(&kit.manifest.env),
                target_env: env.into(),
                target_env_name: l.name.into(),
            });
        }
        self.plan(l, env, &src, scope, Arc::new(kit))
    }

    fn writes_for(&self, l: &Layout, scope: &Scope, t: &DiskTree, kit: &MapTree, us: &[&Unit]) -> Result<Vec<Write>, KitError> {
        let mut out: Vec<Write> = vec![];
        let mut confs: BTreeMap<String, Vec<ConfOp>> = BTreeMap::new();
        for u in us {
            for f in &u.files {
                let p = self.target(t, f)?;
                if out.iter().any(|w| w.0 == p) {
                    continue;
                }
                let b = kit.read(f).ok_or_else(|| KitError::Corrupt(f.clone()))?;
                if t.read(f).as_deref() != Some(&b[..]) {
                    out.push(Write(p, b, vec![]));
                }
            }
            if let Some(op) = self.localize(l, scope, u) {
                confs.entry(op.file().to_string()).or_default().push(op);
            }
        }
        for (file, ops) in confs {
            let p = self.target(t, &file)?;
            let cur = t.read(&file);
            let refs: Vec<&ConfOp> = ops.iter().collect();
            let new = units::apply_conf(cur.as_deref(), &refs)?;
            if cur.as_deref() != Some(&new[..]) {
                out.push(Write(p, new, ops));
            }
        }
        Ok(out)
    }

    fn get_kit(&self, plan: &ImportPlan) -> Result<Arc<LoadedKit>, KitError> {
        if let Some(k) = self.cache.lock().unwrap().get(&plan.id) {
            return Ok(k.clone());
        }
        let k = source::load(&plan.source, &source::scratch_dir())?;
        if k.sha != plan.kit_sha {
            return Err(KitError::Changed);
        }
        Ok(Arc::new(k))
    }

    /// Write `writes` under journal `m`: remember old bytes, then temp file + rename each.
    fn apply(&self, m: &mut Meta, writes: &[Write]) -> Result<(), KitError> {
        let snaps = self.snaps();
        for Write(p, _, ops) in writes {
            snaps.remember(m, p, ops)?;
        }
        m.state = SnapshotState::Installing;
        snaps.save(m)?;
        for (i, Write(p, b, ops)) in writes.iter().enumerate() {
            if self.crash_after == Some(i) {
                return Err(KitError::Interrupted);
            }
            let mode = m.entries.iter().find(|e| &e.path == p).and_then(|e| e.mode);
            // a shared config is re-read right before the write: the CLI may have changed it
            let res = if ops.is_empty() {
                snap::write_atomic(p, b, mode)
            } else {
                let refs: Vec<&ConfOp> = ops.iter().collect();
                units::apply_conf(std::fs::read(p).ok().as_deref(), &refs).and_then(|nb| snap::write_atomic(p, &nb, mode))
            };
            if let Err(e) = res {
                let _ = snaps.restore(m);
                m.state = SnapshotState::RolledBack;
                let _ = snaps.save(m);
                return Err(e);
            }
        }
        m.state = SnapshotState::Done;
        snaps.save(m)
    }

    pub fn install(&self, plan: &ImportPlan, choices: &Choices) -> Result<SnapshotId, KitError> {
        let l = self.layout(&plan.env)?;
        self.check_scope(&plan.scope)?;
        let kit = self.get_kit(plan)?;
        if kit.manifest.env != plan.env {
            return Err(KitError::WrongEnv {
                kit_env: kit.manifest.env.clone(),
                kit_env_name: env_name(&kit.manifest.env),
                target_env: plan.env.clone(),
                target_env_name: l.name.into(),
            });
        }
        let mt = MapTree(&kit.files);
        let all: Vec<String> = kit.files.keys().cloned().collect();
        let cut = units::cut(l, &self.home.to_string_lossy(), &mt, Some(&all), true)?;
        let t = self.tree(l, &plan.scope);
        let skip: BTreeSet<&str> = choices.skip.iter().map(|s| s.as_str()).collect();
        let mut now: Vec<&Unit> = vec![];
        let mut held: Vec<HeldItem> = vec![];
        let mut project_files = vec![];
        for u in &cut.units {
            if skip.contains(u.key.as_str()) {
                continue;
            }
            let e = self.entry_for(l, &plan.scope, &t, &mt, u);
            if e.change == Change::Same {
                continue;
            }
            match u.held {
                Some(kind) => held.push(HeldItem { key: u.key.clone(), group: u.group, kind, label: u.label.clone(), enabled: false }),
                None => {
                    now.push(u);
                    if let Scope::Project(_) = plan.scope {
                        for r in self.project_rels(l, u) {
                            if !project_files.contains(&r) {
                                project_files.push(r);
                            }
                        }
                    }
                }
            }
        }
        let writes = self.writes_for(l, &plan.scope, &t, &mt, &now)?;

        let id = format!("{}-{}", Local::now().format("%Y%m%d-%H%M%S"), &uuid::Uuid::new_v4().simple().to_string()[..8]);
        let snaps = self.snaps();
        // the kit itself goes into the snapshot: held items are installed from here later
        snaps.private_dir(&id)?;
        let kdir = snaps.dir(&id).join("kit");
        for (rel, b) in kit.files.iter() {
            snap::write_atomic(&kdir.join(rel), b, Some(0o600))?;
        }
        let kj = kit_json_bytes(&kit);
        snap::write_atomic(&kdir.join("kit.json"), &kj, Some(0o600))?;
        let mut m = Meta {
            id: id.clone(),
            env: plan.env.clone(),
            kit_name: kit.manifest.name.clone(),
            source: plan.source.clone(),
            commit: kit.commit.clone(),
            scope: plan.scope.clone(),
            at: Local::now(),
            state: SnapshotState::Installing,
            entries: vec![],
            created_dirs: vec![],
            held,
            project_files,
        };
        self.apply(&mut m, &writes)?;
        snaps.prune(&m.env, &m.id);
        self.cache.lock().unwrap().remove(&plan.id);
        Ok(id)
    }

    /// Turn on one held item (hook, MCP server, scripted skill, running setting) from the
    /// snapshot of its kit; the files it changes join that snapshot, so rollback undoes it too.
    pub fn enable_item(&self, env: &str, item: &HeldRef) -> Result<(), KitError> {
        let l = self.layout(env)?;
        let snaps = self.snaps();
        let mut m = snaps.load(&item.snapshot)?;
        if m.env != env || m.state != SnapshotState::Done {
            return Err(KitError::NotFound(item.snapshot.clone()));
        }
        let Some(pos) = m.held.iter().position(|h| h.key == item.key) else {
            return Err(KitError::NotFound(item.key.clone()));
        };
        if m.held[pos].enabled {
            return Ok(());
        }
        let kit = source::load(&KitSource::Folder(snaps.dir(&m.id).join("kit")), &source::scratch_dir())?;
        let mt = MapTree(&kit.files);
        let all: Vec<String> = kit.files.keys().cloned().collect();
        let cut = units::cut(l, &self.home.to_string_lossy(), &mt, Some(&all), true)?;
        let u = cut.units.iter().find(|u| u.key == item.key).ok_or_else(|| KitError::NotFound(item.key.clone()))?;
        let t = self.tree(l, &m.scope);
        let writes = self.writes_for(l, &m.scope, &t, &mt, &[u])?;
        if let Scope::Project(_) = m.scope {
            for r in self.project_rels(l, u) {
                if !m.project_files.contains(&r) {
                    m.project_files.push(r);
                }
            }
        }
        m.held[pos].enabled = true;
        self.apply(&mut m, &writes)
    }

    pub fn snapshots(&self, env: &str) -> Vec<SnapshotInfo> {
        self.snaps()
            .all()
            .into_iter()
            .filter(|m| m.env == env)
            .map(|m| SnapshotInfo {
                changed: m.entries.iter().map(|e| self.tilde(&e.path)).collect(),
                id: m.id,
                env: m.env,
                kit_name: m.kit_name,
                source: m.source,
                commit: m.commit,
                scope: m.scope,
                at: m.at,
                state: m.state,
                held: m.held,
                project_files: m.project_files,
            })
            .collect()
    }

    /// Put back every file the kit (and items turned on later) changed; remove what it added.
    pub fn rollback(&self, id: &str) -> Result<(), KitError> {
        let snaps = self.snaps();
        let mut m = snaps.load(id)?;
        if matches!(m.state, SnapshotState::RolledBack | SnapshotState::Interrupted) {
            return Ok(());
        }
        snaps.restore(&m)?;
        m.state = SnapshotState::RolledBack;
        snaps.save(&m)
    }

    /// At start: an install left open (the app quit or the disk failed mid-way) is rolled back.
    pub fn recover(&self) -> Vec<Recovered> {
        let snaps = self.snaps();
        snaps.sweep_orphans();
        let mut out = vec![];
        for mut m in snaps.all() {
            if m.state != SnapshotState::Installing {
                continue;
            }
            if snaps.restore(&m).is_ok() {
                m.state = SnapshotState::Interrupted;
                let _ = snaps.save(&m);
                out.push(Recovered { snapshot: m.id.clone(), env: m.env.clone(), kit_name: m.kit_name.clone(), message: "kit_install_interrupted_rolled_back".into() });
            }
        }
        out
    }

    /// Same plan as `preview`, for the source a kit was installed from. Installs nothing.
    pub fn check_update(&self, inst: &Installed) -> Result<ImportPlan, KitError> {
        let mut p = self.preview(&inst.env, inst.source.clone(), &inst.scope)?;
        p.update_from = inst.commit.clone();
        Ok(p)
    }

    // ---------------- secrets ----------------

    pub fn set_secret(&self, env: &str, name: &str, value: &str) -> Result<SecretSaved, KitError> {
        if layout::layout(env).is_none() && !env.starts_with("custom:") {
            return Err(KitError::UnknownEnv(env.into()));
        }
        if !secrets::valid_name(name) {
            return Err(KitError::BadName(name.into()));
        }
        self.secrets().set(self.store.as_ref(), env, name, value)
    }

    /// `(NAME, value)` pairs for `Run.extra_env` of this environment's runs.
    pub fn env_vars(&self, env: &str) -> Vec<(String, String)> {
        let s = self.secrets();
        s.names()
            .get(env)
            .into_iter()
            .flatten()
            .filter_map(|n| s.get(self.store.as_ref(), env, n).map(|v| (n.clone(), v)))
            .collect()
    }
}

/// One file to write: path, bytes, and the config ops behind them (empty for plain files).
struct Write(PathBuf, Vec<u8>, Vec<ConfOp>);

fn dangerous<'a>(us: impl Iterator<Item = &'a Unit>) -> Vec<String> {
    us.filter(|u| u.group == Group::Settings && u.held == Some(HeldKind::Command)).map(|u| u.label.clone()).collect()
}

fn kit_json_bytes(kit: &LoadedKit) -> Vec<u8> {
    let kj = KitJson {
        format: KIT_FORMAT,
        kind: KIT_KIND.into(),
        name: kit.manifest.name.clone(),
        env: kit.manifest.env.clone(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        created_at: Local::now().to_rfc3339(),
        groups: vec![],
        files: kit.files.iter().map(|(p, b)| ManifestFile { path: p.clone(), sha256: source::sha256_hex(b), size: b.len() as u64 }).collect(),
        env_vars: kit.manifest.env_vars.clone(),
    };
    serde_json::to_vec_pretty(&kj).unwrap_or_default()
}
