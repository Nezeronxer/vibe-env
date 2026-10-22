# Автообновление CLI и моделей: исследование для Nightshift (23.10.2026)

Запрос: «когда выходила новая модель, автоматически обновлялся, и все модели автоматически обновлялись, включая Gemini, включая ChatGPT». Код не менялся. Метки: [лок] проверено на этой машине (macOS, arm64), [док] первоисточник по ссылке, [не сверено] вывод без проверки.

Сводка: две разные задачи. (А) версия CLI: у пяти из шести есть свой автоапдейт и команда обновления, Nightshift её только запускает. (Б) список моделей: у Codex, opencode, Cursor он берётся из самой CLI, у Claude, Gemini, Qwen команды нет, и «вышла новая» узнаётся из публичного `models.dev` (проверено живым запросом, 304 по ETag).

## 1. Что уже есть в коде

- `core/src/models/mod.rs`: три слоя каталога: `Adapter::models()` (команда CLI) -> последний снимок в `Store` -> `Adapter::builtin_models()` (список-подсказка). `refresh()` пишет снимок только при успехе команды. `resolve()` даёт `cli_arg`: у Claude «последняя» = алиас семейства (`opus`), у остальных первая версия семейства. Закреплённая версия, которой нет в списке, переезжает на ближайшую (`ModelNote::Replaced`), `Newer` = тихая подпись «есть новее» (G23.1). `next_on_reject`, `stronger_than` - откат при отказе CLI.
- Команда списка в адаптерах: Codex `codex debug models` (разбор не сверен), opencode `opencode models` (разбор по живому выводу), Cursor `--list-models` (не сверен). У Claude, Gemini, Qwen `builtin_models()` зашит в код: `claude.rs` (опирается на docs от 21.10), `gemini.rs`, `qwen.rs`. Список обновляется только выходом приложения.
- Версия CLI: `common.rs: cli_version` (`<bin> --version`, 3 с), `claude.rs: version_at` кеширует на 10 минут. Поиска «последней версии» и команд обновления нет, `install_hint()` - строки «не сверено». `find_bin` ищет в `~/.local/bin`, `~/.npm-global/bin`, `~/.cargo/bin`, `/opt/homebrew/bin`, `/usr/local/bin`.
- Дыра, найденная по ходу: `gemini.rs` сознательно не включает Gemini 4 Argon («пока не обычная модель»), а `models.dev` и OpenRouter его уже перечисляют с 13.10.2026 (см. п.3). Сейчас такой выход можно увидеть только после выпуска новой сборки Nightshift.

## 2. По каждой CLI

Локальное состояние (`which`, `--version`, `claude doctor`; сравнение с `https://registry.npmjs.org/<пакет>/latest`, 23.10):

| CLI | Установлено [лок] | Последняя (npm latest) | Как установлена |
|---|---|---|---|
| Claude Code | 2.1.309 (в начале прогона было 2.1.308: обновился сам, `claude doctor`: native, Auto-updates enabled, channel latest, «Last update attempt: success») | 2.1.309 (dist-tag `stable` = 2.1.308) | native: `~/.local/bin/claude` - симлинк в `~/.local/share/claude/versions/<версия>`; старые версии (2.1.304...308) остаются на диске |
| Codex | не установлен | 0.163.0 (GitHub tag `rust-v0.163.0`, 15.10) | - |
| Gemini CLI | не установлен | 0.65.0 (preview 0.66.0-preview.0, nightly 0.67.0) | - |
| opencode | 1.18.34 | 1.19.4 | curl-установщик: `~/.opencode/bin/opencode`. Алиас `opencode` в zsh - функция-обёртка (пишет tui.json), `command opencode` обходит её |
| Qwen Code | 0.22.0 | 0.26.0 | standalone: `~/.local/bin/qwen` -> `~/.local/lib/qwen-code`, `manifest.json` с версией. Отстаёт на 4 минора при `enableAutoUpdate` по умолчанию true (в `~/.qwen/settings.json` ключа нет): встроенный автоапдейт здесь не сработал, причину не выяснял |
| Cursor CLI | не установлен | npm-пакета нет | - |

