# Архитектура

Workspace из двух крейтов: `core` (`nightshift-core`, библиотека, без Tauri) и `app` (`nightshift-app`, Tauri 2). Интерфейс - `ui/` без фреймворка и сборщика, Tauri отдаёт его как `frontendDist`. Почему так - `docs/adr/0001-yadro-otdelno-ot-tauri.md`.

## Поток данных

- Человек пишет задачу в `ui` -> Tauri-команда `submit_task` (`app/src/main.rs`) -> `Engine::submit` (`core/src/engine/mod.rs`).
- Планировщик движка (поток `Engine::start`) заводит отдельную git-копию: ветка `ns/<short-id>-<slug>`, worktree вне рабочей папки человека (`docs/adr/0002-odna-zadacha-odna-kopiya-vlivanie-po-ocheredi.md`).
- Промпт под выбранную модель собирает `core/src/prompt/`; агента запускает `Adapter` (`core/src/agent/`) внешним процессом, вывод разбирается в `AgentEvent`.
- Статусы (`core::model::Status`): Queued -> Preparing -> Working -> Checking -> Reviewing -> Merging -> Done; боковые: NeedsHuman, PausedLimit, AwaitingOutward, MergeBlocked, DoneNotPushed, Failed, Cancelled, Reverted.
- Проверка - `Project.check_cmd` в копии; провал - новый раунд работника (`attempts`); затем ревью отдельным запуском (`review_rounds`).
- Вливание: `merge --ff-only` на чистой базовой ветке под мьютексом очереди вливаний; push/PR только после разрешения человека (`Finish`, `ask_outward`).
- Движок шлёт `EngineEvent` в `mpsc`; `app` пересылает их окну событиями `task-changed`, `task-log`, `project-changed`, `task-action`, `chat-delta`; окно зовёт команды через `ui/js/core/api.js`.
- Человека зовут по трём поводам: вопрос агента (`NEEDS_HUMAN` -> `Question`), «не вышло», разрешение на действие наружу. Путь уведомления - `trait Notifier` / `Fanout` (`core/src/notify.rs`), системные уведомления - `app/src/notifier.rs`.
- Чат со средой (`core/src/chat/`): сначала словарь команд (`verbs.rs`), иначе ответ модели; строка `NS_TASK: <название>` в конце ответа превращает сообщение в задачу.
- Многоагентные режимы: `core/src/swarm/` и `core/src/engine/{race,crew,council,convoy,lanes,watch}.rs`.
- Каталоги скиллов/MCP/модов и обновления CLI ходят в сеть только через `trait Fetch` (`core/src/net/`, системный `curl`, `docs/adr/0014-curl-vmesto-http-klienta.md`); тесты подставляют `FakeFetch`.

## Границы модулей

- `core::model` - общие типы (serde, `snake_case`); логики нет.
- `core::git` - все git/gh-операции через `Command`; libgit2 нет.
- `core::agent` - флаги CLI, форматы событий, признаки лимита; наружу `Adapter`, `Run`, `AgentEvent`, `FakeAdapter`.
- `core::engine` - машина состояний, промпты ролей, повторы, мьютекс вливания; про окно не знает.
- `core::store` - единственный владелец данных на диске (`state.json`; нечитаемый файл не затирается пустым состоянием); каталог передаётся в `Store::open(dir)`.
- `core::term` - PTY-сессии с кольцевым буфером, живут вне окна (`docs/adr/0012-terminal-multiplexer-v-yadre.md`, `docs/adr/0013-pty-portable-pty-i-raw-kanal.md`).
- `core::kits`, `core::market`, `core::updates`, `core::models`, `core::config` - наборы настроек сред, каталоги, обновления CLI с откатом, радар моделей, конфиг-файл `~/.config/nightshift/config` с сохранением комментариев.
- `app` - только оболочка: окна, трей, хоткей, команды, события, системные уведомления; логики жизни задач нет.
- `ui` - экраны; данные получает только командами и событиями Tauri.

## Соглашения

- Зависимости ядра - закрытый список в `core/Cargo.toml`; новая только если без неё никак.
- Асинхронщины нет: `std::thread` + `std::sync::mpsc`; внешние процессы - `std::process::Command`.
- Нельзя: принудительный push, жёсткий reset по веткам человека, удаление веток не `ns/*`, правка рабочей копии иначе чем `merge --ff-only` на чистой базе.
- Идентификаторы и комментарии - английский; строки для человека - только через словари `ui/i18n*.js` (ru/en); ядро отдаёт короткие английские ключи с параметрами, окно переводит.
- Всё, что уходит в лог и окно, проходит `scrub` (`core/src/kits/scrub.rs`).
- Скачанное недоверенно: лимиты размера, пути без `..` и симлинков наружу, исполняемое выключено до явного согласия; обновление CLI никогда не через `sudo`.
- UI: без `innerHTML` для вывода терминала и данных из сети; цвета, радиусы, зазоры - только токены `ui/css/tokens.css`; по файлу i18n на фичу (`ui/js/v4/i18n-*.js`).
- Аргументы Tauri-команд на стороне JS - camelCase.
- Главный шов тестов - `Engine` + `FakeAdapter` + временный git-репозиторий (`core/tests/common/mod.rs`); разбор вывода CLI проверяется на образцах из `core/tests/fixtures/`; данные тестов - только во временных каталогах.
- Решения с причинами - `docs/adr/`; границы доверия и аудит - `docs/security.md`.
