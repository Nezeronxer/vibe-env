// First run: "Опиши задачу и ложись спать" - three steps (Среда, Модель, Проект); and "Нужен git". Both are bare screens (no rail).
(function () {
  const NS = window.NS;
  const { h, ic, envIcon, clear } = NS;
  const t = (k, p) => NS.t(k, p);

  function mountOnboarding(root) {
    const st = NS.store;
    let sel = (st.s.envs.find((e) => e.state === 'ok') || st.s.envs[0] || {}).id;
    const envsBox = h('div', { class: 'envcards', role: 'radiogroup', 'aria-label': t('onb.env') }), found = h('p', { class: 'onb-found' });
    const modelsBox = h('div', { class: 'onb-models' }), envName = h('span', { class: 'hint' });
    const stateText = (e) => (e.custom ? t('onb.any_cli') : e.state === 'ok' ? t('onb.found_ok') : e.state === 'missing' ? t('onb.not_found') : e.state === 'login' ? t('onb.found_login') : t('onb.unavailable'));
    function draw() {
      clear(envsBox).append(...st.s.envs.map((e) => h('button', { type: 'button', class: 'envcard' + (e.id === sel ? ' on' : '') + (e.state === 'missing' ? ' off' : ''), role: 'radio', 'aria-checked': String(e.id === sel),
        onclick: () => { sel = e.id; draw(); } }, envIcon(e.custom ? 'custom' : e.id), h('b', { text: e.name }), h('small', { class: e.state === 'ok' && !e.custom ? 'ok' : '', text: stateText(e) }))));
      const e = st.env(sel);
      clear(found);
      if (e && e.kitDir && e.state === 'ok') found.append(ic('i-check'), t('onb.kit_found', { dir: e.kitDir }));
      envName.textContent = e ? e.name : '';
      clear(modelsBox);
      if (e && e.state !== 'missing') { modelsBox.append(NS.models.picker(sel)); st.loadCatalog(sel).then(() => { if (root.isConnected && st.env(sel)) { clear(modelsBox).append(NS.models.picker(sel)); } }); }
      else modelsBox.append(h('p', { class: 'hint', text: t('onb.model_after_install') }));
    }
    async function addAndGo(project) {
      if (!project) return;
      const e = st.env(sel);
      NS.nav('env', { env: e && e.state === 'ok' ? e.id : (st.s.envs.find((x) => x.state === 'ok') || {}).id, chat: 'new' });
    }
    NS.onboardingAdded = addAndGo;
    draw();
    root.append(h('div', { class: 'onb' },
      h('div', { class: 'onb-brand' }, ic('i-moon'), 'Nightshift'), h('h1', { text: t('onb.title') }), h('p', { class: 'lead', text: t('onb.lead') }),
      h('ol', { class: 'onb-steps' },
        h('li', {}, h('h2', {}, h('span', { class: 'num', text: '1' }), t('onb.step_env')), h('p', { class: 'hint', text: t('onb.env_d') }), envsBox, found),
        h('li', {}, h('h2', {}, h('span', { class: 'num', text: '2' }), t('onb.step_model'), ' ', envName), modelsBox),
        h('li', {}, h('h2', {}, h('span', { class: 'num', text: '3' }), t('onb.step_project')),
          h('div', { class: 'drop' }, ic('i-folder'), h('div', {}, h('b', { text: t('onb.drop') }), h('span', { text: t('onb.drop_d') })),
            h('button', { type: 'button', class: 'btn primary', text: t('onb.pick'), onclick: async () => addAndGo(await NS.projects.addByDialog()) })))),
      h('p', { class: 'onb-note' }, ic('i-ask', 'amber'), t('onb.three_cases'))));
    return { destroy() { NS.onboardingAdded = null; }, update(evt) { if (evt === 'envs') draw(); } };
  }

  function mountNoGit(root, ctx) {
    const mac = /Mac/i.test(navigator.platform || '');
    root.append(h('div', { class: 'onb' }, h('div', { class: 'onb-brand' }, ic('i-moon'), 'Nightshift'), h('h1', { text: t('git.title') }), h('p', { class: 'lead', text: t('git.d') }),
      h('pre', { class: 'cmd', text: mac ? 'brew install git\n# ' + t('git.or') + ' xcode-select --install' : 'sudo apt install git\n# ' + t('git.or') + ' sudo pacman -S git' }),
      h('button', { type: 'button', class: 'btn primary', text: t('git.recheck'), onclick: () => NS.recheckGit() })));
  }

  NS.registry.register({ id: 'onboarding', layout: 'bare', scroll: true, mount: mountOnboarding });
  NS.registry.register({ id: 'nogit', layout: 'bare', scroll: true, mount: mountNoGit });
})();
