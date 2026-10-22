//! PTY sessions that live in the core, not in a window (story 301): a shell or an env's CLI, the last 2 MB of its
//! output, one window attached at a time, flow control by acks.
use serde::{Deserialize, Serialize};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub type TermId = String;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TermError {
    #[error("term_not_found: {0}")]
    NotFound(String),
    #[error("bad_cwd: {0}")]
    BadCwd(String),
    #[error("pty_failed: {0}")]
    Pty(String),
}

pub type Result<T> = std::result::Result<T, TermError>;

/// What to start. `argv` empty = the person's shell (`$SHELL -l`); `cwd` None = the home folder.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TermSpec {
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default)]
    pub env: Vec<(String, String)>,
    pub cols: u16,
    pub rows: u16,
    #[serde(default)]
    pub title: String,
}

/// What an attached window gets: raw bytes, then once the exit code (None: killed by a signal).
#[derive(Clone, Debug, PartialEq)]
pub enum TermChunk {
    Data(Vec<u8>),
    Exit(Option<i32>),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TermInfo {
    pub id: TermId,
    pub title: String,
    pub cwd: String,
    pub pid: Option<u32>,
    pub running: bool,
    pub exit: Option<i32>,
    pub env_id: Option<String>,
    pub cli_version: Option<String>,
}

/// A terminal as it is restored after a restart: folder and text, no process (story 253).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TermSaved {
    pub title: String,
    pub cwd: String,
    pub text: String,
}

pub type Sink = Box<dyn Fn(TermChunk) + Send>;

/// Output kept per session for a window that attaches later and for the snapshot.
const RING: usize = 2 << 20;
/// Reading stops while this much went to the window without an ack.
const PAUSE_AT: usize = 1 << 20;
/// Pieces read within this time (or up to `MAX_PIECE`) go out as one.
const GLUE: Duration = Duration::from_millis(4);
const MAX_PIECE: usize = 64 << 10;

struct Shared {
    ring: VecDeque<u8>,
    sink: Option<Sink>,
    in_flight: usize,
    exit: Option<Option<i32>>,
    closed: bool,
}

struct Session {
    id: TermId,
    title: String,
    cwd: PathBuf,
    pid: Option<u32>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    shared: Mutex<Shared>,
    wake: Condvar,
}

fn lk<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn pty_err(e: impl std::fmt::Display) -> TermError {
    TermError::Pty(e.to_string())
}

/// Names that look like a secret (the kits' own signs): such a variable never goes from the app into a terminal (story 254).
fn secret_name(name: &str) -> bool {
    let n = name.to_ascii_uppercase();
    ["_KEY", "_TOKEN", "_SECRET", "_PASSWORD"].iter().any(|x| n.ends_with(x))
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize { rows: rows.max(1), cols: cols.max(1), pixel_width: 0, pixel_height: 0 }
}

#[derive(Default)]
pub struct Terms {
    sessions: Mutex<Vec<Arc<Session>>>,
}

impl Terms {
    pub fn new() -> Self {
        Self::default()
    }

    fn get(&self, id: &str) -> Result<Arc<Session>> {
        lk(&self.sessions).iter().find(|s| s.id == id).cloned().ok_or_else(|| TermError::NotFound(id.into()))
    }

