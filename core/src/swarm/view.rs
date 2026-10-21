//! What the window shows of a swarm (stories 156, 160): the shape `ui/js/screens/swarm.js` reads as
//! `Task.swarm_view`. Built by `Engine::swarm_view`; the app puts it on the task it sends.
use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Criterion {
    pub key: String,
    pub value: u8,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Gate {
    pub passed: u32,
    pub total: u32,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Stats {
    pub ins: u32,
    pub del: u32,
    pub minutes: Option<u32>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct FileView {
    pub path: String,
    pub insertions: u32,
    pub deletions: u32,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct VariantView {
    pub id: String,
    pub model: String,
    /// `queued` | `running` | `passed` | `failed`
    pub state: String,
    pub gate: Option<Gate>,
    /// Sum of the four marks divided by four.
    pub score: Option<f32>,
    pub summary: Option<String>,
    pub criteria: Vec<Criterion>,
    /// The reviewer's words on the marks ("Почему такой балл").
    pub why: Option<String>,
    pub stats: Option<Stats>,
    pub cost_usd: Option<f64>,
    pub why_failed: Option<String>,
    pub files: Vec<FileView>,
    /// The copy of the variant (the app opens its diffs from here); it lives until the task is removed.
    pub worktree: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AdviceView {
    pub variant: String,
    pub model: Option<String>,
    pub text: String,
    /// When the app takes the recommended variant itself.
    pub autopick_at: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct PartView {
    pub id: String,
    pub title: String,
    pub files: Vec<String>,
    pub worker: String,
    pub model: String,
    /// `waiting` | `running` | `done` | `failed`
    pub state: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ProposalView {
    pub id: String,
    pub model: String,
    pub text: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct VoteView {
    pub voter: String,
    #[serde(rename = "for")]
    pub for_: String,
    pub why: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct CouncilView {
    pub proposals: Vec<ProposalView>,
    pub votes: Vec<VoteView>,
    pub winner: Option<String>,
    /// `votes`, or `reviewer` when the vote was a tie.
    pub decided_by: Option<String>,
    /// The votes on the finished work (the council reviews too).
    pub review_votes: Vec<VoteView>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct SwarmView {
    pub variants: Vec<VariantView>,
    pub advice: Option<AdviceView>,
    pub picked: Option<String>,
    /// Who picked: `person` or `timer`.
    pub picked_by: Option<String>,
    pub stopped: bool,
    pub parts: Vec<PartView>,
    pub council: Option<CouncilView>,
}
