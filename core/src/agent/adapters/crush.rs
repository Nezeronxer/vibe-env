//! Crush (Charm): `crush run --quiet [-m <model>] [-s <id>] -- <prompt>`.
//! Not installed here. From charmbracelet/crush internal/cmd/run.go: `run [prompt...]`, -q/--quiet, -m/--model
//! ("model" or "provider/model"), -s/--session <id>, -C/--continue; root.go: -s/--session and -C/--continue for the
//! TUI too, `--yolo` only on the root command (not on `run`). `crush run` prints plain text: no JSON mode, no session
//! id in the output, so Done is the whole output at EOF. Without a provider it fails with
//! "no providers configured - please run 'crush' to set up a provider interactively" (run.go).
//! unverified: how `run` handles tool permissions (no flag for it), `--` before the prompt.
use super::common::*;
use crate::agent::{Adapter, AgentEvent, Run};
use std::process::Command;

pub struct Crush {
    bin: String,
}

impl Crush {
    pub fn new() -> Crush {
        Crush { bin: "crush".into() }
    }
    pub fn with_bin(bin: &str) -> Crush {
        Crush { bin: bin.into() }
    }
}

impl Default for Crush {
    fn default() -> Self {
        Self::new()
    }
}

const INSTALL: &str = "npm install -g @charmland/crush";

impl Adapter for Crush {
    fn id(&self) -> &str {
        "crush"
    }
    fn name(&self) -> &str {
        "Crush"
    }
    fn available(&self) -> Result<(), String> {
        check_installed(&self.bin, INSTALL)
    }
    fn cli_path(&self) -> Option<std::path::PathBuf> {
        find_bin(&self.bin)
    }
    fn version(&self) -> Option<String> {
        cli_version(&self.bin)
    }
    fn command(&self, run: &Run) -> Command {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        // no read-only flag: the engine runs readers in a throwaway copy
        c.current_dir(&run.cwd).args(["run", "--quiet"]);
        if let Some(m) = cli_model(run) {
            c.args(["-m", m]);
        }
        c.envs(run.extra_env.iter().map(|(k, v)| (k, v)));
        c.args(&run.extra_args);
        if let Some(s) = &run.resume {
            c.args(["--session", s]);
        }
        c.arg("--").arg(with_system(&run.system, &run.prompt));
        c
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        text_line(line)
    }
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
        text_done(exit_ok, all_text)
    }
    fn install_hint(&self) -> Option<String> {
        Some(format!("{INSTALL} (не сверено)"))
    }
    /// `crush --session <id>` (root.go: "-s, --session Continue a previous session by ID"; not on this machine).
    fn resume_argv(&self, session: &str, _cwd: &std::path::Path) -> Option<Vec<String>> {
        crate::agent::resume_words(&self.bin, &["--session"], session)
    }
    fn interactive_resume(&self, session: &str, cwd: &std::path::Path) -> Option<Command> {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(cwd).args(["--session", super::claude::valid_session(session)?]);
        Some(c)
    }
}
