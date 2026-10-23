// Fakes of the catalog, GitHub publishing and the history for the browser without Tauri (task 24): market_*, gh_publish, git_log_page,
// git_revert_commit, commit_stat, commit_file_diff, pull_base. Switches in the address: ?mkoffline=1 (a source does not answer, its saved list is shown),
// ?pub=protected | nologin | ghmissing (what gh_publish answers), ?hist=<n> (commits, default 1000). The project docs-tool (p3) has no commits.
(function () {
  const T = window.__TAURI__;
  if (!T || !window.__MOCK__) return;
  const q = new URLSearchParams(location.search);
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const NOW = Date.now();

  // ---------- catalog: 10 000 skills, 140 MCP servers, 14 mods ----------
  const WORDS = ['invoice', 'pdf', 'sql', 'review', 'refactor', 'docker', 'react', 'rust', 'testing', 'migrate', 'deploy', 'logs', 'api', 'csv', 'regex', 'commit', 'lint', 'seo', 'charts', 'email', 'auth', 'cache', 'queue', 'i18n', 'a11y'];
  const AUTHORS = ['anthropics', 'vercel-labs', 'nezero', 'mattpocock', 'obra', 'huggingface', 'supabase', 'stripe', 'cloudflare', 'jetbrains'];
  const LIC = ['MIT', 'Apache-2.0', 'MIT', 'BSD-3-Clause', null, 'MIT'];
  const mk = (kind, i, source) => {
    const w1 = WORDS[(i * 7) % WORDS.length], w2 = WORDS[(i * 11 + 3) % WORDS.length];
    const base = { id: source + ':' + kind + '-' + i, kind, name: w1 + '-' + w2 + (i % 5 === 0 ? '-' + i : ''), source, author: AUTHORS[i % AUTHORS.length], license: LIC[i % LIC.length],
      updated: new Date(NOW - (i % 400) * 86400000).toISOString().slice(0, 10), installs: kind === 'skill' ? 90000 - ((i * 37) % 89000) : null, stars: kind !== 'skill' ? 4000 - ((i * 13) % 3900) : null,
      version: '1.' + (i % 9) + '.' + (i % 4), homepage: 'https://github.com/' + AUTHORS[i % AUTHORS.length] + '/' + w1 + '-' + w2, origin: { type: 'none' } };
    base.description = kind === 'skill' ? 'Teaches the agent to work with ' + w1 + ' and ' + w2 + ': steps, checks and ready commands. Скилл для ' + w1 + '.'
      : kind === 'mcp' ? 'MCP server for ' + w1 + ' and ' + w2 + ' - tools the agent can call.' : 'Mod for the Claude Code interface: a ' + w1 + ' panel and a ' + w2 + ' command.';
    return base;
  };
  const CATALOG = { skill: [], mcp: [], mod: [] };
  for (let i = 0; i < 10000; i++) CATALOG.skill.push(mk('skill', i, i % 3 ? 'skills.sh' : 'anthropics-skills'));
  for (let i = 0; i < 140; i++) CATALOG.mcp.push(mk('mcp', i, 'mcp-registry'));
  for (let i = 0; i < 14; i++) CATALOG.mod.push(mk('mod', i, 'awesome-mods'));
  CATALOG.skill[1].name = 'broken-archive'; CATALOG.skill[1].description = 'An item whose download fails (to see the error: nothing is written).';
  const ALL = () => [].concat(CATALOG.skill, CATALOG.mcp, CATALOG.mod);
  const find = (id) => ALL().find((x) => x.id === id);
  const installed = [{ id: 'skills.sh:skill-3', version: 'a1b2c3d', update: null, name: CATALOG.skill[3].name, kind: 'skill', scope: 'global', snapshot: 's1', at: new Date(NOW).toISOString() },
    { id: 'skills.sh:skill-4', version: 'a1b2c3d', update: 'e4f5a6b', name: CATALOG.skill[4].name, kind: 'skill', scope: 'global', snapshot: 's2', at: new Date(NOW).toISOString() },
    { id: 'mcp-registry:mcp-2', version: '0.4.1', update: null, name: CATALOG.mcp[2].name, kind: 'mcp', scope: 'global', snapshot: 's3', at: new Date(NOW).toISOString() }];
  let sources = [
    { id: 'skills.sh', kind: 'skill', url: 'https://skills.sh', enabled: true, api: 'skills_sh', name: 'skills.sh', builtin: true, note: null },
    { id: 'anthropics-skills', kind: 'skill', url: 'https://github.com/anthropics/skills', enabled: true, api: 'marketplace', name: 'anthropics/skills', builtin: true, note: null },
    { id: 'skillsmp', kind: 'skill', url: 'https://skillsmp.com', enabled: false, api: 'skills_mp', name: 'SkillsMP', builtin: true, note: 'terms_not_checked' },
    { id: 'mcp-registry', kind: 'mcp', url: 'https://registry.modelcontextprotocol.io', enabled: true, api: 'mcp_registry', name: 'MCP Registry', builtin: true, note: null },
    { id: 'smithery', kind: 'mcp', url: 'https://registry.smithery.ai', enabled: false, api: 'smithery', name: 'Smithery', builtin: true, note: 'terms_not_checked' },
    { id: 'awesome-mods', kind: 'mod', url: 'https://github.com/example/awesome-claude-mods', enabled: true, api: 'github_dir', name: 'awesome-claude-mods', builtin: true, note: null },
  ];
  const PAGE = 40;
  const plans = new Map();

  // ---------- history: a DAG with feature branches and merges, lanes by the algorithm of core::git ----------
  class Lanes {
    constructor() { this.slots = []; this.from = []; }
    place(sha, parents) {
      const free = () => { const k = this.slots.indexOf(null); return k < 0 ? this.slots.length : k; };
      let node = this.slots.indexOf(sha); if (node < 0) node = free();
      const edges = []; this.slots.forEach((s, i) => { if (s != null) edges.push([this.from[i], s === sha ? node : i]); });
      this.slots.forEach((s, i) => { if (s === sha) this.slots[i] = null; this.from[i] = i; });
      const put = (k, p, f) => { while (this.slots.length <= k) { this.slots.push(null); this.from.push(0); } this.slots[k] = p; this.from[k] = f; };
      if (parents[0]) put(node, parents[0], node);
      parents.slice(1).forEach((p) => put(free(), p, node));
      while (this.slots.length && this.slots[this.slots.length - 1] == null) { this.slots.pop(); this.from.pop(); }
      return [node, edges];
    }
  }
  const SUBJECTS = ['Fix cursor pagination', 'Add invoice export', 'Refactor db session', 'Update dependencies', 'Cover rounding with tests', 'Rename column created_at', 'Speed up report query', 'Fix flaky test',
    'Add rate limit to webhooks', 'Document the API', 'Handle empty cart', 'Исправь округление в итогах', 'Добавь экспорт в CSV', 'Merge branch feature', 'Tidy imports', 'Bump version'];
  const WHO = ['Maya Lin', 'Nezeron', 'Nightshift', 'Dev Patel', 'Sam Okoro'];
  const TASKS = ['t-inv', 't-round', 't-chlog', 't-cur', 't-hero'];
  function buildHistory(n) {
    const old = [];                            // oldest first
    let id = 0;
    const mkc = (parents, subject) => { const c = { sha: (0x1000000 + id * 7919 + 0xabc0000).toString(16).padStart(8, '0') + (id * 2654435761 >>> 0).toString(16).padStart(8, '0') + '0000000000000000000000000000', parents: parents.map((p) => p.sha), subject: subject || SUBJECTS[(id * 5) % SUBJECTS.length] }; id++; old.push(c); return c; };
    let main = mkc([], 'Initial commit');
    while (old.length < n) {
      if (id % 9 === 3 && old.length < n - 12) {
        let b = main; const k = 2 + (id % 4);
        for (let j = 0; j < k; j++) { b = mkc([b]); if (j % 2 === 0) main = mkc([main]); }
        main = mkc([main, b], 'Merge branch feature/' + WORDS[id % WORDS.length]);
        main.merge = true;
      } else main = mkc([main]);
    }
    const rows = old.reverse();                // newest first = topo order
    const lanes = new Lanes();
    const out = rows.map((c, i) => {
      const [lane, e] = lanes.place(c.sha, c.parents);
      if (out_prev) out_prev.edges = e;
      const r = { sha: c.sha, parents: c.parents, author: WHO[i % WHO.length], time: Math.floor((NOW - (i * 3300 + (i % 7) * 400) * 1000) / 1000), subject: c.subject, refs: [], task: null, lane, edges: [] };
      if (c.merge && i % 2 === 0) r.task = TASKS[(i / 2 | 0) % TASKS.length];
      out_prev = r; return r;
    });
    out_prev = null;
    out[0].refs = ['main', 'origin/main']; if (out[40]) out[40].refs = ['tag: v0.3.0']; if (out[120]) out[120].refs = ['tag: v0.2.0', 'ns/inv-pagination'];
    if (out[6]) out[6].refs = ['feature/export']; if (out[Math.floor(n / 2)]) out[Math.floor(n / 2)].refs = ['tag: v0.1.0'];
    return out;
  }
  let out_prev = null;
  const histCache = {};
  const history = (pid) => (histCache[pid] = histCache[pid] || (pid === 'p3' ? [] : buildHistory(+q.get('hist') || 1000)));
  const FILES = [['src/invoices/routes.py', 64, 12], ['src/invoices/cursor.py', 41, 0], ['tests/test_invoices_pagination.py', 88, 3], ['README.md', 2, 1]];
  const DIFF_LINES = [{ kind: 'hunk', text: '@@ -12,7 +12,9 @@ def list_invoices(cursor=None, limit=50):' }, { kind: 'ctx', text: ' ' + '    db = get_session()' },
    { kind: 'del', text: '-    rows = db.query(Invoice).offset(page * 50).limit(50)' }, { kind: 'add', text: '+    after = decode_cursor(cursor) if cursor else None' },
    { kind: 'add', text: '+    rows = db.query(Invoice).order_by(Invoice.created_at, Invoice.id).limit(min(limit, 200) + 1).all()' }, { kind: 'ctx', text: '     return {"items": rows[:limit]}' }];
  const projectOf = (path) => (window.__MOCK_PROJECTS && window.__MOCK_PROJECTS(path)) || (/docs-tool/.test(path) ? 'p3' : /web-site/.test(path) ? 'p2' : 'p1');

  const cmds = {
    market_search: async ({ kind, query, cursor }) => {
      await sleep(120);
      const qq = String(query || '').toLowerCase();
      const list = CATALOG[kind].filter((x) => !qq || (x.name + ' ' + x.description).toLowerCase().includes(qq));
      const at = +cursor || 0;
      const offline = q.get('mkoffline') ? [{ source: kind === 'skill' ? 'skills.sh' : kind === 'mcp' ? 'smithery' : 'awesome-mods', reason: 'no_connection', cached_at: new Date(NOW - 3 * 86400000).toISOString() }] : [];
      return { items: list.slice(at, at + PAGE), cursor: at + PAGE < list.length ? String(at + PAGE) : null, offline };
    },
    market_item: async ({ id }) => {
      await sleep(80);
      const it = find(id);
      if (!it) throw { kind: 'not_found', detail: id };
      const mcp = it.kind === 'mcp';
      return { item: it, readme: it.kind === 'skill' ? '---\nname: ' + it.name + '\ndescription: ' + it.description + '\n---\n\n# ' + it.name + '\n\nWhen the task mentions it, read the files first, then run the checks.\n' : null,
        env_vars: mcp ? [{ name: 'API_TOKEN', secret: true, required: true, description: 'Token of the service' }, { name: 'WORKSPACE', secret: false, required: false, description: 'Workspace id' }] : [],
        runs: mcp ? (+id.split('-').pop() % 2 ? 'https://mcp.example.com/sse' : 'npx -y @example/' + it.name + '@1.2.3') : null, needs: mcp ? ['node'] : [] };
    },
    market_plan: async ({ item, env, scope }) => {
      await sleep(150);
      const it = item.id ? find(item.id) : { id: 'link:' + item.link, kind: item.kind, name: String(item.link).split('/').filter(Boolean).pop() || 'from-link', source: 'link', description: '', author: null, license: null, updated: null, installs: null, stars: null, version: null, homepage: null, origin: { type: 'none' } };
      if (!it) throw { kind: 'not_found', detail: 'item' };
      if (/broken/.test(it.name)) throw { kind: 'timeout', detail: 'timeout' };
      const root = scope === 'global' ? '~/.' + (env === 'claude' ? 'claude' : env) : scope.project + '/.' + (env === 'claude' ? 'claude' : env);
      const mcp = it.kind === 'mcp', mod = it.kind === 'mod';
      const runsRemote = mcp && +it.id.split('-').pop() % 2;
      const plan = { id: 'plan' + plans.size, item: it, env, scope, installed: !!installed.find((x) => x.id === it.id),
        writes: mcp ? [root + '/settings.json (mcpServers.' + it.name + ')'] : mod ? [root + '/mods/' + it.name + '/mod.json', root + '/mods/' + it.name + '/index.js', root + '/mods/' + it.name + '/panel.css'] : [root + '/skills/' + it.name + '/SKILL.md', root + '/skills/' + it.name + '/scripts/run.sh'],
        executable: mcp ? ['mcp:' + it.name] : mod ? [it.name + '/index.js'] : [it.name + '/scripts/run.sh'], runs: mcp ? (runsRemote ? 'https://mcp.example.com/sse' : 'npx -y @example/' + it.name + '@1.2.3') : null, needs: mcp ? ['node'] : [],
        warnings: mcp ? [runsRemote ? 'remote' : 'runs_code'] : mod ? ['runs_code'] : ['runs_code'],
        texts: it.kind === 'skill' ? [{ path: 'skills/' + it.name + '/SKILL.md', text: '# ' + it.name + '\n\nInstructions for the agent.\n' }] : [], kit: {} };
      plans.set(plan.id, plan);
      return plan;
    },
    market_apply: async ({ plan, enabledExec }) => {
      await sleep(250);
      const p = plans.get(plan.id);
      if (!p) throw { kind: 'changed' };
      const at = installed.findIndex((x) => x.id === p.item.id);
      const rec = { id: p.item.id, version: 'f00ba12', update: null, name: p.item.name, kind: p.item.kind, scope: 'global', snapshot: 'snap' + installed.length, at: new Date().toISOString() };
      if (at >= 0) installed[at] = rec; else installed.push(rec);
      window.__MOCK_APPLIED = { id: p.item.id, enabledExec };
      return rec.snapshot;
    },
    market_installed: () => installed.slice(),
    market_remove: ({ id }) => { const at = installed.findIndex((x) => x.id === id); if (at >= 0) installed.splice(at, 1); return null; },
    market_sources: () => sources.slice(),
    market_set_sources: ({ sources: list }) => { sources = list; return sources.slice(); },
    set_secret: () => ({ keyring: true }),

    gh_publish: async ({ name }) => {
      await sleep(600);
      const mode = q.get('pub');
      if (mode === 'protected') throw { kind: 'protected_in_history', paths: ['.env', 'config/keys/id_rsa', 'secrets.json'] };
      if (mode === 'nologin') throw { kind: 'gh_not_logged_in' };
      if (mode === 'ghmissing') throw { kind: 'gh_missing' };
      if (/taken|billing/.test(name)) throw { kind: 'name_taken' };
      return 'https://github.com/dev/' + name;
    },
    git_log_page: async ({ path, skip, limit }) => { await sleep(60); return history(projectOf(path)).slice(skip, skip + Math.min(limit, 1000)); },
    git_revert_commit: ({ sha }) => ({ sha: 'c0ffee' + sha.slice(6, 12), pushed: sha.charCodeAt(0) % 2 === 0 }),
    commit_stat: () => FILES.map(([path, insertions, deletions]) => ({ path, insertions, deletions })),
    commit_file_diff: () => ({ lines: DIFF_LINES, binary: false }),
    pull_base: () => null,
  };
  // the ui asks the commands with real names; the old fakes answer `diff_file` with other rows, so here it is the same format for a task commit
  const invoke0 = T.core.invoke;
  T.core.invoke = (cmd, args) => (cmds[cmd] ? new Promise((res, rej) => { Promise.resolve().then(() => cmds[cmd](args || {})).then(res, rej); }) : invoke0(cmd, args));
  window.NSMockMarket = { catalog: CATALOG, installed, history };
})();
