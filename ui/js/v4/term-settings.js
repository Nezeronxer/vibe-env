// v4 terminal settings (stories 255-257): ready themes, own colours, import of Ghostty / kitty / iTerm2 themes, font, cursor,
// history, padding; the section "Терминал" of the settings (registry point 'settings'). The chosen terminal theme paints the whole
// window (257): after theme.js writes its variables, ours go over them. Pure part (parse, normalize, vars) runs in selftest.
// Kept in settings.ui.tabs.look.term (the free-form JSON that survives the layout saves of tabs.js); a config key of task 20
// writes the same object.
(function (root) {
  // values taken from the official repositories by the import below (sources: docs/themes-sources.md)
  const THEMES = {
    'catppuccin-mocha': { name: 'Catppuccin Mocha', scheme: 'dark', background: '#1e1e2e', foreground: '#cdd6f4', cursor: '#f5e0dc', selection: '#f5e0dc',
      ansi: ['#45475a', '#f38ba8', '#a6e3a1', '#f9e2af', '#89b4fa', '#f5c2e7', '#94e2d5', '#bac2de', '#585b70', '#f38ba8', '#a6e3a1', '#f9e2af', '#89b4fa', '#f5c2e7', '#94e2d5', '#a6adc8'] },
    'tokyo-night': { name: 'Tokyo Night', scheme: 'dark', background: '#1a1b26', foreground: '#c0caf5', cursor: '#c0caf5', selection: '#283457',
      ansi: ['#15161e', '#f7768e', '#9ece6a', '#e0af68', '#7aa2f7', '#bb9af7', '#7dcfff', '#a9b1d6', '#414868', '#ff899d', '#9fe044', '#faba4a', '#8db0ff', '#c7a9ff', '#a4daff', '#c0caf5'] },
    'gruvbox-dark': { name: 'Gruvbox Dark', scheme: 'dark', background: '#282828', foreground: '#ebdbb2', cursor: '#ebdbb2', selection: '#504945',
      ansi: ['#282828', '#cc241d', '#98971a', '#d79921', '#458588', '#b16286', '#689d6a', '#a89984', '#928374', '#fb4934', '#b8bb26', '#fabd2f', '#83a598', '#d3869b', '#8ec07c', '#ebdbb2'] },
    'dracula': { name: 'Dracula', scheme: 'dark', background: '#282a36', foreground: '#f8f8f2', cursor: '#f8f8f2', selection: '#44475a',
      ansi: ['#21222c', '#ff5555', '#50fa7b', '#f1fa8c', '#bd93f9', '#ff79c6', '#8be9fd', '#f8f8f2', '#6272a4', '#ff6e6e', '#69ff94', '#ffffa5', '#d6acff', '#ff92df', '#a4ffff', '#ffffff'] },
    'solarized-light': { name: 'Solarized Light', scheme: 'light', background: '#fdf6e3', foreground: '#657b83', cursor: '#586e75', selection: '#eee8d5',
      ansi: ['#073642', '#dc322f', '#859900', '#b58900', '#268bd2', '#d33682', '#2aa198', '#eee8d5', '#002b36', '#cb4b16', '#586e75', '#657b83', '#839496', '#6c71c4', '#93a1a1', '#fdf6e3'] },
    'one-dark': { name: 'One Dark', scheme: 'dark', background: '#282c34', foreground: '#abb2bf', cursor: '#abb2bf', selection: '#3e4452',
      ansi: ['#2c323c', '#e06c75', '#98c379', '#e5c07b', '#61afef', '#c678dd', '#56b6c2', '#5c6370', '#3e4452', '#e06c75', '#98c379', '#e5c07b', '#61afef', '#c678dd', '#56b6c2', '#abb2bf'] },
  };

  // ---------- import: Ghostty (`key = value`), kitty (`key value`), iTerm2 (.itermcolors plist) ----------
  const hex = (v) => {
    const m = /^#?([0-9a-f]{6}|[0-9a-f]{3})$/i.exec(String(v || '').trim());
    if (!m) return null;
    const s = m[1].length === 3 ? m[1].replace(/./g, (c) => c + c) : m[1];
    return '#' + s.toLowerCase();
  };
  const lum = (c) => { const n = parseInt(c.slice(1), 16); return (0.299 * (n >> 16) + 0.587 * ((n >> 8) & 255) + 0.114 * (n & 255)) / 255; };
  // -> { ok: true, theme } | { ok: false, error: 'not_theme' }; nothing is changed by a refusal
  function parseTheme(text, name) {
    text = String(text || '');
    const th = { name: name || '', background: null, foreground: null, cursor: null, selection: null, ansi: new Array(16).fill(null) };
    if (/<plist[\s>]/.test(text) && /<key>\s*Ansi \d+ Color\s*<\/key>/.test(text)) {
      const re = /<key>\s*([^<]+?)\s*<\/key>\s*<dict>([\s\S]*?)<\/dict>/g;
      let m;
      while ((m = re.exec(text))) {
        const comp = (k) => { const x = new RegExp('<key>\\s*' + k + ' Component\\s*</key>\\s*<real>\\s*([0-9.eE+-]+)\\s*</real>').exec(m[2]); return x ? Math.max(0, Math.min(255, Math.round(parseFloat(x[1]) * 255))) : null; };
        const [r, g, b] = ['Red', 'Green', 'Blue'].map(comp);
        if (r == null || g == null || b == null) continue;
        const c = '#' + [r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('');
        const key = m[1], a = /^Ansi (\d+) Color$/.exec(key);
        if (a && +a[1] < 16) th.ansi[+a[1]] = c;
        else if (key === 'Background Color') th.background = c;
        else if (key === 'Foreground Color') th.foreground = c;
        else if (key === 'Cursor Color') th.cursor = c;
        else if (key === 'Selection Color') th.selection = c;
      }
    } else {
      for (const raw of text.split(/\r?\n/)) {
        const line = raw.replace(/\s+#(?![0-9a-f]{3,6}\b).*$/i, '').trim();
        if (!line || line[0] === '#') continue;
        const g = /^([\w-]+)\s*=\s*(.+)$/.exec(line), k = g ? null : /^([\w-]+)\s+(\S+)$/.exec(line);
        const kv = g || k;
        if (!kv) continue;
        const key = kv[1].toLowerCase(), val = kv[2].trim();
        if (g && key === 'palette') { const p = /^(\d+)\s*=\s*(\S+)$/.exec(val); if (p && +p[1] < 16 && hex(p[2])) th.ansi[+p[1]] = hex(p[2]); continue; }
        const c = hex(val);
        if (!c) continue;
        const col = /^color(\d+)$/.exec(key);
        if (col && +col[1] < 16) th.ansi[+col[1]] = c;
        else if (key === 'background') th.background = c;
        else if (key === 'foreground') th.foreground = c;
        else if (key === 'cursor' || key === 'cursor-color') th.cursor = c;
        else if (key === 'selection_background' || key === 'selection-background') th.selection = c;
      }
    }
    if (!th.background || !th.foreground || th.ansi.slice(0, 8).some((c) => !c)) return { ok: false, error: 'not_theme' };
    for (let i = 8; i < 16; i++) th.ansi[i] = th.ansi[i] || th.ansi[i - 8];
    th.cursor = th.cursor || th.foreground;
    // a selection the colour of the text or the ground would hide the text (One Dark's .itermcolors has it so)
    if (!th.selection || th.selection === th.foreground || th.selection === th.background) th.selection = th.ansi[8];
    th.scheme = lum(th.background) < 0.5 ? 'dark' : 'light';
    return { ok: true, theme: th };
  }

  // ---------- the person's choices (settings.ui.tabs.look.term) ----------
  const CURSORS = ['block', 'bar', 'underline'];
  const TERM0 = { theme: 'nightshift', custom: null, font: '', size: 0, line_height: 1.15, ligatures: false, cursor: 'block', blink: true,
    scrollback: 10000, padding: 10, copy_select: true, osc52: false, notify_secs: 10, external: false, global_key: 'Ctrl+`', prefix: '' };
  const num = (v, lo, hi, d) => { const n = Number(v); return Number.isFinite(n) ? Math.max(lo, Math.min(hi, n)) : d; };
  function validTheme(x) {
    if (!x || typeof x !== 'object' || !hex(x.background) || !hex(x.foreground) || !Array.isArray(x.ansi) || x.ansi.length !== 16 || x.ansi.some((c) => !hex(c))) return null;
    return { name: String(x.name || '').slice(0, 60), scheme: lum(hex(x.background)) < 0.5 ? 'dark' : 'light', background: hex(x.background), foreground: hex(x.foreground),
      cursor: hex(x.cursor) || hex(x.foreground), selection: hex(x.selection) || hex(x.ansi[8]), ansi: x.ansi.map(hex) };
  }
  function termLook(raw) {
    const o = Object.assign({}, TERM0, raw || {});
    o.custom = validTheme(o.custom);
    if (!(o.theme === 'nightshift' || (o.theme === 'custom' && o.custom) || THEMES[o.theme])) o.theme = 'nightshift';
    o.font = typeof o.font === 'string' ? o.font.replace(/[;{}<>]/g, '').slice(0, 80) : '';
    o.size = o.size ? Math.round(num(o.size, 9, 28, 0)) : 0;               // 0 = the size of "Основное"
    o.line_height = Math.round(num(o.line_height, 1, 2, TERM0.line_height) * 100) / 100;
    if (!CURSORS.includes(o.cursor)) o.cursor = TERM0.cursor;
    o.scrollback = Math.round(num(o.scrollback, 1000, 100000, TERM0.scrollback));
    o.padding = Math.round(num(o.padding, 0, 24, TERM0.padding));
    o.notify_secs = Math.round(num(o.notify_secs, 0, 3600, TERM0.notify_secs));
    for (const k of ['ligatures', 'blink', 'copy_select', 'osc52', 'external']) o[k] = !!o[k];
    o.global_key = typeof o.global_key === 'string' ? o.global_key.trim().slice(0, 40) : TERM0.global_key;
    o.prefix = typeof o.prefix === 'string' ? o.prefix.trim().toLowerCase().slice(0, 20) : '';
    return o;
  }
  const fromSettings = (st) => termLook((((st && st.ui && st.ui.tabs) || {}).look || {}).term);
  // the theme object in use: null = Nightshift (theme.js decides)
  const themeOf = (L) => (L.theme === 'custom' ? L.custom : L.theme === 'nightshift' ? null : THEMES[L.theme]);

  // ---------- one table -> xterm and the variables of the whole window ----------
  const ANSI_NAMES = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white'];
  function xterm(th) {
    const o = { background: th.background, foreground: th.foreground, cursor: th.cursor, cursorAccent: th.background, selectionBackground: th.selection };
    th.ansi.forEach((c, i) => { o[i >= 8 ? 'bright' + ANSI_NAMES[i - 8][0].toUpperCase() + ANSI_NAMES[i - 8].slice(1) : ANSI_NAMES[i]] = c; });
    return o;
  }
  const rgb = (c) => { const n = parseInt(c.slice(1), 16); return [n >> 16, (n >> 8) & 255, n & 255]; };
  const mix = (a, b, w) => '#' + rgb(a).map((x, i) => Math.round(x + (rgb(b)[i] - x) * w).toString(16).padStart(2, '0')).join('');
  // the surfaces are steps from the background toward the text: no colour of its own, the theme's tone throughout
  function appVars(th, accent) {
    const bg = th.background, fg = th.foreground, dark = th.scheme === 'dark';
    const v = {
      '--canvas': mix(bg, dark ? '#000000' : fg, dark ? 0.35 : 0.1), '--panel': bg, '--raised': mix(bg, fg, 0.06), '--raised2': mix(bg, fg, 0.12),
      '--line': mix(bg, fg, 0.12), '--line2': mix(bg, fg, 0.2), '--ink': fg, '--ink2': mix(fg, bg, 0.25), '--ink3': mix(fg, bg, 0.42),
      '--accent': accent || th.ansi[4], '--cursor': th.cursor, '--sel': th.selection, '--btn': fg, '--on-btn': bg, '--pane-bg': bg,
    };
    v['--bg'] = v['--canvas'];
    th.ansi.forEach((c, i) => { v['--ansi-' + i] = c; });
    return v;
  }

  const PURE = { THEMES, TERM0, parseTheme, termLook, fromSettings, themeOf, xterm, appVars, validTheme, hex };
  root.NSTermSettings = PURE;
  if (typeof document === 'undefined' || !root.NS || !root.NS.h) return;

  // ---------- the window: the terminal theme over the variables of theme.js (257) ----------
  const NS = root.NS;
  const { h, clear, setVars } = NS;
  const t = (k, p) => NS.t(k, p);
  const S = () => NS.store.s;
  const T = root.NSTheme;
  const lookNow = () => fromSettings(S().settings);
  const rgba = (c, a) => 'rgba(' + rgb(c).join(', ') + ', ' + a + ')';
  T.onTheme((cur) => {
    const th = themeOf(lookNow());
    if (!th) return;
    const el = document.documentElement, baseAccent = (T.THEMES[cur.name] || {}).accent, a = el.style.getPropertyValue('--accent').trim();
    const v = appVars(th, a && a !== baseAccent ? a : null);      // an env colour or the person's accent stays; else the theme's blue
    if (cur.look.opacity < 100) v['--pane-bg'] = rgba(th.background, cur.look.opacity / 100);
    for (const [k, x] of Object.entries(v)) el.style.setProperty(k, x);
    el.setAttribute('data-theme', th.scheme);
    el.style.setProperty('color-scheme', th.scheme);
  });
  // the Nightshift table in the shape of a terminal theme (the start of "свои цвета")
  const nightshift = () => { const n = T.current ? T.current.name : 'dark', x = T.THEMES[n]; return { name: x.name, scheme: x.scheme, background: x.panel, foreground: x.ink, cursor: x.cursor, selection: x.selection, ansi: x.ansi.slice() }; };
  const save = (patch) => NS.settingsV4.setLook({ term: termLook(Object.assign({}, lookNow(), patch)) });
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);

  // a miniature terminal in a theme: prompt, a command, coloured output
  function mini(th) {
    const line = (...parts) => h('span', { class: 'tm-l' }, parts);
    const c = (i, s) => h('b', { class: 'tm-c', dataset: { c: String(i) }, text: s });
    const el = h('div', { class: 'tmini', 'aria-hidden': 'true' },
      line(c(4, '~/invoices'), ' ', c(5, 'main'), ' % cargo test'),
      line(c(2, 'ok'), ' 12 passed; ', c(1, '1 failed')),
      line(c(3, 'warning'), ': unused ', c(6, 'cursor')),
      line('% ', h('i', { class: 'tm-cur' })));
    const vars = { '--tm-bg': th.background, '--tm-fg': th.foreground, '--tm-cur': th.cursor };
    th.ansi.forEach((x, i) => { vars['--tm-' + i] = x; });
    return setVars(el, vars);
  }

  function build() {
    const k = NS.settingsKit, L0 = lookNow();
    // ---- theme cards ----
    const ids = ['nightshift'].concat(Object.keys(THEMES), L0.custom ? ['custom'] : []);
    const cards = h('div', { class: 'cards4 tcards', role: 'radiogroup', 'aria-label': t('termset.theme') });
    const thOf = (id) => (id === 'nightshift' ? nightshift() : id === 'custom' ? lookNow().custom : THEMES[id]);
    const nameOf = (id) => (id === 'nightshift' ? t('termset.t_nightshift') : id === 'custom' ? (lookNow().custom.name || t('termset.t_custom')) : THEMES[id].name);
    const drawCards = (v) => clear(cards).append(...ids.filter((id) => thOf(id)).map((id) => h('button', { type: 'button', role: 'radio', class: 'card4' + (id === v ? ' on' : ''), 'aria-checked': String(id === v),
      onclick: async () => { drawCards(id); try { await save({ theme: id }); drawColors(); } catch (e) { drawCards(v); fail(e); } } },
    mini(thOf(id)), h('span', { class: 'card4-t' }, h('b', { text: nameOf(id) }), h('small', { text: id === 'nightshift' ? t('termset.t_follow') : t('termset.scheme_' + thOf(id).scheme) })))));
    drawCards(L0.theme);

    // ---- own colours: ground, text, cursor, selection, 16 ANSI; live preview, saved on change ----
    const colors = h('div', { class: 'tcolors' });
    const SLOTS = [['background', 'termset.c_bg'], ['foreground', 'termset.c_fg'], ['cursor', 'termset.c_cursor'], ['selection', 'termset.c_sel']]
      .concat([...Array(16).keys()].map((i) => ['ansi.' + i, 'termset.c_' + i]));
    let work = null;
    function drawColors() {
      const cur = themeOf(lookNow()) || nightshift();
      work = JSON.parse(JSON.stringify(cur));
      if (preview && preview.isConnected) { const n = mini(cur); preview.replaceWith(n); preview = n; }
      clear(colors).append(...SLOTS.map(([slot, key]) => {
        const get = () => (slot.startsWith('ansi.') ? work.ansi[+slot.slice(5)] : work[slot]);
        const inp = h('input', { type: 'color', value: get(), 'aria-label': t(key) });
        inp.addEventListener('input', () => {
          if (slot.startsWith('ansi.')) work.ansi[+slot.slice(5)] = inp.value; else work[slot] = inp.value;
          const th = validTheme(Object.assign({}, work, { name: t('termset.t_custom') }));
          if (th) { preview.replaceWith(preview = mini(th)); }
        });
        inp.addEventListener('change', () => { const th = validTheme(Object.assign({}, work, { name: t('termset.t_custom') })); if (th) save({ theme: 'custom', custom: th }).then(() => { if (!ids.includes('custom')) ids.push('custom'); drawCards('custom'); }).catch(fail); });
        return h('label', { class: 'tcol' }, inp, h('span', { text: t(key) }));
      }));
    }
    let preview = mini(themeOf(L0) || nightshift());
    drawColors();

    // ---- import ----
    const file = h('input', { type: 'file', class: 'sr-only', accept: '.itermcolors,.conf,.config,.txt,text/plain,application/xml', 'aria-label': t('termset.import') });
    const importRow = k.row(t('termset.import'), t('termset.import_d'), h('button', { type: 'button', class: 'btn', text: t('termset.import_btn'), onclick: () => file.click() }), file);
    file.addEventListener('change', () => {
      const f = file.files && file.files[0];
      if (!f) return;
      if (f.size > 512 * 1024) { importRow.hint(t('termset.not_theme')); return; }
      const rd = new FileReader();
      rd.onload = async () => {
        const r = parseTheme(String(rd.result || ''), f.name.replace(/\.[^.]+$/, ''));
        file.value = '';
        if (!r.ok) { importRow.hint(t('termset.not_theme')); return; }
        importRow.hint('');
        try { await save({ theme: 'custom', custom: r.theme }); if (!ids.includes('custom')) ids.push('custom'); drawCards('custom'); drawColors(); NS.shell.toast(t('termset.imported', { name: r.theme.name || t('termset.t_custom') })); } catch (e) { fail(e); }
      };
      rd.readAsText(f);
    });

    const num = (title, desc, value, rule, key, unit) => k.numRow({ title, desc, value, unit: unit || '', int: !!rule.int, rule, apply: (v) => save({ [key]: v }) });
    const toggle = (title, desc, key) => k.toggleRow({ title, desc, on: L0[key], apply: (v) => save({ [key]: v }) });
    const fontList = h('datalist', { id: 'tfonts' }, Object.values(T.FONTS).map((f) => h('option', { value: f.split(',')[0].replace(/"/g, '') })));
    const fontRow = k.textRow({ title: t('termset.font'), desc: t('termset.font_d'), value: L0.font, placeholder: t('termset.font_ph'), extra: fontList, apply: (v) => save({ font: v }) });
    fontRow.input.setAttribute('list', 'tfonts');
    return [
      h('div', { class: 'basic4' },
        h('div', { class: 'basic4-l' },
          k.group(t('termset.theme'), [cards], t('termset.theme_d')),
          k.group(t('termset.colors'), [colors], t('termset.colors_d')),
          k.group('', [importRow]),
          k.group(t('termset.text'), [
            fontRow,
            num(t('termset.size'), t('termset.size_d'), L0.size, { min: 0, max: 28, int: true }, 'size', 'px'),
            num(t('termset.line'), '', L0.line_height, { min: 1, max: 2 }, 'line_height'),
            toggle(t('termset.lig'), t('termset.lig_d'), 'ligatures'),
            k.row(t('termset.cursor'), '', k.segment([['block', t('termset.cur_block')], ['bar', t('termset.cur_bar')], ['underline', t('termset.cur_under')]], L0.cursor, t('termset.cursor'), (v) => save({ cursor: v }))),
            toggle(t('termset.blink'), '', 'blink'),
            num(t('termset.scroll'), t('termset.scroll_d'), L0.scrollback, { min: 1000, max: 100000, int: true }, 'scrollback', t('termset.lines')),
            num(t('termset.pad'), '', L0.padding, { min: 0, max: 24, int: true }, 'padding', 'px'),
          ]),
          k.group(t('termset.behaviour'), [
            toggle(t('termset.copy_sel'), t('termset.copy_sel_d'), 'copy_select'),
            toggle(t('termset.osc52'), t('termset.osc52_d'), 'osc52'),
            num(t('termset.notify'), t('termset.notify_d'), L0.notify_secs, { min: 0, max: 3600, int: true }, 'notify_secs', t('termset.sec')),
            toggle(t('termset.external'), t('termset.external_d'), 'external'),
            k.textRow({ title: t('termset.global'), desc: t('termset.global_d'), value: L0.global_key, placeholder: 'Ctrl+`', apply: (v) => save({ global_key: v }) }),
            k.textRow({ title: t('termset.prefix'), desc: t('termset.prefix_d'), value: L0.prefix, placeholder: 'ctrl+b',
              check: (v) => (!v.trim() || /^(ctrl|alt)\+[a-z]$/i.test(v.trim()) ? { ok: true, value: v.trim().toLowerCase() } : { ok: false, hint: t('termset.prefix_bad') }), apply: (v) => save({ prefix: v }) }),
          ])),
        h('div', { class: 'preview4 tpreview' }, h('small', { text: t('look4.preview') }), preview)),
    ];
  }
  NS.registry.extend('settings', { id: 'terminal', level: 'basic', labelKey: 'set.nav.terminal', order: 2, build });
}(typeof window !== 'undefined' ? window : globalThis));
