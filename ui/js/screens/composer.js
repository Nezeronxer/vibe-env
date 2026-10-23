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
      const e = st.env(envId), m = NS.models.label(envId);
      envText.textContent = m || e.name;
      pickBtn.title = e.name + (m ? ' · ' + m : '');
      readSettings(); readStart();
      qSw.set(quality.on); sSw.set(!!swarm);
      applyMode();
      drawHint();
    }
    function applyMode() {
      const e = st.env(envId);
      ta.placeholder = mode.kind === 'answer' ? t('composer.ph_answer', { title: mode.task.title }) : mode.kind === 'hint' ? t('composer.ph_hint', { title: mode.task.title }) : t('composer.ph', { env: e.name });
      modebar.hidden = mode.kind === 'message';
      if (mode.kind !== 'message') modebar.textContent = (mode.kind === 'answer' ? t('composer.mode_answer', { title: mode.task.title }) : t('composer.mode_hint', { title: mode.task.title })) + ' · ' + t('composer.esc');
    }
    function setMode(kind, task) { mode = { kind, task: task || null }; applyMode(); if (kind !== 'message') ta.focus(); }
    function drawHint() {
      const v = L.validateSend(ta.value);
      sendBtn.disabled = !v.ok;
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
    ta.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) { e.preventDefault(); send(); }
      else if (e.key === 'Escape' && mode.kind !== 'message') { e.stopPropagation(); setMode('message'); }
    });

    // ---- send ----
    async function send() {
      const text = ta.value, v = L.validateSend(text);
      if (!v.ok) { if (v.reason === 'empty') ta.focus(); else drawHint(); return; }
      if (hooks.needProject && mode.kind === 'message' && hooks.needProject()) return;     // F11: a new chat needs a chosen project
      const id = gate.begin(key, text);
      if (id == null) return;                      // a double press while the first call is still running
      sendBtn.disabled = true;
      try {
        await dispatch(text, id);
        ta.value = ''; NS.storage.del(key); gate.done(); setMode('message'); grow();
      } catch (e) {
        gate.fail();                               // the same text will reuse this id, the core ignores a repeat
        if (hooks.dropPending) hooks.dropPending(id);
        NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
      }
      drawHint();
    }
    async function dispatch(text, id) {
      const call = NS.api.call;
      if (mode.kind === 'answer') return call('answer', { id: mode.task.id, text });
      if (mode.kind === 'hint') return call('resume_failed', { taskId: mode.task.id, hint: text, model: null });
      const active = hooks.activeTask && hooks.activeTask();
      if (active) {
        const to = hooks.noteTo ? hooks.noteTo() : null;               // swarm: "Всем" = null, a variant letter or "reviewer" (story 156)
        if (hooks.addPending) hooks.addPending({ id, text, at: new Date().toISOString(), to });
        if (active.status === 'needs_human' && active.question) return call('answer', { id: active.id, text });
        return call('note', { taskId: active.id, to, text });      // G05: goes to the next run of the executor
      }
      const target = chat === 'new' || !chat ? { new: { env: envId, project_id: hooks.projectId() } } : { existing: chat };
      if (hooks.addPending) hooks.addPending({ id, text, at: new Date().toISOString() });
      const chatId = await call('send_chat', { target, msgId: id, text });
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
    const offSettings = st.on('settings', (id) => { if (id === envId || id == null) { const m = NS.models.label(envId); envText.textContent = m || st.env(envId).name; } });   // a model chosen in the palette
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
          h('h3', { class: 'when', text: t('tune.when') }), h('div', { class: 'seg', role: 'radiogroup', 'aria-label': t('tune.when') }, startBtns))));
    }

    qSw = makeSwitch(t('composer.quality'), true, (v) => { quality.on = v; save(); if (!tunePanel.hidden) drawTune(); }, t('composer.quality_d'));
    sSw = makeSwitch(t('composer.swarm'), false, (v) => { swarm = v ? { mode: 'race', n: 3 } : null; save(); drawHint(); if (!tunePanel.hidden) drawTune(); }, t('composer.swarm_d'));
    form.append(plate, modebar, h('div', { class: 'cmp-line' }, ta),
      h('div', { class: 'comp-row' }, pickBtn, qSw, sSw, tuneBtn, h('span', { class: 'comp-gap' }), stopBtn, sendBtn),
      hint, swhint, modelPop, tunePanel);
    // under the field, quiet: the keys, and in a new chat where the terminal is
    const box = h('div', { class: 'cmpbox' }, form, h('p', { class: 'cmp-keys', text: t('composer.keys') }), chat === 'new' || !chat ? h('p', { class: 'cmp-note', text: t('composer.note_new') }) : null);
    refresh();
    if (ta.value) setTimeout(grow, 0);
    // an example from the empty chat goes into the field, it is not sent (story 239)
    function insert(text) { ta.value = text; ta.dispatchEvent(new Event('input')); ta.focus(); ta.setSelectionRange(ta.value.length, ta.value.length); }
    return { el: box, form, ta, refresh, setMode, setWork, drawHint, focus: () => ta.focus(), closePops, insert,
      destroy() { usedN.delete(n); offSettings(); document.removeEventListener('click', onDoc); document.removeEventListener('keydown', onKey); } };
  }

  NS.composer = { create };
})();
