use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub type TaskId = String;
pub type ProjectId = String;
/// "claude" | "codex" | "gemini" | "opencode" | "cursor" | "qwen" | "custom:<id>"
pub type EnvId = String;
pub type ChatId = String;

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
    /// A07: the person continues the session in a terminal; the engine does not touch it.
    HandedOff,
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
    /// Kept in the file for old data, read by nobody (spec 15).
    pub reviewer: Option<String>,
    /// Kept in the file for old data, read by nobody (spec 15).
    pub fallback_agent: Option<String>,
    /// (name, command): extra checks run at the gates besides `check_cmd`.
    #[serde(default)]
    pub extra_checks: Vec<(String, String)>,
    #[serde(default)]
    pub run_cmd: Option<String>,
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
            extra_checks: vec![],
            run_cmd: None,
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
    /// What the gates ran at the last pass and how each ended (one entry per check).
    #[serde(default)]
    pub gates: Vec<GateResult>,
}
/// One check of the gates: the project's tests, build, linter, types (spec 142).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GateResult {
    pub name: String,
    pub passed: bool,
    /// Last ~60 lines of its output.
    pub tail: String,
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
    // v2: every field below reads with a default, so a v1 file opens as is
    #[serde(default)]
    pub chat_id: Option<ChatId>,
    /// Model per role, a snapshot of the env settings when the task was created (spec 16).
    #[serde(default)]
    pub models: BTreeMap<RoleKind, ModelChoice>,
    #[serde(default)]
    pub quality: QualityPlan,
    #[serde(default)]
    pub swarm: Option<SwarmSpec>,
    #[serde(default)]
    pub steps: Vec<(Step, StepState)>,
    #[serde(default)]
    pub runs: Vec<RoleRun>,
    #[serde(default)]
    pub start: StartWhen,
    #[serde(default)]
    pub convoy_id: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<TaskId>,
    #[serde(default)]
    pub rendered_prompt: Option<String>,
    #[serde(default)]
    pub cost_usd: Option<f64>,
    #[serde(default)]
    pub budget_usd: Option<f64>,
}

