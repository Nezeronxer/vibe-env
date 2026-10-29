// F14 automation: one screen with the schedule of tasks (#1), suggested tasks from the code (#7) and the CI watchman (#2),
// and a bar button with the count of suggestions. Everything here is switched on by the person; the core starts nothing alone.
// Layout of the schedule: the nearest firing on top (what runs next), then one card per schedule in the manner of an alarm list
// (big time · what · on/off), then the composer: what to do, when (human presets), where, extra options folded away.
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
  const QUICK_TIMES = ['03:00', '09:00', '13:00', '19:00'];

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
  // the short rhythm under the big time: "по будням", "по средам"
  const rhythm = (s) => (s.preset === 'weekly' ? t('auto.dp' + s.weekday) : t('auto.ws.' + s.preset));
  const loc = () => (NS.lang() === 'en' ? 'en' : 'ru');
  const stamp = (iso) => {
    const d = new Date(iso);
    if (isNaN(d)) return '';
    return new Intl.DateTimeFormat(loc(), { weekday: 'short', day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }).format(d);
  };
  // "сегодня в 03:00", "завтра в 03:00", otherwise the date
  const rel = (iso) => {
    const d = new Date(iso);
    if (isNaN(d)) return '';
    const now = new Date();
    const days = Math.round((new Date(d.getFullYear(), d.getMonth(), d.getDate()) - new Date(now.getFullYear(), now.getMonth(), now.getDate())) / 864e5);
    const time = new Intl.DateTimeFormat(loc(), { hour: '2-digit', minute: '2-digit' }).format(d);
    if (days === 0) return t('auto.today', { time });
    if (days === 1) return t('auto.tomorrow', { time });
    if (days === -1) return t('auto.yesterday', { time });
    return stamp(iso);
  };
  // "через 5 ч", "через 40 мин"
  const until = (iso) => {
    const ms = new Date(iso) - Date.now();
    if (!(ms > 0)) return '';
    const m = Math.round(ms / 60000);
    if (m < 60) return t('auto.in_m', { n: Math.max(1, m) });
    if (m < 48 * 60) return t('auto.in_h', { n: Math.round(m / 60) });
    return t('auto.in_d', { n: Math.round(m / 1440) });
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
  // a native select in a shell that draws the chevron (appearance: none loses it)
  const select = (opts, value) => { const s = h('select', {}, opts.map(([v, l]) => h('option', { value: v, text: l }))); s.value = value; return s; };
  const shell = (sel) => h('span', { class: 'a4-sel' }, sel);
  // one choice out of a few, drawn as a segmented control (radio semantics, arrows move the choice)
  function segmented(items, value, label, cls, onpick) {
    const g = h('div', { class: 'a4-seg ' + (cls || ''), role: 'radiogroup', 'aria-label': label });
    let cur = value;
    const btns = items.map(([v, l, title]) => h('button', { type: 'button', role: 'radio', class: 'a4-segb', dataset: { v }, text: l, title: title || null, onclick: () => set(v, true) }));
    function set(v, user) {
      cur = String(v);
      for (const b of btns) { const on = b.dataset.v === cur; b.setAttribute('aria-checked', String(on)); b.tabIndex = on ? 0 : -1; }
      if (user && onpick) onpick(cur);
    }
    g.addEventListener('keydown', (e) => {
      const d = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
      if (!d) return;
      e.preventDefault();
      const i = btns.findIndex((b) => b.dataset.v === cur);
      const n = btns[(i + d + btns.length) % btns.length];
      set(n.dataset.v, true); n.focus();
    });
    g.append(...btns);
    set(cur, false);
    return { el: g, get value() { return cur; }, set value(v) { set(v, false); } };
  }
  const empty = (cls, title, text) => h('li', { class: 'a4-none ' + (cls || '') }, h('b', { text: title }), text ? h('small', { text }) : null);

  // ---------- the screen ----------
  function mount(root0) {
    start();
    const alive = { v: true };
    const next = h('div', { class: 'a4-next', role: 'status' });
    const sched = h('ul', { class: 'a4-list sched' }), sugBox = h('div', { class: 'a4-sug' }), ciBox = h('div', { class: 'a4-ci' });
    const form = buildForm();

    // --- schedule ---
    function drawNext(list) {
      clear(next);
      const on = list.filter((s) => s.enabled && s.next).sort((a, b) => new Date(a.next) - new Date(b.next));
      if (!list.length) { next.hidden = true; return; }
      next.hidden = false;
      if (!on.length) {
        next.classList.add('idle');
        next.append(h('span', { class: 'a4-next-k', text: t('auto.next_h') }), h('b', { class: 'a4-next-t', text: t('auto.next_none') }), h('small', { text: t('auto.next_none_d') }));
        return;
      }
      next.classList.remove('idle');
      const s = on[0];
      next.append(h('span', { class: 'a4-next-k', text: t('auto.next_h') }),
        h('div', { class: 'a4-next-row' }, h('b', { class: 'a4-next-t', text: rel(s.next) }), h('span', { class: 'a4-next-in', text: until(s.next) })),
        h('small', { text: [s.title, projName(s.project_id)].join(' · ') + (on.length > 1 ? ' · ' + t('auto.next_more', { n: on.length - 1 }) : '') }));
    }
    function drawSched() {
      const list = (data && data.schedules) || [];
      drawNext(list);
      clear(sched);
      if (!list.length) sched.append(empty('a4-empty', t('auto.sched_none'), t('auto.sched_none_d')));
      for (const s of list) {
        const skip = s.skipped ? (s.skipped === 'limit' ? t('auto.skip.limit') : t('auto.skip.other', { why: NS.errHuman(s.skipped) })) : '';
        const when = s.enabled && s.next ? t('auto.next', { when: rel(s.next) }) : t('auto.off_hint');
        sched.append(h('li', { class: 'a4-row' + (s.enabled ? ' on' : ''), dataset: { id: s.id } },
          h('div', { class: 'a4-clock', 'aria-label': whenText(s) }, h('span', { class: 'a4-hm', text: s.at }), h('span', { class: 'a4-rh', text: rhythm(s) })),
          h('div', { class: 'a4-main' },
            h('b', { text: s.title }),
            h('small', { text: projName(s.project_id) }),
            h('small', { class: 'a4-when' + (s.enabled ? ' a4-live' : ''), text: when }),
            s.last_run ? h('small', { class: 'mute', text: t('auto.last', { when: rel(s.last_run) }) }) : null,
            skip ? h('small', { class: 'a4-warn' }, ic('i-warn'), h('span', { text: skip })) : null),
          h('div', { class: 'a4-acts' },
            toggle(s.enabled, t('auto.toggle', { name: s.title }), async (v) => { await call('auto_schedule_toggle', { id: s.id, enabled: v }); await load(); }),
            h('div', { class: 'a4-more-acts' },
              h('button', { type: 'button', class: 'btn sm a4-ghost', text: t('auto.edit'), onclick: () => form.edit(s) }),
              h('button', { type: 'button', class: 'linkb a4-del', text: t('auto.remove'), onclick: async () => {
                if (!(await NS.projects.confirm(t('auto.remove_q', { name: s.title }), '', t('auto.remove')))) return;
                try { await call('auto_schedule_remove', { id: s.id }); await load(); } catch (e) { fail(e); }
              } })))));
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
        h('span', { class: 'a4-tg' }, toggle(sg.nightly, t('auto.sug_nightly'), async (v) => { await call('auto_suggest_nightly', { on: v }); await load(); }), h('span', { text: t('auto.sug_nightly'), title: t('auto.sug_nightly_d') })),
        sg.count ? h('span', { class: 'a4-count', text: t('auto.sug_found', { n: sg.count }) }) : null));
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
          const kind = iss ? 'issue' : String(c.marker || 'todo').toLowerCase();
          ul.append(h('li', { class: 'a4-card' + (iss ? ' issue' : ''), dataset: { fp: c.fp } },
            h('div', { class: 'a4-main' },
              h('span', { class: 'a4-loc' },
                h('span', { class: 'a4-tag', dataset: { kind }, text: iss ? t('iss.tag', { n: iss.number }) : (c.marker || 'TODO') }),
                iss ? null : h('code', { text: c.file + ':' + c.line }),
                h('small', { class: 'mute', text: t('auto.sug_in', { project: projName(pid) }) }),
                link ? h('button', { type: 'button', class: 'linkb', text: t('iss.open'), onclick: () => call('open_path', { path: link }).catch(fail) }) : null),
              h('b', { class: c.text ? '' : 'mute', text: c.text || t('auto.sug_empty_text') })),
            h('div', { class: 'a4-acts' },
              h('button', { type: 'button', class: 'btn sm', text: t('auto.sug_do'), onclick: async (ev) => {
                ev.currentTarget.disabled = true;
                try { await call('auto_suggest_do', { projectId: pid, fp: c.fp }); NS.shell.toast(t('auto.sug_started')); } catch (e) { fail(e); }
                await load();
              } }),
              h('button', { type: 'button', class: 'linkb a4-del', text: t('auto.sug_hide'), onclick: async () => { try { await call('auto_suggest_dismiss', { projectId: pid, fp: c.fp }); } catch (e) { fail(e); } await load(); } }))));
        }
      }
      if (!ul.children.length) ul.append(empty('', t('auto.sug_none'), t('auto.sug_none_d')));
      sugBox.append(ul);
    }

    // --- CI watchman ---
    const CI_TONE = { green: 'ok', red: 'bad', conflict: 'bad', pending: 'wait', error: 'warn' };
    function drawCi() {
      const ci = (data && data.ci) || { config: {}, items: [] }, cfg = ci.config || {};
      const set = async (patch) => { await call('auto_ci_set', { config: Object.assign({}, cfg, patch) }); await load(); };
      const att = h('input', { type: 'number', min: 1, max: 10, value: cfg.max_attempts || 2, 'aria-label': t('auto.ci_attempts'), onchange: (e) => set({ max_attempts: Math.max(1, Math.min(10, +e.target.value || 2)) }).catch(fail) });
      const line = (title, desc, ctl) => h('div', { class: 'a4-line' }, h('div', {}, h('b', { text: title }), desc ? h('small', { class: 'mute', text: desc }) : null), ctl);
      clear(ciBox).append(h('div', { class: 'a4-set' + (cfg.enabled ? '' : ' off') },
        line(t('auto.ci_on'), t('auto.ci_on_d'), toggle(cfg.enabled, t('auto.ci_on'), (v) => set({ enabled: v }))),
        line(t('auto.ci_attempts'), t('auto.ci_attempts_d'), att),
        line(t('auto.ci_merge'), t('auto.ci_merge_d'), toggle(cfg.merge_when_green, t('auto.ci_merge'), async (v) => {
          if (v && !cfg.merge_confirmed && !(await NS.projects.confirm(t('auto.ci_merge_q'), t('auto.ci_merge_text'), t('auto.ci_merge_yes')))) throw { cancel: true };
          await set({ merge_when_green: v, merge_confirmed: v ? true : cfg.merge_confirmed });
        }))));
      if (ci.hint) ciBox.append(h('p', { class: 'a4-warn a4-callout', role: 'status' }, ic('i-warn'), h('span', { text: t('auto.ci_' + ci.hint) })));
      const head = h('div', { class: 'a4-subh' }, h('h3', { text: t('auto.ci_prs') }),
        cfg.enabled ? h('button', { type: 'button', class: 'btn sm a4-ghost', text: t('auto.ci_check'), onclick: async () => { try { await call('auto_ci_poll'); } catch (e) { fail(e); } await load(); } }) : null);
      const ul = h('ul', { class: 'a4-list ci' });
      for (const it of ci.items || []) {
        const tone = CI_TONE[it.state] || 'wait';
        ul.append(h('li', { class: 'a4-row a4-pr', dataset: { task: it.task_id, tone } },
          h('span', { class: 'a4-dot', 'aria-hidden': 'true' }),
          h('div', { class: 'a4-main' }, h('b', { text: it.title }),
            h('small', { class: 'a4-meta' }, h('span', { class: 'a4-state', text: t('auto.s.' + it.state) }), h('span', { text: t('auto.attempts', { a: it.attempts, m: it.max }) })),
            it.note ? h('small', { class: 'mute', text: t('auto.n.' + it.note) }) : null)));
      }
      if (cfg.enabled && !ul.children.length && !ci.hint) ul.append(empty('', t('auto.ci_none'), ''));
      if (!cfg.enabled) ul.append(empty('', t('auto.ci_off'), t('auto.ci_off_d')));
      ciBox.append(head, ul);
    }

    const draw = () => { if (!alive.v) return; drawSched(); drawSug(); drawCi(); };
    subs.add(draw);
    draw();
    load();
    const panel = (id, title, desc, ...kids) => h('section', { class: 'a4-panel', id }, h('header', { class: 'a4-ph' }, h('h2', { text: title }), desc ? h('p', { class: 'hint', text: desc }) : null), kids);
    root0.append(h('div', { class: 'auto4' },
      h('header', { class: 'a4-h' },
        h('div', {}, h('h1', { text: t('auto.title') }), h('p', { class: 'hint', text: t('auto.lead') })),
        h('button', { type: 'button', class: 'btn sm a4-ses', text: t('ses.open'), onclick: () => NS.nav('sessions', {}) })),
      h('div', { class: 'a4-grid' },
        h('div', { class: 'a4-col' }, panel('a4-sched', t('auto.sched'), t('auto.sched_d'), next, sched, form.el)),
        h('div', { class: 'a4-col' },
          panel('a4-sug', t('auto.sug'), t('auto.sug_d'), sugBox),
          panel('a4-ci', t('auto.ci'), t('auto.ci_d'), ciBox)))));
    return { destroy() { alive.v = false; subs.delete(draw); } };
  }

  // ---------- the form of a schedule ----------
  function buildForm() {
    let editing = null;
    const tpl = h('div', { class: 'a4-tpl', role: 'group', 'aria-label': t('auto.tpl') });
    const title = h('input', { type: 'text', maxLength: 80, placeholder: t('auto.f_title_ph'), ...NS_QUIET() });
    const prompt = h('textarea', { rows: 4, placeholder: t('auto.f_prompt_ph'), ...NS_QUIET() });
    const projects = () => S.projects.map((p) => [p.id, p.name]);
    // the project of the column head, as the other screens (before: always the first one, even one whose folder is gone)
    const cur = NS.projects && NS.projects.current && NS.projects.current();
    const project = select(projects(), (cur && S.projects.some((p) => p.id === cur)) ? cur : (S.projects[0] || {}).id);
    const envs = () => [['', t('auto.f_env_def')], ...S.envs.filter((e) => e.state === 'ok').map((e) => [e.id, e.name])];
    const env = select(envs(), '');
    const model = select([['', t('auto.f_model_def')]], '');
    const summary = h('p', { class: 'a4-sum', 'aria-live': 'polite' });
    const preset = segmented(PRESETS.map((p) => [p, t('auto.p.' + p)]), 'daily', t('auto.f_preset'), 'a4-preset', () => sync());
    preset.el.querySelectorAll('.a4-segb').forEach((b) => { b.dataset.preset = b.dataset.v; });
    const time = h('input', { type: 'text', value: '09:00', maxLength: 5, inputMode: 'numeric', class: 'a4-time', 'aria-label': t('auto.f_time'), ...NS_QUIET() });
    const quick = segmented(QUICK_TIMES.map((x) => [x, x, t('auto.tq.' + x.slice(0, 2))]), '09:00', t('auto.f_time_quick'), 'a4-quick', (v) => { time.value = v; sync(); });
    time.oninput = () => { quick.value = time.value; sync(); };
    const day = segmented([0, 1, 2, 3, 4, 5, 6].map((d) => [String(d), t('auto.ds' + d), t('auto.d' + d)]), '2', t('auto.f_day'), 'a4-days', () => sync());
    const dayFld = h('div', { class: 'fld a4-day' }, h('span', { class: 'field-l', text: t('auto.f_day') }), day.el);
    const quality = toggle(true, t('auto.f_quality'), () => {});
    const more = h('details', { class: 'a4-more' }, h('summary', {}, ic('i-chev-r'), h('span', { text: t('auto.f_more') })),
      h('div', { class: 'a4-more-in' },
        h('div', { class: 'a4-two' }, field(t('auto.f_env'), shell(env)), field(t('auto.f_model'), shell(model))),
        h('div', { class: 'a4-line' }, h('div', {}, h('b', { text: t('auto.f_quality') }), h('small', { class: 'mute', text: t('auto.f_quality_d') })), quality)));
    const save = h('button', { type: 'button', class: 'btn primary', text: t('auto.add') });
    const cancel = h('button', { type: 'button', class: 'btn sm a4-ghost', text: t('auto.cancel'), hidden: true });
    const heading = h('h3', { text: t('auto.new') });

    async function fillModels() {
      const e = env.value || ((NS.store.project(project.value) || {}).agent) || '';
      const cat = e ? (S.catalogs.get(e) || await NS.store.loadCatalog(e)) : null;
      const keep = model.value;
      clear(model).append(h('option', { value: '', text: t('auto.f_model_def') }), ((cat && cat.families) || []).filter((f) => f.id !== 'cli-default').map((f) => h('option', { value: f.id, text: f.label || f.id })));
      model.value = [...model.options].some((o) => o.value === keep) ? keep : '';
    }
    // the sentence above «Добавить»: what will happen, in words
    function sync() {
      dayFld.hidden = preset.value !== 'weekly';
      const at = time.value.trim();
      const okTime = /^([01]\d|2[0-3]):[0-5]\d$/.test(at);
      time.classList.toggle('bad', !!at && !okTime);
      const s = { preset: preset.value, at: okTime ? at : '--:--', weekday: +day.value };
      summary.textContent = t(editing ? 'auto.sum_edit' : 'auto.sum', { when: whenText(s), project: projName(project.value) || '-' });
    }
    env.onchange = fillModels; project.onchange = () => { fillModels(); sync(); };

    const reset = () => {
      editing = null; title.value = ''; prompt.value = ''; env.value = ''; preset.value = 'daily'; time.value = '09:00'; quick.value = '09:00'; day.value = '2';
      quality.setAttribute('aria-checked', 'true'); more.open = false; el.classList.remove('editing');
      heading.textContent = t('auto.new'); save.textContent = t('auto.add'); cancel.hidden = true; sync(); fillModels();
    };
    cancel.onclick = reset;
    for (const id of ['deps', 'security', 'todo', 'dead_code']) {
      tpl.append(h('button', { type: 'button', class: 'a4-chip', dataset: { tpl: id }, text: t('auto.tpl.' + id), onclick: () => {
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
    const group = (label, ...kids) => h('div', { class: 'a4-grp' }, h('span', { class: 'a4-grp-l', text: label }), kids);
    const el = h('div', { class: 'a4-form', id: 'a4-form' },
      h('div', { class: 'a4-form-h' }, heading, cancel),
      group(t('auto.g_what'),
        h('div', { class: 'a4-tplrow' }, h('span', { class: 'field-l', text: t('auto.tpl') }), tpl),
        field(t('auto.f_title'), title), field(t('auto.f_prompt'), prompt)),
      group(t('auto.g_when'),
        preset.el,
        h('div', { class: 'fld' }, h('span', { class: 'field-l', text: t('auto.f_time') }), h('div', { class: 'a4-timerow' }, time, quick.el)),
        dayFld),
      group(t('auto.g_where'), field(t('auto.f_project'), shell(project)), more),
      h('div', { class: 'a4-foot' }, summary, save));
    sync();
    fillModels();
    return {
      el,
      edit(s) {
        editing = s; title.value = s.title; prompt.value = s.prompt; project.value = s.project_id; env.value = s.env || ''; preset.value = s.preset; time.value = s.at; quick.value = s.at; day.value = String(s.weekday);
        const q = !s.quality || s.quality.on;
        quality.setAttribute('aria-checked', String(q)); more.open = !!(s.env || s.model || !q); el.classList.add('editing');
        heading.textContent = t('auto.edit') + ': ' + s.title; save.textContent = t('auto.save'); cancel.hidden = false; sync();
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
