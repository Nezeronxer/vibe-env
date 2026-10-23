// v4 theme (stories 243-247, 255, 295): one table per theme -> the CSS variables of every screen and the theme object of xterm.
// Pure part (NSTheme.vars / xterm / resolve / look) runs in selftest; applyTheme(look) writes the variables on <html>.
// tokens.css carries the same Dark and Light values for the first paint; selftest checks the two never drift apart.
(function (root) {
  // ANSI order: black red green yellow blue magenta cyan white, then the bright eight. Muted, no neon.
  const THEMES = {
    dark: {
      name: 'Nightshift Dark', scheme: 'dark',
      canvas: '#0b0c0e', panel: '#131417', raised: '#1a1c20', raised2: '#24272c', line: '#26292e', line2: '#353a41',
      ink: '#e4e4e1', ink2: '#acaeb2', ink3: '#8b8f95', accent: '#8fb3d9', cursor: '#e4e4e1', selection: '#2f3a48',
      human: '#f2b134', humanBg: '#2a2212', humanLine: '#5c4818', onHuman: '#1a1405',
      ok: '#5dbb8a', okBg: '#15261e', bad: '#ef6b60', badBg: '#2b1a19', btn: '#e4e4e1', onBtn: '#131417',
      roles: { plan: '#6aa4ee', crit: '#ef6b60', test: '#4cb894', exec: '#48bdc7', gate: '#a2c44f', rev: '#df84b2', integ: '#93a1b4', coord: '#d8d8d4', watch: '#c9a56b', queue: '#93a1b4' },
      ansi: ['#1c1e22', '#e5675d', '#79bf86', '#d9b45f', '#6fa3e0', '#c38bc9', '#5fbfc0', '#c8c9c6',
        '#4a4f57', '#f08a80', '#98d4a2', '#ebcb82', '#93bdf0', '#d6a8db', '#86d4d4', '#f1f1ee'],
      // the terminal's own look (F7): a warm ground a step off the panel, soft text, muted but readable colours (text 12.6:1, colours 1-6 >= 5.9:1)
      term: { background: '#1a1917', foreground: '#dedad2', cursor: '#dedad2', selection: 0.3,
        ansi: ['#2b2a27', '#e0786c', '#8fc08a', '#d9b870', '#78a6dd', '#c692c6', '#6fbdb8', '#cfcbc2',
          '#8a867d', '#ee998d', '#a9d4a3', '#e8cd92', '#9bc0ee', '#d9afd9', '#92d2cc', '#f3f0e8'] },
    },
    light: {
      name: 'Nightshift Light', scheme: 'light',
      canvas: '#dfe1e4', panel: '#f6f6f7', raised: '#ffffff', raised2: '#e9eaec', line: '#d9dbde', line2: '#c2c5ca',
      ink: '#16171a', ink2: '#45474c', ink3: '#5f6268', accent: '#2f6db0', cursor: '#16171a', selection: '#c9d9ec',
      human: '#b46f00', humanBg: '#fbf0d8', humanLine: '#e0b866', onHuman: '#ffffff',
      ok: '#17794d', okBg: '#e3f3ea', bad: '#c2362b', badBg: '#fbe9e7', btn: '#16171a', onBtn: '#f6f6f7',
      roles: { plan: '#2368c7', crit: '#c2362b', test: '#12805a', exec: '#0b7f8a', gate: '#5f7d12', rev: '#b8457f', integ: '#566478', coord: '#2b2c30', watch: '#8a6420', queue: '#566478' },
      ansi: ['#16171a', '#c2362b', '#2d7d46', '#946a00', '#2f6db0', '#9a4a9c', '#16808a', '#d4d5d8',
        '#5f6268', '#d9534a', '#3b9a59', '#b07f00', '#4a86c6', '#b465b6', '#2a9aa4', '#f6f6f7'],
      term: { background: '#fbf9f4', foreground: '#2a2926', cursor: '#2a2926', selection: 0.22,
        ansi: ['#2a2926', '#b8372c', '#2c7440', '#8a6200', '#2a62a8', '#8f4392', '#126f78', '#6b675f',
          '#55524b', '#c0392e', '#2a7c46', '#8f6600', '#2f6db5', '#9a4c9d', '#137a84', '#2a2926'] },
    },
  };
  // the focused pane takes the colour of its env (story 247): from the vendor marks in icons/; mono marks follow the theme accent
  const ENV_ACCENT = { claude: '#d97757', gemini: '#3186ff' };
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
      '--canvas': T.canvas, '--bg': T.canvas, '--panel': T.panel, '--raised': T.raised, '--raised2': T.raised2, '--line': T.line, '--line2': T.line2,
      '--ink': T.ink, '--ink2': T.ink2, '--ink3': T.ink3, '--accent': accent, '--cursor': T.cursor, '--sel': T.selection,
      '--human': T.human, '--human-bg': T.humanBg, '--human-line': T.humanLine, '--on-human': T.onHuman,
      '--ok': T.ok, '--ok-bg': T.okBg, '--bad': T.bad, '--bad-bg': T.badBg, '--btn': T.btn, '--on-btn': T.onBtn,
      '--pane-bg': L.opacity >= 100 ? T.panel : rgba(T.panel, L.opacity / 100),
      '--radius-pane': rp + 'px', '--radius-sm': rs + 'px', '--r': rs + 'px', '--gap': L.gap + 'px',
      '--mono': FONTS[L.font], '--sans': SANS, '--fs': L.font_px + 'px',
    };
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
