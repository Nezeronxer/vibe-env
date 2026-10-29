//! GitHub Copilot CLI: `copilot --output-format=json --allow-all-tools --no-ask-user [--model M] [--resume=<id>] --prompt=<text>`.
//! Not installed here. Flags from docs.github.com/en/copilot/reference/cli-command-reference: -p/--prompt=PROMPT
//! (exits after completion), --output-format=json (JSONL), --model=MODEL, -r/--resume[=VALUE] (an optional value, so
//! always with `=`), --continue, --allow-all-tools, --no-ask-user. The JSONL record shape is not on that page: the
//! fields below are the session events of github/copilot-sdk docs/features/streaming-events.md
//! (`{type, data}`: assistant.message, tool.execution_start/complete, assistant.usage, session.error).
//! unverified: that the CLI's JSONL uses those events, and where the session id is (`sessionId`).
use super::common::*;
use crate::agent::{actions, Adapter, AgentEvent, Run};
use serde_json::Value;
use std::process::Command;

pub struct Copilot {
    bin: String,
}

impl Copilot {
    pub fn new() -> Copilot {
        Copilot { bin: "copilot".into() }
    }
    pub fn with_bin(bin: &str) -> Copilot {
        Copilot { bin: bin.into() }
    }
}

impl Default for Copilot {
    fn default() -> Self {
        Self::new()
    }
}

const INSTALL: &str = "curl -fsSL https://gh.io/copilot-install | bash";

fn scan(line: &str) -> Scan {
    let Some(j) = json_line(line) else { return Scan::default() };
    let d = &j["data"];
    let mut s = Scan { session: j["sessionId"].as_str().or_else(|| d["sessionId"].as_str()).map(String::from), ..Scan::default() };
    let n = |v: &Value| v.as_u64().unwrap_or(0);
    match j["type"].as_str() {
        Some("assistant.message") => {
            if let Some(t) = d["content"].as_str().filter(|t| !t.trim().is_empty()) {
                s.events.push(AgentEvent::Text(t.to_string()));
                // a message that asks for tools is not the final answer
                let asks = d["toolRequests"].as_array().is_some_and(|r| !r.is_empty());
                s.text = (!asks).then(|| t.to_string());
            }
        }
        Some("assistant.message_delta") if j["agentId"].is_null() => {
            if let Some(t) = d["deltaContent"].as_str().filter(|t| !t.is_empty()) {
                s.events.push(AgentEvent::TextDelta(t.to_string()));
            }
        }
        Some("tool.execution_start") => s.events.push(AgentEvent::Action(actions::tool_action(
            d["toolCallId"].as_str().unwrap_or(""),
            d["toolName"].as_str().unwrap_or("tool"),
            &d["arguments"],
        ))),
        Some("tool.execution_complete") => {
            let out = d["result"]["content"].as_str().or_else(|| d["error"]["message"].as_str()).map(String::from);
            s.events.push(AgentEvent::Action(actions::result(d["toolCallId"].as_str().unwrap_or(""), out, d["success"] != false, None)));
        }
        Some("assistant.usage") => {
            let (i, o) = (n(&d["inputTokens"]) + n(&d["cacheReadTokens"]) + n(&d["cacheWriteTokens"]), n(&d["outputTokens"]));
            if i + o > 0 {
                s.events.push(AgentEvent::Tokens { input: i, output: o });
            }
        }
        // errorType is a category (`authentication`, `quota`, `rate_limit`): said in words the shared rules know
        Some("session.error") => {
            let m = d["message"].as_str().unwrap_or("copilot error");
            s.error = Some(match d["errorType"].as_str() {
                Some("authentication") => format!("authentication required: {m}"),
                Some("quota" | "rate_limit") => format!("usage limit reached: {m}"),
                _ => m.to_string(),
            });
        }
        _ => {}
    }
    s
}

impl Adapter for Copilot {
    fn id(&self) -> &str {
        "copilot"
    }
    fn name(&self) -> &str {
        "GitHub Copilot CLI"
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
        c.current_dir(&run.cwd).args(["--output-format=json", "--no-ask-user"]);
        // a read-only role is not auto-approved (and the engine runs it in a throwaway copy)
        if !run.is_read_only() {
            c.arg("--allow-all-tools");
        }
        if let Some(m) = cli_model(run) {
            c.args(["--model", m]);
        }
        c.envs(run.extra_env.iter().map(|(k, v)| (k, v)));
        c.args(&run.extra_args);
        if let Some(s) = &run.resume {
            c.arg(format!("--resume={s}"));
        }
        c.arg(format!("--prompt={}", with_system(&run.system, &run.prompt)));
        c
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        scan(line).events
    }
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
        synth_done(scan, exit_ok, all_text)
    }
    fn install_hint(&self) -> Option<String> {
        Some(format!("{INSTALL} (не сверено)"))
    }
    /// `copilot --resume=<id>` (cli-command-reference: "-r, --resume[=VALUE]"; not on this machine).
    fn resume_argv(&self, session: &str, _cwd: &std::path::Path) -> Option<Vec<String>> {
        let mut v = crate::agent::resume_words(&self.bin, &[], session)?;
        v[1] = format!("--resume={}", v[1]);
        Some(v)
    }
    fn interactive_resume(&self, session: &str, cwd: &std::path::Path) -> Option<Command> {
        let mut c = crate::proc::cmd(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(cwd).arg(format!("--resume={}", super::claude::valid_session(session)?));
        Some(c)
    }
}
