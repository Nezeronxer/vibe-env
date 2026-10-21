pub mod adapters;
mod fake;
pub use fake::{FakeAdapter, FakeStep};

use crate::model::{LimitWindow, ModelFamily, RoleKind, Settings};
use chrono::{DateTime, Local};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// v1 name of the run's role; every `RoleKind` is a role now.
pub type Role = RoleKind;

#[derive(Clone, Debug)]
pub struct Run {
    pub cwd: PathBuf,
    pub prompt: String,
    pub system: String,
    pub resume: Option<String>,
    pub role: Role,
    /// Model flag value for the CLI (from `models::resolve`); None = no flag.
    pub model: Option<String>,
    /// Ask the CLI for its own read-only mode where it has one (`Adapter::read_only_native`).
    /// A Reviewer run is read-only whatever this says.
    pub read_only: bool,
    /// Added to the process environment (kit secrets); never printed or logged.
    pub extra_env: Vec<(String, String)>,
    /// Inline JSON schema of the answer, for CLIs that take one (Claude, Codex); others ignore it.
    pub json_schema: Option<String>,
    /// Remaining budget for this run; passed only to CLIs that report cost (Claude).
    pub max_budget_usd: Option<f64>,
    /// `EnvSettings.extra_args`: appended to the CLI as separate arguments, never through a shell.
    pub extra_args: Vec<String>,
}

impl Run {
    /// A run with every v2 option off.
    pub fn new(cwd: PathBuf, prompt: String, system: String, role: Role) -> Run {
        Run {
            cwd,
            prompt,
            system,
            resume: None,
            role,
            model: None,
            read_only: false,
            extra_env: vec![],
            json_schema: None,
            max_budget_usd: None,
            extra_args: vec![],
        }
    }

    /// Read-only run: asked for, or a Reviewer.
    pub fn is_read_only(&self) -> bool {
        self.read_only || self.role == RoleKind::Reviewer
    }
}

/// What the env does by itself for safety (stories 201, 219).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SandboxInfo {
    /// An OS sandbox is on for writing runs.
    pub sandboxed: bool,
    /// Push, remote and sudo are banned technically (not only in the prompt).
    pub push_blocked: bool,
    /// Commands can reach the network.
    pub network_open: bool,
    /// The agent can write outside the task's copy.
    pub writes_outside_copy: bool,
    /// Why something is closed or open, e.g. `Claude::network_note`.
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AgentEvent {
    Text(String),
    Tool { name: String, summary: String },
    Done { session: Option<String>, text: String, is_error: bool },
    Limit { resets_at: Option<DateTime<Local>> },
    /// Limit windows the env reported during the run (Claude: `rate_limit_event`).
    Usage(Vec<LimitWindow>),
    /// Cost of the run in USD, only from an env that reports it.
    Cost(f64),
    /// The CLI refused the model (text as the CLI said it).
    ModelRejected(String),
    /// The CLI is not logged in.
    AuthRequired(String),
}

/// Neutral SandboxInfo of an env with no protection of its own.
fn unprotected() -> SandboxInfo {
    SandboxInfo { sandboxed: false, push_blocked: false, network_open: true, writes_outside_copy: true, note: None }
}

pub trait Adapter: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    /// Err = install hint, e.g. "not installed: npm i -g ...".
    fn available(&self) -> Result<(), String>;
    /// Fully built command: cwd set, stdout piped.
    fn command(&self, run: &Run) -> Command;
    fn parse(&self, line: &str) -> Vec<AgentEvent>;
    /// For plain-text adapters: emit Done/Limit at EOF.
    fn finish(&self, _exit_ok: bool, _all_text: &str) -> Vec<AgentEvent> {
        vec![]
    }
    /// True only where the push/remote/sudo ban and a sandbox are enforced technically.
    /// The app warns about every adapter where this is false.
    fn guarded(&self) -> bool {
        false
    }
    /// The env's own model list (a CLI command). Err = no such command or it failed.
    fn models(&self) -> Result<Vec<ModelFamily>, String> {
        Err("no model list command".into())
    }
    /// Built-in hint list, strongest first; empty where nothing is verified.
    fn builtin_models(&self) -> Vec<ModelFamily> {
        vec![]
    }
    /// Interactive "continue in terminal" command (arguments as a list, no shell), cwd set.
    /// None = no template, or the session id is not of the expected format.
    fn interactive_resume(&self, _session: &str, _cwd: &Path) -> Option<Command> {
        None
    }
    /// The CLI has its own read-only flag (`Run.read_only` is passed to it). False = the engine
    /// runs such a role in a throwaway copy.
    fn read_only_native(&self) -> bool {
        false
    }
    /// The CLI reports the cost of a run (`AgentEvent::Cost`).
    fn reports_cost(&self) -> bool {
        false
    }
    /// Install command, not verified against the vendor.
    fn install_hint(&self) -> Option<String> {
        None
    }
    fn sandbox_info(&self) -> SandboxInfo {
        unprotected()
    }
    /// A folder the app owns for this adapter's helper files (Codex: the answer schemas). The
    /// engine passes it on `Engine::new` / `set_adapters`, derived from the data folder it was
    /// given, so an adapter never works out a location of its own. A folder set explicitly
    /// on the adapter (`Codex::with_schema_dir`) wins.
    fn set_private_dir(&self, _dir: &Path) {}
}

