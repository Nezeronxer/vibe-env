// v8 (Agentation note 22:08): the work modes in the status bar - «Рой», «Качество», «Фабрика» as switches of the agent in focus,
// and a gear beside them with their settings (swarm mode and size, the quality switch, the factory: open, freeze). The same
// settings the composer and the factory screen use (envSettings.last_swarm / last_quality, settings.swarm_default, freeze).
(function (root) {
  const NS = root.NS;
  if (!NS || !NS.registry) return;
  const { h, ic, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const S = () => NS.store.s;
  const L = root.NSLogic;
  const QUALITY0 = { on: true, spec: true, plan: true, tests_first: true, e2e: false, best_of_n: 2 };
  const MODES = ['race', 'crew', 'council'];
  const envNow = () => (NS.envrail && NS.envrail.filter) || S().route.env || S().lastEnv || ((S().envs.find((e) => e.state === 'ok') || {}).id);
  const fail = (e) => NS.shell.toast(NS.errHuman ? NS.errHuman(NS.api.errText(e)) : NS.api.errText(e), true);

  const swarmOf = (env) => (env ? NS.store.envSettings(env).last_swarm || null : null);
  const qualityOf = (env) => Object.assign({}, QUALITY0, (env && NS.store.envSettings(env).last_quality) || {});
  const swarmDefault = () => Object.assign({ mode: 'race', n: 3 }, (S().settings || {}).swarm_default || {});
  async function setSwarm(env, spec) { await NS.store.saveEnvSettings(env, { last_swarm: spec }); }
  async function setQuality(env, patch) { await NS.store.saveEnvSettings(env, { last_quality: Object.assign(qualityOf(env), patch) }); }
  async function setFrozen(on) { await NS.api.call('freeze', { on }); S().frozen = on; NS.store.emit('frozen', on); }

  let pop = null;
  const closePop = () => { if (pop) { pop.remove(); pop = null; document.removeEventListener('mousedown', onDoc, true); document.removeEventListener('keydown', onKey, true); } };
  const onDoc = (e) => { if (pop && !pop.contains(e.target) && !e.target.closest('.md8-gear')) closePop(); };
  const onKey = (e) => { if (e.key === 'Escape') { e.stopPropagation(); closePop(); } };

  // the gear: one popover, three sections; every change saves at once and redraws the bar
  function openPop(anchor, redraw) {
    if (pop) { closePop(); return; }
    const env = envNow(), e = NS.store.env(env);
    if (!e) return;
    const draw = () => {
      const sw = swarmOf(env), q = qualityOf(env), d = sw || swarmDefault();
      const seg = MODES.map((m) => h('button', { type: 'button', class: 'md8-seg' + (d.mode === m ? ' on' : ''), title: t('swarm.' + m + '_d'), 'aria-pressed': String(d.mode === m),
        onclick: () => setSwarm(env, Object.assign({}, d, { mode: m })).then(after).catch(fail) }, t('swarm.' + m)));
      const n = Math.max(L.SWARM_MIN || 2, Math.min(L.SWARM_MAX || 5, d.n || 3));
      const step = (k) => h('button', { type: 'button', class: 'md8-step', 'aria-label': k > 0 ? '+1' : '-1', text: k > 0 ? '+' : '−', disabled: (k > 0 ? n >= (L.SWARM_MAX || 5) : n <= (L.SWARM_MIN || 2)),
        onclick: () => setSwarm(env, Object.assign({}, d, { n: n + k })).then(after).catch(fail) });
      clear(pop).append(
        h('div', { class: 'md8-h' }, h('b', { text: t('md8.title') }), h('small', { text: e.name })),
        sect(t('md8.swarm'), t('md8.swarm_d'), !!sw, (on) => setSwarm(env, on ? d : null),
          h('div', { class: 'md8-row' }, h('div', { class: 'md8-segs', role: 'group', 'aria-label': t('md8.mode') }, seg)),
          h('div', { class: 'md8-row' }, h('span', { class: 'md8-l', text: t('md8.agents') }), h('div', { class: 'md8-stepper' }, step(-1), h('b', { text: String(n) }), step(1)))),
        sect(t('md8.quality'), t('md8.quality_d'), q.on !== false, (on) => setQuality(env, { on }),
          ...['spec', 'plan', 'tests_first', 'e2e'].map((k) => h('label', { class: 'md8-chk' }, h('input', { type: 'checkbox', checked: !!q[k], disabled: q.on === false, onchange: (ev) => setQuality(env, { [k]: ev.target.checked }).then(after).catch(fail) }), h('span', { text: t('md8.q_' + k) })))),
        sect(t('nav.factory'), t('md8.factory_d'), !S().frozen, (on) => setFrozen(!on),
          h('div', { class: 'md8-row' }, h('button', { type: 'button', class: 'btn sm', text: t('md8.open_factory'), onclick: () => { closePop(); NS.shell.nav('factory', { env }); } }))));
    };
    function sect(title, desc, on, toggle, ...body) {
      const sw = h('button', { type: 'button', class: 'md8-sw' + (on ? ' on' : ''), role: 'switch', 'aria-checked': String(on), 'aria-label': title,
        onclick: () => toggle(!on).then(after).catch(fail) }, h('i'));
      return h('section', { class: 'md8-sec' + (on ? ' on' : '') }, h('div', { class: 'md8-sh' }, h('div', {}, h('b', { text: title }), h('small', { text: desc })), sw), on ? body : null);
    }
    const after = () => { draw(); redraw(); };
    pop = h('div', { class: 'md8-pop', role: 'dialog', 'aria-label': t('md8.title') });
    draw();
    const r = anchor.getBoundingClientRect();
    NS.setVars(pop, { '--x': Math.max(8, Math.min(window.innerWidth - 328, r.right - 320)) + 'px', '--yb': (window.innerHeight - r.top + 8) + 'px' });
    document.body.append(pop);
    setTimeout(() => { document.addEventListener('mousedown', onDoc, true); document.addEventListener('keydown', onKey, true); }, 0);
  }

  NS.registry.extend('bar', { id: 'modes', side: 'center', order: 3, render(el) {
    const draw = () => {
      const env = envNow();
      clear(el);
      if (!env || !NS.store.env(env)) return;
      const sw = swarmOf(env), q = qualityOf(env), frozen = !!S().frozen;
      const pill = (cls, icon, label, on, title, fn) => h('button', { type: 'button', class: 'bm md8-p ' + cls + (on ? ' on' : ''), 'aria-pressed': String(on), title, onclick: fn }, h('i', { class: 'md8-dot', 'aria-hidden': 'true' }), icon, h('span', { text: label }));
      el.append(h('div', { class: 'md8', role: 'group', 'aria-label': t('md8.title') },
        pill('md8-swarm', ic('i-swarm'), sw ? t('md8.swarm_n', { n: sw.n || 3 }) : t('md8.swarm'), !!sw, t('md8.swarm_d'), () => setSwarm(env, sw ? null : swarmDefault()).then(draw).catch(fail)),
        pill('md8-q', ic('i-check'), t('md8.quality'), q.on !== false, t('md8.quality_d'), () => setQuality(env, { on: q.on === false }).then(draw).catch(fail)),
        pill('md8-fact', ic('i-fact'), t('nav.factory'), !frozen, frozen ? t('md8.frozen') : t('nav.factory_d'), () => NS.shell.nav('factory', { env })),
        h('button', { type: 'button', class: 'bm md8-gear', title: t('md8.settings'), 'aria-label': t('md8.settings'), 'aria-haspopup': 'dialog', onclick: (e) => openPop(e.currentTarget, draw) }, ic('i-gear'))));
    };
    draw();
    const off = NS.store.on('frozen', draw);
    return { update: (evt) => { if (['route', 'focus', 'settings', 'envs', 'lang'].includes(evt)) draw(); }, destroy: () => { closePop(); if (typeof off === 'function') off(); } };
  } });
})(typeof window !== 'undefined' ? window : globalThis);
