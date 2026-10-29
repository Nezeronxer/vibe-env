// The data of the app cannot be read at start (G16.4): the file is untouched, a copy is beside it. Two ways out:
// show the folder, or start from a clean sheet (the old file is put aside, nothing is deleted, and only after a confirmation).
(function () {
  const NS = window.NS;
  const { h, ic } = NS;
  const t = (k, p) => NS.t(k, p);
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);

  function mount(root, ctx) {
    const copy = (ctx.params && ctx.params.copy) || '', token = (ctx.params && ctx.params.token) || '';
    root.append(h('div', { class: 'onb' }, h('div', { class: 'onb-brand' }, ic('i-moon'), 'Nightshift'), h('h1', { text: t('data.title') }), h('p', { class: 'lead', text: t('data.lead', { copy }) }),
      h('div', { class: 'row' },
        h('button', { type: 'button', class: 'btn', text: t('data.open'), onclick: () => NS.api.call('open_data_folder').catch(fail) }),
        h('button', { type: 'button', class: 'btn primary', text: t('data.fresh'), onclick: async () => {
          if (!(await NS.projects.confirm(t('data.fresh_title'), t('data.fresh_text'), t('data.fresh_ok')))) return;
          NS.api.call('start_fresh', { token }).catch(fail);
        } }))));
    return {};
  }
  NS.registry.register({ id: 'unreadable', layout: 'bare', scroll: true, mount });

  // the core did not answer at start for another reason than git or the data file: say what, and «Повторить» runs the start again
  NS.registry.register({ id: 'loadfail', layout: 'bare', scroll: true, mount(root, ctx) {
    const why = (ctx.params && ctx.params.error) || '';
    root.append(h('div', { class: 'onb' }, h('div', { class: 'onb-brand' }, ic('i-moon'), 'Nightshift'), h('h1', { text: t('load.title') }), h('p', { class: 'lead', text: t('load.lead') }),
      why ? h('pre', { class: 'cmd', text: why }) : null,
      h('div', { class: 'row' }, h('button', { type: 'button', class: 'btn primary', text: t('git.recheck'), onclick: () => NS.recheckGit() }))));
    return {};
  } });
})();
