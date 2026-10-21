/* Nightshift v2 - макет. Среда - верхний уровень, внутри - чат с выбранной моделью. Все данные выдуманные. */
(function () {
  'use strict';
  var $ = function (s, r) { return (r || document).querySelector(s); };
  var $$ = function (s, r) { return Array.prototype.slice.call((r || document).querySelectorAll(s)); };

  var ROLES = {
    plan: 'Планировщик', crit: 'Критик плана', test: 'Автор тестов', exec: 'Исполнитель',
    gate: 'Ворота', rev: 'Ревьюер', integ: 'Интегратор',
    coord: 'Координатор', watch: 'Надзиратель', queue: 'Очередь слияния'
  };
  function rl(k) { return '<svg class="rl r-' + k + '" aria-hidden="true"><use href="#i-' + k + '"/></svg>'; }
  function ic(n, c) { return '<svg class="ic' + (c ? ' ' + c : '') + '" aria-hidden="true"><use href="#i-' + n + '"/></svg>'; }
  /* оригинальные знаки вендоров (icons/SOURCES.md); mono - чёрный знак, в тёмной теме инвертируется в белый */
  var LOGO = { claude: ['claude-code', 0], codex: ['openai', 1], gemini: ['gemini', 0], opencode: ['opencode', 1], cursor: ['cursor', 1], qwen: ['qwen', 1],
    anthropic: ['claude', 0], openai: ['openai', 1] };
  function eic(e, c) {
    var l = LOGO[e];
    if (l) return '<img class="ic logo' + (l[1] ? ' mono' : '') + (c ? ' ' + c : '') + '" src="icons/' + l[0] + '.svg" alt="">';
    return '<svg class="ic' + (c ? ' ' + c : '') + '" aria-hidden="true"><use href="#e-' + e + '"/></svg>';
  }
  function esc(s) { return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;'); }

  /* ---------- среды. Имена моделей и версии - пример: в приложении они берутся из самой CLI ---------- */
  var ORDER = ['claude', 'codex', 'gemini', 'opencode', 'cursor', 'qwen', 'custom'];
  var ENVS = {
    claude: { n: 'Claude Code', s: 'Claude', ver: '2.1.4', sandbox: true, net: 'по списку', files: 'запрещены', def: 1,
      models: [['Opus', 'Сильнее всех и дороже. Сложное и рискованное.'], ['Sonnet', 'Баланс скорости и качества. Подходит почти всегда.'], ['Haiku', 'Быстро и дёшево. Мелкие правки.']] },
    codex: { n: 'Codex', s: 'Codex', ver: '0.48', sandbox: true, net: 'выключена', files: 'запрещены', def: 0,
      models: [['gpt-5-codex', 'Заточена под код. Хороший выбор по умолчанию.'], ['gpt-5', 'Глубже рассуждает, медленнее.'], ['gpt-5-mini', 'Быстро и дёшево.']] },
    gemini: { n: 'Gemini', s: 'Gemini', ver: '0.9', sandbox: false, net: 'открыта', files: 'читает', def: 0,
      models: [['2.5 Pro', 'Сильнее, очень длинный контекст.'], ['2.5 Flash', 'Быстро и дёшево.']] },
    opencode: { n: 'opencode', s: 'opencode', ver: '0.15', sandbox: false, net: 'открыта', files: 'читает', def: 0,
      models: [['Sonnet', 'Через твой ключ Anthropic.'], ['GPT-5', 'Через твой ключ OpenAI.'], ['Локальная', 'Ollama на этом компьютере, бесплатно.']] },
    cursor: { n: 'Cursor', s: 'Cursor', missing: true, install: 'curl https://cursor.com/install -fsS | bash', def: 0,
      models: [['Auto', 'Cursor сам выбирает модель.'], ['Sonnet', 'Баланс скорости и качества.'], ['GPT-5', 'Глубже рассуждает.']] },
    qwen: { n: 'Qwen Code', s: 'Qwen', missing: true, install: 'npm install -g @qwen-code/qwen-code', def: 0,
      models: [['Qwen3 Coder', 'Открытая модель под код.'], ['Qwen3 Coder Flash', 'Быстрее и дешевле.']] },
    custom: { n: 'Своя команда', s: 'Своя', custom: true, def: 0, models: [['Из команды', 'Модель задаёт сама команда.']] }
  };
  ORDER.forEach(function (k) { ENVS[k].model = ENVS[k].def; });
  var MV = { claude: ['anthropic', 'anthropic', 'anthropic'], codex: ['openai', 'openai', 'openai'], gemini: ['gemini', 'gemini'], opencode: ['anthropic', 'openai', 'custom'],
    cursor: ['cursor', 'anthropic', 'openai'], qwen: ['qwen', 'qwen'], custom: ['custom'] };
  function modelName(k) { return ENVS[k].models[ENVS[k].model][0]; }

  /* ---------- чаты внутри сред ---------- */
  var STEPS7 = ['План', 'Критик', 'Тесты', 'Код', 'Ворота', 'Ревью', 'Влив'];
  var ST = {
    wait: ['Ждёт тебя', 'st-wait', 'ask'], run: ['Работает', 'st-run', 'run'], done: ['Готово', 'st-ok', 'check'],
    fail: ['Не вышло', 'st-bad', 'fail'], queue: ['В очереди', 'st-run', 'dash'], chat: ['', '', 'chat']
  };
  var INV_FEED = [
    { t: 'me', time: '13:58', html: 'Как у нас сейчас устроена пагинация в /invoices?' },
    { t: 'ai', time: '13:58', html: '<p>Через offset: <code>?page=</code>, по 50 записей (src/invoices/routes.py). На большой таблице это медленно, а при вставках записи повторяются между страницами.</p>' },
    { t: 'tool', c: 'read', a: 'src/invoices/routes.py', res: '212 строк' },
    { t: 'me', time: '14:01', html: 'Сделай курсорную пагинацию. Старый мобильный клиент не сломай.' },
    { t: 'card' },
    { t: 'sys', text: 'Это задача. Переписал в промпт и запустил шаги качества.', prompt: true },
    { t: 'msg', r: 'plan', time: '14:02', html: '<p>План без смены схемы:</p><ul><li>курсор = (created_at, id), непрозрачный</li><li>?limit по умолчанию 50, максимум 200</li><li>старый ?page= пока остаётся</li></ul>' },
    { t: 'msg', r: 'crit', time: '14:04', html: '<p>created_at не уникален, без id записи пропадут на границе страниц. И нужен ответ на битый курсор: 400 invalid_cursor.</p>' },
    { t: 'msg', r: 'test', time: '14:08', html: '<p>9 тестов, все красные, как и должно быть до кода.</p>' },
    { t: 'tool', c: 'write', a: 'tests/test_invoices_pagination.py', res: '+118' },
    { t: 'tool', c: 'run', a: 'pytest tests/test_invoices_pagination.py', res: '9 failed', bad: true },
    { t: 'msg', r: 'exec', time: '14:11', html: '<p>Курсор и валидация готовы, подключаю к запросу.</p>' },
    { t: 'tool', c: 'edit', a: 'src/invoices/routes.py', res: '+64 −12' },
    { t: 'tool', c: 'edit', a: 'src/invoices/cursor.py', res: 'новый, +41' },
    { t: 'tool', c: 'run', a: 'pytest tests/test_invoices_pagination.py', res: '7 passed, 2 failed', bad: true },
    { t: 'msg', r: 'gate', time: '14:13', html: '<p>Тесты 7 из 9, линтер и типы чистые. Вливать пока нельзя.</p>' },
    { t: 'me', time: '14:14', html: 'Курсор пусть будет непрозрачным, id наружу не свети.' },
    { t: 'msg', r: 'exec', time: '14:15', html: '<p><span class="live"></span>Подписываю курсор, чиню два теста. Задал вопрос про старый клиент - он в карточке задачи.</p>' }
  ];
  var CHATS = {
    claude: [
      { id: 'inv', proj: 'billing-api', title: 'Пагинация в /invoices', st: 'wait', pre: 'Спрашивает про ?page=', done: 3, live: true, meta: '14 мин', feed: INV_FEED,
        need: { kind: 'Вопрос', icon: 'ask', who: 'Исполнитель', text: 'Мобильный клиент ещё шлёт ?page=. Оставить старый параметр на переходный период или убрать сразу?', acts: [['Оставить page', 'human'], ['Убрать сразу', '']] } },
      { id: 'round', proj: 'billing-api', title: 'Округление в итогах', st: 'run', pre: 'Ворота: 12 из 18 тестов', done: 4, meta: '7 мин' },
      { id: 'how', proj: 'billing-api', title: 'Как устроен биллинг?', st: 'chat', pre: 'Три слоя: routes, service, repo',
        q: 'Как устроен биллинг? Коротко.', a: '<p>Три слоя: <code>routes</code> принимают запрос, <code>service</code> считает суммы и налоги, <code>repo</code> ходит в Postgres. Деньги хранятся в копейках, валюта в отдельном поле.</p>' },
      { id: 'swarm', proj: 'billing-api', title: 'Лимит вебхуков · рой', st: 'run', pre: 'Ревьюер советует вариант A', href: '#swarm' },
      { id: 'chlog', proj: 'billing-api', title: 'Changelog 2.4', st: 'queue', pre: 'Ждёт очереди слияния', done: 6 },
      { id: 'cur', proj: 'billing-api', title: 'Поле currency в payments', st: 'done', pre: 'Влито в main', done: 7 },
      { id: 'hero', proj: 'web-site', title: 'Герой на странице цен', st: 'wait', pre: 'Можно push и PR?', done: 6, live: true, meta: '22 мин',
        need: { kind: 'Разрешение', icon: 'key', who: 'Интегратор', text: 'Готово и проверено, ревьюер доволен. Отправить ветку ns/pricing-hero и открыть PR в main?', acts: [['Push и PR', 'human'], ['Только push', ''], ['Не надо', '']] } }
    ],
    codex: [
      { id: 'acc', proj: 'billing-api', title: 'customer_id → account_id', st: 'run', pre: 'Нет вывода 15 мин, толкнул', done: 3, live: true, meta: '31 мин' },
      { id: 'pf', proj: 'docs-tool', title: 'Поиск на Pagefind', st: 'fail', pre: 'Три попытки, тесты красные', done: 4, live: true, meta: '48 мин',
        need: { kind: 'Не вышло', icon: 'fail', bad: true, who: 'Исполнитель', text: 'Три попытки, тесты индексации красные: страницы с кириллицей в путях не попадают в индекс.', acts: [['Отдать Claude Code', 'human'], ['Дать подсказку', ''], ['Отложить', '']] } }
    ],
    gemini: [
      { id: 'img', proj: 'web-site', title: 'Сжатие картинок', st: 'run', pre: 'Исполнитель: 14 из 40 файлов', done: 3, live: true, meta: '9 мин' },
      { id: 'avif', proj: 'web-site', title: 'Что такое AVIF?', st: 'chat', pre: 'Формат картинок, меньше WebP',
        q: 'Что такое AVIF и стоит ли переходить?', a: '<p>Формат картинок на основе AV1. При том же качестве обычно легче WebP. Браузеры его поддерживают, но кодирование медленнее - для сборки сайта это не проблема.</p>' },
      { id: 'es', proj: 'web-site', title: 'Переводы на испанский', st: 'done', pre: 'Влито в main', done: 7 }
    ],
    opencode: [], cursor: [], qwen: [], custom: []
  };
  function needCount(k) { return CHATS[k].filter(function (c) { return c.need; }).length; }
  function findChat(id) {
    for (var i = 0; i < ORDER.length; i++) {
      var list = CHATS[ORDER[i]];
      for (var j = 0; j < list.length; j++) if (list[j].id === id) return { env: ORDER[i], chat: list[j] };
    }
    return null;
  }

  var state = { env: 'claude', chat: 'inv', open: null, tab: 'lanes', laneMode: 'solo', file: 0 };
  var empty = false; // «Новый чат»

  /* ---------- край окна ---------- */
  function renderRail() {
    $('#rail').innerHTML = ORDER.map(function (k) {
      var e = ENVS[k], n = needCount(k);
      return '<button class="env-b' + (k === state.env ? ' on' : '') + (e.missing ? ' off' : '') + '" data-env="' + k + '" title="' + e.n + (e.missing ? ' - не установлен' : '') + '">' +
        eic(k) + '<span>' + e.s + '</span>' + (n ? '<b class="bn">' + n + '</b>' : '') + '</button>';
    }).join('');
    $$('#rail .env-b').forEach(function (b) {
      b.onclick = function () {
        var k = b.dataset.env, first = CHATS[k].filter(function (c) { return !c.href; })[0];
        state.env = k; state.chat = first ? first.id : null; state.open = null; empty = false;
        if (location.hash === '#main') route(); else location.hash = '#main';
      };
    });
  }

  /* ---------- список чатов среды ---------- */
  function renderSide() {
    var e = ENVS[state.env];
    $('#sideIcon').innerHTML = eic(state.env);
    $('#sideName').textContent = e.n;
    $('#sideVer').textContent = e.missing ? 'не установлен' : e.custom ? '' : modelName(state.env);
    $('#newChat').hidden = !!(e.missing || e.custom);
    var list = CHATS[state.env];
    if (!list.length) {
      $('#sideTasks').innerHTML = '<p class="side-none">' + (e.missing ? 'Сначала установи среду.' : e.custom ? 'Сначала укажи команду.' : 'Чатов пока нет.') + '</p>';
      return;
    }
    var groups = {};
    list.forEach(function (c) { (groups[c.proj] = groups[c.proj] || []).push(c); });
    $('#sideTasks').innerHTML = Object.keys(groups).map(function (p) {
      return '<div class="pgroup"><h3>' + p + '</h3><ul class="tasks">' + groups[p].map(function (c) {
        var on = (location.hash === '#swarm' && c.href) || (!empty && location.hash !== '#swarm' && c.id === state.chat);
        var cls = c.need && c.need.bad ? 'fail' : c.st === 'chat' ? 'talk' : c.st;
        return '<li><a href="' + (c.href || '#c-' + c.id) + '" class="' + (on ? 'on' : '') + '"><svg class="ti ' + cls + '"><use href="#i-' + (c.href ? 'swarm' : ST[c.st][2]) + '"/></svg>' +
          '<div class="tx"><span>' + c.title + '</span><small>' + c.pre + '</small></div></a></li>';
      }).join('') + '</ul></div>';
    }).join('');
  }

  /* ---------- лента ---------- */
  function plural(n) { return n % 10 === 1 && n % 100 !== 11 ? 'действие' : (n % 10 >= 2 && n % 10 <= 4 && (n % 100 < 10 || n % 100 >= 20)) ? 'действия' : 'действий'; }
  var PROMPT = '<b>Цель.</b> Курсорная пагинация в GET /invoices (src/invoices/routes.py).<br><b>Ограничения.</b> Схему БД не менять, старый ?page= не ломать, стиль как в src/payments.<br><b>Готово, когда.</b> Новые тесты зелёные, mypy и ruff чистые, ответ содержит next_cursor.';
  function feedHtml(items, envKey, chat) {
    var out = '', buf = [];
    function flush() {
      if (!buf.length) return;
      var last = buf[buf.length - 1];
      out += '<details class="tools"><summary>' + ic('chev-r', 'tw') + '<span>' + buf.length + ' ' + plural(buf.length) + '</span><i>' +
        buf.map(function (t) { return t.c; }).join(', ') + '</i><em class="' + (last.bad ? 'no' : 'ok') + '">' + last.res + '</em></summary>' +
        buf.map(function (t) { return '<div class="tl"><span>' + t.c + ' ' + t.a + '</span><em class="' + (t.bad ? 'no' : 'ok') + '">' + t.res + '</em></div>'; }).join('') + '</details>';
      buf = [];
    }
    items.forEach(function (m) {
      if (m.t === 'tool') { buf.push(m); return; }
      flush();
      if (m.t === 'card') out += cardHtml(chat);
      else if (m.t === 'sys') out += '<div class="sysline">' + ic('check') + '<span>' + m.text + '</span>' + (m.prompt ? '<button type="button" class="linkb" data-prompt>показать промпт</button>' : '') + '</div>' + (m.prompt ? '<div class="promptbox" id="promptBox" hidden>' + PROMPT + '</div>' : '');
      else if (m.t === 'me') out += '<div class="me"><div><small>Ты' + (m.time ? ' · ' + m.time : '') + '</small>' + m.html + '</div></div>';
      else if (m.t === 'ai') out += '<div class="msg ai"><span class="av">' + eic(envKey) + '</span><div><div class="msg-h"><b>' + ENVS[envKey].n + '</b><small>' + eic(MV[envKey][ENVS[envKey].model], 'mlogo') + modelName(envKey) + '</small><time>' + (m.time || '') + '</time></div><div class="msg-b">' + m.html + '</div></div></div>';
      else {
        var who = m.who ? '<span class="sa">' + m.who + '</span>' : rl(m.r);
        out += '<div class="msg">' + who + '<div><div class="msg-h"><span class="rname r-' + m.r + '">' + (m.rn || ROLES[m.r]) + '</span><small>' + (m.e || '') + '</small><time>' + m.time + '</time></div><div class="msg-b">' + m.html + '</div></div></div>';
      }
    });
    flush();
    return out;
  }
  function defaultFeed(c) {
    if (c.st === 'chat') return [{ t: 'me', time: '', html: c.q }, { t: 'ai', time: '', html: c.a }];
    var f = [{ t: 'me', time: '', html: 'Сделай: ' + c.title.charAt(0).toLowerCase() + c.title.slice(1) }, { t: 'card' }];
    if (c.id === 'acc') f.push({ t: 'msg', r: 'watch', time: '14:21', html: '<p>Исполнитель 15 минут без вывода. Подтолкнул, жду. Если не оживёт за 5 минут - предложу сменить агента.</p>' });
    return f;
  }

  function cardHtml(c) {
    var st = ST[c.st], open = state.open === c.id, done = c.done || 0;
    var bar = STEPS7.map(function (s, i) {
      var cls = i < done || c.st === 'done' ? 'd' : i === done ? (c.st === 'wait' ? 'w' : c.st === 'fail' ? 'f' : c.st === 'queue' ? '' : 'r') : '';
      return '<li class="' + cls + '"><i></i><span>' + s + '</span></li>';
    }).join('');
    var need = '';
    if (c.need) {
      var n = c.need;
      need = '<div class="tc-need' + (n.bad ? ' bad' : '') + '"><div class="tc-need-h"><b>' + ic(n.icon) + n.kind + '</b><span>' + n.who + '</span></div><p>' + n.text + '</p><div class="acts">' +
        n.acts.map(function (a) { return '<button type="button" class="btn sm ' + a[1] + '" data-act="' + esc(a[0]) + '">' + a[0] + '</button>'; }).join('') +
        '<button type="button" class="linkb" data-own>своими словами</button></div></div>';
    } else if (c.answer) need = '<div class="tc-done">' + ic('check') + 'Ты: ' + esc(c.answer) + '. Агент продолжает.</div>';
    return '<article class="tcard' + (c.live && !open ? ' pinned' : '') + (open ? ' open' : '') + '" data-c="' + c.id + '"><header class="tc-h"><span class="st ' + st[1] + '">' + ic(st[2]) + st[0] + '</span><h2>' + c.title + '</h2>' +
      (c.meta ? '<small>' + c.meta + '</small>' : '') + '<button type="button" class="linkb tc-more" aria-expanded="' + open + '">' + (open ? 'Свернуть' : 'Подробнее') + '</button></header>' +
      '<ol class="tc-steps">' + bar + '</ol>' + need + (open ? detailsHtml() : '') + '</article>';
  }

  /* ---------- подробности задачи (раскрытие карточки) ---------- */
  var TABS = [['lanes', 'Дорожки'], ['changes', 'Изменения'], ['checks', 'Проверки'], ['details', 'Детали']];
  function detailsHtml() {
    var body = state.tab === 'lanes' ? lanesHtml() : state.tab === 'changes' ? filesHtml() : state.tab === 'checks' ? checksHtml() : factsHtml();
    return '<div class="tc-det"><div class="tabs" role="tablist">' + TABS.map(function (t) { return '<button role="tab" data-tab="' + t[0] + '" class="' + (t[0] === state.tab ? 'on' : '') + '">' + t[1] + '</button>'; }).join('') + '</div><div class="pane">' + body + '</div></div>';
  }
  var AXIS_MAX = 24;
  var LANES = {
    solo: [
      ['plan', 'Opus', [[0, 2.6, 'план', ''], [5.2, 6, '', '']]],
      ['crit', 'Sonnet', [[2.6, 5.2, '2 дыры', '']]],
      ['test', 'Sonnet', [[6, 9.4, '9 тестов', '']]],
      ['exec', 'Opus', [[9.4, 11.2, 'код', ''], [11.2, 14, 'ждёт', 'wait'], [14, 18.5, 'доделать', 'proj']]],
      ['gate', 'pytest · ruff · mypy', [[10.6, 11.4, '✕', 'fail'], [12.6, 13.4, '✕', 'fail'], [18.5, 19.8, '', 'proj']]],
      ['rev', 'Codex', [[19.8, 22, 'ревью', 'proj']]],
      ['integ', 'Sonnet', [[22, 24, 'влив', 'proj']]]
    ],
    swarm: [
      ['plan', 'Opus', [[0, 2.2, 'план', '']]],
      ['crit', 'Sonnet', [[2.2, 4.4, 'критика', '']]],
      ['test', 'Sonnet', [[4.4, 7.4, '14 тестов', '']]],
      ['exec', 'A · Claude Code', [[7.4, 16.4, 'Redis', '']]],
      ['exec', 'B · Codex', [[7.4, 14.6, 'в памяти', '']]],
      ['exec', 'C · Gemini', [[7.4, 18.6, 'middleware', 'fail']]],
      ['rev', 'Codex', [[19.8, 22, 'выбор', 'proj']]],
      ['integ', 'Sonnet', [[22, 24, 'влив', 'proj']]]
    ]
  };
  function lanesHtml() {
    var now = state.laneMode === 'solo' ? 14 : 18.8;
    var axis = '<div class="lane-axis"><div></div><div class="axis">';
    for (var m = 0; m <= AXIS_MAX; m += 4) axis += '<span style="left:' + (m / AXIS_MAX * 100) + '%">' + m + ' мин</span>';
    axis += '</div></div>';
    var rows = LANES[state.laneMode].map(function (l) {
      return '<div class="lane r-' + l[0] + '"><div class="lane-n">' + rl(l[0]) + '<div><b>' + ROLES[l[0]] + '</b><small>' + l[1] + '</small></div></div><div class="track">' + l[2].map(function (s) {
        var cls = s[3] === 'proj' ? ' proj' : s[3] === 'wait' ? ' waitx' : s[3] === 'fail' ? ' failx' : '';
        return '<div class="seg-b' + cls + '" style="left:' + (s[0] / AXIS_MAX * 100) + '%;width:' + ((s[1] - s[0]) / AXIS_MAX * 100) + '%">' + s[2] + '</div>';
      }).join('') + '</div></div>';
    }).join('');
    return '<div class="lanes-head"><div><b>Кто что делал</b><span class="hint">Пунктир - что ещё впереди</span></div><div class="seg"><button data-lm="solo" class="' + (state.laneMode === 'solo' ? 'on' : '') + '">Один исполнитель</button><button data-lm="swarm" class="' + (state.laneMode === 'swarm' ? 'on' : '') + '">Рой ×3</button></div></div>' +
      '<div class="lanes">' + axis + rows + '<div class="lane-now-wrap"><div class="now" style="left:' + (now / AXIS_MAX * 100) + '%"></div></div></div>' +
      '<div class="pipe-wrap"><div class="pipe-h"><b>Очередь слияния</b><span class="hint">По одному: ребейз, тесты, влив в main</span></div><div class="pipe"><div class="pipe-tube">' +
      '<span class="pitem ghost">Пагинация · после ревью</span><span class="pitem">Changelog 2.4</span><span class="pitem first">' + ic('run') + 'Округление в итогах</span></div><div class="pipe-main">' + ic('branch') + 'main</div></div></div>';
  }
  var FILES = [['src/invoices/routes.py', '+64 −12', 'exec'], ['src/invoices/cursor.py', '+41', 'exec'], ['tests/test_invoices_pagination.py', '+118', 'test'], ['docs/api.md', '+9 −2', 'exec']];
  var DIFFS = [
    [['c', '@@ def list_invoices(...)'], ['d', '    rows = db.query(Invoice).offset(page * 50).limit(50)'], ['a', '    after = decode_cursor(cursor) if cursor else None'], ['a', '    q = db.query(Invoice).order_by(Invoice.created_at, Invoice.id)'], ['a', '    if after: q = q.filter(tuple_(Invoice.created_at, Invoice.id) > after)'], ['a', '    rows = q.limit(min(limit, 200) + 1).all()'], ['n', '    return {"items": rows[:limit], "next_cursor": next_cursor}']],
    [['a', 'def encode_cursor(inv) -> str:'], ['a', '    raw = f"{inv.created_at.isoformat()}|{inv.id}"'], ['a', '    return sign(base64.urlsafe_b64encode(raw.encode()))']],
    [['a', 'def test_boundary_with_equal_created_at(client, seed_invoices):'], ['a', '    seed_invoices(count=5, same_timestamp=True)'], ['a', '    assert not ids(p1) & ids(p2)']],
    [['a', '### GET /invoices'], ['a', 'Параметр `cursor` вместо `page`.'], ['d', 'Параметр `page` (номер страницы).']]
  ];
  function filesHtml() {
    var sel = state.file;
    return '<div class="changes"><ul class="files">' + FILES.map(function (f, i) {
      return '<li class="' + (i === sel ? 'on' : '') + '" data-file="' + i + '">' + rl(f[2]) + '<span>' + f[0] + '</span><small>' + f[1] + '</small></li>';
    }).join('') + '</ul><div class="diff"><div class="dh">' + FILES[sel][0] + ' · ' + ROLES[FILES[sel][2]] + '</div>' + DIFFS[sel].map(function (l, i) {
      return '<div class="l ' + l[0] + '"><i>' + (i + 1) + '</i><span>' + (l[0] === 'a' ? '+ ' : l[0] === 'd' ? '− ' : '  ') + esc(l[1]) + '</span></div>';
    }).join('') + '</div></div>';
  }
  function checksHtml() {
    var C = [['ok', 'Сборка', 'python -m compileall src'], ['ok', 'Линтер', 'ruff · без замечаний'], ['ok', 'Типы', 'mypy · без ошибок'], ['bad', 'Тесты', '7 из 9. Падают: test_invalid_cursor_returns_400, test_limit_over_max'], ['wait', 'Ревью другим движком', 'Codex начнёт после зелёных ворот']];
    return '<ul class="checks">' + C.map(function (c) {
      var col = c[0] === 'ok' ? 'var(--ok)' : c[0] === 'bad' ? 'var(--bad)' : 'var(--ink3)';
      return '<li><svg class="ic" style="color:' + col + '"><use href="#i-' + (c[0] === 'ok' ? 'check' : c[0] === 'bad' ? 'fail' : 'dash') + '"/></svg><div><b>' + c[1] + '</b><small>' + c[2] + '</small></div><span class="hint">' + (c[0] === 'wait' ? 'впереди' : '14:13') + '</span></li>';
    }).join('') + '</ul>';
  }
  function factsHtml() {
    var R = [['plan', 'Opus', 'done', 'готово'], ['crit', 'Sonnet', 'done', 'готово'], ['test', 'Sonnet', 'done', '9 тестов'], ['exec', 'Opus · попытка 1 из 3', 'wait', 'ждёт тебя'], ['gate', 'pytest, ruff, mypy', 'run', '7 из 9'], ['rev', 'Codex, другой движок', 'idle', 'после ворот'], ['integ', 'Sonnet', 'idle', 'в очереди']];
    var I = { done: 'check', run: 'run', wait: 'ask', idle: 'dash' };
    return '<div class="details"><dl class="facts">' +
      '<div><dt>Ветка</dt><dd>' + ic('branch') + 'ns/invoices-pagination</dd></div><div><dt>Изменения</dt><dd><span class="plus">+184</span><span class="minus">−31</span> · 4 файла</dd></div><div><dt>Стоимость</dt><dd>$1.84 из $5</dd></div>' +
      '</dl><section><h2>Состав задачи</h2><ul class="roster">' + R.map(function (r) {
        return '<li class="r-' + r[0] + '">' + rl(r[0]) + '<span class="rn">' + ROLES[r[0]] + '</span><span class="re">' + r[1] + '</span><span class="rs ' + r[2] + '">' + ic(I[r[2]]) + r[3] + '</span></li>';
      }).join('') + '</ul></section></div>';
  }

  /* ---------- главный экран ---------- */
  function renderMain() {
    var e = ENVS[state.env], miss = $('#missing'), convo = $('#convo');
    miss.hidden = !(e.missing || e.custom); convo.hidden = !miss.hidden;
    if (e.missing) {
      miss.innerHTML = eic(state.env) + '<h2>' + e.n + ' не установлен</h2><p>Поставь CLI и войди в аккаунт - среда появится сама. Команда для примера:</p><pre>' + esc(e.install) + '</pre><button class="btn primary">Установить</button> <a class="btn" href="#settings-env-run">Указать путь вручную</a>';
      return;
    }
    if (e.custom) {
      miss.innerHTML = eic('custom') + '<h2>Своя команда</h2><p>Любая CLI, которая принимает промпт и пишет код в папке. {prompt} и {dir} подставим сами.</p><pre>my-agent --dir {dir} --prompt {prompt}</pre><a class="btn primary" href="#settings-env-run">Настроить команду</a>';
      return;
    }
    $('#envPickIcon').innerHTML = eic(state.env);
    $('#envPickText').textContent = e.n + ' · ' + modelName(state.env);
    $('#taskText').placeholder = 'Напиши ' + e.n + ': спроси или поручи. «Сделай…» станет задачей.';
    var c = empty ? null : CHATS[state.env].filter(function (x) { return x.id === state.chat; })[0];
    if (!c) {
      $('#convoTitle').textContent = 'Новый чат'; $('#convoSub').textContent = e.n + ' · ' + modelName(state.env);
      $('#cfeed').innerHTML = '<div class="newchat-empty">' + eic(state.env) + '<h2>Чат с ' + e.n + '</h2><p>Спроси что-нибудь про проект или опиши задачу. Задача получит карточку прямо в ленте, и агенты будут писать сюда же.</p></div>';
      return;
    }
    $('#convoTitle').textContent = c.title;
    $('#convoSub').textContent = c.proj;
    $('#cfeed').innerHTML = feedHtml(c.feed || (c.feed = defaultFeed(c)), state.env, c);
  }
  function toBottom() { var w = $('#feedWrap'); w.scrollTop = w.scrollHeight; }

  $('#cfeed').addEventListener('click', function (ev) {
    var t = ev.target.closest('button, li[data-file]'); if (!t) return;
    var card = t.closest('.tcard'), c = card && findChat(card.dataset.c).chat;
    if (t.hasAttribute('data-prompt')) { var b = $('#promptBox'); b.hidden = !b.hidden; t.textContent = b.hidden ? 'показать промпт' : 'скрыть промпт'; return; }
    if (!c) return;
    if (t.classList.contains('tc-more')) { state.open = state.open === c.id ? null : c.id; renderMain(); if (state.open) $('.tcard.open').scrollIntoView({ block: 'start' }); return; }
    if (t.dataset.tab) { state.tab = t.dataset.tab; renderMain(); return; }
    if (t.dataset.lm) { state.laneMode = t.dataset.lm; renderMain(); return; }
    if (t.dataset.file) { state.file = +t.dataset.file; renderMain(); return; }
    if (t.hasAttribute('data-own')) { var ta = $('#taskText'); ta.value = ''; ta.placeholder = 'Ответ на вопрос «' + c.title + '»…'; ta.focus(); return; }
    if (t.dataset.act) {
      c.answer = t.dataset.act; c.need = null; c.st = 'run'; c.pre = 'Ты ответил, агент продолжает';
      renderMain(); renderSide(); renderRail();
    }
  });

  /* поле чата */
  var TASK_RE = /^(сделай|добавь|почини|исправь|перепиши|напиши|убери|переведи|обнови)/i;
  $('#composer').addEventListener('submit', function () {
    var ta = $('#taskText'), v = ta.value.trim(); if (!v) { ta.focus(); return; }
    var list = CHATS[state.env], c = empty ? null : list.filter(function (x) { return x.id === state.chat; })[0];
    if (!c) {
      c = { id: 'n' + Date.now(), proj: 'billing-api', title: v.length > 40 ? v.slice(0, 38) + '…' : v, st: 'chat', pre: 'Только что', feed: [] };
      list.unshift(c); state.chat = c.id; empty = false;
    }
    c.feed.push({ t: 'me', time: 'сейчас', html: esc(v) });
    if (TASK_RE.test(v)) {
      var t = { id: 'n' + Date.now() + 't', proj: c.proj, title: v.length > 48 ? v.slice(0, 46) + '…' : v, st: 'run', done: 0, live: true, meta: 'только что', pre: 'Планировщик разбирает' };
      list.push(t); c.feed.push({ t: 'card', ref: t.id });
      c.st = 'run'; c.pre = 'Планировщик разбирает'; c.done = 0; c.live = true; c.meta = 'только что'; c.title = c.title;
      c.feed.push({ t: 'sys', text: 'Это задача. Переписал в промпт, запускаю шаги качества.' });
      list.pop();
    } else c.feed.push({ t: 'ai', time: 'сейчас', html: '<p><span class="live"></span>Смотрю код, сейчас отвечу…</p>' });
    ta.value = ''; renderMain(); renderSide(); toBottom();
  });
  $('#taskText').addEventListener('keydown', function (e) { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); $('#sendBtn').click(); } });
  $('#newChat').onclick = function () { empty = true; state.open = null; renderMain(); renderSide(); $('#taskText').focus(); };

  /* выбор модели */
  function modelsHtml(k, attr) {
    var e = ENVS[k];
    return e.models.map(function (m, i) {
      return '<button type="button" class="mcard' + (i === e.model ? ' on' : '') + '" ' + attr + '="' + i + '" role="radio" aria-checked="' + (i === e.model) + '"><span class="radio"></span><b>' + eic(MV[k][i], 'mlogo') + m[0] + (i === e.def ? '<small>по умолчанию</small>' : '') + '</b><span class="d">' + m[1] + '</span></button>';
    }).join('');
  }
  function renderPop() {
    $('#popEnv').textContent = ENVS[state.env].n;
    $('#popModels').innerHTML = modelsHtml(state.env, 'data-mi');
    $$('#popModels .mcard').forEach(function (b) {
      b.onclick = function () { ENVS[state.env].model = +b.dataset.mi; togglePop($('#envPick'), $('#modelPop'), false); renderMain(); renderSide(); };
    });
  }
  function togglePop(btn, pop, open) {
    if (open === undefined) open = pop.hidden;
    $$('.composer .pop').forEach(function (p) { p.hidden = true; });
    $$('#envPick, #tune').forEach(function (b) { b.setAttribute('aria-expanded', 'false'); });
    pop.hidden = !open; btn.setAttribute('aria-expanded', String(open));
    if (open && pop.id === 'modelPop') renderPop();
  }
  $('#envPick').onclick = function () { togglePop($('#envPick'), $('#modelPop')); };
  $('#tune').onclick = function () { togglePop($('#tune'), $('#tunePanel')); };
  document.addEventListener('keydown', function (e) { if (e.key === 'Escape') $$('.composer .pop').forEach(function (p) { p.hidden = true; }); });

  /* Качество и Рой: тихие переключатели */
  $$('.composer .sw').forEach(function (sw) {
    sw.addEventListener('click', function (e) { e.preventDefault(); sw.classList.toggle('on'); sw.setAttribute('aria-checked', sw.classList.contains('on')); });
    sw.parentNode.addEventListener('click', function (e) { if (e.target === sw.parentNode) sw.click(); });
  });
  var STEPS = [['Спека', 'только для расплывчатых задач', true], ['План и критик плана', '', true], ['Тесты сначала', 'исполнителю их не трогать', true], ['Ворота', 'всегда', true, true], ['Ревьюер другим движком', '', true], ['e2e как пользователь', 'только для интерфейсов', false], ['Best-of-N для рискованного', 'деньги, данные, миграции', true]];
  $('#qSteps').innerHTML = STEPS.map(function (s) {
    return '<li><label class="step"><input type="checkbox"' + (s[2] ? ' checked' : '') + (s[3] ? ' disabled' : '') + '><b>' + s[0] + '</b>' + (s[1] ? '<span>' + s[1] + '</span>' : '') + '</label></li>';
  }).join('');
  var swarmN = 3;
  $('#nMinus').onclick = function () { swarmN = Math.max(2, swarmN - 1); $('#nVal').textContent = swarmN; };
  $('#nPlus').onclick = function () { swarmN = Math.min(5, swarmN + 1); $('#nVal').textContent = swarmN; };
  $$('#modes .mode').forEach(function (b) { b.onclick = function () { $$('#modes .mode').forEach(function (x) { x.classList.remove('on'); }); b.classList.add('on'); }; });

  /* ---------- рой ---------- */
  var VARS = [
    { l: 'A', v: 'anthropic', n: 'Claude Code · Opus', st: 'Лучший', ok: true, best: true, sum: 'Скользящее окно в Redis, проверка и запись атомарно. Переживает рестарт.', score: 8.6, tests: '14 из 14', c: [['Корректность', 9], ['Покрытие', 8], ['Простота', 8], ['Риск', 9]], more: '+96 −8 · 9 мин · $0.92' },
    { l: 'B', v: 'openai', n: 'Codex · gpt-5-codex', st: 'Прошёл', ok: true, sum: 'Бакет в памяти процесса. Проще, но в каждом воркере свой счётчик.', score: 7.1, tests: '14 из 14', c: [['Корректность', 6], ['Покрытие', 8], ['Простота', 9], ['Риск', 5]], more: '+71 −5 · 7 мин · $0.61' },
    { l: 'C', v: 'gemini', n: 'Gemini · 2.5 Pro', st: 'Не прошёл', ok: false, sum: 'Отдельный middleware. Два теста на границе окна падают.', score: 4.2, tests: '12 из 14', c: [['Корректность', 4], ['Покрытие', 5], ['Простота', 3], ['Риск', 5]], more: '+143 −12 · 11 мин · $0.48' }
  ];
  $('#variants').innerHTML = VARS.map(function (v) {
    return '<article class="var' + (v.best ? ' best' : '') + '"><div class="var-h"><span class="var-l">' + v.l + '</span>' + eic(v.v) + '<b>' + v.n + '</b><span class="st ' + (v.ok ? 'st-ok' : 'st-bad') + '">' + ic(v.ok ? 'check' : 'fail') + v.st + '</span></div>' +
      '<p class="var-sum">' + v.sum + '</p><div class="var-nums"><div><b>' + v.score.toFixed(1) + '</b><small>балл ревьюера</small></div><div><b>' + v.tests + '</b><small>тесты</small></div></div>' +
      '<details><summary>Почему такой балл</summary>' + v.c.map(function (c) { return '<div class="crit"><span>' + c[0] + '</span><div class="bar"><i style="width:' + c[1] * 10 + '%;background:' + (c[1] >= 7 ? 'var(--ok)' : c[1] >= 5 ? 'var(--human)' : 'var(--bad)') + '"></i></div><b>' + c[1] + '</b></div>'; }).join('') + '<small>' + v.more + '</small></details>' +
      '<div class="var-f">' + (v.ok ? '<button class="btn ' + (v.best ? 'primary' : '') + '">Взять ' + v.l + '</button>' : '') + '<button class="btn">Дифф</button></div></article>';
  }).join('');
  $('#swarmChat').innerHTML = feedHtml([
    { t: 'msg', r: 'plan', e: 'Opus', time: '11:40', html: '<p>Одна спека на троих: 100 запросов в минуту на ключ вебхука, ответ 429 с Retry-After.</p>' },
    { t: 'msg', r: 'exec', who: 'A', rn: 'Исполнитель A', e: 'Claude Code', time: '11:49', html: '<p>Взял Redis: он уже в стеке, счётчик общий для воркеров.</p>' },
    { t: 'tool', c: 'run', a: 'pytest tests/test_webhook_limit.py', res: '14 passed' },
    { t: 'msg', r: 'exec', who: 'B', rn: 'Исполнитель B', e: 'Codex', time: '11:47', html: '<p>Без внешних зависимостей. При нескольких воркерах лимит умножается.</p>' },
    { t: 'msg', r: 'exec', who: 'C', rn: 'Исполнитель C', e: 'Gemini', time: '11:51', html: '<p>Два теста на границе окна падают. Чинить или сдавать как есть?</p>' },
    { t: 'msg', r: 'rev', e: 'Codex', time: '11:55', html: '<p>A единственная устойчива при рестарте. C снимаю с гонки.</p>' },
    { t: 'me', time: '11:56', html: 'Ок, A. Пусть добавит тест с двумя воркерами.' }
  ], 'claude');
  $$('#swTabs button').forEach(function (b) {
    b.onclick = function () { $$('#swTabs button').forEach(function (x) { x.classList.toggle('on', x === b); }); $('#variants').hidden = b.dataset.w !== 'v'; $('#swarmChatWrap').hidden = b.dataset.w !== 'c'; };
  });
  $('#pickA').onclick = function () { this.textContent = 'Берём A'; this.disabled = true; };

  /* ---------- фабрика: все среды ---------- */
  $('#strip').innerHTML = [['coord', 'Координатор', 'работает', 'var(--exec)'], ['exec', 'Исполнители', '3, один завис', 'var(--bad)'], ['rev', 'Ревьюер', 'отдыхает', 'var(--ink3)'], ['watch', 'Надзиратель', 'следит', 'var(--exec)'], ['queue', 'Очередь', '2 задачи', 'var(--ink2)']].map(function (r) {
    return '<span class="r-' + r[0] + '">' + rl(r[0]) + '<b>' + r[1] + '</b><em style="color:' + r[3] + '">' + r[2] + '</em></span>';
  }).join('');
  var CONVOYS = [
    { n: 'Подготовить релиз 2.4', left: 'осталось ~40 мин', p: ['d', 'd', 'd', 'd', 'r', 'r', 'w', ''], items: [['claude', 'Пагинация в /invoices', 'ask'], ['codex', 'customer_id → account_id', 'run'], ['claude', 'Округление в итогах', 'run'], ['claude', 'Changelog 2.4', 'dash']] },
    { n: 'Редизайн страницы цен', left: 'ждёт разрешения', p: ['d', 'd', 'd', 'w'], items: [['gemini', 'Сжатие картинок', 'run'], ['claude', 'Push и PR: ns/pricing-hero', 'key']] },
    { n: 'Починить поиск в доках', left: 'застряло', p: ['d', 'd', 'f', ''], items: [['codex', 'Pagefind и кириллица', 'fail']] }
  ];
  $('#convoys').innerHTML = CONVOYS.map(function (c) {
    return '<article class="cv"><div class="cv-h"><b>' + c.n + '</b><small>' + c.left + '</small></div><div class="prog">' + c.p.map(function (x) { return '<i class="' + x + '"></i>'; }).join('') + '</div><ul>' +
      c.items.map(function (i) { return '<li>' + eic(i[0]) + '<span>' + i[1] + '</span>' + ic(i[2]) + '</li>'; }).join('') + '</ul></article>';
  }).join('');
  var FEED = [
    { time: '14:21', r: 'watch', env: 'codex', html: '<b>Завис 15 мин.</b> Исполнитель на «customer_id → account_id».', prob: 'bad', acts: true },
    { time: '14:19', r: 'exec', env: 'claude', html: 'Вопрос про старый ?page=, ответ в чате задачи.', prob: 'warn' },
    { time: '14:16', r: 'queue', env: 'claude', html: 'Влито в main: <b>Поле currency в payments</b>.' },
    { time: '14:13', r: 'gate', env: 'claude', html: 'Пагинация: тесты 7 из 9.' },
    { time: '14:02', r: 'coord', env: 'claude', html: '«Подготовить релиз 2.4» разбито на 8 задач.' },
    { time: '13:58', r: 'rev', env: 'codex', html: 'Bump axios: ревью пройдено.' }
  ];
  function renderFeed(f) {
    $('#feed').innerHTML = FEED.filter(function (e) { return f === 'all' || e.prob; }).map(function (e) {
      return '<li class="' + (e.prob ? 'prob ' + (e.prob === 'warn' ? 'warn' : '') : '') + '"><time>' + e.time + '</time>' + rl(e.r) + '<div class="ev">' + e.html + '<small>' + eic(e.env) + ENVS[e.env].s + '</small>' +
        (e.acts ? '<div class="acts"><button class="btn sm primary" data-nudge>Подтолкнуть</button><button class="btn sm">Отдать другой среде</button></div>' : '') + '</div></li>';
    }).join('');
    $$('[data-nudge]').forEach(function (b) { b.onclick = function () { b.textContent = 'Толкнул, жду…'; b.disabled = true; }; });
  }
  $$('#feedFilter button').forEach(function (b) {
    b.onclick = function () { $$('#feedFilter button').forEach(function (x) { x.classList.remove('on'); }); b.classList.add('on'); renderFeed(b.dataset.f); };
  });
  renderFeed('all');

  /* ---------- настройки: сначала текущая среда, потом общее ---------- */
  function sw(on) { return '<span class="sw' + (on ? ' on' : '') + '" role="switch" tabindex="0" aria-checked="' + !!on + '"><i></i></span>'; }
  function num(v, u) { return '<input type="number" value="' + v + '" aria-label="значение"><span class="unit">' + u + '</span>'; }
  function sel(opts, cur) { return '<select>' + opts.map(function (o) { return '<option' + (o === cur ? ' selected' : '') + '>' + o + '</option>'; }).join('') + '</select>'; }
  function row(t, d, c) { return '<div class="srow"><div><b>' + t + '</b>' + (d ? '<span class="d">' + d + '</span>' : '') + '</div><div class="ctl">' + c + '</div></div>'; }
  function grp(t, rows) { return '<div class="sgrp">' + (t ? '<h2>' + t + '</h2>' : '') + '<div class="srows">' + rows.join('') + '</div></div>'; }
  var ENV_NAV = [['env-models', 'Модели'], ['env-perms', 'Права и песочница'], ['env-run', 'Запуск и вход']];
  var COMMON_NAV = [['quality', 'Качество'], ['swarm', 'Рой и фабрика'], ['limits', 'Лимиты и бюджет'], ['notify', 'Уведомления'], ['look', 'Внешний вид'], ['keys', 'Горячие клавиши']];
  function envSettings(id) {
    var k = state.env, e = ENVS[k];
    if (id === 'env-models') return ['Модели', 'Модель по умолчанию для чата и задач в ' + e.n + '. Названия - пример: список приложение берёт из самой CLI.', [
      '<div class="sgrp"><h2>По умолчанию</h2><div class="models" id="setModels">' + modelsHtml(k, 'data-smi') + '</div></div>',
      grp('Для ролей', [
        row('Планировщик и критик', 'Думают, кода не пишут.', sel(e.models.map(function (m) { return m[0]; }), e.models[0][0])),
        row('Исполнитель', 'Пишет код.', sel(e.models.map(function (m) { return m[0]; }), modelName(k))),
        row('Ревьюер', 'Лучше из другой среды: другие слепые зоны.', sel(['Codex · gpt-5-codex', 'Claude Code · Opus', 'Gemini · 2.5 Pro'], k === 'codex' ? 'Claude Code · Opus' : 'Codex · gpt-5-codex'))
      ])]];
    if (id === 'env-perms') return ['Права и песочница', 'Что ' + e.n + ' может делать на твоём компьютере без тебя.', [
      (e.sandbox === false ? '<div class="warnbox">' + ic('warn') + '<p><b>' + e.n + ' работает без песочницы.</b> Ограничен папкой worktree, но ходит в сеть и читает домашний каталог. Рискованное лучше отдать Claude Code или Codex.</p></div>' : ''),
      grp('Сейчас', [
        row('Песочница', '', e.sandbox ? '<span class="pill-s ok">' + ic('check') + 'включена</span>' : '<span class="pill-s no">' + ic('warn') + 'нет защиты</span>'),
        row('Сеть', '', '<span>' + (e.net || '-') + '</span>'),
        row('Файлы вне worktree', '', '<span>' + (e.files || '-') + '</span>')
      ]),
      grp('Что требует твоего слова', [
        row('Спрашивать перед push', 'Ветка уходит на GitHub только после «да».', sw(true)),
        row('Спрашивать перед открытием PR', '', sw(true)),
        row('Защищённые пути', '.env, ~/.ssh, ключи и токены не читать и не коммитить.', sw(true))
      ])]];
    return ['Запуск и вход', 'Как Nightshift находит и запускает ' + e.n + '.', [
      grp('', [
        row('Путь к CLI', 'Нашли сами. Поменяй, если стоит несколько версий.', '<input type="text" class="wide" value="' + (e.missing ? '' : '/usr/local/bin/' + k) + '" placeholder="не найден">'),
        row('Вход', '', e.missing ? '<button class="btn sm">Установить</button>' : '<span class="pill-s ok">' + ic('check') + 'вошёл</span>'),
        row('Дополнительные аргументы', 'Добавляются к каждому запуску.', '<input type="text" class="wide" placeholder="например --verbose">')
      ])]];
  }
  var SET = {
    quality: ['Качество', 'Шаги по умолчанию для всех сред. В чате их можно поменять кнопкой «настроить» у поля.', [
      grp('Конвейер', [row('Спека для неясных задач', 'Агент сам решает, расплывчата ли формулировка.', sw(true)), row('План и критик плана', 'Пропускается для правок в одну фразу.', sw(true)), row('Тесты сначала', 'Исполнителю тесты править нельзя.', sw(true)), row('Ворота', 'Сборка, тесты, линтер, типы. Отключить нельзя.', '<span class="pill-s ok">' + ic('check') + 'всегда</span>'), row('e2e как пользователь', 'Только для проектов с интерфейсом.', sw(false))]),
      grp('Ревью', [row('Кругов доработки', 'Дальше - к тебе.', num(3, 'макс.')), row('Best-of-N для рискованного', 'Деньги, безопасность, миграции.', sw(true) + num(2, 'варианта'))])]],
    swarm: ['Рой и фабрика', 'Сколько агентов работает сразу и что делать, если что-то пошло не так.', [
      grp('Параллельность', [row('Агентов одновременно', 'Остальные ждут в очереди.', num(3, 'шт.')), row('Режим роя', '', sel(['Соревнование', 'Бригада', 'Совет'], 'Соревнование')), row('Агентов в рое', 'Больше трёх редко окупается.', num(3, 'шт.'))]),
      grp('Если не идёт', [row('Потолок попыток', 'Потом - «не вышло» и к тебе.', num(3, 'попытки')), row('Завис, если нет вывода', '', num(15, 'мин')), row('Выбирать лучший вариант сам', 'Если ты не ответил.', sw(true) + num(10, 'мин'))])]],
    limits: ['Лимиты и бюджет', 'Ни одна задача не превысит потолок.', [
      grp('Бюджет', [row('На одну задачу', 'Дойдя до потолка, агент спрашивает.', '<span class="unit">$</span>' + num(5, '')), row('В день', 'По всем средам.', '<span class="unit">$</span>' + num(40, '')), row('Предупреждать на', '', num(80, '%'))]),
      grp('Сегодня', [row('Потрачено $12.40', 'Claude Code $8.10 · Codex $3.20 · Gemini $1.10', '<div class="bar" style="width:200px"><i style="width:31%"></i></div>')])]],
    notify: ['Уведомления', 'Тишина по умолчанию. Звук - только когда нужен ты.', [
      grp('Когда звать', [row('Вопрос от агента', '', sw(true)), row('Не вышло', '', sw(true)), row('Разрешение на push и PR', '', sw(true)), row('Задача готова', 'Готовое подождёт.', sw(false))]),
      grp('Как', [row('Звук', '', sel(['Тихий щелчок', 'Колокольчик', 'Без звука'], 'Тихий щелчок')), row('Тихие часы', 'Копится без звука.', '<input type="text" value="23:00" style="width:64px"><span class="unit">до</span><input type="text" value="08:00" style="width:64px">')])]],
    look: ['Внешний вид', '', [grp('', [row('Тема', '', '<div class="seg" id="themeSet"><button data-theme="auto">Система</button><button data-theme="dark">Тёмная</button><button data-theme="light">Светлая</button></div>'), row('Размер шрифта', '', num(13, 'px')), row('Подписи у значков ролей', 'Форма и цвет различают роли, подпись добавляет ясности.', sw(true))])]],
    keys: ['Горячие клавиши', '', [grp('', [row('Новый чат из любого места', '', '<span class="keys"><span>⌥</span><span>⌘</span><span>N</span></span>'), row('Следующая среда', '', '<span class="keys"><span>⌘</span><span>1…7</span></span>'), row('Отправить', '', '<span class="keys"><span>↵</span></span>'), row('Заморозить всех агентов', '', '<span class="keys"><span>⌥</span><span>⌘</span><span>.</span></span>')])]]
  };
  var curSet = 'env-models';
  function renderSettings(id) {
    curSet = id || curSet;
    var k = state.env, e = ENVS[k];
    function link(n) { return '<a href="#settings-' + n[0] + '" class="' + (n[0] === curSet ? 'on' : '') + '">' + n[1] + '</a>'; }
    $('#snav').innerHTML = '<h2>' + eic(k) + e.n + '</h2>' + ENV_NAV.map(link).join('') + '<h2>Для всех сред</h2>' + COMMON_NAV.map(link).join('');
    var s = curSet.indexOf('env-') === 0 ? envSettings(curSet) : SET[curSet];
    var isEnv = curSet.indexOf('env-') === 0;
    $('#sbody').innerHTML = '<h1>' + (isEnv ? eic(k) : '') + s[0] + '</h1>' + (s[1] ? '<p class="lead">' + s[1] + '</p>' : '') + s[2].join('');
    $$('.sw', $('#sbody')).forEach(function (el) { el.onclick = function () { el.classList.toggle('on'); el.setAttribute('aria-checked', el.classList.contains('on')); }; });
    $$('#setModels .mcard').forEach(function (b) { b.onclick = function () { e.model = e.def = +b.dataset.smi; renderSettings(); }; });
    $$('#themeSet button').forEach(function (b) { b.onclick = function () { setTheme(b.dataset.theme); }; });
    syncThemeUi();
  }

  /* ---------- первый запуск ---------- */
  var onbEnv = 'claude';
  function renderOnb() {
    $('#onbEnvs').innerHTML = ORDER.map(function (k) {
      var e = ENVS[k];
      var stx = e.custom ? 'любая CLI' : e.missing ? 'не найден' : 'найден, вошёл';
      return '<button type="button" class="envcard' + (k === onbEnv ? ' on' : '') + (e.missing ? ' off' : '') + '" data-k="' + k + '">' + eic(k) + '<b>' + e.n + '</b><small class="' + (e.missing || e.custom ? '' : 'ok') + '">' + stx + '</small></button>';
    }).join('');
    $('#onbEnvName').textContent = ENVS[onbEnv].n + ' · названия - пример, список берём из CLI';
    $('#onbModels').innerHTML = modelsHtml(onbEnv, 'data-om');
    $$('#onbEnvs .envcard').forEach(function (b) { b.onclick = function () { onbEnv = b.dataset.k; renderOnb(); }; });
    $$('#onbModels .mcard').forEach(function (b) { b.onclick = function () { ENVS[onbEnv].model = +b.dataset.om; renderOnb(); }; });
  }

  /* ---------- тема ---------- */
  var theme = 'auto';
  function setTheme(t) {
    theme = t;
    if (t === 'auto') document.documentElement.removeAttribute('data-theme'); else document.documentElement.setAttribute('data-theme', t);
    syncThemeUi();
  }
  function syncThemeUi() { $$('#themeSeg button, #themeSet button').forEach(function (b) { b.classList.toggle('on', b.dataset.theme === theme); }); }
  $$('#themeSeg button').forEach(function (b) { b.onclick = function () { setTheme(b.dataset.theme); }; });

  /* ---------- роутер: #экран~флаги (light, dark, m - модели, q - настроить, имя среды) ---------- */
  var SCREENS = ['main', 'swarm', 'factory', 'settings', 'empty'];
  function route() {
    var parts = (location.hash || '#main').slice(1).split('~'), h = parts[0], flags = parts.slice(1);
    flags.forEach(function (f) {
      if (f === 'light' || f === 'dark') setTheme(f);
      if (ENVS[f]) { state.env = f; var first = CHATS[f].filter(function (c) { return !c.href; })[0]; state.chat = first ? first.id : null; }
    });
    var screen = 'main', setId = null, navKey = 'main';
    if (h.indexOf('c-') === 0) { var fc = findChat(h.slice(2)); if (fc) { state.env = fc.env; state.chat = fc.chat.id; empty = false; state.open = null; } }
    else if (h === 'lanes' || h === 'changes' || h === 'checks' || h === 'details') { state.env = 'claude'; state.chat = 'inv'; state.open = 'inv'; state.tab = h; navKey = 'lanes'; }
    else if (h === 'swarm' || h === 'factory' || h === 'empty') screen = navKey = h;
    else if (h.indexOf('settings') === 0) { screen = navKey = 'settings'; setId = h.slice(9) || null; }
    else if (h === 'main') state.open = null;
    if (flags.indexOf('m') >= 0) navKey = 'model';
    SCREENS.forEach(function (s) { $('#screen-' + s).hidden = s !== screen; });
    document.body.classList.toggle('is-empty', screen === 'empty');
    document.body.classList.toggle('no-side', screen === 'factory' || screen === 'settings');
    renderRail(); renderSide();
    $$('.rail .env-b').forEach(function (b) { if (screen !== 'main' && screen !== 'swarm') b.classList.remove('on'); });
    $('#railFactory').classList.toggle('on', screen === 'factory');
    $('#railSettings').classList.toggle('on', screen === 'settings');
    if (screen === 'main') {
      $$('.composer .pop').forEach(function (p) { p.hidden = true; });
      renderMain();
      if (state.open) { var o = $('.tcard.open'); if (o) o.scrollIntoView({ block: 'start' }); } else toBottom();
      if (flags.indexOf('m') >= 0) togglePop($('#envPick'), $('#modelPop'), true);
      if (flags.indexOf('q') >= 0) togglePop($('#tune'), $('#tunePanel'), true);
    }
    if (screen === 'settings') renderSettings(setId);
    if (screen === 'empty') renderOnb();
    $$('#mocknav a').forEach(function (a) { a.classList.toggle('on', a.dataset.s === navKey); });
  }
  window.addEventListener('hashchange', route);
  route();
})();
