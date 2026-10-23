// v4 command palette ⌘K / Ctrl+K (story 249): fuzzy search over the registry's 'command' point, recent ones on top,
// the key of each command beside it, Esc closes. Pure ranking (NSPalette.score / rank) runs in selftest.
(function (root) {
  // subsequence match, case-insensitive (ё = е); null = no match. Higher is better: letters in a row, letters at a word start,
  // an early first letter, a shorter label.
  const norm = (s) => String(s || '').toLowerCase().replace(/ё/g, 'е');
  function score(query, text) {
    const q = norm(query).replace(/\s+/g, ''), s = norm(text);
    if (!q) return 0;
    let si = 0, total = 0, run = 0, first = -1;
    for (const ch of q) {
      const at = s.indexOf(ch, si);
      if (at < 0) return null;
      if (first < 0) first = at;
      run = at === si && total > 0 ? run + 1 : 0;
      const start = at === 0 || /[\s\-_/·.:]/.test(s[at - 1]);
      total += 1 + run * 3 + (start ? 4 : 0);
      si = at + 1;
    }
    return total - first * 0.1 - s.length * 0.01;
  }
  // commands: [{id, label, keywords?}] (label already translated); recent: ids, newest first
  function rank(commands, query, recent) {
    recent = recent || [];
    const rec = (id) => { const i = recent.indexOf(id); return i < 0 ? Infinity : i; };
    if (!norm(query).trim()) {
      return commands.slice().sort((a, b) => rec(a.id) - rec(b.id) || 0).map((c) => ({ cmd: c, recent: rec(c.id) !== Infinity }));
    }
    const out = [];
    for (const c of commands) {
      const sc = Math.max(score(query, c.label) ?? -Infinity, (score(query, c.keywords || '') ?? -Infinity) - 2);
      if (sc === -Infinity) continue;
      out.push({ cmd: c, score: sc + (rec(c.id) !== Infinity ? 2 : 0), recent: rec(c.id) !== Infinity });
    }
    return out.sort((a, b) => b.score - a.score);
  }
  const pushRecent = (recent, id, max) => [id].concat((recent || []).filter((x) => x !== id)).slice(0, max || 8);

  // command and search fields: no macOS autocorrect ("настр" -> "настроение"), no capitals, no spelling marks (live run v4, 249)
  const QUIET = Object.freeze({ autocorrect: 'off', autocapitalize: 'off', spellcheck: 'false', autocomplete: 'off' });
  const quiet = (attrs) => Object.assign({}, attrs, QUIET);

  root.NSPalette = { score, rank, pushRecent, QUIET, quiet };
  if (typeof document === 'undefined' || !root.NS || !root.NS.h) return;

  // ---------- the window ----------
  const NS = root.NS;
  const { h, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const KEY = 'ns.palette.recent';
  let box = null;
  const readRecent = () => { try { return JSON.parse(NS.storage.get(KEY) || '[]'); } catch (e) { return []; } };
  const plat = () => (/Mac/i.test(navigator.platform || '') || /[?&]platform=mac/.test(location.search) ? 'mac' : 'linux');
  const keyText = (accel) => (accel ? window.NSLogic.keyLabel(accel, plat()).join('') : '');
  function commands(ctx) {
    return NS.registry.points('command').filter((c) => !c.when || c.when(ctx))
      .map((c) => ({ id: c.id, label: c.label || t(c.labelKey), keywords: [c.group ? t(c.group) : '', c.keywordsKey ? t(c.keywordsKey) : ''].join(' '), group: c.group, keys: typeof c.keys === 'function' ? c.keys() : c.keys, def: c }));
  }
  function close() { if (!box) return; const prev = box.prevFocus; box.remove(); box = null; if (prev && prev.isConnected && prev.focus) prev.focus(); }
  function open(query) {
    if (box) { close(); return; }
    const ctx = NS.shell.ctxFor({});
    const input = h('input', quiet({ type: 'text', class: 'pal-in', role: 'combobox', 'aria-expanded': 'true', 'aria-controls': 'palList', 'aria-label': t('pal.label'), placeholder: t('pal.ph') }));
    const list = h('ul', { class: 'pal-list', id: 'palList', role: 'listbox' });
    const panel = h('div', { class: 'pal', role: 'dialog', 'aria-modal': 'true', 'aria-label': t('pal.label') }, h('div', { class: 'pal-h' }, h('span', { class: 'pal-p', 'aria-hidden': 'true', text: '❯' }), input), list);
    box = h('div', { class: 'pal-back', onmousedown: (e) => { if (e.target === box) close(); } }, panel);
    box.prevFocus = document.activeElement;
    let rows = [], sel = 0;
    const all = commands(ctx);
    function run(r) {
      if (!r) return;
      NS.storage.set(KEY, JSON.stringify(pushRecent(readRecent(), r.cmd.id)));
      close();
      try { r.cmd.def.run(ctx); } catch (e) { NS.shell.toast(String(e && e.message || e), true); }
    }
    function draw() {
      rows = rank(all, input.value, readRecent());
      sel = Math.max(0, Math.min(sel, rows.length - 1));
      clear(list);
      if (!rows.length) list.append(h('li', { class: 'pal-none', text: t('pal.none') }));
      rows.forEach((r, i) => list.append(h('li', { role: 'option', id: 'pal-o' + i, class: 'pal-o' + (i === sel ? ' on' : ''), 'aria-selected': String(i === sel),
        onmousemove: () => { if (sel !== i) { sel = i; mark(); } }, onclick: () => run(r) },
      h('span', { class: 'pal-t', text: r.cmd.label }), r.recent && !input.value.trim() ? h('small', { class: 'pal-r', text: t('pal.recent') }) : null,
      r.cmd.keys ? h('kbd', { text: keyText(r.cmd.keys) }) : null)));
      mark();
    }
    function mark() {
      [...list.children].forEach((li, i) => { li.classList.toggle('on', i === sel); li.setAttribute('aria-selected', String(i === sel)); });
      input.setAttribute('aria-activedescendant', rows.length ? 'pal-o' + sel : '');
      const on = list.children[sel];
      if (on && on.scrollIntoView) on.scrollIntoView({ block: 'nearest' });
    }
    input.addEventListener('input', () => { sel = 0; draw(); });
    input.addEventListener('keydown', (e) => {
      if (e.key === 'ArrowDown') { e.preventDefault(); sel = (sel + 1) % Math.max(1, rows.length); mark(); }
      else if (e.key === 'ArrowUp') { e.preventDefault(); sel = (sel - 1 + rows.length) % Math.max(1, rows.length); mark(); }
      else if (e.key === 'Enter') { e.preventDefault(); run(rows[sel]); }
      else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); close(); }
    });
    document.body.append(box);
    if (query) input.value = query;
    draw();
    input.focus();
  }
  NS.palette = { open, close, get isOpen() { return !!box; } };
})(typeof window !== 'undefined' ? window : globalThis);
