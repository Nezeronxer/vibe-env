//! Codex CLI: `codex exec --json --sandbox workspace-write` (+ `resume <id>`).
//! Not installed here; flags and events from learn.chatgpt.com/docs/non-interactive-mode
//! (--json, --sandbox, `resume <SESSION_ID>`, --skip-git-repo-check, thread.started/item.*/turn.*).
//! `--full-auto` is no longer supported there, so workspace-write is passed explicitly.
use super::common::*;
use crate::agent::{Adapter, AgentEvent, Role, Run};
use std::process::Command;

pub struct Codex {
    bin: String,
}

impl Codex {
    pub fn new() -> Codex {
        Codex { bin: "codex".into() }
    }
    pub fn with_bin(bin: &str) -> Codex {
        Codex { bin: bin.into() }
    }
}

impl Default for Codex {
    fn default() -> Self {
        Self::new()
    }
}

fn scan(line: &str) -> Scan {
    let Some(j) = json_line(line) else { return Scan::default() };
    let mut s = Scan::default();
    let item = &j["item"];
    match (j["type"].as_str(), item["type"].as_str()) {
        (Some("thread.started"), _) => s.session = j["thread_id"].as_str().map(String::from),
        (Some("item.started"), Some("command_execution")) => s.events.push(AgentEvent::Tool {
            name: "shell".into(),
            summary: summarize(&serde_json::json!({ "command": item["command"] })),
        }),
        // unverified: shape of file_change items (changes: [{path, kind}]) -- chatgpt.com docs show only command_execution
        (Some("item.completed"), Some("file_change")) => {
            let paths: Vec<&str> = item["changes"].as_array().into_iter().flatten().filter_map(|c| c["path"].as_str()).collect();
            s.events.push(AgentEvent::Tool { name: "edit".into(), summary: paths.join(", ") });
        }
        (Some("item.completed"), Some("agent_message")) => {
            if let Some(t) = item["text"].as_str() {
                s.events.push(AgentEvent::Text(t.to_string()));
                s.text = Some(t.to_string());
            }
        }
        (Some("turn.completed"), _) => s.ok = true,
        (Some("turn.failed"), _) => s.error = Some(j["error"]["message"].as_str().unwrap_or("codex turn failed").to_string()),
        (Some("error"), _) => s.error = j["message"].as_str().map(String::from),
        _ => {}
    }
    s
}

impl Adapter for Codex {
    fn id(&self) -> &str {
        "codex"
    }
    fn name(&self) -> &str {
        "Codex CLI"
    }
    fn available(&self) -> Result<(), String> {
        check_installed(&self.bin, "npm i -g @openai/codex")
    }
    fn command(&self, run: &Run) -> Command {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        // writes only inside the working copy; the reviewer cannot write at all
        let sandbox = if run.role == Role::Reviewer { "read-only" } else { "workspace-write" };
        c.current_dir(&run.cwd).args(["exec", "--json", "--skip-git-repo-check", "--sandbox", sandbox]);
        if let Some(s) = &run.resume {
            // unverified: whether `resume` accepts --json/--sandbox placed before it; docs show `codex exec resume <ID> "<prompt>"`
            c.args(["resume", s]);
        }
        // unverified: `--` before the prompt is clap convention, not shown in the codex docs
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
