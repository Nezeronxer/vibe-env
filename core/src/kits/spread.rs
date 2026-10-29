//! Spread: every skill and MCP server found in one installed environment goes into each other installed environment
//! that lacks it (compared by name), in that environment's own format. Nothing there is replaced or removed: a name
//! taken by something else is a skip in the report. Writes go through the kit machinery (`preview_built` + `install`):
//! one snapshot per environment, undone by `rollback`. What already runs in one environment goes in turned on; a secret
//! value of a server goes to the keyring of the target environment and the file gets a reference to it, as with kits.

use super::layout::{self, ConfFmt, Layout, KIT_CATALOG_SKILLS, KIT_MCP};
use super::toml_lite::{self as tl, Block};
use super::units::{self, Tree};
use super::{env_name_allowed, snap, source, Choices, Group, HeldRef, KitError, KitSource, Kits, RollbackReport, Scope, SnapshotId};
use crate::market::{render_server, MarketError, Server};
use crate::scrub;
use chrono::{DateTime, Local};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SpreadItem {
    pub env: String,
    /// `skill` | `mcp`
    pub kind: String,
    pub name: String,
    /// The environment it was taken from.
    pub from: String,
}

/// Not spread. `name` is `*` for the whole group of an environment, `<skill>/<file>` for one file of a skill.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SpreadSkip {
    pub env: String,
    pub kind: String,
    pub name: String,
    /// `conflict` (the name is taken by something else), `layout_unverified`, `unparsed`, `unsupported_field`,
    /// `secret_inline` (a secret in a command, an argument or an address), `secret_name`, `secret_conflict`, `bad_name`,
    /// `symlink_outside`, `secret_file`, `too_large`, `not_applied`, a `codex_*` / `header_form` form Codex cannot hold,
    /// or a `KitError` kind.
    pub reason: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct SpreadReport {
    pub at: Option<DateTime<Local>>,
    pub added: Vec<SpreadItem>,
    pub skipped: Vec<SpreadSkip>,
    /// One per environment written into: `rollback` of each undoes the spread.
    pub snapshots: Vec<SnapshotId>,
    /// `<env>/<NAME>` put into the keyring (names only).
    pub secrets: Vec<String>,
    #[serde(default)]
    pub undone: bool,
}

/// An MCP server read from an environment's config. Strings hold `{VAR}` where the config refers to a variable.
#[derive(Clone, Debug)]
struct Mcp {
    local: Option<(String, Vec<String>)>,
    /// (`streamable-http` | `sse`, url, headers)
    remote: Option<crate::market::Remote>,
    env: Vec<(String, String)>,
}

impl Mcp {
    /// The same server: same program and arguments, or the same address (variables and secrets may differ).
    fn same(&self, o: &Mcp) -> bool {
        self.local == o.local && self.remote.as_ref().map(|r| &r.1) == o.remote.as_ref().map(|r| &r.1)
    }
}

type Parsed = Result<Option<Mcp>, &'static str>;

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn var_ref() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\{env:([A-Za-z_][A-Za-z0-9_]*)\}").unwrap())
}

/// The whole value refers to a variable (`{X}`, `Bearer {X}`): a `{word}` inside a longer secret is part of the secret.
fn has_var(s: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(?:(?i:bearer|token|basic)\s+)?\{[A-Za-z_][A-Za-z0-9_\-]*\}$").unwrap()).is_match(s.trim())
}

/// `${X}` (Claude Code, Gemini, Qwen) and `{env:X}` (opencode) → `{X}`.
// shortcut: a literal `{word}` in a value reads as a variable too, and `${X:-default}` stays as written; rare in configs
fn norm(s: &str) -> String {
    var_ref().replace_all(s, |c: &regex::Captures| format!("{{{}}}", c.get(1).or(c.get(2)).map_or("", |m| m.as_str()))).to_string()
}

fn strs(v: Option<&Value>, f: &dyn Fn(&str) -> String) -> Result<Vec<String>, &'static str> {
    match v {
        None => Ok(vec![]),
        Some(Value::Array(a)) => a.iter().map(|x| x.as_str().map(f).ok_or("unparsed")).collect(),
        _ => Err("unparsed"),
    }
}

