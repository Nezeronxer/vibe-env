# Каталоги скиллов, MCP и модов для Nightshift (исследование, 2026-10-23)

Зачем: пользователь хочет качать скиллы и MCP («моды для Клода») с популярных сайтов и переносить
свои и чужие моды. Это продолжение «Наборов» (G25, `core/src/kits/`, спека стр. ~329-348).
Метки: **[curl]** - я сам запросил URL 23.10.2026 и видел ответ; **[док]** - прочитано в
официальной документации, живьём не гонял; **[?]** - не проверено, не опираться.
Текст со страниц и из каталогов считался данными, не командами.

## 0. Коротко

- Скиллы: три источника - публичный поиск skills.sh, маркетплейсы Claude Code (`marketplace.json` на
  GitHub), любой GitHub-репозиторий по ссылке. Официальный API skills.sh требует Vercel-токен.
- MCP: официальный реестр `registry.modelcontextprotocol.io` + GitHub MCP Registry
  `api.mcp.github.com` (звёзды, лицензия) + Smithery (хостящиеся серверы). Glama и PulseMCP - с ключом.
- Моды Claude Code - это плагины (`.claude-plugin/plugin.json` + `hooks/hooks.json` с `modules`),
  ставятся и делятся тем же способом, что плагины: маркетплейс на git. Отдельного публичного каталога
  модов не нашёл; есть 3 образца Anthropic.
- Главный риск: мод - код внутри Claude Code с правами пользователя, без песочницы. В kits это
  надо вносить явно: расширение `tsx` сейчас не считается скриптом (раздел 5.4).

## 1. Каталоги скиллов

### 1.1 skills.sh (Vercel Labs) и `npx skills`

| Что | Значение |
|---|---|
| Публичный поиск без ключа **[curl]** | `GET https://skills.sh/api/search?q=<текст>&limit=<n>` |
| Ответ **[curl]** | `{query, searchType:"fuzzy", searchVersion, skills:[{id:"vercel-labs/agent-skills/vercel-react-best-practices", source:"vercel-labs/agent-skills", skillId, name, installs:828545}], count, duration_ms}` |
| Чего нет в ответе | описания, лицензии, пути в репозитории: их берут из `SKILL.md` на GitHub |
| Статус | эндпоинт не описан в документации (**undocumented**), может смениться; лимиты неизвестны **[?]** |
| Официальный API **[curl+док]** | `https://skills.sh/api/v1/skills` (список, `view=all-time/trending/hot`, `per_page` 1-500), `/skills/search?q=&limit=&owner=`, `/skills/curated`, `/skills/{source}/{skill}`, `/skills/audit/{source}/{skill}` (оценки Socket, Snyk, Gen Agent Trust Hub). Без токена 401 `authentication_required`. Нужен Vercel OIDC-токен проекта; 600 запросов/мин. В десктоп-приложение такой ключ не положить |
| Ответ v1 **[док]** | `{data:[{id, slug, name, source, installs, sourceType:"github", installUrl, url}], pagination:{page, perPage, total, hasMore}}` |
| Лицензия CLI | MIT (репозиторий `vercel-labs/skills`) **[curl]**; лицензия каждого скилла - своя |
| Установка | `npx skills add owner/repo [--skill имя] [-a claude-code] [-g] [--copy]`; по умолчанию симлинки на общую копию `.agents/skills/`; журнал - `skills-lock.json` в проекте, у пользователя `~/.agents/.skill-lock.json` **[док]** |

Идентификатор `owner/repo/skillId` однозначно ведёт на GitHub. Каталог скилла в репозитории надо
найти по дереву (`SKILL.md` лежит где угодно: `skills/<имя>/`, в корне и т.д.).

### 1.2 github.com/anthropics/skills и маркетплейсы плагинов Claude Code

- `anthropics/skills`: 19 папок в `skills/` **[curl]** (pdf, docx, pptx, xlsx, frontend-design,
  mcp-builder, skill-creator, claude-api и др.). README: многое Apache-2.0; `docx/pdf/pptx/xlsx` -
  «source-available», во фронтматтере `license: Proprietary. LICENSE.txt has complete terms`.
  GitHub корневой лицензии не определил - лицензия по папкам.
