// State of the window + a tiny event bus. Screens read NS.store.s and subscribe with NS.store.on(name, fn); core events
// (task-changed, chat-msg, ...) are turned into bus events in app.js, each screen patches only what changed.
(function () {
  const NS = (window.NS = window.NS || {});
  const { call } = NS.api;
  const L = window.NSLogic;

  // localStorage can throw (private window, blocked data): fall back to memory, never break the page
  const mem = new Map();
  NS.storage = {
    get(k) { try { const v = localStorage.getItem(k); return v == null ? (mem.has(k) ? mem.get(k) : null) : v; } catch (e) { return mem.has(k) ? mem.get(k) : null; } },
    set(k, v) { mem.set(k, v); try { localStorage.setItem(k, v); } catch (e) { /* memory copy stays */ } },
    del(k) { mem.delete(k); try { localStorage.removeItem(k); } catch (e) { /* nothing to clean */ } },
  };

  const s = {
    envs: [], projects: [], tasks: new Map(), chats: new Map(), settings: null, catalogs: new Map(), limits: new Map(), repos: new Map(),
    lastLog: new Map(), frozen: false, snoozed: new Set(), route: { screen: 'chat' }, lastEnv: null, lastProject: null,
  };
  try { for (const id of JSON.parse(NS.storage.get('ns.snoozed') || '[]')) s.snoozed.add(id); } catch (e) { /* no snoozed list yet */ }

  const subs = {};
  const on = (name, fn) => { (subs[name] = subs[name] || []).push(fn); return () => { subs[name] = subs[name].filter((f) => f !== fn); }; };
  const emit = (name, payload) => { for (const fn of (subs[name] || []).slice()) fn(payload); };

  const env = (id) => s.envs.find((e) => e.id === id);
  const project = (id) => s.projects.find((p) => p.id === id);
  const envTasks = (envId) => [...s.tasks.values()].filter((t) => t.agent === envId);
  const chatTasks = (chatId) => [...s.tasks.values()].filter((t) => t.chat_id === chatId);
  const chatsOf = (envId) => s.chats.get(envId) || [];
  const findChat = (id) => { for (const list of s.chats.values()) { const c = list.find((x) => x.id === id); if (c) return c; } return null; };

  async function loadChats(envId) {
    try { s.chats.set(envId, (await call('list_chats', { env: envId })) || []); } catch (e) { s.chats.set(envId, []); }
    return s.chats.get(envId);
  }
  async function loadAll() {
    const [envs, projects, tasks, settings] = await Promise.all([call('list_envs'), call('list_projects'), call('list_tasks'), call('get_settings')]);
    s.envs = envs.map(L.normalizeEnv);
    s.projects = projects; s.settings = settings;
    s.tasks = new Map(tasks.map((t) => [t.id, t]));
    window.NSI18n.setLang(settings.language || 'auto');
    await Promise.all(s.envs.filter((e) => e.state !== 'missing').map((e) => loadChats(e.id)));
  }
  async function reloadEnvs() {
    s.envs = (await call('list_envs')).map(L.normalizeEnv);
    await Promise.all(s.envs.filter((e) => e.state !== 'missing' && !s.chats.has(e.id)).map((e) => loadChats(e.id)));
    emit('envs');
  }
  async function loadCatalog(envId, refresh) {
    try { s.catalogs.set(envId, await call(refresh ? 'refresh_catalog' : 'get_catalog', { env: envId })); } catch (e) { if (refresh) throw e; }
    return s.catalogs.get(envId) || null;
  }
  async function loadLimits(envId) {
    try { s.limits.set(envId, await call('get_limits', { env: envId })); } catch (e) { s.limits.set(envId, null); }
    return s.limits.get(envId);
  }
  async function loadRepo(projectId) {
    try { s.repos.set(projectId, await call('repo_info', { id: projectId })); } catch (e) { s.repos.set(projectId, null); }
    return s.repos.get(projectId);
  }

  // per-env settings (remembered quality, swarm, start, default model): saved as soon as they change, because the core reads them on send
  const envSettings = (envId) => ((s.settings && s.settings.envs) || {})[envId] || {};
  async function saveEnvSettings(envId, patch) {
    s.settings.envs = s.settings.envs || {};
    s.settings.envs[envId] = Object.assign({}, s.settings.envs[envId], patch);
    await call('set_settings', { settings: s.settings });
    emit('settings', envId);
  }
  async function saveSettings(patch) {
    Object.assign(s.settings, patch);
    await call('set_settings', { settings: s.settings });
    emit('settings', null);
  }
  function snooze(taskId) {
    s.snoozed.add(taskId);
    NS.storage.set('ns.snoozed', JSON.stringify([...s.snoozed]));
    emit('task', s.tasks.get(taskId));
  }

  NS.store = { s, on, emit, env, project, envTasks, chatTasks, chatsOf, findChat, loadAll, reloadEnvs, loadChats, loadCatalog, loadLimits, loadRepo, envSettings, saveEnvSettings, saveSettings, snooze };
  NS.t = (k, p) => window.NSI18n.t(k, p);
  NS.lang = () => window.NSI18n.lang();
})();
(function () {
  const NS = window.NS;
  // core error keys ("hidden_file_committed", "internal_error: boom") -> err.<key> in the dictionary; unknown text is shown as is
  NS.errHuman = (raw) => {
    const s = String(raw == null ? '' : raw).trim();
    if (!s) return '';
    const I = window.NSI18n;
    if (I.has('err.' + s)) return NS.t('err.' + s);
    const m = /^([a-z_]+):\s*(.*)$/.exec(s);
    return m && I.has('err.' + m[1]) ? NS.t('err.' + m[1], { detail: m[2] }) : s;
  };
  const p2 = (n) => String(n).padStart(2, '0');
  NS.hhmm = (v) => { const d = new Date(v); return isNaN(d) ? '' : `${p2(d.getHours())}:${p2(d.getMinutes())}`; };
  NS.isNotGit = (e) => /not a git/i.test(NS.api.errText(e));
})();
