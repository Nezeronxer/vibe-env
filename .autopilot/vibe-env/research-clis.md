# Агентские CLI: проверенные факты для Nightshift

Сверка 2026-10-21. [лок] - вывод на этой машине; [док] - официальная документация; «не подтв.» - в источниках не найдено.
Установлены: claude 2.1.306, opencode 1.18.34, qwen 0.22.0. Не установлены (`command -v` пусто): codex, gemini, cursor-agent - только документация. Доки Codex - learn.chatgpt.com.

Сводка: список моделей есть у Cursor (`--list-models`), Codex (`codex debug models`), opencode (`opencode models`); у Claude, Gemini, Qwen команды нет. Процент и время сброса лимита официально есть у Claude (statusline JSON, `/usage`) и частично у Codex (`/usage`, статусбар); у остальных не подтв. Неинтерактивный JSON-вывод есть у всех шести.

## 1. Claude Code (`claude`)
- Модель: `--model <alias|полное имя>`; алиасы `fable`, `opus`, `sonnet` - «alias for the latest model»; полное имя с номером принимается [лок: claude --help]. Список моделей: команды нет [лок]; не подтв.
- Неинтерактив: `-p`, `--output-format text|json|stream-json`, `--input-format stream-json`, `--json-schema`, `--max-budget-usd` [лок]. Продолжение: `-c/--continue`, `-r/--resume [id|имя]`, `--session-id`, `--fork-session` [лок]. Ошибки в stream-json: категория `rate_limit` [док: code.claude.com/docs/en/headless].
- Только чтение: `--permission-mode plan` (также acceptEdits, auto, manual, dontAsk, bypassPermissions); `--tools ""`; `--disallowedTools`; `--restricted` (убирает Bash и код-инструменты, WebFetch, игнорирует user/project-настройки) [лок]. Отдельного флага песочницы в --help нет [лок].
- Настройки: ~/.claude (settings.json, CLAUDE.md, skills, hooks, agents), .claude/ в проекте; `--settings`, `--setting-sources`, `--agents <json>` (субагенты), `claude mcp add`, `claude plugin` [лок]. Хуки и субагенты есть (`--bare` их отключает) [лок].
- Лимит, как сообщает: категория `rate_limit` в событии ошибки [док headless]. Текст и код выхода - не подтв.
- Расход (п.7): statusline JSON - `rate_limits.five_hour.used_percentage`, `rate_limits.seven_day.used_percentage` (0-100), `resets_at` (Unix-секунды); `rate_limits.spend_limit` за apps-gateway [док: code.claude.com/docs/en/statusline]. В `-p stream-json`/result - не подтв. (в headless-доке не описаны; строки этих полей есть в бинаре 2.1.306 [лок: grep]).
- Без statusline: `/usage` показывает полоски лимитов плана; при ограничении эндпоинта - данные до 60 мин назад, `r` - повтор [док: code.claude.com/docs/en/costs]. В неинтерактивном режиме - не подтв.

## 2. OpenAI Codex CLI (`codex`) - не установлен
- Модель: `-m/--model`, `-c model=...`, `-p/--profile`; ключ `model = "..."` в ~/.codex/config.toml; .codex/config.toml проекта - только в доверенном проекте [док: learn.chatgpt.com/docs/developer-commands?surface=cli; /docs/config-file/config-basic]. Версия - строкой. Алиас «последняя» - не подтв.
- Список моделей: `codex debug models` (каталог, экспериментально); в TUI - `/model` [док developer-commands].
- Неинтерактив: `codex exec "промпт"` (или `-` для stdin); `--json` - JSONL; `-o` - последнее сообщение в файл; `--output-schema` [док]. Продолжение: `codex exec resume [SESSION_ID]`, `--last`, `--all`; `codex exec fork` [док].
- Только чтение: `--sandbox read-only` (`-s`), `-a on-request|never`; `--yolo` - без песочницы [док].
- Настройки: ~/.codex/config.toml, .codex/config.toml. Правила: AGENTS.override.md / AGENTS.md в ~/.codex и от корня проекта до cwd [док: learn.chatgpt.com/docs/agents-md]. Хуки: фича `hooks` (по умолчанию включена), hooks.json или inline [hooks] [док config-basic]. Субагенты: фича `multi_agent`, `/subagents` [док]. MCP: `codex mcp add|list|get|remove|login` в config.toml [док]. Скиллы - не подтв.
- Лимит, как сообщает: текст и код - не подтв.
- Расход (п.7): `/status` по описанию - модель, approval, writable roots, остаток контекста, токены; про лимиты не сказано. `/usage` - токены аккаунта и «rate-limit reset». Пункт статусбара «rate limits» через `/statusline` / `tui.status_line` [док developer-commands]. Поля лимитов в `exec --json` - не подтв.