### Claude Code (док: https://code.claude.com/docs/en/setup, https://code.claude.com/docs/en/model-config)
- Установка: нативный `curl -fsSL https://claude.ai/install.sh | bash` (macOS/Linux); Homebrew-cask `claude-code` (stable) и `claude-code@latest`; WinGet; apt/dnf/apk (нужен sudo); `npm install -g @anthropic-ai/claude-code` (Node 22+, внутри тот же нативный бинарь).
- Обновление: нативная установка обновляется сама в фоне при старте и периодически, вступает в силу при следующем запуске. `claude update` (alias `upgrade`) - вручную [лок: `claude update --help`]. brew/WinGet/apt сами не обновляются; для brew/WinGet есть `CLAUDE_CODE_PACKAGE_MANAGER_AUTO_UPDATE=1`. npm: `npm install -g @anthropic-ai/claude-code@latest` (не `npm update -g`).
- Настройка: `autoUpdatesChannel` = `latest` | `stable` (stable - версия примерно недельной давности без крупных регрессий); `minimumVersion` - нижняя планка; `DISABLE_AUTOUPDATER=1` в `env` отключает фон, `DISABLE_UPDATES` блокирует и ручное. Док. прямо предупреждает: новой модели может потребоваться версия новее, чем отдаёт `stable`.
- Установленная/последняя без ключей: `claude --version`; `https://registry.npmjs.org/-/package/@anthropic-ai/claude-code/dist-tags` -> `{latest, stable, next}` [лок: latest 2.1.309, stable 2.1.308]; GitHub `anthropics/claude-code/releases/latest` (v2.1.309, в теле - changelog). Откат: установщик принимает конкретный номер (`bash -s <версия>`, точная строка в док. при выгрузке обрезана - сверить), у npm `@<версия>`.
- Список моделей: команды нет. Алиасы `opus`, `sonnet`, `haiku`, `fable`, `best`, `opusplan` разрешаются самой CLI в «новейшую» (на API Anthropic: Opus 5.5, Sonnet 5.5, Haiku 5.5, Fable 5.1; на Bedrock/Foundry - более старые). Полное имя (`claude-opus-5-5`) закрепляет версию. Значит «новая модель у Claude» = обновление самой CLI, алиас переедет сам.
- macOS и Linux: одинаково (нативный установщик); на Linux apt/dnf - только вручную с sudo.

### Codex (док: https://developers.openai.com/codex/cli, /codex/cli/reference, /codex/models, /codex/config-reference; README https://github.com/openai/codex)
- Установка: `curl -fsSL https://chatgpt.com/codex/install.sh | sh` (macOS/Linux, качает с releases.openai.com, запасной путь - GitHub Releases); `npm install -g @openai/codex`; `brew install --cask codex` (cask 0.163.0 [лок: brew info]).
- Обновление: `codex update` (Stable: «Check for and apply a Codex CLI update when the installed release supports self-update»); повторный запуск установщика; `npm i -g @openai/codex@latest`; `brew upgrade --cask codex`. Конфиг: `check_for_update_on_startup` (проверка при старте), `features.in_app_updates` (в `requirements.toml` можно выключить).
- Версии без ключей: `codex --version`; npm `latest` 0.163.0 (в dist-tags много `alpha`/`release-*`: брать только `latest`); GitHub тег `rust-v<версия>`.
- Список моделей: `codex debug models` печатает сырой каталог JSON и по умолчанию обновляет его с удалённого endpoint; `--bundled` - только зашитый в бинарь, без сети (Experimental). Свой каталог: `model_catalog_json` (путь), `model_providers.<id>.model_catalog_url`. Формат вывода не сверен (Codex не установлен).
- Важно для «включая ChatGPT»: доступность зависит от плана и входа. По docs/codex/models: GPT-5.5 снят из Codex для входа через ChatGPT с 14.10.2026 (API-ключ не затронут); на Plus/Pro/Business/Enterprise/Edu выбирать `gpt-6.1-sol` «when available», на Free/Go - `gpt-6-luna`. `models.dev` об этом не знает (gpt-5.5 там не помечен снятым), поэтому для Codex истина - `codex debug models`.
- macOS и Linux: все три способа есть; brew на Linux - только Linuxbrew.