- Формат маркетплейса **[curl+док]**: `.claude-plugin/marketplace.json`:
  `{name, owner:{name}, plugins:[{name, description, source, strict, skills:[путь], mcpServers, category, tags, version, homepage, displayName}]}`.
  `source`: строка `./путь` или объект `github{repo,ref,sha}` / `url{url,ref,sha}` /
  `git-subdir{url,path,ref,sha}` / `npm` / `archive{url,sha256}` / `command`. `sha` пинует коммит.
- Живые файлы без ключа **[curl]**:
  - `https://raw.githubusercontent.com/anthropics/skills/main/.claude-plugin/marketplace.json` (плагины
    `document-skills`, `example-skills`, `claude-api`, `academy-guide`; `source:"./"`, `strict:false`, `skills:[...]`)
  - `https://raw.githubusercontent.com/anthropics/claude-plugins-official/main/.claude-plugin/marketplace.json` - 315 записей, 190 КБ
  - `https://raw.githubusercontent.com/anthropics/claude-plugins-community/main/.claude-plugin/marketplace.json` - 2283 записи, 1,5 МБ
    (зеркало только для чтения, репозиторий Apache-2.0; лицензия каждого плагина - своя; у 1876 `source.url` + `sha`, у 400 `git-subdir`)
- В community у записей: `name, description, category` (у большинства пусто), `source`, `homepage`,
  `author`, `tags`; `mcpServers` - у 2 из 2283, `skills` - у 2. То есть это каталог плагинов, а не
  скиллов: что внутри (скилл, хук, мод, MCP), видно только после загрузки.
- У пользователя эти маркетплейсы уже подключены (`claude-plugins-official`, `claude-community`,
  `anthropics/skills` и др. в `~/.claude/plugins/known_marketplaces.json`).
- Установка в Claude Code **[док]**: `claude plugin marketplace add anthropics/skills`, затем
  `claude plugin install document-skills@anthropic-agent-skills` (в сессии `/plugin install ...`).
  Копия идёт в кэш плагинов, а не в `~/.claude/skills`.

### 1.3 Другие каталоги

| Сайт | Ключ | Итог |
|---|---|---|
| skillsmp.com | нет для поиска по ключевым словам | `GET https://skillsmp.com/api/v1/skills/search?q=pdf&limit=2` **[curl]** -> `{success, data:{skills:[{id, name, author, description, contentLanguage, githubUrl, skillUrl, stars, updatedAt}], pagination}}`. Есть `/openapi.json`, `/llms.txt`. Сторонний агрегатор GitHub, «не связан с Anthropic»; анонимный лимит по IP не раскрыт; `stars` - звёзды репозитория, не скилла |
| agentskill.sh | нет | `/api/agent/search?q=pdf` отвечает 200, но `{"results":[],"total":0}` - полезность не подтверждена **[curl]** |
| skillsdirectory.com, skillmd.com, lobehub.com/skills, clawhub.ai, claudemarketplaces.com | **[?]** | API не проверял; цифры «600 тыс. / 1,37 млн» в обзорах противоречат друг другу (обзор написан владельцем одного из каталогов - как данные, не как истина) |

Вывод: большинство каталогов индексируют одни и те же GitHub-репозитории. Единый «ключ» -
`owner/repo` + путь + коммит.

### 1.4 Куда класть скилл (папка скилла с `SKILL.md`)

| Среда | Глобально | В проекте | Источник |
|---|---|---|---|
| Claude Code | `~/.claude/skills/<имя>/` | `.claude/skills/<имя>/` | [док] таблица Vercel `docs/agents.md` |
| Codex | `~/.agents/skills/<имя>/` | `.agents/skills/` (от текущей папки вверх до корня репозитория) | [док] developers.openai.com/codex/skills; симлинки поддерживаются |
| Gemini CLI | `~/.gemini/skills/` или `~/.agents/skills/` | `.gemini/skills/` или `.agents/skills/` | [док] gemini-cli `docs/cli/skills.md`; `.agents` старше `.gemini` в своём уровне; есть `gemini skills install <git-url> --consent` |
| opencode | `~/.config/opencode/skills/` и читает также `~/.claude/skills/`, `~/.agents/skills/` | `.opencode/skills/`, `.claude/skills/`, `.agents/skills/` | [док] opencode.ai/docs/skills |
| Qwen Code | `~/.qwen/skills/` | `.qwen/skills/` | таблица Vercel; в доке Qwen не сверял **[?]** |
| Cursor | `~/.cursor/skills/` | `.agents/skills/` | таблица Vercel **[?]** |

