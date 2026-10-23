// v4 settings in two levels (stories 293, 295, G57): "Основное" - theme cards, accent, font and size, corners, gap, opacity, Clawd,
// three ready looks, all with a live preview; "Расширенное" - every other key (the v2 sections) on one page with a search,
// "Открыть конфиг" (config_open) and the errors of the config file (config_errors / config-changed).
// theme and font_px are core fields (settings.look); the v4 extras ride in settings.ui.tabs.look (the only free-form JSON of
// Settings) until the config file (core::config) owns them.
(function () {
  const NS = window.NS;
  const { h, ic, clear, setVars } = NS;
  const T = window.NSTheme;
  const t = (k, p) => NS.t(k, p);
  const S = NS.store.s;
  const K = () => NS.settingsKit;
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);

  const lookNow = () => T.fromSettings(S.settings);
  // one change: core fields to settings.look, the rest to settings.ui.tabs.look; applied at once
  async function setLook(patch) {
    const core = {}, extra = {};
    for (const [k, v] of Object.entries(patch)) (k === 'theme' || k === 'font_px' ? core : extra)[k] = v;
    const next = {};
    if (Object.keys(core).length) next.look = Object.assign({}, S.settings.look, core);
    if (Object.keys(extra).length) {
      const tabs = Object.assign({}, (S.settings.ui && S.settings.ui.tabs) || {});
      tabs.look = Object.assign({}, tabs.look, extra);
      next.ui = Object.assign({}, S.settings.ui, { tabs });
    }
    T.applyTheme(T.look(Object.assign({}, lookNow(), patch)));      // the preview does not wait for the core
    await K().commit(next);
  }
  function applyPreset(id) {
    const p = T.PRESETS[id];
    document.body.dataset.preset = id;
    NS.shell.toggleSide(!!p.side);
    return setLook({ preset: id, radius: p.radius, gap: p.gap, opacity: p.opacity });
  }
  const markPreset = () => { document.body.dataset.preset = lookNow().preset; };
  window.addEventListener('ns-ready', markPreset);

  // a miniature of the window in a given theme: canvas, two panes (feed with ❯ and $), the bar
  function mini(themeName, lk) {
    const el = h('div', { class: 'mini', 'aria-hidden': 'true' },
      h('div', { class: 'mini-tabs' }, h('i', { class: 'on' }), h('i'), h('i')),
      h('div', { class: 'mini-panes' },
        h('div', { class: 'mini-pane focus' }, h('b', { text: '❯ ' + t('look4.pv_cmd') }), h('span', { class: 'cmd', text: '$ pytest -q' }), h('span', { class: 'ok', text: t('look4.pv_out') })),
        h('div', { class: 'mini-pane' }, h('span', { class: 'mute', text: '· Read src/invoices/routes.py' }), h('span', { class: 'add', text: '+ cursor = encode(last.id)' }), h('span', { class: 'del', text: '- page = int(q["page"])' }))),
      h('div', { class: 'mini-bar' }, h('span', { text: 'Claude Code' }), h('span', { text: 'main ↑3' }), h('span', { class: 'grow' }), h('span', { text: '14:20' })));
    if (themeName) setVars(el, T.vars(themeName, lk, 'claude'));
    return el;
  }
  function cards(name, opts, value, apply) {
    const box = h('div', { class: 'cards4', role: 'radiogroup', 'aria-label': name });
    const draw = (v) => clear(box).append(...opts.map((o) => h('button', { type: 'button', role: 'radio', class: 'card4' + (o.value === v ? ' on' : ''), 'aria-checked': String(o.value === v),
      onclick: async () => { draw(o.value); try { await apply(o.value); } catch (e) { draw(v); fail(e); } } }, o.preview, h('span', { class: 'card4-t' }, h('b', { text: o.label }), o.desc ? h('small', { text: o.desc }) : null))));
    draw(value);
    return box;
  }
  function range(label, value, min, max, unit, apply) {
    const out = h('output', { text: value + unit });
    const inp = h('input', { type: 'range', min, max, step: 1, value: String(value), 'aria-label': label });
    inp.addEventListener('input', () => { out.textContent = inp.value + unit; T.applyTheme(T.look(Object.assign({}, lookNow(), { [inp.dataset.k]: Number(inp.value) }))); });
    inp.addEventListener('change', () => apply(Number(inp.value)).catch(fail));
    return [inp, out];
  }

  function basic(env, fx) {
    const L0 = lookNow(), k = K();
    const sys = !!(window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches);
    const themeOpts = [['system', 'look4.t_system'], ['dark', 'look4.t_dark'], ['light', 'look4.t_light']]
      .map(([v, key]) => ({ value: v, label: t(key), preview: mini(T.resolve(v, sys), L0) }));
    const presetOpts = ['hypr', 'minimal', 'classic'].map((id) => {
      const p = T.PRESETS[id], pv = mini(null);
      pv.classList.add('preset-' + id);
      setVars(pv, { '--radius-pane': T.RADIUS[p.radius][0] + 'px', '--radius-sm': T.RADIUS[p.radius][1] + 'px', '--gap': p.gap + 'px' });
      return { value: id, label: t('look4.p_' + id), desc: t('look4.p_' + id + '_d'), preview: pv };
    });
    const swatches = ['env', '#8fb3d9', '#5fbfc0', '#79bf86', '#d9b45f', '#e5675d', '#d97757'];
    const accentBox = h('div', { class: 'swatches', role: 'radiogroup', 'aria-label': t('look4.accent') });
    const drawAccent = (v) => clear(accentBox).append(...swatches.map((c) => {
      const b = h('button', { type: 'button', role: 'radio', class: 'swatch' + (c === v ? ' on' : '') + (c === 'env' ? ' env' : ''), 'aria-checked': String(c === v), title: c === 'env' ? t('look4.a_env') : c,
        onclick: async () => { drawAccent(c); try { await setLook({ accent: c }); } catch (e) { drawAccent(v); fail(e); } } }, c === 'env' ? h('span', { text: t('look4.a_env') }) : h('i'));
      if (c !== 'env') setVars(b, { '--c': c });
      return b;
    }));
    drawAccent(L0.accent);
    const fontSel = h('select', { 'aria-label': t('look4.font') }, Object.keys(T.FONTS).map((f) => h('option', { value: f, selected: f === L0.font, text: f === 'system' ? t('look4.f_system') : T.FONTS[f].split(',')[0].replace(/"/g, '') })));
    fontSel.onchange = () => setLook({ font: fontSel.value }).catch(fail);
    const size = k.numRow({ title: t('look4.size'), desc: '', value: L0.font_px, unit: 'px', int: true, rule: { min: 11, max: 20, int: true }, apply: (v) => setLook({ font_px: v }) });
    const [gapIn, gapOut] = range(t('look4.gap'), L0.gap, 0, 16, 'px', (v) => setLook({ gap: v })); gapIn.dataset.k = 'gap';
    const [opIn, opOut] = range(t('look4.opacity'), L0.opacity, 70, 100, '%', (v) => setLook({ opacity: v })); opIn.dataset.k = 'opacity';
    const preview = h('div', { class: 'preview4' }, h('small', { text: t('look4.preview') }), mini(null));
    return [
      h('div', { class: 'basic4' },
        h('div', { class: 'basic4-l' },
          k.group(t('look4.preset'), [cards(t('look4.preset'), presetOpts, L0.preset, applyPreset)]),
          k.group(t('look4.theme'), [cards(t('look4.theme'), themeOpts, L0.theme, (v) => setLook({ theme: v }))]),
          k.group('', [
            k.row(t('look4.accent'), t('look4.accent_d'), accentBox),
            k.row(t('look4.font'), t('look4.font_d'), fontSel),
            size,
            k.row(t('look4.radius'), '', k.segment([['sharp', t('look4.r_sharp')], ['soft', t('look4.r_soft')], ['round', t('look4.r_round')]], L0.radius, t('look4.radius'), (v) => setLook({ radius: v }))),
            k.row(t('look4.gap'), '', gapIn, gapOut),
            k.row(t('look4.opacity'), '', opIn, opOut),
            k.row(t('look4.clawd'), t('look4.clawd_d'), k.segment([['animate', t('look4.c_animate')], ['static', t('look4.c_static')], ['hide', t('look4.c_hide')]], L0.clawd, t('look4.clawd'), (v) => setLook({ clawd: v }))),
          ])),
        preview),
    ];
  }

  // ---------- "Расширенное": every v2 section on one page, a search over the rows, the config file ----------
  async function openConfig() {
    try { await NS.api.call('config_open'); } catch (e) { NS.shell.toast(/not_built|unknown command/.test(NS.api.errText(e)) ? t('adv.config_missing') : NS.errHuman(NS.api.errText(e)), true); }
  }
  let configErrors = null;
  function errorsBox() {
    const box = h('div', { class: 'cfg-errs', role: 'status' });
    const draw = (list) => {
      clear(box);
      if (!list || !list.length) { box.hidden = true; return; }
      box.hidden = false;
      box.append(h('h3', {}, ic('i-warn'), t('adv.errors')), h('ul', {}, list.map((e) => h('li', { text: t('adv.err_line', { line: e.line, key: e.key, msg: e.msg }) }))));
    };
    draw(configErrors);
    NS.api.call('config_errors').then((x) => { configErrors = Array.isArray(x) ? x : (x && x.errors) || []; draw(configErrors); }).catch(() => draw(configErrors));
    box.redraw = () => draw(configErrors);
    return box;
  }
  function advanced(env, fx) {
    const k = K(), SEC = k.SEC;
    const search = h('input', window.NSPalette.quiet({ type: 'search', class: 'adv-q', placeholder: t('adv.search'), 'aria-label': t('adv.search') }));
    const none = h('p', { class: 'hint adv-none', hidden: true, text: t('adv.none') });
    const parts = [];
    for (const { id } of window.NSLogic.SETTINGS_SECTIONS) {
      if (id === 'kits' || !SEC[id]) continue;          // the configs (kits) are a screen of their own
      let nodes = [];
      try { nodes = SEC[id].build(env, fx) || []; } catch (e) { nodes = [h('p', { class: 'hint', text: String(e.message || e) })]; }
      parts.push(h('section', { class: 'adv-sec', dataset: { sec: id } }, h('h2', { class: 'adv-h', text: t('set.nav.' + id) + (SEC[id].env ? ' · ' + env.name : '') }), nodes));
    }
    for (const d of NS.registry.points('settings').filter((x) => x.level === 'advanced' && x.id !== 'advanced')) {
      parts.push(h('section', { class: 'adv-sec', dataset: { sec: d.id } }, h('h2', { class: 'adv-h', text: t(d.labelKey) }), d.build(env, fx) || []));
    }
    const filter = () => {
      const q = search.value.trim().toLowerCase();
      let any = false;
      for (const sec of parts) {
        const head = sec.querySelector('.adv-h').textContent.toLowerCase();
        let shown = 0;
        for (const r of sec.querySelectorAll('.srow')) {
          const hit = !q || head.includes(q) || r.textContent.toLowerCase().includes(q);
          r.hidden = !hit; if (hit) shown++;
        }
        for (const g of sec.querySelectorAll('.sgrp')) g.hidden = !!q && ![...g.querySelectorAll('.srow')].some((r) => !r.hidden) && !g.textContent.toLowerCase().includes(q);
        sec.hidden = !!q && !shown && !head.includes(q);
        if (!sec.hidden) any = true;
      }
      none.hidden = any;
    };
    search.addEventListener('input', filter);
    setTimeout(() => search.focus(), 0);
    return [
      h('div', { class: 'adv-top' }, search, h('button', { type: 'button', class: 'btn', onclick: openConfig }, ic('i-folder'), t('adv.config')), h('small', { class: 'hint', text: t('adv.config_d') })),
      errorsBox(), none, ...parts,
    ];
  }

  NS.registry.extend('settings', { id: 'basic', level: 'basic', labelKey: 'set.nav.basic', order: 1, build: basic });
  NS.registry.extend('settings', { id: 'advanced', level: 'advanced', labelKey: 'set.nav.advanced', order: 1, build: advanced });

  // ---------- palette commands: theme, look, Clawd, config ----------
  const cmd = (c) => NS.registry.extend('command', c);
  [['system', 'look4.t_system'], ['dark', 'look4.t_dark'], ['light', 'look4.t_light']].forEach(([v, key], i) =>
    cmd({ id: 'look.theme.' + v, get label() { return t('cmd.theme', { name: t(key) }); }, group: 'cmd.g.view', order: 70 + i, run: () => setLook({ theme: v }).catch(fail) }));
  ['hypr', 'minimal', 'classic'].forEach((id, i) => cmd({ id: 'look.preset.' + id, get label() { return t('cmd.preset', { name: t('look4.p_' + id) }); }, group: 'cmd.g.view', order: 75 + i, run: () => applyPreset(id).catch(fail) }));
  ['animate', 'static', 'hide'].forEach((m, i) => cmd({ id: 'look.clawd.' + m, get label() { return t('cmd.clawd', { mode: t('look4.c_' + m) }); }, group: 'cmd.g.view', order: 80 + i, run: () => setLook({ clawd: m }).catch(fail) }));
  cmd({ id: 'config.open', labelKey: 'cmd.config', order: 62, run: openConfig });

  // config-changed {errors}: the file was edited by hand - take the settings again, show the errors (story 294)
  NS.api.listen('config-changed', async (pl) => {
    configErrors = (pl && pl.errors) || [];
    try { S.settings = await NS.api.call('get_settings'); K().applyLook(); NS.store.emit('settings', null); } catch (e) { /* the old settings stay */ }
  }).catch(() => {});
  NS.settingsV4 = { setLook, applyPreset, lookNow, openConfig };
})();