impl Default for Task {
    /// A blank queued task (empty ids): fill what you need, `..Task::default()` the rest.
    fn default() -> Self {
        Task {
            id: String::new(),
            project_id: String::new(),
            title: String::new(),
            prompt: String::new(),
            agent: default_agent(),
            status: Status::Queued,
            branch: String::new(),
            worktree: None,
            session: None,
            attempts: 0,
            review_rounds: 0,
            question: None,
            resume_at: None,
            outcome: Outcome::default(),
            created_at: Local::now(),
            started_at: None,
            finished_at: None,
            chat_id: None,
            models: BTreeMap::new(),
            quality: QualityPlan::default(),
            swarm: None,
            steps: vec![],
            runs: vec![],
            start: StartWhen::Now,
            convoy_id: None,
            depends_on: vec![],
            rendered_prompt: None,
            cost_usd: None,
            budget_usd: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LogEvent {
    pub at: DateTime<Local>,
    pub kind: LogKind,
    pub text: String,
    #[serde(default)]
    pub role: Option<RoleKind>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub variant: Option<String>,
}

impl LogEvent {
    pub fn new(at: DateTime<Local>, kind: LogKind, text: &str) -> LogEvent {
        LogEvent { at, kind, text: text.to_string(), role: None, model: None, variant: None }
    }
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

/// Settings. `#[serde(default)]` on the struct: any missing key takes the spec 29 default,
/// so the app works without ever opening the settings screen (G16.1).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// Agent runs per env (R19i.5; spec 32), not tasks overall.
    pub max_parallel: u32,
    pub paused: bool,
    pub language: String,
    pub hotkey: String,
    /// Kept in the file, read by nobody (G03: the Telegram bot is gone).
    pub telegram: TelegramSettings,
    pub custom_agents: Vec<CustomAgent>,
    pub default_project: Option<ProjectId>,
    pub envs: BTreeMap<EnvId, EnvSettings>,
    pub quality: QualityPlan,
    /// What the swarm switch proposes when turned on (the swarm itself is off by default).
    pub swarm_default: SwarmSpec,
    pub limits: LimitSettings,
    pub notify: NotifySettings,
    pub look: LookSettings,
    pub keys: KeySettings,
    pub keep: KeepSettings,
    /// Silence on the log for this long = stalled.
    pub stall_min: u32,
    /// A swarm picks the winner itself after this long.
    pub autopick_min: u32,
    /// Failed check rounds before a task fails.
    pub attempts: u32,
    pub review_rounds: u32,
    pub keep_awake: bool,
}
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TelegramSettings {
    pub enabled: bool,
    pub chat_id: Option<i64>,
    pub proxy: Option<String>,
    pub digest_at: Option<String>,
}

/// Budgets and the night window (spec 29).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct LimitSettings {
    pub task_usd: f64,
    pub day_usd: f64,
    /// Warn at this share of a budget or limit window, percent.
    pub warn_pct: u8,
    pub night_from: String,
    pub night_to: String,
    /// "Away" after this many minutes without input.
    pub away_min: u32,
}
impl Default for LimitSettings {
    fn default() -> Self {
        LimitSettings { task_usd: 5.0, day_usd: 40.0, warn_pct: 80, night_from: "23:00".into(), night_to: "08:00".into(), away_min: 20 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct NotifySettings {
    /// Sound for a question, a failure, an outward request.
    pub sound: String,
    pub on_done: bool,
    pub quiet_from: String,
    pub quiet_to: String,
}
impl Default for NotifySettings {
    fn default() -> Self {
        NotifySettings { sound: "soft_click".into(), on_done: false, quiet_from: "23:00".into(), quiet_to: "08:00".into() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct LookSettings {
    /// "system" | "light" | "dark"
    pub theme: String,
    pub font_px: u32,
    pub role_labels: bool,
}
impl Default for LookSettings {
    fn default() -> Self {
        LookSettings { theme: "system".into(), font_px: 13, role_labels: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct KeySettings {
    pub new_chat: String,
    /// `{n}` = 1..7
    pub switch_env: String,
    pub send: String,
    pub freeze: String,
}
impl Default for KeySettings {
    fn default() -> Self {
        let mac = cfg!(target_os = "macos");
        let pick = |m: &str, l: &str| if mac { m } else { l }.to_string();
        KeySettings {
            new_chat: pick("Alt+Cmd+N", "Ctrl+Alt+N"),
            switch_env: pick("Cmd+{n}", "Ctrl+{n}"),
            send: "Enter".into(),
            freeze: pick("Alt+Cmd+.", "Ctrl+Alt+."),
        }
    }
}

/// Retention (spec 29, "Хранение").
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct KeepSettings {
    pub archive_days: u32,
    pub log_events: u32,
    pub chat_records: u32,
    /// "While you were away" digest after this many minutes of absence.
    pub summary_after_min: u32,
}
impl Default for KeepSettings {
    fn default() -> Self {
        KeepSettings { archive_days: 7, log_events: 500, chat_records: 200, summary_after_min: 30 }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_parallel: 3,
            paused: false,
            language: "auto".to_string(),
            hotkey: if cfg!(target_os = "macos") { "Alt+Space" } else { "Ctrl+Alt+Space" }.to_string(),
            telegram: TelegramSettings::default(),
            custom_agents: vec![],
            default_project: None,
            envs: BTreeMap::new(),
            quality: QualityPlan::default(),
            swarm_default: SwarmSpec::default(),
            limits: LimitSettings::default(),
            notify: NotifySettings::default(),
            look: LookSettings::default(),
            keys: KeySettings::default(),
            keep: KeepSettings::default(),
            stall_min: 15,
            autopick_min: 10,
            attempts: 3,
            review_rounds: 3,
            keep_awake: true,
        }
    }
}

// ---- v2 types (spec "Типы, которые добавляются") ----

/// `version: None` = "latest".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModelChoice {
    pub family: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ModelFamily {
    pub id: String,
    pub label: String,
    /// Picks the icon.
    pub vendor: String,
    #[serde(default)]
    pub blurb: Option<String>,
    /// Newest first. Empty = only "latest" is known (no versions invented).
    #[serde(default)]
    pub versions: Vec<String>,
    /// The one marked "default" in the env's list.
    #[serde(default)]
    pub default: bool,
    /// version -> the exact identifier the CLI takes (from the env's list or the vendor docs).
    /// A pinned version without an entry here is never started: no name is made up.
    #[serde(default)]
    pub exact: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CatalogSource {
    CliList,
    BuiltIn,
    Snapshot,
    Manual,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Catalog {
    pub env: EnvId,
    pub families: Vec<ModelFamily>,
    pub source: CatalogSource,
    pub at: DateTime<Local>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ResolvedModel {
    /// None = no flag (CLI default).
    pub cli_arg: Option<String>,
    pub label: String,
    pub note: Option<ModelNote>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelNote {
    /// "newer exists": the newest version of the family.
    Newer(String),
    Replaced { from: String, to: String },
    Rejected { tried: String },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RoleKind {
    Planner,
    Critic,
    TestAuthor,
    Worker,
    Reviewer,
    Verifier,
    Integrator,
    Coordinator,
    Watcher,
    Council,
    Chat,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Plan,
    Spec,
    Critique,
    TestsFirst,
    Code,
    Gates,
    E2e,
    Review,
    Merge,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    Pending,
    Running,
    Done,
    Skipped(String),
    Failed(String),
}

/// Spec 29: on; spec only for vague tasks; plan + critic; tests first; e2e off; best-of-2 for risky tasks.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct QualityPlan {
    pub on: bool,
    pub spec: bool,
    pub plan: bool,
    pub tests_first: bool,
    pub e2e: bool,
    pub best_of_n: Option<u8>,
}
impl Default for QualityPlan {
    fn default() -> Self {
        QualityPlan { on: true, spec: true, plan: true, tests_first: true, e2e: false, best_of_n: Some(2) }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SwarmMode {
    Race,
    Crew,
    Council,
}

/// `n` in 2..=5.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SwarmSpec {
    pub mode: SwarmMode,
    pub n: u8,
}
impl Default for SwarmSpec {
    fn default() -> Self {
        SwarmSpec { mode: SwarmMode::Race, n: 3 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RoleRun {
    pub role: RoleKind,
    #[serde(default)]
    pub variant: Option<String>,
    pub model: String,
    #[serde(default)]
    pub session: Option<String>,
    pub state: StepState,
    #[serde(default)]
    pub started_at: Option<DateTime<Local>>,
    #[serde(default)]
    pub finished_at: Option<DateTime<Local>>,
    #[serde(default)]
    pub cost_usd: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StartWhen {
    #[default]
    Now,
    Night,
    AfterMerge { left: u8 },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutwardChoice {
    Push,
    PushAndPr,
    PushOnly,
    No,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LimitWindow {
    pub name: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub used_pct: Option<f32>,
    #[serde(default)]
    pub remaining: Option<String>,
    #[serde(default)]
    pub resets_at: Option<DateTime<Local>>,
    pub exhausted: bool,
    /// Where the number came from, e.g. "rate_limit_event".
    pub source: String,
    pub seen_at: DateTime<Local>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EnvLimits {
    Known(Vec<LimitWindow>),
    /// The env reports nothing; the string says why. No estimates (spec 22).
    NoData(String),
}

/// Per-env settings, `Settings.envs[env]`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct EnvSettings {
    pub default_model: Option<ModelChoice>,
    pub roles: BTreeMap<RoleKind, ModelChoice>,
    pub fallback: Option<ModelChoice>,
    pub custom_models: Vec<String>,
    pub cli_path: Option<PathBuf>,
    /// Passed to the CLI as separate arguments, never through a shell.
    pub extra_args: Vec<String>,
    pub ask_push: bool,
    pub ask_pr: bool,
    pub protected_paths: bool,
    pub last_quality: Option<QualityPlan>,
    pub last_swarm: Option<SwarmSpec>,
    pub last_start: StartWhen,
}
impl Default for EnvSettings {
    fn default() -> Self {
        EnvSettings {
            default_model: None,
            roles: BTreeMap::new(),
            fallback: None,
            custom_models: vec![],
            cli_path: None,
            extra_args: vec![],
            ask_push: true,
            ask_pr: true,
            protected_paths: true,
            last_quality: None,
            last_swarm: None,
            last_start: StartWhen::Now,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Chat {
    pub id: ChatId,
    pub env: EnvId,
    pub project_id: ProjectId,
    pub title: String,
    #[serde(default)]
    pub session: Option<String>,
    #[serde(default)]
    pub convoy: bool,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Who {
    Human,
    Model,
    System,
    Role(RoleKind),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChatMsg {
    /// Made in the window; the idempotency key of `Store::append_chat_msg`.
    pub id: String,
    pub chat_id: ChatId,
    pub at: DateTime<Local>,
    pub who: Who,
    pub text: String,
    #[serde(default)]
    pub task_id: Option<TaskId>,
    #[serde(default)]
    pub delivered_at: Option<DateTime<Local>>,
    // v2, from the review of task 14: what the window shows next to the text
    /// `task` (a task was made of it), `error`, `warn`.
    #[serde(default)]
    pub kind: Option<ChatKind>,
    /// A folded tool action of the model.
    #[serde(default)]
    pub tool: Option<ToolCall>,
    /// Model and run number, e.g. "Opus 5.5 · run 2".
    #[serde(default)]
    pub model: Option<String>,
    /// The message this one answers.
    #[serde(default)]
    pub reply_to: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatKind {
    Task,
    Error,
    Warn,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub name: String,
    pub arg: String,
    pub result: String,
    pub ok: bool,
}
