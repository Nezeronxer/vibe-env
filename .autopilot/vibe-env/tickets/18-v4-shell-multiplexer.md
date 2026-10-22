# 18 - Каркас v4 и терминал-мультиплексор в ядре

**Требования:** G35, G38, G58, G48
**Зависит от:** —
**Зона:** `core/src/term/` · `core/src/net/` · `core/src/market/` · `core/src/updates/` · `core/src/config/` · `core/src/lib.rs` · `core/Cargo.toml` · `Cargo.lock` · `app/src/main.rs` · `app/src/ptycmds.rs` · `app/src/windows.rs` · `app/src/market.rs` · `app/src/updates.rs` · `app/src/config.rs` · `app/src/gitcmds.rs` · `app/src/tray.rs` · `app/Cargo.toml` · `app/tauri.conf.json` · `app/capabilities/` · `ui/index.html` · `ui/vendor/xterm/` · `ui/js/v4/` · `ui/css/v4-shell.css` · `ui/css/v4-term.css` · `ui/css/v4-market.css` · `ui/css/v4-clawd.css` · `ui/css/v4-updates.css` · `core/tests/term.rs`
**Волна:** 12
**Ревью:** да - фундамент (PTY, процессы пользователя, окна)
**Модель:** сильная

## Что должно заработать

Спека: «v4» - истории 248, 250, 252-254, 260, 301, 303 и «Решения по реализации v4»; границы - `interfaces.md` «Границы v4».

**Сначала замер (обязательный стоп).** Минимальная PTY-вкладка в release-сборке: `yes | head -n 200000` и `cat` большого файла - вывод без потерь, ввод отвечает ≤ 50 мс, память окна не растёт неограниченно. Цифры - в отчёт. Не проходит - `BLOCKED: скорость PTY в Tauri` с замерами, дальше не строить.

**`core::term`** целиком по границе: `portable-pty` 0.9, сессии shell (`$SHELL -l`) и argv CLI, кольцевой буфер 2 МБ, attach/detach с досылкой буфера, flow control (`ack`, пауза чтения > 1 МБ без подтверждения), `list`, `snapshot` для восстановления (текст + папка, без процессов), выход процесса с кодом. Секреты приложения (связка ключей, переменные наборов) в окружение PTY не передаются. Тесты ядра на настоящем PTY: echo, resize, буфер при attach, выход с кодом, ack-пауза.

**`app`:** команды `term_*` с выводом через `tauri::ipc::Channel<InvokeResponseBody>` `Raw` (склейка ~4 мс); `window_detach/window_return` - новое окно `index.html?window=<id>` с переданным описанием вкладки, процессы остаются; закрытие окна не убивает сессии, трей показывает число живых сессий; ⌘Q с живыми процессами - один вопрос (история 303); `snapshot` сохраняется при выходе, восстанавливается при старте. CSP: если xterm в WKWebView требует - `style-src 'self' 'unsafe-inline'`, строка «почему» в отчёт.

**Каркас для тасков 19-25 (чтобы волна 12 шла параллельно без общих файлов):** модули-заготовки `core::{net, market, updates, config}` и расширения `core::git` - только типы и сигнатуры из «Границ v4» с `unimplemented`-заглушками, возвращающими ошибку `not_built`; в `app` - файлы `market.rs`, `updates.rs`, `config.rs`, `gitcmds.rs` со всеми командами из «Границ v4» (заглушки) и их регистрация в `main.rs`; в `ui/index.html` - подключение `ui/vendor/xterm/` (xterm 6.0.0 + fit 0.11.0, webgl 0.19.0, search 0.16.0, unicode11 0.9.0, web-links 0.12.0 файлами `lib/*.js` и `css`, `LICENSE`) и пустых файлов v4 по таскам: таск 22 - `ui/js/v4/{theme,panes,palette,settings-v4,i18n-shell}.js`, `ui/css/v4-shell.css`; таск 23 - `ui/js/v4/{term,term-settings,i18n-term,mock-term}.js`, `ui/css/v4-term.css`; таск 24 - `ui/js/v4/{market,history,publish,i18n-market,mock-market}.js`, `ui/css/v4-market.css`; таск 25 - `ui/js/v4/{clawd,updates,practices,i18n-clawd,mock-clawd}.js`, `ui/css/v4-clawd.css`, `ui/css/v4-updates.css`. Файлы `mock-*.js` подключаются только без `window.__TAURI__` (как `dev-mock.js`). Порядок подключения: shell раньше остальных.

## Из брифа, дословно

> «код должен работать в терминале» · «терминал будет считаться эмулятором» (G35)
> «красивый плюс окна» (G38)
> «То есть это должен быть по сути мультиплекс.» (G58)
> «сделай приложение сильным, а то оно сейчас какое-то громоздкое» (G48)

## Критерии приёмки

1. Замер в отчёте: 200 000 строк без потерь, отклик ввода ≤ 50 мс в release.
2. `cargo test -p nightshift-core term` - зелёные: echo через PTY, resize меняет `stty size`, attach после вывода отдаёт буфер, выход процесса даёт код, без ack чтение встаёт.
3. Из окна (временная кнопка в dev-панели или `?term=1`) открывается вкладка-терминал с shell, работает `vim`/`htop`, изменение размера.
4. «Вынести в окно» и обратно - процесс не прерывается; закрыть окно - процесс жив, трей «Сессии: 1»; открыть - вывод на месте.
5. В окружении PTY нет переменных наборов и ключей приложения (тест).
6. Все заглушки v4 компилируются, `cargo test --workspace` и `node scripts/ui-selftest.js` зелёные, clippy чист.