fn pairs(v: Option<&Value>, f: &dyn Fn(&str) -> String) -> Result<Vec<(String, String)>, &'static str> {
    match v {
        None => Ok(vec![]),
        Some(Value::Object(m)) => m.iter().map(|(k, x)| x.as_str().map(|s| (k.clone(), f(s))).ok_or("unparsed")).collect(),
        _ => Err("unparsed"),
    }
}

/// A server of Claude Code, Gemini, Qwen (`mcpServers`) or opencode (`mcp`). Keys it does not know: not moved.
fn from_json(env: &str, v: &Value) -> Parsed {
    let o = v.as_object().ok_or("unparsed")?;
    if o.get("disabled") == Some(&Value::Bool(true)) || o.get("enabled") == Some(&Value::Bool(false)) {
        return Ok(None);
    }
    const KNOWN: &[&str] = &["type", "command", "args", "env", "environment", "url", "httpUrl", "headers", "timeout", "description", "trust", "disabled", "enabled"];
    if o.keys().any(|k| !KNOWN.contains(&k.as_str())) {
        return Err("unsupported_field");
    }
    let s = |k: &str| o.get(k).and_then(Value::as_str);
    let headers = pairs(o.get("headers"), &norm)?;
    if env == "opencode" {
        return match s("type") {
            Some("local") => {
                let cmd = strs(o.get("command"), &norm)?;
                let (c, args) = cmd.split_first().ok_or("unparsed")?;
                Ok(Some(Mcp { local: Some((c.clone(), args.to_vec())), remote: None, env: pairs(o.get("environment"), &norm)? }))
            }
            Some("remote") => Ok(Some(Mcp { local: None, remote: Some(("streamable-http".into(), norm(s("url").ok_or("unparsed")?), headers)), env: vec![] })),
            _ => Err("unparsed"),
        };
    }
    if let Some(c) = s("command") {
        return Ok(Some(Mcp { local: Some((norm(c), strs(o.get("args"), &norm)?)), remote: None, env: pairs(o.get("env"), &norm)? }));
    }
    let (t, url) = match (env, s("type"), s("httpUrl"), s("url")) {
        ("claude", Some("sse"), _, Some(u)) => ("sse", u),
        ("claude", _, _, Some(u)) => ("streamable-http", u),
        (_, _, Some(u), _) => ("streamable-http", u),
        (_, _, None, Some(u)) => ("sse", u),
        _ => return Err("unparsed"),
    };
    Ok(Some(Mcp { local: None, remote: Some((t.into(), norm(url), headers)), env: vec![] }))
}

/// A Codex `[mcp_servers.<name>]` table (and its `.env` / `.http_headers` / `.env_http_headers` tables) as JSON.
fn from_codex(o: &Map<String, Value>) -> Parsed {
    if o.get("enabled") == Some(&Value::Bool(false)) {
        return Ok(None);
    }
    const KNOWN: &[&str] = &["command", "args", "env", "env_vars", "url", "bearer_token_env_var", "http_headers", "env_http_headers", "enabled", "startup_timeout_sec", "startup_timeout_ms", "tool_timeout_sec"];
    if o.keys().any(|k| !KNOWN.contains(&k.as_str())) {
        return Err("unsupported_field");
    }
    let lit = |s: &str| s.to_string(); // Codex expands nothing
    let mut env = pairs(o.get("env"), &lit)?;
    env.extend(strs(o.get("env_vars"), &lit)?.into_iter().map(|n| (n.clone(), format!("{{{n}}}"))));
    if let Some(c) = o.get("command") {
        return Ok(Some(Mcp { local: Some((c.as_str().ok_or("unparsed")?.to_string(), strs(o.get("args"), &lit)?)), remote: None, env }));
    }
    let url = o.get("url").and_then(Value::as_str).ok_or("unparsed")?;
    let mut headers = pairs(o.get("http_headers"), &lit)?;
    headers.extend(pairs(o.get("env_http_headers"), &lit)?.into_iter().map(|(h, v)| (h, format!("{{{v}}}"))));
    if let Some(b) = o.get("bearer_token_env_var") {
        headers.push(("Authorization".into(), format!("Bearer {{{}}}", b.as_str().ok_or("unparsed")?)));
    }
    Ok(Some(Mcp { local: None, remote: Some(("streamable-http".into(), url.into(), headers)), env: vec![] }))
}