### Gemini CLI (док: https://geminicli.com/docs/get-started/installation/, /docs/reference/configuration/, /docs/cli/model/)
- Установка: `npm install -g @google/gemini-cli`, `npx @google/gemini-cli`, Homebrew (formula `gemini-cli` 0.65.0 [лок]), MacPorts, Anaconda. Каналы: stable (тег `latest`, выходит еженедельно из прошлого preview), preview, nightly.
- Обновление: отдельной команды `update` в docs нет (не нашёл). `general.enableAutoUpdate` (по умолчанию true) и `general.enableAutoUpdateNotification` (true) в settings.json; вручную `npm install -g @google/gemini-cli@latest`.
- Версии: `gemini --version`; npm dist-tags `{latest:0.65.0, preview:0.66.0-preview.0, nightly:0.67.0-nightly...}` (тег `false` - мусор); GitHub `google-gemini/gemini-cli` (v0.65.0 20.10).
- Список моделей: команды нет. `/model` показывает «Auto (Gemini 3)» = `gemini-3-pro-preview` + `gemini-3-flash-preview`, «Auto (Gemini 2.5)», «Manual»; флаг `--model`/`-m`; в settings `model.name`, `modelConfigs.aliases`. Страница моделей отстаёт: Gemini 4 там нет, а в v0.66.0-preview.0 появился баннер «Gemini CLI not optimized for Gemini 4» (PR #29774). Вывод: появление id в API не значит, что CLI готова.
- macOS и Linux одинаково (Node).

### opencode (док: https://opencode.ai/docs/, /docs/cli/, /docs/config/)
- Установка: `curl -fsSL https://opencode.ai/install | bash`; `npm install -g opencode-ai`; `brew install anomalyco/tap/opencode` (tap свежее) или formula `opencode` из core; paru/scoop/choco/mise.
- Обновление: `opencode upgrade [target]` (`-m/--method`: curl, npm, pnpm, bun, brew) [лок: `opencode upgrade --help`]; по умолчанию сам скачивает обновления при старте; `"autoupdate": false | true | "notify"` в opencode.json («notify» не работает при установке через пакетный менеджер); `OPENCODE_DISABLE_AUTOUPDATE`.
- Версии: `opencode --version`; npm `latest` 1.19.4; GitHub `anomalyco/opencode/releases/latest` (v1.19.4; старый `sst/opencode` отдаёт 301).
- Риск: есть OpenCode 2. Docs: «OpenCode 1 and OpenCode 2 both use the `opencode` command and cannot be installed side by side». Formula из Homebrew core сейчас 2.1.2, npm `opencode-ai` - 1.x. Автообновление через менеджер пакетов может перескочить мажор (флаги и вывод `run` не сверял).
- Список моделей: `opencode models [provider]`, формат `provider/model`; `--refresh` обновляет кеш из models.dev. Локальный кеш `~/.cache/opencode/models.json` (6.0 МБ) - это тот же `models.dev/api.json`. Без ключей провайдеров в списке только 10 бесплатных `opencode/*` [лок]. Новая модель появляется без обновления самой CLI.

### Qwen Code (док: https://github.com/QwenLM/qwen-code, https://qwenlm.github.io/qwen-code-docs/en/users/configuration/settings/)
- Установка: standalone `curl -fsSL https://qwen-code-assets.oss-cn-hangzhou.aliyuncs.com/installation/install-qwen-standalone.sh | bash`; npm `@qwen-code/qwen-code`; Homebrew (formula `qwen-code` 0.25.3 [лок]).
- Обновление: `qwen update` [лок: есть в `--help`], `/update` внутри; `general.enableAutoUpdate` (true по умолчанию); `QWEN_UPDATE_BASE_URL` - своё зеркало релизов, проверяются SHA256SUMS и подпись.
- Версии: `qwen --version`; npm `latest` 0.26.0 (есть `preview`, `nightly`). GitHub `releases/latest` вернул `sdk-typescript-v0.1.22` - тег не той сущности, для Qwen брать npm.
- Список моделей: команды нет; `model.name`, `modelProviders` в settings, выбор через `/auth` и `/model`. Список зависит от входа (Coding Plan, OAuth, свой ключ).
- Платформы: standalone/npm - macOS и Linux.

### Cursor CLI (док: https://cursor.com/docs/cli/installation, /docs/cli/reference/parameters)
- Установка: `curl https://cursor.com/install -fsS | bash`; в docs бинарь называется `agent` (`agent --version`), а `cursor.rs` по умолчанию ищет `cursor-agent`: искать оба имени, проверить после установки.
- Обновление: «will try to auto-update by default»; вручную `agent update`.
- Версии: npm-пакета нет, публичного JSON не нашёл; только `agent --version` и `agent update`.
- Список моделей: `agent models` (команда) и `--list-models` (флаг): «available models for this account», нужен вход. В `models.dev` провайдера Cursor нет.

## 3. Источник «вышла новая модель» без ключей пользователя

Эндпоинты вендоров `GET /v1/models` без ключа: Anthropic 401, OpenAI 401, Google 403 [лок: curl]. Для подписочных пользователей они не годятся.

models.dev (`https://models.dev/api.json`, проверено живым запросом 23.10.2026):
- Один JSON 6.06 МБ, 232 провайдера, 9147 моделей, CORS `*`, Cloudflare. `ETag` + `If-None-Match` -> `304` [лок: проверил]. Репозиторий `anomalyco/models.dev`; коммиты бота «chore(sync)» идут каждые несколько минут (22.10 - пять за час), то есть сайт живой.
- Структура: `{<provider_id>: {id, name, env[], npm, doc, models: {<model_id>: {...}}}}`. Поля модели: `id`, `name`, `family`, `release_date`, `last_updated`, `status` (нет | `deprecated` | `beta` | `alpha`), `reasoning`, `reasoning_options`, `tool_call`, `modalities{input,output}`, `limit{context,output}`, `cost`, `open_weights`, `knowledge`, `canonical_model_id`, `description`.
- Провайдеры: `anthropic` (17 моделей), `openai` (51), `google` (38), `alibaba` (59, DashScope-идентификаторы Qwen), `opencode` (130), `openrouter` (390), `github-copilot` (36). `cursor` нет.
- Свежесть [лок]: `anthropic`: claude-haiku-5-5 (07.10), sonnet-5-5 (28.09), opus-5-5 (22.09), fable-5-1 (01.09). `openai`: gpt-6.1-sol (29.09), gpt-6-luna, gpt-6-sol (22.09), gpt-6-astra (04.09). `google`: gemini-4-argon (13.10), gemini-3.8-flash (02.09). `alibaba`: qwen3.8-max (03.08), qwen3.8-flash (26.08). Совпадает с docs вендоров (Claude: model-config; Codex: /codex/models). У Anthropic в id нет точки (`claude-opus-5-5`), у GitHub Copilot с точкой - сопоставлять только в рамках своего провайдера.
- Минусы: `status` отстаёт (gpt-5.5 не помечен снятым при том, что из Codex-с-ChatGPT он снят 14.10); id API-вендора не равен списку CLI с входом по подписке (нет `*-codex`, ChatGPT-планов, Coding Plan у Qwen); в массе «мусор» (image, tts, embedding, live, computer-use, dated-копии).
- Запасной источник: OpenRouter `https://openrouter.ai/api/v1/models` (без ключа, 200, 468 моделей, поле `created` - unix-время; gemini-4-argon 13.10, claude-haiku-5.5 07.10). Подходит как сверка, id отличаются (`anthropic/claude-haiku-5.5`).
- Страницы вендоров (docs Claude, developers.openai.com/codex/models, geminicli.com): точнее для CLI, но это HTML, который отстаёт (Gemini) или меняется. Парсить не стоит; использовать только как человеку ссылку в уведомлении.

Что надёжнее: список из самой CLI (Codex, opencode, Cursor) - истина «что работает у этого человека». `models.dev` - радар для Claude, Gemini, Qwen и ранний сигнал для остальных. Частота: старт приложения + раз в 6 часов с `If-None-Match` (304 бесплатный); модели выходят днями, чаще незачем.

Сопоставление id вендора -> модель в CLI:
| CLI | Правило | Пример |
|---|---|---|
| Claude | `family` models.dev (`claude-opus`) -> алиас (`opus`); номер из id `claude-opus-5-5` -> «5.5». Алиас разрешает сама CLI | `--model opus` или `--model claude-opus-5-5` |
| Codex | id совпадает со строкой `-m`; проверять наличие в `codex debug models` | `codex -m gpt-6.1-sol` (docs) |
| Gemini | id совпадает со строкой `-m` [не сверено на CLI]; в models.dev есть и API-алиасы `gemini-pro-latest`, `gemini-flash-latest` (по датам записей `flash-latest` 13.08 отстаёт от 3.8-flash 02.09, на них не полагаться) | `gemini -m gemini-3.8-flash` |
| opencode | тождественно: `<provider>/<model id>` | `opencode/…`, `anthropic/claude-opus-5-5` |
| Qwen | id DashScope (`qwen3.8-max`) [не сверено: попадёт ли в список Coding Plan / OAuth] | `qwen -m qwen3.8-max` |
| Cursor | только `agent models` | - |

Фильтр «настоящая новая модель» для models.dev: `output == ["text"]`, `tool_call == true`, `status` не `deprecated`, без id с `image|tts|embedding|live|realtime|audio|computer-use|customtools|preview-tts` и без дат-копий (`-20251101`); «новая» = id нет в прошлом снимке (а не по `release_date`: дата может быть задним числом); версия берётся из id тем же разбором, что `version_cmp`.

## 4. Предложение: механизм «свежесть» (G23.4 и далее)

Принцип: Nightshift ничего не ставит молча. Версия CLI и список моделей - два независимых слоя; обновление CLI всегда явное и безопасное, список моделей обновляется сам.

### 4.1 Слой моделей (автоматически, без вопросов)
1. Источники по приоритету: список из CLI (`models()`; обновлять при открытии среды и раз в 24 ч, после обновления CLI - сразу) -> `models.dev` (раз в 6 ч, `If-None-Match`, кеш на диске) для Claude, Gemini, Qwen и как «ранний сигнал» для остальных -> снимок в `Store` -> `builtin_models()` как последний запас. `builtin_models()` остаётся, но его роль - первый запуск без сети.
2. «Последняя» переходит сама (G23): Claude - алиас (CLI решает), Codex и opencode - первая версия в списке CLI, Gemini и Qwen - первая в списке из `models.dev` после фильтра. Правило для Gemini и Qwen: внутри одной генерации (3.7 -> 3.8) переход сам; новая генерация (3 -> 4) попадает в каталог с пометкой «новое, не проверено CLI» и становится «последней» только по кнопке или после успешного пробного запуска. Причина: Gemini 4 уже в API, а CLI пишет «not optimized».
3. Закреплённая версия не двигается (G23.1/23.2 как есть).
4. Уведомление: один раз на id (список `seen_models` в `Store`): «Вышла Haiku 5.5. [Переключить роли на неё] [Позже]»; в «Модели» среды бейдж «новое» до первого выбора. Для Claude, если установленная CLI старше версии, которая знает модель, - сразу вторая кнопка «Обновить Claude Code» (п.4.2).
5. Если модель из `models.dev` CLI отвергла (`next_on_reject` уже есть) - в каталоге остаётся «недоступна на этом входе», без ошибок задаче.

### 4.2 Слой версий CLI
Проверка (фон, при старте и раз в 6 ч): установленная - `--version` (кеш 10 мин, как `claude.rs`); последняя - npm registry (`/<пакет>/latest`; для Claude ещё `dist-tags`: `latest`/`stable`), для Cursor и standalone-Qwen - нет надёжного публичного источника, значит только по `agent update`/`qwen update`. GitHub API (60 запросов/ч без токена) не использовать как основной, у Qwen он вернул не тот релиз.

Что показываем: в карточке среды строка «Claude Code 2.1.308 -> 2.1.309 [Обновить]»; для CLI с собственным автоапдейтом (Claude native, Qwen, opencode curl, Cursor, Gemini при `enableAutoUpdate`) - только информация («обновится само»), кнопка по желанию. Не трогаем настройки CLI пользователя (`autoUpdatesChannel`, `DISABLE_AUTOUPDATER` и т.п.): они его.

Как обновляем (по кнопке, а не молча; тумблер «обновлять сами в простое» - выключен по умолчанию):
| CLI | Команда (способ определяем по `realpath` бинаря из `cli_path()`) |
|---|---|
| Claude | native: `claude update`; brew: `brew upgrade --cask claude-code[@latest]`; npm: `npm i -g @anthropic-ai/claude-code@latest`; apt/dnf - только показать команду |
| Codex | `codex update`; иначе `npm i -g @openai/codex@latest` или `brew upgrade --cask codex` |
| Gemini | `npm i -g @google/gemini-cli@latest` (или `brew upgrade gemini-cli`) |
| opencode | `opencode upgrade -m <метод>`; не переходить через мажор (1 -> 2) автоматически |
| Qwen | `qwen update`; иначе npm / brew |
| Cursor | `agent update` |

Защита работающей задачи:
- «Ворота простоя» на среду: обновление стартует, только если ни один агент этой среды не запущен; на время обновления новые запуски этой среды ждут в очереди (секунды), таймаут 120 с. Если идёт задача - кнопка «Обновить после текущих задач».
- Почему: нативные установки кладут новую версию рядом и переключают симлинк, запущенные процессы держат старый файл (локально в `versions/` лежат 2.1.304...309); а `npm i -g` заменяет файлы на месте, док. Claude прямо говорит, что это удаляет бинарь, который ещё используют сессии [док: setup, раздел про общий диск]. Для остальных CLI поведение при замене запущенного бинаря не проверял [не сверено].
- Версия, записанная в запуске (`Run.cli_version`), остаётся для истории; кеш версии сбрасывается после обновления.

Откат:
- Перед обновлением пишем в `Store` `{env, method, from, to, at}`. После обновления: `--version` и `--help` обязаны отработать за 5 с и версия совпасть с ожидаемой; иначе для npm/brew-путей автоматически `npm i -g <пакет>@<from>` / `brew` (для brew-cask откат ненадёжен - тогда только предупреждение).
- Claude native: ставит конкретный номер установщиком (`curl -fsSL https://claude.ai/install.sh | bash -s <версия>`, сверить строку) или канал `stable`; старые версии лежат в `versions/`. Кнопка «Вернуть <from>» остаётся в карточке на 7 дней.

### 4.3 Расписание
- Старт приложения и раз в 6 ч: `models.dev` (304), npm-версии. Раз в 24 ч и при открытии среды: команды списка CLI. После любого обновления CLI: список этой среды сразу.
- Без сети или при ошибке: молча остаёмся на снимке, в «Модели» среды дата «список от …» (G23 уже требует).
- Опрос не расходует токены и ключи: все запросы публичные, GET без cookie.

### 4.4 Риски и ограничения
- Глобальный npm: на macOS с Homebrew-нодой `npm prefix` = `/opt/homebrew` и принадлежит пользователю [лок: npm prefix], на Linux обычно `/usr` или `/usr/local` -> EACCES. Никогда `sudo npm` (док. Claude: «Do NOT use sudo npm install -g»). Если `npm prefix -g` не доступен на запись - не предлагать кнопку, показать команду с пользовательским префиксом (`~/.npm-global`, `find_bin` его уже ищет) или перейти на standalone-установщик.
- Brew: общий лок, `brew upgrade` без имени пакета обновит всё - всегда с именем пакета/cask и `HOMEBREW_NO_AUTO_UPDATE=1`; у Claude brew-cask сам не обновляется; на Linux brew почти не встречается.
- Две установки одной CLI (npm + native): обновлять ту, что вернул `cli_path()`, и показать предупреждение о второй.
- Мажорные скачки (opencode 1 -> 2, будущий Codex/Gemini 1.0): только уведомление, не автообновление.
- Дрейф `stable`/`latest`: Claude `stable` на сутки-неделю старше `latest`; новая модель может требовать `latest` - показывать причину в кнопке.
- Модель в `models.dev` не значит «доступна этому человеку» (ChatGPT-план, Coding Plan, регион): финальная проверка - отказ CLI, он уже обрабатывается.
- Windows - вне объёма (команды в таблице те же, способы установки другие).
- Автоапдейты самих CLI идут независимо от Nightshift (Claude обновился у меня за время этого прогона): версия может смениться между задачами; кеш версии 10 минут и запись версии в запуск это покрывают.

### 4.5 Что делать в коде (оценка, не начато)
1. Модуль `core/src/updates/`: `latest_version(adapter)` (npm), `install_method(path)`, `update_cmd(adapter, method)`, ворота простоя в движке.
2. Модуль `core/src/models/radar.rs`: загрузка `models.dev` с ETag, фильтр, `new_since(snapshot)`; в `Adapter` - `modelsdev_provider() -> Option<&str>`.
3. `Store`: `seen_models`, `update_log`. UI: строка версии в карточке среды, бейдж «новое», тост по G23.4.
4. Проверки (до кода): поставить Codex, Gemini, Cursor на тестовой машине и сверить вывод `codex debug models`, `agent models`, как `gemini -m gemini-4-argon` ведёт себя на 0.65, и жив ли `codex update` на npm-установке; `gemini` - есть ли `update`.

## 5. Не проверено
- Формат вывода `codex debug models`, `agent models`/`--list-models` (CLI не установлены).
- Принимает ли Gemini CLI 0.65 id `gemini-4-argon`, `gemini-3.8-flash`; появится ли `qwen3.8-max` в Qwen Code.
- Что делает замена бинаря во время работы агента у Codex/Gemini/Qwen/opencode/Cursor.
- Применяется ли автоапдейт Claude/других CLI в неинтерактивном режиме `-p` (Nightshift запускает их так).
- Точная строка установки конкретной версии Claude в docs (обрезана при выгрузке текста).
- Причина, по которой Qwen на этой машине остался на 0.22.0.

Источники (открыты 23.10.2026): https://code.claude.com/docs/en/setup · https://code.claude.com/docs/en/model-config · https://developers.openai.com/codex/cli · https://developers.openai.com/codex/cli/reference · https://developers.openai.com/codex/models · https://developers.openai.com/codex/config-reference · https://github.com/openai/codex · https://geminicli.com/docs/get-started/installation/ · https://geminicli.com/docs/reference/configuration/ · https://geminicli.com/docs/cli/model/ · https://opencode.ai/docs/ · https://opencode.ai/docs/cli/ · https://opencode.ai/docs/config/ · https://github.com/QwenLM/qwen-code · https://qwenlm.github.io/qwen-code-docs/en/users/configuration/settings/ · https://cursor.com/docs/cli/installation · https://cursor.com/docs/cli/reference/parameters · https://models.dev/api.json · https://openrouter.ai/api/v1/models · https://registry.npmjs.org/ · https://api.github.com/repos/.
