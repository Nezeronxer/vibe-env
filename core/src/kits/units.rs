//! A kit (or an environment folder) seen as "units": one skill, one rules file, one hook,
//! one MCP server, one settings key. The same cutting is used for the inventory, the export
//! and the import plan, so what is counted is what is moved.

use super::layout::{ConfFmt, HookShape, Layout, KIT_MCP};
use super::toml_lite::{self as tl, Block};
use super::{Group, HeldKind, KitError};
use regex::Regex;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub(crate) trait Tree {
    fn read(&self, rel: &str) -> Option<Vec<u8>>;
    /// Every file under `dir` (recursive), as kit-relative paths.
    fn list(&self, dir: &str) -> Vec<String>;
}

pub(crate) struct MapTree<'a>(pub &'a BTreeMap<String, Vec<u8>>);

impl Tree for MapTree<'_> {
    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        self.0.get(rel).cloned()
    }
    fn list(&self, dir: &str) -> Vec<String> {
        let p = format!("{dir}/");
        self.0.keys().filter(|k| k.starts_with(&p)).cloned().collect()
    }
}

/// The environment's own folder on disk (global or project scope).
pub(crate) struct DiskTree<'a> {
    pub layout: &'a Layout,
    /// Home folder (global scope) or project root.
    pub base: PathBuf,
    pub project: bool,
    /// Symlinks are followed only while they stay inside this folder.
    pub allowed: PathBuf,
}

const SENSITIVE: &[&str] = &[".ssh", ".gnupg", ".aws", ".kube", ".docker", ".password-store", "Keychains", ".netrc"];

/// Key stores and credential files: never read for a kit, never written by one.
pub(crate) fn forbidden(real: &Path) -> bool {
    let parts: Vec<String> = real.components().map(|x| x.as_os_str().to_string_lossy().to_string()).collect();
    parts.iter().any(|x| SENSITIVE.contains(&x.as_str())) || parts.windows(2).any(|w| w[0] == ".config" && w[1] == "gh")
}

const SKIP_NAMES: &[&str] = &[".DS_Store", "node_modules", "__pycache__", ".venv", ".git"];

impl DiskTree<'_> {
    pub fn path(&self, rel: &str) -> PathBuf {
        if let (KIT_MCP, Some(m)) = (rel, &self.layout.mcp_file) {
            return self.base.join(if self.project { m.project } else { m.global });
        }
        if self.project {
            self.base.join(self.layout.project_rel(rel))
        } else {
            self.base.join(self.layout.home_dir).join(rel)
        }
    }

    /// A symlink is followed only into the allowed folder and never into key stores.
    fn inside(&self, p: &Path) -> bool {
        p.canonicalize().map(|c| c.starts_with(&self.allowed) && !forbidden(&c)).unwrap_or(false)
    }

    fn walk(&self, dir: &Path, rel: &str, depth: usize, out: &mut Vec<String>) {
        if depth > 16 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name().to_string_lossy().to_string();
            if SKIP_NAMES.contains(&name.as_str()) {
                continue;
            }
            let p = e.path();
            let r = format!("{rel}/{name}");
            let Ok(lmd) = std::fs::symlink_metadata(&p) else { continue };
            if lmd.file_type().is_symlink() && !self.inside(&p) {
                continue;
            }
            let Ok(md) = std::fs::metadata(&p) else { continue };
            if md.is_dir() {
                self.walk(&p, &r, depth + 1, out);
            } else if md.is_file() {
                out.push(r);
            }
        }
    }
}

impl Tree for DiskTree<'_> {
    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        let p = self.path(rel);
        let lmd = std::fs::symlink_metadata(&p).ok()?;
        if lmd.file_type().is_symlink() && !self.inside(&p) {
            return None;
        }
        std::fs::metadata(&p).ok().filter(|m| m.is_file())?;
        std::fs::read(&p).ok()
    }
    fn list(&self, dir: &str) -> Vec<String> {
        let p = self.path(dir);
        if std::fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_symlink()) && !self.inside(&p) {
            return vec![];
        }
        let mut out = vec![];
        self.walk(&p, dir, 0, &mut out);
        out
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum TomlSel {
    Exact(String),
    Sub(String, String),
    Prefix(String),
}

