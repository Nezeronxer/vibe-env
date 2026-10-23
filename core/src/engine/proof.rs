//! F15 (#3) "Проверить как пользователь": after green gates the project's `run_cmd` starts in the task's copy, the
//! page is shot (desktop, after the main button, phone width) and the files go onto the task. Never fails the task:
//! whatever stops it is a note (`Proof.skipped`).
use super::{lk, Engine};
use crate::model::*;
use crate::preview::{self, Capture};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long the started app may take to answer on its port.
const READY: Duration = Duration::from_secs(60);
/// The most shots kept on a task.
const MAX_SHOTS: usize = 5;

/// The started app; its whole process group goes when this is dropped.
struct App(Child);
impl Drop for App {
    fn drop(&mut self) {
        crate::agent::kill_group(self.0.id());
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn answers(host: &str, port: u16) -> bool {
    (host, port).to_socket_addrs().map(|a| a.into_iter().any(|a| TcpStream::connect_timeout(&a, Duration::from_millis(300)).is_ok())).unwrap_or(false)
}

impl Engine {
    /// Binds the screenshot tool and the folder the shots go to (`<dir>/<task id>/`).
    pub fn set_proof(&self, capture: Arc<dyn Capture>, dir: PathBuf) {
        *lk(&self.proof) = Some((capture, dir));
    }

    /// None = cancelled while it ran.
    pub(super) fn proof_step(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Option<()> {
        if !p.proof {
            t.outcome.proof = None;
            return Some(());
        }
        self.stage(t, "proof");
        let r = self.shoot(t, p, cancel);
        if cancel.load(Ordering::SeqCst) {
            return None;
        }
        t.outcome.proof = Some(match r {
            Ok(shots) => {
                self.log(&t.id, LogKind::Check, &format!("proof: {} shots", shots.len()));
                Proof { shots, skipped: None }
            }
            Err(why) => {
                self.log(&t.id, LogKind::Check, &format!("proof skipped: {why}"));
                Proof { shots: vec![], skipped: Some(why) }
            }
        });
        Some(())
    }

    fn shoot(&self, t: &Task, p: &Project, cancel: &AtomicBool) -> Result<Vec<String>, String> {
        let (capture, dir) = lk(&self.proof).clone().ok_or(preview::NO_TOOL)?;
        let run = p.run_cmd.clone().filter(|c| !c.trim().is_empty()).ok_or("no_run_cmd")?;
        let url = p.app_url.clone().filter(|u| !u.trim().is_empty()).or_else(|| t.port.map(|n| format!("http://localhost:{n}/"))).ok_or("no_app_url")?;
        let url = preview::local_url(&url)?;
        let wt = t.worktree.clone().ok_or("no_worktree")?;
        capture.available(&wt)?;
        let (host, port) = preview::host_port(&url);
        // something already on the port would be shot instead of the task's copy (the person's own dev server)
        if answers(&host, port) {
            return Err(format!("port_busy: {port}"));
        }
        let mut c = Command::new("sh");
        c.arg("-c").arg(&run).current_dir(&wt).envs(Self::copy_env(t)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut c, 0);
        self.log(&t.id, LogKind::Check, &format!("$ {run}"));
        let mut app = App(c.spawn().map_err(|_| "app_not_started")?);
        let start = Instant::now();
        while !answers(&host, port) {
            if cancel.load(Ordering::SeqCst) {
                return Err("cancelled".into());
            }
            if app.0.try_wait().ok().flatten().is_some() || start.elapsed() > READY {
                return Err("app_not_started".into());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        let out = dir.join(&t.id);
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).map_err(|e| format!("capture_failed: {e}"))?;
        let files = capture.shoot(&url, &wt, &out)?;
        drop(app);
        let shots: Vec<String> = files
            .iter()
            .filter(|f| f.parent() == Some(out.as_path()) && f.is_file())
            .filter_map(|f| f.file_name().map(|n| n.to_string_lossy().into_owned()))
            .take(MAX_SHOTS)
            .collect();
        if shots.is_empty() {
            return Err("capture_failed: no shots".into());
        }
        Ok(shots)
    }
}
