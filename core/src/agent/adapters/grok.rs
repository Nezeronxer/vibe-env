//! Grok Build (xAI, github.com/xai-org/grok-build): `grok --output-format streaming-messages-json --yolo [-m M] [-r <id>] --single=<prompt>`.
//! Not installed here. From crates/codegen/xai-grok-pager/docs/user-guide/14-headless-mode.md: -p/--single runs one
//! prompt headless, -m/--model, -r/--resume <id>, -c/--continue, --yolo (auto-approve), --permission-mode
//! (`dontAsk` = only pre-approved tools and the built-in read-only ones, 22-permissions-and-safety.md);
//! `streaming-messages-json` is the Claude Code stream-json shape (system init, assistant, user tool_result, result
//! with session_id). Failure: "Authentication failed" -> `grok login` (02-authentication.md).
//! unverified: everything above was read, not run; `--single=<prompt>` with `=`.
use super::claude::parse_line;
use super::common::*;
use crate::agent::{Adapter, AgentEvent, Run};
use std::process::Command;

pub struct Grok {
    bin: String,
}

impl Grok {
    pub fn new() -> Grok {
        Grok { bin: "grok".into() }
    }
    pub fn with_bin(bin: &str) -> Grok {
        Grok { bin: bin.into() }
    }
}

impl Default for Grok {
    fn default() -> Self {
        Self::new()
    }
}

const INSTALL: &str = "curl -fsSL https://x.ai/cli/install.sh | bash";

impl Adapter for Grok {
    fn id(&self) -> &str {
        "grok"
    }
    fn name(&self) -> &str {
        "Grok Build"
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
        c.current_dir(&run.cwd).args(["--output-format", "streaming-messages-json"]);
        if run.is_read_only() {
            c.args(["--permission-mode", "dontAsk"]);
        } else {
            c.arg("--yolo");
        }
        if let Some(m) = cli_model(run) {
            c.args(["-m", m]);
        }
        c.envs(run.extra_env.iter().map(|(k, v)| (k, v)));
        c.args(&run.extra_args);
        if let Some(s) = &run.resume {
            c.args(["--resume", s]);
        }
        c.arg(format!("--single={}", with_system(&run.system, &run.prompt)));
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
    /// `grok --resume <id>` (14-headless-mode.md, 17-sessions.md; not on this machine).
    fn resume_argv(&self, session: &str, _cwd: &std::path::Path) -> Option<Vec<String>> {
        crate::agent::resume_words(&self.bin, &["--resume"], session)
    }
    fn interactive_resume(&self, session: &str, cwd: &std::path::Path) -> Option<Command> {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(cwd).args(["--resume", super::claude::valid_session(session)?]);
        Some(c)
    }
}
