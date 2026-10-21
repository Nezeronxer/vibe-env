# Интерфейсы

## Правила проекта (для каждого исполнителя)

- **Стек:** Rust (stable, cargo 1.96) - workspace из двух крейтов: `core` (библиотека, без Tauri) и `app` (Tauri 2). Интерфейс - `ui/`: обычные HTML/CSS/JS без фреймворка и без сборщика, отдаётся Tauri как `frontendDist`.
- **Команды:** `cargo test -p nightshift-core` - тесты ядра; `cargo tauri dev` - запуск (из `app/`); `cargo tauri build` - сборка.
- **Зависимости ядра - только эти** (новую - только если без неё никак, и с одной строкой «почему» в отчёте): `serde`, `serde_json`, `chrono` (serde), `uuid` (v4, serde), `thiserror`, `regex`, `reqwest` (blocking, json, rustls-tls) - только в telegram, `keyring` - только в telegram, `tempfile` - dev. Асинхронщины нет: потоки `std::thread` + каналы `std::sync::mpsc`. Внешние процессы - `std::process::Command`.
- **git и gh** - системные бинари через `Command`, никакого libgit2.
- **Нельзя:** `git push --force`, `git reset --hard` по веткам пользователя, удалять ветки, не созданные приложением (`ns/*`), трогать рабочую копию человека иначе как `merge --ff-only` на чистой main.
- **Не установлена зависимость / нет инструмента** → верни `BLOCKED: <что нужно>`, не ставь глобально сам. Cargo-зависимости из списка выше - ставь (`cargo add`).
- **Язык:** идентификаторы и комментарии - английский; строки для человека - через словарь `ui/i18n.js` (ru/en). Сообщения ядра человеку (причины ошибок, итоги) - короткие английские ключи + параметры, UI переводит; в Telegram - по языку из настроек.
- Каталог данных: передаётся в `Store::open(dir)`; в приложении - `app_data_dir()` Tauri. Тесты - только во временных каталогах.

## Границы, решённые в спецификации

| Модуль | Владеет | Выставляет | Прячет |
|---|---|---|---|
| `core::model` | общие типы | всё из блока «Типы» ниже | - |
| `core::store` | проекты, задачи, настройки на диске | `Store::open(dir) -> Result<Store>`; `projects()`, `upsert_project(Project)`, `remove_project(id)`, `tasks()`, `task(id)`, `upsert_task(&Task)`, `append_log(id, &LogEvent)`, `read_log(id, limit) -> Vec<LogEvent>`, `settings()`, `set_settings(Settings)` | формат файлов, атомарная запись (tmp + rename) |
| `core::git` | все git/gh-операции | см. «git» ниже | команды, разбор вывода, правила продвижения main |
| `core::agent` | запуск движков и разбор вывода | `trait Adapter`, `Run`, `AgentEvent`, `adapters(&Settings)`, `FakeAdapter` | флаги CLI, форматы событий, распознавание лимита |
| `core::engine` | жизненный цикл задач | `Engine` (ниже) | машина состояний, промпты, `NEEDS_HUMAN`, повторы, мьютекс вливания |
| `core::notify` | что и куда сообщить | `trait Notifier`, `Notice`, `Fanout` | - |
| `core::telegram` | бот | `Telegram::start(...)`, реализует `Notifier` | Bot API, long polling, соответствие сообщение↔задача |
| `app` | окно, трей, хоткей, системные уведомления | Tauri-команды (ниже), события `task-changed`, `task-log` | - |
| `ui` | экраны | - | - |

### Типы (`core::model`)

