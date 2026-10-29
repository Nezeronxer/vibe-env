// v8 (Agentation note 22:07): the corner of the head shows the live mark of the agent in focus, like the agent marks of product
// films - it breathes when the agent is free, a light runs round it while it works, it pulses warm when it waits for the person.
// Claude keeps Clawd (clawd.js); every other agent gets its mark here. Registered after Clawd: the last 'tabsCorner' wins.
(function (root) {
  // ---------- pure (selftest) ----------
  // the state of an agent from the states of its terminals: someone waits > someone works > free
  function markState(states) { const xs = states || []; return xs.includes('wait') ? 'wait' : xs.includes('run') ? 'run' : 'idle'; }
  root.NSAgentMark = { markState };
  const NS = root.NS;
  if (!NS || !NS.registry || typeof document === 'undefined') return;
  const { h, envIcon, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const S = () => NS.store.s;
  const clawdDef = () => NS.registry.points('tabsCorner').find((d) => d.id === 'clawd');
  const envNow = () => (NS.envrail && NS.envrail.filter) || (NS.tabs && NS.tabs.active && NS.tabs.active.env) || S().lastEnv || '';
  function statesOf(env) {
    const st = NS.tabs && NS.tabs.state, views = NS.term && NS.term.views;
    if (!st || !root.NSPanes || !views) return [];
    return root.NSPanes.allPanes(st).filter((x) => x.pane.env === env).map((x) => { const v = views.get(x.pane.id); return v ? v.state() : ''; });
  }

  function mount(el, ctx) {
    let env = null, inner = null, timer = 0, mark = null;
    function draw() {
      const now = envNow();
      if (now === env && (now === 'claude' ? inner : mark)) return;
      env = now;
      if (inner && inner.destroy) inner.destroy();
      inner = null; mark = null;
      clear(el);
      const cd = clawdDef();
      if (env === 'claude' && cd) { inner = cd.mount(el, ctx) || null; return; }
      const e = NS.store.env(env);
      if (!e) return;
      mark = h('span', { class: 'am8', role: 'img', 'aria-label': e.name, dataset: { env: String(env).startsWith('custom') ? 'custom' : env, st: 'idle' } },
        h('span', { class: 'am8-ring', 'aria-hidden': 'true' }), h('span', { class: 'am8-core' }, envIcon(e.custom ? 'custom' : e.id)));
      el.append(mark);
    }
    // the terminals change state without an event the corner hears: a cheap look twice a second, a write only on a change
    function tick() {
      if (!el.isConnected) { clearInterval(timer); return; }
      if (document.hidden) return;
      draw();
      if (!mark) return;
      const s = markState(statesOf(env));
      if (mark.dataset.st !== s) { mark.dataset.st = s; mark.title = (NS.store.env(env) || {}).name + ' · ' + t('am8.' + s); }
    }
    draw(); tick();
    timer = setInterval(tick, 500);
    return { update: (evt, p) => { draw(); if (inner && inner.update) inner.update(evt, p); }, destroy: () => { clearInterval(timer); if (inner && inner.destroy) inner.destroy(); clear(el); } };
  }
  NS.registry.extend('tabsCorner', { id: 'agentmark', mount });
})(typeof window !== 'undefined' ? window : globalThis);
