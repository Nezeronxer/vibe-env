// v7 dev-mock: the backend commands of terminal agents (app/src/agentcmds.rs) and ?demo=agents - four agents in two projects.
(function () {
  const T = window.__TAURI__;
  if (!T || !window.__MOCK__) return;
  const DIFF = 'diff --git a/src/invoices/cursor.py b/src/invoices/cursor.py\n--- a/src/invoices/cursor.py\n+++ b/src/invoices/cursor.py\n@@ -1,8 +1,19 @@\n import base64\n+import hmac\n \n-def encode(page):\n-    return str(page)\n+def encode(last_id, secret):\n+    raw = str(last_id).encode()\n+    sig = hmac.new(secret, raw, "sha256").hexdigest()[:16]\n+    return base64.urlsafe_b64encode(raw + b"." + sig.encode()).decode()\n \n def decode(token):\n-    return int(token)\n+    raw, sig = base64.urlsafe_b64decode(token).rsplit(b".", 1)\n+    return int(raw)\n';
  const files = [{ path: 'src/invoices/cursor.py', status: 'M', add: 14, del: 3 }, { path: 'src/invoices/routes.py', status: 'M', add: 6, del: 2 }, { path: 'tests/test_cursor.py', status: '?', add: 41, del: 0 }];
  const cmds = {
    agent_worktree: ({ projectId, name }) => ({ path: '/Users/dev/.nightshift/copies/' + projectId + '-term-' + String(name || 'agent').slice(0, 12).replace(/\W+/g, '-'), branch: 'ns/term-' + String(name || 'agent').slice(0, 14).toLowerCase().replace(/[^a-z0-9а-я]+/gi, '-') }),
    agent_worktree_remove: () => null,
    agent_changes: ({ cwd }) => ({ repo: true, branch: /copies/.test(cwd) ? 'ns/term-fix' : 'main', base: 'main', own_worktree: /copies/.test(cwd), files }),
    agent_file_diff: () => ({ text: DIFF, truncated: false }),
    agent_revert_file: () => null,
    agent_merge: () => ({ merged: true, sha: 'a1b2c3d' }),
    term_usage: ({ env }) => (env === 'claude' ? { tokens_in: 48210, tokens_out: 6120, usd: null } : env === 'codex' ? { tokens_in: 12890, tokens_out: 2210, usd: null } : { tokens_in: null, tokens_out: null, usd: null }),
    notify_agent: () => null,
  };
  const invoke0 = T.core.invoke;
  T.core.invoke = (cmd, args) => (cmds[cmd] ? Promise.resolve(cmds[cmd](args || {})) : invoke0(cmd, args));
  const q = new URLSearchParams(location.search);
  if (q.get('demo') !== 'agents') return;
  window.addEventListener('ns-ready', () => setTimeout(() => {
    const NS = window.NS;
    const open = (env, cwd, project, title, prompt, branch) => { const f = NS.term.envFields(env, { cwd, title, prompt, branch }); f.project = project; NS.term.open(f); };
    open('claude', '/Users/dev/code/billing-api', 'p1', 'Пагинация в /invoices', 'Сделай курсорную пагинацию в /invoices и тесты');
    open('codex', '/Users/dev/.nightshift/copies/p1-term-tests', 'p1', 'Тесты для payments', 'Покрой тестами payments', 'ns/term-tests-payments');
    open('gemini', '/Users/dev/code/web-site', 'p2', 'Герой на странице цен', '');
    open('opencode', '/Users/dev/code/web-site', 'p2', 'Ревью вёрстки', 'Сделай ревью вёрстки страницы цен');
    setTimeout(() => NS.tabs.focusPane(NS.tabs.state.tabs[0] && NS.tabs.state.tabs[1].focus), 400);
  }, 500));
})();
