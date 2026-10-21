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

~~`Engine` реализует для Telegram trait `core::telegram::Control`~~ - снято: Telegram-бот убран пользователем (G03).

### `core::notify`

```rust
#[derive(Clone, Debug, PartialEq)] pub enum NoticeKind { Question, Failed, Outward, Done, Limit }
#[derive(Clone, Debug)] pub struct Notice { pub kind: NoticeKind, pub task_id: TaskId, pub project: String, pub title: String, pub body: String, pub options: Vec<String> }
pub trait Notifier: Send + Sync { fn send(&self, n: &Notice); }
pub struct Fanout(pub Vec<Arc<dyn Notifier>>);   // impl Notifier
pub struct NullNotifier; pub struct RecordingNotifier(pub Mutex<Vec<Notice>>); // for tests
```

Звук - у `Question`, `Failed`, `Outward`; `Done`, `Limit` - тихо.

### `core::telegram` - СНЯТ (G03, модуль удалён), блок ниже только для истории

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
`list_agents -> [{id, name, available: bool, hint}]`, `get_settings`, `set_settings(settings)`,
`github_info(projectId) -> {ghUser|ghHint, repoUrl, branches:[{name, merged}]}`, `delete_merged_branches(projectId)`, `pr_state(taskId)`,
`open_path(path)`, `open_editor(path)`, `open_terminal(path)`.
События: `task-changed` (Task), `task-log` ({id, event}), `project-changed` (id). Имена аргументов - camelCase на стороне JS (правило Tauri).

## Швы для тестов

1. **`Engine` + `FakeAdapter` + временный git-репозиторий** - главный: истории 2, 4-9, 17-22, 24, 28, 30, 34.
2. **`Adapter::parse` / `finish`** на образцах реального вывода в `core/tests/fixtures/`.
3. **`git`** на временных репозиториях: три правила продвижения main, конфликт, revert.

## Границы v2, решённые в спецификации (копия из spec.md)

Существующие модули остаются со своими сигнатурами (`interfaces.md` - закон); v2 их только расширяет.
Добавление варианта в перечисление или поля в структуру ломает сопоставления и литералы внутри
репозитория - их правит тот же таск, что добавляет. Новые поля данных читаются с умолчанием.

