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
  "updatedAt": "2026-10-21T13:34:57+03:00",
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
      "finishedAt": "2026-10-21T08:42:22+03:00"
    },
    {
      "id": "briefing",
      "status": "done",
      "startedAt": "2026-10-21T08:42:22+03:00",
      "finishedAt": "2026-10-21T08:53:31+03:00",
      "note": "4 вопроса + 2 дополнения"
    },
    {
      "id": "spec",
      "status": "done",
      "startedAt": "2026-10-21T12:15:27+03:00",
      "note": "v2: 221 история, G2 - не покрыто 0, наполовину 5",
      "finishedAt": "2026-10-21T13:22:54+03:00"
    },
    {
      "id": "plan",
      "status": "active",
      "startedAt": "2026-10-21T13:22:54+03:00",
      "note": "перенарезка v2, ярус T3"
    },
    {
      "id": "build",
      "status": "active",
      "startedAt": "2026-10-21T09:02:22+03:00",
      "note": "готово 5 тасков (01, 02, 03, 05, 06-основа); нарезка v2 пишется"
    },
    {
      "id": "review",
      "status": "active",
      "startedAt": "2026-10-21T10:07:22+03:00",
      "note": "проверены 01, 02, 03, 05, 06"
    },
    {
      "id": "final",
      "status": "pending"
    }
  ],
  "requirements": {
    "total": 24,
    "done": 1,
    "inTicket": 23,
    "inSpec": 0,
    "placeholder": 0,
    "deferred": 0,
    "dropped": 1
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
        "core/",
        "core/src/agent/adapters/",
        "core/src/engine/",
        "core/src/telegram/",
        "app/",
        "ui/"
      ],
      "status": "done",
      "retries": 0,
      "repairs": 1,
      "startedAt": "2026-10-21T09:03:14+03:00",
      "finishedAt": "2026-10-21T10:14:53+03:00",
      "commit": "483310e",
      "tests": {
        "passed": 40,
        "failed": 0
      },
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
        "core/src/agent/"
      ],
      "status": "done",
      "retries": 0,
      "repairs": 2,
      "startedAt": "2026-10-21T10:14:53+03:00",
      "concerns": [
        "Codex/Gemini/Cursor - флаги только по документации",
        "коммит ждёт окончания правки agent/mod.rs таском 03"
      ],
      "finishedAt": "2026-10-21T12:04:29+03:00",
      "commit": "3ce27ee",
      "tests": {
        "passed": 27,
        "failed": 0
      }
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
        "core/src/engine/"
      ],
      "status": "done",
      "retries": 0,
      "repairs": 2,
      "startedAt": "2026-10-21T10:14:53+03:00",
      "tests": {
        "passed": 36,
        "failed": 0
      },
      "concerns": [
        "в доработку движка v2: тест на не-UTF-8 вывод, тест retry после вливания, ns-hidden: ошибка чтения кроме NotFound → Err",
        "state между ходами в .pending/<id>.json, не в Task"
      ],
      "finishedAt": "2026-10-21T12:04:29+03:00",
      "commit": "45cc523"
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
        "app/"
      ],
      "status": "done",
      "retries": 0,
      "repairs": 2,
      "startedAt": "2026-10-21T13:15:59+03:00",
      "finishedAt": "2026-10-21T13:34:57+03:00",
      "commit": "ae4bc2b",
      "tests": {
        "passed": 6,
        "failed": 0
      },
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
        "R15i.2",
        "R15i.4",
        "R15i.5",
        "A03",
        "A04"
      ],
      "blockedBy": [
        "01"
      ],
      "wave": 2,
      "zone": [
        "ui/"
      ],
      "status": "done",
      "retries": 0,
      "repairs": 1,
      "startedAt": "2026-10-21T10:17:09+03:00",
      "finishedAt": "2026-10-21T10:40:37+03:00",
      "commit": "6f709b8",
      "concerns": [
        "пользователь попросил кардинально переделать интерфейс - это база для переделки"
      ]
    },
    {
      "id": "07",
      "title": "Open-source: README, лицензия, сборки в CI",
      "requirements": [
        "R02",
        "R20i",
        "R20i.1",
        "R12",
        "R13",
        "R21"
      ],
      "blockedBy": [
        "05",
        "06"
      ],
      "wave": 4,
      "zone": [
        ".github/",
        "docs/"
      ],
      "status": "pending",
      "retries": 0,
      "repairs": 0
    }
  ],
  "singlePass": null,
  "tests": {
    "passed": 110,
    "failed": 0
  },
  "debt": {
    "placeholders": [
      "Подпись macOS - нужен Apple Developer"
    ],
    "assumptions": [
      "Название «Nightshift» (рабочее)",
      "Лицензия MIT",
      "Второй живой движок - opencode (Codex не установлен)"
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
    "found": 7,
    "fixed": 3,
    "deferred": 0,
    "note": "v1: 2 найдено, 2 исправлено. v2: не покрыто 0; наполовину 5 - рой открывает отдельный чат (исправлено), руководства вендоров по промптам - собрать в таске 09, Pinterest не смотрели (открытое место), плагин аудита - читаем SKILL.md без включения (решение), «Mona/Sola» - распознавание речи"
  },
  "blind": null
}
