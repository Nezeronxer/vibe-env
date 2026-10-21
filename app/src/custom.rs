//! "Проверить" an own command (G02.1): runs it with a test prompt in an empty temporary folder and shows what it said.
use crate::hub::{Hub, Res};
use nightshift_core::agent::adapters::Custom;
use nightshift_core::agent::{Adapter, Run};
use nightshift_core::model::RoleKind;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::Stdio;
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq)]
pub struct CheckRun {
    /// stdout and stderr, cut to the last 8000 characters.
    pub output: String,
    /// None = it did not end by itself (killed after the time).
    pub code: Option<i32>,
    pub timed_out: bool,
}

const WAIT: Duration = Duration::from_secs(30);
const PROMPT: &str = "Reply with the single word: ok";

fn drain(mut r: impl Read + Send + 'static) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut b = vec![];
        let _ = r.read_to_end(&mut b);
        String::from_utf8_lossy(&b).into_owned()
    })
}

impl Hub {
    pub fn check_custom(&self, id: &str) -> Res<CheckRun> {
        self.check_custom_within(id, WAIT)
    }

    pub fn check_custom_within(&self, id: &str, wait: Duration) -> Res<CheckRun> {
        let agent = self.settings().custom_agents.into_iter().find(|c| c.id == id).ok_or_else(|| format!("agent_not_found: {id}"))?;
        if agent.run.trim().is_empty() {
            return Err("custom_not_configured".into());
        }
        let dir = std::env::temp_dir().join(format!("nightshift-check-{}-{}", std::process::id(), nanos()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let out = (|| {
            let mut c = Custom(agent).command(&Run::new(dir.clone(), PROMPT.into(), String::new(), RoleKind::Worker));
            c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).process_group(0);
            let mut child = c.spawn().map_err(|e| format!("run_failed: {e}"))?;
            let (o, e) = (drain(child.stdout.take().unwrap()), drain(child.stderr.take().unwrap()));
            let t0 = Instant::now();
            let (code, timed_out) = loop {
                if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
                    break (st.code(), false);
                }
                if t0.elapsed() >= wait {
                    // the whole group: the command may have started children
                    let _ = std::process::Command::new("kill").args(["-KILL", &format!("-{}", child.id())]).status();
                    let _ = child.kill();
                    let _ = child.wait();
                    break (None, true);
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            let text = format!("{}{}", o.join().unwrap_or_default(), e.join().unwrap_or_default());
            let skip = text.chars().count().saturating_sub(8000);
            Ok(CheckRun { output: text.chars().skip(skip).collect(), code, timed_out })
        })();
        let _ = std::fs::remove_dir_all(&dir);
        out
    }
}

fn nanos() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use crate::testkit::*;
    use nightshift_core::agent::FakeAdapter;
    use nightshift_core::model::CustomAgent;
    use std::time::Duration;

    fn rig_with(run: &str) -> Rig {
        Rig::with(FakeAdapter::new(vec![]), |s| s.custom_agents = vec![CustomAgent { id: "custom:t".into(), name: "T".into(), run: run.into(), resume: None }])
    }

    #[test]
    fn the_command_runs_with_the_test_prompt_in_an_empty_temporary_folder_and_its_output_and_code_come_back() {
        let r = rig_with("echo \"said: {prompt}\"; echo \"cwd=$(pwd -P)\"; ls -A {cwd} | wc -l; echo oops >&2; exit 3");
        let c = r.hub.check_custom("custom:t").unwrap();
        assert_eq!(c.code, Some(3));
        assert!(!c.timed_out);
        assert!(c.output.contains("said: Reply with the single word: ok"), "{}", c.output);
        assert!(c.output.lines().any(|l| l.trim() == "0"), "the folder was empty: {}", c.output);
        assert!(c.output.contains("oops"), "stderr is shown too");
        let cwd = c.output.lines().find_map(|l| l.strip_prefix("cwd=")).unwrap();
        assert!(!std::path::Path::new(cwd).exists(), "the folder is removed: {cwd}");
        assert_eq!(r.hub.check_custom("custom:none").unwrap_err(), "agent_not_found: custom:none");
    }

    #[test]
    fn a_command_that_does_not_end_is_stopped_with_its_children() {
        let r = rig_with("sleep 30 & sleep 30");
        let t0 = std::time::Instant::now();
        let c = r.hub.check_custom_within("custom:t", Duration::from_millis(300)).unwrap();
        assert!(c.timed_out && c.code.is_none());
        assert!(t0.elapsed() < Duration::from_secs(5));
    }
}
