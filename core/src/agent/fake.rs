//! Scripted adapter for tests: each invocation of `command()` consumes the next script.
//! The script runs as a real `sh` process (writes files into the run's cwd and prints one JSON
//! line per event), so `run_blocking`, cancel and the git worktree behave as with a real CLI.
use super::{Adapter, AgentEvent, Run};
use crate::model::{LimitWindow, ModelFamily, RoleKind};
use std::collections::HashMap;
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
    /// Limit windows the env reports during the run.
    Usage(Vec<LimitWindow>),
    /// Cost of the run in USD.
    Cost(f64),
    /// The CLI refuses the model; the text is what it said.
    RejectModel(String),
    /// The CLI is not logged in.
    Auth(String),
    /// v3: an action event as it is (start or result).
    Action(crate::model::ToolAction),
    /// v3: a piece of streamed text.
    Delta(String),
}

/// Which runs a role script answers: a role, and optionally one swarm variant.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    role: RoleKind,
    variant: Option<String>,
}

pub struct FakeAdapter {
    id: String,
    scripts: Vec<Vec<FakeStep>>,
    /// Per (role, variant) queues, consumed in order by the runs that match.
    by_role: HashMap<Key, Vec<Vec<FakeStep>>>,
    models: Option<Vec<ModelFamily>>,
    read_only_native: bool,
    calls: Mutex<Vec<Run>>,
    /// How many runs each queue has served, and how many went to `scripts`.
    served: Mutex<(HashMap<Key, usize>, usize)>,
}

impl FakeAdapter {
    /// One script per invocation; when exhausted, further invocations just say "ok".
    pub fn new(scripts: Vec<Vec<FakeStep>>) -> FakeAdapter {
        FakeAdapter {
            id: "fake".into(),
            scripts,
            by_role: HashMap::new(),
            models: None,
            read_only_native: false,
            calls: Mutex::new(vec![]),
            served: Mutex::new((HashMap::new(), 0)),
        }
    }
    /// Scripts for runs of `role`, in order. They win over the plain list given to `new`.
    pub fn with_role(mut self, role: RoleKind, scripts: Vec<Vec<FakeStep>>) -> FakeAdapter {
        self.by_role.insert(Key { role, variant: None }, scripts);
        self
    }
    /// Scripts for one swarm variant of `role`; a run belongs to the variant when its `model` equals
    /// `variant` or a component of its cwd does. Wins over `with_role`. Order of runs across variants
    /// does not matter, each variant has its own queue.
    pub fn with_variant(mut self, role: RoleKind, variant: &str, scripts: Vec<Vec<FakeStep>>) -> FakeAdapter {
        self.by_role.insert(Key { role, variant: Some(variant.to_string()) }, scripts);
        self
    }
    /// What `models()` returns (default: Err, like Claude, Gemini and Qwen).
    pub fn with_models(mut self, models: Vec<ModelFamily>) -> FakeAdapter {
        self.models = Some(models);
        self
    }
    pub fn with_read_only_native(mut self, yes: bool) -> FakeAdapter {
        self.read_only_native = yes;
        self
    }
    fn script_for(&self, run: &Run) -> Vec<FakeStep> {
        let in_cwd = |v: &str| run.cwd.components().any(|c| c.as_os_str() == v);
        let variant = self
            .by_role
            .keys()
            .find(|k| k.role == run.role && k.variant.as_ref().is_some_and(|v| run.model.as_deref() == Some(v) || in_cwd(v)))
            .cloned();
        let key = variant.or_else(|| {
            let k = Key { role: run.role, variant: None };
            self.by_role.contains_key(&k).then_some(k)
        });
        let mut served = self.served.lock().unwrap();
        let step = match key {
            Some(k) => {
                let n = served.0.entry(k.clone()).or_insert(0);
                *n += 1;
                self.by_role[&k].get(*n - 1).cloned()
            }
            None => {
                served.1 += 1;
                self.scripts.get(served.1 - 1).cloned()
            }
        };
        step.unwrap_or_else(|| vec![FakeStep::Done("ok".into())])
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
        let steps = self.script_for(run);
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
                FakeStep::Usage(w) => sh.push_str(&ev(serde_json::json!({"t":"usage","windows":w}))),
                FakeStep::Cost(c) => sh.push_str(&ev(serde_json::json!({"t":"cost","usd":c}))),
                FakeStep::RejectModel(m) => sh.push_str(&ev(serde_json::json!({"t":"reject_model","text":m}))),
                FakeStep::Auth(m) => sh.push_str(&ev(serde_json::json!({"t":"auth","text":m}))),
                FakeStep::Sleep(ms) => sh.push_str(&format!("sleep {}\n", ms as f64 / 1000.0)),
                FakeStep::Action(a) => sh.push_str(&ev(serde_json::json!({"t":"action","action":a}))),
                FakeStep::Delta(t) => sh.push_str(&ev(serde_json::json!({"t":"delta","text":t}))),
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
            Some("usage") => serde_json::from_value(v["windows"].clone()).map(AgentEvent::Usage).into_iter().collect(),
            Some("cost") => v["usd"].as_f64().map(AgentEvent::Cost).into_iter().collect(),
            Some("reject_model") => vec![AgentEvent::ModelRejected(s("text"))],
            Some("auth") => vec![AgentEvent::AuthRequired(s("text"))],
            Some("action") => serde_json::from_value(v["action"].clone()).map(AgentEvent::Action).into_iter().collect(),
            Some("delta") => vec![AgentEvent::TextDelta(s("text"))],
            _ => vec![],
        }
    }
    fn models(&self) -> Result<Vec<ModelFamily>, String> {
        self.models.clone().ok_or_else(|| "no model list command".to_string())
    }
    fn read_only_native(&self) -> bool {
        self.read_only_native
    }
    fn reports_cost(&self) -> bool {
        true
    }
    fn finish(&self, exit_ok: bool, _all_text: &str) -> Vec<AgentEvent> {
        if exit_ok {
            vec![]
        } else {
            vec![AgentEvent::Done { session: None, text: "fake agent failed".into(), is_error: true }]
        }
    }
}
