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
  // story 240: the core sends model.desc.<id>; "(… docs, …; не сверено с CLI)" is only a tooltip
  const descOf = (f) => { const d = window.NSLive.modelDesc(f, t, (k) => window.NSI18n.has(k)); return { desc: d.text, note: d.note }; };
  const fmtDate = (iso) => new Date(iso).toLocaleDateString(NS.lang() === 'ru' ? 'ru-RU' : 'en-GB', { day: 'numeric', month: 'long' });

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
      try { await st.saveEnvSettings(envId, Object.assign({ default_model: choice }, extra || {})); } catch (e) { NS.shell.toast(NS.errHuman(NS.api.errText(e)), true); return; }
      draw(); if (onChange) onChange(choice);
    }
    function versions(fam, pinned) {
      const row = L.versionRow(fam, pinned, t);
      if (!row.items.length) return null;
      const wrap = h('div', { class: 'vers', role: 'radiogroup', 'aria-label': t('model.version') },
        row.items.map((it) => h('button', { type: 'button', class: 'v' + (it.on ? ' on' : ''), role: 'radio', 'aria-checked': String(it.on),
          onclick: (e) => { e.stopPropagation(); choose({ family: fam.id, version: it.value }); } }, it.pinned ? ic('i-lock', 'lk') : null, it.text)));
      if (row.newer) wrap.append(h('small', { class: 'newer', text: t('model.newer', { v: row.newer }) }));
      return wrap;
    }
    function draw() {
      const cat = st.s.catalogs.get(envId), cur = choiceOf(envId);
      const fams = ((cat && cat.families) || []).filter((f) => f.id !== CLI_DEFAULT);   // that entry is drawn as our own last card
      const curFam = cur ? cur.family : (L.defaultFamily(cat) || {}).id;
      const cards = [];
      const titleOf = (f) => { const v = (f.id === curFam && cur && cur.version) || (f.versions && f.versions[0]); return v ? `${f.label} ${v}` : f.label; };
      const famCard = (f) => card({ on: f.id === curFam, icon: vendorIcon(f.vendor, 'mlogo'), title: titleOf(f),
        pinned: f.id === curFam && cur && cur.version, isDefault: f.default, ...descOf(f),
        extra: f.id === curFam ? versions(f, cur && cur.version) : null, onpick: () => choose({ family: f.id, version: null }) });
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

  NS.models = { label, picker, CLI_DEFAULT };
})();
