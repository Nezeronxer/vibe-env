//! The error report and the app log (G71).
//!
//! `collect` writes a zip a person can hand to an agent: versions, the OS, the CLIs, the settings without
//! secrets, the states of tasks and chats without their texts, the tail of the app log and a file that says what
//! is inside. It goes by a white list of fields: nothing is copied whole from the data folder, so chat texts,
//! prompts, diffs, task logs, copies of projects and kit secrets never get in. Every text that does get in passes
//! `scrub::clean` (token shapes, `NAME=value`, home paths -> `~`). Nothing is sent anywhere.
//!
//! `AppLog` is `<data>/logs/app.log`: one line per event (a key and short facts, never a message text), rotated
//! at 1 MB, five files kept.
use crate::scrub;
use chrono::Local;
use serde_json::{json, Map, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub const LOG_DIR: &str = "logs";
pub const LOG_FILE: &str = "app.log";
/// One file of the log grows to this size before it is rotated.
pub const LOG_MAX_BYTES: u64 = 1024 * 1024;
/// Files of the log kept: `app.log` and `app.log.1` .. `app.log.4`.
pub const LOG_KEEP: usize = 5;
/// The longest line the log keeps (an event, not a text).
const LINE_MAX: usize = 300;
/// Lines of the log in the report.
pub const REPORT_LOG_LINES: usize = 300;

/// What the app knows and the data folder does not: the versions of the CLIs of the envs.
#[derive(Clone, Debug, Default)]
pub struct Extra {
    /// (env id, version as the CLI says it; None = not installed or not read)
    pub envs: Vec<(String, Option<String>)>,
}

/// The app log: `<data>/logs/app.log`, rotated.
pub struct AppLog {
    dir: PathBuf,
    max_bytes: u64,
    keep: usize,
    home: String,
}

impl AppLog {
    pub fn new(data_dir: &Path) -> AppLog {
        AppLog::with_limits(data_dir, LOG_MAX_BYTES, LOG_KEEP)
    }

    pub fn with_limits(data_dir: &Path, max_bytes: u64, keep: usize) -> AppLog {
        let home = std::env::var("HOME").unwrap_or_default();
        AppLog { dir: data_dir.join(LOG_DIR), max_bytes, keep: keep.max(1), home }
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join(LOG_FILE)
    }

    /// One line: time, level, the event. The event is scrubbed, put on one line and cut short.
    pub fn write(&self, level: &str, event: &str) {
        let one: String = scrub::clean(event, &self.home).replace(['\n', '\r'], " ").chars().take(LINE_MAX).collect();
        let line = format!("{} {} {}\n", Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%:z"), level, one);
        if fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let path = self.path();
        if fs::metadata(&path).is_ok_and(|m| m.len() + line.len() as u64 > self.max_bytes) {
            self.rotate();
        }
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = f.write_all(line.as_bytes());
        }
    }

    fn rotate(&self) {
        let name = |i: usize| if i == 0 { self.path() } else { self.dir.join(format!("{LOG_FILE}.{i}")) };
        let _ = fs::remove_file(name(self.keep - 1));
        for i in (0..self.keep - 1).rev() {
            let _ = fs::rename(name(i), name(i + 1));
        }
    }

    /// The last `n` lines, oldest first (from the rotated file too when the current one is short).
    pub fn tail(&self, n: usize) -> Vec<String> {
        let mut lines: Vec<String> = vec![];
        for i in 0..self.keep {
            let p = if i == 0 { self.path() } else { self.dir.join(format!("{LOG_FILE}.{i}")) };
            let Ok(text) = fs::read_to_string(&p) else { break };
            let mut these: Vec<String> = text.lines().map(String::from).collect();
            these.append(&mut lines);
            lines = these;
            if lines.len() >= n {
                break;
            }
        }
        let skip = lines.len().saturating_sub(n);
        lines.into_iter().skip(skip).collect()
    }
}

static GLOBAL: OnceLock<Mutex<AppLog>> = OnceLock::new();

/// Opens the app log of `data_dir` for `log` (once; later calls are ignored).
pub fn init(data_dir: &Path) {
    let _ = GLOBAL.set(Mutex::new(AppLog::new(data_dir)));
}

/// One event in the app log (after `init`; before it, nothing). `event` is a key with short facts, never the
/// text of a message, a prompt or an answer.
pub fn log(level: &str, event: &str) {
    if let Some(l) = GLOBAL.get() {
        l.lock().unwrap_or_else(|e| e.into_inner()).write(level, event);
    }
}

// ---- the report ----

/// Settings strings that are words of the app, not the person's data; any other string is only said to be set.
const SETTING_WORDS: &[&str] = &["language", "theme", "sound", "hotkey", "family", "version", "mode", "night_from", "night_to", "quiet_from", "quiet_to", "density", "accent"];
/// Settings subtrees that hold the person's commands, contacts or paths: left out whole.
const SETTING_OUT: &[&str] = &["custom_agents", "telegram", "role_tools", "default_project", "keys"];

fn settings_view(v: &Value) -> Value {
    fn walk(key: &str, v: &Value) -> Value {
        match v {
            Value::Object(m) => {
                let mut out = Map::new();
                for (k, x) in m {
                    if SETTING_OUT.contains(&k.as_str()) {
                        out.insert(k.clone(), json!("<left out>"));
                    } else {
                        out.insert(k.clone(), walk(k, x));
                    }
                }
                Value::Object(out)
            }
            Value::Array(a) => Value::Array(a.iter().map(|x| walk(key, x)).collect()),
            Value::String(s) if SETTING_WORDS.contains(&key) && s.len() <= 40 => Value::String(s.clone()),
            Value::String(s) if s.is_empty() => json!(""),
            Value::String(_) => json!("<set>"),
            other => other.clone(),
        }
    }
    walk("", v)
}

/// A state of a step or a run as its name only: `"done"`, `{"failed": "<text>"}` -> `"failed"`.
fn state_name(v: &Value) -> Value {
    let name = match v {
        Value::String(s) => s.clone(),
        Value::Object(m) => m.keys().next().cloned().unwrap_or_default(),
        _ => String::new(),
    };
    json!(word(&name))
}

/// A key-like word (`[a-z0-9_]`, up to 40); anything else is not a word of the app and is dropped.
fn word(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty() && s.len() <= 40 && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')).then(|| s.to_string())
}

/// The error of a task as its key: `agent_failed: <text>` -> `agent_failed`; free text -> `<text>`.
fn error_key(v: &Value) -> Value {
    match v.as_str() {
        None => Value::Null,
        Some(s) => json!(word(s.split([':', ' ', '\n']).next().unwrap_or("")).unwrap_or_else(|| "<text>".into())),
    }
}

fn pick(v: &Value, keys: &[&str]) -> Map<String, Value> {
    let mut out = Map::new();
    for k in keys {
        if let Some(x) = v.get(*k) {
            out.insert(k.to_string(), x.clone());
        }
    }
    out
}

fn task_view(t: &Value) -> Value {
    let mut out = pick(t, &["id", "project_id", "agent", "attempts", "review_rounds", "resume_at", "created_at", "started_at", "finished_at", "cost_usd", "budget_usd", "context_pct"]);
    out.insert("status".into(), state_name(&t["status"]));
    if let Some(s) = t.get("swarm").filter(|s| !s.is_null()) {
        out.insert("swarm".into(), json!({ "mode": state_name(&s["mode"]), "n": s["n"] }));
    }
    let steps: Vec<Value> = t["steps"].as_array().map(|a| a.iter().map(|x| json!([state_name(&x[0]), state_name(&x[1])])).collect()).unwrap_or_default();
    out.insert("steps".into(), json!(steps));
    let runs: Vec<Value> = t["runs"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| {
                    let mut m = pick(r, &["variant", "started_at", "finished_at", "cost_usd"]);
                    m.insert("role".into(), state_name(&r["role"]));
                    m.insert("model".into(), json!(r["model"].as_str().map(|s| s.chars().take(60).collect::<String>())));
                    m.insert("state".into(), state_name(&r["state"]));
                    Value::Object(m)
                })
                .collect()
        })
        .unwrap_or_default();
    out.insert("runs".into(), json!(runs));
    out.insert("error".into(), error_key(&t["outcome"]["error"]));
    out.insert("has_question".into(), json!(!t["question"].is_null()));
    Value::Object(out)
}