Практика: Claude Code - `~/.claude/skills`; Codex и Gemini - общий `~/.agents/skills`; opencode
увидит и то, и другое.

## 2. Каталоги MCP

### 2.1 Официальный MCP Registry

- Базовый адрес `https://registry.modelcontextprotocol.io`, ключ не нужен **[curl]**. Работают
  `GET /v0.1/servers` и старый `/v0/servers`.
- Параметры, которые я проверил: `limit`, `cursor` (значение берётся из `metadata.nextCursor`, вид
  `имя:версия`), `search` (подстрока в имени), `version=latest` (без него приходят все версии, у
  старых `isLatest:false`), `updated_since` (RFC 3339, принят). Деталь:
  `GET /v0.1/servers/{имя с %2F}/versions/latest`.
- Ответ: `{servers:[{server:{$schema, name, title, description, version, repository{url,source,subfolder}, packages[], remotes[]}, _meta:{"io.modelcontextprotocol.registry/official":{status, publishedAt, updatedAt, isLatest}}}], metadata:{nextCursor, count}}`.
- **Грабля:** у меня большие страницы (`limit=30`, `100`, без фильтра) обрывались на ~12 КБ и висли
  40-60 с, `limit=10` отдавался за 0,8 с, `search=playwright&limit=5` - нормально. Возможно, дело в моей
  сети. В клиенте: `limit<=10`, таймаут 15 с, повтор, кэш.
- `server.json`, поля **[curl]**:
  - `packages[]`: `registryType` (видел `npm`, `oci`), `registryBaseUrl`, `identifier`, `version`,
    `runtimeHint` (`npx`), `transport{type:"stdio"}`, `runtimeArguments[{type:"positional|named", name, value, description, variables}]`,
    `packageArguments`, `environmentVariables[{name, description, isRequired, isSecret, default, format}]`.
  - `remotes[]`: `{type:"streamable-http|sse", url, headers[{name, value:"Bearer {smithery_api_key}", isSecret, isRequired, description}]}`.
    Подстановки в `{фигурных}` скобках - это то, что пользователь вводит сам.
  - Пакеты других типов (`pypi`, `nuget`, `mcpb`) в живых данных не видел **[?]**.
- Имена - обратный DNS: `io.github.github/github-mcp-server`, `ai.smithery/...`. Лицензия
  репозитория реестра у GitHub - `NOASSERTION`; условия использования данных не нашёл **[?]**.

### 2.2 GitHub MCP Registry

`GET https://api.mcp.github.com/v0.1/servers?limit=1` **[curl]**, без ключа. Тот же формат плюс в
`_meta["io.modelcontextprotocol.registry/publisher-provided"].github`: `stars, license, readme,
primaryLanguage, pushedAt, nameWithOwner, displayName`. Пагинация: `metadata{count, nextCursor,
per_page, total:604, total_pages}`. Курированный список (604 сервера), годится для сортировки по
звёздам и показа лицензии. Ответ тяжёлый из-за `readme` - просить по 5-10.

### 2.3 Smithery

- `GET https://registry.smithery.ai/servers?q=<текст>&pageSize=<n>&page=<n>` **[curl]**, без ключа.
  Элемент: `{id, qualifiedName, namespace, displayName, description, iconUrl, verified, useCount, remote, isDeployed, unlisted, inactive, createdAt, homepage, bySmithery, owner, score}`;
  `pagination{currentPage, pageSize, totalPages, totalCount}`.