/// Just enough of a TOML table body: `key = value` with strings, arrays and inline tables; anything else is None.
struct TomlBody<'a> {
    s: &'a [u8],
    i: usize,
}

impl TomlBody<'_> {
    fn ws(&mut self, lines: bool) {
        while let Some(&c) = self.s.get(self.i) {
            match c {
                b' ' | b'\t' | b'\r' => self.i += 1,
                b'\n' if lines => self.i += 1,
                b'#' if lines => {
                    while self.s.get(self.i).is_some_and(|&c| c != b'\n') {
                        self.i += 1;
                    }
                }
                _ => break,
            }
        }
    }
    fn string(&mut self) -> Option<String> {
        let q = *self.s.get(self.i)?;
        if self.s.get(self.i..self.i + 3) == Some(&[q, q, q][..]) {
            return None; // multi-line strings are not read
        }
        let start = self.i;
        self.i += 1;
        while let Some(&c) = self.s.get(self.i) {
            self.i += 1;
            if c == b'\\' && q == b'"' {
                self.i += 1;
            } else if c == q {
                let raw = std::str::from_utf8(&self.s[start..self.i]).ok()?;
                return if q == b'"' { serde_json::from_str(raw).ok() } else { Some(raw[1..raw.len() - 1].to_string()) };
            } else if c == b'\n' {
                return None;
            }
        }
        None
    }
    fn key(&mut self) -> Option<String> {
        if matches!(self.s.get(self.i), Some(b'"' | b'\'')) {
            return self.string();
        }
        let start = self.i;
        while self.s.get(self.i).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'-') {
            self.i += 1;
        }
        (self.i > start).then(|| String::from_utf8_lossy(&self.s[start..self.i]).to_string())
    }
    fn value(&mut self) -> Option<Value> {
        match *self.s.get(self.i)? {
            b'"' | b'\'' => self.string().map(Value::String),
            b'[' => {
                self.i += 1;
                let mut a = vec![];
                loop {
                    self.ws(true);
                    if self.s.get(self.i) == Some(&b']') {
                        self.i += 1;
                        return Some(Value::Array(a));
                    }
                    a.push(self.value()?);
                    self.ws(true);
                    match self.s.get(self.i)? {
                        b',' => self.i += 1,
                        b']' => {}
                        _ => return None,
                    }
                }
            }
            b'{' => {
                self.i += 1;
                let mut m = Map::new();
                loop {
                    self.ws(false);
                    if self.s.get(self.i) == Some(&b'}') {
                        self.i += 1;
                        return Some(Value::Object(m));
                    }
                    let (k, v) = self.pair()?;
                    m.insert(k, v);
                    self.ws(false);
                    match self.s.get(self.i)? {
                        b',' => self.i += 1,
                        b'}' => {}
                        _ => return None,
                    }
                }
            }
            _ => {
                let start = self.i;
                while self.s.get(self.i).is_some_and(|c| c.is_ascii_alphanumeric() || b"+-._".contains(c)) {
                    self.i += 1;
                }
                let w = std::str::from_utf8(&self.s[start..self.i]).ok()?;
                match w {
                    "true" => Some(Value::Bool(true)),
                    "false" => Some(Value::Bool(false)),
                    _ => serde_json::from_str::<serde_json::Number>(&w.replace('_', "")).ok().map(Value::Number),
                }
            }
        }
    }
    fn pair(&mut self) -> Option<(String, Value)> {
        let k = self.key()?;
        self.ws(false);
        if self.s.get(self.i) != Some(&b'=') {
            return None; // dotted keys and the like
        }
        self.i += 1;
        self.ws(false);
        Some((k, self.value()?))
    }
    fn parse(text: &str) -> Option<Map<String, Value>> {
        let body = text.split_once('\n').map_or("", |(_, b)| b); // the header line
        let mut p = TomlBody { s: body.as_bytes(), i: 0 };
        let mut m = Map::new();
        loop {
            p.ws(true);
            if p.i >= p.s.len() {
                return Some(m);
            }
            let (k, v) = p.pair()?;
            m.insert(k, v);
        }
    }
}

