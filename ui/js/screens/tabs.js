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
    let st = P.wsRestore(saved[0], keep) || P.wsRestore(saved[1], keep);
    if (st) for (const x of P.allPanes(st)) if (x.pane.kind === 'agent' && x.pane.term) st = P.wsSetPane(st, x.pane.id, { term: null });   // v10
    // v10: an agent's CLI saved as a bare terminal (the v7 way; a shell has no env) comes back as its chat - every session looks the same
    if (st) for (const x of P.allPanes(st)) {
      const q = x.pane;
      if (q.kind !== 'term' || !q.env || !NS.store.env(q.env)) continue;
      // an agent started with a task («Новый агент» → «Запустить») is a terminal on purpose, often in its own copy
      // (q.branch): it comes back as it was and continues its talk; before, it became an empty «Новый чат» and lost the copy
      if (q.branch || q.prompt) continue;
      if (q.termId) NS.api.call('term_close', { id: q.termId }).catch(() => {});
      const chat = q.chat && NS.store.findChat(q.chat) ? q.chat : 'new';
      st = P.wsSetPane(st, q.id, { kind: 'agent', chat, termId: undefined, argv: undefined, cwd: undefined, title: undefined, prompt: undefined, branch: undefined });
    }
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
    const switched = next.active !== state.active;
    state = next;
    persist();
    if (ws) { ws.draw(); if (switched && ws.switched) ws.switched(); }
    syncRoute();
  }
  const activeTab = () => P.tabOf(state);
  const focused = () => P.focusedPane(state);
  // the shell (bar, chat list, theme accent) follows the focused agent pane
  function syncRoute() {
    const p = focused();
    if (p && p.env && ws) NS.shell.setRoute(p.env, p.kind === 'agent' ? p.chat : undefined);   // v8: a terminal in focus too
    if (p && NS.applyTheme) NS.applyTheme(undefined, p.env || null);
    if (NS.shell.barUpdate) NS.shell.barUpdate('focus');
  }

  // ---------- API for shell.js, chat.js and the palette ----------
  // nav('env', {env, chat}) lands here: the pane that shows that chat gets the focus, otherwise the focused agent pane shows it
  function show(env, chat) {
    restore();
    const mountWs = () => { if (!ws) NS.shell.mountScreen('tabs', {}); };
    if (chat === undefined && state.tabs.length && !ws) { mountWs(); syncRoute(); return; }      // the start of the window: restored tabs
    // v7: the first agent is a terminal - unless the window opens on a chat (?chat=, a notification) or a check asks ?first=chat
    if (chat === undefined && !state.tabs.length && !/[?&](first=chat|chat=)/.test(location.search)) {
      const p0 = freshPane();
      if (p0) { state = P.wsOpen(state, uid('tab-'), p0); persist(); mountWs(); if (ws) ws.draw(); syncRoute(); return; }
    }
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
  // v10 (Agentation 29.10): a session is a chat over its CLI; «Терминал» shows the same talk in the real CLI in the same pane,
  // «Чат» ends that CLI and the core brings its talk back into the chat (back_from_terminal)
  const paneById = (id) => { const x = P.allPanes(state).find((y) => y.pane.id === id); return x ? x.pane : null; };
  async function chatView(paneId, view) {
    const p = paneById(paneId);
    if (!p || p.kind !== 'agent' || p.chat === 'new') return;
    if (view === 'term') {
      if (p.term) return;
      await NS.chatShell.openTerminal(p.chat, { place: (spec) => { if (paneById(paneId)) setState(P.wsSetPane(state, paneId, { term: spec })); } });
      return;
    }
    if (!p.term) return;
    NS.api.call('term_close', { id: p.term.termId }).catch(() => {});
    setState(P.wsSetPane(state, paneId, { term: null }));
  }
  // the head of a session pane: the env, the title and the switch; the chat draws its own (chat.js), the terminal view uses this one
  function viewSwitch(pane) {
    const on = pane.term ? 'term' : 'chat', fresh = pane.chat === 'new';
    const b = (v, label) => h('button', { type: 'button', role: 'radio', class: v === on ? 'on' : '', 'aria-checked': String(v === on), text: t(label),
      disabled: v === 'term' && fresh, title: v === 'term' ? t(fresh ? 'view10.term_wait' : 'view10.term_d') : t('view10.chat_d'), onclick: () => chatView(pane.id, v) });
    return h('div', { class: 'seg view10', role: 'radiogroup', 'aria-label': t('view10.label') }, b('chat', 'view10.chat'), b('term', 'view10.term'));
  }
  function dropChat(chatId) {
    let st = state;
    for (const x of P.allPanes(state)) if (x.pane.kind === 'agent' && x.pane.chat === chatId) st = P.wsSetPane(st, x.pane.id, { chat: 'new' });
    setState(st);
  }
  const curEnv = () => { const f = focused(); return fEnv() || (f && f.env) || S().lastEnv || ((S().envs.find((e) => e.state === 'ok') || {}).id); };
  // v8: the agent picked in the column (NS.envrail) shows only its sessions: in the list, in the tabs above the terminal, for ⌘1…9
  const fEnv = () => (NS.envrail ? NS.envrail.filter : '');
  const visTabs = () => { const f = fEnv(); return f ? state.tabs.filter((tb) => P.leaves(tb.root).some((p) => p.env === f)) : state.tabs; };
  function filterChanged() {
    const vis = visTabs();
    if (vis.length && !vis.some((x) => x.id === state.active)) { setState(P.wsActivate(state, vis[0].id)); focusActive(); return; }
    if (ws) ws.draw();
  }
  // a fresh pane: of the given kind (task 23: 'term'), or an agent session of the env in focus
  function freshPane(kind) {
    if (kind === 'chat') { const env = curEnv(); return env ? agentPane(env, 'new') : null; }
    if (kind && kind !== 'agent') {
      const def = NS.registry.points('paneKind').find((d) => d.kind === kind);
      return Object.assign({ id: uid('p-'), kind }, def && def.create ? def.create(NS.shell.ctxFor({})) : {});
    }
    const env = curEnv();
    if (!env) return null;
    // v10 (Agentation 29.10): a session opens as a chat; the CLI runs under it and «Терминал» in its head shows the same talk in the CLI
    return agentPane(env, 'new');
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
    // Linux: Ctrl+W/T/D inside a terminal belong to the shell (delete word, transpose, EOF); the app answers to Ctrl+Shift+… there
    if (!mac && !e.shiftKey && code !== 'Tab' && !/^Digit/.test(code) && e.target && e.target.classList && e.target.classList.contains('xterm-helper-textarea')) return;
    let a = null;
    if (mod && e.shiftKey && !e.altKey && (code === 'Enter' || e.key === 'Enter')) a = { type: 'zoom' };
    else if (mod && e.altKey && !e.shiftKey && /^Arrow(Up|Down|Left|Right)$/.test(code || e.key)) a = { type: 'focus', dir: (code || e.key).slice(5).toLowerCase() };
    else a = V.tabKeyAction(e, platform(), keysOf());
    if (!a) return;
    e.preventDefault();
    e.stopPropagation();
    const ids = visTabs().map((x) => x.id), i = ids.indexOf(state.active);
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
      if (pane.term) {
        // the same pane id: a redraw detaches the CLI (it keeps running), only «Чат» or closing the pane ends it
        const e = NS.store.env(pane.env), body = h('div', { class: 'view10-body' });
        el.append(h('header', { class: 'pane-h view10-h' }, h('span', { class: 'pane-ic' }, envIcon(e && e.custom ? 'custom' : pane.env)),
          h('span', { class: 'pane-t', text: kindDef('agent').title(pane) }), h('span', { class: 'pane-gap' }), viewSwitch(pane)), body);
        return kindDef('term').mount(body, Object.assign({}, pane.term, { id: pane.id, kind: 'term', env: pane.env, chat: pane.chat }));
      }
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
    sig: (pane) => pane.env + '|' + pane.chat + '|' + (pane.term ? pane.term.termId : ''),
    title(pane) {
      if (pane.chat === 'new') return t('tabs.new_chat');
      const c = NS.store.findChat(pane.chat), task = NS.store.chatTasks(pane.chat)[0];
      return (c && c.title) || (task && task.title) || t('chat.untitled');
    },
  });
  const kindDef = (kind) => NS.registry.points('paneKind').find((d) => d.kind === kind) || null;
  function paneTitle(p) { const d = kindDef(p.kind); return d && d.title ? d.title(p) : d ? t(d.labelKey) : p.kind; }
  // a chat whose tasks were all cancelled or undone: a stamped, neutral dot instead of a green "done"
  const stamped = (p, kind) => { const tk = kind === 'done' && p.kind === 'agent' && p.chat !== 'new' ? NS.store.chatTasks(p.chat) : []; return tk.length > 0 && tk.every((x) => x.status === 'cancelled' || x.status === 'reverted'); };
  const stateOf = (p) => (p.kind !== 'agent' || p.chat === 'new' ? 'talk' : L.chatState(NS.store.chatTasks(p.chat)).kind);
  const short = (s) => (s.length > TITLE_MAX ? s.slice(0, TITLE_MAX - 1) + '…' : s);

  // ---------- the screen ----------
  function mount(root) {
    restore();
    // v7: the agents are a column on the left (tabs as cards: what each terminal says, its state, its project); panes on the right
    const strip = h('div', { class: 'tabstrip rail7-list', role: 'tablist', 'aria-orientation': 'vertical', 'aria-label': t('agents.rail') });
    const corner = h('div', { class: 'tabs-corner' });
    const nav = h('nav', { class: 'ws-nav rail7-foot', 'aria-label': t('nav.label') });
    const newBtn = h('button', { type: 'button', class: 'rail7-new', 'aria-haspopup': 'dialog', title: t('agents.new_d') + ' · ' + (platform() === 'mac' ? '⌘T' : 'Ctrl+T'),
      onclick: (e) => { e.stopPropagation(); if (NS.agents) NS.agents.launcher(e.currentTarget); else openPlus(e.currentTarget); } }, ic('i-plus'), h('span', { text: t('agents.new') }));
    // v8: the mark and the name, the project under them (recent, open a folder, create); the agents column on the far left
    const head = h('div', { class: 'rail7-h' }, h('span', { class: 'rail7-brand' }, ic('i-ns', 'rail7-logo'), h('span', { text: 'Nightshift' })), corner);
    const proj = NS.projects.picker(() => NS.projects.current(), () => { if (NS.shell.barUpdate) NS.shell.barUpdate('project'); }, { plain: true });
    const rail = h('aside', { class: 'rail7' }, head, h('div', { class: 'rail7-proj' }, proj.el), newBtn, strip, nav);
    const envCol = h('nav', { class: 'er8-col' });
    const envInst = NS.envrail ? NS.envrail.mount(envCol) : null;
    const area = h('div', { class: 'panes' });
    const tabs8 = h('div', { class: 'tabs8', role: 'tablist', 'aria-label': t('tabs8.label') });
    // v8: the GitHub panel left of the terminals (gh8.js), folded by its button at the end of the session tabs
    const ghEl = h('aside', { class: 'gh8-col', 'aria-label': 'GitHub' });
    let ghInst = null;
    const ghShow = (on) => { if (!NS.gh8) return; NS.gh8.setOpen(on); ghEl.hidden = !on; if (on && !ghInst) ghInst = NS.gh8.mount(ghEl); drawTabs8(); };
    const wsEl = h('section', { class: 'ws ws7' }, envCol, rail, h('div', { class: 'ws7-main' }, tabs8, h('div', { class: 'ws7-body' }, area, ghEl)));
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
        state.tabs.length > 1 ? item(t('tabs.close_others'), async () => {
          const others = state.tabs.filter((x) => x.id !== tb.id);
          if (!(await mayClose(others.flatMap((x) => P.leaves(x.root))))) return;
          let st = state; for (const x of others) st = P.wsClose(st, x.id); setState(P.wsActivate(st, tb.id));
        }) : null);
      const r = anchor.getBoundingClientRect(), rr = wsEl.getBoundingClientRect();
      setVars(menu, { '--x': Math.max(0, Math.min(r.left - rr.left, rr.width - 220)) + 'px', '--y': (r.bottom - rr.top + 4) + 'px' });
      wsEl.append(menu);
      setTimeout(() => document.addEventListener('click', onDocMenu), 0);
    }
    // "+" (F8): a small menu in the look of the model menu - a chat, the CLI of the env in a terminal, a bare shell
    function openPlus(anchor) {
      if (menu && menu.classList.contains('plusmenu')) { closeMenu(); anchor.focus(); return; }
      closeMenu();
      const mac = platform() === 'mac', K = (a) => window.NSLogic.keyLabel(mac ? a : a.replace('Shift+Cmd', 'Ctrl+Shift').replace('Cmd', 'Ctrl'), mac ? 'mac' : 'linux').join('');
      const env = NS.store.env(curEnv()), cli = env && env.state !== 'missing' && NS.term && NS.term.envTerm ? env : null;
      const row = (icon, label, desc, key, fn, off) => h('div', { class: 'mrow plus-row', role: 'menuitem', tabIndex: off ? -1 : 0, title: desc, 'aria-disabled': off ? 'true' : null,
        onclick: () => { if (off) return; closeMenu(); anchor.focus(); fn(); }, onkeydown: (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.currentTarget.click(); } } },
        h('span', { class: 'plus-ic', 'aria-hidden': 'true' }, icon), h('span', { class: 'mn', text: label }), key ? h('kbd', { text: key }) : null);
      const rows = [
        row(env ? envIcon(env.custom ? 'custom' : env.id) : ic('i-chat'), t('plus.chat'), t('plus.chat_d'), '', () => newTab('chat')),
        cli ? row(h('span', { class: 't-glyph', text: '>_' }), t('plus.term_cli', { env: cli.name }), t('plus.term_cli_d', { env: cli.name }), '', () => NS.term.envTerm(cli.id)) : null,
        row(h('span', { class: 't-glyph', text: '>_' }), t('plus.term_shell'), t('plus.term_shell_d'), K('Shift+Cmd+T'), () => (NS.term && NS.term.open ? NS.term.open() : null), !(NS.term && NS.term.open))];
      menu = h('div', { class: 'menu plusmenu', role: 'menu', 'aria-label': t('plus.label'),
        onkeydown: (e) => {
          const items = [...menu.querySelectorAll('[role=menuitem]:not([aria-disabled])')], i = items.indexOf(document.activeElement);
          if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); items[(i + (e.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length].focus(); }
          else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); closeMenu(); anchor.focus(); }
        } }, rows);
      const r = anchor.getBoundingClientRect(), rr = wsEl.getBoundingClientRect();
      setVars(menu, { '--x': Math.max(0, Math.min(r.left - rr.left, rr.width - 280)) + 'px', '--y': (r.bottom - rr.top + 4) + 'px' });
      wsEl.append(menu);
      setTimeout(() => document.addEventListener('click', onDocMenu), 0);
      const first = menu.querySelector('[role=menuitem]'); if (first) first.focus();
    }
    // the entrances that used to hide behind ⌘K: the factory, the settings, the palette (F8)
    function drawNav() {
      const mac = platform() === 'mac';
      const nb = (cls, icon, label, desc, fn) => h('button', { type: 'button', class: 'navb ' + cls, title: desc, 'aria-label': label, onclick: fn }, icon, h('span', { class: 'nav-l', text: label }));
      // v7: the factory, the swarm and the automation live in ⌘K and in "Новый агент"; the screen keeps the palette and the settings
      NS.append(clear(nav),
        [NS.agents ? NS.agents.waitButton() : null,     // drawn first, shown as its own row on top (CSS order)
          nb('nb-pal', h('kbd', { text: mac ? '⌘K' : 'Ctrl+K' }), t('agents.search'), t('nav.palette_d'), () => NS.palette.open()),
          NS.auto ? NS.auto.navButton() : null]);       // v10: the settings are the gear under the agents (envrail.js), not a second button here
    }
    function tabEl(tb, i) {
      const on = tb.id === state.active;
      const p = P.findPane(tb.root, tb.focus) || P.leaves(tb.root)[0];
      const n = P.count(tb.root), kind = stateOf(p), title = paneTitle(p), env = p.env ? NS.store.env(p.env) : null;
      const word = { run: 'tabs.st_run', wait: 'tabs.st_wait', done: 'tabs.st_done', fail: 'tabs.st_fail', queue: 'tabs.st_queue', talk: 'tabs.st_talk' }[kind];
      // role=tab sits on the title: WebKit hands an AX press of an element to a control inside it, so a tab holding its × closed on VoiceOver "select"
      const el = h('div', { class: 'tab' + (on ? ' on' : ''), draggable: 'true',
        title: (env ? env.name + ' · ' : '') + title + ' · ' + t(word) + (i < 9 ? ' · ' + (platform() === 'mac' ? '⌘' : 'Ctrl+') + (i + 1) : ''), dataset: { tab: tb.id },
        onclick: () => { setState(P.wsActivate(state, tb.id)); focusActive(); },
        onauxclick: (e) => { if (e.button === 1) { e.preventDefault(); closeTab(tb.id); } },
        onmousedown: (e) => { if (e.button === 1) e.preventDefault(); },
        oncontextmenu: (e) => { e.preventDefault(); openMenu(tb, el); },
        onkeydown: (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setState(P.wsActivate(state, tb.id)); } else if (e.key === 'Delete') { e.preventDefault(); closeTab(tb.id); } },
        ondragstart: (e) => { dragId = tb.id; e.dataTransfer.effectAllowed = 'move'; try { e.dataTransfer.setData('text/plain', tb.id); } catch (er) { /* some engines refuse */ } el.classList.add('drag'); },
        ondragend: () => { dragId = null; el.classList.remove('drag'); },
        ondragover: (e) => { if (dragId && dragId !== tb.id) { e.preventDefault(); el.classList.add('over'); } },
        ondragleave: () => el.classList.remove('over'),
        ondrop: (e) => { e.preventDefault(); el.classList.remove('over'); if (dragId && dragId !== tb.id) setState(P.wsMove(state, dragId, state.tabs.findIndex((x) => x.id === tb.id))); } },
      h('span', { class: 'tab-ic' + (p.kind === 'term' ? ' is-term' : '') }, p.kind === 'term' && !p.env ? h('span', { class: 't-glyph', 'aria-hidden': 'true', text: '>_' }) : p.env ? envIcon(env && env.custom ? 'custom' : p.env) : ic('i-dash')),
      h('span', { class: 'tab-t', role: 'tab', 'aria-selected': String(on), tabIndex: on ? 0 : -1 }, h('span', { class: 'tab-l', text: short(title) })),
      n > 1 ? h('small', { class: 'tab-n', text: String(n), title: n + ' · ' + t('tabs.label') }) : null,
      h('button', { type: 'button', class: 'tab-x', 'aria-label': t('tabs.close'), title: t('tabs.close') + ' · Delete', text: '×', onclick: (e) => { e.stopPropagation(); closeTab(tb.id); } }),
      // v7: the card - the last lines of the terminal (term.js fills them), the spend (agents7.js); v8: the state as a word with
      // its dot under the output, the shortcut only in the tooltip, the env name only in the tooltip (the column says it)
      h('div', { class: 'tab-pv', 'aria-hidden': 'true' }),
      h('div', { class: 'tab-meta' }, h('span', { class: 'tab-sw' }, h('i', { class: 'tdot ' + kind + (stamped(p, kind) ? ' stamp' : ''), 'aria-hidden': 'true' }), h('span', { class: 'tab-st', text: t(word) })),
        p.branch ? h('span', { class: 'tab-br', text: p.branch }) : null, h('span', { class: 'tab-use', text: NS.agents ? NS.agents.useOf(p.id) : '' })));
      el.dataset.state = kind;
      if (p.env) el.dataset.env = String(p.env).startsWith('custom') ? 'custom' : p.env;
      return el;
    }
    function drawStrip() {
      // v7: cards grouped by project (the order of the groups = where each first appears); ⌘1…9 follow what is on screen
      const f = fEnv(), vis = visTabs();
      const G = NS.agents ? NS.agents.groups(vis, (tb) => P.findPane(tb.root, tb.focus) || P.leaves(tb.root)[0]) : null;
      // the saved order follows the groups only while every tab is on screen (a filtered list must not drop the others)
      if (G && !f && G.order.join() !== state.tabs.map((x) => x.id).join()) { state = Object.assign({}, state, { tabs: G.order.map((id) => state.tabs.find((x) => x.id === id)) }); persist(); }
      let at = 0;
      const list = G ? G.groups.flatMap((g) => [G.groups.length > 1 || g.key ? NS.agents.groupHead(g) : null].concat(g.tabs.map((tb) => tabEl(tb, at++)))).filter(Boolean)
        : vis.slice(0, STRIP_MAX).map(tabEl);
      const rest = G ? [] : vis.slice(STRIP_MAX);
      const more = rest.length ? h('button', { type: 'button', class: 'tab-more', text: t('tabs.more', { n: rest.length }), onclick: (e) => {
        e.stopPropagation(); closeMenu();
        menu = h('div', { class: 'menu tabmenu', role: 'menu' }, rest.map((tb) => h('button', { type: 'button', role: 'menuitem', text: short(paneTitle(P.findPane(tb.root, tb.focus))), onclick: () => { closeMenu(); setState(P.wsActivate(state, tb.id)); } })));
        const r = e.target.getBoundingClientRect(), rr = wsEl.getBoundingClientRect();
        setVars(menu, { '--x': Math.max(0, r.right - rr.left - 220) + 'px', '--y': (r.bottom - rr.top + 4) + 'px' });
        wsEl.append(menu); setTimeout(() => document.addEventListener('click', onDocMenu), 0);
      } }) : null;
      for (const c of [...strip.children]) if (c !== sel) c.remove();          // the highlight stays the same element: it glides
      if (!sel.isConnected) strip.prepend(sel);
      const sc = h('div', { class: 'tabs-scroll' }, list);
      NS.append(strip, [sc, more]);
      selRO.disconnect(); selRO.observe(sc);                    // a card grows when its preview arrives: the highlight follows
      const a = strip.querySelector('.tab.on');
      if (a && a.scrollIntoView) a.scrollIntoView({ block: 'nearest', inline: 'nearest' });
      placeSel();
    }
    // v7: the highlight under the card in focus moves to it (transform + height, a spring), in the colour of its env
    const sel = h('div', { class: 'rail7-sel', 'aria-hidden': 'true' });
    function placeSel() {
      const a = strip.querySelector('.tab.on');
      if (!a) { setVars(sel, { '--o': '0' }); return; }
      const cs = getComputedStyle(a).getPropertyValue('--env');
      const r = a.getBoundingClientRect(), sr = strip.getBoundingClientRect();
      setVars(sel, { '--y': (r.top - sr.top + strip.scrollTop) + 'px', '--h': r.height + 'px', '--o': '1', '--sel-env': cs ? cs.trim() : 'var(--amber)' });
    }
    const selRO = new ResizeObserver(() => placeSel());
    new ResizeObserver(() => placeSel()).observe(strip);

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
    // v8: the sessions of the agent in the column as tabs above the terminal, "+" opens one more (⌘T)
    function drawTabs8() {
      const env = NS.store.env(curEnv()), name = env ? env.name : '';
      const tab = (tb) => {
        const p = P.findPane(tb.root, tb.focus) || P.leaves(tb.root)[0], on = tb.id === state.active;
        return h('div', { class: 't8' + (on ? ' on' : ''), title: paneTitle(p), dataset: { t8: tb.id, env: p.env ? (String(p.env).startsWith('custom') ? 'custom' : p.env) : '' },
          onclick: () => { setState(P.wsActivate(state, tb.id)); focusActive(); },
          onauxclick: (e) => { if (e.button === 1) { e.preventDefault(); closeTab(tb.id); } },
          onkeydown: (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setState(P.wsActivate(state, tb.id)); focusActive(); } else if (e.key === 'Delete') { e.preventDefault(); closeTab(tb.id); } } },
        h('span', { class: 't8-ic' }, p.kind === 'term' && !p.env ? h('span', { class: 't-glyph', 'aria-hidden': 'true', text: '>_' }) : p.env ? envIcon(String(p.env).startsWith('custom') ? 'custom' : p.env) : ic('i-dash')),
        // role=tab on the label, not on the box with the ×: WebKit hands an AX press to a control inside the pressed element
        h('span', { class: 't8-l', role: 'tab', 'aria-selected': String(on), tabIndex: on ? 0 : -1, text: short(paneTitle(p)) }),
        h('button', { type: 'button', class: 't8-x', 'aria-label': t('tabs.close'), title: t('tabs.close') + ' · Delete', text: '×', onclick: (e) => { e.stopPropagation(); closeTab(tb.id); } }));
      };
      const add = h('button', { type: 'button', class: 't8-new', title: (name ? t('tabs8.new_d', { env: name }) : t('tabs8.new')) + ' · ' + (platform() === 'mac' ? '⌘T' : 'Ctrl+T'), 'aria-label': t('tabs8.new'), onclick: () => newTab() }, ic('i-plus'));
      const gh = NS.gh8 ? h('button', { type: 'button', class: 't8-gh' + (NS.gh8.open ? ' on' : ''), title: t('gh8.toggle'), 'aria-label': t('gh8.toggle'), 'aria-pressed': String(!!NS.gh8.open), onclick: () => ghShow(!NS.gh8.open) }, ic('i-branch'), h('span', { text: 'GitHub' })) : null;
      NS.append(clear(tabs8), [h('div', { class: 't8-list' }, visTabs().map(tab)), add, h('span', { class: 'grow' }), gh]);
      const a = tabs8.querySelector('.t8.on');
      if (a && a.scrollIntoView) a.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    }
    // an agent of the column with no sessions yet: a quiet card instead of another agent's terminal
    function drawEmpty() {
      const env = NS.store.env(fEnv());
      area.replaceChildren(h('div', { class: 'empty8' }, env ? envIcon(env.custom ? 'custom' : env.id) : null, h('p', { text: t('tabs8.empty', { env: env ? window.NSEnvRail.shortName(env) : '' }) }),
        h('button', { type: 'button', class: 'btn primary', text: t('tabs8.start'), onclick: () => newTab() })), ...[...mounted.values()].map((m) => { m.el.hidden = true; return m.el; }));
    }
    function draw() { closeMenu(); drawStrip(); drawTabs8(); if (fEnv() && !visTabs().length) drawEmpty(); else drawPanes(); }
    const focus = () => {
      const f = focused(), m = f && mounted.get(f.id);
      if (!m) return;
      if (m.inst && m.inst.focus) m.inst.focus();
      else if (m.inst && m.inst.update) m.inst.update('focus-input');
    };
    let swT = 0;
    const switched = () => { area.classList.remove('switch7'); void area.offsetWidth; area.classList.add('switch7'); clearTimeout(swT); swT = setTimeout(() => area.classList.remove('switch7'), 320); };
    ws = { draw, focus, openPlus, switched };
    // v7: a card dropped on the terminals opens beside the one in focus (left half → left/right split, lower third → down)
    area.addEventListener('dragover', (e) => { if (dragId && dragId !== state.active) { e.preventDefault(); area.classList.add('drop7'); } });
    area.addEventListener('dragleave', (e) => { if (!area.contains(e.relatedTarget)) area.classList.remove('drop7'); });
    area.addEventListener('drop', (e) => { area.classList.remove('drop7'); if (!dragId || dragId === state.active) return; e.preventDefault(); const r = area.getBoundingClientRect(); besidePane(dragId, e.clientY - r.top > r.height * 0.66 ? 'v' : 'h'); });
    const offExt = NS.registry.onExtend((point) => { if (point === 'tabsCorner') drawCorner(); if (point === 'paneHeader' || point === 'paneKind') { for (const id of [...mounted.keys()]) destroyPane(id); drawPanes(); } });
    drawCorner(); drawNav();
    ghEl.hidden = true;
    draw();
    if (NS.gh8 && NS.gh8.open) ghShow(true);
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
        if (['task', 'chats', 'chatmsg', 'envs', 'taskremoved'].includes(evt)) { drawStrip(); drawTabs8(); }
        if (evt === 'project' || evt === 'lang') proj.label();
        if (ghInst && ghInst.update) ghInst.update(evt, payload);
        if (envInst && envInst.update) envInst.update(evt, payload);
        if (evt === 'task' && payload && payload.chat_id && payload.swarm) {      // a chat that became a swarm switches its pane to the swarm screen
          for (const x of P.allPanes(state)) if (x.pane.chat === payload.chat_id) { const m = mounted.get(x.pane.id); if (m) m.sig = ''; }
          drawPanes();
        }
        forward(evt, payload);
      },
      destroy() {
        ws = null; closeMenu(); offExt(); proj.destroy(); if (envInst) envInst.destroy(); if (ghInst && ghInst.destroy) ghInst.destroy();
        if (cornerInst && cornerInst.destroy) cornerInst.destroy();
        for (const id of [...mounted.keys()]) destroyPane(id);
      },
    };
  }

  NS.registry.register({ id: 'tabs', layout: 'chats', mount });
  function focusPane(paneId) { setState(P.wsFocusPane(state, paneId)); focusActive(); }
  // v7: a pane of one tab goes beside the focused pane of the active tab (a card dropped on the terminal area)
  function besidePane(tabId, dir) {
    const from = P.tabOf(state, tabId), to = activeTab();
    if (!from || !to || from.id === to.id) return;
    if (P.count(to.root) >= P.MAX_PANES) { NS.shell.toast(t('pane.max')); return; }
    const p = P.findPane(from.root, from.focus);
    if (!p) return;
    p.moved = true;                                    // the terminal keeps running while its pane changes place
    let st = P.wsClosePane(P.wsActivate(state, from.id), p.id);
    st = P.wsActivate(st, to.id);
    st = P.wsSplit(st, dir === 'v' ? 'v' : 'h', p);
    setState(st);
    setTimeout(() => { delete p.moved; }, 0);
    focusActive();
  }
  NS.tabs = { show, setChat, chatView, viewSwitch, dropChat, focusPane, newTab, closeTab, closePane, placeOf, putBack, split, zoom, focusDir, equalize, besidePane, save: persist, freshPane, filterChanged, visTabs,
    openPlus: (a) => ws && ws.openPlus && ws.openPlus(a),
    get state() { return state; }, get active() { return focused(); }, get activeTab() { return activeTab(); } };
})();
