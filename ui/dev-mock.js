// Fake Tauri backend for viewing the UI in a plain browser. Loaded by mock-loader.js only outside Tauri (NSLogic.mockWanted).
// Command and event names follow interfaces.md ("Границы v2", row `app`); payload shapes are the ones js/ expects (see report).
// Query: ?scenario=full|empty|nogit  ?theme=dark|light  ?lang=ru|en  ?env=<id>  ?chat=<id|new>  ?open=<taskId>  ?pop=model|tune  ?limits=exhausted
(function () {
  const q = new URLSearchParams(location.search);
  if (q.get('theme')) document.documentElement.setAttribute('data-theme', q.get('theme'));
  const scenario = q.get('scenario') || 'full';
  const now = Date.now();
  const iso = (ms) => new Date(ms).toISOString();
  const ago = (min) => iso(now - min * 60000);
  const today = (h, m) => { const d = new Date(); d.setHours(h, m, 0, 0); return iso(d.getTime()); };
  const nextAt = (h, m, addDays) => { const d = new Date(); d.setHours(h, m, 0, 0); if (d <= now || addDays) d.setDate(d.getDate() + (addDays || 1)); return iso(d.getTime()); };

  // ---------- data ----------
  let settings = { max_parallel: 3, paused: false, language: q.get('lang') || 'auto', hotkey: 'Alt+Space', custom_agents: [], default_project: null, envs: {}, attempts: 3, limits: { warn_at: 80 } };
  const envs = [
    { id: 'claude', name: 'Claude Code', installed: true, version: '2.1.4', kit_dir: '~/.claude' },
    { id: 'codex', name: 'Codex', installed: true, version: '0.48', kit_dir: '~/.codex' },
    { id: 'gemini', name: 'Gemini', installed: true, version: '0.9' },
    { id: 'opencode', name: 'opencode', installed: true, version: '0.15' },
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
    list_projects: () => projects, list_tasks: () => tasks, get_settings: () => settings, set_settings: ({ settings: s }) => { settings = s; return null; },
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
    note: ({ taskId, text }) => { const tk = byId(taskId); pushMsg(M(tk.chat_id, 'n' + Date.now(), 0, 0, 'human', text, { at: iso(Date.now()), task_id: taskId })); return null; },
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
  window.__TAURI__ = {
    core: { invoke: (cmd, args) => { if (!cmds[cmd]) return Promise.reject('unknown command ' + cmd); try { return Promise.resolve(cmds[cmd](args || {})); } catch (e) { return Promise.reject(String(e)); } } },
    event: { listen: (name, fn) => { (subs[name] = subs[name] || []).push(fn); return Promise.resolve(() => {}); } },
    dialog: { open: () => Promise.resolve('/Users/dev/code/new-project') },
  };
  window.__MOCK__ = true;

  // deep links for screenshots, after the app is up
  window.addEventListener('ns-ready', () => {
    if (scenario === 'full') emit('task-log', { id: 't-round', event: { kind: 'check', text: 'Ворота: 12 из 18 тестов' } });
    const env = q.get('env');
    if (env) window.NS.nav('env', { env, chat: q.get('chat') || undefined });
    setTimeout(() => {
      const open = q.get('open');
      if (open) { const b = document.querySelector('[data-task="' + open + '"] .tc-more'); if (b) b.click(); }
      if (q.get('file')) setTimeout(() => { const li = document.querySelector('.flist li[role=button]'); if (li) li.click(); }, 150);
      if (q.get('pop') === 'model') document.querySelector('.envpick').click();
      if (q.get('pop') === 'tune') document.getElementById('tune').click();
    }, 400);
  });
})();
