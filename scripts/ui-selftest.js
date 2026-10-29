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
check('v7: the dark canvas is graphite (B >= R, near black) and the light canvas is near white', ((c) => c.b >= c.r && c.r < 24)(rgbOf(darkCss['--canvas'])) && ((c) => c.r > 220 && c.b >= c.r)(rgbOf(lightCss['--canvas'])), [darkCss['--canvas'], lightCss['--canvas']]);
const px = (v) => parseFloat(String(v || '').replace('var(--radius-pane)', darkCss['--radius-pane']).replace('var(--radius-sm)', darkCss['--radius-sm']).replace('var(--r)', darkCss['--radius-sm']));
const all = sheets.map(css).join('\n');
const radiusOf = (sel) => { const re = new RegExp('(^|[},\\s])' + sel.replace(/[.[\]=]/g, '\\$&') + '\\s*[,{][^}]*?border-radius:\\s*([^;}]+)', 'gm'); let m, last = null; while ((m = re.exec(all))) last = m[2].split(/\s/)[0]; return last; };
const corners = { '.pane': radiusOf('.pane'), '.tcard': radiusOf('.tcard'), '.composer': radiusOf('.composer'), 'input[type=text]': radiusOf('input[type=text]'), '.card4': radiusOf('.card4'), '.bar': radiusOf('.bar') };
check('corners: pane, task card, input field, cards and the bar are rounded (border-radius > 0 from the tokens)', Object.values(corners).every((v) => px(v) > 0), corners);
// v6 (task 29): the menu "/" as pure logic - the "/word" under the caret, the sections for a query, the text after a choice
{
  const sctx = vm.createContext({});
  vm.runInContext(fs.readFileSync(path.join(ui, 'js/v4/slash.js'), 'utf8'), sctx, { filename: 'slash.js' });
  const S = sctx.NSSlash;
  const tk = (x, c) => JSON.stringify(S && S.token(x, c));
  check('G82: «/» starts the menu at the start of the field or after a space, not inside a word or after a space after the name',
    !!S && tk('/re', 3) === JSON.stringify({ start: 0, end: 3, query: 're' }) && tk('fix /gi', 7) === JSON.stringify({ start: 4, end: 7, query: 'gi' }) && tk('a/b', 3) === 'null' && tk('/a b', 4) === 'null' && tk('/', 1) === JSON.stringify({ start: 0, end: 1, query: '' }),
    S && [tk('/re', 3), tk('fix /gi', 7), tk('a/b', 3), tk('/a b', 4)]);
  const inv = { skills: [{ name: 'graphify', description: 'Граф кода' }, { name: 'review', description: 'Ревью диффа' }, { name: 'preview-ui', description: '' }], commands: [{ name: 'deploy', description: 'Выкатить review-стенд' }], mcp: [{ name: 'github', description: '' }], mods: [] };
  const sec = S ? S.filter(inv, 'rev') : [];
  const names = (k) => (sec.find((x) => x.key === k) || { items: [] }).items.map((x) => x.name);
  check('G82: the search takes the name (the start first) and then the description; sections Скиллы, Команды, MCP, Моды in that order; only skills and commands insert',
    !!S && sec.map((x) => x.key).join() === 'skills,commands,mcp,mods' && names('skills').join() === 'review,preview-ui' && names('commands').join() === 'deploy' && !names('mcp').length && sec[0].insertable && !sec[2].insertable,
    sec.map((x) => [x.key, x.items.map((i) => i.name)]));
  const ins = S ? S.insert('look /rev please', { start: 5, end: 9, query: 'rev' }, 'review') : null;
  check('G82: a choice puts «/name » in place of the typed word and the caret after it', !!ins && ins.text === 'look /review please' && ins.caret === 13, ins);
}
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
  await p.goto('http://127.0.0.1:' + srv.address().port + '/index.html?lang=ru&platform=mac&first=chat');
  await p.waitForTimeout(1200);
  const r = await p.evaluate(() => {
    const clear = (el) => { for (let e = el; e; e = e.parentElement) if (+getComputedStyle(e).opacity === 0) return true; return false; };   // a group faded out whole (the pane tools until hover)
    const vis = (el) => { const s = getComputedStyle(el), b = el.getBoundingClientRect(); return b.width > 0 && b.height > 0 && s.visibility !== 'hidden' && !clear(el); };
    // Clawd is a character in the tab corner, not a control; v8: the agents column is navigation, like the tab strip
    const buttons = [...document.querySelectorAll('button, [role=button]')].filter((x) => !x.closest('.cfeed, .clawd, .er8-col') && vis(x));
    const rad = (sel) => { const el = document.querySelector(sel); return el ? parseFloat(getComputedStyle(el).borderTopLeftRadius) : -1; };
    return { buttons: buttons.length, names: buttons.map((x) => x.getAttribute('aria-label') || x.textContent.trim()).slice(0, 20), pane: rad('.pane'), card: rad('.tcard'), field: rad('.composer'), bar: rad('.bar') };
  });
  const out = [];
  out.push((r.buttons <= 24 ? 'PASS ' : 'FAIL ') + 'main view: at most 24 visible buttons outside the feed and the agents column (14 + the three entrances of F8 + "Новый чат" of the chat list that F9 shows by default + "Спросить" of F12 + "Автоматика" of F14 + "Приложить картинку" of v6 G65.2, shown only for an env that reads images + v8 the project and "+" of the session tabs + v8 the GitHub panel toggle) (' + r.buttons + ')' + (r.buttons <= 24 ? '' : ' ' + JSON.stringify(r.names)));
  out.push((r.pane > 0 && r.card > 0 && r.field > 0 && r.bar > 0 ? 'PASS ' : 'FAIL ') + 'computed border-radius > 0: pane ' + r.pane + ', card ' + r.card + ', field ' + r.field + ', bar ' + r.bar);
  // v10: the settings menu of ten items holds every section exactly once, and every item has its name in ru and en
  const nv = await p.evaluate(() => {
    const N = NS.settingsNav, items = N.NAV.concat(N.NAV_ADV), all = items.flatMap((n) => n.parts), D = NSI18n.dicts;
    const ids = NSLogic.SETTINGS_SECTIONS.map((x) => x.id).concat(NS.registry.points('settings').map((d) => d.id));
    return { n: N.NAV.length, dup: all.filter((x, i) => all.indexOf(x) !== i), miss: ids.filter((x) => !all.includes(x)),
      noStr: items.flatMap((n) => ['ru', 'en'].filter((l) => !D[l]['nav10.' + n.id]).map((l) => l + ':' + n.id)) };
  });
  out.push((nv.n === 10 && !nv.dup.length && !nv.miss.length && !nv.noStr.length ? 'PASS ' : 'FAIL ') + 'v10 settings menu: 10 items, every section in exactly one, names in ru and en' + (nv.dup.length || nv.miss.length || nv.noStr.length || nv.n !== 10 ? ' ' + JSON.stringify(nv) : ''));
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
  await b.close(); b = await launch();       // v6 task 27 in a fresh browser too
  await f27(b, 'http://127.0.0.1:' + srv.address().port, out);
  await b.close(); b = await launch();       // v6 issues and sessions in a fresh browser too
  await v6is(b, 'http://127.0.0.1:' + srv.address().port, out);
  await b.close(); b = await launch();       // v6 task 28 (lane on its limit, spending, about, the release bar) in a fresh browser too
  await ops6(b, 'http://127.0.0.1:' + srv.address().port, out);
  await b.close(); b = await launch();       // v6 task 29 (the menu "/", images, the chat in the terminal) in a fresh browser too
  await shell6(b, 'http://127.0.0.1:' + srv.address().port, out);
  await b.close(); b = await launch();       // look8 (themes, own theme, transparency, gradients) in a fresh browser too
  await look8(b, 'http://127.0.0.1:' + srv.address().port, out);
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
  await p.goto(base + '/index.html?lang=ru&platform=mac&first=chat');
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
  await p.evaluate(() => { const e = NS.store.s.envs.find((x) => x.id === 'opencode'); if (e) NS.shell.nav('env', { env: 'opencode' }); }); await p.waitForTimeout(800);   // v8: pickEnv on the workspace picks the agent column
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
  await p.goto(base + '/index.html?lang=ru&platform=mac&first=chat');
  await p.waitForTimeout(1500);
  // 1 (v10, Agentation 29.10): Рой / Качество are the pills in the middle of the bar, not a second pair of switches in the field
  const sw = await p.evaluate(() => ({ field: document.querySelectorAll('.composer .comp-row .tg').length, bar: [...document.querySelectorAll('.bar-c .md8-p')].map((x) => x.textContent.trim()) }));
  ok('v10 F8 1: no Рой / Качество switches in the field; the bar keeps them in its middle', sw.field === 0 && sw.bar.some((x) => /Качество/.test(x)) && sw.bar.some((x) => /Рой/.test(x)), sw);
  const tune = await p.evaluate(() => { const t = document.querySelector('.composer .tune4'); const r = t.getBoundingClientRect(); return { w: r.width, text: t.textContent.trim(), icon: !!t.querySelector('svg'), title: t.title }; });
  ok('F8 1: "⋯" is a gear with the word "Настроить" and a title', tune.text === 'Настроить' && tune.icon && tune.w > 50 && tune.title.length > 10, tune);
  // 2: the entrances (v7: the foot of the agents column keeps ⌘K and the settings; the factory moved to "Новый агент" and ⌘K)
  const here = () => p.evaluate(() => NS.shell.current && NS.shell.current.def.id);
  // v10: one way to the settings - the gear at the foot of the agents column; no second "Настройки" by the search
  const vis = await p.evaluate(() => ['.er8-set', '.navb.nb-pal', '.navb.nb-set'].map((q) => { const e = document.querySelector(q); const r = e && e.getBoundingClientRect(); return e ? { w: r.width, text: e.textContent.trim(), label: e.getAttribute('aria-label') } : null; }));
  ok('v10 F8 2: the settings gear is under the agents, "⌘K Поиск и команды" by the search, no second settings button', vis[0] && vis[0].w > 20 && /Настройки/.test(vis[0].label) && vis[1] && /Поиск и команды/.test(vis[1].text) && !vis[2], vis);
  await p.click('.rail7-new'); await p.waitForTimeout(300);
  await p.click('.l7-more-b:has-text("Фабрика")'); await p.waitForTimeout(500);
  const fact = await here();
  await p.evaluate(() => NS.shell.back()); await p.waitForTimeout(400);
  await p.click('.er8-set'); await p.waitForTimeout(500);
  const set = await here();
  await p.evaluate(() => NS.shell.back()); await p.waitForTimeout(400);
  await p.evaluate(() => document.dispatchEvent(new KeyboardEvent('keydown', { code: 'Comma', key: ',', metaKey: true, bubbles: true, cancelable: true }))); await p.waitForTimeout(500);   // a synthetic key: a real ⌘, would reach the browser itself
  const comma = await here();
  await p.evaluate(() => NS.shell.back()); await p.waitForTimeout(400);
  ok('F8 2: "Новый агент → Фабрика" opens the factory, the gear and ⌘, open the settings', fact === 'factory' && set === 'settings' && comma === 'settings', { fact, set, comma });
  await p.click('.navb.nb-pal'); await p.waitForTimeout(300);
  ok('F8 2: "⌘K Поиск и команды" opens the palette', await p.evaluate(() => !!document.querySelector('.pal-in') && NS.palette.isOpen), null);
  await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  // 3: "Новый агент" (v7, in place of the "+" menu; F11: the terminal opens in the project that was chosen, not in a guessed one)
  await p.evaluate(() => NS.projects.choose('p1'));
  await p.click('.rail7-new'); await p.waitForTimeout(300);
  const l7 = await p.evaluate(() => ({ envs: [...document.querySelectorAll('.launch7 .l7-env')].map((x) => x.textContent.trim()), more: [...document.querySelectorAll('.launch7 .l7-more-b')].map((x) => x.textContent.trim()), go: !!document.querySelector('.launch7 .l7-go') }));
  ok('v7 F8 3: "Новый агент" shows the installed envs, "Запустить" and the chat, a plain terminal, the swarm and the factory', l7.envs.length >= 2 && /Claude Code/.test(l7.envs[0]) && l7.go && l7.more.length === 4 && /Чат/.test(l7.more[0]) && /Пустой терминал/.test(l7.more[1]), l7);
  await p.click('.l7-more-b:has-text("Пустой терминал")'); await p.waitForTimeout(900);
  const term = await p.evaluate(() => ({ kind: NS.tabs.active.kind, glyph: (document.querySelector('.tab.on .tab-ic .t-glyph') || {}).textContent, label: document.querySelector('.tab.on .tab-l').textContent, menu: !!document.querySelector('.launch7') }));
  ok('F8 3: "Пустой терминал" opens a terminal tab with the >_ icon, named "Терминал · …"; the launcher closes', term.kind === 'term' && term.glyph === '>_' && /^Терминал · /.test(term.label) && !term.menu, term);
  await p.click('.rail7-new'); await p.waitForTimeout(300);
  await p.click('.l7-more-b:has-text("Чат")'); await p.waitForTimeout(900);
  const chat = await p.evaluate(() => ({ kind: NS.tabs.active.kind, glyph: !!document.querySelector('.tab.on .tab-ic .t-glyph'), img: !!document.querySelector('.tab.on .tab-ic svg, .tab.on .tab-ic img') }));
  ok('F8 3: "Чат с агентом" opens a chat tab with the icon of the environment, not >_', chat.kind === 'agent' && !chat.glyph && chat.img, chat);
  ok('F8 4: the empty chat carries the hint about the terminal', await p.evaluate(() => /Тот же разговор в CLI - «Терминал» в шапке/.test((document.querySelector('.cmp-note') || {}).textContent || '')), null);
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
  await p.goto(base + '/index.html?lang=ru&platform=mac&first=chat');
  await p.waitForTimeout(1600);
  // 5 (v10): the chat list repeated the sessions column and is gone - no bar button, ⌘B does nothing, a saved "on" opens off
  const side = () => p.evaluate(() => { const e = document.querySelector('.side'); return { on: document.body.classList.contains('side-on'), w: e ? e.getBoundingClientRect().width : 0, btn: !!document.querySelector('.bm-side') }; });
  const s0 = await side();
  ok('v10 F9 5: no chat list and no button for it; the agents column is there', !s0.on && s0.w === 0 && !s0.btn && await p.evaluate(() => document.querySelector('.rail7').getBoundingClientRect().width > 200), s0);
  await p.evaluate(() => document.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyB', key: 'b', metaKey: true, bubbles: true, cancelable: true }))); await p.waitForTimeout(250);
  await p.evaluate(() => NS.storage.set('ns.side7', '1')); await p.reload(); await p.waitForTimeout(1500);
  ok('v10 F9 5: ⌘B does not bring it back, nor a saved "on" after a reload', !(await side()).on && (await side()).w === 0, null);
  // v10: a session is a chat; «Терминал» shows the same talk in the CLI in the same pane, «Чат» brings the feed back
  await p.evaluate(() => NS.tabs.newTab()); await p.waitForTimeout(500);
  const k0 = await p.evaluate(() => NS.tabs.active && NS.tabs.active.kind);
  ok('v10: "+" opens a chat, not a bare terminal', k0 === 'agent', k0);
  // v10: a layout saved the v7 way (Claude's CLI as a bare terminal) comes back as a chat; a plain shell stays a terminal
  await p.evaluate(() => { const P = window.NSPanes; let st = { v: 2, tabs: [], active: null };
    st = P.wsOpen(st, 'tab-a', { id: 'p-cli', kind: 'term', env: 'claude', argv: ['claude'], title: 'Claude Code' });
    st = P.wsOpen(st, 'tab-b', { id: 'p-sh', kind: 'term', argv: ['zsh'], title: 'zsh' });
    st = P.wsOpen(st, 'tab-c', { id: 'p-job', kind: 'term', env: 'claude', argv: ['claude'], cwd: '/tmp/wt', branch: 'ns/term-1-x', prompt: 'do it', started: true, title: 'do it' });
    NS.storage.set('ns.tabs', P.wsSerialize(st)); });
  await p.reload(); await p.waitForTimeout(1600);
  const kinds = await p.evaluate(() => Object.fromEntries(window.NSPanes.allPanes(NS.tabs.state).filter((x) => ['p-cli', 'p-sh', 'p-job'].includes(x.pane.id)).map((x) => [x.pane.id, x.pane.kind])));
  ok('v10: a saved CLI terminal of Claude reopens as a chat, a plain shell stays a terminal', kinds['p-cli'] === 'agent' && kinds['p-sh'] === 'term', kinds);
  ok('v11: an agent started with a task in its own copy comes back as its terminal, not an empty chat', kinds['p-job'] === 'term', kinds);
  await p.evaluate(() => NS.storage.del('ns.tabs')); await p.reload(); await p.waitForTimeout(1600);   // the next checks expect the page's own layout
  await p.evaluate(() => { const x = window.NSPanes.allPanes(NS.tabs.state).find((y) => y.pane.kind === 'agent' && y.pane.chat !== 'new'); if (x) NS.tabs.focusPane(x.pane.id); }); await p.waitForTimeout(500);
  const v0 = await p.evaluate(() => { const pid = NS.tabs.active.id, el = document.querySelector('.pane[data-pane="' + pid + '"]'); return { pid, sw: !!(el && el.querySelector('.seg.view10')), feed: !!(el && el.querySelector('.cfeed')) }; });
  await p.evaluate((pid) => NS.tabs.chatView(pid, 'term'), v0.pid); await p.waitForTimeout(900);
  const v1 = await p.evaluate((pid) => { const el = document.querySelector('.pane[data-pane="' + pid + '"]'); return { term: !!(el && el.querySelector('.t-pane')), feed: !!(el && el.querySelector('.cfeed')), field: !!(NS.tabs.active.term) }; }, v0.pid);
  await p.evaluate((pid) => NS.tabs.chatView(pid, 'chat'), v0.pid); await p.waitForTimeout(700);
  const v2 = await p.evaluate((pid) => { const el = document.querySelector('.pane[data-pane="' + pid + '"]'); return { term: !!(el && el.querySelector('.t-pane')), feed: !!(el && el.querySelector('.cfeed')) }; }, v0.pid);
  ok('v10: «Чат | Терминал» in the head; «Терминал» puts the CLI in the same pane, «Чат» brings the feed back', v0.sw && v0.feed && v1.term && !v1.feed && v1.field && !v2.term && v2.feed, { v0, v1, v2 });
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
  await p.evaluate(() => NS.tabs.newTab('chat')); await p.waitForTimeout(900);
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
  await r2.goto(base + '/index.html?lang=ru&platform=mac&first=chat');
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
    await p.goto(base + '/index.html?lang=ru&platform=mac&first=chat');
    await p.waitForTimeout(1600);
    const R = await p.evaluate(() => {
      const cs = (el) => getComputedStyle(el), q = (s) => document.querySelector(s);
      const bgOf = (el) => { for (let e = el; e; e = e.parentElement) { const c = cs(e).backgroundColor; if (c && !/rgba\(.*, 0\)|transparent/.test(c)) return c; } return cs(document.body).backgroundColor; };
      const feed = q('.cfeed'), sec = q('.msg-h small') || q('.sysline'), pane = q('.pane');
      const lampOn = q('.tasks a .lamp.on-air'), lampCall = q('.tasks a .lamp.calling');
      const lim = q('.bm-lim.lim8'), fill = lim && lim.querySelector('.lim8-fill');
      return { appBg: cs(q('.app')).backgroundImage, canvas: cs(document.documentElement).getPropertyValue('--canvas').trim(), theme: document.documentElement.getAttribute('data-theme'),
        text: cs(feed).color, textBg: bgOf(feed), sec: sec && cs(sec).color, secBg: sec && bgOf(sec), paneBg: pane && cs(pane).backgroundColor,
        lampOn: !!lampOn, lampCall: !!lampCall, callAnim: lampCall && cs(lampCall).animationName, onAirBg: lampOn && cs(lampOn).backgroundImage,
        lim: !!lim, limP: fill && fill.style.getPropertyValue('--p'), limText: lim && lim.textContent, limW: fill && fill.parentElement.getBoundingClientRect().width, who: (q('.lim8-who') || {}).textContent || '',
        gauge: !!q('.bm-lim .gauge'), stations: document.querySelectorAll('.tc-steps li i').length };
    });
    ok('v7 1 ' + scheme + ': the window ground is a faint radial wash of the live accent over the canvas', /radial-gradient/.test(R.appBg) && R.theme === scheme, R);
    const canvasRgb = (() => { const n = parseInt(R.canvas.slice(1), 16); return [n >> 16, (n >> 8) & 255, n & 255]; })();
    ok('v7 1 ' + scheme + ': the canvas is ' + (scheme === 'dark' ? 'graphite (B >= R, near black)' : 'near white (B >= R)'), canvasRgb[2] >= canvasRgb[0] && (scheme === 'dark' ? canvasRgb[0] < 24 : canvasRgb[0] > 220), canvasRgb);
    ok('v8 ' + scheme + ': the limits in the status bar are bars named by the agent (no VU meter, no model in the name), the numbers stay', R.lim && R.who && R.limW > 0 && !R.gauge && /\d+%/.test(R.limText) && !/Opus|Sonnet/.test(R.limText) && +R.limP >= 0 && +R.limP <= 1, R);
    ok('F10 4 ' + scheme + ': message text >= 7:1 and the quiet line >= 4.5:1 against what is drawn behind them', ratio(mix(R.text), mix(R.textBg)) >= 7 && (!R.sec || ratio(mix(R.sec), mix(R.secBg)) >= 4.5), { text: ratio(mix(R.text), mix(R.textBg)), sec: R.sec && ratio(mix(R.sec), mix(R.secBg)), R });
    ok('F10 3 ' + scheme + ': the stages of the task card are one signal line of stations', R.stations >= 7, R.stations);
    await p.close();
  }
  // v7 2: the aura of a working agent card in its env colour, the breathing of the ones that wait out of sight, «Ждут: N ⌘J»
  const a = await b.newPage({ viewport: { width: 1440, height: 900 }, colorScheme: 'dark' });
  await a.goto(base + '/index.html?lang=ru&platform=mac&demo=agents');
  await a.waitForTimeout(3300);
  const run = await a.evaluate(() => { const c = document.querySelector('.rail7-list .tab[data-state="run"]'); return c && { anim: getComputedStyle(c, '::after').animationName, env: getComputedStyle(c).getPropertyValue('--env').trim() }; });
  ok('v7 2: a working agent card wears a running light (aura7) in the colour of its env', !!run && run.anim === 'aura7' && /^#[0-9a-f]{6}$/i.test(run.env), run);
  await a.waitForTimeout(6500);
  const wt = await a.evaluate(() => { const c = document.querySelector('.rail7-list .tab[data-state="wait"]'); return { n: document.querySelectorAll('.rail7-list .tab[data-state="wait"]').length, anim: c && getComputedStyle(c, '::after').animationName, btn: (document.querySelector('.nb-wait:not([hidden])') || {}).textContent }; });
  ok('v7 2: agents that went quiet out of sight breathe (breathe7) and «Ждут: N ⌘J» shows in the foot', wt.n >= 1 && wt.anim === 'breathe7' && /Ждут: \d/.test(wt.btn || ''), wt);
  // the line of the «ждёт тебя» notification is the whole wrapped line, so a key keeps its prefix for the scrub
  const wrap = await a.evaluate(async () => {
    const v = NS.term.views.get(NS.tabs.active.id), line = 'KEY=sk-ant-' + 'x'.repeat(3 * v.term.cols);
    v.term.write('\r\n' + line + '\r\n');
    await new Promise((r) => setTimeout(r, 200));
    const got = v.tail(1)[0] || ''; return { ok: got.startsWith('KEY=sk-ant-x'), got, cols: v.term.cols };
  });
  ok('v7 2: the last line of a terminal joins a line wrapped at its width', wrap.ok, wrap);
  // the pane tools appear on hover without moving «Изменения»: a click aimed at it must not land on «split»
  const chgX = () => a.evaluate(() => { const b = [...document.querySelectorAll('.pane-h .pt-l')].find((x) => /Изменения/.test(x.textContent)); return b ? Math.round(b.getBoundingClientRect().left) : null; });
  await a.mouse.move(2, 2); await a.waitForTimeout(150);
  const x0 = await chgX();
  const box = await a.evaluate(() => { const r = document.querySelector('.pane-h').getBoundingClientRect(); return { x: r.left + 40, y: r.top + 60 }; });
  await a.mouse.move(box.x, box.y); await a.waitForTimeout(150);
  const x1 = await chgX();
  ok('v7 2: hovering a pane shows its tools and «Изменения» stays where it was', x0 !== null && x0 === x1, { x0, x1 });
  await a.close();
  // 2: no aura and no breathing under prefers-reduced-motion
  const q = await b.newPage({ viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce' });
  await q.goto(base + '/index.html?lang=ru&platform=mac&demo=agents');
  await q.waitForTimeout(3300);
  const rmRun = await q.evaluate(() => { const c = document.querySelector('.rail7-list .tab[data-state="run"]'); return c ? getComputedStyle(c, '::after').animationName : 'no-run'; });
  await q.waitForTimeout(6500);
  const rm = await q.evaluate(() => { const c = document.querySelector('.rail7-list .tab[data-state="wait"]'); return { wait: c ? getComputedStyle(c, '::after').animationName : 'no-wait', t: getComputedStyle(document.documentElement).getPropertyValue('--t').trim() }; });
  ok('v7 2: with prefers-reduced-motion the aura does not run and the waiting card does not breathe', (rmRun === 'none' || rmRun === 'no-run') && rm.wait === 'none', { rmRun, rm });
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
  const sent = await p.evaluate(() => ({ menu: !document.querySelector('.newchat-empty .pj-pop').hidden, chats: NS.store.s.chats ? [...NS.store.s.chats.values()].flat().length : 0, left: document.querySelector('.composer textarea').value, pending: document.querySelectorAll('.me.sending').length }));
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
  // v6 (task 27): the message goes into the core's queue first (chat_queue_add, first) and the core sends it after the cancel
  const sentAfter = n2.findIndex((c) => c[0] === 'chat_queue_add' && c[1].first === true && /без кэша/.test(c[1].text)), cancelAt = n2.findIndex((c) => c[0] === 'cancel');
  const q2 = await p.evaluate(() => ({ queued: document.querySelectorAll('.me.queued').length, mine: [...document.querySelectorAll('.cfeed .me .me-b')].map((x) => x.textContent.trim()).filter((x) => /без кэша/.test(x)).length }));
  ok('F12 1: «Сразу» puts the message first in the core queue, cancels the run, then the message goes and stops being grey', cancelAt >= 0 && sentAfter >= 0 && sentAfter < cancelAt && q2.queued === 0 && q2.mine >= 1, { n2: n2.map((c) => c[0]), q2 });
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
  ok('v10 F14: the entrance «Автоматика» sits after «⌘K Поиск» in the foot of the sessions column (the settings are the gear under the agents) and carries the count of suggestions', nav && /^Автоматика\s*5$/.test(nav.text) && /Поиск/.test(nav.next) && nav.after === null, nav);
  await p.click('.nb-auto'); await p.waitForTimeout(500);
  const rows = () => p.evaluate(() => [...document.querySelectorAll('#a4-sched .a4-row')].map((r) => ({ t: r.querySelector('b').textContent, on: r.classList.contains('on'), sw: r.querySelector('[role=switch]').getAttribute('aria-checked'), txt: r.textContent })));
  const r0 = await rows();
  ok('F14 1: the schedule lists the tasks with preset, time and the next firing; the held one says why (limit under 10%)', r0.length === 3 && r0[0].txt.includes('03:00') && /по будням/.test(r0[0].txt) && /следующий запуск/.test(r0[0].txt) && /меньше 10% лимита/.test(r0[1].txt) && !r0[2].on, r0);
  const near = await p.evaluate(() => { const n = document.querySelector('#a4-sched .a4-next'); return n && { text: n.textContent, hidden: n.hidden, bg: getComputedStyle(document.querySelector('#a4-sched')).backgroundImage }; });
  ok('F14 1: the schedule panel leads with the nearest run and wears a soft accent gradient', near && !near.hidden && /Ближайший запуск/.test(near.text) && /gradient/.test(near.bg), near);
  await p.click('#a4-sched .a4-row:nth-child(1) [role=switch]'); await p.waitForTimeout(300);
  const r1 = await rows();
  ok('F14 1: the switch turns a schedule off (the core agrees after a reload) and its «next» disappears', r1[0].sw === 'false' && !r1[0].on && !/следующий запуск/.test(r1[0].txt), r1[0]);
  // a new one from a template
  await p.click('[data-tpl=todo]'); await p.waitForTimeout(100);
  const filled = await p.evaluate(() => ({ title: document.querySelector('#a4-form input[type=text]').value, prompt: document.querySelector('#a4-form textarea').value.length > 20 }));
  ok('F14 1: a template fills the name and the task text', filled.title === 'Добить TODO' && filled.prompt, filled);
  await p.fill('.a4-time', '25:99'); await p.click('#a4-form .btn.primary'); await p.waitForTimeout(300);
  ok('F14 1: a bad time is refused, nothing is added', (await rows()).length === 3, null);
  await p.fill('.a4-time', '02:15');
  await p.click('#a4-form [data-preset=weekly]'); await p.waitForTimeout(100);
  const dayShown = await p.evaluate(() => !document.querySelector('.a4-day').hidden && /раз в неделю, среда в 02:15/.test(document.querySelector('#a4-form .a4-sum').textContent));
  await p.click('#a4-form .btn.primary'); await p.waitForTimeout(400);
  const r2 = await rows();
  ok('F14 1: «Раз в неделю» shows the day of the week; saving adds a row that is still off (the person switches it on)', dayShown && r2.length === 4 && r2[3].t === 'Добить TODO' && /02:15/.test(r2[3].txt) && /по средам/.test(r2[3].txt) && !r2[3].on, { dayShown, last: r2[3] });
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

// v6 (G67, G68) - issues of GitHub in «Предлагаю», the question «Закрыть issue #N?», sessions from the terminal (mock-sessions.js plays the core)
async function v6is(b, base, out) {
  const ok = (name, cond, detail) => out.push((cond ? 'PASS ' : 'FAIL ') + name + (cond ? '' : ' ' + JSON.stringify(detail)));
  const open = async (qs) => {
    const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
    await p.goto(base + '/index.html?lang=ru&platform=mac&env=claude&chat=new' + (qs || '')); await p.waitForTimeout(1300);
    return p;
  };
  // issues next to the TODOs
  let p = await open('&issues=on&ask=1');
  await p.click('.nb-auto'); await p.waitForTimeout(500);
  const cards = () => p.evaluate(() => ({ n: document.querySelectorAll('#a4-sug .a4-card').length, issues: [...document.querySelectorAll('#a4-sug .a4-card.issue')].map((c) => c.textContent), todos: document.querySelectorAll('#a4-sug .a4-card:not(.issue)').length, nav: String(NS.auto.count()), links: document.querySelectorAll('#a4-sug .a4-card.issue .a4-loc .linkb').length }));
  const c0 = await cards();
  ok('G67: the issues of the project stand in «Предлагаю» next to the TODO cards, marked «issue #N» with a link; the TODOs are all still there', c0.issues.length === 2 && c0.todos === 5 && /issue #58/.test(c0.issues[0]) && /Падает экспорт счетов/.test(c0.issues[0]) && c0.links === 2, c0);
  ok('G67: the bar button counts suggestions and the open question (5 + 2 + 1)', c0.nav === '8', c0);
  const ask = await p.evaluate(() => { const a = document.querySelector('#a4-sug .a4-ask'); return a && { text: a.textContent, btns: [...a.querySelectorAll('button')].map((x) => x.textContent) }; });
  ok('G67.2: a merged issue task asks «Закрыть issue #44?» with «Закрыть» and «Оставить» and says nothing is closed before the answer', ask && /Закрыть issue #44\?/.test(ask.text) && ask.btns.join('|') === 'Закрыть|Оставить' && /Пока не нажмёшь «Закрыть», issue остаётся открытым/.test(ask.text), ask);
  await p.click('#a4-sug .a4-ask .btn.primary'); await p.waitForTimeout(400);
  const after = await p.evaluate(async () => ({ asks: document.querySelectorAll('#a4-sug .a4-ask').length, data: await NS.api.call('auto_get').then((d) => d.issues.asks.length) }));
  ok('G67.2: after «Закрыть» the question is gone for good', after.asks === 0 && after.data === 0, after);
  await p.click('#a4-sug .a4-card.issue:nth-child(1) .btn'); await p.waitForTimeout(500);
  const made = await p.evaluate(async () => (await NS.api.call('list_tasks')).filter((t) => /^Issue #58/.test(t.prompt)).length);
  const c1 = await cards();
  ok('G67.1: «Сделать» on an issue queues a task from it and the card is gone', made === 1 && c1.issues.length === 1, { made, c1 });
  await p.close();
  p = await open('&issues=on');
  await p.click('.nb-auto'); await p.waitForTimeout(500);
  await p.click('#a4-sug .a4-card.issue:nth-child(1) .a4-acts .linkb'); await p.waitForTimeout(400);
  const c2 = await p.evaluate(() => ({ issues: document.querySelectorAll('#a4-sug .a4-card.issue').length }));
  ok('G67.1: «Не предлагать такое» removes an issue card', c2.issues === 1, c2);
  await p.close();
  // no gh
  p = await open('&issues=nogh');
  await p.click('.nb-auto'); await p.waitForTimeout(500);
  const quiet = await p.evaluate(() => ({ note: (document.querySelector('.a4-issue-note') || {}).textContent, todos: document.querySelectorAll('#a4-sug .a4-card').length, issues: document.querySelectorAll('#a4-sug .a4-card.issue').length }));
  ok('G67: without gh the issues give a quiet line with the reason and the TODO cards work as before', /issues недоступны: не найден gh/.test(quiet.note || '') && quiet.todos === 5 && quiet.issues === 0, quiet);
  await p.close();
  // sessions
  p = await open('&screen=sessions');
  const rows = () => p.evaluate(() => [...document.querySelectorAll('.ses-row')].map((r) => ({ txt: r.textContent, bad: r.classList.contains('bad'), off: r.querySelector('button').disabled })));
  await p.waitForTimeout(400);
  const r0 = await rows();
  ok('G68: the project shows its sessions from the terminal: date, first words, the number of messages, the branch', r0.length === 5 && /Разбери, почему тесты по налогам/.test(r0[0].txt) && /38 сообщ\./.test(r0[0].txt) && /ветка fix\/tax-rounding/.test(r0[0].txt), r0[0]);
  ok('G68.2: a file that cannot be read is marked «не читается», cannot be continued, and the others are shown', r0[3].bad && /не читается/.test(r0[3].txt) && r0[3].off && !r0[4].off && /больше 4000 сообщ\./.test(r0[4].txt), r0[3]);
  await p.click('.ses-row:nth-child(1) .btn'); await p.waitForTimeout(400);
  const cont = await p.evaluate(async () => ({ off: document.querySelector('.ses-row .btn').disabled, task: (await NS.api.call('list_tasks')).filter((t) => t.resume_session === '9f1c2e7a-4b1d-4e0a-9c55-0d6a7b21c001').length }));
  ok('G68.1: «Продолжить» queues a task that resumes that session and the button goes off', cont.off && cont.task === 1, cont);
  const lay = await p.evaluate(() => document.documentElement.scrollWidth > innerWidth);
  ok('G68: the sessions screen has no horizontal scroll at 1440', !lay, lay);
  await p.close();
  p = await open('&screen=sessions&sessions=empty'); await p.waitForTimeout(400);
  ok('G68.2: no sessions - a plain sentence, not an empty screen', /Сессий из терминала для этого проекта нет/.test(await p.evaluate(() => (document.querySelector('.ses-none') || {}).textContent || '')), null);
  await p.close();
  p = await open('&screen=sessions&sessions=many'); await p.waitForTimeout(400);
  const n1 = (await rows()).length;
  await p.click('.ses-foot .btn'); await p.waitForTimeout(300);
  const n2 = (await rows()).length;
  await p.click('.ses-foot .btn'); await p.waitForTimeout(300);
  const n3 = (await rows()).length, more = await p.evaluate(() => !!document.querySelector('.ses-foot .btn'));
  ok('G68.3: hundreds of sessions come by fifty, the newest on top, and «Показать ещё» ends with the last page', n1 === 50 && n2 === 100 && n3 === 130 && !more && /Задача номер 130/.test((await rows())[0].txt), { n1, n2, n3, more });
  const words = await p.evaluate(() => { const d = NSI18n.dicts; const keys = Object.keys(d.ru).filter((k) => /^iss\.|^ses\.|^err\.(session_|no_question|bad_session_id|no_home)/.test(k)); return { n: keys.length, missing: keys.filter((k) => !(k in d.en)), built: ['gh_not_installed', 'gh_not_logged_in', 'gh_error'].map((s) => NSI18n.has('iss.why.' + s)) }; });
  ok('G67/G68: every new word exists in ru and en', words.n > 30 && !words.missing.length && words.built.every(Boolean), words);
  await p.close();
}

// v6 task 27 - "думает" right after sending (G80, G84), the thoughts, the smooth print (G81) and the lines of the app in words (G62)
async function f27(b, base, out) {
  const ok = (name, cond, detail) => out.push((cond ? 'PASS ' : 'FAIL ') + name + (cond ? '' : ' ' + JSON.stringify(detail)));
  const open = async (qs, opts) => { const p = await b.newPage(Object.assign({ viewport: { width: 1440, height: 900 } }, opts || {})); await p.goto(base + '/index.html?lang=ru&platform=mac&' + qs); await p.waitForTimeout(1700); return p; };
  const send = (p, text) => p.evaluate((text) => {
    const ta = document.querySelector('.composer textarea'); ta.value = text; ta.dispatchEvent(new Event('input'));
    document.querySelector('#sendBtn').click();
    const lb = document.querySelector('.cfeed .livebar'), mine = [...document.querySelectorAll('.cfeed .me .me-b')].some((x) => x.textContent.includes(text));
    return { shown: !lb.hidden && lb.classList.contains('thinking'), verb: lb.querySelector('.lv-v').textContent, mine, last: document.querySelector('.cfeed').lastElementChild === lb };
  }, text);
  // 1: the same frame as the bubble, the env's own words, the thoughts folded
  let p = await open('env=claude&chat=how&reply=5000&thoughts=1&verbs=Колдую,Варю');
  const s1 = await send(p, 'Почему кэш живёт 5 минут?');
  ok('G80: «думает» shows in the same frame as the sent bubble, where the answer will be', s1.shown && s1.mine && s1.last, s1);
  ok('G84: the indicator speaks the words of `spinnerVerbs` when the settings give them', s1.verb === 'Колдую', s1.verb);
  await p.waitForTimeout(2300);
  const s2 = await p.evaluate(() => { const lb = document.querySelector('.cfeed .livebar'); return { verb: lb.querySelector('.lv-v').textContent, meta: lb.querySelector('.lv-m').textContent, anim: getComputedStyle(lb.querySelector('.lv-v')).animationName }; });
  ok('G80: the word changes after ~2 s, the timer counts seconds, the word shimmers', s2.verb === 'Варю' && /\(0:0[2-3]/.test(s2.meta) && s2.anim === 'think-shimmer', s2);
  const th = await p.evaluate(() => { const t = document.querySelector('.cfeed .thought'); return t && { label: t.querySelector('b').textContent, folded: t.querySelector('.th-b').hidden, color: getComputedStyle(t).color, ink: getComputedStyle(document.documentElement).getPropertyValue('--ink3').trim() }; });
  ok('G80.1: the thoughts are one grey folded line «Думает»', th && th.label === 'Думает' && th.folded, th);
  await p.click('.cfeed .th-h'); await p.waitForTimeout(100);
  const th2 = await p.evaluate(() => { const b = document.querySelector('.cfeed .th-b'); return { open: !b.hidden, text: b.textContent }; });
  ok('G80.1: a click opens the text of the thoughts', th2.open && /кэш отчётов/.test(th2.text), th2);
  await p.evaluate(() => { window.__realNow = Date.now; const real = Date.now.bind(Date); Date.now = () => real() + 65000; });
  await p.waitForTimeout(400);
  const s3 = await p.evaluate(() => { const lb = document.querySelector('.cfeed .livebar'); const st = lb.querySelector('.lv-stop'); return { verb: lb.querySelector('.lv-v').textContent, meta: lb.querySelector('.lv-m').textContent, stop: st && !st.hidden && st.textContent }; });
  ok('G80.2: after 60 s «Долго думает · 1:05» and «Стоп»', s3.verb === 'Долго думает' && /^· 1:0[5-9]$/.test(s3.meta) && s3.stop === 'Стоп', s3);
  await p.evaluate(() => { Date.now = window.__realNow; });
  await p.waitForTimeout(3500);
  const s4 = await p.evaluate(() => { const lb = document.querySelector('.cfeed .livebar'); return { hidden: lb.hidden || !lb.classList.contains('thinking'), answer: [...document.querySelectorAll('.cfeed .msg.ai .msg-b')].some((x) => /Смотрю код/.test(x.textContent)) }; });
  ok('G80: «думает» goes away with the first text of the answer', s4.hidden && s4.answer, s4);
  await p.close();
  // 2: reduced motion: no shimmer, the word stays
  p = await open('env=claude&chat=how&reply=5000', { reducedMotion: 'reduce' });
  await send(p, 'Где тесты?');
  await p.waitForTimeout(2400);
  const r1 = await p.evaluate(() => { const v = document.querySelector('.cfeed .livebar .lv-v'); return { verb: v.textContent, anim: getComputedStyle(v).animationName, dot: getComputedStyle(document.querySelector('.cfeed .livebar .lv-spin')).animationName }; });
  ok('G80.2: with reduced motion nothing shimmers or pulses and the word does not turn', r1.verb === 'Думаю' && r1.anim === 'none' && r1.dot === 'none', r1);
  await p.close();
  // 3: the smooth print: a streamed answer (chat-delta) and a whole message (a CLI without deltas) grow by ≤ 40 characters a frame in one node
  const measure = (p) => p.evaluate(() => new Promise((done) => {
    let node = null, text = null, prev = 0; const steps = [], start = performance.now(); let same = true;
    const tick = () => {
      const rv = document.querySelector('.cfeed .msg.streaming .rv');
      if (rv && !node) { node = rv; text = rv.firstChild; }
      if (node) {
        if (rv && rv !== node) same = false;
        if (node.firstChild !== text) same = false;
        const n = text ? text.length : 0; if (n > prev) steps.push(n - prev); prev = n;
        if (!node.isConnected) return done({ steps: steps.length, max: Math.max(0, ...steps), same, total: prev, md: !!document.querySelector('.cfeed .msg.ai:not(.streaming) .msg-b .md, .cfeed .msg:not(.streaming) .msg-b') });
      }
      if (performance.now() - start > 25000) return done({ timeout: true, steps: steps.length, same });
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  }));
  p = await open('env=claude&chat=cache&live=fast');
  const m1 = await measure(p);
  ok('G81: a streamed answer grows in one node, ≤ 40 characters a frame, then markdown takes its place', !m1.timeout && m1.same && m1.steps > 10 && m1.max <= 40 && m1.md, m1);
  await p.close();
  p = await open('env=claude&chat=how&reply=300&long=1');
  const pm = measure(p);
  await send(p, 'Расскажи подробно');
  const m2 = await pm;
  ok('G81: a whole message of a CLI without deltas prints the same way (one node, ≤ 40 a frame)', !m2.timeout && m2.same && m2.steps > 10 && m2.max <= 40, m2);
  await p.close();
  // 4: G81.1 - scrolled up, the feed is not dragged down by the print
  p = await open('env=claude&chat=how&reply=300&long=1');
  await send(p, 'Ещё подробнее');
  await p.waitForTimeout(1000);
  const sc = await p.evaluate(() => new Promise((done) => { const w = document.querySelector('.feed-wrap'); w.scrollTop = 0; w.dispatchEvent(new Event('scroll')); setTimeout(() => done({ top: w.scrollTop, h: w.scrollHeight - w.clientHeight }), 700); }));
  ok('G81.1: a person who scrolled up stays where they are while the answer prints', sc.top < 80 && sc.h > 200, sc);
  await p.close();
  // 5: the lines of the app in words (G62): «Отвечу после HH:MM», «Не ответил: …» with «Повторить», no raw keys
  p = await open('env=claude&chat=how');
  await p.evaluate(() => {
    const at = new Date().toISOString();
    NS.store.emit('chatmsg', { id: 'sl1', chat_id: 'how', at, who: 'system', text: 'will_answer_after: 12:30', kind: 'warn', reply_to: 'x' });
    NS.store.emit('chatmsg', { id: 'sl2', chat_id: 'how', at, who: 'system', text: 'not_answered: interrupted', kind: 'error', reply_to: 'x' });
    NS.store.emit('chatmsg', { id: 'sl3', chat_id: 'how', at, who: 'system', text: 'note_not_passed: task_already_merged', kind: 'warn', reply_to: 'x' });
  });
  await p.waitForTimeout(300);
  const ln = await p.evaluate(() => [...document.querySelectorAll('.cfeed .sysline')].slice(-3).map((x) => x.textContent.trim()));
  ok('G62: system lines are in words, not raw keys; an interrupted answer has «Повторить»', /Лимит исчерпан\. Отвечу после 12:30/.test(ln[0] || '') && /^Не ответил: приложение закрылось посреди ответа.*Повторить$/.test(ln[1] || '') && /задача уже влита/.test(ln[2] || '') && !ln.some((x) => /[a-z]+_[a-z]+/.test(x)), ln);
  await p.close();
}

// v6 (task 28): a lane of the swarm waiting for its limit (G64.1), «Расходы» (G70), «О программе» with the check of the release and
// the error report (G69, G71), the bar «Вышла версия X»
async function ops6(b, base, out) {
  const ok = (name, cond, detail) => out.push((cond ? 'PASS ' : 'FAIL ') + name + (cond ? '' : ' ' + JSON.stringify(detail)));
  const open = async (qs) => {
    const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
    await p.goto(base + '/index.html?lang=ru&platform=mac&first=chat' + qs); await p.waitForTimeout(1500);
    return p;
  };
  let p = await open('&screen=swarm&swstate=paused');
  const lane = await p.evaluate(() => ({ lanes: [...document.querySelectorAll('.lane.paused')].map((l) => l.textContent), chip: [...document.querySelectorAll('.st.st-wait')].map((x) => x.textContent), running: document.querySelectorAll('.lane:not(.paused)').length }));
  ok('G64.1: a lane on the limit says «ждёт лимит до HH:MM» on its track and its card, the other lanes stay', lane.lanes.length === 1 && /C · .*ждёт лимит до \d\d:\d\d/.test(lane.lanes[0]) && lane.chip.some((x) => /ждёт лимит до \d\d:\d\d/.test(x)) && lane.running >= 2, lane);
  await p.close();
  p = await open('&screen=spend');
  const sp = await p.evaluate(() => { const rows = [...document.querySelectorAll('.sp-table tbody tr')].map((r) => [...r.cells].map((c) => c.textContent)); return { rows, foot: (document.querySelector('.sp-table tfoot') || {}).textContent, scroll: document.documentElement.scrollWidth > innerWidth }; });
  ok('G70: «Расходы» is a table day × project with money and tokens; the chat is a row «чат»; an env without money says «нет данных»', sp.rows.length === 8 && sp.rows.some((r) => /чат/.test(r[1]) && r[3] === '$0.18') && sp.rows.some((r) => r[2] === 'Codex' && r[3] === 'нет данных') && sp.rows.some((r) => /812/.test(r[4])) && !sp.scroll, sp);
  await p.click('.sp-range button:nth-child(3)'); await p.waitForTimeout(400);
  const all = await p.evaluate(() => [...document.querySelectorAll('.sp-table tbody tr.sp-month')].map((r) => r.cells[0].textContent));
  ok('G70.1: «Всё время» shows days older than 400 folded into a month row', all.length === 1 && /за месяц/.test(all[0]), all);
  await p.close();
  p = await open('&screen=spend&spend=empty');
  ok('G70.1: no spending yet - «Расходов пока нет», not an empty table', /Расходов пока нет/.test(await p.evaluate(() => (document.querySelector('.sp-empty') || {}).textContent || '')) && !(await p.evaluate(() => document.querySelector('.sp-table'))), null);
  await p.close();
  p = await open('&screen=about');
  const off = await p.evaluate(() => ({ text: (document.querySelector('#about-updates') || {}).textContent, check: (document.querySelector('#about-check') || {}).disabled, bar: !!document.querySelector('.appupd6') }));
  ok('G69: without release_feed the check is off and «О программе» says «адрес обновлений не задан»; no bar', /адрес обновлений не задан/.test(off.text || '') && off.check === true && !off.bar, off);
  await p.click('#about-diag-go'); await p.waitForTimeout(400);
  const toast = await p.evaluate(() => document.body.textContent.includes('Отчёт готов: /Users/dev/Downloads/nightshift-report-'));
  ok('G71: «Собрать отчёт об ошибке» makes the zip in Downloads and says where', toast, null);
  await p.close();
  p = await open('&update=error&screen=about');
  const err = await p.evaluate(() => (document.querySelector('#about-updates') || {}).textContent || '');
  ok('G69.1: a failed check is quiet (no bar) and «О программе» shows its time and error', /Последняя проверка: /.test(err) && /Не вышло проверить: нет сети/.test(err) && !(await p.evaluate(() => document.querySelector('.appupd6'))), err);
  await p.close();
  p = await open('&update=new');
  const bar = await p.evaluate(() => (document.querySelector('.appupd6') || {}).textContent || '');
  await p.click('.appupd6 .linkb'); await p.waitForTimeout(300);
  const gone = await p.evaluate(async () => ({ bar: !!document.querySelector('.appupd6'), show: (await NS.api.call('app_update_state')).show }));
  ok('G69: «Вышла версия X» with a link to the release; closed, it is gone and not shown again for that version', /Вышла версия 0\.2\.0/.test(bar) && /Открыть релиз/.test(bar) && !gone.bar && gone.show === false, { bar, gone });
  const words = await p.evaluate(() => { const d = NSI18n.dicts; const keys = Object.keys(d.ru).filter((k) => /^(spend|about|appupd|diag)\.|^swarm\.(paused|part_paused)/.test(k)); return { n: keys.length, missing: keys.filter((k) => !(k in d.en)) }; });
  ok('G64/G69/G70/G71: every new word exists in ru and en', words.n > 40 && !words.missing.length, words);
  await p.close();
}

// v6 (task 29): the chat as a shell of the CLI - the menu "/" (G82, G84.1), images in the field (G65), "В терминале" and back (G83, G85)
async function shell6(b, base, out) {
  const ok = (name, cond, detail) => out.push((cond ? 'PASS ' : 'FAIL ') + name + (cond ? '' : ' ' + JSON.stringify(detail)));
  const open = async (qs) => { const p = await b.newPage({ viewport: { width: 1440, height: 900 } }); await p.goto(base + '/index.html?lang=ru&platform=mac&' + qs); await p.waitForTimeout(1700); return p; };
  const type = (p, text) => p.evaluate((text) => { const ta = document.querySelector('.composer textarea'); ta.focus(); ta.value = text; ta.setSelectionRange(text.length, text.length); ta.dispatchEvent(new Event('input')); }, text);
  const pop = (p) => p.evaluate(() => { const m = document.querySelector('.composer .slash6'); return m && { open: !m.hidden, secs: [...m.querySelectorAll('.sl-sec h4 span:first-child')].map((x) => x.textContent), opts: [...m.querySelectorAll('.sl-o b')].map((x) => x.textContent), empty: (m.querySelector('.sl-empty') || {}).textContent || '', mods: (m.querySelector('.sl-mods h4 .sl-term') || {}).textContent || '' }; });
  // 1: the menu "/" of Claude Code: four sections, the search, Enter inserts
  let p = await open('env=claude&chat=how&log=1');
  await type(p, '/'); await p.waitForTimeout(300);
  const m1 = await pop(p);
  ok('G82: «/» in the field opens the menu of the env: Скиллы, Команды, MCP, Моды', m1 && m1.open && m1.secs.join() === 'Скиллы,Команды,MCP,Моды' && m1.opts.includes('/review'), m1);
  ok('G84.1: the section «Моды» says «рисуют в терминале - открыть»', m1 && m1.mods === 'рисуют в терминале - открыть', m1);
  await type(p, 'look /rel'); await p.waitForTimeout(200);
  const m2 = await pop(p);
  ok('G82: the typed name filters the menu', m2 && m2.opts.join() === '/release-notes', m2);
  await p.press('.composer textarea', 'Enter'); await p.waitForTimeout(150);
  const v = await p.evaluate(() => ({ field: document.querySelector('.composer textarea').value, open: !document.querySelector('.composer .slash6').hidden, sent: JSON.parse(document.documentElement.getAttribute('data-calls') || '[]').some((c) => c[0] === 'send_chat') }));
  ok('G82: Enter puts «/release-notes » into the field and closes the menu, nothing is sent', v.field === 'look /release-notes ' && !v.open && !v.sent, v);
  await type(p, '/'); await p.waitForTimeout(150); await p.press('.composer textarea', 'Escape'); await p.waitForTimeout(100);
  ok('G82: Esc closes the menu', !(await pop(p)).open, null);
  // 2: an image pasted with ⌘V: a thumbnail with ×, × removes it, a second one goes with the message as a path
  await type(p, '');
  const paste = () => p.evaluate(() => {
    const b = Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg=='), (c) => c.charCodeAt(0));
    const dt = new DataTransfer(); dt.items.add(new File([b], 'shot.png', { type: 'image/png' }));
    document.querySelector('.composer textarea').dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }));
  });
  const clip = await p.evaluate(() => { const c = document.querySelector('.composer .clip6'); return c && !c.hidden; });
  ok('G65: the clip «Приложить картинку» shows for an env that reads images', clip, clip);
  await paste(); await p.waitForTimeout(300);
  const t1 = await p.evaluate(() => ({ n: document.querySelectorAll('.atts6 .att6 img').length, x: (document.querySelector('.att6-x') || {}).getAttribute && document.querySelector('.att6-x').getAttribute('aria-label') }));
  ok('G65: ⌘V of a screenshot shows its thumbnail over the field with ×', t1.n === 1 && t1.x === 'Убрать картинку', t1);
  await p.click('.att6-x'); await p.waitForTimeout(150);
  ok('G65: × takes the image off', (await p.evaluate(() => document.querySelectorAll('.atts6 .att6').length)) === 0, null);
  await paste(); await p.waitForTimeout(300);
  await type(p, 'Что не так на скриншоте?');
  await p.click('#sendBtn'); await p.waitForTimeout(400);
  const s1 = await p.evaluate(() => { const c = JSON.parse(document.documentElement.getAttribute('data-calls') || '[]').filter((x) => x[0] === 'send_chat').pop(); return { args: c && c[1], strip: document.querySelectorAll('.atts6 .att6').length, chip: [...document.querySelectorAll('.cfeed .me .att6-c')].map((x) => x.textContent) }; });
  ok('G65: the image goes with the text as a path in the app\'s data; the strip empties; the message shows it', s1.args && Array.isArray(s1.args.attachments) && s1.args.attachments.length === 1 && /nightshift\/attachments\//.test(s1.args.attachments[0]) && s1.strip === 0 && s1.chip.length === 1, s1);
  // 3: a file dropped on the field is an image of the message, not a project; outside the field it is not taken
  const d1 = await p.evaluate(() => { const r = document.querySelector('.composer').getBoundingClientRect(), dpr = devicePixelRatio; const inField = NS.composerDrop({ paths: ['/Users/dev/Desktop/shot.png'], position: { x: (r.left + 20) * dpr, y: (r.top + 10) * dpr } }); const outside = NS.composerDrop({ paths: ['/Users/dev/code/other'], position: { x: 5, y: 5 } }); return { inField, outside }; });
  await p.waitForTimeout(200);
  const d2 = await p.evaluate(() => document.querySelectorAll('.atts6 .att6 img').length);
  ok('G65.2: a file dropped on the field is attached (not «open the folder»); elsewhere the window keeps its own drop', d1.inField === true && d1.outside === false && d2 === 1, { d1, d2 });
  await p.close();
  // 4: an env without images: no clip, a pasted picture is refused in words
  p = await open('env=gemini&chat=avif');
  const g1 = await p.evaluate(() => { const c = document.querySelector('.composer .clip6'); return c ? c.hidden : 'none'; });
  await p.evaluate(() => {
    const dt = new DataTransfer(); dt.items.add(new File([new Uint8Array([137, 80, 78, 71])], 'a.png', { type: 'image/png' }));
    document.querySelector('.composer textarea').dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }));
  });
  await p.waitForTimeout(300);
  const g2 = await p.evaluate(() => ({ toast: document.body.textContent.includes('Эта среда не принимает картинки'), n: document.querySelectorAll('.atts6 .att6').length }));
  ok('G65.2: an env without images has no clip and a pasted image says «Эта среда не принимает картинки»', g1 === true && g2.toast && g2.n === 0, { g1, g2 });
  await p.close();
  // 5: an env with nothing: «В среде нет скиллов - открыть Каталог»
  p = await open('env=claude&chat=how&slash=empty');
  await type(p, '/'); await p.waitForTimeout(300);
  const e1 = await pop(p);
  ok('G82: an env with nothing says «В среде нет скиллов - открыть Каталог»', e1 && e1.open && e1.empty === 'В среде нет скиллов - открыть Каталог', e1);
  await p.close();
  // 6 (v10): «Терминал» in the head of the chat puts the same session (the chat's copy) in the same pane - no new tab; «Чат» brings the feed back
  p = await open('env=claude&chat=how');
  const tabs0 = await p.evaluate(() => NS.tabs.state.tabs.length);
  const items = await p.evaluate(() => { const b = document.querySelector('.convo-h .menu-btn'); if (b) b.click(); return [...document.querySelectorAll('.convo-h .menu [role=menuitem]')].map((x) => x.textContent); });
  await p.click('.convo-h .view10 button:has-text("Терминал")'); await p.waitForTimeout(900);
  const tm = await p.evaluate(() => ({ term: !!document.querySelector('.view10-body .t-pane'), cwd: (document.querySelector('.t-pane .t-cwd') || {}).textContent || '', feed: !!document.querySelector('.cfeed'), tabs: NS.tabs.state.tabs.length,
    head: !!document.querySelector('.view10-h .view10 button.on:not([disabled])') }));
  ok('G83/v10: «Терминал» shows the session in the chat\'s copy in the same pane (no new tab), no second «В терминале» in the menu', !items.includes('В терминале') && tm.term && /\.chats\/how/.test(tm.cwd) && !tm.feed && tm.tabs === tabs0 && tm.head, { items, tm, tabs0 });
  await p.click('.view10-h .view10 button:has-text("Чат")'); await p.waitForTimeout(700);
  const back = await p.evaluate(() => ({ term: !!document.querySelector('.t-pane'), feed: !!document.querySelector('.cfeed') }));
  ok('G83/v10: «Чат» ends the CLI and brings the feed back', !back.term && back.feed, back);
  await p.close();
  // 7: no session yet: the reason in words and the explicit choice «Новый разговор в терминале»
  p = await open('env=claude&chat=how&term=nosession');
  await p.click('.convo-h .view10 button:has-text("Терминал")'); await p.waitForTimeout(400);
  const n1 = await p.evaluate(() => { const d = document.querySelector('dialog[open]'); return d && { text: d.textContent, ok: (d.querySelector('.btn.primary') || {}).textContent }; });
  ok('G83.2: no session - the reason in words and the choice «Новый разговор в терминале», not silently', n1 && /ещё нет разговора/.test(n1.text) && n1.ok === 'Новый разговор в терминале' && !(await p.evaluate(() => !!document.querySelector('.t-pane'))), n1);
  await p.click('dialog[open] .btn.primary'); await p.waitForTimeout(900);
  ok('G83.2: after the choice a new conversation opens in the terminal of the same pane', await p.evaluate(() => !!document.querySelector('.t-pane')), null);
  const words = await p.evaluate(() => { const d = NSI18n.dicts; const keys = Object.keys(d.ru).filter((k) => /^(slash|shell|img)\.|^err\.(no_session|cannot_resume|session_not_found|opened_in_terminal|continued_in_terminal|back_from_terminal|not_an_image|image_too_big|too_many_images|images_not_supported|attachment_not_found)$/.test(k)); return { n: keys.length, missing: keys.filter((k) => !(k in d.en)) }; });
  ok('G82/G83/G65: every new word exists in ru and en', words.n > 30 && !words.missing.length, words);
  await p.close();
}

// look8 - the drawn window: nine theme cards, OLED paints true black, an own theme from the editor (live, refused under 4.5:1,
// saved, deleted), window transparency (a see-through ground) and accent gradients on the panes
async function look8(b, base, out) {
  const ok = (name, cond, detail) => out.push((cond ? 'PASS ' : 'FAIL ') + name + (cond ? '' : ' ' + JSON.stringify(detail)));
  const p = await b.newPage({ viewport: { width: 1440, height: 900 }, colorScheme: 'dark' });
  const errs = [];
  p.on('pageerror', (e) => errs.push(String(e)));
  await p.goto(base + '/index.html?lang=ru&platform=mac&screen=settings&section=look');
  await p.waitForTimeout(1200);
  const cards = await p.evaluate(() => [...document.querySelectorAll('.th8-cards .card4')].map((c) => c.querySelector('.card4-t b').textContent));
  ok('look8 1: «Внешний вид → Тема» shows sixteen cards - the system one and fifteen ready themes (v9: designer palettes)', cards.length === 16 && cards.includes('Tokyo Night') && cards.includes('Чёрная OLED') && cards.includes('Светлая тёплая') && cards.includes('Полночь'), cards);
  await p.evaluate(() => [...document.querySelectorAll('.th8-cards .card4')].find((c) => /OLED/.test(c.textContent)).click());
  await p.waitForTimeout(400);
  const oled = await p.evaluate(() => ({ canvas: getComputedStyle(document.documentElement).getPropertyValue('--canvas').trim(), look: document.documentElement.dataset.look, theme: document.documentElement.dataset.theme, saved: NS.store.s.settings.look.theme }));
  ok('look8 2: a card applies the theme at once and saves it (OLED: canvas #000000, data-theme dark, data-look oled)', oled.canvas === '#000000' && oled.look === 'oled' && oled.theme === 'dark' && oled.saved === 'oled', oled);
  await p.evaluate(() => document.querySelector('.th8-tools .btn').click());
  await p.waitForTimeout(300);
  const ed = await p.evaluate(async () => {
    const box = document.querySelector('.th8-ed');
    if (!box) return null;
    const wait = (ms) => new Promise((r) => setTimeout(r, ms));
    const hexes = [...box.querySelectorAll('.cf8-hex')];
    const set = (i, v) => { hexes[i].value = v; hexes[i].dispatchEvent(new Event('input', { bubbles: true })); };
    set(1, '#203040'); await wait(60);
    const live = getComputedStyle(document.documentElement).getPropertyValue('--panel').trim();
    set(2, '#2a3a4a'); await wait(60);
    const blocked = box.querySelector('.btn.primary').disabled, low = box.querySelector('.th8-ratio').classList.contains('low');
    set(2, '#f4f6f8');
    const name = box.querySelector('.th8-name'); name.value = 'Проверка'; name.dispatchEvent(new Event('input'));
    await wait(60);
    box.querySelector('.btn.primary').click();
    await wait(400);
    const L = NS.store.s.settings.ui.tabs.look;
    return { live, blocked, low, themes: (L.themes || []).map((x) => x.name + ':' + x.panel), theme: NS.store.s.settings.look.theme, look: document.documentElement.dataset.look, cards: document.querySelectorAll('.th8-cards .card4').length, open: !!document.querySelector('.th8-ed') };
  });
  ok('look8 3: own theme - the editor previews on the whole window while typing, refuses text under 4.5:1, saves a named theme and selects it',
    !!ed && ed.live === '#203040' && ed.blocked && ed.low && ed.themes.join() === 'Проверка:#203040' && /^u/.test(ed.theme) && ed.look === 'custom' && ed.cards === 17 && !ed.open, ed);
  const gl = await p.evaluate(async () => {
    const cs = (el) => getComputedStyle(el), scr = document.querySelector('.screen, .pane'), before = cs(scr).backgroundImage;
    await NS.settingsV4.setLook({ glass: 40, fx: true });
    const de = document.documentElement;
    return { glass: de.hasAttribute('data-glass'), fx: de.hasAttribute('data-fx'), body: cs(document.body).backgroundColor, app: cs(document.querySelector('.app')).backgroundColor, washed: cs(scr).backgroundImage !== before };
  });
  ok('look8 4: transparency 40% - the page ground is see-through (body transparent, the canvas at 60%); gradients wash the panes with the accent', gl.glass && gl.fx && gl.body === 'rgba(0, 0, 0, 0)' && /0\.6\)$/.test(gl.app) && gl.washed, gl);
  const del = await p.evaluate(async () => {
    const wait = (ms) => new Promise((r) => setTimeout(r, ms));
    await NS.settingsV4.setLook({ glass: 0, fx: false });
    [...document.querySelectorAll('.th8-tools .btn')].find((x) => /Проверка/.test(x.textContent)).click(); await wait(200);
    document.querySelector('.th8-ed .btn.danger').click(); await wait(200);
    document.querySelector('#dlg .btn.primary').click(); await wait(400);
    return { themes: (NS.store.s.settings.ui.tabs.look.themes || []).length, theme: NS.store.s.settings.look.theme, cards: document.querySelectorAll('.th8-cards .card4').length, glass: document.documentElement.hasAttribute('data-glass') };
  });
  ok('look8 5: deleting the selected own theme (after a confirmation) falls back to the system theme; glass 0 takes data-glass away', del.themes === 0 && del.theme === 'system' && del.cards === 16 && !del.glass, del);
  await p.goto(base + '/index.html?lang=ru&platform=mac');
  await p.waitForTimeout(1200);
  const term = await p.evaluate(async () => {
    const wait = (ms) => new Promise((r) => setTimeout(r, ms)), cs = (sel) => getComputedStyle(document.querySelector(sel)).backgroundColor;
    const gl = () => document.querySelectorAll('.t-host canvas:not(.xterm-link-layer)').length;
    if (!document.querySelector('.t-pane')) { NS.tabs.newTab('term'); await wait(900); }     // v10: a session opens as a chat - a bare terminal for this check
    const solid = { ground: cs('.t-pane'), webgl: gl() };
    await NS.settingsV4.setLook({ glass: 40 }); await wait(300);
    const thin = { ground: cs('.t-pane'), host: cs('.t-host'), pane: cs('.pane.k-term'), webgl: gl() };
    await NS.settingsV4.setLook({ glass: 0 }); await wait(300);
    return { solid, thin, back: { ground: cs('.t-pane'), webgl: gl() } };
  });
  ok('look8 6: a terminal under a see-through window keeps one translucent ground (no stacked layers) and draws with the DOM renderer; glass 0 brings the solid ground and WebGL back',
    /^rgb\(/.test(term.solid.ground) && /, 0\.55\)$/.test(term.thin.ground) && term.thin.host === 'rgba(0, 0, 0, 0)' && term.thin.pane === 'rgba(0, 0, 0, 0)' && term.solid.webgl > 0 && term.thin.webgl === 0 && /^rgb\(/.test(term.back.ground) && term.back.webgl === term.solid.webgl, term);
  ok('look8: no page errors', !errs.length, errs);
  await p.close();
}