impl TomlSel {
    pub fn matches(&self, h: &str) -> bool {
        match self {
            TomlSel::Exact(x) => h == x,
            TomlSel::Sub(p, n) => tl::in_prefix(h, p) && tl::sub_name(h, p).as_deref() == Some(n.as_str()),
            TomlSel::Prefix(p) => tl::in_prefix(h, p),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum ConfOp {
    JsonSet { file: String, path: Vec<String>, value: Value },
    JsonPush { file: String, path: Vec<String>, value: Value },
    TomlKey { file: String, key: String, text: String },
    TomlTables { file: String, sel: TomlSel, text: String },
}

impl ConfOp {
    pub fn file(&self) -> &str {
        match self {
            ConfOp::JsonSet { file, .. } | ConfOp::JsonPush { file, .. } | ConfOp::TomlKey { file, .. } | ConfOp::TomlTables { file, .. } => file,
        }
    }

    /// The key as a human finds it in the file (for reports).
    pub fn label(&self) -> String {
        match self {
            ConfOp::JsonSet { path, .. } | ConfOp::JsonPush { path, .. } => path.join("."),
            ConfOp::TomlKey { key, .. } => key.clone(),
            ConfOp::TomlTables { sel: TomlSel::Exact(h) | TomlSel::Prefix(h), .. } => h.clone(),
            ConfOp::TomlTables { sel: TomlSel::Sub(p, n), .. } => format!("{p}.{n}"),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Unit {
    pub key: String,
    pub group: Group,
    pub label: String,
    pub held: Option<HeldKind>,
    /// Kit files written as they are.
    pub files: Vec<String>,
    pub conf: Option<ConfOp>,
    /// Scripts the unit runs (hook scripts, skill scripts): shown in full before install.
    pub scripts: Vec<String>,
    pub event: Option<String>,
}

impl Unit {
    fn file(group: Group, key: String, label: String, files: Vec<String>) -> Unit {
        Unit { key, group, label, held: None, files, conf: None, scripts: vec![], event: None }
    }
}

const SCRIPT_EXT: &[&str] = &[
    "sh", "bash", "zsh", "fish", "py", "js", "mjs", "cjs", "ts", "mts", "rb", "pl", "php", "ps1", "bat", "cmd", "exe", "lua", "jar", "bin", "so", "dylib",
];

pub(crate) fn is_script(rel: &str, bytes: Option<&[u8]>) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    ext.is_some_and(|e| SCRIPT_EXT.contains(&e.as_str())) || bytes.is_some_and(|b| b.starts_with(b"#!")) || (!name.contains('.') && bytes.is_some_and(|b| std::str::from_utf8(b).is_err()))
}

/// A setting goes in without a tick only when its key is on the environment's list of safe keys (`safe_settings`) and
/// nothing in it runs a program (status line, key helpers, env, notify…); the rest is held like a hook.
fn held_setting(l: &Layout, key: &str, text: &str) -> Option<HeldKind> {
    let top = key.split('.').next().unwrap_or(key);
    (!l.safe_settings.contains(&top) || runs_something(key, text)).then_some(HeldKind::Command)
}

fn runs_something(key: &str, text: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| {
        Regex::new(r#"(?i)(command|helper|statusline|refresh|hook|plugin|mcp|^env$|shell|script|exec|notify|"cmd"|"args"|\bargs\s*=|\bcommand\s*=|permission|allow|sandbox|approval|trust|bypass|yolo|danger|^tools$|autoaccept)"#).unwrap()
    });
    r.is_match(key) || r.is_match(text)
}

/// A skill or subagent file that runs something by itself: frontmatter with hooks, MCP servers, tools or permissions
/// (`hooks`, `mcpServers`, `permissionMode`, `allowed-tools`, `tools`, opencode `permission`), a `!`command`` line in
/// the text, or frontmatter that does not close (it cannot be read reliably, so it is held too). Keys are looked for
/// anywhere in the frontmatter (flow style `{hooks: …}` too): a false hit only means one more tick.
pub(crate) fn runs_in_markdown(b: &[u8]) -> bool {
    static FM: OnceLock<Regex> = OnceLock::new();
    let s = String::from_utf8_lossy(b);
    if s.contains("!`") {
        return true;
    }
    let s = s.trim_start_matches('\u{feff}');
    let Some(rest) = s.strip_prefix("---") else { return false };
    let Some(end) = rest.find("\n---") else { return true };
    let fm = FM.get_or_init(|| {
        Regex::new(r#"(?i)(?:^|[\s{,?])["']?(hooks|mcp_?servers|permission_?mode|permissions?|allowed[-_]?tools|tools)["']?\s*:"#).unwrap()
    });
    fm.is_match(&rest[..end])
}

/// Script files a hook command points to, as kit paths (`~/.claude/hooks/x.py` → `hooks/x.py`).
pub(crate) fn referenced_scripts(l: &Layout, home: &str, command: &str, exists: &dyn Fn(&str) -> bool) -> Vec<String> {
    if l.home_dir.is_empty() {
        return vec![];
    }
    let hd = regex::escape(l.home_dir);
    let pd = regex::escape(l.project_dir);
    let me = if home.len() > 1 { format!("|{}", regex::escape(home.trim_end_matches('/'))) } else { String::new() };
    let re = Regex::new(&format!(
        r#"(?:~|\$HOME|\$\{{HOME\}}|/(?:Users|home)/[A-Za-z0-9._\-]+{me})/{hd}/([^\s"'`;|&<>()]+)|(?:\$[A-Z_]*PROJECT_DIR|\$\{{[A-Z_]*PROJECT_DIR\}}|"\$[A-Z_]*PROJECT_DIR")"?/{pd}/([^\s"'`;|&<>()]+)"#
    ))
    .unwrap();
    let mut out = vec![];
    for c in re.captures_iter(command) {
        let rel = c.get(1).or(c.get(2)).map(|m| m.as_str().to_string()).unwrap_or_default();
        if super::source::safe_rel(&rel).is_ok() && exists(&rel) && !out.contains(&rel) {
            out.push(rel);
        }
    }
    out
}

fn commands_in(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if let (true, Value::String(s)) = (matches!(k.as_str(), "command" | "cmd"), x) {
                    out.push(s.clone());
                }
                commands_in(x, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| commands_in(x, out)),
        Value::String(s) => out.push(s.clone()),
        _ => {}
    }
}

pub(crate) fn short_hash(v: &Value) -> String {
    super::source::sha256_hex(v.to_string().as_bytes())[..12].to_string()
}

pub(crate) struct Cut {
    pub units: Vec<Unit>,
    /// Kit files no unit takes.
    pub loose: Vec<String>,
    /// Found but not moved: (where, why) - e.g. Codex inline `hooks = …` (`inline_form`).
    pub notes: Vec<(String, String)>,
}

/// Cut a tree into units. `strict` = a kit (bad config is an error); otherwise the user's own
/// folder (a config that does not parse is reported as one problem and left alone).
pub(crate) fn cut(l: &Layout, home: &str, t: &dyn Tree, all_files: Option<&[String]>, strict: bool) -> Result<Cut, KitError> {
    let mut units = vec![];
    let mut taken: BTreeSet<String> = BTreeSet::new();
    let exists = |rel: &str| t.read(rel).is_some();

    if l.available(Group::Rules) {
        for r in l.rules {
            if exists(r) {
                taken.insert(r.to_string());
                units.push(Unit::file(Group::Rules, format!("rules:{r}"), r.to_string(), vec![r.to_string()]));
            }
        }
    }
    if let (true, Some(dir)) = (l.available(Group::Skills), l.skills) {
        let mut by: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for f in t.list(dir) {
            let rest = &f[dir.len() + 1..];
            let name = rest.split('/').next().unwrap_or(rest).to_string();
            by.entry(name).or_default().push(f);
        }
        for (name, files) in by {
            let scripts: Vec<String> = files.iter().filter(|f| is_script(f, t.read(f).as_deref())).cloned().collect();
            taken.extend(files.iter().cloned());
            let mut u = Unit::file(Group::Skills, format!("skill:{name}"), name, files);
            if !scripts.is_empty() || u.files.iter().any(|f| !scripts.contains(f) && t.read(f).is_some_and(|b| runs_in_markdown(&b))) {
                u.held = Some(HeldKind::ScriptedSkill);
                u.scripts = scripts;
            }
            units.push(u);
        }
    }
    if let (true, Some(dir)) = (l.available(Group::Agents), l.agents) {
        for f in t.list(dir) {
            taken.insert(f.clone());
            let label = f[dir.len() + 1..].to_string();
            let mut u = Unit::file(Group::Agents, format!("agent:{label}"), label, vec![f.clone()]);
            if t.read(&f).is_some_and(|b| is_script(&f, Some(&b)) || runs_in_markdown(&b)) {
                u.held = Some(HeldKind::ScriptedSkill);
            }
            units.push(u);
        }
    }
    if let (true, Some(hf)) = (l.available(Group::Hooks), l.hooks_file) {
        if let Some(b) = t.read(hf) {
            taken.insert(hf.to_string());
            let mut cmds = vec![];
            if let Ok(v) = serde_json::from_slice::<Value>(&b) {
                commands_in(&v, &mut cmds);
            }
            let scripts = scripts_of(l, home, &cmds, &exists);
            taken.extend(scripts.iter().cloned());
            let mut files = vec![hf.to_string()];
            files.extend(scripts.iter().cloned());
            units.push(Unit {
                key: format!("hook:{hf}"),
                group: Group::Hooks,
                label: hf.to_string(),
                held: Some(HeldKind::Hook),
                files,
                conf: None,
                scripts,
                event: None,
            });
        }
    }
    let mut notes = vec![];
    if let (true, Some(_), Some(key)) = (l.available(Group::Mcp), &l.mcp_file, l.mcp_key) {
        if let Some(b) = t.read(KIT_MCP) {
            taken.insert(KIT_MCP.to_string());
            match serde_json::from_slice::<Value>(&b) {
                Ok(v) => {
                    if let Some(Value::Object(servers)) = v.get(key) {
                        for (name, sv) in servers {
                            units.push(mcp_unit(KIT_MCP, key, name, sv));
                        }
                    }
                }
                Err(_) if strict => return Err(KitError::Corrupt(format!("not json: {KIT_MCP}"))),
                Err(_) => {}
            }
        }
    }
    if let Some((cf, fmt)) = l.config {
        if let Some(b) = t.read(cf) {
            taken.insert(cf.to_string());
            match fmt {
                ConfFmt::Json => match serde_json::from_slice::<Value>(&b) {
                    Ok(Value::Object(m)) => json_units(l, home, cf, &m, &exists, &mut units, &mut taken),
                    _ if strict => return Err(KitError::Corrupt(format!("not json: {cf}"))),
                    _ => {}
                },
                ConfFmt::Toml => match std::str::from_utf8(&b) {
                    Ok(s) => toml_units(l, home, cf, s, &exists, &mut units, &mut notes),
                    Err(_) if strict => return Err(KitError::Corrupt(format!("not text: {cf}"))),
                    Err(_) => {}
                },
            }
        }
    }
    let loose = all_files.map(|a| a.iter().filter(|f| !taken.contains(*f)).cloned().collect()).unwrap_or_default();
    Ok(Cut { units, loose, notes })
}

fn scripts_of(l: &Layout, home: &str, cmds: &[String], exists: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut s: Vec<String> = vec![];
    for c in cmds {
        for r in referenced_scripts(l, home, c, exists) {
            if !s.contains(&r) {
                s.push(r);
            }
        }
    }
    s
}

fn json_units(l: &Layout, home: &str, cf: &str, m: &Map<String, Value>, exists: &dyn Fn(&str) -> bool, units: &mut Vec<Unit>, taken: &mut BTreeSet<String>) {
    let hooks_key = l.hooks_key.filter(|_| l.available(Group::Hooks));
    let mcp_key = l.mcp_key.filter(|_| l.available(Group::Mcp));
    for (k, v) in m {
        if Some(k.as_str()) == l.hooks_key.map(|h| h.0) {
            let Some((_, shape)) = hooks_key else { continue };
            let mut push_hook = |path: Vec<String>, event: Option<String>, entry: &Value, units: &mut Vec<Unit>| {
                let mut cmds = vec![];
                commands_in(entry, &mut cmds);
                let scripts = scripts_of(l, home, &cmds, exists);
                taken.extend(scripts.iter().cloned());
                let label = match &event {
                    Some(e) => format!("{e}: {}", cmds.first().cloned().unwrap_or_default()),
                    None => cmds.first().cloned().unwrap_or_default(),
                };
                units.push(Unit {
                    key: format!("hook:{}:{}", event.as_deref().unwrap_or(k), short_hash(entry)),
                    group: Group::Hooks,
                    label,
                    held: Some(HeldKind::Hook),
                    files: scripts.clone(),
                    conf: Some(ConfOp::JsonPush { file: cf.into(), path, value: entry.clone() }),
                    scripts,
                    event,
                });
            };
            match (shape, v) {
                (HookShape::EventMap, Value::Object(ev)) => {
                    for (e, list) in ev {
                        match list {
                            Value::Array(a) => a.iter().for_each(|x| push_hook(vec![k.clone(), e.clone()], Some(e.clone()), x, units)),
                            other => push_hook(vec![k.clone(), e.clone()], Some(e.clone()), other, units),
                        }
                    }
                }
                (HookShape::List, Value::Array(a)) => a.iter().for_each(|x| push_hook(vec![k.clone()], None, x, units)),
                _ => {
                    // unknown shape: one unit for the whole key, still held
                    let mut cmds = vec![];
                    commands_in(v, &mut cmds);
                    let scripts = scripts_of(l, home, &cmds, exists);
                    taken.extend(scripts.iter().cloned());
                    units.push(Unit {
                        key: format!("hook:{k}"),
                        group: Group::Hooks,
                        label: k.clone(),
                        held: Some(HeldKind::Hook),
                        files: scripts.clone(),
                        conf: Some(ConfOp::JsonSet { file: cf.into(), path: vec![k.clone()], value: v.clone() }),
                        scripts,
                        event: None,
                    });
                }
            }
        } else if Some(k.as_str()) == l.mcp_key && l.mcp_file.is_none() {
            let Some(_) = mcp_key else { continue };
            if let Value::Object(servers) = v {
                for (name, sv) in servers {
                    units.push(mcp_unit(cf, k, name, sv));
                }
            }
        } else if Some(k.as_str()) == l.rules_key {
            if l.available(Group::Rules) {
                units.push(Unit {
                    key: format!("rules:{k}"),
                    group: Group::Rules,
                    label: k.clone(),
                    held: None,
                    files: vec![],
                    conf: Some(ConfOp::JsonSet { file: cf.into(), path: vec![k.clone()], value: v.clone() }),
                    scripts: vec![],
                    event: None,
                });
            }
        } else if l.available(Group::Settings) {
            let held = held_setting(l, k, &v.to_string());
            units.push(Unit {
                key: format!("setting:{k}"),
                group: Group::Settings,
                label: k.clone(),
                held,
                files: vec![],
                conf: Some(ConfOp::JsonSet { file: cf.into(), path: vec![k.clone()], value: v.clone() }),
                scripts: vec![],
                event: None,
            });
        }
    }
}

fn mcp_unit(file: &str, key: &str, name: &str, sv: &Value) -> Unit {
    Unit {
        key: format!("mcp:{name}"),
        group: Group::Mcp,
        label: name.to_string(),
        held: Some(HeldKind::Mcp),
        files: vec![],
        conf: Some(ConfOp::JsonSet { file: file.into(), path: vec![key.to_string(), name.to_string()], value: sv.clone() }),
        scripts: vec![],
        event: None,
    }
}

fn toml_units(l: &Layout, home: &str, cf: &str, s: &str, exists: &dyn Fn(&str) -> bool, units: &mut Vec<Unit>, notes: &mut Vec<(String, String)>) {
    let blocks = tl::parse(s);
    let mcp = l.mcp_key.filter(|_| l.available(Group::Mcp));
    let hooks = l.hooks_key.map(|h| h.0);
    let mut seen_mcp: BTreeSet<String> = BTreeSet::new();
    let mut hooks_done = false;
    for b in &blocks {
        match b {
            Block::Key { key, text } => {
                if Some(key.as_str()) == hooks || Some(key.as_str()) == l.mcp_key {
                    // shortcut: inline `hooks = …` / `mcp_servers = …` are not taken apart; reported, never moved
                    notes.push((format!("{cf}#{key}"), "inline_form".into()));
                    continue;
                }
                if l.available(Group::Settings) {
                    units.push(Unit {
                        key: format!("setting:{key}"),
                        group: Group::Settings,
                        label: key.clone(),
                        held: held_setting(l, key, text),
                        files: vec![],
                        conf: Some(ConfOp::TomlKey { file: cf.into(), key: key.clone(), text: text.clone() }),
                        scripts: vec![],
                        event: None,
                    });
                }
            }
            Block::Table { header, .. } => {
                if let Some(p) = l.mcp_key.filter(|p| tl::in_prefix(header, p)) {
                    let Some(name) = tl::sub_name(header, p) else { continue };
                    if mcp.is_none() || !seen_mcp.insert(name.clone()) {
                        continue;
                    }
                    let sel = TomlSel::Sub(p.into(), name.clone());
                    units.push(Unit {
                        key: format!("mcp:{name}"),
                        group: Group::Mcp,
                        label: name,
                        held: Some(HeldKind::Mcp),
                        files: vec![],
                        conf: Some(ConfOp::TomlTables { file: cf.into(), text: tl::tables_text(&blocks, &|h| sel.matches(h)), sel }),
                        scripts: vec![],
                        event: None,
                    });
                } else if let Some(h) = hooks.filter(|h| tl::in_prefix(header, h)) {
                    if hooks_done || !l.available(Group::Hooks) {
                        continue;
                    }
                    hooks_done = true;
                    let sel = TomlSel::Prefix(h.into());
                    let text = tl::tables_text(&blocks, &|x| sel.matches(x));
                    let scripts = scripts_of(l, home, std::slice::from_ref(&text), exists);
                    units.push(Unit {
                        key: format!("hook:{cf}"),
                        group: Group::Hooks,
                        label: format!("[{h}] {cf}"),
                        held: Some(HeldKind::Hook),
                        files: scripts.clone(),
                        conf: Some(ConfOp::TomlTables { file: cf.into(), sel, text }),
                        scripts,
                        event: None,
                    });
                } else if l.available(Group::Settings) {
                    let sel = TomlSel::Exact(header.clone());
                    let text = tl::tables_text(&blocks, &|x| sel.matches(x));
                    units.push(Unit {
                        key: format!("setting:[{header}]"),
                        group: Group::Settings,
                        label: format!("[{header}]"),
                        held: held_setting(l, header, &text),
                        files: vec![],
                        conf: Some(ConfOp::TomlTables { file: cf.into(), sel, text }),
                        scripts: vec![],
                        event: None,
                    });
                }
            }
            Block::Other(_) => {}
        }
    }
}

// ---- applying config changes ----

pub(crate) fn json_get<'a>(root: &'a Value, path: &[String]) -> Option<&'a Value> {
    let mut cur = root;
    for p in path {
        cur = cur.as_object()?.get(p)?;
    }
    Some(cur)
}

fn json_slot<'a>(root: &'a mut Value, path: &[String]) -> &'a mut Value {
    let mut cur = root;
    for p in path {
        if !cur.is_object() {
            *cur = Value::Object(Map::new());
        }
        cur = cur.as_object_mut().unwrap().entry(p.clone()).or_insert(Value::Null);
    }
    cur
}

/// Apply ops (all on the same file) to its current text; None = the file is not usable.
pub(crate) fn apply_conf(current: Option<&[u8]>, ops: &[&ConfOp]) -> Result<Vec<u8>, KitError> {
    let Some(first) = ops.first() else { return Ok(current.unwrap_or_default().to_vec()) };
    match first {
        ConfOp::JsonSet { .. } | ConfOp::JsonPush { .. } => {
            let mut root: Value = match current {
                Some(b) if !b.iter().all(|c| c.is_ascii_whitespace()) => serde_json::from_slice(b)
                    .map_err(|_| KitError::Corrupt(format!("config is not valid JSON: {}", first.file())))?,
                _ => Value::Object(Map::new()),
            };
            for op in ops {
                match op {
                    ConfOp::JsonSet { path, value, .. } => *json_slot(&mut root, path) = value.clone(),
                    ConfOp::JsonPush { path, value, .. } => {
                        let slot = json_slot(&mut root, path);
                        if !slot.is_array() {
                            *slot = Value::Array(vec![]);
                        }
                        let a = slot.as_array_mut().unwrap();
                        if !a.contains(value) {
                            a.push(value.clone());
                        }
                    }
                    _ => return Err(KitError::Corrupt("mixed config formats".into())),
                }
            }
            let mut s = serde_json::to_string_pretty(&root).map_err(|e| KitError::Io(e.to_string()))?;
            s.push('\n');
            Ok(s.into_bytes())
        }
        ConfOp::TomlKey { .. } | ConfOp::TomlTables { .. } => {
            let cur = match current {
                Some(b) => std::str::from_utf8(b).map_err(|_| KitError::Corrupt(format!("config is not text: {}", first.file())))?,
                None => "",
            };
            let mut blocks = tl::parse(cur);
            for op in ops {
                match op {
                    ConfOp::TomlKey { key, text, .. } => tl::set_key(&mut blocks, key, text),
                    ConfOp::TomlTables { sel, text, .. } => tl::set_tables(&mut blocks, &|h| sel.matches(h), text),
                    _ => return Err(KitError::Corrupt("mixed config formats".into())),
                }
            }
            Ok(tl::render(&blocks).into_bytes())
        }
    }
}

/// What the target currently has for this op: (present, same, old text, new text).
pub(crate) fn conf_state(current: Option<&[u8]>, op: &ConfOp) -> (bool, bool, String, String) {
    match op {
        ConfOp::JsonSet { path, value, .. } => {
            let root: Option<Value> = current.and_then(|b| serde_json::from_slice(b).ok());
            let old = root.as_ref().and_then(|r| json_get(r, path)).cloned();
            let new_t = serde_json::to_string_pretty(value).unwrap_or_default();
            match old {
                None => (false, false, String::new(), new_t),
                Some(o) => (true, &o == value, serde_json::to_string_pretty(&o).unwrap_or_default(), new_t),
            }
        }
        ConfOp::JsonPush { path, value, .. } => {
            let root: Option<Value> = current.and_then(|b| serde_json::from_slice(b).ok());
            let has = root.as_ref().and_then(|r| json_get(r, path)).and_then(|a| a.as_array()).is_some_and(|a| a.contains(value));
            (has, has, String::new(), serde_json::to_string_pretty(value).unwrap_or_default())
        }
        ConfOp::TomlKey { key, text, .. } => {
            let cur = current.and_then(|b| std::str::from_utf8(b).ok()).unwrap_or("");
            let old = tl::parse(cur).into_iter().find_map(|b| match b {
                Block::Key { key: k, text } if &k == key => Some(text),
                _ => None,
            });
            match old {
                None => (false, false, String::new(), text.clone()),
                Some(o) => (true, o.trim() == text.trim(), o, text.clone()),
            }
        }
        ConfOp::TomlTables { sel, text, .. } => {
            let cur = current.and_then(|b| std::str::from_utf8(b).ok()).unwrap_or("");
            let old = tl::tables_text(&tl::parse(cur), &|h| sel.matches(h));
            if old.trim().is_empty() {
                (false, false, String::new(), text.clone())
            } else {
                (true, old.trim() == text.trim(), old, text.clone())
            }
        }
    }
}

/// Rewrite every string of an op through `f` (secret scrub on export, `~` expansion on install).
pub(crate) fn map_op(op: &ConfOp, f: &mut dyn FnMut(&str) -> String, fj: &mut dyn FnMut(&mut Value)) -> ConfOp {
    match op {
        ConfOp::JsonSet { file, path, value } => {
            let mut v = value.clone();
            fj(&mut v);
            ConfOp::JsonSet { file: file.clone(), path: path.clone(), value: v }
        }
        ConfOp::JsonPush { file, path, value } => {
            let mut v = value.clone();
            fj(&mut v);
            ConfOp::JsonPush { file: file.clone(), path: path.clone(), value: v }
        }
        ConfOp::TomlKey { file, key, text } => ConfOp::TomlKey { file: file.clone(), key: key.clone(), text: f(text) },
        ConfOp::TomlTables { file, sel, text } => ConfOp::TomlTables { file: file.clone(), sel: sel.clone(), text: f(text) },
    }
}

pub(crate) fn map_strings(v: &mut Value, f: &mut dyn FnMut(&str) -> String) {
    match v {
        Value::String(s) => *s = f(s),
        Value::Array(a) => a.iter_mut().for_each(|x| map_strings(x, f)),
        Value::Object(m) => m.values_mut().for_each(|x| map_strings(x, f)),
        _ => {}
    }
}

/// Full text of a unit as shown before install: the config entry and every script it runs.
pub(crate) fn unit_text(u: &Unit, t: &dyn Tree) -> String {
    match &u.conf {
        Some(ConfOp::JsonSet { value, .. }) | Some(ConfOp::JsonPush { value, .. }) => serde_json::to_string_pretty(value).unwrap_or_default(),
        Some(ConfOp::TomlKey { text, .. }) | Some(ConfOp::TomlTables { text, .. }) => text.clone(),
        None => u.files.iter().filter(|f| !u.scripts.contains(f)).map(|f| String::from_utf8_lossy(&t.read(f).unwrap_or_default()).to_string()).collect::<Vec<_>>().join("\n"),
    }
}

// ---- key-level undo for shared config files ----

/// What `op` is about to change, as it is now (None = absent): kept in the snapshot.
pub(crate) fn conf_old(current: Option<&[u8]>, op: &ConfOp) -> Option<String> {
    match op {
        ConfOp::JsonSet { path, .. } | ConfOp::JsonPush { path, .. } => {
            let root: Value = current.and_then(|b| serde_json::from_slice(b).ok())?;
            json_get(&root, path).map(|v| v.to_string())
        }
        ConfOp::TomlKey { key, .. } => tl::parse(std::str::from_utf8(current?).ok()?).into_iter().find_map(|b| match b {
            Block::Key { key: k, text } if &k == key => Some(text),
            _ => None,
        }),
        ConfOp::TomlTables { sel, .. } => {
            let t = tl::tables_text(&tl::parse(std::str::from_utf8(current?).ok()?), &|h| sel.matches(h));
            (!t.trim().is_empty()).then_some(t)
        }
    }
}

fn json_remove(root: &mut Value, path: &[String]) {
    let Some((last, parents)) = path.split_last() else { return };
    let mut cur = &mut *root;
    for p in parents {
        match cur.as_object_mut().and_then(|m| m.get_mut(p)) {
            Some(n) => cur = n,
            None => return,
        }
    }
    if let Some(m) = cur.as_object_mut() {
        m.remove(last);
    }
    // drop parents the install created and left empty
    if !parents.is_empty() && json_get(root, parents).is_some_and(|v| v.as_object().is_some_and(|m| m.is_empty())) {
        json_remove(root, parents);
    }
}

/// Put back only what the ops touched, leaving every other key of the live file as it is now.
pub(crate) fn undo_conf(current: Option<&[u8]>, backups: &[(ConfOp, Option<String>)]) -> Result<Vec<u8>, KitError> {
    let Some((first, _)) = backups.first() else { return Ok(current.unwrap_or_default().to_vec()) };
    let bad = || KitError::Corrupt(format!("config changed shape: {}", first.file()));
    match first {
        ConfOp::JsonSet { .. } | ConfOp::JsonPush { .. } => {
            let mut root: Value = match current {
                Some(b) if !b.iter().all(|c| c.is_ascii_whitespace()) => serde_json::from_slice(b).map_err(|_| bad())?,
                _ => Value::Object(Map::new()),
            };
            for (op, old) in backups.iter().rev() {
                let old: Option<Value> = old.as_deref().and_then(|o| serde_json::from_str(o).ok());
                match op {
                    ConfOp::JsonSet { path, .. } => match old {
                        Some(v) => *json_slot(&mut root, path) = v,
                        None => json_remove(&mut root, path),
                    },
                    ConfOp::JsonPush { path, value, .. } => {
                        let had = old.as_ref().and_then(|o| o.as_array()).is_some_and(|a| a.contains(value));
                        if !had {
                            if let Some(a) = json_slot(&mut root, path).as_array_mut() {
                                if let Some(i) = a.iter().position(|x| x == value) {
                                    a.remove(i);
                                }
                            }
                        }
                        if old.is_none() && json_get(&root, path).is_some_and(|v| v.as_array().is_some_and(|a| a.is_empty()) || v.is_null()) {
                            json_remove(&mut root, path);
                        }
                    }
                    _ => return Err(bad()),
                }
            }
            let mut s = serde_json::to_string_pretty(&root).map_err(|e| KitError::Io(e.to_string()))?;
            s.push('\n');
            Ok(s.into_bytes())
        }
        ConfOp::TomlKey { .. } | ConfOp::TomlTables { .. } => {
            let cur = match current {
                Some(b) => std::str::from_utf8(b).map_err(|_| bad())?,
                None => "",
            };
            let mut blocks = tl::parse(cur);
            for (op, old) in backups.iter().rev() {
                match (op, old) {
                    (ConfOp::TomlKey { key, .. }, Some(t)) => tl::set_key(&mut blocks, key, t),
                    (ConfOp::TomlKey { key, .. }, None) => blocks.retain(|b| !matches!(b, Block::Key { key: k, .. } if k == key)),
                    (ConfOp::TomlTables { sel, .. }, old) => tl::set_tables(&mut blocks, &|h| sel.matches(h), old.as_deref().unwrap_or("")),
                    _ => return Err(bad()),
                }
            }
            Ok(tl::render(&blocks).into_bytes())
        }
    }
}

/// Same content (JSON by value, TOML by text), so the old bytes can go back exactly.
pub(crate) fn same_conf(a: &[u8], b: &[u8]) -> bool {
    match (serde_json::from_slice::<Value>(a), serde_json::from_slice::<Value>(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => String::from_utf8_lossy(a).trim() == String::from_utf8_lossy(b).trim(),
    }
}

/// Nothing left but an empty object / blank text.
pub(crate) fn empty_conf(b: &[u8]) -> bool {
    serde_json::from_slice::<Value>(b).is_ok_and(|v| v.as_object().is_some_and(|m| m.is_empty())) || String::from_utf8_lossy(b).trim().is_empty()
}
