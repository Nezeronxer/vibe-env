# 02 - Адаптеры агентных сред

**Требования:** R14i, R14i.2, G02, G02.1, G02.2, A01, R17i
**Blocked by:** 01
**Зона:** `core/src/agent/adapters/`, `core/tests/fixtures/agents/`, функция `adapters()` в `core/src/agent/`
**Волна:** 2
**Status:** ready

## Что должно заработать

Реализации `Adapter` для: **Claude Code** (`claude -p … --output-format stream-json --verbose`, `--resume`, `--append-system-prompt`, `--permission-mode bypassPermissions` + `--settings` с deny `Bash(git push:*)`, `Bash(git remote:*)`, `Bash(sudo:*)` и песочницей, если ключи песочницы есть в документации установленной версии; ревьюер - `--permission-mode plan`), **opencode** (`opencode run --format json`, `--session`, `--dir`), **Codex CLI** (`codex exec --json`, продолжение сессии, автономный режим с записью только в рабочую копию), **Gemini CLI** (headless-режим с JSON-выводом, автоподтверждение), **Cursor Agent** (`cursor-agent -p --output-format stream-json`), **Qwen Code** (как Gemini CLI - форк), **своя команда** из `Settings.custom_agents` (шаблоны `{prompt}` `{cwd}` `{session}` с экранированием для shell, вывод - текст, `finish()` выдаёт `Done` по коду выхода).

Для каждого: `available()` (бинарь в PATH; в сообщении - команда установки), разбор событий в `AgentEvent` (текст, инструменты одной строкой, итог с id сессии, ошибка), распознавание лимита подписки (`usage limit`, `limit reached`, `rate limit`, `resets at …` → `Limit{resets_at}`; время разобрать, если оно есть).

Флаги **проверить по документации** каждого CLI (у Claude и opencode - ещё и `--help` на этой машине; остальные не установлены - по официальным докам/README, без установки). Где флаг не подтверждён - комментарий `// unverified: <источник>`.

## Из брифа, дословно

> «Также должны работать все генные среды, то есть не только Klot, код.»
> «агент оповещает человека, когда это нужно»

## Разделы спецификации

Истории 29, 31-34 (32, 32a, 32b); Решения §3, §4, §5, §6, §14; Границы: core::agent.

## Критерии приёмки

- [ ] `adapters(&Settings)` возвращает 6 встроенных + все custom
- [ ] Тест разбора на образце вывода для каждого из 6 (фикстуры в `core/tests/fixtures/agents/`): текст, инструмент, Done с session id, ошибка; для Claude и opencode - образцы сняты с реального запуска на этой машине (короткая задача «ответь ok» во временной папке), для остальных - собраны по документации и помечены в имени файла `*-from-docs.jsonl`
- [ ] Тест распознавания лимита: 3+ формулировки, с временем и без
- [ ] Тест своей команды: шаблон с кавычками/пробелами в prompt не ломает shell (экранирование), код 0 → Done, код ≠0 → Done{is_error}
- [ ] `command()` для Worker у Claude содержит запреты push/remote/sudo и НЕ содержит deny на `Read(*.pem)`; для Reviewer - режим только чтения
- [ ] `available()` у неустановленного движка → Err с подсказкой установки
- [ ] В отчёте: какие флаги подтверждены `--help`, какие - только документацией
