// Boot: load state, pick the first screen, turn core events into store events (each screen patches only what changed).
(function () {
  const NS = window.NS;
  const { api, store } = NS;
  const s = store.s;
  const L = window.NSLogic;
  const fail = (e) => NS.shell.toast(NS.errHuman(api.errText(e)), true);

  // ---------- core events -> bus ----------
  function wireCore() {
    api.listen('task-changed', (task) => { s.tasks.set(task.id, task); store.emit('task', task); });
    api.listen('task-removed', (id) => { const prev = s.tasks.get(id); s.tasks.delete(id); if (prev) store.emit('taskremoved', prev); });
    api.listen('task-log', (pl) => {
      const ev = pl.event || {};
      if (['stage', 'check', 'agent'].includes(L.variant(ev.kind)[0]) && ev.text) s.lastLog.set(pl.id, String(ev.text).split('\n')[0].slice(0, 120));
      store.emit('tasklog', pl);
    });
    api.listen('project-changed', async (id) => {
      try { s.projects = await api.call('list_projects'); s.repos.delete(id); } catch (e) { fail(e); }
      store.emit('project', id);
    });
    api.listen('chat-msg', async (msg) => {
      if (!store.findChat(msg.chat_id)) { await Promise.all(s.envs.filter((e) => e.state !== 'missing').map((e) => store.loadChats(e.id))); for (const e of s.envs) store.emit('chats', e.id); }
      store.emit('chatmsg', msg);
    });
    // v3: one action arrives twice (start, result), linked by action.id; the chat list shows the current one as the task line
    api.listen('task-action', (pl) => {
      const a = pl && pl.action;
      if (!a) return;
      if (window.NSLive.inputOf(a).kind !== 'plan') s.lastLog.set(pl.id, ('● ' + a.name + ' ' + (NS.actions ? NS.actions.argOf(window.NSLive.inputOf(a), a) : '')).slice(0, 120));
      store.emit('taskaction', pl);
    });
    api.listen('chat-action', (pl) => { if (pl && pl.action) store.emit('chataction', pl); });
    api.listen('chat-delta', (pl) => { if (pl && pl.msgId) store.emit('chatdelta', pl); });
    api.listen('limits-changed', async (envId) => { await store.loadLimits(envId); store.emit('limits', envId); });
    api.listen('catalog-changed', async (envId) => { await store.loadCatalog(envId); store.emit('catalog', envId); });
    api.listen('frozen', (v) => store.emit('frozen', !!v));
    api.listen('focus-input', () => store.emit('focus-input'));
    // a click on a system notification: the chat of that task, where its "Нужен ты" block is (R15i.8)
    api.listen('open-task', (id) => { const tk = s.tasks.get(id); if (tk) NS.nav('env', { env: tk.agent, chat: tk.chat_id || undefined }); });
    api.listen('tauri://drag-enter', () => document.body.classList.add('dragging'));
    api.listen('tauri://drag-leave', () => document.body.classList.remove('dragging'));
    api.listen('tauri://drag-drop', async (pl) => {
      document.body.classList.remove('dragging');
      const path = pl && pl.paths && pl.paths[0];
      if (!path) return;
      const p = await NS.projects.addByPath(path);
      if (!p) return;
      if (NS.onboardingAdded) NS.onboardingAdded(p); else NS.shell.toast(NS.t('proj.added', { name: p.name }));
    });
    store.on('taskremoved', (prev) => { store.emit('task', Object.assign({}, prev, { status: 'reverted' })); });
    store.on('tasklog', (pl) => { const tk = s.tasks.get(pl.id); if (tk) store.emit('task', tk); });
  }

  // ---------- start ----------
  const isGitMissing = (e) => /git/i.test(api.errText(e)) && /(not found|missing|not installed|no such)/i.test(api.errText(e));
  async function start() {
    // data that cannot be read: nothing else is asked of the core, the person chooses (G16.4)
    try { const ds = await api.call('data_state'); if (ds && ds.unreadable) return NS.nav('unreadable', { copy: ds.unreadable, token: ds.token }); } catch (e) { /* an older core or the browser mock: go on */ }
    try { await store.loadAll(); }
    catch (e) {
      if (isGitMissing(e)) return NS.nav('nogit');
      NS.shell.toast(NS.errHuman(api.errText(e)), true);
      return NS.nav('placeholder');
    }
    document.documentElement.lang = NS.lang();
    NS.shell.renderRail();
    const saved = NS.storage.get('ns.lastenv');
    const target = (s.envs.find((e) => e.id === saved && e.state === 'ok') || s.envs.find((e) => e.state === 'ok') || s.envs[0] || {}).id;
    if (!s.projects.length || !target) return NS.nav('onboarding');
    NS.nav('env', { env: target });
  }
  NS.recheckGit = start;

  let lastCheck = 0;
  async function recheckEnvs() {
    if (Date.now() - lastCheck < 5000) return;     // G17.3: on return to the window and every 5 minutes
    lastCheck = Date.now();
    const cur = NS.shell.current;
    if (cur && cur.def.id === 'nogit') return start();
    try { await store.reloadEnvs(); } catch (e) { /* the old list stays on screen */ }
  }
  window.addEventListener('focus', recheckEnvs);
  setInterval(recheckEnvs, 5 * 60000);


  NS.shell.wire();
  wireCore();
  start().then(() => window.dispatchEvent(new Event('ns-ready')));
})();
