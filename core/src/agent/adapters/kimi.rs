//! Kimi Code CLI (Moonshot AI): `kimi --output-format stream-json [-m <alias>] [--session <id>] --prompt=<text>`.
//! Not installed here. From MoonshotAI/kimi-code docs/en/reference/kimi-command.md: -p/--prompt runs one prompt with the
//! `auto` permission policy (--yolo/--auto/--plan are refused with --prompt), --output-format text|stream-json,
//! -S/--session <id> resumes, -c/--continue, -m/--model <alias>. The stream-json lines are OpenAI-style messages
//! (apps/kimi-code/src/cli/prompt-render.ts): `{"role":"assistant","content","tool_calls":[{id,function{name,arguments}}]}`,
//! `{"role":"tool","tool_call_id","content"}`, `{"role":"meta","type":"session.resume_hint","session_id"}`; no final
//! result line, so Done is built at EOF. Errors go to stderr with a non-zero exit.
//! unverified: everything above was read, not run; `--prompt=<text>` with `=`.
use super::common::*;
use crate::agent::{actions, Adapter, AgentEvent, Run};
use std::process::Command;

pub struct Kimi {
    bin: String,
}

impl Kimi {
    pub fn new() -> Kimi {
        Kimi { bin: "kimi".into() }
    }
    pub fn with_bin(bin: &str) -> Kimi {
        Kimi { bin: bin.into() }
    }
}

impl Default for Kimi {
    fn default() -> Self {
        Self::new()
    }
}

const INSTALL: &str = "curl -fsSL https://code.kimi.com/kimi-code/install.sh | bash";

fn scan(line: &str) -> Scan {
    let Some(j) = json_line(line) else { return Scan::default() };
    let mut s = Scan::default();
    match j["role"].as_str() {
        Some("assistant") => {
            let calls = j["tool_calls"].as_array().map(Vec::as_slice).unwrap_or_default();
            // the text is written before the calls of the same message; with calls it is not the final answer
            if let Some(t) = j["content"].as_str().filter(|t| !t.trim().is_empty()) {
                s.events.push(AgentEvent::Text(t.to_string()));
                s.text = calls.is_empty().then(|| t.to_string());
            }
            for c in calls {
                let f = &c["function"];
                // `arguments` is a JSON string (stringifyJsonValue); a non-JSON one stays text
                let args = f["arguments"].as_str().and_then(|a| serde_json::from_str(a).ok()).unwrap_or_else(|| f["arguments"].clone());
                s.events.push(AgentEvent::Action(actions::tool_action(c["id"].as_str().unwrap_or(""), f["name"].as_str().unwrap_or("tool"), &args)));
            }
        }
        Some("tool") => {
            let out = j["content"].as_str().map(String::from);
            s.events.push(AgentEvent::Action(actions::result(j["tool_call_id"].as_str().unwrap_or(""), out, true, None)));
        }
        Some("meta") if j["type"] == "session.resume_hint" => s.session = j["session_id"].as_str().map(String::from),
        _ => {}
    }
    s
}

impl Adapter for Kimi {
    fn id(&self) -> &str {
        "kimi"
    }
    fn name(&self) -> &str {
        "Kimi Code"
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
        // no read-only flag in print mode (--plan is refused with --prompt): the engine runs readers in a throwaway copy
        c.current_dir(&run.cwd).args(["--output-format", "stream-json"]);
        if let Some(m) = cli_model(run) {
            c.args(["-m", m]);
        }
        c.envs(run.extra_env.iter().map(|(k, v)| (k, v)));
        c.args(&run.extra_args);
        if let Some(s) = &run.resume {
            c.args(["--session", s]);
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
    /// `kimi --session <id>` (kimi-command.md: "-S, --session [id] Resume a session"; not on this machine).
    fn resume_argv(&self, session: &str, _cwd: &std::path::Path) -> Option<Vec<String>> {
        crate::agent::resume_words(&self.bin, &["--session"], session)
    }
    fn interactive_resume(&self, session: &str, cwd: &std::path::Path) -> Option<Command> {
        let mut c = crate::proc::cmd(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(cwd).args(["--session", super::claude::valid_session(session)?]);
        Some(c)
    }
}
