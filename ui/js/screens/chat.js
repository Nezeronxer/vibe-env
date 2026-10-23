// Screen "chat of an env": header (env · model, project, branch, limits), the feed, the input field.
// Registered in the screen registry as 'chat' (layout 'chats': rail + chat list + this screen).
(function () {
  const NS = window.NS;
  const { h, ic, roleIcon, envIcon, vendorIcon, clear, richText } = NS;
  const L = window.NSLogic;
  const t = (k, p) => NS.t(k, p);

  function mount(root, ctx) {
    const st = NS.store, S = st.s;
    const envId = ctx.params.env;
    let chatId = ctx.params.chat;                      // chat id or 'new'
    const env = st.env(envId);
    const isNew = chatId === 'new';
    const chatObj = isNew ? null : st.findChat(chatId);
    let msgs = [], more = false, loading = false, pending = [];
    const cards = new Map();                            // task id -> card controller (kept across re-renders: open state lives there)
    const openGroups = new Set();
    let selProject = (chatObj && chatObj.project_id) || S.lastProject || (S.settings && S.settings.default_project) || (S.projects[0] && S.projects[0].id) || null;
    const alive = { v: true };
    const ext = ctx.params.ext || {};
    const tabId = ctx.params.tab || null;              // the tab this chat lives in (tabs.js): a new chat id goes back to that tab
    const go = (chat) => (tabId && NS.tabs ? NS.tabs.setChat(tabId, envId, chat) : NS.nav('env', { env: envId, chat }));
    const V = window.NSLive;                  // the swarm screen reuses this chat as its main surface (story 156): extra composer control, note target

    // ---------- header ----------
    const title = h('h1', { class: 'pane-ttl' }), sub = h('span', { class: 'pane-sub' });
    const limits = NS.limits.limitsBar(envId);
    const menuBtn = h('button', { type: 'button', class: 'btn sm menu-btn', 'aria-haspopup': 'menu', 'aria-label': t('chat.menu'), text: '⋯' });
    const projSel = h('select', { class: 'projsel', 'aria-label': t('chat.project') });
    // v4: this header is the pane's title row (tabs.js adds the registry slots and the pane tools); env, model and limits are in the bar
    const header = h('header', { class: 'convo-h pane-h' }, h('span', { class: 'pane-ic' }, envIcon(env.custom ? 'custom' : envId)),
      title, sub, isNew ? projSel : null, !isNew ? menuBtn : null);
    function fillProjects() {
      clear(projSel);
      for (const p of S.projects) projSel.append(h('option', { value: p.id, text: p.name, selected: p.id === selProject }));
      projSel.append(h('option', { value: '__add', text: t('chat.add_project') }));
    }
    projSel.onchange = async () => {
      if (projSel.value === '__add') { const p = await NS.projects.addByDialog(); if (!alive.v) return; if (p) selProject = p.id; fillProjects(); drawHeader(); return; }
      selProject = projSel.value; S.lastProject = selProject; drawHeader();
    };
    function drawHeader() {
      const c = isNew ? null : st.findChat(chatId), tk = isNew ? null : st.chatTasks(chatId)[0];
      title.textContent = isNew ? t('tabs.new_chat') : (c && c.title) || (tk && tk.title) || t('chat.untitled');
      title.title = env.name + (NS.models.label(envId) ? ' · ' + NS.models.label(envId) : '');
      const p = st.project(selProject);
      const repo = p && S.repos.get(p.id);
      const branch = repo ? (repo.current_branch || p.base_branch) + (repo.ahead ? ' ↑' + repo.ahead : '') + (repo.behind ? ' ↓' + repo.behind : '') : '';
      sub.textContent = [p && p.name, branch].filter(Boolean).join(' · ');
      if (p && !S.repos.has(p.id)) st.loadRepo(p.id).then(() => alive.v && drawHeader());
    }
    let closeChatMenu = () => {};
    menuBtn.onclick = (e) => {
      e.stopPropagation();
      closeChatMenu();
      const busy = st.chatTasks(chatId).some((x) => ['wait', 'run', 'queue'].includes(L.kindOf(x)));
      const m = h('div', { class: 'menu menu-r', role: 'menu' },
        h('button', { type: 'button', role: 'menuitem', text: t('chat.terminal'), onclick: async () => {      // A07.2: the same session in the CLI of the env, read-only copy
          closeChatMenu();
          try { await NS.api.call('open_chat_terminal', { id: chatId }); feed.append(h('div', { class: 'sysline bad' }, ic('i-warn'), h('span', { text: t('chat.terminal_warn') }))); toBottom(); }
          catch (er) { NS.shell.toast(NS.errHuman(NS.api.errText(er)), true); }
        } }),
        h('button', { type: 'button', role: 'menuitem', disabled: busy, title: busy ? t('chat.delete_busy') : '', text: t('chat.delete'),
        onclick: async () => {
          closeChatMenu();
          if (!(await NS.projects.confirm(t('chat.delete'), t('chat.delete_text'), t('chat.delete')))) return;
          try { await NS.api.call('remove_chat', { id: chatId }); await st.loadChats(envId); if (tabId && NS.tabs) NS.tabs.dropChat(chatId); else NS.nav('env', { env: envId, chat: null }); } catch (er) { NS.shell.toast(NS.errHuman(NS.api.errText(er)), true); }
        } }), busy ? h('small', { text: t('chat.delete_busy') }) : null);
      menuBtn.after(m);
      const close = (ev) => { if (!m.contains(ev.target)) closeChatMenu(); };
      closeChatMenu = () => { m.remove(); document.removeEventListener('click', close); closeChatMenu = () => {}; };
      setTimeout(() => document.addEventListener('click', close), 0);
    };

    // ---------- feed ----------
    // Messages, action lines (task-action / chat-action / ChatMsg.tool of v2), streamed text (chat-delta) in one timeline.
    // Every render reuses the nodes whose data did not change (cache by key + signature), so open outputs and cards keep their state.
    const feed = h('div', { class: 'cfeed' }), wrap = h('div', { class: 'feed-wrap' }, feed);
    const nearBottom = () => wrap.scrollHeight - wrap.scrollTop - wrap.clientHeight < 80;
    const toBottom = () => { wrap.scrollTop = wrap.scrollHeight; };
    const acts = new Map();          // key -> {key, at, role, taskId, action}
    const streams = new Map();       // msg id -> {text, at}: the answer printed by chat-delta
    const typing = new Map();        // msg id -> true: a whole live message printed word by word
    const open = new Set();          // expanded outputs, diffs, glued reads
    const cache = new Map();         // node key -> {sig, nodes}
    const olderLog = new Map();      // task id -> "before" for "показать раньше" (the log keeps the last 500 events)
    let loaded = false;
    const reduced = () => !!(window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches);
    const live = NS.actions.liveBar();
    const ts = (v) => (typeof v === 'number' ? v : Date.parse(v) || 0);
    const LOG_LIMIT = ((S.settings && S.settings.keep && S.settings.keep.log_events) || 500);

    function activeTask() {
      const mine = st.chatTasks(chatId).filter((x) => x.status !== 'handed_off' && ['wait', 'run', 'queue'].includes(L.kindOf(x)));
      return mine.find((x) => x.status === 'needs_human' && x.question) || mine[0] || null;
    }
    const hooks = {
      answerMode: (task) => composer.setMode('answer', task), hintMode: (task) => composer.setMode('hint', task),
    };
    function cardFor(taskId) {
      let c = cards.get(taskId);
      if (!c) { const task = S.tasks.get(taskId); if (!task) return null; c = NS.taskCard.create(task, hooks); cards.set(taskId, c); }
      return c;
    }
    const runningRun = (task) => (task.runs || []).slice().reverse().find((r) => L.variant(r.state)[0] === 'running');
    const isLiveTask = (task) => !!task && L.kindOf(task) === 'run' && !!L.info(task.status).live;
    // start and result of one action, and the chat's mirror of a task's action, are one row (V.upsertAction, selftested)
    const upsertAct = (owner, action, at, role) => V.upsertAction(acts, owner, action, at, role);
    function humanNode(m, isPending) {
      const note = !isPending && m.task_id;
      const to = m.to ? ' · ' + (m.to === 'reviewer' ? t('swarm.to_reviewer') : t('swarm.to_variant', { v: m.to })) : '';     // a note to a swarm: who gets it (story 156)
      // v4 (story 244): my message is a prompt line `❯ text`, the time and delivery at the right edge
      return h('div', { class: 'me' }, h('span', { class: 'pr', 'aria-hidden': 'true', text: '❯' }), h('div', { class: 'me-b' }, h('span', { class: 'sr-only', text: t('feed.you') }), richText(m.text)),
        h('small', { class: 'me-m' }, NS.hhmm(m.at), isPending ? ' · ' + t('msg.sending') : note ? ' · ' + (m.delivered_at ? t('msg.delivered', { time: NS.hhmm(m.delivered_at) }) : t('msg.will_pass')) + to : ''));
    }
    function systemNodes(m) {
      const out = [];
      if (m.kind === 'task') {
        const c = m.task_id && cardFor(m.task_id), task = m.task_id && S.tasks.get(m.task_id);
        if (c) out.push(c.el);
        const box = task && task.rendered_prompt ? h('pre', { class: 'promptbox', hidden: true, text: task.rendered_prompt }) : null;
        const code = task && L.stepBar(task).find((s) => s.key === 'code');
        const canUndo = task && code && code.state === 'pending' && ['run', 'queue'].includes(L.kindOf(task));
        const showBtn = h('button', { type: 'button', class: 'linkb', text: t('feed.show_prompt'), onclick: () => { box.hidden = !box.hidden; showBtn.textContent = box.hidden ? t('feed.show_prompt') : t('feed.hide_prompt'); } });
        out.push(h('div', { class: 'sysline' }, ic('i-check'), h('span', { text: t('feed.task_made') }), box ? showBtn : null,
          canUndo ? h('button', { type: 'button', class: 'linkb', text: t('feed.was_question'), onclick: () => NS.api.call('not_a_task', { taskId: m.task_id }).catch((e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true)) }) : null));
        if (box) out.push(box);
      } else if (m.kind === 'error') {
        out.push(h('div', { class: 'sysline bad' }, ic('i-fail'), h('span', { text: t('feed.no_answer', { why: NS.errHuman(m.text) }) }),
          h('button', { type: 'button', class: 'linkb', text: t('feed.retry'), onclick: () => NS.api.call('retry_chat', { id: chatId, msgId: m.reply_to || m.id }).catch((e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true)) })));
      } else out.push(h('div', { class: 'sysline' + (m.kind === 'warn' ? ' bad' : '') }, ic(m.kind === 'warn' ? 'i-warn' : 'i-check'), h('span', { text: NS.errHuman(m.text) })));
      return out;
    }
    // vendor mark of the model that answers (the env mark when the family is unknown)
    function modelIcon() {
      const cat = S.catalogs.get(envId), ch = st.envSettings(envId).default_model;
      const fam = cat && (cat.families || []).find((f) => f.id === (ch ? ch.family : (L.defaultFamily(cat) || {}).id));
      return fam ? vendorIcon(fam.vendor, 'mlogo') : envIcon(envId, 'mlogo');
    }
    // the body of an agent message: markdown, or a word-by-word print of a live message (story 227)
    function bodyOf(m) {
      const body = h('div', { class: 'msg-b' });
      if (!typing.has(m.id)) { body.append(richText(m.text)); return body; }
      const plan = V.typePlan(m.text);
      const caret = h('span', { class: 'caret', 'aria-hidden': 'true' });
      let i = 0, shown = '';
      const step = () => {
        if (!alive.v) return;
        // the printed part goes through markdown too: no raw ** or backticks on screen even mid-word (story 228)
        if (i < plan.chunks.length) { shown += plan.chunks[i++]; const stick = nearBottom(); clear(body).append(richText(shown), caret); setTimeout(step, plan.ms); if (stick) toBottom(); return; }
        typing.delete(m.id);
        clear(body).append(richText(m.text));
        drawLive();
      };
      setTimeout(step, 0);
      return body;
    }
    function msgNode(m) {
      const who = L.whoOf(m);
      if (who.kind === 'human') return humanNode(m, false);
      if (who.kind === 'system') return systemNodes(m);
      const rk = who.kind === 'role' ? L.roleKey(who.role) : null;
      const model = m.model ? m.model + ' · ' + t('feed.separate_run') : '';
      if (who.kind === 'model' || !rk) {
        const mlabel = m.model || NS.models.label(envId) || '';
        return h('div', { class: 'msg ai' }, h('span', { class: 'av' }, envIcon(envId)),
          h('div', {}, h('div', { class: 'msg-h' }, h('b', { text: env.name }), h('small', {}, modelIcon(), mlabel), h('time', { text: NS.hhmm(m.at) })), bodyOf(m)));
      }
      return h('div', { class: 'msg' }, roleIcon(rk), h('div', {}, h('div', { class: 'msg-h' }, h('span', { class: 'rname r-' + rk, text: t('role.' + rk) + (m.variant ? ' ' + m.variant : '') }), h('small', { text: model }), h('time', { text: NS.hhmm(m.at) })),
        bodyOf(m)));
    }
    // the answer while chat-delta pieces arrive: plain text with a caret, markdown once the whole message is there
    function streamNode(sx) {
      const mlabel = NS.models.label(envId) || '';
      return h('div', { class: 'msg ai streaming' }, h('span', { class: 'av' }, envIcon(envId)),
        h('div', {}, h('div', { class: 'msg-h' }, h('b', { text: env.name }), h('small', {}, modelIcon(), mlabel)),
          h('div', { class: 'msg-b' }, richText(sx.text), h('span', { class: 'caret', 'aria-hidden': 'true' }))));
    }
    const emptyNode = () => {
      const ex = [['onb3.ex1', 'onb3.ex1_t'], ['onb3.ex2', 'onb3.ex2_t'], ['onb3.ex3', 'onb3.ex3_t']];
      const proj = st.project(selProject);
      return h('div', { class: 'newchat-empty' }, envIcon(envId), h('h2', { text: t('feed.empty_title', { env: env.name }) }), h('p', { text: proj ? t('onb3.ex_d') : t('feed.empty') }),
        proj ? h('div', { class: 'examples', role: 'group', 'aria-label': t('onb3.ex_title') }, ex.map(([label, text]) => h('button', { type: 'button', class: 'btn ex', text: t(label),
          onclick: () => composer.insert(t(text)) }))) : null);
    };
    const toArr = (x) => [].concat(x);
    const actOpts = (e) => ({ open, live: e.taskId ? isLiveTask(S.tasks.get(e.taskId)) : !!(streams.size || typing.size || [...acts.values()].some((x) => !x.taskId && !x.action.done)) });

    function timeline() {
      const items = [];
      // one action may come from the chat record (get_chat: ChatMsg.action), the task log and a live event: the live/log copy wins
      const liveIds = new Set([...acts.values()].map((e) => String(e.action.id)));
      for (const m of msgs) {
        if (V.isServiceTool(m.action) || V.isServiceTool(m.tool)) continue;
        if (m.action) { if (!liveIds.has(String(m.action.id))) items.push({ at: ts(m.at), type: 'act', e: { key: 'm:' + m.id, at: ts(m.at), role: (m.who && L.whoOf(m).role) || 'chat', taskId: m.task_id || null, action: m.action } }); continue; }
        if (m.tool) items.push({ at: ts(m.at), type: 'act', e: { key: 'm:' + m.id, at: ts(m.at), role: 'legacy', taskId: m.task_id || null, action: V.toolToAction(m.tool, m.id) } });
        else items.push({ at: ts(m.at), type: 'msg', m });
      }
      for (const e of acts.values()) if (V.inputOf(e.action).kind !== 'plan') items.push({ at: e.at, type: 'act', e });
      for (const [id, sx] of streams) if (!msgs.some((x) => x.id === id)) items.push({ at: sx.at, type: 'stream', id, s: sx });
      return items.sort((a, b) => a.at - b.at);
    }
    let rendering = 0;
    const schedule = () => { if (!rendering) rendering = (window.requestAnimationFrame || setTimeout)(() => { rendering = 0; if (alive.v) renderFeed(); }); };
    function renderFeed() {
      const stick = nearBottom();
      const prevTop = wrap.scrollTop;
      const nodes = [], used = new Set();
      const put = (key, sig, make) => {
        used.add(key);
        let c = cache.get(key);
        if (!c || c.sig !== sig) { c = { sig, nodes: make() }; cache.set(key, c); }
        nodes.push(...c.nodes);
      };
      const items = timeline();
      if (olderLog.size) put('older', 'older', () => [h('button', { type: 'button', class: 'linkb older', text: t('act3.older'), onclick: loadOlderLog })]);
      if (!items.length && !pending.length) put('empty', String(selProject), () => [emptyNode()]);
      let run = [];
      const flush = () => {
        for (const g of V.groupActions(run)) {
          if (g.type === 'group') { const o = actOpts(g.items[0]); put('g:' + g.items[0].key, g.items.map((e) => e.key + (e.action.done ? '1' : '0')).join() + o.live, () => [NS.actions.groupNode(g.items, o)]); }
          else { const o = actOpts(g.item); put('a:' + g.item.key, JSON.stringify(g.item.action) + o.live, () => [NS.actions.actionNode(g.item.action, o)]); }
        }
        run = [];
      };
      for (const it of items) {
        if (it.type === 'act') { run.push(it.e); continue; }
        flush();
        if (it.type === 'msg') put('m:' + it.m.id, JSON.stringify(it.m) + (it.m.task_id && cards.has(it.m.task_id) ? '+' : ''), () => toArr(msgNode(it.m)));
        else put('s:' + it.id, it.s.text, () => [streamNode(it.s)]);
      }
      flush();
      for (const p of pending) put('p:' + p.id, p.text, () => [humanNode(p, true)]);
      for (const k of [...cache.keys()]) if (!used.has(k)) cache.delete(k);
      nodes.push(live.el);
      const same = nodes.length === feed.childNodes.length && nodes.every((n, i) => feed.childNodes[i] === n);
      if (!same) feed.replaceChildren(...nodes);
      drawLive();
      pin();
      if (stick) toBottom(); else wrap.scrollTop = prevTop;
    }
    // the live line: verb by the last action of the running task (or of the chat answer), timer of the step, role and model, the plan
    function drawLive() {
      const mine = st.chatTasks(chatId);
      const tk = mine.filter(isLiveTask).sort((a, b) => ts(b.started_at || b.created_at) - ts(a.started_at || a.created_at))[0];
      const lastOf = (pred) => { let best = null; for (const e of acts.values()) if (pred(e) && V.inputOf(e.action).kind !== 'plan' && (!best || e.at >= best.at)) best = e; return best; };
      const planOf = (pred) => { let best = null; for (const e of acts.values()) if (pred(e) && V.inputOf(e.action).kind === 'plan' && (!best || e.at >= best.at)) best = e; return best ? V.inputOf(best.action).items || [] : null; };
      if (tk) {
        const run = runningRun(tk), rk = run && L.roleKey(run.role);
        const last = lastOf((e) => e.taskId === tk.id);
        const streaming = streams.size > 0 || typing.size > 0;
        return live.set({ verbKey: V.actionVerb(last && last.action, streaming && (!last || last.action.done), { role: rk, lastRole: last && last.role }), since: ts(run && run.started_at) || ts(tk.started_at),
          who: [rk ? t('role.' + rk) : null, run && run.model].filter(Boolean).join(' · '), plan: planOf((e) => e.taskId === tk.id) });
      }
      const queued = mine.find((x) => x.status === 'queued');
      const q = queued && V.queueInfo([...S.tasks.values()], queued.id, (S.settings && S.settings.max_parallel) || 3);
      if (q) return live.set({ queue: q.after ? t('live.queue', q) : t('live.queue_short', q) });
      const chatLast = lastOf((e) => !e.taskId);
      const chatBusy = streams.size > 0 || typing.size > 0 || (chatLast && !chatLast.action.done);
      if (chatBusy) {
        const sx = [...streams.values()][0];
        return live.set({ verbKey: V.actionVerb(chatLast && !chatLast.action.done ? chatLast.action : null, streams.size > 0 || typing.size > 0), since: (sx && sx.at) || (chatLast && chatLast.at) || Date.now(),
          who: [env.name, NS.models.label(envId)].filter(Boolean).join(' · '), plan: planOf((e) => !e.taskId) });
      }
      live.set(null);
    }
    // the card of the running or waiting task stays at the top while the history scrolls (G24.2)
    function pin() {
      const mine = st.chatTasks(chatId).filter((x) => ['wait', 'run'].includes(L.kindOf(x)) && L.info(x.status).live);
      mine.sort((a, b) => (L.kindOf(a) === 'wait' ? 0 : 1) - (L.kindOf(b) === 'wait' ? 0 : 1) || Date.parse(b.created_at) - Date.parse(a.created_at));
      for (const [id, c] of cards) c.setPinned(mine[0] && mine[0].id === id);
    }

    // the task log on disk keeps actions with output and diff: the feed comes back after a restart (story 230)
    async function readLog(id, before) {
      try { return (await NS.api.call('get_task_log', { id, before, limit: LOG_LIMIT })) || []; }
      catch (e) { if (before) return []; try { return (await NS.api.call('get_log', { id, limit: LOG_LIMIT })) || []; } catch (e2) { return []; } }
    }
    const roleOfEv = (ev) => (ev.role ? L.roleKey(L.variant(ev.role)[0]) || String(L.variant(ev.role)[0]) : 'exec');
    async function loadTaskLog(id, before) {
      const evs = await readLog(id, before || null);
      if (!alive.v) return;
      let first = null;
      for (const ev of evs) {
        if (!first || ts(ev.at) < first) first = ts(ev.at);
        if (ev && ev.action) upsertAct(id, ev.action, ts(ev.at), roleOfEv(ev));
      }
      if (evs.length >= LOG_LIMIT && first) olderLog.set(id, new Date(first).toISOString()); else olderLog.delete(id);
      schedule();
    }
    function loadOlderLog() { for (const [id, before] of [...olderLog]) loadTaskLog(id, before); }

    async function loadOlder() {
      if (loading || !more || !msgs.length) return;
      loading = true;
      const before = L.nextBefore(msgs), prevH = wrap.scrollHeight;
      try {
        const batch = (await NS.api.call('get_chat', { id: chatId, before, limit: L.MSG_LIMIT })) || [];
        if (!alive.v) return;
        more = L.hasMore(batch.length, L.MSG_LIMIT);
        msgs = L.mergeMsgs(msgs, batch);
        renderFeed(); wrap.scrollTop = wrap.scrollHeight - prevH;
      } catch (e) { if (alive.v) NS.shell.toast(NS.errHuman(NS.api.errText(e)), true); }
      loading = false;
    }
    wrap.addEventListener('scroll', () => { if (wrap.scrollTop < 60) loadOlder(); });

    const composer = NS.composer.create({ envId, chat: chatId, hooks: {
      projectId: () => selProject, activeTask, noteTo: ext.noteTo,
      addPending(p) { pending.push(p); renderFeed(); toBottom(); },
      dropPending(id) { pending = pending.filter((p) => p.id !== id); schedule(); },
      sent: async (newId) => {
        if (isNew && newId) {
          NS.storage.del(L.draftKey(envId, null));
          await st.loadChats(envId);
          if (alive.v) go(newId);
        }
      },
    } });

    const convoy = chatObj && chatObj.convoy && NS.factory ? NS.factory.convoyBar(chatId) : null;       // "Поручение": overall progress "3 из 5" (story 163)
    root.append(h('section', { class: 'convo' }, header, convoy ? convoy.el : null, wrap, composer.el));
    if (ext.decorate) ext.decorate(composer);
    if (isNew) fillProjects();
    drawHeader();
    renderFeed();

    // ---------- data ----------
    (async () => {
      if (!isNew) {
        try {
          const batch = (await NS.api.call('get_chat', { id: chatId, before: null, limit: L.MSG_LIMIT })) || [];
          more = L.hasMore(batch.length, L.MSG_LIMIT);
          msgs = L.mergeMsgs([], batch);
          if (alive.v) { renderFeed(); toBottom(); }
        } catch (e) { if (alive.v) NS.shell.toast(NS.errHuman(NS.api.errText(e)), true); }
        await Promise.all(st.chatTasks(chatId).map((tk) => loadTaskLog(tk.id)));
        if (alive.v) toBottom();
      }
      loaded = true;
    })();
    st.loadLimits(envId).then(() => alive.v && (limits.refresh(), composer.drawHint()));
    st.loadCatalog(envId).then(() => { if (!alive.v) return; drawHeader(); composer.refresh(); cards.forEach((c, id) => S.tasks.get(id) && c.update(S.tasks.get(id))); });
    const tick = setInterval(() => cards.forEach((c) => c.tick()), 30000);
    // Esc closes pop-ups first (composer.js), then an open task card
    const onKey = (e) => { if (e.key === 'Escape' && !e.defaultPrevented) for (const c of cards.values()) if (c.open) { c.close(); break; } };
    document.addEventListener('keydown', onKey);

    return {
      update(evt, payload) {
        if (evt === 'task') {
          if (convoy) convoy.update();
          const c = cards.get(payload.id);
          if (c) c.update(payload);               // only this card is redrawn
          if (payload.chat_id === chatId) schedule();   // a card that arrived after its "Это задача" line, the live line, action spinners
          pin();
        } else if (evt === 'chatmsg') {
          if (payload.chat_id !== chatId) return;
          pending = pending.filter((p) => !(p.id === payload.id || (L.whoOf(payload).kind === 'human' && p.text === payload.text)));
          const known = msgs.some((x) => x.id === payload.id);
          const agent = ['model', 'role'].includes(L.whoOf(payload).kind) && !payload.tool && payload.text;
          if (streams.has(payload.id)) streams.delete(payload.id);                     // the streamed answer is complete: markdown replaces it
          else if (loaded && !known && agent && !reduced()) typing.set(payload.id, true);   // no partial messages from this CLI: print by words
          msgs = L.mergeMsgs(msgs, [payload]);
          schedule();
        } else if (evt === 'taskaction') {
          const tk = S.tasks.get(payload.id);
          if (!tk || tk.chat_id !== chatId) return;
          const run = runningRun(tk);
          upsertAct(payload.id, payload.action, Date.now(), (run && L.roleKey(run.role)) || 'exec');
          schedule();
        } else if (evt === 'chataction') {
          if (payload.id !== chatId) return;
          upsertAct('chat:' + chatId, payload.action, Date.now(), 'chat');
          schedule();
        } else if (evt === 'chatdelta') {
          if (payload.id !== chatId || msgs.some((x) => x.id === payload.msgId)) return;
          const sx = streams.get(payload.msgId) || { text: '', at: Date.now() };
          sx.text += String(payload.text || '');
          streams.set(payload.msgId, sx);
          schedule();
        } else if (evt === 'limits') { if (payload === envId) { limits.refresh(); composer.drawHint(); } }
        else if (evt === 'catalog' || evt === 'settings') { drawHeader(); composer.refresh(); }
        else if (evt === 'project' || evt === 'chats') { drawHeader(); }
        else if (evt === 'focus-input') composer.focus();
      },
      destroy() { alive.v = false; live.destroy(); if (convoy) convoy.destroy(); closeChatMenu(); NS.taskCard.closeMenu(); clearInterval(tick); document.removeEventListener('keydown', onKey); limits.destroy(); composer.destroy(); },
    };
  }

  NS.registry.register({ id: 'chat', layout: 'chats', mount });
})();
