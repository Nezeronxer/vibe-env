// v13 "С GitHub…": the GitHub account of gh in the app - who is signed in, their repositories with a search, any
// public one by `owner/name` or a link; the chosen one is cloned into a folder (~/Projects by default) and becomes the
// project. Signing in is `gh auth login --web` in a terminal tab: the app never sees a token.
(function (root) {
  // `owner/name` from a name or a github.com link; null otherwise (the core checks it again)
  function fullName(s) {
    const v = String(s || '').trim();
    const m = /^(?:(?:https?:\/\/)?github\.com\/|git@github\.com:)([^/\s]+)\/([^/\s]+?)(?:\.git)?(?:\/.*)?$/.exec(v) || /^([^/\s]+)\/([^/\s]+?)(?:\.git)?$/.exec(v);
    if (!m) return null;
    const ok = (w, re) => w.length <= 100 && !/^[-.]/.test(w) && re.test(w);
    return ok(m[1], /^[A-Za-z0-9-]+$/) && ok(m[2], /^[A-Za-z0-9._-]+$/) ? m[1] + '/' + m[2] : null;
  }
  // the error of gh_account -> what the dialog shows: install, login, or the text
  const state = (e) => (/gh_not_installed/.test(e) ? 'install' : /gh_not_logged_in/.test(e) ? 'login' : 'error');
  const installCmd = (ua) => (/Windows/.test(ua) ? 'winget install --id GitHub.cli' : /Mac/.test(ua) ? 'brew install gh' : 'https://cli.github.com');
  root.NSGhClone = { fullName, state, installCmd };
  const NS = root.NS;
  if (!NS || !NS.h || typeof document === 'undefined') return;

  const { h, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const call = (c, a) => NS.api.call(c, a);
  const ua = () => (typeof navigator !== 'undefined' ? navigator.userAgent : '');

  function login(done) {
    done(null);
    if (NS.term && NS.term.open) NS.term.open({ argv: ['gh', 'auth', 'login', '--web', '--hostname', 'github.com', '--git-protocol', 'https'], title: 'gh auth login' });
    NS.shell.toast(t('ghc.after_login'));
  }

  // -> the new project or null
  function dialog() {
    return new Promise((resolve) => {
      const dlg = document.getElementById('dlg');
      let parent = NS.storage.get('ns.projParent') || '~/Projects', repos = [], pick = null, busy = false;
      const done = (v) => { dlg.close(); dlg.replaceChildren(); resolve(v); };
      const head = h('p', { class: 'ghc-who', text: t('ghc.checking') });
      const body = h('div', { class: 'ghc-b' });
      const err = h('small', { class: 'pj-err', role: 'alert', hidden: true });
      const go = h('button', { type: 'button', class: 'btn primary', text: t('ghc.clone'), disabled: true });
      const acts = h('div', { class: 'dlg-acts' }, h('button', { type: 'button', class: 'btn', text: t('dlg.cancel'), onclick: () => done(null) }), go);
      dlg.replaceChildren(h('div', { class: 'ghc' }, h('h2', { text: t('ghc.title') }), head, body, err, acts));
      dlg.oncancel = () => resolve(null);
      dlg.showModal();

      function sync() { go.disabled = busy || !pick || !parent; go.textContent = t(busy ? 'ghc.cloning' : 'ghc.clone'); }
      async function check() {
        clear(body); head.textContent = t('ghc.checking'); go.hidden = true;
        let who;
        try { who = await call('gh_account'); } catch (e) { return signedOut(NS.api.errText(e)); }
        head.textContent = t('ghc.signed_in', { login: who });
        go.hidden = false;
        list();
      }
      function signedOut(e) {
        const s = state(e);
        head.textContent = t('ghc.s_' + s);
        const again = h('button', { type: 'button', class: 'btn', text: t('ghc.again'), onclick: check });
        if (s === 'install') clear(body).append(h('p', {}, h('code', { text: installCmd(ua()) })), h('div', { class: 'pj-row' }, again));
        else if (s === 'login') clear(body).append(h('p', { class: 'mute', text: t('ghc.login_how') }),
          h('div', { class: 'pj-row' }, h('button', { type: 'button', class: 'btn primary', text: t('ghc.login'), onclick: () => login(done) }), again));
        else clear(body).append(h('p', { class: 'mute', text: e }), h('div', { class: 'pj-row' }, again));
      }
      function list() {
        const q = h('input', { type: 'search', class: 'pj-name', placeholder: t('ghc.search_ph'), 'aria-label': t('ghc.search'), autocomplete: 'off', spellcheck: 'false' });
        const rows = h('div', { class: 'ghc-list', role: 'listbox', 'aria-label': t('ghc.repos') });
        const where = h('span', { class: 'pj-where', text: parent, title: parent });
        const choose = h('button', { type: 'button', class: 'btn sm', text: t('pj.choose'), onclick: async () => { const p = await NS.projects.pickFolder(); if (p) { parent = p; where.textContent = p; where.title = p; sync(); } } });
        clear(body).append(q, rows, h('div', { class: 'pj-f' }, h('span', { text: t('ghc.where') }), h('div', { class: 'pj-row' }, where, choose)));
        const row = (full, sub, small) => {
          const on = pick === full;
          return h('div', { class: 'mrow ghc-r' + (on ? ' on' : ''), role: 'option', 'aria-selected': String(on), tabIndex: 0, title: full,
            onclick: () => { pick = full; err.hidden = true; draw(); sync(); },
            ondblclick: () => { pick = full; sync(); clone(); },
            onkeydown: (e) => { if (e.key === 'Enter') { e.preventDefault(); pick = full; sync(); clone(); } } },
          h('span', { class: 'mn' }, full, sub ? h('small', { class: 'pj-sub', text: sub }) : null), small ? h('small', { text: small }) : null);
        };
        function draw() {
          // a pasted link is looked up by its owner/name (the whole link matched nothing, though the repository was in the list)
          const typed = fullName(q.value), v = (typed || q.value.trim()).toLowerCase();
          const found = repos.filter((r) => !v || r.nameWithOwner.toLowerCase().includes(v) || String(r.description || '').toLowerCase().includes(v));
          clear(rows);
          const shown = [];
          if (typed && !repos.some((r) => r.nameWithOwner.toLowerCase() === typed.toLowerCase())) { rows.append(row(typed, t('ghc.by_name'), '')); shown.push(typed); }
          for (const r of found.slice(0, 100)) {
            const tags = [r.isPrivate ? t('ghc.private') : '', r.isFork ? t('ghc.fork') : '', r.primaryLanguage && r.primaryLanguage.name].filter(Boolean).join(' · ');
            rows.append(row(r.nameWithOwner, r.description || '', tags));
            shown.push(r.nameWithOwner);
          }
          // the choice lives only while its row is on screen: "Склонировать" cloned a repository the search had hidden
          if (pick && !shown.includes(pick)) pick = null;
          if (!rows.children.length) rows.append(h('p', { class: 'hint', text: repos.length ? t('ghc.none_q') : t('ghc.none') }));
        }
        q.addEventListener('input', () => { const typed = fullName(q.value); if (typed) pick = (repos.find((r) => r.nameWithOwner.toLowerCase() === typed.toLowerCase()) || {}).nameWithOwner || typed; draw(); sync(); });
        rows.append(h('p', { class: 'hint', text: t('ghc.loading') }));
        call('gh_repos').then((r) => { repos = Array.isArray(r) ? r : []; }).catch((e) => { err.textContent = NS.api.errText(e); err.hidden = false; }).then(() => { if (dlg.open) { draw(); sync(); } });
        setTimeout(() => q.focus(), 0);
      }
      async function clone() {
        if (busy || !pick) return;
        busy = true; err.hidden = true; sync();
        try {
          const p = await call('clone_project', { parent, repo: pick });
          NS.storage.set('ns.projParent', parent);
          NS.store.s.projects = await call('list_projects');
          NS.projects.choose(p.id);
          done(p);
        } catch (e) { busy = false; sync(); err.textContent = NS.errHuman(NS.api.errText(e)); err.hidden = false; }
      }
      go.onclick = clone;
      check();
    });
  }
  NS.projects.githubDialog = dialog;
})(typeof window !== 'undefined' ? window : globalThis);
