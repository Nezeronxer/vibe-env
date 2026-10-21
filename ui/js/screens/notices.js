// Bars on top of the window: "Настройки сброшены, старые - в <файл>" (G16.4), "Системные уведомления выключены - <как включить>" (R15i.7),
// "установка <имя> прервана - откатил" (G25.9) and the one-time "<Среда> работает без песочницы…" with "Понятно" before the first task (R17i.3).
// The first three come from the app as events (names chosen here, see the report of task 15); the fourth depends on the env on screen.
(function () {
  const NS = window.NS;
  const { h, ic, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const S = NS.store.s;
  S.notices = S.notices || {};
  const bar = () => document.getElementById('noticeBar');
  const SBX = (id) => 'ns.sbxack.' + id;      // R17i.3: "понятно" is kept per env (the core has no field for it yet, see the report)
  const dismissed = new Set();                // blocked-notifications bar hidden for this run only

  function line(key, text, onOk) {
    return h('div', { class: 'nbar', role: 'status', dataset: { notice: key } }, ic('i-warn'), h('p', {}, text), h('button', { type: 'button', class: 'btn sm', text: t('notice.ok'), onclick: onOk }));
  }
  function draw() {
    const el = bar(); if (!el) return;
    clear(el);
    const n = S.notices;
    if (n.replaced) el.append(line('replaced', t('notice.reset', { file: n.replaced }), () => { n.replaced = null; draw(); }));
    if (n.blocked && !dismissed.has('blocked')) {
      const how = typeof n.blocked === 'string' ? n.blocked : t(NS.platform && NS.platform() === 'mac' ? 'notice.how_mac' : 'notice.how_linux');
      el.append(line('blocked', t('notice.blocked') + ' - ' + how, () => { dismissed.add('blocked'); draw(); }));
    }
    for (const name of n.recovered || []) el.append(line('recovered', t('kits.interrupted', { name }), () => { n.recovered = n.recovered.filter((x) => x !== name); draw(); }));
    if (n.sandbox) {
      const e = NS.store.env(n.sandbox);
      if (e) el.append(line('sandbox', [h('b', { text: t('perm.nosandbox', { env: e.name }) + ' ' }), t('perm.nosandbox_d')], () => { NS.storage.set(SBX(e.id), '1'); n.sandbox = null; draw(); }));
    }
  }

  const listen = (name, fn) => NS.api.listen(name, fn).catch(() => {});
  listen('settings-replaced', (pl) => { S.notices.replaced = (pl && (pl.path || pl.file)) || (typeof pl === 'string' ? pl : '?'); draw(); });
  listen('notify-blocked', (pl) => { dismissed.delete('blocked'); S.notices.blocked = (pl && (pl.how || pl.hint)) || (typeof pl === 'string' ? pl : true); draw(); document.dispatchEvent(new CustomEvent('ns-notice')); });
  listen('tray-state', (pl) => { S.notices.trayMissing = !!pl && pl.available === false; document.dispatchEvent(new CustomEvent('ns-notice')); });
  // payload = core Recovered{snapshot, env, kit_name, message}
  listen('kits-recovered', (pl) => { const name = typeof pl === 'string' ? pl : pl && (pl.kit_name || pl.name); if (name) { S.notices.recovered = (S.notices.recovered || []).concat(name); draw(); } });

  // before the first task in an env without a sandbox, once per env
  function onRoute(route) {
    const e = route && ['chat', 'swarm'].includes(route.screen) ? NS.store.env(route.env) : null;
    S.notices.sandbox = e && e.sandbox && e.sandbox.sandboxed === false && !NS.storage.get(SBX(e.id)) ? e.id : null;
    draw();
  }
  NS.notices = { onRoute, draw };
})();
