//! Factory Droid: `droid exec -o json [--auto medium] [-m <model>] [-s <id>] -- <prompt>`.
//! Not installed here. From docs.factory.com/droid-cli/cli-reference.md and droid-exec/overview.md: `droid exec` is
//! read-only without `--auto` (low|medium|high; medium = edits, installs, builds, local commits; high adds git push, so
//! it is never used), -m/--model, -s/--session-id <id> continues a session, -o json prints one line
//! `{"type":"result","is_error","result","session_id","usage"}` at the end (the Claude Code result shape). The
//! documented stream-json events (system, message, tool_call, tool_result, completion) have no field list, so the
//! one-line json is used: no live actions, only the answer.
//! unverified: `--` before the prompt.
use super::claude::parse_line;
use super::common::*;
use crate::agent::{Adapter, AgentEvent, Run};
use std::process::Command;

pub struct Droid {
    bin: String,
}

impl Droid {
    pub fn new() -> Droid {
        Droid { bin: "droid".into() }
    }
    pub fn with_bin(bin: &str) -> Droid {
        Droid { bin: bin.into() }
    }
}

impl Default for Droid {
    fn default() -> Self {
        Self::new()
    }
}

const INSTALL: &str = "curl -fsSL https://app.factory.ai/cli | sh";

impl Adapter for Droid {
    fn id(&self) -> &str {
        "droid"
    }
    fn name(&self) -> &str {
        "Factory Droid"
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
        c.current_dir(&run.cwd).args(["exec", "-o", "json"]);
        // read-only is the default of `droid exec`
        if !run.is_read_only() {
            c.args(["--auto", "medium"]);
        }
        if let Some(m) = cli_model(run) {
            c.args(["-m", m]);
        }
        c.envs(run.extra_env.iter().map(|(k, v)| (k, v)));
        c.args(&run.extra_args);
        if let Some(s) = &run.resume {
            c.args(["--session-id", s]);
        }
        c.arg("--").arg(with_system(&run.system, &run.prompt));
        c
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        parse_line(line)
    }
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
        finish_without_result(exit_ok, all_text)
    }
    fn read_only_native(&self) -> bool {
        true
    }
    fn install_hint(&self) -> Option<String> {
        Some(format!("{INSTALL} (не сверено)"))
    }
    /// `droid --resume <id>` (cli-reference: "droid --resume [sessionId]"; not on this machine).
    fn resume_argv(&self, session: &str, _cwd: &std::path::Path) -> Option<Vec<String>> {
        crate::agent::resume_words(&self.bin, &["--resume"], session)
    }
    fn interactive_resume(&self, session: &str, cwd: &std::path::Path) -> Option<Command> {
        let mut c = crate::proc::cmd(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(cwd).args(["--resume", super::claude::valid_session(session)?]);
        Some(c)
    }
}
