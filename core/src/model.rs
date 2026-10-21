use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type TaskId = String;
pub type ProjectId = String;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queued,
    Preparing,
    Working,
    Checking,
    Reviewing,
    Merging,
    NeedsHuman,
    PausedLimit,
    AwaitingOutward,
    MergeBlocked,
    Done,
    DoneNotPushed,
    Failed,
    Cancelled,
    Reverted,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Finish {
    Local,
    Push,
    Pr,
}

fn default_copy_files() -> Vec<String> {
    vec![".env*".to_string()]
}
fn default_true() -> bool {
    true
}
fn default_agent() -> String {
    "claude".to_string()
}
fn default_finish() -> Finish {
    Finish::Local
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub path: PathBuf,
    pub base_branch: String,
    pub check_cmd: Option<String>,
    pub setup_cmd: Option<String>,
    #[serde(default = "default_copy_files")]
    pub copy_files: Vec<String>,
    #[serde(default = "default_finish")]
    pub finish: Finish,
    #[serde(default = "default_true")]
    pub ask_outward: bool,
    #[serde(default = "default_agent")]
    pub agent: String,
    pub reviewer: Option<String>,
    pub fallback_agent: Option<String>,
}

impl Project {
    /// New project with spec defaults.
    pub fn new(name: &str, path: PathBuf, base_branch: &str) -> Project {
        Project {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            path,
            base_branch: base_branch.to_string(),
            check_cmd: None,
            setup_cmd: None,
            copy_files: default_copy_files(),
            finish: Finish::Local,
            ask_outward: true,
            agent: default_agent(),
            reviewer: None,
            fallback_agent: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Question {
    pub text: String,
    pub options: Vec<String>,
    pub kind: QuestionKind,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum QuestionKind {
    Agent,
    Outward,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Outcome {
    pub summary: Option<String>,
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub tests: Option<TestResult>,
    pub review: Option<Review>,
    pub commit: Option<String>,
    pub pr_url: Option<String>,
    pub error: Option<String>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TestResult {
    pub passed: bool,
    pub tail: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Review {
    pub approved: bool,
    pub issues: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Task {
    pub id: TaskId,
    pub project_id: ProjectId,
    pub title: String,
    pub prompt: String,
    pub agent: String,
    pub status: Status,
    pub branch: String,
    pub worktree: Option<PathBuf>,
    pub session: Option<String>,
    pub attempts: u32,
    pub review_rounds: u32,
    pub question: Option<Question>,
    pub resume_at: Option<DateTime<Local>>,
    pub outcome: Outcome,
    pub created_at: DateTime<Local>,
    pub started_at: Option<DateTime<Local>>,
    pub finished_at: Option<DateTime<Local>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LogEvent {
    pub at: DateTime<Local>,
    pub kind: LogKind,
    pub text: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum LogKind {
    Stage,
    Agent,
    Tool,
    Check,
    Review,
    Git,
    Error,
    Human,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CustomAgent {
    pub id: String,
    pub name: String,
    pub run: String,
    pub resume: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    pub max_parallel: u32,
    pub paused: bool,
    pub language: String,
    pub hotkey: String,
    pub telegram: TelegramSettings,
    pub custom_agents: Vec<CustomAgent>,
    pub default_project: Option<ProjectId>,
}
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TelegramSettings {
    pub enabled: bool,
    pub chat_id: Option<i64>,
    pub proxy: Option<String>,
    pub digest_at: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_parallel: 2,
            paused: false,
            language: "auto".to_string(),
            hotkey: if cfg!(target_os = "macos") { "Alt+Space" } else { "Ctrl+Alt+Space" }.to_string(),
            telegram: TelegramSettings::default(),
            custom_agents: vec![],
            default_project: None,
        }
    }
}