/// None: some server is written in a form this reader does not see (`[mcp_servers]` with `gh = { … }`, a dotted
/// `mcp_servers.gh.command = …`) - adding `[mcp_servers.gh]` then would declare it twice and break the whole file.
fn codex_servers(text: &str, prefix: &str) -> Option<BTreeMap<String, Parsed>> {
    let blocks = tl::parse(text);
    let dotted = |k: &str| { let k = k.trim().trim_matches('"'); k == prefix || k.starts_with(&format!("{prefix}.")) };
    let hidden = blocks.iter().any(|b| match b {
        Block::Key { key, .. } => dotted(key),
        Block::Table { header, .. } => header == prefix,
        _ => false,
    });
    if hidden {
        return None;
    }
    let mut tables: BTreeMap<String, Option<Map<String, Value>>> = BTreeMap::new();
    for b in blocks {
        let Block::Table { header, text } = b else { continue };
        if !tl::in_prefix(&header, prefix) {
            continue;
        }
        let Some(name) = tl::sub_name(&header, prefix) else { continue };
        let base = format!("{prefix}.{name}");
        let entry = tables.entry(name).or_insert_with(|| Some(Map::new()));
        let (Some(m), Some(body)) = (entry.as_mut(), TomlBody::parse(&text)) else {
            *entry = None;
            continue;
        };
        match header.strip_prefix(&base).map(|r| r.trim_start_matches('.')) {
            Some("") => m.extend(body),
            Some(sub @ ("env" | "http_headers" | "env_http_headers")) => {
                m.insert(sub.into(), Value::Object(body));
            }
            _ => *entry = None, // a quoted name or a table this does not know
        }
    }
    Some(tables.into_iter().map(|(n, m)| (n, m.ok_or("unparsed").and_then(|m| from_codex(&m)))).collect())
}

/// A secret left in a command, an argument, an address or an ordinary variable: what the export's scrubber would cut.
fn secret_inline(s: &str, home: &str) -> bool {
    scrub::clean(s, home) != scrub::Scrubber::new(home).home_paths(s)
}

fn market_reason(e: MarketError) -> String {
    match e {
        MarketError::Unsupported(why) => why,
        MarketError::Kit(k) => k.kind().into(),
        MarketError::EnvUnsupported { .. } => "layout_unverified".into(),
        _ => "io".into(),
    }
}

fn ok_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 64 && !n.starts_with(['.', '-']) && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

/// What one environment gets: kit files, the items behind them, secrets to set first.
#[derive(Default)]
struct Batch {
    files: BTreeMap<String, Vec<u8>>,
    items: Vec<SpreadItem>,
    secrets: BTreeMap<String, String>,
}

impl Kits {
    fn spread_file(&self) -> PathBuf {
        self.kits_dir().join("spread.json")
    }

    /// The report of the last spread that wrote something (or of the first one).
    pub fn spread_last(&self) -> Option<SpreadReport> {
        std::fs::read(self.spread_file()).ok().and_then(|b| serde_json::from_slice(&b).ok())
    }

    /// Roll back every snapshot of the last spread.
    pub fn spread_undo(&self) -> Result<RollbackReport, KitError> {
        let _g = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let mut r = self.spread_last().ok_or_else(|| KitError::NotFound("spread".into()))?;
        let mut out = RollbackReport::default();
        for id in r.snapshots.iter().rev() {
            let k = self.rollback(id)?;
            out.kept_changed.extend(k.kept_changed);
            out.kept_files.extend(k.kept_files);
        }
        // the secrets it put in go too: else every run of that environment kept getting the token
        for es in &r.secrets {
            if let Some((env, name)) = es.split_once('/') {
                self.secrets().forget(self.store.as_ref(), env, name)?;
            }
        }
        r.undone = true;
        snap::write_atomic(&self.spread_file(), &serde_json::to_vec_pretty(&r).map_err(super::io)?, Some(0o600))?;
        Ok(out)
    }

