// Fakes of v6 for the browser without Tauri (loaded by mock-loader-v4.js after mock-auto.js): issues in «Предлагаю», the question «Закрыть issue #N?»,
// and sessions from the terminal. Query: ?issues=on (issues of GitHub) | nogh | nologin (why they are missing), ?ask=1 (a merged task asks to close its issue),
// ?sessions=empty | many (130 sessions, pages of 50).
(function () {
  const T = window.__TAURI__;
  if (!T || !window.__MOCK__) return;
  const q = new URLSearchParams(location.search);
  const issuesMode = q.get('issues') || '';
  const invoke0 = T.core.invoke;
  const subs = {};
  const listen0 = T.event.listen;
  T.event.listen = (name, fn) => { (subs[name] = subs[name] || []).push(fn); return listen0(name, fn); };
  const emit = (name, payload) => (subs[name] || []).forEach((fn) => setTimeout(() => fn({ payload }), 0));
  const iso = (ms) => new Date(ms).toISOString();
  const ago = (min) => iso(Date.now() - min * 60000);

  // ---------- issues ----------
  const issue = (number, title, url) => ({ fp: 'issue#' + number, file: '', line: 0, marker: 'issue', text: title, prompt: 'Issue #' + number, source: { kind: 'issue', number, url: url || 'https://github.com/o/billing-api/issues/' + number } });
  let cards = issuesMode === 'on' ? [issue(58, 'Падает экспорт счетов в PDF, если в названии есть кавычки'), issue(61, 'Тёмная тема в письмах-квитанциях')] : [];
  let asks = q.get('ask') ? [{ task_id: 'c1', project_id: 'p1', number: 44, url: 'https://github.com/o/billing-api/issues/44', title: 'Курсорная пагинация в списке счетов' }] : [];
  const why = { nogh: 'gh_not_installed', nologin: 'gh_not_logged_in' }[issuesMode] || null;
  const cmds = {
    auto_get: async () => {
      const d = await invoke0('auto_get');
      if (cards.length) {
        const c = Object.assign({}, d.suggest.cards);
        c.p1 = cards.concat(c.p1 || []);
        d.suggest = Object.assign({}, d.suggest, { cards: c, count: d.suggest.count + cards.length });
      }
      d.issues = { notes: why ? { p1: why } : {}, asks };
      return d;
    },
    auto_issue_answer: ({ taskId }) => {
      if (!asks.some((a) => a.task_id === taskId)) throw 'no_question';
      asks = asks.filter((a) => a.task_id !== taskId);
      return null;
    },
    sessions_list: ({ projectId, page }) => {
      const all = SESSIONS(projectId);
      const p = page || 0;
      return { items: all.slice(p * 50, p * 50 + 50), total: all.length, page: p, page_size: 50, more: (p + 1) * 50 < all.length };
    },
    session_continue: async ({ projectId, id }) => {
      if (!SESSIONS(projectId).some((s) => s.id === id)) throw 'session_not_found: ' + id;
      const tid = 'ses-' + Date.now();
      (await invoke0('list_tasks')).push({ id: tid, project_id: projectId, agent: 'claude', title: 'Продолжить сессию из терминала', prompt: 'Продолжить сессию из терминала', status: 'queued', created_at: iso(Date.now()), outcome: {}, steps: [], runs: [], resume_session: id });
      return tid;
    },
  };
  const S = (id, minAgo, first_prompt, messages, extra) => Object.assign({ id, started_at: ago(minAgo), first_prompt, messages, approx: false, branch: null, unreadable: false }, extra);
  function SESSIONS(pid) {
    const mode = q.get('sessions');
    if (mode === 'empty' || pid !== 'p1') return [];
    if (mode === 'many') return Array.from({ length: 130 }, (_, i) => S('s' + String(i).padStart(3, '0'), 30 + i * 90, 'Задача номер ' + (130 - i) + ': поправить отступы в таблице счетов', 4 + (i % 40), { branch: i % 3 ? 'main' : 'feat/invoices-' + i }));
    return [
      S('9f1c2e7a-4b1d-4e0a-9c55-0d6a7b21c001', 60 * 5, 'Разбери, почему тесты по налогам падают после перехода на новый курс округления', 38, { branch: 'fix/tax-rounding' }),
      S('2b7d90ce-61aa-4f3c-8a10-5c4e1d9f7002', 60 * 27, 'Добавь постраничный вывод в список счетов и обнови тесты', 112, { branch: 'feat/invoices-cursor' }),
      S('c40a58be-0e21-47a9-b3d6-8e2f6a1d5003', 60 * 24 * 4, 'Что делает модуль reports/cache.py и почему он держит всё в памяти?', 9, { branch: 'main' }),
      S('e8d1f6a2-9a37-4c0b-8f41-3b7c2a5e6004', 60 * 24 * 9, '', 0, { unreadable: true }),
      S('71ab33f0-5c18-4d62-a9e7-0f4d8c2b1005', 60 * 24 * 20, 'Сделай скрипт миграции данных клиентов в новую схему и проверь на копии базы', 4000, { approx: true, branch: 'chore/migrate' }),
    ];
  }
  const invoke1 = T.core.invoke;
  T.core.invoke = (cmd, args) => {
    // dismissing or doing an issue card is mine; the rest goes on down the chain
    if ((cmd === 'auto_suggest_dismiss' || cmd === 'auto_suggest_do') && args && String(args.fp).startsWith('issue#')) {
      const c = cards.find((x) => x.fp === args.fp);
      cards = cards.filter((x) => x.fp !== args.fp);
      emit('auto-changed', null);
      if (cmd === 'auto_suggest_dismiss') return Promise.resolve(null);
      if (!c) return Promise.reject('suggestion_not_found');
      const id = 'auto-' + Date.now();
      return invoke1('list_tasks').then((l) => { l.push({ id, project_id: args.projectId, agent: 'claude', title: c.prompt.split('\n')[0], prompt: c.prompt, status: 'queued', created_at: iso(Date.now()), outcome: {}, steps: [], runs: [] }); return id; });
    }
    if (!cmds[cmd]) return invoke1(cmd, args);
    try { const r = Promise.resolve(cmds[cmd](args || {})); if (cmd === 'auto_issue_answer') r.then(() => emit('auto-changed', null), () => {}); return r; } catch (e) { return Promise.reject(e); }
  };
})();
