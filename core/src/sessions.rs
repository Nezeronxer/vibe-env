//! G68 - sessions of Claude Code started in a terminal: the heads of `~/.claude/projects/<key>/*.jsonl`, read only.
//! The key is the folder the session was started in with every character that is not an ASCII letter or digit
//! turned into `-` (checked against the live folder: `/Users/x/Моя папка/app` -> `-Users-x-----------app`).
//! Nothing here writes into a session file; `copy_into` only reads one and writes a copy under another key.
use chrono::{DateTime, Local};
use serde::Serialize;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

/// Sessions per page, the newest first (story 330).
pub const PAGE: usize = 50;
/// Bytes of one line that are looked at; the rest of a longer line is skipped, not held in memory.
const MAX_LINE: u64 = 1 << 20;
/// A file is read for the head and the message count up to this many bytes; a longer one counts as "about".
const MAX_READ: u64 = 8 << 20;
const MAX_PROMPT: usize = 200;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SessionHead {
    pub id: String,
    pub started_at: DateTime<Local>,
    pub first_prompt: String,
    /// Lines of the user and the assistant; a lower bound when `approx`.
    pub messages: u32,
    pub approx: bool,
    pub branch: Option<String>,
    /// The file could not be read as a session (story 329): shown with a mark, the rest of the list is intact.
    pub unreadable: bool,
}

/// The folder Claude keeps its files in: `CLAUDE_CONFIG_DIR`, else `~/.claude`.
pub fn claude_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    std::env::var_os("HOME").filter(|h| !h.is_empty()).map(|h| PathBuf::from(h).join(".claude"))
}

/// `/a/Моя папка/b` -> `-a-----------b`.
pub fn key_for(path: &Path) -> String {
    // symlinks resolved as far as the path exists (a copy that is already removed still gets the key it had)
    let mut rest = PathBuf::new();
    let mut base = path.to_path_buf();
    let real = loop {
        if let Ok(r) = base.canonicalize() {
            break if rest.as_os_str().is_empty() { r } else { r.join(&rest) };
        }
        let (Some(name), Some(parent)) = (base.file_name().map(PathBuf::from), base.parent().map(Path::to_path_buf)) else { break path.to_path_buf() };
        rest = if rest.as_os_str().is_empty() { name } else { name.join(&rest) };
        base = parent;
    };
    real.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

/// Same format check as the adapters make before `--resume` (`[A-Za-z0-9._:-]`, up to 128, no leading `-`).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && !id.starts_with('-') && id.chars().all(|c| c.is_ascii_alphanumeric() || "._:-".contains(c))
}

/// One page of the sessions of `project_path` from `~/.claude`.
pub fn list(project_path: &Path, page: usize) -> Vec<SessionHead> {
    claude_dir().map(|d| list_in(&d.join("projects"), project_path, page)).unwrap_or_default()
}

/// How many session files the project has (the window needs it for "more").
pub fn count(project_path: &Path) -> usize {
    claude_dir().map(|d| count_in(&d.join("projects"), project_path)).unwrap_or(0)
}

pub fn count_in(projects: &Path, project_path: &Path) -> usize {
    files_in(projects, project_path).len()
}

/// The head of session `id` when it is one of the project's own files.
pub fn find_in(projects: &Path, project_path: &Path, id: &str) -> Option<SessionHead> {
    files_in(projects, project_path).into_iter().find(|f| f.0 == id).map(|(id, path, mtime)| head(&id, &path, mtime))
}

/// `list` with the `projects` folder given (tests use a temporary one).
pub fn list_in(projects: &Path, project_path: &Path, page: usize) -> Vec<SessionHead> {
    files_in(projects, project_path).into_iter().skip(page.saturating_mul(PAGE)).take(PAGE).map(|(id, path, mtime)| head(&id, &path, mtime)).collect()
}

/// Regular `.jsonl` files of the project's key, newest first by modification time. A symlink is not followed.
fn files_in(projects: &Path, project_path: &Path) -> Vec<(String, PathBuf, DateTime<Local>)> {
    let Ok(rd) = std::fs::read_dir(projects.join(key_for(project_path))) else { return vec![] };
    let mut out: Vec<(String, PathBuf, DateTime<Local>)> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            let id = path.file_name()?.to_str()?.strip_suffix(".jsonl")?.to_string();
            let md = std::fs::symlink_metadata(&path).ok().filter(|m| m.is_file())?;
            valid_id(&id).then(|| (id, path, md.modified().map(DateTime::<Local>::from).unwrap_or_else(|_| Local::now())))
        })
        .collect();
    out.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    out
}

