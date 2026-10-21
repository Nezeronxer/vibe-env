# 01 - Каркас, общие типы, хранилище, git

**Требования:** R01, R19i, R06.4, R06.5, G01.5, G01.6, R17i.1, R11.2, R11.3
**Blocked by:** -
**Зона:** корень workspace (`Cargo.toml`), `core/` (кроме `core/src/agent/adapters/`, `core/src/engine/`, `core/src/telegram/`), `app/` (только заготовка крейта), `ui/` (только заглушка `index.html`)
**Волна:** 1
**Status:** ready

## Что должно заработать

Cargo workspace из `core` (библиотека `nightshift-core`) и `app` (крейт Tauri 2, пока пустое окно с `ui/index.html`). В `core` - модули `model`, `store`, `git`, `notify`, а также каркас `agent` (trait `Adapter`, `Run`, `AgentEvent`, `run_blocking`, `FakeAdapter`, пустой `adapters()`), каркас `telegram` (только trait `Control` и сигнатуры, тело - `todo!()` не допускается: пустые реализации, возвращающие ошибку «not implemented»), каркас `engine` (пустой модуль). Всё - строго по `interfaces.md`, чтобы три следующих таска шли параллельно.

`git` - главная работа таска: worktree на задачу, коммит, перебазирование с конфликтом, squash в main по трём правилам продвижения (рабочая копия человека на main и чистая → `merge --ff-only`; на main и грязная → `AdvanceBlocked`; на другой ветке → `update-ref` с проверкой старого значения), revert, push без force, fetch+ff, PR через `gh`, `repo_info` (ahead/behind относительно origin), `detect_commands` (npm/cargo/pytest/go/make).

`store` - JSON в каталоге данных, атомарная запись, лог задачи построчно в jsonl.

## Из брифа, дословно

> «человек даёт задачу, всё остальное делают агенты»
> «внедрить ветки GitHub, push, Origin, это вот всё остальное, чтобы было удобней»

## Разделы спецификации

Решения §7, §8, §10, §13; Границы и швы (store, git, model, notify, каркас agent); Глубина: Обрыв, Рост, Права.

## Критерии приёмки

- [ ] `cargo test -p nightshift-core` зелёный; `cargo build -p nightshift-app` собирается (окно-заглушка)
- [ ] Все типы и сигнатуры из `interfaces.md` существуют и компилируются (включая каркасы agent/telegram/engine)
- [ ] `store`: запись/чтение проектов, задач, настроек, лог; переживает «обрыв» (tmp-файл не портит state.json); тест
- [ ] `git`: тесты на временных репо - worktree создаётся/удаляется; commit_all без изменений → None; конфликт rebase → `Conflict`; squash даёт ровно один коммит на main с сообщением задачи
- [ ] `git`: три правила продвижения main - каждое своим тестом, в т.ч. незакоммиченный файл человека остаётся нетронутым
- [ ] `git`: revert отменяет squash-коммит; push в локальный bare-репо как origin работает, отказ push → `GitError::Push`, никогда `--force` (grep в коде)
- [ ] `detect_commands` на фикстурах: package.json со скриптом test → `npm test` + `npm ci`/`npm install`; Cargo.toml → `cargo test`; pyproject → `pytest`; go.mod → `go test ./...`; Makefile с test → `make test`; пусто → None
- [ ] `FakeAdapter` + `run_blocking`: сценарий из шагов пишет файлы в cwd и отдаёт события; cancel убивает процесс (fake может быть внутрипроцессным - тогда cancel прерывает сценарий)
- [ ] `.gitignore` с `target/`; никаких секретов в коде
- [ ] `gh_status`, `pr_state`, `list_ns_branches`, `delete_merged_ns_branches` - удаляют только влитые ветки `ns/*`; тест на временном репо (gh - через подменяемый путь к бинарю или пропуск, если gh не залогинен)
