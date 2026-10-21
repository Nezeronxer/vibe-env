//! Gemini CLI: `gemini -p <prompt> --output-format stream-json --approval-mode yolo`.
//! Not installed here; from google-gemini/gemini-cli docs (cli-reference.md, headless.md):
//! -p, -o stream-json, --approval-mode {default,auto_edit,yolo,plan}, -r <session id>.
//! The event names are documented (init/message/tool_use/tool_result/error/result); field names
//! below are unverified: headless.md has no JSON examples.
use super::common::*;
use crate::agent::{Adapter, AgentEvent, Run};
use crate::model::ModelFamily;
use std::process::Command;

pub struct Gemini {
    bin: String,
}

impl Gemini {
    pub fn new() -> Gemini {
        Gemini { bin: "gemini".into() }
    }
    pub fn with_bin(bin: &str) -> Gemini {
        Gemini { bin: bin.into() }
    }
}

impl Default for Gemini {
    fn default() -> Self {
        Self::new()
    }
}

fn scan(line: &str) -> Scan {
    let Some(j) = json_line(line) else { return Scan::default() };
    let mut s = Scan::default();
    match j["type"].as_str() {
        Some("init") => s.session = j["session_id"].as_str().map(String::from),
        Some("message") if j["role"] == "assistant" => {
            if let Some(t) = j["content"].as_str() {
                s.events.push(AgentEvent::Text(t.to_string()));
                s.text = Some(t.to_string());
            }
        }
        Some("tool_use") => s.events.push(AgentEvent::Tool {
            name: j["tool_name"].as_str().unwrap_or("tool").to_string(),
            summary: summarize(&j["parameters"]),
        }),
        Some("error") if j["severity"] == "error" => s.error = j["message"].as_str().map(String::from),
        Some("result") if j["status"] == "success" => s.ok = true,
        Some("result") => {
            s.error = Some(j["error"]["message"].as_str().unwrap_or("gemini failed").to_string());
        }
        _ => {}
    }
    s
}

impl Adapter for Gemini {
    fn id(&self) -> &str {
        "gemini"
    }
    fn name(&self) -> &str {
        "Gemini CLI"
    }
    fn available(&self) -> Result<(), String> {
        check_installed(&self.bin, "npm i -g @google/gemini-cli")
    }
    fn command(&self, run: &Run) -> Command {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        let mode = if run.is_read_only() { "plan" } else { "yolo" };
        c.current_dir(&run.cwd).args(["--output-format", "stream-json"]).arg(format!("--approval-mode={mode}"));
        if let Some(m) = &run.model {
            c.args(["-m", m]);
        }
        c.envs(run.extra_env.iter().map(|(k, v)| (k, v)));
        c.args(&run.extra_args);
        if let Some(s) = &run.resume {
            // unverified: cli-reference documents `-r <session-id> "query"`, not whether it works with -p
            c.args(["--resume", s]);
        }
        // `--prompt=<text>` keeps a prompt that starts with "-" from being read as a flag
        c.arg(format!("--prompt={}", with_system(&run.system, &run.prompt)));
        c
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        scan(line).events
    }
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
        synth_done(scan, exit_ok, all_text)
    }
    fn read_only_native(&self) -> bool {
        true
    }
    fn install_hint(&self) -> Option<String> {
        Some("npm i -g @google/gemini-cli (не сверено)".into())
    }
    fn interactive_resume(&self, session: &str, cwd: &std::path::Path) -> Option<Command> {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(cwd).args(["--resume", super::claude::valid_session(session)?]);
        Some(c)
    }
    /// No list command (research-clis §3). Names: Gemini API models page (ai.google.dev/gemini-api/docs/models),
    /// read 2026-10-21, not checked against the CLI. Only stable / preview text models; Gemini 4 Argon
    /// ("new model" banner) is left out until the page lists it as a regular model.
    fn builtin_models(&self) -> Vec<ModelFamily> {
        let note = |b: &str| format!("{b} (Google docs, 2026-10-21; не сверено с CLI)");
        vec![
            builtin_family("pro", "Gemini Pro", "google", &note("Stronger, long context."), &[("3.1", "gemini-3.1-pro-preview"), ("2.5", "gemini-2.5-pro")]),
            builtin_family(
                "flash",
                "Gemini Flash",
                "google",
                &note("Fast and cheap."),
                &[("3.8", "gemini-3.8-flash"), ("3.6", "gemini-3.6-flash"), ("2.5", "gemini-2.5-flash")],
            ),
            builtin_family(
                "flash-lite",
                "Gemini Flash-Lite",
                "google",
                &note("Fastest and cheapest."),
                &[("3.5", "gemini-3.5-flash-lite"), ("3.1", "gemini-3.1-flash-lite"), ("2.5", "gemini-2.5-flash-lite")],
            ),
        ]
    }
}