| Модуль | Владеет | Выставляет | Прячет |
|---|---|---|---|
| `core::model` | общие типы | всё из «Типы» v1 + блок «Типы, которые добавляются» выше | - |
| `core::store` | проекты, задачи, чаты, настройки, снимки каталога и лимитов на диске | v1: `Store::open`, `projects`, `project`, `upsert_project`, `remove_project`, `tasks`, `task`, `upsert_task`, `remove_task`, `append_log`, `read_log`, `settings`, `set_settings`; + `chats(env) -> Vec<Chat>`, `chat(id)`, `upsert_chat(&Chat)`, `remove_chat(id)`, `append_chat_msg(&ChatMsg) -> Result<bool, StoreError>` (false = такой id уже есть), `read_chat(id, before: Option<DateTime<Local>>, limit) -> Vec<ChatMsg>`, `catalog(env) -> Option<Catalog>`, `set_catalog(Catalog)`, `limits(env) -> Option<EnvLimits>`, `set_limits(env, EnvLimits)` | формат файлов, атомарную запись, чтение файлов v1, копию испорченного файла настроек |
| `core::git` | все git/gh-операции | v1 целиком; + `create_readonly_copy(repo, dir, base) -> Result<PathBuf, GitError>` (worktree без ветки), `refresh_readonly_copy(wt, base)` (к свежей base, неотслеживаемое удаляется - только в копиях приложения), `diff_file(wt, base, path, max_lines) -> FileDiff`, `changed_paths(wt, from, to) -> Vec<String>`, `restore_paths(wt, from, paths)`, `ff_branch(repo, branch, to) -> Result<(), GitError>` (только вперёд, для частей бригады), `clone_shallow(url, dest) -> Result<(), GitError>` (только `https://`, `ssh://`, `git@`, локальный путь) | команды, разбор вывода, правила продвижения main |
| `core::agent` | запуск сред и разбор их вывода | v1: `trait Adapter`, `Run`, `AgentEvent`, `adapters(&Settings)`, `run_blocking`, `run_lines`, `FakeAdapter`; `Role` → все `RoleKind`; `Run` + `model: Option<String>`, `read_only: bool`, `extra_env: Vec<(String, String)>`, `json_schema: Option<String>`; `AgentEvent` + `Usage(Vec<LimitWindow>)`, `Cost(f64)`, `ModelRejected(String)`, `AuthRequired(String)`; `Adapter` + `models() -> Result<Vec<ModelFamily>, String>`, `builtin_models() -> Vec<ModelFamily>`, `interactive_resume(session, cwd) -> Option<Command>`, `read_only_native() -> bool`, `reports_cost() -> bool`, `install_hint() -> Option<String>`, `sandbox_info() -> SandboxInfo` (все - с поведением по умолчанию); `FakeAdapter` + сценарии по роли и варианту, `with_models`, `FakeStep::{Usage, Cost, RejectModel, Auth}` | флаги CLI, форматы событий, распознавание лимита, входа и отказа модели, встроенные подсказки моделей |
| `core::models` *(новый)* | каталог моделей и версий среды | `refresh(env, &dyn Adapter, &Store) -> Catalog`, `catalog(env, &dyn Adapter, &Store) -> Catalog` (снимок, иначе подсказка), `resolve(&Catalog, &ModelChoice) -> ResolvedModel`, `next_on_reject(&Catalog, &ModelChoice) -> Option<ModelChoice>` (87), `stronger_than(&Catalog, &ModelChoice) -> Option<ModelChoice>` (171) | порядок источников, правило «ближайшей», «есть новее» |
| `core::prompt` *(новый)* | разбор задачи и промпты ролей | `brief_request(task_text, project_hint) -> String`, `parse_brief(&str) -> Result<Brief, String>`, `render(vendor, RoleKind, &Brief, &RoleInputs) -> String`, `fenced(label, text) -> String` (переезжает из движка); `Brief { goal, constraints, done_when, files, vague, trivial, risky: Option<String>, plan, original }` | шаблоны вендоров, системные добавки ролей, схемы JSON-ответов |
| `core::swarm` *(новый)* | правила роя | `assign_models(&Catalog, worker, n) -> Vec<ModelChoice>`, `check_split(&[Part]) -> Result<(), Overlap>`, `tally(&[Vote]) -> Tally`, `rank(&[VariantResult]) -> Vec<VariantId>`, `best_failed(&[VariantResult]) -> Option<VariantId>` | формулы отбора и подсчёта голосов; только чистые функции, без процессов |
| `core::engine` | жизненный цикл задач: роли, шаги, рой, поручения, Надзиратель, места, лимиты, заморозка | v1 целиком (`new`, `new_with_clock`, `start`, `shutdown`, `subscribe`, `submit`, `answer`, `approve_outward`, `cancel`, `retry`, `remove`, `undo`, `push_base`, `pull_base`, `pause_all`, `store`); `NewTask` + `chat_id`, `quality`, `swarm`, `start`, `model` (с `Default`); + `note(id, to: Option<String>, text)`, `answer_outward(id, OutwardChoice)`, `resume_failed(id, hint: Option<String>, model: Option<ModelChoice>)`, `pick_variant(id, variant)`, `stop_swarm(id)`, `nudge(id)`, `switch_model(id, ModelChoice)`, `freeze(bool)`, `hand_off(id) -> Result<HandOff, String>`, `take_back(id)`, `start_now(id)`, `not_a_task(id)`, `submit_convoy(project_id, env, text) -> Result<String, String>`, `set_presence(away: bool)`, `limits(env) -> EnvLimits`, `summary_since(at) -> Summary`; `EngineEvent` + `LimitsChanged(EnvId)`, `CatalogChanged(EnvId)`, `Frozen(bool)` | машину состояний, порядок шагов, разбор сигналов агента, повторы, таймеры (Надзиратель, автовыбор, ночь), мьютекс вливания, места на среду, состояние «между ходами» |
| `core::chat` *(новый)* | переписка с моделью среды | `Chats::new(Arc<Engine>)`, `send(ChatTarget::{Existing(ChatId), New{env, project_id}}, msg_id, text) -> Result<ChatId, String>`, `list(env) -> Vec<ChatSummary>`, `messages(id, before, limit) -> Vec<ChatMsg>`, `retry(id, msg_id)`, `remove(id)`, `subscribe() -> Receiver<ChatEvent>` | копию чата, сессию чата, распознавание задачи (словарь глаголов + `NS_TASK`), пересказ при потерянной сессии, архив |
| `core::kits` *(новый)* | наборы и секреты сред | `Kits::new(home, data_dir)`, `inventory(env, scope) -> Inventory`, `export(env, &Selection, ExportDest) -> Result<ExportReport, KitError>`, `preview(env, KitSource, scope) -> Result<ImportPlan, KitError>`, `install(&ImportPlan, &Choices) -> Result<SnapshotId, KitError>`, `enable_item(env, item)`, `snapshots(env)`, `rollback(SnapshotId)`, `recover()`, `check_update(&Installed) -> Result<ImportPlan, KitError>`, `set_secret(env, name, value)`, `env_vars(env) -> Vec<(String, String)>` (→ `Run.extra_env`) | раскладки сред (§26), `kit.json`, поиск секретов, журнал и снимки, связку ключей, проверки архивов, путей и симлинков |
| `core::notify` | что и куда сообщить | без изменений: `Notifier`, `Notice`, `NoticeKind`, `Fanout`, `NullNotifier`, `RecordingNotifier`; «нужен вход» и «бюджет» - `Question` | - |
| `core::telegram` | - | **снят (G03)**, модуль удалён | - |
| `app` (Tauri) | окно, трей, клавиши, уведомления, ОС | команды v1 + `list_envs`, `get_catalog(env)`, `refresh_catalog(env)`, `get_limits(env)`, `list_chats(env)`, `get_chat(id, before, limit)`, `send_chat(target, msgId, text)`, `retry_chat(id, msgId)`, `remove_chat(id)`, `note(taskId, to, text)`, `answer_outward(taskId, choice)`, `resume_failed(taskId, hint, model)`, `pick_variant`, `stop_swarm`, `nudge`, `switch_model`, `freeze(on)`, `hand_off(taskId)`, `take_back(taskId)`, `start_now`, `not_a_task`, `submit_convoy`, `summary_since(at)`, `diff_file(taskId, path)`, `check_custom(id)`, `install_env(env)`, `login_env(env)`, `kits_*` (inventory, export, preview, install, enable, snapshots, rollback, check_update), `set_secret(env, name, value)`; события v1 + `chat-msg`, `limits-changed`, `catalog-changed`, `frozen` | присутствие и блокировку экрана, запрет сна, один экземпляр и `--new-chat`, запуск терминала, тихие часы и склейку уведомлений |
| `ui` | экраны по макету | - | разметку, `logic.js` (группировка, приоритет статусов, формат лимитов - чистые функции) |

**Швы для тестов** - как в v1, плюс один неизбежный:

