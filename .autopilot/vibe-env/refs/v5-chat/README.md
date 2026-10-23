# v5-chat: визуальные референсы чата для Nightshift

Дата сбора: 2026-10-23. Картинки лежат рядом, имя = `NN-источник-что.png`.

## Что за «VibeDeck»

Название неоднозначно. Найдено несколько разных продуктов:

- **muhBilal/vibedeck** (GitHub) - «Multi-terminal AI Agent Cockpit & IDE Workspace»: Tauri + Vue, несколько агентов (Claude Code, Codex, Gemini и др.) в терминальной сетке плюс файловый менеджер и git-контекст. Ближе всего к запросу, это и берём за основной.
- **vibe-deck.app** - управление локальными AI-инструментами из браузера (отдельный продукт).
- **getvibedeck.com** - Stream Deck-плагин для Claude Code / Codex, платный ($19). Не про чат.
- **ai-vibedeck.com**, **hharsha98/vibedeck** - прочие одноимённые проекты, в этом сборе не использовались.

## Приложения и шоты

**01. VibeDeck Cockpit** (muhBilal/vibedeck, `docs/preview.png`). Тёмная тема, три панели-терминала рядом с плашкой статуса и бейджами агентов (`RUNNING`, `AI ACTIVE`). Цветовая кодировка агентов: Claude оранжевый, Codex зелёный, Gemini фиолетовый. Минус: шрифт моноширинный везде, чат как таковой не виден, это скорее терминал.

**02. Vibe Kanban, обзор** (BloopAI/vibe-kanban, `vibe-kanban-screenshot-overview.png`). Канбан с колонками To do / In progress / In review / Done, справа панель выполнения агента: шаги, вызовы инструментов, поле «Continue working on this task» с выбором модели. Важно: сам проект объявлен закрытым (README: «Vibe Kanban is sunsetting»), брать как образец структуры, а не как живой продукт.

**03. Vibe Kanban, workspace** (тот же репо, `vibe-kanban-screenshot-workspace.png`). Тот же приём, но диффы и чат агента на одном экране: слева список workspace, в центре diff со строками `+/-`, справа панель Git и Notes. Лучший шот про показ изменений агента.

**04. Vibecode, главная** (vibecodeapp.com, скриншот страницы). Тёмная тема, композер с переключателем модели (`Opus 5.5 (Med)`), кнопка `Build it` и чипы-подсказки под полем. Очень чистое поле ввода, это ориентир для нижней панели Nightshift.

