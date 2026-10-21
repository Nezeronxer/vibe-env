// Fake Tauri backend for viewing the UI in a plain browser. Loaded by mock-loader.js only outside Tauri (no __TAURI_INTERNALS__).
// Query: ?scenario=empty|noagent  ?theme=dark|light  ?lang=ru|en  ?project=<id>  ?open=<taskId>  ?settings=project|general
(function () {
  const q = new URLSearchParams(location.search);
  if (q.get('theme')) document.documentElement.setAttribute('data-theme', q.get('theme'));
  const scenario = q.get('scenario') || 'full';
  const now = Date.now();
  const ago = (min) => new Date(now - min * 60000).toISOString();
  const ahead = (min) => new Date(now + min * 60000).toISOString();

  let settings = { max_parallel: 2, paused: q.get('paused') === '1', language: q.get('lang') || 'auto', hotkey: 'Alt+Space',
    telegram: { enabled: false, chat_id: null, proxy: null, digest_at: null },
    custom_agents: [{ id: 'aider', name: 'Aider', run: 'aider --message {prompt}', resume: null }], default_project: null };
  let agents = [
    { id: 'claude', name: 'Claude Code', available: scenario !== 'noagent', hint: 'npm i -g @anthropic-ai/claude-code' },
    { id: 'codex', name: 'Codex CLI', available: true, hint: 'npm i -g @openai/codex' },
    { id: 'gemini', name: 'Gemini CLI', available: false, hint: 'npm i -g @google/gemini-cli' },
    { id: 'opencode', name: 'opencode', available: true, hint: 'npm i -g opencode-ai' },
    { id: 'aider', name: 'Aider', available: true, hint: '' },
  ];
  const P = (id, name, path, extra) => Object.assign({ id, name, path, base_branch: 'main', check_cmd: 'npm test', setup_cmd: 'npm ci',
    copy_files: ['.env*'], finish: 'local', ask_outward: true, agent: 'claude', reviewer: null, fallback_agent: null }, extra);
  let projects = scenario === 'empty' ? [] : [
    P('p1', 'billing-api', '/Users/dev/code/billing-api', { finish: 'push', fallback_agent: 'codex' }),
    P('p2', 'web-site', '/Users/dev/code/web-site', { finish: 'pr', agent: 'codex', reviewer: 'claude' }),
    P('p3', 'docs-tool', '/Users/dev/code/docs-tool', { check_cmd: null, setup_cmd: null }),
  ];
  const T = (id, project_id, title, status, extra) => Object.assign({
    id, project_id, title, prompt: title, agent: project_id === 'p2' ? 'codex' : 'claude', status, branch: 'ns/' + id.slice(2, 8).padEnd(6, '0') + '-' + title.toLowerCase().replace(/[^a-z0-9]+/g, '-').slice(0, 24).replace(/-$/, ''),
    worktree: ['queued', 'done', 'cancelled', 'reverted'].includes(status) ? null : '/Users/dev/.nightshift/wt/' + id, session: 's-' + id, attempts: 0, review_rounds: 0,
    question: null, resume_at: null, outcome: {}, created_at: ago(120), started_at: null, finished_at: null }, extra);
  const GOOD = { passed: true, tail: 'running 42 tests\n...........................................\ntest result: ok. 42 passed; 0 failed' };
  const RED = { passed: false, tail: 'FAIL src/invoice.test.ts\n  ● totals › rounds half up\n    expected 10.05\n    received 10.04\n\nTests: 1 failed, 41 passed' };
  let tasks = scenario === 'empty' ? [] : [
    T('t-needq', 'p1', 'Add pagination to the invoices endpoint', 'needs_human', { started_at: ago(14), created_at: ago(20),
      question: { kind: 'agent', text: 'The invoices table is large. Should pagination be cursor-based (stable under inserts) or offset-based (simpler)?', options: ['Cursor-based', 'Offset-based'] } }),
    T('t-needout', 'p2', 'Redesign the pricing page hero', 'awaiting_outward', { started_at: ago(50), created_at: ago(60),
      question: { kind: 'outward', text: 'Done and checked. Open a pull request for ns/needou-redesign-the-pricing?', options: ['Open PR', 'No'] },
      outcome: { summary: 'New hero with price toggle', files_changed: 5, insertions: 142, deletions: 38, tests: GOOD, review: { approved: true, issues: [] } } }),
    T('t-xss', 'p3', '<img src=x onerror=alert(1)>', 'needs_human', { started_at: ago(5), created_at: ago(6), branch: 'ns/xss000-<img src=x onerror=alert(1)>',
      question: { kind: 'agent', text: 'Is <img src=x onerror=alert(1)> safe to render?', options: ['<img src=x onerror=alert(1)>', 'No'] } }),
    T('t-work', 'p1', 'Fix rounding in invoice totals', 'working', { started_at: ago(7), created_at: ago(8), attempts: 1 }),
    T('t-check', 'p1', 'Rename customer_id to account_id in the schema', 'checking', { started_at: ago(22), created_at: ago(25) }),
    T('t-prep', 'p2', 'Add dark mode toggle to the header', 'preparing', { started_at: ago(0.2), created_at: ago(0.3) }),
    T('t-review', 'p2', 'Compress hero images', 'reviewing', { started_at: ago(31), created_at: ago(33), outcome: { files_changed: 9, insertions: 12, deletions: 4, tests: GOOD } }),
    T('t-merge', 'p2', 'Update footer links', 'merging', { started_at: ago(40), created_at: ago(42), outcome: { files_changed: 2, insertions: 6, deletions: 6, tests: GOOD, review: { approved: true, issues: [] } } }),
    T('t-limit', 'p2', 'Migrate the blog to the new layout', 'paused_limit', { started_at: ago(90), created_at: ago(95), resume_at: ahead(95) }),
    T('t-blocked', 'p3', 'Fix typos in the README', 'merge_blocked', { started_at: ago(30), created_at: ago(35), outcome: { files_changed: 1, insertions: 3, deletions: 3, tests: null, review: { approved: true, issues: [] } } }),
    T('t-q1', 'p1', 'Write a changelog entry for 2.4', 'queued', { created_at: ago(3) }),
    T('t-q2', 'p1', 'Add rate limiting to the webhook receiver', 'queued', { created_at: ago(2) }),
    T('t-done1', 'p1', 'Add currency field to payments', 'done', { started_at: ago(180), finished_at: ago(160), created_at: ago(185), review_rounds: 1,
      outcome: { summary: 'Added currency column and validation', files_changed: 6, insertions: 120, deletions: 14, tests: GOOD, review: { approved: true, issues: [] }, commit: 'a3f9c21d8e' } }),
    T('t-donenp', 'p1', 'Bump axios to the latest minor', 'done_not_pushed', { started_at: ago(260), finished_at: ago(250), created_at: ago(262),
      outcome: { summary: 'Bumped axios 1.7 to 1.9', files_changed: 2, insertions: 20, deletions: 20, tests: GOOD, review: { approved: true, issues: [] }, commit: '77be10c4aa', error: 'push rejected: origin is ahead of main' } }),
    T('t-donepr', 'p2', 'Add sitemap generation', 'done', { started_at: ago(300), finished_at: ago(285), created_at: ago(305),
      outcome: { summary: 'Generates sitemap.xml at build time', files_changed: 4, insertions: 77, deletions: 2, tests: GOOD, review: { approved: true, issues: [] }, pr_url: 'https://github.com/acme/web-site/pull/118' } }),
    T('t-nottests', 'p3', 'Describe config options in the docs', 'done', { started_at: ago(400), finished_at: ago(390), created_at: ago(402),
      outcome: { summary: 'New "Configuration" page', files_changed: 3, insertions: 88, deletions: 0, tests: null, review: { approved: true, issues: [] }, commit: 'c02d55e1b7' } }),
    T('t-fail1', 'p1', 'Switch the job queue to Redis streams', 'failed', { started_at: ago(500), finished_at: ago(470), created_at: ago(505), attempts: 3,
      outcome: { summary: null, files_changed: 8, insertions: 210, deletions: 96, tests: RED, error: 'tests still failing after 3 attempts' } }),
    T('t-fail2', 'p2', 'Upgrade the build tooling', 'failed', { started_at: ago(700), finished_at: ago(660), created_at: ago(705), review_rounds: 2,
      outcome: { files_changed: 11, insertions: 300, deletions: 280, tests: GOOD, review: { approved: false, issues: ['The new config drops the legacy browser targets.', 'Source maps are no longer emitted in production.'] }, error: 'review requested changes twice' } }),
    T('t-cancel', 'p3', 'Draft an architecture overview', 'cancelled', { started_at: ago(900), finished_at: ago(895), created_at: ago(905) }),
    T('t-revert', 'p3', 'Reformat all docs headings', 'reverted', { started_at: ago(1000), finished_at: ago(990), created_at: ago(1002),
      outcome: { summary: 'Normalised headings', files_changed: 14, insertions: 40, deletions: 40, tests: null, review: { approved: true, issues: [] }, commit: 'e19ab44c01' } }),
    T('t-old', 'p1', 'Remove the deprecated v1 routes', 'done', { started_at: ago(60 * 24 * 12), finished_at: ago(60 * 24 * 12 - 20), created_at: ago(60 * 24 * 12 + 5),
      outcome: { summary: 'Dropped /v1/* routes', files_changed: 7, insertions: 4, deletions: 210, tests: GOOD, review: { approved: true, issues: [] }, commit: '5d7120aa33' } }),
  ];

  // ---- logs ----
  const mk = (min, kind, text) => ({ at: ago(min), kind, text });
  const logs = {};
  logs['t-work'] = [mk(7, 'stage', 'preparing'), mk(6.8, 'git', 'worktree created on ns/twork0-fix-rounding-in-invoice'), mk(6.5, 'stage', 'working'),
    mk(6.4, 'agent', 'Looking at how totals are computed in src/invoice.ts.'), mk(6.2, 'tool', 'Read src/invoice.ts'), mk(6, 'tool', 'Grep "toFixed" src/'),
    mk(5.5, 'agent', 'The total is rounded per line item and then summed. Switching to round once at the end.'), mk(5, 'tool', 'Edit src/invoice.ts'),
    mk(4.5, 'stage', 'checking'), mk(4.4, 'check', 'npm test'), mk(3.9, 'check', 'FAIL src/invoice.test.ts: expected 10.05, received 10.04'),
    mk(3.8, 'stage', 'working'), mk(3.7, 'agent', 'One test expects half-up rounding. Adjusting the helper.'), mk(3.2, 'tool', 'Edit src/money.ts'), mk(2, 'tool', 'Edit src/invoice.test.ts')];
  logs['t-xss'] = [mk(5, 'stage', 'working'), mk(4, 'agent', '<img src=x onerror=alert(1)>')];
  logs['t-needq'] = [mk(14, 'stage', 'preparing'), mk(13, 'stage', 'working'), mk(12, 'agent', 'Reading the current /invoices handler.'), mk(11, 'tool', 'Read src/routes/invoices.ts'),
    mk(10, 'agent', 'The table has about 2M rows. I need to know which pagination style you prefer.'), mk(9.8, 'stage', 'needs_human')];
  logs['t-fail1'] = [mk(500, 'stage', 'preparing'), mk(498, 'stage', 'working'), mk(490, 'stage', 'checking'), mk(488, 'check', 'npm test'), mk(487, 'error', 'FAIL src/queue.test.ts'),
    mk(486, 'stage', 'working'), mk(480, 'stage', 'checking'), mk(478, 'error', 'FAIL src/queue.test.ts'), mk(472, 'error', 'tests still failing after 3 attempts'), mk(470, 'stage', 'failed')];
  logs['t-done1'] = [mk(180, 'stage', 'preparing'), mk(178, 'stage', 'working'), mk(170, 'stage', 'checking'), mk(166, 'check', 'npm test: 42 passed'), mk(165, 'stage', 'reviewing'),
    mk(163, 'review', 'approved'), mk(162, 'stage', 'merging'), mk(160, 'git', 'squashed to main a3f9c21'), mk(160, 'stage', 'done')];
  const defaultLog = (tk) => [mk(30, 'stage', 'preparing'), mk(29, 'stage', 'working'), mk(25, 'agent', 'Working on: ' + tk.title), mk(20, 'tool', 'Edit src/index.ts')];

  // ---- bus ----
  const subs = {};
  const emit = (name, payload) => (subs[name] || []).forEach((fn) => setTimeout(() => fn({ payload }), 0));
  const byId = (id) => tasks.find((x) => x.id === id);
  const changed = (tk) => emit('task-changed', JSON.parse(JSON.stringify(tk)));
  const pushLog = (id, ev) => { (logs[id] = logs[id] || []).push(ev); emit('task-log', { id, event: ev }); };
  const fail = (m) => Promise.reject(m);

  const repos = { p1: { is_repo: true, base_branch: 'main', current_branch: 'main', dirty: false, origin_url: 'git@github.com:acme/billing-api.git', web_url: 'https://github.com/acme/billing-api', ahead: 3, behind: 0 },
    p2: { is_repo: true, base_branch: 'main', current_branch: 'feature/nav', dirty: true, origin_url: 'https://github.com/acme/web-site.git', web_url: 'https://github.com/acme/web-site', ahead: 0, behind: 2 },
    p3: { is_repo: true, base_branch: 'main', current_branch: 'main', dirty: false, origin_url: null, web_url: null, ahead: 0, behind: 0 } };
  let branches = [{ name: 'ns/twork0-fix-rounding', merged: false }, { name: 'ns/tdone1-add-currency-field', merged: true }, { name: 'ns/tdonen-bump-axios', merged: true }, { name: 'ns/told00-remove-v1-routes', merged: true }];

  const cmds = {
    list_projects: () => projects,
    add_project: ({ path }) => {
      if (/nogit/.test(path)) return fail('not a git repository: ' + path);
      const p = P('p' + (projects.length + 1), path.split('/').filter(Boolean).pop() || 'project', path);
      projects.push(p); repos[p.id] = { is_repo: true, base_branch: 'main', current_branch: 'main', dirty: false, origin_url: null, web_url: null, ahead: 0, behind: 0 }; return p;
    },
    update_project: ({ project }) => { projects = projects.map((p) => (p.id === project.id ? project : p)); return null; },
    remove_project: ({ id }) => { projects = projects.filter((p) => p.id !== id); return null; },
    init_repo: () => null,
    repo_info: ({ id }) => repos[id] || { is_repo: true, base_branch: 'main', current_branch: 'main', dirty: false, origin_url: null, ahead: 0, behind: 0 },
    list_tasks: () => tasks,
    get_log: ({ id, limit }) => (logs[id] || defaultLog(byId(id) || { title: '' })).slice(-limit),
    submit_task: ({ projectId, prompt, agent }) => {
      const tk = T('t-n' + Date.now(), projectId, prompt.split('\n')[0].slice(0, 80), 'queued', { prompt, agent: agent || (projects.find((p) => p.id === projectId) || {}).agent, created_at: new Date().toISOString() });
      tasks.push(tk); changed(tk);
      setTimeout(() => { tk.status = 'working'; tk.started_at = new Date().toISOString(); changed(tk); }, 1500);
      return tk.id;
    },
    answer: ({ id, text }) => { const tk = byId(id); if (!tk || tk.status !== 'needs_human') return fail('task is not waiting'); tk.status = 'working'; tk.question = null; changed(tk); pushLog(id, { at: new Date().toISOString(), kind: 'human', text }); return null; },
    approve_outward: ({ id, yes }) => { const tk = byId(id); tk.status = yes ? 'done' : 'done_not_pushed'; tk.question = null; tk.finished_at = new Date().toISOString(); changed(tk); return null; },
    cancel: ({ id }) => { const tk = byId(id); tk.status = 'cancelled'; tk.finished_at = new Date().toISOString(); changed(tk); return null; },
    retry: ({ id }) => { const tk = byId(id); tk.status = 'queued'; tk.attempts = 0; tk.outcome = {}; tk.finished_at = null; changed(tk); return null; },
    remove_task: ({ id }) => { tasks = tasks.filter((x) => x.id !== id); emit('task-removed', id); return null; },
    get_diffstat: ({ taskId }) => (['t-done1', 't-fail1'].includes(taskId)
      ? [{ path: 'src/payments/currency.ts', insertions: 64, deletions: 3 }, { path: 'src/payments/validate.ts', insertions: 31, deletions: 9 }, { path: 'migrations/0042_currency.sql', insertions: 25, deletions: 2 }]
      : fail('command get_diffstat not found')),
    undo: ({ id }) => { const tk = byId(id); tk.status = 'reverted'; changed(tk); return null; },
    push_base: ({ projectId }) => { if (repos[projectId]) repos[projectId].ahead = 0; return null; },
    pull_base: ({ projectId }) => { if (repos[projectId]) repos[projectId].behind = 0; return null; },
    pause_all: ({ paused }) => { settings.paused = paused; return null; },
    list_agents: () => agents,
    get_settings: () => settings,
    set_settings: ({ settings: s }) => { settings = s; agents = agents.filter((a) => ['claude', 'codex', 'gemini', 'opencode'].includes(a.id)).concat(s.custom_agents.map((c) => ({ id: c.id, name: c.name, available: true, hint: '' }))); return null; },
    github_info: ({ projectId }) => (projectId === 'p3' ? { ghHint: 'not signed in: run gh auth login', repoUrl: null, branches: [] }
      : { ghUser: 'dev-user', repoUrl: (repos[projectId] || {}).web_url, branches }),
    delete_merged_branches: () => { branches = branches.filter((b) => !b.merged); return null; },
    pr_state: () => 'open',
    open_path: ({ path }) => { console.log('[mock] open_path', path); return null; },
    open_editor: ({ path }) => { console.log('[mock] open_editor', path); return null; },
    open_terminal: ({ path }) => { console.log('[mock] open_terminal', path); return null; },
  };

  window.__TAURI__ = {
    core: { invoke: (cmd, args) => {
      if (!cmds[cmd]) return Promise.reject('unknown command ' + cmd);
      try { return Promise.resolve(cmds[cmd](args || {})); } catch (e) { return Promise.reject(String(e)); }
    } },
    event: { listen: (name, fn) => { (subs[name] = subs[name] || []).push(fn); return Promise.resolve(() => {}); } },
  };
  window.__MOCK__ = true;
  window.__MOCK_LOG = (id, kind, text) => pushLog(id, { at: new Date().toISOString(), kind, text });

  // live log feed for the working task so autoscroll can be seen
  let n = 0;
  const lines = [['agent', 'Re-running the suite after the rounding fix.'], ['tool', 'Bash npm test'], ['check', 'npm test: 42 passed'], ['agent', 'All green, preparing the summary.']];
  if (scenario === 'full' && !q.get('quiet')) setInterval(() => { if (n < lines.length) { pushLog('t-work', { at: new Date().toISOString(), kind: lines[n][0], text: lines[n][1] }); n++; } }, 4000);

  // deep links for screenshots
  window.addEventListener('ns-ready', () => {
    if (q.get('project')) window.NS.selectProject(q.get('project'));
    const open = q.get('open'); if (open) window.NS.openCard(open);
    const st = q.get('settings');
    if (st === 'project') window.NS.openProjectSettings(q.get('project') || 'p1');
    if (st === 'general') window.NS.openGeneralSettings();
    if (q.get('add') === '1') window.NS.openAddDialog();
  });
})();
