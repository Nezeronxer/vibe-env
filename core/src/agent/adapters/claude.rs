//! Claude Code: `claude -p ... --output-format stream-json --verbose`.
//! Flags confirmed with `claude --help` (2.1.306) and a real run: -p, --output-format, --verbose,
//! --permission-mode (plan | bypassPermissions), --settings <json>, --append-system-prompt, --resume.
//! Settings keys (permissions.deny, sandbox.*) confirmed in code.claude.com/docs (settings, sandboxing).
use super::common::*;
use crate::agent::{Adapter, AgentEvent, Role, Run};
use regex::Regex;
use serde_json::json;
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub struct Claude {
    bin: String,
}

impl Claude {
    pub fn new() -> Claude {
        Claude { bin: "claude".into() }
    }
    /// Another binary name or path (tests, non-standard installs).
    pub fn with_bin(bin: &str) -> Claude {
        Claude { bin: bin.into() }
    }
}

impl Default for Claude {
    fn default() -> Self {
        Self::new()
    }
}

/// `--settings` JSON. No `Read(...)` deny on purpose: in the sandbox those become OS-level
/// denials, and blocking `*.pem` breaks TLS for every subprocess.
fn settings_json(open_network: bool) -> String {
    let mut permissions = json!({ "deny": ["Bash(git push:*)", "Bash(git remote:*)", "Bash(sudo:*)"] });
    if open_network {
        // Why: sandboxed commands get no network unless the host is allowed, and `claude -p` cannot ask, so
        // npm/pip/cargo would fail. Risk: the agent can then send any file of the working copy (including
        // copied .env files) to any host. Only emitted when `claude --version` >= 2.1.186 (bare `*` support).
        permissions["allow"] = json!(["WebFetch(domain:*)"]);
    }
    json!({
        "permissions": permissions,
        // writes only to the worktree (+ its main .git, handled by Claude Code); no sandbox on the host -> run anyway
        "sandbox": { "enabled": true, "autoAllowBashIfSandboxed": true, "failIfUnavailable": false, "allowUnsandboxedCommands": false }
    })
    .to_string()
}

const MIN_WILDCARD: (u32, u32, u32) = (2, 1, 186);
const VERSION_TTL: Duration = Duration::from_secs(600);

/// `claude --version`, re-read at most every 10 minutes per binary (the user may upgrade while the app runs).
fn version_at(bin: &str, now: Instant) -> Option<(u32, u32, u32)> {
    static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Option<(u32, u32, u32)>)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some((at, v)) = cache.lock().unwrap().get(bin) {
        if now.saturating_duration_since(*at) < VERSION_TTL {
            return *v;
        }
    }
    let path = find_bin(bin).unwrap_or_else(|| bin.into());
    let v = Command::new(path).arg("--version").stdin(Stdio::null()).output().ok().and_then(|o| {
        let s = String::from_utf8_lossy(&o.stdout).into_owned();
        let c = Regex::new(r"(\d+)\.(\d+)\.(\d+)").ok()?.captures(&s)?;
        Some((c[1].parse().ok()?, c[2].parse().ok()?, c[3].parse().ok()?))
    });
    cache.lock().unwrap().insert(bin.to_string(), (now, v));
    v
}

fn supports_wildcard_domain(bin: &str) -> bool {
    version_at(bin, Instant::now()).is_some_and(|v| v >= MIN_WILDCARD)
}

impl Claude {
    /// Why the sandbox has no open network (npm/pip/cargo will fail inside it), or None when it is on.
    pub fn network_note(&self) -> Option<String> {
        match version_at(&self.bin, Instant::now()) {
            Some(v) if v >= MIN_WILDCARD => None,
            Some((a, b, c)) => Some(format!(
                "Claude Code {a}.{b}.{c} is older than 2.1.186: sandboxed commands have no network access (npm/pip/cargo installs fail)"
            )),
            None => Some("Claude Code version could not be read: sandboxed commands have no network access (npm/pip/cargo installs fail)".into()),
        }
    }
}

impl Adapter for Claude {
    fn id(&self) -> &str {
        "claude"
    }
    fn name(&self) -> &str {
        "Claude Code"
    }
    fn available(&self) -> Result<(), String> {
        check_installed(&self.bin, "npm i -g @anthropic-ai/claude-code")
    }
    fn guarded(&self) -> bool {
        true // deny rules for push/remote/sudo + sandbox are passed on every run
    }
    fn command(&self, run: &Run) -> Command {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(&run.cwd).args(["-p", "--output-format", "stream-json", "--verbose"]);
        let mode = if run.role == Role::Reviewer { "plan" } else { "bypassPermissions" };
        c.args(["--permission-mode", mode, "--settings", &settings_json(supports_wildcard_domain(&self.bin))]);
        if !run.system.trim().is_empty() {
            c.args(["--append-system-prompt", &run.system]);
        }
        if let Some(s) = &run.resume {
            c.args(["--resume", s]);
        }
        c.arg("--").arg(&run.prompt);
        c
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        parse_line(line)
    }
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
        finish_without_result(exit_ok, all_text)
    }
}

/// Claude-style stream-json (also spoken by Qwen Code): assistant / result events.
pub fn parse_line(line: &str) -> Vec<AgentEvent> {
    let Some(j) = json_line(line) else { return vec![] };
    match j["type"].as_str() {
        Some("assistant") => j["message"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| match b["type"].as_str() {
                Some("text") => b["text"].as_str().filter(|t| !t.trim().is_empty()).map(|t| AgentEvent::Text(t.to_string())),
                Some("tool_use") => Some(AgentEvent::Tool {
                    name: b["name"].as_str().unwrap_or("tool").to_string(),
                    summary: summarize(&b["input"]),
                }),
                _ => None,
            })
            .collect(),
        Some("result") => {
            let is_error = j["is_error"].as_bool().unwrap_or(false) || j["subtype"].as_str().is_some_and(|s| s != "success");
            let text = j["result"]
                .as_str()
                .or_else(|| j["error"]["message"].as_str())
                .or_else(|| j["subtype"].as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if is_error {
                if let Some(resets_at) = detect_limit(&text) {
                    return vec![AgentEvent::Limit { resets_at }];
                }
            }
            vec![AgentEvent::Done { session: j["session_id"].as_str().map(String::from), text, is_error }]
        }
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn version_is_reread_after_ten_minutes() {
        let d = tempfile::tempdir().unwrap();
        let ver = d.path().join("v");
        let bin = d.path().join("claude");
        std::fs::write(&bin, format!("#!/bin/sh\ncat {}\n", ver.display())).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let b = bin.to_str().unwrap();
        let t0 = Instant::now();
        std::fs::write(&ver, "2.1.100 (Claude Code)\n").unwrap();
        assert_eq!(version_at(b, t0), Some((2, 1, 100)));
        std::fs::write(&ver, "2.1.200 (Claude Code)\n").unwrap();
        assert_eq!(version_at(b, t0 + Duration::from_secs(540)), Some((2, 1, 100)), "still cached");
        assert_eq!(version_at(b, t0 + Duration::from_secs(660)), Some((2, 1, 200)), "expired");
    }
}
