// F14 automation: one screen with the schedule of tasks (#1), suggested tasks from the code (#7) and the CI watchman (#2),
// and a bar button with the count of suggestions. Everything here is switched on by the person; the core starts nothing alone.
(function (root) {
  const NS = root.NS;
  if (!NS || !NS.h) return;
  const { h, ic, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const S = NS.store.s;
  const call = (c, a) => NS.api.call(c, a);
  for (const c of ['auto_get', 'auto_schedule_save', 'auto_schedule_remove', 'auto_schedule_toggle', 'auto_suggest_scan', 'auto_suggest_do', 'auto_suggest_dismiss', 'auto_suggest_nightly', 'auto_ci_set', 'auto_ci_poll', 'auto_issue_answer']) NS.api.COMMANDS.add(c);
  NS.api.EVENTS.add('auto-changed');
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
  const PRESETS = ['daily', 'weekdays', 'weekly'];

  // ---------- data: one copy, everybody who draws listens ----------
  let data = null, started = false;
  const subs = new Set();
  async function load() {
    try { data = await call('auto_get'); } catch (e) { return data; }
    subs.forEach((f) => f());
    return data;
  }
  function start() {
    if (started) return;
    started = true;
    NS.api.listen('auto-changed', load);
    load();
  }
  const asks = () => (data && data.issues && data.issues.asks) || [];
  // the button carries what waits for the person: suggestions and the questions «Закрыть issue #N?»
  const count = () => (data && data.suggest ? data.suggest.count : 0) + asks().length;
  // "каждый день в 03:00", "раз в неделю, среда в 03:00"
  const whenText = (s) => t('auto.w.' + s.preset, { time: s.at, day: t('auto.d' + s.weekday) });
  const stamp = (iso) => {
    const d = new Date(iso);
    if (isNaN(d)) return '';
    return new Intl.DateTimeFormat(NS.lang() === 'en' ? 'en' : 'ru', { weekday: 'short', day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }).format(d);
  };
  const projName = (id) => (NS.store.project(id) || { name: id }).name;
  NS.auto = { load, count, whenText, subs };

  // ---------- small controls ----------
  function toggle(on, label, apply) {
    const b = h('button', { type: 'button', role: 'switch', class: 'tg', 'aria-checked': String(!!on), 'aria-label': label }, h('span', { class: 'sw' }, h('i')));
    b.onclick = async () => {
      const v = b.getAttribute('aria-checked') !== 'true';
      b.setAttribute('aria-checked', String(v));
      try { await apply(v); } catch (e) { b.setAttribute('aria-checked', String(!v)); if (!(e && e.cancel)) fail(e); }
    };
    return b;
  }
  const field = (label, ctl, cls) => h('label', { class: 'fld ' + (cls || '') }, h('span', { class: 'field-l', text: label }), ctl);
  const select = (opts, value) => { const s = h('select', {}, opts.map(([v, l]) => h('option', { value: v, text: l }))); s.value = value; return s; };

  // ---------- the screen ----------
  function mount(root0) {
    start();
    const alive = { v: true };
    const sched = h('ul', { class: 'a4-list' }), sugBox = h('div', { class: 'a4-sug' }), ciBox = h('div', { class: 'a4-ci' });
    const form = buildForm();

    // --- schedule ---
    function drawSched() {
      const list = (data && data.schedules) || [];
      clear(sched);
      if (!list.length) sched.append(h('li', { class: 'a4-none', text: t('auto.sched_none') }));
      for (const s of list) {
        const skip = s.skipped ? (s.skipped === 'limit' ? t('auto.skip.limit') : t('auto.skip.other', { why: NS.errHuman(s.skipped) })) : '';
        sched.append(h('li', { class: 'a4-row' + (s.enabled ? ' on' : ''), dataset: { id: s.id } },
          h('span', { class: 'lamp ' + (s.enabled ? 'armed' : 'idle'), 'aria-hidden': 'true' }),
          h('div', { class: 'a4-main' },
            h('b', { text: s.title }),
            h('small', { text: [projName(s.project_id), whenText(s)].join(' · ') }),
            h('small', { class: 'mute', text: [s.enabled && s.next ? t('auto.next', { when: stamp(s.next) }) : '', s.last_run ? t('auto.last', { when: stamp(s.last_run) }) : ''].filter(Boolean).join(' · ') }),
            skip ? h('small', { class: 'a4-warn', text: skip }) : null),
          h('div', { class: 'a4-acts' },
            toggle(s.enabled, t('auto.toggle', { name: s.title }), async (v) => { await call('auto_schedule_toggle', { id: s.id, enabled: v }); await load(); }),
            h('button', { type: 'button', class: 'btn sm', text: t('auto.edit'), onclick: () => form.edit(s) }),
            h('button', { type: 'button', class: 'linkb', text: t('auto.remove'), onclick: async () => {
              if (!(await NS.projects.confirm(t('auto.remove_q', { name: s.title }), '', t('auto.remove')))) return;
              try { await call('auto_schedule_remove', { id: s.id }); await load(); } catch (e) { fail(e); }
            } }))));
      }
    }

    // --- suggestions ---
    let finding = false;
    async function answer(ev, a, close) {
      ev.currentTarget.disabled = true;
      try { await call('auto_issue_answer', { taskId: a.task_id, close }); NS.shell.toast(t(close ? 'iss.closed' : 'iss.kept', { n: a.number })); } catch (e) { fail(e); }
      await load();
    }
    function drawSug() {
      const sg = (data && data.suggest) || { cards: {}, count: 0, nightly: false };
      const find = h('button', { type: 'button', class: 'btn primary', disabled: finding, text: finding ? t('auto.sug_finding') : t('auto.sug_find'), onclick: async () => {
        finding = true; drawSug();
        try { for (const p of S.projects) await call('auto_suggest_scan', { projectId: p.id }); } catch (e) { fail(e); }
        finding = false; await load();
      } });
      clear(sugBox).append(h('div', { class: 'a4-bar' }, find,
        h('span', { class: 'a4-tg' }, toggle(sg.nightly, t('auto.sug_nightly'), async (v) => { await call('auto_suggest_nightly', { on: v }); await load(); }), h('span', { text: t('auto.sug_nightly') })),
        h('span', { class: 'hint grow', text: sg.count ? t('auto.sug_found', { n: sg.count }) : '' })));
      // G67.2: «Закрыть issue #N?» - nothing is closed until «Закрыть»
      const asked = asks();
      if (asked.length) {
        sugBox.append(h('section', { class: 'a4-asks', 'aria-label': t('iss.ask_h') }, h('h3', { text: t('iss.ask_h') }), asked.map((a) => h('div', { class: 'a4-ask', dataset: { task: a.task_id } },
          h('div', { class: 'a4-main' }, h('b', { text: t('iss.ask_q', { n: a.number }) }), h('small', { text: a.title }), h('small', { class: 'mute', text: t('iss.ask_d') })),
          h('div', { class: 'a4-acts' },
            h('button', { type: 'button', class: 'btn sm primary', text: t('iss.close'), onclick: (ev) => answer(ev, a, true) }),
            h('button', { type: 'button', class: 'btn sm', text: t('iss.keep'), onclick: (ev) => answer(ev, a, false) }))))));
      }
      const why = Object.values((data && data.issues && data.issues.notes) || {})[0];
      if (why) sugBox.append(h('p', { class: 'hint a4-issue-note', role: 'status', text: t('iss.note', { why: root.NSI18n.has('iss.why.' + why) ? t('iss.why.' + why) : why }) }));
      const ul = h('ul', { class: 'a4-list sug' });
      for (const [pid, cards] of Object.entries(sg.cards || {})) {
        for (const c of cards) {
          const iss = c.source && c.source.kind === 'issue' ? c.source : null;
          const link = iss && /^https:\/\//.test(iss.url || '') ? iss.url : '';
          ul.append(h('li', { class: 'a4-card' + (iss ? ' issue' : ''), dataset: { fp: c.fp } },
            h('div', { class: 'a4-main' },
              h('span', { class: 'a4-loc' }, iss ? h('code', { text: t('iss.tag', { n: iss.number }) }) : h('code', { text: c.file + ':' + c.line }), h('small', { class: 'mute', text: t('auto.sug_in', { project: projName(pid) }) }),
                link ? h('button', { type: 'button', class: 'linkb', text: t('iss.open'), onclick: () => call('open_path', { path: link }).catch(fail) }) : null),
              h('b', { text: c.text || t('auto.sug_empty_text') }),
              h('small', { class: 'mute', text: c.marker })),
            h('div', { class: 'a4-acts' },
              h('button', { type: 'button', class: 'btn sm', text: t('auto.sug_do'), onclick: async (ev) => {
                ev.currentTarget.disabled = true;
                try { await call('auto_suggest_do', { projectId: pid, fp: c.fp }); NS.shell.toast(t('auto.sug_started')); } catch (e) { fail(e); }
                await load();
              } }),
              h('button', { type: 'button', class: 'linkb', text: t('auto.sug_hide'), onclick: async () => { try { await call('auto_suggest_dismiss', { projectId: pid, fp: c.fp }); } catch (e) { fail(e); } await load(); } }))));
        }
      }
      if (!ul.children.length) ul.append(h('li', { class: 'a4-none', text: t('auto.sug_none') }));
      sugBox.append(ul);
    }

    // --- CI watchman ---
    function drawCi() {
      const ci = (data && data.ci) || { config: {}, items: [] }, cfg = ci.config || {};
      const set = async (patch) => { await call('auto_ci_set', { config: Object.assign({}, cfg, patch) }); await load(); };
      const att = h('input', { type: 'number', min: 1, max: 10, value: cfg.max_attempts || 2, 'aria-label': t('auto.ci_attempts'), onchange: (e) => set({ max_attempts: Math.max(1, Math.min(10, +e.target.value || 2)) }).catch(fail) });
      clear(ciBox).append(
        h('div', { class: 'a4-line' }, h('div', {}, h('b', { text: t('auto.ci_on') })), toggle(cfg.enabled, t('auto.ci_on'), (v) => set({ enabled: v }))),
        h('div', { class: 'a4-line' }, h('div', {}, h('b', { text: t('auto.ci_attempts') })), att),
        h('div', { class: 'a4-line' }, h('div', {}, h('b', { text: t('auto.ci_merge') }), h('small', { class: 'mute', text: t('auto.ci_merge_d') })),
          toggle(cfg.merge_when_green, t('auto.ci_merge'), async (v) => {
            if (v && !cfg.merge_confirmed && !(await NS.projects.confirm(t('auto.ci_merge_q'), t('auto.ci_merge_text'), t('auto.ci_merge_yes')))) throw { cancel: true };
            await set({ merge_when_green: v, merge_confirmed: v ? true : cfg.merge_confirmed });
          })));
      if (ci.hint) ciBox.append(h('p', { class: 'a4-warn', role: 'status', text: t('auto.ci_' + ci.hint) }));
      const ul = h('ul', { class: 'a4-list ci' });
      for (const it of ci.items || []) {
        ul.append(h('li', { class: 'a4-row', dataset: { task: it.task_id } },
          h('span', { class: 'lamp ' + ({ green: 'done', red: 'failed', conflict: 'failed', pending: 'idle', error: 'stamp' }[it.state] || 'idle'), 'aria-hidden': 'true' }),
          h('div', { class: 'a4-main' }, h('b', { text: it.title }),
            h('small', { text: [t('auto.s.' + it.state), t('auto.attempts', { a: it.attempts, m: it.max })].join(' · ') }),
            it.note ? h('small', { class: 'mute', text: t('auto.n.' + it.note) }) : null)));
      }
      if (cfg.enabled && !ul.children.length && !ci.hint) ul.append(h('li', { class: 'a4-none', text: t('auto.ci_none') }));
      if (!cfg.enabled) ul.append(h('li', { class: 'a4-none', text: t('auto.ci_off') }));
      ciBox.append(ul);
      if (cfg.enabled) ciBox.append(h('button', { type: 'button', class: 'btn sm', text: t('auto.ci_check'), onclick: async () => { try { await call('auto_ci_poll'); } catch (e) { fail(e); } await load(); } }));
    }

    const draw = () => { if (!alive.v) return; drawSched(); drawSug(); drawCi(); };
    subs.add(draw);
    draw();
    load();
    const panel = (id, title, desc, ...kids) => h('section', { class: 'a4-panel', id }, h('h2', { text: title }), desc ? h('p', { class: 'hint', text: desc }) : null, kids);
    root0.append(h('div', { class: 'auto4' },
      h('header', { class: 'a4-h' }, h('h1', { text: t('auto.title') }), h('p', { class: 'hint', text: t('auto.lead') }),
        h('button', { type: 'button', class: 'btn sm a4-ses', text: t('ses.open'), onclick: () => NS.nav('sessions', {}) })),
      h('div', { class: 'a4-grid' },
        h('div', { class: 'a4-col' }, panel('a4-sched', t('auto.sched'), t('auto.sched_d'), sched, form.el)),
        h('div', { class: 'a4-col' },
          panel('a4-ci', t('auto.ci'), t('auto.ci_d'), ciBox),
          panel('a4-sug', t('auto.sug'), t('auto.sug_d'), sugBox)))));
    return { destroy() { alive.v = false; subs.delete(draw); } };
  }

  // ---------- the form of a schedule ----------
  function buildForm() {
    let editing = null;
    const tpl = h('div', { class: 'a4-tpl', role: 'group', 'aria-label': t('auto.tpl') });
    const title = h('input', { type: 'text', maxLength: 80, ...NS_QUIET() });
    const prompt = h('textarea', { rows: 4, ...NS_QUIET() });
    const projects = () => S.projects.map((p) => [p.id, p.name]);
    const project = select(projects(), (S.projects[0] || {}).id);
    const envs = () => [['', t('auto.f_env_def')], ...S.envs.filter((e) => e.state === 'ok').map((e) => [e.id, e.name])];
    const env = select(envs(), '');
    const model = select([['', t('auto.f_model_def')]], '');
    const preset = select(PRESETS.map((p) => [p, t('auto.p.' + p)]), 'daily');
    const time = h('input', { type: 'text', value: '03:00', maxLength: 5, inputMode: 'numeric', class: 'a4-time', ...NS_QUIET() });
    const day = select([0, 1, 2, 3, 4, 5, 6].map((d) => [String(d), t('auto.d' + d)]), '2');
    const dayFld = field(t('auto.f_day'), day, 'a4-day');
    const quality = toggle(true, t('auto.f_quality'), () => {});
    const save = h('button', { type: 'button', class: 'btn primary', text: t('auto.add') });
    const cancel = h('button', { type: 'button', class: 'linkb', text: t('auto.cancel'), hidden: true });
    const heading = h('h3', { text: t('auto.new') });

    async function fillModels() {
      const e = env.value || ((NS.store.project(project.value) || {}).agent) || '';
      const cat = e ? (S.catalogs.get(e) || await NS.store.loadCatalog(e)) : null;
      const keep = model.value;
      clear(model).append(h('option', { value: '', text: t('auto.f_model_def') }), ((cat && cat.families) || []).filter((f) => f.id !== 'cli-default').map((f) => h('option', { value: f.id, text: f.label || f.id })));
      model.value = [...model.options].some((o) => o.value === keep) ? keep : '';
    }
    const sync = () => { dayFld.hidden = preset.value !== 'weekly'; };
    preset.onchange = sync; env.onchange = fillModels; project.onchange = fillModels;
    sync();

    const reset = () => {
      editing = null; title.value = ''; prompt.value = ''; env.value = ''; preset.value = 'daily'; time.value = '03:00'; day.value = '2';
      quality.setAttribute('aria-checked', 'true'); heading.textContent = t('auto.new'); save.textContent = t('auto.add'); cancel.hidden = true; sync(); fillModels();
    };
    cancel.onclick = reset;
    for (const [id] of [['deps'], ['security'], ['todo'], ['dead_code']]) {
      tpl.append(h('button', { type: 'button', class: 'btn sm', dataset: { tpl: id }, text: t('auto.tpl.' + id), onclick: () => {
        const tp = ((data && data.templates) || []).find((x) => x.id === id);
        title.value = t('auto.tpl.' + id); prompt.value = tp ? tp.prompt : ''; title.focus();
      } }));
    }
    save.onclick = async () => {
      const at = time.value.trim();
      if (!title.value.trim() || !prompt.value.trim() || !/^([01]\d|2[0-3]):[0-5]\d$/.test(at)) { NS.shell.toast(t('auto.bad_form'), true); return; }
      const q = quality.getAttribute('aria-checked') === 'true';
      const body = { id: editing ? editing.id : '', title: title.value.trim(), prompt: prompt.value.trim(), project_id: project.value, env: env.value || null,
        model: model.value ? { family: model.value, version: null } : null, quality: q ? null : { on: false, spec: true, plan: true, tests_first: true, e2e: false, best_of_n: 2 },
        preset: preset.value, at, weekday: +day.value, enabled: editing ? editing.enabled : false };
      save.disabled = true;
      try { await call('auto_schedule_save', { schedule: body }); NS.shell.toast(t('auto.saved')); reset(); await load(); } catch (e) { fail(e); }
      save.disabled = false;
    };
    const el = h('div', { class: 'a4-form', id: 'a4-form' }, heading,
      h('div', { class: 'a4-tplrow' }, h('span', { class: 'field-l', text: t('auto.tpl') }), tpl),
      field(t('auto.f_title'), title), field(t('auto.f_prompt'), prompt),
      h('div', { class: 'a4-two' }, field(t('auto.f_project'), project), field(t('auto.f_env'), env), field(t('auto.f_model'), model)),
      h('div', { class: 'a4-two' }, field(t('auto.f_preset'), preset), field(t('auto.f_time'), time), dayFld),
      h('div', { class: 'a4-line' }, h('div', {}, h('b', { text: t('auto.f_quality') }), h('small', { class: 'mute', text: t('auto.f_quality_d') })), quality),
      h('div', { class: 'a4-foot' }, save, cancel));
    fillModels();
    return {
      el,
      edit(s) {
        editing = s; title.value = s.title; prompt.value = s.prompt; project.value = s.project_id; env.value = s.env || ''; preset.value = s.preset; time.value = s.at; day.value = String(s.weekday);
        quality.setAttribute('aria-checked', String(!s.quality || s.quality.on)); heading.textContent = t('auto.edit') + ': ' + s.title; save.textContent = t('auto.save'); cancel.hidden = false; sync();
        fillModels().then(() => { model.value = (s.model && s.model.family) || ''; });
        el.scrollIntoView({ block: 'nearest' });
      },
    };
  }
  function NS_QUIET() { return root.NSPalette && root.NSPalette.QUIET ? root.NSPalette.QUIET : { autocomplete: 'off', autocorrect: 'off', autocapitalize: 'off', spellcheck: false }; }

  // ---------- entrances: the bar button with the count, the palette ----------
  NS.registry.register({ id: 'automation', layout: 'full', scroll: true, mount });
  NS.registry.extend('command', { id: 'auto.open', labelKey: 'auto.cmd', group: 'mk.group', order: 74, run: () => NS.nav('automation', {}) });
  // the entrance between ⌘K and "Настройки" in the foot of the agents column (tabs.js draws it): a bolt and the count of suggestions
  function navButton() {
    start();
    const n = h('b', { class: 'bn' });
    const b = h('button', { type: 'button', class: 'navb nb-auto', title: t('auto.cmd'), 'aria-label': t('auto.title'), onclick: () => NS.nav('automation', {}) }, ic('i-bolt'), h('span', { class: 'nav-l', text: t('auto.title') }), n);
    const f = () => {
      if (!b.isConnected) { subs.delete(f); return; }
      n.textContent = count() > 99 ? '99+' : String(count()); n.hidden = !count();
      b.title = t('auto.cmd') + (count() ? ' · ' + t('auto.sug_count', { n: count() }) : '');
    };
    n.hidden = true; subs.add(f);
    return b;
  }
  NS.auto.navButton = navButton;
  root.NSAuto = { whenText, count };
})(typeof window !== 'undefined' ? window : globalThis);