/// Six built-in engines (at `EnvSettings.cli_path` when set), then every custom command from the settings.
pub fn adapters(settings: &Settings) -> Vec<Arc<dyn Adapter>> {
    use adapters::*;
    let bin = |id: &str, default: &str| {
        settings
            .envs
            .get(id)
            .and_then(|e| e.cli_path.as_ref())
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| default.to_string())
    };
    let mut v: Vec<Arc<dyn Adapter>> = vec![
        Arc::new(Claude::with_bin(&bin("claude", "claude"))),
        Arc::new(Codex::with_bin(&bin("codex", "codex"))),
        Arc::new(Gemini::with_bin(&bin("gemini", "gemini"))),
        Arc::new(OpenCode::with_bin(&bin("opencode", "opencode"))),
        Arc::new(Cursor::with_bin(&bin("cursor", "cursor-agent"))),
        Arc::new(Qwen::with_bin(&bin("qwen", "qwen"))),
    ];
    // a custom command must not shadow a built-in engine (nor an earlier custom one)
    for c in settings.custom_agents.iter().cloned() {
        if !v.iter().any(|a| a.id() == c.id) {
            v.push(Arc::new(Custom(c)));
        }
    }
    v
}

/// Spawns `command()`, feeds each stdout line to `parse`, calls `finish` at EOF and kills the
/// child as soon as `cancel` is set. Every event goes to `on_event` and into the result.
pub fn run_blocking(
    a: &dyn Adapter,
    run: &Run,
    on_event: &mut dyn FnMut(&AgentEvent),
    cancel: &AtomicBool,
) -> std::io::Result<Vec<AgentEvent>> {
    let mut events = Vec::new();
    let mut all_text = String::new();
    let r = run_lines(a.command(run), cancel, &mut |line| {
        all_text.push_str(&line);
        all_text.push('\n');
        for e in a.parse(&line) {
            on_event(&e);
            events.push(e);
        }
    })?;
    if let Some((ok, err)) = r {
        if !ok && !err.is_empty() {
            all_text.push_str(&err);
        }
        for e in a.finish(ok, &all_text) {
            on_event(&e);
            events.push(e);
        }
    }
    Ok(events)
}

/// Runs `cmd` in its own process group and passes each stdout line to `on_line`.
/// `cancel` kills the whole group (-> `Ok(None)`); when the main process exits, whatever it
/// left in its group is killed too, so leftovers cannot hold the pipes open.
/// Returns (exit ok, stderr). Shared by agents and the engine's check / setup commands.
pub fn run_lines(
    mut cmd: Command,
    cancel: &AtomicBool,
    on_line: &mut dyn FnMut(String),
) -> std::io::Result<Option<(bool, String)>> {
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0); // own group so cancel reaches grandchildren
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    let mut child = Group(cmd.spawn()?);
    let stdout = child.0.stdout.take().expect("stdout piped");
    let mut stderr = child.0.stderr.take().expect("stderr piped");

    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        // lossy: one non-UTF-8 line must not end the stream and lose the rest
        let mut r = BufReader::new(stdout);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            if !matches!(r.read_until(b'\n', &mut buf), Ok(n) if n > 0) {
                break;
            }
            if buf.ends_with(b"\n") {
                buf.pop();
                if buf.ends_with(b"\r") {
                    buf.pop();
                }
            }
            if tx.send(String::from_utf8_lossy(&buf).into_owned()).is_err() {
                break;
            }
        }
    });
    let (err_tx, err_rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        let _ = err_tx.send(s);
    });

    let mut status = None;
    let mut drain_until: Option<Instant> = None;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Ok(None); // `Group` kills the group on the way out
        }
        if status.is_none() {
            if let Some(s) = child.0.try_wait()? {
                // the main process is gone: whatever it left behind in its group must not hold the pipes
                kill_group(child.0.id());
                status = Some(s);
                drain_until = Some(Instant::now() + Duration::from_secs(2));
            }
        }
        if drain_until.is_some_and(|t| Instant::now() > t) {
            break; // something escaped the group and still holds stdout; stop waiting for EOF
        }
        match rx.recv_timeout(Duration::from_millis(30)) {
            Ok(line) => on_line(line),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    // stdout closed before the process exited: wait for it, still honouring cancel
    let status = match status {
        Some(s) => s,
        None => loop {
            if cancel.load(Ordering::SeqCst) {
                return Ok(None);
            }
            if let Some(s) = child.0.try_wait()? {
                break s;
            }
            std::thread::sleep(Duration::from_millis(30));
        },
    };
    let err = err_rx.recv_timeout(Duration::from_secs(2)).unwrap_or_default();
    Ok(Some((status.success(), err)))
}

/// The child and its process group; on every way out of `run_lines` (return, `?`, a panic in
/// `on_line`) the group is killed and the child reaped.
struct Group(std::process::Child);
impl Drop for Group {
    fn drop(&mut self) {
        kill_group(self.0.id());
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// SIGKILL the whole process group of a child started with `process_group(0)`.
fn kill_group(pid: u32) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill").args(["-KILL", "--", &format!("-{pid}")]).output();
    }
    #[cfg(not(unix))]
    let _ = pid;
}
