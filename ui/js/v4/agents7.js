// v7 agents column (просьба 25.10): every agent is a terminal running an env CLI; tabs.js draws them as cards on the left,
// term.js fills each card with what its terminal says and its state. Here: project groups, "ждёт тебя" (sound, system
// notification, ⌘J to the next one), the launcher "Новый агент", the spend on the card, agents and output in ⌘K, the
// "Изменения" panel of an agent (diff, revert a file, merge its own copy). Pure part (NSAgents) runs in selftest.
(function (root) {
  // ---------- pure ----------
  // tabs -> groups by key (the order of groups = where each key first appears, tabs keep their order inside)
  function groupTabs(tabs, keyOf) {
    const map = new Map();
    for (const tb of tabs) { const k = keyOf(tb) || ''; if (!map.has(k)) map.set(k, []); map.get(k).push(tb); }
    const groups = [...map].map(([key, list]) => ({ key, tabs: list }));
    return { groups, order: groups.flatMap((g) => g.tabs.map((x) => x.id)) };
  }
  // the project of a folder: the longest project path that is the folder or holds it
  function projectOfPath(cwd, projects) {
    const c = String(cwd || '').replace(/\/+$/, '');
    let best = null;
    for (const p of projects || []) { const pp = String(p.path || '').replace(/\/+$/, ''); if (pp && (c === pp || c.startsWith(pp + '/')) && (!best || pp.length > best.path.length)) best = { id: p.id, path: pp }; }
    return best ? best.id : '';
  }
  // 1234 -> "1.2k", 2500000 -> "2.5M"; the card shows tokens only when the CLI's own files gave them
  const fmtTok = (n) => (n >= 1e6 ? (n / 1e6).toFixed(1).replace(/\.0$/, '') + 'M' : n >= 1e3 ? (n / 1e3).toFixed(1).replace(/\.0$/, '') + 'k' : String(n));
  function useText(u, t) {
    if (!u || (u.tokens_in == null && u.tokens_out == null && u.usd == null)) return '';
    const tok = (u.tokens_in || 0) + (u.tokens_out || 0);
    return [tok ? t('agents.tok', { n: fmtTok(tok) }) : null, u.usd != null ? '$' + u.usd.toFixed(2) : null].filter(Boolean).join(' · ');
  }
  // the next waiting pane after the focused one, in the order of the cards (wraps around); null = nobody waits
  function nextWaiting(order, waiting, current) {
    if (!waiting.length) return null;
    const i = order.indexOf(current);
    for (let k = 1; k <= order.length; k++) { const id = order[(i + k + order.length) % order.length]; if (waiting.includes(id)) return id; }
    return waiting[0];
  }
  // a unified diff -> lines with a kind for colour (no HTML: the window draws text nodes)
  const diffLines = (text) => String(text || '').split('\n').map((x) => ({ text: x, kind: /^@@/.test(x) ? 'hunk' : /^(\+\+\+|---|diff |index )/.test(x) ? 'meta' : /^\+/.test(x) ? 'add' : /^-/.test(x) ? 'del' : 'ctx' }));
  const TEMPLATES = ['fix', 'tests', 'review', 'explain'];
  root.NSAgents = { groupTabs, projectOfPath, fmtTok, useText, nextWaiting, diffLines, TEMPLATES };
  if (typeof document === 'undefined' || !root.NS || !root.NS.h) return;

  // ---------- the window ----------
  const NS = root.NS;
  const { h, ic, envIcon, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const S = () => NS.store.s;
  const P = () => root.NSPanes;
  const call = (cmd, args) => NS.api.call(cmd, args);
  const toast = (m, bad) => NS.shell && NS.shell.toast(m, bad);
  const errText = (e) => (NS.errHuman ? NS.errHuman(NS.api.errText(e)) : NS.api.errText(e));
  const mac = () => /Mac/i.test(navigator.platform || '') || /[?&]platform=mac/.test(location.search);
  const views = () => (NS.term && NS.term.views ? [...NS.term.views.values()] : []);
  const SET_KEY = 'ns.agents7';
  const prefs = () => { try { return Object.assign({ sound: true, notify: true }, JSON.parse(NS.storage.get(SET_KEY) || '{}')); } catch (e) { return { sound: true, notify: true }; } };
  const setPref = (k, v) => NS.storage.set(SET_KEY, JSON.stringify(Object.assign(prefs(), { [k]: v })));

  // ---------- groups ----------
  function projectOf(pane) {
    if (!pane) return '';
    if (pane.project) return pane.project;
    if (pane.kind === 'agent' && pane.chat && pane.chat !== 'new') { const c = NS.store.findChat(pane.chat); if (c && c.project_id) return c.project_id; }
    return projectOfPath(pane.cwd, S().projects);
  }
  function groups(tabs, lead) { return groupTabs(tabs, (tb) => projectOf(lead(tb))); }
  let hueSeq = new Map();
  function groupHead(g) {
    const p = g.key ? NS.store.project(g.key) : null;
    if (!hueSeq.has(g.key)) hueSeq.set(g.key, hueSeq.size % 6);
    return h('div', { class: 'grp7', role: 'presentation', dataset: { hue: String(hueSeq.get(g.key)) } },
      h('i', { class: 'grp7-dot', 'aria-hidden': 'true' }), h('span', { class: 'grp7-n', text: p ? p.name : t('agents.no_project') }), h('small', { text: String(g.tabs.length) }));
  }

  // ---------- "ждёт тебя": the button, ⌘J, the call (sound + system notification) ----------
  const waiting = () => views().filter((v) => v.state() === 'wait');
  let waitEl = null;
  function waitButton() {
    waitEl = h('button', { type: 'button', class: 'navb nb-wait', hidden: true, onclick: jumpNext });
    drawWait();
    return waitEl;
  }
  function drawWait() {
    if (!waitEl) return;
    const n = waiting().length;
    waitEl.hidden = !n;
    if (n) clear(waitEl).append(h('i', { class: 'nb-wait-dot', 'aria-hidden': 'true' }), h('span', { class: 'nav-l', text: t('agents.waiting', { n }) }), h('kbd', { text: mac() ? '⌘J' : 'Ctrl+J' }));
    waitEl.title = t('agents.wait_jump');
  }
  function jumpNext() {
    if (!NS.tabs) return;
    const st = NS.tabs.state, order = P().allPanes(st).map((x) => x.pane.id);
    const id = nextWaiting(order, waiting().map((v) => v.pane.id), NS.tabs.active && NS.tabs.active.id);
    if (!id) { toast(t('agents.nobody_waits')); return; }
    if (NS.shell && NS.shell.current && NS.shell.current.def.id !== 'tabs' && NS.shell.back) NS.shell.back();
    NS.tabs.focusPane(id);
  }
  function changed() {
    drawWait();
    if (NS.shell && NS.shell.barUpdate) NS.shell.barUpdate('task');      // the env module of the bar redraws and sets the title count
  }
  // a soft two-note chime made on the spot (no sound files); the audio context opens on the first click (autoplay rules)
  let ac = null;
  root.addEventListener('pointerdown', () => { if (!ac && (root.AudioContext || root.webkitAudioContext)) try { ac = new (root.AudioContext || root.webkitAudioContext)(); } catch (e) { ac = null; } }, { once: true, capture: true });
  function chime() {
    if (!ac) return;
    const now = ac.currentTime;
    [[784, 0], [1175, 0.11]].forEach(([f, at]) => {
      const o = ac.createOscillator(), g = ac.createGain();
      o.type = 'sine'; o.frequency.value = f;
      g.gain.setValueAtTime(0.0001, now + at); g.gain.exponentialRampToValueAtTime(0.07, now + at + 0.02); g.gain.exponentialRampToValueAtTime(0.0001, now + at + 0.42);
      o.connect(g).connect(ac.destination); o.start(now + at); o.stop(now + at + 0.45);
    });
  }
  function calling(view) {
    const pr = prefs(), env = view.env();
    const name = (env && env.name) || t('term.kind');
    const body = view.tail(1)[0] || t('agents.wait_d');
    if (pr.sound) chime();
    if (pr.notify && !document.hasFocus()) {
      call('notify_agent', { title: t('agents.wait_t', { name }), body, pane: view.pane.id }).catch(() => {
        try { if (root.Notification && root.Notification.permission === 'granted') new root.Notification(t('agents.wait_t', { name }), { body }); } catch (e) { /* none */ }
      });
    }
    changed();
  }
  NS.api.listen('agent-notify-click', (p) => { if (p && p.pane && NS.tabs) { if (NS.shell && NS.shell.back && NS.shell.current && NS.shell.current.def.id !== 'tabs') NS.shell.back(); NS.tabs.focusPane(p.pane); } }).catch(() => {});
  root.addEventListener('keydown', (e) => {
    if (e.defaultPrevented || e.isComposing || e.altKey || e.code !== 'KeyJ') return;
    const inTerm = !!(e.target && e.target.classList && e.target.classList.contains('xterm-helper-textarea'));
    // ⌘J; on Linux Ctrl+J is a newline in a shell, so inside a terminal it is Ctrl+Shift+J
    if (mac() ? e.metaKey && !e.shiftKey : e.ctrlKey && (e.shiftKey || !inTerm)) { e.preventDefault(); e.stopPropagation(); jumpNext(); }
  }, true);

  // ---------- the spend on each card: tokens (and $ when the CLI wrote it) from the CLI's own files ----------
  const usage = new Map();          // pane id -> {tokens_in, tokens_out, usd}
  const USAGE_ENVS = ['claude', 'codex'];
  async function pollUsage() {
    if (!NS.tabs || document.hidden) return;
    for (const { pane } of P().allPanes(NS.tabs.state)) {
      if (pane.kind !== 'term' || !USAGE_ENVS.includes(pane.env) || !pane.cwd || !pane.started) continue;
      try { usage.set(pane.id, await call('term_usage', { cwd: pane.cwd, env: pane.env, sinceMs: pane.started })); } catch (e) { usage.delete(pane.id); }
    }
    drawUsage();
  }
  function drawUsage() {
    if (!NS.tabs) return;
    for (const el of document.querySelectorAll('.tab[data-tab]')) {
      const tb = NS.tabs.state.tabs.find((x) => x.id === el.dataset.tab), p = tb && P().findPane(tb.root, tb.focus);
      const slot = el.querySelector('.tab-use'), txt = p ? useText(usage.get(p.id), t) : '';
      if (slot && slot.textContent !== txt) { slot.textContent = txt; slot.title = txt ? t('agents.use_d') : ''; }
    }
  }
  setInterval(pollUsage, 20000);
  root.addEventListener('ns-ready', () => setTimeout(pollUsage, 3000));

  // ---------- "Новый агент": env, model, where, the task ----------
  let pop = null;
  const LKEY = 'ns.launch7';
  const closePop = () => { if (pop) { pop.remove(); pop = null; document.removeEventListener('mousedown', onDocPop, true); } };
  const onDocPop = (e) => { if (pop && !pop.contains(e.target)) closePop(); };
  async function launcher(anchor) {
    if (pop) { closePop(); return; }
    const last = (() => { try { return JSON.parse(NS.storage.get(LKEY) || '{}'); } catch (e) { return {}; } })();
    const envs = S().envs.filter((e) => e.state === 'ok' && !e.custom);
    const focusEnv = NS.tabs && NS.tabs.active && NS.tabs.active.env;
    let envId = (envs.find((e) => e.id === (focusEnv || last.env)) || envs[0] || {}).id;
    let own = !!last.own;
    let proj = NS.store.project(projectOf(NS.tabs && NS.tabs.active) || NS.projects.current()) || null;
    const projSel = S().projects.length > 1 ? h('select', { class: 'l7-model l7-proj', 'aria-label': t('agents.l_project'), onchange: (e) => { proj = NS.store.project(e.target.value) || null; drawWhere(); } },
      h('option', { value: '', text: t('agents.no_project') }), ...S().projects.map((p) => h('option', { value: p.id, text: p.name, selected: !!proj && p.id === proj.id }))) : null;
    const envRow = h('div', { class: 'l7-envs', role: 'radiogroup', 'aria-label': t('agents.l_env') });
    const model = h('select', { class: 'l7-model', 'aria-label': t('agents.l_model') });
    const task = h('textarea', root.NSPalette.quiet({ class: 'l7-task', rows: 3, placeholder: t('agents.l_task_ph'), 'aria-label': t('agents.l_task') }));
    const where = h('div', { class: 'l7-where', role: 'radiogroup', 'aria-label': t('agents.l_where') });
    const tpl = h('div', { class: 'l7-tpl' }, TEMPLATES.map((k) => h('button', { type: 'button', class: 'chip7', text: t('agents.tpl_' + k), onclick: () => { task.value = t('agents.tpl_' + k + '_p'); task.focus(); } })));
    const go = h('button', { type: 'button', class: 'btn primary l7-go', onclick: () => run() }, h('span', { text: t('agents.l_go') }), h('kbd', { text: mac() ? '⌘↵' : 'Ctrl+↵' }));
    function drawEnvs() {
      clear(envRow).append(...envs.map((e) => h('button', { type: 'button', role: 'radio', 'aria-checked': String(e.id === envId), class: 'l7-env' + (e.id === envId ? ' on' : ''), dataset: { env: e.id },
        onclick: () => { envId = e.id; drawEnvs(); drawModels(); } }, envIcon(e.id), h('span', { text: e.name }))));
      if (!envs.length) envRow.append(h('p', { class: 'mute', text: t('agents.l_no_env') }));
    }
    function drawModels() {
      if (!envId) return;
      const cat = S().catalogs.get(envId);
      if (!cat) { NS.store.loadCatalog(envId).then(() => pop && drawModels()).catch(() => {}); }
      const fams = ((cat && cat.families) || []).filter((f) => f.id !== NS.models.CLI_DEFAULT);
      const cur = NS.store.envSettings(envId).default_model, curFam = cur && cur.family;
      clear(model).append(h('option', { value: '', text: t('model.cli_default') }), ...fams.map((f) => h('option', { value: f.id, text: f.label, selected: f.id === curFam })));
    }
    function drawWhere() {
      const opt = (val, label, desc, off) => h('button', { type: 'button', role: 'radio', 'aria-checked': String(own === val), class: 'l7-w' + (own === val ? ' on' : ''), disabled: !!off, title: desc,
        onclick: () => { own = val; drawWhere(); } }, h('b', { text: label }), h('small', { text: desc }));
      clear(where).append(opt(false, t('agents.l_here', { name: proj ? proj.name : '~' }), t('agents.l_here_d')),
        opt(true, t('agents.l_own'), proj ? t('agents.l_own_d') : t('agents.l_own_off'), !proj));
    }
    async function run() {
      if (!envId) return;
      const env = NS.store.env(envId), cat = S().catalogs.get(envId);
      const fam = model.value, mArg = fam ? root.NSTerm.modelArg({ family: fam, version: null }, cat, envId) : null;
      const prompt = task.value.trim();
      NS.storage.set(LKEY, JSON.stringify({ env: envId, own }));
      const extra = { model: mArg, prompt: prompt || undefined, title: prompt ? prompt.split('\n')[0].slice(0, 40) : env.name, cwd: proj ? proj.path : undefined };
      if (own && proj) {
        go.disabled = true;
        try { const w = await call('agent_worktree', { projectId: proj.id, name: prompt || env.name }); extra.cwd = w.path; extra.branch = w.branch; }
        catch (e) { go.disabled = false; toast(errText(e), true); return; }
      }
      closePop();
      const f = NS.term.envFields(envId, extra);
      if (proj) f.project = proj.id;
      NS.term.open(f);
    }
    const more = (icon, label, fn) => h('button', { type: 'button', class: 'l7-more-b', onclick: () => { closePop(); fn(); } }, icon, h('span', { text: label }));
    pop = h('div', { class: 'launch7', role: 'dialog', 'aria-label': t('agents.new'),
      onkeydown: (e) => {
        if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); closePop(); anchor.focus(); }
        else if (e.key === 'Enter' && (mac() ? e.metaKey : e.ctrlKey)) { e.preventDefault(); run(); }
      } },
    h('div', { class: 'l7-h' }, h('b', { text: t('agents.new') }), h('small', { text: t('agents.new_d') })),
    h('label', { class: 'l7-lbl', text: t('agents.l_env') }), envRow,
    h('label', { class: 'l7-lbl', text: t('agents.l_model') }), model,
    projSel ? h('label', { class: 'l7-lbl', text: t('agents.l_project') }) : null, projSel,
    h('label', { class: 'l7-lbl', text: t('agents.l_where') }), where,
    h('label', { class: 'l7-lbl', text: t('agents.l_task') }), task, tpl,
    h('div', { class: 'l7-foot' }, go),
    h('div', { class: 'l7-more' },
      more(ic('i-chat'), t('agents.m_chat'), () => NS.tabs.newTab('chat')),
      more(h('span', { class: 't-glyph', text: '>_' }), t('agents.m_shell'), () => NS.term.open()),
      more(ic('i-swarm'), t('agents.m_swarm'), () => swarmChat()),
      more(ic('i-fact'), t('nav.factory'), () => NS.shell.nav('factory', { env: S().lastEnv }))));
    drawEnvs(); drawModels(); drawWhere();
    const r = anchor.getBoundingClientRect();
    NS.setVars(pop, { '--x': r.left + 'px', '--y': (r.bottom + 8) + 'px' });
    document.body.append(pop);
    setTimeout(() => document.addEventListener('mousedown', onDocPop, true), 0);
    task.focus();
  }
  // "Рой": a chat of the env in focus with the swarm switched on for the next task
  async function swarmChat() {
    const env = (NS.tabs && NS.tabs.active && NS.tabs.active.env) || S().lastEnv;
    if (!env) return;
    const st = NS.store.envSettings(env);
    if (!st.last_swarm) await NS.store.saveEnvSettings(env, { last_swarm: { mode: 'race', n: 3 } }).catch(() => {});
    NS.tabs.newTab('chat');
  }

  // ---------- ⌘K: agents, their output, the switches that left the screen ----------
  const stWord = (s) => t('term.st_' + s);
  NS.registry.extend('paletteSource', { id: 'agents',
    items() {
      if (!NS.tabs) return [];
      return P().allPanes(NS.tabs.state).filter((x) => x.pane.kind === 'term').map(({ pane }) => {
        const v = NS.term.views.get(pane.id), env = pane.env && NS.store.env(pane.env);
        return { id: 'agent.' + pane.id, label: t('agents.p_go', { name: pane.ptitle || pane.title || (env && env.name) || t('term.kind') }), keywords: [env && env.name, pane.cwd, pane.branch].filter(Boolean).join(' '),
          badge: v ? stWord(v.state()) : null, def: { run: () => NS.tabs.focusPane(pane.id) } };
      });
    },
    search(q) {
      if (String(q).trim().length < 3) return [];
      return views().flatMap((v) => v.search(q.trim()).slice(0, 3).map((hit, i) => ({ id: 'out.' + v.pane.id + '.' + i, label: '«' + hit.text + '»', badge: v.pane.ptitle || v.pane.title || t('term.kind'),
        def: { run: () => { NS.tabs.focusPane(v.pane.id); setTimeout(() => v.reveal(q.trim()), 60); } } }))).slice(0, 12);
    } });
  const cmd = (c) => NS.registry.extend('command', c);
  const curEnv = () => (NS.tabs && NS.tabs.active && NS.tabs.active.env) || S().lastEnv;
  cmd({ id: 'agents.new', labelKey: 'agents.new', group: 'cmd.g.agents', order: 1, run: () => { const b = document.querySelector('.rail7-new'); if (b) launcher(b); } });
  cmd({ id: 'agents.next', labelKey: 'agents.wait_jump', group: 'cmd.g.agents', keys: 'Cmd+J', order: 2, run: jumpNext });
  cmd({ id: 'agents.chat', labelKey: 'agents.m_chat', group: 'cmd.g.agents', order: 3, run: () => NS.tabs.newTab('chat') });
  cmd({ id: 'agents.swarm', labelKey: 'agents.m_swarm', group: 'cmd.g.agents', order: 4, run: swarmChat });
  cmd({ id: 'agents.factory', labelKey: 'nav.factory', group: 'cmd.g.agents', order: 5, run: () => NS.shell.nav('factory', { env: curEnv() }) });
  cmd({ id: 'agents.quality', group: 'cmd.g.agents', order: 6, get label() { const e = curEnv(), q = e && NS.store.envSettings(e).last_quality; return t(q && q.on === false ? 'agents.q_on' : 'agents.q_off'); },
    when: () => !!curEnv(), run: () => { const e = curEnv(), q = NS.store.envSettings(e).last_quality || { on: true }; NS.store.saveEnvSettings(e, { last_quality: Object.assign({}, q, { on: q.on === false }) }).then(() => toast(t(q.on === false ? 'agents.q_now_on' : 'agents.q_now_off'))).catch((er) => toast(errText(er), true)); } });
  cmd({ id: 'agents.sound', group: 'cmd.g.agents', order: 7, get label() { return t(prefs().sound ? 'agents.sound_off' : 'agents.sound_on'); }, run: () => { setPref('sound', !prefs().sound); if (prefs().sound) chime(); } });
  cmd({ id: 'agents.notify', group: 'cmd.g.agents', order: 8, get label() { return t(prefs().notify ? 'agents.notify_off' : 'agents.notify_on'); }, run: () => setPref('notify', !prefs().notify) });
  if (NS.auto && NS.auto.open) cmd({ id: 'agents.auto', labelKey: 'agents.m_auto', group: 'cmd.g.agents', order: 9, run: () => NS.auto.open() });

  // ---------- "Изменения" of an agent: what it changed in its folder, a file's diff, revert, merge its own copy ----------
  function changesPanel(pane, anchorPane) {
    const old = anchorPane.querySelector('.chg7');
    if (old) { old.remove(); return; }
    const list = h('div', { class: 'chg7-list' }), diff = h('div', { class: 'chg7-diff' }), head = h('div', { class: 'chg7-h' });
    const panel = h('aside', { class: 'chg7', 'aria-label': t('agents.chg') }, head, list, diff);
    anchorPane.append(panel);
    async function load() {
      clear(list).append(h('p', { class: 'mute', text: t('agents.chg_loading') }));
      let c;
      try { c = await call('agent_changes', { cwd: pane.cwd }); } catch (e) { clear(list).append(h('p', { class: 'bad', text: errText(e) })); return; }
      NS.append(clear(head), [h('b', { text: t('agents.chg') }), h('small', { class: 'mute', text: c.branch ? c.branch + (c.base ? ' → ' + c.base : '') : '' }),
        c.own_worktree && c.files.length ? h('button', { type: 'button', class: 'btn primary sm', text: t('agents.merge', { base: c.base || 'main' }), onclick: () => merge(c) }) : null,
        h('button', { type: 'button', class: 'pt', 'aria-label': t('term.find_close'), text: '×', onclick: () => panel.remove() })]);
      if (!c.repo) { clear(list).append(h('p', { class: 'mute', text: t('agents.chg_norepo') })); return; }
      if (!c.files.length) { clear(list).append(h('p', { class: 'mute', text: t('agents.chg_none') })); clear(diff); return; }
      // the path is cut from the left (direction: rtl); the marks keep a leading «.» of `.gitattributes` in front
      clear(list).append(...c.files.map((f) => h('div', { class: 'chg7-f', role: 'button', tabIndex: 0, dataset: { st: f.status }, onclick: () => show(f), onkeydown: (e) => { if (e.key === 'Enter') show(f); } },
        h('i', { class: 'chg7-s', text: f.status }), h('span', { class: 'chg7-p', text: '\u200e' + f.path + '\u200e', title: f.path }),
        h('small', { class: 'chg7-n' }, f.add ? h('span', { class: 'add', text: '+' + f.add }) : null, f.del ? h('span', { class: 'del', text: '−' + f.del }) : null),
        h('button', { type: 'button', class: 'pt', title: t('agents.revert'), 'aria-label': t('agents.revert') + ' ' + f.path, text: '↺', onclick: (e) => { e.stopPropagation(); revert(f); } }))));
      show(c.files[0]);
    }
    async function show(f) {
      for (const el of list.querySelectorAll('.chg7-f')) el.classList.toggle('on', el.querySelector('.chg7-p').textContent === f.path);
      clear(diff).append(h('p', { class: 'mute', text: t('agents.chg_loading') }));
      try {
        const d = await call('agent_file_diff', { cwd: pane.cwd, file: f.path });
        const text = typeof d === 'string' ? d : d.text;
        NS.append(clear(diff), [h('pre', { class: 'chg7-pre' }, diffLines(text).map((l) => h('span', { class: 'dl ' + l.kind, text: l.text + '\n' }))), d && d.truncated ? h('p', { class: 'mute', text: t('agents.chg_cut') }) : null]);
      } catch (e) { clear(diff).append(h('p', { class: 'bad', text: errText(e) })); }
    }
    async function revert(f) {
      if (!(await NS.projects.confirm(t('agents.revert_q', { file: f.path }), f.status === '?' ? t('agents.revert_new') : t('agents.revert_d'), t('agents.revert')))) return;
      try { await call('agent_revert_file', { cwd: pane.cwd, file: f.path }); toast(t('agents.reverted', { file: f.path })); load(); } catch (e) { toast(errText(e), true); }
    }
    async function merge(c) {
      if (!(await NS.projects.confirm(t('agents.merge_q', { base: c.base || 'main' }), t('agents.merge_d', { n: c.files.length }), t('agents.merge', { base: c.base || 'main' })))) return;
      try { const r = await call('agent_merge', { cwd: pane.cwd, message: pane.title || 'agent work' }); toast(r && r.merged ? t('agents.merged', { base: c.base || 'main' }) : t('agents.not_merged')); load(); } catch (e) { toast(errText(e), true); }
    }
    load();
  }
  NS.registry.extend('paneHeader', { id: 'agent7', kinds: ['term'], order: 40, render(el, pane) {
    if (!pane.cwd) return;
    const chat = pane.env ? h('button', { type: 'button', class: 'pt pt-l', title: t('agents.as_chat_d'), onclick: (e) => { e.stopPropagation(); NS.tabs.focusPane(pane.id); NS.tabs.split('h', 'chat'); } }, ic('i-chat'), h('span', { text: t('agents.as_chat') })) : null;
    const chg = h('button', { type: 'button', class: 'pt pt-l', title: t('agents.chg_d'), onclick: (e) => { e.stopPropagation(); changesPanel(pane, el.closest('.pane')); } }, ic('i-branch'), h('span', { text: t('agents.chg') }));
    el.append(...[chat, chg].filter(Boolean));
  } });

  NS.agents = { waitingCount: () => waiting().length, groups, groupHead, changed, calling, waitButton, launcher, jumpNext, projectOf, chime, useOf: (paneId) => useText(usage.get(paneId), t) };
})(typeof window !== 'undefined' ? window : globalThis);
