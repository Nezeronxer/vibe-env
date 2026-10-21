pub mod adapters;
mod fake;
pub use fake::{FakeAdapter, FakeStep};

use crate::model::Settings;
use chrono::{DateTime, Local};
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
pub enum Role {
    Worker,
    Reviewer,
}

#[derive(Clone, Debug)]
pub struct Run {
    pub cwd: PathBuf,
    pub prompt: String,
    pub system: String,
    pub resume: Option<String>,
    pub role: Role,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AgentEvent {
    Text(String),
    Tool { name: String, summary: String },
    Done { session: Option<String>, text: String, is_error: bool },
    Limit { resets_at: Option<DateTime<Local>> },
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
}

/// Six built-in engines, then every custom command from the settings.
pub fn adapters(settings: &Settings) -> Vec<Arc<dyn Adapter>> {
    use adapters::*;
    let mut v: Vec<Arc<dyn Adapter>> = vec![
        Arc::new(Claude::new()),
        Arc::new(Codex::new()),
        Arc::new(Gemini::new()),
        Arc::new(OpenCode::new()),
        Arc::new(Cursor::new()),
        Arc::new(Qwen::new()),
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
