//! Cursor Agent: `cursor-agent -p --output-format stream-json [--force] [--resume <id>]`.
//! Not installed here. Output format from cursor.com/docs/cli/reference/output-format (system/user/
//! assistant/tool_call/result events). unverified: --force (write without asking), --resume and
//! the read-only default of print mode without --force -- not on that page (cursor.com/docs/cli).
use super::claude::parse_line;
use super::common::*;
use crate::agent::{Adapter, AgentEvent, Role, Run};
use std::process::Command;

pub struct Cursor {
    bin: String,
}

impl Cursor {
    pub fn new() -> Cursor {
        Cursor { bin: "cursor-agent".into() }
    }
    pub fn with_bin(bin: &str) -> Cursor {
        Cursor { bin: bin.into() }
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for Cursor {
    fn id(&self) -> &str {
        "cursor"
    }
    fn name(&self) -> &str {
        "Cursor Agent"
    }
    fn available(&self) -> Result<(), String> {
        check_installed(&self.bin, "curl https://cursor.com/install -fsS | bash")
    }
    fn command(&self, run: &Run) -> Command {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(&run.cwd).args(["-p", "--output-format", "stream-json"]);
        if run.role == Role::Worker {
            c.arg("--force");
        }
        if let Some(s) = &run.resume {
            c.args(["--resume", s]);
        }
        // unverified: `--` before the positional prompt (cursor.com/docs/cli does not show it)
        c.arg("--").arg(with_system(&run.system, &run.prompt));
        c
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        let Some(j) = json_line(line) else { return vec![] };
        if j["type"] == "tool_call" && j["subtype"] == "started" {
            // tool_call is keyed by tool type: {"writeToolCall": {"args": {...}}}
            if let Some((key, v)) = j["tool_call"].as_object().and_then(|o| o.iter().next()) {
                let name = key.strip_suffix("ToolCall").unwrap_or(key);
                let args = if key == "function" { &v["arguments"] } else { &v["args"] };
                return vec![AgentEvent::Tool { name: name.to_string(), summary: summarize(args) }];
            }
            return vec![];
        }
        parse_line(line)
    }
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
        finish_without_result(exit_ok, all_text)
    }
}
