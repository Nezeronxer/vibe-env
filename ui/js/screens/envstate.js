// Env that cannot take a chat yet: not installed, needs login, unavailable, custom command not configured (R14i.2, G17.2, G17.3, G18).
(function () {
  const NS = window.NS;
  const { h, ic, envIcon, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);

  function mount(root, ctx) {
    const st = NS.store;
    const env = st.env(ctx.params.env);
    const box = h('section', { class: 'missing' });
    root.append(h('div', { class: 'missing-wrap' }, box));
    const recheck = async () => { try { await st.reloadEnvs(); NS.nav('env', { env: env.id }); } catch (e) { fail(e); } };
    const call = NS.api.call;

    function missing() {
      const pathRow = h('div', { class: 'pathrow', hidden: true });
      const inp = h('input', { type: 'text', 'aria-label': t('envs.path'), placeholder: t('envs.path_ph') });
      pathRow.append(inp, h('button', { type: 'button', class: 'btn', text: t('envs.path_save'), onclick: async () => {
        if (!inp.value.trim()) return;
        try { await st.saveEnvSettings(env.id, { cli_path: inp.value.trim() }); await recheck(); } catch (e) { fail(e); }
      } }));
      NS.append(box, [envIcon(env.id), h('h2', { text: t('envs.missing', { env: env.name }) }), h('p', { text: t('envs.missing_d') }),
        env.hint ? h('pre', { text: env.hint }) : null,
        h('div', { class: 'acts-row' },
          h('button', { type: 'button', class: 'btn primary', text: t('envs.install'), onclick: () => call('install_env', { env: env.id }).catch(fail) }),
          h('button', { type: 'button', class: 'btn', text: t('envs.path_manual'), onclick: () => { pathRow.hidden = !pathRow.hidden; if (!pathRow.hidden) inp.focus(); } })), pathRow]);
    }
    function login() {
      box.append(envIcon(env.id), h('h2', { text: t('envs.login', { env: env.name }) }), h('p', { text: t('envs.login_d') }),
        h('div', { class: 'acts-row' }, h('button', { type: 'button', class: 'btn primary', text: t('envs.login_btn'), onclick: () => call('login_env', { env: env.id }).catch(fail) }),
          h('button', { type: 'button', class: 'btn', text: t('envs.recheck'), onclick: recheck })));
    }
    function unavailable() {
      box.append(envIcon(env.id), h('h2', { text: t('envs.unavailable', { env: env.name, why: NS.errHuman(env.reason) }) }), h('p', { text: t('envs.unavailable_d') }),
        h('div', { class: 'acts-row' }, h('button', { type: 'button', class: 'btn', text: t('envs.recheck'), onclick: recheck })));
    }
    function custom() {
      const agent = (st.s.settings.custom_agents || []).find((a) => 'custom:' + a.id === env.id || a.id === env.id) || { id: env.id.replace(/^custom:/, ''), name: env.name, run: '', resume: null };
      const name = h('input', { type: 'text', value: agent.name, 'aria-label': t('custom.name') });
      const run = h('input', { type: 'text', value: agent.run, placeholder: 'my-agent --dir {cwd} --prompt {prompt}', 'aria-label': t('custom.run') });
      const resume = h('input', { type: 'text', value: agent.resume || '', 'aria-label': t('custom.resume') });
      const out = h('p', { class: 'hint', role: 'status' });
      const form = h('div', { class: 'cform', hidden: !!agent.run }, h('label', {}, h('span', { class: 'field-l', text: t('custom.name') }), name), h('label', {}, h('span', { class: 'field-l', text: t('custom.run') }), run),
        h('label', {}, h('span', { class: 'field-l', text: t('custom.resume') }), resume),
        h('button', { type: 'button', class: 'btn primary', text: t('custom.save'), onclick: async () => {
          const next = Object.assign({}, agent, { name: name.value.trim() || agent.name, run: run.value.trim(), resume: resume.value.trim() || null });
          const list = (st.s.settings.custom_agents || []).filter((a) => a.id !== agent.id).concat(next);
          try { await st.saveSettings({ custom_agents: list }); await recheck(); } catch (e) { fail(e); }
        } }));
      box.append(envIcon('custom'), h('h2', { text: t('custom.title') }), h('p', { text: t('custom.d') }), h('pre', { text: 'my-agent --dir {cwd} --prompt {prompt}\n{prompt} {cwd} {session}' }),
        h('div', { class: 'acts-row' }, h('button', { type: 'button', class: 'btn primary', text: t('custom.setup'), onclick: () => { form.hidden = !form.hidden; if (!form.hidden) run.focus(); } }),
          h('button', { type: 'button', class: 'btn', text: t('custom.check'), onclick: async () => {
            try { const r = await call('check_custom', { id: agent.id }); out.textContent = typeof r === 'string' ? r : t('custom.ok'); } catch (e) { out.textContent = NS.errHuman(NS.api.errText(e)); }
          } })), form, out);
    }
    ({ missing, login, unavailable, unset: custom }[env.state] || unavailable)();
    return { update(evt) { if (evt === 'envs') { const e2 = st.env(env.id); if (e2 && e2.state === 'ok') NS.nav('env', { env: env.id }); } } };
  }
  NS.registry.register({ id: 'envstate', layout: 'chats', mount });
})();
