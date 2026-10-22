// The input field: exactly five controls - "<Среда> · <Модель>", "Качество", "Рой", "настроить", "Отправить ↵" (G19).
// Everything rarer lives under "настроить" (quality steps, swarm mode/count, "Когда начать").
(function () {
  const NS = window.NS;
  const { h, ic, envIcon, clear } = NS;
  const L = window.NSLogic;
  const t = (k, p) => NS.t(k, p);
  const QUALITY0 = { on: true, spec: true, plan: true, tests_first: true, e2e: false, best_of_n: 2 };
  const STEPS = [['spec', 'q.spec', 'q.spec_d'], ['plan', 'q.plan', ''], ['tests_first', 'q.tests', 'q.tests_d'], ['gates', 'q.gates', 'q.always', true],
    ['reviewer', 'q.review', 'q.review_d', true], ['e2e', 'q.e2e', 'q.e2e_d'], ['best_of_n', 'q.bestn', 'q.bestn_d']];
  const MODES = [['race', 'swarm.race', 'swarm.race_d'], ['crew', 'swarm.crew', 'swarm.crew_d'], ['council', 'swarm.council', 'swarm.council_d']];

  function makeSwitch(text, on, onToggle) {
    const b = h('button', { type: 'button', role: 'switch', class: 'tg', 'aria-checked': String(!!on) }, h('span', { class: 'sw' }, h('i')), text);
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
    const ta = h('textarea', { id: 'taskText' + sfx, rows: 2, 'aria-label': t('composer.label') });
    const modebar = h('div', { class: 'modebar', hidden: true });
    const hint = h('p', { class: 'cmp-hint', role: 'alert', hidden: true });
    const swhint = h('small', { class: 'swhint', hidden: true });
    const envText = h('span'), pickBtn = h('button', { type: 'button', class: 'envpick', 'aria-expanded': 'false', 'aria-controls': 'modelPop' + sfx }, h('span', { class: 'icw' }, envIcon(envId)), envText, ic('i-chev', 'chev'));
    const sendBtn = h('button', { type: 'submit', class: 'btn primary', id: 'sendBtn' + sfx }, t('composer.send'), h('kbd', { text: '↵' }));
    const tuneBtn = h('button', { type: 'button', class: 'linkb', id: 'tune' + sfx, 'aria-expanded': 'false', 'aria-controls': 'tunePanel' + sfx, text: t('composer.tune') });
    const modelPop = h('div', { class: 'pop', id: 'modelPop' + sfx, hidden: true, role: 'dialog' });
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
      envText.textContent = e.name + (m ? ' · ' + m : '');
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
      hint.hidden = v.reason !== 'too_long';
      hint.textContent = v.reason === 'too_long' ? t('composer.too_long') : '';
      const sh = swarm ? L.swarmHint((L.limitsView(st.s.limits.get(envId)).windows), swarm.n || 3) : null;
      swhint.hidden = !sh;
      swhint.textContent = sh ? t('swarm.hint', sh) : '';
    }

    // ---- draft: one per chat, survives a restart ----
    ta.value = NS.storage.get(key) || '';
    ta.addEventListener('input', () => { if (ta.value) NS.storage.set(key, ta.value); else NS.storage.del(key); drawHint(); });
    ta.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) { e.preventDefault(); send(); }
      else if (e.key === 'Escape' && mode.kind !== 'message') { e.stopPropagation(); setMode('message'); }
    });

    // ---- send ----
    async function send() {
      const text = ta.value, v = L.validateSend(text);
      if (!v.ok) { if (v.reason === 'empty') ta.focus(); else drawHint(); return; }
      const id = gate.begin(key, text);
      if (id == null) return;                      // a double press while the first call is still running
      sendBtn.disabled = true;
      try {
        await dispatch(text, id);
        ta.value = ''; NS.storage.del(key); gate.done(); setMode('message');
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
      for (const [btn, pop] of [[pickBtn, modelPop], [tuneBtn, tunePanel]]) { was = was || !pop.hidden; pop.hidden = true; btn.setAttribute('aria-expanded', 'false'); }
      return was;
    }
    function togglePop(btn, pop, draw) {
      const open = pop.hidden; closePops();
      if (open) { draw(); pop.hidden = false; btn.setAttribute('aria-expanded', 'true'); }
    }
    pickBtn.onclick = (e) => { e.stopPropagation(); togglePop(pickBtn, modelPop, drawModelPop); };
    tuneBtn.onclick = (e) => { e.stopPropagation(); togglePop(tuneBtn, tunePanel, drawTune); };
    const onDoc = (e) => { if (e.target.isConnected && !form.contains(e.target)) closePops(); };
    const onKey = (e) => { if (e.key === 'Escape' && closePops()) e.preventDefault(); };
    document.addEventListener('click', onDoc); document.addEventListener('keydown', onKey);
    form.addEventListener('click', (e) => { if (e.target.isConnected && !e.target.closest('.pop') && !e.target.closest('.envpick') && e.target !== tuneBtn) closePops(); });

    function drawModelPop() {
      const e = st.env(envId);
      clear(modelPop).append(h('div', { class: 'pop-h' }, h('b', { text: t('model.title', { env: e.name }) })),
        NS.models.picker(envId, () => refresh()),
        h('div', { class: 'pop-f' }, h('span', { class: 'hint', text: t('model.other_env') }),
          h('button', { type: 'button', class: 'linkb', text: t('model.default_in_settings'), onclick: () => NS.nav('settings', { section: 'env', env: envId }) })));
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

    qSw = makeSwitch(t('composer.quality'), true, (v) => { quality.on = v; save(); if (!tunePanel.hidden) drawTune(); });
    sSw = makeSwitch(t('composer.swarm'), false, (v) => { swarm = v ? { mode: 'race', n: 3 } : null; save(); drawHint(); if (!tunePanel.hidden) drawTune(); });
    form.append(modebar, ta, hint,
      h('div', { class: 'comp-row' }, pickBtn, h('span', { class: 'vsep' }), qSw, sSw, tuneBtn, h('span', { class: 'grow' }), sendBtn),
      swhint, modelPop, tunePanel);
    refresh();
    // an example from the empty chat goes into the field, it is not sent (story 239)
    function insert(text) { ta.value = text; ta.dispatchEvent(new Event('input')); ta.focus(); ta.setSelectionRange(ta.value.length, ta.value.length); }
    return { el: form, ta, refresh, setMode, drawHint, focus: () => ta.focus(), closePops, insert,
      destroy() { usedN.delete(n); document.removeEventListener('click', onDoc); document.removeEventListener('keydown', onKey); } };
  }

  NS.composer = { create };
})();
