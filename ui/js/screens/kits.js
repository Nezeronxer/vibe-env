// Kits (G25): "<Среда> уже использует твои <каталог>", export, import ("Показать" -> screen "Импорт: <имя>"), snapshots with "Откатить"
// and "Проверить обновление". A hook, an MCP server or a skill with scripts is shown whole (event, command, script text) and stays OFF until
// the person ticks it; "Установить" does nothing until a place is chosen; until it is pressed the disk is untouched (stories 206, 208).
// Everything that comes from a kit is shown as text (textContent), never as markup.
(function () {
  const NS = window.NS;
  const { h, ic, envIcon, clear } = NS;
  const L = window.NSLogic;
  const K = NS.settingsKit;
  const t = (k, p) => NS.t(k, p);
  const st = NS.store, S = st.s;
  const call = (c, a) => NS.api.call(c, a);
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
  const ACK = 'ns.kitack';
  const acked = () => { try { return new Set(JSON.parse(NS.storage.get(ACK) || '[]')); } catch (e) { return new Set(); } };
  const ack = (id) => { const a = acked(); a.add(id); NS.storage.set(ACK, JSON.stringify([...a])); };
  const fmtDate = (iso) => new Date(iso).toLocaleString(NS.lang() === 'ru' ? 'ru-RU' : 'en-GB', { day: 'numeric', month: 'long', hour: '2-digit', minute: '2-digit' });
  const rawErr = (e) => (e instanceof Error ? e.message : e);          // KitError arrives as {kind, ...} or as its JSON text
  const refusalText = (raw, envName) => {
    const r = L.kitRefusal(raw);
    return r.key === 'unknown' ? NS.errHuman(r.detail) : t('kit.err.' + r.key, { detail: r.detail, env: envName });
  };
  const srcOf = (src) => (src ? Object.values(src)[0] : '');           // KitSource {repo|folder|zip: value} -> the value
  const projPath = (id) => ((S.projects.find((p) => p.id === id) || {}).path) || '';

  // what the page remembers between redraws
  const ks = { env: null, ticked: null, dest: 'folder', source: '', refused: null, kept: null };

  K.def('kits', { env: true, live: ['envs'], build(env, fx) {
    if (ks.env !== env.id) Object.assign(ks, { env: env.id, ticked: null, dest: 'folder', source: '', refused: null, kept: null });
    const box = h('div', { class: 'kits' });
    const prefill = fx.ctx.params.source;
    if (prefill && !ks.source) { ks.source = prefill; delete fx.ctx.params.source; setTimeout(() => fx.alive.v && show(), 0); }

    // ---------- import ----------
    const srcIn = h('input', { type: 'text', class: 'wide', placeholder: 'https://github.com/…/my-setup', 'aria-label': t('kits.from'), value: ks.source, spellcheck: false });
    srcIn.oninput = () => { ks.source = srcIn.value; };
    const refusal = h('div', { class: 'refusal' });
    function drawRefusal() {
      clear(refusal);
      const r = ks.refused;
      if (!r) return;
      const k = L.kitRefusal(r.err);
      if (k.key === 'wrong_env') {
        const other = st.env(k.env);
        refusal.append(h('div', { class: 'srows' }, h('div', { class: 'srow foreign' }, h('div', {}, h('b', { text: r.source }), h('span', { class: 'd', text: refusalText(r.err, env.name) })),
          h('div', { class: 'ctl' }, other ? h('button', { type: 'button', class: 'btn', text: t('kits.open_in', { env: other.name }), onclick: () => fx.ctx.nav('kits', { env: other.id, source: r.source }) }) : null))));
      } else {
        refusal.append(h('div', { class: 'srows' }, h('div', { class: 'srow refused' }, h('div', {}, h('b', { text: r.source }), h('span', { class: 'd', role: 'alert', text: refusalText(r.err, env.name) })))));
      }
    }
    async function show(srcOverride) {
      const src = L.kitSourceOf(srcOverride || srcIn.value);
      if (!src) { srcIn.focus(); return; }
      ks.source = srcOf(src); srcIn.value = ks.source; ks.refused = null; drawRefusal();
      try {
        const plan = await call('kits_preview', { env: env.id, source: src, scope: 'global' });
        if (fx.alive.v) fx.ctx.nav('import', { env: env.id, plan, source: src });
      } catch (e) { ks.refused = { source: srcOf(src), err: rawErr(e) }; if (fx.alive.v) fx.redraw(); }     // the page may have been redrawn meanwhile: draw from the remembered state
    }
    const pickZip = async () => {
      const d = window.__TAURI__ && window.__TAURI__.dialog;
      try { const p = d && d.open ? await d.open({ multiple: false, filters: [{ name: 'zip', extensions: ['zip'] }] }) : null; const v = Array.isArray(p) ? p[0] : p; if (v) show(v); } catch (e) { /* the dialog was closed */ }
    };
    const importG = K.group(t('kits.import'), [
      K.row(t('kits.from'), t('kits.from_d'), srcIn, h('button', { type: 'button', class: 'btn', text: t('kits.folder'), onclick: async () => { const p = await NS.projects.pickFolder(); if (p) show(p); } }),
        h('button', { type: 'button', class: 'btn', text: t('kits.zip'), onclick: pickZip }), h('button', { type: 'button', class: 'btn primary', text: t('kits.show'), onclick: () => show() })),
    ]);
    srcIn.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); show(); } });

    // ---------- loaded parts ----------
    const top = h('div'), exportBox = h('div'), snapBox = h('div');
    box.append(top, exportBox, h('div', { class: 'sgrp' }, h('h2', { text: t('kits.import') }), importG.querySelector('.srows'), refusal), snapBox);
    drawRefusal();
    (async () => {
      let inv, snaps = [];
      try { inv = await call('kits_inventory', { env: env.id, scope: 'global' }); } catch (e) { if (fx.alive.v) clear(top).append(h('p', { class: 'empty-note', text: NS.errHuman(NS.api.errText(e)) })); return; }
      try { snaps = (await call('kits_snapshots', { env: env.id })) || []; } catch (e) { snaps = []; }
      if (!fx.alive.v) return;
      drawTop(inv); drawExport(inv); drawSnaps(snaps);
    })();

    function drawTop(inv) {
      const groups = inv.groups || [];
      const usable = (id) => { const g = groups.find((x) => x.group === id); return g && !L.kitGroupOff(g) ? g.count : 0; };
      const F = (id) => [t('kits.f_' + id + '_1'), t('kits.f_' + id + '_2'), t('kits.f_' + id + '_5')];
      const parts = [];
      if (usable('skills')) parts.push(`${usable('skills')} ${L.plural(usable('skills'), F('skills'), NS.lang())}`);
      if (usable('rules')) parts.push(t('kits.rules'));
      if (usable('hooks')) parts.push(`${usable('hooks')} ${L.plural(usable('hooks'), F('hooks'), NS.lang())}`);
      if (usable('mcp')) parts.push(`${usable('mcp')} MCP`);
      clear(top);
      if (groups.length && groups.every((g) => L.kitGroupOff(g) === 'unverified')) top.append(h('div', { class: 'kitnow' }, envIcon(env.custom ? 'custom' : env.id), h('p', {}, h('b', { text: t('kits.unverified', { env: env.name }) }))));
      else if (inv.found && parts.length) top.append(h('div', { class: 'kitnow' }, envIcon(env.custom ? 'custom' : env.id), h('p', {}, h('b', { text: t('kits.uses', { env: env.name, dir: inv.dir || env.kitDir || '' }) + ' ' }), parts.join(', ') + ' - ' + t('kits.no_import'))));
      if ((inv.dangerous || []).length) top.append(h('p', { class: 'kitnote', text: t('kits.dangerous', { list: inv.dangerous.join(', ') }) }));
    }

    function drawExport(inv) {
      const groups = inv.groups || [];
      if (!ks.ticked) ks.ticked = new Set(groups.filter((g) => !L.kitGroupOff(g)).map((g) => g.group));
      const go = h('button', { type: 'button', class: 'btn primary', text: t('kits.export_go'), onclick: async () => {
        const sel = L.exportSelection(groups, ks.ticked);
        const dir = await NS.projects.pickFolder();
        if (!dir) return;
        const name = env.id.replace(/[^a-z0-9]+/gi, '-') + '-kit', path = dir.replace(/\/$/, '') + '/' + name;
        const dest = ks.dest === 'zip' ? { zip: path + '.zip' } : ks.dest === 'git' ? { git: path } : { folder: path };
        go.disabled = true;
        try {
          const r = await call('kits_export', { env: env.id, selection: { name, groups: sel, scope: 'global' }, dest });
          NS.shell.toast(t('kits.exported_n', { path: (r && r.dest) || path, n: (r && r.secrets_replaced) || 0 }));
        } catch (e) { NS.shell.toast(refusalText(rawErr(e), env.name), true); }
        go.disabled = L.exportSelection(groups, ks.ticked).length === 0;
      } });
      const sync = () => { go.disabled = L.exportSelection(groups, ks.ticked).length === 0; };
      const list = groups.map((g) => {
        const off = L.kitGroupOff(g);
        const cb = h('input', { type: 'checkbox', checked: !off && ks.ticked.has(g.group), disabled: !!off, 'aria-label': t('kit.g.' + g.group),
          onclick: (e) => e.stopPropagation(), onchange: () => { if (cb.checked) ks.ticked.add(g.group); else ks.ticked.delete(g.group); sync(); } });
        const { shown, more } = L.capList(g.items || [], 6);
        return h('details', { class: 'kg' + (off ? ' off' : '') }, h('summary', { onclick: (e) => { if (off) e.preventDefault(); } }, cb, h('b', { text: t('kit.g.' + g.group) }),
          off === 'unverified' ? h('small', { text: t('kits.layout_unverified', { env: env.name }) }) : null, h('small', { text: String(g.count) })),
        h('ul', {}, shown.map((x) => h('li', { text: x })), more > 0 ? h('li', { class: 'hint', text: t('sum.more', { n: more }) }) : null));
      });
      const seg = K.segment([['folder', t('kits.d_folder')], ['zip', 'zip'], ['git', t('kits.d_git')]], ks.dest, t('kits.where'), async (v) => { ks.dest = v; });
      sync();
      clear(exportBox).append(h('div', { class: 'sgrp' }, h('h2', { text: t('kits.export') }), h('div', { class: 'srows kit' }, list, K.row(t('kits.where'), t('kits.export_d'), seg, go))));
    }

    function drawSnaps(snaps) {
      clear(snapBox);
      if (ks.kept) snapBox.append(h('div', { class: 'kitstrip' }, ic('i-warn'), h('p', { text: t('kits.kept', { list: ks.kept.join('; ') }) }), h('button', { type: 'button', class: 'btn sm', text: t('notice.ok'), onclick: (e) => { ks.kept = null; e.currentTarget.closest('.kitstrip').remove(); } })));
      for (const x of snaps.filter((y) => y.state === 'interrupted' && !acked().has(y.id))) snapBox.append(h('div', { class: 'kitstrip' }, ic('i-warn'), h('p', { text: t('kits.interrupted', { name: x.kit_name }) }), h('button', { type: 'button', class: 'btn sm', text: t('notice.ok'), onclick: (e) => { ack(x.id); e.currentTarget.closest('.kitstrip').remove(); } })));
      const real = snaps.filter((x) => ['done', 'not_applied'].includes(x.state));
      if (!real.length) return;
      const rows = real.map((x) => {
        const bar = [];
        if (x.source && x.source.repo) bar.push(h('button', { type: 'button', class: 'btn sm', text: t('kits.check_update'), onclick: async (e) => {
          e.currentTarget.disabled = true;
          try {
            const installed = { env: env.id, kit_name: x.kit_name, source: x.source, commit: x.commit, scope: x.scope, snapshot: x.id };
            const plan = await call('kits_check_update', { installed });
            fx.ctx.nav('import', { env: env.id, plan, source: x.source, update: true });
          } catch (er) { NS.shell.toast(refusalText(rawErr(er), env.name), true); e.currentTarget.disabled = false; }
        } }));
        bar.push(h('button', { type: 'button', class: 'btn sm', text: t('kits.rollback'), onclick: async () => {
          if (!(await NS.projects.confirm(t('kits.rollback'), t('kits.rollback_text', { name: x.kit_name }), t('kits.rollback')))) return;
          try {
            const rep = (await call('kits_rollback', { id: x.id })) || {};
            const kept = [].concat(rep.kept_changed || [], rep.kept_files || []);
            ks.kept = kept.length ? kept : null;
            NS.shell.toast(t('kits.rolled_back', { name: x.kit_name }));
            fx.redraw();
          } catch (e) { NS.shell.toast(refusalText(rawErr(e), env.name), true); }
        } }));
        const off = (x.held || []).filter((d) => !d.enabled);
        const dis = off.length ? h('div', { class: 'dis' }, off.map((d) => h('div', {}, h('span', { class: 'tag', text: t('kits.off') }), h('b', { text: d.label }),
          h('span', { class: 'grow' }), h('button', { type: 'button', class: 'btn sm', text: t('kits.enable'), onclick: async (e) => {
            e.currentTarget.disabled = true;
            try { await call('kits_enable', { env: env.id, item: { snapshot: x.id, key: d.key } }); fx.redraw(); } catch (er) { NS.shell.toast(refusalText(rawErr(er), env.name), true); e.currentTarget.disabled = false; }
          } })))) : null;
        const warn = x.state === 'not_applied' ? h('p', { class: 'dis', role: 'alert', text: t('kits.not_applied', { env: env.name }) }) : null;
        return h('li', {}, h('div', {}, h('b', { text: x.kit_name }), h('span', { class: 'd', text: ' ' + fmtDate(x.at) + ' · ' + srcOf(x.source) })), h('div', { class: 'ctl' }, bar), warn, dis);
      });
      snapBox.append(h('div', { class: 'sgrp' }, h('h2', { text: t('kits.snaps') }), h('ul', { class: 'srows snaps' }, rows)));
    }
    return [box];
  } });

  // ---------- "Импорт: <имя>" ----------
  K.def('import', { env: true, lead: false, title: (env, fx) => t(fx.extra.update ? 'import.update' : 'import.title', { name: (fx.extra.plan || {}).kit_name || '' }),
    build(env, fx) {
      let plan = fx.extra.plan;
      const source = fx.extra.source;
      if (!plan) return [h('p', { class: 'empty-note', text: t('import.none') }), h('button', { type: 'button', class: 'btn', text: t('kits.back'), onclick: () => fx.ctx.nav('kits', { env: env.id }) })];
      const root = h('div', { class: 'importpage' });
      let scope = null, projectId = (S.projects.find((p) => p.id === S.lastProject) || S.projects[0] || {}).id || null;     // scope: null | 'global' | 'project' - nothing is chosen for the person
      const coreScope = () => (scope === 'project' ? { project: projPath(projectId) } : 'global');
      const enabled = new Set();                    // every held entry is off until ticked (story 208)
      const secrets = new Map();
      const srcLabel = source ? srcOf(source) : '';

      let installedId = null;                       // once installed the plan is spent: "Установить" never runs the same plan again
      async function install(btn) {
        btn.disabled = true;
        try {
          for (const [name, value] of secrets) {
            if (!value) continue;
            const r = await call('set_secret', { env: env.id, name, value });          // SecretSaved: "keyring" is silent, {file: {path, warning}} is shown
            const f = r && r.file;
            if (f) NS.shell.toast(t('import.secret_file', { name, path: f.path || '', warning: f.warning || '' }), true);
          }
          installedId = await call('kits_install', { plan, choices: { skip: [] } });
        } catch (e) { NS.shell.toast(refusalText(rawErr(e), env.name), true); btn.disabled = false; return; }
        try {
          for (const key of L.enableKeys(plan, enabled)) await call('kits_enable', { env: env.id, item: { snapshot: installedId, key } });     // ticked ones are switched on from the snapshot
          NS.shell.toast(t('import.done', { name: plan.kit_name }));
          fx.ctx.nav('kits', { env: env.id });
        } catch (e) { NS.shell.toast(refusalText(rawErr(e), env.name), true); if (fx.alive.v) draw(); }     // installed, switching on failed: the rest is done from the snapshot list
      }

      function draw() {
        clear(root);
        root.append(h('p', { class: 'lead', text: fx.extra.update ? t('import.update_lead') : t('import.lead', { env: env.name, source: srcLabel }) }));

        // where to put it: nothing is chosen for the person
        const scopeRow = (id, title, dir, extra) => h('label', { class: 'scope' }, h('div', {}, h('b', { text: title }), h('span', { class: 'd', text: dir })),
          h('input', { type: 'radio', name: 'scope', checked: scope === id, onchange: async () => { scope = id; await repreview(); } }), extra || null);
        const gDir = env.kitDir || '~';
        const projSel = S.projects.length > 1 ? h('select', { 'aria-label': t('import.project'), onchange: async (e) => { projectId = e.target.value; if (scope === 'project') await repreview(); else draw(); } },
          S.projects.map((p) => h('option', { value: p.id, text: p.name, selected: p.id === projectId }))) : null;
        root.append(h('div', { class: 'sgrp' }, h('h2', { text: t('import.where') }), h('div', { class: 'srows' },
          scopeRow('global', t('import.global'), scope === 'global' && plan.target_dir ? plan.target_dir : gDir),
          scopeRow('project', t('import.project'), scope === 'project' && plan.target_dir ? plan.target_dir : projPath(projectId), projSel ? h('div', { class: 'note' }, projSel) : null),
          scope === 'project' && plan.uncommitted_in_project ? h('div', { class: 'scope' }, h('p', { class: 'note', text: t('import.uncommitted') })) : null)));

        // what will change: + added, ~ replaced (old one goes to a snapshot), = already there
        const lines = L.groupChanges(plan.entries || []);
        const note = (l) => (l.cls === 'add' ? t(l.names.length > 1 ? 'chg.add_n' : 'chg.add_1') : l.cls === 'same' ? t('chg.same') : t(l.names.length > 1 ? 'chg.rep_n' : 'chg.rep_1'));
        const items = lines.map((l) => {
          const open = h('div', { class: 'dfbox', hidden: true }, l.diff ? NS.taskCard.diffBox({ lines: l.diff }, { worktree: '', project_id: '' }, l.names.join(', ')) : null);
          const more = l.cls === 'replace' && l.diff ? h('button', { type: 'button', class: 'linkb', text: t('chg.diff'), onclick: () => { open.hidden = !open.hidden; } }) : null;
          return h('li', { class: l.cls }, h('span', { class: 'sg', text: l.sign, 'aria-label': t('chg.sign_' + l.cls) }), h('b', { text: t('kit.g.' + l.group) + ': ' + l.names.join(', ') }),
            h('small', {}, note(l), more ? ' · ' : '', more), open);
        });
        root.append(h('div', { class: 'sgrp' }, h('h2', { text: t('import.changes') }), items.length ? h('ul', { class: 'srows chg' }, items) : h('p', { class: 'empty-note', text: fx.extra.update ? t('import.no_updates') : t('import.nothing') })));

        // variables the kit needs: typed here, they go to the key store and never into a file
        const need = (plan.env_vars || []).filter((v) => v.missing);
        if (need.length) {
          root.append(h('div', { class: 'sgrp' }, h('h2', { text: t('import.vars') }), h('div', { class: 'srows' }, need.map((v) => {
            const inp = h('input', { type: 'password', placeholder: t('import.value'), 'aria-label': v.name, autocomplete: 'off', value: secrets.get(v.name) || '', oninput: () => secrets.set(v.name, inp.value) });
            return h('div', { class: 'secret' }, h('div', {}, h('b', { text: v.name }), h('span', { class: 'd', text: t('import.var_d', { who: (v.used_by || []).join(', ') }) })), inp);
          })), h('p', { class: 'kitnote', text: t('kits.terminal_note') })));
        }

        // code that would run by itself: shown whole (command and the text of every script), off until ticked
        const heldBy = (kind) => L.heldEntries(plan).filter((x) => x.held.kind === kind);
        const warn = h('small', {}, ic('i-warn'), t('import.runs_alone'));
        const heldBox = (kind, titleKey) => {
          const list = heldBy(kind);
          if (!list.length) return;
          root.append(h('div', { class: 'sgrp' }, h('h2', {}, t(titleKey), h('span', { class: 'hint', text: ' ' + t('import.hooks_d') })), h('div', { class: 'srows' }, list.map((x) =>
            h('label', { class: 'hook' }, h('input', { type: 'checkbox', checked: enabled.has(x.key), onchange: (e) => { if (e.target.checked) enabled.add(x.key); else enabled.delete(x.key); } }),
              h('div', {}, h('b', {}, x.held.event || x.label, kind === 'scripted_skill' ? h('span', { class: 'tag', text: ' ' + t('import.has_scripts') }) : null),
                h('code', { text: x.held.text }), (x.held.scripts || []).map((sc) => [h('small', { class: 'hint', text: sc.path }), h('pre', { 'aria-label': t('import.script'), text: sc.text })]), warn.cloneNode(true)))))));
        };
        heldBox('hook', 'import.hooks'); heldBox('mcp', 'import.mcp'); heldBox('scripted_skill', 'import.scripts'); heldBox('command', 'import.held_command');
        if ((plan.skipped || []).length) root.append(h('div', { class: 'sgrp' }, h('h2', { text: t('import.skipped') }), h('ul', { class: 'srows chg' }, plan.skipped.map((x) =>
          h('li', { class: 'same' }, h('span', { class: 'sg', text: '-' }), h('b', { text: x.path }), h('small', { text: t('kit.skip.' + x.reason) })))))) ;

        // nothing is written before this button, and it waits for a choice
        const go = installedId ? h('button', { type: 'button', class: 'btn primary', text: t('import.installed'), onclick: () => fx.ctx.nav('kits', { env: env.id }) })
          : h('button', { type: 'button', class: 'btn primary', text: t('import.install'), disabled: !L.canInstall(plan, scope), onclick: (e) => install(e.currentTarget) });
        root.append(h('div', { class: 'sgrp kitgo' }, go, h('button', { type: 'button', class: 'btn', text: t('dlg.cancel'), onclick: () => fx.ctx.nav('kits', { env: env.id }) }),
          h('span', { class: 'hint', text: installedId ? t('import.installed_d') : scope ? t('import.snapshot') : t('import.choose_where') })));
      }
      // "what changes" depends on the place: ask the core again when the person chooses it
      async function repreview() {
        if (!source) return draw();
        try { plan = await call('kits_preview', { env: env.id, source, scope: coreScope() }); } catch (e) { NS.shell.toast(refusalText(rawErr(e), env.name), true); }
        draw();
      }
      draw();
      return [root];
    } });

  NS.registry.register({ id: 'kits', layout: 'full', keepEnv: true, railId: 'settings', mount(root, ctx) { return K.frame(root, ctx, 'kits'); } });
  NS.registry.register({ id: 'import', layout: 'full', keepEnv: 'kits', railId: 'settings', mount(root, ctx) { return K.frame(root, ctx, 'import', { plan: ctx.params.plan, source: ctx.params.source, update: ctx.params.update }); } });
})();