- Деталь `GET /servers/{qualifiedName}` **[curl]**: `{qualifiedName, deploymentUrl, connections[{type:"http", deploymentUrl, configSchema}], security, tools[{name, description, inputSchema}]}`.
- Хостящиеся серверы Smithery в официальном реестре требуют `Authorization: Bearer {smithery_api_key}` -
  нужен аккаунт пользователя. Условия использования Smithery не читал (их сайт `smithery.ai/docs`
  не отозвался) **[?]**: перед включением в приложение прочитать.

### 2.4 Не подходят

- **Glama** `https://glama.ai/api/mcp/v1/servers`: 401, нужен ключ, и по тексту ответа лицензия
  данных требует видимой ссылки на Glama на каждой странице **[curl]**.
- **PulseMCP**: `v0beta` отвечает 410 `API_SUNSET`; `/v0.1/servers` - 401, нужен заголовок `X-API-Key` **[curl]**.
- **mcp.so**: сайт открывается, публичного API не нашёл (`/api/servers` не ответил) **[?]**.

### 2.5 Как добавить MCP в каждую среду

| Среда | Команда | Файл и формат |
|---|---|---|
| Claude Code **[док]** | `claude mcp add --transport http <имя> <url> [--header "K: V"]`; `claude mcp add --transport stdio <имя> --env K=V -- <команда> <арги>`; `claude mcp add-json <имя> '<json>'`; `--scope local/project/user` | `~/.claude.json` -> `mcpServers` (user), `.mcp.json` (project). `{"type":"http\|sse\|ws","url","headers"}` или `{"command","args","env"}`. `url` без `type` - ошибка конфигурации. Уже так в `layout.rs` |
| Codex **[док]** | `codex mcp add <имя> --env K=V -- <команда>` | `~/.codex/config.toml`: `[mcp_servers.<имя>]` с `command, args, env, env_vars` или `url, bearer_token_env_var, http_headers, env_http_headers`; общие `startup_timeout_sec, enabled, enabled_tools`. Пример из доки: `[mcp_servers.figma] url="https://mcp.figma.com/mcp" bearer_token_env_var="FIGMA_OAUTH_TOKEN"` |
| Gemini CLI **[док]** | правка файла | `~/.gemini/settings.json` -> `mcpServers.<имя>`: `command, args, env, cwd` (stdio), `url` (SSE), `httpUrl` (streamable HTTP), `headers`, `timeout` (по умолчанию 600000 мс), `trust` (true = без подтверждений - не ставить), `includeTools, excludeTools`, `oauth` |
| opencode **[док]** | правка файла | `opencode.json` -> `mcp.<имя>`: локальный `{"type":"local","command":["npx","-y","пакет"],"environment":{},"enabled":true,"timeout":5000}`, удалённый `{"type":"remote","url","headers","enabled","oauth"}`. Команда - массив, а не строка |
| Qwen | как Gemini (`mcpServers` в `~/.qwen/settings.json`) | в доке Qwen не сверял **[?]** |

### 2.6 Из `server.json` в конфиг среды

- `packages[npm]` -> `command = runtimeHint ("npx")`, аргументы: positional из `runtimeArguments`,
  затем `identifier@version`, затем `packageArguments`; версию пинуть (как делает скрипт пользователя).
- `packages[oci]` -> `docker run -i --rm ...` из `runtimeArguments`: нужен Docker, показывать явно.
- `remotes[]` -> `http`/`sse`; заголовки с `{placeholder}` и `isSecret` -> недостающие переменные (G25.3), значение
  в связку ключей, в файле только ссылка на переменную (Codex: `bearer_token_env_var`/`env_http_headers`;
  Claude Code: `${VAR}` в `headers`).
- `environmentVariables` -> `EnvVarNeed` (имя, кому нужна, секретная ли).

## 3. Моды Claude Code

**Что это** **[док]**: мод - плагин, чьи обработчики событий (вызов инструмента, отправка промпта,
отрисовка панели) выполняются внутри Claude Code как функции. Не путать с «settings hooks» (команда в
`settings.json`). Документация: `https://code.claude.com/docs/en/plugins/mods/overview.md` (там же
`create`, `reference`, `api`, `events`, `interface`, `gallery`, `test`, `troubleshoot`, `admin`).
Работает в CLI, Desktop (Code) и VS Code, с Claude Code v2.1.287.

