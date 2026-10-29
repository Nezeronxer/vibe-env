// First run in one step (v3) and "Нужен git". Both are bare screens (no rail).
(function () {
  const NS = window.NS;
  const { h, ic, envIcon, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  // the command as the column's card shows it: the core's « (не сверено)» is a note for the code, not text for the person
  const cmdLine = (e) => { const p = window.NSEnvRail && window.NSEnvRail.installPlan(e); return p ? p.line : (e.hint || ''); };

  // First run in one step (stories 237-239): the folder; the env and the model are picked by themselves
  // (first signed-in env in the order Claude Code, Codex, opencode, Gemini, Qwen, Cursor; model "as in the CLI").
  // No env signed in: the install list or "Войди в …"; the folder can still be chosen, "Отправить" stays off.
  function mountOnboarding(root) {
    const st = NS.store, V = window.NSLive;
    const box = h('div', { class: 'onb onb1' });
    root.append(box);
    let folder = null;
    async function chosen(project) {
      if (!project) return;
      const pick = V.pickEnv(st.s.envs);
      if (pick.why === 'ok') {
        const env = pick.env;
        if (!st.envSettings(env.id).default_model) { try { await st.saveEnvSettings(env.id, { default_model: { family: NS.models.CLI_DEFAULT, version: null } }); } catch (e) { /* the composer shows the CLI default anyway */ } }
        st.s.lastProject = project.id;
        return NS.nav('env', { env: env.id, chat: 'new' });
      }
      folder = project; draw();
    }
    NS.onboardingAdded = chosen;
    function envBlock(pick) {
      const call = NS.api.call, fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
      const recheck = h('button', { type: 'button', class: 'btn', text: t('onb3.recheck'), onclick: async () => { try { await st.reloadEnvs(); } catch (e) { fail(e); } draw(); } });
      if (pick.why === 'login') {
        return h('section', { class: 'onb-env' }, h('h2', {}, envIcon(pick.env.id), t('onb3.login_title', { env: pick.env.name })), h('p', { text: t('onb3.login_d') }),
          h('div', { class: 'acts-row' }, h('button', { type: 'button', class: 'btn primary', text: t('onb3.login_btn'), onclick: () => call('login_env', { env: pick.env.id }).catch(fail) }), recheck));
      }
      const list = st.s.envs.filter((e) => !e.custom).sort((a, b) => V.ENV_ORDER.indexOf(a.id) - V.ENV_ORDER.indexOf(b.id));
      return h('section', { class: 'onb-env' }, h('h2', { text: t('onb3.none_title') }), h('p', { text: t('onb3.none_d') }),
        h('ul', { class: 'inst' }, list.map((e) => h('li', {}, envIcon(e.id), h('b', { text: e.name }), cmdLine(e) ? h('code', { text: cmdLine(e) }) : h('span'),
          h('button', { type: 'button', class: 'btn sm', text: t('onb3.install'), onclick: () => call('install_env', { env: e.id }).catch(fail) })))),
        h('div', { class: 'acts-row' }, recheck));
    }
    function draw() {
      const pick = V.pickEnv(st.s.envs);
      const pickBtn = h('button', { type: 'button', class: 'btn primary', text: t('onb3.pick'), onclick: async () => chosen(await NS.projects.addByDialog()) });
      const drop = h('div', { class: 'drop big', role: 'group', 'aria-label': t('onb3.drop') }, ic('i-folder'),
        h('div', {}, h('b', { text: folder ? t('onb3.folder', { path: folder.path || folder.name }) : t('onb3.drop') }), h('span', { text: t('onb3.drop_d') })), pickBtn);
      const line = pick.why === 'ok' ? h('p', { class: 'onb-line' }, envIcon(pick.env.id), t('onb3.line', { env: pick.env.name })) : null;
      const off = pick.why !== 'ok' && folder ? h('div', { class: 'onb-off' }, h('textarea', { rows: 2, disabled: true, placeholder: t('onb3.ph'), 'aria-label': t('onb3.ph') }),
        h('div', { class: 'comp-row' }, h('span', { class: 'hint', text: t('onb3.send_off') }), h('span', { class: 'grow' }), h('button', { type: 'button', class: 'btn primary', disabled: true, title: t('onb3.send_off') }, t('composer.send')))) : null;
      NS.append(clear(box), [h('div', { class: 'onb-brand' }, ic('i-moon'), 'Nightshift'), h('h1', { text: t('onb3.title') }), h('p', { class: 'lead', text: t('onb3.lead') }),
        drop, line, pick.why === 'ok' ? null : envBlock(pick), off, h('p', { class: 'onb-note' }, ic('i-ask', 'amber'), t('onb.three_cases'))]);
    }
    draw();
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
