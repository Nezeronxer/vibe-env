//! Qwen Code (fork of Gemini CLI): `qwen -p <prompt> -o stream-json --yolo [-r <id>]`.
//! Confirmed with `qwen --help` (0.22.0): -p, -o stream-json, -r/--resume <id>; a real run showed
//! --yolo is accepted and the stream-json is Claude-compatible (result event, `error.message`).
//! unverified: `--approval-mode plan` for the reviewer (not in --help; per qwen-code docs).
use super::claude::parse_line;
use super::common::*;
use crate::agent::{Adapter, AgentEvent, Run};
use crate::model::ModelFamily;
use std::process::Command;

pub struct Qwen {
    bin: String,
}

impl Qwen {
    pub fn new() -> Qwen {
        Qwen { bin: "qwen".into() }
    }
    pub fn with_bin(bin: &str) -> Qwen {
        Qwen { bin: bin.into() }
    }
}

impl Default for Qwen {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for Qwen {
    fn id(&self) -> &str {
        "qwen"
    }
    fn name(&self) -> &str {
        "Qwen Code"
    }
    fn available(&self) -> Result<(), String> {
        check_installed(&self.bin, "npm i -g @qwen-code/qwen-code")
    }
    fn cli_path(&self) -> Option<std::path::PathBuf> {
        find_bin(&self.bin)
    }
    fn version(&self) -> Option<String> {
        cli_version(&self.bin)
    }
    fn command(&self, run: &Run) -> Command {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(&run.cwd).args(["-o", "stream-json"]);
        // No read-only flag in `qwen --help` 0.22: a read-only role just does not get --yolo (and the engine
        // runs it in a throwaway copy).
        if !run.is_read_only() {
            c.arg("--yolo");
        }
        if let Some(m) = &run.model {
            c.args(["-m", m]);
        }
        c.envs(run.extra_env.iter().map(|(k, v)| (k, v)));
        c.args(&run.extra_args);
        if let Some(s) = &run.resume {
            c.args(["--resume", s]);
        }
        c.arg(format!("--prompt={}", with_system(&run.system, &run.prompt)));
        c
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        parse_line(line)
    }
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
        finish_without_result(exit_ok, all_text)
    }
    fn install_hint(&self) -> Option<String> {
        Some("npm install -g @qwen-code/qwen-code (не сверено)".into())
    }
    fn interactive_resume(&self, session: &str, cwd: &std::path::Path) -> Option<Command> {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(cwd).args(["--resume", super::claude::valid_session(session)?]);
        Some(c)
    }
    /// No list command in the CLI. Names: Qwen Code docs, "Alibaba Cloud Coding Plan" model table
    /// (qwenlm.github.io/qwen-code-docs, model-providers), read 2026-10-21, not checked against the CLI;
    /// the table is what `/auth` configures with a Coding Plan key, other providers may differ.
    fn builtin_models(&self) -> Vec<ModelFamily> {
        const SRC: &str = "Qwen Code docs, 2026-10-21; не сверено с CLI";
        vec![
            builtin_family("qwen3-max-2026-01-23", "Qwen3 Max", "qwen", SRC /* Max model with thinking. */, &[]),
            builtin_family(
                "qwen-plus",
                "Qwen Plus",
                "qwen",
                SRC /* Advanced model with thinking (3.6: Pro subscribers only). */,
                &[("3.7", "qwen3.7-plus"), ("3.6", "qwen3.6-plus"), ("3.5", "qwen3.5-plus")],
            ),
            builtin_family("qwen3-coder-plus", "Qwen3 Coder Plus", "qwen", SRC /* Optimized for coding. */, &[]),
            builtin_family("qwen3-coder-next", "Qwen3 Coder Next", "qwen", SRC /* Experimental coding model. */, &[]),
        ]
    }
}
