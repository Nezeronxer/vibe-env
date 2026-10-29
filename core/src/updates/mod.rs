//! CLI versions, how each CLI was installed, its update with rollback, and new models (stories 274-277, 299;
//! research-updates.md). Never `sudo`: an update runs the CLI's own command or the manager that installed it
//! (found by the real path of the binary), with no stdin, so nothing can ask for a password.
use crate::agent::adapters::common::capture;
use crate::agent::Adapter;
use crate::model::{Catalog, CatalogSource, ModelFamily};
use crate::models::radar;
use crate::net::Fetch;
use crate::store::Store;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub const MODELS_DEV: &str = "https://models.dev/api.json";
const MODELS_DEV_MAX: usize = 16 << 20;
const NPM_MAX: usize = 2 << 20;
/// `--version` and `--help` after an update must answer within this (research-updates §4.2).
const PROBE: Duration = Duration::from_secs(5);
const RUN_MAX: Duration = Duration::from_secs(600);
/// How long an update waits for the env's running launches; then the next scheduled check tries again.
const WAIT_MAX: Duration = Duration::from_secs(30 * 60);
const HISTORY_MAX: usize = 200;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum UpdateError {
    #[error("env_not_found: {0}")]
    EnvNotFound(String),
    #[error("not_installed: {0}")]
    NotInstalled(String),
    /// No update command known for this install (Gemini outside npm/brew, an own command).
    #[error("no_update_command: {0}")]
    NoCommand(String),
    /// 1.x -> 2.x: only reported, never installed by itself.
    #[error("major_jump: {from} -> {to}")]
    MajorJump { from: String, to: String },
    /// The env kept running launches for `WAIT_MAX`.
    #[error("env_busy: {0}")]
    Busy(String),
}

impl UpdateError {
    pub fn key(&self) -> &'static str {
        match self {
            UpdateError::EnvNotFound(_) => "env_not_found",
            UpdateError::NotInstalled(_) => "not_installed",
            UpdateError::NoCommand(_) => "no_update_command",
            UpdateError::MajorJump { .. } => "major_jump",
            UpdateError::Busy(_) => "env_busy",
        }
    }
}

pub type Result<T> = std::result::Result<T, UpdateError>;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    Native,
    Npm,
    Brew,
    Unknown,
}

