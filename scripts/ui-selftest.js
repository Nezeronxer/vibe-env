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
  out.push((r.buttons <= 18 ? 'PASS ' : 'FAIL ') + 'main view: at most 18 visible buttons outside the feed (14 + the three entrances of F8 + "Новый чат" of the chat list that F9 shows by default) (' + r.buttons + ')' + (r.buttons <= 18 ? '' : ' ' + JSON.stringify(r.names)));
  out.push((r.pane > 0 && r.card > 0 && r.field > 0 && r.bar > 0 ? 'PASS ' : 'FAIL ') + 'computed border-radius > 0: pane ' + r.pane + ', card ' + r.card + ', field ' + r.field + ', bar ' + r.bar);
  await f5(b, 'http://127.0.0.1:' + srv.address().port, out);
  await f7(b, 'http://127.0.0.1:' + srv.address().port, out);
  await b.close(); b = await launch();       // F8 runs in a fresh browser: after F5 and F7 in one browser it died on the next page (headless Chrome on macOS)
  await f8(b, 'http://127.0.0.1:' + srv.address().port, out);
  await b.close(); b = await launch();       // F9 in a fresh browser too
  await f9(b, 'http://127.0.0.1:' + srv.address().port, out);
  await b.close(); b = await launch();       // F10 in a fresh browser too
  await f10(b, 'http://127.0.0.1:' + srv.address().port, out);
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
  // 3: the "+" menu
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
