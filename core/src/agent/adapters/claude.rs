//! Claude Code: `claude -p ... --output-format stream-json --verbose`.
//! Flags confirmed with `claude --help` (2.1.306) and a real run: -p, --output-format, --verbose,
//! --permission-mode (plan | bypassPermissions), --settings <json>, --append-system-prompt, --resume.
//! Settings keys (permissions.deny, sandbox.*) confirmed in code.claude.com/docs (settings, sandboxing).
use super::common::*;
use crate::agent::{Adapter, AgentEvent, Run, SandboxInfo};
use crate::model::{LimitWindow, ModelFamily};
use chrono::{Local, TimeZone};
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

/// `--settings` JSON. The only `Read(...)` deny is `~/.ssh` (R17i.2): in the sandbox those become OS-level
/// denials, and blocking `*.pem` would break TLS for every subprocess.
fn settings_json(open_network: bool) -> String {
    let mut permissions = json!({ "deny": ["Bash(git push:*)", "Bash(git remote:*)", "Bash(sudo:*)", "Read(~/.ssh/**)"] });
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

type Version = (u32, u32, u32);

/// binary -> (when read, what it said)
type VersionCache = Mutex<HashMap<String, (Instant, Option<Version>)>>;

/// `claude --version`, re-read at most every 10 minutes per binary (the user may upgrade while the app runs).
fn version_at(bin: &str, now: Instant) -> Option<Version> {
    static CACHE: OnceLock<VersionCache> = OnceLock::new();
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
    fn cli_path(&self) -> Option<std::path::PathBuf> {
        find_bin(&self.bin)
    }
    fn version(&self) -> Option<String> {
        cli_version(&self.bin)
    }
    fn guarded(&self) -> bool {
        true // deny rules for push/remote/sudo + sandbox are passed on every run
    }
    fn command(&self, run: &Run) -> Command {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(&run.cwd).args(["-p", "--output-format", "stream-json", "--verbose"]);
        let mode = if run.is_read_only() { "plan" } else { "bypassPermissions" };
        c.args(["--permission-mode", mode, "--settings", &settings_json(supports_wildcard_domain(&self.bin))]);
        if let Some(m) = &run.model {
            c.args(["--model", m]);
        }
        if let Some(s) = &run.json_schema {
            c.args(["--json-schema", s]);
        }
        if let Some(b) = run.max_budget_usd {
            c.args(["--max-budget-usd", &b.to_string()]);
        }
        c.envs(run.extra_env.iter().map(|(k, v)| (k, v)));
        c.args(&run.extra_args);
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
        let Some(j) = json_line(line) else { return vec![] };
        match j["type"].as_str() {
            Some("rate_limit_event") => rate_limit_events(&j["rate_limit_info"], Local::now()),
            Some("result") => {
                // cost comes before Done, so the engine has it when the run ends
                let cost = j["total_cost_usd"].as_f64().map(AgentEvent::Cost);
                cost.into_iter().chain(parse_line(line)).collect()
            }
            _ => parse_line(line),
        }
    }
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
        finish_without_result(exit_ok, all_text)
    }
    fn reports_cost(&self) -> bool {
        true
    }
    fn read_only_native(&self) -> bool {
        true
    }
    fn install_hint(&self) -> Option<String> {
        Some("npm i -g @anthropic-ai/claude-code (не сверено)".into())
    }
    fn sandbox_info(&self) -> SandboxInfo {
        let note = self.network_note();
        SandboxInfo { sandboxed: true, push_blocked: self.guarded(), network_open: note.is_none(), writes_outside_copy: false, note }
    }
    fn interactive_resume(&self, session: &str, cwd: &std::path::Path) -> Option<Command> {
        let mut c = Command::new(find_bin(&self.bin).unwrap_or_else(|| self.bin.clone().into()));
        c.current_dir(cwd).args(["--resume", valid_session(session)?]);
        Some(c)
    }
    /// No list command in the CLI (`claude --help`); aliases are "the latest model" of a family.
    /// Versions and identifiers: Claude Platform docs (models overview, "Model IDs and versioning",
    /// model-deprecations), read 2026-10-21, not checked against the CLI. Identifier rule from the docs:
    /// `claude-{name}-{major}[-{minor}]`; `claude-opus-5-5` was also seen live in the init event of the
    /// saved run. Order (fable, opus, sonnet, haiku) = the docs' comparison table, strongest first.
    fn builtin_models(&self) -> Vec<ModelFamily> {
        let note = |b: &str| format!("{b} (Anthropic docs, 2026-10-21; не сверено с CLI)");
        vec![
            builtin_family("fable", "Fable", "anthropic", &note("Demanding reasoning, long agentic work."), &[("5.1", "claude-fable-5-1")]),
            builtin_family(
                "opus",
                "Opus",
                "anthropic",
                &note("Long-running agentic coding and knowledge work."),
                &[("5.5", "claude-opus-5-5"), ("5", "claude-opus-5"), ("4.8", "claude-opus-4-8"), ("4.7", "claude-opus-4-7"), ("4.6", "claude-opus-4-6")],
            ),
            builtin_family(
                "sonnet",
                "Sonnet",
                "anthropic",
                &note("Speed and intelligence."),
                &[("5.5", "claude-sonnet-5-5"), ("5", "claude-sonnet-5"), ("4.6", "claude-sonnet-4-6")],
            ),
            builtin_family("haiku", "Haiku", "anthropic", &note("High-volume, latency-sensitive tasks."), &[("5.5", "claude-haiku-5-5"), ("4.5", "claude-haiku-4-5")]),
        ]
    }
}

/// `rate_limit_event` -> Usage (the 5 h and weekly windows); a status other than allowed -> Limit too.
/// `utilization` is a share 0..1, `resetsAt` Unix seconds (research-clis, "Проверено живым запуском").
fn rate_limit_events(info: &serde_json::Value, now: chrono::DateTime<Local>) -> Vec<AgentEvent> {
    let at = |v: &serde_json::Value| v.as_i64().and_then(|t| Local.timestamp_opt(t, 0).single());
    let status = info["status"].as_str().unwrap_or("allowed");
    // unverified: "allowed_warning" is a warning, not a stop (only the plain "allowed" is documented in the brief)
    let blocked = !status.starts_with("allowed");
    let hit = info["rateLimitType"].as_str();
    let mut windows = vec![];
    for name in ["five_hour", "seven_day"] {
        let w = &info["unifiedWindows"][name];
        let Some(used) = w["utilization"].as_f64() else { continue };
        windows.push(LimitWindow {
            name: name.into(),
            model: None,
            used_pct: Some((used * 100.0) as f32),
            remaining: None,
            resets_at: at(&w["resetsAt"]),
            exhausted: used >= 1.0 || (blocked && hit == Some(name)),
            source: "rate_limit_event".into(),
            seen_at: now,
        });
    }
    let mut out = vec![];
    if !windows.is_empty() {
        out.push(AgentEvent::Usage(windows));
    }
    if blocked {
        out.push(AgentEvent::Limit { resets_at: at(&info["resetsAt"]) });
    }
    out
}

/// The id only if it is safe to put into a terminal command.
pub(super) fn valid_session(s: &str) -> Option<&str> {
    valid_session_id(s).then_some(s)
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
                // unverified: the shape of the rate_limit error category (headless docs name only the word)
                let category = [&j["error"], &j["error_category"], &j["category"]].iter().any(|v| *v == "rate_limit");
                match classify_failure(&text, "") {
                    AgentEvent::Done { .. } if category => return vec![AgentEvent::Limit { resets_at: None }],
                    AgentEvent::Done { .. } => {}
                    other => return vec![other],
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
