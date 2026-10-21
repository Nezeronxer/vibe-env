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
  "updatedAt": "2026-10-21T10:13:20+03:00",
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
      "startedAt": "2026-10-21T08:53:31+03:00",
      "finishedAt": "2026-10-21T09:02:22+03:00"
    },
    {
      "id": "plan",
      "status": "done",
      "startedAt": "2026-10-21T09:02:22+03:00",
      "finishedAt": "2026-10-21T09:02:22+03:00",
      "note": "7 тасков в 4 волны, ярус T2"
    },
    {
      "id": "build",
      "status": "active",
      "startedAt": "2026-10-21T09:02:22+03:00",
      "note": "0 из 7 тасков готовы"
    },
    {
      "id": "review",
      "status": "active",
      "startedAt": "2026-10-21T10:07:22+03:00",
      "note": "на ревью 01"
    },
    {
      "id": "final",
      "status": "pending"
    }
  ],
  "requirements": {
    "total": 23,
    "done": 0,
    "inTicket": 23,
    "inSpec": 0,
    "placeholder": 0,
    "deferred": 0,
    "dropped": 0
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
      "status": "review",
      "retries": 0,
      "repairs": 1,
      "startedAt": "2026-10-21T09:03:14+03:00"
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
      "status": "pending",
      "retries": 0,
      "repairs": 0
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
      "status": "pending",
      "retries": 0,
      "repairs": 0
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
        "core/src/telegram/"
      ],
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
        "04",
        "06"
      ],
      "wave": 3,
      "zone": [
        "app/"
      ],
      "status": "pending",
      "retries": 0,
      "repairs": 0
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
      "status": "pending",
      "retries": 0,
      "repairs": 0
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
  "tests": null,
  "debt": {
    "placeholders": [
      "Токен Telegram-бота - вписываешь сам в настройках",
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
    "A02 задачи с телефона /new - ради R04",
    "A03 утренняя сводка - ради R07",
    "A04 время и попытки в карточке - ради R03",
    "A05 запасной движок при лимите - ради R05"
  ],
  "coverage": {
    "found": 2,
    "fixed": 2,
    "deferred": 0,
    "note": "GitHub «всё остальное» расписан (ист. 27a); раздел аналогов дописан по research.md"
  },
  "blind": null
}
