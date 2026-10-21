# Аналоги - разбор 2026-10-21

Собрано субагентом через WebFetch/WebSearch (Chrome не был подключён). «открыто» = страница прочитана, «по поиску» = только выдача поиска. Не открылись: Conductor, Devin, Factory, документация Warp.

| Продукт | Форма, платформы | Лицензия | Агенты | Изоляция | Проверка результата | Оповещения / лимиты | Источник |
|---|---|---|---|---|---|---|---|
| Conductor | нативное mac-приложение | закрыт | Claude Code, Codex | worktree | diff, checks, PR | н/у | по поиску |
| Claude Squad | TUI | AGPL-3.0 | Claude Code, Codex, Gemini, Aider, OpenCode, Amp | worktree + tmux | diff в TUI, ручной коммит | н/у | открыто github.com/smtg-ai/claude-squad |
| Vibe Kanban | npx, веб-UI | Apache-2.0 | 10+ | ветка на workspace | diff с комментариями, PR | н/у; проект «sunsetting» | открыто github.com/BloopAI/vibe-kanban |
| Crystal → Nimbalyst | Electron, mac/win/lin | MIT | Claude Code, Codex | worktree | принять/отклонить правку | push с iOS; Crystal deprecated 02.2026 | открыто |
| Sculptor (Imbue) | десктоп mac/Linux | MIT | Claude Code, любой терминальный | worktree, контейнеры экспериментально | ревью, мерж, PR | н/у; research preview | открыто |
| Terragon | облако | Apache-2.0 | Claude Code, Codex, Amp, Gemini | контейнер | PR | браузер, Slack; закрыт 16.01.2026 | открыто |
| Omnara | self-host/облако | Apache-2.0 | свой цикл | песочницы | дашборд | Slack/Discord с одобрением | открыто |
| Happy | мобильный клиент + CLI | MIT | Claude, Codex, Grok | н/у | н/у | E2E, продолжение сессии с телефона | открыто happy.engineering |
| Superset | Electron, mac (Linux экспериментально) | Elastic 2.0 | ~24 CLI | worktree (сами пишут: не песочница) | diff с комментариями | значки, звук, док | открыто |
| Cursor cloud agents | облако | закрыт | модели Cursor | облачная VM | PR + скриншоты/видео/логи | Slack; платно | открыто |
| Codex app/cloud | облако + mac-приложение | н/у | Codex | облако / worktree | файлы, тесты, PR | н/у | открыто |
| Jules | веб + GitHub | н/у | Google | VM | план на утверждение, PR | браузер | открыто |
| OpenHands | веб localhost, CLI | MIT | свой + Claude Code, Codex, Gemini, OpenCode, ACP | хост/Docker | н/у | Slack, GitHub, Linear | открыто |
| Warp | терминал | AGPL v3 | свой + Claude Code, Codex, Gemini | н/у | ревью кода | нативные уведомления | открыто/по поиску |
| Claude Code Desktop/web/Remote | mac/win/Linux beta, веб, мобильный | н/у | Claude | worktree / VM | diff, /code-review | системные + push; auto-continue после сброса лимита | открыто code.claude.com |
| Uzi | CLI Go | MIT | claude, codex, aider, cursor | worktree + tmux | checkpoint | н/у | открыто |
| Container Use | MCP + CLI | Apache-2.0 | любой MCP | контейнер + ветка | логи | н/у | открыто |
| Backlog.md | CLI/TUI/веб | MIT | через MCP | нет | спека-план-код, DoD | н/у | открыто |
| Orca | десктоп mac/win/lin + мобильный | MIT | Claude Code, Codex, Cursor, OpenCode, любой CLI | worktree | сравнение агентов | алерты, push; показывает время сброса лимитов | открыто github.com/stablyai/orca |
| Parallel Code | Electron mac/lin + Android | MIT | Claude Code, Codex, Gemini, Copilot | worktree | inline-комментарии | push на телефон при «Needs input» | открыто |
| Emdash | десктоп mac/win/lin | Apache-2.0 | 10+ | worktree | diff, PR, CI | статусы через хуки агентов | открыто |
| T3 Code, Paseo | Electron + мобильный | MIT / Apache-2.0 | несколько CLI | worktree | diff, PR | удалённый доступ, голос | открыто |

## Слабые места у всех (по разбору)

1. Лимиты подписки: авто-продолжение только у самого Claude Code; никто не переключает движок при лимите («Claude кончился - продолжаем на Codex»).
2. Системные уведомления + телефон + мессенджер вместе - ни у кого; Telegram - только сторонний мост.
3. Цепочка «тесты → самопроверка → независимое ревью → только потом зовём человека» на десктопе - никто; локальные инструменты показывают diff и ждут человека.
4. Worktree - не песочница; песочница по умолчанию почти нигде.
5. Живучесть: Terragon закрыт, Vibe Kanban sunsetting, Crystal deprecated; данные в обычных файлах переживают инструмент.
6. Linux - второй сорт почти у всех.
7. Конфликты при слиянии параллельных веток - автоматически почти никто.
8. Перекрёстное ревью разными агентами - только в зачатке.

## Фишки, которые стоит взять

Auto-continue после сброса лимита (Claude Code Desktop); push на телефон при «Needs input» (Parallel Code); статусы и звук (Superset); `.worktreeinclude` для `.env` (Claude Code Desktop); local-first, 127.0.0.1, без телеметрии (Backlog.md, Paseo); автоприём подтверждений (Claude Squad `-y`); задача из чата одной командой (Emdash, Terragon).
