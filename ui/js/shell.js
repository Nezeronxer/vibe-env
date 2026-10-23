// Window frame v4 (stories 246, 249, 296): the workspace (tabs + panes, tabs.js) over the bar at the bottom; the chat list is a
// hidden side panel (⌘B); the env rail is gone - the env lives in the bar and the palette (⌥⌘1…7 still work). Routing,
// shortcuts, event -> patch wiring. Full screens (settings, factory, kits) take the workspace area; Esc goes back.
(function () {
  const NS = window.NS;
  const { h, ic, envIcon, clear } = NS;
  const L = window.NSLogic;
  const { s, on } = NS.store;
  const t = (k, p) => NS.t(k, p);
  const $ = (id) => document.getElementById(id);
  // the v4 frame from the v3 markup of index.html (not in this task's zone): no env rail, a bar under the workspace
  { const rail = $('rail'); if (rail) rail.remove(); if (!$('bar')) $('app').append(h('footer', { class: 'bar', id: 'bar', role: 'toolbar' })); }
  const plat = () => (/Mac/i.test(navigator.platform || '') || /[?&]platform=mac/.test(location.search) ? 'mac' : 'linux');

  let current = null;                       // { def, inst, params }
  const lastChat = new Map();               // env -> chat id last opened there
  const rowCache = new Map();               // chat id -> { li, sig }
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
  // nav('env', {env, chat?}) opens the env in the workspace: its chat in the focused pane, or the "not installed / login" state
  function nav(screen, params) {
    params = params || {};
    if (screen === 'env') {
      const e = NS.store.env(params.env);
      if (!e) return nav('onboarding');
      s.lastEnv = e.id; NS.storage.set('ns.lastenv', e.id);
      if (NS.tabs) {
        const chat0 = params.chat === null ? undefined : params.chat;
        if (chat0 !== undefined) lastChat.set(e.id, chat0);
        return NS.tabs.show(e.id, chat0);
      }
      return mount(e.state !== 'ok' ? 'envstate' : 'chat', { env: e.id, chat: params.chat || 'new' });
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
    document.body.dataset.layout = layout;
    document.body.classList.toggle('is-empty', layout === 'bare');
    root.classList.toggle('screen-scroll', !!def.scroll);
    current = { def, params, inst: null };
    if (layout === 'chats') renderSide();
    if (layout === 'full') root.append(h('button', { type: 'button', class: 'back4', title: 'Esc', onclick: back }, h('span', { 'aria-hidden': 'true', text: '←' }), t('shell4.back')));
    current.inst = def.mount(root, ctxFor(params)) || null;
    barUpdate('route');
    if (NS.notices) NS.notices.onRoute(s.route);
  }
  const back = () => nav('env', { env: s.lastEnv || ((s.envs.find((e) => e.state === 'ok') || {}).id) });
  const defaultChat = (envId) => lastChat.get(envId) || firstChat(envId) || 'new';
  // the workspace tells which env and chat are in focus: the bar and the chat list follow it
  function setRoute(envId, chat) {
    s.route = Object.assign({}, s.route, { env: envId, chat });
    s.lastEnv = envId; NS.storage.set('ns.lastenv', envId);
    if (chat) lastChat.set(envId, chat);
    renderSide(); barUpdate('route');
  }
  const forward = (evt, payload) => { if (current && current.inst && current.inst.update) current.inst.update(evt, payload); };
  // an env picked in the bar, the palette or by ⌥⌘n: a full screen that belongs to an env (settings, factory) follows it
  function pickEnv(envId) {
    const keep = current && current.def.keepEnv;
    if (keep) nav(typeof keep === 'string' ? keep : current.def.id, Object.assign({}, current.params, { env: envId }));
    else nav('env', { env: envId });
  }
  const callsTotal = () => s.envs.reduce((n, e) => n + L.envCounts(NS.store.envTasks(e.id), e.id, s.snoozed).calls, 0);
  function patchTitle() { const n = callsTotal(); document.title = n ? `(${n}) Nightshift` : 'Nightshift'; }

  // ---------- the bar (story 296): modules from the registry ('bar'), drawn once, each patches itself on events ----------
  const barInst = new Map();
  function renderBar() {
    const bar = $('bar');
    if (!bar) return;
    for (const m of barInst.values()) if (m && m.destroy) m.destroy();
    barInst.clear();
    const left = h('div', { class: 'bar-l' }), right = h('div', { class: 'bar-r' });
    for (const def of NS.registry.points('bar')) {
      const el = h('div', { class: 'mod mod-' + def.id, dataset: { mod: def.id } });
      (def.side === 'right' ? right : left).append(el);
      barInst.set(def.id, def.render(el, ctxFor({})) || null);
    }
    clear(bar).append(left, right);
    bar.setAttribute('aria-label', t('bar.label'));
    patchTitle();
  }
  function barUpdate(evt, payload) { for (const m of barInst.values()) if (m && m.update) m.update(evt, payload); }
  const envNow = () => NS.store.env(s.route.env) || NS.store.env(s.lastEnv) || s.envs[0] || null;
  const btn = (cls, label, kids, onclick) => h('button', { type: 'button', class: 'bm ' + cls, title: label, 'aria-label': label, onclick }, kids);
  let envMenu = null;
  const closeEnvMenu = () => { if (envMenu) { envMenu.remove(); envMenu = null; document.removeEventListener('click', onDocEnv); } };
  const onDocEnv = (e) => { if (envMenu && !envMenu.contains(e.target)) closeEnvMenu(); };
  function openEnvMenu(anchor) {
    closeEnvMenu();
    const mod = plat() === 'mac' ? '⌥⌘' : 'Ctrl+Alt+';
    envMenu = h('div', { class: 'menu barmenu', role: 'menu' }, s.envs.map((e, i) => {
      const c = L.envCounts(NS.store.envTasks(e.id), e.id, s.snoozed);
      const missing = e.state === 'missing' || e.state === 'unavailable';
      return h('button', { type: 'button', role: 'menuitem', class: e.id === (envNow() || {}).id ? 'on' : '', onclick: () => { closeEnvMenu(); pickEnv(e.id); } },
        envIcon(e.custom ? 'custom' : e.id), h('span', { class: 'grow', text: e.name }), missing ? h('small', { text: t('env.missing_short') }) : null,
        c.calls ? h('b', { class: 'bn', text: L.badgeText(c.calls) }) : null, i < 7 ? h('kbd', { text: mod + (i + 1) }) : null);
    }));
    const r = anchor.getBoundingClientRect();
    NS.setVars(envMenu, { '--x': r.left + 'px', '--yb': (window.innerHeight - r.top + 6) + 'px' });
    document.body.append(envMenu);
    setTimeout(() => document.addEventListener('click', onDocEnv), 0);
  }
  const R = NS.registry;
  R.extend('bar', { id: 'env', side: 'left', order: 10, render(el) {
    const draw = () => {
      const e = envNow();
      if (!e) { clear(el); return; }
      const n = callsTotal();
      clear(el).append(btn('bm-env', t('bar.env'), [envIcon(e.custom ? 'custom' : e.id), h('span', { text: e.name }), n ? h('b', { class: 'bn', title: t('bar.calls', { n }), text: L.badgeText(n) }) : null], (ev) => { ev.stopPropagation(); openEnvMenu(ev.currentTarget); }));
      patchTitle();
    };
    draw();
    return { update: (evt) => { if (['route', 'envs', 'task', 'focus', 'lang'].includes(evt)) draw(); }, destroy: closeEnvMenu };
  } });
  R.extend('bar', { id: 'model', side: 'left', order: 20, render(el) {
    const draw = () => {
      const e = envNow();
      if (e && e.state === 'ok' && !s.catalogs.has(e.id)) NS.store.loadCatalog(e.id).then(() => s.catalogs.has(e.id) && draw());
      const m = e && e.state === 'ok' ? NS.models.label(e.id) : '';
      clear(el).append(m ? btn('bm-model', t('bar.model'), [h('span', { class: 'bm-k', text: '◆' }), h('span', { text: m })], () => { const p = document.querySelector('.pane.focus .envpick'); if (p) p.click(); }) : '');
    };
    draw();
    return { update: (evt) => { if (['route', 'catalog', 'settings', 'envs', 'focus'].includes(evt)) draw(); } };
  } });
  R.extend('bar', { id: 'branch', side: 'left', order: 30, render(el) {
    const draw = () => {
      const chat = s.route.chat && s.route.chat !== 'new' ? NS.store.findChat(s.route.chat) : null;
      const pid = (chat && chat.project_id) || NS.projects.current();
      const p = NS.store.project(pid);
      if (!p) { clear(el).append(h('span', { class: 'bm-t mute', text: t('bar.no_project') })); return; }
      const repo = s.repos.get(p.id);
      if (!s.repos.has(p.id)) NS.store.loadRepo(p.id).then(draw);
      const br = repo ? (repo.current_branch || p.base_branch) + (repo.ahead ? ' ↑' + repo.ahead : '') + (repo.behind ? ' ↓' + repo.behind : '') : p.base_branch || '';
      clear(el).append(h('span', { class: 'bm-t', title: t('bar.branch') }, ic('i-branch'), h('span', { text: p.name }), br ? h('span', { class: 'mute', text: br }) : null));
    };
    draw();
    return { update: (evt) => { if (['route', 'project', 'focus', 'chats'].includes(evt)) draw(); } };
  } });
  // a limit as a VU meter: a cream face, an arc, a red zone at the end and a needle; --n (0..1) is the used share, the numbers beside it stay
  let vuSeq = 0;
  function vuMeter(pct) {
    const SV = 'http://www.w3.org/2000/svg', id = 'vug' + (vuSeq++);
    const e = (tag, a, kids) => { const x = document.createElementNS(SV, tag); for (const k in a) x.setAttribute(k, a[k]); for (const c of kids || []) x.append(c); return x; };
    const ticks = [-52, -26, 0, 26, 52].map((deg) => { const r = deg * Math.PI / 180, sx = Math.sin(r), cx = Math.cos(r);
      return e('line', { class: 'vu-tick', x1: (19 + 11 * sx).toFixed(2), y1: (19 - 11 * cx).toFixed(2), x2: (19 + 13.2 * sx).toFixed(2), y2: (19 - 13.2 * cx).toFixed(2) }); });
    const svg = e('svg', { class: 'vu', viewBox: '0 0 38 22', 'aria-hidden': 'true', focusable: 'false' }, [
      e('defs', {}, [e('linearGradient', { id, x1: '0', y1: '0', x2: '0', y2: '1' }, [e('stop', { offset: '0', class: 'vu-s0' }), e('stop', { offset: '1', class: 'vu-s1' })])]),
      e('rect', { class: 'vu-face', x: '.5', y: '.5', width: '37', height: '21', rx: '3.5', fill: 'url(#' + id + ')' }),
      e('path', { class: 'vu-zone', d: 'M26.77 6.18A15 15 0 0 1 30.82 9.76' }),
      e('path', { class: 'vu-arc', d: 'M7.18 9.76A15 15 0 0 1 30.82 9.76', pathLength: '100' }),
      ...ticks,
      e('g', { class: 'vu-needle' }, [e('line', { x1: '19', y1: '19', x2: '19', y2: '6.2' })]),
      e('circle', { class: 'vu-pivot', cx: '19', cy: '19', r: '1.8' })]);
    svg.style.setProperty('--n', String(Math.max(0, Math.min(1, pct / 100))));
    return svg;
  }
  R.extend('bar', { id: 'limits', side: 'right', order: 10, render(el) {
    const draw = () => {
      const e = envNow();
      clear(el);
      if (!e) return;
      const v = L.limitsView(s.limits.get(e.id));
      if (v.kind !== 'known' || !v.windows.length) { el.append(h('span', { class: 'bm-t mute', text: t('bar.no_limits') })); return; }
      const warnAt = ((s.settings || {}).limits || {}).warn_pct;
      for (const w of L.pickWindows(v.windows, 2).shown) {
        const lvl = L.limitLevel(w, warnAt), pct = w.exhausted ? 100 : Math.round(w.used_pct || 0);
        el.append(h('span', { class: 'bm-lim ' + lvl, title: L.formatLimit(w, Date.now(), t) }, h('span', { class: 'mute', text: L.windowName(w, t) }),
          vuMeter(pct), h('span', { text: pct + '%' })));
      }
    };
    draw();
    const e0 = envNow();
    if (e0 && !s.limits.has(e0.id)) NS.store.loadLimits(e0.id).then(draw);
    return { update: (evt) => { if (['route', 'limits', 'focus'].includes(evt)) { const e = envNow(); if (e && !s.limits.has(e.id)) NS.store.loadLimits(e.id).then(draw); else draw(); } } };
  } });
  R.extend('bar', { id: 'clock', side: 'right', order: 80, render(el) {
    const span = h('time', { class: 'bm-t', title: t('bar.clock') });
    el.append(span);
    const tick = () => { span.textContent = NS.hhmm(Date.now()); };
    tick();
    const timer = setInterval(() => { if (!document.hidden) tick(); }, 15000);   // a hidden window does no work (story 248)
    return { destroy: () => clearInterval(timer) };
  } });
  R.extend('bar', { id: 'palette', side: 'right', order: 90, render(el) {
    el.append(btn('bm-pal', t('bar.palette'), [h('kbd', { text: plat() === 'mac' ? '⌘K' : 'Ctrl+K' })], () => NS.palette.open()));
  } });
  R.extend('bar', { id: 'chats', side: 'right', order: 95, render(el) {
    const b = btn('bm-side', t('bar.chats'), [ic('i-chat')], () => toggleSide());
    el.append(b);
    return { update: (evt) => { if (evt === 'side') b.classList.toggle('on', document.body.classList.contains('side-on')); } };
  } });

  // ---------- chat list (hidden side panel, ⌘B) ----------
  const iconName = (item) => (item.task ? (item.task.swarm ? 'swarm' : L.iconOf(item.task)) : 'chat');
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
  // the tally lamp of a row: on air (red) = an agent works, calls (amber, blinks) = waits for you, done (green), a ring = idle, a dashed ring = stamped
  function lampOf(item) {
    const tk = item.task, live = tk && L.info(tk.status).live;
    if (tk && (tk.status === 'cancelled' || tk.status === 'reverted')) return 'stamp';
    if (item.kind === 'wait') return 'calling';
    if (item.kind === 'run') return live ? 'on-air' : 'idle';
    if (item.kind === 'done') return 'done';
    if (item.kind === 'fail') return 'failed';
    return 'idle';
  }
  function rowFor(item, active) {
    const c = item.chat, line = chatLine(item), icon = iconName(item);
    const sig = [c.title, line, icon, item.kind, active, item.task && item.task.status].join('\u0001');
    const hit = rowCache.get(c.id);
    if (hit && hit.sig === sig) return hit.li;
    const a = h('a', { href: '#c-' + c.id, class: active ? 'on' : '', 'aria-current': active ? 'page' : null,
        onclick: (ev) => { ev.preventDefault(); nav('env', { env: c.env, chat: c.id }); } },
      h('span', { class: 'lamp ' + lampOf(item), 'aria-hidden': 'true' }),
      h('div', { class: 'tx' }, h('span', { text: c.title || t('chat.untitled') }), h('small', { text: line }),
        item.task ? h('span', { class: 'sr-only', text: t(L.labelKey(item.task)) }) : null));
    const li = h('li', {}, a);
    rowCache.set(c.id, { li, sig });
    return li;
  }
  function renderSide() {
    const side = $('side');
    if (!side || !document.body.classList.contains('side-on')) return;
    const e = NS.store.env(s.route.env) || envNow(); if (!e) return;
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
    // F9: the list is grouped by what it needs from me - "Ждёт тебя", "Работает", "Готово" (chats without a task: "Разговоры"), the newest first
    const flat = g.groups.flatMap((x) => x.items);
    const when = (it) => Date.parse(it.chat.updated_at) || 0;
    for (const [kinds, key] of [[['wait', 'fail'], 'side.g_wait'], [['run', 'queue'], 'side.g_run'], [['done'], 'side.g_done'], [['talk'], 'side.g_talk']]) {
      const items = flat.filter((it) => kinds.includes(it.kind)).sort((x, y) => when(y) - when(x));
      if (!items.length) continue;
      side.append(h('div', { class: 'pgroup' }, h('h3', { text: t(key) }),
        h('ul', { class: 'tasks' }, items.map((it) => { seen.add(it.chat.id); return rowFor(it, s.route.chat === it.chat.id); }))));
    }
    if (g.archive.length) {
      side.append(h('details', { class: 'archive' }, h('summary', { text: t('side.archive', { n: g.archive.length }) }),
        h('ul', { class: 'tasks' }, g.archive.map((it) => { seen.add(it.chat.id); return rowFor(it, s.route.chat === it.chat.id); }))));
    }
    for (const id of [...rowCache.keys()]) if (!seen.has(id)) rowCache.delete(id);
  }
  // ⌘B: the chat list is shown by default since F9 (a user who hid it keeps it hidden: ns.side = 0)
  function toggleSide(force) {
    const v = force === undefined ? !document.body.classList.contains('side-on') : !!force;
    document.body.classList.toggle('side-on', v);
    NS.storage.set('ns.side', v ? '1' : '0');
    if (v) renderSide();
    barUpdate('side');
  }
  const sideNeeded = (envId) => document.body.classList.contains('side-on') && s.route.env === envId;

  // ---------- event wiring ----------
  function wire() {
    on('task', (task) => {
      if (!task) return;
      if (sideNeeded(task.agent)) renderSide();
      barUpdate('task', task);
      forward('task', task);
    });
    on('chatmsg', (msg) => {
      const c = NS.store.findChat(msg.chat_id);
      if (c) { c.updated_at = msg.at; if (!msg.tool && msg.text) c.last = msg.text.split('\n')[0]; if (sideNeeded(c.env)) renderSide(); }
      forward('chatmsg', msg);
    });
    on('chats', (envId) => { if (sideNeeded(envId)) renderSide(); barUpdate('chats', envId); forward('chats', envId); });
    for (const k of ['limits', 'catalog', 'settings', 'project', 'tasklog', 'focus-input', 'taskaction', 'chataction', 'chatdelta', 'taskremoved']) on(k, (p) => {
      if (k === 'catalog' || k === 'settings') renderSide();
      if (['limits', 'catalog', 'settings', 'project'].includes(k)) barUpdate(k, p);
      forward(k, p);
    });
    on('frozen', (v) => {
      const bar = $('frozenBar'); clear(bar);
      bar.hidden = !v;
      if (v) bar.append(h('span', { text: t('frozen.msg') }), h('button', { type: 'button', class: 'btn sm', text: t('frozen.undo'),
        onclick: () => NS.api.call('freeze', { on: false }).catch((e) => toast(NS.errHuman(NS.api.errText(e)), true)) }));
      s.frozen = !!v;
      forward('frozen', !!v);
    });
    on('envs', () => { renderSide(); barUpdate('envs'); forward('envs'); });
    R.onExtend((point) => { if (point === 'bar') renderBar(); });
  }

  // ---------- keys: envs ⌥⌘1…7, palette ⌘K, chats ⌘B, Esc out of a full screen ----------
  document.addEventListener('keydown', (e) => {
    if (e.defaultPrevented || (e.target && e.target.closest && e.target.closest('.keyfield'))) return;
    const mac = plat() === 'mac', mod = mac ? e.metaKey : e.ctrlKey;
    const code = e.code || '';
    if (mod && !e.altKey && !e.shiftKey && (code === 'KeyK' || /^[kл]$/i.test(e.key))) { e.preventDefault(); NS.palette.open(); return; }
    if (mod && !e.altKey && !e.shiftKey && (code === 'Comma' || e.key === ',' || e.key === 'б')) { e.preventDefault(); nav('settings', { env: s.lastEnv, section: 'basic' }); return; }
    if (mod && !e.altKey && !e.shiftKey && (code === 'KeyB' || /^[bи]$/i.test(e.key)) && !document.body.classList.contains('is-empty')) { e.preventDefault(); toggleSide(); return; }
    if (e.key === 'Escape' && current && (current.def.layout || 'full') === 'full' && !document.querySelector('dialog[open]') && !(NS.palette && NS.palette.isOpen)
      && !(e.target && e.target.closest && e.target.closest('input, textarea, select, .pop, .menu'))) { e.preventDefault(); back(); return; }
    const pattern = window.NSLive.migrateSwitchEnv(((s.settings || {}).keys || {}).switch_env, plat());
    const accel = L.accelFromEvent(e, plat()), d = /^Digit([1-7])$/.exec(code);
    if (!accel || !d || accel !== pattern.replace('{n}', d[1])) return;
    const env = s.envs[+d[1] - 1];
    if (env) { e.preventDefault(); pickEnv(env.id); }
  });

  // ---------- palette commands of the shell (story 249); other tasks add theirs through NS.registry.extend('command', …) ----------
  const ws = () => NS.tabs && current && current.def.id === 'tabs';
  const toWs = (fn) => () => { if (!ws()) back(); fn(); };
  const mk = (accel) => () => (plat() === 'mac' ? accel : accel.replace('Shift+Cmd', 'Ctrl+Shift').replace('Alt+Cmd', 'Ctrl+Alt').replace('Cmd', 'Ctrl'));
  [
    { id: 'pane.new_agent', labelKey: 'cmd.new_agent', group: 'cmd.g.layout', keys: mk('Cmd+T'), order: 10, run: toWs(() => NS.tabs.newTab()) },
    { id: 'pane.split_right', labelKey: 'cmd.split_right', group: 'cmd.g.layout', keys: mk('Cmd+D'), order: 20, run: toWs(() => NS.tabs.split('h')) },
    { id: 'pane.split_down', labelKey: 'cmd.split_down', group: 'cmd.g.layout', keys: mk('Shift+Cmd+D'), order: 21, run: toWs(() => NS.tabs.split('v')) },
    { id: 'pane.zoom', labelKey: 'cmd.zoom', group: 'cmd.g.layout', keys: mk('Shift+Cmd+Enter'), order: 22, run: toWs(() => NS.tabs.zoom()) },
    { id: 'pane.equalize', labelKey: 'cmd.equalize', group: 'cmd.g.layout', order: 23, run: toWs(() => NS.tabs.equalize()) },
    { id: 'pane.close', labelKey: 'cmd.close_pane', group: 'cmd.g.layout', keys: mk('Cmd+W'), order: 24, run: toWs(() => NS.tabs.closePane()) },
    { id: 'shell.side', labelKey: 'cmd.side', group: 'cmd.g.view', keys: mk('Cmd+B'), order: 30, run: () => toggleSide() },
    { id: 'shell.settings', labelKey: 'cmd.settings', order: 60, keys: mk('Cmd+,'), run: () => nav('settings', { env: s.lastEnv, section: 'basic' }) },
    { id: 'shell.settings_adv', labelKey: 'cmd.settings_adv', order: 61, run: () => nav('settings', { env: s.lastEnv, section: 'advanced' }) },
  ].forEach((c) => R.extend('command', c));
  // model commands ("Модель: Opus 5.5"...) are built by the palette from NS.models.commands for the env in focus
  function envCommands() {
    const mod = plat() === 'mac' ? 'Alt+Cmd+' : 'Ctrl+Alt+';
    s.envs.forEach((e, i) => R.extend('command', { id: 'env.' + e.id, label: t('cmd.env', { env: e.name }), group: 'cmd.g.env', keys: i < 7 ? mod + (i + 1) : null, order: 40 + i / 100, run: () => pickEnv(e.id) }));
    // screens that used to sit on the rail (factory, settings) are commands now
    for (const d of R.list().filter((x) => x.rail && !x.placeholder)) if (d.id !== 'settings') R.extend('command', { id: 'screen.' + d.id, labelKey: d.rail.labelKey, order: 50 + d.rail.order, run: () => nav(d.id, d.keepEnv ? { env: s.lastEnv } : {}) });
  }

  function renderAll() { renderBar(); renderSide(); if (current) mount(current.def.id, current.params); }
  function renderRail() { envCommands(); renderBar(); }       // v3 name kept: app.js and the settings call it once the envs are known

  NS.shell = { nav, toast, wire, renderAll, renderRail, renderBar, renderSide, barUpdate, toggleSide, ctxFor, setRoute, defaultChat, pickEnv, back,
    mountScreen: (id, params) => mount(id, params),
    get current() { return current; } };
  NS.nav = nav;
  { const saved = NS.storage.get('ns.side'); if (saved === '1' || (saved == null && window.innerWidth >= 900)) document.body.classList.add('side-on'); }
})();
