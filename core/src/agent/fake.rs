//! Scripted adapter for tests: each invocation of `command()` consumes the next script.
//! The script runs as a real `sh` process (writes files into the run's cwd and prints one JSON
//! line per event), so `run_blocking`, cancel and the git worktree behave as with a real CLI.
use super::{Adapter, AgentEvent, Run};
use std::process::Command;
use std::sync::Mutex;

#[derive(Clone, Debug)]
pub enum FakeStep {
    /// Write `content` to a path relative to the run's cwd.
    Write(String, String),
    Say(String),
    Done(String),
    /// Usage-limit message (no reset time).
    Limit,
    /// Exit non-zero.
    Fail,
    /// Extra over interfaces.md: pause for N ms (lets tests exercise cancel).
    Sleep(u64),
}

pub struct FakeAdapter {
    id: String,
    scripts: Vec<Vec<FakeStep>>,
    calls: Mutex<Vec<Run>>,
}

impl FakeAdapter {
    /// One script per invocation; when exhausted, further invocations just say "ok".
    pub fn new(scripts: Vec<Vec<FakeStep>>) -> FakeAdapter {
        FakeAdapter { id: "fake".into(), scripts, calls: Mutex::new(vec![]) }
    }
    pub fn with_id(mut self, id: &str) -> FakeAdapter {
        self.id = id.to_string();
        self
    }
    /// Every `Run` this adapter was asked to execute, in order.
    pub fn calls(&self) -> Vec<Run> {
        self.calls.lock().unwrap().clone()
    }
}

fn q(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

impl Adapter for FakeAdapter {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        "Fake"
    }
    fn available(&self) -> Result<(), String> {
        Ok(())
    }
    fn command(&self, run: &Run) -> Command {
        let n = {
            let mut c = self.calls.lock().unwrap();
            c.push(run.clone());
            c.len() - 1
        };
        let steps = self.scripts.get(n).cloned().unwrap_or_else(|| vec![FakeStep::Done("ok".into())]);
        let session = run.resume.clone().unwrap_or_else(|| format!("fake-session-{n}"));
        let mut sh = String::new();
        for s in steps {
            let ev = |v: serde_json::Value| format!("printf '%s\\n' {}\n", q(&v.to_string()));
            match s {
                FakeStep::Write(path, content) => {
                    sh.push_str(&format!(
                        "mkdir -p \"$(dirname {p})\" && printf '%s' {c} > {p}\n",
                        p = q(&path),
                        c = q(&content)
                    ));
                    sh.push_str(&ev(serde_json::json!({"t":"tool","name":"Write","summary":path})));
                }
                FakeStep::Say(t) => sh.push_str(&ev(serde_json::json!({"t":"say","text":t}))),
                FakeStep::Done(t) => {
                    sh.push_str(&ev(serde_json::json!({"t":"done","text":t,"session":session})))
                }
                FakeStep::Limit => sh.push_str(&ev(serde_json::json!({"t":"limit"}))),
                FakeStep::Fail => sh.push_str("exit 1\n"),
                FakeStep::Sleep(ms) => sh.push_str(&format!("sleep {}\n", ms as f64 / 1000.0)),
            }
        }
        let mut c = Command::new("sh");
        c.arg("-c").arg(sh).current_dir(&run.cwd).stdout(std::process::Stdio::piped());
        c
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { return vec![] };
        let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
        match v["t"].as_str() {
            Some("say") => vec![AgentEvent::Text(s("text"))],
            Some("tool") => vec![AgentEvent::Tool { name: s("name"), summary: s("summary") }],
            Some("done") => vec![AgentEvent::Done {
                session: v["session"].as_str().map(str::to_string),
                text: s("text"),
                is_error: false,
            }],
            Some("limit") => vec![AgentEvent::Limit { resets_at: None }],
            _ => vec![],
        }
    }
    fn finish(&self, exit_ok: bool, _all_text: &str) -> Vec<AgentEvent> {
        if exit_ok {
            vec![]
        } else {
            vec![AgentEvent::Done { session: None, text: "fake agent failed".into(), is_error: true }]
        }
    }
}