1. **`Engine` (+ `Chats` поверх него) + `FakeAdapter` + временный git-репозиторий + часы-заглушка** - главный. Сценарии фейка - по роли и варианту (в рое порядок запусков недетерминирован). Через него: 2, 4-9, 17-22, 24, 28, 30, 34, 34a, 62-66, 69-71, 79-82, 87-88, 98-100, 105-107, 109-110, 112, 117, 119, 139, 141-143, 147-151, 153-159, 163-165, 170, 172-173, 177-178, 187.
2. **`Adapter::parse` / `finish` / `models`** на сохранённых образцах настоящего вывода (в т.ч. `rate_limit_event` Claude, `opencode models`, ошибки входа и отказа модели): 31, 34, 74, 84, 97.
3. **`git` на временных репозиториях**: 19-22, 109 (копия только для чтения), 141 (`restore_paths`), 154 (`ff_branch`).
4. **`Kits` на временном домашнем каталоге и временном каталоге данных** *(новый)*: 204-216. Неизбежен: наборы работают с файлами CLI, движок в них не участвует.

`models`, `prompt`, `swarm` - чистые функции, проверяются через шов 1 (и своими юнит-тестами, но это не шов: их сигнатуры не обещаются UI). Интерфейс - `selftest.html` для `logic.js` + ручная проверка по макету.

## Что построили таски

(дописывается по мере готовности тасков)

### Из таска 01 - каркас