    /// Skills folder of `l` in the home and its kit folder: its own (`skills`) or the catalog one (`catalog-skills`).
    fn skills_root(&self, l: &Layout) -> Option<(PathBuf, &'static str)> {
        match (l.available(Group::Skills), l.skills, l.catalog_skills) {
            (true, Some(d), _) => Some((self.home.join(l.home_dir).join(d), "skills")),
            (_, _, Some((g, _))) => Some((self.home.join(g), KIT_CATALOG_SKILLS)),
            _ => None,
        }
    }

    /// Skill folders (with a `SKILL.md`) of `l`, by name; a folder that resolves outside the home is not one.
    fn skills_of(&self, l: &Layout) -> BTreeMap<String, PathBuf> {
        let Some((root, _)) = self.skills_root(l) else { return BTreeMap::new() };
        let home = self.canon_home();
        std::fs::read_dir(root)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let real = e.path().canonicalize().ok()?;
                (ok_name(&name) && real.starts_with(&home) && !units::forbidden(&real) && real.join("SKILL.md").is_file()).then_some((name, real))
            })
            .collect()
    }

    /// Every file of a skill folder (relative), never past the folder itself: a symlink out of it is skipped.
    fn skill_files(&self, dir: &Path, rel: &str, depth: usize, out: &mut BTreeMap<String, Vec<u8>>, skip: &mut Vec<(String, &'static str)>) {
        if depth > 16 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir.join(rel)) else { return };
        let mut es: Vec<_> = rd.flatten().collect();
        es.sort_by_key(|e| e.file_name());
        for e in es {
            let name = e.file_name().to_string_lossy().to_string();
            if units::SKIP_NAMES.contains(&name.as_str()) {
                continue;
            }
            let r = if rel.is_empty() { name } else { format!("{rel}/{name}") };
            if out.len() >= source::MAX_FILES {
                skip.push((r, "too_large"));
                return;
            }
            let Ok(real) = e.path().canonicalize() else { continue };
            let link = e.file_type().is_ok_and(|t| t.is_symlink());
            if !real.starts_with(dir) {
                skip.push((r, "symlink_outside"));
            } else if real.is_dir() && link {
                // a linked folder is never walked: a link to `.` or a parent walked the skill again (2^17 times with two)
                skip.push((r, "symlink_dir"));
            } else if real.is_dir() {
                self.skill_files(dir, &r, depth + 1, out, skip);
            } else if super::is_secret_file(&r) {
                skip.push((r, "secret_file"));
            } else if std::fs::metadata(&real).map_or(true, |m| m.len() > source::MAX_FILE) {
                skip.push((r, "too_large"));
            } else if let Ok(b) = std::fs::read(&real) {
                out.insert(r, b);
            }
        }
    }

    /// MCP servers of `l` in the home; None = no MCP layout, or a config that does not read (nothing goes into it).
    fn servers_of(&self, l: &Layout) -> Option<BTreeMap<String, Parsed>> {
        let key = l.mcp_key.filter(|_| l.available(Group::Mcp))?;
        let (rel, fmt) = match (&l.mcp_file, l.config) {
            (Some(_), _) => (KIT_MCP, ConfFmt::Json),
            (None, Some((cf, fmt))) => (cf, fmt),
            _ => return None,
        };
        let Some(b) = self.tree(l, &Scope::Global).read(rel) else { return Some(BTreeMap::new()) };
        match fmt {
            ConfFmt::Json => {
                let v: Value = serde_json::from_slice(&b).ok()?;
                let servers = v.get(key).and_then(Value::as_object).cloned().unwrap_or_default();
                Some(servers.iter().map(|(n, sv)| (n.clone(), from_json(l.id, sv))).collect())
            }
            ConfFmt::Toml => codex_servers(std::str::from_utf8(&b).ok()?, key),
        }
    }

    /// The server as `env` will hold it: secret values of variables and headers go to the keyring of `env` (in
    /// `pending` until the batch is written) and the file refers to them by name.
    fn hide_secrets(&self, env: &str, name: &str, m: &Mcp, pending: &mut BTreeMap<String, String>) -> Result<Server, &'static str> {
        let home = self.home.to_string_lossy().to_string();
        let mut texts: Vec<&str> = vec![];
        if let Some((c, a)) = &m.local {
            texts.push(c);
            texts.extend(a.iter().map(String::as_str));
        }
        if let Some((_, u, _)) = &m.remote {
            texts.push(u);
        }
        if texts.iter().any(|s| secret_inline(s, &home)) {
            return Err("secret_inline");
        }
        let mut keep = |var: String, value: &str| -> Result<String, &'static str> {
            if !env_name_allowed(&var) {
                return Err("secret_name");
            }
            let known = pending.get(&var).cloned().or_else(|| self.secrets().get(self.store.as_ref(), env, &var));
            match known.as_deref() {
                Some(k) if k != value => return Err("secret_conflict"),
                Some(_) => {}   // already there (an earlier spread or the human): not ours to forget on undo
                None => {
                    pending.insert(var.clone(), value.to_string());
                }
            }
            Ok(format!("{{{var}}}"))
        };
        let mut envs = vec![];
        for (k, v) in &m.env {
            let secret = scrub::is_secret_name(k) && !v.is_empty() && !has_var(v);
            if secret {
                envs.push((k.clone(), keep(k.clone(), v)?));
            } else if secret_inline(v, &home) {
                return Err("secret_inline");
            } else {
                envs.push((k.clone(), v.clone()));
            }
        }
        let remote = match &m.remote {
            None => None,
            Some((t, u, hs)) => {
                let mut out = vec![];
                for (h, v) in hs {
                    if !scrub::is_secret_name(h) || has_var(v) {
                        if secret_inline(v, &home) {
                            return Err("secret_inline");
                        }
                        out.push((h.clone(), v.clone()));
                        continue;
                    }
                    let (scheme, val) = match v.split_once(' ') {
                        Some((s, rest)) if ["bearer", "basic", "token"].contains(&s.to_ascii_lowercase().as_str()) => (format!("{s} "), rest.trim()),
                        _ => (String::new(), v.as_str()),
                    };
                    let suffix = if h.eq_ignore_ascii_case("authorization") { "TOKEN".to_string() } else { scrub::var_name(h) };
                    out.push((h.clone(), format!("{scheme}{}", keep(format!("{}_{suffix}", scrub::var_name(name)), val)?)));
                }
                Some((t.clone(), u.clone(), out))
            }
        };
        Ok(Server { local: m.local.clone(), remote, env: envs })
    }

    /// Spread the skills and MCP servers of every installed environment (`installed`, ids) into the others. A second
    /// call with nothing new writes nothing and makes no snapshot.
    pub fn spread(&self, installed: &[&str]) -> SpreadReport {
        let _g = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let mut seen = BTreeSet::new();
        let envs: Vec<&'static Layout> = installed.iter().filter_map(|e| layout::layout(e)).filter(|l| !l.home_dir.is_empty() && seen.insert(l.id)).collect();
        let skills: Vec<BTreeMap<String, PathBuf>> = envs.iter().map(|l| self.skills_of(l)).collect();
        let servers: Vec<Option<BTreeMap<String, Parsed>>> = envs.iter().map(|l| self.servers_of(l)).collect();
        // the first environment that has a name gives it
        let mut skill_src: BTreeMap<&str, (&str, &Path)> = BTreeMap::new();
        let mut mcp_src: BTreeMap<&str, (&str, &Mcp)> = BTreeMap::new();
        for (i, l) in envs.iter().enumerate() {
            for (n, p) in &skills[i] {
                skill_src.entry(n.as_str()).or_insert((l.id, p.as_path()));
            }
            for (n, m) in servers[i].iter().flatten() {
                if let Ok(Some(m)) = m {
                    mcp_src.entry(n.as_str()).or_insert((l.id, m));
                }
            }
        }
        let mut rep = SpreadReport { at: Some(Local::now()), ..Default::default() };
        let skip = |rep: &mut SpreadReport, env: &str, kind: &str, name: &str, reason: &str| {
            rep.skipped.push(SpreadSkip { env: env.into(), kind: kind.into(), name: name.into(), reason: reason.into() })
        };
        // a server its own environment holds in a form that cannot travel (Claude's `headersHelper`, …) is said once,
        // under the environment it lives in - before, it simply never appeared anywhere
        if envs.len() > 1 {
            for (i, l) in envs.iter().enumerate() {
                for (n, m) in servers[i].iter().flatten() {
                    if let Err(why) = m {
                        if !mcp_src.contains_key(n.as_str()) {
                            skip(&mut rep, l.id, "mcp", n, why);
                        }
                    }
                }
            }
        }
        // a skill folder is read once, whatever number of environments get it: (files, skipped files)
        type Walked = (BTreeMap<String, Vec<u8>>, Vec<(String, &'static str)>);
        let mut walked: BTreeMap<&str, Walked> = BTreeMap::new();
        for (i, l) in envs.iter().enumerate() {
            let env = l.id;
            let mut b = Batch::default();
            // skills
            match self.skills_root(l) {
                None if skill_src.values().any(|(from, _)| *from != env) => skip(&mut rep, env, "skill", "*", "layout_unverified"),
                None => {}
                Some((root, kit_dir)) => {
                    for (name, (from, dir)) in &skill_src {
                        if *from == env {
                            continue;
                        }
                        if std::fs::symlink_metadata(root.join(name)).is_ok() {
                            if std::fs::read(root.join(name).join("SKILL.md")).ok() != std::fs::read(dir.join("SKILL.md")).ok() {
                                skip(&mut rep, env, "skill", name, "conflict");
                            }
                            continue;
                        }
                        let (files, bad) = walked.entry(*name).or_insert_with(|| {
                            let (mut f, mut s) = (BTreeMap::new(), vec![]);
                            self.skill_files(dir, "", 0, &mut f, &mut s);
                            (f, s)
                        });
                        if files.values().map(|v| v.len() as u64).sum::<u64>() > source::MAX_TOTAL {
                            skip(&mut rep, env, "skill", name, "too_large");
                            continue;
                        }
                        for (r, why) in bad.iter() {
                            skip(&mut rep, env, "skill", &format!("{name}/{r}"), why);
                        }
                        for (r, v) in files.iter() {
                            b.files.insert(format!("{kit_dir}/{name}/{r}"), v.clone());
                        }
                        b.items.push(SpreadItem { env: env.into(), kind: "skill".into(), name: name.to_string(), from: from.to_string() });
                    }
                }
            }
            // MCP servers
            match &servers[i] {
                None if mcp_src.values().any(|(from, _)| *from != env) => {
                    let why = if l.available(Group::Mcp) { "unparsed" } else { "layout_unverified" };
                    skip(&mut rep, env, "mcp", "*", why);
                }
                None => {}
                Some(have) => {
                    let mut toml = String::new();
                    let mut json: Option<(String, Value)> = None;
                    for (name, (from, m)) in &mcp_src {
                        if *from == env {
                            continue;
                        }
                        if let Some(cur) = have.get(*name) {
                            if !matches!(cur, Ok(Some(c)) if c.same(m)) {
                                skip(&mut rep, env, "mcp", name, "conflict");
                            }
                            continue;
                        }
                        if !ok_name(name) || name.contains('.') {
                            skip(&mut rep, env, "mcp", name, "bad_name");
                            continue;
                        }
                        let mut pending = b.secrets.clone();
                        let rendered = self.hide_secrets(env, name, m, &mut pending).map_err(String::from).and_then(|s| render_server(env, name, &s).map_err(market_reason));
                        let (file, bytes) = match rendered {
                            Ok(x) => x,
                            Err(why) => {
                                skip(&mut rep, env, "mcp", name, &why);
                                continue;
                            }
                        };
                        if file.ends_with(".toml") {
                            toml.push_str(&String::from_utf8_lossy(&bytes));
                        } else {
                            let v: Value = serde_json::from_slice(&bytes).unwrap_or_default();
                            let (_, acc) = json.get_or_insert_with(|| (file.clone(), Value::Object(Map::new())));
                            merge(acc, v);
                        }
                        b.secrets = pending;
                        b.items.push(SpreadItem { env: env.into(), kind: "mcp".into(), name: name.to_string(), from: from.to_string() });
                    }
                    if !toml.is_empty() {
                        b.files.insert(l.config.map_or("config.toml", |c| c.0).to_string(), toml.into_bytes());
                    }
                    if let Some((file, v)) = json {
                        b.files.insert(file, serde_json::to_vec_pretty(&v).unwrap_or_default());
                    }
                }
            }
            if !b.items.is_empty() {
                self.spread_into(env, &envs, b, &mut rep);
            }
        }
        // kept: the last spread that wrote something (the one "undo" takes back); a first run is kept whatever it did
        if !rep.added.is_empty() || !self.spread_file().exists() {
            if let Ok(b) = serde_json::to_vec_pretty(&rep) {
                let _ = snap::write_atomic(&self.spread_file(), &b, Some(0o600));
            }
        }
        rep
    }

    /// Install one environment's batch, and turn on what the install held (servers, skills with scripts): they run in
    /// another environment already.
    fn spread_into(&self, env: &str, envs: &[&Layout], b: Batch, rep: &mut SpreadReport) {
        let fail = |rep: &mut SpreadReport, items: &[SpreadItem], why: &str| {
            rep.skipped.extend(items.iter().map(|it| SpreadSkip { env: it.env.clone(), kind: it.kind.clone(), name: it.name.clone(), reason: why.into() }))
        };
        let mut put: Vec<&str> = vec![];
        // a batch that does not get written takes its new secrets back: nothing of it may reach the runs of `env`
        let unput = |put: &[&str]| for n in put { let _ = self.secrets().forget(self.store.as_ref(), env, n); };
        for (n, v) in &b.secrets {
            if let Err(e) = self.set_secret(env, n, v) {
                unput(&put);
                return fail(rep, &b.items, e.kind());
            }
            put.push(n);
        }
        let froms: Vec<&str> = envs.iter().map(|l| l.id).filter(|e| *e != env).collect();
        let src = KitSource::Repo(format!("spread:{}", froms.join(",")));
        let plan = match self.preview_built(env, "spread", src, None, &Scope::Global, b.files, vec![]) {
            Ok(p) => p,
            Err(e) => {
                unput(&put);
                return fail(rep, &b.items, e.kind());
            }
        };
        let snap = match self.install(&plan, &Choices::default()) {
            Ok(id) => id,
            Err(KitError::NotApplied { snapshot, .. }) => {
                unput(&put);
                rep.snapshots.push(snapshot);
                return fail(rep, &b.items, "not_applied");
            }
            Err(e) => {
                unput(&put);
                return fail(rep, &b.items, e.kind());
            }
        };
        rep.secrets.extend(put.iter().map(|n| format!("{env}/{n}")));
        rep.snapshots.push(snap.clone());
        let held = self.snapshots(env).into_iter().find(|s| s.id == snap).map(|s| s.held).unwrap_or_default();
        let mut off: BTreeSet<String> = BTreeSet::new();
        for h in held {
            if let Err(e) = self.enable_item(env, &HeldRef { snapshot: snap.clone(), key: h.key.clone() }) {
                let why = if matches!(e, KitError::NotApplied { .. }) { "not_applied" } else { e.kind() };
                rep.skipped.push(SpreadSkip { env: env.into(), kind: if h.group == Group::Mcp { "mcp" } else { "skill" }.into(), name: h.label.clone(), reason: why.into() });
                off.insert(h.label);
            }
        }
        rep.added.extend(b.items.into_iter().filter(|it| !off.contains(&it.name)));
    }
}

fn merge(acc: &mut Value, v: Value) {
    match (acc, v) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, x) in b {
                match a.get_mut(&k) {
                    Some(y) => merge(y, x),
                    None => {
                        a.insert(k, x);
                    }
                }
            }
        }
        (a, v) => *a = v,
    }
}