**Структура** (по пяти локальным модам в `~/.claude/mods/`: arch-pilot, clawd-run, proxy-lock,
tolya-guards, tolya-text):
```
<мод>/.claude-plugin/plugin.json   {"name","version","description","types"?}
<мод>/hooks/hooks.json             {"modules":["./register.tsx"]}
<мод>/hooks/register.ts(x)         export const register: Register = (on, options) => {...}
<мод>/hooks/*.test.ts              тесты (claude plugin test)
<мод>/tsconfig.json                {"extends":"./.claude-plugin/types/tsconfig.json"}
<мод>/types/index.d.ts             контракт $.state (только у модов с состоянием)
<мод>/skills/, bin/                необязательно (у arch-pilot есть оба)
<мод>/.claude-plugin/types/        пишет сам движок, в git игнорируется - в экспорт не брать
```
Мод может быть привязан к машине: `proxy-lock` проверяет порты 7899/10808 через `ss -ltnH`, это
Linux-специфика. Перенос не гарантирует работу.

**Как они подключены у пользователя:** `env.CLAUDE_CODE_PLUGIN_DIRS` в `~/.claude/settings.json` -
пути через двоеточие (не маркетплейс). Папка `mods/` синхронизируется `claude_sync.py` (`LINKED`).

**Способы поделиться и поставить** **[док]**:
1. Репозиторий git с `.claude-plugin/marketplace.json` (`{name, owner:{name}, plugins:[{name, source:"./"}]}`),
   установка: `/plugin install <мод> --marketplace <owner>/<repo>` или в оболочке
   `claude plugin marketplace add <owner>/<repo>` + `claude plugin install <мод>@<маркетплейс>`.
   Папка как маркетплейс (`claude plugin marketplace add <папка>`) читается на месте.
2. Папка или zip для одной сессии: `claude --plugin-dir <путь>` (есть `--plugin-url` для zip из релиза).
3. Положить папку плагина с `.claude-plugin/plugin.json` в `~/.claude/skills/<имя>/` - загрузится на
   каждом старте (если администратор не запретил).
4. `CLAUDE_CODE_PLUGIN_DIRS` (способ пользователя).
После установки из оболочки в открытой сессии нужен `/reload-plugins`.

**Публичный каталог модов:** отдельного не нашёл. Есть образцы Anthropic, Apache-2.0, «как есть, без
поддержки» **[curl]**: `github.com/anthropics/claude-code-playground/tree/main/claude-code/mods`
(`token-weather`, `blast-radius`, `replay-theater`, README и `.claude-plugin`). В `marketplace.json`
признака «это мод» нет: определять по файлу `hooks/hooks.json` с ключом `modules` после загрузки.

**Что исполняемо и риск** **[док]**: код с правами пользователя, не в песочнице; читает файлы,
переменные окружения и `settings.json` (в них бывают ключи), запускает процессы, ходит в сеть, видит
каждый промпт и вызов инструмента и может их менять, одобрять вызовы без вопроса, тратить подписку.
Заранее можно посмотреть без запуска: `claude plugin validate <папка>` печатает строки `hooks:`,
`calls:` (`$.fs.read/write`, `$.process.run/spawn`, `$.http.fetch`, `$.env.get/set`, `$.mcp.call`,
`$.model.complete`, `$.prompt.submit`, `$.session.send`), `env reads:`, `env writes:`. Выключатели:
плагин в `/plugin`, `--safe-mode`, `disableAllHooks: true`.

## 4. Что пользователь уже делает у себя

- `~/bin/update-skills-mcp.sh` (launchd, раз в час с проверкой «прошло 20 ч») - старый вариант:
  `git pull --ff-only` для клонов в `~/.claude/skills/*` и `~/.agents/skills/*`, `claude plugin
  marketplace update` + `plugin update`, пины npm-MCP в `~/.claude.json` поднимаются по `npm view`,
  `~/mcp/*` подтягиваются и пересобираются, `uv tool upgrade`. Рядом лежит более полный `~/.claude/hooks/update-skills-mcp.py` (1130 строк, читает реестр ниже); какой из двух запускает launchd, не проверял.
