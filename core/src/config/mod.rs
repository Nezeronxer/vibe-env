//! The config file `~/.config/nightshift/config` (stories 293, 294): `key = value` lines like Ghostty/kitty, `#`
//! starts a comment line. Keys are the settings' own fields, dotted and with dashes (`look.font-px`,
//! `max-parallel`); a wrong line is an error with its number and the rest still applies. The settings screen
//! writes into the same file, keeping the person's comments and order.
use crate::model::{Settings, SETTING_RANGES};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ConfigFileError {
    #[error("unknown_key: {0}")]
    UnknownKey(String),
    #[error("io: {0}")]
    Io(String),
}

pub type Result<T> = std::result::Result<T, ConfigFileError>;

/// One line the file could not apply. `msg` is a key the window translates: `unknown_key`, `need_number`,
/// `need_bool`, `out_of_range: <min>..<max>`, `bad_value`, `bad_line`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConfigError {
    pub line: usize,
    pub key: String,
    pub msg: String,
}

/// Window state, removed features and runtime switches: not settings a person writes.
const SKIP: [&str; 4] = ["ui", "telegram", "paused", "default_project"];
/// Friendlier names that mean the same key.
const ALIASES: [(&str, &str); 1] = [("font-size", "look.font-px")];

fn io(e: std::io::Error) -> ConfigFileError {
    ConfigFileError::Io(e.to_string())
}

fn leaves(v: &Value, path: &mut Vec<String>, out: &mut Vec<(String, Vec<String>, Value)>) {
    for (k, x) in v.as_object().into_iter().flatten() {
        if path.is_empty() && SKIP.contains(&k.as_str()) {
            continue;
        }
        path.push(k.clone());
        match x.as_object() {
            Some(o) if !o.is_empty() => leaves(x, path, out),
            _ => out.push((path.join(".").replace('_', "-"), path.clone(), x.clone())),
        }
        path.pop();
    }
}

/// Every key of the file with its path in `Settings` and its default, in the settings' order.
pub fn keys() -> Vec<(String, Vec<String>, Value)> {
    let mut out = vec![];
    leaves(&serde_json::to_value(Settings::default()).unwrap_or_default(), &mut vec![], &mut out);
    out
}

fn canonical(key: &str) -> &str {
    ALIASES.iter().find(|(a, _)| *a == key).map_or(key, |(_, k)| k)
}

fn get<'a>(v: &'a Value, path: &[String]) -> Option<&'a Value> {
    path.iter().try_fold(v, |v, k| v.get(k))
}

fn set(v: &mut Value, path: &[String], x: Value) {
    let mut cur = v;
    for k in &path[..path.len() - 1] {
        if !cur.is_object() {
            *cur = Value::Object(Map::new());
        }
        cur = cur.as_object_mut().expect("object").entry(k.clone()).or_insert_with(|| Value::Object(Map::new()));
    }
    if let Some(o) = cur.as_object_mut() {
        o.insert(path[path.len() - 1].clone(), x);
    }
}

fn merge(base: &mut Value, patch: &Value) {
    match (base.as_object_mut(), patch.as_object()) {
        (Some(b), Some(p)) => {
            for (k, x) in p {
                match b.get_mut(k) {
                    Some(y) if y.is_object() && x.is_object() => merge(y, x),
                    _ => {
                        b.insert(k.clone(), x.clone());
                    }
                }
            }
        }
        _ => *base = patch.clone(),
    }
}

/// The text of a value as the person writes it; typed by the key's default.
fn parse(raw: &str, default: &Value) -> std::result::Result<Value, &'static str> {
    if raw.starts_with('"') {
        return serde_json::from_str::<String>(raw).map(Value::String).map_err(|_| "bad_value");
    }
    if raw.starts_with('[') || raw.starts_with('{') {
        return serde_json::from_str(raw).map_err(|_| "bad_value");
    }
    let num = || raw.parse::<i64>().map(Value::from).or_else(|_| raw.parse::<f64>().map(Value::from)).ok();
    let boolean = || match raw {
        "true" => Some(Value::Bool(true)),
        "false" => Some(Value::Bool(false)),
        _ => None,
    };
    match default {
        Value::Number(_) => num().ok_or("need_number"),
        Value::Bool(_) => boolean().ok_or("need_bool"),
        Value::String(_) => Ok(Value::String(raw.into())),
        Value::Null if raw.is_empty() => Ok(Value::Null),
        Value::Null => Ok(boolean().or_else(num).unwrap_or_else(|| Value::String(raw.into()))),
        _ => Err("bad_value"),
    }
}

/// How a value is written back: bare where it reads back the same, else JSON.
fn render(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => {
            let plain = !s.is_empty() && s.trim() == s && !s.starts_with(['"', '[', '{']) && s != "true" && s != "false" && s.parse::<f64>().is_err();
            if plain {
                s.clone()
            } else {
                Value::String(s.clone()).to_string()
            }
        }
        other => other.to_string(),
    }
}

/// `key = value` of a line, if it is one.
fn split_line(line: &str) -> Option<(&str, &str)> {
    line.split_once('=').map(|(k, v)| (k.trim(), v.trim()))
}

fn range_error(path: &[String], v: &Value) -> Option<String> {
    let name = path.join(".");
    let (_, min, max, _) = SETTING_RANGES.iter().find(|r| r.0 == name)?;
    let x = v.as_f64()?;
    (x < *min || x > *max).then(|| format!("out_of_range: {min}..{max}"))
}

