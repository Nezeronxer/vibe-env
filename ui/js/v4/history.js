// v4 history of a project (task 24, stories 281-284): the commit graph from CommitRow (lane, edges), branch and tag marks, search,
// 200 commits per portion, the files and the diff of a commit, "Откатить". The layout is pure (NSHistory.layout) and runs in selftest.
(function (root) {
  const LANES = 8;                       // lane colours repeat after this many (gc0 … gc7 in v4-market.css)
  // CommitRow[] -> geometry for one SVG per row. `edges` of a row run from its middle (lane `a`) to the middle of the next row (lane `b`);
  // a line is drawn in the colour of the branch it belongs to: the node's own line takes the lane it goes to, a passing or merging one keeps its own.
  function layout(rows, laneW, rowH) {
    const W = laneW || 14, H = rowH || 34, cy = H / 2;
    const x = (lane) => lane * W + W / 2;
    let maxLane = 0;
    const out = rows.map((r) => {
      maxLane = Math.max(maxLane, r.lane, ...r.edges.map((e) => Math.max(e[0], e[1])));
      const paths = r.edges.map(([a, b]) => ({
        c: (a === r.lane ? b : a) % LANES,
        d: a === b ? `M${x(a)},${cy} L${x(b)},${cy + H}` : `M${x(a)},${cy} C${x(a)},${cy + H * 0.6} ${x(b)},${cy + H * 0.4} ${x(b)},${cy + H}`,
      }));
      return { lane: r.lane, c: r.lane % LANES, cx: x(r.lane), cy, merge: r.parents.length > 1, paths };
    });
    return { width: (maxLane + 1) * W, rowH: H, rows: out };
  }
  // seconds since the epoch -> a key of the dictionary and its number
  function relTime(sec, nowMs) {
    const d = Math.max(0, Math.floor((nowMs - sec * 1000) / 1000));
    if (d < 60) return { key: 'hist.now' };
    if (d < 3600) return { key: 'hist.min', n: Math.floor(d / 60) };
    if (d < 86400) return { key: 'hist.hour', n: Math.floor(d / 3600) };
    if (d < 86400 * 14) return { key: 'hist.day', n: Math.floor(d / 86400) };
    return { key: 'hist.date', date: new Date(sec * 1000).toISOString().slice(0, 10) };
  }
  // search over the loaded commits: message, author, hash, branch and tag names (the files of a commit are asked per commit, not here)
  function filterRows(rows, q) {
    const s = String(q || '').trim().toLowerCase();
    if (!s) return rows;
    return rows.filter((r) => [r.subject, r.author, r.sha, r.refs.join(' '), r.task || ''].join('\n').toLowerCase().includes(s));
  }
  const refKind = (ref) => (/^tag: /.test(ref) ? 'tag' : /^[^/]+\/./.test(ref) && !/^(feature|fix|ns|hotfix|release)\//.test(ref) ? 'remote' : 'branch');
  const refLabel = (ref) => ref.replace(/^tag: /, '');
  // the task of a commit: the id of the trailer, or the 8-letter prefix taken from the branch
  const taskOf = (row, tasks) => (row.task ? [...tasks].find((t) => t.id === row.task || t.id.startsWith(row.task)) || null : null);
  // a task commit is undone like the task ("Откатить"), any other one with git revert after a confirmation (story 284)
  const revertPlan = (row, task) => (task ? { kind: 'undo', id: task.id } : { kind: 'revert', sha: row.sha });
  const isEmptyRepoError = (msg) => /does not have any commits|bad default revision|unknown revision|no commits|ambiguous argument 'HEAD'/i.test(String(msg || ''));
  root.NSHistory = { layout, relTime, filterRows, refKind, refLabel, taskOf, revertPlan, isEmptyRepoError, LANES, ROW_H: 34, PAGE: 200 };
  if (typeof document === 'undefined' || !root.NS || !root.NS.h) return;

  // ---------- the screen ----------
  const NS = root.NS;
  const { h, ic, envIcon, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const S = NS.store.s;
  const call = (c, a) => NS.api.call(c, a);
  const SVGNS = 'http://www.w3.org/2000/svg';
  const sv = (tag, attrs) => { const e = document.createElementNS(SVGNS, tag); for (const [k, v] of Object.entries(attrs || {})) e.setAttribute(k, v); return e; };
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
  const safeHttps = (u) => root.NSMarket.safeHttps(u);
  // the two commands for commits that do not belong to a task (core::git::commit_stat / commit_file_diff); the task-less call goes through them
  NS.api.COMMANDS.add('commit_stat'); NS.api.COMMANDS.add('commit_file_diff');
  const ago = (sec) => { const r = relTime(sec, Date.now()); return r.key === 'hist.date' ? t(r.key, { date: r.date }) : t(r.key, { n: r.n }); };

  function mount(root0, ctx) {
    const alive = { v: true };
    const st = { pid: ctx.params.project || S.lastProject || ((S.settings || {}).default_project) || (S.projects[0] || {}).id, rows: [], done: false, loading: false, q: '', sel: null, files: null, diff: null, hl: null, err: null, repo: null, seq: 0 };
    const project = () => NS.store.project(st.pid);

    const picker = h('select', { class: 'hs-proj', 'aria-label': t('hist.project'), onchange: (e) => { st.pid = e.target.value; S.lastProject = st.pid; reset(); } },
      S.projects.map((p) => h('option', { value: p.id, text: p.name, selected: p.id === st.pid })));
    const input = h('input', { type: 'text', class: 'mk-q', placeholder: t('hist.search_ph'), 'aria-label': t('hist.search'), spellcheck: false, autocomplete: 'off' });
    const status = h('div', { class: 'hs-repo' });
    const head = h('div', { class: 'mk-h' }, h('div', { class: 'mk-t' }, h('h1', { text: t('hist.title') })), picker, input, status);
    const spacer = h('div', { class: 'mk-vs' });
    const scroller = h('div', { class: 'mk-vl hs-vl', tabindex: 0, role: 'list', 'aria-label': t('hist.list') }, spacer);
    const side = h('aside', { class: 'mk-side hs-side', 'aria-live': 'polite' });
    root0.append(h('div', { class: 'mk hs' }, head, h('div', { class: 'mk-body' }, h('div', { class: 'mk-main' }, scroller), side)));

    const visible = () => filterRows(st.rows, st.q);
    let geo = null, geoFor = null;
    const graph = () => { if (geoFor !== st.rows.length) { geo = layout(st.rows); geoFor = st.rows.length; } return geo; };

    function rowEl(r, i, g, showGraph) {
      const task = taskOf(r, S.tasks.values());
      const gcell = h('span', { class: 'hs-g' });
      if (showGraph && g) {
        const gr = g.rows[i], svg = sv('svg', { width: g.width, height: g.rowH, class: 'hs-svg', 'aria-hidden': 'true' });
        for (const p of gr.paths) svg.append(sv('path', { d: p.d, class: 'gp gc' + p.c, fill: 'none', 'stroke-width': 2 }));
        svg.append(sv('circle', { cx: gr.cx, cy: gr.cy, r: gr.merge ? 4 : 4.5, class: 'gn gc' + gr.c + (gr.merge ? ' mg' : ''), 'stroke-width': 2 }));
        gcell.append(svg);
      }
      const el = h('div', { class: 'hrow' + (st.sel && st.sel.sha === r.sha ? ' on' : ''), role: 'listitem', tabindex: -1, dataset: { c: String(r.lane % LANES) }, onclick: () => select(r),
        onmouseenter: () => { scroller.dataset.hl = String(r.lane % LANES); }, onmouseleave: () => { delete scroller.dataset.hl; } },
        gcell,
        h('span', { class: 'hs-msg' },
          task ? h('span', { class: 'hs-task' }, envIcon(task.agent), h('span', { text: task.title })) : null,
          h('span', { class: 'hs-sub', text: r.subject }),
          r.refs.map((x) => h('span', { class: 'ref ' + refKind(x), text: refLabel(x) }))),
        h('span', { class: 'hs-who mute', text: r.author }), h('time', { class: 'hs-when mute', title: new Date(r.time * 1000).toLocaleString(), text: ago(r.time) }),
        h('code', { class: 'hs-sha', text: r.sha.slice(0, 7) }));
      if (showGraph && g) NS.setVars(el, { '--gw': g.width + 'px' });
      return el;
    }
    let raf = 0;
    function paint() {
      raf = 0;
      if (!alive.v) return;
      const rows = visible(), showGraph = !st.q.trim(), g = showGraph ? graph() : null;
      const r = root.NSMarket.vrange(scroller.scrollTop, scroller.clientHeight || 600, NSHistory.ROW_H, rows.length, 6);
      NS.setVars(spacer, { '--h': r.height + 'px' });
      spacer.replaceChildren(...rows.slice(r.start, r.end).map((row, k) => { const el = rowEl(row, showGraph ? r.start + k : 0, g, showGraph); NS.setVars(el, { '--y': (r.start + k) * NSHistory.ROW_H + 'px' }); return el; }));
      if (!rows.length) spacer.append(h('p', { class: 'mk-none', text: st.loading ? t('mk.loading') : st.err ? NS.errHuman(NS.api.errText(st.err)) : st.q ? t('mk.none_q') : t('hist.empty') }));
      if (!st.done && !st.loading && scroller.scrollTop + (scroller.clientHeight || 600) > r.height - NSHistory.ROW_H * 30) load();
    }
    const schedule = () => { if (!raf) raf = requestAnimationFrame(paint); };
    scroller.addEventListener('scroll', schedule);
    input.addEventListener('input', () => { st.q = input.value; scroller.scrollTop = 0; paint(); if (st.q.trim() && !st.done) load(); });

    async function load() {
      const p = project();
      if (!p || st.loading || st.done) return;
      const seq = st.seq;
      st.loading = true;
      try {
        const page = (await call('git_log_page', { path: p.path, skip: st.rows.length, limit: NSHistory.PAGE })) || [];
        if (!alive.v || seq !== st.seq) return;
        st.rows = st.rows.concat(page);
        if (page.length < NSHistory.PAGE) st.done = true;
      } catch (e) {
        if (!alive.v || seq !== st.seq) return;
        st.done = true;
        if (!isEmptyRepoError(NS.api.errText(e))) st.err = e;
      }
      st.loading = false; paint();
      // a search over what is loaded only: keep reading while there are portions
      if (st.q.trim() && !st.done) load();
    }
    function reset() { st.seq++; st.rows = []; st.done = false; st.loading = false; st.sel = null; st.err = null; geoFor = null; scroller.scrollTop = 0; drawSide(); drawStatus(); load(); }

    // ----- the repository line: published -> link, ahead / behind, Push / Pull; not yet -> "Опубликовать" -----
    async function drawStatus() {
      const p = project();
      clear(status);
      if (!p) return;
      let repo = S.repos.get(p.id);
      if (repo === undefined) repo = await NS.store.loadRepo(p.id);
      if (!alive.v || !project() || project().id !== p.id) return;
      clear(status);
      if (!repo) return;
      const web = safeHttps(repo.web_url);
      if (!web) { status.append(h('button', { type: 'button', class: 'btn', text: t('pub.open'), onclick: () => NS.publish.open(p.id, { onDone: () => { S.repos.delete(p.id); drawStatus(); } }) })); return; }
      const sync = async (cmd) => { try { await call(cmd, { projectId: p.id }); S.repos.delete(p.id); NS.shell.toast(t(cmd === 'push_base' ? 'pub.pushed' : 'pub.pulled')); reset(); } catch (e) { fail(e); } };
      status.append(h('button', { type: 'button', class: 'linkb', title: web, text: web.replace(/^https:\/\//, ''), onclick: () => call('open_path', { path: web }).catch(fail) }),
        h('span', { class: 'mute', title: t('pub.ahead_behind'), text: '↑' + (repo.ahead || 0) + ' ↓' + (repo.behind || 0) }),
        h('button', { type: 'button', class: 'btn sm', text: t('pub.push'), onclick: () => sync('push_base') }),
        h('button', { type: 'button', class: 'btn sm', text: t('pub.pull'), onclick: () => sync('pull_base') }));
    }

    // ----- a commit: files, diff, "Откатить" -----
    function select(r) {
      st.sel = r; st.files = null; st.diff = null; paint(); drawSide();
      const p = project(), task = taskOf(r, S.tasks.values());
      const ask = task ? call('get_diffstat', { taskId: task.id }) : call('commit_stat', { path: p.path, sha: r.sha });
      ask.then((f) => { if (st.sel === r) { st.files = f || []; drawSide(); } }).catch((e) => { if (st.sel === r) { st.files = { error: e }; drawSide(); } });
    }
    async function showDiff(path) {
      const r = st.sel, p = project(), task = taskOf(r, S.tasks.values());
      st.diff = { path, lines: null }; drawSide();
      try {
        const d = task ? await call('diff_file', { taskId: task.id, path }) : await call('commit_file_diff', { path: p.path, sha: r.sha, file: path });
        if (st.sel === r && st.diff && st.diff.path === path) st.diff = { path, lines: (d && d.lines) || [], binary: d && d.binary, truncated: d && d.truncated };
      } catch (e) { if (st.sel === r) st.diff = { path, lines: [{ kind: 'ctx', text: NS.errHuman(NS.api.errText(e)) }] }; }
      drawSide();
    }
    async function revert(r) {
      const p = project(), task = taskOf(r, S.tasks.values()), plan = revertPlan(r, task);
      const ok = await NS.projects.confirm(t(plan.kind === 'undo' ? 'hist.undo_q' : 'hist.revert_q', { sha: r.sha.slice(0, 7) }), t(plan.kind === 'undo' ? 'hist.undo_text' : 'hist.revert_text'), t(plan.kind === 'undo' ? 'hist.undo' : 'hist.revert'));
      if (!ok) return;
      try {
        if (plan.kind === 'undo') await call('undo', { id: plan.id });
        else {
          const res = await call('git_revert_commit', { path: p.path, sha: plan.sha });
          NS.shell.toast(res && res.pushed ? t('hist.reverted_pushed', { sha: String((res && res.sha) || '').slice(0, 7) }) : t('hist.reverted', { sha: String((res && res.sha) || '').slice(0, 7) }));
        }
        S.repos.delete(p.id); reset();
      } catch (e) { fail(e); }
    }
    function drawSide() {
      if (!alive.v) return;
      clear(side);
      const r = st.sel;
      if (!r) { side.append(h('div', { class: 'mk-empty' }, h('p', { text: st.rows.length ? t('hist.pick') : '' }))); return; }
      const task = taskOf(r, S.tasks.values()), web = safeHttps((S.repos.get(st.pid) || {}).web_url);
      side.append(h('div', { class: 'mk-card' },
        h('h2', { text: r.subject }),
        task ? h('p', { class: 'hs-task big' }, envIcon(task.agent), h('span', { text: task.title })) : null,
        h('div', { class: 'mk-tags' }, r.refs.map((x) => h('span', { class: 'ref ' + refKind(x), text: refLabel(x) }))),
        h('dl', { class: 'mk-meta' }, h('div', { class: 'mk-kv' }, h('dt', { text: t('hist.commit') }), h('dd', {}, h('code', { text: r.sha }))),
          h('div', { class: 'mk-kv' }, h('dt', { text: t('mk.author') }), h('dd', { text: r.author })), h('div', { class: 'mk-kv' }, h('dt', { text: t('hist.when') }), h('dd', { text: new Date(r.time * 1000).toLocaleString() }))),
        h('div', { class: 'mk-acts' }, h('button', { type: 'button', class: 'btn', text: task ? t('hist.undo') : t('hist.revert'), onclick: () => revert(r) }),
          web ? h('button', { type: 'button', class: 'btn', text: t('pub.view'), onclick: () => call('open_path', { path: web + '/commit/' + r.sha }).catch(fail) }) : null),
        h('h3', { text: t('hist.files') }),
        st.files === null ? h('p', { class: 'mute', text: t('mk.loading') }) : st.files.error ? h('p', { class: 'mk-err', text: NS.errHuman(NS.api.errText(st.files.error)) })
          : !st.files.length ? h('p', { class: 'mute', text: t('hist.no_files') })
          : h('ul', { class: 'hs-files' }, st.files.map((f) => h('li', {}, h('button', { type: 'button', class: 'hs-file' + (st.diff && st.diff.path === f.path ? ' on' : ''), onclick: () => showDiff(f.path) },
            h('span', { class: 'hs-fp', text: f.path }), h('span', { class: 'ok', text: '+' + f.insertions }), h('span', { class: 'bad', text: '−' + f.deletions }))))),
        st.diff ? h('div', { class: 'hs-diff' }, st.diff.lines === null ? h('p', { class: 'mute', text: t('mk.loading') }) : st.diff.binary ? h('p', { class: 'mute', text: t('hist.binary') })
          : st.diff.lines.map((l) => h('div', { class: 'dl ' + (l.kind === 'add' ? 'a' : l.kind === 'del' ? 'd' : l.kind === 'hunk' ? 'hk' : ''), role: 'presentation' },
            h('i', {}), h('b', { text: l.kind === 'add' ? '+' : l.kind === 'del' ? '−' : '' }), h('span', { text: String(l.text).replace(/^[+\- ]/, (m) => (l.kind === 'hunk' ? m : '')) })))) : null));
    }
    drawStatus(); drawSide(); load();
    return { destroy() { alive.v = false; } };
  }
  NS.registry.register({ id: 'history', layout: 'full', mount });
  NS.registry.extend('command', { id: 'history.open', labelKey: 'hist.cmd', group: 'mk.group', order: 73, run: () => NS.nav('history', {}) });
  NS.history = { open: (projectId) => NS.nav('history', { project: projectId }) };
})(typeof window !== 'undefined' ? window : globalThis);
