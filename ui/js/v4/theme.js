// v4 theme (stories 243-247, 255, 295): one table per theme -> the CSS variables of every screen and the theme object of xterm.
// Pure part (NSTheme.vars / xterm / resolve / look) runs in selftest; applyTheme(look) writes the variables on <html>.
// tokens.css carries the same Dark and Light values for the first paint; selftest checks the two never drift apart.
(function (root) {
  // ANSI order: black red green yellow blue magenta cyan white, then the bright eight. Muted, no neon.
  const THEMES = {
    dark: {
      name: 'Nightshift Dark', scheme: 'dark',
      // v7: graphite, a hair of blue, hairlines instead of shadows; the one live colour is the env of the agent in focus
      canvas: '#0b0c0f', panel: '#111216', raised: '#17181d', raised2: '#1e2026', line: '#24262d', line2: '#343741', bg: '#08090b',
      ink: '#eceef3', ink2: '#b9bdc8', ink3: '#8a8f9c', accent: '#8e95ff', cursor: '#eceef3', selection: '#2a3150',
      human: '#f5b14c', humanBg: '#2a2112', humanLine: '#5e4720', onHuman: '#14100a',
      ok: '#5fd38d', okBg: '#11221a', bad: '#f2706a', badBg: '#2b1517', btn: '#eceef3', onBtn: '#111216',
      // materials (the names stayed from v5): amber = the live accent (vars() puts the env colour here), cream = a light face
      studio: { amber: '#8e95ff', onAmber: '#0b0c0f', cream: '#eceef3', creamDeep: '#c9ccd4', creamInk: '#111216', brass: '#3a3d47', needle: '#8e95ff', onair: '#ff6b5e', side: '#0e0f12', side2: '#0b0c0f' },
      roles: { plan: '#8aaedb', crit: '#ee7866', test: '#72c2a2', exec: '#62bcc2', gate: '#b5c668', rev: '#dc94b9', integ: '#a3a7b3', coord: '#e1e3ea', watch: '#d3ab6c', queue: '#a3a7b3' },
      ansi: ['#1b1d23', '#f2706a', '#5fd38d', '#e8c06a', '#7aa7f5', '#c792ea', '#5cc8c8', '#d3d6de',
        '#4a4e5a', '#ff8f87', '#86e0a8', '#f2d48f', '#9cc0ff', '#d9b0f2', '#86dcdc', '#f4f5f8'],
      // the terminal: a step under the panel, soft white text, colours 1-6 >= 6:1
      term: { background: '#0d0e11', foreground: '#e4e6eb', cursor: '#e4e6eb', selection: 0.3,
        ansi: ['#1f2128', '#f07a72', '#6fd39a', '#e9c46a', '#7fb0ff', '#d09af0', '#66cccc', '#d6d9e0',
          '#5c6170', '#ff958d', '#8fe2b0', '#f3d68e', '#a3c6ff', '#ddb6f5', '#8fe0e0', '#f5f6f8'] },
    },
    light: {
      name: 'Nightshift Light', scheme: 'light',
      canvas: '#eceef2', panel: '#ffffff', raised: '#f7f8fa', raised2: '#eef0f3', line: '#e2e4e9', line2: '#cfd2d9', bg: '#f6f7f9',
      ink: '#15161a', ink2: '#3e424b', ink3: '#5a5f6a', accent: '#4f56d8', cursor: '#15161a', selection: '#d9ddff',
      human: '#9a5b00', humanBg: '#fdf1dc', humanLine: '#e7b765', onHuman: '#ffffff',
      ok: '#147a43', okBg: '#e3f4ea', bad: '#c0362c', badBg: '#fbe6e4', btn: '#15161a', onBtn: '#ffffff',
      studio: { amber: '#4f56d8', onAmber: '#ffffff', cream: '#ffffff', creamDeep: '#eceef2', creamInk: '#15161a', brass: '#d5d8de', needle: '#4f56d8', onair: '#d93a2e', side: '#f3f4f7', side2: '#eceef2' },
      roles: { plan: '#1f5cb3', crit: '#b02a1a', test: '#0f6e4e', exec: '#0a6c76', gate: '#52690f', rev: '#a63b70', integ: '#4f5d70', coord: '#15161a', watch: '#80591a', queue: '#4f5d70' },
      ansi: ['#15161a', '#c0362c', '#2a7a45', '#8f6400', '#2f62c0', '#8f45a0', '#127580', '#d8dbe1',
        '#5a5f6a', '#cf4436', '#33904f', '#a87500', '#3d76d6', '#a456b6', '#1a8a96', '#f6f7f9'],
      term: { background: '#fbfbfc', foreground: '#1d1f24', cursor: '#1d1f24', selection: 0.22,
        ansi: ['#1d1f24', '#b8322a', '#26703f', '#875c00', '#2a5db8', '#8a409a', '#116d77', '#5f6470',
          '#4f5460', '#c23d30', '#2b7c48', '#8f6200', '#2f66c4', '#9348a3', '#137783', '#1d1f24'] },
    },
  };
  // the focused agent paints the window with its env (story 247, v7): per theme, so the accent reads on both grounds
  const ENV_ACCENT = {
    dark: { claude: '#e8865f', codex: '#4cc38a', gemini: '#5b9cff', qwen: '#9b8cff', opencode: '#f0b35a', cursor: '#d6d9e0' },
    light: { claude: '#b8532c', codex: '#0f7a4a', gemini: '#2259cc', qwen: '#5e43d6', opencode: '#975a06', cursor: '#3e424b' },
  };
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
    const accent = L.accent !== 'env' ? L.accent : (ENV_ACCENT[T.scheme] || {})[env] || T.accent;
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
    v['--amber'] = accent; v['--needle'] = accent;      // v7: the live accent follows the env in focus
    for (const [k, c] of Object.entries(ENV_ACCENT[T.scheme] || {})) v['--env-' + k] = c;   // v7: every agent card wears its env
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
