// The input field: the model, "Качество", "Рой", "Настроить" and a round send button (G19).
// F9: a rounded field that grows from 1 to 8 lines, the controls in a row under the text, the plate "what the agent does · Стоп" on top while it works.
// Everything rarer lives under "настроить" (quality steps, swarm mode/count, "Когда начать").
(function () {
  const NS = window.NS;
  const { h, ic, envIcon, clear } = NS;
  const L = window.NSLogic;
  const t = (k, p) => NS.t(k, p);
  const QUALITY0 = { on: true, spec: true, plan: true, tests_first: true, e2e: false, best_of_n: 2 };
  const STEPS = [['spec', 'q.spec', 'q.spec_d'], ['plan', 'q.plan', ''], ['tests_first', 'q.tests', 'q.tests_d'], ['gates', 'q.gates', 'q.always', true],
    ['reviewer', 'q.review', 'q.review_d', true], ['e2e', 'q.e2e', 'q.e2e_d'], ['best_of_n', 'q.bestn', 'q.bestn_d']];
  const MAX_H = 8 * 22;       // eight lines of the field
  const MODES = [['race', 'swarm.race', 'swarm.race_d'], ['crew', 'swarm.crew', 'swarm.crew_d'], ['council', 'swarm.council', 'swarm.council_d']];

  function makeSwitch(text, on, onToggle, title) {
    const b = h('button', { type: 'button', role: 'switch', class: 'tg', title: title || text, 'aria-checked': String(!!on) }, h('span', { class: 'sw' }, h('i')), text);
    b.onclick = () => { const v = b.getAttribute('aria-checked') !== 'true'; b.setAttribute('aria-checked', String(v)); onToggle(v); };
    b.set = (v) => b.setAttribute('aria-checked', String(!!v));
    return b;
  }
  const modeOf = (sw) => (sw && L.variant(sw.mode)[0]) || 'race';

  // ids stay unique when several panels each have an input field (tabs.js): the first one keeps the plain ids
  const usedN = new Set();
  // v6 G65.2: the input fields on screen; a file dropped on one of them is an image for it, not "open this folder" (app.js asks first)
  const fields = new Set();
  NS.composerDrop = (pl) => {
    const paths = (pl && pl.paths) || [], pos = pl && pl.position, dpr = window.devicePixelRatio || 1;
    if (!paths.length || !pos) return false;
    const imgs = paths.filter((p) => /\.(png|jpe?g|gif|webp)$/i.test(String(p)));
    if (!imgs.length) return false;                // a folder or another file: app.js adds it as a project
    const x = pos.x / dpr, y = pos.y / dpr;
    for (const f of fields) {
      const r = f.form.getBoundingClientRect();
      if (r.width && x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) { f.dropPaths(imgs); return true; }
    }
    return false;
  };
  function create(opts) {
    let n = 1; while (usedN.has(n)) n++;
    usedN.add(n);
    const sfx = n === 1 ? '' : '-' + n;
    const { envId, chat, hooks } = opts;
    const st = NS.store, key = L.draftKey(envId, chat === 'new' ? null : chat);
    const gate = L.createSendGate(() => (crypto.randomUUID ? crypto.randomUUID() : 'm-' + Date.now() + '-' + Math.random().toString(16).slice(2)));
    let mode = { kind: 'message', task: null };
    const form = h('form', { class: 'composer', onsubmit: (e) => { e.preventDefault(); send(); } });
    const ta = h('textarea', { id: 'taskText' + sfx, rows: 1, 'aria-label': t('composer.label'), spellcheck: false });
    const grow = () => { NS.setVars(ta, { height: 'auto' }); NS.setVars(ta, { height: Math.min(ta.scrollHeight, MAX_H) + 'px' }); };
    const modebar = h('div', { class: 'modebar', hidden: true });
    const hint = h('p', { class: 'cmp-hint', role: 'alert', hidden: true });
    const swhint = h('small', { class: 'swhint', hidden: true });
    const envText = h('span'), pickBtn = h('button', { type: 'button', class: 'envpick', 'aria-expanded': 'false', 'aria-haspopup': 'listbox', 'aria-controls': 'modelPop' + sfx }, h('span', { class: 'icw' }, envIcon(envId)), envText, ic('i-chev', 'chev'));
    const SVGNS = 'http://www.w3.org/2000/svg';
    const svgOf = (cls, paths, fill) => { const v = document.createElementNS(SVGNS, 'svg'); v.setAttribute('viewBox', '0 0 16 16'); v.setAttribute('class', cls); v.setAttribute('aria-hidden', 'true');
      for (const d of paths) { const p = document.createElementNS(SVGNS, 'path'); p.setAttribute('d', d); if (fill) p.setAttribute('fill', 'currentColor'); v.append(p); } return v; };
    const sendBtn = h('button', { type: 'submit', class: 'send4', id: 'sendBtn' + sfx, 'aria-label': t('composer.send'), title: t('composer.send') + ' (Enter)' }, svgOf('send-g', ['M8 13V3', 'M3.5 7.5L8 3l4.5 4.5']));
    // F12 #8: while the executor works, a message waits in the queue; "Сразу" stops the run and passes it first
    const nowBtn = h('button', { type: 'button', class: 'now4', id: 'nowBtn' + sfx, text: t('chatx.now'), title: t('chatx.now_d'), hidden: true });
    // F12 #11: the next step as a grey line in the empty field; Tab takes it, any key hides it
    let sugg = null;
    const tabTag = h('kbd', { class: 'cmp-tab', 'aria-hidden': 'true', text: 'Tab', hidden: true });
    const hintOn = () => NS.storage.get('ns.hint_next') !== '0';
    // v6 G65: images of the message - thumbnails over the text, × takes one off; the clip shows only for an env that reads images
    const atts = [];
    const attStrip = h('div', { class: 'atts6', role: 'list', 'aria-label': t('img.limit'), hidden: true });
    const clipBtn = h('button', { type: 'button', class: 'clip6', id: 'clipBtn' + sfx, 'aria-label': t('img.attach'), title: t('img.attach') + ' · ' + t('img.limit'), hidden: true },
      svgOf('clip-g', ['M10.6 4.4 5.9 9.1a1.5 1.5 0 0 0 2.1 2.1l4.9-4.9a2.9 2.9 0 0 0-4.1-4.1L3.9 7.1a4.3 4.3 0 0 0 6.1 6.1l4.2-4.2']));
    let caps = null;                                   // {images, resume, checked} of the env, from `chat_shell`
    const projectOf = () => (hooks.projectId ? hooks.projectId() : null);
    const chatOf = () => (hooks.chatId ? hooks.chatId() : (chat === 'new' || !chat ? null : chat));
    if (NS.slash) NS.slash.shell(envId, projectOf()).then((r) => { caps = r || { images: false }; clipBtn.hidden = !caps.images; });
    const toastErr = (k) => NS.shell.toast(NS.errHuman(k), true);
    function drawAtts() {
      clear(attStrip);
      atts.forEach((a, i) => attStrip.append(h('div', { class: 'att6', role: 'listitem' },
        a.preview ? h('img', { src: a.preview, alt: t('img.alt', { n: i + 1 }) }) : h('span', { class: 'att6-n', text: t('img.chip') }),
        h('button', { type: 'button', class: 'att6-x', 'aria-label': t('img.remove'), title: t('img.remove'), text: '×', onclick: () => {
          atts.splice(atts.indexOf(a), 1); drawAtts(); drawHint(); ta.focus();
          NS.api.call('chat_detach', { path: a.path }).catch(() => {});
        } }))));
      attStrip.hidden = !atts.length;
    }
    // can an image be taken now: the env reads images, the message goes to the model (not to a task), room for one more
    function canTake() {
      if (!caps || !caps.images) { toastErr('images_not_supported'); return false; }
      if (mode.kind !== 'message' || (hooks.activeTask && hooks.activeTask())) { NS.shell.toast(t('img.task'), true); return false; }
      if (atts.length >= L5) { toastErr('too_many_images'); return false; }
      return true;
    }
    const L5 = 5, MB10 = 10 * 1024 * 1024;
    async function addFile(file) {
      if (!canTake()) return;
      if (!/^image\/(png|jpe?g|gif|webp)$/.test(file.type || '')) return toastErr('not_an_image');
      if (file.size > MB10) return toastErr('image_too_big');
      const url = await new Promise((res, rej) => { const r = new FileReader(); r.onload = () => res(String(r.result)); r.onerror = () => rej(r.error); r.readAsDataURL(file); }).catch(() => null);
      if (!url) return toastErr('not_an_image');
      try {
        const a = await NS.api.call('chat_attach', { id: chatOf(), data: url.slice(url.indexOf(',') + 1) });
        atts.push({ path: a.path, preview: url }); drawAtts(); drawHint();
      } catch (e) { toastErr(NS.api.errText(e)); }
    }
    async function addPath(path) {
      if (!canTake()) return;
      try { const a = await NS.api.call('chat_attach_path', { id: chatOf(), path }); atts.push({ path: a.path, preview: a.preview || null }); drawAtts(); drawHint(); }
      catch (e) { toastErr(NS.api.errText(e)); }
    }
    clipBtn.onclick = async () => {
      if (!canTake()) return;
      const d = window.__TAURI__ && window.__TAURI__.dialog;
      let p = null;
      try { p = d && d.open ? await d.open({ multiple: true, filters: [{ name: 'image', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp'] }] }) : null; } catch (e) { /* the dialog was closed */ }
      for (const x of [].concat(p || []).slice(0, L5 - atts.length)) await addPath(x);
    };
    const stopBtn = h('button', { type: 'button', class: 'send4 stop4', id: 'stopBtn' + sfx, 'aria-label': t('composer.stop'), title: t('composer.stop'), hidden: true }, svgOf('stop-g', ['M4.5 4.5h7v7h-7z'], true));
    // the plate over the field: who works and on what, with "Стоп" when there is a task to cancel (pin-85)
    let work = null;
    const wbWho = h('b'), wbWhat = h('span');
    const wbStop = h('button', { type: 'button', class: 'wb-stop', text: t('work.stop'), title: t('composer.stop') });
    const plate = h('div', { class: 'workbar', role: 'status', 'aria-label': t('work.label'), hidden: true }, h('span', { class: 'wb-dot', 'aria-hidden': 'true' }), h('div', { class: 'wb-t' }, wbWho, wbWhat), wbStop);
    const doStop = () => { if (work && work.stop) work.stop(); };
    stopBtn.onclick = doStop; wbStop.onclick = doStop;
    function setWork(w) {
      work = w || null;
      plate.hidden = !work;
      if (work) { wbWho.textContent = work.who || ''; wbWhat.textContent = work.what ? ' · ' + work.what : ''; wbStop.hidden = !work.stop; }
      drawHint();
    }
    const tuneBtn = h('button', { type: 'button', class: 'tune4', id: 'tune' + sfx, 'aria-expanded': 'false', 'aria-controls': 'tunePanel' + sfx, 'aria-label': t('composer.tune'), title: t('composer.tune_d') }, ic('i-gear'), h('span', { class: 'tune-w', text: t('composer.tune') }));
    const modelPop = h('div', { class: 'pop mmenu', id: 'modelPop' + sfx, hidden: true });
    const tunePanel = h('div', { class: 'pop', id: 'tunePanel' + sfx, hidden: true, role: 'dialog' });
    let quality, swarm, qSw, sSw;

    const isBusy = () => !!(hooks.busy && !hooks.noteTo && hooks.busy());
    const settings = () => st.envSettings(envId);
    function readSettings() {
      const s = settings();
      quality = Object.assign({}, QUALITY0, s.last_quality || {});
      swarm = s.last_swarm || null;
    }
    const save = () => st.saveEnvSettings(envId, { last_quality: quality, last_swarm: swarm, last_start: startVal }).catch((e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true));
    let startVal = 'now';
    const readStart = () => { startVal = L.variant(settings().last_start || 'now')[0] === 'night' ? 'night' : 'now'; };

    // ---- label, placeholder, switches ----
    function refresh() {
      const e = st.env(envId), m = NS.models.buttonLabel(envId);
      envText.textContent = m || e.name;
      pickBtn.title = e.name + (m ? ' · ' + m : '');
      readSettings(); readStart();
      qSw.set(quality.on); sSw.set(!!swarm);
      applyMode();
      drawHint();
    }
    function applyMode() {
      const e = st.env(envId);
      const showSugg = mode.kind === 'message' && !!sugg && hintOn();
      tabTag.hidden = !showSugg || !!ta.value;
      ta.placeholder = mode.kind === 'answer' ? t('composer.ph_answer', { title: mode.task.title }) : mode.kind === 'hint' ? t('composer.ph_hint', { title: mode.task.title }) : showSugg ? sugg : t('composer.ph', { env: e.name });
      modebar.hidden = mode.kind === 'message';
      if (mode.kind !== 'message') modebar.textContent = (mode.kind === 'answer' ? t('composer.mode_answer', { title: mode.task.title }) : t('composer.mode_hint', { title: mode.task.title })) + ' · ' + t('composer.esc');
    }
    function setMode(kind, task) { mode = { kind, task: task || null }; applyMode(); if (kind !== 'message') ta.focus(); }
    function drawHint() {
      const v = L.validateSend(ta.value);
      sendBtn.disabled = !v.ok;
      nowBtn.hidden = !(v.ok && mode.kind === 'message' && isBusy());
      tabTag.hidden = !(sugg && hintOn() && mode.kind === 'message') || !!ta.value;
      const showStop = !!(work && work.stop) && !ta.value.trim();           // working and nothing typed: the round button stops the run
      stopBtn.hidden = !showStop; sendBtn.hidden = showStop;
      hint.hidden = v.reason !== 'too_long';
      hint.textContent = v.reason === 'too_long' ? t('composer.too_long') : '';
      const sh = swarm ? L.swarmHint((L.limitsView(st.s.limits.get(envId)).windows), swarm.n || 3) : null;
      swhint.hidden = !sh;
      swhint.textContent = sh ? t('swarm.hint', sh) : '';
    }

    // ---- draft: one per chat, survives a restart ----
    ta.value = NS.storage.get(key) || '';
    ta.addEventListener('input', () => { if (ta.value) NS.storage.set(key, ta.value); else NS.storage.del(key); drawHint(); grow(); });
    // G65: ⌘V of a screenshot (a file on the clipboard) becomes an image of the message; text pastes as text
    ta.addEventListener('paste', (e) => {
      const files = [...((e.clipboardData && e.clipboardData.items) || [])].filter((i) => i.kind === 'file').map((i) => i.getAsFile()).filter(Boolean);
      if (!files.length) return;
      e.preventDefault();
      files.forEach((f) => addFile(f));
    });
    // a file dragged onto the field in a browser (in the app the window's drop event comes through NS.composerDrop)
    form.addEventListener('dragover', (e) => { if (e.dataTransfer && [...e.dataTransfer.types].includes('Files')) { e.preventDefault(); form.classList.add('drop6'); } });
    form.addEventListener('dragleave', () => form.classList.remove('drop6'));
    form.addEventListener('drop', (e) => { const f = e.dataTransfer && e.dataTransfer.files; form.classList.remove('drop6'); if (!f || !f.length) return; e.preventDefault(); e.stopPropagation(); [...f].forEach((x) => addFile(x)); });
    const me = { form, dropPaths: (paths) => { paths.forEach((p) => addPath(p)); } };
    fields.add(me);
    // G82: the menu "/" of the env over the field
    const slash = NS.slash ? NS.slash.attach({ ta, form, envId, projectId: projectOf, onTerminal: hooks.openTerminal || null }) : null;
    ta.addEventListener('keydown', (e) => {
      if (slash && slash.key(e)) return;
      if (e.key === 'Tab' && !e.shiftKey && sugg && hintOn() && !ta.value && mode.kind === 'message') { e.preventDefault(); const s = sugg; sugg = null; applyMode(); insert(s); return; }
      if (sugg && !['Shift', 'Meta', 'Control', 'Alt'].includes(e.key)) { sugg = null; applyMode(); }
      if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) { e.preventDefault(); send(); }
      else if (e.key === 'Escape' && mode.kind !== 'message') { e.stopPropagation(); setMode('message'); }
    });

    // ---- send ----
    async function send() {
      const text = ta.value, v = L.validateSend(text);
      if (!v.ok) { if (v.reason === 'empty') ta.focus(); else drawHint(); return; }
      if (hooks.needProject && mode.kind === 'message' && hooks.needProject()) return;     // F11: a new chat needs a chosen project
      // G83: the same conversation is open in the terminal: sending from here is asked first
      if (mode.kind === 'message' && hooks.inTerminal && hooks.inTerminal() && !(await NS.projects.confirm(t('shell.term_warn_t'), t('shell.term_warn'), t('shell.term_send')))) return;
      const files = atts.map((a) => a.path);
      if (mode.kind === 'message' && isBusy() && hooks.enqueue) {                              // F12 #8: the agent works: the message waits, grey, until it is done
        hooks.enqueue(text, files); ta.value = ''; NS.storage.del(key); atts.length = 0; drawAtts(); grow(); drawHint(); return;
      }
      let id = gate.begin(key, text);
      if (id == null) return;                      // a double press while the first call is still running
      // G62.3: the id is born before the call and kept with the draft: the same text sent again after a restart (or a
      // crash before the answer) carries the same id, and the core takes it once
      const idKey = key + '.id', saved = String(NS.storage.get(idKey) || ''), cut = saved.indexOf('\n');
      if (cut > 0 && saved.slice(cut + 1) === text) id = saved.slice(0, cut);
      NS.storage.set(idKey, id + '\n' + text);
      sendBtn.disabled = true;
      try {
        await dispatch(text, id, files);
        ta.value = ''; NS.storage.del(key); NS.storage.del(idKey); gate.done(); setMode('message'); grow();
        if (files.length) { atts.length = 0; drawAtts(); }
      } catch (e) {
        gate.fail();                               // the same text will reuse this id, the core ignores a repeat
        if (hooks.dropPending) hooks.dropPending(id);
        NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
      }
      drawHint();
    }
    nowBtn.onclick = () => {
      const text = ta.value;
      if (!L.validateSend(text).ok || !hooks.now) return;
      hooks.now(text, atts.map((a) => a.path)); ta.value = ''; NS.storage.del(key); atts.length = 0; drawAtts(); grow(); drawHint();
    };
    async function dispatch(text, id, files) {
      const call = NS.api.call;
      if (mode.kind === 'answer') return call('answer', { id: mode.task.id, text });
      if (mode.kind === 'hint') return call('resume_failed', { taskId: mode.task.id, hint: text, model: null });
      const active = hooks.activeTask && hooks.activeTask();
      // an ordinary chat sends through the chat: the core passes it to the task (note or answer) and keeps it in the history
      // under msgId, which also clears the pending bubble; only the swarm, with its "кому", talks to the task directly
      if (active && (hooks.noteTo || chat === 'new' || !chat)) {
        const to = hooks.noteTo ? hooks.noteTo() : null;               // swarm: "Всем" = null, a variant letter or "reviewer" (story 156)
        if (hooks.addPending) hooks.addPending({ id, text, at: new Date().toISOString(), to });
        const done = () => { if (hooks.dropPending) hooks.dropPending(id); };
        if (active.status === 'needs_human' && active.question) return call('answer', { id: active.id, text }).then(done);
        return call('note', { taskId: active.id, to, text }).then(done);      // G05: goes to the next run of the executor
      }
      const target = chat === 'new' || !chat ? { new: { env: envId, project_id: hooks.projectId() } } : { existing: chat };
      if (hooks.addPending) hooks.addPending({ id, text, at: new Date().toISOString() });
      const chatId = await call('send_chat', files && files.length ? { target, msgId: id, text, attachments: files } : { target, msgId: id, text });
      if (hooks.sent) hooks.sent(chatId);
    }

    // ---- pops ----
    function closePops() {                         // -> true when something was open
      let was = false;
      const back = !modelPop.hidden && modelPop.contains(document.activeElement);   // the focus must not fall to <body> with the menu
      for (const [btn, pop] of [[pickBtn, modelPop], [tuneBtn, tunePanel]]) { was = was || !pop.hidden; pop.hidden = true; btn.setAttribute('aria-expanded', 'false'); }
      if (back) pickBtn.focus();
      return was;
    }
    function togglePop(btn, pop, draw) {
      const open = pop.hidden; closePops();
      if (open) { draw(); pop.hidden = false; btn.setAttribute('aria-expanded', 'true'); if (pop === modelPop) { place(); modelPop.firstChild.focusStart(); } }
    }
    pickBtn.onclick = (e) => { e.stopPropagation(); togglePop(pickBtn, modelPop, drawModelPop); };
    tuneBtn.onclick = (e) => { e.stopPropagation(); togglePop(tuneBtn, tunePanel, drawTune); };
    const onDoc = (e) => { if (e.target.isConnected && !form.contains(e.target)) closePops(); };
    const onKey = (e) => { if (e.key === 'Escape' && closePops()) e.preventDefault(); };
    document.addEventListener('click', onDoc); document.addEventListener('keydown', onKey);
    const offSettings = st.on('settings', (id) => { if (id === envId || id == null) { const m = NS.models.buttonLabel(envId); envText.textContent = m || st.env(envId).name; } });   // a model or a thinking level chosen in the palette
    form.addEventListener('click', (e) => { if (e.target.isConnected && !e.target.closest('.pop') && !e.target.closest('.envpick') && e.target !== tuneBtn) closePops(); });

    // the menu hangs over the button: its right edge on the button's, never past the pane, and no taller than the room above it
    function place() {
      const f = form.getBoundingClientRect(), b = pickBtn.getBoundingClientRect(), w = modelPop.offsetWidth;
      NS.setVars(modelPop, { '--mr': Math.max(0, Math.min(f.right - b.right, f.width - w)) + 'px', '--mh': Math.max(160, Math.min(360, b.top - 12)) + 'px' });
    }
    function drawModelPop() {
      clear(modelPop).append(NS.models.menu(envId, { onDone: () => { closePops(); refresh(); } }));
    }
    function drawTune() {
      const q = quality;
      const steps = STEPS.map(([id, label, desc, locked]) => {
        const checked = locked ? true : id === 'best_of_n' ? !!q.best_of_n : !!q[id];
        const cb = h('input', { type: 'checkbox', checked, disabled: !!locked || !q.on, onchange: () => {
          if (id === 'best_of_n') quality.best_of_n = cb.checked ? 2 : null; else quality[id] = cb.checked;
          save();
        } });
        return h('li', {}, h('label', { class: 'step' }, cb, h('b', { text: t(label) }), desc ? h('span', { text: t(desc) }) : null));
      });
      const modeBtns = MODES.map(([id, label, desc]) => h('button', { type: 'button', class: 'mode' + (modeOf(swarm) === id ? ' on' : ''), role: 'radio', 'aria-checked': String(modeOf(swarm) === id),
        onclick: () => { swarm = Object.assign({ n: 3 }, swarm, { mode: id }); sSw.set(true); save(); drawTune(); drawHint(); } }, h('b', { text: t(label) }), h('span', { text: t(desc) })));
      const n = (swarm && swarm.n) || 3;
      const setN = (v) => { swarm = Object.assign({ mode: 'race' }, swarm, { n: Math.max(2, Math.min(5, v)) }); sSw.set(true); save(); drawTune(); drawHint(); };
      const startBtns = [['now', 'start.now'], ['night', 'start.night']].map(([v, k]) => h('button', { type: 'button', class: startVal === v ? 'on' : '', role: 'radio', 'aria-checked': String(startVal === v),
        text: t(k), onclick: () => { startVal = v; save(); drawTune(); } }));
      clear(tunePanel).append(h('div', { class: 'tune' },
        h('div', {}, h('h3', { text: t('tune.quality') }), h('ul', { class: 'steps' }, steps)),
        h('div', {}, h('h3', { text: t('tune.swarm') }), h('div', { class: 'modes', role: 'radiogroup', 'aria-label': t('tune.swarm_mode') }, modeBtns),
          h('div', { class: 'count' }, h('span', { class: 'field-l', text: t('tune.agents') }),
            h('div', { class: 'stepper' }, h('button', { type: 'button', 'aria-label': t('tune.less'), text: '−', onclick: () => setN(n - 1) }), h('output', { text: String(n) }),
              h('button', { type: 'button', 'aria-label': t('tune.more'), text: '+', onclick: () => setN(n + 1) }))),
          h('h3', { class: 'when', text: t('tune.when') }), h('div', { class: 'seg', role: 'radiogroup', 'aria-label': t('tune.when') }, startBtns),
          makeSwitch(t('chatx.hint_on'), hintOn(), (v) => { NS.storage.set('ns.hint_next', v ? '1' : '0'); if (!v) { tabTag.hidden = true; } applyMode(); }, t('chatx.hint_on_d')))));
    }

    qSw = makeSwitch(t('composer.quality'), true, (v) => { quality.on = v; save(); if (!tunePanel.hidden) drawTune(); }, t('composer.quality_d'));
    sSw = makeSwitch(t('composer.swarm'), false, (v) => { swarm = v ? { mode: 'race', n: 3 } : null; save(); drawHint(); if (!tunePanel.hidden) drawTune(); }, t('composer.swarm_d'));
    form.append(plate, modebar, attStrip, h('div', { class: 'cmp-line' }, ta, tabTag),
      // v10 (Agentation 29.10): Рой / Качество live in the middle of the bar; here only «Настроить» for this message - no duplicate switches
      h('div', { class: 'comp-row' }, pickBtn, tuneBtn, h('span', { class: 'comp-gap' }), clipBtn, stopBtn, nowBtn, sendBtn),
      hint, swhint, modelPop, tunePanel);
    // under the field, quiet: the keys, and in a new chat where the terminal is
    const box = h('div', { class: 'cmpbox' }, form, h('p', { class: 'cmp-keys', text: t('composer.keys') }), chat === 'new' || !chat ? h('p', { class: 'cmp-note', text: t('composer.note_new') }) : null);
    refresh();
    if (ta.value) setTimeout(grow, 0);
    // an example from the empty chat goes into the field, it is not sent (story 239)
    function insert(text) { ta.value = text; ta.dispatchEvent(new Event('input')); ta.focus(); ta.setSelectionRange(ta.value.length, ta.value.length); }
    // F12: text a queued message sends (the same path as a typed one), and the grey suggestion
    const deliver = (text, id) => dispatch(text, id || (crypto.randomUUID ? crypto.randomUUID() : 'q-' + Date.now()));
    function suggest(text) { sugg = text || null; applyMode(); drawHint(); }
    return { el: box, form, ta, refresh, setMode, setWork, drawHint, deliver, suggest, accept: () => ta.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', cancelable: true })), focus: () => ta.focus(), closePops, insert,
      atts: () => atts.slice(), addPath,
      destroy() { usedN.delete(n); fields.delete(me); offSettings(); document.removeEventListener('click', onDoc); document.removeEventListener('keydown', onKey); } };
  }

  NS.composer = { create };
})();
