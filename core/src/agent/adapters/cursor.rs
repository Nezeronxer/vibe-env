//! Cursor Agent: `cursor-agent -p --output-format stream-json [--force] [--resume <id>]`.
//! Not installed here. Output format from cursor.com/docs/cli/reference/output-format (system/user/
//! assistant/tool_call/result events). unverified: --force (write without asking), --resume and
//! the read-only default of print mode without --force -- not on that page (cursor.com/docs/cli).
use super::claude::parse_line;
use super::common::*;
use crate::agent::{Adapter, AgentEvent, Run};
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
    fn command(&self, run: &Run) -> Command {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(&run.cwd).args(["-p", "--output-format", "stream-json"]);
        if run.is_read_only() {
            c.args(["--mode", "ask"]); // unverified: parameters page lists `--mode ask` (no edits)
        } else {
            c.arg("--force");
        }
        if let Some(m) = &run.model {
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
    /// `agent --resume <id>` (binary name as configured, see `bin`).
    fn interactive_resume(&self, session: &str, cwd: &std::path::Path) -> Option<Command> {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(cwd).args(["--resume", super::claude::valid_session(session)?]);
        Some(c)
    }
}
