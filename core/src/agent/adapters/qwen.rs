//! Qwen Code (fork of Gemini CLI): `qwen -p <prompt> -o stream-json --yolo [-r <id>]`.
//! Confirmed with `qwen --help` (0.22.0): -p, -o stream-json, -r/--resume <id>; a real run showed
//! --yolo is accepted and the stream-json is Claude-compatible (result event, `error.message`).
//! unverified: `--approval-mode plan` for the reviewer (not in --help; per qwen-code docs).
use super::claude::parse_line;
use super::common::*;
use crate::agent::{Adapter, AgentEvent, Role, Run};
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
    fn command(&self, run: &Run) -> Command {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(&run.cwd).args(["-o", "stream-json"]);
        if run.role == Role::Reviewer {
            c.args(["--approval-mode", "plan"]);
        } else {
            c.arg("--yolo");
        }
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
}
