// Quiet limit bars in the chat header (G26): two most used windows, "ещё N" popover, warning >= 80% (never amber),
// exhausted + one consequence line, "нет данных", grey when older than 6 h.
(function () {
  const NS = window.NS;
  const { h, ic, clear, setVars } = NS;
  const L = window.NSLogic;
  const t = (k, p) => NS.t(k, p);

  function windowsOf(envId) { return L.limitsView(NS.store.s.limits.get(envId)); }

  function bar(w, now, warnAt) {
    const level = L.limitLevel(w, warnAt), fresh = L.limitFreshness(w, now);
    const pct = w.exhausted ? 100 : Math.max(0, Math.min(100, w.used_pct == null ? 0 : w.used_pct));
    const text = L.formatLimit(w, now, t);
    const cls = 'lim ' + level + (fresh !== 'fresh' ? ' stale' : '');
    const el = h('div', { class: cls, title: t('lim.seen', { time: NS.hhmm(w.seen_at) }) },
      h('span', { class: 'lt' }, level === 'warn' ? ic('i-warn', 'warn-i') : null, text,
        level === 'warn' && w.used_pct != null ? h('em', { text: ' · ' + t('lim.left_pct', { n: Math.max(0, Math.round(100 - w.used_pct)) }) }) : null,
        fresh === 'stale' ? h('em', { text: ' · ' + t('lim.at_time', { time: NS.hhmm(w.seen_at) }) }) : null),
      h('span', { class: 'lb' }, setVars(h('i'), { '--w': pct + '%' })));
    return el;
  }

  function limitsBar(envId) {
    const el = h('div', { class: 'limits', 'aria-label': t('lim.label') });
    let pop = null;
    function closePop() { if (pop) { pop.remove(); pop = null; el.querySelector('.more') && el.querySelector('.more').setAttribute('aria-expanded', 'false'); } }
    function draw() {
      closePop();
      clear(el);
      const v = windowsOf(envId), now = Date.now(), lim = ((NS.store.s.settings || {}).limits) || {}, warnAt = lim.warn_pct != null ? lim.warn_pct : lim.warn_at;
      if (v.kind !== 'known' || !v.windows.length) {
        const why = v.kind === 'nodata' && v.why ? v.why : t('lim.after_first');
        el.append(h('div', { class: 'lim none', title: why }, h('span', { class: 'lt', text: t('lim.no_data') }), h('small', { text: why })));
        return;
      }
      const { shown, rest } = L.pickWindows(v.windows, 2);
      const row = h('div', { class: 'limrow' }, shown.map((w) => bar(w, now, warnAt)));
      if (rest.length) {
        const more = h('button', { type: 'button', class: 'linkb more', 'aria-expanded': 'false', text: t('lim.more', { n: rest.length }),
          onclick: (e) => {
            e.stopPropagation();
            if (pop) return closePop();
            pop = h('div', { class: 'menu limpop', role: 'dialog', 'aria-label': t('lim.label') }, ...v.windows.map((w) => bar(w, now, warnAt)));
            el.append(pop); more.setAttribute('aria-expanded', 'true');
          } });
        row.append(more);
      }
      el.append(row);
      const tasks = NS.store.envTasks(envId);
      const fb = NS.store.envSettings(envId).fallback;
      const cat = NS.store.s.catalogs.get(envId);
      const line = L.exhaustedLine(v.windows, { fallback: fb ? L.modelLabel(fb, cat) : null,
        nightOnly: tasks.length > 0 && tasks.every((x) => L.kindOf(x) !== 'run' && x.start && L.variant(x.start)[0] === 'night') });
      if (line) el.append(h('small', { class: 'cons', text: t(line.key, line.params) }));
    }
    const onDoc = (e) => { if (pop && !pop.contains(e.target)) closePop(); };
    document.addEventListener('click', onDoc);
    const onKey = (e) => { if (e.key === 'Escape') closePop(); };
    document.addEventListener('keydown', onKey);
    const timer = setInterval(draw, 60000);
    draw();
    return { el, refresh: draw, destroy() { clearInterval(timer); document.removeEventListener('click', onDoc); document.removeEventListener('keydown', onKey); } };
  }

  NS.limits = { limitsBar, windowsOf };
})();