- `~/.claude/skills-sources.json` (симлинк в Dropbox `claude-shared/claude/files/`, копия
  `skills-sources.json.bak-2026-10-20`): `repos{"owner/repo": {skills:{имя: путь_в_репо}, files?, _note?}}` (14 репозиториев),
  `_excluded{имя: причина}`, `packages{uv_tools, pip}`, `mcp_remote{сервер: {ssh, service, health_url}}`.
- Механика обновления: `git clone --depth 1`, по каждому скиллу сравнение хеша дерева, копия со
  штампом-бэкапом, поиск переехавшего скилла по `name:` (`_find_moved`), чужие и личные скиллы вне
  реестра не трогаются. Подхватывает `~/.agents/.skill-lock.json` от `npx skills` (поля
  `source, sourceType, skillPath`) и вписывает такие скиллы в реестр сам.
- Что переиспользовать: (1) запись «источник» `{repo, путь}` как формат `origin` элемента каталога;
  (2) сравнение хеша дерева для «Проверить обновление» (G25.7); (3) чтение `.skill-lock.json` -
  показать «поставлено через npx skills»; (4) правило «обновлять только то, что поставил каталог»;
  (5) `_excluded` как «не обновлять» с причиной; (6) известные грабли: в корне апстрима нет
  `SKILL.md` (случай `reverse-skill`), `eol=crlf` в `.gitattributes`.

## 5. Итог

### 5.1 Рекомендуемые источники встроенного каталога

| Вид | Источники (по приоритету) | URL, проверенные живьём |
|---|---|---|
| Скиллы | 1. skills.sh (популярность) 2. маркетплейсы Anthropic (официальный + `anthropics/skills`, при желании community) 3. любой GitHub `owner/repo` по ссылке (+ при желании SkillsMP) | `https://skills.sh/api/search?q=&limit=`; `https://raw.githubusercontent.com/anthropics/skills/main/.claude-plugin/marketplace.json`; `.../anthropics/claude-plugins-official/main/.claude-plugin/marketplace.json`; `https://skillsmp.com/api/v1/skills/search?q=` |
| MCP | 1. официальный реестр 2. GitHub MCP Registry (звёзды, лицензия) 3. Smithery (хостящиеся, после чтения условий) | `https://registry.modelcontextprotocol.io/v0.1/servers?version=latest&search=&limit=10`; `https://api.mcp.github.com/v0.1/servers?limit=5`; `https://registry.smithery.ai/servers?q=&pageSize=10` |
| Моды | 1. образцы Anthropic (`claude-code-playground`) 2. свои моды пользователя из `~/.claude/mods` (экспорт и импорт набором) 3. любой git-репозиторий с `.claude-plugin/marketplace.json` по ссылке | `https://api.github.com/repos/anthropics/claude-code-playground/contents/claude-code/mods`; ссылка от пользователя |

Для всех: HTTPS, список разрешённых хостов (добавить `raw.githubusercontent.com`, `api.github.com`,
`codeload.github.com`), кэш с ETag, таймаут 15 с. Анонимный GitHub API - 60 запросов/ч **[curl]**:
для скачивания папки скилла лучше клон с проверкой дерева (уже есть в `source.rs`), а не `contents`.

### 5.2 Формат одного элемента каталога

```json
{
  "id": "skills.sh:vercel-labs/agent-skills/web-design-guidelines",
  "kind": "skill | mcp | mod",
  "name": "web-design-guidelines",
  "description": "из SKILL.md / server.json, обычный текст",
  "catalog": {"name": "skills.sh", "url": "https://skills.sh/..."},
  "origin": {"type": "github", "repo": "owner/repo", "path": "skills/x", "ref": "<sha>"},
  "popularity": {"installs": 828545, "stars": null, "use_count": null},
  "license": "MIT",
  "envs": ["claude", "codex", "gemini", "opencode"],
  "runs_code": {"scripts": ["scripts/run.py"], "command": "npx -y pkg@1.2.3", "in_process": false},
  "env_vars": [{"name": "API_KEY", "secret": true, "required": true}],
  "needs": ["node"],
  "audit": null
}
```
`origin.type`: `github | git | npm | pypi | oci | remote | archive`. `ref` - всегда коммит (ветку
разворачивать в sha при показе). `license: null` показывать как «лицензия не указана».

