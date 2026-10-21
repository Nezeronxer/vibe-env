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
use std::time::Duration;

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
}

/// Real adapters are added by ticket 02.
pub fn adapters(_settings: &Settings) -> Vec<Arc<dyn Adapter>> {
    vec![]
}

/// Spawns `command()`, feeds each stdout line to `parse`, calls `finish` at EOF and kills the
/// child as soon as `cancel` is set. Every event goes to `on_event` and into the result.
pub fn run_blocking(
    a: &dyn Adapter,
    run: &Run,
    on_event: &mut dyn FnMut(&AgentEvent),
    cancel: &AtomicBool,
) -> std::io::Result<Vec<AgentEvent>> {
    let mut cmd = a.command(run);
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0); // own group so cancel reaches grandchildren
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().expect("stdout piped");
    let mut stderr = child.stderr.take().expect("stderr piped");

    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let err_thread = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });

    let mut events = Vec::new();
    let mut all_text = String::new();
    let emit = |evs: Vec<AgentEvent>, events: &mut Vec<AgentEvent>, on: &mut dyn FnMut(&AgentEvent)| {
        for e in evs {
            on(&e);
            events.push(e);
        }
    };
    let mut cancelled = false;
    loop {
        if cancel.load(Ordering::SeqCst) {
            kill_group(child.id());
            let _ = child.kill();
            let _ = child.wait();
            cancelled = true;
            break;
        }
        match rx.recv_timeout(Duration::from_millis(30)) {
            Ok(line) => {
                all_text.push_str(&line);
                all_text.push('\n');
                emit(a.parse(&line), &mut events, on_event);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    if !cancelled {
        let status = child.wait()?;
        let err = err_thread.join().unwrap_or_default();
        if !status.success() && !err.is_empty() {
            all_text.push_str(&err);
        }
        emit(a.finish(status.success(), &all_text), &mut events, on_event);
    }
    Ok(events)
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
