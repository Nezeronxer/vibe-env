// Fakes of task 25 for the browser without Tauri (loaded by mock-loader-v4.js over dev-mock.js, which stays untouched):
//   cli_status / cli_update / updates_history / cli_update_mode, events cli-updated, new-model (stories 274-277);
//   startup_tax, compact, Task.context_pct / handoff / port / db_name (305-309);
//   the swarm on dev-mock (291): a chat sent with "Рой" on gets a task with swarm, lanes (runs with variants) and "· рой" in the title.
// Query: ?ctx=75 - context of the running tasks; ?handoff=1 - a hand-off note, ports and database on them;
//        ?newmodel=auto|ask - a new model arrives after start; ?cliupd=1 - "CLI updated" after start.
(function () {
  const T = window.__TAURI__;
  if (!T || !window.__MOCK__) return;
  const q = new URLSearchParams(location.search);
  const subs = {};
  const listen0 = T.event.listen;
  T.event.listen = (name, fn) => { (subs[name] = subs[name] || []).push(fn); return listen0(name, fn); };
  const emit = (name, payload) => (subs[name] || []).forEach((fn) => setTimeout(() => fn({ payload }), 0));
  const invoke0 = T.core.invoke;
  const iso = (ms) => new Date(ms).toISOString();
  const ago = (min) => iso(Date.now() - min * 60000);
  const tasks = () => invoke0('list_tasks');            // dev-mock hands out its own array: changes here are its changes
  const ACTIVE = ['preparing', 'working', 'checking', 'reviewing', 'merging'];

  // ---------- CLI updates ----------
  const status = {
    claude: { installed: '2.1.4', latest: '2.1.5', method: 'native', cmd: ['claude', 'update'], major: false, path: '/Users/dev/.local/bin/claude' },
    codex: { installed: '0.47.0', latest: '0.47.0', method: 'npm', cmd: ['npm', 'install', '-g', '@openai/codex@latest'], major: false, path: '/opt/homebrew/bin/codex' },
    gemini: { installed: '0.9.1', latest: '1.0.0', method: 'brew', cmd: ['brew', 'upgrade', 'gemini-cli'], major: true, path: '/opt/homebrew/bin/gemini' },
  };
  const modes = {};
  const hist = {
    claude: [
      { env: 'claude', from: '2.1.1', to: '2.1.2', ok: false, output: 'claude --version: no answer', at: ago(60 * 24 * 6), method: 'native', rolled_back: true },
      { env: 'claude', from: '2.1.1', to: '2.1.3', ok: true, output: '', at: ago(60 * 24 * 3), method: 'native', rolled_back: false },
      { env: 'claude', from: '2.1.3', to: '2.1.4', ok: true, output: '', at: ago(60 * 20), method: 'native', rolled_back: false }],
  };
  const st = (env) => status[env] || { installed: null, latest: null, method: 'unknown', cmd: [], major: false, path: '' };
  const cmds = {
    cli_status: ({ env }) => Object.assign({}, st(env), { mode: modes[env] || 'auto', updating: false }),
    updates_history: ({ env }) => hist[env] || [],
    cli_update_mode: ({ env, mode }) => { modes[env] = mode; return null; },
    cli_update: ({ env }) => new Promise((ok) => setTimeout(() => {
      const s = st(env);
      const log = { env, from: s.installed, to: s.latest, ok: true, output: '', at: iso(Date.now()), method: s.method, rolled_back: false };
      (hist[env] = hist[env] || []).push(log);
      s.installed = s.latest; s.major = false;
      emit('cli-updated', { env, from: log.from, to: log.to });
      if (env === 'claude') setTimeout(() => emit('new-model', { env, id: 'claude-opus-6', name: 'Opus 6', auto: true }), 500);
      ok(log);
    }, 900)),
    // ---------- practices ----------
    startup_tax: ({ env, project }) => {
      const rules = [{ path: '/Users/dev/.claude/CLAUDE.md', lines: 236, tokens: 3120, long: true }];
      if (project) rules.push({ path: '/Users/dev/code/billing-api/CLAUDE.md', lines: 48, tokens: 610, long: false });
      if (env !== 'claude') rules[0] = { path: '/Users/dev/code/billing-api/AGENTS.md', lines: 61, tokens: 820, long: false };
      const r = rules.reduce((s, x) => s + x.tokens, 0);
      return { rules, skills: 14, skill_tokens: 1180, agents: 3, agent_tokens: 240, mcp_servers: env === 'claude' ? ['github', 'playwright'] : [], total_tokens: r + 1180 + 240 };
    },
    compact: async ({ taskId, keep }) => {
      const tk = (await tasks()).find((x) => x.id === taskId);
      if (!tk || !ACTIVE.includes(tk.status)) throw 'nothing_to_compact';
      setTimeout(() => {
        tk.handoff = 'Цель: ' + tk.title + '\nСделано: тесты на TTL, блокировка в ReportCache.\nРешения: ttl в конструкторе, monotonic.\nДальше: линтер.\nФайлы: src/reports/cache.py\nСохранить: ' + (keep || 'изменения кода и решения');
        tk.context_pct = 21;
        emit('task-changed', tk);
      }, 800);
      return null;
    },
  };

  // ---------- the swarm on dev-mock (story 291) ----------
  cmds.send_chat = async (args) => {
    const id = await invoke0('send_chat', args);
    const settings = await invoke0('get_settings');
    const chat = (await Promise.all(['claude', 'codex', 'gemini', 'opencode', 'cursor', 'qwen'].map((e) => invoke0('list_chats', { env: e })))).flat().find((c) => c.id === id);
    const env = (chat && chat.env) || (args.target.new && args.target.new.env);
    const sw = env && settings.envs && settings.envs[env] && settings.envs[env].last_swarm;
    if (!sw) return id;
    const known = new Set((await tasks()).map((x) => x.id));
    let tries = 0;
    const look = async () => {          // dev-mock makes the task ~0.7 s after the message
      const tk = (await tasks()).find((x) => x.chat_id === id && !known.has(x.id));
      if (!tk) { if (++tries < 20) setTimeout(look, 150); return; }
      const n = Math.max(2, Math.min(5, sw.n || 3)), now = Date.now();
      const letters = 'ABCDE'.slice(0, n).split('');
      const models = ['Opus 5.5', 'Sonnet 5.5', 'Sonnet 5.5', 'Opus 5.5', 'Sonnet 5.5'];
      tk.swarm = { mode: sw.mode || 'race', n };
      tk.runs = [{ role: 'planner', model: 'Opus 5.5', state: 'done', started_at: iso(now - 4000), finished_at: iso(now - 1000) }]
        .concat(letters.map((v, i) => ({ role: 'worker', model: models[i], state: 'running', variant: v, started_at: iso(now - 800 + i * 200) })));
      tk.swarm_view = { variants: letters.map((v, i) => ({ id: v, model: models[i], state: 'running', gate: null, score: null, summary: '', criteria: [], stats: null, cost_usd: null, files: [] })),
        advice: null, picked: null, stopped: false, parts: [], council: null };
      const tag = window.NSI18n ? window.NSI18n.t('mock.swarm_tag') : '· рой';
      if (chat && !chat.title.endsWith(tag)) chat.title += ' ' + tag;
      const mine = window.NS && window.NS.store && window.NS.store.findChat(id);
      if (mine && mine !== chat && !mine.title.endsWith(tag)) mine.title += ' ' + tag;
      emit('task-changed', tk);
      if (window.NS && window.NS.store) window.NS.store.emit('chats', env);
    };
    setTimeout(look, 750);
    return id;
  };

  T.core.invoke = (cmd, args) => {
    if (!cmds[cmd]) return invoke0(cmd, args);
    try { return Promise.resolve(cmds[cmd](args || {})); } catch (e) { return Promise.reject(e); }
  };

  // ---------- deep links for screenshots ----------
  window.addEventListener('ns-ready', () => setTimeout(async () => {
    const ctx = Number(q.get('ctx'));
    if (ctx || q.get('handoff')) {
      for (const tk of await tasks()) {
        if (!ACTIVE.includes(tk.status) && tk.status !== 'needs_human') continue;
        if (ctx) tk.context_pct = ctx;
        if (q.get('handoff')) {
          tk.handoff = 'Цель: ' + tk.title + '\nСделано: курсор и валидация, 7 из 9 тестов.\nРешения: курсор непрозрачный, старый ?page= пока живёт.\nДальше: два теста на границе страницы.\nФайлы: src/invoices/routes.py, src/invoices/cursor.py';
          tk.port = 4010; tk.db_name = 'billing_api_' + tk.id.replace(/\W/g, '_');
        }
        emit('task-changed', tk);
      }
    }
    if (q.get('newmodel')) emit('new-model', { env: 'claude', id: 'claude-opus-6', name: 'Opus 6', auto: q.get('newmodel') !== 'ask' });
    if (q.get('cliupd')) emit('cli-updated', { env: 'claude', from: '2.1.4', to: '2.1.5' });
  }, 500));
})();