**07. Dribbble: AI Chat App UI, iOS dark + voice** (dribbble.com/shots/26530714, [страница](https://dribbble.com/shots/26530714-AI-Chat-App-UI-iOS-Dark-Mode-with-Voice-Pro-Upsell-Tools)). Тёмный фон, ответы без пузыря, быстрые подсказки-пилюли, поле ввода с иконками `+`, настройками и микрофоном. Градиент есть в шапке (`Upgrade Pro`) и обводке поля: акцент, но заметный.

**08. Dribbble: AI Chat Assistant, Dark Mode** (dribbble.com/shots/26233036, [страница](https://dribbble.com/shots/26233036-AI-Chat-Assistant-App-UI-Dark-Mode-Experience)). Тёмный фон, ответы ассистента как белые карточки, поле ввода внизу с иконками камеры и голоса. Брендинг там чужой (ChatGPT-подобный), в Nightshift его не копируем, берём только раскладку.

**09. Cursor, главная** (cursor.com). Режим «агенты»: список `IN PROGRESS 2` и `READY FOR REVIEW 3` с числом минут и счётчиком `+135 -21` по каждой задаче. Самый сильный референс для списка агентов и статусов.

**12. Zed, главная** (zed.dev). Тёмный редактор, агентная панель с задачами (`Add AccessKit support...`) и подписи `Run agents in parallel`. Шот маркетинговый, но показывает, как агентные задачи выглядят в боковой панели.

**13. Claude Code UI, desktop** (siteboon/claudecodeui, `public/screenshots/desktop-main.png`, светлая тема). Двухколоночный макет: слева проекты и сессии со счётчиком сообщений и временем, справа лента. Инструменты показаны как свёрнутые блоки `Using Read` / `Using Write` с `View input parameters` и `Tool Result`. Лучший пример того, как показывать вызовы инструментов. Композер внизу с подсказкой `@ to reference files` и горячими клавишами.

**14. Claude Code UI, mobile** (тот же репо, `mobile-chat.png`). Тот же чат на телефоне: дифф `ToolRenderer.tsx` с красными строками удаления, нижняя панель вкладок `Chat / Shell / Files / Git / Tasks / More`, индикатор использования контекста `15.5%`.

**15. Claude Code UI, Tools Settings** (тот же репо, `tools-modal.png`). Модалка настроек разрешений: переключатель `Skip permission prompts`, список `Allowed Tools` с готовыми кнопками (`Read`, `Edit`, `Bash(git status:*)`). Хороший образец для окна разрешений Nightshift.

## Берём (для Nightshift)

1. Лента: сообщения пользователя справа пузырём, ответы агента без пузыря на всю ширину (13, 07).
2. Вызовы инструментов: свёрнутая строка `Using Read` с раскрытием параметров и результатом, а не сырой лог (13).
3. Диффы: показывать `+`/`-` в тонированном блоке прямо в ленте, со сворачиванием (02, 03, 14).
4. Список агентов с состоянием: группы `In progress` / `Ready for review` и счётчик времени (09, 02).
5. Нижнее поле: одно поле на всю ширину с иконками слева (`+`, вложения, микрофон) и выбором модели прямо в композере (04, 07).
6. Подсказка `Enter to send · Shift+Enter new line · @ to reference files` мелким текстом под полем (13).
7. Статус-бар с числом агентов и процентом контекста (01, 14).
8. Цвет агента как код: по одному акценту на агента (Claude оранжевый, Codex зелёный), в остальном монохром (01).

## Избегаем

- Градиентных пилюль и обводок во всей шапке и поле (07): один акцент на экран достаточно.
- Моноширинного шрифта во всём интерфейсе: в 01 он делает чат похожим на терминал, а для сообщений это неудобно.
- Переусложнённых панелей на одном экране (03, 12): три колонки плюс диффы сразу, новичок не найдёт поле ввода.
- Маркетинговых героев вместо реального интерфейса (04, 09, 12): для внутреннего вида нужны реальные экраны, а не лендинги.
- Копирования чужих брендов и иконок (08 с ChatGPT-подобной графикой).

## Источники

- muhBilal/vibedeck: https://github.com/muhBilal/vibedeck (шот `docs/preview.png`)
- BloopAI/vibe-kanban: https://github.com/BloopAI/vibe-kanban (README, шоты `packages/public/`); сайт https://vibekanban.com (объявлен sunsetting)
- Vibecode: https://www.vibecodeapp.com/ (скриншот главной)
- Cursor: https://cursor.com/ (скриншот главной)
- Zed: https://zed.dev/ (скриншот главной)
- Claude Code UI (siteboon): https://github.com/siteboon/claudecodeui (`public/screenshots/`)
- Dribbble 26530714: https://dribbble.com/shots/26530714-AI-Chat-App-UI-iOS-Dark-Mode-with-Voice-Pro-Upsell-Tools
- Dribbble 26233036: https://dribbble.com/shots/26233036-AI-Chat-Assistant-App-UI-Dark-Mode-Experience
- Другие: https://getvibedeck.com/ и https://vibe-deck.app/ (шотов не взято: getvibedeck.com отдал пустую страницу, vibe-deck.app дал только иконку, ai-vibedeck.com без og:image); Vibespace (hharsha98/vibedeck) описан по README, шота нет.

## Что не вышло

- Mobbin: MCP требует платный план, поиск через него не сработал.
- Pinterest и Behance не смотрели: до них не дошли, задача ограничилась Dribbble и рабочими продуктами.
- Dribbble: из выдачи взяты только 2 шота, поиск по тегам `dark-ui` не делали (паузы 15 с между открытиями).
- Chat-шоты Dribbble и Muzli (`muz.li/inspiration/chat-ui/`) не скачаны, поэтому чат-референсов из галерей 2 из 11.