pub struct Config {
    pub path: PathBuf,
}

impl Config {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// `~/.config/nightshift/config`, on macOS too (story 293).
    pub fn default_path() -> PathBuf {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config/nightshift/config")
    }

    fn text(&self) -> Result<String> {
        match std::fs::read_to_string(&self.path) {
            Ok(t) => Ok(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(io(e)),
        }
    }

    /// A patch of `Settings` (JSON, only the keys the file sets) and the lines that were wrong. No file = empty.
    pub fn load(&self) -> Result<(Value, Vec<ConfigError>)> {
        let text = self.text()?;
        let known = keys();
        let defaults = serde_json::to_value(Settings::default()).unwrap_or_default();
        let mut patch = Value::Object(Map::new());
        let mut errors = vec![];
        for (i, line) in text.lines().enumerate() {
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            let mut err = |key: &str, msg: String| errors.push(ConfigError { line: i + 1, key: key.into(), msg });
            let Some((key, raw)) = split_line(t) else {
                err(t, "bad_line".into());
                continue;
            };
            let Some((_, path, default)) = known.iter().find(|k| k.0 == canonical(key)) else {
                err(key, "unknown_key".into());
                continue;
            };
            let value = match parse(raw, default) {
                Ok(v) => v,
                Err(m) => {
                    err(key, m.into());
                    continue;
                }
            };
            if let Some(m) = range_error(path, &value) {
                err(key, m);
                continue;
            }
            let mut next = patch.clone();
            set(&mut next, path, value);
            let mut whole = defaults.clone();
            merge(&mut whole, &next);
            if serde_json::from_value::<Settings>(whole).is_err() {
                err(key, "bad_value".into());
                continue;
            }
            patch = next;
        }
        Ok((patch, errors))
    }

    /// `current` with the file's keys over it, and the file's errors (a wrong key keeps `current`'s value).
    pub fn apply(&self, current: &Settings) -> Result<(Settings, Vec<ConfigError>)> {
        let (patch, errors) = self.load()?;
        let mut v = serde_json::to_value(current).unwrap_or_default();
        merge(&mut v, &patch);
        Ok((serde_json::from_value(v).unwrap_or_else(|_| current.clone()), errors))
    }

    /// Sets one key: its line is replaced in place, else it goes under its commented default, else at the end.
    pub fn write_key(&self, key: &str, value: &str) -> Result<()> {
        let key = canonical(key);
        if !keys().iter().any(|k| k.0 == key) {
            return Err(ConfigFileError::UnknownKey(key.into()));
        }
        let text = self.text()?;
        let mut lines: Vec<String> = text.lines().map(String::from).collect();
        let line = format!("{key} = {value}");
        let is_key = |l: &str| split_line(l).is_some_and(|(k, _)| canonical(k) == key);
        if let Some(i) = lines.iter().rposition(|l| !l.trim_start().starts_with('#') && is_key(l)) {
            lines[i] = line;
        } else if let Some(i) = lines.iter().position(|l| l.trim_start().strip_prefix('#').is_some_and(|r| is_key(r.trim()))) {
            lines.insert(i + 1, line);
        } else {
            lines.push(line);
        }
        self.write(&(lines.join("\n") + "\n"))
    }

    fn write(&self, text: &str) -> Result<()> {
        let dir = self.path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir).map_err(io)?;
        let tmp = dir.join(".config.nightshift-tmp");
        std::fs::write(&tmp, text).map_err(io)?;
        std::fs::rename(&tmp, &self.path).map_err(io)
    }

    /// The settings screen saved `s`: every key whose value differs from what the file gives is written.
    /// No file = nothing (it is made by "Open config"). Returns how many keys were written.
    pub fn write_settings(&self, s: &Settings) -> Result<usize> {
        if !self.path.exists() {
            return Ok(0);
        }
        let (file, _) = self.apply(&Settings::default())?;
        let (now, was) = (serde_json::to_value(s).unwrap_or_default(), serde_json::to_value(file).unwrap_or_default());
        let mut n = 0;
        for (key, path, _) in keys() {
            match (get(&now, &path), get(&was, &path)) {
                (Some(a), b) if Some(a) != b => {
                    self.write_key(&key, &render(a))?;
                    n += 1;
                }
                _ => {}
            }
        }
        Ok(n)
    }

    /// "Open config" with no file: every key, commented, with its default. false = the file was already there.
    pub fn create_default(&self) -> Result<bool> {
        if self.path.exists() {
            return Ok(false);
        }
        let mut text = String::from("# Nightshift config: one `key = value` per line, `#` starts a comment.\n# Every line below is a default: remove the `#` to change it.\n\n");
        for (key, _, default) in keys() {
            text.push_str(&format!("# {key} = {}\n", render(&default)));
        }
        self.write(&text)?;
        Ok(true)
    }

    /// Calls `cb` with the file's errors after every change (mtime or size polled once a second, no extra crate).
    pub fn watch(&self, cb: Box<dyn Fn(Vec<ConfigError>) + Send>) -> Result<()> {
        let path = self.path.clone();
        let stamp = move |p: &Path| std::fs::metadata(p).ok().map(|m| (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len()));
        let mut last = stamp(&path);
        std::thread::Builder::new()
            .name("config-watch".into())
            .spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                let now = stamp(&path);
                if now != last {
                    last = now;
                    cb(Config::new(path.clone()).load().map(|x| x.1).unwrap_or_default());
                }
            })
            .map(|_| ())
            .map_err(io)
    }
}