fn chat_view(c: &Value) -> Value {
    let mut out = pick(c, &["id", "env", "project_id", "created_at", "updated_at", "archived"]);
    out.insert("queued".into(), json!(c["queue"].as_array().map_or(0, |q| q.len())));
    out.insert("waits_for_limit".into(), json!(c.get("wait").is_some_and(|w| !w.is_null())));
    Value::Object(out)
}

const WHAT_INSIDE: &str = "Nightshift error report / отчёт об ошибке

What is inside (только техническое):
- versions.json - the app version, OS, architecture, the CLI version of every env
- settings.json - the settings; strings other than words of the app are shown only as <set>,
  own commands, contacts and paths are <left out>
- tasks.json - per task: id, project id, env, status, steps, runs (role, model, state, times, cost),
  the error key; no titles, prompts, answers, diffs or logs
- chats.json - per chat: id, env, times, how many messages wait; no texts
- data-files.json - names and sizes of the app's own files in the data folder
- app.log - the last lines of the app log (events only)

What is NOT inside (не попадает): chat texts, prompts, answers, diffs, task logs, copies of projects,
terminal output, kit files and secrets. Everything above passed a scrub of tokens, passwords and home
paths (~). Nothing was sent anywhere: the archive is only on this disk.
";

/// Builds the report zip in `out_dir`; the path of the zip.
pub fn collect(data_dir: &Path, out_dir: &Path) -> Result<PathBuf, String> {
    collect_with(data_dir, out_dir, &Extra::default())
}