- Тесты ядра: `cargo test -p nightshift-core` (файлы `core/tests/{store,git,gh,agent}.rs`); один файл - `--test git`. Сборка окна: `cargo build -p nightshift-app` (tauri-cli не нужен).
- `Store`: мутации возвращают `Result<_, StoreError>`; `append_log(&self, …)`; `read_log(id, limit: usize) -> Vec<LogEvent>`; есть `remove_task(id)`, `project(id)`.
- `model`: `Project::new(name, path, base)`, `Default for Settings`.
- `git`: `dirty` - только отслеживаемые файлы. `open_pr` сам пушит ветку, затем `gh pr create`. `NIGHTSHIFT_GH` подменяет бинарь gh (для тестов). Влитая squash-ем ветка помечается ref `refs/ns-merged/<branch>`. `squash_to_base`, если main сдвинулась после rebase → `GitError::Other("... moved since rebase")` - движок должен сделать rebase заново.
- `agent`: `FakeAdapter::new(scripts).with_id(id)`, `.calls() -> Vec<Run>` (что передавалось), `FakeStep::Sleep(ms)`; fake запускается как `sh -c`, печатает JSON-строки. `Run`, `AgentEvent`: Clone/Debug. `adapters()` пока пустой - заполняет таск 02.
- `git`: `RepoInfo.web_url: Option<String>` - https-ссылка из origin (git@/https/ssh → https://host/o/r, без логина/токена; локальный путь → None). Запасная подпись Nightshift - только если в самом репо нет user.name/email.
- `agent::run_blocking`: cancel убивает всю группу процессов (unix); stderr дочитывается через join - фоновый процесс с открытым stderr задержит возврат.
- `notify`: `NoticeKind::is_loud()`.
- `telegram`: `TelegramHandle` (пока пустой Notifier), `load_token` → None, `save_token` → Err - реализует таск 04.

### Из таска 06 - интерфейс

- `ui/`: `index.html`, `style.css`, `app.js`, `i18n.js` (ru/en), `logic.js` (группировка, архив, счётчики - чистые функции), `dev-mock.js` (только без `window.__TAURI__`), `selftest.html` (14 проверок logic.js).
- UI вызывает ровно команды из списка выше с camelCase-аргументами + слушает `tauri://drag-drop/-enter/-leave`.
- «Повторить push» у `done_not_pushed` → `push_base(projectId)`; ссылки открываются через `open_path(url)`; не-git папка распознаётся по ошибке `not a git repo` от `add_project`.

### Из таска 02 - адаптеры

- `agent::adapters`: `Claude`, `Codex`, `Gemini`, `OpenCode`, `Cursor`, `Qwen` (`new()`, `with_bin(bin)`), `Custom(CustomAgent)`. `adapters(&Settings)` - 6 встроенных + custom, дубли id отброшены.
- `Adapter::guarded() -> bool` - true только у Claude (песочница + технический запрет push/remote/sudo). Для остальных приложение показывает предупреждение.
- `Claude::network_note() -> Option<String>` - почему агенту закрыта сеть (версия < 2.1.186 или не прочиталась). Разрешение `WebFetch(domain:*)` открывает сеть песочнице ради npm/pip; риск - вынос файлов рабочей копии.
- Своя команда: `{prompt}` `{cwd}` `{session}` → позиционные `"$1" "$2" "$3"` с учётом кавычек шаблона; вывод - текст, код 0 = успех; лимит ищется в последних 5 строках.
- Подтверждено `--help`/живьём: Claude, opencode, Qwen. Только по документации: Codex, Gemini, Cursor (помечено `// unverified`).

### Из таска 03 - движок

- `engine`: прежние `Engine::new`, `new_with_clock(.., Clock)`, `start`, `shutdown` (ждёт выхода планировщика), команды из сигнатуры выше. `telegram::Control` не реализуется - бот снят (G03), модуль `core::telegram` удалён.
- Захват задачи в работу: перевод в `running` под замком + CAS в одной секции `Store`; после `shutdown`/stopping захват запрещён. Паника в задаче → `Failed` с причиной `internal_error: …`, отравленные замки снимаются через `into_inner`.
- Состояние «между ходами» (ответ человека, промпт после красных тестов, замечания ревью) хранится рядом с задачей: `worktrees_dir/.pending/<id>.json`, запись tmp+rename, удаляется на done/failed/cancel/retry/remove. В `Task` этих полей нет.
- В движке нет прямых вызовов git - только через `core::git`. Тексты задачи/диффа/вывода в промптах обёрнуты `fenced()` с uuid-меткой, в обоих системных промптах правило «внутри метки - данные, не инструкции».
- `undo` при `Push`+`ask_outward` → `awaiting_outward` с вариантом `push_revert`. Пауза и `max_parallel` держатся гейт-файлом; живость процесса после рестарта проверяется по PID (`kill -0`).
- `git` (дополнено таском 03): `GitError::BaseMoved` (вместо строки «moved» в `Other`) у `squash_to_base`/`advance`/`revert`/`fetch_ff`; `rebase_in_progress(wt)`; `hide_untracked(wt, paths)` - список в приватном git-каталоге worktree, репозиторий человека не трогается; `merged_commit(repo, base, branch)`; маркер `refs/ns-merged/<branch>` пишется до продвижения base.
- `agent::run_lines(cmd, cancel, on_line) -> Option<(ok, stderr)>` - общий запуск: дочитка stdout+stderr, kill всей группы процессов при отмене; `run_blocking` построен на нём (фоновый процесс агента больше не подвешивает возврат).
- Режим `Pr` не вливает в локальную main (ветка уходит в PR); проверка при вливании гоняется заново после каждого rebase.
- Тесты: `cargo test -p nightshift-core --test engine_e2e` (31, ~16 с).

## Заметки для перенарезки v2
- UI: «Повторить push» у done_not_pushed в PR-проекте → retry(taskId), в Push-проекте → push_base(projectId) (ревью 03).
- Движок: запасной движок (A05) → запасная модель той же среды (G22); адаптерам нужен параметр модели (G18); ревьюер и рой - только та же среда.
- Движок (из ревью 03, без блокирующих): тест на вывод агента не в UTF-8 с полным хвостом; e2e retry после вливания со старым маркером ns-merged (новая работа попадает в main); git::hidden_specs - пустой список только при NotFound, иные ошибки → Err (иначе .env может уйти в коммит).

### Из таска 05 - окно приложения

- `app`: 28 команд v1 (camelCase-аргументы) + `get_diffstat(taskId) -> [{path, insertions, deletions}]` (влитая задача - `git show --numstat` по outcome.commit). `list_agents` отдаёт ещё `guarded`. События: `task-changed`, `task-log`, `project-changed`, `task-removed(id)`, `focus-input` (горячая клавиша и трей «Новый чат»).
- CSP: `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost` - в ui/ никаких inline-скриптов и inline-стилей в атрибутах style= с JS-вставкой. capabilities: `core:default` + `dialog:allow-open`; уведомления и клавиша - из Rust.
- Смена `custom_agents` пересоздаёт Engine (адаптеры фиксируются при `Engine::new`). `fix_path()` при старте добавляет PATH логин-шелла и каталоги brew - иначе из GUI не находятся gh и CLI сред.
- Трей: заголовок-счётчик `2 ▶ 1 ?` на маке, подсказка везде, заголовок окна `(N) Nightshift`; закрытие прячет окно, на маке без Dock (ActivationPolicy::Accessory). Тесты: `cargo test -p nightshift-app`. Бандл: `cargo tauri build` из `app/` → `target/release/bundle/` (.app, .dmg; deb/appimage - цели в конфиге, собираются на Linux).
- Движок v2 (из ревью 05): `Engine::set_adapters(Vec<Arc<dyn Adapter>>)` - смена своих команд без остановки задач; публичная `engine::is_active(&TaskStatus)` для счётчика трея.

### Из таска 08 - ядро v2 (типы, хранилище, адаптеры, каталог, git)

- `model`: весь блок «Типы, которые добавляются»; `Task::default()` (пустые id, `Queued`) - заполнять нужное и `..Task::default()`; `LogEvent::new(at, kind, text)`; все новые поля читаются с умолчанием (`Settings` целиком `#[serde(default)]`). Новые структуры настроек (в спецификации только имена полей): `LimitSettings {task_usd 5, day_usd 40, warn_pct 80, night_from "23:00", night_to "08:00", away_min 20}`, `NotifySettings {sound "soft_click", on_done false, quiet_from/to}`, `LookSettings {theme "system", font_px 13, role_labels true}`, `KeySettings {new_chat, switch_env "{n}", send, freeze}`, `KeepSettings {archive_days 7, log_events 500, chat_records 200, summary_after_min 30}`; `Settings.swarm_default: SwarmSpec` (Race, 3 - что предлагает переключатель; сам рой выключен: `Task.swarm = None`). `max_parallel` по умолчанию 3. `Project` + `extra_checks`, `run_cmd`. `Status::HandedOff`. `RoleKind` - `Copy + Ord + Hash` (ключ `BTreeMap`).
- `store`: `Store::settings_recovered() -> Option<&Path>` - копия испорченного блока настроек, сделанная при `open` (`state.json.corrupt-<время>`); задачи/проекты/чаты остаются, настройки - умолчания. Файл, не читаемый как JSON, умолчаниями не заменяется: `open` возвращает `StoreError::Unreadable { copy }` (копия рядом, сам файл не тронут) - UI показывает «данные не прочитаны» и путь. `chats(env)`, `chat(id)`, `upsert_chat(&Chat)`, `remove_chat(id)` (вместе с лентой), `append_chat_msg(&ChatMsg) -> Result<bool>` (`false` = id в этом чате уже есть), `read_chat(id, before: Option<DateTime<Local>>, limit) -> Vec<ChatMsg>` (старые первыми; следующая страница - `before` = `at` первой записи), `catalog(env)`, `set_catalog(Catalog)`, `limits(env)`, `set_limits(env, EnvLimits)` (файлы `catalog.json`, `limits.json`, ключ - среда; `&self`, не `&mut`).
- `agent`: `Role` теперь `type Role = RoleKind`. `Run` + `model`, `read_only`, `extra_env`, `json_schema`, **`max_budget_usd: Option<f64>`** (в спецификации поля не было; передаётся `--max-budget-usd` только Claude) и **`extra_args: Vec<String>`** (`EnvSettings.extra_args`, отдельными аргументами); `Run::new(cwd, prompt, system, role)` - всё остальное выключено; `Run::is_read_only()` = `read_only` или `Reviewer`. `AgentEvent` + `Usage`, `Cost`, `ModelRejected`, `AuthRequired` (последние два, как и `Limit`, заменяют итоговый `Done`). `Adapter` + `models()`, `builtin_models()`, `interactive_resume(session, cwd)`, `read_only_native()`, `reports_cost()`, `install_hint()`, `sandbox_info()`. `SandboxInfo {sandboxed, push_blocked, network_open, writes_outside_copy, note}` (в спецификации не описана; минимум по историям 201 и 219). `adapters(&Settings)` берёт `EnvSettings.cli_path` как путь к бинарю.
- Окна лимитов Claude: `LimitWindow.name` = `"five_hour"` / `"seven_day"`, `used_pct` в процентах, `source` = `"rate_limit_event"`. Статус, начинающийся с `allowed` (в т.ч. `allowed_warning`), не лимит - отступление от «не allowed», образца `allowed_warning` и значения блокирующего статуса нет, не подтверждено.
- `FakeAdapter`: `with_role(role, scripts)`, `with_variant(role, variant, scripts)` (вариант узнаётся по `Run.model` или по компоненту `cwd`), `with_models`, `with_read_only_native`; `FakeStep::{Usage(Vec<LimitWindow>), Cost(f64), RejectModel(String), Auth(String)}`; `reports_cost() == true`.
- `models`: `refresh(env, &dyn Adapter, &Store)` (команда списка → снимок; при сбое - снимок, затем встроенная подсказка), `catalog(env, &dyn Adapter, &Store)` (без запуска CLI: снимок, иначе подсказка), `resolve`, `next_on_reject`, `stronger_than`, `exact_name(family, version)`, `version_cmp`. В каждом каталоге первым идёт `ModelFamily { id: "default", vendor: "cli", default: true }` (resolve → без флага), в конце - `custom_models` с `vendor: "custom"`; обе подписи английские, переводит UI по `vendor`. Закреплённая версия = точный идентификатор из `ModelFamily.exact[version]` (новое поле, `version → id для CLI`: из списка среды как напечатала CLI, у встроенных - из документации вендора); `resolve` сам имён не строит. Нет версии в каталоге → ближайшая по правилу 91 (`Replaced`), у семейства без версий → его id (`Replaced { to: "latest" }`), семейства нет в каталоге → без флага и `Rejected { tried }`. `exact_name` удалена.
- `git`: `FileDiff {path, text, truncated, binary, size}`; `create_readonly_copy(repo, dir, base)` (`dir` - путь самой копии, её ещё нет), `refresh_readonly_copy(wt, base)` (отказывает, если `wt` не связанный worktree без ветки), `diff_file`, `changed_paths`, `restore_paths`, `ff_branch` (только `ns/*`), `clone_shallow` (локальный путь - только абсолютный существующий каталог).
- Встроенные списки Claude, Gemini, Qwen (`builtin_models`) содержат версии и идентификаторы из документации вендоров на 2026-10-21 (Claude Platform docs: models overview + Model IDs and versioning, правило `claude-{name}-{major}[-{minor}]`, `claude-opus-5-5` виден и в живом запуске; Gemini API models page; Qwen Code docs, таблица Coding Plan) с пометкой «не сверено с CLI» в `blurb`. Открытые места: Gemini 4 Argon на странице помечен «новая модель» и не внесён; список Qwen верен для ключа Coding Plan; `haiku`/`flash-lite` как алиасы CLI не проверены.
- Codex `--output-schema`: схема пишется в `<каталог данных>/nightshift/schemas/<uuid>.json` (0700, tmp+rename, старше суток удаляются); `Codex::with_schema_dir(dir)`; ошибка записи = запуск падает с `Done{is_error}` «cannot write the answer schema file».
- git: `refresh_readonly_copy` работает только с копиями, у которых есть метка `ns-readonly-copy` в приватном git-каталоге (ставит `create_readonly_copy`); `restore_paths` проверяет, что настоящий путь каждой папки внутри копии, до любых изменений; `diff_file` читает вывод git потоком и режет по `max_lines` и по 256 КиБ.

### Из таска 14 - интерфейс v2, часть 1 (контракт данных для таска 13)

- `ui/`: обычные `<script>` (не ES-модули - нужен `file://` для selftest), общий глобальный `NS`. Экран: свой файл в `js/screens/`, тег `<script>` перед `js/app.js`, `NS.registry.register({id, layout: 'chats'|'full'|'bare', mount(root, ctx)})`; тот же id заменяет заглушку. Заглушки ждут таск 15: `factory`, `settings`, `swarm`, `kits`, `import`, `summary`. Стили: `css/` (tokens, base, components, shell, chat, onboarding, need); янтарь `--human` - только need.css. Язык: `ctx.setLang(setting)`. Значки: `ui/icons/` + SOURCES.md.
- `js/core/api.js` - белый список: только команды v1 и строки `app` из «Границ v2», аргументы camelCase. События: task-changed, task-log, task-removed, project-changed, chat-msg, limits-changed, catalog-changed, frozen, focus-input, tauri drag-drop.
- **Формы JSON, которые ui ожидает (таск 13 сериализует ровно так; enum - snake_case):**
  - `list_envs` → `[{id, name, installed, needs_login?, error?, version, install_hint, kit_dir, custom, configured}]`
  - `ChatTarget` → `{existing: id}` | `{new: {env, project_id}}`
  - `EnvLimits` → `{known: [LimitWindow...]}` | `{no_data: "почему"}`
  - `FileDiff` → `{lines: [{kind: "add"|"del"|"hunk"|"ctx", text}], binary, size_kb}` (ядро отдаёт `{path, text, truncated, binary, size}` - app переводит)
  - «как в CLI по умолчанию» = `ModelChoice.family == "default"` (семейство id "default", vendor "cli")
  - `ChatMsg` + необязательные `kind` (task|error|warn), `tool {name, arg, result, ok}`, `model`, `reply_to`
  - `Task` + `rendered_prompt`, `runs`; `Outcome.gates`; сводка чата + `last`
- Нет команды: «Отложить» (сейчас только в браузере), «Нужен git» (распознаётся по тексту ошибки «git … not found»); A07.2 (чат в терминале) не сделан.

### Из таска 12 - наборы

- `Kits::new(home, data_dir)`, `.with_secret_store(Box<dyn SecretStore>)` (шов для тестов; по умолчанию системная связка ключей, крейт `keyring` 3.6 - разрешён для наборов, §33), `#[doc(hidden)] crash_after_writes(n)` - имитация сбоя.
- `inventory(env, &Scope) -> Inventory{env, env_name, scope, dir, found, groups:[GroupInfo{group, available, active, note ("layout_unverified" у несверенных), count, items}]}`.
- `export(env, &Selection{name, groups, scope}, ExportDest::{Folder, Zip, Git}) -> ExportReport{dest, files, env_vars, secrets_replaced, skipped, commit}`.
- `preview(env, KitSource::{Repo, Folder, Zip}, &Scope::{Global, Project(path)}) -> ImportPlan{id, env, kit_name, source, commit, scope, target_dir, entries:[PlanEntry{key, group, label, target, change: Add|Replace|Same, diff, held: Held{kind: Hook|Mcp|ScriptedSkill|Command, event, text, scripts:[{path, text}]}}], env_vars:[{name, used_by, missing}], skipped, project_files, uncommitted_in_project, update_from, kit_sha}`.
- `install(&ImportPlan, &Choices{skip}) -> SnapshotId(String)`; `enable_item(env, &HeldRef{snapshot, key})` - включает выключенный хук/MCP/«опасную» настройку по одному; `snapshots(env) -> Vec<SnapshotInfo{id, kit_name, source, commit, scope, at, state, held, project_files, changed}>`, `.installed()`; `rollback(id)`; `recover() -> Vec<Recovered{snapshot, env, kit_name, message}>` - вызывать при старте app; `check_update(&Installed)`; `set_secret -> SecretSaved::{Keyring, File{path, warning}}`; `env_vars(env)` → `Run.extra_env`.
- `KitError` (serde tag `kind`): NotAKit, BadZip, UnsafePath, Symlink, FileTooLarge, KitTooLarge, TooManyFiles, Corrupt, WrongEnv{kit_env, kit_env_name, target_env, target_env_name}, CloneFailed, UnknownEnv, DestNotEmpty, NotFound, Changed, BadName, Interrupted, Io. Тексты - ключами (`kit_install_interrupted_rolled_back`), перевод в ui.
- `kit.json = {format: 1, kind: "nightshift-kit", name, env, app_version, created_at, groups, files:[{path, sha256, size}], env_vars:[{name, used_by}]}`.
- Выключены при установке (включаются по одному через enable_item): хуки, MCP, скиллы со скриптами, ключи настроек, которые запускают программы или меняют права (env, statusLine, permissions, hooks, notify, sandbox…). UI показывает список выключенного.
- Несверенные группы (`layout_unverified`, ui подписывает «раскладка <среды> не сверена»): Cursor целиком, скиллы Codex/Gemini/Qwen, субагенты Codex; MCP Claude/opencode - на сверке.
- (после правок 12) MCP Claude: глобальные - `mcpServers` в `~/.claude.json` (в набор только этот ключ), проектные - `.mcp.json`; opencode - ключ `mcp` в opencode.json. Общие конфиги (~/.claude.json, .mcp.json, settings.json Claude/Gemini/Qwen, opencode.json, config.toml Codex) пишутся по ключам: перечитать → изменить только ключи набора → tmp+rename; откат - только ключи набора. `Inventory.dangerous`/`ExportReport.dangerous` - имена опасных ключей настроек; `skipped` с причиной `inline_form` (Codex `hooks = …`/`mcp_servers = …`). Снимки: 0700, 20 последних / 30 дней на среду. Клон набора: свой ограниченный (blob:limit=10m, BatchMode, тайм-аут 120 с).
- ВНИМАНИЕ для всех: глобальный gitignore пользователя игнорирует `*.log` - новые файлы-образцы с таким расширением добавлять `git add -f` (координатор) или называть иначе.

### Из таска 15 - интерфейс v2, часть 2 (контракт для таска 13)

- Экраны: `js/screens/{settings,settings-projects,kits,factory,swarm,notices,summary}.js`, стили `css/{lanes,swarm,factory,settings,kits,notices}.css`, словарь `i18n-v2.js`. Рой открывается по чату (`shell.js`), «Заморожено» - на краю.
- Команды (camelCase): `pick_variant{taskId, variant}`, `stop_swarm{taskId}`, `nudge{taskId}`, `switch_model{taskId, model}`, `submit_convoy{projectId, env, text} -> chatId`, `summary_since{at} -> {done, failed, waiting, spent, limits}`, `note{taskId, to: null|"A"|"reviewer", text}`, `get_log{id, limit}`, `kits_*{env, scope, …}`, `update_project{project}`, `push_base/pull_base/delete_merged_branches/github_info{projectId}`.
- **Сверх «Границ v2» (таск 13 добавляет в app):** команда `open_chat_terminal{id}` (A07.2); события `settings-replaced`, `notify-blocked`, `tray-state`, `kits-recovered`.
- Формы, которые ждёт ui: `Task.swarm_view{variants, advice, picked, stopped, parts, council}`, `RoleRun.variant`, `ChatMsg.to`, `list_envs[].sandbox{…}` и `.path`.
- **Наборы - ui переходит на формы ядра из «Из таска 12»** (`ImportPlan.entries/env_vars`, `held.kind/scripts[{path,text}]`, `Inventory.groups/dangerous`, `Scope::Global|Project(path)`, `KitSource`, `ExportDest`, `HeldRef{snapshot,key}`, `KitError.kind` → перевод ключом). app сериализует enum ядра как есть (serde по умолчанию), без перевода.
- Нет полей в ядре (таск 09/13): модели вариантов роя (G16.9), отметка «понятно» о песочнице (R17i.3), признак Wayland, переключатели уведомлений кроме «Готово».
- (после 12a) `rollback(id) -> RollbackReport{kept_changed: Vec<String> ("~/.claude/settings.json: model"), kept_files: Vec<String>}` - ui показывает, что откат оставил, потому что человек/CLI это поменяли. `SnapshotState::NotApplied` и `KitError::NotApplied{snapshot, cli, keys}` - «CLI перезаписала файл, закрой <cli> и повтори»; ui показывает это явно, не как успех. Файл набора удаляется при откате только если не тронут после установки.
- `diff_file{taskId, path, variant?}` - необязательный вариант роя (дифф копии варианта); app (таск 13) принимает.

### Из таска 09 - движок v2

- `NewTask` + `chat_id, quality, swarm, start, model` (Default). `EngineEvent` + `LimitsChanged, CatalogChanged, Frozen`.
- `Engine`: `set_adapters` (смена своих команд без остановки задач), `set_env_vars(EnvVars)` (секреты наборов → Run.extra_env), `note(id, to, text) -> NoteResult{Queued, Answered, NotDelivered(key)}`, `answer_outward`, `resume_failed`, `switch_model`, `snooze(id, until)` / `is_snoozed`, `freeze` / `is_frozen`, `hand_off -> HandOff{task_id, env, worktree, session, command: Option<Command>, resumed}`, `take_back`, `start_now`, `not_a_task -> Result<String>`, `set_presence(away)`, `limits(env)`, `refresh_catalog`, `summary_since -> Summary{since, done, failed, waiting, spent_usd, limits}`; `pub fn is_active(&TaskStatus)`.
- `model`: `ChatMsg` + `kind: ChatKind`, `tool: ToolCall`, `model`, `reply_to`; `Outcome.gates: Vec<GateResult{name, passed, tail}>`.
- `Adapter::set_private_dir` (каталог схем Codex из каталога данных). `git::try_hidden_committed`; `hidden_specs` → Err при ошибке чтения, кроме NotFound.
- `prompt`: `Vendor`, `Brief` (+spec), `RoleInputs`, `Conflict`, `render`, `render_simple`, `system_for`, `parse_signals`, `parse_brief/critique/verdict/verify/spec`, `fenced`. Источники шаблонов - `core/src/prompt/SOURCES.md` (Anthropic, OpenAI + Codex guide, Google Gemini, Alibaba Qwen; opencode/Cursor - по вендору модели).
- Ключи строк ленты (ui переводит): `model_replaced, model_rejected, model_not_in_catalog, switched_to, prompt_not_rewritten, tests_read_only, plan_skipped, budget_exceeded, day_budget_exceeded, auth_required, env_unavailable, waiting_limit_reset, hidden_list_unreadable, pr_not_opened, v1_setting_ignored`.
- Поведение: ревьюер - из среды задачи; кругов ревью 3; конфликт вливания решает Интегратор; лимит → запасная модель той же среды; повтор входа/недоступности раз в 60 с; «Добавить $5» на день и дневной расход среды - на диске (`Store::day_spend`, `day_spend.json`), retry и удаление задачи их не трогают; потолок запуска - меньшее из остатка задачи и остатка дня; при заморозке Интегратор останавливается и не стартует, git-часть вливания доводится. Строка ленты - `<ключ>: <подробности>`.
- Ожидание входа: задача помечена `state.special == auth`; метку снимают только успешный запуск, отмена и удаление (кнопка «Login» - нет); очередь среды закрыта, пока есть помеченная задача в активном статусе / NeedsHuman / PausedLimit / MergeBlocked. `switch_model` в NeedsHuman переключает спросившую роль (`state.asked`), её сессия сбрасывается. Под заморозкой каждый круг Интегратора ждёт разморозки с начатым rebase.
- G14 (решение пользователя): при выключенном «Качестве» исполнитель получает текст как есть, `rendered_prompt` пуст, в ленте строка `prompt_not_rewritten: quality_off`.
- Тесты: `core/tests/engine_v2*.rs`, `engine_prompt.rs`, общий harness `core/tests/common/` (`Routed` - сценарии фейка по роли), таймаут ожидания 90 с.
- Не сделано, таск 10: рой, Надзиратель, `submit_convoy`, `pick_variant`, `stop_swarm`, `nudge`.

### Из таска 11 - чат со средой

- `core::chat::Chats::new(engine, adapters: Vec<Arc<dyn Adapter>>, dir: PathBuf /* копии чатов, не worktrees движка */, clock: Clock) -> Arc<Chats>`; `set_adapters`, `set_env_vars(EnvVars)`, `shutdown()`. Таск 13 передаёт те же адаптеры и часы, что Engine, и зовёт `set_adapters` вместе с движком.
- `send(ChatTarget::{Existing(id), New{env, project_id}}, msg_id, text) -> Result<ChatId, String>` (JSON `{existing: id}` | `{new: {env, project_id}}`), `list(env) -> Vec<ChatSummary>`, `messages(id, before, limit)` (страницы по 200), `retry(id, msg_id)`, `remove(id)`, `subscribe() -> Receiver<ChatEvent>`, `not_a_task(chat, task_id)`, `terminal(id) -> Terminal{env, copy, session, command: Option<Command>, resumed}`.
- `ChatEvent::{Msg(ChatMsg), Changed(ChatId), Removed(ChatId)}`. `ChatSummary{id, env, project_id, title, state, task_id, last, updated_at, archived, convoy, swarm}`; `ChatState` по срочности: `WaitsYou, Failed, Working, Queued, Done, Talk` (NeedsHuman/AwaitingOutward/MergeBlocked → WaitsYou; Failed/DoneNotPushed → Failed; PausedLimit/HandedOff → Working). Архив в конце, свежие первыми; архив через `Settings.keep.archive_days`.
- Строки чата (роль System, `ключ: подробности`, `reply_to` = сообщение человека): `this_is_task`, `task_not_created`, `not_answered` (model_rejected|auth_required|…), `will_answer_after: HH:MM`, `new_session_started`, `note_queued`, `note_delivered: HH:MM`, `answer_passed`, `note_not_passed`, `back_to_question`. Подпись ответа - `ChatMsg.model` = «<адаптер> · <модель версия>».
- Адресат заметки: задача в NeedsHuman с вопросом агента, иначе последняя открытая задача чата. Ошибки: `empty_message`, `chat_not_found`, `cancel_task_first`, `already_coding`. Копия чата: `<dir>/<chat id>`.
- Не сделано: очередь ответов и ожидание лимита живут в памяти (после перезапуска - `retry`); цепочки запасных моделей при отказе модели в чате нет; `ChatMsg.tool` чат не пишет.

### Из таска 10 - рой и фабрика

- `Engine`: `pick_variant(id, letter)`, `stop_swarm(id)`, `nudge(id)`, `submit_convoy(project_id, env, text) -> chat_id`, `swarm_view(id) -> Option<SwarmView>` (таск 13 кладёт в `Task.swarm_view`), `swarm_copy(id, variant) -> Option<PathBuf>` (для `diff_file{variant}`). `submit` отказывает рою вне 2..=5: `swarm_size: ...`. `note(.., to)` принимает "A".."E" и "reviewer"; `to: None` в рое - всем.
- `model::StepState::Queued(DateTime)`: ждущий шаг Merge = Queued(at), вливающийся = Running.
- `swarm::view::SwarmView` (serde) = то, что читает `ui/js/screens/swarm.js`: `{variants[{id,model,state queued|running|passed|failed,gate,score,summary,criteria,why,stats,cost_usd,why_failed,files,worktree}], advice{variant,model,text,autopick_at}, picked, picked_by, stopped, parts[{id,title,files,worker,model,state}], council{proposals,votes[{voter,for,why}],winner,decided_by,review_votes}}`.
- `core::swarm`: `assign_models`, `check_split`, `tally`, `rank`, `best_failed`, `letters`, `covers`; разбор ответов - `swarm::parse`.
- Ключи ленты: `variant_passed, variant_failed, race_advice, picked, swarm_stopped, part_landed, part_failed, part_outside_files, split_overlap, split_sequential, crew_split_failed, council_proposal, council_vote, council_decided, council_member_out, council_no_proposals, council_review_vote, council_review_tie, convoy, watcher, nudge, dependency_blocked, waiting_dependency`. Ключи `Outcome.error`: `swarm_failed`, `crew_part_failed`, `convoy_split_failed`, `deferred_too_often`, `stalled`, `swarm_stopped`.
- Поручение: чат «Поручение: <фраза>», карточки - `ChatMsg{who: System, kind: Task, task_id}`; движок пишет их в store без события `chat-msg` (таск 13 доставляет).
- Третий `DEFERRED` теперь проваливает задачу (`deferred_too_often`, ист. 165), а не игнорируется. Создание/удаление worktree сериализовано (`wt_lock`).
- Упрощено: лимит в рое ставит на паузу всю задачу; зависание полосы проваливает её без толчка; NEEDS_HUMAN от варианта/части/участника совета = полоса выбыла; Надзиратель продолжает сессию, только если её id известен; автовыбор не сдвигается заморозкой; копии вариантов живут до «Удалить». Best-of-N для рискованных задач не построен.
- (ремонт 10) Запуск роли и полосы - один путь: `engine/runs.rs` (`fold`, `after_run`, `start_on_fallback`, `budget_breach`, `stall_decision`, `add_cost`) зовут и `run_role`, и `run_lane`; полоса кончается так же, как задача (лимит → пауза всей задачи, нет CLI → `env_unavailable` и ожидание, бюджет → вопрос, зависание → толчок, диагноз, перезапуск как попытка). Все записи в `.git` проекта - под `with_git` (один замок на проект). `take_seat` ждёт на Condvar.
- (ремонт 10) Вопрос полосы - `QuestionKind::Agent` с текстом `Variant A: …` / `Part B: …` / `Member C: …`, одно уведомление, ответ уходит в сессию полосы. `answer()` на остановленном рое → `Err("swarm_stopped")`, выйти из этого состояния можно только `pick_variant`. Новые ключи ленты: `prompt_not_rewritten: quality_off` (раз на задачу), `copy_not_removed: <path>: <err>`, `watcher: … nudged <role> <lane>`, `watcher: restarted <role> <lane> …`. Строка `note delivered` пишется перед стартом запуска.
