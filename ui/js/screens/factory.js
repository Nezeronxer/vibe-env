// Factory (G08): everything one env produces - "Фабрика · <Среда> · все агенты этой среды, каждый своим запуском" (only this env, story 17).
// Header with "Работают X из N", "Сегодня $ из $", the env's limits and "Заморозить всё"; the aggregated roles; the tasks as lanes (last hour,
// scroll back, at most 20, then "ещё N"); the merge queue as a tube into main; the feed of the last 500 events with "Все / Проблемы";
// "Поручить" (submit_convoy). Lanes exist only here and in the swarm (story 124).
(function () {
  const NS = window.NS;
  const { h, ic, roleIcon, envIcon, clear, setVars } = NS;
  const L = window.NSLogic;
  const t = (k, p) => NS.t(k, p);
  const st = NS.store, S = st.s;
  const call = (c, a) => NS.api.call(c, a);
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
  const LANE_ROLES = ['plan', 'crit', 'test', 'exec', 'gate', 'rev', 'integ'];
  const SCROLL_MIN = 30;                       // "прокрутка назад": half an hour per click
  const FEED_TASKS = 30;                       // logs of this many newest tasks fill the feed at the start

  const legend = () => h('p', { class: 'legend' }, ...LANE_ROLES.map((k) => h('span', { class: 'r-' + k }, roleIcon(k), h('span', { class: 'rlab', text: t('role.' + k) }))), h('span', { class: 'proj-l', text: t('factory.ahead') }));

  // "Поручение": overall progress "3 из 5" on top of its chat (story 163)
  function convoyBar(chatId) {
    const el = h('div', { class: 'convo-bar' });
    function draw() {
      const tasks = st.chatTasks(chatId).sort((a, b) => Date.parse(a.created_at) - Date.parse(b.created_at));
      const p = L.convoyProgress(tasks);
      clear(el).append(h('div', { class: 'cv-h' }, h('b', { text: t('convoy.title') }), h('span', { class: 'hint', text: t('convoy.progress', { done: p.done, total: p.total }) })),
        h('div', { class: 'prog', role: 'img', 'aria-label': t('convoy.progress', { done: p.done, total: p.total }) }, p.parts.map((c) => h('i', { class: c }))));
    }
    draw();
    return { el, update: draw, destroy() {} };
  }
  NS.factory = { convoyBar };

  function mount(root, ctx) {
    const envs = S.envs;
    const envId = ctx.params.env || S.lastEnv || (envs.find((e) => e.state === 'ok') || envs[0] || {}).id;
    const env = st.env(envId) || { id: envId, name: envId || '', state: 'ok' };
    const alive = { v: true };
    if (env.id && S.route.env !== env.id) { S.route.env = env.id; NS.shell.barUpdate('route'); }
    let offset = 0, filter = 'all', evs = [], timer = 0, closeMenu = null;      // closeMenu: the open model menu, if any
    const tasks = () => st.envTasks(envId);

    // ---------- header ----------
    const limits = NS.limits.limitsBar(envId);
    const stats = h('div', { class: 'fact-stats' });
    const freezeBtn = h('button', { type: 'button', class: 'btn danger', onclick: async (e) => {
      const b = e.currentTarget; b.disabled = true;
      try { await call('freeze', { on: !S.frozen }); } catch (er) { fail(er); }
      b.disabled = false;
    } });
    const head = h('header', { class: 'fact-h' }, h('div', {}, h('h1', { text: t('factory.title') }), h('span', { class: 'fact-sub' }, envIcon(env.custom ? 'custom' : env.id), t('factory.sub', { env: env.name }))), stats, limits.el, h('span', { class: 'grow' }), freezeBtn);
    function drawHead() {
      const s = L.factoryStats(tasks(), Date.now());
      const places = (S.settings || {}).max_parallel;
      const cap = ((S.settings || {}).limits || {}).day_usd;
      clear(stats).append(h('div', {}, h('span', { text: t('factory.working') }), h('b', { text: places ? t('factory.of', { a: s.working, b: places }) : String(s.working) })),
        h('div', {}, h('span', { text: t('factory.today') }), h('b', { text: s.reportsCost ? L.formatBudget(s.spent, cap, t) : t('factory.no_cost') })));
      clear(freezeBtn).append(ic('i-pause'), S.frozen ? t('factory.unfreeze') : t('factory.freeze'));
      freezeBtn.title = t('factory.freeze_d');
    }

    // ---------- aggregated roles ----------
    const strip = h('div', { class: 'strip', 'aria-label': t('factory.roles') });
    function drawStrip() {
      const rows = L.rolesStrip(tasks());
      clear(strip);
      if (!rows.length) { strip.append(h('span', { class: 'none', text: t('factory.no_agents') })); return; }
      for (const r of rows) {
        const label = r.key === 'exec' && r.count > 1 ? t('factory.executors', { n: r.count }) : t('role.' + r.key);
        const st2 = r.key === 'queue' ? t('factory.queue_n', { n: r.count, w: L.plural(r.count, [t('factory.task_1'), t('factory.task_2'), t('factory.task_5')], NS.lang()) })
          : [r.models.join(', '), r.state === 'work' ? (r.key === 'coord' ? t('factory.works') : '') : r.state === 'idle' ? t('factory.idle') : t('factory.watching')].filter(Boolean).join(' · ');
        strip.append(h('span', { class: 'r-' + r.key }, roleIcon(r.key), h('b', { class: 'rlab', text: label }), h('em', { class: r.state === 'work' ? 'work' : '', text: st2 })));
      }
    }

    // ---------- lanes ----------
    const lanesBox = h('div'), pipeBox = h('div');
    const statusLine = (r) => {
      const tk = r.task;
      if (r.kind === 'queue' && tk.start && L.variant(tk.start)[0] === 'night') return h('small', { class: 'night' }, ic('i-moon', 'moon'), t('factory.night'), h('button', { type: 'button', class: 'btn sm lane-sub-act', text: t('act.start_now'), onclick: () => call('start_now', { taskId: tk.id }).catch(fail) }));
      if (r.kind === 'wait') return h('small', { class: 'need', text: t('st.wait').toLowerCase() });
      if (r.kind === 'fail') return h('small', { text: NS.errHuman(tk.outcome && tk.outcome.error) || t('st.fail').toLowerCase() });
      if (r.kind === 'run') return h('small', { text: S.lastLog.get(tk.id) || t(L.labelKey(tk)).toLowerCase() });
      return h('small', { text: t(L.labelKey(tk)).toLowerCase() });
    };
    function drawLanes() {
      const now = Date.now(), view = L.factoryRows(tasks(), now, { offsetMin: offset });
      const span = view.t1 - view.t0;
      const axis = h('div', { class: 'axis' });
      for (let m = Math.ceil(view.t0 / 600000) * 600000; m <= view.t1; m += 600000) axis.append(setVars(h('span', { text: NS.hhmm(m) }), { '--l': ((m - view.t0) * 100 / span) + '%' }));
      const rows = view.rows.map((r) => {
        const open = () => NS.nav('env', { env: envId, chat: r.task.chat_id });
        const sw = r.task.swarm;
        return h('div', { class: 'lane' }, h('div', { class: 'lane-n' }, sw ? ic('i-swarm') : null, h('div', {}, h('b', {}, h('button', { type: 'button', title: r.task.title, text: r.task.title, onclick: open })), statusLine(r))),
          h('div', { class: 'track' }, r.segs.map((s) => setVars(h('div', { class: 'seg-b r-' + s.role + (s.state === 'proj' ? ' proj' : s.state === 'fail' ? ' fail' : s.state === 'wait' ? ' wait' : ''), title: t('role.' + s.role) + (s.model ? ' · ' + s.model : ''),
            text: s.state === 'wait' ? t('factory.wait') : s.state === 'proj' ? (s.role === 'integ' ? t('step.merge').toLowerCase() : '') : s.role === 'gate' ? t('step.gates').toLowerCase() : t('role.' + s.role).toLowerCase().slice(0, 12) }), { '--l': s.left + '%', '--w': s.width + '%' }))));
      });
      const nowPct = L.placeBar(now, now + 1, view.t0, view.t1);
      const ctl = h('div', { class: 'lanes-ctl' },
        h('button', { type: 'button', class: 'btn sm', text: '‹ ' + t('factory.earlier'), onclick: () => { offset += SCROLL_MIN; drawLanes(); } }),
        h('button', { type: 'button', class: 'btn sm', disabled: offset === 0, text: t('factory.later') + ' ›', onclick: () => { offset = Math.max(0, offset - SCROLL_MIN); drawLanes(); } }),
        offset ? h('small', { text: t('factory.back', { n: offset }) }) : h('small', { text: t('factory.last_hour') }));
      clear(lanesBox);
      if (!tasks().length) { lanesBox.append(h('p', { class: 'empty-note', text: t('factory.no_tasks') })); return; }
      NS.append(lanesBox, [legend(), ctl, view.rows.length ? h('div', { class: 'lanes flush' }, h('div', { class: 'lane-axis' }, h('div'), axis), rows,
        nowPct ? h('div', { class: 'lane-now-wrap' }, setVars(h('div', { class: 'now' }, h('span', { text: t('swarm.now') })), { '--l': nowPct.left + '%' })) : null) : h('p', { class: 'empty-note', text: t('factory.none_here') }),
      view.more > 0 ? h('p', { class: 'lanes-more', text: t('sum.more', { n: view.more }) }) : null]);
    }
    function drawPipe() {
      const q = L.mergeQueue(tasks());
      const items = q.slice().reverse().map((x) => {
        const title = x.task.title;
        if (x.kind === 'head') return h('span', { class: 'pitem first' }, ic('i-run'), title);
        if (x.kind === 'blocked') return h('span', { class: 'pitem blocked', title: t('card.merge_blocked') }, ic('i-pause'), title);
        if (x.kind === 'ghost') return h('span', { class: 'pitem ghost' }, title + ' · ' + t('factory.after_review'));
        return h('span', { class: 'pitem', text: title });
      });
      clear(pipeBox).append(h('div', { class: 'pipe-wrap' }, h('div', { class: 'pipe-h' }, h('b', { text: t('factory.pipe') }), h('span', { class: 'hint', text: t('factory.pipe_d') })),
        h('div', { class: 'pipe' }, h('div', { class: 'pipe-tube' }, items.length ? items : h('span', { class: 'pitem empty', text: t('factory.empty') })), h('div', { class: 'pipe-main' }, ic('i-branch'), 'main'))));
    }

    // ---------- "Поручить" ----------
    const input = h('input', { type: 'text', id: 'ol', placeholder: t('factory.one_line_ph'), 'aria-label': t('factory.one_line') });
    const projSel = h('select', { 'aria-label': t('chat.project'), hidden: S.projects.length < 2 }, S.projects.map((p) => h('option', { value: p.id, text: p.name, selected: p.id === (S.lastProject || (S.projects[0] || {}).id) })));
    const goBtn = h('button', { class: 'btn primary', type: 'submit', text: t('factory.assign') });
    const form = h('form', { class: 'oneline', onsubmit: async (e) => {
      e.preventDefault();
      const text = input.value.trim();
      if (!text) { input.focus(); return; }
      const projectId = projSel.value || (S.projects[0] || {}).id;
      if (!projectId) { NS.shell.toast(t('factory.no_project'), true); return; }
      goBtn.disabled = true;
      try {
        const chatId = await call('submit_convoy', { projectId, env: envId, text });
        input.value = '';
        await st.loadChats(envId);
        if (alive.v) NS.nav('env', { env: envId, chat: chatId });
      } catch (er) { fail(er); }
      goBtn.disabled = false;
    } }, h('label', { class: 'sr-only', for: 'ol', text: t('factory.one_line') }), input, projSel, goBtn);
    const convoys = h('div', { class: 'convoys' });
    function drawConvoys() {
      const list = st.chatsOf(envId).filter((c) => c.convoy).sort((a, b) => Date.parse(b.updated_at) - Date.parse(a.updated_at)).slice(0, 4);
      clear(convoys).append(...list.map((c) => {
        const p = L.convoyProgress(st.chatTasks(c.id));
        return h('button', { type: 'button', class: 'cv', onclick: () => NS.nav('env', { env: envId, chat: c.id }) }, h('div', { class: 'cv-h' }, h('b', { text: c.title }), h('span', { class: 'hint', text: t('convoy.progress', { done: p.done, total: p.total }) })),
          h('div', { class: 'prog' }, p.parts.map((x) => h('i', { class: x }))));
      }));
    }

    // ---------- feed ----------
    const feedList = h('ul', { class: 'feed' }), count = h('small');
    const seg = h('div', { class: 'seg', role: 'group', 'aria-label': t('factory.feed') });
    function drawFeed() {
      const all = L.feedEntries(evs, tasks(), Date.now()), f = L.filterFeed(all, filter);
      clear(seg).append(h('button', { type: 'button', class: filter === 'all' ? 'on' : '', 'aria-pressed': String(filter === 'all'), text: t('factory.all'), onclick: () => { filter = 'all'; drawFeed(); } }),
        h('button', { type: 'button', class: filter === 'problems' ? 'on' : '', 'aria-pressed': String(filter === 'problems'), onclick: () => { filter = 'problems'; drawFeed(); } }, t('factory.problems'), h('small', { text: String(f.count) })));
      clear(feedList);
      if (!f.entries.length) feedList.append(h('li', { class: 'empty-note' }, h('span'), h('span'), h('span', { text: filter === 'problems' ? t('factory.no_problems') : t('factory.feed_empty') })));
      for (const e of f.entries) {
        const tk = S.tasks.get(e.taskId);
        feedList.append(h('li', { class: e.level ? 'prob ' + e.level : '' }, h('time', { text: NS.hhmm(e.at) }), roleIcon(e.role),
          h('div', { class: 'ev' }, e.need ? t('factory.asked', { q: e.text }) : e.text, e.model ? h('small', { text: e.model }) : null,
            tk && tk.chat_id ? h('small', {}, h('button', { type: 'button', class: 'linkb', text: tk.title, onclick: () => NS.nav('env', { env: envId, chat: tk.chat_id }) })) : null,
            e.acts ? actions(e, tk) : null)));
      }
    }
    // "Подтолкнуть" and "Сменить модель" (story 164) - never required, the Watcher does it by itself
    function actions(e, tk) {
      const box = h('div', { class: 'acts' });
      const nudge = h('button', { type: 'button', class: 'btn sm primary', text: t('factory.nudge'), onclick: async () => {
        nudge.disabled = true;
        try { await call('nudge', { taskId: e.taskId }); nudge.textContent = t('factory.nudged'); } catch (er) { fail(er); nudge.disabled = false; }
      } });
      const sw = h('button', { type: 'button', class: 'btn sm', 'aria-haspopup': 'menu', text: t('factory.switch'), onclick: (ev) => {
        ev.stopPropagation();
        const old = box.querySelector('.menu'); if (old) { old.remove(); return; }
        const cur = tk && tk.models && tk.models.worker && tk.models.worker.family;
        const fams = (((S.catalogs.get(envId) || {}).families) || []).filter((f) => f.id !== 'default' && f.id !== cur);
        const menu = h('div', { class: 'menu', role: 'menu' }, fams.length ? fams.map((f) => h('button', { type: 'button', role: 'menuitem', onclick: async () => {
          menu.remove();
          try { await call('switch_model', { taskId: e.taskId, model: { family: f.id, version: null } }); NS.shell.toast(t('factory.switched', { model: f.label })); } catch (er) { fail(er); }
        } }, h('span', { text: L.modelLabel({ family: f.id, version: null }, S.catalogs.get(envId)) }))) : h('small', { text: t('model.no_list') }));
        box.append(menu);
        const shut = () => { menu.remove(); document.removeEventListener('click', close); document.removeEventListener('keydown', close); if (closeMenu === shut) closeMenu = null; };
        const close = (x) => { if (x.type === 'keydown' ? x.key === 'Escape' : !menu.contains(x.target)) shut(); };
        closeMenu = shut;
        setTimeout(() => { if (closeMenu === shut) { document.addEventListener('click', close); document.addEventListener('keydown', close); } }, 0);
      } });
      box.append(nudge, sw);
      return box;
    }
    async function loadFeed() {
      const mine = tasks().sort((a, b) => Date.parse(b.started_at || b.created_at) - Date.parse(a.started_at || a.created_at)).slice(0, FEED_TASKS);
      const got = await Promise.all(mine.map((tk) => call('get_log', { id: tk.id, limit: 40 }).then((list) => (list || []).map((x) => Object.assign({ task_id: tk.id }, x))).catch(() => [])));
      if (!alive.v) return;
      evs = got.flat();
      drawFeed();
    }

    // ---------- page ----------
    const left = h('section', { class: 'fpanel', 'aria-labelledby': 'fc' }, h('h2', { id: 'fc', text: t('factory.tasks') }), form, convoys, lanesBox, pipeBox);
    const right = h('section', { class: 'fpanel', 'aria-labelledby': 'ff' }, h('div', { class: 'fpanel-h' }, h('h2', { id: 'ff', text: t('factory.feed') }), seg), feedList);
    root.append(h('div', { class: 'factory' }, head, strip, h('div', { class: 'fact-grid' }, left, right)));
    function all() { drawHead(); drawStrip(); drawLanes(); drawPipe(); drawConvoys(); drawFeed(); }
    const queue = () => { if (timer) return; timer = setTimeout(() => { timer = 0; if (alive.v) all(); }, 150); };
    all();
    st.loadLimits(envId).then(() => alive.v && limits.refresh());
    st.loadCatalog(envId);
    loadFeed();
    const tick = setInterval(() => alive.v && all(), 30000);

    return {
      update(evt, payload) {
        if (evt === 'tasklog' && payload && tasks().some((x) => x.id === payload.id)) { evs.push(Object.assign({ task_id: payload.id }, payload.event || {})); if (evs.length > L.FEED_MAX * 2) evs = evs.slice(-L.FEED_MAX); queue(); }
        else if (evt === 'task' && payload && payload.agent === envId) queue();
        else if (['frozen', 'settings', 'chats', 'catalog', 'envs'].includes(evt)) queue();
        else if (evt === 'limits' && payload === envId) limits.refresh();
      },
      destroy() { alive.v = false; if (closeMenu) closeMenu(); clearInterval(tick); clearTimeout(timer); limits.destroy(); },
    };
  }

  NS.registry.register({ id: 'factory', layout: 'full', keepEnv: true, rail: { zone: 'foot', icon: 'i-fact', labelKey: 'rail.factory', order: 1 }, mount });
})();
