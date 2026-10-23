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
  const b = await chromium.launch(process.env.NS_CHROME ? { executablePath: process.env.NS_CHROME } : {});
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
  out.push((r.buttons <= 14 ? 'PASS ' : 'FAIL ') + 'main view: at most 14 visible buttons outside the feed (' + r.buttons + ')' + (r.buttons <= 14 ? '' : ' ' + JSON.stringify(r.names)));
  out.push((r.pane > 0 && r.card > 0 && r.field > 0 && r.bar > 0 ? 'PASS ' : 'FAIL ') + 'computed border-radius > 0: pane ' + r.pane + ', card ' + r.card + ', field ' + r.field + ', bar ' + r.bar);
  process.stdout.write(out.join('\n') + '\n');
  if (out.some((l) => l.startsWith('FAIL'))) process.exitCode = 1;
  await b.close(); srv.close();
})().catch((e) => { console.error('browser check: ' + e.message); process.exitCode = 1; });
else process.stdout.write('SKIP drawn-window checks (set NS_PLAYWRIGHT to run them)\n');