/// Reads the beginning of one file; never fails: a file that cannot be read gives a head marked `unreadable`.
fn head(id: &str, path: &Path, mtime: DateTime<Local>) -> SessionHead {
    let mut h = SessionHead { id: id.into(), started_at: mtime, first_prompt: String::new(), messages: 0, approx: false, branch: None, unreadable: false };
    let Ok(f) = std::fs::File::open(path) else {
        h.unreadable = true;
        return h;
    };
    let total = f.metadata().map(|m| m.len()).unwrap_or(0);
    h.approx = total > MAX_READ;
    let mut rd = BufReader::new(f.take(MAX_READ));
    let (mut buf, mut objects, mut lines, mut got_time) = (Vec::new(), 0u32, 0u32, false);
    loop {
        buf.clear();
        let n = match rd.by_ref().take(MAX_LINE).read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        let whole = buf.ends_with(b"\n") || (n as u64) < MAX_LINE;
        if !whole {
            // a line longer than the cap: the rest of it is dropped, the head of it still counts
            let mut sink = Vec::new();
            while rd.by_ref().take(MAX_LINE).read_until(b'\n', &mut sink).map(|k| k > 0 && !sink.ends_with(b"\n")).unwrap_or(false) {
                sink.clear();
            }
        }
        let text = String::from_utf8_lossy(&buf);
        let line = text.trim();
        if line.is_empty() {
            continue;
        }
        lines += 1;
        if !line.starts_with('{') {
            continue;
        }
        let is_msg = line.contains("\"type\":\"user\"") || line.contains("\"type\":\"assistant\"");
        // every whole line is parsed: a broken line is skipped, a file without one good line is "unreadable"
        let v: Option<Value> = if whole { serde_json::from_str(line).ok().filter(Value::is_object) } else { None };
        if v.is_some() || !whole {
            objects += 1;
        } else {
            continue;
        }
        if is_msg {
            h.messages += 1;
        }
        if let Some(v) = v {
            if !got_time {
                if let Some(t) = v["timestamp"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()) {
                    h.started_at = t.with_timezone(&Local);
                    got_time = true;
                }
            }
            if h.branch.is_none() {
                h.branch = v["gitBranch"].as_str().filter(|b| !b.is_empty() && *b != "HEAD").map(|b| b.chars().take(120).collect());
            }
            if h.first_prompt.is_empty() && v["type"] == "user" && v["isMeta"] != true && v["isSidechain"] != true {
                h.first_prompt = prompt_text(&v["message"]["content"]);
            }
        }
    }
    // text lines but not one of them a JSON object: not a session file (story 329)
    h.unreadable = lines > 0 && objects == 0;
    h
}

/// The person's own words of a user record: a plain string or `text` parts; tool results and command wrappers give nothing.
fn prompt_text(content: &Value) -> String {
    let raw = match content {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().filter(|p| p["type"] == "text").filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join(" "),
        _ => String::new(),
    };
    let t = raw.trim();
    if t.starts_with("<command-") || t.starts_with("<local-command") || t.starts_with("Caveat:") || t.starts_with("[Request interrupted") {
        return String::new();
    }
    t.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_PROMPT).collect()
}

/// Puts a copy of session `id` of `project_path` under the key of `worktree`, where `claude --resume` looks for it when it
/// runs there. The source file is only read. Already there = left as it is (the session may have moved on).
pub fn copy_into(project_path: &Path, id: &str, worktree: &Path) -> Result<PathBuf, String> {
    let root = claude_dir().ok_or("no_home")?.join("projects");
    copy_into_in(&root, project_path, id, worktree)
}

pub fn copy_into_in(projects: &Path, project_path: &Path, id: &str, worktree: &Path) -> Result<PathBuf, String> {
    if !valid_id(id) {
        return Err(format!("bad_session_id: {id}"));
    }
    let src = projects.join(key_for(project_path)).join(format!("{id}.jsonl"));
    if !std::fs::symlink_metadata(&src).is_ok_and(|m| m.is_file()) {
        return Err(format!("session_not_found: {id}"));
    }
    let dir = projects.join(key_for(worktree));
    let dst = dir.join(format!("{id}.jsonl"));
    if dst == src {
        return Ok(dst);
    }
    if std::fs::symlink_metadata(&dst).is_ok() {
        return Ok(dst);
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("session_copy_failed: {e}"))?;
    let tmp = dir.join(format!("{id}.jsonl.tmp"));
    std::fs::copy(&src, &tmp).map_err(|e| format!("session_copy_failed: {e}"))?;
    std::fs::rename(&tmp, &dst).map_err(|e| format!("session_copy_failed: {e}"))?;
    Ok(dst)
}
