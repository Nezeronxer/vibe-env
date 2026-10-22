// Pure helpers of v3 (stories 225-242): markdown and highlight tokens, unified diff rows, folding, gluing reads,
// the live line (verb, plan window, typing), tabs state, tab keys and the busy check, queue line, first-run env pick, model words.
// No DOM: runs in selftest (node vm). md.js, hl.js, actions.js and tabs.js turn these results into nodes.
(function (root) {
  const norm = (s) => String(s == null ? '' : s).toLowerCase().replace(/[_\s-]/g, '');

  // ---------- markdown (story 228) ----------
  const safeHref = (url) => { const u = String(url == null ? '' : url); return /^https?:\/\/[^\s]+$/i.test(u) ? u : null; };

  // inline: `code`, **bold** / __bold__, *italic*, [text](url), bare http(s) addresses. Unknown marks stay text.
  const INLINE = /(`+)([^`]|[^`][\s\S]*?[^`])\1(?!`)|\*\*(?=\S)([\s\S]*?\S)\*\*|__(?=\S)([\s\S]*?\S)__|\*(?=[^\s*])([^*\n]*?[^\s*]|[^\s*])\*(?!\*)|\[([^\]\n]+)\]\(([^)\s]+)\)|(https?:\/\/[^\s<>()]*[^\s<>().,;:!?'"*])/g;
  function parseInline(text) {
    const s = String(text == null ? '' : text), out = [];
    const push = (node) => { const last = out[out.length - 1]; if (node.t === 'text' && last && last.t === 'text') last.s += node.s; else if (node.t !== 'text' || node.s) out.push(node); };
    const re = new RegExp(INLINE.source, 'g');      // own copy: the nested calls below must not move the outer position
    let at = 0, m;
    while ((m = re.exec(s))) {
      push({ t: 'text', s: s.slice(at, m.index) });
      if (m[1]) push({ t: 'code', s: m[2] });
      else if (m[3] != null) push({ t: 'b', c: parseInline(m[3]) });
      else if (m[4] != null) push({ t: 'b', c: parseInline(m[4]) });
      else if (m[5] != null) push({ t: 'i', c: parseInline(m[5]) });
      else if (m[6] != null) { const href = safeHref(m[7]); if (href) push({ t: 'a', href, c: parseInline(m[6]) }); else parseInline(m[6]).forEach(push); }
      else if (m[8]) push({ t: 'a', href: m[8], c: [{ t: 'text', s: m[8] }] });
      at = re.lastIndex;
    }
    push({ t: 'text', s: s.slice(at) });
    return out;
  }
  function withBreaks(lines) {
    const out = [];
    lines.forEach((l, i) => { if (i) out.push({ t: 'br' }); out.push(...parseInline(l)); });
    return out;
  }
  const UL = /^\s*[-*+]\s+(.*)$/, OL = /^\s*(\d{1,9})[.)]\s+(.*)$/, H = /^(#{1,6})\s+(.*?)\s*#*\s*$/, FENCE = /^\s*(```+|~~~+)\s*([\w+#.-]*)\s*$/;
  function parseMarkdown(text) {
    const lines = String(text == null ? '' : text).replace(/\r\n?/g, '\n').split('\n');
    const blocks = [];
    let para = null, list = null;
    const flush = () => { if (para) blocks.push({ type: 'p', inline: withBreaks(para) }); para = null; if (list) blocks.push(list.block); list = null; };
    for (let i = 0; i < lines.length; i++) {
      const l = lines[i];
      const f = FENCE.exec(l);
      if (f) {
        flush();
        const body = [];
        let j = i + 1;
        for (; j < lines.length; j++) { if (new RegExp('^\\s*' + f[1][0] + '{' + f[1].length + ',}\\s*$').test(lines[j])) break; body.push(lines[j]); }
        blocks.push({ type: 'code', lang: f[2].toLowerCase(), text: body.join('\n') });
        i = j;
        continue;
      }
      if (!l.trim()) { flush(); continue; }
      const hd = H.exec(l);
      if (hd) { flush(); blocks.push({ type: 'h', level: Math.min(3, hd[1].length), inline: parseInline(hd[2]) }); continue; }
      if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(l)) { flush(); blocks.push({ type: 'hr' }); continue; }
      if (/^\s*\|/.test(l)) {
        flush();
        const rows = [l];
        while (i + 1 < lines.length && /^\s*\|/.test(lines[i + 1])) rows.push(lines[++i]);
        blocks.push({ type: 'table', text: rows.join('\n') });
        continue;
      }
      if (/^\s*>\s?/.test(l)) { flush(); blocks.push({ type: 'quote', inline: parseInline(l.replace(/^\s*>\s?/, '')) }); continue; }
      const u = UL.exec(l), o = OL.exec(l);
      if (u || o) {
        const kind = u ? 'ul' : 'ol';
        if (para) { blocks.push({ type: 'p', inline: withBreaks(para) }); para = null; }
        if (!list || list.kind !== kind) { if (list) blocks.push(list.block); list = { kind, block: kind === 'ul' ? { type: 'ul', items: [] } : { type: 'ol', start: Number(o[1]), items: [] }, raw: [] }; }
        list.raw.push(u ? u[1] : o[2]);
        list.block.items.push(parseInline(u ? u[1] : o[2]));
        continue;
      }
      if (list && /^\s{2,}\S/.test(l)) {          // continuation of a list item
        const k = list.block.items.length - 1;
        list.raw[k] += ' ' + l.trim();
        list.block.items[k] = parseInline(list.raw[k]);
        continue;
      }
      if (list) { blocks.push(list.block); list = null; }
      (para = para || []).push(l);
    }
    flush();
    return blocks;
  }

  // ---------- highlight (story 226): own light tokens per language, the rest is plain monospace ----------
  const EXT = { rs: 'rs', py: 'py', js: 'js', mjs: 'js', cjs: 'js', jsx: 'js', ts: 'js', tsx: 'js', json: 'json', toml: 'toml', md: 'md', markdown: 'md',
    sh: 'sh', bash: 'sh', zsh: 'sh', css: 'css', html: 'html', htm: 'html', svelte: 'html', vue: 'html' };
  const LANG_ALIAS = { rust: 'rs', python: 'py', javascript: 'js', typescript: 'js', shell: 'sh', console: 'sh', yaml: 'toml', yml: 'toml', ini: 'toml' };
  function langOf(path) {
    const m = /\.([A-Za-z0-9]+)$/.exec(String(path || '').split('/').pop());
    return m ? EXT[m[1].toLowerCase()] || '' : '';
  }
  const fenceLang = (lang) => { const l = String(lang || '').toLowerCase(); return EXT[l] || LANG_ALIAS[l] || ''; };
  const kw = (words) => new RegExp('\\b(?:' + words.split(' ').join('|') + ')\\b', 'y');
  const STR2 = /"(?:[^"\\\n]|\\.)*"?|'(?:[^'\\\n]|\\.)*'?/y, NUM = /\b(?:0x[\da-fA-F_]+|\d[\d_]*(?:\.\d+)?(?:[eE][+-]?\d+)?)\b/y;
  const RULES = {
    js: [['com', /\/\/[^\n]*|\/\*[\s\S]*?(?:\*\/|$)/y], ['str', /"(?:[^"\\\n]|\\.)*"?|'(?:[^'\\\n]|\\.)*'?|`(?:[^`\\]|\\.)*`?/y], ['num', NUM],
      ['kw', kw('const let var function return if else for while do switch case break continue new class extends import from export default async await try catch finally throw typeof instanceof in of this null undefined true false interface type enum')]],
    rs: [['com', /\/\/[^\n]*|\/\*[\s\S]*?(?:\*\/|$)/y], ['str', /b?"(?:[^"\\]|\\.)*"?/y], ['num', NUM],
      ['kw', kw('fn let mut pub impl struct enum match if else for while loop return use mod crate self Self super trait where as in ref move async await const static type true false Some None Ok Err dyn unsafe')]],
    py: [['com', /#[^\n]*/y], ['str', /"""[\s\S]*?(?:"""|$)|'''[\s\S]*?(?:'''|$)|[rbfu]{0,2}"(?:[^"\\\n]|\\.)*"?|[rbfu]{0,2}'(?:[^'\\\n]|\\.)*'?/y], ['num', NUM],
      ['kw', kw('def class return if elif else for while in import from as with try except finally raise None True False lambda yield pass break continue and or not is async await global nonlocal')]],
    json: [['str', STR2], ['num', /-?\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b/y], ['kw', kw('true false null')]],
    toml: [['com', /#[^\n]*/y], ['kw', /^\s*\[[^\]\n]*\]/my], ['str', STR2], ['num', NUM], ['kw', kw('true false')]],
    md: [['kw', /^#{1,6}[^\n]*/my], ['str', /`[^`\n]*`/y]],
    sh: [['com', /#[^\n]*/y], ['str', STR2], ['kw', kw('if then else elif fi for do done case esac function export local return in while until echo cd set unset source')]],
    css: [['com', /\/\*[\s\S]*?(?:\*\/|$)/y], ['str', STR2], ['kw', /@[\w-]+/y], ['num', /#[\da-fA-F]{3,8}\b|\b\d+(?:\.\d+)?(?:px|em|rem|%|vh|vw|s|ms|deg)?\b/y]],
    html: [['com', /<!--[\s\S]*?(?:-->|$)/y], ['kw', /<\/?[A-Za-z][\w-]*|\/?>/y], ['str', STR2]],
  };
  // -> [{k: ''|'kw'|'str'|'com'|'num', s}], neighbours of one kind merged
  function tokenize(code, lang) {
    const s = String(code == null ? '' : code), rules = RULES[lang] || null, out = [];
    const push = (k, piece) => { const last = out[out.length - 1]; if (last && last.k === k) last.s += piece; else out.push({ k, s: piece }); };
    if (!rules) return s ? [{ k: '', s }] : [];
    let i = 0;
    while (i < s.length) {
      let hit = null;
      for (const [k, re] of rules) { re.lastIndex = i; const m = re.exec(s); if (m && m[0]) { hit = [k, m[0]]; break; } }
      if (hit) { push(hit[0], hit[1]); i += hit[1].length; }
      else {
        const w = /[A-Za-z0-9_$]+|[^A-Za-z0-9_$]/y; w.lastIndex = i;
        const piece = w.exec(s)[0];
        push('', piece); i += piece.length;
      }
    }
    return out;
  }

  // ---------- unified diff (story 226) ----------
  function parseDiff(text) {
    const rows = [];
    let o = 0, n = 0;
    for (const l of String(text == null ? '' : text).replace(/\r\n?/g, '\n').replace(/\n$/, '').split('\n')) {
      if (/^(---|\+\+\+) /.test(l) || /^(diff --git|index |new file mode|deleted file mode|similarity index|rename (from|to))/.test(l)) continue;
      const hk = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(l);
      if (hk) { o = Number(hk[1]); n = Number(hk[2]); rows.push({ kind: 'hunk', old: null, new: null, text: l }); continue; }
      if (l.startsWith('\\')) continue;                       // "\ No newline at end of file"
      if (l[0] === '+') rows.push({ kind: 'add', old: null, new: n++, text: l.slice(1) });
      else if (l[0] === '-') rows.push({ kind: 'del', old: o++, new: null, text: l.slice(1) });
      else if (l[0] === ' ' || (l === '' && rows.length)) rows.push({ kind: 'ctx', old: o++, new: n++, text: l.slice(1) });
    }
    return rows;
  }
  const diffStat = (rows) => ({ add: rows.filter((r) => r.kind === 'add').length, del: rows.filter((r) => r.kind === 'del').length });

  // ---------- folding (stories 225, 229): show `limit` lines, never more than `hard`; the rest is "ещё N" / cut ----------
  function foldLines(text, limit, hard) {
    const s = String(text == null ? '' : text).replace(/\r\n?/g, '\n').replace(/\n$/, '');
    if (!s) return { lines: [], hidden: 0, cut: 0, total: 0 };
    const all = s.split('\n'), cap = Math.min(all.length, hard || Infinity), shown = Math.min(limit, cap);
    return { lines: all.slice(0, shown), hidden: cap - shown, cut: all.length - cap, total: all.length };
  }

  // ---------- actions (ToolAction from the core, legacy ChatMsg.tool) ----------
  const KIND = { read: 'read', edit: 'edit', multiedit: 'edit', write: 'write', bash: 'bash', search: 'search', grep: 'search', glob: 'search', web: 'web', plan: 'plan', other: 'other' };
  // serde may tag the input inside ({kind:"Read", path}) or outside ({"Read": {path}}); both -> {kind:'read', ...fields}
  function inputOf(action) {
    const inp = (action && action.input) || {};
    if (typeof inp === 'string') return { kind: KIND[norm(inp)] || 'other' };
    if (inp.kind != null) { const kind = KIND[norm(inp.kind)] || 'other'; return Object.assign({ kind }, inp, { kind }); }
    const k = Object.keys(inp)[0];
    if (k == null) return { kind: 'other' };
    const kind = KIND[norm(k)] || 'other';
    return Object.assign({ kind }, inp[k] && typeof inp[k] === 'object' ? inp[k] : {}, { kind });
  }
  // ChatMsg.tool {name: read|edit|write|run|..., arg, result, ok} of v2 -> the same line as a ToolAction
  const LEGACY = { read: ['Read', 'read', 'path'], edit: ['Edit', 'edit', 'path'], write: ['Write', 'write', 'path'], run: ['Bash', 'bash', 'command'], bash: ['Bash', 'bash', 'command'],
    grep: ['Grep', 'search', 'pattern'], glob: ['Glob', 'search', 'pattern'], search: ['Grep', 'search', 'pattern'], web: ['WebFetch', 'web', 'target'], fetch: ['WebFetch', 'web', 'target'] };
  function toolToAction(tool, id) {
    const l = LEGACY[norm(tool.name)];
    const input = l ? { kind: l[1], [l[2]]: tool.arg || '' } : { kind: 'other', summary: [tool.name, tool.arg].filter(Boolean).join(' ') };
    return { id, name: l ? l[0] : String(tool.name || ''), input, output: tool.result == null ? null : String(tool.result), ok: tool.ok !== false, done: true };
  }
  // the tool by which Claude Code hands over a role's answer by schema: our decision, not work to show (an action, a ChatMsg.tool, a log line)
  const SERVICE_TOOLS = ['StructuredOutput'];
  const isServiceTool = (x) => !!x && SERVICE_TOOLS.includes(x.name != null ? String(x.name) : String(x.text || '').split(':')[0].trim());
  // one action -> one feed row (G30). `acts`: Map key -> {key, at, role, taskId, action}; `owner`: a task id or 'chat:<id>'.
  // The core sends an action twice (start, result) with one id, and mirrors a task's action into its chat (chat-action):
  // all copies land in one row, the task's copy wins the owner and the role; a late start never undoes a result.
  function upsertAction(acts, owner, action, at, role) {
    if (!action || action.id == null || isServiceTool(action)) return null;
    const id = String(action.id), isChat = String(owner).startsWith('chat:');
    let key = owner + ':' + id, prev = acts.get(key);
    if (!prev) {
      for (const [k, e] of acts) {
        if (String(e.action.id) !== id || (isChat ? !e.taskId : !!e.taskId)) continue;
        prev = e;
        if (isChat) key = k;                         // the mirror of a task's action: update the task's row
        else acts.delete(k);                         // the mirror came first: the row moves under the task
        break;
      }
    }
    const fromTask = !isChat;
    let merged = action;
    if (prev) {
      const late = prev.action.done && !action.done;
      merged = late ? Object.assign({}, action, prev.action) : Object.assign({}, prev.action, action, { input: action.input || prev.action.input });
    }
    acts.set(key, { key, at: prev ? prev.at : at, role: prev && (prev.taskId || !fromTask) ? prev.role : role, taskId: fromTask ? owner : prev ? prev.taskId : null, action: merged });
    return key;
  }
  const GLUE = ['read', 'search'];
  // consecutive Read/Grep/Glob of one role -> one row (2+); everything else stays its own line
  function groupActions(entries) {
    const out = [];
    let run = [];
    const close = () => { if (run.length > 1) out.push({ type: 'group', items: run }); else if (run.length) out.push({ type: 'one', item: run[0] }); run = []; };
    // a started action has ok=false until its result: only a finished failure breaks the run
    const gluable = (e) => GLUE.includes(inputOf(e.action).kind) && !(e.action.done && e.action.ok === false);
    for (const e of entries) {
      if (gluable(e) && (!run.length || run[0].role === e.role)) run.push(e);
      else { close(); if (gluable(e)) run.push(e); else out.push({ type: 'one', item: e }); }
    }
    close();
    return out;
  }
  function readSummary(items) {
    let files = 0, searches = 0;
    for (const e of items) { if (inputOf(e.action).kind === 'read') files++; else searches++; }
    return { files, searches };
  }

  // ---------- the live line (story 227) ----------
  // who: {role, lastRole} - role keys of the running step and of the last action; reading roles never "write code"
  const READERS = ['rev', 'crit', 'plan', 'gate', 'watch', 'coord'], WRITERS = ['exec', 'test', 'integ'];
  function actionVerb(last, streaming, who) {
    if (streaming) return 'live.answering';
    const role = who && who.role, reader = READERS.includes(role), idle = reader ? 'live.reviewing' : 'live.thinking';
    if (last && role && who.lastRole && who.lastRole !== role) last = null;       // an action of the step before
    if (!last) return idle;
    const k = inputOf(last).kind, by = (who && who.lastRole) || role;
    const writes = (k === 'edit' || k === 'write') && (!by || WRITERS.includes(by));
    if (!last.done) return writes ? 'live.writing' : { bash: 'live.running', read: 'live.reading', search: 'live.reading', web: 'live.web', plan: 'live.planning' }[k] || idle;
    return writes ? 'live.writing' : idle;
  }
  const planState = (s) => ({ pending: 'pending', active: 'active', inprogress: 'active', done: 'done', completed: 'done', dropped: 'dropped', cancelled: 'dropped' }[norm(s)] || 'pending');
  // more than 7 items: the current one ±2, the rest are counted above and below (story 242)
  function planView(items, max) {
    const all = (items || []).map((x) => ({ text: String(x.text || x.content || ''), state: planState(x.state || x.status) }));
    if (all.length <= (max || 7)) return { items: all, before: 0, after: 0 };
    let cur = all.findIndex((x) => x.state === 'active');
    if (cur < 0) cur = all.findIndex((x) => x.state === 'pending');
    if (cur < 0) cur = all.length - 1;
    const from = Math.max(0, Math.min(cur - 2, all.length - 5)), to = Math.min(all.length, from + 5);
    return { items: all.slice(from, to), before: from, after: all.length - to };
  }
  // a whole message printed word by word: 30-60 ms a word, never longer than 1.5 s (several words a tick when long)
  function typePlan(text, maxMs) {
    const s = String(text == null ? '' : text), budget = maxMs || 1500;
    const words = s.match(/\S+\s*|\s+/g) || [];
    if (!words.length) return { chunks: [], ms: 0 };
    const ms = Math.max(30, Math.min(60, Math.floor(budget / words.length)));
    const per = Math.max(1, Math.ceil(words.length / Math.floor(budget / ms)));
    const chunks = [];
    for (let i = 0; i < words.length; i += per) chunks.push(words.slice(i, i + per).join(''));
    return { chunks, ms };
  }

  // ---------- tabs (stories 232-235): {tabs:[{id, env, chat}], active, panels:[tab ids], prev} ----------
  const MAX_PANELS = 4;
  const tabsEmpty = () => ({ tabs: [], active: null, panels: [], prev: null });
  const has = (st, id) => st.tabs.some((x) => x.id === id);
  function tabsActivate(st, id) {
    if (!has(st, id) || st.active === id) return st;
    let panels;
    if (st.panels.includes(id)) panels = st.panels;
    else if (st.panels.length > 1 && st.panels.includes(st.active)) panels = st.panels.map((p) => (p === st.active ? id : p));
    else panels = [id];
    return Object.assign({}, st, { active: id, panels, prev: st.active });
  }
  function tabsOpen(st, tab) {
    const i = st.tabs.findIndex((x) => x.id === st.active);
    const tabs = st.tabs.slice();
    tabs.splice(i < 0 ? tabs.length : i + 1, 0, tab);
    return tabsActivate(Object.assign({}, st, { tabs }), tab.id);
  }
  function tabsClose(st, id) {
    const i = st.tabs.findIndex((x) => x.id === id);
    if (i < 0) return st;
    const tabs = st.tabs.filter((x) => x.id !== id);
    if (!tabs.length) return tabsEmpty();
    let panels = st.panels.filter((p) => p !== id), active = st.active;
    if (active === id) {
      const pi = st.panels.indexOf(id);
      active = panels.length ? panels[Math.max(0, pi - 1)] : (has({ tabs }, st.prev) ? st.prev : tabs[Math.min(i, tabs.length - 1)].id);
    }
    if (!panels.length) panels = [active];
    return { tabs, active, panels, prev: st.prev === id ? null : st.prev };
  }
  function tabsMove(st, id, to) {
    const tab = st.tabs.find((x) => x.id === id);
    if (!tab) return st;
    const tabs = st.tabs.filter((x) => x.id !== id);
    tabs.splice(Math.max(0, Math.min(to, tabs.length)), 0, tab);
    return Object.assign({}, st, { tabs });
  }
  // ⌘D on tab `id`: a hidden tab joins the split on the right; the shown one gets the previous tab (or `fresh`, a new tab) on its left
  function tabsSplit(st, id, fresh) {
    if (!has(st, id) || st.panels.length >= MAX_PANELS) return st;
    if (!st.panels.includes(id)) return Object.assign({}, st, { panels: st.panels.concat(id), active: id, prev: st.active });
    let tabs = st.tabs;
    let other = st.prev && has(st, st.prev) && !st.panels.includes(st.prev) ? st.prev : (st.tabs.find((x) => !st.panels.includes(x.id)) || {}).id;
    if (!other && fresh) { tabs = st.tabs.concat(fresh); other = fresh.id; }
    if (!other) return st;
    const panels = st.panels.slice();
    panels.splice(panels.indexOf(id), 0, other);
    return Object.assign({}, st, { tabs, panels, active: id });
  }
  function tabsUnsplit(st, id) {
    const pid = id || st.active, i = st.panels.indexOf(pid);
    if (st.panels.length < 2 || i < 0) return st;
    const panels = st.panels.filter((p) => p !== pid);
    const active = st.active === pid ? panels[Math.max(0, i - 1)] : st.active;
    return Object.assign({}, st, { panels, active, prev: pid });
  }
  function tabsFocusPanel(st, dir) {
    const i = st.panels.indexOf(st.active);
    if (st.panels.length < 2 || i < 0) return st;
    return Object.assign({}, st, { active: st.panels[(i + dir + st.panels.length) % st.panels.length] });
  }
  const tabsSetChat = (st, id, env, chat) => Object.assign({}, st, { tabs: st.tabs.map((x) => (x.id === id ? Object.assign({}, x, { env, chat }) : x)) });
  const tabsSerialize = (st) => JSON.stringify({ v: 1, tabs: st.tabs.map((x) => ({ id: x.id, env: x.env, chat: x.chat })), active: st.active, panels: st.panels });
  // `known(env, chat)` says whether the chat still exists; an empty "new" chat always comes back
  function tabsRestore(raw, known) {
    let o;
    try { o = typeof raw === 'string' ? JSON.parse(raw) : raw; } catch (e) { return null; }
    if (!o || !Array.isArray(o.tabs)) return null;
    const tabs = o.tabs.filter((x) => x && x.id && x.env && x.chat && (x.chat === 'new' || known(x.env, x.chat)));
    if (!tabs.length) return null;
    const ok = (id) => tabs.some((x) => x.id === id);
    let panels = (Array.isArray(o.panels) ? o.panels : []).filter(ok).slice(0, MAX_PANELS);
    let active = ok(o.active) ? o.active : panels[0] || tabs[0].id;
    if (!panels.includes(active)) panels = panels.length > 1 ? panels.concat(active).slice(-MAX_PANELS) : [active];
    if (!panels.length) panels = [active];
    return { tabs, active, panels, prev: null };
  }

  // ---------- tab keys (stories 232, 236) ----------
  const tabKeyDefaults = (platform) => (platform === 'mac'
    ? { tab_new: 'Cmd+T', tab_close: 'Cmd+W', tab_split: 'Cmd+D', tab_unsplit: 'Shift+Cmd+D' }
    : { tab_new: 'Ctrl+T', tab_close: 'Ctrl+W', tab_split: 'Ctrl+D', tab_unsplit: 'Ctrl+Shift+D' });
  // fixed, not editable: numbers, next/previous, panels
  const fixedTabKeys = (platform) => {
    const m = platform === 'mac' ? 'Cmd' : 'Ctrl', list = [];
    for (let n = 1; n <= 9; n++) list.push(m + '+' + n);
    if (platform === 'mac') list.push('Shift+Cmd+]', 'Shift+Cmd+[');
    list.push('Ctrl+Tab', 'Ctrl+Shift+Tab');
    return list;
  };
  const accelOf = (ev, platform) => (root.NSLogic && root.NSLogic.accelFromEvent ? root.NSLogic.accelFromEvent(ev, platform) : null);
  // the physical key when an engine leaves `code` empty: from keyCode, a Latin letter, or the Russian ЙЦУКЕН letter on that key
  const RU = 'йцукенгшщзфывапролдячсмить', US = 'qwertyuiopasdfghjklzxcvbnm';
  function physCode(ev) {
    if (ev.code) return ev.code;
    const kc = Number(ev.keyCode) || 0;
    if (kc >= 65 && kc <= 90) return 'Key' + String.fromCharCode(kc);
    if (kc >= 48 && kc <= 57) return 'Digit' + (kc - 48);
    const k = String(ev.key || '').toLowerCase(), i = k.length === 1 ? RU.indexOf(k) : -1;
    if (i >= 0) return 'Key' + US[i].toUpperCase();
    if (/^[a-z]$/.test(k)) return 'Key' + k.toUpperCase();
    if (/^\d$/.test(k)) return 'Digit' + k;
    return k === 'tab' ? 'Tab' : '';
  }
  function tabKeyAction(ev0, platform, keys) {
    const ev = ev0.code ? ev0 : { code: physCode(ev0), key: ev0.key, metaKey: ev0.metaKey, ctrlKey: ev0.ctrlKey, altKey: ev0.altKey, shiftKey: ev0.shiftKey };
    const mac = platform === 'mac', mod = mac ? ev.metaKey : ev.ctrlKey, code = ev.code || '';
    const k = Object.assign(tabKeyDefaults(platform), keys || {});
    const accel = accelOf(ev, platform);
    if (accel) {
      if (accel === k.tab_new) return { type: 'new' };
      if (accel === k.tab_close) return { type: 'close' };
      if (accel === k.tab_split) return { type: 'split' };
      if (accel === k.tab_unsplit) return { type: 'unsplit' };
    }
    if (code === 'Tab' && ev.ctrlKey && !ev.altKey && !ev.metaKey) return { type: ev.shiftKey ? 'prev' : 'next' };
    if (mod && mac && ev.shiftKey && !ev.altKey && (code === 'BracketRight' || code === 'BracketLeft')) return { type: code === 'BracketRight' ? 'next' : 'prev' };
    if (mod && ev.altKey && !ev.shiftKey && (code === 'ArrowLeft' || code === 'ArrowRight')) return { type: 'panel', dir: code === 'ArrowLeft' ? -1 : 1 };
    const d = /^Digit([1-9])$/.exec(code);
    if (d && mod && !ev.altKey && !ev.shiftKey && !(mac && ev.ctrlKey)) return d[1] === '9' ? { type: 'last' } : { type: 'goto', n: Number(d[1]) };
    return null;
  }
  const migrateSwitchEnv = (accel, platform) => {
    if (accel === 'Cmd+{n}' || (platform === 'mac' && !accel)) return 'Alt+Cmd+{n}';
    if (accel === 'Ctrl+{n}' || !accel) return 'Ctrl+Alt+{n}';
    return accel;
  };
  // pairs of rows that share a combination; a "{n}" pattern means 1…7; the fixed tab numbers count as row "tab_digits"
  function keyConflicts(keys, platform) {
    const owners = [];
    for (const [name, accel] of Object.entries(keys || {})) {
      if (!accel) continue;
      const list = /\{n\}/.test(accel) ? [1, 2, 3, 4, 5, 6, 7].map((n) => accel.replace('{n}', n)) : [accel];
      owners.push([name, list.map((x) => x.toLowerCase())]);
    }
    owners.push(['tab_digits', fixedTabKeys(platform).map((x) => x.toLowerCase())]);
    const out = [];
    for (let i = 0; i < owners.length; i++) for (let j = i + 1; j < owners.length; j++) {
      if (owners[i][1].some((x) => owners[j][1].includes(x))) out.push([owners[i][0], owners[j][0]]);
    }
    return out;
  }

  // ---------- queue line (story 234) ----------
  const LIVE_RUN = ['preparing', 'working', 'checking', 'reviewing', 'merging'];
  function queueInfo(tasks, taskId, maxParallel) {
    const me = tasks.find((x) => x.id === taskId);
    if (!me || me.status !== 'queued' || (me.start && norm(typeof me.start === 'string' ? me.start : Object.keys(me.start)[0]) === 'night')) return null;
    const mine = tasks.filter((x) => x.agent === me.agent);
    const running = mine.filter((x) => LIVE_RUN.includes(x.status));
    if (running.length < (maxParallel || 3)) return null;
    const queue = mine.filter((x) => x.status === 'queued').sort((a, b) => Date.parse(a.created_at) - Date.parse(b.created_at));
    const first = running.slice().sort((a, b) => Date.parse(a.started_at || a.created_at) - Date.parse(b.started_at || b.created_at))[0];
    return { pos: queue.findIndex((x) => x.id === taskId) + 1, after: first ? first.title : null };
  }

  // ---------- first run (stories 237-238) ----------
  const ENV_ORDER = ['claude', 'codex', 'opencode', 'gemini', 'qwen', 'cursor'];
  function pickEnv(envs) {
    const byOrder = ENV_ORDER.map((id) => (envs || []).find((e) => e.id === id)).filter(Boolean);
    const ok = byOrder.find((e) => e.state === 'ok');
    if (ok) return { why: 'ok', env: ok };
    const login = byOrder.find((e) => e.state === 'login' || e.state === 'unavailable');
    if (login) return { why: 'login', env: login };
    return { why: 'none', env: null };
  }

  // ---------- model words (story 240): the core sends the key model.desc.<id>; the source note goes to the tooltip ----------
  function modelDesc(fam, t, has) {
    const blurb = String((fam && fam.blurb) || '');
    let text = '', note = (fam && fam.source_note) || '';
    if (/^model\.desc\./.test(blurb) && has(blurb)) text = t(blurb);
    else if (fam && has('model.desc.' + fam.id)) text = t('model.desc.' + fam.id);
    else {
      const m = /^(.*?)\s*\(([^()]*(?:docs|сверено|verified)[^()]*)\)\s*$/i.exec(blurb);
      text = m ? m[1] : /^model\.desc\./.test(blurb) ? '' : blurb;
      if (m && !note) note = m[2];
    }
    return { text, note };
  }

  const api = { safeHref, parseInline, parseMarkdown, langOf, fenceLang, tokenize, parseDiff, diffStat, foldLines, inputOf, toolToAction, isServiceTool, upsertAction, groupActions, readSummary,
    actionVerb, planView, planState, typePlan, MAX_PANELS, tabsEmpty, tabsOpen, tabsActivate, tabsClose, tabsMove, tabsSplit, tabsUnsplit, tabsFocusPanel, tabsSetChat,
    tabsSerialize, tabsRestore, tabKeyDefaults, fixedTabKeys, tabKeyAction, migrateSwitchEnv, keyConflicts, queueInfo, ENV_ORDER, pickEnv, modelDesc };
  if (typeof module !== 'undefined') module.exports = api;
  root.NSLive = api;
})(typeof window !== 'undefined' ? window : globalThis);
