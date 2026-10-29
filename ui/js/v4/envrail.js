// v8 (Agentation notes 28.10.2026): the column of agents left of the sessions - a mark and a short name under it, «Все» on top.
// A click makes the agent current: the sessions and the tabs above the terminal show only its sessions, "+" opens it.
// An agent whose CLI is not found is dimmed under «Добавить»; its card installs it in a built-in terminal tab (install()).
(function (root) {
  // ---------- pure (selftest) ----------
  const SHORT = { codex: 'ChatGPT', claude: 'Claude', opencode: 'opencode', copilot: 'Copilot', droid: 'Droid', grok: 'Grok' };
  // "Claude Code" -> "Claude", "Gemini CLI" -> "Gemini", "Qwen Code" -> "Qwen"; Codex is what people call ChatGPT
  function shortName(env) {
    if (!env) return '';
    if (SHORT[env.id]) return SHORT[env.id];
    const n = String(env.name || env.id).replace(/\s+(CLI|Code)$/i, '').trim();
    return n.length > 10 ? n.split(/\s+/)[0] : n;
  }
  // the ones to work with, then the ones to add; a custom command is never "missing" (it has no installer)
  const missing = (e) => e.state === 'missing' && !e.custom;
  function railOrder(envs) { return { ok: envs.filter((e) => !missing(e)), add: envs.filter(missing) }; }
  // what the install button does: argv straight into a terminal, or a line typed into a shell for the person to check (a pipe)
  function installPlan(env) {
    const i = env && env.install;
    if (i && Array.isArray(i.argv) && i.argv.length && !i.piped) return { argv: i.argv.slice(), typed: null, line: i.line || i.argv.join(' ') };
    const hint = env && (env.hint || env.install_hint), line = (i && i.line) || (hint ? String(hint).split(' (')[0].trim() : '');
    return line ? { argv: [], typed: line, line } : null;
  }
  root.NSEnvRail = { shortName, railOrder, installPlan };
  const NS = root.NS;
  if (!NS || !NS.h) return;

  // ---------- the column ----------
  const { h, ic, envIcon, clear, setVars } = NS;
  const t = (k, p) => NS.t(k, p);
  const S = () => NS.store.s;
  const KEY = 'ns.env8';
  let filter = (() => { try { return NS.storage.get(KEY) || ''; } catch (e) { return ''; } })();
  let el = null, pop = null, sig = '', timer = 0;

  // who works and who waits, per env: the terminals know it (term.js views), the chats by their tasks
  function counts() {
    const out = new Map(), st = NS.tabs && NS.tabs.state, views = NS.term && NS.term.views;
    if (!st || !root.NSPanes) return out;
    for (const x of root.NSPanes.allPanes(st)) {
      const env = x.pane.env;
      if (!env) continue;
      const c = out.get(env) || { n: 0, run: 0, wait: 0 };
      c.n++;
      const v = views && views.get(x.pane.id), s = v ? v.state() : '';
      if (s === 'run') c.run++;
      if (s === 'wait') c.wait++;
      out.set(env, c);
    }
    return out;
  }
  function setFilter(id) {
    filter = id || '';
    try { NS.storage.set(KEY, filter); } catch (e) { /* the choice lives only in this window */ }
    if (filter) { S().lastEnv = filter; if (NS.applyTheme) NS.applyTheme(undefined, filter); }
    draw();
    if (NS.tabs && NS.tabs.filterChanged) NS.tabs.filterChanged();
    if (NS.shell && NS.shell.barUpdate) NS.shell.barUpdate('route');
  }
  function item(env, c, i) {
    const on = filter === env.id, miss = missing(env);
    const n = c ? c.wait || c.run : 0, mac = /Mac/i.test(navigator.platform || '');
    const tip = miss ? env.name + ' · ' + t('env8.missing') : t('env8.pick_d', { env: env.name }) + (i < 9 ? ' · ' + (mac ? '⌥⌘' : 'Ctrl+Alt+') + (i + 1) : '');
    return h('button', { type: 'button', class: 'er8-i' + (on ? ' on' : '') + (miss ? ' miss' : ''), role: miss ? null : 'tab', 'aria-selected': miss ? null : String(on), title: tip,
      dataset: { env: String(env.id).startsWith('custom') ? 'custom' : env.id }, onclick: (e) => (miss ? openCard(env, e.currentTarget) : setFilter(env.id)) },
    h('span', { class: 'er8-ic' }, envIcon(env.custom ? 'custom' : env.id)), h('span', { class: 'er8-n', text: shortName(env) }),
    n ? h('b', { class: 'er8-b' + (c.wait ? ' wait' : ' run'), title: c.wait ? t('env8.wait_n', { n: c.wait }) : t('env8.run_n', { n: c.run }), text: c.wait ? String(c.wait) : '' }) : null);
  }
  function draw() {
    if (!el) return;
    const envs = S().envs || [], { ok, add } = railOrder(envs), c = counts();
    if (filter && !envs.some((e) => e.id === filter && !missing(e))) filter = '';
    sig = sigOf(c);
    const all = h('button', { type: 'button', class: 'er8-i er8-all' + (filter ? '' : ' on'), role: 'tab', 'aria-selected': String(!filter), title: t('env8.all_d'), onclick: () => setFilter('') },
      h('span', { class: 'er8-ic' }, ic('i-all')), h('span', { class: 'er8-n', text: t('env8.all') }));
    clear(el).append(h('div', { class: 'er8-list', role: 'tablist', 'aria-orientation': 'vertical', 'aria-label': t('env8.rail') }, all, ok.map((e, i) => item(e, c.get(e.id), i))),
      add.length ? h('div', { class: 'er8-add' }, h('span', { class: 'er8-lbl', text: t('env8.get') }), add.map((e) => item(e, null, 99))) : null,
      // settings sit under the agents (Agentation note 29.10.2026): the gear is where people look for it
      h('div', { class: 'er8-foot' }, h('button', { type: 'button', class: 'er8-i er8-set', title: t('nav.settings_d'), 'aria-label': t('nav.settings'),
        onclick: () => NS.shell.nav('settings', { env: filter || S().lastEnv, section: 'basic' }) }, h('span', { class: 'er8-ic' }, ic('i-gear')), h('span', { class: 'er8-n', text: t('nav.settings') }))));
  }
  const sigOf = (c) => (S().envs || []).map((e) => e.id + e.state).join() + '|' + filter + '|' + [...c].map(([k, v]) => k + v.run + '.' + v.wait).join();
  // the terminals change state without an event the column hears: a cheap look twice a second, a redraw only on a change
  function watch() { clearInterval(timer); timer = setInterval(() => { if (!el || !el.isConnected) { clearInterval(timer); return; } if (!document.hidden && sigOf(counts()) !== sig) draw(); }, 500); }
  function mount(host) { el = host; el.classList.add('er8'); draw(); watch(); return { update: (evt) => { if (['envs', 'task', 'focus', 'lang'].includes(evt)) draw(); }, destroy: () => { clearInterval(timer); closeCard(); el = null; } }; }

  // ---------- the card of an agent to add ----------
  const closeCard = () => { if (pop) { pop.remove(); pop = null; document.removeEventListener('mousedown', onDoc, true); document.removeEventListener('keydown', onKey, true); } };
  const onDoc = (e) => { if (pop && !pop.contains(e.target)) closeCard(); };
  const onKey = (e) => { if (e.key === 'Escape') { e.stopPropagation(); closeCard(); } };
  function openCard(env, anchor) {
    if (pop && pop.dataset.env === env.id) { closeCard(); return; }
    closeCard();
    const plan = installPlan(env);
    pop = h('div', { class: 'er8-card', role: 'dialog', 'aria-label': env.name, dataset: { env: env.id } },
      h('div', { class: 'er8-ch' }, envIcon(env.id), h('b', { text: env.name }), h('small', { text: t('env8.missing') })),
      plan ? h('code', { class: 'er8-cmd', text: plan.line }) : h('p', { class: 'er8-d', text: t('env8.no_hint', { env: env.name }) }),
      plan ? h('p', { class: 'er8-d', text: t(plan.typed ? 'env8.piped_d' : 'env8.install_d') }) : null,
      h('div', { class: 'er8-acts' },
        plan ? h('button', { type: 'button', class: 'btn primary sm er8-go', text: t('env8.install'), onclick: () => { closeCard(); install(env); } }) : null,
        h('button', { type: 'button', class: 'btn sm', text: t('env8.settings'), onclick: () => { closeCard(); NS.shell.nav('settings', { env: env.id, section: 'basic' }); } })));
    const r = anchor.getBoundingClientRect();
    setVars(pop, { '--x': (r.right + 8) + 'px', '--y': Math.min(r.top, window.innerHeight - 220) + 'px' });
    document.body.append(pop);
    setTimeout(() => { document.addEventListener('mousedown', onDoc, true); document.addEventListener('keydown', onKey, true); }, 0);
    const go = pop.querySelector('.er8-go'); if (go) go.focus();
  }
  // the install runs where the person sees it: a terminal tab of the app; the env list is asked again when it is done (term.js)
  function install(env) {
    const plan = installPlan(env);
    if (!plan) { NS.shell.toast(t('env8.no_hint', { env: env.name }), true); return; }
    if (!NS.term || !NS.term.open) return;
    if (filter) setFilter('');          // the install tab belongs to no agent yet: show it under «Все»
    NS.term.open({ argv: plan.argv, typed: plan.typed || undefined, install: env.id, title: t('env8.installing', { env: env.name }), cwd: null });
  }
  // the terminal of an install is done (its process ended, or its shell is back at the prompt): ask the envs again
  async function installed(envId) {
    try { await NS.api.call('recheck_envs'); await NS.store.reloadEnvs(); } catch (e) { return false; }    // list_envs alone answers from its cache
    const e = NS.store.env(envId);
    if (!e || e.state === 'missing') return false;
    NS.shell.toast(t('env8.installed', { env: e.name }));
    setFilter(e.id);
    return true;
  }

  NS.envrail = { mount, setFilter, install, installed, get filter() { return filter; } };
})(typeof window !== 'undefined' ? window : globalThis);
