// v4 publishing on GitHub (task 24, stories 278-280): a dialog with the name, private / public, the description, what leaves and the secret check;
// the confirmation is asked every time (an outward action, D01). Pure parts (name, error -> what to show) run in selftest.
(function (root) {
  // the folder name made into a GitHub repository name: letters, digits, . _ -
  function repoName(path) {
    const base = String(path || '').replace(/[\\/]+$/, '').split(/[\\/]/).pop() || '';
    const s = base.replace(/[^A-Za-z0-9._-]+/g, '-').replace(/-{2,}/g, '-').replace(/^[-.]+|-+$/g, '').slice(0, 100);
    return s && !/\.git$/i.test(s) ? s : s.replace(/\.git$/i, '') || 'project';
  }
  const validName = (n) => /^[A-Za-z0-9._-]{1,100}$/.test(n) && n !== '.' && n !== '..' && !/\.git$/i.test(n);
  // core::git::PublishError {kind, paths?, detail?} -> a dictionary key, its parameters and the one action that helps
  function errView(e) {
    const o = e && typeof e === 'object' ? e : { kind: String(e || '') };
    const kind = o.kind || 'publish_failed';
    const paths = Array.isArray(o.paths) ? o.paths.map(String) : [];
    const action = { gh_missing: 'install', gh_not_logged_in: 'login', name_taken: 'rename', bad_name: 'rename' }[kind] || null;
    const known = ['gh_missing', 'gh_not_logged_in', 'name_taken', 'bad_name', 'protected_in_history'].includes(kind);
    return { key: 'pub.err.' + (known ? kind : 'publish_failed'), action, paths, detail: o.detail == null ? '' : String(o.detail) };
  }
  root.NSPublishLogic = { repoName, validName, errView };
  if (typeof document === 'undefined' || !root.NS || !root.NS.h) return;

  const NS = root.NS;
  const { h, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const S = NS.store.s;
  const call = (c, a) => NS.api.call(c, a);

  // "Войти в GitHub": a terminal tab with `gh auth login` (the term pane of task 23); without it, the system terminal in the project folder
  function loginInTerminal(p) {
    if (NS.term && NS.term.open) { NS.term.open({ argv: ['gh', 'auth', 'login'], cwd: p.path, title: 'gh auth login' }); return; }
    call('open_terminal', { path: p.path }).catch((e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true));
  }

  function open(projectId, opts) {
    const p = NS.store.project(projectId || NS.projects.current());
    if (!p) { NS.shell.toast(t('pub.no_project'), true); return; }
    const dlg = document.getElementById('dlg');
    const st = { name: repoName(p.path), priv: true, desc: '', stage: 'form', err: null, url: null, commits: null, branch: '' };
    const close = () => { dlg.close(); dlg.replaceChildren(); dlg.classList.remove('wide'); };
    dlg.oncancel = () => { dlg.classList.remove('wide'); };

    // what leaves: the branch and the number of commits; the secrets check runs in the core before anything is sent
    NS.store.loadRepo(p.id).then((r) => { st.branch = (r && (r.current_branch || r.base_branch)) || p.base_branch || ''; draw(); });
    call('git_log_page', { path: p.path, skip: 0, limit: 1000 }).then((rows) => { st.commits = (rows || []).length; draw(); }).catch(() => { st.commits = 0; draw(); });

    async function go() {
      st.stage = 'busy'; draw();
      try { st.url = await call('gh_publish', { path: p.path, name: st.name, private: st.priv, desc: st.desc }); st.stage = 'done'; S.repos.delete(p.id); if (opts && opts.onDone) opts.onDone(st.url); }
      catch (e) { st.err = errView(typeof e === 'string' ? safeParse(e) : e); st.stage = 'error'; }
      draw();
    }
    const safeParse = (s) => { try { return JSON.parse(s); } catch (e) { return { kind: s }; } };

    function draw() {
      if (!dlg.open) return;
      const acts = [], kids = [h('h2', { text: t('pub.title', { name: p.name }) })];
      if (st.stage === 'form') {
        const name = h('input', { type: 'text', value: st.name, spellcheck: false, autocomplete: 'off', 'aria-label': t('pub.name'), oninput: (e) => { st.name = e.target.value.trim(); bad.hidden = validName(st.name); next.disabled = !validName(st.name); } });
        const bad = h('p', { class: 'mk-err', hidden: validName(st.name), text: t('pub.err.bad_name') });
        const desc = h('input', { type: 'text', value: st.desc, spellcheck: false, 'aria-label': t('pub.desc'), placeholder: t('pub.desc_ph'), oninput: (e) => { st.desc = e.target.value; } });
        const vis = h('div', { class: 'seg', role: 'radiogroup', 'aria-label': t('pub.visibility') },
          h('button', { type: 'button', role: 'radio', 'aria-checked': String(st.priv), class: st.priv ? 'on' : '', text: t('pub.private'), onclick: () => { st.priv = true; draw(); } }),
          h('button', { type: 'button', role: 'radio', 'aria-checked': String(!st.priv), class: st.priv ? '' : 'on', text: t('pub.public'), onclick: () => { st.priv = false; draw(); } }));
        const next = h('button', { type: 'button', class: 'btn primary', disabled: !validName(st.name), text: t('pub.next'), onclick: () => { st.stage = 'confirm'; draw(); } });
        kids.push(h('label', { class: 'pubf' }, h('span', { text: t('pub.name') }), name), bad, h('div', { class: 'pubf' }, h('span', { text: t('pub.visibility') }), vis,
          h('small', { class: 'mute', text: t(st.priv ? 'pub.private_d' : 'pub.public_d') })), h('label', { class: 'pubf' }, h('span', { text: t('pub.desc') }), desc),
          h('div', { class: 'pubwhat' }, h('h3', { text: t('pub.what') }),
            h('ul', {}, h('li', { text: t('pub.what_branch', { branch: st.branch || '…' }) }), h('li', { text: st.commits == null ? t('mk.loading') : t('pub.what_commits', { n: st.commits >= 1000 ? '1000+' : st.commits }) }),
              h('li', { text: t('pub.what_secrets') }))));
        acts.push(h('button', { type: 'button', class: 'btn', text: t('dlg.cancel'), onclick: close }), next);
      } else if (st.stage === 'confirm') {
        kids.push(h('p', { class: 'pubask', text: t(st.priv ? 'pub.ask_private' : 'pub.ask_public', { name: st.name, branch: st.branch || '', n: st.commits == null ? '' : st.commits >= 1000 ? '1000+' : st.commits }) }),
          st.priv ? null : h('p', { class: 'mk-warn' }, NS.ic('i-warn'), h('span', { text: t('pub.public_warn') })));
        acts.push(h('button', { type: 'button', class: 'btn', text: t('pub.back'), onclick: () => { st.stage = 'form'; draw(); } }), h('button', { type: 'button', class: 'btn primary', text: t('pub.do'), onclick: go }));
      } else if (st.stage === 'busy') {
        kids.push(h('p', { class: 'mute', role: 'status', text: t('pub.busy') }));
      } else if (st.stage === 'done') {
        kids.push(h('p', { text: t('pub.done') }), h('button', { type: 'button', class: 'linkb', text: st.url, onclick: () => root.NSMarket.safeHttps(st.url) && call('open_path', { path: st.url }).catch(() => {}) }));
        acts.push(h('button', { type: 'button', class: 'btn', text: t('mk.close'), onclick: close }), h('button', { type: 'button', class: 'btn primary', text: t('pub.view'), onclick: () => { if (root.NSMarket.safeHttps(st.url)) call('open_path', { path: st.url }).catch(() => {}); close(); } }));
      } else {
        const e = st.err;
        kids.push(h('p', { class: 'mk-err', role: 'alert', text: t(e.key, { detail: e.detail }) }));
        if (e.paths.length) kids.push(h('ul', { class: 'pubpaths' }, e.paths.map((x) => h('li', {}, h('code', { text: x })))), h('p', { class: 'mute', text: t('pub.paths_d') }));
        if (e.action === 'install') kids.push(h('p', { class: 'mute' }, h('code', { text: root.NSGhClone ? root.NSGhClone.installCmd(navigator.userAgent) : 'brew install gh' })));
        acts.push(h('button', { type: 'button', class: 'btn', text: t('mk.close'), onclick: close }));
        if (e.action === 'login') acts.push(h('button', { type: 'button', class: 'btn primary', text: t('pub.login'), onclick: () => { close(); loginInTerminal(p); } }));
        if (e.action === 'rename' || e.paths.length) acts.push(h('button', { type: 'button', class: 'btn primary', text: e.paths.length ? t('pub.again') : t('pub.rename'), onclick: () => { st.stage = 'form'; draw(); } }));
      }
      clear(dlg).append(...kids, h('div', { class: 'dlg-acts' }, acts));
    }
    dlg.classList.add('wide');
    dlg.showModal();
    draw();
  }
  root.NS.registry.extend('command', { id: 'publish.open', labelKey: 'pub.cmd', group: 'mk.group', order: 74, when: () => !!NS.store.s.projects.length, run: () => open() });
  NS.publish = { open };
})(typeof window !== 'undefined' ? window : globalThis);
