// The practices of G53 in the window (stories 305-312), through the extension points of task 22:
//   paneHeader 'ctx'  - the context bar of an agent session; from 70% a quiet hint and "Сжать" with "что сохранить" → compact;
//   details 'handoff' - the hand-off note, the ports and the database of the task copy;
//   settings 'roles4' - models of the service roles (Watcher, queue Coordinator, prompt rewrite) and the allowed tools of roles;
//   settings 'save4'  - "Сжимать вывод" and the project flag "Изолировать порты и базу";
//   NS.practices.taxBlock(env, fx) - the startup tax of an env, drawn in Settings → Среды (updates.js).
(function () {
  const NS = window.NS;
  const { h, clear, setVars } = NS;
  const t = (k, p) => NS.t(k, p);
  const S = NS.store.s;
  const L = window.NSLogic;
  const call = (c, a) => NS.api.call(c, a);
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
  const WARN_PCT = 70;
  const LIVE = ['preparing', 'working', 'checking', 'reviewing', 'merging', 'needs_human', 'paused_limit'];
  const num = (n) => Number(n || 0).toLocaleString(window.NSI18n.lang() === 'en' ? 'en-US' : 'ru-RU');

  // ---------- context bar (305) ----------
  function ctxSlot(el, pane) {
    let pop = null;
    const closePop = () => { if (pop) { pop.remove(); pop = null; document.removeEventListener('pointerdown', onDoc, true); } };
    const onDoc = (e) => { if (pop && !pop.contains(e.target) && !el.contains(e.target)) closePop(); };
    const taskOf = () => NS.store.chatTasks(pane.chat).filter((x) => LIVE.includes(x.status)).sort((a, b) => String(b.created_at).localeCompare(String(a.created_at)))[0] || null;
    function openPop(tk, btn) {
      closePop();
      const keep = h('input', { type: 'text', class: 'wide', value: t('pr.keep_def'), 'aria-label': t('pr.keep'), spellcheck: false });
      const go = h('button', { type: 'button', class: 'btn sm primary', text: t('pr.compact_go'), onclick: async () => {
        go.disabled = true;
        try { await call('compact', { taskId: tk.id, keep: keep.value.trim() || null }); NS.shell.toast(t('pr.compact_sent')); closePop(); } catch (e) { go.disabled = false; fail(e); }
      } });
      keep.addEventListener('keydown', (e) => { if (e.key === 'Enter') go.click(); if (e.key === 'Escape') { closePop(); btn.focus(); } });
      pop = h('div', { class: 'ctx4-pop', role: 'dialog', 'aria-label': t('pr.compact') },
        h('p', { text: t('pr.ctx_hint') }), h('label', {}, h('span', { text: t('pr.keep') }), keep), h('div', { class: 'ctx4-pop-b' }, go));
      el.append(pop);
      document.addEventListener('pointerdown', onDoc, true);
      keep.focus(); keep.select();
    }
    function draw() {
      const tk = taskOf();
      clear(el);
      el.hidden = !tk;
      if (!tk) { closePop(); return; }
      const pct = tk.context_pct;
      if (pct == null) { el.append(h('span', { class: 'ctx4 none', text: t('pr.ctx_none') })); return; }
      const p = Math.max(0, Math.min(100, Math.round(pct)));
      const warn = p >= WARN_PCT;
      const meter = h('span', { class: 'ctx4-m', role: 'meter', 'aria-valuemin': '0', 'aria-valuemax': '100', 'aria-valuenow': String(p), 'aria-label': t('pr.ctx', { pct: p }) }, h('i'));
      setVars(meter, { '--p': p + '%' });
      const box = h('span', { class: 'ctx4' + (warn ? ' warn' : ''), title: warn ? t('pr.ctx_hint') : '' }, meter, h('span', { text: t('pr.ctx', { pct: p }) }));
      if (warn) {
        box.append(h('small', { text: t('pr.ctx_short') }));
        const btn = h('button', { type: 'button', class: 'btn sm', text: t('pr.compact'), 'aria-haspopup': 'dialog', onclick: () => (pop ? closePop() : openPop(tk, btn)) });
        box.append(btn);
      }
      el.append(box);
    }
    draw();
    return { update(evt) { if (evt === 'task' || evt === 'taskremoved') { if (!pop) draw(); } }, destroy: closePop };
  }
  NS.registry.extend('paneHeader', { id: 'ctx', kinds: ['agent'], order: 40, render: ctxSlot });

  // ---------- details: hand-off note, ports and database (307, 309) ----------
  NS.registry.extend('details', { id: 'handoff', order: 60, render(task) {
    if (!task.handoff && !task.port && !task.db_name) return null;
    const box = h('div', { class: 'handoff4' });
    if (task.port || task.db_name) box.append(h('p', { class: 'ports4' }, [task.port ? t('pr.ports', { from: task.port, to: task.port + 9 }) : null, task.db_name ? t('pr.db', { name: task.db_name }) : null].filter(Boolean).join(' · '),
      h('small', { text: t('pr.env_vars') })));
    if (task.handoff) box.append(h('h4', { text: t('pr.handoff') }), h('pre', { class: 'handoff4-t', text: task.handoff }));
    return box;
  } });

  // ---------- startup tax (306) ----------
  function taxBlock(env, fx) {
    const body = h('div', { class: 'srows' }, h('p', { class: 'hint', text: '…' }));
    const projects = S.projects || [];
    let project = S.lastProject && projects.some((p) => p.id === S.lastProject) ? S.lastProject : (projects[0] || {}).id || '';
    const sel = h('select', { 'aria-label': t('pr.tax_project') }, h('option', { value: '', text: t('pr.tax_global') }), projects.map((p) => h('option', { value: p.id, text: p.name, selected: p.id === project })));
    async function load() {
      let x;
      try { x = await call('startup_tax', { env: env.id, project: project || null }); } catch (e) { if (fx.alive.v) clear(body).append(h('p', { class: 'hint', text: NS.errHuman(NS.api.errText(e)) })); return; }
      if (!fx.alive.v) return;
      const K = NS.settingsKit;
      const rules = (x.rules || []).map((r) => {
        const row = K.row(r.path.replace(/^\/Users\/[^/]+|^\/home\/[^/]+/, '~'), r.long ? t('pr.tax_long', { n: num(r.lines) }) : num(r.lines) + ' ' + (window.NSI18n.lang() === 'en' ? 'lines' : 'строк'), h('span', { class: 'tok4', text: t('pr.tax_tok', { n: num(r.tokens) }) }));
        if (r.long) row.classList.add('long4');
        return row;
      });
      clear(body).append(
        K.row(t('pr.tax'), t('pr.tax_d'), h('b', { class: 'tax4-total', text: t('pr.tax_total', { n: num(x.total_tokens) }) })),
        ...(rules.length ? rules : [K.row(t('pr.tax_rules'), '', h('span', { class: 'hint', text: t('pr.tax_none') }))]),
        K.row(t('pr.tax_skills', { n: x.skills || 0 }), '', h('span', { class: 'tok4', text: t('pr.tax_tok', { n: num(x.skill_tokens) }) })),
        K.row(t('pr.tax_agents', { n: x.agents || 0 }), '', h('span', { class: 'tok4', text: t('pr.tax_tok', { n: num(x.agent_tokens) }) })),
        K.row((x.mcp_servers || []).length ? t('pr.tax_mcp', { list: x.mcp_servers.join(', ') }) : t('pr.tax_mcp_none'), '', null));
    }
    sel.onchange = () => { project = sel.value; load(); };
    load();
    return h('div', { class: 'sgrp' }, h('h2', {}, t('pr.tax'), h('span', { class: 'tax4-sel' }, sel)), body);
  }

  // ---------- Settings → Роли (310, 312) ----------
  const SERVICE = ['watcher', 'coordinator', 'planner'];
  const TOOL_ROLES = ['planner', 'reviewer', 'worker'];
  const defTools = (role) => (role === 'worker' ? ['Read', 'Grep', 'Glob', 'Edit', 'Write', t('pr.t_checks')] : ['Read', 'Grep', 'Glob']);
  function roles(env, fx) {
    const K = NS.settingsKit;
    const cat = S.catalogs.get(env.id);
    if (!cat && env.state !== 'missing') NS.store.loadCatalog(env.id).then(() => fx.alive.v && fx.redraw()).catch(() => {});
    const es = () => NS.store.envSettings(env.id);
    const models = SERVICE.map((role) => {
      const cur = (es().roles || {})[role] || null;
      return K.selectRow({ title: t('pr.r.' + role), desc: t('pr.r.' + role + '_d'), value: cur ? L.choiceKey(cur) : '', options: L.roleOptions(cat, t, 'pr.fast', cur),
        apply: async (v) => { const r = Object.assign({}, es().roles); const c = L.choiceFromKey(v); if (c) r[role] = c; else delete r[role]; await K.commitEnv(env.id, { roles: r }); } });
    });
    const tools = TOOL_ROLES.map((role) => K.textRow({ title: t('pr.t.' + role), desc: t('pr.t_def', { list: defTools(role).join(', ') }),
      value: ((S.settings.role_tools || {})[role] || []).join(', '), placeholder: defTools(role).join(', '),
      apply: (v) => { const all = Object.assign({}, S.settings.role_tools); const list = v.split(',').map((x) => x.trim()).filter(Boolean); if (list.length) all[role] = list; else delete all[role]; return K.commit({ role_tools: all }); } }));
    return [h('p', { class: 'lead', text: t('pr.roles_lead') }), K.group(t('pr.r_models', { env: env.name }), models), K.group(t('pr.tools'), tools, t('pr.tools_d'))];
  }

  // ---------- Settings → Экономия (308, 309) ----------
  function savings(env, fx) {
    const K = NS.settingsKit;
    const save = async (p, v) => {
      const next = Object.assign({}, p, { isolate_ports: v });
      K.touch();
      await call('update_project', { project: next });
      const i = S.projects.findIndex((x) => x.id === p.id);
      if (i >= 0) S.projects[i] = next;
    };
    const projects = (S.projects || []).map((p) => K.toggleRow({ title: p.name, desc: t('pr.isolate_d'), on: !!p.isolate_ports, apply: (v) => save(p, v) }));
    return [
      h('p', { class: 'lead', text: t('pr.save_lead') }),
      K.group('', [K.toggleRow({ title: t('pr.compress'), desc: t('pr.compress_d'), on: S.settings.compress_output !== false, apply: (v) => K.commit({ compress_output: v }) })]),
      K.group(t('pr.isolate'), projects.length ? projects : [h('p', { class: 'hint', text: t('pr.no_projects') })]),
    ];
  }
  NS.registry.extend('settings', { id: 'roles4', level: 'basic', labelKey: 'pr.nav_roles', order: 30, build: roles });
  NS.registry.extend('settings', { id: 'save4', level: 'basic', labelKey: 'pr.nav_save', order: 40, build: savings });
  NS.practices = { taxBlock, WARN_PCT };
})();
