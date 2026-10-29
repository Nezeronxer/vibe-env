// v4 theme (stories 243-247, 255, 295): one table per theme -> the CSS variables of every screen and the theme object of xterm.
// Pure part (NSTheme.vars / xterm / resolve / look / custom) runs in selftest; applyTheme(look) writes the variables on <html>.
// tokens.css carries the same Dark and Light values for the first paint; selftest checks the two never drift apart.
// look8: six more ready themes, the person's own themes (5 colours -> a full table), window transparency and accent gradients.
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
    // look8: true black for OLED panels - the canvas and the terminal are #000, panes one hair above so the hairlines still read
    oled: {
      name: 'Чёрная OLED', scheme: 'dark', flat: true,
      canvas: '#000000', panel: '#0a0a0c', raised: '#121215', raised2: '#1a1a1e', line: '#1e1e23', line2: '#2d2d33', bg: '#000000',
      ink: '#f1f1f4', ink2: '#bdbdc6', ink3: '#8f8f9a', accent: '#8e95ff', cursor: '#f1f1f4', selection: '#252a48',
      human: '#f5b14c', humanBg: '#1f180c', humanLine: '#57421d', onHuman: '#14100a',
      ok: '#5fd38d', okBg: '#0b1a12', bad: '#f2706a', badBg: '#230f11', btn: '#f1f1f4', onBtn: '#0a0a0c',
      studio: { amber: '#8e95ff', onAmber: '#000000', cream: '#f1f1f4', creamDeep: '#c9c9d1', creamInk: '#0a0a0c', brass: '#34343b', needle: '#8e95ff', onair: '#ff6b5e', side: '#050506', side2: '#000000' },
      roles: { plan: '#8aaedb', crit: '#ee7866', test: '#72c2a2', exec: '#62bcc2', gate: '#b5c668', rev: '#dc94b9', integ: '#a3a7b3', coord: '#e1e3ea', watch: '#d3ab6c', queue: '#a3a7b3' },
      ansi: ['#1a1a1e', '#f2706a', '#5fd38d', '#e8c06a', '#7aa7f5', '#c792ea', '#5cc8c8', '#d3d6de',
        '#4a4a54', '#ff8f87', '#86e0a8', '#f2d48f', '#9cc0ff', '#d9b0f2', '#86dcdc', '#f6f6f8'],
      term: { background: '#000000', foreground: '#e8e8ec', cursor: '#e8e8ec', selection: 0.32,
        ansi: ['#1c1c21', '#f07a72', '#6fd39a', '#e9c46a', '#7fb0ff', '#d09af0', '#66cccc', '#d6d6de',
          '#5a5a66', '#ff958d', '#8fe2b0', '#f3d68e', '#a3c6ff', '#ddb6f5', '#8fe0e0', '#f6f6f8'] },
    },
    // look8: deep navy night, a cool blue accent, a faint indigo wash from the top-left corner
    midnight: {
      name: 'Полночь', scheme: 'dark', grad: '#1c2a5c',
      canvas: '#090d1a', panel: '#0e1324', raised: '#141a2e', raised2: '#1a2138', line: '#222a44', line2: '#323c5c', bg: '#060912',
      ink: '#e6ebf7', ink2: '#b2bcd6', ink3: '#8892b0', accent: '#7ea4ff', cursor: '#e6ebf7', selection: '#24335e',
      human: '#f5b14c', humanBg: '#272012', humanLine: '#5b4622', onHuman: '#14100a',
      ok: '#64d39a', okBg: '#0f2120', bad: '#f4757a', badBg: '#2a1420', btn: '#e6ebf7', onBtn: '#0e1324',
      studio: { amber: '#7ea4ff', onAmber: '#090d1a', cream: '#e6ebf7', creamDeep: '#c3cbe0', creamInk: '#0e1324', brass: '#2f3854', needle: '#7ea4ff', onair: '#ff6b70', side: '#0b1020', side2: '#090d1a' },
      roles: { plan: '#8ab0e6', crit: '#ef7c74', test: '#74c6aa', exec: '#66c0cc', gate: '#b8c86e', rev: '#dc98c4', integ: '#a5acc2', coord: '#e2e7f4', watch: '#d6af72', queue: '#a5acc2' },
      ansi: ['#1b2137', '#f4757a', '#64d39a', '#e6be6e', '#7ea4ff', '#bb9af7', '#6cc9de', '#cdd4ea',
        '#45507a', '#ff8d92', '#8ce3b4', '#f1d290', '#a0bcff', '#cfb4fa', '#94dceb', '#eef1fa'],
      term: { background: '#0b1020', foreground: '#dfe5f4', cursor: '#dfe5f4', selection: 0.3,
        ansi: ['#1d2340', '#f4757a', '#6fd6a0', '#e6be6e', '#82a8ff', '#c2a2f8', '#70cde2', '#c9d1e8',
          '#4a5580', '#ff9196', '#94e6ba', '#f2d494', '#a6c0ff', '#d4bafb', '#98def0', '#eef1fa'] },
    },
    // look8: warm graphite - brown-black like a dimmed workshop, sand text, a sage accent (amber stays the "ждёт тебя" colour)
    graphite: {
      name: 'Графит тёплый', scheme: 'dark',
      canvas: '#110f0d', panel: '#181614', raised: '#1f1d1a', raised2: '#272421', line: '#2d2a26', line2: '#3d3934', bg: '#0c0b0a',
      ink: '#efeae2', ink2: '#c6beb1', ink3: '#9c9385', accent: '#8fc2a8', cursor: '#efeae2', selection: '#2c3a33',
      human: '#f5b14c', humanBg: '#2a2112', humanLine: '#5e4720', onHuman: '#14100a',
      ok: '#7fcf8e', okBg: '#18221a', bad: '#f0786a', badBg: '#2b1714', btn: '#efeae2', onBtn: '#181614',
      studio: { amber: '#8fc2a8', onAmber: '#110f0d', cream: '#efeae2', creamDeep: '#cfc7ba', creamInk: '#181614', brass: '#3b3731', needle: '#8fc2a8', onair: '#ff6f5c', side: '#141210', side2: '#110f0d' },
      roles: { plan: '#93b0d4', crit: '#ec7c68', test: '#82c2a0', exec: '#70bcb4', gate: '#bcc46c', rev: '#d898b4', integ: '#aaa398', coord: '#e6e0d6', watch: '#d8ae6c', queue: '#aaa398' },
      ansi: ['#24211e', '#f0786a', '#a8c77a', '#e6b65c', '#86a9d6', '#c99ad6', '#7cc2b4', '#d9d2c6',
        '#5a554d', '#f59684', '#bcd890', '#f0c878', '#a0bfe6', '#d8b0e2', '#96d4c7', '#f4efe7'],
      term: { background: '#141210', foreground: '#ece6dc', cursor: '#ece6dc', selection: 0.3,
        ansi: ['#262320', '#f07c6c', '#a8c77a', '#e6b65c', '#8aaddb', '#cc9edb', '#7cc4b6', '#d9d2c6',
          '#5e584f', '#f79a88', '#bcd890', '#f0c878', '#a4c2e8', '#dab4e4', '#98d6c9', '#f4efe7'] },
    },
    // look8: GitHub-dimmed-like dusk - slate blue-grey instead of black, softer for long evening sessions
    dimmed: {
      name: 'Сумерки', scheme: 'dark',
      canvas: '#1a1f26', panel: '#21262e', raised: '#272d36', raised2: '#2d343e', line: '#353c47', line2: '#444c58', bg: '#161a20',
      ink: '#e8edf2', ink2: '#bcc5cf', ink3: '#a1abb7', accent: '#74b4f6', cursor: '#e8edf2', selection: '#2e4563',
      human: '#f2b55a', humanBg: '#342b1d', humanLine: '#6a5228', onHuman: '#14100a',
      ok: '#78d08e', okBg: '#213027', bad: '#f5847a', badBg: '#3a2427', btn: '#e8edf2', onBtn: '#21262e',
      studio: { amber: '#74b4f6', onAmber: '#1a1f26', cream: '#e8edf2', creamDeep: '#c6ced8', creamInk: '#21262e', brass: '#46505d', needle: '#74b4f6', onair: '#ff7d72', side: '#1d2229', side2: '#1a1f26' },
      roles: { plan: '#9cbfe8', crit: '#f2887a', test: '#86cfb2', exec: '#76c8d0', gate: '#c2d07c', rev: '#e2a4c6', integ: '#b0b8c4', coord: '#e6ebf1', watch: '#dcb678', queue: '#b0b8c4' },
      ansi: ['#2d333b', '#f5847a', '#78d08e', '#ddb050', '#74b4f6', '#d4b2f2', '#7ad0d8', '#c5ccd5',
        '#5a636e', '#ff9b92', '#9ee0ac', '#e8c470', '#98c8fa', '#e2c6f8', '#9cdee4', '#eef2f6'],
      term: { background: '#1d2229', foreground: '#dfe5ec', cursor: '#dfe5ec', selection: 0.3,
        ansi: ['#2d333b', '#f68a80', '#80d495', '#ddb050', '#7ab8f8', '#d8b8f4', '#7ed4dc', '#c5ccd5',
          '#5e6772', '#ffa098', '#a4e2b0', '#e8c470', '#9ccafa', '#e4cafa', '#a0e0e6', '#eef2f6'] },
    },
    // look8: mocha - a purple-grey night with pastel colours and a mauve accent; a soft mauve wash from the corner
    mocha: {
      name: 'Мокко', scheme: 'dark', grad: '#3a2850',
      canvas: '#121220', panel: '#1b1b2b', raised: '#222234', raised2: '#2a2a3e', line: '#30314a', line2: '#43455c', bg: '#0e0e18',
      ink: '#dfe4fa', ink2: '#bdc3e0', ink3: '#9da2c2', accent: '#c6a4f6', cursor: '#dfe4fa', selection: '#3a3358',
      human: '#f5c07a', humanBg: '#2e2620', humanLine: '#62503a', onHuman: '#14100a',
      ok: '#a2e09c', okBg: '#1f2a26', bad: '#f38aa6', badBg: '#33202c', btn: '#dfe4fa', onBtn: '#1b1b2b',
      studio: { amber: '#c6a4f6', onAmber: '#121220', cream: '#dfe4fa', creamDeep: '#bec3dc', creamInk: '#1b1b2b', brass: '#3e3f56', needle: '#c6a4f6', onair: '#f58aa0', side: '#161626', side2: '#121220' },
      roles: { plan: '#8eb4f8', crit: '#f38aa6', test: '#94dccf', exec: '#88d4e6', gate: '#c8dc94', rev: '#f2b8d6', integ: '#aeb2cc', coord: '#e4e8fa', watch: '#f2cf9a', queue: '#aeb2cc' },
      ansi: ['#2a2b3c', '#f38aa6', '#a2e09c', '#f7dfa8', '#8eb4f8', '#c6a4f6', '#92dfd2', '#bac0dc',
        '#585a70', '#f6a0b8', '#b8eab2', '#fae8c0', '#a8c6fa', '#d6bcf8', '#aae8de', '#e4e8f8'],
      term: { background: '#171726', foreground: '#d8def6', cursor: '#d8def6', selection: 0.3,
        ansi: ['#2a2b3c', '#f38aa6', '#a2e09c', '#f7dfa8', '#8eb4f8', '#c6a4f6', '#92dfd2', '#bac0dc',
          '#5c5e74', '#f6a0b8', '#b8eab2', '#fae8c0', '#a8c6fa', '#d6bcf8', '#aae8de', '#e4e8f8'] },
    },
    // look8: warm paper - cream surfaces, brown-black ink, an ink-blue accent; the light theme for daylight and long reading
    warm: {
      name: 'Светлая тёплая', scheme: 'light', grad: '#f3dcc0',
      canvas: '#eee8dd', panel: '#fcf9f3', raised: '#f6f1e8', raised2: '#eee7da', line: '#e3dbcc', line2: '#cfc5b2', bg: '#f6f1e8',
      ink: '#1f1b16', ink2: '#463f35', ink3: '#5f574b', accent: '#3a52bd', cursor: '#1f1b16', selection: '#dfe0f4',
      human: '#8f5200', humanBg: '#f8ead2', humanLine: '#e2b264', onHuman: '#ffffff',
      ok: '#1f6e3a', okBg: '#e3eedc', bad: '#b23426', badBg: '#f6e0d8', btn: '#1f1b16', onBtn: '#fcf9f3',
      studio: { amber: '#3a52bd', onAmber: '#ffffff', cream: '#fffdf8', creamDeep: '#eee8dd', creamInk: '#1f1b16', brass: '#d8cfbd', needle: '#3a52bd', onair: '#c83a2a', side: '#f4eee3', side2: '#eee8dd' },
      roles: { plan: '#1f5aa8', crit: '#a62c1a', test: '#0f6a4a', exec: '#0a6670', gate: '#4e640e', rev: '#9c386a', integ: '#53595f', coord: '#1f1b16', watch: '#7a5418', queue: '#53595f' },
      ansi: ['#1f1b16', '#b23426', '#2a7040', '#855e00', '#2c5ab4', '#8a4298', '#126e78', '#d6cfc2',
        '#5f574b', '#c2402f', '#32844b', '#9c6e00', '#3a6ac8', '#9c50aa', '#18808a', '#f6f1e8'],
      term: { background: '#fdfaf4', foreground: '#24201a', cursor: '#24201a', selection: 0.2,
        ansi: ['#24201a', '#ae3226', '#2a6c3c', '#7f5800', '#2a58b0', '#86409a', '#106a72', '#5e574c',
          '#4e473d', '#b83c2e', '#2e7644', '#875e00', '#3060ba', '#9046a2', '#14727c', '#24201a'] },
    },
  };
  // the focused agent paints the window with its env (story 247, v7): per scheme, so the accent reads on both grounds
  const ENV_ACCENT = {
    dark: { claude: '#e8865f', codex: '#4cc38a', gemini: '#5b9cff', qwen: '#9b8cff', opencode: '#f0b35a', cursor: '#d6d9e0',
      kimi: '#38c6e8', copilot: '#d58cf0', crush: '#ff79c6', amp: '#c4d36a', goose: '#d9c2a0', droid: '#f47c6c', aider: '#9ee06b', grok: '#b8c2d6' },
    light: { claude: '#b8532c', codex: '#0f7a4a', gemini: '#2259cc', qwen: '#5e43d6', opencode: '#975a06', cursor: '#3e424b',
      kimi: '#07708a', copilot: '#8a2fb0', crush: '#b3186f', amp: '#586d0c', goose: '#6e5634', droid: '#b12e22', aider: '#3d7a12', grok: '#454e63' },
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
  // the person's choices (settings.look + settings.ui.tabs.look) with defaults; anything out of range falls back.
  // look8: glass = window transparency in % (0 = opaque, as before), blur = the macOS vibrancy under a see-through window,
  // fx = soft accent gradients on the rail, panes and cards, themes = the person's own themes (see custom())
  const LOOK0 = { theme: 'system', accent: 'env', font: 'system', font_px: 13, radius: 'soft', gap: 6, opacity: 100, clawd: 'animate', preset: 'hypr',
    glass: 0, blur: true, fx: false, themes: [] };
  const GLASS_MAX = 60, THEMES_MAX = 24;
  // three ready "rice" presets (story 296): what each changes; the theme stays the person's
  const PRESETS = {
    hypr: { radius: 'soft', gap: 6, opacity: 100, side: false, bar: 'full' },
    minimal: { radius: 'sharp', gap: 2, opacity: 100, side: false, bar: 'min' },
    classic: { radius: 'soft', gap: 0, opacity: 100, side: true, bar: 'full' },
  };
  const HEX = /^#[0-9a-f]{6}$/i;

  // ---------- colour arithmetic (WCAG contrast, mixing) ----------
  const rgb = (c) => { const n = parseInt(c.slice(1), 16); return [n >> 16, (n >> 8) & 255, n & 255]; };
  const mix = (a, b, w) => '#' + rgb(a).map((x, i) => Math.round(x + (rgb(b)[i] - x) * w).toString(16).padStart(2, '0')).join('');
  const lum = (c) => rgb(c).map((v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); }).reduce((a, v, i) => a + v * [0.2126, 0.7152, 0.0722][i], 0);
  const contrast = (a, b) => { const x = lum(a), y = lum(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };
  // c moved toward `to` until it reads >= min on every ground (the person's colour wins when it already reads)
  function readable(c, grounds, to, min) {
    for (let w = 0; w <= 1.001; w += 0.05) { const x = w ? mix(c, to, w) : c; if (grounds.every((g) => contrast(x, g) >= min)) return x; }
    return to;
  }

  // ---------- the person's own theme: 5 colours (+ a gradient) -> the full table ----------
  // stored in settings.ui.tabs.look.themes; only #rrggbb colours pass, a name is plain text - nothing else reaches CSS
  const cleanName = (s) => String(s == null ? '' : s).replace(/[\u0000-\u001f\u007f<>]/g, '').trim().slice(0, 40);
  function customSpec(raw) {
    if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return null;
    const o = { id: /^u[0-9a-z]{1,16}$/.test(String(raw.id)) ? String(raw.id) : null, name: cleanName(raw.name) };
    for (const k of ['bg', 'panel', 'ink']) { if (!HEX.test(String(raw[k]))) return null; o[k] = String(raw[k]).toLowerCase(); }
    o.accent = raw.accent === 'env' || raw.accent == null ? 'env' : HEX.test(String(raw.accent)) ? String(raw.accent).toLowerCase() : null;
    o.grad = raw.grad == null || raw.grad === false || raw.grad === '' ? null : HEX.test(String(raw.grad)) ? String(raw.grad).toLowerCase() : undefined;
    if (o.accent === null || o.grad === undefined) return null;
    return o;
  }
  // JSON of a theme for export / from import: { nightshift_theme: 1, name, bg, panel, ink, accent, grad }
  const exportTheme = (spec) => JSON.stringify({ nightshift_theme: 1, name: spec.name, bg: spec.bg, panel: spec.panel, ink: spec.ink, accent: spec.accent, grad: spec.grad || null }, null, 2);
  function importTheme(text) {
    let raw;
    try { raw = JSON.parse(String(text || '')); } catch (e) { return { ok: false, why: 'json' }; }
    if (!raw || typeof raw !== 'object' || raw.nightshift_theme !== 1) return { ok: false, why: 'format' };
    const s = customSpec(Object.assign({}, raw, { id: null }));
    return s ? { ok: true, spec: s } : { ok: false, why: 'colors' };
  }
  function custom(spec) {
    const dark = lum(spec.bg) < 0.18, B = THEMES[dark ? 'dark' : 'light'];
    const { bg, panel, ink } = spec, step = (w) => mix(panel, ink, w);
    const raised = step(0.04), raised2 = step(0.08), grounds = [bg, panel, raised, raised2];
    const accent = spec.accent !== 'env' ? spec.accent : readable(B.accent, [panel], ink, 4.5);
    const status = (c) => readable(c, [panel], ink, 4.5);
    const ok = status(B.ok), bad = status(B.bad), human = status(B.human);
    const termBg = mix(panel, bg, 0.5);
    return {
      name: spec.name || 'Своя тема', scheme: dark ? 'dark' : 'light', custom: true, ownAccent: spec.accent !== 'env', grad: spec.grad || null,
      canvas: bg, panel, raised, raised2, line: step(0.13), line2: step(0.21), bg: mix(bg, dark ? '#000000' : '#ffffff', 0.3),
      ink, ink2: readable(mix(ink, panel, 0.25), grounds, ink, 4.6), ink3: readable(mix(ink, panel, 0.42), grounds, ink, 4.6),
      accent, cursor: ink, selection: mix(panel, accent, 0.25),
      human, humanBg: mix(panel, human, 0.12), humanLine: mix(panel, human, 0.35), onHuman: B.onHuman,
      ok, okBg: mix(panel, ok, 0.1), bad, badBg: mix(panel, bad, 0.1), btn: ink, onBtn: panel,
      studio: { amber: accent, onAmber: contrast(accent, bg) >= contrast(accent, '#ffffff') ? bg : '#ffffff', cream: dark ? ink : '#ffffff', creamDeep: dark ? mix(ink, panel, 0.15) : bg,
        creamInk: dark ? panel : ink, brass: step(0.18), needle: accent, onair: status(B.studio.onair), side: mix(bg, panel, 0.5), side2: bg },
      roles: Object.fromEntries(Object.entries(B.roles).map(([k, c]) => [k, status(c)])),
      ansi: B.ansi.slice(),
      term: { background: termBg, foreground: ink, cursor: ink, selection: B.term.selection,
        ansi: B.term.ansi.map((c, i) => (i >= 1 && i <= 6) || (i >= 9 && i <= 14) ? readable(c, [termBg], ink, 4.5) : c) },
    };
  }
  // v9: famous designer palettes as ready themes - the colours of each project's own palette file (tokyonight.nvim,
  // rose-pine/neovim, nordtheme, dracula-theme, catppuccin/palette, gruvbox), built like an own theme, then text, accent,
  // statuses and terminal colours nudged until they read (ink 7:1, the rest 4.5:1). adapt = env colours are nudged too.
  function designed(p) {
    const dark = lum(p.bg) < 0.18, to = dark ? '#ffffff' : '#000000';
    let ink = p.ink, T = null;
    for (let i = 0; i < 12; i++) {
      T = custom({ bg: p.bg, panel: p.panel, ink, accent: p.accent, grad: p.grad || null });
      if ([T.canvas, T.panel, T.raised, T.raised2].every((g) => contrast(ink, g) >= 7)) break;
      ink = mix(ink, to, 0.12);
    }
    const on = (c) => readable(c, [T.panel], ink, 4.5);
    const tb = p.term || T.term.background, tAnsi = p.ansi.map((c, i) => (i >= 1 && i <= 6) || (i >= 9 && i <= 14) ? readable(c, [tb], ink, 4.5) : c);
    const ok = on(p.ok), bad = on(p.bad), human = on(p.human), accent = on(p.accent);
    delete T.custom; delete T.ownAccent;
    return Object.assign(T, { name: p.name, adapt: true, accent, selection: mix(T.panel, accent, 0.25), ok, okBg: mix(T.panel, ok, 0.12), bad, badBg: mix(T.panel, bad, 0.12),
      human, humanBg: mix(T.panel, human, 0.12), humanLine: mix(T.panel, human, 0.35),
      studio: Object.assign(T.studio, { amber: accent, needle: accent, onAmber: contrast(accent, p.bg) >= contrast(accent, '#ffffff') ? p.bg : '#ffffff' }),
      ansi: p.ansi.slice(), term: { background: tb, foreground: readable(ink, [tb], to, 7), cursor: accent, selection: T.term.selection, ansi: tAnsi } });
  }
  const DESIGNED = {
    tokyo: { name: 'Tokyo Night', bg: '#16161e', panel: '#1a1b26', ink: '#c0caf5', accent: '#7aa2f7', ok: '#9ece6a', bad: '#f7768e', human: '#e0af68', grad: '#2a2f55', term: '#1a1b26',
      ansi: ['#15161e', '#f7768e', '#9ece6a', '#e0af68', '#7aa2f7', '#bb9af7', '#7dcfff', '#a9b1d6', '#414868', '#f7768e', '#9ece6a', '#e0af68', '#7aa2f7', '#bb9af7', '#7dcfff', '#c0caf5'] },
    rosepine: { name: 'Rosé Pine', bg: '#191724', panel: '#1f1d2e', ink: '#e0def4', accent: '#ebbcba', ok: '#9ccfd8', bad: '#eb6f92', human: '#f6c177', grad: '#3b2a3e', term: '#1f1d2e',
      ansi: ['#26233a', '#eb6f92', '#31748f', '#f6c177', '#9ccfd8', '#c4a7e7', '#ebbcba', '#e0def4', '#6e6a86', '#eb6f92', '#31748f', '#f6c177', '#9ccfd8', '#c4a7e7', '#ebbcba', '#e0def4'] },
    nord: { name: 'Nord', bg: '#2e3440', panel: '#3b4252', ink: '#eceff4', accent: '#88c0d0', ok: '#a3be8c', bad: '#bf616a', human: '#ebcb8b', term: '#2e3440',
      ansi: ['#3b4252', '#bf616a', '#a3be8c', '#ebcb8b', '#81a1c1', '#b48ead', '#88c0d0', '#e5e9f0', '#4c566a', '#bf616a', '#a3be8c', '#ebcb8b', '#81a1c1', '#b48ead', '#8fbcbb', '#eceff4'] },
    dracula: { name: 'Dracula', bg: '#21222c', panel: '#282a36', ink: '#f8f8f2', accent: '#bd93f9', ok: '#50fa7b', bad: '#ff5555', human: '#ffb86c', term: '#282a36',
      ansi: ['#21222c', '#ff5555', '#50fa7b', '#f1fa8c', '#bd93f9', '#ff79c6', '#8be9fd', '#f8f8f2', '#6272a4', '#ff6e6e', '#69ff94', '#ffffa5', '#d6acff', '#ff92df', '#a4ffff', '#ffffff'] },
    gruvbox: { name: 'Gruvbox', bg: '#282828', panel: '#32302f', ink: '#ebdbb2', accent: '#fe8019', ok: '#b8bb26', bad: '#fb4934', human: '#fabd2f', term: '#282828',
      ansi: ['#282828', '#cc241d', '#98971a', '#d79921', '#458588', '#b16286', '#689d6a', '#a89984', '#928374', '#fb4934', '#b8bb26', '#fabd2f', '#83a598', '#d3869b', '#8ec07c', '#ebdbb2'] },
    latte: { name: 'Catppuccin Latte', bg: '#e6e9ef', panel: '#eff1f5', ink: '#4c4f69', accent: '#1e66f5', ok: '#40a02b', bad: '#d20f39', human: '#df8e1d', term: '#eff1f5',
      ansi: ['#5c5f77', '#d20f39', '#40a02b', '#df8e1d', '#1e66f5', '#ea76cb', '#179299', '#acb0be', '#6c6f85', '#d20f39', '#40a02b', '#df8e1d', '#1e66f5', '#ea76cb', '#179299', '#bcc0cc'] },
    dawn: { name: 'Rosé Pine Dawn', bg: '#faf4ed', panel: '#fffaf3', ink: '#464261', accent: '#286983', ok: '#56949f', bad: '#b4637a', human: '#ea9d34', grad: '#f2d9cf', term: '#fffaf3',
      ansi: ['#f2e9e1', '#b4637a', '#286983', '#ea9d34', '#56949f', '#907aa9', '#d7827e', '#464261', '#9893a5', '#b4637a', '#286983', '#ea9d34', '#56949f', '#907aa9', '#d7827e', '#464261'] },
  };
  for (const [id, p] of Object.entries(DESIGNED)) THEMES[id] = designed(p);
  const BUILTIN = Object.keys(THEMES);

  // the table of a theme id: a ready one, or the person's own (cached on THEMES, not enumerable - Object.keys(THEMES) stays the ready list)
  function table(name, lk) {
    if (BUILTIN.includes(name)) return THEMES[name];
    const s = ((lk && lk.themes) || []).find((x) => x && x.id === name);
    if (!s) return THEMES[name] && THEMES[name].custom ? THEMES[name] : THEMES.dark;
    const T = custom(s);
    Object.defineProperty(THEMES, name, { value: T, enumerable: false, configurable: true, writable: true });
    return T;
  }

  function look(raw) {
    const o = Object.assign({}, LOOK0, raw || {});
    o.themes = (Array.isArray(o.themes) ? o.themes : []).map(customSpec).filter((s) => s && s.id).slice(0, THEMES_MAX);
    if (!['system'].concat(BUILTIN).includes(o.theme) && !o.themes.some((s) => s.id === o.theme)) o.theme = LOOK0.theme;
    if (o.accent !== 'env' && !HEX.test(String(o.accent))) o.accent = LOOK0.accent;
    if (!FONTS[o.font]) o.font = LOOK0.font;
    o.font_px = Math.max(11, Math.min(20, Math.round(Number(o.font_px)) || LOOK0.font_px));
    if (!RADIUS[o.radius]) o.radius = LOOK0.radius;
    o.gap = Math.max(0, Math.min(16, Math.round(Number(o.gap)))); if (!Number.isFinite(o.gap)) o.gap = LOOK0.gap;
    o.opacity = Math.max(70, Math.min(100, Math.round(Number(o.opacity)) || LOOK0.opacity));
    o.glass = Math.max(0, Math.min(GLASS_MAX, Math.round(Number(o.glass)) || 0));
    o.blur = o.blur !== false; o.fx = o.fx === true;
    if (!['animate', 'static', 'hide'].includes(o.clawd)) o.clawd = LOOK0.clawd;
    if (!PRESETS[o.preset]) o.preset = LOOK0.preset;
    return o;
  }
  // settings.look (theme, font_px: the core's fields) + settings.ui.tabs.look (the v4 extras, kept with the layout)
  const fromSettings = (st) => look(Object.assign({}, (st && st.look) || {}, (st && st.ui && st.ui.tabs && st.ui.tabs.look) || {}));
  const resolve = (theme, systemDark) => (theme === 'system' || !theme ? (systemDark ? 'dark' : 'light') : theme);
  const rgba = (hex, a) => { const n = parseInt(hex.slice(1), 16); return 'rgba(' + (n >> 16) + ', ' + ((n >> 8) & 255) + ', ' + (n & 255) + ', ' + a + ')'; };
  // look8 glass: the canvas lets the desktop through by `glass`; surfaces (rail, panes, terminal, bar) lie on the canvas with an own
  // alpha of .55, so together they let through 45% of what the canvas does - denser, the text keeps a ground (glass 45: 55% / 80%)
  const alphas = (L) => { const g = L.glass / 100; return { canvas: 1 - g, surface: g > 0 ? 0.55 : 1 }; };
  const paneAlpha = (L) => Math.round((L.opacity / 100) * alphas(L).surface * 1000) / 1000;
  // the window wash: the accent lamp from the top (v7) and, when the theme has one, its own gradient from the top-left corner
  const LAMP = 'radial-gradient(1200px 520px at 50% -18%, color-mix(in srgb, var(--amber) var(--lamp-a), transparent), transparent 70%), '
    + 'radial-gradient(800px 600px at 110% 110%, color-mix(in srgb, var(--amber) var(--lamp-b), transparent), transparent 70%)';
  const themeFx = (T, L) => (T.grad ? 'linear-gradient(155deg, ' + rgba(T.grad, Math.round(alphas(L).canvas * 100) / 100) + ' 0%, transparent 62%)' : 'none');
  const windowFx = (T, L) => [T.flat ? null : LAMP, T.grad ? themeFx(T, L) : null].filter(Boolean).join(', ') || 'none';   // OLED: no lamp, true black
  // -> { '--name': value } for one resolved theme; env = the env id of the focused pane (accent 'env')
  function vars(name, lk, env) {
    const L = look(lk), T = table(name, L);
    const accent = L.accent !== 'env' ? L.accent : T.ownAccent ? T.accent : envAccent(T, env) || T.accent;
    const [rp, rs] = RADIUS[L.radius];
    const pa = paneAlpha(L), A = alphas(L);
    const v = {
      '--canvas': T.canvas, '--bg': T.bg, '--panel': T.panel, '--raised': T.raised, '--raised2': T.raised2, '--line': T.line, '--line2': T.line2,
      '--ink': T.ink, '--ink2': T.ink2, '--ink3': T.ink3, '--accent': accent, '--cursor': T.cursor, '--sel': T.selection,
      '--human': T.human, '--human-bg': T.humanBg, '--human-line': T.humanLine, '--on-human': T.onHuman,
      '--ok': T.ok, '--ok-bg': T.okBg, '--bad': T.bad, '--bad-bg': T.badBg, '--btn': T.btn, '--on-btn': T.onBtn,
      '--pane-bg': pa >= 1 ? T.panel : rgba(T.panel, pa),
      '--glass': Math.round(A.canvas * 100) + '%', '--glass-s': Math.round(A.surface * 100) + '%', '--window-fx': windowFx(T, L), '--theme-fx': themeFx(T, L),
      '--radius-pane': rp + 'px', '--radius-sm': rs + 'px', '--r': rs + 'px', '--gap': L.gap + 'px',
      '--mono': FONTS[L.font], '--sans': SANS, '--fs': L.font_px + 'px',
    };
    for (const [k, c] of Object.entries(T.studio)) v['--' + k.replace(/[A-Z]/g, (m) => '-' + m.toLowerCase())] = c;
    v['--amber'] = accent; v['--needle'] = accent;      // v7: the live accent follows the env in focus
    for (const k of Object.keys(ENV_ACCENT.dark)) v['--env-' + k] = envAccent(T, k);   // v7: every agent card wears its env
    for (const [k, c] of Object.entries(T.roles)) v['--' + k] = c;
    T.ansi.forEach((c, i) => { v['--ansi-' + i] = c; });
    return v;
  }
  // the env colour on this theme: the table of its scheme; an own theme moves it toward the text until it reads on the panel
  function envAccent(T, env) {
    const c = (ENV_ACCENT[T.scheme] || {})[env];
    return c && (T.custom || T.adapt) ? readable(c, [T.panel], T.ink, 4.5) : c;
  }
  const ANSI_NAMES = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white'];
  // the theme object of xterm 6 (task 23 passes it to `new Terminal({ theme })` and to `term.options.theme` on a change)
  function xterm(name, lk) {
    const L = look(lk), T = table(name, L), pa = paneAlpha(L);
    const X = T.term;   // selection = a translucent accent, so the text under it keeps its colour
    const o = { background: pa >= 1 ? X.background : rgba(X.background, pa), foreground: X.foreground, cursor: X.cursor, cursorAccent: X.background, selectionBackground: rgba(T.accent, X.selection) };
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
    const name = resolve(L.theme, dark), T = table(name, L);
    const el = document.documentElement;
    for (const [k, v] of Object.entries(vars(name, L, envNow))) el.style.setProperty(k, v);
    el.setAttribute('data-theme', T.scheme);           // tokens.css and the screens know two schemes; the look id rides apart
    el.setAttribute('data-look', T.custom ? 'custom' : name);
    el.style.setProperty('color-scheme', T.scheme);
    el.setAttribute('data-radius', L.radius);
    el.toggleAttribute('data-glass', L.glass > 0);
    el.toggleAttribute('data-thin', paneAlpha(L) < 1);   // see-through panes: a terminal pane leaves its ground to the terminal
    el.toggleAttribute('data-fx', L.fx);
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
  root.NSTheme = { THEMES, BUILTIN, ENV_ACCENT, RADIUS, FONTS, PRESETS, LOOK0, GLASS_MAX, THEMES_MAX, look, fromSettings, resolve, vars, xterm, xtermFont, applyTheme, onTheme,
    table, custom, customSpec, exportTheme, importTheme, contrast, mix, paneAlpha, get current() { return current; } };
  if (root.NS) Object.assign(root.NS, { applyTheme, theme: root.NSTheme });
})(typeof window !== 'undefined' ? window : globalThis);
