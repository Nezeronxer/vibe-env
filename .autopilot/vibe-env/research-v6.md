# Сравнение: есть ли у аналогов семь функций (первоисточники)

Дата проверки: 2026-10-23. Читал только через OpenCLI в Chrome (сессия v6res), для кода на GitHub дополнительно `gh api`. Каждая ссылка ниже была открыта в браузере; для файлов кода текст брал из DOM страницы GitHub или через `gh api` (то же содержимое).

Охват: Claude Code Desktop, OpenAI Codex (приложение + CLI), Cursor, Conductor, Vibe Kanban, opcode, claudecodeui (CloudCLI), GitHub Copilot (cloud agent и приложение Copilot), Jules. Devin открывал одну страницу (GitHub-интеграция). Crystal: репозиторий помечен как устаревший (deprecated, февраль 2026, заменён на Nimbalyst), его функции не проверял. Vibe Kanban: проект закрывается (см. https://www.vibekanban.com/blog/shutdown, открыта), но репозиторий и доки есть.

---

## 1. Вставка/перетаскивание картинок (скриншотов) в чат агента

**Вердикт: подтверждено у 7 продуктов** (Claude Code Desktop, Cursor, GitHub Copilot, Vibe Kanban, opcode, claudecodeui, Codex; ещё Jules на старте задачи).

| Продукт | Что именно | Ссылка (открыта) |
|---|---|---|
| Claude Code Desktop | Картинки, PDF и другие файлы прикрепляются кнопкой **+** или перетаскиванием прямо в поле ввода; прямо названо «скриншоты багов, макеты» | https://code.claude.com/docs/en/desktop |
| Cursor | «Image input»: перетащить файл в чат или вставить из буфера Cmd+V/Ctrl+V, включая скриншоты | https://cursor.com/docs/agent/prompting |
| GitHub Copilot cloud agent | В промпт можно добавить скриншоты и макеты вставкой, перетаскиванием или загрузкой (png, jpeg, gif, webp) | https://docs.github.com/en/copilot/how-tos/use-copilot-agents/cloud-agent/use-cloud-agent-on-github |
| Vibe Kanban | «Attaching Images»: кнопка-скрепка и drag-and-drop картинок в чат | https://github.com/BloopAI/vibe-kanban/blob/main/docs/workspaces/chat-interface.mdx |
| opcode | В `FloatingPromptInput.tsx` есть `handlePaste` (ветка `image/`), слушатель Tauri drag-drop и превью встроенных картинок | https://raw.githubusercontent.com/winfunc/opcode/main/src/components/FloatingPromptInput.tsx |
| claudecodeui | `useChatComposerState.ts`: при вставке из буфера берутся элементы `image/*` и файлы-картинки | https://github.com/siteboon/claudecodeui/blob/main/src/modules/chat/hooks/useChatComposerState.ts |
| OpenAI Codex | Приложение: changelog 26.205 «Attach or drop any file type», 26.417 «non-image file pasting in the composer» (то есть картинки вставляются). Страница про картинки: вставка в интерактивный композер, `codex -i`, веб-композер: attach/paste/drag | https://developers.openai.com/codex/changelog и https://developers.openai.com/codex/image-inputs |
| Jules | Картинки drag-and-drop или выбором файла только при создании задачи (до 5 МБ), в последующих сообщениях нельзя | https://jules.google/docs/running-tasks/ |

Оговорка: у Conductor в доке сказано «Attach notes, specs, screenshots, and logs» (https://www.conductor.build/docs/first-workspace, открыта), но способ (вставка/drag) не указан, поэтому в счёт не беру.

Не проверено: Sculptor, Terragon, Devin.

---

## 2. GitHub-issue как задача агента (и закрытие/привязка issue при мерже)

**Вердикт: «issue -> задача» подтверждено у 5 продуктов; «закрытие/привязка issue при мерже» не нашёл ни в одном официальном источнике.**

| Продукт | Что именно | Ссылка (открыта) |
|---|---|---|
| GitHub Copilot cloud agent | Назначить issue на Copilot (Assignees -> Copilot): агент берёт заголовок, описание и комментарии, делает PR и просит ревью | https://docs.github.com/en/copilot/how-tos/use-copilot-agents/cloud-agent/use-cloud-agent-on-github |
| Приложение GitHub Copilot (десктоп) | «Starting a session from an issue»: **New session** с уже загруженным контекстом issue; есть agent merge, который сам вливает PR и выключается после мержа | https://docs.github.com/en/copilot/how-tos/github-copilot-app/managing-issues-and-pull-requests |
| Jules | Метка `jules` на любом GitHub-issue запускает задачу (запись в changelog) | https://jules.google/docs/changelog/ |
| Conductor | Workspace можно создать из ветки, PR, GitHub-issue или Linear-issue; есть гайд «From issue to PR» (агент наследует заголовок и описание; после мержа гайд советует архивировать workspace, закрытия issue в тексте нет) | https://www.conductor.build/docs/guides/issue-to-pr и https://www.conductor.build/docs/concepts/workflow |
| Cursor | Cloud agent запускается комментарием `@cursor` в GitHub PR или issue; из Linear можно делегировать issue агенту | https://cursor.com/docs/cloud-agent и https://cursor.com/docs/integrations/linear |
| Claude Code (GitHub Actions, не Desktop) | `@claude` в issue или комментарии запускает агента в workflow репозитория | https://code.claude.com/docs/en/github-actions |

Про «закрытие/привязку при мерже»: прямых слов вроде «PR закроет issue» (`Closes #N`) ни в доках Copilot, Jules, Conductor, Cursor, ни в Claude Code не нашёл. Ближайшее: у Cursor для Linear «Cloud Agents show real-time status in Linear and create PRs automatically» (статус в трекере, но это Linear, не GitHub); у Claude Code Desktop авто-архив сессии после мержа PR (это сессия, не issue). У Claude Code Desktop самого импорта GitHub-issue в доке Desktop не нашёл. Devin (https://docs.devin.ai/integrations/gh, открыта): триггер по комментарию на открытом PR, комментарии на issue там игнорируются.

Не проверено: Sculptor, Terragon, Vibe Kanban (у него свои kanban-issue, а не GitHub), opcode, claudecodeui (в README есть только сторонний плагин «GitHub Issues Board»).

---

## 3. Показ и продолжение существующих сессий Claude Code CLI (`~/.claude/projects`) в GUI

**Вердикт: подтверждено у 4 продуктов.**

| Продукт | Что именно | Ссылка (открыта) |
|---|---|---|
| Claude Code Desktop | `/resume` в Desktop показывает сессии, начатые из CLI на этом компьютере (поиск по названию, папке, ветке, превью); продолжает ту же сессию, не копию. Обратно: `/desktop` и `claude --desktop --resume <id>` | https://code.claude.com/docs/en/desktop |
| opcode | «Visual Project Browser: все проекты из `~/.claude/projects/`», «Session History: просмотр и возобновление прошлых сессий» | https://github.com/winfunc/opcode |
| claudecodeui (CloudCLI) | «all your existing sessions are discovered automatically», «auto-discovers every session from your `~/.claude` folder» | https://github.com/siteboon/claudecodeui/blob/main/README.md |
| OpenAI Codex app | Импорт из Claude Code: инструкции, настройки, навыки, проекты и недавние чаты; авто-синхронизация в Settings > Import (запись changelog 2026-08-11) | https://developers.openai.com/codex/import и https://developers.openai.com/codex/changelog |

Не проверено: Cursor, Conductor (у него свои workspaces, импорта `~/.claude/projects` в доках не видел), Sculptor, Terragon, Vibe Kanban.

---

## 4. Уведомление и автообновление самого десктоп-приложения

**Вердикт: подтверждено у 5 продуктов.**

| Продукт | Что именно | Ссылка (открыта) |
|---|---|---|
| Claude Code Desktop | «On macOS and Windows the app auto-updates on launch»; Desktop может сам перезапуститься для установки скачанного обновления; ручная проверка: Claude > Check for Updates… (macOS) / Help > Check for Updates… (Windows); политики MDM управляют автообновлением | https://code.claude.com/docs/en/desktop |
| Conductor | Changelog 0.90.0: «Automatic restarts for updates wait for you to confirm when a run script is running or an open chat has an unsent prompt» | https://www.conductor.build/changelog |
| Cursor | Release notes (24 сен 2026): уведомление о новой версии для установок .deb/.rpm с кнопками Update (команда обновления) и Later | https://cursor.com/docs/release-notes |
| claudecodeui | `VersionUpgradeModal`: показывает заметки о релизе и запускает обновление (npm/git) | https://github.com/siteboon/claudecodeui/blob/main/src/modules/version-upgrade/VersionUpgradeModal.tsx |
| OpenAI Codex app | Changelog 26.818: «Eligible local chats interrupted by an app update now resume automatically after the app restarts» | https://developers.openai.com/codex/changelog |

Не проверено: Vibe Kanban, opcode (апдейтер в Tauri-конфиге не искал подробно), Sculptor, Terragon.

---

## 5. История расходов и использования по дням/проектам внутри приложения (как ccusage)

**Вердикт: подтверждено у 3 продуктов с разбивкой по времени/проекту; у ещё двух есть только расходы в рамках сессии.**

| Продукт | Что именно | Ссылка (открыта) |
|---|---|---|
| opcode | «Usage Analytics Dashboard»: расходы и токены по модели, проекту и периоду, графики трендов, экспорт; «Monitor costs by model, project, and date» | https://github.com/winfunc/opcode |
| Conductor | Changelog 0.91.0: профиль участника команды показывает heatmap его **ежедневного** использования токенов (для командных организаций) | https://www.conductor.build/changelog |
| Cursor | Settings > Plan & Usage (график накопленных расходов), страницы Usage с фильтрами и выбором дат; кольцо использования в статус-баре чата | https://cursor.com/docs/release-notes |
| Claude Code (CLI; в Desktop только кольцо) | `/usage` (алиасы `/cost`, `/stats`): стоимость сессии, лимиты плана, статистика активности; в Desktop клик по кольцу рядом с выбором модели показывает использование плана и контекста, истории по дням не нашёл | https://code.claude.com/docs/en/commands и https://code.claude.com/docs/en/desktop |
| claudecodeui | Команда `/cost` в чате открывает модал с токенами сессии; по дням/проектам встроенного нет, только сторонние плагины в README | https://github.com/siteboon/claudecodeui/blob/main/README.md |

Не проверено: Codex app (в changelog нашёл «usage stats и графики токенов» только в записях iOS-приложения, их не засчитал), Sculptor, Terragon, Vibe Kanban (есть индикатор контекста, не историю).

---

## 6. Кнопка «собрать диагностику / выгрузить логи» для баг-репорта

**Вердикт: подтверждено у 2 десктоп-приложений + у CLI-аналогов тех же продуктов.**

| Продукт | Что именно | Ссылка (открыта) |
|---|---|---|
| Claude Code Desktop | Help > Troubleshooting > Show Logs in Finder / File Explorer / File Manager: zip с логами Desktop (в нём `ssh.log`); док советует просмотреть файлы перед отправкой, в них могут быть имена хостов | https://code.claude.com/docs/en/desktop |
| OpenAI Codex app | Слэш-команда `/feedback` в композере: «Open the feedback dialog to submit feedback and optionally include logs»; в changelog 26.901 отдельно показан результат загрузки диагностики | https://developers.openai.com/codex/reference/slash-commands и https://developers.openai.com/codex/changelog |
| Claude Code CLI | `/bug` и `/feedback`: отчёт с выбором объёма истории сессии и экраном согласия; `/doctor` диагностирует установку и настройки | https://code.claude.com/docs/en/commands |
| Codex CLI | `codex doctor` - локальный диагностический отчёт для обращения в поддержку; `/feedback`: «Send logs to the Codex maintainers» | https://developers.openai.com/codex/cli/slash-commands |

Побочно: у Jules в верхней панели есть кнопка feedback (без логов), https://jules.google/docs/feedback/ (открыта). У Conductor, Cursor, Vibe Kanban, opcode, claudecodeui кнопку выгрузки диагностики в доках не нашёл (у Vibe Kanban только запуск с `RUST_LOG=debug`).

Не проверено: Sculptor, Terragon, Devin.

---

## 7. Индикатор «думает» сразу после отправки (анимация, таймер/меняющиеся слова) и плавный стрим текста

**Вердикт: подтверждено у 4 продуктов (в основном как терминальный/чатовый индикатор); «плавный стрим» явно описан у Cursor.**

Отдельное подтверждение про Claude Code CLI (запрошено): **да, есть и вращающиеся глаголы, и таймер.**

| Продукт | Что именно | Ссылка (открыта) |
|---|---|---|
| Claude Code CLI | Док `spinnerVerbs`: «While a turn is in progress, the spinner shows a rotating verb such as "Accomplishing", "Architecting", or "Baking"» (список можно дополнить или заменить) | https://code.claude.com/docs/en/settings-reference |
| Claude Code CLI | Changelog: 2.1.145 «Fixed the spinner and elapsed-time display freezing…» (то есть таймер есть); 2.1.141 спиннер «warms to amber after 10 seconds» при долгом размышлении; 2.1.271 хук-статус в спиннере «with elapsed time»; 2.1.144 «past-tense built-ins like "Worked for 5s"» после хода; запись о строке статуса размышления с обновлением «elapsed time and token counts» | https://code.claude.com/docs/en/changelog |
| claudecodeui | `ActivityIndicator.tsx`: список слов (thinking, processing, analyzing, working, computing, reasoning) меняется каждые 4 секунды (`elapsedSeconds / 4`), рядом счётчик «Ns / Nm Ns» с `setInterval` 1 с, пульсирующая точка и кнопка прерывания | https://github.com/siteboon/claudecodeui/blob/main/src/modules/chat/composer/ActivityIndicator.tsx |
| Codex CLI (TUI) | `status_indicator_widget.rs` и тесты: строка `Working (0s • esc to interrupt)` со счётчиком секунд | https://github.com/openai/codex/blob/main/codex-rs/tui/src/status_indicator_widget.rs |
| Cursor | Release notes: «Smoother scrolling while an agent streams» - чат плавно «скользит» к новому тексту вместо рывков (окт и сен 2026); в сводке хода показывается «Worked for» с длительностью | https://cursor.com/docs/release-notes |

Не проверено: индикатор «думает» в самом Claude Code Desktop и в Codex app (в доках Desktop описан только режим просмотра Thinking, https://code.claude.com/docs/en/desktop; про анимацию и таймер там ничего нет); Conductor (в changelog только поиск по «thinking» и общая полировка); Sculptor, Terragon, Devin, Jules.

---

## Общие оговорки

- Доки меняются: часть страниц OpenAI вынесена на learn.chatgpt.com, а у Codex записи changelog для iOS и десктопа идут вперемешку; я засчитывал только записи с заголовком «Codex app».
- Страница https://code.claude.com/docs/en/settings-reference длиннее 200 тысяч символов, раздел `spinnerVerbs` лежит во втором фрагменте; цитата сверена по тексту открытой страницы.
- Что искал и не нашёл (отрицательные результаты), помечено «не нашёл» выше; это значит «нет в просмотренных страницах», а не «функции точно нет».