```rust
pub type TaskId = String;   // uuid v4
pub type ProjectId = String;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queued, Preparing, Working, Checking, Reviewing, Merging,
    NeedsHuman, PausedLimit, AwaitingOutward, MergeBlocked,
    Done, DoneNotPushed, Failed, Cancelled, Reverted,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Finish { Local, Push, Pr }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub path: PathBuf,
    pub base_branch: String,          // detected, e.g. "main"
    pub check_cmd: Option<String>,    // None = no tests in project
    pub setup_cmd: Option<String>,
    pub copy_files: Vec<String>,      // globs, default [".env*"]
    pub finish: Finish,               // default Local
    pub ask_outward: bool,            // default true
    pub agent: String,                // adapter id, default "claude"
    pub reviewer: Option<String>,     // None = same as agent
    pub fallback_agent: Option<String>, // A05: continue here on limit; None = wait for reset
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Question { pub text: String, pub options: Vec<String>, pub kind: QuestionKind }
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum QuestionKind { Agent, Outward }  // Outward = "push/PR?" yes/no

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Outcome {
    pub summary: Option<String>,      // worker's final text, trimmed
    pub files_changed: u32, pub insertions: u32, pub deletions: u32,
    pub tests: Option<TestResult>,    // None = no check_cmd
    pub review: Option<Review>,
    pub commit: Option<String>,       // sha on base
    pub pr_url: Option<String>,
    pub error: Option<String>,        // why failed / not pushed / blocked
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TestResult { pub passed: bool, pub tail: String }   // tail = last ~60 lines
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Review { pub approved: bool, pub issues: Vec<String> }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Task {
    pub id: TaskId,
    pub project_id: ProjectId,
    pub title: String,                // first line of prompt, ≤80 chars
    pub prompt: String,
    pub agent: String,
    pub status: Status,
    pub branch: String,               // "ns/<short-id>-<slug>"
    pub worktree: Option<PathBuf>,
    pub session: Option<String>,      // worker session id for resume
    pub attempts: u32,                // failed check rounds
    pub review_rounds: u32,
    pub question: Option<Question>,
    pub resume_at: Option<DateTime<Local>>,  // PausedLimit / MergeBlocked retry
    pub outcome: Outcome,
    pub created_at: DateTime<Local>,
    pub started_at: Option<DateTime<Local>>,
    pub finished_at: Option<DateTime<Local>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LogEvent { pub at: DateTime<Local>, pub kind: LogKind, pub text: String }
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum LogKind { Stage, Agent, Tool, Check, Review, Git, Error, Human }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CustomAgent { pub id: String, pub name: String, pub run: String, pub resume: Option<String> }
// run/resume - shell templates with {prompt} {cwd} {session}; plain-text output, exit 0 = ok

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    pub max_parallel: u32,            // default 2
    pub paused: bool,
    pub language: String,             // "auto" | "ru" | "en"
    pub hotkey: String,               // default "Alt+Space" mac / "Ctrl+Alt+Space" linux
    pub telegram: TelegramSettings,
    pub custom_agents: Vec<CustomAgent>,
    pub default_project: Option<ProjectId>,
}
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TelegramSettings {
    pub enabled: bool,
    pub chat_id: Option<i64>,         // set by pairing
    pub proxy: Option<String>,        // None = HTTPS_PROXY env
    pub digest_at: Option<String>,    // "08:30" or None = off
}
// The bot token is NOT here: keyring service "nightshift", user "telegram-bot-token".
```

### `core::git`

