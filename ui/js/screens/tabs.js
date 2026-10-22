// Tabs and split panels (stories 232-236): a strip over the chat, "+", a menu per tab, drag to reorder, state dots;
// up to 4 panels (2 side by side, 3-4 in a 2x2 grid), each with its own feed, live line and input field.
// The state is pure (NSLive.tabs*, selftested); it is kept in localStorage and copied to settings.ui.tabs.
// Keys are handled here, in the window (no global shortcuts): see NSLive.tabKeyAction.
(function () {
  const NS = window.NS;
  const V = window.NSLive;
  const L = window.NSLogic;
  const { h, ic, envIcon, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const KEY = 'ns.tabs', NARROW = 900, STRIP_MAX = 12, TITLE_MAX = 24, KEEP_MOUNTED = 8;
  const S = () => NS.store.s;

  let state = V.tabsEmpty(), restored = false, ws = null, saveTimer = 0;
  const newId = () => 'tab-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 7);
  const platform = () => (/Mac/i.test(navigator.platform || '') || /[?&]platform=mac/.test(location.search) ? 'mac' : 'linux');

  function restore() {
    if (restored) return;
    restored = true;
    const known = (env, chat) => !!NS.store.env(env) && !!NS.store.findChat(chat);
    const st = V.tabsRestore(NS.storage.get(KEY), known) || V.tabsRestore(((S().settings || {}).ui || {}).tabs || null, known);
    if (st) state = st;
  }
  function persist() {
    const raw = V.tabsSerialize(state);
    NS.storage.set(KEY, raw);
    clearTimeout(saveTimer);                // a copy in the settings, so another start of the window finds it too (spec: ui.tabs)
    saveTimer = setTimeout(() => {
      const st = S().settings;
      if (!st) return;
      st.ui = Object.assign({}, st.ui, { tabs: JSON.parse(raw) });
      NS.api.call('set_settings', { settings: st }).catch(() => { /* the localStorage copy stays */ });
    }, 1500);
  }
  function setState(next) {
    if (next === state) return;
    state = next;
    persist();
    if (ws) ws.draw();
    syncRoute();
  }
  const activeTab = () => state.tabs.find((x) => x.id === state.active) || null;
  function syncRoute() {
    const tb = activeTab();
    if (tb && ws) NS.shell.setRoute(tb.env, tb.chat);
  }
  const keysOf = () => Object.assign(V.tabKeyDefaults(platform()), ((S().settings || {}).keys) || {});

  // ---------- API for shell.js and chat.js ----------
  // nav('env', {env, chat}) lands here: an open tab of that chat is activated, otherwise the active tab shows it
  function show(env, chat) {
    restore();
    const mountWs = () => { if (!ws) NS.shell.mountScreen('tabs', {}); };
    if (chat === undefined && state.tabs.length && !ws) { mountWs(); syncRoute(); return; }           // the start of the window: restored tabs
    if (chat === undefined || chat === null) {
      const same = state.tabs.find((x) => x.env === env && x.id === state.active) || state.tabs.slice().reverse().find((x) => x.env === env);
      if (same) { mountWs(); setState(V.tabsActivate(state, same.id)); if (ws) ws.draw(); syncRoute(); return; }
      chat = NS.shell.defaultChat(env);
    }
    const hit = state.tabs.find((x) => x.env === env && x.chat === chat);
    if (hit) state = V.tabsActivate(state, hit.id);
    else if (!state.tabs.length) state = V.tabsOpen(state, { id: newId(), env, chat });
    else state = V.tabsSetChat(state, state.active, env, chat);
    persist();
    mountWs();
    if (ws) ws.draw();
    syncRoute();
  }
  function setChat(tabId, env, chat) { setState(V.tabsSetChat(state, tabId, env, chat || 'new')); }
  function dropChat(chatId) {
    let st = state;
    for (const tb of state.tabs.filter((x) => x.chat === chatId)) st = V.tabsSetChat(st, tb.id, tb.env, 'new');
    setState(st);
  }
  function newTab() {
    const cur = activeTab(), env = (cur && cur.env) || S().lastEnv || ((S().envs.find((e) => e.state === 'ok') || {}).id);
    if (!env) return;
    setState(V.tabsOpen(state, { id: newId(), env, chat: 'new' }));
  }
  function closeTab(id) {
    let st = V.tabsClose(state, id);
    if (!st.tabs.length) { const tb = state.tabs.find((x) => x.id === id); st = V.tabsOpen(st, { id: newId(), env: tb.env, chat: 'new' }); }
    setState(st);
  }
  function split(id) {
    if (window.innerWidth < NARROW) { NS.shell.toast(t('tabs.narrow')); return; }
    if (state.panels.length >= V.MAX_PANELS) { NS.shell.toast(t('tabs.max')); return; }
    const tb = state.tabs.find((x) => x.id === id);
    setState(V.tabsSplit(state, id, tb ? { id: newId(), env: tb.env, chat: 'new' } : null));
  }

  // ---------- keys (window handler, active only while the tabs screen is shown) ----------
  document.addEventListener('keydown', (e) => {
    if (!ws || e.defaultPrevented) return;
    if (document.querySelector('dialog[open]')) return;
    if (e.target && e.target.closest && e.target.closest('.keyfield')) return;      // the hotkey field in the settings records keys
    const a = V.tabKeyAction(e, platform(), keysOf());
    if (!a) return;
    e.preventDefault();
    const ids = state.tabs.map((x) => x.id), i = ids.indexOf(state.active);
    if (a.type === 'new') newTab();
    else if (a.type === 'close') { if (state.active) closeTab(state.active); }
    else if (a.type === 'split') { if (state.active) split(state.active); }
    else if (a.type === 'unsplit') setState(V.tabsUnsplit(state, state.active));
    else if (a.type === 'goto') { if (ids[a.n - 1]) setState(V.tabsActivate(state, ids[a.n - 1])); }
    else if (a.type === 'last') { if (ids.length) setState(V.tabsActivate(state, ids[ids.length - 1])); }
    else if (a.type === 'next' || a.type === 'prev') { if (ids.length > 1) setState(V.tabsActivate(state, ids[(i + (a.type === 'next' ? 1 : -1) + ids.length) % ids.length])); }
    else if (a.type === 'panel') setState(V.tabsFocusPanel(state, a.dir));
  });

  // ---------- the screen ----------
  function tabTitle(tb) {
    if (tb.chat === 'new') return t('tabs.new_chat');
    const c = NS.store.findChat(tb.chat);
    const task = NS.store.chatTasks(tb.chat)[0];
    return (c && c.title) || (task && task.title) || t('chat.untitled');
  }
  const short = (s) => (s.length > TITLE_MAX ? s.slice(0, TITLE_MAX - 1) + '…' : s);
  const stateOf = (tb) => (tb.chat === 'new' ? 'talk' : L.chatState(NS.store.chatTasks(tb.chat)).kind);

  function mount(root) {
    restore();
    const strip = h('div', { class: 'tabstrip', role: 'tablist', 'aria-label': t('tabs.label') });
    const hint = h('div', { class: 'ws-hint', role: 'status', hidden: true, text: t('tabs.narrow') });
    const panes = h('div', { class: 'panes' });
    root.append(h('section', { class: 'ws' }, strip, hint, panes));
    const mounted = new Map();       // tab id -> {el, inst, sig, used}
    let menu = null, dragId = null;
    const closeMenu = () => { if (menu) { menu.remove(); menu = null; document.removeEventListener('click', onDocMenu); } };
    const onDocMenu = (e) => { if (menu && !menu.contains(e.target)) closeMenu(); };

    function openMenu(tb, anchor) {
      closeMenu();
      const shown = state.panels.includes(tb.id) && state.panels.length > 1;
      const item = (label, fn, disabled) => h('button', { type: 'button', role: 'menuitem', text: label, disabled: !!disabled, onclick: () => { closeMenu(); fn(); } });
      menu = h('div', { class: 'menu tabmenu', role: 'menu', 'aria-label': t('tabs.menu') },
        item(t('tabs.split'), () => split(tb.id), state.panels.length >= V.MAX_PANELS || window.innerWidth < NARROW),
        shown ? item(t('tabs.unsplit'), () => setState(V.tabsUnsplit(state, tb.id))) : null,
        item(t('tabs.close'), () => closeTab(tb.id)),
        state.tabs.length > 1 ? item(t('tabs.close_others'), () => { let st = state; for (const x of state.tabs) if (x.id !== tb.id) st = V.tabsClose(st, x.id); setState(V.tabsActivate(st, tb.id)); }) : null,
        window.innerWidth < NARROW ? h('small', { text: t('tabs.narrow') }) : null);
      const r = anchor.getBoundingClientRect(), rr = root.getBoundingClientRect();
      NS.setVars(menu, { '--x': Math.max(0, Math.min(r.left - rr.left, rr.width - 220)) + 'px', '--y': (r.bottom - rr.top + 2) + 'px' });
      root.append(menu);
      setTimeout(() => document.addEventListener('click', onDocMenu), 0);
    }

    function tabEl(tb, i) {
      const on = tb.id === state.active, kind = stateOf(tb), title = tabTitle(tb), env = NS.store.env(tb.env);
      const shown = state.panels.includes(tb.id) && state.panels.length > 1;
      const word = { run: 'tabs.st_run', wait: 'tabs.st_wait', done: 'tabs.st_done', fail: 'tabs.st_fail', queue: 'tabs.st_queue', talk: 'tabs.st_talk' }[kind];
      const menuBtn = h('button', { type: 'button', class: 'tab-x tab-m', 'aria-label': t('tabs.menu'), 'aria-haspopup': 'menu', text: '⋯', onclick: (e) => { e.stopPropagation(); openMenu(tb, el); } });
      const el = h('div', { class: 'tab' + (on ? ' on' : '') + (shown ? ' shown' : ''), role: 'tab', 'aria-selected': String(on), tabIndex: on ? 0 : -1, draggable: 'true',
        title: (env ? env.name + ' · ' : '') + title + ' · ' + t(word) + (i < 9 ? ' · ' + (platform() === 'mac' ? '⌘' : 'Ctrl+') + (i + 1) : ''), dataset: { tab: tb.id },
        onclick: () => setState(V.tabsActivate(state, tb.id)),
        onauxclick: (e) => { if (e.button === 1) { e.preventDefault(); closeTab(tb.id); } },
        onmousedown: (e) => { if (e.button === 1) e.preventDefault(); },
        oncontextmenu: (e) => { e.preventDefault(); openMenu(tb, el); },
        onkeydown: (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setState(V.tabsActivate(state, tb.id)); } },
        ondragstart: (e) => { dragId = tb.id; e.dataTransfer.effectAllowed = 'move'; try { e.dataTransfer.setData('text/plain', tb.id); } catch (er) { /* some engines refuse */ } el.classList.add('drag'); },
        ondragend: () => { dragId = null; el.classList.remove('drag'); },
        ondragover: (e) => { if (dragId && dragId !== tb.id) { e.preventDefault(); el.classList.add('over'); } },
        ondragleave: () => el.classList.remove('over'),
        ondrop: (e) => { e.preventDefault(); el.classList.remove('over'); if (dragId && dragId !== tb.id) setState(V.tabsMove(state, dragId, state.tabs.findIndex((x) => x.id === tb.id))); } },
      h('span', { class: 'tab-ic' }, envIcon(env && env.custom ? 'custom' : tb.env)),
      h('span', { class: 'tab-t', text: short(title) }),
      h('i', { class: 'tdot ' + kind, 'aria-hidden': 'true' }), h('span', { class: 'sr-only', text: t(word) }),
      menuBtn,
      h('button', { type: 'button', class: 'tab-x', 'aria-label': t('tabs.close'), text: '×', onclick: (e) => { e.stopPropagation(); closeTab(tb.id); } }));
      return el;
    }
    function drawStrip() {
      const list = state.tabs.map(tabEl);
      const rest = state.tabs.slice(STRIP_MAX);
      const more = rest.length ? h('button', { type: 'button', class: 'btn sm tab-more', text: t('tabs.more', { n: rest.length }), onclick: (e) => {
        e.stopPropagation(); closeMenu();
        menu = h('div', { class: 'menu tabmenu', role: 'menu' }, rest.map((tb) => h('button', { type: 'button', role: 'menuitem', text: short(tabTitle(tb)), onclick: () => { closeMenu(); setState(V.tabsActivate(state, tb.id)); } })));
        const r = e.target.getBoundingClientRect(), rr = root.getBoundingClientRect();
        NS.setVars(menu, { '--x': Math.max(0, r.right - rr.left - 220) + 'px', '--y': (r.bottom - rr.top + 2) + 'px' });
        root.append(menu); setTimeout(() => document.addEventListener('click', onDocMenu), 0);
      } }) : null;
      NS.append(clear(strip), [h('div', { class: 'tabs-scroll' }, list), h('button', { type: 'button', class: 'tab-plus', 'aria-label': t('tabs.new'), title: t('tabs.new') + ' (' + (platform() === 'mac' ? '⌘T' : 'Ctrl+T') + ')', onclick: newTab }, ic('i-plus')), more]);
      const a = strip.querySelector('.tab.on');
      if (a && a.scrollIntoView) a.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    }
    function paneFor(tb) {
      const sig = tb.env + '|' + tb.chat;
      let m = mounted.get(tb.id);
      if (m && m.sig === sig) { m.used = Date.now(); return m; }
      if (m) { if (m.inst && m.inst.destroy) m.inst.destroy(); m.el.remove(); }
      const el = h('div', { class: 'pane', dataset: { tab: tb.id } });
      el.addEventListener('pointerdown', () => { if (state.active !== tb.id && state.panels.includes(tb.id)) setState(V.tabsActivate(state, tb.id)); }, true);
      const env = NS.store.env(tb.env);
      let id = 'chat';
      const params = { env: tb.env, chat: tb.chat, tab: tb.id };
      if (!env || env.state !== 'ok') id = 'envstate';
      else {
        const sw = NS.registry.get('swarm');
        const swt = tb.chat !== 'new' && sw && !sw.placeholder ? L.swarmTaskOf(NS.store.chatTasks(tb.chat)) : null;
        if (swt) { id = 'swarm'; params.task = swt.id; }
      }
      const def = NS.registry.get(id);
      const inst = def.mount(el, NS.shell.ctxFor(params)) || null;
      m = { el, inst, sig, used: Date.now() };
      mounted.set(tb.id, m);
      return m;
    }
    function drawPanes() {
      const narrow = window.innerWidth < NARROW;
      const visible = narrow ? [state.active] : state.panels;
      hint.hidden = !(narrow && state.panels.length > 1);
      const els = [];
      for (const id of visible) { const tb = state.tabs.find((x) => x.id === id); if (tb) els.push(paneFor(tb).el); }
      for (const [id, m] of mounted) {
        const vis = visible.includes(id);
        m.el.hidden = !vis;
        m.el.classList.toggle('on', id === state.active && visible.length > 1);
      }
      // drop panes of closed tabs; keep a few hidden ones mounted (their feed and draft stay), the oldest go
      for (const [id, m] of [...mounted]) if (!state.tabs.some((x) => x.id === id)) { if (m.inst && m.inst.destroy) m.inst.destroy(); m.el.remove(); mounted.delete(id); }
      const hidden = [...mounted].filter(([id]) => !visible.includes(id)).sort((a, b) => a[1].used - b[1].used);
      while (hidden.length > KEEP_MOUNTED) { const [id, m] = hidden.shift(); if (m.inst && m.inst.destroy) m.inst.destroy(); m.el.remove(); mounted.delete(id); }
      panes.className = 'panes n' + els.length;
      const want = visible.map((id) => mounted.get(id)).filter(Boolean).map((m) => m.el);
      const keep = [...mounted.values()].map((m) => m.el).filter((el) => !want.includes(el));
      const order = want.concat(keep);
      if (order.length !== panes.childNodes.length || order.some((el, i) => panes.childNodes[i] !== el)) panes.replaceChildren(...order);
    }
    function draw() { closeMenu(); drawStrip(); drawPanes(); }
    const onResize = () => drawPanes();
    window.addEventListener('resize', onResize);
    ws = { draw };
    draw();
    const forward = (evt, payload) => {
      for (const [id, m] of mounted) {
        if (!m.inst || !m.inst.update) continue;
        if (evt === 'focus-input' && id !== state.active) continue;
        m.inst.update(evt, payload);
      }
    };
    return {
      update(evt, payload) {
        if (['task', 'chats', 'chatmsg', 'envs', 'taskremoved'].includes(evt)) drawStrip();
        if (evt === 'task' && payload && payload.chat_id) {      // a chat that became a swarm switches its pane to the swarm screen
          for (const tb of state.tabs.filter((x) => x.chat === payload.chat_id)) { const m = mounted.get(tb.id); if (m && payload.swarm && m.sig.indexOf('|swarm') < 0 && NS.registry.get('swarm') && !NS.registry.get('swarm').placeholder) { m.sig = ''; } }
          drawPanes();
        }
        forward(evt, payload);
      },
      destroy() {
        ws = null; closeMenu(); window.removeEventListener('resize', onResize);
        for (const m of mounted.values()) if (m.inst && m.inst.destroy) m.inst.destroy();
        mounted.clear();
      },
    };
  }

  NS.registry.register({ id: 'tabs', layout: 'chats', mount });
  NS.tabs = { show, setChat, dropChat, newTab, closeTab, split, get state() { return state; }, get active() { return activeTab(); } };
})();
