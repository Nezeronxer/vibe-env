// A task as one card in the chat feed (G24): status word + icon, title, time, step strip, the "Нужен ты" block,
// and "Подробнее" = one panel with Changes / Checks / Details. No tabs, no lanes (lanes only live in Swarm and Factory).
(function () {
  const NS = window.NS;
  const { h, ic, roleIcon, envIcon, vendorIcon, clear } = NS;
  const L = window.NSLogic;
  const t = (k, p) => NS.t(k, p);
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
  const call = (cmd, args) => NS.api.call(cmd, args).catch(fail);

  const KIND_CLASS = { wait: 'st-wait', run: 'st-run', queue: 'st-run', done: 'st-ok', fail: 'st-bad', talk: '' };
  const NEED_ICON = { question: 'i-ask', outward: 'i-key', fail: 'i-fail' };
  const MAX_FILES = 200, MAX_LINES = 2000;

  function statusBadge(task) {
    const stamped = task.status === 'cancelled' || task.status === 'reverted';   // an undone task is stamped, never dropped
    const hold = L.kindOf(task) === 'run' && !L.info(task.status).live;            // paused on a limit, blocked merge: not on air
    return h('span', { class: 'st ' + (stamped ? 'st-stamp' : hold ? 'st-hold' : L.info(task.status).warn ? 'st-bad' : KIND_CLASS[L.kindOf(task)]) }, ic('i-' + L.iconOf(task)), t(L.labelKey(task)));
  }
  function elapsed(task) {
    const start = task.started_at && Date.parse(task.started_at);
    if (!start) return '';
    const end = task.finished_at ? Date.parse(task.finished_at) : Date.now();
    return L.formatDuration(end - start, NS.lang());
  }
  const workerRole = (task) => {
    const run = (task.runs || []).slice().reverse().find((r) => L.roleKey(L.variant(r.role)[0]) && L.variant(r.state)[0] === 'running') || (task.runs || []).find((r) => L.roleKey(r.role) === 'exec');
    return run ? L.roleKey(run.role) : 'exec';
  };
  const workPath = (task) => task.worktree || (NS.store.project(task.project_id) || {}).path;

  // ---------- F13: snapshots of the copy ("Вернуть код сюда"), shared with the chat ----------
  const snapCache = new Map(), snapWatch = new Map();
  let snapBus = false;
  const snapsOf = (taskId) => {
    if (!snapCache.has(taskId)) snapCache.set(taskId, NS.api.call('task_snapshots', { taskId }).then((l) => l || []).catch(() => []));
    return snapCache.get(taskId);
  };
  const rollbackable = (task) => !!task && !!task.worktree && !task.taken_from && !(L.kindOf(task) === 'run' && L.info(task.status).live) && !['done', 'cancelled', 'reverted'].includes(task.status);
  // a button that asks twice (the current state is saved, so this is reversible, but it still moves the code); pick(list) = the snapshot it returns to
  function rollbackBtn(taskId, pick, cls) {
    const b = h('button', { type: 'button', class: cls || 'linkb snap-back', hidden: true, title: t('snap.rollback_d') });
    let n = null;
    const label = () => { b.textContent = t('snap.rollback'); b.prepend(ic('i-back')); };
    const refresh = async () => {
      const task = NS.store.s.tasks.get(taskId);
      const sn = rollbackable(task) ? pick(await snapsOf(taskId)) : null;
      n = sn ? sn.n : null;
      b.hidden = n == null;
      if (!b.dataset.armed) label();
    };
    b.onclick = async () => {
      if (!b.dataset.armed) { b.dataset.armed = '1'; b.textContent = t('snap.sure'); setTimeout(() => { delete b.dataset.armed; label(); }, 3000); return; }
      delete b.dataset.armed; label();
      try {
        const saved = await NS.api.call('restore_snapshot', { taskId, n });
        snapCache.delete(taskId);
        NS.shell.toast(t('snap.done', { n, saved }));
        refresh();
      } catch (e) { fail(e); }
    };
    if (!snapBus) { snapBus = true; NS.store.on('task', (task) => { snapCache.delete(task.id); for (const f of [...(snapWatch.get(task.id) || [])]) f(); }); }
    const set = snapWatch.get(taskId) || new Set();
    snapWatch.set(taskId, set);
    const again = () => { if (!b.isConnected && b.dataset.seen) set.delete(again); else { if (b.isConnected) b.dataset.seen = '1'; refresh(); } };
    set.add(again);
    refresh();
    return b;
  }
  // the snapshot taken first at or after a moment: what the code was when the person wrote
  const snapAfter = (at) => (list) => list.find((s) => Date.parse(s.at) >= Date.parse(at) - 1000) || null;

  // ---------- F13: the list "До мержа" ----------
  const todoDraft = new Map();
  function todoList(task) {
    const todos = task.todos || [];
    const locked = ['cancelled', 'reverted'].includes(task.status);
    const list = h('ul', { class: 'todos' }, todos.map((x) => h('li', { class: x.done ? 'done' : '' },
      h('label', {}, h('input', { type: 'checkbox', checked: !!x.done, disabled: locked, 'aria-label': x.text, onchange: (e) => call('todo_set', { taskId: task.id, todoId: x.id, done: e.target.checked }) }),
        h('span', { text: x.text }), x.by === 'reviewer' ? h('small', { class: 'by', text: t('todo.by_reviewer') }) : null),
      locked ? null : h('button', { type: 'button', class: 'linkb', 'aria-label': t('todo.remove'), title: t('todo.remove'), text: '×', onclick: () => call('todo_remove', { taskId: task.id, todoId: x.id }) }))));
    if (locked) return todos.length ? list : null;
    const input = h('input', { type: 'text', class: 'todo-in', placeholder: t('todo.add'), 'aria-label': t('todo.add'), value: todoDraft.get(task.id) || '', maxlength: '200' });
    const add = () => { const v = input.value.trim(); if (!v) return; todoDraft.delete(task.id); input.value = ''; call('todo_add', { taskId: task.id, text: v }); };
    input.oninput = () => todoDraft.set(task.id, input.value);
    input.onkeydown = (e) => { if (e.key === 'Enter') { e.preventDefault(); add(); } };
    return h('div', { class: 'todo-box' }, todos.length ? list : h('p', { class: 'hint', text: t('todo.empty') }),
      h('div', { class: 'todo-add' }, input, h('button', { type: 'button', class: 'btn sm', text: t('todo.add_btn'), onclick: add })));
  }
  NS.rollbackBtn = rollbackBtn; NS.snapAfter = snapAfter;

  // ---------- the block "Нужен ты" ----------
  let closeMenu = null;   // the open "▾" menu of this card, if any (one card per task, one menu at a time)
  function needBlock(task, hooks) {
    const project = NS.store.project(task.project_id);
    const kind = L.kindOf(task);
    const role = t('role.' + workerRole(task));
    const snoozed = NS.store.s.snoozed.has(task.id);
    const answer = (text) => call('answer', { id: task.id, text });
    if (task.status === 'needs_human' && task.question && /^todos_open/.test(task.question.text)) {
      const el = needEl('question', t('need.todos_open'), t('step.merge'), NS.errHuman(task.question.text),
        [h('button', { type: 'button', class: 'btn sm human', text: t('need.todos_cont'), onclick: () => answer('Continue') })]);
      el.insertBefore(todoList(task), el.querySelector('.acts'));
      return el;
    }
    if (task.status === 'needs_human' && task.question) {
      const q = task.question;
      return needEl('question', t('need.question'), role, q.text, [
        ...(q.options || []).map((o, i) => h('button', { type: 'button', class: 'btn sm' + (i === 0 ? ' human' : ''), text: o, onclick: () => answer(o) })),
        h('button', { type: 'button', class: 'linkb', text: t('need.own_words'), onclick: () => hooks.answerMode(task) })]);
    }
    if (task.status === 'awaiting_outward') {
      const q = task.question || { text: '' };
      return needEl('outward', t('need.outward'), t('role.integ'), q.text, L.outwardChoices(project).map(([val, key], i) =>
        h('button', { type: 'button', class: 'btn sm' + (i === 0 ? ' human' : ''), text: t(key), onclick: () => call('answer_outward', { taskId: task.id, choice: val }) })));
    }
    if (task.status === 'failed' && !snoozed) {
      const cat = NS.store.s.catalogs.get(task.agent);
      const cur = (task.models && (task.models.worker || task.models.Worker)) || NS.store.envSettings(task.agent).default_model;
      const stronger = L.strongerModel(cat, cur);
      const acts = [];
      if (stronger) {
        const resume = (choice) => call('resume_failed', { taskId: task.id, hint: null, model: choice });
        const menuBtn = h('button', { type: 'button', class: 'btn sm human', 'aria-haspopup': 'menu', 'aria-label': t('need.pick_other'), text: '▾' });
        menuBtn.onclick = (e) => {
          e.stopPropagation();
          if (closeMenu) closeMenu();
          const fams = ((cat && cat.families) || []).filter((f) => !cur || f.id !== cur.family);
          const m = h('div', { class: 'menu', role: 'menu' }, fams.map((f) => h('button', { type: 'button', role: 'menuitem', text: L.modelLabel({ family: f.id, version: null }, cat),
            onclick: () => { closeMenu(); resume({ family: f.id, version: null }); } })));
          menuBtn.after(m);
          const close = (ev) => { if (!m.contains(ev.target) && closeMenu) closeMenu(); };
          closeMenu = () => { m.remove(); document.removeEventListener('click', close); closeMenu = null; };
          setTimeout(() => closeMenu && document.addEventListener('click', close), 0);
        };
        acts.push(h('span', { class: 'split' },
          h('button', { type: 'button', class: 'btn sm human', text: t('need.stronger', { model: L.modelLabel(stronger, cat) }), onclick: () => resume(stronger) }), menuBtn));
      }
      acts.push(h('button', { type: 'button', class: 'btn sm', text: t('need.hint'), onclick: () => hooks.hintMode(task) }));
      acts.push(h('button', { type: 'button', class: 'linkb', text: t('need.snooze'), onclick: () => NS.store.snooze(task.id) }));
      return needEl('fail', t('need.fail'), role, NS.errHuman(task.outcome && task.outcome.error) || t('need.fail_generic'), acts, true);
    }
    if (task.status === 'failed' && snoozed) {
      return h('div', { class: 'tc-done' }, ic('i-dash'), t('card.snoozed'), h('button', { type: 'button', class: 'linkb', text: t('act.retry'), onclick: () => call('retry', { id: task.id }) }));
    }
    return null;
  }
  function needEl(kind, title, role, text, acts, bad) {
    return h('div', { class: 'tc-need' + (bad ? ' bad' : ''), role: 'group', 'aria-label': title },
      h('div', { class: 'tc-need-h' }, h('b', {}, ic(NEED_ICON[kind]), title), h('span', { text: role })),
      h('p', { text }), h('div', { class: 'acts' }, acts));
  }

  // "Забрать к себе" / "Отдать агенту": the project folder on the task's branch and back
  const canTake = (task) => !!task.worktree && !task.taken_from && ['failed', 'needs_human'].includes(task.status);
  function takeBtn(task, which) {
    const cmd = which === 'give' ? 'give_to_agent' : 'take_to_main';
    return h('button', { type: 'button', class: 'btn sm', text: t('take.' + which), title: t('take.' + which + '_d'),
      onclick: async () => { try { await NS.api.call(cmd, { taskId: task.id }); NS.shell.toast(t('take.done_' + which)); } catch (e) { fail(e); } } });
  }

  // quiet one-line states: done, not pushed, paused, handed off, night start
  function stateLine(task) {
    const o = task.outcome || {}, project = NS.store.project(task.project_id);
    if (task.status === 'done') {
      return h('div', { class: 'tc-done' + (o.summary ? ' sum' : '') }, ic('i-check'), o.summary ? NS.renderMarkdown(o.summary) : (o.commit ? t('card.merged', { sha: o.commit.slice(0, 7) }) : t('st.done')),     // the agent's summary is markdown (story 228)
        o.pr_url ? h('button', { type: 'button', class: 'linkb', text: t('card.open_pr'), onclick: () => call('open_path', { path: o.pr_url }) }) : null);
    }
    if (task.status === 'done_not_pushed') {
      const cmd = L.retryPushCommand(project);
      return h('div', { class: 'tc-done warn' }, ic('i-warn'), NS.errHuman(o.error) || t('st.done_not_pushed'),
        h('button', { type: 'button', class: 'btn sm', text: t('act.retry_push'), onclick: () => (cmd === 'retry' ? call('retry', { id: task.id }) : call('push_base', { projectId: task.project_id })) }));
    }
    if (task.taken_from) {
      return h('div', { class: 'tc-done' }, ic('i-branch'), t('take.taken', { prev: t('take.taken_prev', { prev: task.taken_from }) }), takeBtn(task, 'give'));
    }
    if (task.status === 'handed_off') return h('div', { class: 'tc-done' }, ic('i-run'), t('card.handed'), h('button', { type: 'button', class: 'btn sm', text: t('act.take_back'), onclick: () => call('take_back', { taskId: task.id }) }));
    if (task.status === 'paused_limit') return h('div', { class: 'tc-done' }, ic('i-pause'), task.resume_at ? t('card.paused_until', { time: NS.hhmm(task.resume_at) }) : t('card.paused'));
    if (task.status === 'merge_blocked') return h('div', { class: 'tc-done' }, ic('i-pause'), t('card.merge_blocked'));
    if (task.status === 'queued' && task.start && L.variant(task.start)[0] === 'night') {
      return h('div', { class: 'tc-done' }, ic('i-moon'), t('card.night'), h('button', { type: 'button', class: 'btn sm', text: t('act.start_now'), onclick: () => call('start_now', { taskId: task.id }) }));
    }
    return null;
  }

  // ---------- details: Changes / Checks / Details ----------
  function diffBox(d, task, path) {
    if (d.binary) return h('p', { class: 'hint', text: t('diff.binary', { kb: d.size_kb != null ? d.size_kb : Math.max(1, Math.round((d.size || 0) / 1024)) }) });
    const raw = (d.lines || []).map((l) => (typeof l === 'string' ? { kind: l[0] === '+' ? 'add' : l[0] === '-' ? 'del' : l.startsWith('@@') ? 'hunk' : 'ctx', text: l.replace(/^[+-]/, '') } : l));
    const { shown, more } = L.capList(raw, MAX_LINES);
    const cls = { add: 'a', del: 'd', hunk: 'c', ctx: 'n' };
    const box = h('div', { class: 'diff', role: 'region', 'aria-label': path }, shown.map((l, i) => h('div', { class: 'l ' + (cls[l.kind] || 'n') },
      h('i', { text: String(i + 1) }), h('span', { text: (l.kind === 'add' ? '+ ' : l.kind === 'del' ? '- ' : '  ') + l.text }))));
    const extra = more + (d.truncated || 0);
    if (extra > 0) box.append(h('div', { class: 'more-row' }, t('diff.more', { n: extra }),
      h('button', { type: 'button', class: 'linkb', text: t('act.editor'), onclick: () => call('open_editor', { path: workPath(task) }) })));
    return box;
  }

  function createCard(task0, hooks) {
    let task = task0, open = false, files = null, selFile = null, snaps = null;
    const diffs = new Map();
    const el = h('article', { class: 'tcard', dataset: { task: task.id } });
    const top = h('div', { class: 'tc-top' }), det = h('div', { class: 'tc-det', hidden: true });
    const more = h('button', { type: 'button', class: 'linkb tc-more', 'aria-expanded': 'false', onclick: () => setOpen(!open) });
    el.append(top, det);

    function renderTop() {
      const bar = L.stepBar(task);
      NS.append(clear(top), [
        h('header', { class: 'tc-h' }, statusBadge(task), h('h2', { text: task.title }), h('small', { class: 'elapsed', text: elapsed(task) }), more),
        h('ol', { class: 'tc-steps', 'aria-label': t('card.steps') }, bar.map((s) => {
          const li = h('li', { class: { done: 'd', run: 'r', wait: 'w', fail: 'f', skip: 's', pending: '' }[s.state] },
            h('i'), h('span', { text: t('step.' + s.key) }), h('span', { class: 'sr-only', text: ' (' + t('stepst.' + s.state) + ')' }));
          if (s.state === 'skip') li.title = t('card.skipped', { why: s.reason ? NS.errHuman(s.reason) : (task.quality && task.quality.on === false ? t('skip.quality_off') : '') });
          return li;
        })),
        needBlock(task, hooks), stateLine(task)]);
      more.textContent = open ? t('card.collapse') : t('card.more');
      more.setAttribute('aria-expanded', String(open));
    }

    async function loadFiles() {
      if (files) return;
      try { files = (await NS.api.call('get_diffstat', { taskId: task.id })) || []; } catch (e) { files = []; }
      snaps = await snapsOf(task.id);
    }
    function changesBlock() {
      const o = task.outcome || {};
      const total = files || [];
      const { shown, more: rest } = L.capList(total, MAX_FILES);
      const ins = o.insertions != null ? o.insertions : total.reduce((a, f) => a + (f.insertions || 0), 0);
      const del = o.deletions != null ? o.deletions : total.reduce((a, f) => a + (f.deletions || 0), 0);
      const n = o.files_changed != null ? o.files_changed : total.length;
      const list = h('ul', { class: 'flist' });
      for (const f of shown) {
        const on = selFile === f.path;
        list.append(h('li', { class: on ? 'on' : '', tabIndex: 0, role: 'button', 'aria-expanded': String(on),
          onclick: () => pickFile(f.path), onkeydown: (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pickFile(f.path); } } },
        ic('i-chev-r', 'tw'), h('span', { text: f.path }), h('small', {}, h('span', { class: 'plus', text: '+' + (f.insertions || 0) }), ' ', h('span', { class: 'minus', text: '−' + (f.deletions || 0) }))));
        if (on) list.append(h('li', { class: 'fdiff' }, diffs.has(f.path) ? diffBox(diffs.get(f.path), task, f.path) : h('p', { class: 'hint', text: t('diff.loading') })));
      }
      if (rest > 0) list.append(h('li', { class: 'fmore' }, h('span', { text: t('diff.files_more', { n: rest }) }),
        h('button', { type: 'button', class: 'linkb', text: t('act.editor'), onclick: () => call('open_editor', { path: workPath(task) }) })));
      return h('section', { class: 'db' }, h('h3', {}, t('det.changes'), h('small', {}, t('det.files', { n, w: L.plural(n, [t('det.file_1'), t('det.file_2'), t('det.file_5')], NS.lang()) }), ' ', h('span', { class: 'plus', text: '+' + ins }), ' ', h('span', { class: 'minus', text: '−' + del }))),
        total.length ? list : h('p', { class: 'hint', text: t('det.no_changes') }));
    }
    async function pickFile(path) {
      selFile = selFile === path ? null : path;
      renderDet();
      if (selFile && !diffs.has(path)) {
        try { diffs.set(path, (await NS.api.call('diff_file', { taskId: task.id, path })) || { lines: [] }); } catch (e) { diffs.set(path, { lines: [{ kind: 'ctx', text: NS.errHuman(NS.api.errText(e)) }] }); }
        renderDet();
      }
    }
    function checksBlock() {
      const o = task.outcome || {}, project = NS.store.project(task.project_id) || {};
      const past = ['reviewing', 'merging', 'awaiting_outward', 'done', 'done_not_pushed'].includes(task.status);
      const rows = [];
      const row = (state, title, text, tail) => rows.push(h('li', {}, ic(state === 'ok' ? 'i-check' : state === 'bad' ? 'i-fail' : 'i-dash', 'chk ' + state),
        h('div', {}, h('b', { text: title }), h('span', { text }), tail ? h('details', { class: 'tail' }, h('summary', { text: t('chk.output') }), h('pre', { text: tail })) : null)));
      for (const g of o.gates || []) row(g.ok ? 'ok' : 'bad', g.name, g.note || (g.ok ? t('chk.clean') : t('chk.failed')), g.tail);
      if (o.tests) row(o.tests.passed ? 'ok' : 'bad', t('chk.tests'), o.tests.passed ? t('chk.tests_ok') : t('chk.tests_bad'), o.tests.tail);
      else if (past && !project.check_cmd) row('ok', t('chk.tests'), t('chk.no_tests'));
      else row('wait', t('chk.tests'), t('chk.later'));
      if (o.review) row(o.review.approved ? 'ok' : 'bad', t('chk.review'), o.review.approved ? t('chk.review_ok') : (o.review.issues || []).join('; ') || t('chk.review_bad'));
      else row('wait', t('chk.review'), t('chk.after_tests'));
      return h('section', { class: 'db' }, h('h3', { text: t('det.checks') }), h('ul', { class: 'clist' }, rows));
    }
    function detailsBlock() {
      const envObj = NS.store.env(task.agent), cat = NS.store.s.catalogs.get(task.agent);
      const choice = task.models && (task.models.worker || task.models.Worker);
      const worker = (task.runs || []).find((r) => L.roleKey(r.role) === 'exec');
      const fam = choice && cat && (cat.families || []).find((f) => f.id === choice.family);
      const model = choice ? L.modelLabel(choice, cat) : worker && worker.model;
      const maxTries = ((NS.store.s.settings || {}).attempts) || 3;
      const cost = task.cost_usd != null ? '$' + task.cost_usd.toFixed(2) + (task.budget_usd != null ? ' ' + t('det.of') + ' $' + task.budget_usd : '') : t('det.cost_unknown');
      const sep = h('span', { text: '·' });
      const roster = (task.runs || []).filter((r) => L.roleKey(r.role)).map((r) => {
        const k = L.roleKey(r.role), st = L.variant(r.state)[0];
        return h('span', { class: 'r-' + k + ' rs ' + ({ done: 'done', running: 'run', failed: 'wait' }[st] || 'idle') }, roleIcon(k), h('b', { text: t('role.' + k) + (r.variant ? ' ' + r.variant : '') }), ' ' + (r.model || ''),
          ' ', h('small', { text: t('runst.' + (st || 'pending')) }));
      });
      const acts = L.taskActions(task).map((a) => actionBtn(a));
      if (task.taken_from) acts.push(takeBtn(task, 'give'));
      else if (canTake(task)) acts.push(takeBtn(task, 'take'));
      return h('section', { class: 'db' }, h('h3', { text: t('det.details') }),
        h('p', { class: 'fline' }, ic('i-branch'), task.branch, sep, model ? [fam ? vendorIcon(fam.vendor, 'mlogo') : envIcon(task.agent, 'mlogo'), model + ', ' + t('det.attempt', { n: Math.min(task.attempts + 1, maxTries), m: maxTries })] : null, sep, cost),
        roster.length ? h('p', { class: 'rline' }, roster) : null,
        h('small', { text: t('det.each_role', { env: envObj ? envObj.name : '' }) }),
        acts.length ? h('div', { class: 'acts-row' }, acts) : null);
    }
    function todosBlock() {
      if (task.status === 'cancelled' || task.status === 'reverted') return null;
      if (task.status === 'done' && !(task.todos || []).length) return null;
      const open = (task.todos || []).filter((x) => !x.done).length;
      return h('section', { class: 'db todos-db' }, h('h3', {}, t('todo.title'), open ? h('small', { text: t('todo.open', { n: open }) }) : null), h('p', { class: 'hint', text: t('todo.hint') }), todoList(task));
    }
    function turnsBlock() {
      if (!rollbackable(task) || !snaps || !snaps.length) return null;
      return h('section', { class: 'db turns-db' }, h('h3', { text: t('snap.title') }), h('p', { class: 'hint', text: t('snap.hint') }),
        h('ul', { class: 'turns' }, snaps.map((s) => h('li', {}, h('span', { text: t('snap.row', { n: s.n, role: t('role.' + (L.roleKey(s.label) || s.label)), time: NS.hhmm(s.at) }) }),
          rollbackBtn(task.id, (list) => list.find((x) => x.n === s.n) || null)))));
    }
    function actionBtn(a) {
      const project = NS.store.project(task.project_id);
      const run = {
        cancel: () => call('cancel', { id: task.id }), retry: () => call('retry', { id: task.id }), remove: () => call('remove_task', { id: task.id }), undo: () => call('undo', { id: task.id }),
        retry_push: () => (L.retryPushCommand(project) === 'retry' ? call('retry', { id: task.id }) : call('push_base', { projectId: task.project_id })),
        folder: () => call('open_path', { path: workPath(task) }), editor: () => call('open_editor', { path: workPath(task) }),
        terminal: () => call('hand_off', { taskId: task.id }), take_back: () => call('take_back', { taskId: task.id }), start_now: () => call('start_now', { taskId: task.id }),
      }[a];
      const sure = a === 'remove' || a === 'undo';
      const b = h('button', { type: 'button', class: 'btn sm' + (a === 'remove' || a === 'cancel' ? ' danger' : ''), text: t('act.' + a) });
      b.onclick = () => {
        if (sure && !b.dataset.armed) { b.dataset.armed = '1'; b.textContent = t('act.sure'); setTimeout(() => { delete b.dataset.armed; b.textContent = t('act.' + a); }, 3000); return; }
        run();
      };
      return b;
    }
    function renderDet() {
      // v4: blocks of other tasks (handoff note, ports and base, …) come from the registry ('details')
      const extra = NS.registry.points('details').map((d) => { try { return d.render(task, NS.shell.ctxFor({ task: task.id })); } catch (e) { return null; } }).filter(Boolean);
      const typing = document.activeElement && det.contains(document.activeElement) && document.activeElement.classList.contains('todo-in');
      clear(det).append(...[changesBlock(), checksBlock(), todosBlock(), turnsBlock(), detailsBlock(), ...extra].filter(Boolean));
      if (typing) { const i = det.querySelector('.todo-in'); if (i) { i.focus(); i.setSelectionRange(i.value.length, i.value.length); } }
    }
    async function setOpen(v) {
      open = v; det.hidden = !v; el.classList.toggle('open', v);
      if (v) { renderDet(); await loadFiles(); renderDet(); }
      renderTop();
      if (v) el.scrollIntoView({ block: 'start' });
    }
    renderTop();
    return {
      el, get id() { return task.id; }, get open() { return open; },
      update(next) { task = next; files = null; snaps = null; renderTop(); if (open) { renderDet(); loadFiles().then(renderDet); } },
      close() { if (open) setOpen(false); },
      tick() { const e = top.querySelector('.elapsed'); if (e) e.textContent = elapsed(task); },
      setPinned(v) { el.classList.toggle('pinned', !!v && !open); },
    };
  }

  NS.taskCard = { create: createCard, diffBox, closeMenu: () => { if (closeMenu) closeMenu(); } };   // the screen calls closeMenu() on destroy
})();
