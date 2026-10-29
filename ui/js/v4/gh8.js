// v8 (Agentation notes 22:09, 22:37): the GitHub panel right of the terminals, under its button, so the chat keeps the centre - the branch tree of the project (the commit graph of
// core::git::log_page, drawn by NSHistory.layout) and its open pull requests (`gh pr list` through gh_prs, the login of gh,
// no token in the app). Folds away with its button in the session tabs; the state is kept per window.
(function (root) {
  // ---------- pure (selftest) ----------
  // the checks of a PR in one word: fail beats pending beats ok; none -> ''
  function checks(rollup) {
    const xs = Array.isArray(rollup) ? rollup : [];
    if (!xs.length) return '';
    const v = xs.map((c) => String(c.conclusion || c.state || c.status || '').toUpperCase());
    if (v.some((x) => /FAIL|ERROR|CANCEL|TIMED_OUT|ACTION_REQUIRED/.test(x))) return 'fail';
    if (v.some((x) => !x || /PENDING|QUEUED|IN_PROGRESS|EXPECTED|WAITING/.test(x))) return 'run';
    return 'ok';
  }
  root.NSGh8 = { checks };
  const NS = root.NS;
  if (!NS || !NS.h || typeof document === 'undefined') return;

  const { h, ic, clear, setVars } = NS;
  const t = (k, p) => NS.t(k, p);
  const S = () => NS.store.s;
  const H = () => root.NSHistory;
  const call = (c, a) => NS.api.call(c, a);
  const SVGNS = 'http://www.w3.org/2000/svg';
  const sv = (tag, attrs) => { const e = document.createElementNS(SVGNS, tag); for (const [k, v] of Object.entries(attrs || {})) e.setAttribute(k, v); return e; };
  const KEY = 'ns.gh8';
  const prefs = () => { try { return Object.assign({ open: false, tab: 'tree' }, JSON.parse(NS.storage.get(KEY) || '{}')); } catch (e) { return { open: false, tab: 'tree' }; } };
  const save = (p) => { try { NS.storage.set(KEY, JSON.stringify(Object.assign(prefs(), p))); } catch (e) { /* only this window */ } };
  const ROWS = 80, ROW_H = 30;
  const ago = (sec) => { const r = H().relTime(sec, Date.now()); return r.key === 'hist.date' ? t(r.key, { date: r.date }) : t(r.key, { n: r.n }); };
  const project = () => NS.store.project(NS.projects.current());
  const openUrl = (u) => { const ok = root.NSMarket && root.NSMarket.safeHttps ? root.NSMarket.safeHttps(u) : /^https:\/\//.test(u) ? u : null; if (ok) call('open_path', { path: ok }).catch(() => {}); };

  function mount(el) {
    el.classList.add('gh8');
    let tab = prefs().tab, data = { pid: null, rows: null, prs: null, prErr: '', logErr: '' }, alive = true;
    const head = h('div', { class: 'gh8-h' }), body = h('div', { class: 'gh8-b' });
    el.append(head, body);
    async function load(force) {
      const p = project();
      if (!p) { data = { pid: null, rows: null, prs: null }; draw(); return; }
      if (!force && data.pid === p.id && data.rows) { draw(); return; }
      data = { pid: p.id, rows: null, prs: null, prErr: '', logErr: '' };
      draw();
      call('git_log_page', { path: p.path, skip: 0, limit: ROWS }).then((r) => { data.rows = r || []; }).catch((e) => { data.rows = []; data.logErr = NS.api.errText(e); }).then(() => alive && draw());
      call('gh_prs', { path: p.path }).then((r) => { data.prs = Array.isArray(r) ? r : []; }).catch((e) => { data.prs = []; data.prErr = NS.api.errText(e); }).then(() => alive && draw());
    }
    function seg(id, label, n) {
      return h('button', { type: 'button', role: 'tab', class: 'gh8-seg' + (tab === id ? ' on' : ''), 'aria-selected': String(tab === id), onclick: () => { tab = id; save({ tab }); draw(); } },
        h('span', { text: label }), n != null ? h('small', { text: String(n) }) : null);
    }
    function draw() {
      const p = project();
      clear(head).append(
        h('div', { class: 'gh8-t' }, ic('i-branch'), h('b', { text: p ? p.name : 'GitHub' }),
          h('button', { type: 'button', class: 'gh8-ico', title: t('gh8.refresh'), 'aria-label': t('gh8.refresh'), onclick: () => load(true) }, ic('i-refresh'))));
      // append(null) would print "null" without a project
      if (p) head.append(h('div', { class: 'gh8-segs', role: 'tablist' }, seg('tree', t('gh8.tree')), seg('prs', t('gh8.prs'), data.prs ? data.prs.length : null)));
      clear(body);
      if (!p) { body.append(h('p', { class: 'gh8-empty', text: t('gh8.no_project') })); return; }
      // a project whose folder is gone has no history to show: say that, not «Коммитов пока нет»
      const repo = NS.store.s.repos.get(p.id);
      if (repo === undefined) NS.store.loadRepo(p.id).then(() => alive && draw()).catch(() => {});
      if (repo === null || (repo && repo.is_repo === false)) { body.append(h('p', { class: 'gh8-empty', text: t('gh8.gone', { path: p.path }) })); return; }
      if (tab === 'tree') drawTree(p); else drawPrs();
    }
    function drawTree(p) {
      if (!data.rows) { body.append(h('p', { class: 'gh8-empty', text: t('gh8.loading') })); return; }
      if (!data.rows.length) { body.append(h('p', { class: 'gh8-empty', text: data.logErr && !H().isEmptyRepoError(data.logErr) ? data.logErr : t('gh8.no_commits') })); return; }
      const g = H().layout(data.rows, 12, ROW_H);
      const list = h('div', { class: 'gh8-tree' });
      data.rows.forEach((r, i) => {
        const L = g.rows[i], svg = sv('svg', { class: 'gh8-g', width: String(g.width), height: String(ROW_H), viewBox: `0 0 ${g.width} ${ROW_H}`, 'aria-hidden': 'true' });
        for (const pth of L.paths) svg.append(sv('path', { d: pth.d, class: 'gc' + pth.c, fill: 'none', 'stroke-width': '1.6' }));
        svg.append(sv('circle', { cx: String(L.cx), cy: String(L.cy), r: L.merge ? '2.6' : '3.4', class: 'gc' + L.c + (L.merge ? ' gh8-merge' : '') }));
        const refs = r.refs.filter((x) => !/HEAD/.test(x)).slice(0, 2).map((x) => h('span', { class: 'gh8-ref ' + H().refKind(x), text: H().refLabel(x) }));
        list.append(h('button', { type: 'button', class: 'gh8-c', title: r.subject + '\n' + r.sha.slice(0, 8) + ' · ' + r.author, onclick: () => NS.history && NS.history.open(p.id) },
          svg, h('span', { class: 'gh8-cm' }, h('span', { class: 'gh8-s' }, ...refs, h('span', { class: 'gh8-sub', text: r.subject })), h('small', { text: r.author + ' · ' + ago(r.time) }))));
      });
      body.append(list, h('button', { type: 'button', class: 'gh8-more', text: t('gh8.all_history'), onclick: () => NS.history && NS.history.open(p.id) }));
    }
    function drawPrs() {
      if (!data.prs) { body.append(h('p', { class: 'gh8-empty', text: t('gh8.loading') })); return; }
      if (data.prErr) {
        const login = /not_logged_in|auth login/i.test(data.prErr), missing = /not_installed/i.test(data.prErr);
        body.append(h('div', { class: 'gh8-hint' }, h('b', { text: t(missing ? 'gh8.no_gh' : login ? 'gh8.no_login' : 'gh8.err') }), h('small', { text: missing ? 'https://cli.github.com' : login ? 'gh auth login' : data.prErr }),
          login && NS.term && NS.term.open ? h('button', { type: 'button', class: 'btn sm', text: t('gh8.login_btn'), onclick: () => NS.term.open({ argv: ['gh', 'auth', 'login'], title: 'gh auth login', cwd: null }) }) : null));
        return;
      }
      if (!data.prs.length) { body.append(h('p', { class: 'gh8-empty', text: t('gh8.no_prs') })); return; }
      body.append(h('div', { class: 'gh8-prs' }, data.prs.map((pr) => {
        const ck = checks(pr.statusCheckRollup), rv = String(pr.reviewDecision || '').toUpperCase();
        return h('button', { type: 'button', class: 'gh8-pr', title: pr.url, onclick: () => openUrl(pr.url) },
          h('span', { class: 'gh8-pr-h' }, h('i', { class: 'gh8-ck ' + (ck || 'none'), title: ck ? t('gh8.ck_' + ck) : t('gh8.ck_none'), 'aria-label': ck ? t('gh8.ck_' + ck) : t('gh8.ck_none') }),
            h('span', { class: 'gh8-pr-t', text: pr.title }), h('small', { class: 'gh8-num', text: '#' + pr.number })),
          h('span', { class: 'gh8-pr-m' }, h('span', { class: 'gh8-ref branch', text: pr.headRefName }), pr.isDraft ? h('span', { class: 'gh8-tag', text: t('gh8.draft') }) : null,
            rv === 'APPROVED' ? h('span', { class: 'gh8-tag ok', text: t('gh8.approved') }) : rv === 'CHANGES_REQUESTED' ? h('span', { class: 'gh8-tag bad', text: t('gh8.changes') }) : null,
            pr.updatedAt ? h('small', { text: ago(Date.parse(pr.updatedAt) / 1000) }) : null));
      })));
    }
    load();
    return { update: (evt) => { if (evt === 'project') load(); if (evt === 'lang') draw(); }, destroy: () => { alive = false; } };
  }

  NS.gh8 = { mount, get open() { return prefs().open; }, setOpen: (v) => save({ open: !!v }) };
})(typeof window !== 'undefined' ? window : globalThis);
