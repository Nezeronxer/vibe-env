//! opencode: `opencode run --format json --dir <cwd> [--session <id>] [--auto]`.
//! Confirmed with `opencode run --help` (1.18.34) and real runs: --format json, --session, --dir,
//! --auto (auto-approve), --agent. The stream has no final event, so Done is built at EOF.
use super::common::*;
use crate::agent::{Adapter, AgentEvent, Role, Run};
use std::process::Command;

pub struct OpenCode {
    bin: String,
}

impl OpenCode {
    pub fn new() -> OpenCode {
        OpenCode { bin: "opencode".into() }
    }
    pub fn with_bin(bin: &str) -> OpenCode {
        OpenCode { bin: bin.into() }
    }
}

impl Default for OpenCode {
    fn default() -> Self {
        Self::new()
    }
}

fn scan(line: &str) -> Scan {
    let Some(j) = json_line(line) else { return Scan::default() };
    let mut s = Scan { session: j["sessionID"].as_str().map(String::from), ..Scan::default() };
    match j["type"].as_str() {
        Some("text") => {
            if let Some(t) = j["part"]["text"].as_str() {
                s.events.push(AgentEvent::Text(t.to_string()));
                s.text = Some(t.to_string());
            }
        }
        Some("tool_use") => s.events.push(AgentEvent::Tool {
            name: j["part"]["tool"].as_str().unwrap_or("tool").to_string(),
            summary: summarize(&j["part"]["state"]["input"]),
        }),
        Some("error") => {
            let m = j["error"]["data"]["message"].as_str().or_else(|| j["error"]["name"].as_str());
            s.error = Some(m.unwrap_or("opencode error").to_string());
        }
        _ => {}
    }
    s
}

impl Adapter for OpenCode {
    fn id(&self) -> &str {
        "opencode"
    }
    fn name(&self) -> &str {
        "opencode"
    }
    fn available(&self) -> Result<(), String> {
        check_installed(&self.bin, "npm i -g opencode-ai")
    }
    fn command(&self, run: &Run) -> Command {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(&run.cwd).args(["run", "--format", "json", "--dir"]).arg(&run.cwd);
        if run.role == Role::Reviewer {
            c.args(["--agent", "plan"]); // read-only built-in agent; unverified: opencode.ai/docs/agents
        } else {
            c.arg("--auto");
        }
        if let Some(s) = &run.resume {
            c.args(["--session", s]);
        }
        c.arg("--").arg(with_system(&run.system, &run.prompt));
        c
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        scan(line).events
    }
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
        synth_done(scan, exit_ok, all_text)
    }
}
