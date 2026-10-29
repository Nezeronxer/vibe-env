// CLI updates and new models in the window (stories 274-277, G43): Settings → "Среды" (registry point `settings`): version,
// latest, how it was installed, "Авто / Только сообщать / Выкл", "Проверить сейчас", history; the startup tax of the env (306,
// drawn by practices.js); popups for the events of app: new-model ("теперь X (было Y)" [Вернуть], or a question for pinned roles),
// cli-updated, cli-update-failed (with the command to run by hand), cli-update-available (Только сообщать).
(function () {
  const NS = window.NS;
  const { h, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const S = NS.store.s;
  const call = (c, a) => NS.api.call(c, a);
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
  // commands and events of task 20/25 that api.js (a file of task 14) does not list yet
  for (const c of ['cli_update_mode', 'startup_tax', 'compact']) NS.api.COMMANDS.add(c);
  for (const e of ['cli-update-failed', 'cli-update-available']) NS.api.EVENTS.add(e);
  const envName = (id) => { const e = NS.store.env(id); return e ? e.name : id; };

  // ---------- popups: a stack bottom right, each with its own buttons ----------
  let stack = null;
  function pop(text, actions, opts) {
    if (!stack) { stack = h('div', { class: 'pops4', role: 'region', 'aria-live': 'polite' }); document.body.append(stack); }
    const o = opts || {};
    let timer = null;
    const close = () => { clearTimeout(timer); card.classList.add('out'); setTimeout(() => card.remove(), 160); };
    const btns = (actions || []).map((a, i) => h('button', { type: 'button', class: 'btn sm' + (i === 0 ? ' primary' : ''), text: a.label,
      onclick: async () => { close(); try { await a.run(); } catch (e) { fail(e); } } }));
    const card = h('div', { class: 'pop4' + (o.bad ? ' bad' : ''), role: o.bad ? 'alert' : 'status' },
      h('p', { text }), o.code ? h('code', { text: o.code }) : null,
      h('div', { class: 'pop4-b' }, btns, h('button', { type: 'button', class: 'linkb', text: t('upd.close'), onclick: close })));
    stack.append(card);
    if (!actions || !actions.length) timer = setTimeout(close, o.bad ? 12000 : 6000);
    card.addEventListener('pointerenter', () => clearTimeout(timer));
    return card;
  }

  // ---------- new models (story 275) ----------
  // "было": the family in the catalog the window still holds (new-model comes before catalog-changed), by its label
  function famOf(env, name) {
    const cat = S.catalogs.get(env);
    const fams = ((cat && cat.families) || []).filter((f) => f.id !== 'default');
    return fams.filter((f) => name.startsWith(f.label + ' ') || name === f.label).sort((a, b) => b.label.length - a.label.length)[0] || null;
  }
  async function onNewModel(n) {
    if (!n || !n.env) return;
    const fam = famOf(n.env, n.name || '');
    const es = NS.store.envSettings(n.env);
    const roles = Object.assign({}, es.roles);
    const pinned = fam ? Object.keys(roles).filter((r) => roles[r] && roles[r].family === fam.id && roles[r].version) : [];
    const was = fam && fam.versions && fam.versions[0];
    const env = envName(n.env);
    if (n.auto) {
      const undo = async () => {
        // the roles on "последняя" of this family (and the default model) go back to the old version, pinned
        const patch = { roles: Object.assign({}, roles) };
        for (const r of Object.keys(patch.roles)) if (patch.roles[r] && patch.roles[r].family === fam.id && !patch.roles[r].version) patch.roles[r] = { family: fam.id, version: was };
        const dm = es.default_model;
        if (!dm || dm.family === 'default' || (dm.family === fam.id && !dm.version)) patch.default_model = { family: fam.id, version: was };
        await NS.store.saveEnvSettings(n.env, patch);
        NS.shell.toast(t('upd.reverted', { was: fam.label + ' ' + was }));
      };
      pop(fam && was ? t('upd.new_was', { env, name: n.name, was: fam.label + ' ' + was }) : t('upd.new_auto', { env, name: n.name }),
        fam && was ? [{ label: t('upd.revert'), run: undo }] : []);
    }
    if (!n.auto || pinned.length) {
      const sw = async () => {
        if (pinned.length) { for (const r of pinned) roles[r] = { family: fam.id, version: null }; await NS.store.saveEnvSettings(n.env, { roles }); }
        else await NS.store.saveEnvSettings(n.env, { default_model: { family: fam ? fam.id : n.id, version: null } });
        NS.shell.toast(t('upd.switched', { name: n.name }));
      };
      pop(t('upd.new_ask', { name: n.name }), [{ label: t('upd.switch'), run: sw }, { label: t('upd.later'), run: () => {} }]);
    }
  }
  const listen = (name, fn) => NS.api.listen(name, fn).catch(() => {});
  listen('new-model', onNewModel);
  listen('cli-updated', (p) => { if (p) pop(t('upd.done', { env: envName(p.env), from: p.from || '?', to: p.to || '?' })); });
  listen('cli-update-failed', (p) => { if (p) pop(t('upd.failed', { env: envName(p.env), back: p.rolled_back ? t('upd.failed_back') : '' }), [], { bad: true, code: (p.cmd || []).join(' ') }); });
  listen('cli-update-available', (p) => { if (p) pop(t('upd.available', { env: envName(p.env), from: p.from || '?', to: p.to || '?' }), [{ label: t('upd.update', { v: p.to }), run: () => call('cli_update', { env: p.env }) }]); });

  // ---------- Settings → Среды ----------
  const fmtAt = (iso) => { try { return new Date(iso).toLocaleString(window.NSI18n.lang() === 'en' ? 'en-GB' : 'ru-RU', { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }); } catch (e) { return iso; } };
  function build(env, fx) {
    const K = NS.settingsKit;
    const cli = h('div', { class: 'srows' }, h('p', { class: 'hint', text: t('upd.checking') }));
    const hist = h('div', { class: 'srows' });
    const alive = () => fx.alive.v;
    async function load(fresh) {
      let s;
      try { s = await call('cli_status', { env: env.id }); } catch (e) {
        if (!alive()) return;
        const why = NS.api.errText(e);
        clear(cli).append(h('p', { class: 'hint', text: /not_built|unknown command/.test(why) ? t('upd.not_built') : NS.errHuman(why) }));
        return;
      }
      if (!alive()) return;
      const newer = s.latest && s.installed && s.latest !== s.installed;
      const check = h('button', { type: 'button', class: 'btn sm', text: t('upd.check'), onclick: async () => { check.disabled = true; check.textContent = t('upd.checking'); await load(true); } });
      const upd = newer ? h('button', { type: 'button', class: 'btn sm primary', text: t('upd.update', { v: s.latest }), onclick: async () => {
        upd.disabled = true; upd.textContent = t('upd.updating');
        try { await call('cli_update', { env: env.id }); } catch (e) { fail(e); }
        if (alive()) { load(); loadHist(); }
      } }) : null;
      clear(cli).append(
        K.row({ text: t('upd.version'), icon: 'l-package' }, s.path ? t('upd.path') + ': ' + s.path : '', h('b', { class: 'ver4', text: s.installed || t('upd.none') })),
        K.row({ text: t('upd.latest'), icon: 'l-sparkles' }, s.major && newer ? t('upd.major', { v: s.latest }) : '', h('span', { class: 'ver4' + (newer ? ' newer' : ''), text: s.latest || t('upd.unknown') }), !newer && s.installed && s.latest ? h('small', { class: 'hint', text: t('upd.fresh') }) : null),
        K.row({ text: t('upd.method'), icon: 'l-wrench' }, s.cmd && s.cmd.length ? t('upd.cmd') + ': ' + s.cmd.join(' ') : '', h('span', { text: t('upd.m.' + (s.method || 'unknown')) })),
        K.row({ text: t('upd.mode'), icon: 'l-refresh-cw' }, t('upd.mode_d'), K.segment([['auto', t('upd.auto')], ['notify', t('upd.notify')], ['off', t('upd.off')]], s.mode || 'auto', t('upd.mode'), (v) => call('cli_update_mode', { env: env.id, mode: v }))),
        K.row('', '', check, upd));
      if (fresh) NS.shell.toast(newer ? t('upd.available', { env: env.name, from: s.installed, to: s.latest }) : t('upd.fresh'));
    }
    async function loadHist() {
      let list = [];
      try { list = (await call('updates_history', { env: env.id })) || []; } catch (e) { list = []; }
      if (!alive()) return;
      clear(hist).append(list.length ? h('ul', { class: 'uhist' }, list.slice().reverse().slice(0, 20).map((x) => h('li', { class: x.ok ? '' : 'bad' },
        h('time', { text: fmtAt(x.at) }), h('span', { text: t(x.ok ? 'upd.h_ok' : 'upd.h_fail', { from: x.from || '?', to: x.to || '?' }) }),
        x.rolled_back ? h('small', { text: t('upd.h_back', { from: x.from || '?' }) }) : null, h('small', { class: 'm', text: t('upd.m.' + (x.method || 'unknown')) }))))
        : h('p', { class: 'hint', text: t('upd.history_none') }));
    }
    load(); loadHist();
    return [
      h('p', { class: 'lead', text: t('upd.lead', { env: env.name }) }),
      h('div', { class: 'sgrp' }, h('h2', { text: env.name }), cli),
      h('div', { class: 'sgrp' }, h('h2', { text: t('upd.history') }), hist),
      NS.practices ? NS.practices.taxBlock(env, fx) : null,
    ];
  }
  NS.registry.extend('settings', { id: 'envs4', level: 'basic', labelKey: 'upd.nav', order: 20, build });
  NS.registry.extend('command', { id: 'upd.open', get label() { return t('upd.nav') + ': ' + t('upd.check'); }, group: 'cmd.g.view', order: 90, run: (ctx) => ctx.nav('settings', { section: 'envs4' }) });
  NS.updates = { pop, onNewModel };
})();
