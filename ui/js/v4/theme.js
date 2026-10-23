// v4 theme (stories 243-247, 255, 295): one table per theme -> the CSS variables of every screen and the theme object of xterm.
// Pure part (NSTheme.vars / xterm / resolve / look) runs in selftest; applyTheme(look) writes the variables on <html>.
// tokens.css carries the same Dark and Light values for the first paint; selftest checks the two never drift apart.
(function (root) {
  // ANSI order: black red green yellow blue magenta cyan white, then the bright eight. Muted, no neon.
  const THEMES = {
    dark: {
      name: 'Nightshift Dark', scheme: 'dark',
      canvas: '#1f160e', panel: '#130e09', raised: '#261c13', raised2: '#32261a', line: '#392d1f', line2: '#5a4830', bg: '#0f0b08',
      ink: '#f3eadb', ink2: '#d6c9b0', ink3: '#ad9e82', accent: '#f2a33a', cursor: '#f3eadb', selection: '#4d3a1c',
      human: '#f6b445', humanBg: '#2d2110', humanLine: '#6e5120', onHuman: '#1d1405',
      ok: '#86cb98', okBg: '#14241a', bad: '#f26a55', badBg: '#301814', btn: '#efe3c8', onBtn: '#2a1e12',
      // the studio: the amber needle, the cream meter face, brass hairlines, the ON AIR red, walnut for the side list
      studio: { amber: '#f2a33a', onAmber: '#231703', cream: '#efe3c8', creamDeep: '#d6c49b', creamInk: '#2d2114', brass: '#b8935a', needle: '#c4620d', onair: '#ff5d48', side: '#38230f', side2: '#27180a' },
      roles: { plan: '#8aaedb', crit: '#ee7866', test: '#72c2a2', exec: '#62bcc2', gate: '#b5c668', rev: '#dc94b9', integ: '#ada594', coord: '#e8decb', watch: '#d3ab6c', queue: '#ada594' },
      ansi: ['#241c14', '#e5675d', '#86c48e', '#e0b765', '#78a6dd', '#c691bd', '#69bdb7', '#d5cab4',
        '#5a4e3c', '#f08a80', '#a2d6a8', '#efcf8f', '#9bbff0', '#dcaedc', '#8bd3cf', '#f6efde'],
      // the terminal's own look (F7, F10): warm coal off the panel, cream text, muted amber / red / green (text 14:1, colours 1-6 >= 6.7:1)
      term: { background: '#1b150f', foreground: '#ece1ca', cursor: '#f2a33a', selection: 0.3,
        ansi: ['#2d251b', '#e47e70', '#98c78f', '#e2b668', '#82aadb', '#cc96bd', '#72c0b6', '#d8ceb8',
          '#8f836b', '#f09d90', '#b2d9a8', '#f1cd8e', '#a7c4ea', '#deb4d3', '#96d4cb', '#f6efde'] },
    },
    light: {
      name: 'Nightshift Light', scheme: 'light',
      canvas: '#e4d7bb', panel: '#f5ecd8', raised: '#fdf8ec', raised2: '#ebdfc4', line: '#d9caa6', line2: '#bca97c', bg: '#faf3e3',
      ink: '#2b1f14', ink2: '#54432e', ink3: '#68563d', accent: '#8f5200', cursor: '#2b1f14', selection: '#f1d79f',
      human: '#8a4f00', humanBg: '#fbeccb', humanLine: '#d9a54a', onHuman: '#ffffff',
      ok: '#17693f', okBg: '#dfeedd', bad: '#b02a1a', badBg: '#f8e0d8', btn: '#2b1f14', onBtn: '#f5ecd8',
      studio: { amber: '#f2a33a', onAmber: '#231703', cream: '#f8efd8', creamDeep: '#e6d6b0', creamInk: '#2b1f14', brass: '#9a7430', needle: '#b4580a', onair: '#c4311f', side: '#e8dbbd', side2: '#dfd0ad' },
      roles: { plan: '#1f5cb3', crit: '#b02a1a', test: '#0f6e4e', exec: '#0a6c76', gate: '#52690f', rev: '#a63b70', integ: '#4f5d70', coord: '#2b1f14', watch: '#80591a', queue: '#4f5d70' },
      ansi: ['#2b1f14', '#b02a1a', '#2d7d46', '#946a00', '#2f6db0', '#9a4a9c', '#16808a', '#d8cdb5',
        '#68563d', '#c8402f', '#3b9a59', '#b07f00', '#4a86c6', '#b465b6', '#2a9aa4', '#f8f1e0'],
      term: { background: '#fbf4e2', foreground: '#2b2015', cursor: '#8f5200', selection: 0.22,
        ansi: ['#2b2015', '#b0301f', '#2a703c', '#8a5d00', '#2a62a8', '#8f4392', '#126f78', '#6b6050',
          '#574c3b', '#bc3a28', '#2a7c46', '#8f6000', '#2f6db5', '#9a4c9d', '#137a84', '#2b2015'] },
    },
  };
  // the focused pane takes the colour of its env (story 247): from the vendor marks in icons/; mono marks follow the theme accent
  const ENV_ACCENT = { gemini: '#3186ff' };   // claude takes the theme's amber: one live accent
  const RADIUS = { sharp: [4, 3], soft: [12, 8], round: [18, 12] };
  const FONTS = {
    system: 'ui-monospace, SFMono-Regular, Menlo, Consolas, "DejaVu Sans Mono", monospace',
    jetbrains: '"JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, monospace',
    fira: '"Fira Code", ui-monospace, SFMono-Regular, Menlo, monospace',
    plex: '"IBM Plex Mono", ui-monospace, SFMono-Regular, Menlo, monospace',
    iosevka: '"Iosevka Term", "Iosevka", ui-monospace, SFMono-Regular, Menlo, monospace',
  };
  const SANS = 'system-ui, -apple-system, "Segoe UI", "Noto Sans", "Helvetica Neue", sans-serif';
  // the person's choices (settings.look + settings.ui.tabs.look) with defaults; anything out of range falls back
  const LOOK0 = { theme: 'system', accent: 'env', font: 'system', font_px: 13, radius: 'soft', gap: 6, opacity: 100, clawd: 'animate', preset: 'hypr' };
  // three ready "rice" presets (story 296): what each changes; the theme stays the person's
  const PRESETS = {
    hypr: { radius: 'soft', gap: 6, opacity: 100, side: false, bar: 'full' },
    minimal: { radius: 'sharp', gap: 2, opacity: 100, side: false, bar: 'min' },
    classic: { radius: 'soft', gap: 0, opacity: 100, side: true, bar: 'full' },
  };
  const HEX = /^#[0-9a-f]{6}$/i;
  function look(raw) {
    const o = Object.assign({}, LOOK0, raw || {});
    if (!['system', 'dark', 'light'].includes(o.theme)) o.theme = LOOK0.theme;
    if (o.accent !== 'env' && !HEX.test(String(o.accent))) o.accent = LOOK0.accent;
    if (!FONTS[o.font]) o.font = LOOK0.font;
    o.font_px = Math.max(11, Math.min(20, Math.round(Number(o.font_px)) || LOOK0.font_px));
    if (!RADIUS[o.radius]) o.radius = LOOK0.radius;
    o.gap = Math.max(0, Math.min(16, Math.round(Number(o.gap)))); if (!Number.isFinite(o.gap)) o.gap = LOOK0.gap;
    o.opacity = Math.max(70, Math.min(100, Math.round(Number(o.opacity)) || LOOK0.opacity));
    if (!['animate', 'static', 'hide'].includes(o.clawd)) o.clawd = LOOK0.clawd;
    if (!PRESETS[o.preset]) o.preset = LOOK0.preset;
    return o;
  }
  // settings.look (theme, font_px: the core's fields) + settings.ui.tabs.look (the v4 extras, kept with the layout)
  const fromSettings = (st) => look(Object.assign({}, (st && st.look) || {}, (st && st.ui && st.ui.tabs && st.ui.tabs.look) || {}));
  const resolve = (theme, systemDark) => (theme === 'dark' || theme === 'light' ? theme : systemDark ? 'dark' : 'light');
  const rgba = (hex, a) => { const n = parseInt(hex.slice(1), 16); return 'rgba(' + (n >> 16) + ', ' + ((n >> 8) & 255) + ', ' + (n & 255) + ', ' + a + ')'; };
  // -> { '--name': value } for one resolved theme; env = the env id of the focused pane (accent 'env')
  function vars(name, lk, env) {
    const T = THEMES[name] || THEMES.dark, L = look(lk);
    const accent = L.accent !== 'env' ? L.accent : ENV_ACCENT[env] || T.accent;
    const [rp, rs] = RADIUS[L.radius];
    const v = {
      '--canvas': T.canvas, '--bg': T.bg, '--panel': T.panel, '--raised': T.raised, '--raised2': T.raised2, '--line': T.line, '--line2': T.line2,
      '--ink': T.ink, '--ink2': T.ink2, '--ink3': T.ink3, '--accent': accent, '--cursor': T.cursor, '--sel': T.selection,
      '--human': T.human, '--human-bg': T.humanBg, '--human-line': T.humanLine, '--on-human': T.onHuman,
      '--ok': T.ok, '--ok-bg': T.okBg, '--bad': T.bad, '--bad-bg': T.badBg, '--btn': T.btn, '--on-btn': T.onBtn,
      '--pane-bg': L.opacity >= 100 ? T.panel : rgba(T.panel, L.opacity / 100),
      '--radius-pane': rp + 'px', '--radius-sm': rs + 'px', '--r': rs + 'px', '--gap': L.gap + 'px',
      '--mono': FONTS[L.font], '--sans': SANS, '--fs': L.font_px + 'px',
    };
    for (const [k, c] of Object.entries(T.studio)) v['--' + k.replace(/[A-Z]/g, (m) => '-' + m.toLowerCase())] = c;
    for (const [k, c] of Object.entries(T.roles)) v['--' + k] = c;
    T.ansi.forEach((c, i) => { v['--ansi-' + i] = c; });
    return v;
  }
  const ANSI_NAMES = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white'];
  // the theme object of xterm 6 (task 23 passes it to `new Terminal({ theme })` and to `term.options.theme` on a change)
  function xterm(name, lk) {
    const T = THEMES[name] || THEMES.dark, L = look(lk);
    const X = T.term;   // selection = a translucent accent, so the text under it keeps its colour
    const o = { background: L.opacity >= 100 ? X.background : rgba(X.background, L.opacity / 100), foreground: X.foreground, cursor: X.cursor, cursorAccent: X.background, selectionBackground: rgba(T.accent, X.selection) };
    X.ansi.forEach((c, i) => { o[(i >= 8 ? 'bright' + ANSI_NAMES[i - 8][0].toUpperCase() + ANSI_NAMES[i - 8].slice(1) : ANSI_NAMES[i])] = c; });
    return o;
  }
  const xtermFont = (lk) => { const L = look(lk); return { fontFamily: FONTS[L.font], fontSize: L.font_px }; };

  // ---------- DOM: apply to <html>, follow the system theme, tell the listeners (xterm panes) ----------
  let current = null, envNow = null;
  const subs = [];
  function applyTheme(lk, env) {
    if (typeof document === 'undefined') return null;
    if (env !== undefined) envNow = env;
    const L = look(lk === undefined ? (current && current.look) : lk);
    const dark = !!(root.matchMedia && root.matchMedia('(prefers-color-scheme: dark)').matches);
    const name = resolve(L.theme, dark);
    const el = document.documentElement;
    for (const [k, v] of Object.entries(vars(name, L, envNow))) el.style.setProperty(k, v);
    el.setAttribute('data-theme', name);
    el.style.setProperty('color-scheme', name);
    el.setAttribute('data-radius', L.radius);
    current = { name, look: L, xterm: xterm(name, L), font: xtermFont(L) };
    for (const fn of subs.slice()) fn(current);
    return current;
  }
  if (root.matchMedia) {
    const mq = root.matchMedia('(prefers-color-scheme: dark)');
    const re = () => { if (current && current.look.theme === 'system') applyTheme(current.look); };
    if (mq.addEventListener) mq.addEventListener('change', re); else if (mq.addListener) mq.addListener(re);
  }
  const onTheme = (fn) => { subs.push(fn); return () => subs.splice(subs.indexOf(fn), 1); };
  root.NSTheme = { THEMES, ENV_ACCENT, RADIUS, FONTS, PRESETS, LOOK0, look, fromSettings, resolve, vars, xterm, xtermFont, applyTheme, onTheme, get current() { return current; } };
  if (root.NS) Object.assign(root.NS, { applyTheme, theme: root.NSTheme });
})(typeof window !== 'undefined' ? window : globalThis);
