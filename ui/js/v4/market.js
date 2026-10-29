// v4 catalog (task 24, stories 266-273): skills, MCP servers and mods from the sources of core::market.
// Pure helpers (NSMarket: the virtual window, the install plan view, the source rows) run in selftest; the screen below uses them.
// Everything that comes from a source is untrusted: it lands through textContent only, links only when they are https.
(function (root) {
  // ---------- pure ----------
  // the rows of a long list that must exist for a scroll position: a window plus a few spare rows, never the whole 10 000
  function vrange(scrollTop, viewH, rowH, total, over) {
    const o = over == null ? 4 : over;
    const first = Math.floor(Math.max(0, scrollTop) / rowH), last = Math.ceil((Math.max(0, scrollTop) + Math.max(0, viewH)) / rowH);
    const start = Math.max(0, Math.min(total, first - o)), end = Math.max(start, Math.min(total, last + o));
    return { start, end, height: total * rowH };
  }
  const safeHttps = (u) => {
    const s = String(u == null ? '' : u).trim();
    if (!/^https:\/\/[^\s/?#]+/i.test(s)) return null;
    try { return typeof URL === 'function' ? new URL(s).href : s; } catch (e) { return null; }
  };
  // the next portion joins the list; an item seen twice (two sources, a repeated cursor) stays once, in its first place
  function mergeItems(list, more) {
    const seen = new Set(list.map((i) => i.id));
    return list.concat((more || []).filter((i) => i && i.id && !seen.has(i.id) && seen.add(i.id)));
  }
  // what the plan of core::market shows before "Применить": what is written, and what runs code (off until ticked)
  function planView(plan, ticked) {
    const on = ticked || new Set(), w = plan.warnings || [];
    const exec = (plan.executable || []).map((key) => ({ key, on: on.has(key) }));
    return { writes: plan.writes || [], exec, enabled: exec.filter((e) => e.on).map((e) => e.key), runs: plan.runs || null, needs: plan.needs || [],
      runsCode: w.includes('runs_code'), remote: w.includes('remote'), docker: w.includes('needs_docker'), texts: plan.texts || [], installed: !!plan.installed };
  }
  // "установлено" / "есть обновление" of one item against market_installed
  function itemState(item, installed) {
    const rec = (installed || []).find((x) => x.id === item.id);
    return { installed: !!rec, update: rec && rec.update ? rec.update : null, version: rec ? rec.version : null };
  }
  const fmtCount = (n) => (n == null ? '' : n >= 1e6 ? (n / 1e6).toFixed(1).replace(/\.0$/, '') + 'M' : n >= 1e3 ? (n / 1e3).toFixed(1).replace(/\.0$/, '') + 'k' : String(n));
  // core errors: {kind, detail} or a text
  function errParts(e) {
    if (e && typeof e === 'object' && e.kind) return { kind: String(e.kind), detail: e.detail == null ? '' : String(e.detail) };
    const s = typeof e === 'string' ? e : (e && e.message) || JSON.stringify(e || '');
    const m = /^([a-z_]+):\s*(.*)$/.exec(s);
    return m ? { kind: m[1], detail: m[2] } : { kind: s, detail: '' };
  }
  const validSource = (kind, url) => ['skill', 'mcp', 'mod'].includes(kind) && !!safeHttps(url);
  root.NSMarket = { vrange, safeHttps, mergeItems, planView, itemState, fmtCount, errParts, validSource, ROW_H: 62 };
  if (typeof document === 'undefined' || !root.NS || !root.NS.h) return;

  // ---------- the screen ----------
  const NS = root.NS;
  const { h, ic, envIcon, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const S = NS.store.s;
  const call = (c, a) => NS.api.call(c, a);
  const KINDS = ['skill', 'mcp', 'mod'];
  const PAGE_PX = 600;
  const errText = (e) => {
    const p = errParts(e), I = root.NSI18n, k = 'mk.err.' + p.kind, kk = 'kit.err.' + p.kind;
    return I.has(k) ? t(k, { detail: p.detail }) : I.has(kk) ? t(kk, { detail: p.detail, env: '' }) : t('mk.err.other', { detail: p.kind + (p.detail ? ': ' + p.detail : '') });
  };
  const fail = (e) => NS.shell.toast(errText(e), true);
  const dateOnly = (v) => (v ? String(v).slice(0, 10) : '');
  const openLink = (u) => { const x = safeHttps(u); if (x) call('open_path', { path: x }).catch(fail); };

  function mount(root0, ctx) {
    const alive = { v: true };
    const st = { kind: KINDS.includes(ctx.params.kind) ? ctx.params.kind : 'skill', q: '', items: [], cursor: null, loading: false, done: false, offline: [], err: null,
      sel: null, detail: null, plan: null, ticked: new Set(), vars: new Map(), scope: 'global', panel: 'item', sources: [], installed: [], busy: false, link: false, seq: 0 };
    const envId = () => ctx.params.env || S.lastEnv || ((S.envs.find((e) => e.state === 'ok') || {}).id);
    // mods exist for Claude Code only (G40): a mod always goes to its folder
    const envFor = () => (st.kind === 'mod' ? 'claude' : envId());
    const env = () => NS.store.env(envFor()) || { id: envFor(), name: envFor() || '' };
    const scopeValue = () => (st.scope === 'global' ? 'global' : { project: (NS.store.project(st.scope) || {}).path || '' });

    // header
    const kindBtns = KINDS.map((k) => h('button', { type: 'button', role: 'tab', dataset: { kind: k }, onclick: () => setKind(k), text: t('mk.kind_' + k) }));
    const input = h('input', root.NSPalette.quiet({ type: 'text', class: 'mk-q', placeholder: t('mk.search_ph'), 'aria-label': t('mk.search') }));
    const linkIn = h('input', { type: 'text', class: 'mk-link', placeholder: t('mk.link_ph'), 'aria-label': t('mk.link'), spellcheck: false });
    const linkBox = h('div', { class: 'mk-linkbox', hidden: true }, linkIn,
      h('button', { type: 'button', class: 'btn', text: t('mk.folder'), onclick: async () => { const p = await NS.projects.pickFolder(); if (p) { linkIn.value = p; planFor({ kind: st.kind, link: p }); } } }),
      h('button', { type: 'button', class: 'btn primary', text: t('mk.check'), onclick: () => linkIn.value.trim() && planFor({ kind: st.kind, link: linkIn.value.trim() }) }));
    const head = h('div', { class: 'mk-h' },
      h('div', { class: 'mk-t' }, h('h1', { text: t('mk.title') }), h('span', { class: 'mute', text: env().name })),
      h('div', { class: 'seg', role: 'tablist', 'aria-label': t('mk.title') }, kindBtns), input,
      h('button', { type: 'button', class: 'btn', onclick: () => { st.link = !st.link; linkBox.hidden = !st.link; if (st.link) linkIn.focus(); }, text: t('mk.by_link') }),
      h('button', { type: 'button', class: 'btn', onclick: () => showSources(), text: t('mk.sources') }));
    const notes = h('div', { class: 'mk-notes', role: 'status' });
    const spacer = h('div', { class: 'mk-vs' });
    const scroller = h('div', { class: 'mk-vl', tabindex: 0, role: 'listbox', 'aria-label': t('mk.list') }, spacer);
    const side = h('aside', { class: 'mk-side', 'aria-live': 'polite' });
    const body = h('div', { class: 'mk-body' }, h('div', { class: 'mk-main' }, scroller), side);
    root0.append(h('div', { class: 'mk' }, head, linkBox, notes, body));

    // ----- list -----
    const rowOf = (it, i) => {
      const s = itemState(it, st.installed);
      const row = h('button', { type: 'button', class: 'mk-row' + (st.sel && st.sel.id === it.id ? ' on' : ''), role: 'option', 'aria-selected': String(!!(st.sel && st.sel.id === it.id)), onclick: () => select(it) },
        h('span', { class: 'mk-n' }, h('b', { text: it.name }), h('span', { class: 'mk-a mute', text: [it.author, it.source].filter(Boolean).join(' · ') })),
        h('span', { class: 'mk-d', text: it.description || '' }),
        h('span', { class: 'mk-m' },
          s.update ? h('span', { class: 'tag upd', text: t('mk.update') }) : s.installed ? h('span', { class: 'tag ok', text: t('mk.installed') }) : null,
          it.installs != null || it.stars != null ? h('span', { class: 'mute', text: (it.installs != null ? '↓ ' + fmtCount(it.installs) : '★ ' + fmtCount(it.stars)) }) : null));
      NS.setVars(row, { '--y': i * NSMarket.ROW_H + 'px' });
      return row;
    };
    let raf = 0;
    function paint() {
      raf = 0;
      if (!alive.v) return;
      const r = vrange(scroller.scrollTop, scroller.clientHeight || PAGE_PX, NSMarket.ROW_H, st.items.length);
      NS.setVars(spacer, { '--h': r.height + 'px' });
      const rows = st.items.slice(r.start, r.end).map((it, k) => rowOf(it, r.start + k));
      spacer.replaceChildren(...rows);
      if (!st.items.length && !st.loading) spacer.append(h('p', { class: 'mk-none', text: st.err ? errText(st.err) : st.q ? t('mk.none_q') : t('mk.none') }));
      if (st.loading && !st.items.length) spacer.append(h('p', { class: 'mk-none', text: t('mk.loading') }));
      if (!st.done && !st.loading && st.cursor && scroller.scrollTop + (scroller.clientHeight || PAGE_PX) > r.height - NSMarket.ROW_H * 8) load(false);
    }
    const schedule = () => { if (!raf) raf = requestAnimationFrame(paint); };
    scroller.addEventListener('scroll', schedule);
    function drawNotes() {
      clear(notes);
      for (const o of st.offline) {
        notes.append(h('p', { class: 'mk-off' }, ic('i-warn'), h('span', { text: o.cached_at ? t('mk.offline_cached', { source: o.source, date: dateOnly(o.cached_at) }) : t('mk.offline', { source: o.source }) })));
      }
      notes.hidden = !st.offline.length;
    }
    async function load(fresh) {
      if (st.loading) return;
      const seq = ++st.seq;
      if (fresh) { st.items = []; st.cursor = null; st.done = false; st.offline = []; st.err = null; scroller.scrollTop = 0; }
      st.loading = true; paint();
      try {
        const page = await call('market_search', { kind: st.kind, query: st.q, cursor: st.cursor });
        if (!alive.v || seq !== st.seq) return;
        st.items = mergeItems(st.items, page.items);
        st.cursor = page.cursor || null; st.done = !page.cursor;
        if (fresh || page.offline.length) st.offline = page.offline || [];
      } catch (e) { if (!alive.v || seq !== st.seq) return; st.err = e; st.done = true; fail(e); }
      st.loading = false; drawNotes(); paint();
    }
    async function loadInstalled() {
      try { st.installed = (await call('market_installed', { env: envFor() })) || []; } catch (e) { st.installed = []; }
      if (alive.v) { paint(); drawSide(); }
    }
    let timer = 0;
    input.addEventListener('input', () => { clearTimeout(timer); timer = setTimeout(() => { st.q = input.value.trim(); load(true); }, 280); });
    function setKind(k) {
      st.kind = k; st.sel = null; st.detail = null; st.plan = null; st.panel = 'item'; input.value = ''; st.q = '';
      kindBtns.forEach((b) => { const on = b.dataset.kind === k; b.classList.toggle('on', on); b.setAttribute('aria-selected', String(on)); });
      load(true); loadInstalled(); drawSide();
    }

    // ----- side: card / plan / sources -----
    function select(it) {
      st.sel = it; st.detail = null; st.plan = null; st.panel = 'item'; st.ticked = new Set(); st.vars = new Map();
      paint(); drawSide();
      const id = it.id;
      call('market_item', { id }).then((d) => { if (alive.v && st.sel && st.sel.id === id) { st.detail = d; drawSide(); } }).catch((e) => { if (alive.v && st.sel && st.sel.id === id) { st.detail = { error: e }; drawSide(); } });
    }
    const kv = (k, v) => (v ? h('div', { class: 'mk-kv' }, h('dt', { text: k }), h('dd', { text: v })) : null);
    function scopePicker() {
      const sel = h('select', { class: 'mk-scope', 'aria-label': t('mk.scope'), onchange: () => { st.scope = sel.value; } },
        h('option', { value: 'global', text: t('mk.scope_global'), selected: st.scope === 'global' }),
        S.projects.map((p) => h('option', { value: p.id, text: t('mk.scope_project', { name: p.name }), selected: st.scope === p.id })));
      return h('label', { class: 'mk-scoperow' }, h('span', { class: 'mute', text: t('mk.scope') }), sel);
    }
    function drawItem() {
      if (!st.sel) return h('div', { class: 'mk-empty' }, h('p', { text: t('mk.pick') }), h('p', { class: 'mute', text: t('mk.trust') }));
      const d = st.detail && !st.detail.error ? st.detail : null;
      // the card's own data (SKILL.md: description, license) wins over the list row: a skill of a plugin set or from skills.sh has none there
      const it = d && d.item ? Object.assign({}, st.sel, { description: d.item.description || st.sel.description, license: d.item.license || st.sel.license }) : st.sel;
      const s = itemState(it, st.installed);
      const home = safeHttps(it.homepage);
      const out = h('div', { class: 'mk-card' },
        h('h2', { text: it.name }),
        h('div', { class: 'mk-tags' }, s.update ? h('span', { class: 'tag upd', text: t('mk.update_to', { v: s.update }) }) : null, s.installed ? h('span', { class: 'tag ok', text: t('mk.installed') }) : null,
          h('span', { class: 'tag', text: t('mk.kind_' + it.kind) })),
        h('p', { class: 'mk-desc', text: it.description || t('mk.no_desc') }),
        h('dl', { class: 'mk-meta' }, kv(t('mk.author'), it.author), kv(t('mk.source'), it.source), kv(t('mk.license'), it.license || t('mk.no_license')),
          kv(t('mk.updated'), dateOnly(it.updated)), kv(t('mk.version'), it.version), kv(t('mk.installs'), it.installs != null ? fmtCount(it.installs) : ''), kv(t('mk.stars'), it.stars != null ? fmtCount(it.stars) : '')),
        home ? h('button', { type: 'button', class: 'linkb', text: t('mk.homepage'), title: home, onclick: () => openLink(home) }) : null);
      if (st.detail && st.detail.error) out.append(h('p', { class: 'mk-err', text: errText(st.detail.error) }));
      if (d) {
        if (d.runs) out.append(h('div', { class: 'mk-runs' }, h('span', { class: 'mute', text: it.kind === 'mcp' && /^https?:/.test(d.runs) ? t('mk.remote_at') : t('mk.runs') }), h('code', { text: d.runs })));
        if (d.needs && d.needs.length) out.append(h('p', { class: 'mute', text: t('mk.needs', { list: d.needs.join(', ') }) }));
        if (d.env_vars && d.env_vars.length) out.append(h('div', { class: 'mk-vars' }, h('span', { class: 'mute', text: t('mk.vars') }), d.env_vars.map((v) => h('code', { class: 'tag', text: v.name + (v.required ? ' *' : '') }))));
        if (d.readme) out.append(h('details', { class: 'mk-readme' }, h('summary', { text: t('mk.readme') }), h('pre', {}, h('code', { text: d.readme.slice(0, 20000) }))));
      }
      out.append(h('div', { class: 'mk-acts' }, scopePicker(),
        h('button', { type: 'button', class: 'btn primary', disabled: st.busy || (it.kind === 'mod' && !envFor()), text: s.update ? t('mk.update_do') : t('mk.install'), onclick: () => planFor({ id: it.id }) }),
        s.installed ? h('button', { type: 'button', class: 'btn danger', disabled: st.busy, text: t('mk.remove'), onclick: () => removeItem(it) }) : null));
      return out;
    }
    async function planFor(ref) {
      st.busy = true; drawSide();
      try {
        const plan = await call('market_plan', { item: ref, env: envFor(), scope: scopeValue() });
        if (!alive.v) return;
        if (ref.link) { st.sel = plan.item; st.detail = null; }
        st.plan = plan; st.ticked = new Set(); st.vars = new Map(); st.panel = 'plan';
      } catch (e) { fail(e); }
      st.busy = false; drawSide();
    }
    async function removeItem(it) {
      if (!(await NS.projects.confirm(t('mk.remove_q', { name: it.name }), t('mk.remove_text'), t('mk.remove')))) return;
      st.busy = true; drawSide();
      try { await call('market_remove', { env: envFor(), id: it.id }); NS.shell.toast(t('mk.removed', { name: it.name })); await loadInstalled(); } catch (e) { fail(e); }
      st.busy = false; drawSide();
    }
    async function applyPlan() {
      const p = st.plan, v = planView(p, st.ticked);
      st.busy = true; drawSide();
      try {
        // values of variables go to the keyring by name (set_secret), never into the plan or a file
        for (const [name, value] of st.vars) if (value) await call('set_secret', { env: envFor(), name, value });
        await call('market_apply', { plan: { id: p.id }, enabledExec: v.enabled });
        NS.shell.toast(t('mk.done', { name: p.item.name }));
        st.plan = null; st.panel = 'item'; st.vars = new Map();
        await loadInstalled();
      } catch (e) { fail(e); }
      st.busy = false; paint(); drawSide();
    }
    function drawPlan() {
      const p = st.plan, v = planView(p, st.ticked);
      const out = h('div', { class: 'mk-plan' }, h('h2', { text: t('mk.plan_title', { name: p.item.name }) }));
      if (v.runsCode) out.append(h('p', { class: 'mk-warn' }, ic('i-warn'), h('span', { text: it0(p) === 'mod' ? t('mk.warn_mod') : t('mk.warn_code') })));
      if (v.remote) out.append(h('p', { class: 'mk-warn' }, ic('i-warn'), h('span', { text: t('mk.warn_remote', { url: v.runs || '' }) })));
      if (v.docker) out.append(h('p', { class: 'mk-warn' }, ic('i-warn'), h('span', { text: t('mk.warn_docker') })));
      if (v.runs && !v.remote) out.append(h('div', { class: 'mk-runs' }, h('span', { class: 'mute', text: t('mk.runs') }), h('code', { text: v.runs })));
      out.append(h('h3', { text: t('mk.writes') }), v.writes.length ? h('ul', { class: 'mk-files' }, v.writes.map((w) => h('li', {}, h('code', { text: w })))) : h('p', { class: 'mute', text: t('mk.writes_none') }));
      if (v.exec.length) {
        out.append(h('h3', { text: t('mk.exec') }), h('p', { class: 'mute', text: t('mk.exec_d') }),
          h('ul', { class: 'mk-exec' }, v.exec.map((e) => h('li', {}, (() => {
            const note = h('span', { class: 'mute', text: t(e.on ? 'mk.exec_on' : 'mk.exec_off') });
            return h('label', {}, h('input', { type: 'checkbox', checked: e.on, onchange: (ev) => { if (ev.target.checked) st.ticked.add(e.key); else st.ticked.delete(e.key); note.textContent = t(ev.target.checked ? 'mk.exec_on' : 'mk.exec_off'); } }),
              h('code', { text: e.key }), note);
          })()))));
      }
      const vars = (st.detail && st.detail.env_vars) || [];
      if (vars.length) {
        NS.append(out, [h('h3', { text: t('mk.vars_title') }), h('p', { class: 'mute', text: t('mk.vars_d') }),
          vars.map((x) => h('label', { class: 'mk-var' }, h('span', { text: x.name + (x.required ? ' *' : '') }), x.description ? h('small', { class: 'mute', text: x.description }) : null,
            h('input', { type: x.secret ? 'password' : 'text', autocomplete: 'off', spellcheck: false, value: st.vars.get(x.name) || '', oninput: (ev) => st.vars.set(x.name, ev.target.value) })))]);
      }
      if (v.texts.length) NS.append(out, [h('h3', { text: t('mk.texts') }), v.texts.map((x) => h('details', { class: 'mk-readme' }, h('summary', {}, h('code', { text: x.path })), h('pre', {}, h('code', { text: String(x.text).slice(0, 20000) }))))]);
      out.append(h('div', { class: 'mk-acts' }, h('button', { type: 'button', class: 'btn', text: t('dlg.cancel'), onclick: () => { st.plan = null; st.panel = 'item'; drawSide(); } }),
        h('button', { type: 'button', class: 'btn primary', disabled: st.busy, text: t('mk.apply'), onclick: applyPlan })));
      return out;
    }
    const it0 = (p) => (p.item && p.item.kind) || st.kind;

    // ----- sources (story 269) -----
    async function showSources() {
      st.panel = 'sources'; drawSide();
      try { st.sources = (await call('market_sources')) || []; } catch (e) { fail(e); }
      if (alive.v) drawSide();
    }
    async function saveSources(list) {
      try { st.sources = (await call('market_set_sources', { sources: list })) || list; } catch (e) { fail(e); }
      drawSide(); load(true);
    }
    function drawSources() {
      const kindSel = h('select', { 'aria-label': t('mk.src_kind') }, KINDS.map((k) => h('option', { value: k, text: t('mk.kind_' + k) })));
      const url = h('input', { type: 'text', placeholder: t('mk.src_url_ph'), 'aria-label': t('mk.src_url'), spellcheck: false });
      const bad = h('p', { class: 'mk-err', hidden: true, text: t('mk.src_https') });
      const out = h('div', { class: 'mk-src' }, h('h2', { text: t('mk.sources') }), h('p', { class: 'mute', text: t('mk.src_d') }),
        h('ul', { class: 'mk-srcs' }, st.sources.map((s) => h('li', {},
          h('label', {}, h('input', { type: 'checkbox', checked: s.enabled, onchange: (ev) => saveSources(st.sources.map((x) => (x.id === s.id ? Object.assign({}, x, { enabled: ev.target.checked }) : x))) }),
            h('span', { class: 'mk-sn' }, h('b', { text: s.name || s.id }), h('small', { class: 'mute', text: t('mk.kind_' + s.kind) + (s.note && root.NSI18n.has('mk.note_' + s.note) ? ' · ' + t('mk.note_' + s.note) : '') }))),
          s.builtin ? null : h('button', { type: 'button', class: 'btn sm', text: t('mk.src_remove'), onclick: () => saveSources(st.sources.filter((x) => x.id !== s.id)) })))),
        h('h3', { text: t('mk.src_add') }), h('div', { class: 'mk-srcadd' }, kindSel, url,
          h('button', { type: 'button', class: 'btn', text: t('mk.src_add_do'), onclick: () => {
            if (!validSource(kindSel.value, url.value.trim())) { bad.hidden = false; return; }
            const u = url.value.trim();
            saveSources(st.sources.concat([{ id: 'custom-' + Date.now().toString(36), kind: kindSel.value, url: safeHttps(u), enabled: true, api: 'marketplace', name: u.replace(/^https:\/\//, ''), builtin: false, note: null }]));
          } })), bad,
        h('div', { class: 'mk-acts' }, h('button', { type: 'button', class: 'btn', text: t('mk.src_back'), onclick: () => { st.panel = 'item'; drawSide(); } })));
      return out;
    }
    function drawSide() { if (!alive.v) return; clear(side).append(st.panel === 'sources' ? drawSources() : st.panel === 'plan' && st.plan ? drawPlan() : drawItem()); }

    setKind(st.kind);
    return { destroy() { alive.v = false; clearTimeout(timer); } };
  }
  NS.registry.register({ id: 'catalog', layout: 'full', keepEnv: true, mount });
  const go = (kind) => () => { const e = NS.store.env(NS.store.s.lastEnv) || {}; NS.nav('catalog', { env: e.id, kind }); };
  NS.registry.extend('command', { id: 'market.skill', labelKey: 'mk.cmd_skill', group: 'mk.group', order: 70, run: go('skill') });
  NS.registry.extend('command', { id: 'market.mcp', labelKey: 'mk.cmd_mcp', group: 'mk.group', order: 71, run: go('mcp') });
  NS.registry.extend('command', { id: 'market.mod', labelKey: 'mk.cmd_mod', group: 'mk.group', order: 72, run: go('mod') });
  NS.registry.extend('bar', { id: 'catalog', side: 'right', order: 5, render(el) {
    el.append(h('button', { type: 'button', class: 'bm', title: t('mk.title'), 'aria-label': t('mk.title'), onclick: go('skill') }, h('span', { text: t('mk.bar') })));
  } });
  NS.market = { open: (kind, envId) => NS.nav('catalog', { env: envId, kind }) };
})(typeof window !== 'undefined' ? window : globalThis);
