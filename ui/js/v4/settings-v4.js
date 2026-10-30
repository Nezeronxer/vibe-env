// v4 settings in two levels (stories 293, 295, G57): "Основное" - theme cards, accent, font and size, corners, gap, opacity, Clawd,
// three ready looks, all with a live preview; "Расширенное" - every other key (the v2 sections) on one page with a search,
// "Открыть конфиг" (config_open) and the errors of the config file (config_errors / config-changed).
// theme and font_px are core fields (settings.look); the v4 extras ride in settings.ui.tabs.look (the only free-form JSON of
// Settings) until the config file (core::config) owns them.
// look8: nine theme cards (system, eight ready ones) plus the person's own (editor with live preview, JSON import / export),
// the group «Окно и материалы» - window transparency, the macOS blur (window_glass), pane opacity, accent gradients.
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
  // the env is the one these settings are for (Agentation 28.10): its name in the bar, its accent in the panes
  function mini(themeName, lk) {
    const env = NS.store.env(S.route.env) || { id: 'claude', name: 'Claude Code' };
    const el = h('div', { class: 'mini', 'aria-hidden': 'true' },
      h('div', { class: 'mini-tabs' }, h('i', { class: 'on' }), h('i'), h('i')),
      h('div', { class: 'mini-panes' },
        h('div', { class: 'mini-pane focus' }, h('b', { text: '❯ ' + t('look4.pv_cmd') }), h('span', { class: 'cmd', text: '$ pytest -q' }), h('span', { class: 'ok', text: t('look4.pv_out') })),
        h('div', { class: 'mini-pane' }, h('span', { class: 'mute', text: '· Read src/invoices/routes.py' }), h('span', { class: 'add', text: '+ cursor = encode(last.id)' }), h('span', { class: 'del', text: '- page = int(q["page"])' }))),
      h('div', { class: 'mini-bar' }, h('span', { text: env.name }), h('span', { text: 'main ↑3' }), h('span', { class: 'grow' }), h('span', { text: '14:20' })));
    if (themeName) setVars(el, T.vars(themeName, lk, env.custom ? null : env.id));
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
    const fill = () => NS.setVars(inp, { '--p': String((Number(inp.value) - min) / ((max - min) || 1)) });   // the filled part of the track (settings10.css)
    fill();
    inp.addEventListener('input', () => { fill(); out.textContent = inp.value + unit; T.applyTheme(T.look(Object.assign({}, lookNow(), { [inp.dataset.k]: Number(inp.value) }))); });
    inp.addEventListener('change', () => apply(Number(inp.value)).catch(fail));
    return [inp, out];
  }

  // ---------- look8: the theme cards (ready + own), the own-theme editor, import / export ----------
  const THEME_KEY = { system: 'look4.t_system', dark: 'look4.t_dark', light: 'look4.t_light' };
  const themeLabel = (id, L) => (THEME_KEY[id] ? t(THEME_KEY[id]) : T.BUILTIN.includes(id) ? t('look8.t_' + id) : ((L.themes.find((s) => s.id === id) || {}).name || t('look8.own_name')));
  const sysDark = () => !!(window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches);
  const newId = () => 'u' + Date.now().toString(36) + Math.floor(Math.random() * 1296).toString(36);
  // a colour field: the native picker as a swatch + the hex as text (paste a value); fires on(hex) for every valid change
  function colorField(label, value, on) {
    const pick = h('input', { type: 'color', value, 'aria-label': label });
    const hex = h('input', { type: 'text', class: 'cf8-hex', value, maxLength: 7, spellcheck: false, 'aria-label': label + ' (hex)' });
    pick.addEventListener('input', () => { hex.value = pick.value; hex.removeAttribute('aria-invalid'); on(pick.value); });
    hex.addEventListener('input', () => {
      const v = hex.value.trim().replace(/^([0-9a-f]{6})$/i, '#$1').toLowerCase();
      if (/^#[0-9a-f]{6}$/.test(v)) { hex.removeAttribute('aria-invalid'); pick.value = v; on(v); } else hex.setAttribute('aria-invalid', 'true');
    });
    const box = h('div', { class: 'cf8' }, pick, hex);
    box.set = (v) => { pick.value = v; hex.value = v; hex.removeAttribute('aria-invalid'); };
    return box;
  }
  const ratioText = (r) => t('look8.ratio', { r: (Math.floor(r * 10) / 10).toFixed(1) });

  function themeBlock() {
    const box = h('div', { class: 'th8' });
    let editing = null, pasting = false;
    const file = h('input', { type: 'file', class: 'sr-only', accept: '.json,application/json,text/plain', 'aria-label': t('look8.from_file') });
    const note = h('p', { class: 'th8-note', role: 'alert', hidden: true });
    const say = (txt) => { note.hidden = !txt; note.textContent = txt || ''; };
    async function addSpec(spec) {
      const L = lookNow();
      if (L.themes.length >= T.THEMES_MAX) { say(t('look8.too_many', { n: T.THEMES_MAX })); return; }
      const s = Object.assign({}, spec, { id: newId() });
      await setLook({ themes: L.themes.concat([s]), theme: s.id });
      pasting = false; say(''); draw();
      NS.shell.toast(t('look8.imported', { name: s.name || t('look8.own_name') }));
    }
    function importText(text) {
      const r = T.importTheme(text);
      if (!r.ok) { say(t('look8.bad_' + r.why)); return; }
      addSpec(r.spec).catch(fail);
    }
    file.addEventListener('change', () => {
      const f = file.files && file.files[0];
      file.value = '';
      if (!f) return;
      if (f.size > 64 * 1024) { say(t('look8.bad_format')); return; }
      const rd = new FileReader();
      rd.onload = () => importText(String(rd.result || ''));
      rd.readAsText(f);
    });

    function editor() {
      const w = editing;
      const pid = w.id || 'upreview';
      const live = () => {
        const L = lookNow(), spec = Object.assign({}, w, { id: pid });
        T.applyTheme(T.look(Object.assign({}, L, { themes: L.themes.filter((x) => x.id !== pid).concat([spec]), theme: pid })));
        const r = T.contrast(w.ink, w.panel);
        ratio.textContent = ratioText(r);
        ratio.classList.toggle('low', r < 4.5);
        saveBtn.disabled = r < 4.5;
        inkRow.hint(r < 4.5 ? t('look8.low', { ratio: ratioText(r) }) : '');
      };
      const name = h('input', { type: 'text', class: 'wide th8-name', value: w.name || '', maxLength: 40, placeholder: t('look8.name_ph'), 'aria-label': t('look8.name') });
      name.addEventListener('input', () => { w.name = name.value; });
      const field = (key, label) => colorField(label, w[key], (v) => { w[key] = v; live(); });
      const ratio = h('span', { class: 'th8-ratio', title: t('look8.ratio_d') });
      const inkRow = K().row(t('look8.c_ink'), t('look8.c_ink_d'), ratio, field('ink', t('look8.c_ink')));
      // accent: by environment (default) or an own colour
      const accentPick = colorField(t('look8.c_accent'), w.accent === 'env' ? (T.THEMES[lookNow().theme] || T.THEMES.dark).accent || '#8e95ff' : w.accent, (v) => { w.accent = v; segAcc.redraw('own'); live(); });
      const segAcc = h('div', { class: 'seg', role: 'radiogroup', 'aria-label': t('look8.c_accent') });
      segAcc.redraw = (v) => clear(segAcc).append(...[['env', t('look8.a_env')], ['own', t('look8.a_own')]].map(([val, txt]) => h('button', { type: 'button', role: 'radio', class: v === val ? 'on' : '', 'aria-checked': String(v === val), text: txt,
        onclick: () => { w.accent = val === 'env' ? 'env' : accentPick.querySelector('input[type=color]').value; segAcc.redraw(val); accentPick.classList.toggle('off', val === 'env'); live(); } })));
      segAcc.redraw(w.accent === 'env' ? 'env' : 'own');
      accentPick.classList.toggle('off', w.accent === 'env');
      // the window gradient: on/off + its colour
      const gradPick = colorField(t('look8.c_grad'), w.grad || T.mix(w.bg, w.accent === 'env' ? '#8e95ff' : w.accent, 0.35), (v) => { w.grad = v; gradSw.set(true); gradPick.classList.remove('off'); live(); });
      const gradSw = K().sw(!!w.grad, t('look8.c_grad'), async (on) => { w.grad = on ? gradPick.querySelector('input[type=color]').value : null; gradPick.classList.toggle('off', !on); live(); });
      gradPick.classList.toggle('off', !w.grad);
      const saveBtn = h('button', { type: 'button', class: 'btn primary', text: t('look8.save'), onclick: async () => {
        if (T.contrast(w.ink, w.panel) < 4.5) return;
        const L = lookNow(), spec = T.customSpec(Object.assign({}, w, { id: w.id || newId(), name: name.value || t('look8.own_name') }));
        if (!spec) return;
        const list = L.themes.some((x) => x.id === spec.id) ? L.themes.map((x) => (x.id === spec.id ? spec : x)) : L.themes.concat([spec]);
        if (list.length > T.THEMES_MAX) { say(t('look8.too_many', { n: T.THEMES_MAX })); return; }
        try { await setLook({ themes: list, theme: spec.id }); editing = null; draw(); NS.shell.toast(t('look8.saved', { name: spec.name })); } catch (e) { fail(e); }
      } });
      const cancel = h('button', { type: 'button', class: 'btn', text: t('look8.cancel'), onclick: () => { editing = null; T.applyTheme(lookNow()); draw(); } });
      const json = h('textarea', { class: 'th8-json', rows: 7, readOnly: true, spellcheck: false, hidden: true, 'aria-label': t('look8.export') });
      const exp = h('button', { type: 'button', class: 'btn', onclick: async () => {
        const txt = T.exportTheme(Object.assign({}, w, { name: name.value || t('look8.own_name') }));
        json.value = txt; json.hidden = false; json.select();
        try { await navigator.clipboard.writeText(txt); NS.shell.toast(t('look8.copied')); } catch (e) { /* the text stays selected in the field - ⌘C */ }
      } }, t('look8.export'));
      const del = w.id ? h('button', { type: 'button', class: 'btn danger', text: t('look8.delete'), onclick: async () => {
        if (!(await NS.projects.confirm(t('look8.del_t', { name: w.name || t('look8.own_name') }), t('look8.del_x'), t('look8.delete')))) return;
        const L = lookNow();
        try { await setLook({ themes: L.themes.filter((x) => x.id !== w.id), theme: L.theme === w.id ? 'system' : L.theme }); editing = null; draw(); } catch (e) { fail(e); }
      } }) : null;
      setTimeout(live, 0);
      // the preview is the whole window: leaving Settings (or any redraw) without «Сохранить» puts the saved look back
      const watch = setInterval(() => {
        if (edBox.isConnected) return;
        clearInterval(watch);
        if (editing === w) editing = null;
        if (!document.querySelector('.th8-ed')) T.applyTheme(lookNow());   // another editor took over: its preview stays
      }, 400);
      const edBox = h('div', { class: 'th8-ed', role: 'group', 'aria-label': w.id ? t('look8.ed_edit') : t('look8.ed_new') },
        h('div', { class: 'th8-ed-h' }, h('b', { text: w.id ? t('look8.ed_edit') : t('look8.ed_new') }), h('small', { text: t('look8.ed_d') })),
        h('div', { class: 'srows' },
          K().row(t('look8.name'), '', name),
          K().row(t('look8.c_bg'), t('look8.c_bg_d'), field('bg', t('look8.c_bg'))),
          K().row(t('look8.c_panel'), t('look8.c_panel_d'), field('panel', t('look8.c_panel'))),
          inkRow,
          K().row(t('look8.c_accent'), t('look8.c_accent_d'), segAcc, accentPick),
          K().row(t('look8.c_grad'), t('look8.c_grad_d'), gradPick, gradSw)),
        h('div', { class: 'th8-acts' }, saveBtn, cancel, h('span', { class: 'grow' }), exp, del), json);
      return edBox;
    }

    function draw() {
      const L = lookNow(), sys = sysDark();
      const ids = ['system'].concat(T.BUILTIN, L.themes.map((s) => s.id));
      const opts = ids.map((id) => {
        const real = id === 'system' ? T.resolve('system', sys) : id, tb = T.table(real, L);
        return { value: id, label: themeLabel(id, L), desc: id === 'system' ? t('look8.sys_d') : (tb.custom ? t('look8.own') + ' · ' : '') + t(tb.scheme === 'dark' ? 'look8.dark' : 'look8.light'), preview: mini(real, L) };
      });
      const own = L.themes.find((s) => s.id === L.theme);
      const edit = (spec) => { editing = spec; pasting = false; say(''); draw(); };
      const tools = h('div', { class: 'th8-tools' },
        h('button', { type: 'button', class: 'btn', onclick: () => {
          // a new theme starts from the one on screen: its canvas, panel, text and accent
          const cur = T.current ? T.table(T.current.name, L) : T.THEMES.dark;
          edit({ id: null, name: '', bg: cur.canvas, panel: cur.panel, ink: cur.ink, accent: 'env', grad: cur.grad || null });
        } }, ic('i-plus'), t('look8.create')),
        own ? h('button', { type: 'button', class: 'btn', text: t('look8.edit', { name: own.name || t('look8.own_name') }), onclick: () => edit(Object.assign({}, own)) }) : null,
        h('span', { class: 'grow' }),
        h('button', { type: 'button', class: 'btn', text: t('look8.import'), 'aria-expanded': String(pasting), onclick: () => { pasting = !pasting; editing = null; say(''); draw(); } }));
      let paste = null;
      if (pasting) {
        const area = h('textarea', { class: 'th8-json', rows: 6, spellcheck: false, placeholder: '{ "nightshift_theme": 1, "name": "…", "bg": "#101014", … }', 'aria-label': t('look8.paste') });
        paste = h('div', { class: 'th8-paste' }, h('small', { text: t('look8.paste') }), area,
          h('div', { class: 'th8-acts' }, h('button', { type: 'button', class: 'btn primary', text: t('look8.paste_ok'), onclick: () => importText(area.value) }),
            h('button', { type: 'button', class: 'btn', text: t('look8.from_file'), onclick: () => file.click() }),
            h('button', { type: 'button', class: 'btn', text: t('look8.cancel'), onclick: () => { pasting = false; say(''); draw(); } })));
        setTimeout(() => area.focus(), 0);
      }
      const grid = cards(t('look4.theme'), opts, L.theme, async (v) => { editing = null; await setLook({ theme: v }); draw(); });
      grid.classList.add('th8-cards');
      clear(box).append(...[grid, tools, note, file, paste, editing ? editor() : null].filter(Boolean));
    }
    draw();
    return box;
  }

  // ---------- look8: the window - transparency, the macOS blur, pane opacity, accent gradients ----------
  NS.api.COMMANDS.add('window_glass');
  const isMac = () => (K() && K().platform ? K().platform() : 'linux') === 'mac';
  // the blur under a see-through window (app: window_glass -> Tauri set_effects on macOS); asked only when the want flips
  let blurOn = false;
  T.onTheme((cur) => {
    const want = cur.look.glass > 0 && cur.look.blur && isMac() && !document.body.classList.contains('t-window');
    if (want === blurOn || !window.__TAURI__) return;
    blurOn = want;
    NS.api.call('window_glass', { blur: want }).catch(() => {});   // dev-mock and an old app answer "unknown command": the page stays as it is
  });
  function windowGroup(L0) {
    const k = K();
    const [glIn, glOut] = range(t('look8.glass'), L0.glass, 0, T.GLASS_MAX, '%', (v) => setLook({ glass: v })); glIn.dataset.k = 'glass';
    const [opIn, opOut] = range(t('look4.opacity'), L0.opacity, 70, 100, '%', (v) => setLook({ opacity: v })); opIn.dataset.k = 'opacity';
    const rows = [
      k.row({ text: t('look8.glass'), icon: 'l-blend' }, t('look8.glass_d'), glIn, glOut),
      isMac() ? k.toggleRow({ icon: 'l-droplet', title: t('look8.blur'), desc: t('look8.blur_d'), on: L0.blur, apply: (v) => setLook({ blur: v }) }) : null,
      k.row({ text: t('look4.opacity'), icon: 'l-eye' }, t('look8.opacity_d'), opIn, opOut),
      k.toggleRow({ icon: 'l-sparkles', title: t('look8.fx'), desc: t('look8.fx_d'), on: L0.fx, apply: (v) => setLook({ fx: v }) }),
    ].filter(Boolean);
    return k.group(t('look8.window'), rows, null, 'i-window');
  }

  // "Скиллы и MCP во всех агентах" (Kits::spread): the switch (settings.spread_kits), under it what the last spread did,
  // "Разложить сейчас" and "Откатить последнюю"
  const fmtAt = (iso) => { try { return new Date(iso).toLocaleString(NS.lang() === 'en' ? 'en-GB' : 'ru-RU', { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }); } catch (e) { return iso; } };
  function spreadRows(k) {
    const sum = h('small', { class: 'hint', role: 'status' });
    // what was skipped and why, folded: «пропущено 6» alone said nothing
    const why = h('details', { class: 'spread-why', hidden: true });
    const reason = (c) => { const k = 'spread.r_' + c, v = t(k); return v === k ? c : v; };
    let show = (r) => {
      sum.textContent = !r ? t('spread.none') : t(r.undone ? 'spread.undone' : 'spread.last', { date: fmtAt(r.at), n: r.added.length, skip: r.skipped.length });
      const sk = (r && !r.undone && r.skipped) || [];
      why.hidden = !sk.length;
      clear(why).append(h('summary', { text: t('spread.why') }), h('ul', {}, sk.map((x) => h('li', {}, h('b', { text: x.name === '*' ? t('spread.all_' + x.kind) : x.name }), ' · ' + ((NS.store.env(x.env) || {}).name || x.env) + ' · ' + reason(x.reason)))));
    };
    const load = () => NS.api.call('kits_spread_last').then(show).catch(() => show(null));
    const run = async () => { try { show(await NS.api.call('kits_spread')); } catch (e) { fail(e); } };
    // an undone spread came back on the next start (the switch stayed on): the undo turns the automatic one off too
    const undo = async () => {
      try { await NS.api.call('kits_spread_undo'); await k.commit({ spread_kits: false }); sw.sw && sw.sw.set && sw.sw.set(false); NS.shell.toast(t('spread.undo_off')); load(); } catch (e) { fail(e); }
    };
    const undoBtn = h('button', { type: 'button', class: 'btn', onclick: undo, text: t('spread.undo') });
    const show0 = show;
    show = (r) => { show0(r); undoBtn.hidden = !r || !!r.undone || !(r.added || []).length; };
    load();
    const sw = k.toggleRow({ icon: 'l-package', title: t('spread.title'), desc: t('spread.desc'), on: (S.settings || {}).spread_kits !== false,
      apply: async (v) => { await k.commit({ spread_kits: v }); if (v) run(); } });
    return [
      sw,
      h('div', { class: 'srow spread-sub' }, h('div', {}, sum, why), h('div', { class: 'ctl' },
        h('button', { type: 'button', class: 'btn', onclick: run, text: t('spread.now') }), undoBtn)),
    ];
  }

  // v9 (Agentation 28.10): "Основное" keeps the layout, language and Clawd; everything about colour lives in "Внешний вид"
  function basic(env, fx) {
    const L0 = lookNow(), k = K();
    const presetOpts = ['hypr', 'minimal', 'classic'].map((id) => {
      const p = T.PRESETS[id], pv = mini(null);
      pv.classList.add('preset-' + id);
      setVars(pv, { '--radius-pane': T.RADIUS[p.radius][0] + 'px', '--radius-sm': T.RADIUS[p.radius][1] + 'px', '--gap': p.gap + 'px' });
      return { value: id, label: t('look4.p_' + id), desc: t('look4.p_' + id + '_d'), preview: pv };
    });
    const lookS = (S.settings && S.settings.look) || {}, langNow = (S.settings || {}).language || 'auto';
    const preview = h('div', { class: 'preview4' }, h('small', { text: t('look4.preview') }), mini(null));
    return [
      h('div', { class: 'basic4' },
        h('div', { class: 'basic4-l' },
          k.group(t('look4.preset'), [cards(t('look4.preset'), presetOpts, L0.preset, applyPreset)], t('look9.preset_d')),
          k.group(t('look9.general'), [
            k.row({ text: t('set.v.lang'), icon: 'l-languages' }, '', k.segment([['auto', t('set.v.auto')], ['ru', 'Русский'], ['en', 'English']], langNow, t('set.v.lang'), (v) => fx.ctx.setLang(v))),
            k.toggleRow({ icon: 'l-type', title: t('set.v.labels'), desc: t('set.v.labels_d'), on: lookS.role_labels, apply: (v) => k.commit({ look: Object.assign({}, S.settings.look, { role_labels: v }) }) }),
            k.row({ text: t('look4.clawd'), icon: 'l-bot' }, t('look4.clawd_d'), k.segment([['animate', t('look4.c_animate')], ['static', t('look4.c_static')], ['hide', t('look4.c_hide')]], L0.clawd, t('look4.clawd'), (v) => setLook({ clawd: v }))),
            ...spreadRows(k),
            (S.notices || {}).trayMissing ? h('div', { class: 'tray-line' }, ic('i-warn'), h('span', { text: t('set.v.tray') })) : null,
          ].filter(Boolean)),
          h('button', { type: 'button', class: 'goto9', onclick: () => fx.ctx.nav('settings', { env: env.id, section: 'look' }) }, ic('i-palette'),
            h('span', {}, h('b', { text: t('look9.goto') }), h('small', { text: t('look9.goto_d', { theme: themeLabel(L0.theme, L0) }) })), ic('i-chev-r'))),
        preview),
    ];
  }

  // v9: "Внешний вид" - the customization panel: ready themes, the selection colour, an own theme, the window, type and shape
  const ACCENTS = ['env', '#7aa2f7', '#4fa3e0', '#3fb8af', '#5cb176', '#c3c95a', '#e0af68', '#ef8a5b', '#e5675d', '#ec7fae', '#a98bf0', '#9aa3b5'];
  function accentPicker(L0) {
    const box = h('div', { class: 'acc9', role: 'radiogroup', 'aria-label': t('look9.accent') });
    const own = h('input', { type: 'color', class: 'acc9-own', 'aria-label': t('look9.accent_own'), value: L0.accent !== 'env' ? L0.accent : '#7aa2f7' });
    const pickIt = async (c, prev) => { draw(c); try { await setLook({ accent: c }); } catch (e) { draw(prev); fail(e); } };
    own.addEventListener('change', () => pickIt(own.value.toLowerCase(), lookNow().accent));
    function draw(v) {
      const custom = v !== 'env' && !ACCENTS.includes(v);
      clear(box).append(...ACCENTS.map((c) => {
        const b = h('button', { type: 'button', role: 'radio', class: 'acc9-s' + (c === v ? ' on' : '') + (c === 'env' ? ' env' : ''), 'aria-checked': String(c === v),
          'aria-label': c === 'env' ? t('look9.a_env') : c, title: c === 'env' ? t('look9.a_env_d') : c, onclick: () => pickIt(c, v) }, c === 'env' ? h('span', { text: t('look9.a_env') }) : null);
        if (c !== 'env') setVars(b, { '--c': c });
        return b;
      }), h('label', { class: 'acc9-s acc9-plus' + (custom ? ' on' : ''), title: t('look9.accent_own') }, custom ? null : ic('i-plus'), own));
      if (custom) setVars(box.lastChild, { '--c': v });
    }
    draw(L0.accent);
    return box;
  }
  function appearance(env, fx) {
    const L0 = lookNow(), k = K();
    const fontSel = h('select', { 'aria-label': t('look4.font') }, Object.keys(T.FONTS).map((f) => h('option', { value: f, selected: f === L0.font, text: f === 'system' ? t('look4.f_system') : T.FONTS[f].split(',')[0].replace(/"/g, '') })));
    fontSel.onchange = () => setLook({ font: fontSel.value }).catch(fail);
    const [gapIn, gapOut] = range(t('look4.gap'), L0.gap, 0, 16, 'px', (v) => setLook({ gap: v })); gapIn.dataset.k = 'gap';
    return [
      k.group(t('look9.themes'), [themeBlock()], t('look9.themes_d'), 'i-palette'),
      k.group(t('look9.accent'), [accentPicker(L0)], t('look9.accent_d'), 'i-drop'),
      windowGroup(L0),
      k.group(t('look9.type'), [
        k.row({ text: t('look4.font'), icon: 'l-type' }, t('look4.font_d'), fontSel),
        k.numRow({ icon: 'l-a-large-small', title: t('look4.size'), desc: t('set.v.font_d'), value: L0.font_px, unit: 'px', int: true, rule: { min: 11, max: 20, int: true }, apply: (v) => setLook({ font_px: v }) }),
        k.row({ text: t('look4.radius'), icon: 'l-square-round-corner' }, '', k.segment([['sharp', t('look4.r_sharp')], ['soft', t('look4.r_soft')], ['round', t('look4.r_round')]], L0.radius, t('look4.radius'), (v) => setLook({ radius: v }))),
        k.row({ text: t('look4.gap'), icon: 'l-between-horizontal-start' }, '', gapIn, gapOut),
      ], null, 'i-type'),
    ];
  }
  // the v2 section "Внешний вид" (settings.js) is drawn by this panel now
  if (K() && K().SEC && K().SEC.look) Object.assign(K().SEC.look, { build: appearance });

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
    // Esc: a typed query is cleared first, an empty field leaves the settings as Esc does in every other section
    const search = h('input', window.NSPalette.quiet({ type: 'search', class: 'adv-q', placeholder: t('adv.search'), 'aria-label': t('adv.search'),
      onkeydown: (e) => { if (e.key === 'Escape' && !search.value && !document.querySelector('dialog[open]') && NS.shell.back) { e.preventDefault(); NS.shell.back(); } } }));
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
  ['system'].concat(T.BUILTIN).forEach((v, i) =>
    cmd({ id: 'look.theme.' + v, get label() { return t('cmd.theme', { name: themeLabel(v, lookNow()) }); }, group: 'cmd.g.view', order: 70 + i * 0.1, run: () => setLook({ theme: v }).catch(fail) }));
  cmd({ id: 'look.fx', get label() { return t('look8.cmd_fx'); }, group: 'cmd.g.view', order: 71, run: () => setLook({ fx: !lookNow().fx }).catch(fail) });
  ['hypr', 'minimal', 'classic'].forEach((id, i) => cmd({ id: 'look.preset.' + id, get label() { return t('cmd.preset', { name: t('look4.p_' + id) }); }, group: 'cmd.g.view', order: 75 + i, run: () => applyPreset(id).catch(fail) }));
  ['animate', 'static', 'hide'].forEach((m, i) => cmd({ id: 'look.clawd.' + m, get label() { return t('cmd.clawd', { mode: t('look4.c_' + m) }); }, group: 'cmd.g.view', order: 80 + i, run: () => setLook({ clawd: m }).catch(fail) }));
  cmd({ id: 'config.open', labelKey: 'cmd.config', order: 62, run: openConfig });

  // config-changed {errors}: the file was edited by hand - take the settings again, show the errors (story 294)
  NS.api.listen('config-changed', async (pl) => {
    configErrors = (pl && pl.errors) || [];
    try { S.settings = await NS.api.call('get_settings'); K().applyLook(); NS.store.emit('settings', null); } catch (e) { /* the old settings stay */ }
  }).catch(() => {});
  NS.settingsV4 = { setLook, applyPreset, lookNow, openConfig, themeLabel };
})();
