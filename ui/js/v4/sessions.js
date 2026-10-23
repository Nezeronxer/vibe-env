// G68: sessions of Claude Code from a terminal, per project. The core only reads the files (~/.claude/projects); «Продолжить» queues an
// ordinary task in a separate git copy that resumes the session. Pages of 50, the newest first (story 330).
(function (root) {
  const NS = root.NS;
  if (!NS || !NS.h) return;
  const { h, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const S = NS.store.s;
  const call = (c, a) => NS.api.call(c, a);
  for (const c of ['sessions_list', 'session_continue']) NS.api.COMMANDS.add(c);
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
  const stamp = (iso) => {
    const d = new Date(iso);
    if (isNaN(d)) return '';
    return new Intl.DateTimeFormat(NS.lang() === 'en' ? 'en' : 'ru', { day: 'numeric', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit' }).format(d);
  };
  const msgs = (s) => t(s.approx ? 'ses.messages_more' : 'ses.messages', { n: s.messages });

  function mount(root0, ctx) {
    const alive = { v: true };
    const st = { pid: (ctx && ctx.params && ctx.params.project) || (NS.projects && NS.projects.current && NS.projects.current()) || (S.projects[0] || {}).id || '', items: [], total: 0, page: 0, more: false, loading: false, seq: 0 };
    const list = h('ul', { class: 'a4-list ses-list' }), foot = h('div', { class: 'ses-foot' });
    const pick = h('select', { 'aria-label': t('ses.project') }, S.projects.map((p) => h('option', { value: p.id, text: p.name })));
    if (st.pid) pick.value = st.pid;

    function draw() {
      if (!alive.v) return;
      clear(list);
      if (!S.projects.length) list.append(h('li', { class: 'a4-none', text: t('ses.no_project') }));
      else if (st.loading && !st.items.length) list.append(h('li', { class: 'a4-none', text: t('ses.loading') }));
      else if (!st.items.length) list.append(h('li', { class: 'a4-none ses-none', text: t('ses.none') }));
      for (const s of st.items) {
        const go = h('button', { type: 'button', class: 'btn sm', text: t('ses.continue'), disabled: s.unreadable, onclick: async () => {
          go.disabled = true;
          try { await call('session_continue', { projectId: st.pid, id: s.id }); NS.shell.toast(t('ses.started')); } catch (e) { fail(e); go.disabled = false; }
        } });
        list.append(h('li', { class: 'a4-row ses-row' + (s.unreadable ? ' bad' : ''), dataset: { id: s.id } },
          h('span', { class: 'lamp ' + (s.unreadable ? 'failed' : 'idle'), 'aria-hidden': 'true' }),
          h('div', { class: 'a4-main' },
            h('b', { text: s.first_prompt || t('ses.empty_prompt') }),
            h('small', { class: 'mute', text: [stamp(s.started_at), s.unreadable ? t('ses.unreadable') : msgs(s), s.branch ? t('ses.branch', { name: s.branch }) : ''].filter(Boolean).join(' · ') })),
          h('div', { class: 'a4-acts' }, go)));
      }
      clear(foot);
      if (st.total) foot.append(h('small', { class: 'mute', text: t('ses.count', { n: st.total }) }));
      if (st.more) foot.append(h('button', { type: 'button', class: 'btn sm', disabled: st.loading, text: t('ses.more'), onclick: () => load(st.page + 1) }));
    }
    async function load(page) {
      if (!st.pid) { draw(); return; }
      const seq = ++st.seq;
      st.loading = true; if (!page) { st.items = []; st.page = 0; } draw();
      try {
        const r = await call('sessions_list', { projectId: st.pid, page: page || 0 });
        if (seq !== st.seq) return;
        st.items = (page ? st.items : []).concat(r.items || []);
        Object.assign(st, { total: r.total || 0, more: !!r.more, page: r.page || 0 });
      } catch (e) { if (seq === st.seq) fail(e); }
      if (seq === st.seq) { st.loading = false; draw(); }
    }
    pick.onchange = () => { st.pid = pick.value; load(0); };
    root0.append(h('div', { class: 'auto4 ses6' },
      h('header', { class: 'a4-h' }, h('h1', { text: t('ses.title') }), h('p', { class: 'hint', text: t('ses.lead') })),
      h('section', { class: 'a4-panel', id: 'ses-panel' }, h('label', { class: 'fld ses-pick' }, h('span', { class: 'field-l', text: t('ses.project') }), pick), list, foot)));
    draw(); load(0);
    return { destroy() { alive.v = false; } };
  }
  NS.registry.register({ id: 'sessions', layout: 'full', scroll: true, mount });
  NS.registry.extend('command', { id: 'sessions.open', labelKey: 'ses.cmd', group: 'mk.group', order: 75, run: () => NS.nav('sessions', {}) });
  NS.sessions = { open: (projectId) => NS.nav('sessions', { project: projectId }) };
})(typeof window !== 'undefined' ? window : globalThis);