```rust
pub struct RepoInfo { pub is_repo: bool, pub base_branch: Option<String>, pub current_branch: Option<String>,
                      pub dirty: bool, pub origin_url: Option<String>, pub ahead: u32, pub behind: u32 }
pub struct FileStat { pub path: String, pub insertions: u32, pub deletions: u32 }
pub enum GitError { Conflict(String), AdvanceBlocked(String) /* dirty main */, Push(String), Gh(String), Other(String) }

pub fn repo_info(repo: &Path) -> RepoInfo;
pub fn init_repo(path: &Path) -> Result<(), GitError>;                       // git init + initial commit
pub fn create_worktree(repo: &Path, worktrees_dir: &Path, task_id: &str, branch: &str, base: &str) -> Result<PathBuf, GitError>;
pub fn remove_worktree(repo: &Path, wt: &Path, delete_branch: bool) -> Result<(), GitError>;
pub fn commit_all(wt: &Path, msg: &str) -> Result<Option<String>, GitError>; // None = nothing to commit
pub fn rebase_onto(wt: &Path, base: &str) -> Result<(), GitError>;           // Conflict leaves rebase in progress for the agent to resolve
pub fn rebase_continue(wt: &Path) -> Result<(), GitError>;
pub fn diff(wt: &Path, base: &str, max_bytes: usize) -> String;
pub fn diffstat(wt: &Path, base: &str) -> Vec<FileStat>;
pub fn squash_to_base(repo: &Path, wt: &Path, base: &str, msg: &str) -> Result<String, GitError>; // returns new base sha; 3 advance rules from spec §8
pub fn revert(repo: &Path, base: &str, sha: &str) -> Result<String, GitError>;
pub fn push(repo: &Path, branch: &str) -> Result<(), GitError>;               // never --force
pub fn fetch_ff(repo: &Path, base: &str) -> Result<(), GitError>;
pub fn open_pr(repo: &Path, branch: &str, base: &str, title: &str, body: &str) -> Result<String, GitError>; // gh pr create → url
pub fn gh_status(repo: &Path) -> Result<String, GitError>;   // "user" or Err(hint)
pub fn pr_state(repo: &Path, url: &str) -> Result<String, GitError>; // open|merged|closed
pub fn list_ns_branches(repo: &Path) -> Vec<(String, bool /*merged*/)>;
pub fn delete_merged_ns_branches(repo: &Path) -> Result<u32, GitError>;
pub fn detect_commands(repo: &Path) -> (Option<String> /*check*/, Option<String> /*setup*/);
```

### `core::agent`

```rust
#[derive(Clone, Debug, PartialEq)] pub enum Role { Worker, Reviewer }
pub struct Run { pub cwd: PathBuf, pub prompt: String, pub system: String, pub resume: Option<String>, pub role: Role }

#[derive(Clone, Debug, PartialEq)]
pub enum AgentEvent {
    Text(String),
    Tool { name: String, summary: String },
    Done { session: Option<String>, text: String, is_error: bool },
    Limit { resets_at: Option<DateTime<Local>> },
}

pub trait Adapter: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn available(&self) -> Result<(), String>;     // Err = "not installed: npm i -g …"
    fn command(&self, run: &Run) -> Command;       // fully built, cwd set, stdout piped
    fn parse(&self, line: &str) -> Vec<AgentEvent>;
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> { vec![] } // for plain-text adapters: emit Done/Limit at EOF
}
pub fn adapters(settings: &Settings) -> Vec<Arc<dyn Adapter>>;     // claude, codex, gemini, opencode, cursor, qwen + custom
pub fn run_blocking(a: &dyn Adapter, run: &Run, on_event: &mut dyn FnMut(&AgentEvent), cancel: &AtomicBool) -> std::io::Result<Vec<AgentEvent>>;
pub struct FakeAdapter { /* scripted: Vec<Vec<FakeStep>> per invocation */ }
pub enum FakeStep { Write(String /*rel path*/, String /*content*/), Say(String), Done(String), Limit, Fail }
```

`run_blocking` spawns `command()`, feeds every stdout line to `parse`, calls `finish` at EOF, kills the child when `cancel` is set. The worker system rules (no push, `NEEDS_HUMAN:` format) are passed in `Run.system`; each adapter puts them where its CLI accepts them.

### `core::engine`

```rust
pub enum EngineEvent { TaskChanged(Task), Log(TaskId, LogEvent), ProjectChanged(ProjectId) }
pub struct NewTask { pub project_id: ProjectId, pub prompt: String, pub agent: Option<String> }

impl Engine {
    pub fn new(store: Store, adapters: Vec<Arc<dyn Adapter>>, notifier: Arc<dyn Notifier>, worktrees_dir: PathBuf) -> Arc<Engine>;
    pub fn start(self: &Arc<Self>);                         // scheduler thread; resumes interrupted tasks
    pub fn subscribe(&self) -> Receiver<EngineEvent>;
    pub fn submit(&self, t: NewTask) -> Result<TaskId, String>;
    pub fn answer(&self, id: &str, text: &str) -> Result<(), String>;     // NeedsHuman(Agent) only
    pub fn approve_outward(&self, id: &str, yes: bool) -> Result<(), String>;
    pub fn cancel(&self, id: &str) -> Result<(), String>;
    pub fn retry(&self, id: &str) -> Result<(), String>;
    pub fn remove(&self, id: &str) -> Result<(), String>;
    pub fn undo(&self, id: &str) -> Result<(), String>;
    pub fn push_base(&self, project_id: &str) -> Result<(), String>;
    pub fn pull_base(&self, project_id: &str) -> Result<(), String>;
    pub fn pause_all(&self, paused: bool);
    pub fn store(&self) -> &Mutex<Store>;                   // read access for UI listing
}
```

