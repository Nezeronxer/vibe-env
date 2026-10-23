# 31 — Чистая программа для скачавших и чистая копия для публикации

**Требования:** G76, G77, G77i, G74, G75.1, G63
**Зависит от:** 27, 28, 29, 30
**Зона:** `ui/` (вынос мок- и тестовых файлов, загрузчики в релизе) · `scripts/ui-selftest.js` и `ui/selftest.*` (новые пути) · `app/tauri.conf.json` · `Cargo.toml` (`[profile.release]`) · `.cargo/config.toml` (новый) · `LICENSE` · `scripts/check-clean-build.sh`, `scripts/make-public-copy.sh` (новые) · `.github/workflows/release.yml` · `README.md`, `README.ru.md` (чек-лист публикации) · `core/src/engine/roles.rs`, `core/src/chat/` (только `extra_env`: `NIGHTSHIFT_PROJECT`, `NIGHTSHIFT_PROJECT_NAME`) · `~/Dropbox/…/claude-shared/once/linux/` скрипт и `claude-shared/claude/needs-sudo.linux.txt` (вне репозитория)
**Волна:** 32
**Ревью:** да — утечка данных автора
**Модель:** сильная

## Что должно заработать

Скачавший получает чистое приложение: без мок- и тестовых файлов, без ника и путей автора, без чего-либо с машины сборки; первый запуск пустой. Скрипт проверки сборки ищет запретные строки и входит в CI релиза. Скрипт собирает чистую копию для публикации одним первым коммитом без истории и `.autopilot/` - но не публикует. Каждый запуск агента знает проект через `NIGHTSHIFT_PROJECT`. Скрипт для Arch собирает и проверяет Linux-сборку.

## Из брифа, дословно

> «когда пользователи будут скачивать мою программу, у них должна устанавливаться чистая программа»
> «если у них будут мои скиллы, мои MCP и моё всё, это может быть утечкой»
> «Что уходит в публичный репозиторий?» → «Чистая копия»
> «Когда публикуем репозиторий на GitHub?» → «После v5 и аудита»
> «Codex, Gemini CLI, Cursor и пакеты Linux… Как проверяем?» → «Скрипт на Arch»; «Какие среды проверяем вживую?» → «Ни одной из них»

## Разделы спецификации

v6: истории 351, 353-357; «Решения по реализации v6» (чистая сборка, окружение запусков). Код: `notes-v6.md` §10, §11.

## Критерии приёмки

- [ ] В релизном бандле нет `dev-mock.js`, `mock-*.js`, `mock-loader*.js`, `selftest.*` (проверено распаковкой `.app`); dev-режим и selftest работают как раньше
- [ ] `scripts/check-clean-build.sh <бандл>`: ищет `nezeronxer`, `/Users/<имя сборщика>`, `claude-shared`, `memory/`, `.claude/skills`, `mcp__`, шаблоны токенов; на текущей сборке до правок - красный, после - зелёный; шаг в `release.yml`
- [ ] Пути крейтов убраны (`--remap-path-prefix`, `strip`); copyright «Nightshift contributors» в `tauri.conf.json`, Info.plist, LICENSE
- [ ] Первый запуск на пустом `<data>` и HOME: ни проектов, ни чатов, ни наборов; конфиг не создаётся (тест/скрипт)
- [ ] `scripts/make-public-copy.sh <папка>`: белый список, без `.autopilot/`, `.impeccable/`, `graphify-out/`, `CLAUDE.md`, `.DS_Store`, истории; `git init` и один коммит; gitleaks и `check-clean-build` по копии зелёные; ничего не пушит
- [ ] `NIGHTSHIFT_PROJECT` и `NIGHTSHIFT_PROJECT_NAME` в окружении всех ролей и чата (тест)
- [ ] Скрипт Arch в `claude-shared/once/linux/`: идемпотентный, без вопросов, sudo-пакеты строкой в `needs-sudo.linux.txt`; собирает AppImage из исходников, ставит в `~/.local/bin`, гоняет тесты, selftest, `check-clean-build`, проверяет claude/opencode/qwen, пишет лог
- [ ] README: чек-лист публикации через чистую копию; публикация - только по «публикуй»
