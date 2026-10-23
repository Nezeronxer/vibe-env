// Action lines of the feed, like Claude Code in a terminal (stories 225-229, 242):
//   ● Read src/x.py          ● Edit src/x.py  +12 −3       ● Bash pytest -q
//   └ 212 строк               └ diff with numbers, colour    └ first 8 lines, "ещё N строк"
// NS.actions.actionNode(action, opts) -> Node, groupNode(entries, opts) -> Node ("Прочитал N файлов"), liveBar() -> controller.
// Folding, diff rows, gluing and verbs are pure (NSLive, selftested). Monospace only for code, output and diff.
(function () {
  const NS = window.NS;
  const V = window.NSLive;
  const L = window.NSLogic;
  const { h, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const OUT_LINES = 8, OUT_MAX = 2000, DIFF_LINES = 40, WRITE_LINES = 20;
  const pl = (n, base) => L.plural(n, [t(base + '_1'), t(base + '_2'), t(base + '_5')], NS.lang());

  // the argument shown after the tool name
  function argOf(inp, action) {
    switch (inp.kind) {
      case 'read': case 'edit': case 'write': return inp.path || '';
      case 'bash': return inp.command || '';
      case 'search': return inp.pattern || '';
      case 'web': return inp.target || '';
      default: return inp.summary || '';
    }
  }
  const nameOf = (action, inp) => {
    const n = String(action.name || '');
    const mcp = /^mcp__([^_]+(?:_[^_]+)*)__(.+)$/.exec(n);       // MCP: "server · tool"
    if (mcp) return mcp[1] + ' · ' + mcp[2];
    return n || { read: 'Read', edit: 'Edit', write: 'Write', bash: 'Bash', search: 'Grep', web: 'WebFetch' }[inp.kind] || 'Tool';
  };

  // "ещё N строк" / "показать всё" toggle that keeps its state in opts.open (the feed redraws nodes, the choice stays)
  function toggler(key, opts, label, onLabel, redraw) {
    const open = !!(opts.open && opts.open.has(key));
    return h('button', { type: 'button', class: 'act-more', 'aria-expanded': String(open), text: open ? onLabel : label,
      onclick: () => { if (opts.open) { if (opts.open.has(key)) opts.open.delete(key); else opts.open.add(key); } redraw(); } });
  }

  const CUT = /\n?\[(output|diff)_truncated: (\d+) bytes\]\s*$/;
  const cutOf = (text) => { const m = CUT.exec(String(text || '')); return m ? { text: String(text).slice(0, m.index), cut: true } : { text: String(text || ''), cut: false }; };
  function outBlock(raw, key, opts, bad) {
    const c = cutOf(raw), text = c.text;
    const box = h('div', { class: 'act-out' + (bad ? ' bad' : '') });
    const draw = () => {
      const open = !!(opts.open && opts.open.has(key));
      const f = V.foldLines(text, open ? OUT_MAX : OUT_LINES, OUT_MAX);
      clear(box).append(h('pre', { class: 'act-pre' }, h('code', { text: f.lines.join('\n') })));
      if (f.hidden > 0 || (open && f.total > OUT_LINES)) box.append(toggler(key, opts, t('act3.more_lines', { n: f.hidden, w: pl(f.hidden, 'act3.line') }), t('act3.collapse'), draw));
      if (c.cut && !(open && f.cut > 0)) box.append(h('div', { class: 'act-cut' }, h('span', { text: t('act3.cut') })));
      if (open && f.cut > 0) box.append(h('div', { class: 'act-cut' }, h('span', { text: t('act3.cut') }),
        h('button', { type: 'button', class: 'linkb', text: t('act3.open_log'), onclick: () => showLog(text) })));
    };
    draw();
    return box;
  }
  function showLog(text) {
    const dlg = document.getElementById('dlg');
    dlg.replaceChildren(h('h2', { text: t('act3.log_title') }), h('pre', { class: 'act-log' }, h('code', { text: text })),
      h('div', { class: 'dlg-acts' }, h('button', { type: 'button', class: 'btn', text: t('act3.close'), onclick: () => { dlg.close(); dlg.replaceChildren(); } })));
    dlg.classList.add('wide');
    dlg.onclose = () => dlg.classList.remove('wide');
    dlg.showModal();
  }

  // diff rows: one number column (new line for + and context, old for -), sign, highlighted code
  function diffRows(rows, lang) {
    return rows.map((r) => {
      if (r.kind === 'hunk') return h('div', { class: 'dl hk' }, h('i'), h('b'), h('span', { text: r.text.replace(/^@@[^@]*@@\s?/, '') || '⋯' }));
      const no = r.kind === 'del' ? r.old : r.new;
      const code = NS.highlight(r.text, lang);
      return h('div', { class: 'dl ' + (r.kind === 'add' ? 'a' : r.kind === 'del' ? 'd' : 'n') }, h('i', { text: no == null ? '' : String(no) }),
        h('b', { text: r.kind === 'add' ? '+' : r.kind === 'del' ? '-' : ' ' }), code);
    });
  }
  function diffBlock(diff, path, key, opts) {
    const c = cutOf(diff);
    const rows = V.parseDiff(c.text), lang = V.langOf(path);
    const box = h('div', { class: 'act-diff' });
    const draw = () => {
      const open = !!(opts.open && opts.open.has(key));
      const shown = open ? rows : rows.slice(0, DIFF_LINES);
      clear(box).append(h('div', { class: 'dg', role: 'region', 'aria-label': path }, diffRows(shown, lang)));
      if (c.cut) box.append(h('div', { class: 'act-note', text: t('act3.cut') }));
      if (rows.length > DIFF_LINES) box.append(toggler(key, opts, t('act3.show_all') + ' · ' + t('act3.more_lines', { n: rows.length - DIFF_LINES, w: pl(rows.length - DIFF_LINES, 'act3.line') }), t('act3.collapse'), draw));
    };
    draw();
    return { el: box, stat: V.diffStat(rows) };
  }
  function writeBlock(inp) {
    const head = String(inp.head || ''), lines = inp.lines != null ? Number(inp.lines) : head.split('\n').length;
    const f = V.foldLines(head, WRITE_LINES, WRITE_LINES), lang = V.langOf(inp.path);
    const box = h('div', { class: 'act-diff' }, h('div', { class: 'dg' }, f.lines.map((l, i) => h('div', { class: 'dl a' }, h('i', { text: String(i + 1) }), h('b', { text: '+' }), NS.highlight(l, lang)))));
    const rest = Math.max(0, lines - f.lines.length);
    if (rest > 0) box.append(h('div', { class: 'act-note', text: t('act3.more_lines', { n: rest, w: pl(rest, 'act3.line') }) }));
    return box;
  }

  // opts: { open: Set of expanded keys, live: the task/chat is still running (an unfinished action spins) }
  function actionNode(action, opts) {
    opts = opts || {};
    const inp = V.inputOf(action), key = String(action.id || '');
    const running = !action.done && opts.live !== false;
    const bad = action.done && action.ok === false;
    const st = running ? 'run' : bad ? 'bad' : 'ok';
    // v4 (story 244): a command is a `$ command` block, a read or a search one `· Read path` line, an edit `± Edit path` with its diff
    const mark = { bash: '$', read: '·', search: '·', web: '·', edit: '±', write: '±' }[inp.kind] || '●';
    const head = h('div', { class: 'act-h' }, h('span', { class: 'dot', 'aria-hidden': 'true', text: mark }),
      inp.kind === 'bash' ? h('span', { class: 'sr-only', text: nameOf(action, inp) }) : h('b', { class: 'act-n', text: nameOf(action, inp) }),
      h('span', { class: 'act-arg', text: argOf(inp, action) }));
    const el = h('div', { class: 'act k-' + inp.kind + ' s-' + st, dataset: { act: key } }, head);
    el.append(h('span', { class: 'sr-only', text: running ? t('act3.running') : bad ? t('act3.error') : '' }));
    const out = action.output == null ? '' : String(action.output);
    const body = [];
    if (bad) {
      body.push(h('div', { class: 'act-l bad' }, h('span', { class: 'cn', text: '└' }), h('span', { text: t('act3.error') + (out ? ': ' + out.split('\n')[0].slice(0, 300) : '') })));
      if (out.includes('\n')) body.push(outBlock(out.split('\n').slice(1).join('\n'), key + ':out', opts, true));
    } else if (inp.kind === 'edit') {
      if (inp.diff) { const d = diffBlock(inp.diff, inp.path, key + ':diff', opts); head.append(h('span', { class: 'act-stat' }, h('span', { class: 'plus', text: '+' + d.stat.add }), ' ', h('span', { class: 'minus', text: '−' + d.stat.del }))); body.push(d.el); }
      else if (action.done && out.trim()) body.push(h('div', { class: 'act-l' }, h('span', { class: 'cn', text: '└' }), h('span', { text: out.split('\n')[0] })));
    } else if (inp.kind === 'write') {
      const n = inp.lines != null ? Number(inp.lines) : String(inp.head || '').split('\n').length;
      head.append(h('span', { class: 'act-stat' }, h('span', { class: 'plus', text: '+' + n })));
      if (inp.head) body.push(writeBlock(inp));
    } else if (inp.kind === 'read') {
      const n = out ? out.replace(/\n$/, '').split('\n').length : 0;
      if (action.done && n) body.push(h('div', { class: 'act-l' }, h('span', { class: 'cn', text: '└' }), h('span', { text: n + ' ' + pl(n, 'act3.line') })));
    } else if (inp.kind === 'search') {
      if (action.done) { const n = inp.hits != null ? Number(inp.hits) : out ? out.replace(/\n$/, '').split('\n').length : 0; body.push(h('div', { class: 'act-l' }, h('span', { class: 'cn', text: '└' }), h('span', { text: t('act3.hits', { n, w: pl(n, 'act3.hit') }) }))); }
    } else if (inp.kind === 'bash' || inp.kind === 'web' || inp.kind === 'other') {
      if (running && inp.kind === 'bash') body.push(h('div', { class: 'act-l mute' }, h('span', { class: 'cn', text: '└' }), h('span', { text: t('act3.running') })));
      // "(нет вывода)" means something only for a command; a fetch or another tool without output just has no line
      else if (action.done && out) body.push(outBlock(out, key + ':out', opts, false));
      else if (action.done && inp.kind === 'bash') body.push(h('div', { class: 'act-l mute' }, h('span', { class: 'cn', text: '└' }), h('span', { text: t('act3.no_output') })));
    }
    if (body.length) el.append(h('div', { class: 'act-b' }, body));
    return el;
  }

  // consecutive reads/searches of one role: "● Прочитал 6 файлов" that opens into the lines
  function groupNode(entries, opts) {
    opts = opts || {};
    const s = V.readSummary(entries), key = 'g:' + entries[0].key;
    const parts = [];
    if (s.files) parts.push(t('act3.read_n', { n: s.files, w: pl(s.files, 'act3.file') }));
    if (s.searches) parts.push((parts.length ? '' : '') + t('act3.search_n', { n: s.searches, w: pl(s.searches, 'act3.times') }));
    const running = entries.some((e) => !e.action.done) && opts.live !== false;
    const text = parts.join(', ');
    const d = h('details', { class: 'act act-g s-' + (running ? 'run' : 'ok'), open: !!(opts.open && opts.open.has(key)),
      ontoggle: () => { if (!opts.open) return; if (d.open) opts.open.add(key); else opts.open.delete(key); } },
    h('summary', { class: 'act-h' }, h('span', { class: 'dot', 'aria-hidden': 'true', text: '·' }), h('b', { class: 'act-n', text: text.charAt(0).toUpperCase() + text.slice(1) }),
      h('span', { class: 'act-chev', 'aria-hidden': 'true', text: '›' })),
    h('div', { class: 'act-b' }, entries.map((e) => { const inp = V.inputOf(e.action); return h('div', { class: 'act-l' }, h('span', { class: 'cn', text: '└' }), h('span', { class: 'act-tn', text: nameOf(e.action, inp) }), h('span', { class: 'act-arg', text: argOf(inp, e.action) })); })));
    return d;
  }

  // the live line at the bottom of a feed: spinner, verb, timer of the step, role and model; the plan of the agent under it
  function liveBar() {
    const verb = h('b', { class: 'lv-v' }), meta = h('span', { class: 'lv-m' }), plan = h('div', { class: 'lv-plan', hidden: true });
    const line = h('div', { class: 'lv-line' }, h('span', { class: 'lv-spin', 'aria-hidden': 'true', text: '✻' }), verb, meta);
    const el = h('div', { class: 'livebar', role: 'status', 'aria-label': t('live.label'), hidden: true }, line, plan);
    let since = 0, who = '', planSig = '';
    function tickText() { meta.textContent = '(' + [since ? L.formatElapsed(Date.now() - since) : null, who].filter(Boolean).join(' · ') + ')'; }
    const timer = setInterval(() => { if (!el.hidden && since) tickText(); }, 1000);
    function drawPlan(items) {
      const sig = JSON.stringify(items || null);
      if (sig === planSig) return;
      planSig = sig;
      if (!items || !items.length) { plan.hidden = true; clear(plan); return; }
      const v = V.planView(items);
      const mark = { pending: '☐', active: '◐', done: '☑', dropped: '☐' };
      NS.append(clear(plan), [h('div', { class: 'lv-ph', text: t('live.plan') }),
        v.before ? h('div', { class: 'pl more', text: t('live.plan_more', { n: v.before }) }) : null,
        h('ul', {}, v.items.map((x) => h('li', { class: 'pl ' + x.state }, h('span', { class: 'pm', 'aria-hidden': 'true', text: mark[x.state] }), x.state === 'dropped' ? h('s', { text: x.text }) : h('span', { text: x.text })))),
        v.after ? h('div', { class: 'pl more', text: t('live.plan_more', { n: v.after }) }) : null]);
      plan.hidden = false;
    }
    return {
      el,
      // state: null (hidden) | {verbKey, since, who, plan, queue}
      set(state) {
        if (!state) { el.hidden = true; return; }
        el.hidden = false;
        el.classList.toggle('queued', !!state.queue);
        if (state.queue) { verb.textContent = state.queue; meta.textContent = ''; since = 0; who = ''; drawPlan(null); return; }
        verb.textContent = t(state.verbKey);
        since = state.since || 0; who = state.who || '';
        tickText();
        drawPlan(state.plan);
      },
      destroy() { clearInterval(timer); },
    };
  }

  NS.actions = { actionNode, groupNode, liveBar, argOf };
})();