pub fn collect_with(data_dir: &Path, out_dir: &Path, extra: &Extra) -> Result<PathBuf, String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let clean = |v: &Value| scrub::clean(&serde_json::to_string_pretty(v).unwrap_or_default(), &home);
    let state: Value = fs::read(data_dir.join("state.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);

    let versions = json!({
        "app": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "created_at": Local::now().to_rfc3339(),
        "envs": extra.envs.iter().map(|(id, v)| json!({ "env": id, "version": v.as_deref().map(|s| s.chars().take(60).collect::<String>()) })).collect::<Vec<_>>(),
    });
    let settings = settings_view(&state["settings"]);
    let tasks: Vec<Value> = state["tasks"].as_array().map(|a| a.iter().map(task_view).collect()).unwrap_or_default();
    let chats: Vec<Value> = state["chats"].as_array().map(|a| a.iter().map(chat_view).collect()).unwrap_or_default();
    // names of the app's own files at the top of the data folder (never inside chats/, logs/, worktrees/, kits/)
    let mut files: Vec<Value> = fs::read_dir(data_dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| {
                    let dir = e.file_type().is_ok_and(|t| t.is_dir());
                    let size = if dir { None } else { e.metadata().ok().map(|m| m.len()) };
                    json!({ "name": e.file_name().to_string_lossy(), "dir": dir, "bytes": size })
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort_by_key(|v| v["name"].as_str().unwrap_or("").to_string());
    let log: Vec<String> = AppLog::new(data_dir).tail(REPORT_LOG_LINES).iter().map(|l| scrub::clean(l, &home)).collect();

    fs::create_dir_all(out_dir).map_err(|e| format!("out_dir: {e}"))?;
    let name = format!("nightshift-report-{}.zip", Local::now().format("%Y%m%d-%H%M%S"));
    let path = out_dir.join(name);
    let file = fs::File::create(&path).map_err(|e| format!("zip: {e}"))?;
    let mut z = zip::ZipWriter::new(file);
    let opt = zip::write::SimpleFileOptions::default().unix_permissions(0o644);
    let entries: Vec<(&str, String)> = vec![
        ("README.txt", WHAT_INSIDE.to_string()),
        ("versions.json", clean(&versions)),
        ("settings.json", clean(&settings)),
        ("tasks.json", clean(&json!(tasks))),
        ("chats.json", clean(&json!(chats))),
        ("data-files.json", clean(&json!(files))),
        ("app.log", log.join("\n")),
    ];
    for (n, body) in entries {
        z.start_file(n, opt).map_err(|e| format!("zip: {e}"))?;
        z.write_all(body.as_bytes()).map_err(|e| format!("zip: {e}"))?;
    }
    z.finish().map_err(|e| format!("zip: {e}"))?;
    Ok(path)
}
