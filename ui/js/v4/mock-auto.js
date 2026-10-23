// Fakes of F14 for the browser without Tauri (loaded by mock-loader-v4.js after mock-clawd.js): auto_get and the commands behind the
// automation screen. Query: ?auto=empty (nothing set up), ?ci=off|nogh|nologin (the watchman's state).
(function () {
  const T = window.__TAURI__;
  if (!T || !window.__MOCK__) return;
  const q = new URLSearchParams(location.search);
  const invoke0 = T.core.invoke;
  const subs = {};
  const listen0 = T.event.listen;
  T.event.listen = (name, fn) => { (subs[name] = subs[name] || []).push(fn); return listen0(name, fn); };
  const emit = (name, payload) => (subs[name] || []).forEach((fn) => setTimeout(() => fn({ payload }), 0));
  const iso = (ms) => new Date(ms).toISOString();
  const ago = (min) => iso(Date.now() - min * 60000);
  const nextOf = (s) => {
    const [hh, mm] = s.at.split(':').map(Number);
    for (let i = 0; i < 9; i++) {
      const d = new Date(); d.setDate(d.getDate() + i); d.setHours(hh, mm, 0, 0);
      const wd = (d.getDay() + 6) % 7;
      if (d.getTime() > Date.now() && (s.preset === 'daily' || (s.preset === 'weekdays' && wd < 5) || (s.preset === 'weekly' && wd === s.weekday))) return iso(d.getTime());
    }
    return null;
  };
  const S = (id, title, prompt, project_id, preset, at, weekday, enabled, extra) => Object.assign({ id, title, prompt, project_id, env: null, model: null, quality: null, preset, at, weekday, enabled, created_at: ago(60 * 24 * 9), last_run: null, last_task: null, skipped: null }, extra);
  const empty = q.get('auto') === 'empty';
  const TEMPLATES = [
    ['deps', 'Update the project\'s dependencies to their latest compatible versions, one ecosystem at a time. Run the tests after each step and keep only the updates that pass.'],
    ['security', 'Audit the project for security problems: secrets in the tree, injection into commands and queries, unsafe file paths, outdated dependencies with known advisories.'],
    ['todo', 'Find the TODO, FIXME, XXX and HACK comments in the tracked files and resolve the ones that are small and clear.'],
    ['dead_code', 'Find code that nothing uses and remove what is certainly dead; list the doubtful cases instead of deleting them.'],
  ].map(([id, prompt]) => ({ id, prompt }));
  let schedules = empty ? [] : [
    S('s1', 'Аудит безопасности', 'Проверь проект на секреты в дереве, инъекции в команды и запросы, небезопасные пути.', 'p1', 'weekdays', '03:00', 0, true, { last_run: ago(60 * 9 + 20), last_task: 't-audit' }),
    S('s2', 'Обновить зависимости', 'Обнови зависимости до последних совместимых версий, после каждого шага гоняй тесты.', 'p2', 'weekly', '02:30', 2, true, { skipped: 'limit' }),
    S('s3', 'Почистить мёртвый код', 'Найди код, которым никто не пользуется, и удали то, что точно мёртвое.', 'p1', 'daily', '04:15', 0, false),
  ];
  const card = (file, line, marker, text) => ({ fp: file + '::' + marker.toLowerCase() + ' ' + text.toLowerCase(), file, line, marker, text, prompt: marker + ' ' + file + ': ' + text });
  let cards = empty ? {} : {
    p1: [card('src/invoices/routes.py', 88, 'TODO', 'cursor pagination for the archive list'), card('src/reports/cache.py', 41, 'FIXME', 'the ttl is not monotonic, a clock jump drops the whole cache'), card('src/billing/tax.py', 17, 'HACK', 'rounding done by hand until the rates table lands')],
    p2: [card('web/js/menu.js', 203, 'TODO', 'close the menu on Escape, not only on a click outside'), card('web/css/print.css', 9, 'XXX', '')],
  };
  let nightly = false, lastScan = null;
  let ci = { enabled: q.get('ci') !== 'off' && !empty, max_attempts: 2, merge_when_green: false, merge_confirmed: false };
  const hint = () => (ci.enabled ? { nogh: 'gh_not_installed', nologin: 'gh_not_logged_in' }[q.get('ci')] || null : null);
  const items = () => (!ci.enabled || hint() ? [] : [
    { task_id: 'c1', title: 'Курсорная пагинация в GET /invoices', url: 'https://github.com/o/billing-api/pull/41', state: 'green', attempts: 0, max: ci.max_attempts, note: ci.merge_when_green ? 'ask_merge' : null },
    { task_id: 'c2', title: 'Кэш отчётов с TTL', url: 'https://github.com/o/billing-api/pull/43', state: 'red', attempts: 1, max: ci.max_attempts, note: 'fix_started' },
    { task_id: 'c3', title: 'Меню по Escape', url: 'https://github.com/o/web-site/pull/12', state: 'pending', attempts: 0, max: ci.max_attempts, note: null },
    { task_id: 'c4', title: 'Печатная версия счёта', url: 'https://github.com/o/web-site/pull/9', state: 'conflict', attempts: ci.max_attempts, max: ci.max_attempts, note: 'gave_up' },
  ]);
  const count = () => Object.values(cards).reduce((n, c) => n + c.length, 0);
  const cmds = {
    auto_get: () => ({
      schedules: schedules.map((s) => Object.assign({}, s, { next: s.enabled ? nextOf(s) : null })), templates: TEMPLATES,
      suggest: { nightly, count: count(), cards, last_scan: lastScan }, ci: { config: ci, hint: hint(), items: items() },
    }),
    auto_schedule_save: ({ schedule }) => {
      if (!schedule.title.trim()) throw 'empty_title';
      if (!/^([01]\d|2[0-3]):[0-5]\d$/.test(schedule.at)) throw 'bad_time: ' + schedule.at;
      const i = schedules.findIndex((s) => s.id === schedule.id);
      if (i >= 0) { schedules[i] = Object.assign({}, schedules[i], schedule, { skipped: null }); return schedules[i].id; }
      const s = Object.assign({}, schedule, { id: 's' + (schedules.length + 10), created_at: iso(Date.now()), last_run: null, last_task: null, skipped: null });
      schedules.push(s); return s.id;
    },
    auto_schedule_remove: ({ id }) => { schedules = schedules.filter((s) => s.id !== id); return null; },
    auto_schedule_toggle: ({ id, enabled }) => { const s = schedules.find((x) => x.id === id); if (!s) throw 'schedule_not_found'; s.enabled = enabled; s.skipped = null; return null; },
    auto_suggest_scan: ({ projectId }) => { lastScan = iso(Date.now()); return (cards[projectId] || []).length; },
    auto_suggest_do: async ({ projectId, fp }) => {
      const c = (cards[projectId] || []).find((x) => x.fp === fp);
      if (!c) throw 'suggestion_not_found';
      // dev-mock has no submit_task: the task lands in its list, as the core would put it there (no chat card, so no event)
      const id = 'auto-' + Date.now();
      (await invoke0('list_tasks')).push({ id, project_id: projectId, agent: 'claude', title: c.marker + ': ' + c.text, prompt: c.prompt, status: 'queued', created_at: iso(Date.now()), outcome: {}, steps: [], runs: [] });
      cards[projectId] = cards[projectId].filter((x) => x.fp !== fp);
      return id;
    },
    auto_suggest_dismiss: ({ projectId, fp }) => { cards[projectId] = (cards[projectId] || []).filter((x) => x.fp !== fp); return null; },
    auto_suggest_nightly: ({ on }) => { nightly = on; return null; },
    auto_ci_set: ({ config }) => { ci = Object.assign({}, ci, config); return null; },
    auto_ci_poll: () => null,
  };
  const invoke1 = T.core.invoke;
  T.core.invoke = (cmd, args) => {
    if (!cmds[cmd]) return invoke1(cmd, args);
    try { return Promise.resolve(cmds[cmd](args || {})); } catch (e) { return Promise.reject(e); }
  };
  // every change tells the window, as the core does
  for (const c of ['auto_schedule_save', 'auto_schedule_remove', 'auto_schedule_toggle', 'auto_suggest_scan', 'auto_suggest_do', 'auto_suggest_dismiss', 'auto_ci_set']) {
    const f = cmds[c];
    cmds[c] = (a) => { const r = f(a); Promise.resolve(r).then(() => emit('auto-changed', null)).catch(() => {}); return r; };
  }
})();
