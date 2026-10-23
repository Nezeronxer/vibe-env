// Settings: the frame (current env first, then "Для всех сред", then "Проекты", story 194) and the sections of the env and of all envs.
// Values come from the core (get_settings), the window keeps no table of defaults; a wrong input shows a hint under the field and is not applied (story 196);
// changes go to NEW runs only (story 197). Kits and projects live in kits.js / settings-projects.js and register their sections here.
(function () {
  const NS = window.NS;
  const { h, ic, envIcon, clear } = NS;
  const L = window.NSLogic;
  const t = (k, p) => NS.t(k, p);
  const st = NS.store, S = st.s;
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);

  // the OS decides the default keys; the mock can pretend with ?platform=linux
  const platform = () => (window.__MOCK__ && new URLSearchParams(location.search).get('platform')) || (/Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent || '') ? 'mac' : 'linux');
  NS.platform = platform;

  // ---------- reading and writing ----------
  // values come from the core (get_settings): the window keeps no table of defaults. An env that was never touched has no block yet:
  // only empty collections are filled in, and the "ask" switches read "on" unless the core says false (its default is on)
  const sget = (k) => (S.settings || {})[k] || {};
  const envSet = (envId) => Object.assign({ roles: {}, custom_models: [], extra_args: [] }, st.envSettings(envId));
  const on = (v) => v !== false;

  // the whole settings object goes to the core; when the core refuses, nothing is applied locally (story 196)
  let quietUntil = 0;
  const touch = () => { quietUntil = Date.now() + 1500; };       // events caused by our own save do not redraw the form under the cursor
  async function commit(patch) {
    touch();
    const next = Object.assign({}, S.settings, patch);
    await NS.api.call('set_settings', { settings: next });
    S.settings = next;
    applyLook();
  }
  function commitEnv(envId, patch) {
    const envs = Object.assign({}, (S.settings || {}).envs);
    envs[envId] = Object.assign({}, envs[envId], patch);
    return commit({ envs });
  }

  // "Внешний вид": theme, size, role labels take effect at once (they are the window's own)
  // v4: the whole look (theme, font, radius, gap, …) goes through theme.js; the extras of v4 live in settings.ui.tabs.look
  function applyLook() {
    const look = sget('look');
    if (window.NSTheme) window.NSTheme.applyTheme(window.NSTheme.fromSettings(S.settings));
    document.body.classList.toggle('no-role-labels', !look.role_labels);
  }
  window.addEventListener('ns-ready', applyLook);

  // ---------- controls ----------
  function row(title, desc, ...ctl) {
    const hint = h('p', { class: 'fhint', role: 'alert', hidden: true });
    const r = h('div', { class: 'srow' }, h('div', {}, h('b', { text: title }), desc ? h('span', { class: 'd', text: desc }) : null), h('div', { class: 'ctl' }, ctl), hint);
    r.hint = (txt) => { hint.hidden = !txt; hint.textContent = txt || ''; };
    return r;
  }
  const group = (title, rows, note) => h('div', { class: 'sgrp' }, title ? h('h2', {}, title, note ? h('span', { class: 'hint', text: note }) : null) : null, h('div', { class: 'srows' }, rows));
  const mark = (el, bad) => { if (bad) el.setAttribute('aria-invalid', 'true'); else el.removeAttribute('aria-invalid'); };
  const errHint = (e) => NS.errHuman(NS.api.errText(e));

  function sw(on, label, apply) {
    const b = h('button', { type: 'button', role: 'switch', class: 'tg', 'aria-checked': String(!!on), 'aria-label': label }, h('span', { class: 'sw' }, h('i')));
    b.onclick = async () => {
      const v = b.getAttribute('aria-checked') !== 'true';
      b.setAttribute('aria-checked', String(v));
      try { await apply(v); } catch (e) { b.setAttribute('aria-checked', String(!v)); fail(e); }
    };
    b.set = (v) => b.setAttribute('aria-checked', String(!!v));
    return b;
  }
  const toggleRow = (o) => { const b = sw(o.on, o.title, o.apply); const r = row(o.title, o.desc, o.extra || null, b); r.sw = b; return r; };
  const always = (title, desc) => row(title, desc, h('span', { class: 'pill-s ok' }, ic('i-check'), t('set.always')));

  function numRow(o) {
    const input = h('input', { type: 'text', class: 'num', inputMode: o.int ? 'numeric' : 'decimal', value: String(o.value), 'aria-label': o.title });
    const r = row(o.title, o.desc, o.prefix ? h('span', { class: 'unit', text: o.prefix }) : null, input, o.unit ? h('span', { class: 'unit', text: o.unit }) : null);
    input.addEventListener('change', async () => {
      const v = L.validateNumber(input.value, o.rule);
      if (!v.ok) { mark(input, true); r.hint(v.why === 'range' ? t('hint.range', { min: o.rule.min, max: o.rule.max }) : t('hint.nan')); return; }   // the typed text stays
      try { await o.apply(v.value); mark(input, false); r.hint(''); } catch (e) { mark(input, true); r.hint(errHint(e)); }
    });
    r.input = input;
    return r;
  }
  // two times "from - to" (quiet hours, night window)
  function timeRow(o) {
    const a = h('input', { type: 'text', class: 'time', value: o.from, 'aria-label': o.title + ' ' + t('set.from') }), b = h('input', { type: 'text', class: 'time', value: o.to, 'aria-label': o.title + ' ' + t('set.to') });
    const r = row(o.title, o.desc, a, h('span', { class: 'unit', text: t('set.to') }), b);
    const go = async () => {
      const x = L.validateTime(a.value), y = L.validateTime(b.value);
      mark(a, !x.ok); mark(b, !y.ok);
      if (!x.ok || !y.ok) { r.hint(t('hint.time')); return; }
      try { await o.apply(x.value, y.value); a.value = x.value; b.value = y.value; r.hint(''); } catch (e) { mark(a, true); r.hint(errHint(e)); }
    };
    a.addEventListener('change', go); b.addEventListener('change', go);
    return r;
  }
  // one text value; check(raw) -> {ok, value} | {ok: false, hint}
  function textRow(o) {
    const input = h('input', { type: 'text', class: 'wide', value: o.value || '', placeholder: o.placeholder || '', 'aria-label': o.title, spellcheck: false });
    const r = row(o.title, o.desc, input, o.extra || null);
    input.addEventListener('change', async () => {
      const v = o.check ? o.check(input.value) : { ok: true, value: input.value.trim() };
      if (!v.ok) { mark(input, true); r.hint(v.hint); return; }
      try { await o.apply(v.value); mark(input, false); r.hint(''); } catch (e) { mark(input, true); r.hint(errHint(e)); }
    });
    r.input = input;
    return r;
  }
  function selectRow(o) {
    const sel = h('select', { 'aria-label': o.title }, o.options.map((x) => h('option', { value: x.value, text: x.text, selected: x.value === o.value })));
    sel.onchange = async () => { try { await o.apply(sel.value); } catch (e) { sel.value = o.value; fail(e); } };
    return row(o.title, o.desc, o.before || null, sel, o.after || null);
  }
  function segment(opts, value, label, apply) {
    const box = h('div', { class: 'seg', role: 'radiogroup', 'aria-label': label });
    const draw = (v) => { clear(box).append(...opts.map(([val, text]) => h('button', { type: 'button', role: 'radio', class: v === val ? 'on' : '', 'aria-checked': String(v === val), text,
      onclick: async () => { const prev = v; draw(val); try { await apply(val); v = val; } catch (e) { draw(prev); fail(e); } } }))); };
    draw(value);
    return box;
  }
  function stepper(value, label, apply) {
    const out = h('output', { text: String(value) });
    let cur = value;
    const minus = h('button', { type: 'button', 'aria-label': t('tune.less'), text: '−' }), plus = h('button', { type: 'button', 'aria-label': t('tune.more'), text: '+' });
    const edges = () => { minus.disabled = cur <= L.SWARM_MIN; plus.disabled = cur >= L.SWARM_MAX; };       // the buttons stop at 2 and 5 (story 159)
    const go = async (d) => { const n = L.stepSwarmN(cur, d); if (n === cur) return; try { await apply(n); cur = n; out.textContent = String(n); edges(); } catch (e) { fail(e); } };
    minus.onclick = () => go(-1); plus.onclick = () => go(1);
    edges();
    return h('div', { class: 'stepper set', role: 'group', 'aria-label': label }, minus, out, plus);
  }

  // hotkey: click, then press the combination. A wrong or taken one stays visible under the field and is not applied (story 196)
  function keyRow(o) {
    const cap = h('div', { class: 'keyfield', tabIndex: 0, role: 'button', 'aria-label': o.title });
    const r = row(o.title, o.desc, cap);
    let capturing = false;
    const caps = (accel) => h('span', { class: 'keys' }, L.keyLabel(accel, platform()).map((k) => h('span', { text: k === '{n}' ? '1…7' : k })));
    const show = (accel) => clear(cap).append(caps(accel));
    const stop = () => { capturing = false; cap.classList.remove('cap'); show(o.get()); };
    show(o.get());
    cap.onclick = () => { if (capturing) return; capturing = true; cap.classList.add('cap'); clear(cap).append(h('em', { text: t('set.press_keys') })); r.hint(''); mark(cap, false); };
    cap.addEventListener('blur', () => { if (capturing) stop(); });
    cap.addEventListener('keydown', async (e) => {
      if (!capturing) { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); cap.click(); } return; }
      e.preventDefault();
      if (e.key === 'Escape') { stop(); r.hint(''); mark(cap, false); return; }
      let accel = L.accelFromEvent(e, platform());
      if (!accel) return;                                        // only a modifier so far
      if (o.pattern) {
        if (!/\+\d$/.test(accel)) { clear(cap).append(caps(accel)); mark(cap, true); r.hint(t('hint.key_digit')); return; }
        accel = accel.replace(/\d$/, '{n}');
      }
      const v = L.validateAccel(accel.replace('{n}', '1'), { taken: o.taken(), needModifier: o.needModifier });
      if (!v.ok) { clear(cap).append(caps(accel)); mark(cap, true); r.hint(v.why === 'busy' ? t('hint.key_busy') : t('hint.key_format')); return; }
      try { await o.apply(accel); mark(cap, false); r.hint(''); stop(); } catch (er) { mark(cap, true); r.hint(errHint(er)); }
    });
    return r;
  }

  // the notices that arrive after the section is drawn (tray, blocked notifications) redraw the section that shows them
  let noticeCb = null;
  document.addEventListener('ns-notice', () => { if (noticeCb) noticeCb(); });

  // ---------- sections ----------
  const SEC = {};
  const def = (id, o) => { SEC[id] = Object.assign({ id }, o); };

  // --- Модели ---
  def('models', { env: true, reset: true, live: ['catalog', 'envs'], build(env, fx) {
    const cat = S.catalogs.get(env.id), es = () => envSet(env.id);
    const nodes = [];
    nodes.push(h('div', { class: 'sgrp' }, h('h2', { text: t('set.models.default') }), NS.models.picker(env.id)));
    // a model for a role: "family|version" in the select; "последняя" = no pinned version, a pinned one shows a lock and "есть новее"
    function roleRow(title, desc, inheritKey, keys, getChoice, setRoles) {
      const cur = getChoice();
      const sel = h('select', { 'aria-label': title }, L.roleOptions(cat, t, inheritKey, cur).map((o) => h('option', { value: o.value, text: o.text, selected: o.value === (cur ? L.choiceKey(cur) : '') })));
      const newer = L.pinNewer(cat, cur);
      const r = row(title, desc, newer ? h('small', { class: 'newer', text: t('model.newer', { v: newer }) }) : null, sel, h('span', { class: 'pin', title: cur && cur.version ? t('set.pinned') : '' }, cur && cur.version ? ic('i-lock') : null));
      sel.onchange = async () => {
        const v = L.choiceFromKey(sel.value), roles = Object.assign({}, es().roles);
        try { await setRoles(v, roles); fx.redraw(); } catch (e) { sel.value = cur ? L.choiceKey(cur) : ''; r.hint(errHint(e)); }
      };
      return r;
    }
    const setRole = (keys) => (v, roles) => { for (const k of keys) { if (v) roles[k] = v; else delete roles[k]; } return commitEnv(env.id, { roles }); };
    nodes.push(group(t('set.models.roles'), [
      roleRow(t('set.role.planner'), t('set.role.planner_d'), 'set.inherit', ['planner', 'critic'], () => es().roles.planner || null, setRole(['planner', 'critic'])),
      roleRow(t('set.role.worker'), t('set.role.worker_d'), 'set.inherit_default', ['worker'], () => es().roles.worker || null, setRole(['worker'])),
      roleRow(t('set.role.reviewer'), t('set.role.reviewer_d', { env: env.name }), 'set.inherit', ['reviewer'], () => es().roles.reviewer || null, setRole(['reviewer'])),
      roleRow(t('set.role.fallback'), t('set.role.fallback_d'), 'set.none', [], () => es().fallback || null, (v) => commitEnv(env.id, { fallback: v })),
    ]));
    // "Своя модель": identifiers as the CLI knows them
    const own = h('input', { type: 'text', class: 'wide', placeholder: t('model.own_ph'), 'aria-label': t('model.own') });
    const chips = h('div', { class: 'chips' }, es().custom_models.map((m) => h('span', { class: 'chip' }, m,
      h('button', { type: 'button', 'aria-label': t('set.remove') + ' ' + m, text: '×', onclick: async () => { try { await commitEnv(env.id, { custom_models: es().custom_models.filter((x) => x !== m) }); fx.redraw(); } catch (e) { fail(e); } } }))));
    const addOwn = async () => { const id = own.value.trim(); if (!id || es().custom_models.includes(id)) return; try { await commitEnv(env.id, { custom_models: es().custom_models.concat(id) }); fx.redraw(); } catch (e) { fail(e); } };
    own.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); addOwn(); } });
    if (es().custom_models.length) nodes.push(group(t('model.own'), [row(t('set.own_title'), t('model.own_d'), chips, own, h('button', { type: 'button', class: 'btn sm', text: t('set.add'), onclick: addOwn }))]));   // adding is in the picker above, here they are managed
    return nodes;
  } });

  // --- Права и песочница ---
  def('perms', { env: true, reset: true, build(env) {
    const sb = env.sandbox, es = () => envSet(env.id), nodes = [];
    if (sb && sb.sandboxed === false) nodes.push(h('div', { class: 'warnbox' }, ic('i-warn'), h('p', {}, h('b', { text: t('perm.nosandbox', { env: env.name }) + ' ' }), t('perm.nosandbox_d'))));
    const pill = (ok, yes, no) => h('span', { class: 'pill-s ' + (ok ? 'ok' : 'no') }, ic(ok ? 'i-check' : 'i-warn'), ok ? yes : no);
    nodes.push(group(t('perm.now'), sb ? [
      row(t('perm.sandbox'), sb.note || '', pill(sb.sandboxed, t('perm.on'), t('perm.none'))),
      row(t('perm.network'), '', h('span', { text: sb.network_open ? t('perm.open') : t('perm.closed') })),
      row(t('perm.files'), '', h('span', { text: sb.writes_outside_copy ? t('perm.can_write') : t('perm.denied') })),
    ] : [row(t('perm.sandbox'), t('perm.unknown'), h('span', { class: 'hint', text: t('perm.no_data') }))]));
    nodes.push(group(t('perm.ask'), [
      toggleRow({ title: t('perm.ask_push'), desc: t('perm.ask_push_d'), on: on(es().ask_push), apply: (v) => commitEnv(env.id, { ask_push: v }) }),
      toggleRow({ title: t('perm.ask_pr'), on: on(es().ask_pr), apply: (v) => commitEnv(env.id, { ask_pr: v }) }),
      toggleRow({ title: t('perm.protected'), desc: t('perm.protected_d'), on: on(es().protected_paths), apply: (v) => commitEnv(env.id, { protected_paths: v }) }),
    ]));
    return nodes;
  } });

  // --- Запуск и вход ---
  def('run', { env: true, reset: true, live: ['envs'], build(env, fx) {
    const es = () => envSet(env.id), nodes = [];
    const recheck = async () => { try { await st.reloadEnvs(); } catch (e) { fail(e); } };
    // the path is checked by asking the core whether the env is found there: a wrong path is not kept
    const pathRow = textRow({ title: t('run.path'), desc: t('run.path_d'), value: es().cli_path || '', placeholder: env.path ? t('run.found', { path: env.path }) : t('run.not_found'),
      check: (raw) => { const v = raw.trim(); return v && !/^(\/|~|[A-Za-z]:[\\/])/.test(v) ? { ok: false, hint: t('hint.path_abs') } : { ok: true, value: v || null }; },
      apply: async (v) => {
        const before = es().cli_path || null;
        await commitEnv(env.id, { cli_path: v });
        if (v) { await st.reloadEnvs(); const e2 = st.env(env.id); if (!e2 || e2.state === 'missing') { await commitEnv(env.id, { cli_path: before }); await st.reloadEnvs(); throw new Error(t('hint.path_missing')); } }
      } });
    const login = env.state === 'ok' ? h('span', { class: 'pill-s ok' }, ic('i-check'), t('run.signed_in'))
      : env.state === 'login' ? h('button', { type: 'button', class: 'btn sm', text: t('run.sign_in'), onclick: async () => { try { await NS.api.call('login_env', { env: env.id }); } catch (e) { fail(e); } } })
      : env.state === 'missing' ? h('button', { type: 'button', class: 'btn sm', text: t('envs.install'), onclick: async () => { try { await NS.api.call('install_env', { env: env.id }); await recheck(); } catch (e) { fail(e); } } })
      : h('span', { class: 'hint', text: env.reason || t('run.unavailable') });
    const args = textRow({ title: t('run.args'), desc: t('run.args_d'), value: L.formatArgs(es().extra_args), placeholder: t('run.args_ph'),
      check: (raw) => { const p = L.parseArgs(raw); return p.error ? { ok: false, hint: t('hint.quotes') } : { ok: true, value: p.args }; },
      apply: (v) => commitEnv(env.id, { extra_args: v }) });
    nodes.push(group('', [pathRow, row(t('run.login'), env.version ? t('run.version', { v: env.version }) : '', login), args]));
    return nodes;
  } });

  // --- Качество ---
  def('quality', { reset: true, build() {
    const q = () => sget('quality'), setQ = (patch) => commit({ quality: Object.assign({}, q(), patch) });
    return [
      group(t('set.q.pipeline'), [
        toggleRow({ title: t('set.q.on'), desc: t('set.q.on_d'), on: q().on, apply: (v) => setQ({ on: v }) }),
        toggleRow({ title: t('q.spec'), desc: t('set.q.spec_d'), on: q().spec, apply: (v) => setQ({ spec: v }) }),
        toggleRow({ title: t('q.plan'), desc: t('set.q.plan_d'), on: q().plan, apply: (v) => setQ({ plan: v }) }),
        toggleRow({ title: t('q.tests'), desc: t('set.q.tests_d'), on: q().tests_first, apply: (v) => setQ({ tests_first: v }) }),
        always(t('q.gates'), t('set.q.gates_d')),
        toggleRow({ title: t('q.e2e'), desc: t('set.q.e2e_d'), on: q().e2e, apply: (v) => setQ({ e2e: v }) }),
      ]),
      group(t('set.q.review'), [
        numRow({ title: t('set.q.rounds'), desc: t('set.q.rounds_d'), value: sget('review_rounds'), unit: t('set.max'), int: true, rule: { min: 1, max: 10, int: true }, apply: (v) => commit({ review_rounds: v }) }),
        (() => {
          const num = h('input', { type: 'text', class: 'num', inputMode: 'numeric', value: String(q().best_of_n || 2), 'aria-label': t('q.bestn') });
          const r = toggleRow({ title: t('q.bestn'), desc: t('set.q.bestn_d'), on: !!q().best_of_n, apply: (v) => setQ({ best_of_n: v ? (Number(num.value) || 2) : null }), extra: h('span', { class: 'unit', text: t('set.variants') }) });
          r.querySelector('.ctl').prepend(num);
          num.addEventListener('change', async () => {
            const v = L.validateNumber(num.value, { min: 2, max: 5, int: true });
            if (!v.ok) { mark(num, true); r.hint(v.why === 'range' ? t('hint.range', { min: 2, max: 5 }) : t('hint.nan')); return; }
            try { await setQ({ best_of_n: v.value }); r.sw.set(true); mark(num, false); r.hint(''); } catch (e) { mark(num, true); r.hint(errHint(e)); }
          });
          return r;
        })(),
      ]),
    ];
  } });

  // --- Рой и фабрика ---
  def('swarm', { reset: true, build() {
    const sd = () => sget('swarm_default');
    const MODES = [['race', t('swarm.race')], ['crew', t('swarm.crew')], ['council', t('swarm.council')]];
    const auto = () => sget('autopick_min');
    const autoNum = h('input', { type: 'text', class: 'num', inputMode: 'numeric', value: String(auto() || 10), 'aria-label': t('set.sw.autopick') });
    const autoRow = toggleRow({ title: t('set.sw.autopick'), desc: t('set.sw.autopick_d'), on: auto() > 0, apply: (v) => commit({ autopick_min: v ? (Number(autoNum.value) || 10) : 0 }), extra: h('span', { class: 'unit', text: t('set.min') }) });
    autoRow.querySelector('.ctl').prepend(autoNum);
    autoNum.addEventListener('change', async () => {
      const v = L.validateNumber(autoNum.value, { min: 1, max: 120, int: true });
      if (!v.ok) { mark(autoNum, true); autoRow.hint(v.why === 'range' ? t('hint.range', { min: 1, max: 120 }) : t('hint.nan')); return; }
      try { await commit({ autopick_min: v.value }); autoRow.sw.set(true); mark(autoNum, false); autoRow.hint(''); } catch (e) { mark(autoNum, true); autoRow.hint(errHint(e)); }
    });
    return [
      group(t('set.sw.parallel'), [
        numRow({ title: t('set.sw.places'), desc: t('set.sw.places_d'), value: sget('max_parallel'), unit: t('set.pcs'), int: true, rule: { min: 1, max: 10, int: true }, apply: (v) => commit({ max_parallel: v }) }),
        row(t('set.sw.mode'), t('set.sw.mode_d'), segment(MODES, sd().mode, t('tune.swarm_mode'), (v) => commit({ swarm_default: Object.assign({}, sd(), { mode: v }) }))),
        row(t('set.sw.n'), t('set.sw.n_d'), stepper(L.clampSwarmN(sd().n), t('set.sw.n'), (n) => commit({ swarm_default: Object.assign({}, sd(), { n }) }))),
        row(t('set.sw.models'), t('set.sw.models_d'), h('span', { class: 'hint', text: t('set.sw.models_v') })),
      ]),
      group(t('set.sw.stuck'), [
        numRow({ title: t('set.sw.attempts'), desc: t('set.sw.attempts_d'), value: sget('attempts'), unit: t('set.tries'), int: true, rule: { min: 1, max: 10, int: true }, apply: (v) => commit({ attempts: v }) }),
        numRow({ title: t('set.sw.stall'), desc: t('set.sw.stall_d'), value: sget('stall_min'), unit: t('set.min'), int: true, rule: { min: 5, max: 120, int: true }, apply: (v) => commit({ stall_min: v }) }),
        autoRow,
      ]),
    ];
  } });

  // --- Лимиты и бюджет ---
  def('limits', { reset: true, live: ['limits'], build(env, fx) {
    const lim = () => sget('limits'), setL = (patch) => commit({ limits: Object.assign({}, lim(), patch) });
    const now = Date.now(), nodes = [];
    nodes.push(group(t('set.l.budget'), [
      numRow({ title: t('set.l.task'), desc: t('set.l.task_d'), prefix: '$', value: lim().task_usd, rule: { min: 0.5, max: 1000 }, apply: (v) => setL({ task_usd: v }) }),
      numRow({ title: t('set.l.day'), desc: t('set.l.day_d'), prefix: '$', value: lim().day_usd, rule: { min: 1, max: 10000 }, apply: (v) => setL({ day_usd: v }) }),
      numRow({ title: t('set.l.warn'), desc: t('set.l.warn_d'), value: lim().warn_pct, unit: '%', int: true, rule: { min: 50, max: 99, int: true }, apply: (v) => setL({ warn_pct: v }) }),
      timeRow({ title: t('set.l.night'), desc: t('set.l.night_d'), from: lim().night_from, to: lim().night_to, apply: (a, b) => setL({ night_from: a, night_to: b }) }),
      numRow({ title: t('set.l.away'), desc: t('set.l.away_d'), value: lim().away_min, unit: t('set.min'), int: true, rule: { min: 5, max: 240, int: true }, apply: (v) => setL({ away_min: v }) }),
    ]));
    // every env has its own windows (the limit belongs to the env, story 102)
    const rows = [];
    for (const e of S.envs.filter((x) => x.state === 'ok')) {
      if (!S.limits.has(e.id)) st.loadLimits(e.id).then(() => fx.alive.v && fx.redraw());
      const v = L.limitsView(S.limits.get(e.id));
      const body = v.kind === 'known' && v.windows.length ? h('div', { class: 'limrow' }, v.windows.map((w) => h('div', { class: 'lim ' + L.limitLevel(w, lim().warn_pct) }, h('span', { class: 'lt', text: L.formatLimit(w, now, t) }),
        h('span', { class: 'lb' }, NS.setVars(h('i'), { '--w': (w.exhausted ? 100 : Math.max(0, Math.min(100, w.used_pct || 0))) + '%' })))))
        : h('small', { text: v.kind === 'nodata' && v.why ? NS.limitWhy(v.why) : t('lim.no_data') });
      rows.push(h('div', { class: 'limwin' }, h('b', {}, envIcon(e.custom ? 'custom' : e.id), e.name), body));
    }
    nodes.push(h('div', { class: 'sgrp' }, h('h2', { text: t('set.l.all') }), h('div', { class: 'srows' }, rows)));
    return nodes;
  } });

  // --- Уведомления ---
  def('notify', { reset: true, build(env, fx) {
    noticeCb = () => fx.alive.v && fx.redraw();
    const n = () => sget('notify'), setN = (patch) => commit({ notify: Object.assign({}, n(), patch) });
    const blocked = (S.notices || {}).blocked;
    return [
      blocked ? h('div', { class: 'warnbox' }, ic('i-warn'), h('p', {}, h('b', { text: t('notice.blocked') + ' ' }), blocked)) : null,
      group(t('set.n.when'), [
        always(t('set.n.question'), t('set.n.always_d')), always(t('set.n.fail')), always(t('set.n.outward')),
        toggleRow({ title: t('set.n.done'), desc: t('set.n.done_d'), on: n().on_done, apply: (v) => setN({ on_done: v }) }),
      ]),
      group(t('set.n.how'), [
        selectRow({ title: t('set.n.sound'), value: n().sound, options: [['soft_click', t('set.n.soft')], ['bell', t('set.n.bell')], ['none', t('set.n.none')]].map(([value, text]) => ({ value, text })), apply: (v) => setN({ sound: v }) }),
        timeRow({ title: t('set.n.quiet'), desc: t('set.n.quiet_d'), from: n().quiet_from, to: n().quiet_to, apply: (a, b) => setN({ quiet_from: a, quiet_to: b }) }),
      ]),
    ].filter(Boolean);
  } });

  // --- Внешний вид ---
  def('look', { reset: true, build(env, fx) {
    noticeCb = () => fx.alive.v && fx.redraw();
    const look = () => sget('look'), setLook = (patch) => commit({ look: Object.assign({}, look(), patch) });
    const langNow = (S.settings || {}).language || 'auto';
    return [
      group('', [
        row(t('set.v.theme'), '', segment([['system', t('set.v.system')], ['dark', t('set.v.dark')], ['light', t('set.v.light')]], look().theme, t('set.v.theme'), (v) => setLook({ theme: v }))),
        numRow({ title: t('set.v.font'), desc: t('set.v.font_d'), value: look().font_px, unit: 'px', int: true, rule: { min: 11, max: 20, int: true }, apply: (v) => setLook({ font_px: v }) }),
        toggleRow({ title: t('set.v.labels'), desc: t('set.v.labels_d'), on: look().role_labels, apply: (v) => setLook({ role_labels: v }) }),
        row(t('set.v.lang'), '', segment([['auto', t('set.v.auto')], ['ru', 'Русский'], ['en', 'English']], langNow, t('set.v.lang'), (v) => fx.ctx.setLang(v))),
        (S.notices || {}).trayMissing ? h('div', { class: 'tray-line' }, ic('i-warn'), h('span', { text: t('set.v.tray') })) : null,
      ].filter(Boolean)),
    ];
  } });

  // --- Горячие клавиши ---
  def('keys', { reset: true, build() {
    // v3 (story 236): tab keys join the list; ⌘1…9 are tabs, envs moved to ⌥⌘1…7; every row is checked against all the others
    const V = window.NSLive, plat = platform();
    const k = () => Object.assign(V.tabKeyDefaults(plat), sget('keys'), { switch_env: V.migrateSwitchEnv(sget('keys').switch_env, plat) });
    const setK = (patch) => commit({ keys: Object.assign({}, k(), patch) });
    const ROWS = ['new_chat', 'send', 'freeze', 'tab_new', 'tab_close', 'tab_split', 'tab_unsplit'];
    const others = (own) => () => ROWS.filter((x) => x !== own).map((x) => k()[x]).concat(V.fixedTabKeys(plat));
    const wayland = plat !== 'mac';
    const names = { new_chat: t('set.k.new'), send: t('set.k.send'), freeze: t('set.k.freeze'), switch_env: t('set.k.next'), tab_new: t('keys3.tab_new'), tab_close: t('keys3.tab_close'),
      tab_split: t('keys3.tab_split'), tab_unsplit: t('keys3.tab_unsplit'), tab_digits: t('keys3.tab_digits') };
    const clash = V.keyConflicts(Object.fromEntries(ROWS.concat('switch_env').map((x) => [x, k()[x]])), plat);
    const tabRow = (id, desc) => keyRow({ title: t('keys3.' + id), desc: desc ? t(desc) : '', get: () => k()[id], needModifier: true, taken: others(id), apply: (v) => setK({ [id]: v }) });
    return [
      group('', [
        keyRow({ title: t('set.k.new'), desc: t('set.k.new_d'), get: () => k().new_chat, needModifier: true, taken: others('new_chat'), apply: (v) => setK({ new_chat: v }) }),
        keyRow({ title: t('set.k.next'), desc: t('set.k.next_d'), get: () => k().switch_env, needModifier: true, pattern: true, taken: () => V.fixedTabKeys(plat), apply: (v) => setK({ switch_env: v }) }),
        keyRow({ title: t('set.k.send'), get: () => k().send, needModifier: false, taken: others('send'), apply: (v) => setK({ send: v }) }),
        keyRow({ title: t('set.k.freeze'), desc: t('set.k.freeze_d'), get: () => k().freeze, needModifier: true, taken: others('freeze'), apply: (v) => setK({ freeze: v }) }),
      ]),
      group(t('tabs.label'), [
        tabRow('tab_new'), tabRow('tab_close', 'keys3.tab_close_d'), tabRow('tab_split', 'keys3.tab_split_d'), tabRow('tab_unsplit'),
        row(t('keys3.fixed'), '', h('span', { class: 'hint', text: t(plat === 'mac' ? 'keys3.fixed_mac' : 'keys3.fixed_pc') })),
      ]),
      ...clash.map(([a, b]) => h('div', { class: 'warnbox' }, ic('i-warn'), h('p', { text: names[a] + ': ' + t('keys3.conflict', { other: names[b] }) }))),
      wayland ? h('div', { class: 'warnbox' }, ic('i-warn'), h('p', {}, t('set.k.wayland'), ' ', h('code', { text: 'nightshift --new-chat' }), '.')) : null,
    ].filter(Boolean);
  } });

  // a section of the registry ('settings' point) looks like one of SEC to the frame
  const extSec = (id) => { const d = NS.registry.points('settings').find((x) => x.id === id); return d ? Object.assign({ live: [] }, d, { title: d.title || (() => t(d.labelKey)) }) : null; };

  // ---------- the frame ----------
  function frame(root, ctx, sectionId, extra) {
    const envs = S.envs;
    const envId = ctx.params.env || S.lastEnv || (envs.find((e) => e.state === 'ok') || envs[0] || {}).id;
    const env = st.env(envId) || envs[0] || { id: envId, name: envId || '', state: 'ok' };
    const alive = { v: true };
    if (env.id && S.route.env !== env.id) { S.route.env = env.id; NS.shell.barUpdate('route'); }     // the bar shows which env these settings belong to
    const nav = h('nav', { class: 'snav', 'aria-label': t('set.label') });
    const body = h('section', { class: 'sbody' });
    root.append(h('div', { class: 'settings' }, nav, body));
    const go = (id) => (id === 'kits' ? ctx.nav('kits', { env: env.id }) : ctx.nav('settings', { env: env.id, section: id }));
    const fx = { ctx, env, extra, alive, redraw: () => draw() };
    function drawNav() {
      const link = (id, key) => { const on = id === sectionId || (id === 'kits' && sectionId === 'import');
        return h('button', { type: 'button', class: on ? 'on' : '', 'aria-current': on ? 'page' : null, text: t(key || 'set.nav.' + id), onclick: () => go(id) }); };
      const ids = (g) => L.SETTINGS_SECTIONS.filter((x) => x.group === g).map((x) => x.id);
      // v4 (story 295): "Основное" first, then "Расширенное" (search over every key) with the sections of v2 under it;
      // sections of other tasks come from the registry ('settings', level basic | advanced)
      const ext = (lvl) => NS.registry.points('settings').filter((d) => d.level === lvl).map((d) => link(d.id, d.labelKey));
      clear(nav).append(h('h2', { text: t('set.group_v4') }), ...ext('basic'), ...ext('advanced'),
        h('h2', {}, envIcon(env.custom ? 'custom' : env.id), env.name), ...ids('env').map((id) => link(id)), h('h2', { text: t('set.group_all') }), ...ids('all').map((id) => link(id)),
        h('div', { class: 'snav-gap' }), ...ids('projects').map((id) => link(id)));
    }
    function draw() {
      const top = body.scrollTop;
      const sec = SEC[sectionId] || extSec(sectionId) || SEC.models;
      clear(body);
      const nodes = sec.build(env, fx) || [];
      const title = sec.title ? sec.title(env, fx) : t('set.nav.' + sec.id);
      NS.append(body, [h('h1', {}, sec.env ? envIcon(env.custom ? 'custom' : env.id) : null, title), sec.lead !== false && t('set.lead.' + sec.id) !== 'set.lead.' + sec.id ? h('p', { class: 'lead', text: t('set.lead.' + sec.id, { env: env.name }) }) : null, nodes]);
      if (!['kits', 'import'].includes(sec.id)) body.append(h('p', { class: 'sfoot-note', text: t('set.new_runs') }));
      if (sec.reset && L.resetPatch(sec.id)) {
        body.append(h('div', { class: 'sfoot' }, h('button', { type: 'button', class: 'linkb', text: t('set.reset'), onclick: async () => {
          if (!(await NS.projects.confirm(t('set.reset_title', { name: t('set.nav.' + sec.id) }), t('set.reset_text'), t('set.reset')))) return;
          const p = L.resetPatch(sec.id);
          try {
            // drop the section's keys: the core reads missing keys as its defaults, then we read the result back
            const next = Object.assign({}, S.settings);
            for (const k of p.settings) delete next[k];
            if (p.env.length) { const envs = Object.assign({}, next.envs), blk = Object.assign({}, envs[env.id]); for (const k of p.env) delete blk[k]; envs[env.id] = blk; next.envs = envs; }
            touch();
            await NS.api.call('set_settings', { settings: next });
            S.settings = await NS.api.call('get_settings');
            applyLook();
            if (p.settings.includes('language')) ctx.setLang(S.settings.language || 'auto');
            NS.shell.toast(t('set.reset_done'));
            if (alive.v) draw();
          } catch (e) { fail(e); }
        } })));
      }
      body.scrollTop = top;
    }
    drawNav(); draw();
    if (SEC[sectionId] && SEC[sectionId].env && env.state !== 'missing') st.loadCatalog(env.id).then(() => alive.v && SEC[sectionId].live && SEC[sectionId].live.includes('catalog') && draw());
    return {
      update(evt) { const sec = SEC[sectionId]; if (Date.now() < quietUntil) return; if (sec && sec.live && sec.live.includes(evt) && alive.v) draw(); },
      destroy() { alive.v = false; if (SEC[sectionId] && SEC[sectionId].destroy) SEC[sectionId].destroy(); },
    };
  }

  NS.settingsKit = { touch, SEC, def, frame, row, group, numRow, timeRow, textRow, toggleRow, selectRow, segment, sw, always, commit, commitEnv, sget, envSet, platform, mark, errHint, applyLook };

  // "section: 'env'" is what the first-run composer passes (model of the env)
  NS.registry.register({ id: 'settings', layout: 'full', keepEnv: true, rail: { zone: 'foot', icon: 'i-gear', labelKey: 'rail.settings', order: 2 },
    mount(root, ctx) { const sec = ctx.params.section; return frame(root, ctx, !sec ? 'basic' : sec === 'env' ? 'models' : sec); } });
})();
