//! What a terminal agent used, read from the CLI's own session files (read only, nothing is priced here):
//! Claude Code `~/.claude/projects/<key of the folder>/*.jsonl` (`assistant` lines with `message.usage`), Codex
//! `~/.codex/sessions/**/rollout-*.jsonl` (`token_count` events). Formats are the CLIs' own and undocumented, so every
//! line that does not fit is skipped. No file or no matching record = `None` ("no data"), never a guess.
use crate::sessions;
use chrono::DateTime;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// A line of a session file longer than this is skipped (a tool's output, a picture).
const LINE_MAX: u64 = 1 << 20;
/// Newest files looked at; older ones in the window are left out.
const FILES_MAX: usize = 200;
/// Bytes read from one file.
const FILE_BYTES_MAX: u64 = 256 << 20;

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct Usage {
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    /// Only what the records themselves carry as a cost (`costUSD`).
    pub usd: Option<f64>,
}

/// Usage of `env`'s sessions that ran in exactly `cwd`, from records at or after `since_ms` (Unix ms).
pub fn usage(cwd: &Path, env: &str, since_ms: u64) -> Usage {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let codex = std::env::var_os("CODEX_HOME").filter(|d| !d.is_empty()).map(PathBuf::from).or_else(|| home.map(|h| h.join(".codex")));
    usage_in(sessions::claude_dir().as_deref(), codex.as_deref(), cwd, env, since_ms)
}

/// `usage` with the CLIs' folders given (tests use temporary ones).
pub fn usage_in(claude: Option<&Path>, codex: Option<&Path>, cwd: &Path, env: &str, since_ms: u64) -> Usage {
    match (env, claude, codex) {
        ("claude", Some(c), _) => claude_usage(&c.join("projects"), cwd, since_ms),
        ("codex", _, Some(c)) => codex_usage(&c.join("sessions"), cwd, since_ms),
        _ => Usage::default(),
    }
}

/// Calls `f` for every JSON line of `path` until it says stop; a bad or too long line is skipped, a half-written last
/// line too.
fn each_json(path: &Path, mut f: impl FnMut(&Value) -> bool) {
    let Ok(file) = std::fs::File::open(path) else { return };
    let mut r = BufReader::new(file.take(FILE_BYTES_MAX));
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match (&mut r).take(LINE_MAX + 1).read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if buf.len() as u64 > LINE_MAX && !buf.ends_with(b"\n") {
            let mut skip = Vec::new();
            let _ = r.read_until(b'\n', &mut skip);
            continue;
        }
        if let Ok(j) = serde_json::from_slice::<Value>(&buf) {
            if !f(&j) {
                break;
            }
        }
    }
}

/// `*.jsonl` files (named by `want`) below `dir` to `depth` levels, changed at or after `since_ms`, the newest `FILES_MAX`.
fn files_since(dir: &Path, depth: usize, since_ms: u64, want: &dyn Fn(&str) -> bool) -> Vec<PathBuf> {
    let since = SystemTime::UNIX_EPOCH + Duration::from_millis(since_ms);
    let mut found: Vec<(SystemTime, PathBuf)> = vec![];
    let mut stack = vec![(dir.to_path_buf(), 0)];
    while let Some((d, level)) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let Ok(meta) = std::fs::symlink_metadata(e.path()) else { continue };
            if meta.is_dir() && level < depth {
                stack.push((e.path(), level + 1));
            } else if meta.is_file() && e.file_name().to_str().is_some_and(want) {
                if let Some(t) = meta.modified().ok().filter(|t| *t >= since) {
                    found.push((t, e.path()));
                }
            }
        }
    }
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    found.into_iter().take(FILES_MAX).map(|f| f.1).collect()
}

fn ms_of(j: &Value) -> Option<u64> {
    let t = DateTime::parse_from_rfc3339(j["timestamp"].as_str()?).ok()?.timestamp_millis();
    u64::try_from(t).ok()
}

/// The record's folder is `cwd` (or `cwd` through a link). Different spellings of one folder are looked up once.
struct SameDir<'a> {
    cwd: &'a Path,
    real: Option<PathBuf>,
    seen: HashMap<String, bool>,
}

