<!-- autopilot:start -->
# vibe-env (Nightshift)

Среда для вайб-кодинга (macOS/Linux): человек ставит задачу, агенты доводят сами; человека зовут только за вопросом, после «не вышло» и за разрешением на push/PR. Новая функция проверяется этим принципом.

## Команды

```sh
cargo build
cargo test --workspace                                                 # ~2 мин, 2 живых теста ignored
cargo test -p nightshift-core --test engine_e2e                        # один тест-файл: имена в core/tests/
cargo test -p nightshift-core --test market -- --ignored --nocapture   # живой, нужна сеть
cargo test -p nightshift-core --test updates -- --ignored --nocapture  # живой, нужна сеть
node scripts/ui-selftest.js                                            # без окна; с NS_PLAYWRIGHT и NS_CHROME ещё проверки нарисованного окна
cd ui && python3 -m http.server 8765 --bind 127.0.0.1                  # интерфейс в браузере с dev-mock, без Tauri
cd app && cargo tauri dev
cd app && cargo tauri build --bundles app,dmg                          # Nightshift.app и .dmg
```

## Где что

- Архитектура, поток данных, соглашения - `docs/architecture.md`; решения с причинами - `docs/adr/`; границы доверия - `docs/security.md`.
- Задачи, рой, чат - `core/src/engine/`, `core/src/swarm/`, `core/src/chat/`; типы - `core/src/model.rs`; диск - `core/src/store.rs`.
- Адаптеры CLI-агентов и разбор их вывода - `core/src/agent/`; образцы вывода - `core/tests/fixtures/`.
- Tauri-команды и события - `app/src/main.rs` (регистрация) и соседние `app/src/*.rs`; права окна - `app/capabilities/`, `app/tauri.conf.json`.
- Интерфейс - `ui/index.html`, `ui/js/`, токены `ui/css/tokens.css`, словари `ui/i18n.js`, мок без Tauri - `ui/dev-mock.js`.
- Тесты ядра - `core/tests/`, общий каркас `core/tests/common/mod.rs`; тесты UI - `ui/selftest.js` (список скриптов в `ui/selftest.html`).
- Установка и упаковка - `scripts/install.sh`, `app/installer/aur/PKGBUILD`.

## Подводные камни

- Живые тесты `market` и `updates` ходят в сеть и помечены `#[ignore]`: `cargo test --workspace` их пропускает.
- `tauri-plugin-single-instance` (`app/src/main.rs`): запущенный старый экземпляр не даёт подняться новому, закрой его перед проверкой сборки.
- Системные диалоги рисует `tauri-plugin-dialog` (`app/src/main.rs`), не страница: в `ui/dev-mock.js` и в скриншоте окна их нет.
- Терминалы: при скрытии окна сессии надо отключать (`visibilitychange` в `ui/js/v4/term.js`), иначе WebKit глушит таймеры, xterm не подтверждает запись, flow control ставит процесс на паузу.
- Нечитаемый `state.json` не затирается пустым состоянием, рядом остаётся копия; тесты открывают `Store` только во временных каталогах.
- Строки для человека - только через `ui/i18n*.js`; `ui/vendor/` (xterm) руками не править.

## Окружение

`NIGHTSHIFT_GH` (путь к `gh`, по умолчанию `gh` из PATH), `NS_PLAYWRIGHT`, `NS_CHROME` (проверки окна в `scripts/ui-selftest.js`), `SHELL`, `TERMINAL`, `PATH`, `HOME`. Данные - `app_data_dir()` Tauri; конфиг - `~/.config/nightshift/config`.

## Как здесь работает Autopilot

Сборка ведётся навыком `/autopilot`. Требования, спецификация и таски — в `.autopilot/`.
Прогресс — `.autopilot/dashboard.html`. Правило: требование из `manifest.md`
может снять только пользователь.

Если работа продолжается — скажи «продолжи автопилот»: состояние поднимется
из `.autopilot/state.js`, переспрашивать ничего не нужно.
<!-- autopilot:end -->