## 3. Google Gemini CLI (`gemini`) - не установлен
- Модель: `-m/--model` - алиас или конкретное имя, по умолчанию `auto`; `/model` - диалог Pro/Flash/Auto [док: docs/cli/cli-reference.md, docs/cli/model.md, github.com/google-gemini/gemini-cli]. Список моделей: команды нет - не подтв.
- Неинтерактив: `-p`; `-o json|stream-json` (события init, message, tool_use, tool_result, error, result с расходом по моделям). Коды выхода: 0 успех, 1 сбой/API, 42 неверный ввод, 53 лимит ходов [док docs/cli/headless.md]. Продолжение: `-r/--resume` (последняя, индекс, UUID), `--list-sessions`, `/resume` [док docs/cli/session-management.md].
- Только чтение: `--approval-mode=plan` (default, auto_edit, yolo, plan) [док docs/cli/plan-mode.md]. Песочница: `-s/--sandbox` [док cli-reference].
- Настройки: ~/.gemini/settings.json; .gemini/settings.json проекта важнее [док docs/cli/settings.md]. Правила: GEMINI.md; имена через `context.fileName`, напр. AGENTS.md [док docs/cli/gemini-md.md]. Субагенты: .gemini/agents/*.md, ~/.gemini/agents/*.md [док docs/core/subagents.md]. Хуки: в settings.json [док docs/hooks/index.md]. MCP: `mcpServers` в settings.json [док docs/tools/mcp-server.md].
- Лимит, как сообщает: при сбое модели, в т.ч. из-за квоты, - fallback с запросом согласия [док docs/cli/model-routing.md]. Текст сообщения о квоте - не подтв.
- Расход (п.7): команды `/stats` или `/quota` в cli-reference нет. Токены сессий - в ~/.gemini/tmp/<hash>/chats [док session-management]. Окна, проценты, сброс - не подтв.

## 4. opencode (`opencode`) - установлен, 1.18.34
- Модель: `-m/--model provider/model` для `run` и TUI [лок]; ключ `model` в opencode.json. Приоритет: флаг, конфиг, последняя использованная [док: opencode.ai/docs/models/]. Алиас «последняя» - не подтв.
- Список моделей: `opencode models [provider]` [лок]. Локально - только 10 бесплатных `opencode/*`; `opencode models anthropic` -> «Provider not found» [лок].
- Неинтерактив: `opencode run "..."`, `--format json` - сырые JSON-события [лок; док opencode.ai/docs/cli/]. Продолжение: `-c`, `-s <id>`, `--fork`; `opencode session list` [лок].
- Только чтение: `--agent <имя>` [лок]. Какой агент read-only и есть ли песочница - не подтв. (Agents и Permissions не открывал). `--auto` - авто-одобрение, кроме запрещённого [лок].
- Настройки: .well-known/opencode, ~/.config/opencode/opencode.json, OPENCODE_CONFIG, opencode.json проекта, каталоги .opencode/ (agents, commands, plugins) [док: opencode.ai/docs/config/]. Правила: ключ `instructions` (globs, напр. AGENTS.md). Субагенты: раздел Agents, детали не подтв. Хуки: через плагины, ключ `plugin`; отдельной модели хуков не подтв. MCP: раздел «MCP servers», ключ `mcp` не сверял.
- Лимит, как сообщает: текст и код - не подтв.
- Расход (п.7): `opencode stats` - «token usage and cost statistics» [лок]. Лимитов, окон, процентов не найдено - не подтв.

## 5. Cursor CLI - не установлен
- В доке бинарь - `agent` (`agent -p`); `cursor-agent` на машине не найден [лок].
- Модель: `--model <model>`; версия строкой; алиас - не подтв. Список моделей: `--list-models` [док: cursor.com/docs/cli/reference/parameters].
- Неинтерактив: `-p/--print`, `--output-format text|json|stream-json`, `--stream-partial-output` [док parameters; cursor.com/docs/cli/headless]. Продолжение: `--resume [chatId]`, `--continue` (= `--resume=-1`), `agent ls`, `agent resume` [док cursor.com/docs/cli/overview].
- Только чтение: `--mode ask` (без правок), `--plan` / `--mode plan`. Песочница: `--sandbox enabled|disabled`, `/sandbox`. `-f/--force` (`--yolo`) разрешает команды, если не запрещены [док parameters, overview].
- Настройки: в навигации доки есть Rules, Skills, Subagents, Hooks, MCP; пути файлов не открывал - не подтв.
- Лимит, как сообщает: текст и код - не подтв.
- Расход (п.7): по тарифу - «weekly pool» для моделей Cursor на платных планах; у Plus/Premium/Ultra - два месячных пула и on-demand; сторонние модели - по API-цене из prepaid credit [док: cursor.com/docs/models]. Процент и сброс через CLI - не подтв.

## 6. Qwen Code (`qwen`) - установлен, 0.22.0
- Модель: `-m/--model`; `--fallback-model` (до 3, при ошибках ёмкости) [лок]. Алиас - не подтв. Список моделей: команды нет [лок]; не подтв.
- Неинтерактив: `-p`; `-o text|json|stream-json`; `-i` - промпт, затем интерактив [лок]. Продолжение: `-c/--continue`, `-r/--resume [id]`, `qwen sessions list` [лок]. Сессии - JSONL в ~/.qwen/projects/<cwd>/chats [док: github.com/QwenLM/qwen-code, headless.md].
- Только чтение: режим Plan - «read-only analysis» [док docs/users/features/approval-mode.md]. Флага выбора режима в --help 0.22 нет - не подтв. Песочница: `-s/--sandbox`; `--safe-mode` отключает кастомизации [лок].
- Настройки: ~/.qwen/settings.json, .qwen/settings.json проекта [док docs/users/configuration/settings.md]; локально файл есть, ключи $version, hooks, ui [лок]. Правила: QWEN.md [док settings.md]. Субагенты: .qwen/agents/, ~/.qwen/agents/ (Markdown) [док docs/users/features/sub-agents.md]. Хуки: ключ `hooks` и `qwen hooks` [лок]. MCP: `qwen mcp add|list|remove|reconnect|approve|reject` [лок], `mcpServers` в settings.
- Лимит, как сообщает: текст и код - не подтв.
- Расход (п.7): бесплатный Qwen OAuth прекращён 2026-04-15; QwenCloud Token Plan - «monthly Credit quota» [док docs/users/configuration/auth.md]. Команда или поля текущего расхода - не подтв.

## Проверено живым запуском (21.10.2026, claude 2.1.306)
`claude -p "…" --output-format stream-json --verbose` присылает событие
`{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","resetsAt":<unix>,"rateLimitType":"five_hour","overageStatus":…,"isUsingOverage":false,"unifiedWindows":{"five_hour":{"utilization":0.54,"resetsAt":<unix>},"seven_day":{"utilization":0.16,"resetsAt":<unix>}}}}`
- `utilization` - доля 0..1, `resetsAt` - Unix-секунды. Приходит в каждом прогоне задачи, отдельный запрос не нужен.
- Для Claude Code пункт 7 подтверждён: лимиты берутся из этого события в ходе работы; в простое - последнее значение с отметкой «на HH:MM».

# Новые среды v8 (сверка 2026-10-28)

Ни одного из этих CLI на машине нет, ставить глобально нельзя: всё ниже - [док] или [исх] (исходник в официальном репозитории), в коде помечено `unverified`. Пакеты сверены командами `npm view`, `brew info --json=v2`, PyPI JSON. Фразы «не вошёл» в `common.rs::strong_auth` - из источников, а не из живого вывода.

## 7. Kimi Code CLI (`kimi`) - Moonshot AI
- Источник: github.com/MoonshotAI/kimi-code (README, docs/en/reference/kimi-command.md, configuration/data-locations.md, env-vars.md, guides/getting-started.md), apps/kimi-code/src/cli/prompt-render.ts [исх]. Старый Python `kimi-cli` (PyPI 1.52.0) помечен «Archived ... Use Kimi Code CLI instead».
- Установка (mac/linux): `curl -fsSL https://code.kimi.com/kimi-code/install.sh | bash` (рекомендуемая, кладёт в `~/.kimi-code/bin`); npm `npm install -g @moonshot-ai/kimi-code` (2.4.0 по `npm view`); brew `kimi-code` 2.3.3 (homebrew-core).
- Модель: `-m/--model <alias>` (алиас из config.toml, напр. `kimi-code/kimi-for-coding`).
- Неинтерактив: `-p/--prompt`, `--output-format text|stream-json`; в `-p` права `auto`, `--yolo/--auto/--plan` с `--prompt` запрещены - режима только чтения нет. stream-json: строки `{"role":"assistant","content","tool_calls":[{id,function{name,arguments}}]}`, `{"role":"tool","tool_call_id","content"}`, `{"role":"meta","type":"session.resume_hint","session_id"}`; итогового result нет -> Scan + synth_done.
- Продолжение: `-S/--session <id>` (скрытый алиас `-r/--resume`), `-c/--continue`.
- Папка: `~/.kimi-code` (config.toml, sessions, credentials), переменная `KIMI_CODE_HOME`. Вход: `/login` или `kimi login`.
- «Не вошёл»: «OAuth login expired» (changelog), в prompt-режиме без модели - ошибка «no model is configured» с отсылкой к login (changelog, точный текст не найден).
- unverified: всё; `--prompt=<текст>` через `=`.

## 8. GitHub Copilot CLI (`copilot`)
- Источник: github.com/github/copilot-cli README; docs.github.com/en/copilot/reference/cli-command-reference (через `/api/article/body`); события JSONL - github/copilot-sdk docs/features/streaming-events.md [док SDK].
- Установка: `curl -fsSL https://gh.io/copilot-install | bash` (без root - в `~/.local/bin`); `brew install copilot-cli` (cask 1.0.105); `npm install -g @github/copilot` (1.0.105).
- Модель: `--model=MODEL` (`auto` - выбор Copilot).
- Неинтерактив: `-p/--prompt=PROMPT`, `--output-format=json` (JSONL), `-s/--silent`, `--allow-all-tools`, `--allow-all`/`--yolo`, `--no-ask-user`, `--mode plan`. Формат строк JSONL на странице CLI не описан; разбор по событиям SDK (`assistant.message`, `tool.execution_start/complete`, `assistant.usage`, `session.error` с `errorType` authentication/quota/rate_limit). Итога нет -> Scan + synth_done.
- Продолжение: `-r/--resume[=VALUE]` (значение необязательное, поэтому `--resume=<id>`), `--continue`.
- Папка: `~/.copilot`, переменная `COPILOT_HOME` (`--config-dir` устарел). Вход: `/login`, `copilot login`, токен `GH_TOKEN`/`GITHUB_TOKEN`/`COPILOT_GITHUB_TOKEN`.
- Обновление: `copilot update` [док].
- unverified: что JSONL CLI = события SDK; поле `sessionId`; поведение читателя без `--allow-all-tools` в `-p`.

## 9. Crush (`crush`) - Charm
- Источник: github.com/charmbracelet/crush README, internal/cmd/run.go, internal/cmd/root.go [исх].
- Установка: `brew install charmbracelet/tap/crush`; `npm install -g @charmland/crush` (0.98.2) - в приложении npm (одна строка для mac и linux, без пайпа).
- Модель: `crush run -m/--model <model|provider/model>`; у TUI флага модели нет.
- Неинтерактив: `crush run [prompt...]`, `-q/--quiet`; вывод только текст, JSON нет, id сессии не печатается -> весь вывод = ответ.
- Продолжение: `-s/--session <id>`, `-C/--continue` (и у `run`, и у корня).
- Права: `-y/--yolo` только у корневой команды, у `run` нет; как `run` спрашивает разрешения - не подтв.
- Папка: `~/.config/crush` (crushrc, crush.json по XDG), данные `~/.local/share/crush`; переменные `CRUSH_GLOBAL_CONFIG`, `CRUSH_GLOBAL_DATA` (в терминал не переносятся).
- «Не вошёл»: «no providers configured - please run 'crush' to set up a provider interactively» (run.go).

## 10. Amp (`amp`) - Sourcegraph / ampcode.com
- Источник: ampcode.com/docs/markdown/{cli, cli/execute-mode, cli/streaming-json, cli/settings, threads, tools} (индекс https://ampcode.com/llms.txt); install.sh; ampcode.com/news/npm-package-changes.
- Установка: `curl -fsSL https://ampcode.com/install.sh | bash` (в `~/.amp/bin`, ссылка в каталог из PATH); npm `@ampcode/cli` (старое `@sourcegraph/amp` - алиас).
- Модель: флага нет (режим выбирается «циферблатом»).
- Неинтерактив: `-x/--execute "<промпт>"`, `--stream-json` (только с --execute) - формат Claude Code (system init с session_id, assistant/user, result). Инструменты без подтверждений (docs tools), режима только чтения нет.
- Продолжение: `amp threads continue <T-id>` (с `--execute` - тоже).
- Папка: `~/.config/amp/settings.json`, проект `.amp/settings.json`; `AMP_HOME` - папка установки, не настроек. Вход: `amp login`, токен `AMP_API_KEY`.
- Обновление: `amp update` [док].
- unverified: `--execute=<промпт>` через `=`, порядок `threads continue <id> --execute`.

## 11. goose (`goose`) - Block, репозиторий теперь github.com/aaif-goose/goose
- Источник: documentation/docs/guides/goose-cli-commands.md, getting-started/installation.md, guides/config-files.md, environment-variables.md, guides/running-tasks.md; crates/goose-cli/src/session/mod.rs (StreamEvent), crates/goose-provider-types/src/conversation/message.rs [исх].
- Установка: `curl -fsSL https://github.com/aaif-goose/goose/releases/download/stable/download_cli.sh | bash` (в `~/.local/bin`); `brew install block-goose-cli` (1.56.0).
- Модель: `goose run --model` (+ `--provider`); переменные `GOOSE_PROVIDER`, `GOOSE_MODEL`.
- Неинтерактив: `goose run -t/--text`, `--output-format text|json|stream-json`; stream-json: `{"type":"message","message":{role,content:[text|toolRequest|toolResponse|error]}}`, `{"type":"error","error"}`, `{"type":"complete",input_tokens,output_tokens}`. Id сессии в потоке нет.
- Права: `GOOSE_MODE` auto (по умолчанию) / approve / smart_approve / chat; только чтения нет (chat - вообще без инструментов).
- Продолжение: `goose session --resume --session-id <id>`; `goose run -n <имя> -r`.
- Папка: `~/.config/goose/config.yaml`, ключи `~/.config/goose/secrets.yaml` или связка ключей; переменная `GOOSE_PATH_ROOT`.
- «Не вошёл»: «No api key passed in» (config-files.md); блок error с `kind: authentication`.
- Обновление: `goose update` [док].
- unverified: поля toolRequest/toolResponse (`toolCall.value.{name,arguments}`, `toolResult.status`).

## 12. Factory Droid (`droid`)
- Источник: github.com/Factory-AI/factory README; docs.factory.com/droid-cli/cli-reference.md, droid-exec/overview.md, droid-cli/settings.md (индекс docs.factory.com/llms.txt).
- Установка: `curl -fsSL https://app.factory.ai/cli | sh` (в `~/.local/bin`); `npm -g install droid` (0.251.0, то же что `@factory/cli`).
- Модель: `-m/--model <id>`.
- Неинтерактив: `droid exec`, `-o text|json|stream-json|stream-jsonrpc`. По умолчанию только чтение; `--auto low|medium|high` (high разрешает git push - в приложении не используется, у писателя `medium`). `-o json` - одна строка формата Claude result (`type, is_error, result, session_id, usage`) - её и разбираем. События stream-json (system, message, tool_call, tool_result, completion) без списка полей.
- Продолжение: `droid exec -s/--session-id <id>`; TUI `droid --resume [id]`.
- Папка: `~/.factory` (settings.json); ключ `FACTORY_API_KEY`. Вход: `/login`.
- Обновление: `droid update`, откат `droid update --version X` [док].
- unverified: `--` перед промптом; фраза «не вошёл» не найдена.

## 13. Aider (`aider`)
- Источник: github.com/Aider-AI/aider README, aider/website/docs/{install.md, scripting.md, config/options.md}, aider/onboarding.py [исх]; PyPI `aider-chat` 0.86.2.
- Установка: `curl -LsSf https://aider.chat/install.sh | sh`; `python -m pip install aider-install && aider-install`; `uv tool install --force --python python3.12 --with pip aider-chat@latest`.
- Модель: `--model`.
- Неинтерактив: `--message` (короткий `-m` у aider - это сообщение, не модель), `--yes-always`, `--no-pretty`, `--no-stream`; вывод - текст, JSON нет. Только чтение: `--dry-run`.
- Продолжение: сессий нет; история чата - файл в папке, `--restore-chat-history`.
- Папка: `~/.aider.conf.yml` или в репозитории; ключи - переменные провайдеров.
- «Не вошёл»: «No LLM model was specified and no API keys were provided.» (onboarding.py; после неё aider предлагает OAuth OpenRouter).
- unverified: всё; `--message=<текст>` через `=`; код выхода при отсутствии ключа.

## 14. Grok Build (`grok`) - xAI, официальный
- Официальность: репозиторий github.com/xai-org/grok-build в организации xai-org («SpaceXAI's coding agent harness and TUI»), страница x.ai/cli. npm `grok-cli`, `@vibe-kit/grok-cli` - сторонние, не берём.
- Источник: README; crates/codegen/xai-grok-pager/docs/user-guide/{02-authentication, 05-configuration, 14-headless-mode, 17-sessions, 22-permissions-and-safety}.md.
- Установка: `curl -fsSL https://x.ai/cli/install.sh | bash`.
- Модель: `-m/--model` (напр. `grok-4.6`).
- Неинтерактив: `-p/--single`, `--output-format plain|json|streaming-json|streaming-messages-json`; `streaming-messages-json` - формат Claude Code stream-json (init, assistant, user tool_result, result с session_id) - его и разбираем. `--yolo` - всё без вопросов; `--permission-mode dontAsk` - только заранее разрешённое и встроенные читающие инструменты (для читателя).
- Продолжение: `-r/--resume <id>`, `-c/--continue`, `-s/--session-id` (создаёт НОВУЮ с этим UUID).
- Папка: `~/.grok` (config.toml, auth.json), переменные `GROK_HOME`, `GROK_CONFIG`, `GROK_CONFIG_PATH`; ключ `XAI_API_KEY`. Вход: `grok login`.
- «Не вошёл»: «Authentication failed» -> `grok logout`, `grok login` (02-authentication.md).
- Обновление: `grok update`, `grok update --version X` [док].
- unverified: `--single=<текст>` через `=`; `dontAsk` как режим только чтения в headless.

## Что из этого в коде
- Раскладки конфигов (`kits/layout.rs`) для §7-14 - заглушки как у Cursor (`home_dir: ""`, `unverified: ALL`): группы конфигов для них выключены, пока формат правил/хуков/MCP не сверен живым CLI.
- Каталоги моделей: models.dev-провайдер дан только `copilot` (`github-copilot`) и `grok` (`xai`); у Kimi `-m` берёт алиасы из config.toml, а не id каталога.
- В терминал переносятся `KIMI_CODE_HOME`, `COPILOT_HOME`, `GROK_HOME`, `GOOSE_PATH_ROOT` (только абсолютный путь).