`Engine` реализует для Telegram trait `core::telegram::Control { submit, answer, approve_outward, status_text, digest_text }` - объявлен в `core::telegram`, impl - в `core::engine` (тикет 03 пишет impl по этой сигнатуре).

### `core::notify`

```rust
#[derive(Clone, Debug, PartialEq)] pub enum NoticeKind { Question, Failed, Outward, Done, Limit }
#[derive(Clone, Debug)] pub struct Notice { pub kind: NoticeKind, pub task_id: TaskId, pub project: String, pub title: String, pub body: String, pub options: Vec<String> }
pub trait Notifier: Send + Sync { fn send(&self, n: &Notice); }
pub struct Fanout(pub Vec<Arc<dyn Notifier>>);   // impl Notifier
pub struct NullNotifier; pub struct RecordingNotifier(pub Mutex<Vec<Notice>>); // for tests
```

Звук - у `Question`, `Failed`, `Outward`; `Done`, `Limit` - тихо.

### `core::telegram`

```rust
pub trait Control: Send + Sync {
    fn submit(&self, project_hint: Option<&str>, prompt: &str) -> Result<String, String>;  // returns title
    fn answer(&self, task_id: &str, text: &str) -> Result<(), String>;
    fn approve_outward(&self, task_id: &str, yes: bool) -> Result<(), String>;
    fn status_text(&self) -> String;
    fn digest_text(&self) -> String;
}
pub struct Telegram;  // Telegram::start(settings: TelegramSettings, token: Option<String>, control: Arc<dyn Control>, on_paired: Box<dyn Fn(i64)+Send>) -> Arc<TelegramHandle>
// TelegramHandle: impl Notifier; fn pairing_code(&self) -> String; fn status(&self) -> String; fn restart(&self, settings, token)
pub fn load_token() -> Option<String>; pub fn save_token(t: &str) -> Result<(), String>;   // keyring, fallback file 0600
```

### Tauri-команды (`app`) - для `ui`

`list_projects`, `add_project(path)`, `update_project(project)`, `remove_project(id)`, `init_repo(path)`, `repo_info(id)`,
`list_tasks`, `get_log(id, limit)`, `submit_task(project_id, prompt, agent?)`, `answer(id, text)`, `approve_outward(id, yes)`,
`cancel(id)`, `retry(id)`, `remove_task(id)`, `undo(id)`, `push_base(project_id)`, `pull_base(project_id)`, `pause_all(paused)`,
`list_agents -> [{id, name, available: bool, hint}]`, `get_settings`, `set_settings(settings)`, `set_tg_token(token)`, `tg_status -> {status, pairing_code}`,
`github_info(projectId) -> {ghUser|ghHint, repoUrl, branches:[{name, merged}]}`, `delete_merged_branches(projectId)`, `pr_state(taskId)`,
`open_path(path)`, `open_editor(path)`, `open_terminal(path)`.
События: `task-changed` (Task), `task-log` ({id, event}), `project-changed` (id). Имена аргументов - camelCase на стороне JS (правило Tauri).

## Швы для тестов

1. **`Engine` + `FakeAdapter` + временный git-репозиторий** - главный: истории 2, 4-9, 17-22, 24, 28, 30, 34.
2. **`Adapter::parse` / `finish`** на образцах реального вывода в `core/tests/fixtures/`.
3. **`git`** на временных репозиториях: три правила продвижения main, конфликт, revert.

## Что построили таски

(дописывается по мере готовности тасков)
