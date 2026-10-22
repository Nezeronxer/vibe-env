# Разбор аналогов: терминал для Nightshift (G33-G35, G49)

Дата: 2026-10-23. Исследование, код приложения не писался. Что прочитано: README и доки продуктов, ветка Show HN про cmux, блоги «почему перешёл на Ghostty», обзоры Warp. Звёзды, лицензии, релизы - GitHub API (`https://api.github.com/repos/<владелец>/<репо>`, снято 23.10.2026); версии пакетов - реестры npm и crates.io. Рекламные «сравнения» (nimbalyst.com, agentsroom.dev, aitoolsbox) пристрастны - из них взято только описание устройства, не оценки. Живых отзывов с Reddit не читал (поиск отдал блоги), поэтому «хвалят» ниже = блоги, HN, README. Что не проверено - в конце.

## 1. Аналоги: что хвалят и чем выделяется дизайн

Терминалы:
- **Warp** ([repo](https://github.com/warpdotdev/warp), Rust, 65,6 тыс. звёзд, клиент AGPL-3.0, UI-крейты MIT, [релизы](https://github.com/warpdotdev/warp/releases)).
  Хвалят: Blocks - команда и её вывод одной единицей, можно копировать, искать, закладывать, делиться ([доки](https://docs.warp.dev/terminal/blocks)); объяснение ошибки прямо под упавшей командой ([обзор](https://dev.to/jovan_chan_9500711396d4e6/warp-terminal-review-2026-open-source-ade-the-20-build-plan-and-who-should-actually-pay-for-it-5cin)); свои ключи моделей (BYOK) в платном плане (тот же обзор); запуск сторонних CLI-агентов (Claude Code, Codex) внутри ([README](https://github.com/warpdotdev/warp)).
  Дизайн: вывод разбит на карточки-блоки, ввод как в редакторе. Минус по отзывам: возможности не переносятся в другой терминал ([обзор](https://automationswitch.com/ai-coding-assistants/warp)).
- **Ghostty** ([repo](https://github.com/ghostty-org/ghostty), Zig, 62,5 тыс., MIT, тег v1.3.1).
  Хвалят: скорость на потоке вывода Claude Code, где iTerm2 лагал ([блог](https://cedricsarigumba.com/posts/why-i-use-ghostty-as-my-terminal-on-macos/)); один текстовый конфиг, перезагрузка по Cmd+, (там же); сотни встроенных тем, пара `light:X,dark:Y` ([доки](https://ghostty.org/docs/features)); Quick Terminal - выезжает сверху по хоткею (доки там же).
  Дизайн: родные для ОС вкладки и сплиты, отступы вокруг текста (`window-padding` 8-10 px), лёгкая прозрачность с размытием, вкладки в заголовке окна ([конфиг из блога](https://cedricsarigumba.com/posts/why-i-use-ghostty-as-my-terminal-on-macos/)).
- **WezTerm** ([repo](https://github.com/wezterm/wezterm), Rust, 29,3 тыс.; последний релиз 2024-02-03, но коммиты идут до 19.10.2026; лицензию API не определил).
  Хвалят: конфиг на Lua с горячей перезагрузкой, мультиплексор локально и по SSH (панели/вкладки/окна), поиск по скроллбэку, SSH-клиент с родными вкладками ([features](https://wezterm.org/features.html)). Из него же берётся PTY-крейт (раздел 3).
- **kitty** ([repo](https://github.com/kovidgoyal/kitty), 35,4 тыс., GPL-3.0, [v0.49.2](https://github.com/kovidgoyal/kitty/releases)).
  Хвалят: всё с клавиатуры, раскладки окон как в тайловом менеджере, конфиг одним файлом (кладут в git), «kittens» - расширения, прыжок к предыдущему приглашению оболочки `ctrl+shift+z` ([обзор](https://sw.kovidgoyal.net/kitty/overview/)).
- **iTerm2** ([repo](https://github.com/gnachman/iTerm2), 18,2 тыс., GPL-2.0).
  Хвалят: неактивные панели чуть затемнены, поэтому видно активную; hotkey-окно; поиск с regex; история вставки; «мгновенный повтор» - прокрутка во времени ([features](https://iterm2.com/features.html)).
- **Wave Terminal** ([repo](https://github.com/wavetermdev/waveterm), 22,6 тыс., Apache-2.0, [v0.14.5](https://github.com/wavetermdev/waveterm/releases)).
  Хвалят: перетаскиваемые блоки (терминал, редактор, браузер, ИИ в одной раскладке), разворот любого блока на весь экран и назад, SSH-сессии, переживающие обрыв сети и перезапуск, предпросмотр файлов (md, картинки, PDF), своя CLI `wsh`, фоновые картинки и темы вкладок ([README](https://github.com/wavetermdev/waveterm)).
- **Tabby** ([repo](https://github.com/Eugeny/tabby), TypeScript, 75,1 тыс., MIT, [v1.0.238](https://github.com/Eugeny/tabby/releases)).
  Хвалят: менеджер SSH/serial, запоминает вкладки, многоаккордные горячие клавиши, плагины и темы, «не давится быстрым выводом», шифрованное хранилище секретов ([README](https://github.com/Eugeny/tabby)). Для нас: терминал на TypeScript с 75 тыс. звёзд (стек внутри не проверял).
- **Rio** ([repo](https://github.com/raphamorim/rio), Rust, 7,6 тыс., MIT, [v0.5.28](https://github.com/raphamorim/rio/releases)).
  Выделяется эффектами: README показывает сплиты с CRT-шейдером на macOS и размытый фон на Linux. Декоративные эффекты - не наш путь; интересно разве что размытие фона как опция.
- **cmux** ([repo](https://github.com/manaflow-ai/cmux), Swift, 28,4 тыс., [v0.65.2](https://github.com/manaflow-ai/cmux/releases); лицензия противоречива: README пишет GPL, пост на HN - AGPL, API - NOASSERTION; код не копируем).
  Хвалят (автор и комментаторы [Show HN](https://news.ycombinator.com/item?id=47079718), 198 очков): синее кольцо вокруг панели и подсветка вкладки, когда агенту нужен человек; Cmd+Shift+U - к последнему непрочитанному; вертикальные вкладки с веткой git, PR, портами и текстом последнего уведомления; встроенный скриптуемый браузер рядом; CLI и сокет для автоматизации; читает конфиг Ghostty ([README](https://github.com/manaflow-ai/cmux)).
  Что просил один из комментаторов: палитру команд Cmd+Shift+P, зум панели, свои хоткеи (тот же тред). Предупреждение автора: «большинство оркестраторов - Electron/Tauri, и производительность меня раздражала» - это наш риск, см. раздел 3.
- **Zed, терминал** ([repo](https://github.com/zed-industries/zed), 92 тыс., v1.25.2; лицензию API не определил).
  Терминал - панель снизу/сбоку или вкладка в центре рядом с файлами; рабочая папка по проекту; деление Cmd+D; определение venv ([доки](https://zed.dev/docs/terminal)). Отзывов не читал - только доки.

Оболочки над CLI-агентами:
- **Conductor** (Melty Labs, закрытый, только macOS на Apple Silicon по [обзору](https://agentsroom.dev/blog/best-multi-agent-coding-tools)). Рабочее пространство = worktree + ветка + терминал + diff + путь к PR; старт из задачи GitHub/Linear/PR, скрипты подготовки, копирование `.env`, Cmd+Shift+N ([доки](https://www.conductor.build/docs/guides/parallel-agents/run-multiple-claude-code-sessions)). Хвалят приятное ревью diff (обзор выше).
- **Claude Squad** ([repo](https://github.com/smtg-ai/claude-squad), Go, 8,6 тыс., AGPL-3.0, [v1.0.20](https://github.com/smtg-ai/claude-squad/releases)). tmux + worktree на агента, ревью до применения, автопринятие запросов. Минусы: только терминал, нужны tmux и gh, нет общего обзора.
- **Crystal** ([repo](https://github.com/stravu/crystal)): объявлен устаревшим в феврале 2026, преемник закрытый Nimbalyst. **Vibe Kanban**: Bloop закрыла проект в апреле 2026 ([источник](https://agentsroom.dev/blog/best-multi-agent-coding-tools)). Урок: категория нестабильна, не зависеть от чужого движка.
- **opencode TUI** ([repo](https://github.com/anomalyco/opencode), 220 тыс., MIT, [v1.19.4](https://github.com/anomalyco/opencode/releases)): `@` - нечёткий поиск файла, `!` - команда оболочки, `/` - слэш-команды, лидер-клавиша `ctrl+x` ([доки](https://opencode.ai/docs/tui/)).
- Ранее прочитано в `research-ui.md`: Superset (статус агента в сайдбаре, звук при завершении, diff с комментариями агенту).

## 2. Что взять в агентный терминал Nightshift

Сложность: S - до дня, M - 2-4 дня, L - неделя+. Оценки мои, по объёму кода, не измерены.

| # | Функция | Откуда | Зачем | Сл. |
|---|---|---|---|---|
| 1 | Рабочее пространство = worktree + ветка + панели терминала | Conductor, Claude Squad | Агенты не затирают друг друга; поле `worktree` в ядре уже есть (`core/src/model.rs`) - привязать к нему панели | M |
| 2 | Вертикальный сайдбар: ветка, PR, порты, последняя строка уведомления | cmux | При 6+ вкладках заголовки нечитаемы | M |
| 3 | Кольцо внимания на панели + «к последнему непрочитанному» (сигналы OSC 9/99/777 и хуки агентов) | cmux | Один сигнал «нужен ты» - совпадает с янтарём `--human` | S |
| 4 | Сплиты, зум панели, приглушение неактивных, перетаскивание | iTerm2, Wave, запрос на HN | Основа «сплитов» из брифа | M |
| 5 | Палитра команд Cmd+Shift+P и команды проекта из файла | cmux, Zed | Открываемость; её просили на HN | S |
| 6 | Метки приглашений (OSC 133), прыжок к прошлой/следующей команде, «копировать вывод команды» | kitty, Warp Blocks | Читать длинный вывод агента; без своего редактора ввода | M |
| 7 | Конфиг-файл с горячей перезагрузкой, темы парой light/dark | Ghostty, WezTerm, kitty | Кастомизация из брифа; конфиг можно положить в git | M |
| 8 | Быстрый терминал по глобальной клавише (выезжает сверху) | Ghostty, iTerm2 | Терминал в одно нажатие; плагин `tauri-plugin-global-shortcut` уже в `app/Cargo.toml` | S |
| 9 | Сессия живёт в ядре, окно только прикрепляется; вывод возвращается при открытии | Wave, Tabby, Ghostty (`window-save-state`) | Нужно для отдельных окон и перезапуска интерфейса | M |
| 10 | Вынос панели/вкладки в отдельное окно | WezTerm, Ghostty | «Отдельные окна» из брифа; зависит от №9 | M |
| 11 | Поиск по скроллбэку, кликабельные ссылки и пути `файл:строка` | WezTerm, iTerm2 | Дёшево (готовые аддоны) и сразу полезно | S |
| 12 | Панель diff/checks/PR рядом с терминалом | Conductor, cmux (браузер) | Связка с GitHub из брифа (публикация, история) | L |

Не брать: блоки Warp целиком (нужен свой редактор ввода и интеграция с оболочкой, привязка к продукту); встроенную генерацию команд ИИ (дублирует агентов); размытие фона окна как обязательное (см. вопрос ниже).

### Изюминка без крема, градиентов, неона и кнопок-таблеток
Идеи опираются на разобранное выше; это предложения, не измерения.
- **Панели как отдельные скруглённые карточки** с зазором 6-8 px, в зазоре виден фон окна; вместо линий-разделителей. Радиус панели 10-12 px, вкладки и поля 8 px, мелкие элементы 6 px; внешний радиус = внутренний + зазор. Сейчас `--r: 4px` в `ui/css/tokens.css`.
- **Внутренний отступ холста 10-12 px** (как `window-padding` в Ghostty), межстрочный интервал чуть шире (`adjust-cell-height = 2` в том же конфиге) - текст «дышит».
- **Активная панель - не рамка, а контраст**: неактивные приглушены (iTerm2), у активной тонкая линия `--line2`. Кольцо внимания - плоская янтарная линия 1,5 px без свечения; янтарь по-прежнему только для «нужен человек».
- **Вкладки - строки-прямоугольники** со скруглением 8 px в вертикальном сайдбаре; выбранная = `--raised`, не цветная заливка. Значок роли - квадрат 8 px со скруглением 2 px (одна форма и цвет на роль, как в `research-ui.md`).
- **Шрифт**: один моноширинный с вариативным весом, лигатуры выключены по умолчанию; интерфейс - системный `--sans`. Размер 13-14 px, на тёмном фоне вес чуть выше обычного (аналог `font-thicken` из конфига Ghostty).
- **Движение 120-160 мс `ease-out`**: появление/закрытие панели (анимация `flex-grow`), выезд быстрого терминала сверху (как в Ghostty), перестановка вкладок. Без пружин и отскоков; `prefers-reduced-motion` отключает всё.
- **Темы**: графит (текущая), холодный синий, мох, светлая серая; каждая парой light/dark; без кремовых и бежевых фонов.
- **Хром окна**: у `titleBarStyle` в Tauri есть `"Overlay"` (содержимое под заголовком; нужна своя зона перетаскивания, окно не тянется, пока не в фокусе) и `trafficLightPosition` ([конфиг Tauri](https://v2.tauri.app/reference/config/)). Родное скругление окна macOS остаётся - прозрачность не нужна.

Вопрос к пользователю (не блокирует): прозрачное окно с размытием на macOS требует `macOSPrivateApi` - по [доке Tauri](https://v2.tauri.app/reference/config/) он «включает API прозрачного фона». По моим сведениям это приватные API Apple и они мешают публикации в App Store; на этой странице такой оговорки не нашёл - проверить, если App Store нужен. По умолчанию делаем без прозрачности.

## 3. Технически: Tauri 2 + ванильный JS

Сейчас в проекте: `tauri = "2"` ([актуальный 2.12.5](https://crates.io/crates/tauri), Apache-2.0 OR MIT), `withGlobalTauri: true`, `frontendDist: ../ui`, CSP `script-src 'self'; style-src 'self'` (`app/tauri.conf.json`). Собственного PTY нет (`app/src/term.rs` только запускает внешний терминал).

### 3.1 Эмулятор: xterm.js 6.0.0, без npm-сборки
Версии - `dist-tags.latest` в реестре ([@xterm/xterm](https://registry.npmjs.org/@xterm/xterm)), размеры - мои замеры распакованных tgz из реестра; лицензия у всех MIT.

| Пакет | Версия | Файл для копирования | Байт | gzip |
|---|---|---|---|---|
| [@xterm/xterm](https://www.npmjs.com/package/@xterm/xterm) | 6.0.0 (2025-12-22) | `lib/xterm.js`, `css/xterm.css` | 488 663 + 7 112 | 120 477 + 2 512 |
| [addon-fit](https://www.npmjs.com/package/@xterm/addon-fit) | 0.11.0 | `lib/addon-fit.js` | 1 521 | 657 |
| [addon-webgl](https://www.npmjs.com/package/@xterm/addon-webgl) | 0.19.0 | `lib/addon-webgl.js` | 247 535 | 66 945 |
| [addon-search](https://www.npmjs.com/package/@xterm/addon-search) | 0.16.0 | `lib/addon-search.js` | 78 826 | 22 196 |
| [addon-unicode11](https://www.npmjs.com/package/@xterm/addon-unicode11) | 0.9.0 | `lib/addon-unicode11.js` | 52 489 | 16 426 |
| [addon-web-links](https://www.npmjs.com/package/@xterm/addon-web-links) | 0.12.0 | `lib/addon-web-links.js` | 3 100 | 1 445 |
| [addon-serialize](https://www.npmjs.com/package/@xterm/addon-serialize) | 0.14.0 | `lib/addon-serialize.js` | 16 441 | 5 152 |

Итого набор из шести первых строк (без serialize, с CSS): 879 246 байт (≈859 КиБ), gzip 230 658. Каждый - один минимизированный файл без зависимостей.
- **Без сборки можно**: файлы `lib/*.js` - UMD (заголовок `(function(e,t){...})(globalThis,...)`), подключаются обычным `<script src="vendor/xterm/xterm.js">`. Глобалы: `Terminal`; аддоны - `new FitAddon.FitAddon()`, `WebglAddon.WebglAddon`, `SearchAddon.SearchAddon`, `Unicode11Addon.Unicode11Addon`, `WebLinksAddon.WebLinksAddon`, `SerializeAddon.SerializeAddon`. Проверено запуском в песочнице Node для fit, unicode11, web-links, serialize; для xterm, webgl, search заголовок тот же, но без DOM они не поднялись - проверить в окне.
- **Как положить**: один раз скачать `https://registry.npmjs.org/@xterm/xterm/-/xterm-6.0.0.tgz` (и аддоны), скопировать файлы выше и `LICENSE` (MIT требует сохранить notice) в `ui/vendor/xterm/`, в `VERSIONS.txt` записать версии и sha256. `.map` не копировать.
- **Что изменилось в 6.0** ([релиз](https://github.com/xtermjs/xterm.js/releases/tag/6.0.0)): canvas-рендерер удалён (#5105) - остаются WebGL и DOM; новый скролл-бар из VS Code (#5096); синхронизированный вывод DEC 2026 (#5453) - TUI вроде Claude Code мерцают меньше; удалены `windowsMode` и `fastScrollModifier`; `alt` больше не подменяется на `ctrl+стрелки`. Пакет `@xterm/addon-canvas` (0.7.0, 2024-04-05) для 6.x не нужен.
- **Риск CSP**: xterm создаёт `<style>` и пишет в `textContent` (код `lib/xterm.js`: скролл-бар, DOM-рендерер). При `style-src 'self'` такие стили будут отклонены. Ожидаемое решение - `style-src 'self' 'unsafe-inline'` при неизменном `script-src 'self'`; проверить в прототипе на WKWebView, не принимать на веру.
- **WebGL**: у браузеров ограничено число живых WebGL-контекстов (число не измерял). Включать аддон только у видимых панелей, у скрытых - `dispose()` аддона и DOM-рендерер; проверить на 8+ панелях.
- **Unicode**: `addon-unicode11` + `term.unicode.activeVersion = '11'`. Есть и [addon-unicode-graphemes 0.4.0](https://www.npmjs.com/package/@xterm/addon-unicode-graphemes) (57 947 байт) для составных эмодзи - не пробовал, сравнить в прототипе.
- **Лигатуры**: [addon-ligatures 0.10.0](https://www.npmjs.com/package/@xterm/addon-ligatures) поставляется только как `.mjs` (203 978 байт), UMD нет - позже, через `<script type="module">`.
- **API для наших функций** (есть в `typings/xterm.d.ts`): `parser.registerOscHandler(id, cb)` - OSC 133 и уведомления; `registerMarker`, `registerDecoration` - метки приглашений; `registerLinkProvider` - `файл:строка`; `write(Uint8Array, cb)` - сырые байты pty.
- **Альтернативы**: [ghostty-web 0.4.0](https://www.npmjs.com/package/ghostty-web) (MIT, 3 тыс. звёзд, WASM ≈400 КБ, API совместим с xterm по [README](https://github.com/coder/ghostty-web); версия 0.x, совместимость аддонов не проверял) - присматриваться, не брать сейчас. Разбор экрана на стороне Rust нужен только для статуса агента; для него хватит простого сканера OSC-последовательностей, а [vt100 0.18.1](https://crates.io/crates/vt100) (MIT) или [alacritty_terminal 0.26.0](https://crates.io/crates/alacritty_terminal) (Apache-2.0) - если понадобится полная модель экрана.

### 3.2 PTY: portable-pty 0.9.0
[portable-pty 0.9.0](https://crates.io/crates/portable-pty): MIT, часть WezTerm, обновлён 2025-02-11, 22,9 млн загрузок; macOS, Linux, Windows (ConPTY - задел на будущее) ([docs.rs](https://docs.rs/portable-pty/0.9.0/portable_pty/)). Новее версии нет.
Альтернативы: [pty-process 0.5.4](https://crates.io/crates/pty-process) (MIT, обновлён 2026-10-19; Unix-кандидат, описание не читал); [nix 0.31.3](https://crates.io/crates/nix) (MIT, низкоуровневые вызовы ОС); готовый [tauri-plugin-pty 0.3.1](https://crates.io/crates/tauri-plugin-pty) (MIT, 51 тыс. загрузок; README: «Developing!») - не берём: плагин навязывает свой JS-API, а своё ядро короче.

**Проверено запуском на этой машине (macOS)**: проба на `portable-pty =0.9.0` запустила `/bin/sh`, прочитала вывод, записала команду, сменила размер на 120x40, `stty size` ответил `40 120`, процесс завершился кодом 0. Основа рабочего кода:
```rust
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
let pair = native_pty_system().openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })?;
let mut cmd = CommandBuilder::new("/bin/sh");          // оболочка человека; аргументы списком, не строкой
cmd.env("TERM", "xterm-256color"); cmd.env("COLORTERM", "truecolor");
let mut child = pair.slave.spawn_command(cmd)?;
drop(pair.slave);                                       // slave закрыть после spawn
let mut reader = pair.master.try_clone_reader()?;       // читать в отдельном std::thread (блокирующий)
let mut writer = pair.master.take_writer()?;            // писать: writer.write_all(bytes) (держать в Mutex)
pair.master.resize(PtySize { rows: 40, cols: 120, pixel_width: 0, pixel_height: 0 })?;  // по ResizeObserver
```
Практика (из общей логики pty, отдельно не измерялась): оболочку запускать логином (`-l`), как Terminal.app; у приложения, запущенного через Finder, нет `LANG` и нормального `PATH` - переиспользовать `app/src/pathfix.rs` и задать `LANG=...UTF-8`; `TERM_PROGRAM=Nightshift` для интеграции оболочки; завершение - `child.clone_killer()`; сессии - `HashMap<id, Session>` в состоянии Tauri.

### 3.3 Вывод PTY в webview без лагов
По [доке Tauri](https://v2.tauri.app/develop/calling-frontend/): события (`emit`) не рассчитаны на малую задержку и большой поток, полезная нагрузка - JSON-строка; для потока и порядка нужны **каналы** (`Channel`). По [docs.rs/Channel](https://docs.rs/tauri/2.12.5/tauri/ipc/struct.Channel.html): сообщения идут строго по порядку; бинарные данные без JSON - только через `Channel<InvokeResponseBody>` с `InvokeResponseBody::Raw(Vec<u8>)`, в JS приходят как `ArrayBuffer`; **`Channel<Vec<u8>>` так не работает** - уйдёт JSON-массив чисел.
Схема:
1. Поток чтения на сессию читает до 64 КиБ и склеивает куски: ждёт первый, затем добирает всё, что пришло за ~4 мс или до 64 КиБ, и шлёт одно `Raw`. Так 60 кадров/с не раздуваются тысячами мелких сообщений. Числа 4 мс и 64 КиБ - стартовые, подобрать замером.
2. Байты не переводить в строку на Rust: UTF-8-символ может разорваться на границе куска. `term.write(new Uint8Array(buf))` - xterm рассчитан на сырые байты pty и трактует их как UTF-8 (комментарий к `write` в `typings/xterm.d.ts` пакета 6.0.0). Тест: 4-байтовое эмодзи через границу куска.
3. Обратное давление: у канала его нет. По [руководству xterm](https://xtermjs.org/docs/guides/flowcontrol/): `write` буферизует до 50 МБ и дальше теряет данные, пропускная способность 5-35 МБ/с; пауза на каждый кусок убивает скорость. Делаем «пакетный» вариант: JS считает записанное и раз в ~100 КБ шлёт `invoke('pty_ack', {id, n})` из колбэка `write`; ядро ведёт счётчик «в полёте» и перестаёт читать pty выше верхней отметки (стартово 256 КБ; правило руководства - не больше 500 К), ОС сама блокирует процесс. Проверка: `yes`, затем Ctrl-C - реакция в пределах доли секунды.
4. Ввод: `term.onData(s => invoke('pty_write', {id, data: s}))`, мышь и бинарный ввод - `onBinary`. Размер: `fit()` по `ResizeObserver` с задержкой ~50 мс, затем `pty_resize`; не вызывать `fit` у скрытой панели.
5. Окно только прикрепляется: ядро держит кольцевой буфер последних ~1 МБ сырого вывода, новое окно получает его перед живым потоком. Для точного восстановления экрана (включая альтернативный экран vim/TUI) проверить `addon-serialize` как замену буферу. Конец процесса - отдельное сообщение (JSON в тот же канал, различать по `instanceof ArrayBuffer`; проверить, что канал так умеет, иначе второй канал).
6. С `withGlobalTauri: true` в ванильном JS: `const {invoke, Channel} = window.__TAURI__.core; const ch = new Channel(); ch.onmessage = b => term.write(new Uint8Array(b), onDone);`. Права для команд - в capabilities.

## 4. Риски и приёмка прототипа (порядок работ)
1. Прототип: одна панель xterm + WebGL + `portable-pty` + канал `Raw`. Приёмка: `yes | head -c 200M` без зависания окна; Ctrl-C отвечает быстро; `vim`, `htop`, Claude Code рисуются; эмодзи через границу куска целы; CSP не ругается в консоли.
2. Скорость - главный риск: автор cmux ушёл с Tauri/Electron из-за неё ([HN](https://news.ycombinator.com/item?id=47079718)). Мерить до того, как строить интерфейс вокруг.
3. Затем №9 (сессия в ядре) и №4 (сплиты), потом №10 (окна), №3 (кольцо), остальное.
4. Linux: WebKitGTK и WebGL не проверялись - держать DOM-рендерер запасным.
5. Безопасность (аудит нового проекта обязателен): команды `pty_*` принимают только id своих сессий, `cwd`/`argv` от интерфейса не доверять без списка; вывод pty - недоверенный текст, OSC-ссылки открывать только после явного клика и проверки схемы; буфер, который отдаётся новому окну, не должен без нужды содержать секреты агентов.

## 5. Что не проверено
- Живых отзывов с Reddit/X не читал (DuckDuckGo отдал блоги и обзоры); «хвалят» - из блогов, HN и README.
- Лицензии WezTerm, Zed, cmux - API вернул NOASSERTION; для идей не важно, для копирования кода надо открывать `LICENSE`.
- Conductor закрыт: данные из его доков и стороннего обзора, исходники не смотрел.
- Код приложения не менялся; рабочая проба PTY - в одноразовом проекте вне репозитория. Пример Tauri-части (раздел 3.3) собран по docs.rs, не скомпилирован.
- Объёмы S/M/L - оценка по составу работ.
