// Fake Tauri backend for viewing the UI in a plain browser. Loaded by mock-loader.js only outside Tauri (NSLogic.mockWanted).
// Command and event names follow interfaces.md ("Границы v2", row `app`); payload shapes are the ones js/ expects (see report).
// Query: ?scenario=full|empty|nogit  ?theme=dark|light  ?lang=ru|en  ?env=<id>  ?chat=<id|new>  ?open=<taskId>  ?pop=model|tune  ?limits=exhausted
(function () {
  const q = new URLSearchParams(location.search);
  if (q.get('theme')) document.documentElement.setAttribute('data-theme', q.get('theme'));
  const scenario = q.get('scenario') || 'full';
  if (q.get('away')) { try { localStorage.setItem('ns.leftAt', String(Date.now() - 3 * 3600000)); } catch (e) { /* the banner is simply not shown */ } }   // the window was left 3 hours ago
  const now = Date.now();
  const iso = (ms) => new Date(ms).toISOString();
  const ago = (min) => iso(now - min * 60000);
  const today = (h, m) => { const d = new Date(); d.setHours(h, m, 0, 0); return iso(d.getTime()); };
  const nextAt = (h, m, addDays) => { const d = new Date(); d.setHours(h, m, 0, 0); if (d <= now || addDays) d.setDate(d.getDate() + (addDays || 1)); return iso(d.getTime()); };

  // ---------- data ----------
  // the mock plays the core: get_settings returns every value, a key that is missing in set_settings reads as its default (serde(default))
  const mac = (q.get('platform') || 'mac') === 'mac';
  const DEFAULTS = { max_parallel: 5, paused: false, language: q.get('lang') || 'auto', hotkey: 'Alt+Space', custom_agents: [], default_project: null, envs: {}, stall_min: 15, autopick_min: 10, attempts: 3, review_rounds: 3, keep_awake: true,
    quality: { on: true, spec: true, plan: true, tests_first: true, e2e: false, best_of_n: 2 }, swarm_default: { mode: 'race', n: 3 },
    limits: { task_usd: 5, day_usd: 40, warn_pct: 80, night_from: '23:00', night_to: '08:00', away_min: 20 }, notify: { sound: 'soft_click', on_done: false, quiet_from: '23:00', quiet_to: '08:00' },
    look: { theme: 'system', font_px: 13, role_labels: true }, keys: { new_chat: mac ? 'Alt+Cmd+N' : 'Ctrl+Alt+N', switch_env: mac ? 'Cmd+{n}' : 'Ctrl+{n}', send: 'Enter', freeze: mac ? 'Alt+Cmd+.' : 'Ctrl+Alt+.' },
    keep: { archive_days: 7, log_events: 500, chat_records: 200, summary_after_min: 30 } };
  const ENV_DEFAULTS = { default_model: null, roles: {}, fallback: null, custom_models: [], cli_path: null, extra_args: [], ask_push: true, ask_pr: true, protected_paths: true };
  const withDefaults = (st) => { const o = JSON.parse(JSON.stringify(DEFAULTS)); for (const k of Object.keys(st)) o[k] = (st[k] && typeof st[k] === 'object' && !Array.isArray(st[k]) && DEFAULTS[k] && k !== 'envs') ? Object.assign({}, DEFAULTS[k], st[k]) : st[k];
    o.envs = {}; for (const [id, b] of Object.entries(st.envs || {})) o.envs[id] = Object.assign({}, ENV_DEFAULTS, b); return o; };
  let settings = withDefaults({ language: q.get('lang') || 'auto', look: { theme: q.get('theme') || 'system', font_px: q.get('font') ? Number(q.get('font')) : 13, role_labels: q.get('labels') !== 'off' } });
  const envs = [
    { id: 'claude', name: 'Claude Code', installed: true, version: '2.1.4', kit_dir: '~/.claude', path: '/usr/local/bin/claude', sandbox: { sandboxed: true, push_blocked: true, network_open: false, writes_outside_copy: false, note: 'песочница среды включена, git push запрещён' } },
    { id: 'codex', name: 'Codex', installed: true, version: '0.48', kit_dir: '~/.codex', path: '/usr/local/bin/codex', sandbox: { sandboxed: true, push_blocked: false, network_open: false, writes_outside_copy: false, note: '' } },
    { id: 'gemini', name: 'Gemini', installed: true, version: '0.9', path: '/opt/homebrew/bin/gemini', sandbox: { sandboxed: false, push_blocked: false, network_open: true, writes_outside_copy: true, note: '' } },
    { id: 'opencode', name: 'opencode', installed: true, version: '0.15', path: '/usr/local/bin/opencode', sandbox: { sandboxed: false, push_blocked: false, network_open: true, writes_outside_copy: true, note: '' } },
    { id: 'cursor', name: 'Cursor', installed: false, install_hint: 'curl https://cursor.com/install -fsS | bash' },
    { id: 'qwen', name: 'Qwen Code', installed: false, install_hint: 'npm install -g @qwen-code/qwen-code' },
    { id: 'custom:mine', name: 'Своя', custom: true, configured: false },
  ];
  const F = (id, label, vendor, versions, blurb, def) => ({ id, label, vendor, versions, blurb, default: !!def });
  const catalogs = {
    claude: [F('opus', 'Opus', 'anthropic', ['5.5', '4.8', '4.5'], 'Сильнее всех и дороже. Сложное и рискованное.'), F('sonnet', 'Sonnet', 'anthropic', ['5.5', '4.6', '4.5'], 'Баланс скорости и качества. Подходит почти всегда.', true), F('haiku', 'Haiku', 'anthropic', ['5.5', '4.5'], 'Быстро и дёшево. Мелкие правки.')],
    codex: [F('gpt-codex', 'GPT-Codex', 'openai', ['5.1', '5'], 'Заточена под код. Хороший выбор по умолчанию.', true), F('gpt', 'GPT', 'openai', ['5.1', '5'], 'Глубже рассуждает, медленнее.'), F('gpt-mini', 'GPT mini', 'openai', ['5.1', '5'], 'Быстро и дёшево.')],
    gemini: [F('pro', 'Gemini Pro', 'google', ['3', '2.5'], 'Сильнее, очень длинный контекст.', true), F('flash', 'Gemini Flash', 'google', ['3', '2.5'], 'Быстро и дёшево.')],
    opencode: [F('sonnet', 'Sonnet', 'anthropic', ['5.5', '4.6'], 'Через твой ключ Anthropic.', true), F('opus', 'Opus', 'anthropic', ['5.5', '4.8'], 'Через твой ключ Anthropic.'), F('haiku', 'Haiku', 'anthropic', ['5.5'], 'Быстро и дёшево.'),
      F('gpt', 'GPT', 'openai', ['5.1', '5'], 'Через твой ключ OpenAI.'), F('gpt-mini', 'GPT mini', 'openai', ['5.1'], 'Быстро и дёшево.'), F('gemini-pro', 'Gemini Pro', 'google', ['3'], 'Через твой ключ Google.'),
      F('gemini-flash', 'Gemini Flash', 'google', ['3'], 'Быстро и дёшево.'), F('qwen-coder', 'Qwen Coder', 'qwen', ['3', '2.5'], 'Открытая модель под код.'), F('llama', 'Llama', 'local', [], 'Ollama на этом компьютере, бесплатно.'),
      F('deepseek', 'DeepSeek', 'deepseek', ['3'], 'Через твой ключ DeepSeek.')],
  };
  const exhausted = q.get('limits') === 'exhausted';
  const W = (name, pct, resetsAt, extra) => Object.assign({ name, model: null, used_pct: pct, remaining: null, resets_at: resetsAt, exhausted: false, source: 'cli', seen_at: ago(10) }, extra);
  const limits = {
    claude: { known: [W('five_hour', exhausted ? 100 : 54, nextAt(19, 0), { exhausted }), W('seven_day', 16, nextAt(10, 0, 5)), W('seven_day_opus', 31, nextAt(10, 0, 5), { model: 'opus' }), W('seven_day_sonnet', 22, nextAt(10, 0, 5), { model: 'sonnet' }), W('day', 7, nextAt(0, 0))] },
    codex: { no_data: 'Codex не сообщает лимиты в неинтерактивном режиме' },
    gemini: { known: [W('day', 85, nextAt(3, 0))] },
    opencode: null,
  };
  const P = (id, name, finish, check) => ({ id, name, path: '/Users/dev/code/' + name, base_branch: 'main', check_cmd: check === false ? null : 'pytest', setup_cmd: null, copy_files: ['.env*'], finish, ask_outward: true, agent: 'claude', reviewer: null, fallback_agent: null });
  let projects = scenario === 'empty' ? [] : [P('p1', 'billing-api', 'push'), P('p2', 'web-site', 'pr'), P('p3', 'docs-tool', 'local', false)];
  const repos = { p1: { is_repo: true, base_branch: 'main', current_branch: 'main', ahead: 3, behind: 0 }, p2: { is_repo: true, base_branch: 'main', current_branch: 'main', ahead: 0, behind: 0 }, p3: { is_repo: true, base_branch: 'main', current_branch: 'main', ahead: 0, behind: 0 } };
  const STEPS7 = ['plan', 'spec', 'critique', 'tests_first', 'code', 'gates', 'review', 'merge'];
  const steps = (cur, spec) => STEPS7.map((s, i) => [s, s === 'spec' ? { skipped: spec || 'задача не расплывчатая' } : i < STEPS7.indexOf(cur) ? 'done' : s === cur ? 'running' : 'pending']);
  const T = (id, project_id, agent, chat_id, title, status, extra) => Object.assign({ id, project_id, chat_id, agent, title, prompt: title, status, branch: (extra && extra.branch) || 'ns/' + id.slice(2, 8) + '-' + title.toLowerCase().replace(/[^a-z0-9]+/g, '-').slice(0, 22),
    worktree: '/Users/dev/.nightshift/wt/' + id, session: 's-' + id, attempts: 0, review_rounds: 0, question: null, resume_at: null, outcome: {}, created_at: ago(30), started_at: ago(14), finished_at: null,
    quality: { on: true }, steps: steps('code'), runs: [], start: 'now', models: { worker: { family: 'opus', version: null } }, cost_usd: 1.84, budget_usd: 5,
    rendered_prompt: 'Цель. Курсорная пагинация в GET /invoices.\nОграничения. Схему БД не менять, старый ?page= не ломать.\nГотово, когда. Новые тесты зелёные, ответ содержит next_cursor.' }, extra);
  const RUNS = [{ role: 'planner', model: 'Opus 5.5', state: 'done' }, { role: 'critic', model: 'Sonnet 5.5', state: 'done' }, { role: 'test_author', model: 'Sonnet 5.5', state: 'done' }, { role: 'worker', model: 'Opus 5.5', state: 'running' }, { role: 'reviewer', model: 'Opus 5.5', state: 'pending' }, { role: 'integrator', model: 'Sonnet 5.5', state: 'pending' }];
  const RED = { passed: false, tail: 'FAILED tests/test_invoices_pagination.py::test_invalid_cursor_returns_400\nFAILED tests/test_invoices_pagination.py::test_limit_over_max\n7 passed, 2 failed' };
  let tasks = scenario === 'empty' ? [] : [
    T('t-inv', 'p1', 'claude', 'inv', 'Пагинация в /invoices', 'needs_human', { branch: 'ns/invoices-pagination', runs: RUNS, outcome: { tests: RED, files_changed: 4, insertions: 184, deletions: 31 },
      question: { kind: 'agent', text: 'Мобильный клиент ещё шлёт ?page=. Оставить старый параметр на переходный период или убрать сразу?', options: ['Оставить page', 'Убрать сразу'] } }),
    T('t-round', 'p1', 'claude', 'round', 'Округление в итогах', 'checking', { branch: 'ns/rounding-totals', steps: steps('gates'), started_at: ago(7), runs: RUNS.map((r) => Object.assign({}, r, { state: r.role === 'worker' ? 'done' : r.state })) }),
    T('t-chlog', 'p1', 'claude', 'chlog', 'Changelog 2.4', 'queued', { steps: steps('plan').map((p) => [p[0], 'pending']), started_at: null, cost_usd: null }),
    T('t-cur', 'p1', 'claude', 'cur', 'Поле currency в payments', 'done', { steps: steps('merge').map((p) => [p[0], p[1] === 'running' || p[1] === 'pending' ? 'done' : p[1]]), finished_at: ago(160), started_at: ago(180),
      outcome: { summary: 'Влито в main', commit: 'a3f9c21d8e', tests: { passed: true, tail: '42 passed' }, review: { approved: true, issues: [] }, files_changed: 6, insertions: 120, deletions: 14 } }),
    T('t-hero', 'p2', 'claude', 'hero', 'Герой на странице цен', 'awaiting_outward', { branch: 'ns/pricing-hero', steps: steps('merge'), started_at: ago(22), outcome: { tests: { passed: true, tail: '30 passed' }, review: { approved: true, issues: [] }, files_changed: 5, insertions: 142, deletions: 38 },
      question: { kind: 'outward', text: 'Готово и проверено, ревьюер доволен. Отправить ветку ns/pricing-hero и открыть PR в main?', options: [] } }),
    T('t-acc', 'p1', 'codex', 'acc', 'customer_id → account_id', 'working', { started_at: ago(31), steps: steps('code'), models: { worker: { family: 'gpt-codex', version: null } } }),
    T('t-pf', 'p3', 'codex', 'pf', 'Поиск на Pagefind', 'failed', { attempts: 2, steps: steps('gates').map((p) => (p[0] === 'gates' ? ['gates', { failed: 'tests' }] : p)), started_at: ago(48), finished_at: ago(2), cost_usd: null, budget_usd: null,
      models: { worker: { family: 'gpt-codex', version: null } }, outcome: { error: 'tests still failing after 3 attempts', tests: RED } }),
    T('t-img', 'p2', 'gemini', 'img', 'Сжатие картинок', 'working', { started_at: ago(9), steps: steps('code'), cost_usd: null, models: {} }),
    T('t-es', 'p2', 'gemini', 'es', 'Переводы на испанский', 'done', { steps: steps('merge').map((p) => [p[0], p[1] === 'running' || p[1] === 'pending' ? 'done' : p[1]]), finished_at: ago(300), started_at: ago(330), cost_usd: null,
      outcome: { summary: 'Влито в main', commit: 'c02d55e1b7', files_changed: 3, insertions: 88, deletions: 0 } }),
  ];
  const C = (id, env, project_id, title, last, ageMin) => ({ id, env, project_id, title, last, session: null, convoy: false, created_at: ago(ageMin + 60), updated_at: ago(ageMin), archived: false });
  let chats = scenario === 'empty' ? [] : [C('inv', 'claude', 'p1', 'Пагинация в /invoices', '', 1), C('round', 'claude', 'p1', 'Округление в итогах', '', 7), C('how', 'claude', 'p1', 'Как устроен биллинг?', 'Три слоя: routes, service, repo', 20),
    C('chlog', 'claude', 'p1', 'Changelog 2.4', '', 30), C('cur', 'claude', 'p1', 'Поле currency в payments', '', 160), C('hero', 'claude', 'p2', 'Герой на странице цен', '', 22),
    C('acc', 'codex', 'p1', 'customer_id → account_id', '', 31), C('pf', 'codex', 'p3', 'Поиск на Pagefind', '', 2), C('img', 'gemini', 'p2', 'Сжатие картинок', '', 9),
    C('avif', 'gemini', 'p2', 'Что такое AVIF?', 'Формат картинок, меньше WebP', 40), C('es', 'gemini', 'p2', 'Переводы на испанский', '', 300), C('old', 'claude', 'p1', 'Старый разговор про CI', 'Лучше кэшировать node_modules', 60 * 24 * 9)];
  const M = (chat, id, hh, mm, who, text, extra) => Object.assign({ id: chat + '-' + id, chat_id: chat, at: today(hh, mm), who, text, task_id: null, delivered_at: null }, extra);
  const R = (r) => ({ role: r });
  const msgs = {
    inv: [M('inv', 1, 13, 58, 'human', 'Как у нас сейчас устроена пагинация в /invoices?'),
      M('inv', 2, 13, 58, 'model', 'Через offset: `?page=`, по 50 записей (src/invoices/routes.py). На большой таблице это медленно, а при вставках записи повторяются между страницами.', { model: 'Sonnet 5.5' }),
      M('inv', 3, 13, 59, 'system', '', { tool: { name: 'read', arg: 'src/invoices/routes.py', result: '212 строк', ok: true } }),
      M('inv', 4, 14, 1, 'human', 'Сделай курсорную пагинацию. Старый мобильный клиент не сломай.'),
      M('inv', 5, 14, 1, 'system', 'Это задача.', { kind: 'task', task_id: 't-inv' }),
      M('inv', 6, 14, 2, R('planner'), 'План без смены схемы:\n\n- курсор = (created_at, id), непрозрачный\n- ?limit по умолчанию 50, максимум 200\n- старый ?page= пока остаётся', { model: 'Opus 5.5' }),
      M('inv', 7, 14, 4, R('critic'), 'created_at не уникален, без id записи пропадут на границе страниц. И нужен ответ на битый курсор: 400 invalid_cursor.', { model: 'Sonnet 5.5' }),
      M('inv', 8, 14, 8, R('test_author'), '9 тестов, все красные, как и должно быть до кода.', { model: 'Sonnet 5.5' }),
      M('inv', 9, 14, 8, 'system', '', { tool: { name: 'write', arg: 'tests/test_invoices_pagination.py', result: '+118', ok: true } }),
      M('inv', 10, 14, 9, 'system', '', { tool: { name: 'run', arg: 'pytest tests/test_invoices_pagination.py', result: '9 failed', ok: false } }),
      M('inv', 11, 14, 11, R('worker'), 'Курсор и валидация готовы, подключаю к запросу.', { model: 'Opus 5.5' }),
      M('inv', 12, 14, 11, 'system', '', { tool: { name: 'edit', arg: 'src/invoices/routes.py', result: '+64 −12', ok: true } }),
      M('inv', 13, 14, 12, 'system', '', { tool: { name: 'edit', arg: 'src/invoices/cursor.py', result: 'новый, +41', ok: true } }),
      M('inv', 14, 14, 12, 'system', '', { tool: { name: 'run', arg: 'pytest tests/test_invoices_pagination.py', result: '7 passed, 2 failed', ok: false } }),
      M('inv', 15, 14, 13, R('verifier'), 'Тесты 7 из 9, линтер и типы чистые. Вливать пока нельзя.', { model: 'pytest · ruff · mypy' }),
      M('inv', 16, 14, 14, 'human', 'Курсор пусть будет непрозрачным, id наружу не свети.', { task_id: 't-inv', delivered_at: today(14, 15) }),
      M('inv', 17, 14, 15, R('worker'), 'Подписываю курсор, чиню два теста. Задал вопрос про старый клиент - он в карточке задачи.', { model: 'Opus 5.5' })],
    how: [M('how', 1, 13, 40, 'human', 'Как устроен биллинг? Коротко.'), M('how', 2, 13, 40, 'model', 'Три слоя: `routes` принимают запрос, `service` считает суммы и налоги, `repo` ходит в Postgres. Деньги хранятся в копейках, валюта в отдельном поле.', { model: 'Sonnet 5.5' })],
    round: [M('round', 1, 13, 50, 'human', 'Сделай: округление в итогах'), M('round', 2, 13, 50, 'system', '', { kind: 'task', task_id: 't-round' })],
    hero: [M('hero', 1, 13, 30, 'human', 'Сделай героя на странице цен'), M('hero', 2, 13, 30, 'system', '', { kind: 'task', task_id: 't-hero' })],
    pf: [M('pf', 1, 13, 10, 'human', 'Сделай поиск на Pagefind'), M('pf', 2, 13, 10, 'system', '', { kind: 'task', task_id: 't-pf' })],
    acc: [M('acc', 1, 13, 20, 'human', 'Переименуй customer_id в account_id'), M('acc', 2, 13, 20, 'system', '', { kind: 'task', task_id: 't-acc' }),
      M('acc', 3, 13, 40, R('watcher'), 'Исполнитель 15 минут без вывода. Подтолкнул, жду. Если не оживёт за 5 минут - предложу сменить агента.', { model: 'GPT mini 5.1' })],
    avif: [M('avif', 1, 13, 15, 'human', 'Что такое AVIF и стоит ли переходить?'), M('avif', 2, 13, 15, 'model', 'Формат картинок на основе AV1. При том же качестве обычно легче WebP. Браузеры его поддерживают, но кодирование медленнее - для сборки сайта это не проблема.', { model: 'Gemini Pro 3' })],
  };
  const files = { 't-inv': [['src/invoices/routes.py', 64, 12], ['src/invoices/cursor.py', 41, 0], ['tests/test_invoices_pagination.py', 118, 0], ['docs/api.md', 9, 2]] };
  const DIFF = [{ kind: 'hunk', text: '@@ def list_invoices(...)' }, { kind: 'del', text: '    rows = db.query(Invoice).offset(page * 50).limit(50)' }, { kind: 'add', text: '    after = decode_cursor(cursor) if cursor else None' },
    { kind: 'add', text: '    q = db.query(Invoice).order_by(Invoice.created_at, Invoice.id)' }, { kind: 'add', text: '    rows = q.limit(min(limit, 200) + 1).all()' }, { kind: 'ctx', text: '    return {"items": rows[:limit], "next_cursor": next_cursor}' }];

  // ---------- bus ----------
  const subs = {};
  const emit = (name, payload) => (subs[name] || []).forEach((f) => setTimeout(() => f({ payload }), 0));
  const byId = (id) => tasks.find((x) => x.id === id);
  const changed = (tk) => emit('task-changed', tk);
  const chatMsgs = (id) => (msgs[id] = msgs[id] || []);
  const pushMsg = (m) => { chatMsgs(m.chat_id).push(m); emit('chat-msg', m); };
  const fail = (m) => { throw m; };
  const TASK_RE = /^(сделай|добавь|почини|исправь|перепиши|напиши|убери|переведи|обнови|make|add|fix)/i;

  const cmds = {
    list_envs: () => (scenario === 'nogit' ? fail('git not found: install git') : envs),
    list_projects: () => projects, list_tasks: () => tasks, get_settings: () => settings, set_settings: ({ settings: s }) => { settings = withDefaults(s); return null; },
    repo_info: ({ id }) => repos[id], list_agents: () => envs.map((e) => ({ id: e.id, name: e.name, available: !!e.installed, hint: e.install_hint || '' })),
    add_project: ({ path }) => { if (/nogit/.test(path)) fail('not a git repo: ' + path); const p = P('p' + (projects.length + 1), path.split('/').pop(), 'local'); projects.push(p); repos[p.id] = { is_repo: true, base_branch: 'main', current_branch: 'main', ahead: 0, behind: 0 }; return p; },
    init_repo: () => null,
    get_catalog: ({ env }) => (catalogs[env] ? { env, families: catalogs[env], source: 'built_in', at: ago(60 * 5) } : null),
    refresh_catalog: ({ env }) => (catalogs[env] ? { env, families: catalogs[env], source: 'cli_list', at: iso(Date.now()) } : null),
    get_limits: ({ env }) => limits[env] || null,
    list_chats: ({ env }) => chats.filter((c) => c.env === env),
    get_chat: ({ id, before, limit }) => chatMsgs(id).filter((m) => !before || Date.parse(m.at) < Date.parse(before)).slice(-limit),
    send_chat: ({ target, msgId, text }) => {
      let id = target.existing;
      if (!id) { id = 'n' + Date.now(); chats.unshift(C(id, target.new.env, target.new.project_id, text.slice(0, 40), '', 0)); }
      if (chatMsgs(id).some((m) => m.id === msgId)) return id;               // the core ignores a repeat of the same id
      pushMsg({ id: msgId, chat_id: id, at: iso(Date.now()), who: 'human', text, task_id: null, delivered_at: null });
      setTimeout(() => {
        if (TASK_RE.test(text)) {
          const c = chats.find((x) => x.id === id), tk = T('t-n' + Date.now(), c.project_id, c.env, id, text.slice(0, 60), 'working', { steps: steps('plan'), started_at: iso(Date.now()), runs: [], cost_usd: null });
          tasks.push(tk); changed(tk); pushMsg({ id: 'm' + Date.now(), chat_id: id, at: iso(Date.now()), who: 'system', text: '', kind: 'task', task_id: tk.id });
        } else pushMsg({ id: 'm' + Date.now(), chat_id: id, at: iso(Date.now()), who: 'model', text: 'Смотрю код, сейчас отвечу.', model: 'Sonnet 5.5' });
      }, 700);
      return id;
    },
    remove_chat: ({ id }) => { chats = chats.filter((c) => c.id !== id); return null; }, retry_chat: () => null,
    answer: ({ id, text }) => { const tk = byId(id); tk.status = 'working'; tk.question = null; changed(tk); pushMsg(M(tk.chat_id, 'a' + Date.now(), 0, 0, 'human', text, { at: iso(Date.now()), task_id: id })); return null; },
    note: ({ taskId, to, text }) => { const tk = byId(taskId); pushMsg(M(tk.chat_id, 'n' + Date.now(), 0, 0, 'human', text, { at: iso(Date.now()), task_id: taskId, to: to || null })); return null; },
    answer_outward: ({ taskId, choice }) => { const tk = byId(taskId); tk.status = choice === 'no' ? 'done_not_pushed' : 'done'; tk.question = null; changed(tk); return null; },
    resume_failed: ({ taskId }) => { const tk = byId(taskId); tk.status = 'working'; tk.outcome = {}; changed(tk); return null; },
    cancel: ({ id }) => { const tk = byId(id); tk.status = 'cancelled'; changed(tk); return null; }, retry: ({ id }) => { const tk = byId(id); tk.status = 'queued'; changed(tk); return null; },
    remove_task: ({ id }) => { tasks = tasks.filter((x) => x.id !== id); emit('task-removed', id); return null; }, undo: ({ id }) => { const tk = byId(id); tk.status = 'reverted'; changed(tk); return null; },
    push_base: () => null, hand_off: ({ taskId }) => { const tk = byId(taskId); tk.status = 'handed_off'; changed(tk); return null; }, take_back: ({ taskId }) => { const tk = byId(taskId); tk.status = 'checking'; changed(tk); return null; },
    start_now: ({ taskId }) => { const tk = byId(taskId); tk.status = 'working'; changed(tk); return null; }, not_a_task: () => null, freeze: ({ on }) => { emit('frozen', on); return null; },
    get_diffstat: ({ taskId }) => (files[taskId] || []).map(([path, insertions, deletions]) => ({ path, insertions, deletions })),
    diff_file: () => ({ lines: DIFF, binary: false }),
    open_path: () => null, open_editor: () => null, open_terminal: () => null, install_env: () => null, login_env: () => null, check_custom: () => 'ok',
  };
  // ================= task 15: swarm, factory, settings, kits, summary =================
  // Query: ?screen=swarm|factory|settings|kits|import|summary  ?swarm=race|crew|council  ?scenario=fact50  ?section=<settings section>  ?platform=linux
  //        ?away=few|many|none (the "Пока тебя не было" banner)  ?notice=replaced|blocked|kits  ?tray=off  ?click=<css selector>|<css selector>  ?type=<selector>::<value>~~...  ?press=<selector>::<KeyCode>:<mods>  ?scroll=<selector>  ?log=1  ?src=<kit source>
  const runsAt = (startAgo, list) => list.map(([role, model, state, a, b, variant]) => ({ role, model, state, variant: variant || null, started_at: a == null ? null : ago(startAgo - a), finished_at: b == null ? null : ago(startAgo - b) }));
  const wantFactory = q.get('screen') === 'factory' || scenario === 'fact50';
  if (scenario !== 'empty') {
    // runs with times for the lanes of the factory
    byId('t-inv').runs = runsAt(30, [['planner', 'Opus 5.5', 'done', 0, 3], ['critic', 'Sonnet 5.5', 'done', 3, 6], ['test_author', 'Sonnet 5.5', 'done', 6, 10], ['coordinator', 'Opus 5.5', 'running', 2, null], ['worker', 'Opus 5.5', 'running', 10, null], ['reviewer', 'Opus 5.5', 'pending'], ['integrator', 'Sonnet 5.5', 'pending']]);
    byId('t-round').runs = runsAt(40, [['planner', 'Opus 5.5', 'done', 0, 4], ['test_author', 'Sonnet 5.5', 'done', 4, 8], ['worker', 'Opus 5.5', 'done', 8, 30], ['verifier', 'pytest', 'running', 30, null], ['reviewer', 'Opus 5.5', 'pending']]);
    byId('t-cur').runs = runsAt(180, [['planner', 'Opus 5.5', 'done', 0, 3], ['worker', 'Sonnet 5.5', 'done', 3, 25], ['reviewer', 'Opus 5.5', 'done', 25, 28], ['integrator', 'Sonnet 5.5', 'done', 28, 30]]);
    byId('t-hero').runs = runsAt(35, [['planner', 'Opus 5.5', 'done', 0, 5], ['worker', 'Opus 5.5', 'done', 5, 20], ['verifier', 'pytest', 'done', 20, 24], ['reviewer', 'Opus 5.5', 'done', 24, 30], ['integrator', 'Sonnet 5.5', 'running', 30, null]]);
    byId('t-acc').runs = runsAt(31, [['planner', 'GPT 5.1', 'done', 0, 4], ['test_author', 'GPT-Codex 5', 'done', 4, 8], ['worker', 'GPT-Codex 5', 'running', 8, null]]);
    byId('t-pf').runs = runsAt(48, [['planner', 'GPT 5.1', 'done', 0, 4], ['worker', 'GPT-Codex 5', 'failed', 4, 30], ['verifier', 'pytest', 'failed', 30, 34]]);
    byId('t-img').runs = runsAt(9, [['planner', 'Gemini Pro 3', 'done', 0, 3], ['worker', 'Gemini Flash 3', 'running', 3, null]]);
    // the swarm of the mockup: the chat of the swarm is the main surface (story 156)
    const swMode = q.get('swarm') || 'race';
    const files3 = [{ path: 'src/webhooks/limiter.py', insertions: 62, deletions: 3 }, { path: 'src/webhooks/routes.py', insertions: 21, deletions: 5 }, { path: 'tests/test_webhook_limit.py', insertions: 13, deletions: 0 }];
    const CRIT = (a, b, c, d) => [{ key: 'correctness', value: a }, { key: 'coverage', value: b }, { key: 'simplicity', value: c }, { key: 'risk', value: d }];
    const swRuns = { race: runsAt(20, [['planner', 'Opus 5.5', 'done', 0, 2], ['critic', 'Sonnet 5.5', 'done', 2, 4], ['test_author', 'Sonnet 5.5', 'done', 4, 7],
      ['worker', 'Opus 5.5', 'done', 7, 16, 'A'], ['verifier', 'pytest', 'done', 16, 17, 'A'], ['worker', 'Sonnet 5.5', 'done', 7, 14, 'B'], ['verifier', 'pytest', 'done', 14, 15, 'B'],
      ['worker', 'Sonnet 5.5', 'done', 7, 18, 'C'], ['verifier', 'pytest', 'failed', 18, 19, 'C']]),
    crew: runsAt(20, [['coordinator', 'Opus 5.5', 'done', 0, 3], ['worker', 'Sonnet 5.5', 'done', 3, 12, 'A'], ['verifier', 'pytest', 'done', 12, 13, 'A'], ['worker', 'Sonnet 5.5', 'done', 3, 9, 'B'], ['verifier', 'pytest', 'done', 9, 10, 'B'], ['worker', 'Sonnet 5.5', 'running', 3, null, 'C']]),
    council: runsAt(20, [['council', 'Opus 5.5', 'done', 0, 5, 'A'], ['council', 'Sonnet 5.5', 'done', 0, 4, 'B'], ['council', 'Sonnet 5.5', 'done', 0, 6, 'C'], ['council', 'Opus 5.5', 'done', 6, 9, 'A'], ['council', 'Sonnet 5.5', 'done', 6, 9, 'B'], ['council', 'Sonnet 5.5', 'done', 6, 10, 'C'],
      ['worker', 'Sonnet 5.5', 'running', 10, null, 'B']]) };
    const swView = {
      race: { variants: [
        { id: 'A', model: 'Opus 5.5', state: 'passed', gate: { passed: 14, total: 14 }, score: 8.6, summary: 'Скользящее окно в Redis, проверка и запись атомарно. Переживает рестарт.', criteria: CRIT(9, 8, 8, 9), stats: { ins: 96, del: 8, minutes: 9 }, cost_usd: 0.92, files: files3 },
        { id: 'B', model: 'Sonnet 5.5', state: 'passed', gate: { passed: 14, total: 14 }, score: 7.1, summary: 'Бакет в памяти процесса. Проще, но в каждом воркере свой счётчик.', criteria: CRIT(6, 8, 9, 5), stats: { ins: 71, del: 5, minutes: 7 }, cost_usd: 0.61, files: files3.slice(0, 2) },
        { id: 'C', model: 'Sonnet 5.5', state: 'failed', gate: { passed: 12, total: 14 }, score: 4.2, summary: 'Отдельный middleware. Два теста на границе окна падают.', criteria: CRIT(4, 5, 3, 5), stats: { ins: 143, del: 12, minutes: 11 }, cost_usd: 0.48, why_failed: 'tests still failing after 3 attempts', files: files3.slice(1) }],
        advice: { variant: 'A', model: 'Opus 5.5', text: 'Счётчик переживает рестарт и считает атомарно.', autopick_at: iso(now + 6 * 60000) } },
      crew: { parts: [
        { id: 'A', title: 'Хранилище счётчика (Redis)', files: ['src/webhooks/store.py', 'tests/test_store.py'], worker: 'Исполнитель A', model: 'Sonnet 5.5', state: 'done' },
        { id: 'B', title: 'Проверка лимита в обработчике', files: ['src/webhooks/limiter.py', 'src/webhooks/routes.py'], worker: 'Исполнитель B', model: 'Sonnet 5.5', state: 'done' },
        { id: 'C', title: 'Ответ 429 с Retry-After и документация', files: ['src/webhooks/errors.py', 'docs/webhooks.md'], worker: 'Исполнитель C', model: 'Sonnet 5.5', state: 'running' }] },
      council: { council: { proposals: [
        { id: 'A', model: 'Opus 5.5', text: 'Скользящее окно в Redis: общий счётчик для всех воркеров, переживает рестарт.' },
        { id: 'B', model: 'Sonnet 5.5', text: 'Токен-бакет в Redis с атомарным Lua-скриптом: проще окна и без пиков на границе минуты.' },
        { id: 'C', model: 'Sonnet 5.5', text: 'Счётчик в памяти процесса: без новых зависимостей, но лимит умножается на число воркеров.' }],
        votes: [{ voter: 'A', for: 'B', why: 'проще окна, тот же эффект' }, { voter: 'B', for: 'B' }, { voter: 'C', for: 'A', why: 'надёжнее при нескольких воркерах' }] } } };
    // states of the race for the screenshots: ?swstate=stopped | allfailed | queued
    const sws = q.get('swstate');
    if (sws === 'stopped') swView.race.stopped = true;
    if (sws === 'allfailed') { swView.race.variants.forEach((v) => { v.state = 'failed'; v.why_failed = 'tests still failing after 3 attempts'; }); swView.race.variants[0].gate = { passed: 13, total: 14 }; swView.race.advice = null; }
    if (sws === 'queued') { swView.race.variants[2] = { id: 'C', model: 'Sonnet 5.5', state: 'queued' }; swRuns.race = swRuns.race.filter((r) => r.variant !== 'C'); }
    tasks.push(T('t-sw', 'p1', 'claude', 'swarm', 'Лимит запросов для приёма вебхуков', 'working', { branch: 'ns/webhook-limit', started_at: ago(20), swarm: { mode: swMode, n: 3 }, runs: swRuns[swMode], swarm_view: swView[swMode], cost_usd: 2.01, steps: steps('code') }));
    chats.push(C('swarm', 'claude', 'p1', 'Лимит вебхуков · рой', '', 2));
    msgs.swarm = [
      M('swarm', 1, 11, 40, R('planner'), 'Одна спека на троих: 100 запросов в минуту на ключ вебхука, ответ 429 с Retry-After.', { model: 'Opus 5.5' }),
      M('swarm', 2, 11, 49, R('worker'), 'Взял Redis: он уже в стеке, счётчик общий для воркеров.', { model: 'Opus 5.5', variant: 'A' }),
      M('swarm', 3, 11, 49, 'system', '', { tool: { name: 'run', arg: 'pytest tests/test_webhook_limit.py', result: '14 passed', ok: true } }),
      M('swarm', 4, 11, 47, R('worker'), 'Без внешних зависимостей. При нескольких воркерах лимит умножается.', { model: 'Sonnet 5.5', variant: 'B' }),
      M('swarm', 5, 11, 51, R('worker'), 'Два теста на границе окна падают. Чинить или сдавать как есть?', { model: 'Sonnet 5.5', variant: 'C' }),
      M('swarm', 6, 11, 55, R('reviewer'), 'A единственная устойчива при рестарте. C снимаю с гонки.', { model: 'Opus 5.5' }),
      M('swarm', 7, 11, 56, 'human', 'Ок, A. Пусть добавит тест с двумя воркерами.', { task_id: 't-sw', delivered_at: today(11, 57), to: 'A' })];
    // the factory of the mockup: a merge queue with something in it, and 50 tasks for the cap of 20
    const mk = (id, title, status, startAgo, extra) => { const tk = T(id, 'p1', 'claude', 'inv', title, status, Object.assign({ started_at: ago(startAgo), steps: steps('merge') }, extra)); tasks.push(tk); return tk; };
    if (wantFactory && scenario !== 'fact50') {
      mk('t-mg', 'Миграция на Postgres 16', 'merging', 50, { runs: runsAt(50, [['planner', 'Opus 5.5', 'done', 0, 4], ['worker', 'Sonnet 5.5', 'done', 4, 38], ['reviewer', 'Opus 5.5', 'done', 38, 44], ['integrator', 'Sonnet 5.5', 'running', 44, null]]) });
      mk('t-rv', 'Логирование запросов', 'reviewing', 25, { runs: runsAt(25, [['planner', 'Opus 5.5', 'done', 0, 3], ['worker', 'Sonnet 5.5', 'done', 3, 18], ['reviewer', 'Opus 5.5', 'running', 18, null]]) });
      mk('t-night', 'Обновить зависимости', 'queued', 5, { start: 'night', started_at: null, steps: steps('plan').map((p) => [p[0], 'pending']), runs: [] });
    }
    if (scenario === 'fact50') {
      const st50 = ['needs_human', 'failed', 'queued', 'done', 'working', 'reviewing', 'merging', 'queued'];
      for (let i = 0; i < 50; i++) { const s = st50[i % st50.length]; mk('t-f' + i, 'Задача ' + (i + 1) + ': ' + ['схема', 'тесты', 'доки', 'рефакторинг', 'перевод'][i % 5], s, 5 + (i * 7) % 55,
        { runs: runsAt(5 + (i * 7) % 55, [['planner', 'Opus 5.5', 'done', 0, 2], ['worker', 'Sonnet 5.5', s === 'working' ? 'running' : 'done', 2, s === 'working' ? null : 4 + i % 5]]), finished_at: s === 'done' ? ago(2 + i % 30) : null,
          question: s === 'needs_human' ? { kind: 'agent', text: 'Вопрос по задаче ' + (i + 1), options: [] } : null, cost_usd: 0.2 + (i % 7) * 0.11 }); }
    }
  }
  // logs of the tasks for the feed of the factory
  const LOGS = {
    't-inv': [{ at: ago(26), kind: 'stage', role: 'coordinator', text: '«Подготовить релиз 2.4» разбито на 5 задач.' }, { at: ago(22), kind: 'agent', role: 'test_author', model: 'Sonnet 5.5', text: 'Автор тестов (Sonnet 5.5): 9 тестов, все красные как надо.' },
      { at: ago(8), kind: 'check', role: 'verifier', text: 'Пагинация: тесты 7 из 9.' }],
    't-cur': [{ at: ago(160), kind: 'git', role: 'integrator', text: 'Влито в main: Поле currency в payments.' }],
    't-round': [{ at: ago(3), kind: 'agent', role: 'watcher', text: 'Округление: нет вывода 15 минут, толкнул, жду.' }],
    't-acc': [{ at: ago(10), kind: 'agent', role: 'watcher', text: 'Завис 15 мин. Исполнитель (GPT-Codex 5) на «customer_id → account_id».' }],
    't-pf': [{ at: ago(2), kind: 'error', text: 'tests still failing after 3 attempts' }],
  };
  const convoyChats = new Map();
  const NOTICES = { replaced: ['settings-replaced', { path: '~/.local/share/nightshift/state.json.corrupt-2026-10-21' }], blocked: ['notify-blocked', { how: 'Системные настройки - Уведомления - Nightshift - Разрешить' }], kits: ['kits-recovered', { name: 'team-setup' }] };

  // ---------- kits (the forms of core::kits: serde defaults, snake_case enums) ----------
  const KITDIR = { claude: '~/.claude', codex: '~/.codex', gemini: '~/.gemini', opencode: '~/.config/opencode', cursor: '~/.cursor', qwen: '~/.qwen' };
  const names = (p, n) => Array.from({ length: n }, (_, i) => p + (i + 1));
  const INV = (env, scope) => {
    const g = (group, count, items, unverified) => ({ group, available: !unverified, active: !unverified && count > 0, note: unverified ? 'layout_unverified' : null, count, items });
    const cl = env === 'claude', dir = scope && scope.project ? scope.project : KITDIR[env] || '-';
    if (env === 'cursor' || env === 'custom:mine') return { env, env_name: env, scope, dir, found: false, dangerous: [], groups: ['skills', 'rules', 'hooks', 'mcp', 'agents', 'settings'].map((id) => g(id, 0, [], true)) };
    const skills = cl ? ['code-review', 'release-notes', 'db-migrate'].concat(names('skill-', 39)) : names('skill-', 4);
    return { env, env_name: env, scope, dir, found: true, dangerous: cl ? ['env', 'statusLine', 'permissions'] : [],
      groups: [g('skills', skills.length, skills, env !== 'claude' && env !== 'opencode'), g('rules', 2, [dir + '/rules.md', 'billing-api/rules.md']),
        g('hooks', cl ? 9 : 0, cl ? ['перед каждой командой: проверка опасных команд', 'после правки: форматирование'].concat(names('hook-', 7)) : []), g('mcp', cl ? 6 : 1, cl ? ['github', 'postgres'].concat(names('mcp-', 4)) : ['github']),
        g('agents', 3, ['explorer', 'reviewer', 'planner']), g('settings', 1, ['settings без ключей и токенов'])] };
  };
  const ent = (key, group, label, change, extra) => Object.assign({ key, group, label, target: KITDIR.claude + '/' + label, change, diff: null, held: null }, extra);
  const PLAN = (env, source, scope, update) => ({ id: 'plan1', env, kit_name: update ? 'team-setup' : 'my-setup', source, commit: 'a1b2c3d', scope, target_dir: scope && scope.project ? scope.project + '/.claude' : KITDIR[env] || '~',
    entries: update ? [ent('hooks:Stop', 'hooks', 'Stop', 'replace', { diff: '-./notify.sh\n+./notify.sh --quiet', held: { kind: 'hook', event: 'Stop', text: './notify.sh --quiet', scripts: [] } }), ent('skills:code-review', 'skills', 'code-review', 'same')]
      : [ent('skills:code-review', 'skills', 'code-review', 'same'), ent('skills:api-docs', 'skills', 'api-docs', 'add', { held: { kind: 'scripted_skill', event: null, text: 'skills/api-docs/SKILL.md', scripts: [{ path: 'scripts/gen.sh', text: '#!/bin/sh\nset -e\nnpx openapi-generator ./spec.yaml\n' }, { path: 'scripts/lint.py', text: 'import sys\nprint("ok")\n' }] } }),
        ent('rules:CLAUDE.md', 'rules', 'CLAUDE.md', 'replace', { diff: '@@ -1,2 +1,3 @@\n # Правила\n-Пиши коротко.\n+Пиши коротко и по делу.\n+Тесты - сначала.' }),
        ent('mcp:sentry', 'mcp', 'sentry', 'add', { held: { kind: 'mcp', event: null, text: 'npx -y @sentry/mcp-server', scripts: [] } }), ent('skills:changelog', 'skills', 'changelog', 'add'), ent('skills:release-notes', 'skills', 'release-notes', 'same'),
        ent('hooks:PreToolUse:Bash', 'hooks', 'PreToolUse · Bash', 'add', { held: { kind: 'hook', event: 'PreToolUse · Bash', text: 'python3 ~/.claude/hooks/block-danger.py "$TOOL_INPUT"', scripts: [{ path: 'hooks/block-danger.py', text: '#!/usr/bin/env python3\nimport json, sys\ncmd = json.load(sys.stdin)["command"]\nif "sudo" in cmd:\n    print("blocked", file=sys.stderr)\n    sys.exit(2)\n' }] } }),
        ent('hooks:PostToolUse:Edit', 'hooks', 'PostToolUse · Edit', 'add', { held: { kind: 'hook', event: 'PostToolUse · Edit', text: 'npx prettier --write "$FILE_PATH"', scripts: [] } }),
        ent('hooks:Stop', 'hooks', 'Stop', 'add', { held: { kind: 'hook', event: 'Stop', text: 'curl -s -X POST "$NOTIFY_URL" -d "задача готова"', scripts: [] } }),
        ent('settings:statusLine', 'settings', 'statusLine', 'add', { held: { kind: 'command', event: null, text: '"statusLine": {"type": "command", "command": "~/.claude/statusline.sh"}', scripts: [] } })],
    env_vars: update ? [] : [{ name: 'GITHUB_TOKEN', used_by: ['mcp:github'], missing: true }, { name: 'NOTIFY_URL', used_by: ['hook:Stop'], missing: true }, { name: 'SENTRY_DSN', used_by: ['mcp:sentry'], missing: false }],
    skipped: update ? [] : [{ path: 'config.toml', reason: 'inline_form' }], project_files: [], uncommitted_in_project: !!(scope && scope.project), update_from: update ? '9f8e7d6' : null, kit_sha: 'abc' });
  let snaps = [{ id: 'sn1', env: 'claude', kit_name: 'team-setup', source: { repo: 'https://github.com/acme/team-setup' }, commit: '9f8e7d6', scope: 'global', at: ago(60 * 30), state: 'done', changed: ['~/.claude/settings.json'], project_files: [],
    held: [{ key: 'hooks:Stop', group: 'hooks', kind: 'hook', label: 'Stop · ./notify.sh', enabled: false }, { key: 'settings:statusLine', group: 'settings', kind: 'command', label: 'statusLine (запускает программу)', enabled: false }, { key: 'mcp:github', group: 'mcp', kind: 'mcp', label: 'github', enabled: true }] },
    { id: 'sn0', env: 'claude', kit_name: 'old-kit', source: { folder: '/Users/dev/kits/old' }, commit: null, scope: 'global', at: ago(60 * 70), state: 'interrupted', held: [], changed: [], project_files: [] }];
  const kitErr = (o) => { throw o; };

  Object.assign(cmds, {
    get_log: ({ id, limit }) => (LOGS[id] || []).slice(0, limit),
    nudge: () => null, switch_model: () => null,
    pick_variant: ({ taskId, variant }) => { const tk = byId(taskId); if (tk && tk.swarm_view) { tk.swarm_view = Object.assign({}, tk.swarm_view, { picked: variant }); changed(tk); } return null; },
    stop_swarm: ({ taskId }) => { const tk = byId(taskId); if (tk) { tk.swarm_view = Object.assign({}, tk.swarm_view, { stopped: true }); changed(tk); } return null; },
    submit_convoy: ({ projectId, env, text }) => {
      const id = 'cv' + Date.now(), c = C(id, env, projectId, 'Поручение: ' + text, '', 0); c.convoy = true; chats.unshift(c);
      ['Подготовить ветку', 'Обновить версии', 'Прогнать миграции', 'Собрать changelog', 'Проверить сборку'].forEach((title, i) => { const tk = T(id + '-' + i, projectId, env, id, title, i < 3 ? 'done' : i === 3 ? 'working' : 'queued', { started_at: ago(10 - i), steps: steps(i < 3 ? 'merge' : 'code') }); tasks.push(tk); changed(tk); chatMsgs(id).push(M(id, 'k' + i, 0, 0, 'system', '', { at: iso(Date.now() + i), kind: 'task', task_id: tk.id })); });
      return id;
    },
    summary_since: () => {
      const m = q.get('away');
      if (m === 'none') return { done: [], failed: [], waiting: [], spent: [], limits: [] };
      const done = [{ task_id: 't-cur', env: 'claude', title: 'Поле currency в payments', commit: 'a3f9c21d8e' }, { task_id: 't-es', env: 'gemini', title: 'Переводы на испанский', commit: 'c02d55e1b7' }];
      if (m === 'many') for (let i = 0; i < 12; i++) done.push({ task_id: 't-cur', env: 'claude', title: 'Готовая задача ' + (i + 3), commit: 'f00d' + i + 'aa1b2' });
      return { done, failed: [{ task_id: 't-pf', env: 'codex', title: 'Поиск на Pagefind', reason: 'tests still failing after 3 attempts' }],
        waiting: [{ task_id: 't-inv', env: 'claude', title: 'Пагинация в /invoices' }, { task_id: 't-hero', env: 'claude', title: 'Герой на странице цен' }],
        spent: [{ env: 'claude', usd: 8.1 }, { env: 'codex', usd: 3.2 }], limits: [{ env: 'claude', window: W('five_hour', 85, nextAt(19, 0)) }] };
    },
    set_secret: () => null,
    kits_inventory: ({ env, scope }) => INV(env, scope),
    kits_preview: ({ env, source, scope }) => {
      const v = String(Object.values(source)[0]);
      if (/codex/i.test(v)) kitErr({ kind: 'wrong_env', kit_env: 'codex', kit_env_name: 'Codex', target_env: env, target_env_name: (envs.find((e) => e.id === env) || {}).name || env });
      if (/bad/i.test(v)) kitErr({ kind: 'bad_zip' }); if (/big/i.test(v)) kitErr({ kind: 'file_too_large', path: 'hooks/blob.bin', size: 12000000 }); if (/link/i.test(v)) kitErr({ kind: 'symlink', path: 'skills/x -> /etc' });
      if (/clone/i.test(v)) kitErr({ kind: 'clone_failed', path: 'repository not found' });
      const p = PLAN(env, source, scope, false);
      if (scope && scope.project) p.entries = p.entries.map((x) => (x.change === 'replace' ? Object.assign({}, x, { change: 'add', diff: null }) : x));
      return p;
    },
    kits_install: ({ plan }) => { const id = 'sn' + Date.now(); snaps.unshift({ id, env: plan.env, kit_name: plan.kit_name, source: plan.source, commit: plan.commit, scope: plan.scope, at: iso(Date.now()), state: 'done', changed: [], project_files: [],
      held: plan.entries.filter((e) => e.held).map((e) => ({ key: e.key, group: e.group, kind: e.held.kind, label: e.label, enabled: false })) }); return id; },
    kits_enable: ({ item }) => { snaps.forEach((s) => { if (s.id === item.snapshot) s.held.forEach((d) => { if (d.key === item.key) d.enabled = true; }); }); return null; },
    kits_snapshots: () => snaps,
    kits_rollback: ({ id }) => { snaps = snaps.filter((s) => s.id !== id); return { kept_changed: ['~/.claude/settings.json: model'], kept_files: [] }; },
    kits_check_update: ({ installed }) => PLAN(installed.env, installed.source, installed.scope, true),
    kits_export: ({ dest }) => ({ dest: Object.values(dest)[0], files: ['kit.json', 'skills/code-review/SKILL.md'], env_vars: [], secrets_replaced: 2, skipped: [], dangerous: ['env'], commit: null }),
    update_project: ({ project }) => { projects = projects.map((p) => (p.id === project.id ? project : p)); if (/nope/.test(project.path)) repos[project.id] = null; else repos[project.id] = repos[project.id] || { is_repo: true, base_branch: 'main', current_branch: 'main', ahead: 0, behind: 0 }; return null; },
    remove_project: ({ id }) => { projects = projects.filter((p) => p.id !== id); return null; },
    github_info: ({ projectId }) => (projectId === 'p3' ? { gh_hint: 'gh не вошёл: gh auth login', repo_url: null, branches: [] }
      : { gh_user: 'nezeronxer', repo_url: 'git@github.com:nezeronxer/' + projectId + '.git', branches: [{ name: 'ns/a1b2c3-invoices-pagination', merged: false }, { name: 'ns/d4e5f6-currency', merged: true }, { name: 'ns/0a9b8c-rounding', merged: true }] }),
    pull_base: () => null, delete_merged_branches: () => null, open_chat_terminal: () => null,
  });
  // an env whose CLI path is wrong is not found (the settings check the path by asking the core)
  const baseList = cmds.list_envs;
  cmds.list_envs = () => baseList().map((e) => ((((settings.envs || {})[e.id] || {}).cli_path || '').includes('nope') ? Object.assign({}, e, { installed: false }) : e));
  const baseRepo = cmds.repo_info;
  cmds.repo_info = ({ id }) => { const r = repos[id]; if (r === null) fail('folder not found: ' + (projects.find((p) => p.id === id) || {}).path); return baseRepo({ id }); };

  window.__TAURI__ = {
    core: { invoke: (cmd, args) => { if (q.get('log')) { const l = JSON.parse(document.documentElement.getAttribute('data-calls') || '[]'); l.push([cmd, args || {}]); document.documentElement.setAttribute('data-calls', JSON.stringify(l)); } if (!cmds[cmd]) return Promise.reject('unknown command ' + cmd); try { return Promise.resolve(cmds[cmd](args || {})); } catch (e) { return Promise.reject(e && typeof e === 'object' ? e : String(e)); } } },
    event: { listen: (name, fn) => { (subs[name] = subs[name] || []).push(fn); return Promise.resolve(() => {}); } },
    dialog: { open: () => Promise.resolve('/Users/dev/code/new-project') },
  };
  window.__MOCK__ = true;

  // deep links for screenshots, after the app is up
  window.addEventListener('ns-ready', () => {
    if (scenario === 'full') emit('task-log', { id: 't-round', event: { kind: 'check', text: 'Ворота: 12 из 18 тестов' } });
    const env = q.get('env');
    const screen = q.get('screen');
    if (q.get('notice') && NOTICES[q.get('notice')]) emit(NOTICES[q.get('notice')][0], NOTICES[q.get('notice')][1]);
    if (q.get('tray') === 'off') emit('tray-state', { available: false });
    if (screen === 'import') { const src = { repo: q.get('src') || 'https://github.com/someone/my-setup' }; window.NS.nav('import', { env: env || 'claude', plan: cmds.kits_preview({ env: env || 'claude', source: src, scope: 'global' }), source: src }); }
    else if (screen === 'swarm') window.NS.nav('env', { env: env || 'claude', chat: 'swarm' });
    else if (screen) window.NS.nav(screen, { env: env || undefined, section: q.get('section') || undefined, source: q.get('src') || undefined });
    else if (env) window.NS.nav('env', { env, chat: q.get('chat') || undefined });
    setTimeout(() => {
      (q.get('click') || '').split('|').filter(Boolean).forEach((sel, i) => setTimeout(() => { const b = document.querySelector(sel); if (b) b.click(); }, 300 * (i + 1)));
      (q.get('type') || '').split('~~').filter(Boolean).forEach((spec, i) => setTimeout(() => { const [sel, val] = spec.split('::'); const el = document.querySelector(sel); if (el) { el.value = val; el.dispatchEvent(new Event('input', { bubbles: true })); el.dispatchEvent(new Event('change', { bubbles: true })); } }, 350 + 20 * i));
      // ?press=<selector>::<KeyCode>:<mods> - focus a hotkey field and press a combination (mods: alt,meta,ctrl,shift)
      (q.get('press') || '').split('|').filter(Boolean).forEach((spec, i) => setTimeout(() => {
        const [sel, key] = spec.split('::'), [code, mods] = key.split(':'), el = document.querySelector(sel);
        if (!el) return;
        el.click();
        const m = (mods || '').split(',');
        el.dispatchEvent(new KeyboardEvent('keydown', { code, key: code, altKey: m.includes('alt'), metaKey: m.includes('meta'), ctrlKey: m.includes('ctrl'), shiftKey: m.includes('shift'), bubbles: true, cancelable: true }));
      }, 450 + 150 * i));
      if (q.get('scroll')) setTimeout(() => { const e = document.querySelector(q.get('scroll')); if (e) e.scrollIntoView({ block: 'start' }); }, 300 * ((q.get('click') || '').split('|').filter(Boolean).length + 1));
      const open = q.get('open');
      if (open) { const b = document.querySelector('[data-task="' + open + '"] .tc-more'); if (b) b.click(); }
      if (q.get('file')) setTimeout(() => { const li = document.querySelector('.flist li[role=button]'); if (li) li.click(); }, 150);
      if (q.get('pop') === 'model') document.querySelector('.envpick').click();
      if (q.get('pop') === 'tune') document.getElementById('tune').click();
    }, 400);
  });
})();
