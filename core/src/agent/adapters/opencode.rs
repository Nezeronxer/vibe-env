//! opencode: `opencode run --format json --dir <cwd> [--session <id>] [--auto]`.
//! Confirmed with `opencode run --help` (1.18.34) and real runs: --format json, --session, --dir,
//! --auto (auto-approve), --agent. The stream has no final event, so Done is built at EOF.
use super::common::*;
use crate::agent::{actions, ActionInput, Adapter, AgentEvent, Run};
use crate::model::ModelFamily;
use std::time::Duration;
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
        // one line per finished tool: its start and its result at once.
        // unverified: part.callID, state.status/output/error and metadata.diff (seen: tool, state.input)
        Some("tool_use") => {
            let part = &j["part"];
            let st = &part["state"];
            let id = part["callID"].as_str().or_else(|| part["id"].as_str()).unwrap_or("");
            let start = actions::tool_action(id, part["tool"].as_str().unwrap_or("tool"), &st["input"]);
            let status = st["status"].as_str().unwrap_or("completed");
            s.events.push(AgentEvent::Action(start.clone()));
            if matches!(status, "completed" | "error") {
                let out = st["output"].as_str().or_else(|| st["error"].as_str()).map(String::from);
                let input = match (&start.input, st["metadata"]["diff"].as_str()) {
                    (ActionInput::Edit { path, .. }, Some(d)) => Some(ActionInput::Edit { path: path.clone(), diff: d.to_string() }),
                    _ => None,
                };
                s.events.push(AgentEvent::Action(actions::result(id, out, status == "completed", input)));
            }
        }
        // G70: each step's tokens (live 1.18.34 shape): input + cache read/write, output + reasoning
        Some("step_finish") => {
            let t = &j["part"]["tokens"];
            let n = |v: &serde_json::Value| v.as_u64().unwrap_or(0);
            let (i, o) = (n(&t["input"]) + n(&t["cache"]["read"]) + n(&t["cache"]["write"]), n(&t["output"]) + n(&t["reasoning"]));
            if i + o > 0 {
                s.events.push(AgentEvent::Tokens { input: i, output: o });
            }
        }
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
    fn cli_path(&self) -> Option<std::path::PathBuf> {
        find_bin(&self.bin)
    }
    fn version(&self) -> Option<String> {
        cli_version(&self.bin)
    }
    fn command(&self, run: &Run) -> Command {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        // --print-logs: with a refused model the JSON stream only says "Unexpected server error"; the reason
        // (`ProviderModelNotFoundError: Model not found: ...`) is on stderr, which `finish` then sees.
        c.current_dir(&run.cwd).args(["run", "--format", "json", "--print-logs", "--log-level", "ERROR", "--dir"]).arg(&run.cwd);
        // No read-only flag here (`--agent plan` is unverified): the engine runs read-only roles in a throwaway copy.
        c.arg("--auto");
        if let Some(m) = cli_model(run) {
            c.args(["-m", m]);
        }
        c.envs(run.extra_env.iter().map(|(k, v)| (k, v)));
        c.args(&run.extra_args);
        if let Some(s) = &run.resume {
            c.args(["--session", s]);
        }
        // G65.3: `-f, --file  file(s) to attach to message` (`opencode run --help`, 1.19.4)
        for f in &run.attachments {
            c.arg(format!("--file={}", f.display()));
        }
        c.arg("--").arg(with_system(&run.system, &crate::agent::with_attachments(&run.prompt, &run.attachments)));
        c
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        scan(line).events
    }
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
        synth_done(scan, exit_ok, all_text)
    }
    fn install_hint(&self) -> Option<String> {
        Some("npm i -g opencode-ai (не сверено)".into())
    }
    /// `opencode models`: one `provider/model` per line (real output, 1.18.34).
    fn models(&self) -> Result<Vec<ModelFamily>, String> {
        let out = capture(&self.bin, &["models"], Duration::from_secs(30))?;
        let rows: Vec<_> = out
            .lines()
            .map(str::trim)
            .filter(|l| is_model_id(l))
            .map(|l| (l.to_string(), None, None, false))
            .collect();
        if rows.is_empty() {
            return Err("opencode models: no models in the output".into());
        }
        Ok(group_models(&rows, |id| id.split('/').next().unwrap_or("opencode").to_string()))
    }
    /// `opencode --session <id>` (`opencode --help` 1.19.4: "-s, --session  session id to continue" of the TUI).
    fn resume_argv(&self, session: &str, _cwd: &std::path::Path) -> Option<Vec<String>> {
        crate::agent::resume_words(&self.bin, &["--session"], session)
    }
    fn resume_checked(&self) -> bool {
        true
    }
    fn supports_images(&self) -> bool {
        true
    }
    fn inventory(&self, cwd: &std::path::Path) -> crate::agent::Inventory {
        crate::agent::inventory::home().map(|h| crate::agent::inventory::scan("opencode", &h, cwd)).unwrap_or_default()
    }
    /// `opencode -s <id>`. unverified: that it works in the interactive mode.
    fn interactive_resume(&self, session: &str, cwd: &std::path::Path) -> Option<Command> {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(cwd).args(["-s", super::claude::valid_session(session)?]);
        Some(c)
    }
}
