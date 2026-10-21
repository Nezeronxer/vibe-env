//! What the engine keeps for a task besides `Task` itself, on disk next to the answer of the
//! person (`worktrees_dir/.pending/`): the breakdown, the notes of the person, the files of the
//! test author, the model pinned to a session. All of it can be recomputed or asked again if the
//! file is lost, none of it is a source of truth for statuses (those live in `Task`).
use super::{lk, Engine};
use crate::model::*;
use crate::prompt::Brief;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A note of the person for the next run of its addressee (G05).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(super) struct Note {
    pub id: String,
    /// None = whoever writes next (the executor).
    pub to: Option<RoleKind>,
    pub text: String,
    pub at: DateTime<Local>,
    pub delivered: bool,
}

/// The model a session was started on: a resumed session stays on it (G18.5).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(super) struct Pin {
    pub cli_arg: Option<String>,
    pub label: String,
    /// `Vendor` name (`crate::prompt`): a resumed run keeps the template of the model it started on.
    pub vendor: String,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub(super) struct TaskState {
    /// The Planner's breakdown (with the person's words in `original`).
    pub brief: Option<Brief>,
    /// Why the prompt is not rewritten (G14.2); None = it is, or it was not tried.
    pub brief_error: Option<String>,
    /// Files the test author wrote, and the commit that holds them.
    pub test_files: Vec<String>,
    pub tests_commit: Option<String>,
    /// The role that asked the person a question and the session to resume with the answer.
    pub asked: Option<RoleKind>,
    pub asked_session: Option<String>,
    pub notes: Vec<Note>,
    pub pinned: BTreeMap<RoleKind, Pin>,
    /// The agent deferred the task (`DEFERRED:`): it waits in the queue with its copy.
    pub deferred: bool,
    /// Integrator runs spent on conflicts of the current merge.
    pub integrator_rounds: u32,
    /// "env does not report cost" was logged for this task.
    pub cost_noted: bool,
    /// A question the engine asked itself (budget, login): its key. None = a role's own question.
    pub special: Option<String>,
    /// The worker starts a new session in the same copy: its prompt is "continue what was begun".
    pub continue_next: bool,
}

impl Engine {
    fn state_path(&self, id: &str) -> PathBuf {
        let safe: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        self.worktrees_dir.join(".pending").join(format!("{safe}.state.json"))
    }

    pub(super) fn state(&self, id: &str) -> TaskState {
        let _g = lk(&self.state_lock);
        self.read_state(id)
    }

    fn read_state(&self, id: &str) -> TaskState {
        std::fs::read(self.state_path(id)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    /// Read, change, write, in one critical section: the person's `note` comes from another thread.
    pub(super) fn update_state<R>(&self, id: &str, f: impl FnOnce(&mut TaskState) -> R) -> Result<R, String> {
        let _g = lk(&self.state_lock);
        let mut s = self.read_state(id);
        let r = f(&mut s);
        let path = self.state_path(id);
        let tmp = path.with_extension("tmp");
        let err = |e: std::io::Error| format!("pending_write_failed: {e}");
        std::fs::create_dir_all(path.parent().unwrap_or(&self.worktrees_dir)).map_err(err)?;
        std::fs::write(&tmp, serde_json::to_vec(&s).map_err(|e| e.to_string())?).map_err(err)?;
        std::fs::rename(&tmp, &path).map_err(err)?;
        Ok(r)
    }

    pub(super) fn clear_state(&self, id: &str) {
        let _g = lk(&self.state_lock);
        let _ = std::fs::remove_file(self.state_path(id));
    }
}
