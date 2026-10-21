//! Codex CLI: `codex exec --json --sandbox workspace-write` (+ `resume <id>`).
//! Not installed here; flags and events from learn.chatgpt.com/docs/non-interactive-mode
//! (--json, --sandbox, `resume <SESSION_ID>`, --skip-git-repo-check, thread.started/item.*/turn.*).
//! `--full-auto` is no longer supported there, so workspace-write is passed explicitly.
use super::common::*;
use crate::agent::{Adapter, AgentEvent, Run};
use crate::model::ModelFamily;
use std::time::Duration;
use std::process::Command;

pub struct Codex {
    bin: String,
    /// Private folder for answer-schema files (`--output-schema` takes a path). Set by
    /// `with_schema_dir` or, from the engine's data folder, by `Adapter::set_private_dir`; never
    /// worked out here. Without one a run that wants a schema fails and says so.
    schema_dir: std::sync::Mutex<Option<std::path::PathBuf>>,
}

impl Codex {
    pub fn new() -> Codex {
        Codex { bin: "codex".into(), schema_dir: std::sync::Mutex::new(None) }
    }
    pub fn with_bin(bin: &str) -> Codex {
        Codex { bin: bin.into(), schema_dir: std::sync::Mutex::new(None) }
    }
    pub fn with_schema_dir(self, dir: &std::path::Path) -> Codex {
        *self.schema_dir.lock().unwrap_or_else(|e| e.into_inner()) = Some(dir.to_path_buf());
        self
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
        let sandbox = if run.is_read_only() { "read-only" } else { "workspace-write" };
        c.current_dir(&run.cwd).args(["exec", "--json", "--skip-git-repo-check", "--sandbox", sandbox]);
        if let Some(m) = &run.model {
            c.args(["-m", m]);
        }
        if let Some(schema) = run.json_schema.as_deref() {
            let dir = self.schema_dir.lock().unwrap_or_else(|e| e.into_inner()).clone();
            match dir.ok_or_else(|| std::io::Error::other("no private folder for it")).and_then(|d| schema_file(&d, schema)) {
                Ok(path) => {
                    c.arg("--output-schema").arg(path); // unverified: documented as a file path, not inline JSON
                }
                // the flag must not vanish silently: the run fails and says why
                Err(e) => {
                    let mut f = Command::new("sh");
                    f.current_dir(&run.cwd).arg("-c").arg("echo \"$1\" >&2; exit 1").arg("sh").arg(format!("cannot write the answer schema file: {e}"));
                    return f;
                }
            }
        }
        c.envs(run.extra_env.iter().map(|(k, v)| (k, v)));
        c.args(&run.extra_args);
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
    fn read_only_native(&self) -> bool {
        true
    }
    fn install_hint(&self) -> Option<String> {
        Some("npm i -g @openai/codex (не сверено)".into())
    }
    /// `codex debug models`. unverified: output shape (docs say "model catalog"); a JSON array or
    /// `{"models": [...]}` of objects with `slug` is expected, plain lines are accepted too.
    fn models(&self) -> Result<Vec<ModelFamily>, String> {
        let out = capture(&self.bin, &["debug", "models"], Duration::from_secs(20))?;
        let fams = parse_models(&out);
        if fams.is_empty() { Err("codex debug models: no models in the output".into()) } else { Ok(fams) }
    }
    // Codex has no interactive resume template (only `codex exec resume`): interactive_resume stays None.
    fn set_private_dir(&self, dir: &std::path::Path) {
        self.schema_dir.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(|| dir.join("schemas"));
    }
}

/// Writes the schema into the private folder under a name unique to this run (tmp + rename), mode 0700 /
/// 0600 on unix. Files older than a day are removed on the way, they belong to finished runs.
fn schema_file(dir: &std::path::Path, schema: &str) -> std::io::Result<std::path::PathBuf> {
    use std::io::Write;
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    std::fs::set_permissions(dir, std::os::unix::fs::PermissionsExt::from_mode(0o700))?;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|a| a > Duration::from_secs(86400));
            if old {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let name = uuid::Uuid::new_v4().to_string();
    let (tmp, path) = (dir.join(format!("{name}.tmp")), dir.join(format!("{name}.json")));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600); // the comment above promises it: set it
    let mut f = opts.open(&tmp)?;
    f.write_all(schema.as_bytes())?;
    f.sync_all()?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

fn parse_models(out: &str) -> Vec<ModelFamily> {
    let json: Option<serde_json::Value> = serde_json::from_str(out.trim()).ok();
    let items: Vec<serde_json::Value> = match json {
        Some(serde_json::Value::Array(a)) => a,
        Some(v) if v["models"].is_array() => v["models"].as_array().cloned().unwrap_or_default(),
        _ => out.lines().map(|l| serde_json::Value::String(l.trim().to_string())).collect(),
    };
    let rows: Vec<_> = items
        .iter()
        .filter_map(|m| {
            let id = m.as_str().map(String::from).or_else(|| ["slug", "id", "name"].iter().find_map(|k| m[k].as_str().map(String::from)))?;
            if !is_model_id(&id) {
                return None;
            }
            let label = m["display_name"].as_str().map(String::from);
            Some((id, label, m["description"].as_str().map(String::from), false))
        })
        .collect();
    group_models(&rows, |_| "openai".into())
}