impl<'a> SameDir<'a> {
    fn new(cwd: &'a Path) -> Self {
        SameDir { cwd, real: crate::proc::canon(&cwd).ok(), seen: HashMap::new() }
    }
    fn is(&mut self, dir: &str) -> bool {
        if let Some(r) = self.seen.get(dir) {
            return *r;
        }
        let p = Path::new(dir);
        let same = p == self.cwd || self.real.as_deref() == Some(p) || (self.real.is_some() && crate::proc::canon(&p).ok() == self.real);
        self.seen.insert(dir.to_string(), same);
        same
    }
}

fn claude_usage(projects: &Path, cwd: &Path, since_ms: u64) -> Usage {
    // the folder name is the path with every other character turned into `-`: two folders can share it, so each record's `cwd` is checked too
    let dir = projects.join(sessions::key_for(cwd));
    // message id -> (in, out, cost); a message is written again for each block it streams, and a resumed session repeats the old ones
    let mut seen: HashMap<String, (u64, u64, Option<f64>)> = HashMap::new();
    let mut same = SameDir::new(cwd);
    let mut anon = 0u64;
    for file in files_since(&dir, 0, since_ms, &|n| n.ends_with(".jsonl")) {
        each_json(&file, |j| {
            let u = &j["message"]["usage"];
            if j["type"] != "assistant" || !u.is_object() || ms_of(j).is_none_or(|t| t < since_ms) {
                return true;
            }
            if j["cwd"].as_str().is_some_and(|c| !same.is(c)) {
                return true;
            }
            let n = |k: &str| u[k].as_u64().unwrap_or(0);
            let (inp, out) = (n("input_tokens") + n("cache_read_input_tokens") + n("cache_creation_input_tokens"), n("output_tokens"));
            let id = j["message"]["id"].as_str().or_else(|| j["requestId"].as_str()).or_else(|| j["uuid"].as_str()).map(String::from).unwrap_or_else(|| {
                anon += 1;
                format!("anon-{anon}")
            });
            let cost = j["costUSD"].as_f64();
            // the snapshots of a streaming message only grow: the biggest one is the whole message
            let e = seen.entry(id).or_insert((0, 0, None));
            e.0 = e.0.max(inp);
            e.1 = e.1.max(out);
            e.2 = match (e.2, cost) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            };
            true
        });
    }
    if seen.is_empty() {
        return Usage::default();
    }
    Usage {
        tokens_in: Some(seen.values().map(|v| v.0).sum()),
        tokens_out: Some(seen.values().map(|v| v.1).sum()),
        usd: seen.values().filter_map(|v| v.2).fold(None, |a, c| Some(a.unwrap_or(0.0) + c)),
    }
}

fn codex_usage(sessions_dir: &Path, cwd: &Path, since_ms: u64) -> Usage {
    let mut same = SameDir::new(cwd);
    let (mut tin, mut tout, mut found) = (0u64, 0u64, false);
    for file in files_since(sessions_dir, 4, since_ms, &|n| n.starts_with("rollout-") && n.ends_with(".jsonl")) {
        let mut in_dir: Option<bool> = None;
        // totals are for the whole session: the last one, less the last one before `since_ms`
        let (mut before, mut last): ((u64, u64), Option<(u64, u64)>) = ((0, 0), None);
        each_json(&file, |j| {
            let p = &j["payload"];
            if let Some(c) = p["cwd"].as_str().filter(|_| matches!(j["type"].as_str(), Some("session_meta" | "turn_context"))) {
                if in_dir.is_none() {
                    in_dir = Some(same.is(c));
                }
            }
            if in_dir == Some(false) {
                return false; // another folder's session
            }
            let t = &p["info"]["total_token_usage"];
            if j["type"] == "event_msg" && p["type"] == "token_count" && t.is_object() {
                let tot = (t["input_tokens"].as_u64().unwrap_or(0), t["output_tokens"].as_u64().unwrap_or(0));
                if ms_of(j).is_some_and(|ms| ms < since_ms) {
                    before = tot;
                } else {
                    last = Some(tot);
                }
            }
            true
        });
        if let (Some(l), Some(true)) = (last, in_dir) {
            tin += l.0.saturating_sub(before.0);
            tout += l.1.saturating_sub(before.1);
            found = true;
        }
    }
    if found { Usage { tokens_in: Some(tin), tokens_out: Some(tout), usd: None } } else { Usage::default() }
}
