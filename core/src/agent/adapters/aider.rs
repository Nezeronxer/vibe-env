//! Aider: `aider --yes-always --no-pretty --no-stream [--model M] --message=<prompt>`.
//! Not installed here. From aider/website/docs/scripting.md and config/options.md: --message (one instruction, apply,
//! exit), --yes-always, --model, --no-pretty, --no-stream, --dry-run. Plain text output, no JSON mode and no sessions
//! (the chat history is a file in the folder, `--restore-chat-history`), so Done is the whole output at EOF and a run
//! is never resumed. Without a key it warns "No LLM model was specified and no API keys were provided."
//! (aider/onboarding.py). Read-only role: `--dry-run` ("Perform a dry run without modifying files").
//! unverified: everything above was read, not run; `--message=<prompt>` with `=`.
use super::common::*;
use crate::agent::{Adapter, AgentEvent, Run};
use std::process::Command;

pub struct Aider {
    bin: String,
}

impl Aider {
    pub fn new() -> Aider {
        Aider { bin: "aider".into() }
    }
    pub fn with_bin(bin: &str) -> Aider {
        Aider { bin: bin.into() }
    }
}

impl Default for Aider {
    fn default() -> Self {
        Self::new()
    }
}

const INSTALL: &str = "curl -LsSf https://aider.chat/install.sh | sh";

impl Adapter for Aider {
    fn id(&self) -> &str {
        "aider"
    }
    fn name(&self) -> &str {
        "Aider"
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
        c.current_dir(&run.cwd).args(["--yes-always", "--no-pretty", "--no-stream"]);
        if run.is_read_only() {
            c.arg("--dry-run");
        }
        if let Some(m) = cli_model(run) {
            c.args(["--model", m]);
        }
        c.envs(run.extra_env.iter().map(|(k, v)| (k, v)));
        c.args(&run.extra_args);
        c.arg(format!("--message={}", with_system(&run.system, &run.prompt)));
        c
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        text_line(line)
    }
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
        text_done(exit_ok, all_text)
    }
    fn read_only_native(&self) -> bool {
        true
    }
    fn install_hint(&self) -> Option<String> {
        Some(format!("{INSTALL} (не сверено)"))
    }
}
