// Runs ui/selftest.js headless (no window). The script list comes from ui/selftest.html, so the two cannot drift.
// Usage: node scripts/ui-selftest.js   (exit code 1 on any failed check)
const vm = require('vm');
const fs = require('fs');
const path = require('path');

const ui = path.join(__dirname, '..', 'ui');
const html = fs.readFileSync(path.join(ui, 'selftest.html'), 'utf8');
const files = [...html.matchAll(/<script\s+src="([^"]+)"/g)].map((m) => m[1]);
if (!files.includes('selftest.js')) { console.error('selftest.html lists no selftest.js'); process.exit(1); }

let log = '';
const ctx = vm.createContext({ console: { log: (s) => { log += s + '\n'; } }, Date, JSON });
for (const f of files) vm.runInContext(fs.readFileSync(path.join(ui, f), 'utf8'), ctx, { filename: f });
process.stdout.write(log.split('\n').filter((l) => !l.startsWith('PASS ')).join('\n'));
if (!/ALL PASSED/.test(log) || /^FAIL /m.test(log)) process.exit(1);

// ---------- v4 (task 22): checks on the stylesheets and, when a browser is at hand, on the drawn window ----------
// no screen keeps its own colour: hex colours live only in tokens.css (and the theme table of js/v4/theme.js);
// the first-paint values of tokens.css are the theme table; panes, cards and fields take their corners from the radius tokens.
const rgbOf = (hex) => { const n = parseInt(String(hex).slice(1), 16); return { r: n >> 16, g: (n >> 8) & 255, b: n & 255 }; };
const css = (f) => fs.readFileSync(path.join(ui, 'css', f), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
const out4 = [];
const check = (name, ok, detail) => out4.push((ok ? 'PASS ' : 'FAIL ') + name + (ok ? '' : ' ' + JSON.stringify(detail)));
const sheets = fs.readdirSync(path.join(ui, 'css')).filter((f) => f.endsWith('.css') && f !== 'tokens.css' && f !== 'selftest.css');
const stray = sheets.flatMap((f) => (css(f).match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(/gi) || []).map((m) => f + ': ' + m));
check('css: no colour outside tokens.css (screens use tokens only)', !stray.length, stray);
const tok = css('tokens.css');
const block = (re) => { const m = re.exec(tok); return m ? Object.fromEntries([...m[1].matchAll(/(--[\w-]+):\s*([^;]+);/g)].map((x) => [x[1], x[2].trim()])) : {}; };
const darkCss = block(/^:root\s*\{([\s\S]*?)\n\}/m), lightCss = block(/:root\[data-theme="light"\]\s*\{([\s\S]*?)\n\}/);
const T = ctx.NSTheme;
const drift = [];
for (const [name, got] of [['dark', darkCss], ['light', lightCss]]) {
  const want = T ? T.vars(name, {}) : {};
  for (const [k, v] of Object.entries(want)) if (/^#/.test(v) && k !== '--pane-bg' && got[k] !== v && !(k === '--accent' && got[k])) drift.push(name + ' ' + k + ' ' + got[k] + ' != ' + v);
}
check('tokens.css carries the theme table of theme.js (Dark and Light)', !!T && !drift.length, drift.slice(0, 6));
// F10: the window ground carries a radial gradient (the lamp), made of tokens; both themes define the studio tokens
check('F10: tokens.css gives the window a radial-gradient ground (--window-fx) and .app paints it', /--window-fx:[^;]*radial-gradient/.test(tok) && /\.app\s*\{[^}]*var\(--window-fx\)/.test(css('shell.css')), null);
check('F10: both themes carry the studio tokens (amber, cream, brass, needle, onair)', ['--amber', '--cream', '--brass', '--needle', '--onair'].every((k) => darkCss[k] && lightCss[k]), [darkCss['--cream'], lightCss['--cream']]);
check('F10: the dark canvas is warm (R > B) and the light canvas is cream', ((c) => c.r > c.b)(rgbOf(darkCss['--canvas'])) && ((c) => c.r > c.b && c.r > 200)(rgbOf(lightCss['--canvas'])), [darkCss['--canvas'], lightCss['--canvas']]);
const px = (v) => parseFloat(String(v || '').replace('var(--radius-pane)', darkCss['--radius-pane']).replace('var(--radius-sm)', darkCss['--radius-sm']).replace('var(--r)', darkCss['--radius-sm']));
const all = sheets.map(css).join('\n');
const radiusOf = (sel) => { const re = new RegExp('(^|[},\\s])' + sel.replace(/[.[\]=]/g, '\\$&') + '\\s*[,{][^}]*?border-radius:\\s*([^;}]+)', 'gm'); let m, last = null; while ((m = re.exec(all))) last = m[2].split(/\s/)[0]; return last; };
const corners = { '.pane': radiusOf('.pane'), '.tcard': radiusOf('.tcard'), '.composer': radiusOf('.composer'), 'input[type=text]': radiusOf('input[type=text]'), '.card4': radiusOf('.card4'), '.bar': radiusOf('.bar') };
check('corners: pane, task card, input field, cards and the bar are rounded (border-radius > 0 from the tokens)', Object.values(corners).every((v) => px(v) > 0), corners);
process.stdout.write('\n' + out4.join('\n') + '\n');
if (out4.some((l) => l.startsWith('FAIL'))) process.exitCode = 1;

// the drawn window (dev-mock): ≤ 14 visible buttons in the main view outside the feed and Clawd, computed corners > 0 - needs playwright-core
// (NS_PLAYWRIGHT=<path to playwright-core>, NS_CHROME=<executable>); without it the check is skipped, not passed
if (process.env.NS_PLAYWRIGHT) (async () => {
  const http = require('http');
  const srv = http.createServer((q, r) => { const f = path.join(ui, decodeURIComponent(q.url.split('?')[0])); fs.readFile(f, (e, b) => { if (e) { r.statusCode = 404; return r.end(); } r.setHeader('content-type', f.endsWith('.js') ? 'text/javascript' : f.endsWith('.css') ? 'text/css' : f.endsWith('.svg') ? 'image/svg+xml' : 'text/html'); r.end(b); }); }).listen(0);
  const { chromium } = require(process.env.NS_PLAYWRIGHT);
  const launch = () => chromium.launch(process.env.NS_CHROME ? { executablePath: process.env.NS_CHROME } : {});
  let b = await launch();
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  await p.goto('http://127.0.0.1:' + srv.address().port + '/index.html?lang=ru&platform=mac');
  await p.waitForTimeout(1200);
  const r = await p.evaluate(() => {
    const vis = (el) => { const s = getComputedStyle(el), b = el.getBoundingClientRect(); return b.width > 0 && b.height > 0 && s.visibility !== 'hidden' && +s.opacity > 0; };
    const buttons = [...document.querySelectorAll('button, [role=button]')].filter((x) => !x.closest('.cfeed, .clawd') && vis(x)); // Clawd is a character in the tab corner, not a control
    const rad = (sel) => { const el = document.querySelector(sel); return el ? parseFloat(getComputedStyle(el).borderTopLeftRadius) : -1; };
    return { buttons: buttons.length, names: buttons.map((x) => x.getAttribute('aria-label') || x.textContent.trim()).slice(0, 20), pane: rad('.pane'), card: rad('.tcard'), field: rad('.composer'), bar: rad('.bar') };
  });
  const out = [];
  out.push((r.buttons <= 20 ? 'PASS ' : 'FAIL ') + 'main view: at most 20 visible buttons outside the feed (14 + the three entrances of F8 + "Новый чат" of the chat list that F9 shows by default + "Спросить" of F12 + "Автоматика" of F14) (' + r.buttons + ')' + (r.buttons <= 20 ? '' : ' ' + JSON.stringify(r.names)));
  out.push((r.pane > 0 && r.card > 0 && r.field > 0 && r.bar > 0 ? 'PASS ' : 'FAIL ') + 'computed border-radius > 0: pane ' + r.pane + ', card ' + r.card + ', field ' + r.field + ', bar ' + r.bar);
  await f5(b, 'http://127.0.0.1:' + srv.address().port, out);
  await f7(b, 'http://127.0.0.1:' + srv.address().port, out);
  await b.close(); b = await launch();       // F8 runs in a fresh browser: after F5 and F7 in one browser it died on the next page (headless Chrome on macOS)
  await f8(b, 'http://127.0.0.1:' + srv.address().port, out);
  await b.close(); b = await launch();       // F9 in a fresh browser too
  await f9(b, 'http://127.0.0.1:' + srv.address().port, out);
  await b.close(); b = await launch();       // F10 in a fresh browser too
  await f10(b, 'http://127.0.0.1:' + srv.address().port, out);
  await b.close(); b = await launch();       // F11 in a fresh browser too
  await f11(b, 'http://127.0.0.1:' + srv.address().port, out);
  await b.close(); b = await launch();       // F13 in a fresh browser too
  await f13(b, 'http://127.0.0.1:' + srv.address().port, out);
  await b.close(); b = await launch();       // F12 in a fresh browser too
  await f12(b, 'http://127.0.0.1:' + srv.address().port, out);
  await b.close(); b = await launch();       // F14 in a fresh browser too
  await f14(b, 'http://127.0.0.1:' + srv.address().port, out);
  await b.close(); b = await launch();       // F15 in a fresh browser too
  await f15(b, 'http://127.0.0.1:' + srv.address().port, out);
  process.stdout.write(out.join('\n') + '\n');
  if (out.some((l) => l.startsWith('FAIL'))) process.exitCode = 1;
  await b.close(); srv.close();
})().catch((e) => { console.error('browser check: ' + e.stack); process.exit(1); });
else process.stdout.write('SKIP drawn-window checks (set NS_PLAYWRIGHT to run them)\n');

// F5 - bugs of the live run v4 on the drawn window (dev-mock): autocorrect in the search fields (249), ⌘W on a busy terminal
// in a split (253), the extra window: its label and the scrollback of the moved terminal (260)
async function f5(b, base, out) {
  const ok = (name, cond, detail) => out.push((cond ? 'PASS ' : 'FAIL ') + name + (cond ? '' : ' ' + JSON.stringify(detail)));
  const QUIET = { autocorrect: 'off', autocapitalize: 'off', spellcheck: 'false', autocomplete: 'off' };
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  await p.goto(base + '/index.html?lang=ru&platform=mac&term=1');
  await p.waitForTimeout(1500);
  const attrs = (sel) => p.evaluate((sel) => { const el = document.querySelector(sel); return el ? Object.fromEntries(['autocorrect', 'autocapitalize', 'spellcheck', 'autocomplete'].map((k) => [k, el.getAttribute(k)])) : null; }, sel);
  const fields = {};
  await p.evaluate(() => NS.palette.open()); await p.waitForTimeout(200);
  fields.palette = await attrs('.pal-in');
  await p.evaluate(() => NS.palette.close());
  await p.evaluate(() => NS.market.open('skill', NS.store.s.lastEnv)); await p.waitForTimeout(400);
  fields.catalog = await attrs('.mk-q');
  await p.evaluate(() => NS.history.open(NS.store.s.projects[0] && NS.store.s.projects[0].id)); await p.waitForTimeout(400);
  fields.history = await attrs('.mk-q');
  await p.evaluate(() => NS.nav('settings', { env: NS.store.s.lastEnv, section: 'advanced' })); await p.waitForTimeout(400);
  fields.keys = await attrs('.adv-q');
  const badF = Object.entries(fields).filter(([, a]) => JSON.stringify(a) !== JSON.stringify(QUIET));
  ok('F5 249: palette, catalog, history and key search fields carry autocorrect/autocapitalize off, spellcheck false, autocomplete off', !badF.length, badF);

  // 253: a split of two terminals, the right one runs `sleep 300`; ⌘W asks and the layout waits; "Отмена" keeps the split
  await p.goto(base + '/index.html?lang=ru&platform=mac&term=1');
  await p.waitForTimeout(1500);
  await p.evaluate(() => NS.term.open(null, 'h')); await p.waitForTimeout(800);
  const layout = () => p.evaluate(() => { const tb = NS.tabs.activeTab; return { root: JSON.stringify(tb.root, (k, v) => (k === 'termId' || k === 'ptitle' ? undefined : v)), focus: tb.focus, tabs: NS.tabs.state.tabs.length, panes: document.querySelectorAll('.panes .pane:not([hidden])').length }; });
  await p.evaluate(() => { const v = NS.term.views.get(NS.tabs.active.id); v.write('sleep 300\r'); });
  await p.waitForTimeout(2000);
  const before = await layout();
  await p.keyboard.press('Meta+KeyW');
  await p.waitForTimeout(400);
  const asking = await p.evaluate(() => !!document.querySelector('dialog[open]'));
  const during = await layout();
  await p.evaluate(() => [...document.querySelectorAll('dialog[open] button')].find((x) => x.textContent === NS.t('dlg.cancel')).click());
  await p.waitForTimeout(400);
  const after = await layout();
  ok('F5 253: ⌘W on a busy terminal in a split asks, and until the answer the layout is the same', asking && before.root.includes('"split"') && JSON.stringify(during) === JSON.stringify(before), { asking, before, during });
  ok('F5 253: "Отмена" leaves the same split with the same pane in focus', JSON.stringify(after) === JSON.stringify(before), { before, after });

  // 260: the extra window (dev-mock: window_tab gives a session with 80 lines of scrollback)
  const w = await b.newPage({ viewport: { width: 820, height: 560 } });
  await w.goto(base + '/index.html?lang=ru&platform=mac&window=w77');
  await w.waitForTimeout(2500);
  const win = await w.evaluate(() => { const v = NS.term.views.get('win'); const btn = document.querySelector('.t-win .t-head .btn'); return { label: btn && btn.textContent, text: v ? v.text() : '' }; });
  ok('F5 260: the button of the extra window speaks the language of the window («Вернуть в главное окно»)', win.label === 'Вернуть в главное окно', win.label);
  ok('F5 260: the terminal of the extra window gets the buffer of the session (its scrollback)', /history line 1\n[\s\S]*history line 80/.test(win.text), win.text.slice(0, 200));
  await w.close(); await p.close();
}

// F7 - the short model menu over the button (size, keys, version chips), the model rows in the palette, the terminal defaults
async function f7(b, base, out) {
  const ok = (name, cond, detail) => out.push((cond ? 'PASS ' : 'FAIL ') + name + (cond ? '' : ' ' + JSON.stringify(detail)));
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  await p.goto(base + '/index.html?lang=ru&platform=mac');
  await p.waitForTimeout(1500);
  const fam = () => p.evaluate(() => { const d = NS.store.envSettings('claude').default_model; return d && [d.family, d.version]; });
  await p.evaluate(() => NS.store.saveEnvSettings('claude', { default_model: { family: 'opus', version: null } }));
  await p.click('.composer .envpick'); await p.waitForTimeout(300);
  const m = await p.evaluate(() => { const r = document.querySelector('.mmenu').getBoundingClientRect(), bt = document.querySelector('.composer .envpick').getBoundingClientRect();
    return { w: r.width, in: r.left >= 0 && r.right <= innerWidth && r.top >= 0, above: r.bottom <= bt.top, rows: [...document.querySelectorAll('.mrow')].map((x) => x.getBoundingClientRect().height) }; });
  ok('F7 1: the model menu is <= 320 px wide, inside the window, above the button; every model is one row <= 36 px', m.w <= 320 && m.in && m.above && m.rows.length >= 3 && m.rows.every((h) => h <= 36), m);
  await p.keyboard.press('ArrowDown'); await p.keyboard.press('ArrowDown'); await p.keyboard.press('Enter'); await p.waitForTimeout(300);
  const shut = await p.evaluate(() => document.querySelector('.mmenu').hidden);
  ok('F7 2: ↓ ↓ Enter saves the second model and closes the menu', shut && (await fam())[0] === 'sonnet', [shut, await fam()]);
  await p.click('.composer .envpick'); await p.waitForTimeout(200);
  await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  const esc = await p.evaluate(() => ({ hidden: document.querySelector('.mmenu').hidden, onBtn: document.activeElement === document.querySelector('.composer .envpick') }));
  ok('F7 2: Esc closes the menu and the focus is back on the button', esc.hidden && esc.onBtn, esc);
  await p.click('.composer .envpick'); await p.waitForTimeout(200);
  await p.click('.mmenu .vers .v >> text=4.6'); await p.waitForTimeout(300);
  ok('F7 2: a version chip saves the version and closes the menu', JSON.stringify(await fam()) === '["sonnet","4.6"]' && await p.evaluate(() => document.querySelector('.mmenu').hidden), await fam());
  // 3: the palette
  await p.evaluate(() => NS.store.saveEnvSettings('claude', { default_model: { family: 'opus', version: null } }));
  await p.evaluate(() => NS.palette.open('sonnet')); await p.waitForTimeout(200);
  const rows = await p.evaluate(() => [...document.querySelectorAll('.pal-o .pal-t')].map((x) => x.textContent));
  await p.keyboard.press('Enter'); await p.waitForTimeout(300);
  ok('F7 3: "sonnet" in the palette gives "Модель: Sonnet 5.5"; Enter saves it for the env', rows[0] === 'Модель: Sonnet 5.5' && (await fam())[0] === 'sonnet', [rows, await fam()]);
  ok('F7 3: the label in the input field follows a model chosen in the palette', (await p.textContent('.composer .envpick')).trim() === 'Sonnet 5.5', await p.textContent('.composer .envpick'));
  // a long list gets search: opencode has 10 models
  await p.evaluate(() => { const e = NS.store.s.envs.find((x) => x.id === 'opencode'); if (e) NS.shell.pickEnv('opencode'); }); await p.waitForTimeout(800);
  await p.click('.composer .envpick'); await p.waitForTimeout(300);
  const long = await p.evaluate(() => { const r = document.querySelector('.mmenu').getBoundingClientRect(); return { search: document.activeElement === document.querySelector('.mmenu .msearch'), h: r.height, top: r.top }; });
  ok('F7 1: a long list opens with the focus in the search field, the menu stays <= 360 px and inside the window', long.search && long.h <= 361 && long.top >= 0, long);
  await p.keyboard.type('qwen'); await p.waitForTimeout(200);
  ok('F7 1: the search filters the rows as you type', (await p.evaluate(() => document.querySelectorAll('.mrow[role=option]').length)) <= 4, await p.evaluate(() => document.querySelectorAll('.mrow[role=option]').length));
  // 4: terminal defaults, then the config wins
  await p.goto(base + '/index.html?lang=ru&platform=mac&term=1'); await p.waitForTimeout(1800);
  const o = () => p.evaluate(() => { const v = NS.term.views.get(NS.tabs.active.id), t = v.term.options; return { fontSize: t.fontSize, lineHeight: t.lineHeight, cursorStyle: t.cursorStyle }; });
  const d = await o();
  ok('F7 4: xterm defaults - fontSize >= 13, lineHeight >= 1.25, cursorStyle bar', d.fontSize >= 13 && d.lineHeight >= 1.25 && d.cursorStyle === 'bar', d);
  await p.evaluate(() => { const st = NS.store.s.settings; st.ui = st.ui || {}; st.ui.tabs = st.ui.tabs || {}; st.ui.tabs.look = Object.assign({}, st.ui.tabs.look, { term: { size: 18, cursor: 'underline' } }); NS.term.views.get(NS.tabs.active.id).applyLook(); });
  const c = await o();
  ok('F7 4: size and cursor set in the config win over the defaults', c.fontSize === 18 && c.cursorStyle === 'underline', c);
  await p.close();
}

// F8 - clarity: visible switches, the entrances to the factory / settings / palette, the "+" menu, a terminal that ended
async function f8(b, base, out) {
  const ok = (name, cond, detail) => out.push((cond ? 'PASS ' : 'FAIL ') + name + (cond ? '' : ' ' + JSON.stringify(detail)));
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  await p.goto(base + '/index.html?lang=ru&platform=mac');
  await p.waitForTimeout(1500);
  // 1: the switches
  const sw = await p.evaluate(() => [...document.querySelectorAll('.composer .tg')].map((g) => { const r = g.querySelector('.sw').getBoundingClientRect(); return { w: r.width, h: r.height, title: g.title, on: g.getAttribute('aria-checked'), text: g.textContent.trim() }; }));
  ok('F8 1: the switches "Качество" and "Рой" are drawn (non-zero .sw) and each has a title', sw.length === 2 && sw.every((x) => x.w > 0 && x.h > 0 && x.title.length > 10) && sw[0].text === 'Качество' && sw[1].text === 'Рой', sw);
  const st = () => p.evaluate(() => { const e = NS.store.envSettings(NS.store.s.lastEnv); return { q: e.last_quality && e.last_quality.on, s: !!e.last_swarm }; });
  await p.click('.composer .tg >> nth=0'); await p.waitForTimeout(250);
  await p.click('.composer .tg >> nth=1'); await p.waitForTimeout(250);
  const after = await p.evaluate(() => [...document.querySelectorAll('.composer .tg')].map((g) => g.getAttribute('aria-checked')));
  const saved = await st();
  ok('F8 1: a click flips aria-checked and saves last_quality.on / last_swarm', after[0] !== sw[0].on && after[1] !== sw[1].on && saved.q === (after[0] === 'true') && saved.s === (after[1] === 'true'), { sw, after, saved });
  const tune = await p.evaluate(() => { const t = document.querySelector('.composer .tune4'); const r = t.getBoundingClientRect(); return { w: r.width, text: t.textContent.trim(), icon: !!t.querySelector('svg'), title: t.title }; });
  ok('F8 1: "⋯" is a gear with the word "Настроить" and a title', tune.text === 'Настроить' && tune.icon && tune.w > 50 && tune.title.length > 10, tune);
  // 2: the entrances
  const here = () => p.evaluate(() => NS.shell.current && NS.shell.current.def.id);
  const vis = await p.evaluate(() => ['.navb.nb-fact', '.navb.nb-set', '.navb.nb-pal'].map((q) => { const e = document.querySelector(q); const r = e && e.getBoundingClientRect(); return e ? { w: r.width, text: e.textContent.trim() } : null; }));
  ok('F8 2: "Фабрика", "Настройки" and "⌘K Команды" are visible in the tab strip', vis.every((x) => x && x.w > 20) && /Фабрика/.test(vis[0].text) && /Настройки/.test(vis[1].text) && /Команды/.test(vis[2].text), vis);
  await p.click('.navb.nb-fact'); await p.waitForTimeout(500);
  const fact = await here();
  await p.evaluate(() => NS.shell.back()); await p.waitForTimeout(400);
  await p.click('.navb.nb-set'); await p.waitForTimeout(500);
  const set = await here();
  await p.evaluate(() => NS.shell.back()); await p.waitForTimeout(400);
  await p.evaluate(() => document.dispatchEvent(new KeyboardEvent('keydown', { code: 'Comma', key: ',', metaKey: true, bubbles: true, cancelable: true }))); await p.waitForTimeout(500);   // a synthetic key: a real ⌘, would reach the browser itself
  const comma = await here();
  await p.evaluate(() => NS.shell.back()); await p.waitForTimeout(400);
  ok('F8 2: the buttons open the factory and the settings, ⌘, opens the settings', fact === 'factory' && set === 'settings' && comma === 'settings', { fact, set, comma });
  await p.click('.navb.nb-pal'); await p.waitForTimeout(300);
  ok('F8 2: "⌘K Команды" opens the palette', await p.evaluate(() => !!document.querySelector('.pal-in') && NS.palette.isOpen), null);
  await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  // 3: the "+" menu (F11: the terminal opens in the project that was chosen, not in a guessed one)
  await p.evaluate(() => NS.projects.choose('p1'));
  await p.click('.tab-plus'); await p.waitForTimeout(250);
  const items = await p.evaluate(() => [...document.querySelectorAll('.plusmenu [role=menuitem]')].map((x) => x.textContent.trim()));
  ok('F8 3: "+" opens a menu of three items - chat, terminal with the CLI, shell', items.length === 3 && /Новый чат/.test(items[0]) && /Терминал с Claude Code/.test(items[1]) && /Терминал \(shell\)/.test(items[2]), items);
  await p.click('.plusmenu [role=menuitem] >> nth=2'); await p.waitForTimeout(900);
  const term = await p.evaluate(() => ({ kind: NS.tabs.active.kind, glyph: (document.querySelector('.tab.on .tab-ic .t-glyph') || {}).textContent, label: document.querySelector('.tab.on .tab-l').textContent, menu: !!document.querySelector('.plusmenu') }));
  ok('F8 3: "Терминал (shell)" opens a terminal tab with the >_ icon, named "Терминал · …"; the menu closes', term.kind === 'term' && term.glyph === '>_' && /^Терминал · /.test(term.label) && !term.menu, term);
  await p.click('.tab-plus'); await p.waitForTimeout(250);
  await p.click('.plusmenu [role=menuitem] >> nth=0'); await p.waitForTimeout(900);
  const chat = await p.evaluate(() => ({ kind: NS.tabs.active.kind, glyph: !!document.querySelector('.tab.on .tab-ic .t-glyph'), img: !!document.querySelector('.tab.on .tab-ic svg, .tab.on .tab-ic img') }));
  ok('F8 3: "Новый чат" opens a chat tab with the icon of the environment, not >_', chat.kind === 'agent' && !chat.glyph && chat.img, chat);
  ok('F8 4: the empty chat carries the hint about the terminal', await p.evaluate(() => /Нужен сам Claude Code в терминале/.test((document.querySelector('.cmp-note') || {}).textContent || '')), null);
  const barBtn = await p.evaluate(() => { const x = document.querySelector('.t-barbtn'); return x && { text: x.textContent.trim(), title: x.title }; });
  ok('F8 3: the status bar button reads ">_ Терминал" and says what it opens', barBtn && barBtn.text === '>_Терминал' || (barBtn && /Терминал/.test(barBtn.text) && /Claude Code в терминале/.test(barBtn.title)), barBtn);
  await p.close();
  // 4: a terminal whose process ended
  const q = await b.newPage({ viewport: { width: 1440, height: 900 } });
  await q.goto(base + '/index.html?lang=ru&platform=mac&term=1');
  await q.waitForTimeout(1800);
  await q.evaluate(() => NS.term.views.get(NS.tabs.active.id).write('exit 2\r')); await q.waitForTimeout(700);
  const dead = await q.evaluate(() => { const d = document.querySelector('.t-dead'); return { shown: !!d && !d.hidden && d.getBoundingClientRect().width > 0, btns: [...document.querySelectorAll('.t-dead button')].map((x) => x.textContent) }; });
  ok('F8 5: after the process exits the card shows "Запустить снова" and "Закрыть вкладку"', dead.shown && dead.btns.join('|') === 'Запустить снова|Закрыть вкладку', dead);
  await q.click('.t-dead-run'); await q.waitForTimeout(900);
  const again = await q.evaluate(() => ({ hidden: document.querySelector('.t-dead').hidden, running: NS.term.views.get(NS.tabs.active.id).running() }));
  ok('F8 5: "Запустить снова" restarts the process', again.hidden && again.running, again);
  await q.evaluate(() => NS.term.views.get(NS.tabs.active.id).write('exit 0\r')); await q.waitForTimeout(700);
  const n0 = await q.evaluate(() => NS.tabs.state.tabs.length);
  await q.click('.t-dead-x'); await q.waitForTimeout(700);
  ok('F8 5: "Закрыть вкладку" closes the tab', await q.evaluate((n) => NS.tabs.state.tabs.length < n || !document.querySelector('.t-dead:not([hidden])'), n0), n0);
  await q.close();
}

// F9 - the minimal chat on the drawn window (dev-mock): the column, steps folded, bubbles, the empty chat, the chat list on a clean start, the plate with "Стоп"
async function f9(b, base, out) {
  const ok = (name, cond, detail) => out.push((cond ? 'PASS ' : 'FAIL ') + name + (cond ? '' : ' ' + JSON.stringify(detail)));
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  await p.goto(base + '/index.html?lang=ru&platform=mac');
  await p.waitForTimeout(1600);
  // 5: a clean start shows the chat list; ⌘B hides it and the choice survives a reload
  const side = () => p.evaluate(() => { const e = document.querySelector('.side'); return { on: document.body.classList.contains('side-on'), w: e ? e.getBoundingClientRect().width : 0, saved: NS.storage.get('ns.side') }; });
  const s0 = await side();
  ok('F9 5: on a clean start (ns.side not set) the chat list is visible, about 240 px', s0.on && s0.w > 200 && s0.w < 290 && s0.saved == null, s0);
  const key = () => p.evaluate(() => document.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyB', key: 'b', metaKey: true, bubbles: true, cancelable: true })));
  await key(); await p.waitForTimeout(250);
  const s1 = await side();
  await p.reload(); await p.waitForTimeout(1500);
  const s2 = await side();
  ok('F9 5: ⌘B hides the chat list and a reload remembers it', !s1.on && s1.w === 0 && s1.saved === '0' && !s2.on && s2.w === 0, { s1, s2 });
  await key(); await p.waitForTimeout(250);
  ok('F9 5: ⌘B shows it again', (await side()).on, null);
  // 1 and 3: the column, the type, the bubbles
  const L = await p.evaluate(() => {
    const feed = document.querySelector('.cfeed'), fr = feed.getBoundingClientRect();
    const msg = document.querySelector('.msg .msg-b'), me = document.querySelector('.me .me-b');
    const cs = (el) => getComputedStyle(el), mr = me && me.getBoundingClientRect();
    return { feedW: fr.width, text: msg && cs(msg).fontFamily, size: msg && cs(msg).fontSize, lh: msg && cs(msg).lineHeight, mono: cs(document.documentElement).getPropertyValue('--mono'),
      meRight: mr && Math.round(fr.right - mr.right), meBg: me && cs(me).backgroundColor, meRadius: me && cs(me).borderTopLeftRadius, msgBg: msg && cs(msg).backgroundColor, msgRight: msg && Math.round(fr.right - msg.getBoundingClientRect().right) };
  });
  ok('F9 1: the feed column is at most 780 px wide on a 1440 window', L.feedW > 300 && L.feedW <= 780, L);
  ok('F9 1: the answer text is proportional (not the mono stack), 14-15 px', L.text && !/monospace|Menlo|Consolas/i.test(L.text) && parseFloat(L.size) >= 14 && parseFloat(L.size) <= 15, L);
  ok('F9 3: my message is a grey bubble pushed to the right edge of the column, the answer has no background', L.meBg && L.meBg !== 'rgba(0, 0, 0, 0)' && L.meRight <= 20 && parseFloat(L.meRadius) >= 12 && L.msgBg === 'rgba(0, 0, 0, 0)' && L.msgRight <= 20, L);
  // 2: steps are folded, a click opens the diff, a group folds into one line
  const f0 = await p.evaluate(() => ({ lines: document.querySelectorAll('.cfeed .stp-h, .cfeed .agrp-h').length, diffs: [...document.querySelectorAll('.cfeed .dg')].filter((x) => x.getBoundingClientRect().height > 0).length,
    groups: [...document.querySelectorAll('.cfeed .agrp')].map((g) => ({ head: g.querySelector('.agrp-h .stp-v').textContent, shown: !g.querySelector('.agrp-b').hidden })) }));
  ok('F9 2: the demo chat has step lines and groups ("3 действия") folded, no diff is visible', f0.lines >= 3 && f0.diffs === 0 && f0.groups.length >= 2 && f0.groups.every((g) => !g.shown) && f0.groups.some((g) => /^3 действия$/.test(g.head)), f0);
  await p.evaluate(() => { for (const g of document.querySelectorAll('.cfeed .agrp-h')) g.click(); });
  await p.waitForTimeout(150);
  await p.evaluate(() => document.querySelector('.cfeed .stp.k-edit .stp-h').click()); await p.waitForTimeout(200);
  const f1 = await p.evaluate(() => { const d = [...document.querySelectorAll('.cfeed .dg')].find((x) => x.getBoundingClientRect().height > 0); return { diff: !!d, mono: d && getComputedStyle(d).fontFamily, add: d ? d.querySelectorAll('.dl.a').length : 0, del: d ? d.querySelectorAll('.dl.d').length : 0 }; });
  ok('F9 2: a click on "Изменил …" opens the tinted diff in a monospace font', f1.diff && /monospace|Menlo|Consolas/i.test(f1.mono) && f1.add > 0 && f1.del > 0, f1);
  await p.evaluate(() => document.querySelector('.cfeed .stp.k-edit .stp-h').click()); await p.waitForTimeout(150);
  ok('F9 2: a second click folds it again', await p.evaluate(() => ![...document.querySelectorAll('.cfeed .dg')].some((x) => x.getBoundingClientRect().height > 0)), null);
  // 4: the empty chat
  await p.evaluate(() => NS.tabs.newTab()); await p.waitForTimeout(900);
  const e0 = await p.evaluate(() => { const r = (q) => { const e = document.querySelector(q); return e ? e.getBoundingClientRect() : null; }; const ta = r('.composer textarea'), h2 = r('.newchat-empty h2');
    return { title: (document.querySelector('.newchat-empty h2') || {}).textContent, h2: h2 && h2.height, ta: ta && ta.width, ex: [...document.querySelectorAll('.examples:not([hidden]) .ex')].filter((x) => x.getBoundingClientRect().height > 0).map((x) => x.querySelector('b').textContent), exVisible: !!r('.examples') && r('.examples').height > 0,
      ph: (document.querySelector('.composer textarea') || {}).placeholder, keys: (document.querySelector('.cmp-keys') || {}).textContent, radius: parseFloat(getComputedStyle(document.querySelector('.cmpbox .composer')).borderTopLeftRadius) }; });
  ok('F9 4: the empty chat shows "Что сделать?", the field with a short placeholder and three examples', e0.title === 'Что сделать?' && e0.ta > 400 && e0.exVisible && e0.ex.length === 3 && e0.ph === 'Что сделать?' && /Enter - отправить/.test(e0.keys) && e0.radius >= 16, e0);
  const before = await p.evaluate(() => document.querySelectorAll('.me').length);
  await p.click('.examples:not([hidden]) .ex >> nth=0'); await p.waitForTimeout(300);
  const e1 = await p.evaluate(() => ({ v: document.querySelector('.composer textarea').value, me: document.querySelectorAll('.me').length }));
  ok('F9 4: a click on an example puts the text into the field and does not send it', /^Сделай/.test(e1.v) && e1.me === before, e1);
  await p.close();
  // 6: the plate over the field and "Стоп" while a task runs
  const q = await b.newPage({ viewport: { width: 1440, height: 900 } });
  await q.goto(base + '/index.html?lang=ru&platform=mac&env=claude&chat=cache');
  await q.waitForTimeout(3800);
  const w0 = await q.evaluate(() => { const pl = document.querySelector('.workbar'), sb = document.querySelector('.stop4'); return { plate: pl && !pl.hidden && pl.getBoundingClientRect().height > 0, text: pl && pl.textContent, stop: sb && !sb.hidden && sb.getBoundingClientRect().width > 0, send: document.querySelector('#sendBtn').hidden }; });
  ok('F9 6: while the agent works a plate "Исполнитель · правит …" and "Стоп" show over the field, the round button is a stop square', w0.plate && /Исполнитель · /.test(w0.text) && /Стоп/.test(w0.text) && w0.stop && w0.send, w0);
  await q.click('.workbar .wb-stop'); await q.waitForTimeout(700);
  const w1 = await q.evaluate(() => ({ plate: !!document.querySelector('.workbar') && !document.querySelector('.workbar').hidden, status: [...NS.store.s.tasks.values()].find((x) => x.id === 't-cache').status }));
  ok('F9 6: "Стоп" cancels the task and the plate goes away', w1.status === 'cancelled' && !w1.plate, w1);
  await q.close();
  // 3: a failed command stays open only while it is the last event
  const r = await b.newPage({ viewport: { width: 1440, height: 900 } });
  await r.goto(base + '/index.html?lang=ru&platform=mac&env=codex&chat=pf');
  await r.waitForTimeout(1800);
  const x0 = await r.evaluate(() => { const bad = document.querySelector('.cfeed .stp.s-bad'); return { bad: !!bad, open: !!bad && !bad.querySelector('.stp-b').hidden, text: bad ? bad.querySelector('.stp-b').textContent : '' }; });
  ok('F9 3: a failed command is a red line, open by default when it is the last event', x0.bad && x0.open && /Cannot find module/.test(x0.text), x0);
  await r.close();
  const r2 = await b.newPage({ viewport: { width: 1440, height: 900 } });
  await r2.goto(base + '/index.html?lang=ru&platform=mac');
  await r2.waitForTimeout(1600);
  await r2.evaluate(() => { for (const g of document.querySelectorAll('.cfeed .agrp-h')) g.click(); });
  const x1 = await r2.evaluate(() => [...document.querySelectorAll('.cfeed .stp.s-bad')].map((e) => !e.querySelector('.stp-b').hidden));
  ok('F9 3: a failed command that is not the last event is folded', x1.length >= 1 && x1.every((v) => !v), x1);
  await r2.close();
}

// F10 - the night radio studio: the ground, the tally lamps, the VU meters, the contrast of the drawn text (dev-mock)
async function f10(b, base, out) {
  const ok = (name, cond, detail) => out.push((cond ? 'PASS ' : 'FAIL ') + name + (cond ? '' : ' ' + JSON.stringify(detail)));
  const mix = (css) => { const m = String(css).match(/[\d.]+/g) || []; return m.slice(0, 3).map(Number); };
  const lum = (c) => c.map((v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); }).reduce((a, v, i) => a + v * [0.2126, 0.7152, 0.0722][i], 0);
  const ratio = (a, c) => { const x = lum(a), y = lum(c); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };
  for (const scheme of ['dark', 'light']) {
    const p = await b.newPage({ viewport: { width: 1440, height: 900 }, colorScheme: scheme });
    await p.goto(base + '/index.html?lang=ru&platform=mac');
    await p.waitForTimeout(1600);
    const R = await p.evaluate(() => {
      const cs = (el) => getComputedStyle(el), q = (s) => document.querySelector(s);
      const bgOf = (el) => { for (let e = el; e; e = e.parentElement) { const c = cs(e).backgroundColor; if (c && !/rgba\(.*, 0\)|transparent/.test(c)) return c; } return cs(document.body).backgroundColor; };
      const feed = q('.cfeed'), sec = q('.msg-h small') || q('.sysline'), pane = q('.pane');
      const lampOn = q('.tasks a .lamp.on-air'), lampCall = q('.tasks a .lamp.calling');
      const vu = q('.bm-lim svg.vu');
      return { appBg: cs(q('.app')).backgroundImage, canvas: cs(document.documentElement).getPropertyValue('--canvas').trim(), theme: document.documentElement.getAttribute('data-theme'),
        text: cs(feed).color, textBg: bgOf(feed), sec: sec && cs(sec).color, secBg: sec && bgOf(sec), paneBg: pane && cs(pane).backgroundColor,
        lampOn: !!lampOn, lampCall: !!lampCall, callAnim: lampCall && cs(lampCall).animationName, onAirBg: lampOn && cs(lampOn).backgroundImage,
        vu: !!vu, vuN: vu && vu.style.getPropertyValue('--n'), vuText: vu && vu.parentElement.textContent, needle: vu && !!vu.querySelector('.vu-needle'), arc: vu && !!vu.querySelector('.vu-arc'),
        gauge: !!q('.bm-lim .gauge'), stations: document.querySelectorAll('.tc-steps li i').length };
    });
    ok('F10 1 ' + scheme + ': the window ground is a radial-gradient (the lamp) over a ' + (scheme === 'dark' ? 'warm' : 'cream') + ' canvas', /radial-gradient/.test(R.appBg) && R.theme === scheme, R);
    const canvasRgb = (() => { const n = parseInt(R.canvas.slice(1), 16); return [n >> 16, (n >> 8) & 255, n & 255]; })();
    ok('F10 1 ' + scheme + ': the canvas is ' + (scheme === 'dark' ? 'warm (R > B)' : 'cream (light, R > B)'), canvasRgb[0] > canvasRgb[2] && (scheme === 'dark' || canvasRgb[0] > 200), canvasRgb);
    ok('F10 2 ' + scheme + ': the chat list has an on-air lamp (running) and an amber lamp for "Ждёт тебя" that pulses', R.lampOn && R.lampCall && R.callAnim && R.callAnim !== 'none' && /radial-gradient/.test(R.onAirBg), R);
    ok('F10 3 ' + scheme + ': the limits in the status bar are VU meters (svg: needle, arc), the numbers stay, no flat gauge', R.vu && R.needle && R.arc && !R.gauge && /\d+%/.test(R.vuText) && +R.vuN >= 0 && +R.vuN <= 1, R);
    ok('F10 4 ' + scheme + ': message text >= 7:1 and the quiet line >= 4.5:1 against what is drawn behind them', ratio(mix(R.text), mix(R.textBg)) >= 7 && (!R.sec || ratio(mix(R.sec), mix(R.secBg)) >= 4.5), { text: ratio(mix(R.text), mix(R.textBg)), sec: R.sec && ratio(mix(R.sec), mix(R.secBg)), R });
    ok('F10 3 ' + scheme + ': the stages of the task card are one signal line of stations', R.stations >= 7, R.stations);
    await p.close();
  }
  // 2: no pulse under prefers-reduced-motion
  const q = await b.newPage({ viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce' });
  await q.goto(base + '/index.html?lang=ru&platform=mac');
  await q.waitForTimeout(1600);
  const rm = await q.evaluate(() => { const l = document.querySelector('.tasks a .lamp.calling'), p = document.querySelector('.pane-lamp.calling'); return { lamp: l && getComputedStyle(l).animationName, pane: p && getComputedStyle(p).animationName, t: getComputedStyle(document.documentElement).getPropertyValue('--t').trim() }; });
  ok('F10 2: with prefers-reduced-motion the amber lamp does not pulse', rm.lamp === 'none' && (!rm.pane || rm.pane === 'none'), rm);
  await q.close();
}

// F11 - the project of a new chat is chosen, never guessed: the button, the menu, the create form
async function f11(b, base, out) {
  const ok = (name, cond, detail) => out.push((cond ? 'PASS ' : 'FAIL ') + name + (cond ? '' : ' ' + JSON.stringify(detail)));
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  await p.goto(base + '/index.html?lang=ru&platform=mac&env=claude&chat=new');
  await p.waitForTimeout(1500);
  const btn = () => p.evaluate(() => { const e = document.querySelector('.newchat-empty .pj-btn'); return e && { text: e.textContent.trim(), empty: e.classList.contains('empty') }; });
  const b0 = await btn();
  ok('F11 2: a new chat on a clean start has no project: the button says «Выбрать проект»', b0 && b0.text === 'Выбрать проект' && b0.empty, b0);
  await p.fill('.composer textarea', 'Сделай кнопку входа'); await p.keyboard.press('Enter'); await p.waitForTimeout(400);
  const sent = await p.evaluate(() => ({ menu: !document.querySelector('.pj-pop').hidden, chats: NS.store.s.chats ? [...NS.store.s.chats.values()].flat().length : 0, left: document.querySelector('.composer textarea').value, pending: document.querySelectorAll('.me.sending').length }));
  ok('F11 2: «Сделай…» without a project is not sent, the project menu opens and the text stays', sent.menu && sent.left === 'Сделай кнопку входа' && sent.pending === 0, sent);
  const m = await p.evaluate(() => ({ rows: [...document.querySelectorAll('.pj-pop .mrow .mn')].map((x) => x.firstChild.textContent), w: document.querySelector('.pj-pop').getBoundingClientRect().width }));
  ok('F11 3: the menu lists the recent projects, then «Открыть папку…» and «Создать проект…»', JSON.stringify(m.rows) === JSON.stringify(['billing-api', 'web-site', 'docs-tool', 'Открыть папку…', 'Создать проект…']), m);
  await p.click('.pj-pop .mrow:has-text("web-site")'); await p.waitForTimeout(300);
  const after = await p.evaluate(() => ({ text: document.querySelector('.newchat-empty .pj-btn').textContent.trim(), last: NS.store.s.lastProject, hdr: document.querySelector('.pane-sub') && document.querySelector('.pane-sub').textContent }));
  ok('F11 3: picking a recent project changes the project of the chat and is remembered', after.text === 'Проект: web-site' && after.last === 'p2' && /web-site/.test(after.hdr || ''), after);
  // open a folder: the system dialog (dev-mock answers with a folder) adds and selects it
  await p.click('.newchat-empty .pj-btn'); await p.click('.pj-pop .mrow:has-text("Открыть папку")'); await p.waitForTimeout(400);
  const opened = await p.evaluate(() => document.querySelector('.newchat-empty .pj-btn').textContent.trim());
  ok('F11 3: «Открыть папку…» adds the folder and selects it', opened === 'Проект: new-project', opened);
  // create: parent folder, name, the core makes the rest
  await p.click('.newchat-empty .pj-btn'); await p.click('.pj-pop .mrow:has-text("Создать проект")'); await p.waitForTimeout(300);
  const dis = await p.evaluate(() => document.querySelector('dialog[open] .btn.primary').disabled);
  await p.click('dialog[open] .pj-row .btn'); await p.waitForTimeout(200);
  await p.fill('dialog[open] .pj-name', 'a/b'); await p.waitForTimeout(100);
  const slash = await p.evaluate(() => ({ dis: document.querySelector('dialog[open] .btn.primary').disabled, err: !document.querySelector('.pj-err').hidden }));
  await p.fill('dialog[open] .pj-name', 'Мой сайт'); await p.keyboard.press('Enter'); await p.waitForTimeout(400);
  const made = await p.evaluate(() => ({ open: !!document.querySelector('dialog[open]'), text: document.querySelector('.newchat-empty .pj-btn').textContent.trim() }));
  ok('F11 3: «Создать проект…» needs a folder and a clean name, then creates and selects the project', dis && slash.dis && slash.err && !made.open && made.text === 'Проект: Мой сайт', { dis, slash, made });
  // the field now sends
  await p.fill('.composer textarea', 'Как это устроено?'); await p.keyboard.press('Enter'); await p.waitForTimeout(500);
  const went = await p.evaluate(() => !document.querySelector('.newchat-empty') || document.querySelector('.composer textarea').value === '');
  ok('F11 1: with a chosen project the message goes', went, went);
  await p.close();
}

// F13 - rollback to a turn, the list "До мержа", "Забрать к себе" / "Отдать агенту"
async function f13(b, base, out) {
  const ok = (name, cond, detail) => out.push((cond ? 'PASS ' : 'FAIL ') + name + (cond ? '' : ' ' + JSON.stringify(detail)));
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  await p.goto(base + '/index.html?lang=ru&platform=mac&env=claude&chat=inv');
  await p.waitForTimeout(1600);
  // 1 - the rollback icon on my reply to a running task (only where a snapshot follows the message)
  const rb = await p.evaluate(() => {
    const me = [...document.querySelectorAll('.me')].find((x) => /Курсор пусть будет/.test(x.textContent));
    const btn = me && me.querySelector('.snap-back');
    const first = [...document.querySelectorAll('.me')].find((x) => /Как у нас сейчас/.test(x.textContent));
    return { has: !!btn, shown: !!btn && !btn.hidden, text: btn && btn.textContent.trim(), plain: first ? !first.querySelector('.snap-back') || first.querySelector('.snap-back').hidden : null };
  });
  ok('F13 1: my reply to a task has «Вернуть код сюда»; a plain chat question has none', rb.has && rb.shown && rb.text === 'Вернуть код сюда' && rb.plain !== false, rb);
  await p.hover('.me:has-text("Курсор пусть будет")');
  await p.click('.me:has-text("Курсор пусть будет") .snap-back');
  const armed = await p.evaluate(() => [...document.querySelectorAll('.me')].find((x) => /Курсор пусть будет/.test(x.textContent)).querySelector('.snap-back').textContent.trim());
  ok('F13 1: the rollback asks once more before it moves the code', armed === 'Точно вернуть?', armed);
  await p.click('.me:has-text("Курсор пусть будет") .snap-back');
  await p.waitForTimeout(400);
  const toast = await p.evaluate(() => document.body.textContent.includes('Код возвращён к ходу 3'));
  ok('F13 1: after the second click the code goes back (to the snapshot after the message) and the state before is kept', toast, toast);
  // 2 - the card: the list, the turns
  await p.click('.tcard .tc-more');
  await p.waitForTimeout(500);
  const card = await p.evaluate(() => {
    const db = document.querySelector('.todos-db'), turns = document.querySelector('.turns-db');
    return { items: db ? [...db.querySelectorAll('.todos li')].map((li) => li.textContent.trim()) : [], open: db && db.querySelector('h3 small') && db.querySelector('h3 small').textContent, checked: db ? [...db.querySelectorAll('input[type=checkbox]')].map((i) => i.checked) : [],
      turns: turns ? turns.querySelectorAll('li').length : 0, backs: turns ? [...turns.querySelectorAll('.snap-back')].filter((x) => !x.hidden).length : 0 };
  });
  ok('F13 2: the card lists «До мержа» (one open, one from the reviewer ticked) and every turn has a rollback', card.items.length === 2 && card.open === '1 открыто' && card.checked.join() === 'false,true' && card.turns >= 3 && card.backs >= 3, card);
  await p.fill('.todos-db .todo-in', 'Проверить мобильный клиент');
  await p.keyboard.press('Enter');
  await p.waitForTimeout(400);
  await p.click('.todos-db li:has-text("Обновить docs") input');
  await p.waitForTimeout(400);
  const after = await p.evaluate(() => { const db = document.querySelector('.todos-db'); return { items: db.querySelectorAll('.todos li').length, open: db.querySelector('h3 small') && db.querySelector('h3 small').textContent }; });
  ok('F13 2: an item can be added and ticked; the count of open ones follows', after.items === 3 && after.open === '1 открыто', after);
  // the merge waits: the need block shows why and the list right in it
  const need = await p.evaluate(() => {
    const t = { ...NS.store.s.tasks.get('t-inv'), question: { kind: 'agent', text: 'todos_open: 1', options: ['Continue'] } };
    NS.store.s.tasks.set(t.id, t); NS.store.emit('task', t);
    const n = document.querySelector('.tcard .tc-need');
    return { text: n && n.textContent, boxes: n ? n.querySelectorAll('.todos input').length : 0, btn: n && [...n.querySelectorAll('button')].map((x) => x.textContent.trim()) };
  });
  ok('F13 2: a task held by the list says so in «Ждёт тебя», with the list and «Проверить ещё раз»', /Мерж ждёт/.test(need.text || '') && /не отмечено пунктов: 1/.test(need.text || '') && need.boxes === 3 && need.btn.includes('Проверить ещё раз'), need);
  await p.screenshot({ path: process.env.NS_SHOTS ? process.env.NS_SHOTS + '/f13-card.png' : '/dev/null' }).catch(() => {});
  // 3 - the folder handoff on a failed task
  await p.goto(base + '/index.html?lang=ru&platform=mac&env=codex&chat=pf');
  await p.waitForTimeout(1500);
  await p.click('.tcard .tc-more');
  await p.waitForTimeout(500);
  const names = () => p.evaluate(() => [...document.querySelectorAll('.tcard .acts-row .btn')].map((x) => x.textContent.trim()));
  const n0 = await names();
  ok('F13 3: a failed task offers «Забрать к себе»', n0.includes('Забрать к себе') && !n0.includes('Отдать агенту'), n0);
  await p.click('.tcard .acts-row .btn:has-text("Забрать к себе")');
  await p.waitForTimeout(500);
  const n1 = await p.evaluate(() => ({ btns: [...document.querySelectorAll('.tcard .btn')].map((x) => x.textContent.trim()), line: (document.querySelector('.tcard .tc-done') || {}).textContent }));
  ok('F13 3: taken: the card says the folder is on the task branch and offers «Отдать агенту»', n1.btns.includes('Отдать агенту') && !n1.btns.includes('Забрать к себе') && /Папка проекта стоит на ветке задачи/.test(n1.line || ''), n1);
  await p.click('.tcard .tc-done .btn:has-text("Отдать агенту")');
  await p.waitForTimeout(500);
  const n2 = await names();
  ok('F13 3: given back: «Забрать к себе» is offered again', n2.includes('Забрать к себе') && !n2.includes('Отдать агенту'), n2);
  await p.close();
}

// F12 - the chat: the queue and "Сразу", the side question, the next-step hint, "Выполнить" on shell blocks, "Остановлено"
async function f12(b, base, out) {
  const ok = (name, cond, detail) => out.push((cond ? 'PASS ' : 'FAIL ') + name + (cond ? '' : ' ' + JSON.stringify(detail)));
  const open = async (qs) => { const p = await b.newPage({ viewport: { width: 1440, height: 900 } }); await p.goto(base + '/index.html?lang=ru&platform=mac&log=1&' + qs); await p.waitForTimeout(1700); return p; };
  const calls = (p) => p.evaluate(() => JSON.parse(document.documentElement.getAttribute('data-calls') || '[]'));
  const names = async (p) => (await calls(p)).map((c) => c[0]);
  // 1: the queue
  let p = await open('env=claude&chat=cache&live=slow');
  await p.fill('.composer textarea', 'Лучше без кэша на запись'); await p.keyboard.press('Enter'); await p.waitForTimeout(400);
  const q1 = await p.evaluate(() => { const q = document.querySelector('.me.queued'); return { on: !!q, text: q && q.querySelector('.me-b').textContent.trim(), op: q && getComputedStyle(q.querySelector('.me-b')).opacity, note: q && q.querySelector('.me-m').textContent, field: document.querySelector('.composer textarea').value }; });
  const n1 = await names(p);
  ok('F12 1: while the agent works a message waits grey «в очереди» and nothing is sent yet', q1.on && /без кэша/.test(q1.text) && parseFloat(q1.op) < 0.8 && /в очереди/.test(q1.note) && !q1.field && !n1.includes('note') && !n1.includes('send_chat'), { q1, n1 });
  await p.fill('.composer textarea', 'И ещё лог'); await p.waitForTimeout(200);
  const nowShown = await p.evaluate(() => { const b = document.querySelector('.now4'); return b && !b.hidden && b.getBoundingClientRect().width > 0 && b.textContent.trim(); });
  ok('F12 1: with a typed message and a working agent the button «Сразу» shows', nowShown === 'Сразу', nowShown);
  await p.fill('.composer textarea', ''); await p.waitForTimeout(100);
  await p.click('.me.queued .q-act >> nth=0'); await p.waitForTimeout(200);
  const edited = await p.evaluate(() => ({ field: document.querySelector('.composer textarea').value, queued: document.querySelectorAll('.me.queued').length }));
  ok('F12 1: «Править» takes the message back to the field and out of the queue', /без кэша/.test(edited.field) && edited.queued === 0, edited);
  await p.keyboard.press('Enter'); await p.waitForTimeout(300);
  await p.click('.me.queued .q-act >> nth=1'); await p.waitForTimeout(200);
  ok('F12 1: «×» removes a queued message', (await p.evaluate(() => document.querySelectorAll('.me.queued').length)) === 0, null);
  await p.fill('.composer textarea', 'Остановись и сделай без кэша'); await p.waitForTimeout(150);
  await p.click('.now4'); await p.waitForTimeout(1800);
  const n2 = await calls(p);
  const sentAfter = n2.findIndex((c) => c[0] === 'send_chat' && /без кэша/.test(c[1].text)), cancelAt = n2.findIndex((c) => c[0] === 'cancel');
  const q2 = await p.evaluate(() => ({ queued: document.querySelectorAll('.me.queued').length, mine: [...document.querySelectorAll('.cfeed .me .me-b')].map((x) => x.textContent.trim()).filter((x) => /без кэша/.test(x)).length }));
  ok('F12 1: «Сразу» cancels the run, then the message goes and stops being grey', cancelAt >= 0 && sentAfter > cancelAt && q2.queued === 0 && q2.mine >= 1, { n2: n2.map((c) => c[0]), q2 });
  await p.close();
  // 2: the side question
  p = await open('env=claude&chat=how');
  const feedBefore = await p.evaluate(() => document.querySelector('.cfeed').children.length);
  await p.evaluate(() => document.activeElement && document.activeElement.blur && document.activeElement.blur());
  await p.evaluate(() => document.dispatchEvent(new KeyboardEvent('keydown', { code: 'Semicolon', key: ';', metaKey: true, bubbles: true, cancelable: true }))); await p.waitForTimeout(300);
  const s1 = await p.evaluate(() => { const e = document.querySelector('.sidechat'); const r = e && e.getBoundingClientRect(); return { shown: !!e && !e.hidden, w: r && Math.round(r.width), right: r && Math.round(document.querySelector('.convo').getBoundingClientRect().right - r.right), focus: document.activeElement && document.activeElement.className }; });
  ok('F12 2: ⌘; opens a narrow panel «Спросить» on the right with the focus in its field', s1.shown && s1.w <= 360 && s1.right === 0 && /sc-in/.test(s1.focus), s1);
  await p.fill('.sc-in', 'Где считаются налоги?'); await p.keyboard.press('Enter'); await p.waitForTimeout(500);
  const s2 = await p.evaluate(() => ({ q: document.querySelector('.sc-q') && document.querySelector('.sc-q').textContent, a: document.querySelector('.sc-a') && document.querySelector('.sc-a').textContent, feed: document.querySelector('.cfeed').children.length }));
  const n3 = await calls(p);
  ok('F12 2: the question gets an answer in the panel, the feed of the task does not change and nothing is sent to the agent', /налоги/.test(s2.q) && /боковой ответ/.test(s2.a) && s2.feed === feedBefore && n3.some((c) => c[0] === 'side_ask' && c[1].id === 'how') && !n3.some((c) => ['send_chat', 'note', 'answer'].includes(c[0])), { s2, feedBefore, n3: n3.map((c) => c[0]) });
  await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  ok('F12 2: Esc closes the panel', await p.evaluate(() => document.querySelector('.sidechat').hidden), null);
  await p.click('.side-btn'); await p.waitForTimeout(200);
  ok('F12 2: the button «Спросить» in the chat header opens it too', await p.evaluate(() => !document.querySelector('.sidechat').hidden), null);
  await p.close();
  // 4: «Выполнить» pastes the command into a terminal and waits for Enter
  p = await open('env=claude&chat=how');
  const rc = await p.evaluate(() => [...document.querySelectorAll('.codewrap .runcmd')].map((x) => x.textContent.trim()));
  ok('F12 4: the ```bash block of the answer has a «Выполнить» button', rc.length === 1 && rc[0] === 'Выполнить', rc);
  await p.click('.runcmd'); await p.waitForTimeout(1500);
  const term = await p.evaluate(() => ({ sessions: [...window.NSMockTerm.sessions.values()].map((s) => ({ line: s.line, buf: s.buf, running: s.running, cwd: s.cwd })) }));
  const sx = term.sessions[term.sessions.length - 1] || {};
  ok('F12 4: the command is in the terminal line of the project, not run (no Enter)', term.sessions.length >= 1 && /tree -L 2 src\/billing/.test(sx.line || '') && !/No such|total|\.py/.test(sx.buf || '') && /billing-api|Users/.test(sx.cwd || ''), term);
  await p.close();
  // 3: the next-step hint
  p = await open('env=claude&chat=cur');
  const h1 = await p.evaluate(() => ({ ph: document.querySelector('.composer textarea').placeholder, tab: !document.querySelector('.cmp-tab').hidden, v: document.querySelector('.composer textarea').value }));
  ok('F12 3: after a finished task the empty field shows a grey suggestion and «Tab»', /^Напиши тесты на «Поле currency/.test(h1.ph) && h1.tab && !h1.v, h1);
  await p.focus('.composer textarea'); await p.keyboard.press('Tab'); await p.waitForTimeout(150);
  const h2 = await p.evaluate(() => ({ v: document.querySelector('.composer textarea').value, tab: document.querySelector('.cmp-tab').hidden }));
  ok('F12 3: Tab puts the suggestion into the field (Enter would send it)', /^Напиши тесты на «Поле currency/.test(h2.v) && h2.tab, h2);
  await p.fill('.composer textarea', ''); await p.keyboard.press('KeyA'); await p.waitForTimeout(100); await p.keyboard.press('Backspace'); await p.waitForTimeout(100);
  const h3 = await p.evaluate(() => ({ ph: document.querySelector('.composer textarea').placeholder, tab: document.querySelector('.cmp-tab').hidden }));
  ok('F12 3: any key hides the suggestion', !/Напиши тесты/.test(h3.ph) && h3.tab, h3);
  await p.click('#tune'); await p.waitForTimeout(200);
  await p.click('#tunePanel .tg:has-text("Подсказывать следующий шаг")'); await p.waitForTimeout(100);
  const off = await p.evaluate(() => NS.storage.get('ns.hint_next'));
  await p.reload(); await p.waitForTimeout(1700);
  const h4 = await p.evaluate(() => document.querySelector('.composer textarea').placeholder);
  ok('F12 3: the setting «Подсказывать следующий шаг» turns it off (and a reload remembers)', off === '0' && !/Напиши тесты/.test(h4), { off, h4 });
  await p.close();
  // 5: a stopped run is «Остановлено», not a red «Не ответил»
  p = await open('env=gemini&chat=avif');
  const st = await p.evaluate(() => { const l = [...document.querySelectorAll('.cfeed .sysline')].pop(); return l && { text: l.textContent.trim(), bad: l.classList.contains('bad'), color: getComputedStyle(l).color }; });
  ok('F12 5: a stopped run reads «Остановлено» in a calm colour, with «Повторить»', st && /^Остановлено/.test(st.text) && !st.bad && !/Не ответил/.test(st.text) && /Повторить/.test(st.text), st);
  await p.close();
}

// F14 - automation: the schedule, the suggested tasks, the CI watchman (dev-mock plays the core: mock-auto.js)
async function f14(b, base, out) {
  const ok = (name, cond, detail) => out.push((cond ? 'PASS ' : 'FAIL ') + name + (cond ? '' : ' ' + JSON.stringify(detail)));
  const open = async (qs) => {
    const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
    await p.goto(base + '/index.html?lang=ru&platform=mac&env=claude&chat=new' + (qs || '')); await p.waitForTimeout(1300);
    return p;
  };
  let p = await open();
  const nav = await p.evaluate(() => { const b = document.querySelector('.nb-auto'); return b && { text: b.textContent.trim(), next: b.previousElementSibling && b.previousElementSibling.textContent.trim(), after: b.nextElementSibling && b.nextElementSibling.textContent.trim() }; });
  ok('F14: the entrance «Автоматика» sits between «Фабрика» and «Настройки» and carries the count of suggestions', nav && /^Автоматика\s*5$/.test(nav.text) && nav.next === 'Фабрика' && nav.after === 'Настройки', nav);
  await p.click('.nb-auto'); await p.waitForTimeout(500);
  const rows = () => p.evaluate(() => [...document.querySelectorAll('#a4-sched .a4-row')].map((r) => ({ t: r.querySelector('b').textContent, on: r.classList.contains('on'), sw: r.querySelector('[role=switch]').getAttribute('aria-checked'), txt: r.textContent })));
  const r0 = await rows();
  ok('F14 1: the schedule lists the tasks with preset, time and the next firing; the held one says why (limit under 10%)', r0.length === 3 && r0[0].txt.includes('по будням в 03:00') && /дальше:/.test(r0[0].txt) && /меньше 10% лимита/.test(r0[1].txt) && !r0[2].on, r0);
  await p.click('#a4-sched .a4-row:nth-child(1) [role=switch]'); await p.waitForTimeout(300);
  const r1 = await rows();
  ok('F14 1: the switch turns a schedule off (the core agrees after a reload) and its «next» disappears', r1[0].sw === 'false' && !r1[0].on && !/дальше:/.test(r1[0].txt), r1[0]);
  // a new one from a template
  await p.click('[data-tpl=todo]'); await p.waitForTimeout(100);
  const filled = await p.evaluate(() => ({ title: document.querySelector('#a4-form input[type=text]').value, prompt: document.querySelector('#a4-form textarea').value.length > 20 }));
  ok('F14 1: a template fills the name and the task text', filled.title === 'Добить TODO' && filled.prompt, filled);
  await p.fill('.a4-time', '25:99'); await p.click('#a4-form .btn.primary'); await p.waitForTimeout(300);
  ok('F14 1: a bad time is refused, nothing is added', (await rows()).length === 3, null);
  await p.fill('.a4-time', '02:15');
  await p.selectOption('#a4-form select:has(option[value=weekly])', 'weekly'); await p.waitForTimeout(100);
  const dayShown = await p.evaluate(() => !document.querySelector('.a4-day').hidden);
  await p.click('#a4-form .btn.primary'); await p.waitForTimeout(400);
  const r2 = await rows();
  ok('F14 1: «Раз в неделю» shows the day of the week; saving adds a row that is still off (the person switches it on)', dayShown && r2.length === 4 && r2[3].t === 'Добить TODO' && /раз в неделю, среда в 02:15/.test(r2[3].txt) && !r2[3].on, { dayShown, last: r2[3] });
  await p.click('#a4-sched .a4-row:nth-child(4) .btn.sm'); await p.waitForTimeout(200);
  const editing = await p.evaluate(() => ({ h: document.querySelector('#a4-form h3').textContent, v: document.querySelector('#a4-form input[type=text]').value, save: document.querySelector('#a4-form .btn.primary').textContent }));
  ok('F14 1: «Править» loads the row into the form', /Править/.test(editing.h) && editing.v === 'Добить TODO' && editing.save === 'Сохранить', editing);
  await p.click('#a4-sched .a4-row:nth-child(4) .linkb'); await p.waitForTimeout(200);
  await p.click('dialog[open] .btn.primary'); await p.waitForTimeout(300);
  ok('F14 1: «Удалить» asks first, then removes the row', (await rows()).length === 3, null);
  // suggestions
  const cards = () => p.evaluate(() => ({ n: document.querySelectorAll('#a4-sug .a4-card').length, nav: String(NS.auto.count()), first: document.querySelector('#a4-sug .a4-card').textContent }));
  const c0 = await cards();
  ok('F14 2: «Предлагаю» lists file:line, the text and the project; the button carries the same count', c0.n === 5 && c0.nav === '5' && /src\/invoices\/routes\.py:88/.test(c0.first) && /billing-api/.test(c0.first), c0);
  await p.click('#a4-sug .a4-card:nth-child(1) .linkb'); await p.waitForTimeout(300);
  const c1 = await cards();
  ok('F14 2: «Не предлагать такое» removes the card and the count drops', c1.n === 4 && c1.nav === '4', c1);
  await p.click('#a4-sug .a4-card:nth-child(1) .btn'); await p.waitForTimeout(500);
  const c2 = await cards();
  const tasks = await p.evaluate(() => NS.api.call('list_tasks').then((l) => l.filter((t) => /cache\.py|FIXME/.test(t.prompt)).length));
  ok('F14 2: «Сделать» queues a task through submit_task and the card is gone', c2.n === 3 && tasks === 1, { c2, tasks });
  await p.click('#a4-sug .a4-tg [role=switch]'); await p.waitForTimeout(200);
  ok('F14 2: «Искать по ночам» is a switch, off until the person turns it on', await p.evaluate(() => NS.api.call('auto_get').then((d) => d.suggest.nightly === true)), null);
  // the watchman
  const ci = await p.evaluate(() => ({ items: [...document.querySelectorAll('#a4-ci .a4-row')].map((r) => r.textContent), on: document.querySelector('#a4-ci [role=switch]').getAttribute('aria-checked'), merge: document.querySelectorAll('#a4-ci [role=switch]')[1].getAttribute('aria-checked') }));
  ok('F14 3: the watchman lists the pull requests with their state and attempts; «слить, когда зелёное» is off by default', ci.items.length === 4 && /зелёный/.test(ci.items[0]) && /красный.*попытки 1 из 2.*Починить CI/.test(ci.items[1]) && /конфликт.*попытки 2 из 2.*дальше сам/.test(ci.items[3]) && ci.on === 'true' && ci.merge === 'false', ci);
  await p.click('#a4-ci .a4-line:nth-child(3) [role=switch]'); await p.waitForTimeout(300);
  const asked = await p.evaluate(() => ({ dlg: !!document.querySelector('dialog[open]'), title: (document.querySelector('dialog[open] h2') || {}).textContent }));
  await p.click('dialog[open] .btn:not(.primary)'); await p.waitForTimeout(300);
  const declined = await p.evaluate(() => NS.api.call('auto_get').then((d) => d.ci.config.merge_when_green));
  ok('F14 3: turning «слить, когда зелёное» on asks for a confirmation the first time; «Отмена» leaves it off', asked.dlg && /слияние/.test(asked.title) && declined === false, { asked, declined });
  await p.click('#a4-ci .a4-line:nth-child(3) [role=switch]'); await p.waitForTimeout(300);
  await p.click('dialog[open] .btn.primary'); await p.waitForTimeout(400);
  ok('F14 3: after «Включить» the setting and its confirmation are stored', await p.evaluate(() => NS.api.call('auto_get').then((d) => d.ci.config.merge_when_green === true && d.ci.config.merge_confirmed === true)), null);
  // words and layout
  const words = await p.evaluate(() => { const d = NSI18n.dicts; const keys = Object.keys(d.ru).filter((k) => /^auto\.|^err\.(empty_title|empty_prompt|bad_time|suggestion_not_found|schedule_not_found)$/.test(k)); return { n: keys.length, missing: keys.filter((k) => !(k in d.en)), built: ['green', 'pending', 'red', 'conflict', 'error'].map((s) => NSI18n.has('auto.s.' + s)).concat(['waiting', 'gave_up', 'ask_merge', 'fix_started', 'merge_set', 'merge_failed', 'fix_failed'].map((s) => NSI18n.has('auto.n.' + s))) }; });
  ok('F14: every automation word exists in ru and en, and so do the ones built from a state', words.n > 80 && !words.missing.length && words.built.every(Boolean), words);
  const lay = await p.evaluate(() => ({ overflow: document.documentElement.scrollWidth > innerWidth, cols: getComputedStyle(document.querySelector('.a4-grid')).gridTemplateColumns.split(' ').length }));
  ok('F14: the screen has no horizontal scroll at 1440 and two columns', !lay.overflow && lay.cols === 2, lay);
  await p.close();
  // gh missing: a hint, no items, the feature stays off
  p = await open('&ci=nogh');
  await p.evaluate(() => NS.nav('automation', {})); await p.waitForTimeout(500);
  const nogh = await p.evaluate(() => ({ hint: (document.querySelector('#a4-ci .a4-warn') || {}).textContent, items: document.querySelectorAll('#a4-ci .a4-row').length }));
  ok('F14 3: without gh the watchman says so and lists nothing', /Не найден gh/.test(nogh.hint || '') && nogh.items === 0, nogh);
  await p.close();
  // nothing set up: empty states, nothing running
  p = await open('&auto=empty');
  await p.evaluate(() => NS.nav('automation', {})); await p.waitForTimeout(500);
  const none = await p.evaluate(async () => {
    const o = { sched: document.querySelector('#a4-sched .a4-none').textContent, sug: document.querySelector('#a4-sug .a4-none').textContent, ci: document.querySelector('#a4-ci .a4-none').textContent, on: document.querySelector('#a4-ci [role=switch]').getAttribute('aria-checked') };
    NS.shell.back(); await new Promise((r) => setTimeout(r, 400));
    o.badge = document.querySelector('.nb-auto .bn').hidden;
    return o;
  });
  ok('F14: a clean start - empty lists, no count on the button, the watchman off', /Расписаний ещё нет/.test(none.sched) && /Пока ничего/.test(none.sug) && /выключен/.test(none.ci) && none.badge === true && none.on === 'false', none);
  await p.close();
}

// F15 - the proof screenshots in the done card, the preview pane, "Указать элемент" into the task chat field
async function f15(b, base, out) {
  const ok = (name, cond, detail) => out.push((cond ? 'PASS ' : 'FAIL ') + name + (cond ? '' : ' ' + JSON.stringify(detail)));
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  await p.goto(base + '/index.html?lang=ru&platform=mac&env=gemini&chat=es');
  await p.waitForTimeout(1600);
  const strip = await p.evaluate(() => { const s = document.querySelector('.tcard .pv4-strip'); return s && { cap: s.querySelector('.pv4-cap').textContent, thumbs: [...s.querySelectorAll('.pv4-thumb')].map((x) => ({ label: x.getAttribute('aria-label'), img: (x.querySelector('img').getAttribute('src') || '').slice(0, 10) })) }; });
  ok('F15 2: the done card carries the strip of proof screenshots (page, after the click, phone), each a loaded picture', strip && strip.thumbs.length === 3 && strip.thumbs.every((x) => x.img === 'data:image') && /Скриншот 2 из 3: После клика/.test(strip.thumbs[1].label), strip);
  await p.click('.tcard .tc-more'); await p.waitForTimeout(500);
  const chk = await p.evaluate(() => [...document.querySelectorAll('.clist li')].map((li) => li.textContent).find((x) => /Как пользователь/.test(x)) || null);
  ok('F15 2: «Проверки» lists the step «Как пользователь» with the count of shots', /Снято скриншотов: 3/.test(chk || ''), chk);
  await p.click('.pv4-thumb >> nth=1'); await p.waitForTimeout(300);
  const big = await p.evaluate(() => { const d = document.querySelector('dialog[open]'); return d && { cls: d.className, cap: (d.querySelector('.pv4-bigcap') || {}).textContent, img: (d.querySelector('img').getAttribute('src') || '').slice(0, 10) }; });
  await p.keyboard.press('ArrowRight'); await p.waitForTimeout(150);
  const next = await p.evaluate(() => (document.querySelector('dialog[open] .pv4-bigcap') || {}).textContent);
  await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  const after = await p.evaluate(() => ({ dlg: !!document.querySelector('dialog[open]'), cardOpen: !document.querySelector('.tcard .tc-det').hidden }));
  ok('F15 2: a click on a thumbnail shows it big; arrows step through, Esc closes and the card stays open', big && /pv4-big/.test(big.cls) && /Скриншот 2 из 3/.test(big.cap) && big.img === 'data:image' && /Скриншот 3 из 3 · Телефон/.test(next || '') && !after.dlg && after.cardOpen, { big, next, after });
  // the preview pane next to the chat
  await p.click('.pv4-det .btn'); await p.waitForTimeout(600);
  const pane = await p.evaluate(() => { const v = document.querySelector('.pv4'); return v && { url: v.querySelector('.pv4-url').value, pick: v.querySelector('.pv4-pick').disabled, st: v.querySelector('.pv4-st').textContent, widths: [...v.querySelectorAll('.pv4-w button')].map((x) => x.textContent), proof: v.querySelector('.pv4-proof [role=switch]').getAttribute('aria-checked'), chat: !!document.querySelector('.convo .tcard[data-task="t-es"]') }; });
  ok('F15 1: «Превью» opens a pane beside the chat with the project address, widths 390/768/desktop and «Указать элемент» off until the window is open', pane && pane.url === 'http://localhost:4010/' && pane.pick === true && /не открыто/.test(pane.st) && pane.widths.join('|') === 'Телефон 390|Планшет 768|Десктоп' && pane.proof === 'true' && pane.chat, pane);
  await p.fill('.pv4-url', 'http://example.com/'); await p.click('.pv4 .btn.primary'); await p.waitForTimeout(200);
  const refused = await p.evaluate(() => ({ st: document.querySelector('.pv4-st').textContent, bad: document.querySelector('.pv4-url').getAttribute('aria-invalid'), pick: document.querySelector('.pv4-pick').disabled }));
  ok('F15 1: an address that is not localhost is refused before anything opens', /Только адреса этого компьютера/.test(refused.st) && refused.bad === 'true' && refused.pick, refused);
  await p.fill('.pv4-url', 'http://localhost:4010'); await p.click('.pv4 .btn.primary'); await p.waitForTimeout(250);
  const opened = await p.evaluate(() => ({ st: document.querySelector('.pv4-st').textContent, pick: document.querySelector('.pv4-pick').disabled }));
  ok('F15 1: a local address opens the preview window and the tools come alive', /Открыто: http:\/\/localhost:4010\//.test(opened.st) && opened.pick === false, opened);
  await p.click('.pv4-w button:nth-child(1)'); await p.waitForTimeout(150);
  ok('F15 1: «Телефон 390» is the chosen width', await p.evaluate(() => document.querySelector('.pv4-w button.on').textContent === 'Телефон 390'), null);
  await p.click('.pv4-pick'); await p.waitForTimeout(500);
  const picked = await p.evaluate(() => ({ field: document.querySelector('.tcard[data-task="t-es"]').closest('.convo').querySelector('.composer textarea').value, st: document.querySelector('.pv4-st').textContent }));
  ok('F15 1: «Указать элемент» puts the element (selector, text, size) into the task chat field', /`main > section\.hero > a\.btn-primary`/.test(picked.field) && /«Empezar gratis»/.test(picked.field) && /240×52/.test(picked.field) && /Вставлено в поле чата/.test(picked.st), picked);
  const words = await p.evaluate(() => { const d = NSI18n.dicts; const keys = Object.keys(d.ru).filter((k) => /^pv\./.test(k)); return { n: keys.length, missing: keys.filter((k) => !(k in d.en)) }; });
  ok('F15: every preview word exists in ru and en', words.n > 30 && !words.missing.length, words);
  const lay = await p.evaluate(() => document.documentElement.scrollWidth > innerWidth);
  ok('F15: no horizontal scroll at 1440 with the preview pane', !lay, lay);
  await p.close();
}
