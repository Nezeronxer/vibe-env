// «О программе» (G69, G71): the version, the check of the app's own release (by the button; by itself once a day in the app),
// the time and the error of the last check, «Собрать отчёт об ошибке». And the bar «Вышла версия X» with the link to the release:
// a closed bar does not come back until a newer version. Without `release_feed` the check is off and the screen says why.
(function (root) {
  const NS = root.NS;
  if (!NS || !NS.h) return;
  const { h, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const call = (c, a) => NS.api.call(c, a);
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
  const locale = () => (NS.lang() === 'en' ? 'en' : 'ru');
  const when = (iso) => { const d = new Date(iso); return isNaN(d) ? iso : new Intl.DateTimeFormat(locale(), { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }).format(d); };
  function why(err) {
    const m = /^http_(\d+)$/.exec(err || '');
    if (m) return t('appupd.why.http', { code: m[1] });
    const k = 'appupd.why.' + err;
    return window.NSI18n.has(k) ? t(k) : t('appupd.why.no_connection');
  }
  const openRelease = (url) => call('open_path', { path: url }).catch(fail);

  // ---------- the bar ----------
  let bar = null;
  function showBar(v) {
    if (!v || !v.latest || !v.url) return;
    if (bar) bar.remove();
    const close = async () => { bar.remove(); bar = null; try { await call('app_update_dismiss', { version: v.latest }); } catch (e) { fail(e); } };
    bar = h('div', { class: 'appupd6', role: 'status', 'aria-live': 'polite' },
      h('span', { class: 'lamp armed', 'aria-hidden': 'true' }),
      h('b', { text: t('appupd.bar', { v: v.latest }) }),
      h('button', { type: 'button', class: 'btn sm primary', text: t('appupd.open'), onclick: () => openRelease(v.url) }),
      h('button', { type: 'button', class: 'linkb', text: t('appupd.later'), onclick: close }));
    document.body.append(bar);
  }
  NS.api.listen('app-update', (p) => showBar(p)).catch(() => {});
  root.addEventListener('ns-ready', () => { call('app_update_state').then((v) => { if (v && v.show) showBar(v); }).catch(() => {}); });

  // ---------- the screen ----------
  function mount(root0) {
    const alive = { v: true };
    const st = { v: null, checking: false, collecting: false };
    const upd = h('section', { class: 'a4-panel', id: 'about-updates' });
    const diag = h('section', { class: 'a4-panel', id: 'about-diag' });
    const ver = h('p', { class: 'about-ver' });

    function drawUpd() {
      if (!alive.v) return;
      const v = st.v;
      clear(upd);
      upd.append(h('h2', { text: t('about.upd_h') }));
      if (!v) return;
      ver.textContent = t('about.version', { v: v.current });
      const lines = [];
      if (!v.feed) {
        lines.push(h('p', { class: 'about-off', text: t('about.no_feed') }), h('p', { class: 'hint', text: t('about.no_feed_d') }));
      } else {
        lines.push(h('p', { text: v.checked_at ? t('about.checked', { when: when(v.checked_at) }) : t('about.never') }));
        if (v.latest) lines.push(h('p', { text: v.latest !== v.current ? t('about.newer', { v: v.latest }) : t('about.latest', { v: v.latest }) + ' ' + t('about.uptodate') }));
        if (v.error) lines.push(h('p', { class: 'a4-warn', text: t('about.err', { why: why(v.error) }) }));
        lines.push(h('p', { class: 'hint', text: t('about.auto') }));
      }
      const btns = h('div', { class: 'about-acts' },
        h('button', { type: 'button', class: 'btn', id: 'about-check', disabled: !v.feed || st.checking, text: st.checking ? t('about.checking') : t('about.check'), onclick: check }),
        v.feed && v.url && v.latest && v.latest !== v.current ? h('button', { type: 'button', class: 'btn', text: t('about.open_release'), onclick: () => openRelease(v.url) }) : null);
      upd.append(...lines, btns);
    }
    function drawDiag() {
      clear(diag);
      const d = root.__TAURI__ && root.__TAURI__.dialog;
      diag.append(h('h2', { text: t('about.diag_h') }), h('p', { class: 'hint', text: t('about.diag_d') }),
        h('div', { class: 'about-acts' },
          h('button', { type: 'button', class: 'btn primary', id: 'about-diag-go', disabled: st.collecting, text: st.collecting ? t('about.diag_busy') : t('about.diag_go'), onclick: () => collect(null) }),
          d && d.open ? h('button', { type: 'button', class: 'btn', disabled: st.collecting, text: t('about.diag_pick'), onclick: async () => {
            let p = null;
            try { p = await d.open({ directory: true, multiple: false }); } catch (e) { /* the dialog was closed */ }
            const dir = Array.isArray(p) ? p[0] : p;
            if (dir) collect(dir);
          } }) : null),
        h('small', { class: 'mute', text: t('about.diag_where') }));
    }
    async function check() {
      st.checking = true; drawUpd();
      try { st.v = await call('app_update_check'); if (st.v && st.v.show) showBar(st.v); } catch (e) { fail(e); }
      st.checking = false; drawUpd();
    }
    async function collect(dir) {
      st.collecting = true; drawDiag();
      try { const path = await call('diag_collect', dir ? { dir } : {}); NS.shell.toast(t('about.diag_done', { path })); } catch (e) { fail(e); }
      st.collecting = false; if (alive.v) drawDiag();
    }
    root0.append(h('div', { class: 'auto4 ops6 about6' },
      h('header', { class: 'a4-h' }, h('h1', { text: t('about.title') }), ver, h('p', { class: 'hint', text: t('about.lead') })),
      h('div', { class: 'about-grid' }, upd, diag)));
    drawUpd(); drawDiag();
    call('app_update_state').then((v) => { st.v = v; drawUpd(); }).catch(fail);
    return { destroy() { alive.v = false; } };
  }
  NS.registry.register({ id: 'about', layout: 'full', scroll: true, mount });
  NS.registry.extend('command', { id: 'about.open', labelKey: 'about.cmd', group: 'mk.group', order: 77, run: () => NS.nav('about', {}) });
  NS.registry.extend('command', { id: 'diag.collect', labelKey: 'diag.cmd', group: 'mk.group', order: 78, run: () => NS.nav('about', {}) });
  NS.about = { open: () => NS.nav('about', {}), showBar };
})(typeof window !== 'undefined' ? window : globalThis);
