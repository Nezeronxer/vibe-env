// v4 workspace (stories 246, 258-259, 296): tabs on top, each tab a tree of panes like a tiling WM (NSPanes, selftested),
// rounded panes on the canvas with a gap, a thin accent border on the focused one, the others dimmed; dividers drag, double
// click makes them equal; a pane is zoomed with ⌘⇧Enter. Pane kinds come from the registry ('paneKind'): 'agent' (the v3 feed)
// is built here, 'term' arrives with task 23. The state is kept in localStorage and copied to settings.ui.tabs.
// Keys (window, capture phase): ⌘T tab · ⌘W pane · ⌘D split right · ⌘⇧D split down · ⌘⇧Enter zoom · ⌘⌥←↑→↓ focus ·
// ⌘1…9, ⌘⇧[ ], Ctrl+Tab tabs. Linux: Ctrl instead of ⌘.
(function () {
  const NS = window.NS;
  const V = window.NSLive;
  const P = new Proxy({}, { get: (_, k) => window.NSPanes[k] });    // panes.js loads after this file (index.html order)
  const L = window.NSLogic;
  const { h, ic, envIcon, clear, setVars } = NS;
  const t = (k, p) => NS.t(k, p);
  const KEY = 'ns.tabs', STRIP_MAX = 12, TITLE_MAX = 26, KEEP_MOUNTED = 12;
  const S = () => NS.store.s;

  let state = { v: 2, tabs: [], active: null }, restored = false, ws = null, saveTimer = 0;
  const uid = (p) => p + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 7);
  const platform = () => (/Mac/i.test(navigator.platform || '') || /[?&]platform=mac/.test(location.search) ? 'mac' : 'linux');
  const agentPane = (env, chat) => ({ id: uid('p-'), kind: 'agent', env, chat: chat || 'new' });

  // ---------- saving: localStorage + settings.ui.tabs (the look extras ride along in .look, see settings-v4.js) ----------
  function savedRaw() { return [NS.storage.get(KEY), ((S().settings || {}).ui || {}).tabs || null]; }
  // only once the chat lists of the saved panes' envs are in: before that every saved chat looks deleted
  function restore() {
    if (restored) return;
    const saved = savedRaw();
    const envsOf = (raw) => { try { const o = typeof raw === 'string' ? JSON.parse(raw) : raw; const st = P.wsRestore(o); return st ? P.allPanes(st).map((x) => x.pane.env).filter(Boolean) : []; } catch (e) { return []; } };
    const waiting = saved.flatMap(envsOf).some((env) => { const e = NS.store.env(env); return e && e.state !== 'missing' && !S().chats.has(env); });
    if (waiting || !S().envs.length) return;
    restored = true;
    const keep = (p) => p.kind !== 'agent' || (!!NS.store.env(p.env) && (p.chat === 'new' || !!NS.store.findChat(p.chat)));
    const st = P.wsRestore(saved[0], keep) || P.wsRestore(saved[1], keep);
    if (st) state = st;
  }
  function persist() {
    const raw = P.wsSerialize(state);
    NS.storage.set(KEY, raw);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      const st = S().settings;
      if (!st) return;
      const prev = (st.ui && st.ui.tabs) || {};
      st.ui = Object.assign({}, st.ui, { tabs: Object.assign(JSON.parse(raw), prev.look ? { look: prev.look } : {}) });
      NS.api.call('set_settings', { settings: st }).catch(() => { /* the localStorage copy stays */ });
    }, 1200);
  }
  function setState(next) {
    if (next === state) return;
    state = next;
    persist();
    if (ws) ws.draw();
    syncRoute();
  }
  const activeTab = () => P.tabOf(state);
  const focused = () => P.focusedPane(state);
  // the shell (bar, chat list, theme accent) follows the focused agent pane
  function syncRoute() {
    const p = focused();
    if (p && p.kind === 'agent' && ws) NS.shell.setRoute(p.env, p.chat);
    if (p && NS.applyTheme) NS.applyTheme(undefined, p.env || null);
    if (NS.shell.barUpdate) NS.shell.barUpdate('focus');
  }

  // ---------- API for shell.js, chat.js and the palette ----------
  // nav('env', {env, chat}) lands here: the pane that shows that chat gets the focus, otherwise the focused agent pane shows it
  function show(env, chat) {
    restore();
    const mountWs = () => { if (!ws) NS.shell.mountScreen('tabs', {}); };
    if (chat === undefined && state.tabs.length && !ws) { mountWs(); syncRoute(); return; }      // the start of the window: restored tabs
    if (chat === undefined || chat === null) {
      const f = focused();
      if (f && f.kind === 'agent' && f.env === env) { mountWs(); ws && ws.draw(); syncRoute(); return; }
      const same = P.allPanes(state).find((x) => x.pane.kind === 'agent' && x.pane.env === env);
      if (same) { mountWs(); setState(P.wsFocusPane(state, same.pane.id)); return; }
      chat = NS.shell.defaultChat(env);
    }
    const hit = P.allPanes(state).find((x) => x.pane.kind === 'agent' && x.pane.env === env && x.pane.chat === chat);
    const f = focused();
    let st = state;
    if (hit) st = P.wsFocusPane(st, hit.pane.id);
    else if (!st.tabs.length) st = P.wsOpen(st, uid('tab-'), agentPane(env, chat));
    else if (f && f.kind === 'agent') st = P.wsSetPane(st, f.id, { env, chat });
    else st = P.wsOpen(st, uid('tab-'), agentPane(env, chat));
    state = st;
    persist();
    mountWs();
    if (ws) ws.draw();
    syncRoute();
  }
  // chat.js calls this with the pane id it was mounted with (ctx.params.tab)
  function setChat(paneId, env, chat) { setState(P.wsSetPane(state, paneId, { env, chat: chat || 'new' })); }
  function dropChat(chatId) {
    let st = state;
    for (const x of P.allPanes(state)) if (x.pane.kind === 'agent' && x.pane.chat === chatId) st = P.wsSetPane(st, x.pane.id, { chat: 'new' });
    setState(st);
  }
  const curEnv = () => { const f = focused(); return (f && f.env) || S().lastEnv || ((S().envs.find((e) => e.state === 'ok') || {}).id); };
  // a fresh pane: of the given kind (task 23: 'term'), or an agent session of the env in focus
  function freshPane(kind) {
    if (kind && kind !== 'agent') {
      const def = NS.registry.points('paneKind').find((d) => d.kind === kind);
      return Object.assign({ id: uid('p-'), kind }, def && def.create ? def.create(NS.shell.ctxFor({})) : {});
    }
    const env = curEnv();
    return env ? agentPane(env, 'new') : null;
  }
  function newTab(kind) {
    const p = freshPane(kind);
    if (!p) return;
    if (!ws) { state = P.wsOpen(state, uid('tab-'), p); persist(); NS.shell.mountScreen('tabs', {}); syncRoute(); focusActive(); return; }
    setState(P.wsOpen(state, uid('tab-'), p));
    focusActive();
  }
  function split(dir, kind) {
    const tb = activeTab();
    if (!tb) return newTab(kind);
    if (P.count(tb.root) >= P.MAX_PANES) { NS.shell.toast(t('pane.max')); return; }
    const p = freshPane(kind);
    if (p) { setState(P.wsSplit(state, dir === 'v' ? 'v' : 'h', p)); focusActive(); }
  }
  // a pane kind may ask before its pane goes (a terminal with a running process, 253): until the answer the layout stays
  // as it is, and "Отмена" leaves the pane where it was
  async function mayClose(panes) {
    for (const p of panes) { const d = kindDef(p.kind); if (d && d.beforeClose && !(await d.beforeClose(p))) return false; }
    return true;
  }
  async function closePane(paneId) {
    const tb = activeTab();
    if (!tb) return;
    const id = paneId || tb.focus, p = P.findPane(tb.root, id);
    if (p && !(await mayClose([p]))) { focusActive(); return; }
    if (!P.tabOf(state, tb.id)) return;
    let st = P.wsClosePane(P.wsActivate(state, tb.id), id);
    if (!st.tabs.length) { const p = freshPane(); if (p) st = P.wsOpen(st, uid('tab-'), p); }   // the window never stays empty
    setState(st);
    focusActive();
  }
  async function closeTab(id) {
    const tb = P.tabOf(state, id);
    if (!tb || !(await mayClose(P.leaves(tb.root)))) return;
    let st = P.wsClose(state, id);
    if (!st.tabs.length) { const p = freshPane(); if (p) st = P.wsOpen(st, uid('tab-'), p); }
    setState(st);
  }
  // where a pane stands, and a fresh pane of `kind` put back there (a terminal back from its own window, 260)
  const placeOf = (paneId) => P.wsPlaceOf(state, paneId);
  function putBack(kind, place) {
    const p = freshPane(kind);
    if (!p) return;
    const st = P.wsPut(state, place, p, uid('tab-'));
    if (!ws) { state = st; persist(); NS.shell.mountScreen('tabs', {}); syncRoute(); focusActive(); return; }
    setState(st);
    focusActive();
  }
  const zoom = () => setState(P.wsZoom(state));
  const focusDir = (dir) => { setState(P.wsFocus(state, dir)); focusActive(); };
  function equalize() {
    const tb = activeTab();
    if (!tb) return;
    let st = state;
    const walk = (n, path) => { if (!n || n.pane) return; st = P.wsRatio(st, path, 0.5); walk(n.split.a, path + 'a'); walk(n.split.b, path + 'b'); };
    walk(tb.root, '');
    setState(st);
  }
  // the cursor goes to the focused pane (its input field, or the terminal)
  function focusActive() { if (ws) (window.requestAnimationFrame || setTimeout)(() => ws && ws.focus()); }

  // ---------- keys (window, capture phase, only while the workspace is shown) ----------
  const keysOf = () => Object.assign(V.tabKeyDefaults(platform()), ((S().settings || {}).keys) || {});
  window.addEventListener('keydown', (e) => {
    if (!ws || e.defaultPrevented || e.isComposing) return;
    if (document.querySelector('dialog[open]') || (NS.palette && NS.palette.isOpen)) return;
    if (e.target && e.target.closest && e.target.closest('.keyfield')) return;      // the hotkey field in the settings records keys
    const mac = platform() === 'mac', mod = mac ? e.metaKey : e.ctrlKey;
    const code = e.code || '';
    let a = null;
    if (mod && e.shiftKey && !e.altKey && (code === 'Enter' || e.key === 'Enter')) a = { type: 'zoom' };
    else if (mod && e.altKey && !e.shiftKey && /^Arrow(Up|Down|Left|Right)$/.test(code || e.key)) a = { type: 'focus', dir: (code || e.key).slice(5).toLowerCase() };
    else a = V.tabKeyAction(e, platform(), keysOf());
    if (!a) return;
    e.preventDefault();
    e.stopPropagation();
    const ids = state.tabs.map((x) => x.id), i = ids.indexOf(state.active);
    if (a.type === 'new') newTab();
    else if (a.type === 'close') closePane();
    else if (a.type === 'split') split('h');
    else if (a.type === 'unsplit') split('v');                 // v4: ⌘⇧D splits down (it took the panel out of a v3 split)
    else if (a.type === 'zoom') zoom();
    else if (a.type === 'focus') focusDir(a.dir);
    else if (a.type === 'panel') focusDir(a.dir < 0 ? 'left' : 'right');
    else if (a.type === 'goto') { if (ids[a.n - 1]) setState(P.wsActivate(state, ids[a.n - 1])); }
    else if (a.type === 'last') { if (ids.length) setState(P.wsActivate(state, ids[ids.length - 1])); }
    else if (a.type === 'next' || a.type === 'prev') { if (ids.length > 1) setState(P.wsActivate(state, ids[(i + (a.type === 'next' ? 1 : -1) + ids.length) % ids.length])); }
    if (!['new', 'split', 'unsplit', 'close', 'focus'].includes(a.type)) focusActive();
  }, true);

  // ---------- pane kind 'agent': the v3 chat / swarm / env state screens inside a pane ----------
  NS.registry.extend('paneKind', { kind: 'agent', labelKey: 'pane.kind.agent', order: 1,
    mount(el, pane) {
      const env = NS.store.env(pane.env);
      let id = 'chat';
      const params = { env: pane.env, chat: pane.chat, tab: pane.id };
      if (!env || env.state !== 'ok') id = 'envstate';
      else {
        const sw = NS.registry.get('swarm');
        const swt = pane.chat !== 'new' && sw && !sw.placeholder ? L.swarmTaskOf(NS.store.chatTasks(pane.chat)) : null;
        if (swt) { id = 'swarm'; params.task = swt.id; }
      }
      return NS.registry.get(id).mount(el, NS.shell.ctxFor(params)) || null;
    },
    sig: (pane) => pane.env + '|' + pane.chat,
    title(pane) {
      if (pane.chat === 'new') return t('tabs.new_chat');
      const c = NS.store.findChat(pane.chat), task = NS.store.chatTasks(pane.chat)[0];
      return (c && c.title) || (task && task.title) || t('chat.untitled');
    },
  });
  const kindDef = (kind) => NS.registry.points('paneKind').find((d) => d.kind === kind) || null;
  function paneTitle(p) { const d = kindDef(p.kind); return d && d.title ? d.title(p) : d ? t(d.labelKey) : p.kind; }
  const stateOf = (p) => (p.kind !== 'agent' || p.chat === 'new' ? 'talk' : L.chatState(NS.store.chatTasks(p.chat)).kind);
  const short = (s) => (s.length > TITLE_MAX ? s.slice(0, TITLE_MAX - 1) + '…' : s);

  // ---------- the screen ----------
  function mount(root) {
    restore();
    const strip = h('div', { class: 'tabstrip', role: 'tablist', 'aria-label': t('tabs.label') });
    const corner = h('div', { class: 'tabs-corner' });
    const top = h('div', { class: 'ws-top' }, strip, corner);
    const area = h('div', { class: 'panes' });
    const wsEl = h('section', { class: 'ws' }, top, area);
    root.append(wsEl);
    const mounted = new Map();       // pane id -> {el, body, inst, sig, used, slots}
    let menu = null, dragId = null;
    const closeMenu = () => { if (menu) { menu.remove(); menu = null; document.removeEventListener('click', onDocMenu); } };
    const onDocMenu = (e) => { if (menu && !menu.contains(e.target)) closeMenu(); };

    // the corner right of the tabs: a slot for Clawd (task 25); the last registered def wins
    let cornerInst = null;
    function drawCorner() {
      const defs = NS.registry.points('tabsCorner');
      const def = defs[defs.length - 1];
      if (cornerInst && cornerInst.destroy) cornerInst.destroy();
      clear(corner);
      cornerInst = def ? def.mount(corner, NS.shell.ctxFor({})) || null : null;
    }

    function openMenu(tb, anchor) {
      closeMenu();
      const item = (label, fn, disabled) => h('button', { type: 'button', role: 'menuitem', text: label, disabled: !!disabled, onclick: () => { closeMenu(); fn(); } });
      menu = h('div', { class: 'menu tabmenu', role: 'menu', 'aria-label': t('tabs.menu') },
        item(t('tabs.close'), () => closeTab(tb.id)),
        state.tabs.length > 1 ? item(t('tabs.close_others'), () => { let st = state; for (const x of state.tabs) if (x.id !== tb.id) st = P.wsClose(st, x.id); setState(P.wsActivate(st, tb.id)); }) : null);
      const r = anchor.getBoundingClientRect(), rr = wsEl.getBoundingClientRect();
      setVars(menu, { '--x': Math.max(0, Math.min(r.left - rr.left, rr.width - 220)) + 'px', '--y': (r.bottom - rr.top + 4) + 'px' });
      wsEl.append(menu);
      setTimeout(() => document.addEventListener('click', onDocMenu), 0);
    }
    function tabEl(tb, i) {
      const on = tb.id === state.active;
      const p = P.findPane(tb.root, tb.focus) || P.leaves(tb.root)[0];
      const n = P.count(tb.root), kind = stateOf(p), title = paneTitle(p), env = p.env ? NS.store.env(p.env) : null;
      const word = { run: 'tabs.st_run', wait: 'tabs.st_wait', done: 'tabs.st_done', fail: 'tabs.st_fail', queue: 'tabs.st_queue', talk: 'tabs.st_talk' }[kind];
      const el = h('div', { class: 'tab' + (on ? ' on' : ''), role: 'tab', 'aria-selected': String(on), tabIndex: on ? 0 : -1, draggable: 'true',
        title: (env ? env.name + ' · ' : '') + title + ' · ' + t(word) + (i < 9 ? ' · ' + (platform() === 'mac' ? '⌘' : 'Ctrl+') + (i + 1) : ''), dataset: { tab: tb.id },
        onclick: () => { setState(P.wsActivate(state, tb.id)); focusActive(); },
        onauxclick: (e) => { if (e.button === 1) { e.preventDefault(); closeTab(tb.id); } },
        onmousedown: (e) => { if (e.button === 1) e.preventDefault(); },
        oncontextmenu: (e) => { e.preventDefault(); openMenu(tb, el); },
        onkeydown: (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setState(P.wsActivate(state, tb.id)); } },
        ondragstart: (e) => { dragId = tb.id; e.dataTransfer.effectAllowed = 'move'; try { e.dataTransfer.setData('text/plain', tb.id); } catch (er) { /* some engines refuse */ } el.classList.add('drag'); },
        ondragend: () => { dragId = null; el.classList.remove('drag'); },
        ondragover: (e) => { if (dragId && dragId !== tb.id) { e.preventDefault(); el.classList.add('over'); } },
        ondragleave: () => el.classList.remove('over'),
        ondrop: (e) => { e.preventDefault(); el.classList.remove('over'); if (dragId && dragId !== tb.id) setState(P.wsMove(state, dragId, state.tabs.findIndex((x) => x.id === tb.id))); } },
      h('span', { class: 'tab-ic' }, p.env ? envIcon(env && env.custom ? 'custom' : p.env) : ic('i-dash')),
      h('span', { class: 'tab-t' }, h('span', { class: 'tab-l', text: short(title) })),
      n > 1 ? h('small', { class: 'tab-n', text: String(n), title: n + ' · ' + t('tabs.label') }) : null,
      h('i', { class: 'tdot ' + kind, 'aria-hidden': 'true' }), h('span', { class: 'sr-only', text: t(word) }),
      h('button', { type: 'button', class: 'tab-x', 'aria-label': t('tabs.close'), text: '×', onclick: (e) => { e.stopPropagation(); closeTab(tb.id); } }));
      return el;
    }
    function drawStrip() {
      const list = state.tabs.slice(0, STRIP_MAX).map(tabEl);
      const rest = state.tabs.slice(STRIP_MAX);
      const more = rest.length ? h('button', { type: 'button', class: 'tab-more', text: t('tabs.more', { n: rest.length }), onclick: (e) => {
        e.stopPropagation(); closeMenu();
        menu = h('div', { class: 'menu tabmenu', role: 'menu' }, rest.map((tb) => h('button', { type: 'button', role: 'menuitem', text: short(paneTitle(P.findPane(tb.root, tb.focus))), onclick: () => { closeMenu(); setState(P.wsActivate(state, tb.id)); } })));
        const r = e.target.getBoundingClientRect(), rr = wsEl.getBoundingClientRect();
        setVars(menu, { '--x': Math.max(0, r.right - rr.left - 220) + 'px', '--y': (r.bottom - rr.top + 4) + 'px' });
        wsEl.append(menu); setTimeout(() => document.addEventListener('click', onDocMenu), 0);
      } }) : null;
      NS.append(clear(strip), [h('div', { class: 'tabs-scroll' }, list), more,
        h('button', { type: 'button', class: 'tab-plus', 'aria-label': t('tabs.new'), title: t('tabs.new') + ' (' + (platform() === 'mac' ? '⌘T' : 'Ctrl+T') + ')', onclick: () => newTab() }, ic('i-plus'))]);
      const a = strip.querySelector('.tab.on');
      if (a && a.scrollIntoView) a.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    }

    // the header of a pane: the kind's own (chat.js draws .pane-h) or a plain one; slots of the registry and the hover tools
    function tools(p) {
      const tb = activeTab(), zoomed = tb && tb.zoom && tb.focus === p.id;
      const b = (label, icon, fn, cls) => h('button', { type: 'button', class: 'pt' + (cls ? ' ' + cls : ''), 'aria-label': label, title: label, onclick: (e) => { e.stopPropagation(); setState(P.wsFocusPane(state, p.id)); fn(); } }, icon);
      return h('div', { class: 'pane-tools' },
        b(t('pane.split_r'), h('span', { class: 'pg pg-r', 'aria-hidden': 'true' }), () => split('h')),
        b(t('pane.split_d'), h('span', { class: 'pg pg-d', 'aria-hidden': 'true' }), () => split('v')),
        P.count(tb ? tb.root : null) > 1 ? b(zoomed ? t('pane.unzoom') : t('pane.zoom'), h('span', { class: 'pg ' + (zoomed ? 'pg-u' : 'pg-z'), 'aria-hidden': 'true' }), zoom) : null,
        b(t('pane.close'), h('span', { class: 'pg-x', 'aria-hidden': 'true', text: '×' }), () => closePane(p.id), 'pt-x'));
    }
    function paneFor(p) {
      const def = kindDef(p.kind);
      const sig = p.kind + '|' + (def && def.sig ? def.sig(p) : JSON.stringify(p));
      let m = mounted.get(p.id);
      if (m && m.sig === sig) { m.used = Date.now(); return m; }
      if (m) destroyPane(p.id);
      const body = h('div', { class: 'pane-body' });
      const el = h('div', { class: 'pane k-' + p.kind, dataset: { pane: p.id } }, body);
      el.addEventListener('pointerdown', () => { const f = focused(); if (!f || f.id !== p.id) setState(P.wsFocusPane(state, p.id)); }, true);
      el.addEventListener('focusin', () => { const f = focused(); if (!f || f.id !== p.id) setState(P.wsFocusPane(state, p.id)); });
      let inst = null;
      if (def) inst = def.mount(body, p, NS.shell.ctxFor({ pane: p })) || null;
      else body.append(h('p', { class: 'pane-missing', text: t('pane.missing_kind', { kind: p.kind }) }));
      let head = body.querySelector('.pane-h');
      if (!head) { head = h('header', { class: 'pane-h' }, h('span', { class: 'pane-ttl', text: paneTitle(p) })); body.prepend(head); }
      const slots = h('div', { class: 'pane-slots' });
      const slotInst = [];
      for (const s of NS.registry.points('paneHeader')) {
        if (s.kinds && !s.kinds.includes(p.kind)) continue;
        const holder = h('span', { class: 'pane-slot', dataset: { slot: s.id } });
        slots.append(holder);
        slotInst.push(s.render(holder, p, NS.shell.ctxFor({ pane: p })) || null);
      }
      const tl = tools(p);
      const menuBtn = head.querySelector('.menu-btn');
      if (menuBtn) { menuBtn.before(slots); menuBtn.before(tl); } else head.append(slots, tl);
      m = { el, body, inst, sig, used: Date.now(), slotInst, tools: tl };
      mounted.set(p.id, m);
      return m;
    }
    function destroyPane(id) {
      const m = mounted.get(id);
      if (!m) return;
      if (m.inst && m.inst.destroy) m.inst.destroy();
      for (const s of m.slotInst) if (s && s.destroy) s.destroy();
      m.el.remove();
      mounted.delete(id);
    }
    // the tree as nested flex boxes; panes are moved into place, never re-created while their kind and data stay
    function build(n, path, tb) {
      if (n.pane) {
        const m = paneFor(n.pane);
        const nt = tools(n.pane); m.tools.replaceWith(nt); m.tools = nt;
        m.el.classList.toggle('focus', tb.focus === n.pane.id);
        m.el.hidden = false;
        return m.el;
      }
      const { dir, ratio, a, b } = n.split;
      const A = h('div', { class: 'sp-a' }, build(a, path + 'a', tb)), B = h('div', { class: 'sp-b' }, build(b, path + 'b', tb));
      setVars(A, { '--f': String(ratio) }); setVars(B, { '--f': String(1 - ratio) });
      const box = h('div', { class: 'split ' + dir });
      const div = h('div', { class: 'divider', role: 'separator', 'aria-orientation': dir === 'h' ? 'vertical' : 'horizontal', title: t('pane.divider'),
        ondblclick: () => setState(P.wsRatio(state, path, 0.5)),
        onpointerdown: (e) => {
          e.preventDefault();
          div.setPointerCapture(e.pointerId);
          const r = box.getBoundingClientRect();
          box.classList.add('dragging');
          const move = (ev) => {
            const v = dir === 'h' ? (ev.clientX - r.left) / r.width : (ev.clientY - r.top) / r.height;
            const c = Math.max(0.1, Math.min(0.9, v));
            setVars(A, { '--f': String(c) }); setVars(B, { '--f': String(1 - c) });
            div.dataset.v = String(c);
          };
          const up = () => { div.removeEventListener('pointermove', move); div.removeEventListener('pointerup', up); box.classList.remove('dragging'); if (div.dataset.v) setState(P.wsRatio(state, path, Number(div.dataset.v))); };
          div.addEventListener('pointermove', move); div.addEventListener('pointerup', up);
        } });
      box.append(A, div, B);
      return box;
    }
    function drawPanes() {
      const tb = activeTab();
      const live = new Set(P.allPanes(state).map((x) => x.pane.id));
      for (const id of [...mounted.keys()]) if (!live.has(id)) destroyPane(id);
      // hidden panes (other tabs) stay mounted up to a limit: their feed and draft stay; the oldest go
      const shown = new Set(tb ? P.leaves(tb.root).map((p) => p.id) : []);
      const hidden = [...mounted].filter(([id]) => !shown.has(id)).sort((x, y) => x[1].used - y[1].used);
      while (hidden.length > KEEP_MOUNTED) destroyPane(hidden.shift()[0]);
      const keep = h('div', { class: 'pane-store', hidden: true });
      for (const [id, m] of mounted) if (!shown.has(id)) { m.el.hidden = true; keep.append(m.el); }
      if (!tb) { area.replaceChildren(keep); return; }
      let tree = build(tb.root, '', tb);
      area.classList.toggle('zoomed', !!tb.zoom);
      if (tb.zoom) { const m = mounted.get(tb.focus); for (const [id, x] of mounted) if (id !== tb.focus) { x.el.hidden = true; keep.append(x.el); } tree = m ? m.el : tree; }
      area.classList.toggle('single', P.count(tb.root) === 1);
      area.replaceChildren(tree, keep);
    }
    function draw() { closeMenu(); drawStrip(); drawPanes(); }
    const focus = () => {
      const f = focused(), m = f && mounted.get(f.id);
      if (!m) return;
      if (m.inst && m.inst.focus) m.inst.focus();
      else if (m.inst && m.inst.update) m.inst.update('focus-input');
    };
    ws = { draw, focus };
    const offExt = NS.registry.onExtend((point) => { if (point === 'tabsCorner') drawCorner(); if (point === 'paneHeader' || point === 'paneKind') { for (const id of [...mounted.keys()]) destroyPane(id); drawPanes(); } });
    drawCorner();
    draw();
    const forward = (evt, payload) => {
      const f = focused();
      for (const [id, m] of mounted) {
        if (evt === 'focus-input' && (!f || id !== f.id)) continue;
        if (m.inst && m.inst.update) m.inst.update(evt, payload);
        for (const s of m.slotInst) if (s && s.update) s.update(evt, payload);
      }
      if (cornerInst && cornerInst.update) cornerInst.update(evt, payload);
    };
    return {
      update(evt, payload) {
        if (['task', 'chats', 'chatmsg', 'envs', 'taskremoved'].includes(evt)) drawStrip();
        if (evt === 'task' && payload && payload.chat_id && payload.swarm) {      // a chat that became a swarm switches its pane to the swarm screen
          for (const x of P.allPanes(state)) if (x.pane.chat === payload.chat_id) { const m = mounted.get(x.pane.id); if (m) m.sig = ''; }
          drawPanes();
        }
        forward(evt, payload);
      },
      destroy() {
        ws = null; closeMenu(); offExt();
        if (cornerInst && cornerInst.destroy) cornerInst.destroy();
        for (const id of [...mounted.keys()]) destroyPane(id);
      },
    };
  }

  NS.registry.register({ id: 'tabs', layout: 'chats', mount });
  NS.tabs = { show, setChat, dropChat, newTab, closeTab, closePane, placeOf, putBack, split, zoom, focusDir, equalize,
    get state() { return state; }, get active() { return focused(); }, get activeTab() { return activeTab(); } };
})();
