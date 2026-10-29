//! Amp (Sourcegraph / ampcode.com): `amp [threads continue <id>] --execute=<prompt> --stream-json`.
//! Not installed here. From ampcode.com/docs/markdown/cli/execute-mode and .../cli/streaming-json: -x/--execute runs one
//! turn and exits; --stream-json (needs --execute) prints Claude Code-compatible lines (system init with session_id,
//! assistant / user tool_result, final `result` with is_error, result, session_id); `amp threads continue <T-id>`
//! continues a thread. Amp does not ask before running tools (docs/markdown/tools), has no read-only mode and no
//! model flag (the mode is picked on "the dial"), so `Run.model` is not passed.
//! unverified: `--execute=<prompt>` with `=`, the order `threads continue <id> --execute`.
use super::claude::parse_line;
use super::common::*;
use crate::agent::{Adapter, AgentEvent, Run};
use std::process::Command;

pub struct Amp {
    bin: String,
}

impl Amp {
    pub fn new() -> Amp {
        Amp { bin: "amp".into() }
    }
    pub fn with_bin(bin: &str) -> Amp {
        Amp { bin: bin.into() }
    }
}

impl Default for Amp {
    fn default() -> Self {
        Self::new()
    }
}

const INSTALL: &str = "curl -fsSL https://ampcode.com/install.sh | bash";

impl Adapter for Amp {
    fn id(&self) -> &str {
        "amp"
    }
    fn name(&self) -> &str {
        "Amp"
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
        let mut c = crate::proc::cmd(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(&run.cwd);
        if let Some(s) = &run.resume {
            c.args(["threads", "continue", s]);
        }
        c.envs(run.extra_env.iter().map(|(k, v)| (k, v)));
        c.args(&run.extra_args);
        c.arg(format!("--execute={}", with_system(&run.system, &run.prompt))).arg("--stream-json");
        c
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        parse_line(line)
    }
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
        finish_without_result(exit_ok, all_text)
    }
    fn install_hint(&self) -> Option<String> {
        Some(format!("{INSTALL} (не сверено)"))
    }
    /// `amp threads continue <T-id>` (docs/markdown/threads, "Continue a Thread Anywhere"; not on this machine).
    fn resume_argv(&self, session: &str, _cwd: &std::path::Path) -> Option<Vec<String>> {
        crate::agent::resume_words(&self.bin, &["threads", "continue"], session)
    }
    fn interactive_resume(&self, session: &str, cwd: &std::path::Path) -> Option<Command> {
        let mut c = crate::proc::cmd(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(cwd).args(["threads", "continue", super::claude::valid_session(session)?]);
        Some(c)
    }
}
