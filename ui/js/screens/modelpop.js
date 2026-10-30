// "Модель <Среда>": model cards with vendor marks, a version row under the selected one, search and vendor groups for long lists,
// "Своя модель" and "Как в CLI по умолчанию". One picker for the composer popover and for the first-run step 2.
(function () {
  const NS = window.NS;
  const { h, ic, vendorIcon, clear } = NS;
  const L = window.NSLogic;
  const t = (k, p) => NS.t(k, p);
  const CLI_DEFAULT = 'default';   // ModelChoice.family "default" (core: family id "default", vendor "cli") = run without a model flag

  const choiceOf = (envId) => NS.store.envSettings(envId).default_model;
  function label(envId) {
    const l = L.modelLabel(choiceOf(envId), NS.store.s.catalogs.get(envId));
    return l === '' ? t('model.cli_default') : l;   // null (no catalog yet) stays null
  }
  // the thinking level (EnvSettings.effort) among the words the env's CLI takes (env.efforts from the core); '' = as the CLI decides
  const effortsOf = (envId) => (NS.store.env(envId) || {}).efforts || [];
  const effortOf = (envId) => { const e = NS.store.envSettings(envId).effort; return effortsOf(envId).includes(e) ? e : ''; };
  // the composer button: "Opus 5.5 · Высокое"
  function buttonLabel(envId) {
    const m = label(envId), e = effortOf(envId);
    return m && e ? m + ' · ' + t('effort.' + e) : m;
  }
  // "Мышление": chips under the model list; a pick is saved at once and the menu stays open
  function effortRow(envId, after) {
    const levels = effortsOf(envId);
    if (!levels.length) return null;
    const cur = effortOf(envId);
    const pick = async (e) => {
      try { await NS.store.saveEnvSettings(envId, { effort: e || null }); } catch (er) { NS.shell.toast(NS.errHuman(NS.api.errText(er)), true); return; }
      after();
    };
    return h('div', { class: 'effort-row', title: t('effort.d') }, h('span', { class: 'hint', text: t('effort.title') }),
      h('div', { class: 'vers', role: 'radiogroup', 'aria-label': t('effort.title') },
        [''].concat(levels).map((l) => h('button', { type: 'button', class: 'v' + (l === cur ? ' on' : ''), role: 'radio', 'aria-checked': String(l === cur),
          text: l ? t('effort.' + l) : t('effort.cli'), onclick: (ev) => { ev.stopPropagation(); pick(l); } }))));
  }
  // story 240: the core sends model.desc.<id>; "(… docs, …; не сверено с CLI)" is only a tooltip
  const descOf = (f) => { const d = window.NSLive.modelDesc(f, t, (k) => window.NSI18n.has(k)); return { desc: d.text, note: d.note }; };
  const fmtDate = (iso) => new Date(iso).toLocaleDateString(NS.lang() === 'ru' ? 'ru-RU' : 'en-GB', { day: 'numeric', month: 'long' });

  // the choice goes to the env settings; false = the save failed (toast shown)
  async function save(envId, choice, extra) {
    try { await NS.store.saveEnvSettings(envId, Object.assign({ default_model: choice }, extra || {})); return true; }
    catch (e) { NS.shell.toast(NS.errHuman(NS.api.errText(e)), true); return false; }
  }
  // "Sonnet 5.5": the chosen version of the chosen family, else the newest one
  const titleOf = (f, cur) => { const v = (cur && cur.family === f.id && cur.version) || (f.versions && f.versions[0]); return v ? `${f.label} ${v}` : f.label; };
  const catFamilies = (envId) => { const cat = NS.store.s.catalogs.get(envId); return ((cat && cat.families) || []).filter((f) => f.id !== CLI_DEFAULT); };   // that entry is our own last row
  function versions(fam, pinned, pick) {
    const row = L.versionRow(fam, pinned, t);
    if (!row.items.length) return null;
    const wrap = h('div', { class: 'vers', role: 'radiogroup', 'aria-label': t('model.version') },
      row.items.map((it) => h('button', { type: 'button', class: 'v' + (it.on ? ' on' : ''), role: 'radio', 'aria-checked': String(it.on),
        onclick: (e) => { e.stopPropagation(); pick({ family: fam.id, version: it.value }); } }, it.pinned ? ic('i-lock', 'lk') : null, it.text)));
    if (row.newer) wrap.append(h('small', { class: 'newer', text: t('model.newer', { v: row.newer }) }));
    return wrap;
  }

  function card(opts) {
    const el = h('div', { class: 'mcard' + (opts.on ? ' on' : ''), role: 'radio', tabIndex: 0, 'aria-checked': String(!!opts.on), title: opts.note ? t('model.source', { note: opts.note }) : null,
      onclick: opts.onpick, onkeydown: (e) => { if ((e.key === 'Enter' || e.key === ' ') && e.target === el) { e.preventDefault(); opts.onpick(); } } },
    h('span', { class: 'radio' }),
    h('b', {}, opts.icon, opts.title, opts.pinned ? ic('i-lock', 'lk') : null, opts.isDefault ? ' ' : null, opts.isDefault ? h('small', { text: t('model.default') }) : null),
    opts.desc ? h('span', { class: 'd', text: opts.desc }) : null, opts.extra || null);
    return el;
  }

  // returns an element that keeps itself in sync with settings/catalog; onChange fires after a choice is saved
  function picker(envId, onChange) {
    const st = NS.store;
    const box = h('div', { class: 'picker' });
    let query = '';
    const search = h('input', { type: 'search', class: 'msearch', placeholder: t('model.search'), 'aria-label': t('model.search'),
      oninput: (e) => { query = e.target.value; draw(); search.focus(); } });
    async function choose(choice, extra) {
      if (!(await save(envId, choice, extra))) return;
      draw(); if (onChange) onChange(choice);
    }
    const versionsOf = (fam, pinned) => versions(fam, pinned, choose);
    function draw() {
      const cat = st.s.catalogs.get(envId), cur = choiceOf(envId);
      const fams = catFamilies(envId);
      const curFam = cur ? cur.family : (L.defaultFamily(cat) || {}).id;
      const cards = [];
      const famCard = (f) => card({ on: f.id === curFam, icon: vendorIcon(f.vendor, 'mlogo'), title: titleOf(f, cur && cur.family === f.id ? cur : null),
        pinned: f.id === curFam && cur && cur.version, isDefault: f.default, ...descOf(f),
        extra: f.id === curFam ? versionsOf(f, cur && cur.version) : null, onpick: () => choose({ family: f.id, version: null }) });
      const grouped = L.needsGrouping(fams);
      function drawCards() {
        const list = grouped ? L.filterFamilies(fams, query) : fams;
        clear(grid);
        if (grouped) for (const g of L.groupByVendor(list)) grid.append(h('h4', { class: 'mgroup' }, vendorIcon(g.vendor, 'mlogo'), g.vendor), ...g.families.map(famCard));
        else grid.append(...list.map(famCard));
        if (grouped && !list.length) grid.append(h('p', { class: 'hint', text: t('model.none_found') }));
        const customOn = cur && cur.family !== CLI_DEFAULT && !fams.some((f) => f.id === cur.family);
        const own = h('input', { type: 'text', class: 'mown', placeholder: t('model.own_ph'), 'aria-label': t('model.own'), value: customOn ? cur.family : '',
          onclick: (e) => e.stopPropagation(), onkeydown: (e) => { e.stopPropagation(); if (e.key === 'Enter' && own.value.trim()) { e.preventDefault(); useOwn(); } } });
        const useOwn = () => { const id = own.value.trim(); const known = st.envSettings(envId).custom_models || []; choose({ family: id, version: null }, { custom_models: known.includes(id) ? known : known.concat(id) }); };
        grid.append(card({ on: customOn, icon: ic('e-custom', 'mlogo'), title: t('model.own'), desc: t('model.own_d'),
          extra: h('div', { class: 'vers' }, own, h('button', { type: 'button', class: 'btn sm', text: t('model.use'), onclick: (e) => { e.stopPropagation(); if (own.value.trim()) useOwn(); } })),
          onpick: () => own.focus() }));
        grid.append(card({ on: !!cur && cur.family === CLI_DEFAULT, icon: ic('e-custom', 'mlogo'), title: t('model.cli_default'), desc: t('model.cli_default_d'),
          onpick: () => choose({ family: CLI_DEFAULT, version: null }) }));
      }
      const grid = h('div', { class: 'models' });
      drawCards();
      const foot = h('div', { class: 'catline' },
        cat ? h('span', { class: 'hint', text: t('model.list_from', { date: fmtDate(cat.at) }) }) : h('span', { class: 'hint', text: t('model.no_list') }),
        h('button', { type: 'button', class: 'linkb', text: t('model.refresh'), onclick: async (e) => {
          e.target.disabled = true;
          try { await st.loadCatalog(envId, true); } catch (er) { NS.shell.toast(NS.errHuman(NS.api.errText(er)), true); }
          draw();
        } }));
      clear(box).append(...(grouped ? [search] : []), grid, foot);
    }
    draw();
    box.redraw = draw;
    return box;
  }

  // the compact drop-down of the input field (F7): one line per model, versions of the current one as small chips, search when the list is long.
  // opts.onDone fires after a choice is saved (the caller closes the menu); returns the element, el.focusStart() puts the focus in.
  function menu(envId, opts) {
    const st = NS.store;
    const el = h('div', { class: 'mmenu-in' });
    let query = '', ownOpen = false;
    const pick = async (choice, extra) => { if (await save(envId, choice, extra) && opts.onDone) opts.onDone(choice); };
    const rows = () => [...el.querySelectorAll('.mrow')];
    function row(o) {
      const r = h('div', { class: 'mrow' + (o.on ? ' on' : ''), role: 'option', tabIndex: -1, 'aria-selected': String(!!o.on), title: o.tip || null,
        onclick: o.onpick, onkeydown: (e) => { if (e.key === 'Enter' && e.target === r) { e.preventDefault(); o.onpick(); } } },
      o.icon || h('span', { class: 'mlogo' }), h('span', { class: 'mn', text: o.title }), o.small ? h('small', { text: o.small }) : null,
      h('span', { class: 'mck', 'aria-hidden': 'true' }, o.on ? ic('i-check') : null));
      return r;
    }
    function draw() {
      const cat = st.s.catalogs.get(envId), cur = choiceOf(envId), fams = catFamilies(envId);
      const curFam = cur ? cur.family : (L.defaultFamily(cat) || {}).id;
      const grouped = L.needsGrouping(fams);
      const list = h('div', { class: 'mm-list', role: 'listbox', 'aria-label': t('model.title', { env: (st.env(envId) || {}).name || '' }) });
      const famRow = (f) => {
        const on = f.id === curFam, d = descOf(f);
        list.append(row({ on, icon: vendorIcon(f.vendor, 'mlogo'), title: titleOf(f, on ? cur : null), small: f.default ? t('model.default') : null,
          tip: d.desc + (d.note ? ' (' + t('model.source', { note: d.note }) + ')' : ''), onpick: () => pick({ family: f.id, version: null }) }));
        if (on) { const v = versions(f, cur && cur.version, pick); if (v) list.append(v); }
      };
      const found = grouped ? L.filterFamilies(fams, query) : fams;
      if (grouped) for (const g of L.groupByVendor(found)) { list.append(h('h4', { class: 'mgroup' }, vendorIcon(g.vendor, 'mlogo'), g.vendor)); g.families.forEach(famRow); }
      else found.forEach(famRow);
      if (grouped && !found.length) list.append(h('p', { class: 'hint', text: t('model.none_found') }));
      const customOn = !!cur && cur.family !== CLI_DEFAULT && !fams.some((f) => f.id === cur.family);
      const foot = h('div', { class: 'mm-foot', role: 'group' });
      foot.append(row({ on: !!cur && cur.family === CLI_DEFAULT, title: t('model.cli_default'), tip: t('model.cli_default_d'), onpick: () => pick({ family: CLI_DEFAULT, version: null }) }));
      if (ownOpen) {
        const own = h('input', { type: 'text', class: 'mown', placeholder: t('model.own_ph'), 'aria-label': t('model.own'), value: customOn ? cur.family : '',
          onkeydown: (e) => {
            if (e.key === 'Enter') { e.preventDefault(); const id = own.value.trim(); if (id) { const known = st.envSettings(envId).custom_models || []; pick({ family: id, version: null }, { custom_models: known.includes(id) ? known : known.concat(id) }); } }
            else if (e.key !== 'Escape') e.stopPropagation();
          } });
        foot.append(h('div', { class: 'mrow own' }, h('span', { class: 'mlogo' }), own));
        queueMicrotask(() => own.focus());
      } else foot.append(row({ on: customOn, title: t('model.own') + '…', small: customOn ? cur.family : null, tip: t('model.own_d'), onpick: () => { ownOpen = true; draw(); } }));
      const eff = effortRow(envId, draw);
      if (eff) foot.append(eff);
      foot.append(h('div', { class: 'catline' },
        h('span', { class: 'hint', text: cat ? t('model.list_from', { date: fmtDate(cat.at) }) : t('model.no_list') }),
        h('button', { type: 'button', class: 'linkb', text: t('model.refresh'), onclick: async (e) => {
          e.target.disabled = true;
          try { await st.loadCatalog(envId, true); } catch (er) { NS.shell.toast(NS.errHuman(NS.api.errText(er)), true); }
          draw();
        } })));
      const search = grouped ? h('input', { type: 'search', class: 'msearch', placeholder: t('model.search'), 'aria-label': t('model.search'), value: query,
        oninput: (e) => { query = e.target.value; draw(); const s2 = el.querySelector('.msearch'); s2.focus(); s2.setSelectionRange(query.length, query.length); } }) : null;
      clear(el).append(...(search ? [search] : []), list, foot);
    }
    // arrows walk the rows (the search field is the row above the first one), Enter picks, Esc is the caller's
    el.addEventListener('keydown', (e) => {
      if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
      const rs = rows(), search = el.querySelector('.msearch'), at = rs.indexOf(document.activeElement);
      if (document.activeElement && document.activeElement.classList.contains('mown')) return;
      e.preventDefault();
      const next = e.key === 'ArrowDown' ? at + 1 : at - 1;
      if (next < 0) { if (search) search.focus(); return; }
      if (rs[next]) { rs[next].focus(); rs[next].scrollIntoView({ block: 'nearest' }); }
    });
    draw();
    el.redraw = draw;
    el.focusStart = () => { const s = el.querySelector('.msearch'); (s || el).focus(); };
    el.tabIndex = -1;
    return el;
  }

  // palette rows (F7): "Модель: Opus 5.5" for each model of the env in focus; the current one carries a badge
  function commands(envId) {
    const st = NS.store;
    if (!envId || !st.s.catalogs.get(envId)) return [];
    const cur = choiceOf(envId), curFam = cur ? cur.family : (L.defaultFamily(st.s.catalogs.get(envId)) || {}).id;
    const mk = (id, name, family, on) => ({ id: 'model.' + id, label: t('cmd.model_row', { m: name }), keywords: 'модель model ' + id, group: 'cmd.g.env', badge: on ? t('model.current') : null,
      def: { run: () => save(envId, { family, version: null }) } });
    const curEff = effortOf(envId), levels = effortsOf(envId);
    const eff = (levels.length ? [''].concat(levels) : []).map((l) => ({ id: 'effort.' + (l || 'cli'),
      label: t('cmd.effort_row', { e: l ? t('effort.' + l) : t('effort.cli') }), keywords: 'мышление думать effort thinking reasoning ' + l, group: 'cmd.g.env',
      badge: l === curEff ? t('model.current') : null, def: { run: () => NS.store.saveEnvSettings(envId, { effort: l || null }).catch((er) => NS.shell.toast(NS.errHuman(NS.api.errText(er)), true)) } }));
    return catFamilies(envId).map((f) => mk(f.id, titleOf(f, null), f.id, f.id === curFam))
      .concat(mk('cli-default', t('model.cli_default'), CLI_DEFAULT, !!cur && cur.family === CLI_DEFAULT), eff);
  }

  NS.models = { label, buttonLabel, picker, menu, commands, CLI_DEFAULT };
})();
