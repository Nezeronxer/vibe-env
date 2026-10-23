// F15: the app preview (#10) and the proof screenshots of "Проверить как пользователь" (#3).
// The preview pane holds the controls; the page itself is a Tauri window of its own (app/src/preview.rs: only localhost,
// no capabilities), never an iframe in this window. "Указать элемент" puts a reference to the clicked element into the
// task chat field. The proof strip is the row of thumbnails in the "готово" card (taskcard.js calls NS.preview.strip).
(function () {
  const NS = window.NS;
  const { h, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const call = (cmd, args) => NS.api.call(cmd, args);
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
  const quiet = (a) => (window.NSPalette ? window.NSPalette.quiet(a) : a);
  for (const c of ['preview_open', 'preview_reload', 'preview_width', 'preview_close', 'preview_pick', 'proof_shot']) NS.api.COMMANDS.add(c);
  NS.api.EVENTS.add('preview-pick');

  const WIDTHS = [['390', 'pv.w_phone'], ['768', 'pv.w_tablet'], ['', 'pv.w_desk']];
  // the same check as core::preview::local_url, for a word at once; the app checks again in Rust
  const LOCAL = /^http:\/\/(localhost|127\.0\.0\.1)(:(\d{1,5}))?([/?#][^\s\\]*)?$/i;
  const isLocal = (s) => { const m = LOCAL.exec(String(s || '').trim()); return !!m && (!m[3] || (+m[3] >= 1 && +m[3] <= 65535)); };

  let adopt = null;                      // the fields of the pane `open` asks for (create() takes them)
  let shown = null;                      // the address in the preview window, while the app thinks it is open
  let picking = null;                    // {task, url, done(text)} of the pane that waits for a pick
  const views = new Set();
  const redraw = () => views.forEach((v) => v.draw());

  const taskOf = (id) => (id ? NS.store.s.tasks.get(id) || null : null);
  const urlFor = (task, project) => (task && task.port ? 'http://localhost:' + task.port + '/' : '') || (project && project.app_url) || '';
  const projectOfTask = (task) => (task ? NS.store.project(task.project_id) : null);
  async function saveProject(p, patch) {
    const next = Object.assign({}, p, patch);
    await call('update_project', { project: next });
    const i = NS.store.s.projects.findIndex((x) => x.id === p.id);
    if (i >= 0) NS.store.s.projects[i] = next;
    return next;
  }

  // the chat field of the task (its card sits in that chat); a project preview goes to the chat in front
  function insertToChat(taskId, text) {
    const card = taskId ? document.querySelector('.tcard[data-task="' + CSS.escape(taskId) + '"]') : null;
    const convo = card ? card.closest('.convo') : taskId ? null : document.querySelector('.convo');
    const ta = convo && convo.querySelector('.composer textarea');
    if (!ta) return false;
    ta.value = ta.value.trim() ? ta.value.replace(/\s+$/, '') + '\n' + text : text;
    ta.dispatchEvent(new Event('input', { bubbles: true }));
    ta.focus(); ta.setSelectionRange(ta.value.length, ta.value.length);
    return true;
  }
  const pickText = (p, url) => t('pv.pick_text', { url, sel: p.selector, text: p.text ? t('pv.pick_q', { text: p.text.slice(0, 80) }) : '', w: p.w, h: p.h });
  NS.api.listen('preview-pick', (p) => {
    if (!picking || !p || !p.selector) return;
    const w = picking; picking = null;
    w.done(insertToChat(w.task, pickText(p, w.url)) ? t('pv.st_picked', { sel: p.selector }) : t('pv.st_nochat'));
  });

  // ---------- the pane ----------
  function mount(el, pane) {
    const task = () => taskOf(pane.task);
    const project = () => NS.store.project(pane.project) || projectOfTask(task());
    const input = h('input', quiet({ type: 'text', class: 'pv4-url', placeholder: t('pv.url_ph'), 'aria-label': t('pv.url'), value: pane.url || urlFor(task(), project()), maxlength: '2048' }));
    const status = h('p', { class: 'pv4-st', 'aria-live': 'polite' });
    let note = null, width = '';
    const say = (text, bad) => { note = { text, bad }; draw(); };
    async function open() {
      const url = input.value.trim();
      if (!isLocal(url)) { input.setAttribute('aria-invalid', 'true'); return say(t('err.not_local_url'), true); }
      input.removeAttribute('aria-invalid');
      try {
        shown = await call('preview_open', { url });
        pane.url = shown; note = null;
        const p = project(), tk = task();
        if (p && !(tk && tk.port) && p.app_url !== shown) saveProject(p, { app_url: shown }).then(redraw).catch(fail);   // the proof step shoots the same address
        redraw();
      } catch (e) { say(NS.errHuman(NS.api.errText(e)), true); }
    }
    input.onkeydown = (e) => { if (e.key === 'Enter') { e.preventDefault(); open(); } };
    const act = (cmd, args) => call(cmd, args).catch((e) => { if (/preview_closed|not_local_url/.test(NS.api.errText(e))) shown = null; say(NS.errHuman(NS.api.errText(e)), true); });
    const reload = h('button', { type: 'button', class: 'btn sm', text: t('pv.reload'), onclick: () => act('preview_reload') });
    const seg = h('div', { class: 'seg pv4-w', role: 'radiogroup', 'aria-label': t('pv.width') }, WIDTHS.map(([px, key]) =>
      h('button', { type: 'button', role: 'radio', dataset: { px }, text: t(key) + (px ? ' ' + px : ''), onclick: () => { width = px; draw(); act('preview_width', { px: px ? +px : null }); } })));
    const pick = h('button', { type: 'button', class: 'btn sm pv4-pick', text: t('pv.pick'), title: t('pv.pick_d'), onclick: async () => {
      try {
        await call('preview_pick');
        picking = { task: pane.task || null, url: shown, done: (text) => say(text) };
        say(t('pv.st_pick'));
      } catch (e) { if (/preview_closed|not_local_url/.test(NS.api.errText(e))) shown = null; say(NS.errHuman(NS.api.errText(e)), true); }
    } });
    const shut = h('button', { type: 'button', class: 'linkb', text: t('pv.close'), onclick: () => { act('preview_close'); shown = null; picking = null; note = null; redraw(); } });
    const proofBox = h('div', { class: 'pv4-proof' });
    const head = h('header', { class: 'pv4-h' });
    el.append(h('section', { class: 'pv4' }, head,
      h('div', { class: 'pv4-row' }, h('label', { class: 'field-l', text: t('pv.url') }), input, h('button', { type: 'button', class: 'btn sm primary', text: t('pv.open'), onclick: open }), shut),
      h('div', { class: 'pv4-row pv4-tools' }, reload, seg, pick), status, proofBox, h('p', { class: 'hint pv4-note', text: t('pv.note') })));

    function draw() {
      const tk = task(), p = project();
      NS.append(clear(head), [h('h2', { text: t('pv.title') }), tk ? h('small', { text: t('pv.of', { title: tk.title }) + (tk.port ? ' · ' + t('pv.port', { port: tk.port }) : '') }) : p ? h('small', { text: p.name }) : null]);
      const open = !!shown;
      reload.disabled = !open; pick.disabled = !open; shut.hidden = !open;
      for (const b of seg.children) { b.disabled = !open; const on = b.dataset.px === width; b.classList.toggle('on', on); b.setAttribute('aria-checked', String(on)); }
      status.textContent = note ? note.text : open ? t('pv.st_open', { url: shown }) : t('pv.st_closed');
      status.classList.toggle('bad', !!(note && note.bad));
      clear(proofBox);
      if (!p) return;
      const sw = h('button', { type: 'button', role: 'switch', class: 'tg', 'aria-checked': String(!!p.proof), 'aria-label': t('pv.proof') }, h('span', { class: 'sw' }, h('i')), h('b', { text: t('pv.proof') }));
      sw.onclick = async () => {
        const v = !p.proof, url = input.value.trim();
        try { await saveProject(p, Object.assign({ proof: v }, v && !p.app_url && isLocal(url) && !(tk && tk.port) ? { app_url: url } : {})); redraw(); } catch (e) { fail(e); }
      };
      const url = urlFor(tk, p);
      proofBox.append(sw, h('p', { class: 'hint', text: t('pv.proof_d') }),
        h('p', { class: 'pv4-facts' }, h('span', { class: p.run_cmd ? '' : 'warn', text: p.run_cmd ? t('pv.proof_run', { cmd: p.run_cmd }) : t('pv.proof_norun') }), ' · ',
          h('span', { text: url ? t('pv.proof_url', { url }) : t('pv.proof_nourl') })));
    }
    const v = { draw, pane };
    views.add(v);
    draw();
    return { focus: () => input.focus(), update: (evt) => { if (evt === 'task' || evt === 'project') draw(); }, destroy: () => { views.delete(v); if (picking && picking.task === (pane.task || null)) picking = null; } };
  }

  function create() {
    const a = adopt; adopt = null;
    if (a) return a;
    const tk = focusedTask();
    return { task: tk ? tk.id : null, project: tk ? tk.project_id : ((NS.store.s.projects[0] || {}).id || null) };
  }
  // the newest task of the chat in front
  function focusedTask() {
    const f = NS.tabs && NS.tabs.active;
    const list = f && f.chat && f.chat !== 'new' ? NS.store.chatTasks(f.chat) : [];
    return list.length ? list[list.length - 1] : null;
  }
  NS.registry.extend('paneKind', { kind: 'preview', labelKey: 'pv.kind', order: 5, create, mount,
    sig: (pane) => pane.id,
    title: (pane) => { const tk = taskOf(pane.task); return t('pv.kind') + (tk ? ': ' + tk.title : ''); } });

  // opens the preview pane beside the chat (a task's, or the project's)
  function open(taskId) {
    const tk = taskOf(taskId);
    adopt = { task: tk ? tk.id : null, project: tk ? tk.project_id : ((NS.store.s.projects[0] || {}).id || null) };
    if (NS.tabs && NS.tabs.split) NS.tabs.split('h', 'preview');
  }
  NS.registry.extend('command', { id: 'preview.open', labelKey: 'pv.cmd', group: 'mk.group', order: 75, when: () => !!NS.store.s.projects.length, run: () => { const tk = focusedTask(); open(tk && tk.id); } });
  NS.registry.extend('details', { id: 'preview', order: 55, render(task) {
    if (!NS.store.project(task.project_id) || ['cancelled', 'reverted'].includes(task.status)) return null;
    return h('div', { class: 'pv4-det' }, h('button', { type: 'button', class: 'btn sm', text: t('pv.det'), onclick: () => open(task.id) }), h('small', { class: 'hint', text: t('pv.det_d') }));
  } });

  // ---------- proof screenshots in the card ----------
  const shots = new Map();               // task|file|finished -> Promise<data URL>
  const shotSrc = (task, file) => {
    const k = task.id + '|' + file + '|' + (task.finished_at || '');
    if (!shots.has(k)) shots.set(k, call('proof_shot', { taskId: task.id, file }).catch(() => ''));
    return shots.get(k);
  };
  const shotName = (file) => t('pv.shot_names').split('|')[(parseInt(file, 10) || 0) - 1] || file;
  function strip(task) {
    const pr = task && task.outcome && task.outcome.proof;
    if (!pr || !(pr.shots || []).length || !['done', 'done_not_pushed', 'awaiting_outward'].includes(task.status)) return null;
    const m = pr.shots.length;
    return h('div', { class: 'pv4-strip' }, h('small', { class: 'pv4-cap', text: t('pv.shots') }),
      h('ul', { class: 'pv4-shots' }, pr.shots.map((f, i) => {
        const img = h('img', { alt: '', decoding: 'async' });
        shotSrc(task, f).then((src) => { if (src) img.src = src; });
        return h('li', {}, h('button', { type: 'button', class: 'pv4-thumb', 'aria-label': t('pv.shot', { n: i + 1, m }) + ': ' + shotName(f), onclick: () => big(task, i) },
          img, h('small', { text: shotName(f) })));
      })));
  }
  function big(task, i0) {
    const list = task.outcome.proof.shots, dlg = document.getElementById('dlg');
    let i = i0;
    const img = h('img', { class: 'pv4-bigimg', alt: '' }), cap = h('p', { class: 'pv4-bigcap' });
    const show = () => { img.alt = shotName(list[i]); cap.textContent = t('pv.shot', { n: i + 1, m: list.length }) + ' · ' + shotName(list[i]); img.removeAttribute('src'); shotSrc(task, list[i]).then((s) => { if (s) img.src = s; }); };
    const close = () => dlg.close();
    dlg.classList.add('pv4-big');
    dlg.replaceChildren(img, h('div', { class: 'dlg-acts' }, cap, h('button', { type: 'button', class: 'btn', text: t('pv.big_close'), onclick: close })));
    dlg.onkeydown = (e) => {
      if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); close(); return; }      // the open card under it stays open
      if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') { e.preventDefault(); i = (i + (e.key === 'ArrowRight' ? 1 : list.length - 1)) % list.length; show(); } };
    dlg.onclick = (e) => { if (e.target === dlg) close(); };       // a click on the dimmed backdrop
    dlg.addEventListener('close', () => { dlg.classList.remove('pv4-big'); dlg.onkeydown = null; dlg.onclick = null; dlg.replaceChildren(); }, { once: true });
    show();
    dlg.showModal();
  }
  // the line of the checks block
  const checkLine = (task) => {
    const pr = task && task.outcome && task.outcome.proof;
    if (!pr) return null;
    return pr.skipped ? { state: 'wait', title: t('pv.chk'), text: t('pv.chk_skip', { why: NS.errHuman(pr.skipped) }) } : { state: 'ok', title: t('pv.chk'), text: t('pv.chk_ok', { n: (pr.shots || []).length }) };
  };

  NS.preview = { open, strip, checkLine, isLocal, insertToChat };
})();
