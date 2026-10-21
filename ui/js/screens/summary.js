// "Пока тебя не было" (A06): after 30+ minutes away, a digest over all envs on top of the main screen - finished (with commits), failed, waiting for you,
// money spent, limits. "Открыть" on a row, "Понятно" hides it, more than 10 rows -> "ещё N", nothing happened -> no digest (stories 175-176).
// The digest is built by the core from the task logs (summary_since), so it survives a restart and sleep; the window only remembers when it was left.
(function () {
  const NS = window.NS;
  const { h, ic, envIcon, clear } = NS;
  const L = window.NSLogic;
  const t = (k, p) => NS.t(k, p);
  const S = NS.store.s;
  const LEFT = 'ns.leftAt', ALIVE = 'ns.alive';
  const num = (k) => Number(NS.storage.get(k)) || null;
  const bar = () => document.getElementById('summaryBar');

  function open(row) {
    const tk = row.task_id && S.tasks.get(row.task_id);
    if (row.env) NS.nav('env', { env: row.env, chat: tk ? tk.chat_id : undefined });
  }
  // the digest as a node: used on top of the main screen (with "Понятно") and as the screen 'summary'
  function node(summary, since, onDismiss) {
    const v = L.summaryRows(summary, 10), now = Date.now();
    const envOf = (id) => (NS.store.env(id) || { id, name: id, custom: false });
    const rows = v.rows.map((r) => {
      const e = envOf(r.env);
      const icon = r.kind === 'wait' ? 'ask' : r.kind === 'fail' ? 'fail' : r.kind === 'done' ? 'check' : 'warn';
      const text = r.kind === 'limit' ? [h('b', { text: t('sum.limit', { env: e.name }) }), h('small', { text: L.formatLimit(r.window, now, t) })]
        : [h('b', { text: r.title }), h('small', { text: r.kind === 'wait' ? t('sum.waits') : r.kind === 'fail' ? NS.errHuman(r.reason) || t('st.fail') : r.commit ? t('sum.commit', { sha: String(r.commit).slice(0, 7) }) : t('st.done') })];
      return h('div', { class: 'sum-row ' + r.kind }, ic('i-' + icon, 'ti ' + (r.kind === 'limit' ? '' : r.kind)), envIcon(e.custom ? 'custom' : e.id), h('span', { class: 'tx' }, text), r.env ? h('button', { type: 'button', class: 'btn sm', text: t('sum.open'), onclick: () => open(r) }) : h('span'));
    });
    return h('section', { class: 'sum', 'aria-label': t('sum.title') },
      h('div', { class: 'sum-h' }, h('b', { text: t('sum.title') }), since ? h('span', { class: 'hint', text: t('sum.for', { time: L.formatDuration(now - since, NS.lang()) }) }) : null),
      h('div', { class: 'sum-rows' }, rows),
      h('div', { class: 'sum-foot' }, v.spent != null ? h('span', { text: t('sum.spent', { usd: L.formatCost(v.spent) }) }) : null, v.more > 0 ? h('span', { text: t('sum.more', { n: v.more }) }) : null, h('span', { class: 'grow' }),
        onDismiss ? h('button', { type: 'button', class: 'btn sm', text: t('notice.ok'), onclick: onDismiss }) : null));
  }

  async function check() {
    const now = Date.now(), afterMin = ((S.settings || {}).keep || {}).summary_after_min || 30;
    const d = L.awayDecision({ leftAt: num(LEFT), lastAlive: num(ALIVE), now, afterMin });
    NS.storage.del(LEFT); NS.storage.set(ALIVE, String(now));
    const el = bar();
    if (!d.show || !el) return;
    let summary;
    try { summary = await NS.api.call('summary_since', { at: new Date(d.since).toISOString() }); } catch (e) { return; }
    if (L.summaryRows(summary, 10).empty) { clear(el); return; }       // nothing happened: no digest
    clear(el).append(node(summary, d.since, () => clear(el)));
  }
  const markLeft = () => NS.storage.set(LEFT, String(Date.now()));
  window.addEventListener('blur', markLeft);
  document.addEventListener('visibilitychange', () => { if (document.hidden) markLeft(); else check(); });
  window.addEventListener('focus', check);
  setInterval(() => NS.storage.set(ALIVE, String(Date.now())), 60000);      // a missed beat = the computer slept
  window.addEventListener('ns-ready', check);
  NS.summary = { check, node };

  NS.registry.register({ id: 'summary', layout: 'full', scroll: true, async mount(root, ctx) {
    const since = ctx.params.since || Date.now() - 12 * 3600000;
    const box = h('div');
    root.append(box);
    try { const s = await NS.api.call('summary_since', { at: new Date(since).toISOString() }); clear(box).append(L.summaryRows(s, 10).empty ? h('p', { class: 'placeholder', text: t('sum.nothing') }) : node(s, since)); }
    catch (e) { box.append(h('p', { class: 'placeholder', text: NS.errHuman(NS.api.errText(e)) })); }
  } });
})();
