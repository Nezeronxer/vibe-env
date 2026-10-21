// Window frame: env rail (left edge), chat list of the selected env, screen area, routing, shortcuts, event -> patch wiring.
(function () {
  const NS = window.NS;
  const { h, ic, envIcon, clear } = NS;
  const L = window.NSLogic;
  const { s, on } = NS.store;
  const t = (k, p) => NS.t(k, p);
  const $ = (id) => document.getElementById(id);

  let current = null;                       // { def, inst, params }
  const lastChat = new Map();               // env -> chat id last opened there
  const rowCache = new Map();               // chat id -> { li, sig }
  const railBtns = new Map();               // env id -> button
  let toastTimer = 0;

  function toast(msg, bad) {
    const el = $('toast');
    el.textContent = msg; el.className = 'toast' + (bad ? ' bad' : ''); el.hidden = false;
    clearTimeout(toastTimer); toastTimer = setTimeout(() => { el.hidden = true; }, bad ? 6000 : 3000);
  }
  const ctxFor = (params) => ({ params, store: NS.store, api: NS.api, t, nav, toast, logic: L, setLang });
  async function setLang(setting) {
    await NS.store.saveSettings({ language: setting });
    window.NSI18n.setLang(setting);
    document.documentElement.lang = window.NSI18n.lang();
    renderAll();
  }

  // ---------- routing ----------
  // nav('env', {env, chat?}) opens the right screen for an env: chat, or the "not installed / login / custom" state
  function nav(screen, params) {
    params = params || {};
    if (screen === 'env') {
      const e = NS.store.env(params.env);
      if (!e) return nav('onboarding');
      s.lastEnv = e.id; NS.storage.set('ns.lastenv', e.id);
      if (e.state !== 'ok') return mount('envstate', { env: e.id });
      let chat = params.chat;
      if (chat === undefined) chat = lastChat.get(e.id) || firstChat(e.id) || 'new';
      lastChat.set(e.id, chat);
      const swarm = NS.registry.get('swarm');
      const swt = chat && chat !== 'new' && swarm && !swarm.placeholder ? L.swarmTaskOf(NS.store.chatTasks(chat)) : null;   // the swarm chat is the main surface of its task (story 156)
      if (swt) return mount('swarm', { env: e.id, chat, task: swt.id });
      return mount('chat', { env: e.id, chat });
    }
    return mount(screen, params);
  }
  function firstChat(envId) {
    const g = L.groupChats(NS.store.chatsOf(envId), NS.store.envTasks(envId), s.projects, Date.now());
    const it = g.groups.length ? g.groups[0].items[0] : null;
    return it ? it.chat.id : null;
  }
  function mount(id, params) {
    const def = NS.registry.get(id) || NS.registry.get('placeholder');
    if (current && current.inst && current.inst.destroy) current.inst.destroy();
    const root = clear($('screen'));
    root.scrollTop = 0;
    s.route = Object.assign({ screen: id }, params);
    const layout = def.layout || 'full';
    document.body.classList.toggle('no-side', layout !== 'chats');
    document.body.classList.toggle('is-empty', layout === 'bare');
    $('app').classList.remove('side-open');
    root.classList.toggle('screen-scroll', !!def.scroll);
    current = { def, params, inst: null };
    renderRail(); if (layout === 'chats') renderSide();
    current.inst = def.mount(root, ctxFor(params)) || null;
    if (NS.notices) NS.notices.onRoute(s.route);
  }
  const forward = (evt, payload) => { if (current && current.inst && current.inst.update) current.inst.update(evt, payload); };

  // ---------- rail ----------
  function totals() { let n = 0; for (const e of s.envs) n += L.envCounts(NS.store.envTasks(e.id), e.id, s.snoozed).calls; return n; }
  function railBtn(e) {
    const missing = e.state === 'missing' || e.state === 'unavailable';
    const short = e.custom ? e.name : e.name.split(' ')[0];
    const b = h('button', { type: 'button', class: 'env-b', dataset: { env: e.id }, title: e.name + (missing ? ' - ' + t('env.missing_short') : ''),
      onclick: () => {
        const keep = current && current.def.keepEnv;
        if (keep) nav(typeof keep === 'string' ? keep : current.def.id, Object.assign({}, current.params, { env: e.id }));
        else nav('env', { env: e.id });
      } }, envIcon(e.custom ? 'custom' : e.id), h('span', { text: short }));
    if (missing) b.append(h('small', { class: 'sub', text: t('env.missing_short') }));
    return b;
  }
  function patchRailBadges(envId) {
    const b = railBtns.get(envId); if (!b) return;
    const c = L.envCounts(NS.store.envTasks(envId), envId, s.snoozed);
    let bn = b.querySelector('.bn'), wk = b.querySelector('.wk');
    const txt = L.badgeText(c.calls);
    if (txt) { if (!bn) { bn = h('b', { class: 'bn' }); b.append(bn); } bn.textContent = txt; bn.title = t('rail.calls', { n: c.calls }); }
    else if (bn) bn.remove();
    if (c.running) { if (!wk) { wk = h('i', { class: 'wk', title: t('rail.working') }); b.append(wk); } } else if (wk) wk.remove();
    const n = totals();
    document.title = n ? `(${n}) Nightshift` : 'Nightshift';
  }
  function renderRail() {
    const rail = clear($('rail'));
    rail.setAttribute('aria-label', t('rail.label'));
    railBtns.clear();
    const envs = h('div', { class: 'rail-envs' });
    for (const e of s.envs) { const b = railBtn(e); railBtns.set(e.id, b); envs.append(b); }
    const foot = h('div', { class: 'rail-foot' });
    for (const d of NS.registry.list().filter((x) => x.rail && x.rail.zone === 'foot').sort((a, b) => a.rail.order - b.rail.order)) {
      foot.append(h('button', { type: 'button', class: 'env-b' + (current && (current.def.id === d.id || current.def.railId === d.id) ? ' on' : ''), title: t(d.rail.labelKey),
        onclick: () => nav(d.id, d.keepEnv ? { env: s.lastEnv } : {}) }, ic(d.rail.icon), h('span', { text: t(d.rail.labelKey) })));
    }
    NS.append(rail, [h('div', { class: 'rail-brand', title: 'Nightshift' }, ic('i-moon')), s.frozen ? h('div', { class: 'rail-frozen', role: 'status', text: t('frozen.edge') }) : null, envs, foot]);
    for (const e of s.envs) patchRailBadges(e.id);
    markRail();
  }
  function markRail() {
    for (const [id, b] of railBtns) {
      const on = !!current && (['chat', 'envstate', 'swarm'].includes(current.def.id) || !!current.def.keepEnv) && s.route.env === id;
      b.classList.toggle('on', on); if (on) b.setAttribute('aria-current', 'page'); else b.removeAttribute('aria-current');
    }
  }

  // ---------- chat list ----------
  const iconName = (item) => (item.task ? (item.task.swarm ? 'swarm' : L.iconOf(item.task)) : 'chat');   // a swarm is a chat of its own with the swarm mark (story 152)
  function chatLine(item) {
    const c = item.chat, tk = item.task;
    if (!tk) return c.last || c.last_text || '';
    const need = tk.question && tk.question.text;
    if (item.kind === 'wait') return need || t(L.labelKey(tk));
    if (item.kind === 'fail') return NS.errHuman(tk.outcome && tk.outcome.error) || t(L.labelKey(tk));
    if (item.kind === 'run') return s.lastLog.get(tk.id) || t(L.labelKey(tk));
    if (item.kind === 'queue') return t('line.queue');
    if (tk.status === 'done') return tk.outcome && tk.outcome.commit ? t('line.merged') : (tk.outcome && tk.outcome.summary) || t(L.labelKey(tk));
    return t(L.labelKey(tk));
  }
  function rowFor(item, active) {
    const c = item.chat, line = chatLine(item), icon = iconName(item);
    const sig = [c.title, line, icon, item.kind, active].join('\u0001');
    const hit = rowCache.get(c.id);
    if (hit && hit.sig === sig) return hit.li;
    const a = h('a', { href: '#c-' + c.id, class: active ? 'on' : '', 'aria-current': active ? 'page' : null,
        onclick: (ev) => { ev.preventDefault(); nav('env', { env: c.env, chat: c.id }); } },
      ic('i-' + icon, 'ti ' + item.kind),
      h('div', { class: 'tx' }, h('span', { text: c.title || t('chat.untitled') }), h('small', { text: line }),
        item.task ? h('span', { class: 'sr-only', text: t(L.labelKey(item.task)) }) : null));
    const li = h('li', {}, a);
    rowCache.set(c.id, { li, sig });
    return li;
  }
  function renderSide() {
    const side = $('side');
    if (!current || current.def.layout !== 'chats') return;
    const e = NS.store.env(s.route.env); if (!e) return;
    const cat = s.catalogs.get(e.id), choice = NS.store.envSettings(e.id).default_model;
    const model = e.state === 'ok' ? L.modelLabel(choice, cat) : null;
    clear(side).setAttribute('aria-label', t('side.label'));
    side.append(h('div', { class: 'side-h' }, h('span', { class: 'icw' }, envIcon(e.custom ? 'custom' : e.id)), h('b', { text: e.name }),
      e.state === 'missing' ? h('small', { text: t('env.missing_short') }) : model ? h('small', { text: model }) : null));
    if (e.state === 'ok') side.append(h('button', { type: 'button', class: 'btn w100 newtask', onclick: () => nav('env', { env: e.id, chat: 'new' }) }, ic('i-plus'), t('chat.new')));
    const chats = NS.store.chatsOf(e.id);
    if (!chats.length) {
      side.append(h('p', { class: 'side-none', text: e.state === 'missing' ? t('side.install_first') : e.state === 'unset' ? t('side.configure_first') : e.state !== 'ok' ? '' : t('side.empty') }));
      return;
    }
    const g = L.groupChats(chats, NS.store.envTasks(e.id), s.projects, Date.now());
    const seen = new Set();
    for (const grp of g.groups) {
      side.append(h('div', { class: 'pgroup' }, h('h3', { text: grp.name }),
        h('ul', { class: 'tasks' }, grp.items.map((it) => { seen.add(it.chat.id); return rowFor(it, s.route.chat === it.chat.id); }))));
    }
    if (g.archive.length) {
      side.append(h('details', { class: 'archive' }, h('summary', { text: t('side.archive', { n: g.archive.length }) }),
        h('ul', { class: 'tasks' }, g.archive.map((it) => { seen.add(it.chat.id); return rowFor(it, s.route.chat === it.chat.id); }))));
    }
    for (const id of [...rowCache.keys()]) if (!seen.has(id)) rowCache.delete(id);
  }
  const sideNeeded = (envId) => current && current.def.layout === 'chats' && s.route.env === envId;

  // ---------- event wiring ----------
  function wire() {
    on('task', (task) => {
      if (!task) return;
      patchRailBadges(task.agent);
      if (sideNeeded(task.agent)) renderSide();
      forward('task', task);
    });
    on('chatmsg', (msg) => {
      const c = NS.store.findChat(msg.chat_id);
      if (c) { c.updated_at = msg.at; if (!msg.tool && msg.text) c.last = msg.text.split('\n')[0]; if (sideNeeded(c.env)) renderSide(); }
      forward('chatmsg', msg);
    });
    on('chats', (envId) => { if (sideNeeded(envId)) renderSide(); forward('chats', envId); });
    for (const k of ['limits', 'catalog', 'settings', 'project', 'tasklog', 'focus-input']) on(k, (p) => { if (k === 'catalog' || k === 'settings') { if (current && current.def.layout === 'chats') renderSide(); } forward(k, p); });
    on('frozen', (v) => {
      const bar = $('frozenBar'); clear(bar);
      bar.hidden = !v;
      if (v) bar.append(h('span', { text: t('frozen.msg') }), h('button', { type: 'button', class: 'btn sm', text: t('frozen.undo'),
        onclick: () => NS.api.call('freeze', { on: false }).catch((e) => toast(NS.errHuman(NS.api.errText(e)), true)) }));
      s.frozen = !!v;
      renderRail();                  // "Заморожено" on the edge of the window (story 169)
      forward('frozen', !!v);
    });
    on('envs', () => { renderRail(); if (current && current.def.layout === 'chats') renderSide(); forward('envs'); });
  }

  document.addEventListener('keydown', (e) => {
    if ((e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey && /^[1-7]$/.test(e.key)) {
      const env = s.envs[+e.key - 1];
      if (env) { e.preventDefault(); nav('env', { env: env.id }); }
    }
  });

  function renderAll() { renderRail(); if (current && current.def.layout === 'chats') renderSide(); if (current) mount(current.def.id, current.params); }

  NS.shell = { nav, toast, wire, renderAll, renderRail, renderSide, toggleSide: () => $('app').classList.toggle('side-open'), ctxFor,
    get current() { return current; } };
  NS.nav = nav;
})();
