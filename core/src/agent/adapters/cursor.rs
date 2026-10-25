//! Cursor Agent: `cursor-agent -p --output-format stream-json [--force] [--resume <id>]`.
//! Not installed here. Output format from cursor.com/docs/cli/reference/output-format (system/user/
//! assistant/tool_call/result events). unverified: --force (write without asking), --resume and
//! the read-only default of print mode without --force -- not on that page (cursor.com/docs/cli).
use super::claude::parse_line;
use super::common::*;
use crate::agent::{actions, Adapter, AgentEvent, Run};
use crate::model::ModelFamily;
use std::time::Duration;
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
    fn cli_path(&self) -> Option<std::path::PathBuf> {
        find_bin(&self.bin)
    }
    fn version(&self) -> Option<String> {
        cli_version(&self.bin)
    }
    fn command(&self, run: &Run) -> Command {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(&run.cwd).args(["-p", "--output-format", "stream-json"]);
        if run.is_read_only() {
            c.args(["--mode", "ask"]); // unverified: parameters page lists `--mode ask` (no edits)
        } else {
            c.arg("--force");
        }
        if let Some(m) = cli_model(run) {
            c.args(["--model", m]);
        }
        c.envs(run.extra_env.iter().map(|(k, v)| (k, v)));
        c.args(&run.extra_args);
        if let Some(s) = &run.resume {
            c.args(["--resume", s]);
        }
        // unverified: `--` before the positional prompt (cursor.com/docs/cli does not show it)
        c.arg("--").arg(with_system(&run.system, &run.prompt));
        c
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        let Some(j) = json_line(line) else { return vec![] };
        if j["type"] == "tool_call" {
            // tool_call is keyed by tool type: {"writeToolCall": {"args": {...}}}
            let Some((key, v)) = j["tool_call"].as_object().and_then(|o| o.iter().next()) else { return vec![] };
            let id = j["call_id"].as_str().unwrap_or("");
            let name = key.strip_suffix("ToolCall").unwrap_or(key);
            let args = if key == "function" { &v["arguments"] } else { &v["args"] };
            return match j["subtype"].as_str() {
                Some("started") => vec![AgentEvent::Action(actions::tool_action(id, name, args))],
                // unverified: call_id and result.success {stdout|content|...} / result.error|failure {message}
                Some("completed") => {
                    let r = &v["result"];
                    let failed = ["error", "failure", "rejected"].iter().find_map(|k| r.get(*k));
                    let ok = failed.is_none();
                    let body = failed.unwrap_or(&r["success"]);
                    let text = ["stdout", "output", "content", "message", "errorMessage"].iter().find_map(|k| body[k].as_str()).map(String::from);
                    let stderr = body["stderr"].as_str().filter(|e| !e.is_empty());
                    let out = match (text, stderr) {
                        (Some(o), Some(e)) => Some(format!("{o}\n{e}")),
                        (o, e) => o.or(e.map(String::from)),
                    };
                    vec![AgentEvent::Action(actions::result(id, out, ok, None))]
                }
                _ => vec![],
            };
        }
        parse_line(line)
    }
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
        finish_without_result(exit_ok, all_text)
    }
    fn read_only_native(&self) -> bool {
        true
    }
    fn install_hint(&self) -> Option<String> {
        Some("curl https://cursor.com/install -fsS | bash (не сверено)".into())
    }
    /// `--list-models`. unverified: lines `id - Label`, "(current)" / "(default)" marks the default
    /// (cursor.com/docs/cli/reference/parameters names the flag, not the output).
    fn models(&self) -> Result<Vec<ModelFamily>, String> {
        let out = capture(&self.bin, &["--list-models"], Duration::from_secs(20))?;
        let rows: Vec<_> = out
            .lines()
            .filter_map(|l| {
                let l = strip_ansi(l);
                let (id, label) = l.trim().split_once(" - ")?;
                if !is_model_id(id) {
                    return None;
                }
                let default = label.contains("(current)") || label.contains("(default)");
                let label = label.replace("(current)", "").replace("(default)", "");
                Some((id.to_string(), Some(label.trim().to_string()), None, default))
            })
            .collect();
        if rows.is_empty() {
            return Err("--list-models: no models in the output".into());
        }
        Ok(group_models(&rows, |_| "cursor".into()))
    }
    /// `cursor-agent --resume <id>` (по документации: cursor.com/docs/cli; not on this machine).
    fn resume_argv(&self, session: &str, _cwd: &std::path::Path) -> Option<Vec<String>> {
        crate::agent::resume_words(&self.bin, &["--resume"], session)
    }
    fn inventory(&self, cwd: &std::path::Path) -> crate::agent::Inventory {
        crate::agent::inventory::home().map(|h| crate::agent::inventory::scan("cursor", &h, cwd)).unwrap_or_default()
    }
    /// `agent --resume <id>` (binary name as configured, see `bin`).
    fn interactive_resume(&self, session: &str, cwd: &std::path::Path) -> Option<Command> {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(cwd).args(["--resume", super::claude::valid_session(session)?]);
        Some(c)
    }
}