/// Per env (story 274): update by itself, only report, or nothing.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Auto,
    Notify,
    Off,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CliStatus {
    pub installed: Option<String>,
    /// npm `latest`; None = no public source (Cursor) or no network.
    pub latest: Option<String>,
    pub method: Method,
    /// The update command (for the person to run by hand when it failed); empty = none known.
    pub cmd: Vec<String>,
    /// `latest` is a new major: reported, not installed.
    pub major: bool,
    pub path: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UpdateLog {
    pub env: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub ok: bool,
    pub output: String,
    pub at: DateTime<Local>,
    pub method: Method,
    pub rolled_back: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NewModel {
    pub env: String,
    pub id: String,
    pub name: String,
    /// "Latest" of an existing family moved to it by itself; false = a new family or a new generation (button).
    pub auto: bool,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct State {
    history: Vec<UpdateLog>,
    /// env -> version before an update that has not finished (story 276: continued at the next start).
    pending: BTreeMap<String, Option<String>>,
    modes: BTreeMap<String, Mode>,
    etag: Option<String>,
    /// env -> model ids already announced (or there at the first look).
    seen: BTreeMap<String, BTreeSet<String>>,
}

type Busy = Arc<dyn Fn(&str) -> bool + Send + Sync>;
/// env, families before, families now, models.dev names by id.
type Found = (String, Vec<ModelFamily>, Vec<ModelFamily>, BTreeMap<String, String>);

pub struct Updates {
    fetch: Box<dyn Fetch>,
    adapters: Mutex<Vec<Arc<dyn Adapter>>>,
    file: Option<PathBuf>,
    busy: Busy,
    updating: Mutex<BTreeSet<String>>,
    state: Mutex<State>,
}

fn lk<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// First `1.2.3[-tag]` in a `--version` output (`2.1.309 (Claude Code)` -> `2.1.309`).
pub fn parse_version(text: &str) -> Option<String> {
    static R: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let r = R.get_or_init(|| regex::Regex::new(r"\d+\.\d+(?:\.\d+)?(?:-[0-9A-Za-z.]+)?").unwrap());
    r.find(text).map(|m| m.as_str().to_string())
}

/// npm package of an env (research-updates §2); Cursor has none.
pub fn npm_package(env: &str) -> Option<&'static str> {
    match env {
        "claude" => Some("@anthropic-ai/claude-code"),
        "codex" => Some("@openai/codex"),
        "gemini" => Some("@google/gemini-cli"),
        "opencode" => Some("opencode-ai"),
        "qwen" => Some("@qwen-code/qwen-code"),
        // research-clis §7-14 (`npm view`, 2026-10-28)
        "kimi" => Some("@moonshot-ai/kimi-code"),
        "copilot" => Some("@github/copilot"),
        "crush" => Some("@charmland/crush"),
        "amp" => Some("@ampcode/cli"),
        "droid" => Some("droid"),
        _ => None,
    }
}

/// Envs whose CLI updates itself with its own command.
fn native_cmd(env: &str) -> Option<&'static str> {
    match env {
        // copilot, amp, goose, droid, grok: `<cli> update` per their docs (research-clis §7-14), not run here
        "claude" | "codex" | "qwen" | "cursor" | "copilot" | "amp" | "goose" | "droid" | "grok" => Some("update"),
        "opencode" => Some("upgrade"),
        _ => None,
    }
}

/// How the CLI got here, by the real path of its binary.
pub fn install_method(env: &str, real: &Path) -> Method {
    let p = real.to_string_lossy();
    if p.contains("/node_modules/") {
        Method::Npm
    } else if p.contains("/Cellar/") || p.contains("/Caskroom/") {
        Method::Brew
    } else if native_cmd(env).is_some() {
        Method::Native
    } else {
        Method::Unknown
    }
}

/// The part of `p` before `marker`, and the path segment right after it.
fn split_at(p: &Path, marker: &str) -> Option<(PathBuf, String)> {
    let s = p.to_string_lossy();
    let i = s.find(marker)?;
    let next = s[i + marker.len()..].split('/').next()?.to_string();
    Some((PathBuf::from(&s[..i]), next))
}

/// The manager next to the install (the same prefix gets the update), else the one in PATH.
fn tool(prefix: &Path, name: &str) -> String {
    let p = prefix.join("bin").join(name);
    if p.is_file() {
        p.to_string_lossy().into_owned()
    } else {
        name.to_string()
    }
}

fn npm_of(path: &Path) -> String {
    let prefix = split_at(path, "/lib/node_modules/").or_else(|| split_at(path, "/node_modules/")).map(|x| x.0).unwrap_or_default();
    tool(&prefix, "npm")
}

/// The update command. `path`: the binary the person calls (Native) or its real path (npm, brew).
pub fn update_cmd(env: &str, method: Method, path: &Path) -> Option<Vec<String>> {
    match method {
        Method::Native => native_cmd(env).map(|c| vec![path.to_string_lossy().into_owned(), c.into()]),
        Method::Npm => npm_package(env).map(|p| vec![npm_of(path), "install".into(), "-g".into(), format!("{p}@latest")]),
        Method::Brew => {
            let (prefix, name, cask) = match split_at(path, "/Caskroom/") {
                Some((p, n)) => (p, n, true),
                None => {
                    let (p, n) = split_at(path, "/Cellar/")?;
                    (p, n, false)
                }
            };
            // a brew upgrade with a name only, and without refreshing all of brew first (research-updates §4.4)
            let mut c = vec!["/usr/bin/env".into(), "HOMEBREW_NO_AUTO_UPDATE=1".into(), tool(&prefix, "brew"), "upgrade".into()];
            if cask {
                c.push("--cask".into());
            }
            c.push(name);
            Some(c)
        }
        Method::Unknown => None,
    }
}

/// Back to `from` with the same manager; None = no reliable way (brew, most native installers).
fn rollback_cmd(env: &str, method: Method, path: &Path, from: &str) -> Option<Vec<String>> {
    match (method, env) {
        (Method::Npm, _) => npm_package(env).map(|p| vec![npm_of(path), "install".into(), "-g".into(), format!("{p}@{from}")]),
        (Method::Native, "opencode") => Some(vec![path.to_string_lossy().into_owned(), "upgrade".into(), from.into()]),
        // `droid update --version X`, `grok update --version X` (their docs; not run here)
        (Method::Native, "droid" | "grok") => Some(vec![path.to_string_lossy().into_owned(), "update".into(), "--version".into(), from.into()]),
        _ => None,
    }
}

fn run(cmd: &[String], max: Duration) -> std::result::Result<String, String> {
    let args: Vec<&str> = cmd[1..].iter().map(String::as_str).collect();
    capture(&cmd[0], &args, max)
}

fn version_of(cli: &Path) -> Option<String> {
    capture(&cli.to_string_lossy(), &["--version"], PROBE).ok().and_then(|o| parse_version(&o))
}

fn healthy(cli: &Path) -> Option<String> {
    let v = version_of(cli)?;
    capture(&cli.to_string_lossy(), &["--help"], PROBE).ok().map(|_| v)
}

fn first_num(v: &str) -> u64 {
    v.split(['.', '-']).next().and_then(|n| n.parse().ok()).unwrap_or(0)
}

fn tail(s: &str) -> String {
    let n = s.chars().count();
    s.chars().skip(n.saturating_sub(4000)).collect()
}

/// A native install that switched its link to a new file (Claude: `versions/<v>`): point the link back.
fn relink(link: &Path, old_real: &Path) -> bool {
    if !link.is_symlink() || !old_real.is_file() || crate::proc::canon(link).ok().as_deref() == Some(old_real) {
        return false;
    }
    let tmp = link.with_file_name(format!(".{}.nightshift-rollback", link.file_name().unwrap_or_default().to_string_lossy()));
    let _ = std::fs::remove_file(&tmp);
    #[cfg(unix)]
    return std::os::unix::fs::symlink(old_real, &tmp).and_then(|_| std::fs::rename(&tmp, link)).is_ok();
    #[cfg(not(unix))] // Windows installs are not links: nothing to point back
    false
}

impl Updates {
    /// History, modes and the seen models stay in memory until `with_dir`.
    pub fn new(fetch: Box<dyn Fetch>, adapters: Vec<Arc<dyn Adapter>>) -> Self {
        Self { fetch, adapters: Mutex::new(adapters), file: None, busy: Arc::new(|_| false), updating: Mutex::new(BTreeSet::new()), state: Mutex::new(State::default()) }
    }

    /// Keeps the state in `<dir>/updates.json`.
    pub fn with_dir(mut self, dir: PathBuf) -> Self {
        let file = dir.join("updates.json");
        *lk(&self.state) = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        self.file = Some(file);
        self
    }

    /// `busy(env)` = the env has a running launch; an update waits until it is false.
    pub fn with_busy(mut self, busy: Busy) -> Self {
        self.busy = busy;
        self
    }

    /// The app's adapters changed (own commands, CLI paths).
    pub fn set_adapters(&self, adapters: Vec<Arc<dyn Adapter>>) {
        *lk(&self.adapters) = adapters;
    }

    fn save(&self, st: &State) {
        let Some(f) = &self.file else { return };
        let _ = std::fs::create_dir_all(f.parent().unwrap_or(Path::new(".")));
        let tmp = f.with_extension("json.tmp");
        if serde_json::to_vec_pretty(st).ok().is_some_and(|b| std::fs::write(&tmp, b).is_ok()) {
            let _ = std::fs::rename(&tmp, f);
        }
    }

    fn adapter(&self, env: &str) -> Result<Arc<dyn Adapter>> {
        lk(&self.adapters).iter().find(|a| a.id() == env).cloned().ok_or_else(|| UpdateError::EnvNotFound(env.into()))
    }

    fn latest(&self, env: &str) -> Option<String> {
        let out = self.fetch.get(&format!("https://registry.npmjs.org/{}/latest", npm_package(env)?), NPM_MAX, None).ok()?;
        let v: Value = serde_json::from_slice(&out.body).ok().filter(|_| out.status == 200)?;
        v["version"].as_str().map(String::from)
    }

    pub fn check(&self, env: &str) -> Result<CliStatus> {
        let cli = self.adapter(env)?.cli_path().ok_or_else(|| UpdateError::NotInstalled(env.into()))?;
        let real = crate::proc::canon(&cli).unwrap_or_else(|_| cli.clone());
        let method = install_method(env, &real);
        let installed = version_of(&cli);
        if let Some(v) = &installed {
            crate::term::set_cli_version(env, v);
        }
        let latest = self.latest(env);
        let major = matches!((&installed, &latest), (Some(i), Some(l)) if first_num(l) > first_num(i));
        let cmd = update_cmd(env, method, if method == Method::Native { &cli } else { &real }).unwrap_or_default();
        Ok(CliStatus { installed, latest, method, cmd, major, path: cli.to_string_lossy().into_owned() })
    }

    /// Waits for the env's launches, runs the update, checks `--version` and `--help`, and rolls back to the
    /// previous version when they fail. Ok = the update ran (see `UpdateLog.ok`); Err = it was not started.
    pub fn update(&self, env: &str) -> Result<UpdateLog> {
        let st = self.check(env)?;
        if st.major {
            return Err(UpdateError::MajorJump { from: st.installed.unwrap_or_default(), to: st.latest.unwrap_or_default() });
        }
        if st.cmd.is_empty() {
            return Err(UpdateError::NoCommand(env.into()));
        }
        let start = Instant::now();
        while (self.busy)(env) {
            if start.elapsed() > WAIT_MAX {
                return Err(UpdateError::Busy(env.into()));
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        let cli = PathBuf::from(&st.path);
        let real_before = crate::proc::canon(&cli).unwrap_or_else(|_| cli.clone());
        // an update cut by the app's exit: the version from before it is the one to go back to
        let from = lk(&self.state).pending.get(env).cloned().unwrap_or(st.installed.clone());
        lk(&self.updating).insert(env.into());
        {
            let mut s = lk(&self.state);
            s.pending.insert(env.into(), from.clone());
            self.save(&s);
        }
        let ran = run(&st.cmd, RUN_MAX);
        let mut output = match &ran {
            Ok(o) => o.clone(),
            Err(e) => e.clone(),
        };
        let now = healthy(&cli);
        let mut rolled_back = false;
        if now.is_none() {
            if let Some(from) = &from {
                let real = if st.method == Method::Native { cli.clone() } else { real_before.clone() };
                if let Some(cmd) = rollback_cmd(env, st.method, &real, from) {
                    output.push_str(&format!("\n$ {}\n", cmd.join(" ")));
                    output.push_str(&run(&cmd, RUN_MAX).unwrap_or_else(|e| e));
                } else if st.method == Method::Native {
                    relink(&cli, &real_before);
                }
                rolled_back = healthy(&cli).as_deref() == Some(from.as_str());
            }
            if !rolled_back {
                output.push_str("\nrollback_unavailable");
            }
        }
        let after = version_of(&cli);
        if let Some(v) = &after {
            crate::term::set_cli_version(env, v);
        }
        let log = UpdateLog {
            env: env.into(),
            from,
            to: if now.is_some() { now.clone() } else { st.latest.clone() },
            ok: ran.is_ok() && now.is_some(),
            output: tail(&output),
            at: Local::now(),
            method: st.method,
            rolled_back,
        };
        lk(&self.updating).remove(env);
        let mut s = lk(&self.state);
        s.pending.remove(env);
        s.history.push(log.clone());
        let n = s.history.len();
        s.history.drain(..n.saturating_sub(HISTORY_MAX));
        self.save(&s);
        Ok(log)
    }

    /// Updates of this env, oldest first.
    pub fn history(&self, env: &str) -> Vec<UpdateLog> {
        lk(&self.state).history.iter().filter(|l| l.env == env).cloned().collect()
    }

    /// Envs whose update was cut by the app's exit: `update` them again at start.
    pub fn interrupted(&self) -> Vec<String> {
        lk(&self.state).pending.keys().cloned().collect()
    }

    /// An update of `env` is running now: new launches of the env should wait.
    pub fn updating(&self, env: &str) -> bool {
        lk(&self.updating).contains(env)
    }

    pub fn mode(&self, env: &str) -> Mode {
        lk(&self.state).modes.get(env).copied().unwrap_or_default()
    }

    pub fn set_mode(&self, env: &str, mode: Mode) {
        let mut s = lk(&self.state);
        s.modes.insert(env.into(), mode);
        self.save(&s);
    }

    /// models.dev with `If-None-Match`; None = unchanged (304) or not readable.
    fn models_dev(&self) -> Option<(Value, Option<String>)> {
        let etag = lk(&self.state).etag.clone();
        let out = self.fetch.get(MODELS_DEV, MODELS_DEV_MAX, etag.as_deref()).ok().filter(|o| o.status == 200)?;
        Some((serde_json::from_slice(&out.body).ok()?, out.etag))
    }

    /// New models since the last look, each once. models.dev for Claude, Gemini, Qwen (merged into their catalog
    /// snapshot); with `cli_lists`, the list commands of the other envs (their truth, story 84). The first look at
    /// an env only remembers what is there.
    pub fn models_radar(&self, store: &Mutex<Store>, cli_lists: bool) -> Vec<NewModel> {
        let api = self.models_dev();
        let mut news = vec![];
        let mut found: Vec<Found> = vec![];
        let adapters = lk(&self.adapters).clone();
        for a in adapters.iter().filter(|a| a.available().is_ok()) {
            let env = a.id().to_string();
            let before = |s: &Store| s.catalog(&env).map(|c| c.families).unwrap_or_else(|| a.builtin_models());
            if let Some(provider) = radar::provider_of(&env) {
                let Some((api, _)) = &api else { continue };
                let listed = radar::chat_models(api, provider);
                let prev = before(&lk(store));
                let mut fams = prev.clone();
                let builtin = a.builtin_models();
                let vendor = builtin.first().map(|f| f.vendor.clone()).unwrap_or_else(|| provider.to_string());
                for (id, name) in radar::ids(&builtin).into_iter().chain(listed.iter().cloned()) {
                    radar::place(&mut fams, &id, &name, &vendor, env != "claude");
                }
                let _ = lk(store).set_catalog(Catalog { env: env.clone(), families: fams.clone(), source: CatalogSource::Snapshot, at: Local::now() });
                found.push((env, prev, fams, listed.into_iter().collect()));
            } else if cli_lists {
                let Ok(fams) = a.models() else { continue };
                if fams.is_empty() {
                    continue;
                }
                let prev = before(&lk(store));
                let _ = lk(store).set_catalog(Catalog { env: env.clone(), families: fams.clone(), source: CatalogSource::CliList, at: Local::now() });
                found.push((env, prev, fams, BTreeMap::new()));
            }
        }
        let mut s = lk(&self.state);
        for (env, prev, fams, names) in found {
            let first = !s.seen.contains_key(&env);
            let seen = s.seen.entry(env.clone()).or_default();
            for (id, label) in radar::ids(&fams) {
                if seen.insert(id.clone()) && !first {
                    let auto = radar::moved_latest(&prev, &fams, &id);
                    news.push(NewModel { env: env.clone(), name: names.get(&id).cloned().unwrap_or(label), id, auto });
                }
            }
        }
        if let Some((_, etag)) = api {
            s.etag = etag;
        }
        self.save(&s);
        news
    }
}
