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
    /// None = whoever writes next (the executor); in a swarm (`all`) every agent.
    pub to: Option<RoleKind>,
    pub text: String,
    pub at: DateTime<Local>,
    pub delivered: bool,
    /// A swarm's variant (or part, or voter) the note is for: "A", "B" ... (G11.4).
    #[serde(default)]
    pub variant: Option<String>,
    /// "To everyone" of a swarm: each agent gets it once, with its next run.
    #[serde(default)]
    pub all: bool,
    /// Whom an `all` note reached: `role` or `role/variant`.
    #[serde(default)]
    pub delivered_to: Vec<String>,
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
    /// What the swarm decided for the executor (the council's approach, a crew part's brief): added to its prompt.
    #[serde(default)]
    pub worker_hint: Option<String>,
    /// The swarm's own bookkeeping (variants, parts, votes).
    #[serde(default)]
    pub swarm: SwarmRt,
    /// `prompt_not_rewritten: quality_off` was written for this task.
    #[serde(default)]
    pub quality_off_noted: bool,
    /// The Watcher nudged the role in this call and waits for the second silence (G08.2).
    #[serde(default)]
    pub nudged: bool,
}

/// Where a lane (a variant, a part of a crew) stands.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(super) enum LaneState {
    /// Waits for a seat.
    #[default]
    Waiting,
    Running,
    /// Race: gates green. Crew: landed in the task's branch.
    Passed,
    Failed,
    /// Stopped by a limit, a login or a freeze: starts again where it was.
    Interrupted,
    /// Its agent asked the person (`NEEDS_HUMAN`): it waits for the answer, then goes on in the same session.
    Asking,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(super) struct Variant {
    pub id: String,
    pub model: ModelChoice,
    pub branch: String,
    pub worktree: PathBuf,
    pub session: Option<String>,
    pub pin: Option<Pin>,
    pub state: LaneState,
    /// Failed gate rounds of this variant.
    pub attempts: u32,
    /// What the executor is told next (the output of red gates).
    pub next_prompt: Option<String>,
    pub gates: Vec<GateResult>,
    pub green: u32,
    pub total: u32,
    pub summary: Option<String>,
    /// correctness, coverage, simplicity, risk.
    pub scores: Option<[u8; 4]>,
    /// The reviewer's words on the marks.
    pub why: Option<String>,
    pub why_failed: Option<String>,
    pub started_at: Option<DateTime<Local>>,
    pub finished_at: Option<DateTime<Local>>,
    /// path, insertions, deletions
    pub files: Vec<(String, u32, u32)>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(super) struct Advice {
    pub variant: String,
    pub text: String,
    pub autopick_at: DateTime<Local>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(super) struct PartRt {
    pub id: String,
    pub title: String,
    pub files: Vec<String>,
    pub brief: String,
    pub model: ModelChoice,
    pub branch: String,
    pub worktree: PathBuf,
    pub session: Option<String>,
    pub pin: Option<Pin>,
    pub state: LaneState,
    pub attempts: u32,
    pub next_prompt: Option<String>,
    pub why_failed: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(super) struct Member {
    pub id: String,
    pub model: ModelChoice,
    pub session: Option<String>,
    pub pin: Option<Pin>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(super) struct VoteRec {
    pub voter: String,
    pub choice: String,
    pub why: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(super) struct Proposal {
    pub id: String,
    pub model: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub(super) struct CouncilRt {
    pub members: Vec<Member>,
    pub proposals: Vec<Proposal>,
    pub votes: Vec<VoteRec>,
    pub winner: Option<String>,
    /// "votes" or "reviewer" (a tie).
    pub decided_by: Option<String>,
    pub decided: bool,
    /// The votes on the last review.
    pub review_votes: Vec<VoteRec>,
    /// Members that gave nothing in a phase (`propose:A`, `vote:B`): they are not asked again.
    #[serde(default)]
    pub out: Vec<String>,
}

/// A question of a lane that has not reached the person yet.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub(super) struct Asked {
    pub label: String,
    /// `Variant`, `Part` or `Member`.
    pub kind: String,
    pub question: String,
    pub options: Vec<String>,
    pub session: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub(super) struct SwarmRt {
    /// Questions of lanes waiting their turn to be put to the person.
    pub asking: Vec<Asked>,
    /// The lane whose question the person is answering now.
    pub asked_lane: Option<String>,
    pub variants: Vec<Variant>,
    pub advice: Option<Advice>,
    pub picked: Option<String>,
    /// `person` or `timer`.
    pub picked_by: Option<String>,
    pub stopped: bool,
    /// The swarm did not get there: the task goes on (`resume_failed`) as a single executor in the best copy.
    pub collapsed: bool,
    pub parts: Vec<PartRt>,
    pub split_tries: u8,
    pub sequential: bool,
    pub landed_all: bool,
    pub council: CouncilRt,
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
        self.write_state(id, &s)?;
        Ok(r)
    }

    /// Under `state_lock` only.
    fn write_state(&self, id: &str, s: &TaskState) -> Result<(), String> {
        let path = self.state_path(id);
        let tmp = path.with_extension("tmp");
        let err = |e: std::io::Error| format!("pending_write_failed: {e}");
        std::fs::create_dir_all(path.parent().unwrap_or(&self.worktrees_dir)).map_err(err)?;
        std::fs::write(&tmp, serde_json::to_vec(s).map_err(|e| e.to_string())?).map_err(err)?;
        std::fs::rename(&tmp, &path).map_err(err)
    }

    pub(super) fn clear_state(&self, id: &str) {
        let _g = lk(&self.state_lock);
        let _ = std::fs::remove_file(self.state_path(id));
    }

    /// A finished task keeps only the record of its swarm (its copies live until the task is removed, G11.5), in one
    /// critical section: a reader never sees an empty state between a clear and a rewrite.
    pub(super) fn keep_only_swarm(&self, id: &str) {
        let _g = lk(&self.state_lock);
        let sw = self.read_state(id).swarm;
        if sw.variants.is_empty() && sw.parts.is_empty() && sw.council.proposals.is_empty() {
            let _ = std::fs::remove_file(self.state_path(id));
        } else {
            let _ = self.write_state(id, &TaskState { swarm: sw, ..TaskState::default() });
        }
    }
}
