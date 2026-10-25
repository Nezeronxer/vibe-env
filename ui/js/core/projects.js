// Adding a project: folder dialog / drop -> add_project; a folder without git asks "Сделать git-репозиторием?" and runs init_repo on the button.
(function () {
  const NS = window.NS;
  const { h, ic, clear } = NS;
  const t = (k, p) => NS.t(k, p);

  // modal question on a native <dialog>: resolves true / false; Esc or "Отмена" = false
  function confirm(title, text, okLabel) {
    return new Promise((resolve) => {
      const dlg = document.getElementById('dlg');
      const done = (v) => { dlg.close(); dlg.replaceChildren(); resolve(v); };
      dlg.replaceChildren(h('h2', { text: title }), h('p', { text }),
        h('div', { class: 'dlg-acts' }, h('button', { type: 'button', class: 'btn', text: t('dlg.cancel'), onclick: () => done(false) }),
          h('button', { type: 'button', class: 'btn primary', text: okLabel, onclick: () => done(true) })));
      dlg.oncancel = () => resolve(false);
      dlg.showModal();
    });
  }

  async function pickFolder() {
    const d = window.__TAURI__ && window.__TAURI__.dialog;
    if (!d || !d.open) return null;
    try { const p = await d.open({ directory: true, multiple: false }); return Array.isArray(p) ? p[0] : p; } catch (e) { return null; }
  }

  // -> the project, or null (cancelled / failed, the reason is already shown)
  async function addByPath(path) {
    const call = NS.api.call;
    let p;
    try { p = await call('add_project', { path }); }
    catch (e) {
      if (!NS.isNotGit(e)) { NS.shell.toast(NS.errHuman(NS.api.errText(e)), true); return null; }
      if (!(await confirm(t('git.init_title'), t('git.init_text'), t('git.init_ok')))) return null;
      try { await call('init_repo', { path }); p = await call('add_project', { path }); }
      catch (e2) { NS.shell.toast(NS.errHuman(NS.api.errText(e2)), true); return null; }
    }
    NS.store.s.projects = await call('list_projects');
    NS.store.s.lastProject = p.id;
    NS.store.emit('project', p.id);
    return p;
  }
  async function addByDialog() { const path = await pickFolder(); return path ? addByPath(path) : null; }

  // the project a new thing goes to: the last explicit choice (kept across restarts) or the one set in settings; never "the first one"
  function current() {
    const st = NS.store, S = st.s;
    const ok = (id) => (id && st.project(id) ? id : null);
    return ok(S.lastProject) || ok(NS.storage.get('ns.lastProject')) || ok(S.settings && S.settings.default_project);
  }
  function choose(id) {
    NS.store.s.lastProject = id;
    NS.storage.set('ns.lastProject', id);
    NS.store.emit('project', id);
  }

  // "Создать проект…": the parent folder from the system dialog, then a name; the core makes the folder, git and the first commit. -> the project or null
  function createDialog() {
    return new Promise((resolve) => {
      const dlg = document.getElementById('dlg');
      let parent = null, busy = false;
      const done = (v) => { dlg.close(); dlg.replaceChildren(); resolve(v); };
      const where = h('span', { class: 'pj-where mute', text: t('pj.no_folder') });
      const name = h('input', { type: 'text', class: 'pj-name', placeholder: t('pj.name_ph'), 'aria-label': t('pj.name'), autocomplete: 'off', spellcheck: 'false' });
      const err = h('small', { class: 'pj-err', role: 'alert', hidden: true });
      const make = h('button', { type: 'button', class: 'btn primary', text: t('pj.make'), disabled: true });
      const bad = (v) => /[\\/]/.test(v) || /^\.+$/.test(v);
      const sync = () => { const v = name.value.trim(); make.disabled = busy || !parent || !v; err.hidden = true; if (v && bad(v)) { err.textContent = t('err.bad_project_name'); err.hidden = false; make.disabled = true; } };
      const choose1 = h('button', { type: 'button', class: 'btn sm', text: t('pj.choose'), onclick: async () => { const p = await pickFolder(); if (p) { parent = p; where.textContent = p; where.classList.remove('mute'); sync(); name.focus(); } } });
      const go = async () => {
        if (make.disabled) return;
        busy = true; sync();
        try {
          const p = await NS.api.call('create_project', { parent, name: name.value.trim() });
          NS.store.s.projects = await NS.api.call('list_projects');
          choose(p.id);
          done(p);
        } catch (e) { busy = false; sync(); err.textContent = NS.errHuman(NS.api.errText(e)); err.hidden = false; }
      };
      name.addEventListener('input', sync);
      name.addEventListener('keydown', (e) => { if (e.key === 'Enter' && !e.isComposing) { e.preventDefault(); go(); } });
      make.onclick = go;
      dlg.replaceChildren(h('h2', { text: t('pj.new_title') }), h('p', { text: t('pj.new_text') }),
        h('div', { class: 'pj-f' }, h('span', { text: t('pj.where') }), h('div', { class: 'pj-row' }, where, choose1)),
        h('label', { class: 'pj-f' }, h('span', { text: t('pj.name') }), name), err,
        h('div', { class: 'dlg-acts' }, h('button', { type: 'button', class: 'btn', text: t('dlg.cancel'), onclick: () => done(null) }), make));
      dlg.oncancel = () => resolve(null);
      dlg.showModal();
    });
  }

  // the project drop-down of a new chat (F11): recent projects, "Открыть папку…", "Создать проект…". Looks like the model menu of F7.
  // -> { el: the button with its menu, open(), close() }; onPick(project) fires after every choice, also after open / create
  function picker(getId, onPick) {
    const st = NS.store, S = st.s;
    const btn = h('button', { type: 'button', class: 'pj-btn', 'aria-haspopup': 'menu', 'aria-expanded': 'false' });
    const pop = h('div', { class: 'pop pj-pop', role: 'menu', 'aria-label': t('chat.project'), hidden: true });
    const el = h('div', { class: 'pj-pick' }, btn, pop);
    const rows = () => [...pop.querySelectorAll('.mrow')];
    function label() {
      const p = st.project(getId());
      clear(btn).append(ic('i-branch', 'pj-ic'), h('span', { class: 'pj-t', text: p ? t('pj.cur', { name: p.name }) : t('pj.pick') }), ic('i-chev', 'chev'));
      btn.classList.toggle('empty', !p);
    }
    function close(refocus) { const was = !pop.hidden; pop.hidden = true; btn.setAttribute('aria-expanded', 'false'); if (was && refocus) btn.focus(); }
    const pick = (p) => { if (!p) return; close(true); choose(p.id); if (onPick) onPick(p); label(); };
    function row(o) {
      const r = h('div', { class: 'mrow' + (o.on ? ' on' : ''), role: 'menuitem', tabIndex: -1, title: o.tip || null, onclick: o.run,
        onkeydown: (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); o.run(); } } },
      h('span', { class: 'mn' }, o.title, o.sub ? h('small', { class: 'pj-sub', text: o.sub }) : null), o.small ? h('small', { text: o.small }) : null,
      h('span', { class: 'mck', 'aria-hidden': 'true' }, o.on ? ic('i-check') : null));
      return r;
    }
    function draw() {
      const cur = getId(), list = h('div', { class: 'mm-list' });
      list.append(h('h4', { class: 'mgroup' }, t('pj.recent')));
      if (!S.projects.length) list.append(h('p', { class: 'hint', text: t('pj.none') }));
      for (const p of S.projects) {
        const repo = S.repos.get(p.id);
        list.append(row({ on: p.id === cur, title: p.name, sub: p.path, tip: p.path, small: repo ? repo.current_branch || p.base_branch : p.base_branch, run: () => pick(p) }));
        if (!S.repos.has(p.id)) st.loadRepo(p.id).then(() => { if (!pop.hidden) draw(); }).catch(() => {});
      }
      const foot = h('div', { class: 'mm-foot', role: 'group' },
        row({ title: t('pj.open'), run: async () => { close(); const p = await addByDialog(); if (p) pick(p); } }),
        row({ title: t('pj.create'), run: async () => { close(); const p = await createDialog(); if (p) pick(p); } }));
      clear(pop).append(list, foot);
    }
    function open() { draw(); pop.hidden = false; btn.setAttribute('aria-expanded', 'true'); (rows().find((r) => r.classList.contains('on')) || rows()[0]).focus(); }
    btn.onclick = (e) => { e.stopPropagation(); if (pop.hidden) open(); else close(); };
    pop.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') { e.stopPropagation(); close(true); return; }
      if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
      e.preventDefault();
      const rs = rows(), at = rs.indexOf(document.activeElement), n = e.key === 'ArrowDown' ? at + 1 : at - 1;
      if (rs[n]) rs[n].focus();
    });
    const onDoc = (e) => { if (!el.isConnected) return; if (!el.contains(e.target)) close(); };
    document.addEventListener('click', onDoc);
    label();
    return { el, open, close, label, destroy: () => document.removeEventListener('click', onDoc) };   // 19: the pane that made it removes it
  }

  NS.projects = { addByPath, addByDialog, pickFolder, confirm, current, choose, createDialog, picker };
})();