### 5.3 Правила безопасности установки

1. Каталог даёт только метаданные. На диск ничего не пишется до экрана «Что изменится» (G25.2);
   установка идёт тем же конвейером, что импорт набора: каталожный элемент собирается в набор в памяти.
2. Лимиты и проверки `source.rs` для скачанного: файл до 10 МБ, всего до 50 МБ, до 5000 файлов,
   симлинки и `..` в путях - отказ.
3. Устанавливается ровно показанный коммит; обновление - только по кнопке, новые и изменённые
   исполняемые части снова выключены (G25.7).
4. Выключено до галочки (как `Held`): любой MCP (команда запуска или удалённый адрес с заголовками);
   хуки; скилл со скриптами, с `` !`cmd` `` или с `hooks/permissions/tools` во фронтматтере
   (`runs_in_markdown`); **любой мод целиком** (код внутри Claude Code), даже если «ничего не делает».
5. Для мода на экране «Что изменится» показать вывод `claude plugin validate <папка>` (строки
   `hooks:`, `calls:`, `env reads/writes:`) и полный текст `register.*`; по умолчанию ставить на
   уровень пользователя только после галочки, в проект - не предлагать.
6. Секреты: из `isSecret`/`{placeholder}` делаются «недостающие переменные», значения - в связку
   ключей; в наборе, логах и файлах CLI значения нет. Ключей каталогов (Vercel, Smithery, Glama) в
   приложении не хранить.
7. Описания, README и `SKILL.md` из каталогов - недоверенный текст: показывать как простой текст,
   без автозагрузки картинок и ссылок, не подавать агентам автоматически (внедрение инструкций).
   `SKILL.md` - инструкции для агента: показывать целиком до установки.
8. Версии пинуть (`пакет@x.y.z`, не `@latest`): `npx`, `uvx`, `docker` выполняют чужой код при первом
   запуске среды, и это видно в показанной команде. Docker-пакеты помечать «нужен Docker».
9. Имя похожее на известное (опечатка), другой владелец у такого же имени - предупреждение с
   владельцем, `verified`, числом установок и датой.
10. `trust: true` (Gemini), разрешения «без подтверждений» в настройках MCP не выставлять.
11. Установка в среду, не подходящую по `envs`, - отказ с причиной (G25.6); мод - только Claude Code.
12. Каталоги с обязательной ссылкой-атрибуцией или платным ключом (Glama, PulseMCP) не включать.

### 5.4 Что найдено в `core/src/kits/` и надо поправить

1. `SCRIPT_EXT` в `units.rs` (строки 181-183) содержит `ts`, `js`, `mts`, но не `tsx`, `jsx`: файл
   `hooks/register.tsx` (как у `clawd-run`) не пометится скриптом. Надёжнее: любая папка с
   `.claude-plugin/plugin.json` целиком считается исполняемой.
2. Группы «Моды» нет (в `kits/` нет упоминаний mods/marketplace/`.claude-plugin`). Нужна новая группа
   только для Claude Code: файлы в `~/.claude/skills/<мод>/` (способ 3 из раздела 3) или регистрация через
   `claude plugin marketplace add <папка>` + `install`. В `settings.json` пользователя мод подключён
   иначе (`CLAUDE_CODE_PLUGIN_DIRS`), это надо учесть при экспорте и в «уже использует».
3. Раскладка скиллов Codex, Gemini, Qwen помечена `unverified`. Документация подтверждает Codex
   `~/.agents/skills`, Gemini `~/.gemini/skills` и `~/.agents/skills`; Qwen и Cursor - только по таблице
   Vercel. По правилу спеки §26 `[CLI-факт]` сначала проверить на установленных CLI.
4. MCP из реестра нужно перевести в формат `.mcp.json` набора (`KIT_MCP`) по правилам 2.6; формат
   каждой среды уже описан в `layout.rs` (`mcpServers` / `mcp_servers` / `mcp`).