    pub fn open(&self, spec: TermSpec) -> Result<TermId> {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"));
        let cwd = spec.cwd.clone().unwrap_or(home);
        if !cwd.is_dir() {
            return Err(TermError::BadCwd(cwd.display().to_string()));
        }
        let argv = if spec.argv.is_empty() { vec![std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()), "-l".into()] } else { spec.argv.clone() };
        let mut cmd = CommandBuilder::new(&argv[0]);
        cmd.args(&argv[1..]);
        cmd.cwd(&cwd);
        // the app's own variables stay in the app; the person's environment goes in as it is
        for (k, _) in std::env::vars_os() {
            if k.to_string_lossy().starts_with("NIGHTSHIFT_") {
                cmd.env_remove(&k);
            }
        }
        for (k, v) in &spec.env {
            if !secret_name(k) && crate::kits::env_value_ok(k, v) {
                cmd.env(k, v);
            }
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "Nightshift");
        if std::env::var_os("LANG").is_none() && std::env::var_os("LC_ALL").is_none() {
            cmd.env("LANG", "en_US.UTF-8");
        }
        let pair = native_pty_system().openpty(size(spec.cols, spec.rows)).map_err(pty_err)?;
        let mut child = pair.slave.spawn_command(cmd).map_err(pty_err)?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().map_err(pty_err)?;
        let writer = pair.master.take_writer().map_err(pty_err)?;
        let title = if spec.title.is_empty() { argv[0].rsplit('/').next().unwrap_or("").to_string() } else { spec.title };
        let s = Arc::new(Session {
            id: uuid::Uuid::new_v4().simple().to_string()[..12].to_string(),
            title,
            cwd,
            pid: child.process_id(),
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            killer: Mutex::new(child.clone_killer()),
            shared: Mutex::new(Shared { ring: VecDeque::new(), sink: None, in_flight: 0, exit: None, closed: false }),
            wake: Condvar::new(),
        });
        // reader: blocks on the pty, waits while the window is more than PAUSE_AT behind (then the OS stops the process)
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(4);
        let r = s.clone();
        std::thread::spawn(move || {
            let mut buf = vec![0u8; MAX_PIECE];
            loop {
                {
                    let mut g = lk(&r.shared);
                    while g.sink.is_some() && g.in_flight > PAUSE_AT && !g.closed {
                        g = r.wake.wait(g).unwrap_or_else(|e| e.into_inner());
                    }
                }
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let mut g = lk(&r.shared);
                        if g.sink.is_some() {
                            g.in_flight += n;
                        }
                        drop(g);
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        // pump: glues the pieces of ~4 ms, keeps them in the ring, hands them to the window; at the end, the exit code
        let p = s.clone();
        std::thread::spawn(move || {
            while let Ok(mut data) = rx.recv() {
                let until = Instant::now() + GLUE;
                while data.len() < MAX_PIECE {
                    match rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
                        Ok(more) => data.extend(more),
                        Err(_) => break,
                    }
                }
                let mut g = lk(&p.shared);
                g.ring.extend(&data);
                let over = g.ring.len().saturating_sub(RING);
                g.ring.drain(..over);
                if let Some(sink) = &g.sink {
                    sink(TermChunk::Data(data));
                }
            }
            let code = child.wait().ok().and_then(|st| if st.signal().is_some() { None } else { Some(st.exit_code() as i32) });
            let mut g = lk(&p.shared);
            g.exit = Some(code);
            // called without the lock: the sink may read `list()` (the app's tray does)
            let sink = g.sink.take();
            drop(g);
            if let Some(sink) = sink {
                sink(TermChunk::Exit(code));
                let mut g = lk(&p.shared);
                if g.sink.is_none() {
                    g.sink = Some(sink);
                }
            }
        });
        let id = s.id.clone();
        lk(&self.sessions).push(s);
        Ok(id)
    }

    pub fn write(&self, id: &str, data: &[u8]) -> Result<()> {
        let s = self.get(id)?;
        let mut w = lk(&s.writer);
        w.write_all(data).and_then(|_| w.flush()).map_err(pty_err)
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<()> {
        let s = self.get(id)?;
        let m = lk(&s.master);
        m.resize(size(cols, rows)).map_err(pty_err)
    }

    /// Ends the process and forgets the session.
    pub fn close(&self, id: &str) -> Result<()> {
        let s = self.get(id)?;
        lk(&self.sessions).retain(|x| x.id != id);
        lk(&s.shared).closed = true;
        s.wake.notify_all();
        let _ = lk(&s.killer).kill();
        Ok(())
    }

    /// The window that attaches gets the buffer now and every new piece through `sink`. `Data` comes under the
    /// session's lock (it must not call back into `Terms`); `Exit` comes without it. A window attached before is replaced.
    pub fn attach(&self, id: &str, sink: Sink) -> Result<Vec<u8>> {
        let s = self.get(id)?;
        let mut g = lk(&s.shared);
        let buf = g.ring.iter().copied().collect();
        if let Some(code) = g.exit {
            // the process is over, nothing else comes: the exit goes out without the lock
            drop(g);
            sink(TermChunk::Exit(code));
            g = lk(&s.shared);
        }
        g.sink = Some(sink);
        g.in_flight = 0;
        drop(g);
        s.wake.notify_all();
        Ok(buf)
    }

    pub fn detach(&self, id: &str) -> Result<()> {
        let s = self.get(id)?;
        let mut g = lk(&s.shared);
        g.sink = None;
        g.in_flight = 0;
        drop(g);
        s.wake.notify_all();
        Ok(())
    }

    /// The window has written `bytes` more.
    pub fn ack(&self, id: &str, bytes: usize) -> Result<()> {
        let s = self.get(id)?;
        let mut g = lk(&s.shared);
        g.in_flight = g.in_flight.saturating_sub(bytes);
        drop(g);
        s.wake.notify_all();
        Ok(())
    }

    pub fn list(&self) -> Vec<TermInfo> {
        lk(&self.sessions)
            .iter()
            .map(|s| {
                let exit = lk(&s.shared).exit;
                TermInfo { id: s.id.clone(), title: s.title.clone(), cwd: s.cwd.display().to_string(), pid: s.pid, running: exit.is_none(), exit: exit.flatten(), env_id: None, cli_version: None }
            })
            .collect()
    }

    pub fn snapshot(&self) -> Vec<TermSaved> {
        lk(&self.sessions)
            .iter()
            .map(|s| {
                let g = lk(&s.shared);
                let (a, b) = g.ring.as_slices();
                TermSaved { title: s.title.clone(), cwd: s.cwd.display().to_string(), text: String::from_utf8_lossy(&[a, b].concat()).into_owned() }
            })
            .collect()
    }
}

/// The processes end with the app (story 303); their text stays in the snapshot the app saved.
impl Drop for Terms {
    fn drop(&mut self) {
        for s in lk(&self.sessions).iter() {
            let _ = lk(&s.killer).kill();
        }
    }
}
