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
