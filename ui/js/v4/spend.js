// «Расходы» (G70): a table day × project, money and tokens, from `spend_history`. A chat's spending is a row of its own («чат»),
// an env that does not report money says «нет данных», never $0. Days older than 400 come folded into months.
(function (root) {
  const NS = root.NS;
  if (!NS || !NS.h) return;
  const { h, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const S = NS.store.s;
  const L = root.NSLogic;
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
  const RANGES = [['r30', 30], ['r90', 90], ['rall', 0]];

  const pad = (n) => String(n).padStart(2, '0');
  const dayIso = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  const locale = () => (NS.lang() === 'en' ? 'en' : 'ru');
  function dayLabel(r) {
    const d = new Date(r.day + 'T12:00:00');
    if (isNaN(d)) return r.day;
    if (r.month) return t('spend.month', { month: new Intl.DateTimeFormat(locale(), { month: 'long', year: 'numeric' }).format(d) });
    return new Intl.DateTimeFormat(locale(), { day: 'numeric', month: 'short', year: 'numeric', weekday: 'short' }).format(d);
  }
  const money = (usd) => (usd == null ? t('spend.no_data') : L.formatCost(usd) || '$0.00');
  const tokens = (n) => (n == null ? t('spend.no_data') : new Intl.NumberFormat(locale()).format(n));
  function projectName(id) {
    if (!id) return t('spend.unknown_project');
    const p = (S.projects || []).find((x) => x.id === id);
    return p ? p.name : t('spend.removed_project');
  }
  const envName = (id) => { const e = NS.store.env && NS.store.env(id); return e ? e.name : id; };
  // a sum where "no data" stays no data unless something reported a number
  const add = (a, b) => (a == null ? b : b == null ? a : a + b);

  function table(rows) {
    // newest day first; inside a day by project name, the chat row after the tasks of the same project
    const sorted = rows.slice().sort((a, b) => (a.day < b.day ? 1 : a.day > b.day ? -1 : 0)
      || projectName(a.project).localeCompare(projectName(b.project)) || (a.chat === b.chat ? 0 : a.chat ? 1 : -1) || String(a.env).localeCompare(String(b.env)));
    const head = h('thead', {}, h('tr', {}, ['spend.day', 'spend.project', 'spend.env', 'spend.usd', 'spend.tin', 'spend.tout'].map((k, i) => h('th', { scope: 'col', class: i >= 3 ? 'sp-n' : '', text: t(k) }))));
    let last = null;
    const body = h('tbody', {}, sorted.map((r) => {
      const key = r.day + (r.month ? 'm' : '');
      const first = key !== last;
      last = key;
      return h('tr', { class: (first ? 'sp-first' : '') + (r.month ? ' sp-month' : '') },
        h('td', { class: 'sp-day', text: first ? dayLabel(r) : '' }),
        h('td', {}, projectName(r.project), r.chat ? h('span', { class: 'sp-tag', text: t('spend.chat') }) : null),
        h('td', { text: envName(r.env) }),
        h('td', { class: 'sp-n' + (r.usd == null ? ' sp-none' : ''), text: money(r.usd) }),
        h('td', { class: 'sp-n' + (r.tokens_in == null ? ' sp-none' : ''), text: tokens(r.tokens_in) }),
        h('td', { class: 'sp-n' + (r.tokens_out == null ? ' sp-none' : ''), text: tokens(r.tokens_out) }));
    }));
    const tot = rows.reduce((s, r) => ({ usd: add(s.usd, r.usd), tin: add(s.tin, r.tokens_in), tout: add(s.tout, r.tokens_out), gaps: s.gaps || r.usd == null }), { usd: null, tin: null, tout: null, gaps: false });
    const foot = h('tfoot', {}, h('tr', {}, h('th', { scope: 'row', colspan: '3', text: t('spend.total') + (tot.gaps && tot.usd != null ? ' · ' + t('spend.part_money') : '') }),
      h('td', { class: 'sp-n', text: money(tot.usd) }), h('td', { class: 'sp-n', text: tokens(tot.tin) }), h('td', { class: 'sp-n', text: tokens(tot.tout) })));
    return h('div', { class: 'sp-wrap' }, h('table', { class: 'sp-table' }, head, body, foot));
  }

  function mount(root0, ctx) {
    const alive = { v: true };
    const st = { range: (ctx && ctx.params && ctx.params.range) || 'r30', rows: null, seq: 0 };
    const box = h('div', { class: 'sp-box' });
    const seg = h('div', { class: 'seg sp-range', role: 'group', 'aria-label': t('spend.range') });
    function drawSeg() {
      clear(seg);
      for (const [k] of RANGES) seg.append(h('button', { type: 'button', class: st.range === k ? 'on' : '', 'aria-pressed': String(st.range === k), text: t('spend.' + k), onclick: () => { st.range = k; drawSeg(); load(); } }));
    }
    function draw() {
      if (!alive.v) return;
      clear(box);
      if (st.rows == null) return;
      if (!st.rows.length) {
        box.append(h('div', { class: 'sp-empty' }, h('b', { text: t('spend.empty') }), h('p', { class: 'hint', text: t('spend.empty_d') })));
        return;
      }
      box.append(table(st.rows));
    }
    async function load() {
      const seq = ++st.seq;
      const days = (RANGES.find((r) => r[0] === st.range) || [0, 0])[1];
      const from = days ? dayIso(new Date(Date.now() - (days - 1) * 86400000)) : null;
      try {
        const rows = await NS.api.call('spend_history', from ? { from } : {});
        if (seq === st.seq) { st.rows = rows || []; draw(); }
      } catch (e) { if (seq === st.seq) { st.rows = []; draw(); fail(e); } }
    }
    root0.append(h('div', { class: 'auto4 ops6 spend6' },
      h('header', { class: 'a4-h' }, h('h1', { text: t('spend.title') }), h('p', { class: 'hint', text: t('spend.lead') })),
      h('section', { class: 'a4-panel', id: 'spend-panel' }, seg, box)));
    drawSeg(); load();
    return { destroy() { alive.v = false; } };
  }
  NS.registry.register({ id: 'spend', layout: 'full', scroll: true, mount });
  NS.registry.extend('command', { id: 'spend.open', labelKey: 'spend.cmd', group: 'mk.group', order: 76, run: () => NS.nav('spend', {}) });
  NS.spend = { open: () => NS.nav('spend', {}), money, tokens };
})(typeof window !== 'undefined' ? window : globalThis);
