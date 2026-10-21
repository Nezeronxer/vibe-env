// Adding a project: folder dialog / drop -> add_project; a folder without git asks "Сделать git-репозиторием?" and runs init_repo on the button.
(function () {
  const NS = window.NS;
  const { h } = NS;
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

  NS.projects = { addByPath, addByDialog, pickFolder, confirm };
})();
