//! goose (Block, now github.com/aaif-goose/goose): `goose run --output-format stream-json [--model M] --text=<prompt>`.
//! Not installed here. From documentation/docs/guides/goose-cli-commands.md (`run`: -t/--text, --model, --provider,
//! --output-format text|json|stream-json, -n/--name, -r/--resume, --no-session; `session`: --session-id, -r/--resume).
//! The stream-json events are the `StreamEvent` enum of crates/goose-cli/src/session/mod.rs: `{"type":"message",
//! "message":{role, content:[{type:"text"|"toolRequest"|"toolResponse"|"error", ...}]}}`, `{"type":"error","error"}`,
//! `{"type":"complete", input_tokens, output_tokens}`. No session id in the stream, so a run is never resumed.
//! Tools run without asking by default (`GOOSE_MODE` = "auto", guides/config-files.md); there is no read-only mode
//! short of `chat` (no tools at all), so readers run in the engine's throwaway copy.
//! unverified: the field names inside toolRequest / toolResponse (`toolCall.value.{name,arguments}`, `toolResult.status`).
use super::common::*;
use crate::agent::{actions, Adapter, AgentEvent, Run};
use serde_json::Value;
use std::process::Command;

pub struct Goose {
    bin: String,
}

impl Goose {
    pub fn new() -> Goose {
        Goose { bin: "goose".into() }
    }
    pub fn with_bin(bin: &str) -> Goose {
        Goose { bin: bin.into() }
    }
}

impl Default for Goose {
    fn default() -> Self {
        Self::new()
    }
}

const INSTALL: &str = "curl -fsSL https://github.com/aaif-goose/goose/releases/download/stable/download_cli.sh | bash";

fn scan(line: &str) -> Scan {
    let Some(j) = json_line(line) else { return Scan::default() };
    let mut s = Scan::default();
    let n = |v: &Value| v.as_u64().unwrap_or(0);
    match j["type"].as_str() {
        Some("message") => {
            let m = &j["message"];
            let blocks = m["content"].as_array().map(Vec::as_slice).unwrap_or_default();
            let asks = blocks.iter().any(|b| b["type"] == "toolRequest");
            for b in blocks {
                match b["type"].as_str() {
                    Some("text") if m["role"] == "assistant" => {
                        if let Some(t) = b["text"].as_str().filter(|t| !t.trim().is_empty()) {
                            s.events.push(AgentEvent::Text(t.to_string()));
                            if !asks {
                                s.text = Some(s.text.take().unwrap_or_default() + t);
                            }
                        }
                    }
                    Some("toolRequest") => {
                        let call = &b["toolCall"]["value"];
                        s.events.push(AgentEvent::Action(actions::tool_action(b["id"].as_str().unwrap_or(""), call["name"].as_str().unwrap_or("tool"), &call["arguments"])));
                    }
                    Some("toolResponse") => {
                        let r = &b["toolResult"];
                        let out = actions::content_text(&r["value"]["content"]);
                        let out = (!out.is_empty()).then_some(out).or_else(|| r["error"].as_str().map(String::from));
                        s.events.push(AgentEvent::Action(actions::result(b["id"].as_str().unwrap_or(""), out, r["status"] != "error", None)));
                    }
                    // an error block's `kind` is a category (authentication, creditsExhausted, ...): said in words the shared rules know
                    Some("error") => {
                        let msg = b["message"].as_str().unwrap_or("goose error");
                        s.error = Some(match b["kind"].as_str() {
                            Some("authentication") => format!("authentication required: {msg}"),
                            Some("creditsExhausted") => format!("usage limit reached: {msg}"),
                            _ => msg.to_string(),
                        });
                    }
                    _ => {}
                }
            }
        }
        Some("error") => s.error = Some(j["error"].as_str().unwrap_or("goose error").to_string()),
        Some("complete") => {
            s.ok = true;
            let (i, o) = (n(&j["input_tokens"]) + n(&j["cache_read_input_tokens"]) + n(&j["cache_write_input_tokens"]), n(&j["output_tokens"]));
            if i + o > 0 {
                s.events.push(AgentEvent::Tokens { input: i, output: o });
            }
        }
        _ => {}
    }
    s
}

impl Adapter for Goose {
    fn id(&self) -> &str {
        "goose"
    }
    fn name(&self) -> &str {
        "goose"
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
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(&run.cwd).args(["run", "--output-format", "stream-json"]);
        if let Some(m) = cli_model(run) {
            c.args(["--model", m]);
        }
        c.envs(run.extra_env.iter().map(|(k, v)| (k, v)));
        c.args(&run.extra_args);
        // `Run.resume` is never set: the stream names no session to continue
        c.arg(format!("--text={}", with_system(&run.system, &run.prompt)));
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
    /// `goose session --resume --session-id <id>` (goose-cli-commands.md, `session`; not on this machine).
    fn resume_argv(&self, session: &str, _cwd: &std::path::Path) -> Option<Vec<String>> {
        crate::agent::resume_words(&self.bin, &["session", "--resume", "--session-id"], session)
    }
    fn interactive_resume(&self, session: &str, cwd: &std::path::Path) -> Option<Command> {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(cwd).args(["session", "--resume", "--session-id", super::claude::valid_session(session)?]);
        Some(c)
    }
}
