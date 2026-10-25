window.STATE =
{
  "slug": "vibe-env",
  "title": "Среда для вайб-кодинга: агенты работают сами",
  "mode": "semi",
  "depth": "deep",
  "polish": null,
  "tier": "T2",
  "briefFile": "2026-10-21-brief.md",
  "memoryFile": "CLAUDE.md",
  "startedAt": "2026-10-21T08:41:14+03:00",
  "updatedAt": "2026-10-23T18:55:01+03:00",
  "finishedAt": null,
  "stages": [
    {
      "id": "preflight",
      "status": "done",
      "startedAt": "2026-10-21T08:41:14+03:00",
      "finishedAt": "2026-10-21T08:42:22+03:00"
    },
    {
      "id": "manifest",
      "status": "done",
      "startedAt": "2026-10-21T08:42:22+03:00",
      "finishedAt": "2026-10-23T00:59:59+03:00"
    },
    {
      "id": "briefing",
      "status": "done",
      "startedAt": "2026-10-21T08:42:22+03:00",
      "finishedAt": "2026-10-23T17:42:35+03:00",
      "note": "v6: 3 развилки + чат и терминал"
    },
    {
      "id": "spec",
      "status": "done",
      "startedAt": "2026-10-21T12:15:27+03:00",
      "note": "v6: истории 313-359, G2 - 6 находок, все закрыты",
      "finishedAt": "2026-10-23T17:42:35+03:00"
    },
    {
      "id": "plan",
      "status": "done",
      "startedAt": "2026-10-21T13:22:54+03:00",
      "note": "v4: 9 тасков (18-26) в волны 12-15, ~21K строк",
      "finishedAt": "2026-10-23T00:59:59+03:00"
    },
    {
      "id": "build",
      "status": "done",
      "startedAt": "2026-10-21T09:02:22+03:00",
      "note": "все 14 тасков готовы; ремонт ядра: исполнитель прошлой сессии пропал, 22.10 18:30 - свежий исполнитель доводит (наборы, 5 нестабильных тестов, диапазоны)",
      "finishedAt": "2026-10-23T00:59:59+03:00"
    },
    {
      "id": "review",
      "status": "done",
      "startedAt": "2026-10-21T10:07:22+03:00",
      "note": "проверены все готовые",
      "finishedAt": "2026-10-23T00:59:59+03:00"
    },
    {
      "id": "final",
      "status": "active",
      "startedAt": "2026-10-23T00:59:59+03:00"
    }
  ],
  "requirements": {
    "total": 107,
    "done": 49,
    "inTicket": 54,
    "inSpec": 0,
    "placeholder": 0,
    "deferred": 1,
    "dropped": 3
  },
  "tickets": [
    {
      "id": "01",
      "title": "Каркас, общие типы, хранилище, git",
      "requirements": [
        "R01",
        "R19i",
        "R06.4",
        "R06.5",
        "G01.5",
        "G01.6",
        "R17i.1",
        "R11.2",
        "R11.3"
      ],
      "blockedBy": [],
      "wave": 1,
      "zone": [
        "Cargo.toml",
        "core/",
        "core/src/agent/adapters/",
        "core/src/engine/",
        "core/src/telegram/",
        "app/",
        "ui/",
        "index.html"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-21T09:03:14+03:00",
      "finishedAt": "2026-10-21T10:14:53+03:00",
      "retries": 0,
      "repairs": 1,
      "tests": {
        "passed": 40,
        "failed": 0
      },
      "commit": "483310e",
      "concerns": [
        "run_blocking может зависнуть на фоновом процессе агента - дозапрос",
        "подпись: кеш и только недостающий ключ - дозапрос"
      ]
    },
    {
      "id": "02",
      "title": "Адаптеры агентных сред",
      "requirements": [
        "R14i",
        "R14i.2",
        "G02",
        "G02.1",
        "G02.2",
        "A01",
        "R17i"
      ],
      "blockedBy": [
        "01"
      ],
      "wave": 2,
      "zone": [
        "core/src/agent/adapters/",
        "core/tests/fixtures/agents/",
        "adapters()",
        "core/src/agent/"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-21T10:14:53+03:00",
      "finishedAt": "2026-10-21T12:04:29+03:00",
      "retries": 0,
      "repairs": 2,
      "tests": {
        "passed": 27,
        "failed": 0
      },
      "commit": "3ce27ee",
      "concerns": [
        "Codex/Gemini/Cursor - флаги только по документации",
        "коммит ждёт окончания правки agent/mod.rs таском 03"
      ]
    },
    {
      "id": "03",
      "title": "Движок задач: цикл «поставил → сделано»",
      "requirements": [
        "A05",
        "R04",
        "R05",
        "R06",
        "R06.1",
        "R06.2",
        "R06.3",
        "R06.4",
        "R06.5",
        "R07",
        "R07.1",
        "R07.2",
        "R16i",
        "R16i.1",
        "R16i.2",
        "R18i",
        "R19i",
        "R19i.1",
        "R19i.2",
        "R19i.3",
        "G01",
        "G01.1",
        "G01.3",
        "G01.4",
        "G01.5",
        "R17i.1",
        "A01"
      ],
      "blockedBy": [
        "01"
      ],
      "wave": 2,
      "zone": [
        "core/src/engine/",
        "core/tests/engine_*.rs"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-21T10:14:53+03:00",
      "finishedAt": "2026-10-21T12:04:29+03:00",
      "retries": 0,
      "repairs": 2,
      "tests": {
        "passed": 36,
        "failed": 0
      },
      "commit": "45cc523",
      "concerns": [
        "в доработку движка v2: тест на не-UTF-8 вывод, тест retry после вливания, ns-hidden: ошибка чтения кроме NotFound → Err",
        "state между ходами в .pending/<id>.json, не в Task"
      ]
    },
    {
      "id": "04",
      "title": "Telegram-бот: оповещения, ответы, задачи с телефона",
      "requirements": [
        "R07",
        "R15i",
        "R15i.1",
        "R15i.2",
        "R15i.3",
        "R15i.4",
        "R15i.5",
        "A02",
        "A03",
        "G01.1"
      ],
      "blockedBy": [
        "01"
      ],
      "wave": 2,
      "zone": [
        "core/src/telegram/",
        "core/tests/telegram_*.rs"
      ],
      "review": null,
      "model": null,
      "status": "pending",
      "retries": 0,
      "repairs": 0
    },
    {
      "id": "05",
      "title": "Приложение: окно, трей, горячая клавиша, уведомления",
      "requirements": [
        "R08",
        "R08.1",
        "R09",
        "R10",
        "R11",
        "R11.1",
        "R11.5",
        "R15i",
        "R18i",
        "R14i.2"
      ],
      "blockedBy": [
        "02",
        "03",
        "06"
      ],
      "wave": 3,
      "zone": [
        "app/",
        "ui/app.js"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-21T13:15:59+03:00",
      "finishedAt": "2026-10-21T13:34:57+03:00",
      "retries": 0,
      "repairs": 2,
      "tests": {
        "passed": 6,
        "failed": 0
      },
      "commit": "ae4bc2b",
      "concerns": [
        "вручную: клик по уведомлению и счётчик в строке меню на маке",
        "Linux-сборка не проверена",
        "пересоздание Engine прерывает задачи - Engine::set_adapters в движке v2"
      ]
    },
    {
      "id": "06",
      "title": "Интерфейс: задачи, карточка, проекты, настройки",
      "requirements": [
        "R03",
        "R03.1",
        "R03.2",
        "R04",
        "R06",
        "R06.3",
        "R07.3",
        "R11",
        "R11.2",
        "R11.3",
        "R11.4",
        "R11.5",
        "R11.6",
        "R11.7",
        "R16i.1",
        "R18i",
        "G01",
        "G01.1",
        "G01.2",
        "G01.3",
        "G01.4",
        "G02",
        "G02.1",
        "G02.2",
        "R14i.2",
        "R15i",
        "A04"
      ],
      "blockedBy": [
        "01"
      ],
      "wave": 2,
      "zone": [
        "ui/"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-21T10:17:09+03:00",
      "finishedAt": "2026-10-21T10:40:37+03:00",
      "retries": 0,
      "repairs": 1,
      "commit": "6f709b8",
      "concerns": [
        "пользователь попросил кардинально переделать интерфейс - это база для переделки"
      ]
    },
    {
      "id": "07",
      "title": "Open-source: README, лицензия, сборки в CI, знаки и товарные знаки",
      "requirements": [
        "R02",
        "R02.1",
        "R12",
        "R13",
        "R20i",
        "R20i.1",
        "R20i.2",
        "R21",
        "G04",
        "G21.2",
        "G25.13",
        "G27"
      ],
      "blockedBy": [
        "13",
        "15"
      ],
      "wave": 5,
      "zone": [
        "README.md",
        "README.ru.md",
        "LICENSE",
        "CONTRIBUTING.md",
        ".github/",
        "docs/",
        "scripts/install.sh",
        "app/tauri.conf.json",
        "bundle",
        "app/installer/"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-21T21:01:10+03:00",
      "finishedAt": "2026-10-21T21:48:23+03:00",
      "retries": 0,
      "repairs": 1,
      "commit": "151cace",
      "concerns": [
        "не опубликовано; заглушки <OWNER>, <REPO>, <REPO_URL>, <SECURITY_CONTACT>, <MAINTAINER>, sha256sums SKIP",
        "скриншот открытого .dmg не снят - фон Finder не отрисовался из оболочки",
        "theSVG /legal и правила брендов вендоров не проверены",
        "воркфлоу не запускались на GitHub; checksums без --clobber; пустой черновик при упавшей сборке",
        "имя «Nightshift» не меняется одной заменой (install.sh, PKGBUILD)",
        "PKGBUILD и AppImage на Linux не проверены"
      ]
    },
    {
      "id": "08",
      "title": "Ядро v2: типы, хранилище, адаптеры с моделью и лимитами, каталог моделей, git",
      "requirements": [
        "R01",
        "R13",
        "R14i",
        "R14i.2",
        "R17i",
        "R17i.2",
        "R17i.3",
        "R19i.5",
        "G02",
        "G02.1",
        "G07.1",
        "G09.1",
        "G11.2",
        "G15.3",
        "G16.1",
        "G16.4",
        "G17",
        "G17.2",
        "G17.3",
        "G18",
        "G18.1",
        "G18.2",
        "G18.3",
        "G18.4",
        "G18.5",
        "G20.5",
        "G20.6",
        "G20.8",
        "G20.11",
        "G22",
        "G22.1",
        "G23",
        "G23.1",
        "G23.2",
        "G24.6",
        "G25.8",
        "G26",
        "G26.1",
        "G26.2",
        "A01",
        "A07",
        "A07.1"
      ],
      "blockedBy": [],
      "wave": 1,
      "zone": [
        "core/src/model.rs",
        "core/src/store.rs",
        "core/src/git.rs",
        "core/src/agent/",
        "adapters/",
        "core/src/models/",
        "core/Cargo.toml",
        "core/tests/{store,git,adapters,agent}.rs",
        "core/tests/fixtures/"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-21T13:36:15+03:00",
      "finishedAt": "2026-10-21T14:19:01+03:00",
      "retries": 0,
      "repairs": 1,
      "tests": {
        "passed": 158,
        "failed": 0
      },
      "commit": "17f8aa7",
      "concerns": [
        "лимиты не-Claude сред - нет данных (источник не подтверждён)",
        "версии Claude/Gemini/Qwen из документации 2026-10-21, не сверены с CLI",
        "форматы Codex/Cursor - по документации"
      ]
    },
    {
      "id": "09",
      "title": "Движок v2: роли, «Качество», промпт под модель, одна среда на задачу, лимиты, ночь, заморозка",
      "requirements": [
        "R01",
        "R03.2",
        "R03.3",
        "R04.1",
        "R05",
        "R05.1",
        "R05.2",
        "R06",
        "R06.1",
        "R06.2",
        "R06.3",
        "R06.4",
        "R06.5",
        "R06.6",
        "R07",
        "R07.1",
        "R07.2",
        "R11.9",
        "R16i",
        "R16i.1",
        "R16i.2",
        "R17i",
        "R17i.1",
        "R17i.2",
        "R18i",
        "R18i.1",
        "R19i",
        "R19i.1",
        "R19i.2",
        "R19i.3",
        "R19i.4",
        "R19i.5",
        "G01",
        "G01.1",
        "G01.5",
        "G01.7",
        "G01.8",
        "G01.9",
        "G05",
        "G05.1",
        "G05.2",
        "G07",
        "G07.1",
        "G07.2",
        "G09",
        "G09.1",
        "G09.2",
        "G09.3",
        "G09.4",
        "G09.5",
        "G09.6",
        "G10",
        "G10.1",
        "G12.1",
        "G14",
        "G14.1",
        "G14.2",
        "G14.3",
        "G14.4",
        "G15",
        "G15.1",
        "G15.2",
        "G15.3",
        "G15.4",
        "G17.2",
        "G17.3",
        "G18.4",
        "G18.5",
        "G22",
        "G22.1",
        "G22.2",
        "G22.3",
        "G23.2",
        "G26.4",
        "A05",
        "A06",
        "A06.1",
        "A07",
        "A07.3",
        "A08",
        "A08.1",
        "A08.3",
        "D01"
      ],
      "blockedBy": [
        "08"
      ],
      "wave": 2,
      "zone": [
        "core/src/engine/",
        "core/src/prompt/",
        "core/tests/engine_*.rs",
        "core/src/git.rs",
        "hidden_specs"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-21T14:19:01+03:00",
      "finishedAt": "2026-10-21T17:10:19+03:00",
      "retries": 0,
      "repairs": 1,
      "tests": {
        "passed": 284,
        "failed": 0
      },
      "commit": "5195c56",
      "concerns": [
        "G14: при выключенном «Качестве» промпт уходит как есть - ждёт решения пользователя"
      ]
    },
    {
      "id": "10",
      "title": "Рой и фабрика в движке: три режима, поручения, Надзиратель, очередь слияния",
      "requirements": [
        "G08",
        "G08.1",
        "G08.2",
        "G08.3",
        "G08.4",
        "G08.7",
        "G09",
        "G11",
        "G11.1",
        "G11.2",
        "G11.3",
        "G11.4",
        "G11.5",
        "G11.6",
        "G11.7",
        "G11.8",
        "G11.9",
        "G12.2",
        "G15",
        "G18.2",
        "G22.2",
        "G26.4",
        "R19i.2",
        "R18i"
      ],
      "blockedBy": [
        "09"
      ],
      "wave": 3,
      "zone": [
        "core/src/engine/",
        "core/src/swarm/",
        "core/tests/engine_swarm*.rs"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-21T17:10:19+03:00",
      "finishedAt": "2026-10-21T19:33:33+03:00",
      "retries": 0,
      "repairs": 1,
      "tests": {
        "passed": 341,
        "failed": 0
      },
      "commit": "b3df9d0",
      "concerns": [
        "бригада вливает часть копированием файлов, а не rebase + ff_branch",
        "автовыбор роя не сдвигается заморозкой",
        "hide_untracked/restore_paths пишут в git копии вне with_git (без гонки, нет комментария); git_dir_of молча откатывается на path при нераспознанном .git",
        "engine_swarm_race::when_every_variant_fails… нестабилен под параллельной нагрузкой (5/5 отдельно зелёный) - починить после ремонта 13"
      ]
    },
    {
      "id": "11",
      "title": "Чат с моделью среды: разговор, задача из сообщения, заметки агентам",
      "requirements": [
        "R03",
        "R04",
        "G05",
        "G05.1",
        "G05.2",
        "G07.1",
        "G20",
        "G20.1",
        "G20.2",
        "G20.3",
        "G20.4",
        "G20.5",
        "G20.6",
        "G20.7",
        "G20.8",
        "G20.9",
        "G20.10",
        "G20.11",
        "G20.12",
        "G22"
      ],
      "blockedBy": [
        "09"
      ],
      "wave": 3,
      "zone": [
        "core/src/chat/",
        "core/tests/chat*.rs"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-21T17:10:19+03:00",
      "finishedAt": "2026-10-21T19:57:25+03:00",
      "retries": 0,
      "repairs": 1,
      "tests": {
        "passed": 351,
        "failed": 0
      },
      "commit": "49f7f73",
      "concerns": [
        "очередь ответов и ожидание лимита чата в памяти - после перезапуска только retry",
        "дубль send(New) после перезапуска создаст второй чат",
        "delivered_at не заполняется - доставка строкой note_delivered"
      ]
    },
    {
      "id": "12",
      "title": "Наборы: экспорт и импорт скиллов, правил, хуков, MCP, субагентов; секреты; снимки и откат",
      "requirements": [
        "G25",
        "G25.1",
        "G25.2",
        "G25.3",
        "G25.4",
        "G25.5",
        "G25.6",
        "G25.7",
        "G25.8",
        "G25.9",
        "G25.10",
        "G25.11",
        "G25.12",
        "G22.2",
        "R17i.2",
        "R13"
      ],
      "blockedBy": [
        "08"
      ],
      "wave": 2,
      "zone": [
        "core/src/kits/",
        "core/tests/kits*.rs"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-21T14:19:01+03:00",
      "finishedAt": "2026-10-21T15:09:27+03:00",
      "retries": 0,
      "repairs": 2,
      "tests": {
        "passed": 20,
        "failed": 0
      },
      "commit": "4015b3a",
      "concerns": [
        "порядок ключей в переписанном JSON - по алфавиту",
        "несверенные группы: Cursor, скиллы Codex/Gemini/Qwen, субагенты Codex",
        "доработка 12a сделана (dcb734e): откат не затирает изменённое, NotApplied, без ленивой догрузки"
      ]
    },
    {
      "id": "13",
      "title": "Приложение v2: новые команды и события, вход и установка сред, присутствие, тихие часы, терминал",
      "requirements": [
        "R07.3",
        "R08",
        "R08.1",
        "R08.2",
        "R09",
        "R10",
        "R10.1",
        "R10.2",
        "R11",
        "R11.1",
        "R11.5",
        "R11.10",
        "R14i",
        "R14i.2",
        "R15i",
        "R15i.6",
        "R15i.7",
        "R15i.8",
        "R17i.3",
        "G01.6",
        "G01.7",
        "G02.1",
        "G15.4",
        "G16.2",
        "G16.3",
        "G16.4",
        "G17.2",
        "G17.3",
        "G18.1",
        "G20",
        "G25",
        "G26",
        "A06.1",
        "A07",
        "A07.2",
        "A08.1",
        "A08.2"
      ],
      "blockedBy": [
        "05",
        "10",
        "11",
        "12"
      ],
      "wave": 4,
      "zone": [
        "app/",
        "tauri.conf.json",
        "ui/"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-21T19:57:42+03:00",
      "finishedAt": "2026-10-21T21:00:58+03:00",
      "retries": 0,
      "repairs": 1,
      "tests": {
        "passed": 403,
        "failed": 0
      },
      "commit": "e81e7c2",
      "concerns": [
        "уже идущий запуск при удалении папки проекта падает, удержание ловит задачи до старта",
        "значения PATH/HOME/XDG_*/CLAUDE_CONFIG_DIR/CODEX_HOME из набора не проверяются - в ремонте после 13",
        "токен start_fresh не граница доверия (защита - CSP и подтверждение в окне)",
        "диапазоны настроек: копия в ui/logic.js без теста на совпадение с model::SETTING_RANGES",
        "CLI, берущая ключ только из extra_env, в терминале его не получит",
        "Linux-сборка (deb, appimage) не проверена; Adapter::version зовёт --version у 6 сред"
      ]
    },
    {
      "id": "14",
      "title": "Интерфейс v2, часть 1: среды, чат, модель и версия, задача в ленте, лимиты, первый запуск, «Нужен ты»",
      "requirements": [
        "R01.1",
        "R03",
        "R03.1",
        "R03.2",
        "R04",
        "R04.1",
        "R07.3",
        "R11.2",
        "R11.6",
        "R11.7",
        "R14i.2",
        "G01.7",
        "G01.8",
        "G02.1",
        "G05",
        "G05.1",
        "G05.2",
        "G06",
        "G12",
        "G12.1",
        "G13",
        "G15.1",
        "G15.2",
        "G15.3",
        "G15.4",
        "G17",
        "G17.1",
        "G17.2",
        "G17.3",
        "G17.4",
        "G17.5",
        "G17.6",
        "G18",
        "G18.1",
        "G18.3",
        "G19",
        "G19.1",
        "G19.2",
        "G19.3",
        "G20",
        "G20.1",
        "G20.3",
        "G20.4",
        "G20.7",
        "G20.9",
        "G20.10",
        "G20.11",
        "G20.12",
        "G21",
        "G21.1",
        "G21.2",
        "G23",
        "G23.1",
        "G23.3",
        "G24",
        "G24.1",
        "G24.2",
        "G24.3",
        "G24.4",
        "G24.5",
        "G24.6",
        "G24.7",
        "G26",
        "G26.3",
        "G26.5",
        "G26.7",
        "A07",
        "A07.1",
        "A07.2",
        "15"
      ],
      "blockedBy": [],
      "wave": 1,
      "zone": [
        "ui/",
        "logic.js"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-21T13:36:15+03:00",
      "finishedAt": "2026-10-21T14:19:01+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 69,
        "failed": 0
      },
      "commit": "dc43c99",
      "concerns": [
        "работает на dev-mock до таска 13",
        "«Отложить» - команда в 09"
      ]
    },
    {
      "id": "15",
      "title": "Интерфейс v2, часть 2: рой, фабрика, настройки, наборы и импорт, «Пока тебя не было»",
      "requirements": [
        "R10.1",
        "R10.2",
        "R11.6",
        "R11.7",
        "R15i.6",
        "R15i.7",
        "R17i.3",
        "G08",
        "G08.1",
        "G08.5",
        "G08.6",
        "G08.7",
        "G11",
        "G11.4",
        "G11.5",
        "G11.7",
        "G11.8",
        "G11.9",
        "G12",
        "G12.2",
        "G15",
        "G16",
        "G16.1",
        "G16.2",
        "G16.3",
        "G16.4",
        "G16.5",
        "G16.6",
        "G16.7",
        "G16.8",
        "G16.9",
        "G25",
        "G25.1",
        "G25.2",
        "G25.3",
        "G25.4",
        "G25.5",
        "G25.6",
        "G25.7",
        "G25.8",
        "G25.9",
        "G25.10",
        "G25.11",
        "G25.13",
        "G26.4",
        "G26.6",
        "A06",
        "A06.1",
        "A08",
        "A08.3",
        "A07.2"
      ],
      "blockedBy": [
        "14"
      ],
      "wave": 2,
      "zone": [
        "ui/"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-21T14:19:01+03:00",
      "finishedAt": "2026-10-21T15:39:32+03:00",
      "retries": 0,
      "repairs": 2,
      "tests": {
        "passed": 148,
        "failed": 0
      },
      "commit": "dd28ba9",
      "concerns": [
        "работает на dev-mock до таска 13",
        "нет полей ядра: модели вариантов роя, «понятно» о песочнице, признак Wayland"
      ]
    },
    {
      "id": "16",
      "title": "Ядро v3: каждое действие агента с вводом, выводом и диффом, частичный текст",
      "requirements": [
        "G29",
        "G30",
        "G30.4",
        "G30.2",
        "G30.3",
        "G32"
      ],
      "blockedBy": [],
      "wave": 10,
      "zone": [
        "core/src/agent/",
        "common.rs",
        "mod.rs",
        "core/src/model.rs",
        "LogEvent.action",
        "core/src/engine/",
        "git diff",
        "core/src/chat/",
        "mirror",
        "app/src/main.rs",
        "app/src/hub.rs",
        "core/Cargo.toml",
        "similar",
        "core/tests/"
      ],
      "review": "да - фундамент для таска 17 и путь секретов в окно",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-22T23:02:47+03:00",
      "finishedAt": "2026-10-23T00:21:27+03:00",
      "retries": 0,
      "repairs": 1,
      "repairFindings": [
        "обрезка диффа до scrub; Edit/Write секретного файла отдаёт дифф/head целиком; вложенные .env не в scrub"
      ],
      "tests": {
        "passed": 435,
        "failed": 0
      },
      "commit": "23d6518"
    },
    {
      "id": "17",
      "title": "Интерфейс v3: лента как в Claude Code, вкладки и разделение, первый экран в один шаг",
      "requirements": [
        "G28",
        "G28.1",
        "G28.2",
        "G28.3",
        "G29",
        "G30",
        "G30.1",
        "G30.4",
        "G31",
        "G31.1",
        "G31.2",
        "G31.3",
        "G31.4",
        "G32"
      ],
      "blockedBy": [],
      "wave": 10,
      "zone": [
        "ui/",
        "ui",
        "model.desc.<id>"
      ],
      "review": "да - главный экран пользователя",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-22T23:02:47+03:00",
      "finishedAt": "2026-10-23T00:17:15+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 195,
        "failed": 0
      },
      "commit": "5962b0e"
    },
    {
      "id": "18",
      "title": "Каркас v4 и терминал-мультиплексор в ядре",
      "requirements": [
        "G35",
        "G38",
        "G58",
        "G48"
      ],
      "blockedBy": [],
      "wave": 12,
      "zone": [
        "core/src/term/",
        "core/src/net/",
        "core/src/market/",
        "core/src/updates/",
        "core/src/config/",
        "core/src/lib.rs",
        "core/Cargo.toml",
        "Cargo.lock",
        "app/src/main.rs",
        "app/src/ptycmds.rs",
        "app/src/windows.rs",
        "app/src/market.rs",
        "app/src/updates.rs",
        "app/src/config.rs",
        "app/src/gitcmds.rs",
        "app/src/tray.rs",
        "app/Cargo.toml",
        "app/tauri.conf.json",
        "app/capabilities/",
        "ui/index.html",
        "ui/vendor/xterm/",
        "ui/js/v4/",
        "ui/css/v4-shell.css",
        "ui/css/v4-term.css",
        "ui/css/v4-market.css",
        "ui/css/v4-clawd.css",
        "ui/css/v4-updates.css",
        "core/tests/term.rs"
      ],
      "review": "да - фундамент (PTY, процессы пользователя, окна)",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T02:06:02+03:00",
      "finishedAt": "2026-10-23T02:52:59+03:00",
      "retries": 0,
      "repairs": 2,
      "repairFindings": [
        "выход процесса PTY при открытом окне не вешает приложение"
      ],
      "tests": {
        "passed": 453,
        "failed": 0
      },
      "commit": "a5276dc"
    },
    {
      "id": "19",
      "title": "Ядро: сеть, каталог скиллов/MCP/модов, моды в наборах",
      "requirements": [
        "G40",
        "G41",
        "G42"
      ],
      "blockedBy": [
        "18"
      ],
      "wave": 13,
      "zone": [
        "core/src/net/",
        "core/src/market/",
        "core/src/kits/",
        "app/src/market.rs",
        "core/tests/market.rs",
        "core/tests/fixtures/market/"
      ],
      "review": "да - загрузка и установка исполняемого кода из сети",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T02:43:56+03:00",
      "finishedAt": "2026-10-23T03:25:57+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 485,
        "failed": 0
      },
      "commit": "55d8fa7"
    },
    {
      "id": "20",
      "title": "Ядро: автообновление CLI и моделей, конфиг-файл",
      "requirements": [
        "G43",
        "G54",
        "G51"
      ],
      "blockedBy": [
        "18"
      ],
      "wave": 13,
      "zone": [
        "core/src/updates/",
        "core/src/config/",
        "core/src/models/",
        "app/src/updates.rs",
        "app/src/config.rs",
        "core/tests/updates.rs",
        "core/tests/config.rs",
        "core/tests/fixtures/updates/"
      ],
      "review": "нет",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T03:18:40+03:00",
      "finishedAt": "2026-10-23T03:46:43+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 500,
        "failed": 0
      },
      "commit": "1b6875b"
    },
    {
      "id": "21",
      "title": "Ядро: публикация на GitHub, история коммитов, приёмы экономии из роликов",
      "requirements": [
        "G44",
        "G45",
        "G53"
      ],
      "blockedBy": [
        "18"
      ],
      "wave": 13,
      "zone": [
        "core/src/git.rs",
        "core/src/engine/",
        "core/src/agent/",
        "core/src/prompt/",
        "core/src/swarm/",
        "core/src/model.rs",
        "app/src/gitcmds.rs",
        "core/tests/git_v4.rs",
        "core/tests/practices.rs"
      ],
      "review": "да - публикация наружу",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T02:43:56+03:00",
      "finishedAt": "2026-10-23T03:41:39+03:00",
      "retries": 0,
      "repairs": 2,
      "repairFindings": [
        "файл только в коммите слияния даёт protected_in_history",
        "revert_commit нестабилен: AdvanceBlocked main has uncommitted changes в ~1/3 прогонов"
      ],
      "tests": {
        "passed": 500,
        "failed": 0
      },
      "commit": "f7e5066"
    },
    {
      "id": "22",
      "title": "Интерфейс v4: вид терминала-райса, раскладка сплитами, палитра, настройки в два уровня",
      "requirements": [
        "G33",
        "G34",
        "G39",
        "G48",
        "G50",
        "G52",
        "G56",
        "G57",
        "G37",
        "G36"
      ],
      "blockedBy": [
        "18"
      ],
      "wave": 13,
      "zone": [
        "ui/js/app.js",
        "ui/js/shell.js",
        "ui/js/core/",
        "ui/js/screens/",
        "ui/css/tokens.css",
        "ui/css/base.css",
        "ui/css/components.css",
        "ui/css/shell.css",
        "ui/css/chat.css",
        "ui/css/tabs.css",
        "ui/css/live.css",
        "ui/css/settings.css",
        "ui/css/onboarding.css",
        "ui/css/need.css",
        "ui/css/lanes.css",
        "ui/css/swarm.css",
        "ui/css/factory.css",
        "ui/css/kits.css",
        "ui/css/notices.css",
        "ui/css/v4-shell.css",
        "ui/js/v4/theme.js",
        "ui/js/v4/panes.js",
        "ui/js/v4/palette.js",
        "ui/js/v4/settings-v4.js",
        "ui/js/v4/i18n-shell.js",
        "ui/i18n.js",
        "ui/i18n-v2.js",
        "ui/i18n-v3.js",
        "ui/logic.js",
        "ui/logic-v3.js",
        "ui/dev-mock.js",
        "ui/selftest.js",
        "scripts/ui-selftest.js",
        "CONTEXT.md"
      ],
      "review": "да - главный экран, все экраны перекрашиваются",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T02:43:56+03:00",
      "finishedAt": "2026-10-23T03:23:06+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 230,
        "failed": 0
      },
      "commit": "ea0ec02"
    },
    {
      "id": "23",
      "title": "Интерфейс v4: терминальные панели, среды как терминалы, окна, кастомизация терминала",
      "requirements": [
        "G35",
        "G36",
        "G37",
        "G38",
        "G49",
        "G54",
        "G55",
        "G58",
        "G51"
      ],
      "blockedBy": [
        "18",
        "22"
      ],
      "wave": 14,
      "zone": [
        "ui/js/v4/term.js",
        "ui/js/v4/term-settings.js",
        "ui/js/v4/i18n-term.js",
        "ui/js/v4/mock-term.js",
        "ui/css/v4-term.css",
        "docs/themes-sources.md"
      ],
      "review": "нет",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T03:19:23+03:00",
      "finishedAt": "2026-10-23T03:57:14+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 248,
        "failed": 0
      },
      "commit": "bebf0f5"
    },
    {
      "id": "24",
      "title": "Интерфейс v4: каталог скиллов/MCP/модов, публикация на GitHub, история коммитов",
      "requirements": [
        "G40",
        "G41",
        "G42",
        "G44",
        "G45"
      ],
      "blockedBy": [
        "18",
        "22"
      ],
      "wave": 14,
      "zone": [
        "ui/js/v4/market.js",
        "ui/js/v4/history.js",
        "ui/js/v4/publish.js",
        "ui/js/v4/i18n-market.js",
        "ui/js/v4/mock-market.js",
        "ui/css/v4-market.css"
      ],
      "review": "нет",
      "model": "обычная",
      "status": "done",
      "startedAt": "2026-10-23T03:41:40+03:00",
      "finishedAt": "2026-10-23T03:58:58+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 276,
        "failed": 1
      },
      "commit": "f9a7769"
    },
    {
      "id": "25",
      "title": "Интерфейс v4: Clawd 60 FPS, обновления и новые модели, приёмы экономии, рой на заглушке",
      "requirements": [
        "G46",
        "G47",
        "G43",
        "G53"
      ],
      "blockedBy": [
        "18",
        "22"
      ],
      "wave": 14,
      "zone": [
        "ui/js/v4/clawd.js",
        "ui/js/v4/updates.js",
        "ui/js/v4/practices.js",
        "ui/js/v4/i18n-clawd.js",
        "ui/js/v4/mock-clawd.js",
        "ui/css/v4-clawd.css",
        "ui/css/v4-updates.css",
        "README.md",
        "docs/clawd.md",
        "app/src/gitcmds.rs"
      ],
      "review": "нет",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T07:42:56+03:00",
      "finishedAt": "2026-10-23T07:57:53+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 501,
        "failed": 0
      },
      "commit": "ac9ff9b"
    },
    {
      "id": "26",
      "title": "Аудит безопасности v4 и живой прогон через computer use",
      "requirements": [
        "G59",
        "G04"
      ],
      "blockedBy": [
        "19",
        "20",
        "21",
        "23",
        "24",
        "25"
      ],
      "wave": 15,
      "zone": [
        "docs/security.md"
      ],
      "review": "нет",
      "model": "сильная",
      "status": "in-progress",
      "startedAt": "2026-10-23T07:57:58+03:00",
      "retries": 0,
      "repairs": 0
    },
    {
      "id": "27",
      "title": "Чат надёжный и живой: очередь на диске, «думает», плавный текст",
      "requirements": [
        "G62",
        "G80",
        "G81",
        "G84"
      ],
      "blockedBy": [],
      "wave": 30,
      "zone": [
        "core/src/chat/",
        "core/src/agent/mod.rs",
        "core/src/agent/adapters/claude.rs",
        "ui_hints",
        "core/tests/chat*.rs",
        "app/src/hub.rs",
        "ui/js/screens/chat.js",
        "ui/js/screens/composer.js",
        "ui/js/core/actions.js",
        "ui/js/core/reveal.js",
        "ui/css/live.css",
        "ui/js/v4/i18n-chat6.js",
        "app/src/main.rs",
        "ui/js/core/api.js",
        "ui/index.html",
        "ui/dev-mock.js",
        "scripts/ui-selftest.js"
      ],
      "review": "нет",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T17:43:05+03:00",
      "retries": 0,
      "repairs": 0,
      "finishedAt": "2026-10-23T18:52:46+03:00",
      "tests": {
        "passed": 600,
        "failed": 0
      },
      "commit": "736d376"
    },
    {
      "id": "28",
      "title": "Рой без общей паузы, расходы по дням, отчёт об ошибке, плашка обновления",
      "requirements": [
        "G64",
        "G69",
        "G70",
        "G71",
        "G71.1"
      ],
      "blockedBy": [],
      "wave": 30,
      "zone": [
        "core/src/swarm/",
        "core/src/engine/lanes.rs",
        "core/src/store.rs",
        "core/src/engine/runs.rs",
        "roles.rs",
        "core/src/diag.rs",
        "core/src/scrub.rs",
        "kits/scrub.rs",
        "core/src/net/",
        "core/tests/engine_swarm_lanes.rs",
        "app/src/diag.rs",
        "ui/js/v4/spend.js",
        "ui/js/v4/about.js",
        "ui/js/v4/i18n-ops6.js",
        "core/src/model.rs",
        "Settings.release_feed",
        "core/src/lib.rs",
        "app/src/main.rs",
        "ui/js/core/api.js",
        "ui/index.html",
        "ui/dev-mock.js",
        "scripts/ui-selftest.js"
      ],
      "review": "да — секреты (сборщик отчёта об ошибке)",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T17:43:05+03:00",
      "retries": 0,
      "repairs": 0,
      "finishedAt": "2026-10-23T18:55:01+03:00",
      "tests": {
        "passed": 600,
        "failed": 0
      },
      "commit": "736d376"
    },
    {
      "id": "29",
      "title": "Чат как оболочка CLI: «/», «В терминале» для всех сред, картинки",
      "requirements": [
        "G65",
        "G65.1",
        "G82",
        "G83",
        "G85",
        "G70"
      ],
      "blockedBy": [
        "27"
      ],
      "wave": 31,
      "zone": [
        "core/src/agent/",
        "inventory",
        "resume_argv",
        "supports_images",
        "core/src/chat/",
        "core/tests/chat_shell.rs",
        "app/src/",
        "ui/js/screens/composer.js",
        "ui/js/screens/chat.js",
        "ui/js/v4/slash.js",
        "ui/js/v4/i18n-shell6.js",
        "ui/css/chat.css",
        "app/src/main.rs",
        "ui/js/core/api.js",
        "ui/index.html",
        "ui/dev-mock.js",
        "scripts/ui-selftest.js",
        "ui/js/v4/term.js"
      ],
      "review": "нет",
      "model": "сильная",
      "status": "in-progress",
      "retries": 0,
      "repairs": 0,
      "startedAt": "2026-10-23T18:52:46+03:00"
    },
    {
      "id": "30",
      "title": "Задачи из GitHub Issues и сессии из терминала",
      "requirements": [
        "G67",
        "G67.1",
        "G68",
        "G68.1"
      ],
      "blockedBy": [],
      "wave": 30,
      "zone": [
        "core/src/suggest.rs",
        "core/src/sessions.rs",
        "core/src/ci.rs",
        "Gh",
        "core/src/engine/mod.rs",
        "resume_session",
        "core/tests/issues_sessions.rs",
        "app/src/auto.rs",
        "app/src/sessions.rs",
        "ui/js/v4/automation.js",
        "ui/js/v4/sessions.js",
        "ui/js/v4/i18n-issues6.js",
        "core/src/lib.rs",
        "core/src/model.rs",
        "app/src/main.rs",
        "ui/js/core/api.js",
        "ui/index.html",
        "ui/dev-mock.js",
        "scripts/ui-selftest.js"
      ],
      "review": "нет",
      "model": "обычная",
      "status": "done",
      "startedAt": "2026-10-23T17:43:05+03:00",
      "retries": 0,
      "repairs": 0,
      "finishedAt": "2026-10-23T18:52:46+03:00",
      "tests": {
        "passed": 600,
        "failed": 0
      },
      "commit": "736d376"
    },
    {
      "id": "31",
      "title": "Чистая программа для скачавших и чистая копия для публикации",
      "requirements": [
        "G76",
        "G77",
        "G77i",
        "G74",
        "G75.1",
        "G63"
      ],
      "blockedBy": [
        "27",
        "28",
        "29",
        "30"
      ],
      "wave": 32,
      "zone": [
        "ui/",
        "scripts/ui-selftest.js",
        "ui/selftest.*",
        "app/tauri.conf.json",
        "Cargo.toml",
        "[profile.release]",
        ".cargo/config.toml",
        "LICENSE",
        "scripts/check-clean-build.sh",
        "scripts/make-public-copy.sh",
        ".github/workflows/release.yml",
        "README.md",
        "README.ru.md",
        "core/src/engine/roles.rs",
        "core/src/chat/",
        "extra_env",
        "NIGHTSHIFT_PROJECT",
        "NIGHTSHIFT_PROJECT_NAME",
        "~/Dropbox/…/claude-shared/once/linux/",
        "claude-shared/claude/needs-sudo.linux.txt"
      ],
      "review": "да — утечка данных автора",
      "model": "сильная",
      "status": "pending",
      "retries": 0,
      "repairs": 0
    },
    {
      "id": "32",
      "title": "Живой прогон в режиме пользователя и полный аудит безопасности v6",
      "requirements": [
        "G60",
        "G75",
        "G78",
        "G79"
      ],
      "blockedBy": [
        "31"
      ],
      "wave": 33,
      "zone": [
        ".autopilot/vibe-env/live-v6.md",
        ".autopilot/vibe-env/research-v6.md",
        "docs/security.md"
      ],
      "review": "нет",
      "model": "сильная",
      "status": "pending",
      "retries": 0,
      "repairs": 0
    },
    {
      "id": "F1",
      "title": "Безопасность: чужой набор не запускает код и не уводит ключи, секреты не коммитятся",
      "requirements": [
        "G25",
        "G25.3",
        "G25.4",
        "G25.5",
        "R17i",
        "R17i.2"
      ],
      "blockedBy": [],
      "wave": 9,
      "zone": [
        "core/src/kits/",
        "core/src/git.rs",
        "core/src/engine/flow.rs",
        "core/src/agent/adapters/claude.rs",
        "core/tests/",
        "kits.rs",
        "git.rs",
        "engine_*",
        "ui/i18n-v2.js",
        "app/src/kitcmds.rs"
      ],
      "review": "да - ремонт ветки (аудит безопасности)",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-22T21:49:47+03:00",
      "finishedAt": "2026-10-22T22:38:56+03:00",
      "retries": 0,
      "repairs": 1,
      "repairFindings": [
        "toml_lite склеивает блоки через ''' в комментарии; метка ns-unprotected в .git, куда пишет агент"
      ],
      "tests": {
        "passed": 423,
        "failed": 0
      },
      "commit": "724f2c8"
    },
    {
      "id": "F10",
      "title": "Вид «Ночная радиостудия»: тёплые градиенты, кремовые шкалы, лампы «в эфире»",
      "requirements": [
        "G06",
        "23.10"
      ],
      "blockedBy": [
        "09"
      ],
      "wave": 20,
      "zone": [
        "ui/css/*.css",
        "ui/js/v4/theme.js",
        "ui/js/screens/chat.js",
        "ui/js/shell.js",
        "ui/js/screens/tabs.js",
        "ui/icons/",
        "scripts/ui-selftest.js",
        "ui/selftest.js"
      ],
      "review": "да (финальный ревьюер impeccable)",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T14:34:49+03:00",
      "finishedAt": "2026-10-23T15:40:00+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 315,
        "failed": 315
      },
      "commit": "423846e"
    },
    {
      "id": "F11",
      "title": "Проект выбирается явно: открыть папку из Finder или создать новый",
      "requirements": [
        "G07",
        "23.10"
      ],
      "blockedBy": [
        "10"
      ],
      "wave": 21,
      "zone": [
        "ui/js/core/projects.js",
        "ui/js/screens/chat.js",
        "ui/js/v4/term.js",
        "ui/js/v4/publish.js",
        "ui/js/v4/history.js",
        "ui/js/shell.js",
        "projects[0]",
        "app/src/proj.rs",
        "app/src/main.rs",
        "ui/js/core/api.js",
        "ui/dev-mock.js",
        "ui/css/chat.css",
        "app/",
        "scripts/ui-selftest.js",
        "ui/selftest.js"
      ],
      "review": "да",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T15:11:45+03:00",
      "finishedAt": "2026-10-23T15:32:55+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 512,
        "failed": 512
      },
      "commit": "a8598fd"
    },
    {
      "id": "F12",
      "title": "Чат: очередь сообщений и «Сразу», боковой вопрос, подсказка следующего шага, «Выполнить» у команды; ошибка остановленного запуска",
      "requirements": [],
      "blockedBy": [],
      "wave": null,
      "zone": [
        "ui/js/screens/chat.js",
        "ui/js/screens/composer.js",
        "ui/js/core/actions.js",
        "ui/css/chat.css",
        "ui/css/live.css",
        "ui/js/v4/sidechat.js",
        "ui/js/v4/i18n-chatx.js",
        "core/src/chat/",
        "app/src/main.rs"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-23T15:44:15+03:00",
      "finishedAt": "2026-10-23T16:35:04+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 551,
        "failed": 0
      },
      "commit": "38b0a7b"
    },
    {
      "id": "F13",
      "title": "Задача: откат на любой ход, чек-лист перед мержем, «Забрать к себе / Отдать агенту»",
      "requirements": [],
      "blockedBy": [],
      "wave": null,
      "zone": [
        "core/src/engine/",
        "core/src/git.rs",
        "core/src/model.rs",
        "core/src/store.rs",
        "app/src/main.rs",
        "ui/js/screens/taskcard.js",
        "ui/js/screens/chat.js",
        "ui/css/need.css",
        "ui/js/v4/i18n-task.js"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-23T15:44:15+03:00",
      "finishedAt": "2026-10-23T16:35:04+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 551,
        "failed": 0
      },
      "commit": "38b0a7b"
    },
    {
      "id": "F14",
      "title": "Автоматика: расписание задач, предложенные задачи, сторож красного CI",
      "requirements": [],
      "blockedBy": [],
      "wave": null,
      "zone": [
        "core/src/schedule/",
        "core/src/suggest/",
        "core/src/ci/",
        "app/src/",
        "presence.rs",
        "updates",
        "core/src/lib.rs",
        "model.rs",
        "app/src/main.rs",
        "ui/js/v4/automation.js",
        "ui/css/v4-automation.css",
        "ui/js/v4/i18n-auto.js",
        "ui/js/shell.js"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-23T15:44:15+03:00",
      "finishedAt": "2026-10-23T16:35:04+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 551,
        "failed": 0
      },
      "commit": "38b0a7b"
    },
    {
      "id": "F15",
      "title": "Доказательство работы и превью приложения с инспектором",
      "requirements": [],
      "blockedBy": [],
      "wave": null,
      "zone": [
        "core/src/engine/",
        "app/src/",
        "ui/js/v4/preview.js",
        "ui/css/v4-preview.css",
        "ui/js/screens/taskcard.js",
        "ui/js/v4/i18n-preview.js",
        "app/src/main.rs",
        "api.js",
        "index.html"
      ],
      "review": null,
      "model": null,
      "status": "done",
      "startedAt": "2026-10-23T16:35:26+03:00",
      "finishedAt": "2026-10-23T17:42:12+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 562,
        "failed": 0
      },
      "commit": "d5219d5"
    },
    {
      "id": "F2",
      "title": "Ядро и приложение: секреты в ленте до конца, пути проекта, служебные инструменты, меню и ⌘W",
      "requirements": [
        "G30",
        "G30.3",
        "G31",
        "G31.4",
        "G32"
      ],
      "blockedBy": [],
      "wave": 11,
      "zone": [
        "core/src/agent/",
        "actions.rs",
        "adapters/*.rs",
        "common.rs",
        "core/src/engine/",
        "edits.rs",
        "app/src/main.rs",
        "core/tests/",
        "app/"
      ],
      "review": "да - путь секретов в окно",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T00:37:59+03:00",
      "finishedAt": "2026-10-23T10:41:52+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 511,
        "failed": 0
      },
      "commit": "4040785"
    },
    {
      "id": "F3",
      "title": "Интерфейс: лента без дублей, склейка чтений вживую, клавиши вкладок, мелочи живого прогона",
      "requirements": [
        "G29",
        "G30",
        "G30.1",
        "G31",
        "G31.1",
        "G31.3",
        "G32"
      ],
      "blockedBy": [],
      "wave": 11,
      "zone": [
        "ui/"
      ],
      "review": "да - главный экран",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T00:37:59+03:00",
      "finishedAt": "2026-10-23T00:58:42+03:00",
      "retries": 0,
      "repairs": 1,
      "repairFindings": [
        "вживую в .app после F3 не работают ⌘T и ⌘W (до F3 ⌘T работал); ⌘D не работал и до"
      ],
      "tests": {
        "passed": 202,
        "failed": 0
      },
      "commit": "3db28ec"
    },
    {
      "id": "F4",
      "title": "Безопасность v4: подтверждённые находки аудита, docs/security.md, лимит кнопок",
      "requirements": [
        "G04",
        "G46"
      ],
      "blockedBy": [
        "25"
      ],
      "wave": 16,
      "zone": [
        "core/src/models/radar.rs",
        "core/src/agent/adapters/",
        "-m",
        "--model",
        "core/src/kits/units.rs",
        "app/src/ptycmds.rs",
        "core/src/term/mod.rs",
        "core/src/git.rs",
        "log_page",
        "scripts/ui-selftest.js",
        "docs/security.md",
        "core/tests/",
        "app/"
      ],
      "review": "да - путь недоверенных данных из каталогов",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T08:05:06+03:00",
      "finishedAt": "2026-10-23T09:51:34+03:00",
      "retries": 0,
      "repairs": 1,
      "tests": {
        "passed": 509,
        "failed": 0
      },
      "commit": "bdd02bf"
    },
    {
      "id": "F5",
      "title": "Живой прогон v4: палитра, закрытие панели, вынесенное окно, выход",
      "requirements": [
        "G48.1",
        "G35.3",
        "G38",
        "G58.2",
        "G59"
      ],
      "blockedBy": [
        "04"
      ],
      "wave": 16,
      "zone": [
        "ui/js/v4/",
        "ui/js/shell.js",
        "app/src/main.rs",
        "quit",
        "app/tauri.conf.json",
        "app/Info.plist",
        "ui/selftest.js",
        "app/"
      ],
      "review": "нет",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T09:56:07+03:00",
      "finishedAt": "2026-10-23T10:11:17+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 511,
        "failed": 0
      },
      "commit": "d4fb5b1"
    },
    {
      "id": "F6",
      "title": "Слепая приёмка v4: gitleaks без ложных срабатываний, честные лимиты на dev-mock",
      "requirements": [
        "G04",
        "G26"
      ],
      "blockedBy": [
        "05"
      ],
      "wave": 17,
      "zone": [
        ".gitleaks.toml",
        "ui/dev-mock.js",
        "ui/js/v4/mock-*.js",
        "docs/security.md",
        "ui/selftest.js"
      ],
      "review": "нет",
      "model": "обычная",
      "status": "done",
      "startedAt": "2026-10-23T10:24:06+03:00",
      "finishedAt": "2026-10-23T10:41:40+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 511,
        "failed": 0
      },
      "commit": "7ae7103"
    },
    {
      "id": "F7",
      "title": "Выбор модели короткой менюшкой, терминал красивее",
      "requirements": [
        "G35",
        "G36"
      ],
      "blockedBy": [
        "05"
      ],
      "wave": 17,
      "zone": [
        "ui/js/screens/modelpop.js",
        "ui/js/screens/composer.js",
        "ui/js/v4/palette.js",
        "ui/js/v4/term.js",
        "ui/js/v4/term-settings.js",
        "ui/js/v4/theme.js",
        "ui/css/chat.css",
        "ui/css/v4-shell.css",
        "ui/css/v4-term.css",
        "ui/js/v4/i18n-*.js",
        "ui/i18n*.js",
        "scripts/ui-selftest.js",
        "ui/selftest.js",
        "ui/dev-mock.js"
      ],
      "review": "да",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T11:48:13+03:00",
      "finishedAt": "2026-10-23T12:01:57+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 310,
        "failed": 310
      },
      "commit": "feda8fc"
    },
    {
      "id": "F8",
      "title": "Понятность: видимые переключатели, вход в настройки и фабрику, чат и терминал различимы",
      "requirements": [
        "G19",
        "G35",
        "246",
        "23.10"
      ],
      "blockedBy": [
        "07"
      ],
      "wave": 18,
      "zone": [
        "ui/js/screens/composer.js",
        "ui/js/shell.js",
        "ui/js/v4/tabs*.js",
        "panes.js",
        "ui/js/v4/term.js",
        "ui/css/v4-shell.css",
        "ui/css/v4-term.css",
        "ui/css/chat.css",
        "ui/selftest.js",
        "scripts/ui-selftest.js",
        "ui/dev-mock.js"
      ],
      "review": "да",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T12:39:28+03:00",
      "finishedAt": "2026-10-23T13:09:57+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 310,
        "failed": 310
      },
      "commit": "3f45eb4"
    },
    {
      "id": "F9",
      "title": "Главный вид: минимальный красивый чат",
      "requirements": [
        "G19",
        "G05",
        "G21",
        "246",
        "23.10"
      ],
      "blockedBy": [
        "08"
      ],
      "wave": 19,
      "zone": [
        "ui/js/screens/chat.js",
        "ui/js/screens/composer.js",
        "ui/js/shell.js",
        "ui/css/chat.css",
        "ui/css/v4-shell.css",
        "ui/css/shell.css",
        ".side",
        "ui/css/tokens.css",
        "ui/dev-mock.js",
        "ui/selftest.js",
        "scripts/ui-selftest.js"
      ],
      "review": "да",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-23T13:55:32+03:00",
      "finishedAt": "2026-10-23T14:33:46+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 310,
        "failed": 310
      },
      "commit": "be03fb4"
    },
    {
      "id": "_v5",
      "title": "# Общее для F12-F15 (фишки v5, просьба 23.10: «Делай все фишки из таблицы»)",
      "requirements": [],
      "blockedBy": [],
      "wave": null,
      "zone": [],
      "review": null,
      "model": null,
      "status": "pending",
      "retries": 0,
      "repairs": 0
    }
  ],
  "singlePass": null,
  "tests": {
    "passed": 600,
    "failed": 0
  },
  "debt": {
    "placeholders": [
      "Подпись macOS - нужен Apple Developer"
    ],
    "assumptions": [
      "Название «Nightshift» (рабочее)",
      "Лицензия MIT",
      "Второй живой движок - opencode (Codex не установлен)",
      "Запрет переменных наборов классами имён: JAVA_HOME, GOPATH, NODE_AUTH_TOKEN и любые *_HOME/*_PATH из набора не задать"
    ],
    "emptyEnv": []
  },
  "additions": [
    "A01 продолжение после лимита - ради R05",
    "A04 время и попытки в карточке - ради R03",
    "A05 запасной движок при лимите - ради R05",
    "A06 «Пока тебя не было» - сводка при открытии - ради G15",
    "A07 «Продолжить в терминале» - та же сессия в CLI - ради G17",
    "A08 ночная очередь - старт, когда тебя нет или сбросился лимит - ради R05"
  ],
  "coverage": {
    "found": 6,
    "fixed": 6,
    "deferred": 0,
    "items": []
  },
  "blind": {
    "checked": 43,
    "matched": 40,
    "mismatches": [
      "G04: gitleaks по истории даёт 91 ложное срабатывание, файла-исключения нет - «gitleaks clean» не выполнен",
      "G26: dev-mock рисует у Gemini «День 85%», README говорит, что настоящие лимиты есть только у Claude Code",
      "G40: в каталоге нет живого источника модов, моды ставятся только «По ссылке»"
    ]
  },
  "rev": 248,
  "beats": [
    "2026-10-22T19:03:58+03:00",
    "2026-10-22T19:04:35+03:00",
    "2026-10-22T19:06:29+03:00",
    "2026-10-22T20:53:31+03:00",
    "2026-10-22T21:05:18+03:00",
    "2026-10-22T21:18:04+03:00",
    "2026-10-22T21:19:09+03:00",
    "2026-10-22T21:19:20+03:00",
    "2026-10-22T21:19:33+03:00",
    "2026-10-22T21:34:21+03:00",
    "2026-10-22T21:34:53+03:00",
    "2026-10-22T21:36:22+03:00",
    "2026-10-22T21:37:13+03:00",
    "2026-10-22T21:37:41+03:00",
    "2026-10-22T21:49:21+03:00",
    "2026-10-22T21:49:40+03:00",
    "2026-10-22T21:49:46+03:00",
    "2026-10-22T21:49:47+03:00",
    "2026-10-22T22:04:21+03:00",
    "2026-10-22T22:19:21+03:00",
    "2026-10-22T22:20:04+03:00",
    "2026-10-22T22:20:05+03:00",
    "2026-10-22T22:24:54+03:00",
    "2026-10-22T22:24:55+03:00",
    "2026-10-22T22:34:22+03:00",
    "2026-10-22T22:38:56+03:00",
    "2026-10-22T22:42:54+03:00",
    "2026-10-22T22:48:42+03:00",
    "2026-10-22T22:49:22+03:00",
    "2026-10-22T23:02:17+03:00",
    "2026-10-22T23:02:31+03:00",
    "2026-10-22T23:02:40+03:00",
    "2026-10-22T23:02:47+03:00",
    "2026-10-22T23:04:22+03:00",
    "2026-10-22T23:19:22+03:00",
    "2026-10-22T23:34:23+03:00",
    "2026-10-22T23:49:23+03:00",
    "2026-10-23T00:04:23+03:00",
    "2026-10-23T00:13:43+03:00",
    "2026-10-23T00:15:58+03:00",
    "2026-10-23T00:17:15+03:00",
    "2026-10-23T00:17:16+03:00",
    "2026-10-23T00:19:23+03:00",
    "2026-10-23T00:21:27+03:00",
    "2026-10-23T00:23:39+03:00",
    "2026-10-23T00:29:46+03:00",
    "2026-10-23T00:34:23+03:00",
    "2026-10-23T00:37:59+03:00",
    "2026-10-23T00:49:04+03:00",
    "2026-10-23T00:49:23+03:00",
    "2026-10-23T00:51:44+03:00",
    "2026-10-23T00:51:45+03:00",
    "2026-10-23T00:51:58+03:00",
    "2026-10-23T00:54:13+03:00",
    "2026-10-23T00:58:42+03:00",
    "2026-10-23T00:59:59+03:00",
    "2026-10-23T01:04:23+03:00",
    "2026-10-23T01:14:43+03:00",
    "2026-10-23T02:00:30+03:00",
    "2026-10-23T02:05:34+03:00",
    "2026-10-23T02:05:40+03:00",
    "2026-10-23T02:05:46+03:00",
    "2026-10-23T02:05:54+03:00",
    "2026-10-23T02:05:55+03:00",
    "2026-10-23T02:06:02+03:00",
    "2026-10-23T02:43:56+03:00",
    "2026-10-23T02:46:01+03:00",
    "2026-10-23T02:46:08+03:00",
    "2026-10-23T02:52:59+03:00",
    "2026-10-23T03:18:40+03:00",
    "2026-10-23T03:19:23+03:00",
    "2026-10-23T03:19:36+03:00",
    "2026-10-23T03:22:41+03:00",
    "2026-10-23T03:22:42+03:00",
    "2026-10-23T03:23:06+03:00",
    "2026-10-23T03:25:56+03:00",
    "2026-10-23T03:25:57+03:00",
    "2026-10-23T03:27:33+03:00",
    "2026-10-23T03:28:17+03:00",
    "2026-10-23T03:28:26+03:00",
    "2026-10-23T03:41:39+03:00",
    "2026-10-23T03:41:40+03:00",
    "2026-10-23T03:46:43+03:00",
    "2026-10-23T03:57:14+03:00",
    "2026-10-23T03:58:58+03:00",
    "2026-10-23T07:42:56+03:00",
    "2026-10-23T07:57:53+03:00",
    "2026-10-23T07:57:58+03:00",
    "2026-10-23T08:05:06+03:00",
    "2026-10-23T08:20:39+03:00",
    "2026-10-23T08:20:45+03:00",
    "2026-10-23T09:42:58+03:00",
    "2026-10-23T09:51:34+03:00",
    "2026-10-23T09:56:07+03:00",
    "2026-10-23T10:11:17+03:00",
    "2026-10-23T10:16:17+03:00",
    "2026-10-23T10:24:06+03:00",
    "2026-10-23T10:41:40+03:00",
    "2026-10-23T10:41:52+03:00",
    "2026-10-23T11:48:13+03:00",
    "2026-10-23T12:01:57+03:00",
    "2026-10-23T12:39:28+03:00",
    "2026-10-23T13:09:57+03:00",
    "2026-10-23T13:55:32+03:00",
    "2026-10-23T14:33:46+03:00",
    "2026-10-23T14:34:49+03:00",
    "2026-10-23T15:11:45+03:00",
    "2026-10-23T15:32:55+03:00",
    "2026-10-23T15:40:00+03:00",
    "2026-10-23T15:44:15+03:00",
    "2026-10-23T16:29:09+03:00",
    "2026-10-23T16:35:04+03:00",
    "2026-10-23T16:35:26+03:00",
    "2026-10-23T16:36:18+03:00",
    "2026-10-23T16:37:26+03:00",
    "2026-10-23T16:39:14+03:00",
    "2026-10-23T16:40:39+03:00",
    "2026-10-23T16:41:37+03:00",
    "2026-10-23T17:01:48+03:00",
    "2026-10-23T17:42:12+03:00",
    "2026-10-23T17:42:35+03:00",
    "2026-10-23T17:42:36+03:00",
    "2026-10-23T17:42:48+03:00",
    "2026-10-23T17:42:49+03:00",
    "2026-10-23T17:42:57+03:00",
    "2026-10-23T17:43:05+03:00",
    "2026-10-23T18:43:52+03:00",
    "2026-10-23T18:44:24+03:00",
    "2026-10-23T18:46:09+03:00",
    "2026-10-23T18:52:46+03:00",
    "2026-10-23T18:55:01+03:00"
  ],
  "log": [
    {
      "t": "2026-10-22T19:04:35+03:00",
      "k": "tests",
      "passed": 0,
      "failed": 0
    },
    {
      "t": "2026-10-22T19:06:29+03:00",
      "k": "tests",
      "passed": 407,
      "failed": 0
    },
    {
      "t": "2026-10-22T21:18:04+03:00",
      "k": "tests",
      "passed": 408,
      "failed": 0
    },
    {
      "t": "2026-10-22T21:19:09+03:00",
      "k": "add",
      "key": "concerns",
      "n": 6
    },
    {
      "t": "2026-10-22T21:19:33+03:00",
      "k": "add",
      "key": "concerns",
      "n": 3
    },
    {
      "t": "2026-10-22T21:34:53+03:00",
      "k": "add",
      "key": "debt.assumptions",
      "n": 1
    },
    {
      "t": "2026-10-22T21:36:22+03:00",
      "k": "tests",
      "passed": 414,
      "failed": 0
    },
    {
      "t": "2026-10-22T21:37:13+03:00",
      "k": "add",
      "key": "concerns",
      "n": 2
    },
    {
      "t": "2026-10-22T21:49:40+03:00",
      "k": "tickets",
      "n": 14
    },
    {
      "t": "2026-10-22T21:49:40+03:00",
      "k": "add",
      "key": "report",
      "n": 2
    },
    {
      "t": "2026-10-22T21:49:46+03:00",
      "k": "tickets",
      "n": 16
    },
    {
      "t": "2026-10-22T21:49:47+03:00",
      "k": "ticket",
      "ids": [
        "F1"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-22T22:20:04+03:00",
      "k": "ticket",
      "ids": [
        "F1"
      ],
      "act": "done",
      "tests": {
        "passed": 420,
        "failed": 0
      }
    },
    {
      "t": "2026-10-22T22:20:05+03:00",
      "k": "ticket",
      "ids": [
        "F1"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-22T22:24:54+03:00",
      "k": "ticket",
      "ids": [
        "F1"
      ],
      "act": "repair",
      "note": "toml_lite склеивает блоки через ''' в комментарии; метка ns-unprotected в .git, куда пишет агент"
    },
    {
      "t": "2026-10-22T22:24:55+03:00",
      "k": "add",
      "key": "concerns",
      "n": 3
    },
    {
      "t": "2026-10-22T22:38:56+03:00",
      "k": "ticket",
      "ids": [
        "F1"
      ],
      "act": "done",
      "tests": {
        "passed": 423,
        "failed": 0
      }
    },
    {
      "t": "2026-10-22T22:42:54+03:00",
      "k": "add",
      "key": "report",
      "n": 3
    },
    {
      "t": "2026-10-22T23:02:17+03:00",
      "k": "gate",
      "g": "G2",
      "ok": true,
      "found": 2,
      "fixed": 2,
      "deferred": 0
    },
    {
      "t": "2026-10-22T23:02:17+03:00",
      "k": "tickets",
      "n": 18
    },
    {
      "t": "2026-10-22T23:02:17+03:00",
      "k": "gate",
      "g": "G3",
      "ok": false,
      "problems": 16
    },
    {
      "t": "2026-10-22T23:02:31+03:00",
      "k": "tickets",
      "n": 18
    },
    {
      "t": "2026-10-22T23:02:31+03:00",
      "k": "gate",
      "g": "G3",
      "ok": false,
      "problems": 14
    },
    {
      "t": "2026-10-22T23:02:40+03:00",
      "k": "tickets",
      "n": 18
    },
    {
      "t": "2026-10-22T23:02:40+03:00",
      "k": "gate",
      "g": "G3",
      "ok": false,
      "problems": 13
    },
    {
      "t": "2026-10-22T23:02:47+03:00",
      "k": "ticket",
      "ids": [
        "16",
        "17"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T00:13:43+03:00",
      "k": "ticket",
      "ids": [
        "16"
      ],
      "act": "done",
      "tests": {
        "passed": 433,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T00:13:43+03:00",
      "k": "ticket",
      "ids": [
        "17"
      ],
      "act": "done",
      "tests": {
        "passed": 195,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T00:13:43+03:00",
      "k": "ticket",
      "ids": [
        "16",
        "17"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-23T00:15:58+03:00",
      "k": "ticket",
      "ids": [
        "16"
      ],
      "act": "repair",
      "note": "обрезка диффа до scrub; Edit/Write секретного файла отдаёт дифф/head целиком; вложенные .env не в scrub"
    },
    {
      "t": "2026-10-23T00:15:58+03:00",
      "k": "add",
      "key": "concerns",
      "n": 2
    },
    {
      "t": "2026-10-23T00:17:15+03:00",
      "k": "ticket",
      "ids": [
        "17"
      ],
      "act": "done",
      "tests": {
        "passed": 195,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T00:17:16+03:00",
      "k": "add",
      "key": "concerns",
      "n": 5
    },
    {
      "t": "2026-10-23T00:21:27+03:00",
      "k": "ticket",
      "ids": [
        "16"
      ],
      "act": "done",
      "tests": {
        "passed": 435,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T00:23:39+03:00",
      "k": "add",
      "key": "concerns",
      "n": 5
    },
    {
      "t": "2026-10-23T00:29:46+03:00",
      "k": "tests",
      "passed": 437,
      "failed": 0
    },
    {
      "t": "2026-10-23T00:37:59+03:00",
      "k": "tickets",
      "n": 20
    },
    {
      "t": "2026-10-23T00:37:59+03:00",
      "k": "gate",
      "g": "G3",
      "ok": false,
      "problems": 13
    },
    {
      "t": "2026-10-23T00:37:59+03:00",
      "k": "ticket",
      "ids": [
        "F2",
        "F3"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T00:49:04+03:00",
      "k": "ticket",
      "ids": [
        "F3"
      ],
      "act": "done",
      "tests": {
        "passed": 200,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T00:49:04+03:00",
      "k": "ticket",
      "ids": [
        "F3"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-23T00:51:44+03:00",
      "k": "ticket",
      "ids": [
        "F3"
      ],
      "act": "done",
      "tests": {
        "passed": 200,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T00:51:45+03:00",
      "k": "add",
      "key": "report",
      "n": 3
    },
    {
      "t": "2026-10-23T00:51:45+03:00",
      "k": "add",
      "key": "concerns",
      "n": 2
    },
    {
      "t": "2026-10-23T00:51:58+03:00",
      "k": "ticket",
      "ids": [
        "F2"
      ],
      "act": "done",
      "tests": {
        "passed": 444,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T00:51:58+03:00",
      "k": "ticket",
      "ids": [
        "F2"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-23T00:51:58+03:00",
      "k": "tests",
      "passed": 444,
      "failed": 0
    },
    {
      "t": "2026-10-23T00:54:13+03:00",
      "k": "ticket",
      "ids": [
        "F3"
      ],
      "act": "repair",
      "note": "вживую в .app после F3 не работают ⌘T и ⌘W (до F3 ⌘T работал); ⌘D не работал и до"
    },
    {
      "t": "2026-10-23T00:54:13+03:00",
      "k": "add",
      "key": "report",
      "n": 1
    },
    {
      "t": "2026-10-23T00:58:42+03:00",
      "k": "ticket",
      "ids": [
        "F3"
      ],
      "act": "done",
      "tests": {
        "passed": 202,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T00:59:59+03:00",
      "k": "stage",
      "id": "final",
      "act": "start"
    },
    {
      "t": "2026-10-23T01:14:43+03:00",
      "k": "stage",
      "id": "briefing",
      "act": "skip",
      "note": "v4 (бриф 23.10): вопросы отклонены пользователем («продолжи Автопилот») — развилки решены как ASSUMPTION в манифесте"
    },
    {
      "t": "2026-10-23T01:14:43+03:00",
      "k": "gate",
      "g": "G1",
      "ok": false,
      "open": 18,
      "total": 71
    },
    {
      "t": "2026-10-23T01:14:43+03:00",
      "k": "stage",
      "id": "spec",
      "act": "start"
    },
    {
      "t": "2026-10-23T02:00:30+03:00",
      "k": "gate",
      "g": "G2",
      "ok": false,
      "found": 7,
      "fixed": 5,
      "deferred": 0
    },
    {
      "t": "2026-10-23T02:00:30+03:00",
      "k": "gate",
      "g": "G1",
      "ok": true,
      "open": 0,
      "total": 80
    },
    {
      "t": "2026-10-23T02:00:30+03:00",
      "k": "stage",
      "id": "spec",
      "act": "done"
    },
    {
      "t": "2026-10-23T02:00:30+03:00",
      "k": "stage",
      "id": "plan",
      "act": "start"
    },
    {
      "t": "2026-10-23T02:05:34+03:00",
      "k": "tickets",
      "n": 29
    },
    {
      "t": "2026-10-23T02:05:34+03:00",
      "k": "gate",
      "g": "G3",
      "ok": false,
      "problems": 34
    },
    {
      "t": "2026-10-23T02:05:40+03:00",
      "k": "tickets",
      "n": 29
    },
    {
      "t": "2026-10-23T02:05:40+03:00",
      "k": "gate",
      "g": "G3",
      "ok": false,
      "problems": 13
    },
    {
      "t": "2026-10-23T02:05:46+03:00",
      "k": "gate",
      "g": "G3",
      "ok": false,
      "problems": 13
    },
    {
      "t": "2026-10-23T02:05:54+03:00",
      "k": "tier",
      "v": "T3"
    },
    {
      "t": "2026-10-23T02:05:55+03:00",
      "k": "stage",
      "id": "plan",
      "act": "done"
    },
    {
      "t": "2026-10-23T02:06:02+03:00",
      "k": "stage",
      "id": "build",
      "act": "start"
    },
    {
      "t": "2026-10-23T02:06:02+03:00",
      "k": "ticket",
      "ids": [
        "18"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T02:43:56+03:00",
      "k": "ticket",
      "ids": [
        "18"
      ],
      "act": "done",
      "tests": {
        "passed": 452,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T02:43:56+03:00",
      "k": "ticket",
      "ids": [
        "18"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-23T02:43:56+03:00",
      "k": "ticket",
      "ids": [
        "22",
        "19",
        "21"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T02:46:01+03:00",
      "k": "ticket",
      "ids": [
        "18"
      ],
      "act": "repair"
    },
    {
      "t": "2026-10-23T02:46:08+03:00",
      "k": "ticket",
      "ids": [
        "18"
      ],
      "act": "repair",
      "note": "выход процесса PTY при открытом окне не вешает приложение"
    },
    {
      "t": "2026-10-23T02:46:08+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T02:46:08+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T02:46:08+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T02:46:08+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T02:52:59+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T02:52:59+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T02:52:59+03:00",
      "k": "ticket",
      "ids": [
        "18"
      ],
      "act": "done",
      "tests": {
        "passed": 453,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T03:18:40+03:00",
      "k": "ticket",
      "ids": [
        "21"
      ],
      "act": "done",
      "tests": {
        "passed": 485,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T03:18:40+03:00",
      "k": "ticket",
      "ids": [
        "21"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-23T03:18:40+03:00",
      "k": "ticket",
      "ids": [
        "20"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T03:18:40+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:18:40+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:18:40+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:18:40+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:19:23+03:00",
      "k": "ticket",
      "ids": [
        "22"
      ],
      "act": "done",
      "tests": {
        "passed": 230,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T03:19:23+03:00",
      "k": "ticket",
      "ids": [
        "22"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-23T03:19:23+03:00",
      "k": "ticket",
      "ids": [
        "23"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T03:19:36+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:19:36+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:22:41+03:00",
      "k": "ticket",
      "ids": [
        "19"
      ],
      "act": "done",
      "tests": {
        "passed": 485,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T03:22:41+03:00",
      "k": "ticket",
      "ids": [
        "19"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-23T03:22:41+03:00",
      "k": "ticket",
      "ids": [
        "21"
      ],
      "act": "repair",
      "note": "файл только в коммите слияния даёт protected_in_history"
    },
    {
      "t": "2026-10-23T03:22:41+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:22:41+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:22:42+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:22:42+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:22:42+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:22:42+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:23:06+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:23:06+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:23:06+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:23:06+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:23:06+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:23:06+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:23:06+03:00",
      "k": "ticket",
      "ids": [
        "22"
      ],
      "act": "done",
      "tests": {
        "passed": 230,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T03:25:56+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:25:57+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:25:57+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:25:57+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:25:57+03:00",
      "k": "ticket",
      "ids": [
        "19"
      ],
      "act": "done",
      "tests": {
        "passed": 485,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T03:27:33+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:27:33+03:00",
      "k": "ticket",
      "ids": [
        "24"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T03:28:17+03:00",
      "k": "ticket",
      "ids": [
        "21"
      ],
      "act": "repair",
      "note": "revert_commit нестабилен: AdvanceBlocked main has uncommitted changes в ~1/3 прогонов"
    },
    {
      "t": "2026-10-23T03:28:26+03:00",
      "k": "ticket",
      "ids": [
        "24"
      ],
      "act": "reset"
    },
    {
      "t": "2026-10-23T03:41:39+03:00",
      "k": "ticket",
      "ids": [
        "21"
      ],
      "act": "done",
      "tests": {
        "passed": 500,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T03:41:39+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:41:40+03:00",
      "k": "ticket",
      "ids": [
        "24"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T03:46:43+03:00",
      "k": "ticket",
      "ids": [
        "20"
      ],
      "act": "done",
      "tests": {
        "passed": 500,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T03:46:43+03:00",
      "k": "ticket",
      "ids": [
        "25"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T03:46:43+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:46:43+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:46:43+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:46:43+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:57:14+03:00",
      "k": "ticket",
      "ids": [
        "23"
      ],
      "act": "done",
      "tests": {
        "passed": 248,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T03:57:14+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:57:14+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:57:14+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:57:14+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T03:58:58+03:00",
      "k": "ticket",
      "ids": [
        "24"
      ],
      "act": "done",
      "tests": {
        "passed": 276,
        "failed": 1
      }
    },
    {
      "t": "2026-10-23T03:58:58+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T07:42:56+03:00",
      "k": "ticket",
      "ids": [
        "25"
      ],
      "act": "reset"
    },
    {
      "t": "2026-10-23T07:42:56+03:00",
      "k": "ticket",
      "ids": [
        "25"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T07:57:53+03:00",
      "k": "ticket",
      "ids": [
        "25"
      ],
      "act": "done",
      "tests": {
        "passed": 501,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T07:57:53+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T07:57:58+03:00",
      "k": "ticket",
      "ids": [
        "26"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T08:05:06+03:00",
      "k": "tickets",
      "n": 30
    },
    {
      "t": "2026-10-23T08:05:06+03:00",
      "k": "ticket",
      "ids": [
        "F4"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T08:20:39+03:00",
      "k": "ticket",
      "ids": [
        "F4"
      ],
      "act": "done",
      "tests": {
        "passed": 506,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T08:20:39+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-23T08:20:45+03:00",
      "k": "ticket",
      "ids": [
        "F4"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-23T09:42:58+03:00",
      "k": "ticket",
      "ids": [
        "F4"
      ],
      "act": "repair"
    },
    {
      "t": "2026-10-23T09:51:34+03:00",
      "k": "ticket",
      "ids": [
        "F4"
      ],
      "act": "done",
      "tests": {
        "passed": 509,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T09:56:07+03:00",
      "k": "tickets",
      "n": 31
    },
    {
      "t": "2026-10-23T09:56:07+03:00",
      "k": "ticket",
      "ids": [
        "F5"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T10:11:17+03:00",
      "k": "ticket",
      "ids": [
        "F5"
      ],
      "act": "done",
      "tests": {
        "passed": 511,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T10:16:17+03:00",
      "k": "add",
      "key": "report",
      "n": 1
    },
    {
      "t": "2026-10-23T10:24:06+03:00",
      "k": "gate",
      "g": "G4",
      "ok": false,
      "checked": 43,
      "matched": 40
    },
    {
      "t": "2026-10-23T10:24:06+03:00",
      "k": "add",
      "key": "report",
      "n": 1
    },
    {
      "t": "2026-10-23T10:24:06+03:00",
      "k": "tickets",
      "n": 32
    },
    {
      "t": "2026-10-23T10:24:06+03:00",
      "k": "ticket",
      "ids": [
        "F6"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T10:41:40+03:00",
      "k": "ticket",
      "ids": [
        "F6"
      ],
      "act": "done",
      "tests": {
        "passed": 511,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T10:41:40+03:00",
      "k": "tests",
      "passed": 511,
      "failed": 0
    },
    {
      "t": "2026-10-23T10:41:52+03:00",
      "k": "ticket",
      "ids": [
        "F2"
      ],
      "act": "done",
      "tests": {
        "passed": 511,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T11:48:13+03:00",
      "k": "tickets",
      "n": 33
    },
    {
      "t": "2026-10-23T11:48:13+03:00",
      "k": "ticket",
      "ids": [
        "F7"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T12:01:57+03:00",
      "k": "ticket",
      "ids": [
        "F7"
      ],
      "act": "done",
      "tests": {
        "passed": 310,
        "failed": 310
      }
    },
    {
      "t": "2026-10-23T12:39:28+03:00",
      "k": "tickets",
      "n": 34
    },
    {
      "t": "2026-10-23T12:39:28+03:00",
      "k": "ticket",
      "ids": [
        "F8"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T13:09:57+03:00",
      "k": "ticket",
      "ids": [
        "F8"
      ],
      "act": "done",
      "tests": {
        "passed": 310,
        "failed": 310
      }
    },
    {
      "t": "2026-10-23T13:55:32+03:00",
      "k": "tickets",
      "n": 35
    },
    {
      "t": "2026-10-23T13:55:32+03:00",
      "k": "ticket",
      "ids": [
        "F9"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T14:33:46+03:00",
      "k": "ticket",
      "ids": [
        "F9"
      ],
      "act": "done",
      "tests": {
        "passed": 310,
        "failed": 310
      }
    },
    {
      "t": "2026-10-23T14:34:49+03:00",
      "k": "tickets",
      "n": 36
    },
    {
      "t": "2026-10-23T14:34:49+03:00",
      "k": "ticket",
      "ids": [
        "F10"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T15:11:45+03:00",
      "k": "tickets",
      "n": 37
    },
    {
      "t": "2026-10-23T15:11:45+03:00",
      "k": "ticket",
      "ids": [
        "F11"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T15:32:55+03:00",
      "k": "ticket",
      "ids": [
        "F11"
      ],
      "act": "done",
      "tests": {
        "passed": 512,
        "failed": 512
      }
    },
    {
      "t": "2026-10-23T15:40:00+03:00",
      "k": "ticket",
      "ids": [
        "F10"
      ],
      "act": "done",
      "tests": {
        "passed": 315,
        "failed": 315
      }
    },
    {
      "t": "2026-10-23T15:44:15+03:00",
      "k": "tickets",
      "n": 42
    },
    {
      "t": "2026-10-23T15:44:15+03:00",
      "k": "ticket",
      "ids": [
        "F12"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T15:44:15+03:00",
      "k": "ticket",
      "ids": [
        "F13"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T15:44:15+03:00",
      "k": "ticket",
      "ids": [
        "F14"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T16:35:04+03:00",
      "k": "ticket",
      "ids": [
        "F12",
        "F13",
        "F14"
      ],
      "act": "done",
      "tests": {
        "passed": 551,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T16:35:26+03:00",
      "k": "ticket",
      "ids": [
        "F15"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T16:36:18+03:00",
      "k": "stage",
      "id": "manifest",
      "act": "start"
    },
    {
      "t": "2026-10-23T16:37:26+03:00",
      "k": "ask",
      "kind": "briefing",
      "note": "три вопроса - что публиковать, какие CLI проверять, вид «думает»"
    },
    {
      "t": "2026-10-23T16:39:14+03:00",
      "k": "answered",
      "kind": "briefing"
    },
    {
      "t": "2026-10-23T16:40:39+03:00",
      "k": "ask",
      "kind": "briefing",
      "note": "один вопрос - чат поверх терминала и моды"
    },
    {
      "t": "2026-10-23T16:41:37+03:00",
      "k": "answered",
      "kind": "briefing"
    },
    {
      "t": "2026-10-23T17:01:48+03:00",
      "k": "gate",
      "g": "G2",
      "ok": true,
      "found": 6,
      "fixed": 6,
      "deferred": 0
    },
    {
      "t": "2026-10-23T17:42:12+03:00",
      "k": "ticket",
      "ids": [
        "F15"
      ],
      "act": "done",
      "tests": {
        "passed": 562,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T17:42:12+03:00",
      "k": "add",
      "key": "concerns",
      "n": 2
    },
    {
      "t": "2026-10-23T17:42:35+03:00",
      "k": "stage",
      "id": "briefing",
      "act": "done"
    },
    {
      "t": "2026-10-23T17:42:35+03:00",
      "k": "gate",
      "g": "G1",
      "ok": true,
      "open": 0,
      "total": 107
    },
    {
      "t": "2026-10-23T17:42:35+03:00",
      "k": "stage",
      "id": "spec",
      "act": "done"
    },
    {
      "t": "2026-10-23T17:42:35+03:00",
      "k": "stage",
      "id": "plan",
      "act": "start"
    },
    {
      "t": "2026-10-23T17:42:36+03:00",
      "k": "tier",
      "v": "T2"
    },
    {
      "t": "2026-10-23T17:42:36+03:00",
      "k": "tickets",
      "n": 48
    },
    {
      "t": "2026-10-23T17:42:36+03:00",
      "k": "gate",
      "g": "G3",
      "ok": false,
      "problems": 162
    },
    {
      "t": "2026-10-23T17:42:48+03:00",
      "k": "tickets",
      "n": 48
    },
    {
      "t": "2026-10-23T17:42:49+03:00",
      "k": "gate",
      "g": "G3",
      "ok": false,
      "problems": 95
    },
    {
      "t": "2026-10-23T17:42:57+03:00",
      "k": "tickets",
      "n": 48
    },
    {
      "t": "2026-10-23T17:42:57+03:00",
      "k": "gate",
      "g": "G3",
      "ok": false,
      "problems": 48
    },
    {
      "t": "2026-10-23T17:42:57+03:00",
      "k": "gate",
      "g": "G3",
      "ok": false,
      "problems": 48
    },
    {
      "t": "2026-10-23T17:43:05+03:00",
      "k": "stage",
      "id": "build",
      "act": "start"
    },
    {
      "t": "2026-10-23T17:43:05+03:00",
      "k": "ticket",
      "ids": [
        "27",
        "28",
        "30"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T18:43:52+03:00",
      "k": "add",
      "key": "concerns",
      "n": 2
    },
    {
      "t": "2026-10-23T18:44:24+03:00",
      "k": "add",
      "key": "concerns",
      "n": 2
    },
    {
      "t": "2026-10-23T18:46:09+03:00",
      "k": "tickets",
      "n": 48
    },
    {
      "t": "2026-10-23T18:46:09+03:00",
      "k": "add",
      "key": "concerns",
      "n": 2
    },
    {
      "t": "2026-10-23T18:52:46+03:00",
      "k": "ticket",
      "ids": [
        "27"
      ],
      "act": "done",
      "tests": {
        "passed": 600,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T18:52:46+03:00",
      "k": "ticket",
      "ids": [
        "30"
      ],
      "act": "done",
      "tests": {
        "passed": 600,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T18:52:46+03:00",
      "k": "ticket",
      "ids": [
        "28"
      ],
      "act": "done",
      "tests": {
        "passed": 600,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T18:52:46+03:00",
      "k": "ticket",
      "ids": [
        "28"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-23T18:52:46+03:00",
      "k": "ticket",
      "ids": [
        "29"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-23T18:55:01+03:00",
      "k": "ticket",
      "ids": [
        "28"
      ],
      "act": "done",
      "tests": {
        "passed": 600,
        "failed": 0
      }
    },
    {
      "t": "2026-10-23T18:55:01+03:00",
      "k": "add",
      "key": "concerns",
      "n": 4
    }
  ],
  "note": "ремонт ядра: исполнитель работает (отметка 01:04)",
  "concerns": [
    "roles.rs:131/184 — заметка помечается delivered до spawn; при ошибке ro_copy/no_worktree теряется",
    "roles.rs:208, lanes.rs:313 — announced_notes до run_agent; при повторе spawn нет строки «note delivered»",
    "state.rs:268 — let _ = write_state глотает ошибку записи состояния",
    "lanes.rs:464 — asked_lane остаётся без вопроса, если ask_role упал",
    "kits/mod.rs:1249 + engine/mod.rs:247 — двойная проверка env, env_value_rejected не доходит до ленты задачи",
    "tests/common/mod.rs:258 — HoldRole теряет extra_env внутреннего Command",
    "agent/mod.rs:201,239 — run_blocking_with/run_lines_with публичные, нет в interfaces.md",
    "kits/mod.rs:453 — переменную с запрещённым именем Manifest молча выкидывает, предпросмотр не показывает её как отклонённую (спека 205/207)",
    "ui_ranges.rs — не проверяет поля ядра без экрана (keep.*)",
    "ПОДТВЕРЖДЕНО, безопасность: core/src/kits/secrets.rs:60 - набор задаёт CLAUDE_ENV_FILE / CLAUDE_CODE_SHELL_PREFIX / OPENCODE_CONFIG_CONTENT / CODE_ASSIST_ENDPOINT → чужой код при запуске агента или токен на чужой адрес; чинить разрешительным списком (только имена-секреты *_API_KEY, *_TOKEN, *_SECRET, *_PASSWORD), а не дописыванием запретов",
    "roles.rs:176, lanes.rs:288 - блок заметок дописывается к тексту каждой попытки, агент может увидеть заметку дважды",
    "units.rs:220 — экранирование в кавычках YAML (\"\\x68ooks\":) не ловится регуляркой; дыра, если Claude Code разбирает frontmatter полным YAML — не проверено",
    "units.rs:213 — ловится только !`; блоки ```! (если Claude Code их поддерживает) мимо — не проверено",
    "git.rs:299 — копии роя и фабрики игнорируют выключенный пользователем флаг protected_paths (неудобство)",
    "actions.rs:342 — хвост carry теряется, если поток кончился Done без Text",
    "app/src/hub.rs — тест склейки 500 строк держится на реальном времени (sleep до 95 мс), может мигать под нагрузкой",
    "tabs.js:18-23 — если список чатов не загружен к первому show(), restore() выкидывает все сохранённые вкладки; restore() только после загрузки чатов",
    "md.js:19-20 — auxclick и dragstart по ссылке не перехвачены (средний клик, перетаскивание)",
    "dev-mock.js:336-340 — пример CACHE_DIFF перемешан (удалённые после добавленных), @@ не сходится с телом; второй чат мока повторяет сценарий первого",
    "tabs.js:94-103 — ⌘W ловится только keydown страницы; в живом окне Tauri стандартное меню может закрыть окно — проверить в .app",
    "logic-v3.js:8 / open.rs:31 — HTTPS:// в верхнем регистре: JS пропускает, Rust отказывает (безопасно, несогласованно)",
    "F2 БЛОК: actions.rs:259,267 — e.file_type() не идёт по симлинку: .env-симлинк в worktree не читается, Scrub пуст, значения уходят в ленту (регрессия 23d6518); нужен fs::metadata для файлов",
    "F2: actions.rs:74 write_input берёт 20 строк до scrub — многострочный секрет (PEM) на границе частично в ленту",
    "F2: actions.rs:252 обход вглубь — одна ветка с 500+ папками съедает лимит, server/.env не читается; обход вширь",
    "F2: common.rs:326 summarize режет 120 символов до scrub",
    "actions.rs:42 многострочный секрет в диффе не заменяется из-за префиксов +/- (было и раньше)",
    "tabs.js:58-73 — persist() при отложенном restore() затирает сохранённые вкладки (сейчас недостижимо)",
    "selftest.js:428 — тест абзацев проверяет парсер, а баг был в CSS; нужен тест на display блоков",
    "T18: пауза flow control навсегда при перезагрузке окна без Destroyed (term/mod.rs:704)",
    "T18: close шлёт сигнал только shell - потомок держит slave, поток и master не освобождаются (term/mod.rs:767)",
    "T18: terms.json с выводом до 2 МБ пишется 0644 - нужен 0600 (ptycmds.rs:265)",
    "T18: все команды app доступны любому окну, нет манифеста команд по окнам (build.rs)",
    "T18 (повторное ревью): detach во время отправки Exit отменяется - is_none возвращает старый sink, Channel висит до attach/close (term/mod.rs:226)",
    "T18: двойной term-exit, если attach попал между снятием sink и Exit (было и до ремонта)",
    "T21: /compact в режиме -p у Claude не сверен; --allowedTools только у Claude",
    "T21: порты, записка и разрешённые инструменты не доходят до полос роя (run_lane); варианты гонки делят порт",
    "T21: сжимается только вывод проверок в промпте, команды агента - нет (нужен прокси вроде RTK)",
    "T21: стартовый налог MCP - только число серверов, без размера схем",
    "T22: закреплённая карточка задачи наезжает на ленту - из-под неё видны обрезанные строки (скриншот shots-22/dark-0.png)",
    "T22: taste FAIL - текст у края на 390px (обрезанный аргумент команды); ❯ принят как требование 244",
    "T19: мод в ~/.claude/mods Claude Code сам не грузит - у пользователя CLAUDE_CODE_PLUGIN_DIRS; нужен шаг подключения или подсказка",
    "T19: не сделано - вывод claude plugin validate, предупреждение о похожем имени (правило 9), copy_files при установке в проект",
    "T19: скиллы Codex/Gemini через catalog-skills/ по документации, на установленных CLI не проверено",
    "Флаки: 2 теста engine_swarm_watch упали в одном прогоне, на повторе прошли",
    "T21: git_revert_commit вызывает git::is_pushed в async без spawn_blocking",
    "T21: тело PR и промпт сравнения вариантов берут summary до 4000 символов",
    "T22: нет переименования вкладки двойным кликом (история 259, tabs.js:243)",
    "T22: закреплённая карточка задачи - видны обрезанные строки ленты над и под ней (chat.css:111)",
    "T22: сплит из сохранения v3 (panels) восстанавливается отдельными вкладками, не сплитом (panes.js:177)",
    "T22: старый раздел «Внешний вид» дублирует тему и шрифт из «Основное» (settings.js:343)",
    "T22: caret-shape: block - проверить в WKWebView, иначе рисованный курсор (v4-shell.css:53)",
    "T22: в dev-mock осталась строка «Вливать пока нельзя»",
    "T19: мод ставится в проект (.claude/mods проекта), а research правило 5 - в проект не предлагать (market/mod.rs:497)",
    "T19: второй распаковщик zip слабее kits/source.rs - 20000 файлов против 5000, нет лимита 10 МБ на файл (market/build.rs:34)",
    "T19 безопасность: curl без -q и --globoff - ~/.curlrc с -k отключает TLS, {a,b} в адресе своего источника даёт много запросов (net/mod.rs:69)",
    "T19: server_key MCP по последнему сегменту имени - разные io.github.a/x и io.github.b/x молча заменяют друг друга (market/build.rs:375)",
    "ПРИОРИТЕТ T21: та же дыра в protected_committed - git log диапазона ветки задачи без -m не видит .env из коммита слияния перед push/PR",
    "ВАЖНО: глобальный init.templatedir=~/.git-templates ставит хук graphify в каждый git init; хук пишет graphify-out/ после коммита - на реальных проектах пользователя merge Nightshift может получать AdvanceBlocked (main грязная). Нужна устойчивость: игнор graphify-out или тесты core/tests/common с --template=",
    "T20: очередь новых запусков на время обновления CLI к движку не подключена; занятость смотрит только задачи, не чаты",
    "T20: отката нет для brew и нативных Qwen/Codex/Cursor - только событие с командой; системного уведомления нет",
    "T20+T22: вид (тема, шрифт, акцент) живёт в settings.ui.tabs.look, а конфиг-файл ui не пишет - G51 «всё конфигом» не закрыт для вида",
    "T20: лишняя фикстура core/tests/fixtures/updates/codex_version.txt",
    "T23: пункт меню чата «Продолжить в терминале» (chat.js) ведёт в системный терминал, встроенный - только из палитры",
    "T23: лигатуры только на DOM-рендерере, аддона ligatures нет",
    "T23: вкладка помечается MutationObserver-обходом tabs.js - хрупко",
    "T23: на главном виде 14 кнопок - ровно предел",
    "T24: REDPROOF нарушен - тесты написаны после кода; критерии проверены только на mock",
    "T25: рисованная проверка UI считает 15 кнопок при лимите 14 (кнопка Clawd) - решить в ремонте по таску 26",
    "F4: NS_PLAYWRIGHT=1 в scripts/ui-selftest.js делает require('1') и виснет (сервер не закрывается в catch); рисованная проверка работает только с путём к playwright-core",
    "app/src/preview.rs - скрипт «Указать элемент» исполняется в контексте страницы превью: страница может подсунуть свой выбор (текст только вставляется в поле); в docs/security.md нужна строка про окно preview (без capabilities, переходы только localhost)",
    "core/src/engine/proof.rs - файлы, оставленные run_cmd в копии и не в .gitignore, попадут в коммит (как у шага e2e)",
    "core/tests/fixtures/claude_stream_thinking.jsonl — формат мыслей по API Anthropic, не снят с живого CLI; проверить в живом прогоне",
    "мысли не сохраняются на диск, видны только в сессии; в задачах - только индикатор «думает»",
    "core/src/sessions.rs — путь проекта длиннее 200 символов: Claude, возможно, добавляет хэш к ключу - не учтено",
    "продолженная сессия идёт с выключенным Качеством (план и критика пропущены)",
    "docs/security.md — строка «нет своего адреса»: теперь есть проверка релиза по release_feed (только если задан) - обновить в аудите 32",
    "совет роя (council) при лимите по-прежнему ставит на паузу всю задачу - гонка и бригада по полосам",
    "app/src/diag.rs:34 — в app.log только старт, сбой проверки обновления и сборка отчёта; падение задачи, пауза по лимиту, сбой CLI не пишутся ключом - хвост лога в отчёте пуст про саму ошибку",
    "core/tests/engine_swarm_lanes.rs:127 — пауза полосы проверена только для гонки, бригада (crew.rs:104,282) без теста",
    "ui/js/screens/swarm.js — «ждёт лимит до HH:MM» в дорожке только у вариантов гонки, у части бригады нет",
    "core/src/diag.rs:132 — ключи карт настроек уходят в zip как есть (сейчас безопасно: только id сред)"
  ],
  "report": [
    "Аудит безопасности: 6 находок, 4 подтверждены (3 разные дыры) - в таск F1; 2 опровергнуты: имя ветки из origin/HEAD как опция git (не доходит до опасных вызовов), session_id без проверки в --resume (вход не контролирует атакующий)",
    "Зависимость glib 0.18.5 (GHSA-wrw7-89jp-8q8g, средняя) приходит через GTK-стек Tauri; код проекта не даёт ей данных атакующего - обновится с Tauri",
    "F1 после повторного ревью: значение TOML, не закрытое к концу файла набора (model = ''' в последней строке), ставится без галочки и ломает конфиг Codex (не дыра, порча)",
    "F1: ветка, меняющая уже отслеживаемый секретный файл (server.key-фикстура), не вливается никогда, ошибка не говорит, что делать - выход только выключить «Защищённые пути»",
    "F1: ключ TOML с экранированной кавычкой (\"a\\\"b\" = [) молча теряется при импорте набора",
    "Лента: в очень долгом чате каждое новое действие перебирает все прошлые (logic-v3.js:179) - может тормозить на тысячах действий",
    "Linux: Ctrl+D и Ctrl+W в поле ввода заняты вкладками - при Emacs-раскладке GTK пропадают «удалить символ/слово»",
    "Упавшее чтение файла показывает «N строк» вместо текста ошибки (actions.js:122)",
    "F2: чтение .env проверяет размер, а потом читает без лимита (подмена между вызовами); путь <копия>/../x показывается как ../x; StructuredOutput без начала попадёт в ленту; меню не перестраивается при смене языка",
    "Словарь: в CONTEXT.md «Набор» стоит среди запрещённых слов (правило 289), а спека и интерфейс до сих пор говорят «Наборы». Нужно решить, какое слово оставить",
    "Моды: публичного каталога модов Claude Code не нашлось - вкладка «Моды» показывает встроенные источники, живые моды ставятся «По ссылке» (git, zip, папка)"
  ],
  "dir": "vibe-env",
  "reqList": [
    {
      "id": "R01",
      "text": "«создать среду для вейп-кодинга»",
      "status": "in-ticket"
    },
    {
      "id": "R02",
      "text": "«выложить её полностью как open-source»",
      "status": "in-ticket"
    },
    {
      "id": "R03",
      "text": "«чтобы удобнее было управлять агентами»",
      "status": "in-ticket"
    },
    {
      "id": "R04",
      "text": "«человек даёт задачу»",
      "status": "in-ticket"
    },
    {
      "id": "R05",
      "text": "«всё остальное делают агенты»",
      "status": "in-ticket"
    },
    {
      "id": "R06",
      "text": "«перепроверять этот цикл не нужно»",
      "status": "in-ticket"
    },
    {
      "id": "R07",
      "text": "«агент оповещает человека, когда это нужно»",
      "status": "in-ticket"
    },
    {
      "id": "R08",
      "text": "«Это приложение»",
      "status": "in-ticket"
    },
    {
      "id": "R09",
      "text": "«под Mag» (macOS)",
      "status": "in-ticket"
    },
    {
      "id": "R10",
      "text": "«и Linux»",
      "status": "in-ticket"
    },
    {
      "id": "R11",
      "text": "«добавить очень удобные вещи, чтобы это было комфортно пользоваться»",
      "status": "in-ticket"
    },
    {
      "id": "R12",
      "text": "«Посмотри, как у других аналогов это сделано»",
      "status": "in-ticket"
    },
    {
      "id": "R13",
      "text": "«может быть, что-то придумай своё»",
      "status": "in-ticket"
    },
    {
      "id": "R14i",
      "text": "(подразумевается) какой движок агентов запускается (Claude Code / Codex / свой цикл на API)",
      "status": "in-ticket"
    },
    {
      "id": "R15i",
      "text": "(подразумевается) куда приходит оповещение (системные уведомления / Telegram / звук)",
      "status": "in-ticket"
    },
    {
      "id": "R16i",
      "text": "(подразумевается) без человеческой проверки нужна автоматическая проверка результата (тесты, ревью агентом), иначе «не перепроверять» = выпускать брак",
      "status": "in-ticket"
    },
    {
      "id": "R17i",
      "text": "(подразумевается) опасные действия агентов (удаление, пуш, деплой) должны быть ограничены, раз человек не смотрит",
      "status": "done"
    },
    {
      "id": "R18i",
      "text": "(подразумевается) человек может видеть, что идёт, и вмешаться/остановить при желании",
      "status": "in-ticket"
    },
    {
      "id": "R19i",
      "text": "(подразумевается) несколько задач/агентов параллельно не мешают друг другу (изоляция рабочих копий)",
      "status": "in-ticket"
    },
    {
      "id": "R20i",
      "text": "(подразумевается) open-source: лицензия, README, установка одной командой",
      "status": "in-ticket"
    },
    {
      "id": "R21",
      "text": "(название)",
      "status": "in-ticket"
    },
    {
      "id": "G01",
      "text": "«внедрить ветки GitHub, push, Origin, это вот всё остальное, чтобы было удобней»",
      "status": "in-ticket"
    },
    {
      "id": "G02",
      "text": "«должны работать все генные среды, то есть не только Klot, код» (агентные среды, не только Claude Code)",
      "status": "in-ticket"
    },
    {
      "id": "G03",
      "text": "«Telegram-бот не надо.»",
      "status": "dropped",
      "why": "пользователь: «Telegram-бот не надо.» - сняты истории 36-41 (бот, привязка, ответ из Telegram, прокси), 39 (A02 задачи с телефона), 53 (A03 утренняя сводка); таск 04 вырезан"
    },
    {
      "id": "G04",
      "text": "«проведи потом полный аудит безопасности, используя плагин для этого и скиллы, которые у тебя. стоят»",
      "status": "in-ticket"
    },
    {
      "id": "G05",
      "text": "«А где мне переписываться с самим агентом?»",
      "status": "done"
    },
    {
      "id": "G06",
      "text": "«возьми референсы интерфейсов таких приложений, допустим, на Pinterest какие-то ещё более-менее уникальные штуки»",
      "status": "done"
    },
    {
      "id": "G07",
      "text": "«где можно, допустим, роль агентов запустить»",
      "status": "done"
    },
    {
      "id": "G08",
      "text": "«или фабрику агентов, как тот человек рассказывал»",
      "status": "in-ticket"
    },
    {
      "id": "G09",
      "text": "«Или то, что сам Андропик рекомендует как использовать»",
      "status": "in-ticket"
    },
    {
      "id": "G10",
      "text": "«какие фишки они применяют для того, чтобы более высокого качества код получать, и таким образом внедри их в одну кнопку»",
      "status": "in-ticket"
    },
    {
      "id": "G11",
      "text": "«сделать галочку, допустим, рой агентов, и открывается отдельный чат»",
      "status": "in-ticket"
    },
    {
      "id": "G12",
      "text": "«и агенты должны быть структурированы везде»",
      "status": "in-ticket"
    },
    {
      "id": "G13",
      "text": "«Я бы интерфейс кардинально переделал.»",
      "status": "in-ticket"
    },
    {
      "id": "G14",
      "text": "«ты из документации преобразуешь её в нормальный промпт под модель, которую я выбираю»",
      "status": "in-ticket"
    },
    {
      "id": "G15",
      "text": "«должно быть минимальное участие человека… сама фишка должна быть в самостоятельности агента»",
      "status": "in-ticket"
    },
    {
      "id": "G16",
      "text": "«должно быть куча полезных настроек, которые ты считаешь полезными, то есть пользователь может настроить приложение под себя»",
      "status": "in-ticket"
    },
    {
      "id": "G17",
      "text": "«если мы хотим код, то мы входим в код, если мы хотим в Кодекс, то мы входим в Кодекс, если мы хотим Джимини, то мы входим в Джимини»",
      "status": "in-ticket"
    },
    {
      "id": "G18",
      "text": "«добавь туда выбор моделей, чтобы можно было именно выбирать модели»",
      "status": "in-ticket"
    },
    {
      "id": "G19",
      "text": "«сейчас всё это намешано, очень сильно нагружен интерфейс, пользователь будет непонятно… отторгает сильно»",
      "status": "done"
    },
    {
      "id": "G20",
      "text": "«где чат с моделью? Как чатиться пользователю?»",
      "status": "in-ticket"
    },
    {
      "id": "G21",
      "text": "«на модель ты должен ставить оригинальные значки самой модели»",
      "status": "done"
    },
    {
      "id": "G22",
      "text": "«агентские среды не должны переплетаться друг с другом… код работает с кодом, Кодекс работает отдельно, Джимини работает отдельно, и все они должны запускать отдельных субагентов для этого»",
      "status": "in-ticket"
    },
    {
      "id": "G23",
      "text": "«Также можно выбирать номера моделей. Ты тоже должен это продумать.»",
      "status": "in-ticket"
    },
    {
      "id": "G24",
      "text": "«зачем нужны дорожки и всё вот это остальное? … зачем это нужно отдельной вкладкой?»",
      "status": "in-ticket"
    },
    {
      "id": "G25",
      "text": "«добавь ещё импортацию всех скиллов и так далее, всех правил… если кто-то скачивает мою среду или я её устанавливаю… Перенос всех хуков»",
      "status": "done"
    },
    {
      "id": "G26",
      "text": "«чтобы в чате были видны пятичасовые лимиты и недельные лимиты»; «вообще любые лимиты агентных средств… переключаешь на Джамини - её лимиты, на Кодекс - лимиты Кодекса»",
      "status": "done"
    },
    {
      "id": "G27",
      "text": "«Также сделай красивый установщик приложения»; «это под все дистрибутивы Linux должно подходить»",
      "status": "in-ticket"
    },
    {
      "id": "G28",
      "text": "«Начальный интерфейс может запутать пользователя»",
      "status": "done"
    },
    {
      "id": "G29",
      "text": "«когда он что-то пишет, сделай какую-то анимацию, что он пишет код. Этот код должен отображаться как в код-коде»",
      "status": "done"
    },
    {
      "id": "G30",
      "text": "«пользователь должен видеть прогресс работы, он должен быть показан качественно… Пользователя не видно, что он там делает, какие команды запускает»",
      "status": "done"
    },
    {
      "id": "G31",
      "text": "«можно было запускать несколько агентов параллельно, несколько сессий, точнее, и мышкой, и сочетанием клавиш»",
      "status": "done"
    },
    {
      "id": "G32",
      "text": "«чтобы начать сам пользоваться»",
      "status": "done"
    },
    {
      "id": "G33",
      "text": "«Я хочу сделать другой дизайн, прямо скруглённый»",
      "status": "done"
    },
    {
      "id": "G34",
      "text": "«это должно быть похоже на терминал»",
      "status": "done"
    },
    {
      "id": "G35",
      "text": "«код должен работать в терминале»; «терминал будет считаться эмулятором»",
      "status": "done"
    },
    {
      "id": "G36",
      "text": "«этот терминал можно было бы кастомизировать»",
      "status": "done"
    },
    {
      "id": "G37",
      "text": "«делать сплиты, девочка»",
      "status": "done"
    },
    {
      "id": "G38",
      "text": "«красивый дизайн, красивый, красивый плюс окна»",
      "status": "done"
    },
    {
      "id": "G39",
      "text": "«это как-то всё выглядит сейчас громоздко… нету какого-то вкуса… нету изюминки»",
      "status": "done"
    },
    {
      "id": "G40",
      "text": "«чтобы туда могли переноситься моды мои, моды чужие»",
      "status": "done"
    },
    {
      "id": "G41",
      "text": "«чтобы можно было качать скиллы с популярных сайтов»",
      "status": "done"
    },
    {
      "id": "G42",
      "text": "«моды для кладка, то есть MCP»",
      "status": "done"
    },
    {
      "id": "G43",
      "text": "«когда выходила новая модель, автоматически обновлялся, и все модели автоматически обновлялись, включая GMI, включая ChatGPT»",
      "status": "done"
    },
    {
      "id": "G44",
      "text": "«внедри туда ещё GitHub… чтобы можно было публиковать»",
      "status": "done"
    },
    {
      "id": "G45",
      "text": "«чтобы можно было смотреть историю изменений, чтобы всё это красиво выглядело»",
      "status": "done"
    },
    {
      "id": "G46",
      "text": "«анимацию персонажа Клота Нукро, этот вот, который я мод писал, то, что он там что-то делает, сделал анимацию в 60 FPS»",
      "status": "done"
    },
    {
      "id": "G47",
      "text": "«Придумай ему какую-нибудь историю этому персонажу»",
      "status": "done"
    },
    {
      "id": "G48",
      "text": "«сделай приложение сильным, а то оно сейчас какое-то громоздкое»",
      "status": "done"
    },
    {
      "id": "G49",
      "text": "«можешь ещё добавить функции по своему усмотрению, какие можешь посмотреть, какие люди добавляют»",
      "status": "done"
    },
    {
      "id": "G50",
      "text": "«Технические термины.»",
      "status": "done"
    },
    {
      "id": "G51",
      "text": "«приложение можно было полностью кастомизировать конфигами, но также настройками можно вручную»",
      "status": "done"
    },
    {
      "id": "G52",
      "text": "«Дизайн разрабатывался через Tast-skills»",
      "status": "done"
    },
    {
      "id": "G53",
      "text": "«Поищи рекомендации от других людей на YouTube… как запускать несколько кодов параллельно и экономить токены, как запускать несколько агентов… как делать фабрики, как делать заводы и внедри»",
      "status": "done"
    },
    {
      "id": "G54",
      "text": "«Если вышло обновление какого-то терминального CLI, то ты его берёшь автоматически, приходит обновление, и пользователь должен перезапустить именно окно с терминалом»",
      "status": "done"
    },
    {
      "id": "G55",
      "text": "«все вот эти Codex, КВН, Джамини — это считается как отдельный терминал… терминальная среда по типу Tmux и так далее и Head»",
      "status": "done"
    },
    {
      "id": "G56",
      "text": "«Дизайн должен быть максимально похож на вот эти Linx, стэтапы, Linx, терминалы»",
      "status": "done"
    },
    {
      "id": "G57",
      "text": "«Продвинутый пользователь может его кастомизировать через настройки. Обычный пользователь может… какие-то базовые вещи кастомить»",
      "status": "done"
    },
    {
      "id": "G58",
      "text": "«То есть это должен быть по сути мультиплекс.»",
      "status": "done"
    },
    {
      "id": "G59",
      "text": "«Приложение тестируешь обязательно, обязательно через мой компьютер… полностью всё проверяешь через компьютер US»",
      "status": "in-ticket"
    },
    {
      "id": "G60",
      "text": "«проведи аудит, что мне в этой программе не хватает и что внедряют многие люди»",
      "status": "in-ticket"
    },
    {
      "id": "G61",
      "text": "«задай мне вопросы, как мне это внедрить и внедряться самостоятельно»",
      "status": "done"
    },
    {
      "id": "G62",
      "text": "«Очередь чата, ожидание лимита и защита от двойной отправки… Сохранять на диск?» → «Да, на диск»",
      "status": "done"
    },
    {
      "id": "G63",
      "text": "«Codex, Gemini CLI, Cursor и пакеты Linux ни разу не запускались вживую. Как проверяем?» → «Скрипт на Arch»",
      "status": "in-ticket"
    },
    {
      "id": "G64",
      "text": "«Лимит у одного агента роя… ставит на паузу всю задачу. Менять?» → «Стоит только он»",
      "status": "done"
    },
    {
      "id": "G65",
      "text": "«Картинки в чате (вставить скриншот, перетащить файл)… Делать?» → «Да»",
      "status": "in-ticket"
    },
    {
      "id": "G66",
      "text": "«Голосовой ввод внутри приложения?» → «Не делаем»",
      "status": "dropped",
      "why": "пользователь: «Не делаем» (Wispr Flow работает в любом окне)"
    },
    {
      "id": "G67",
      "text": "«Задачи из GitHub Issues… Делать?» → «Да, закрытие с вопросом»",
      "status": "done"
    },
    {
      "id": "G68",
      "text": "«Подхват старых сессий: чаты Claude Code, начатые в терминале… Делать?» → «Да»",
      "status": "done"
    },
    {
      "id": "G69",
      "text": "«Обновление самой программы… Как?» → «Плашка со ссылкой»",
      "status": "done"
    },
    {
      "id": "G70",
      "text": "«История расходов… Что добавить?» → «Таблица по дням»",
      "status": "in-ticket"
    },
    {
      "id": "G71",
      "text": "«Кнопка «Собрать отчёт об ошибке»… Делать?» → «Да»",
      "status": "done"
    },
    {
      "id": "G72",
      "text": "«Локальные модели (Ollama / LM Studio через opencode)?» → «Не делаем»",
      "status": "dropped",
      "why": "пользователь: «Не делаем»"
    },
    {
      "id": "G73",
      "text": "«Подпись сборки для macOS - $99 в год» → «Пока без подписи»",
      "status": "deferred",
      "why": "пользователь: «Пока без подписи»; инструкция xattr в README остаётся"
    },
    {
      "id": "G74",
      "text": "«Когда публикуем репозиторий на GitHub?» → «После v5 и аудита»",
      "status": "in-ticket"
    },
    {
      "id": "G75",
      "text": "«Я надеюсь, все скиллы и вся память импортируются, и MCP, соответственно, тоже»",
      "status": "in-ticket"
    },
    {
      "id": "G76",
      "text": "«когда пользователи будут скачивать мою программу, у них должна устанавливаться чистая программа»",
      "status": "in-ticket"
    },
    {
      "id": "G77",
      "text": "«если у них будут мои скиллы, мои MCP и моё всё, это может быть утечкой»",
      "status": "in-ticket"
    },
    {
      "id": "G77i",
      "text": "(подразумевается) публикуемый репозиторий тоже не несёт личного: .autopilot/ (брифы, исследования, скриншоты), пути /Users/nezeronxer, данные dev-mock",
      "status": "in-ticket"
    },
    {
      "id": "G78",
      "text": "«протестируй её в ручном режиме, в режиме пользователя, в интерфейсе»",
      "status": "in-ticket"
    },
    {
      "id": "G79",
      "text": "«и погони аудит полной безопасности»",
      "status": "in-ticket"
    },
    {
      "id": "G80",
      "text": "«Анимация должна быть анимация кода, то, что и у всех остальных нейросетей, агентов, то, что они что-то там думают, ты написал текст, а там пусто»",
      "status": "done"
    },
    {
      "id": "G81",
      "text": "«все эти агенты не печатают плавно, то есть они печатают не в 60 FPS»",
      "status": "done"
    },
    {
      "id": "G82",
      "text": "«она запускается вот, знаешь, как в чате, но а где моды, где скиллы? Ты всё это туда внедрил?»",
      "status": "in-ticket"
    },
    {
      "id": "G83",
      "text": "«сделай вот этот чат как оболочку терминала, то есть это по сути будет как терминальная версия, только ты сделаешь свою версию… как чат… чтобы было более удобней»",
      "status": "in-ticket"
    },
    {
      "id": "G84",
      "text": "«то, что Claude думает, всё это может меняться модами Claude-кода, то есть ты должен туда плавно, ничего не ломая, внедрить также моды Claude-кода, который позволяет ему менять под себя»",
      "status": "done"
    },
    {
      "id": "G85",
      "text": "«И так сделать со всеми терминальными версиями.»",
      "status": "in-ticket"
    }
  ],
  "gates": {
    "G2": {
      "ok": true,
      "at": "2026-10-23T17:01:48+03:00",
      "found": 6,
      "fixed": 6,
      "deferred": 0
    },
    "G3": {
      "ok": false,
      "at": "2026-10-23T17:42:57+03:00",
      "problems": 48
    },
    "G1": {
      "ok": true,
      "at": "2026-10-23T17:42:35+03:00",
      "open": 0,
      "total": 107
    },
    "G4": {
      "ok": false,
      "at": "2026-10-23T10:24:06+03:00",
      "checked": 43,
      "matched": 40
    }
  },
  "baseCommit": "dfd8d3a",
  "skillDir": "/Users/nezeronxer/.agents/skills/autopilot",
  "waiting": null,
  "waits": [
    {
      "kind": "briefing",
      "from": "2026-10-23T16:37:26+03:00",
      "to": "2026-10-23T16:39:14+03:00"
    },
    {
      "kind": "briefing",
      "from": "2026-10-23T16:40:39+03:00",
      "to": "2026-10-23T16:41:37+03:00"
    }
  ]
}
