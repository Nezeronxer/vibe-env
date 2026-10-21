// Settings > "Проекты" (story 200): folder, main branch, check / setup / run commands, files copied into a task copy, finish mode,
// "Спрашивать перед push / PR", the GitHub block (origin, gh login, ns/* branches) and "Убрать проект" (off while tasks run).
// The screen is drawn in the style of the other sections; the mockup has no such section.
(function () {
  const NS = window.NS;
  const { h, ic, clear } = NS;
  const L = window.NSLogic;
  const K = NS.settingsKit;
  const t = (k, p) => NS.t(k, p);
  const st = NS.store, S = st.s;
  const call = (c, a) => NS.api.call(c, a);
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
  let selId = null;
  const gh = new Map();          // project id -> github_info (or null while unknown)

  async function save(p, patch) {
    const next = Object.assign({}, p, patch);
    K.touch();                                   // the echo of our own change must not redraw the form under the cursor
    await call('update_project', { project: next });
    const i = S.projects.findIndex((x) => x.id === p.id);
    if (i >= 0) S.projects[i] = next;
    return next;
  }
  const busy = (p) => st.s.tasks && [...st.s.tasks.values()].some((x) => x.project_id === p.id && ['wait', 'run'].includes(L.kindOf(x)));
  const cmdCheck = (raw) => ({ ok: true, value: raw.trim() || null });

  K.def('projects', { live: ['project'], build(env, fx) {
    const nodes = [];
    if (!S.projects.length) return [h('p', { class: 'empty-note', text: t('proj.none') })];
    if (!selId || !st.project(selId)) selId = S.lastProject && st.project(S.lastProject) ? S.lastProject : S.projects[0].id;
    const p = st.project(selId);
    if (S.projects.length > 1) {
      const sel = h('select', { 'aria-label': t('proj.pick') }, S.projects.map((x) => h('option', { value: x.id, text: x.name, selected: x.id === p.id })));
      sel.onchange = () => { selId = sel.value; fx.redraw(); };
      nodes.push(h('div', { class: 'projsw' }, h('span', { class: 'field-l', text: t('proj.pick') }), sel));
    }
    if (!S.repos.has(p.id)) st.loadRepo(p.id).then(() => fx.alive.v && fx.redraw());
    const missing = S.repos.has(p.id) && !S.repos.get(p.id);
    const reselect = async () => {
      const path = await NS.projects.pickFolder();
      if (!path) return;
      try { await save(p, { path }); S.repos.delete(p.id); await st.loadRepo(p.id); if (!S.repos.get(p.id)) throw new Error(t('hint.path_missing')); fx.redraw(); } catch (e) { fail(e); }
    };
    if (missing) nodes.push(h('div', { class: 'warnbox' }, ic('i-warn'), h('p', {}, h('b', { text: t('proj.missing', { path: p.path }) }), ' ', t('proj.missing_d')), h('button', { type: 'button', class: 'btn sm', text: t('proj.reselect'), onclick: reselect })));

    // folder, main branch, commands
    const pathRow = K.textRow({ title: t('proj.path'), value: p.path, check: (raw) => (raw.trim() ? { ok: true, value: raw.trim() } : { ok: false, hint: t('hint.path_empty') }),
      extra: h('button', { type: 'button', class: 'btn sm', text: t('proj.reselect'), onclick: reselect }),
      apply: async (v) => {
        const before = p.path;
        await save(p, { path: v }); S.repos.delete(p.id); await st.loadRepo(p.id);
        if (!S.repos.get(p.id)) { await save(p, { path: before }); S.repos.delete(p.id); throw new Error(t('hint.path_missing')); }
        fx.redraw();
      } });
    nodes.push(K.group(p.name, [
      pathRow,
      K.textRow({ title: t('proj.branch'), desc: t('proj.branch_d'), value: p.base_branch, check: (raw) => (raw.trim() ? { ok: true, value: raw.trim() } : { ok: false, hint: t('hint.empty') }), apply: (v) => save(p, { base_branch: v }) }),
      K.textRow({ title: t('proj.check'), desc: t('proj.check_d'), value: p.check_cmd || '', placeholder: t('proj.auto'), check: cmdCheck, apply: (v) => save(p, { check_cmd: v }) }),
      K.textRow({ title: t('proj.setup'), desc: t('proj.setup_d'), value: p.setup_cmd || '', placeholder: t('proj.auto'), check: cmdCheck, apply: (v) => save(p, { setup_cmd: v }) }),
      K.textRow({ title: t('proj.run'), desc: t('proj.run_d'), value: p.run_cmd || '', placeholder: t('proj.auto'), check: cmdCheck, apply: (v) => save(p, { run_cmd: v }) }),
    ]));

    // extra checks: name + command
    const checks = (p.extra_checks || []).map(([name, cmd], i) => h('div', { class: 'xrow' }, h('code', { text: name }), h('span', { class: 'hint', text: cmd }), h('span', { class: 'grow' }),
      h('button', { type: 'button', class: 'linkb', text: t('set.remove'), onclick: async () => { try { await save(p, { extra_checks: p.extra_checks.filter((_, j) => j !== i) }); fx.redraw(); } catch (e) { fail(e); } } })));
    const nm = h('input', { type: 'text', placeholder: t('proj.check_name'), 'aria-label': t('proj.check_name') }), cm = h('input', { type: 'text', placeholder: t('proj.check_cmd'), 'aria-label': t('proj.check_cmd') });
    const addCheck = async () => { if (!nm.value.trim() || !cm.value.trim()) return; try { await save(p, { extra_checks: (p.extra_checks || []).concat([[nm.value.trim(), cm.value.trim()]]) }); fx.redraw(); } catch (e) { fail(e); } };
    // files copied into a task copy (never committed, story 49)
    const files = h('div', { class: 'chips' }, (p.copy_files || []).map((f) => h('span', { class: 'chip' }, f,
      h('button', { type: 'button', 'aria-label': t('set.remove') + ' ' + f, text: '×', onclick: async () => { try { await save(p, { copy_files: p.copy_files.filter((x) => x !== f) }); fx.redraw(); } catch (e) { fail(e); } } }))));
    const cf = h('input', { type: 'text', class: 'wide', placeholder: '.env*', 'aria-label': t('proj.copy') });
    const addCopy = async () => { const v = cf.value.trim(); if (!v || (p.copy_files || []).includes(v)) return; try { await save(p, { copy_files: (p.copy_files || []).concat(v) }); fx.redraw(); } catch (e) { fail(e); } };
    cf.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); addCopy(); } });
    nodes.push(K.group(t('proj.more'), [
      K.row(t('proj.extra'), t('proj.extra_d'), h('div', { class: 'xlist' }, checks, h('div', { class: 'xrow' }, nm, cm, h('button', { type: 'button', class: 'btn sm', text: t('set.add'), onclick: addCheck })))),
      K.row(t('proj.copy'), t('proj.copy_d'), files, cf, h('button', { type: 'button', class: 'btn sm', text: t('set.add'), onclick: addCopy })),
      K.selectRow({ title: t('proj.finish'), desc: t('proj.finish_d'), value: p.finish, options: [['local', t('proj.f_local')], ['push', t('proj.f_push')], ['pr', t('proj.f_pr')]].map(([value, text]) => ({ value, text })), apply: (v) => save(p, { finish: v }) }),
      K.toggleRow({ title: t('proj.ask'), desc: t('proj.ask_d'), on: p.ask_outward, apply: (v) => save(p, { ask_outward: v }) }),
    ]));

    // GitHub
    if (!gh.has(p.id)) { gh.set(p.id, null); call('github_info', { projectId: p.id }).then((v) => gh.set(p.id, v || {})).catch(() => gh.set(p.id, {})).then(() => fx.alive.v && fx.redraw()); }
    const gi = gh.get(p.id);
    const origin = gi && (gi.repo_url || gi.repoUrl), user = gi && (gi.gh_user || gi.ghUser), hint = gi && (gi.gh_hint || gi.ghHint);
    const branches = (gi && gi.branches) || [];
    const mergedN = branches.filter((b) => b.merged).length;
    const act = (title, cmd, ok, danger) => h('button', { type: 'button', class: 'btn sm' + (danger ? ' danger' : ''), text: title, onclick: async (e) => {
      const b = e.currentTarget; b.disabled = true;
      try { await call(cmd, { projectId: p.id }); NS.shell.toast(ok); gh.delete(p.id); S.repos.delete(p.id); fx.redraw(); } catch (er) { fail(er); b.disabled = false; }
    } });
    nodes.push(K.group(t('proj.github'), [
      K.row(t('proj.origin'), '', gi ? h('span', { class: 'hint', text: origin || t('proj.no_origin') }) : h('span', { class: 'hint', text: t('proj.loading') })),
      K.row(t('proj.gh'), '', gi ? (user ? h('span', { class: 'pill-s ok' }, ic('i-check'), t('proj.gh_as', { user })) : h('span', { class: 'hint', text: hint || t('proj.gh_none') })) : h('span', { class: 'hint', text: t('proj.loading') })),
      K.row(t('proj.ns'), t('proj.ns_d'), branches.length ? h('ul', { class: 'gh-br' }, branches.map((b) => h('li', {}, h('span', { text: b.name }), b.merged ? h('span', { class: 'm', text: t('proj.merged') }) : null))) : h('span', { class: 'hint', text: t('proj.no_branches') })),
      K.row(t('proj.actions'), '', act(t('proj.push_main'), 'push_base', t('proj.pushed')), act(t('proj.refresh'), 'pull_base', t('proj.refreshed')),
        (() => { const b = act(t('proj.del_merged', { n: mergedN }), 'delete_merged_branches', t('proj.deleted')); b.disabled = !mergedN; return b; })()),
    ]));

    // remove
    const running = busy(p);
    const rm = h('button', { type: 'button', class: 'btn danger', text: t('proj.remove'), disabled: running, title: running ? t('proj.remove_busy') : '', onclick: async () => {
      if (!(await NS.projects.confirm(t('proj.remove'), t('proj.remove_text', { name: p.name }), t('proj.remove')))) return;
      try { await call('remove_project', { id: p.id }); S.projects = await call('list_projects'); selId = null; S.lastProject = null; fx.redraw(); } catch (e) { fail(e); }
    } });
    nodes.push(K.group('', [K.row(t('proj.remove'), running ? t('proj.remove_busy') : t('proj.remove_d'), rm)]));
    return nodes;
  } });
})();
