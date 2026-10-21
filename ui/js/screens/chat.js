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

    // ---------- header ----------
    const title = h('h1'), sub = h('span', { class: 'hint' });
    const limits = NS.limits.limitsBar(envId);
    const menuBtn = h('button', { type: 'button', class: 'btn sm menu-btn', 'aria-haspopup': 'menu', 'aria-label': t('chat.menu'), text: '⋯' });
    const projSel = h('select', { class: 'projsel', 'aria-label': t('chat.project') });
    const header = h('header', { class: 'convo-h' },
      h('button', { type: 'button', class: 'btn sm side-toggle', onclick: () => NS.shell.toggleSide() }, ic('i-chat'), t('chat.chats')),
      title, sub, isNew ? projSel : null, limits.el, !isNew ? menuBtn : null);
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
      const m = NS.models.label(envId);
      title.textContent = env.name + (m ? ' · ' + m : '');
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
      const m = h('div', { class: 'menu menu-r', role: 'menu' }, h('button', { type: 'button', role: 'menuitem', disabled: busy, title: busy ? t('chat.delete_busy') : '', text: t('chat.delete'),
        onclick: async () => {
          closeChatMenu();
          if (!(await NS.projects.confirm(t('chat.delete'), t('chat.delete_text'), t('chat.delete')))) return;
          try { await NS.api.call('remove_chat', { id: chatId }); await st.loadChats(envId); NS.nav('env', { env: envId, chat: null }); } catch (er) { NS.shell.toast(NS.errHuman(NS.api.errText(er)), true); }
        } }), busy ? h('small', { text: t('chat.delete_busy') }) : null);
      menuBtn.after(m);
      const close = (ev) => { if (!m.contains(ev.target)) closeChatMenu(); };
      closeChatMenu = () => { m.remove(); document.removeEventListener('click', close); closeChatMenu = () => {}; };
      setTimeout(() => document.addEventListener('click', close), 0);
    };

    // ---------- feed ----------
    const feed = h('div', { class: 'cfeed' }), wrap = h('div', { class: 'feed-wrap' }, feed);
    const nearBottom = () => wrap.scrollHeight - wrap.scrollTop - wrap.clientHeight < 80;
    const toBottom = () => { wrap.scrollTop = wrap.scrollHeight; };

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
    const toolsNode = (g) => {
      const key = g.items[0].id;
      const bad = g.bad && !/fail|error|ошиб|не прошл/i.test(g.result);
      const d = h('details', { class: 'tools', open: openGroups.has(key), ontoggle: () => { if (d.open) openGroups.add(key); else openGroups.delete(key); } },
        h('summary', {}, ic('i-chev-r', 'tw'), h('span', { text: t('feed.actions', { n: g.count, w: L.plural(g.count, [t('feed.action_1'), t('feed.action_2'), t('feed.action_5')], NS.lang()) }) }),
          h('i', { text: g.names.join(', ') }), g.result ? h('em', { class: g.bad ? 'no' : 'ok', text: (bad ? t('tool.failed') + ' ' : '') + g.result }) : null),
        g.items.map((m) => h('div', { class: 'tl' }, h('span', { text: m.tool.name + ' ' + (m.tool.arg || '') }), h('em', { class: m.tool.ok === false ? 'no' : 'ok', text: m.tool.result || '' }))));
      return d;
    };
    function humanNode(m, isPending) {
      const note = !isPending && m.task_id;
      return h('div', { class: 'me' }, h('div', {}, h('small', { text: t('feed.you') + ' · ' + NS.hhmm(m.at) }), richText(m.text),
        isPending ? h('small', { class: 'deliv', text: t('msg.sending') }) : note ? h('small', { class: 'deliv', text: m.delivered_at ? t('msg.delivered', { time: NS.hhmm(m.delivered_at) }) : t('msg.will_pass') }) : null));
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
    function msgNode(m) {
      const who = L.whoOf(m);
      if (who.kind === 'human') return humanNode(m, false);
      if (who.kind === 'system') return systemNodes(m);
      const rk = who.kind === 'role' ? L.roleKey(who.role) : null;
      const model = m.model ? m.model + ' · ' + t('feed.separate_run') : '';
      if (who.kind === 'model' || !rk) {
        const mlabel = m.model || NS.models.label(envId) || '';
        return h('div', { class: 'msg ai' }, h('span', { class: 'av' }, envIcon(envId)),
          h('div', {}, h('div', { class: 'msg-h' }, h('b', { text: env.name }), h('small', {}, modelIcon(), mlabel), h('time', { text: NS.hhmm(m.at) })), h('div', { class: 'msg-b' }, richText(m.text))));
      }
      return h('div', { class: 'msg' }, roleIcon(rk), h('div', {}, h('div', { class: 'msg-h' }, h('span', { class: 'rname r-' + rk, text: t('role.' + rk) + (m.variant ? ' ' + m.variant : '') }), h('small', { text: model }), h('time', { text: NS.hhmm(m.at) })),
        h('div', { class: 'msg-b' }, richText(m.text))));
    }
    // Full rebuild only on first load, older-page load and chat change; everything else patches single nodes.
    const nodeOf = new Map();      // msg id -> its nodes (tool actions of one run share one node)
    const pendNode = new Map();    // pending id -> node
    const emptyNode = () => h('div', { class: 'newchat-empty' }, envIcon(envId), h('h2', { text: t('feed.empty_title', { env: env.name }) }), h('p', { text: t('feed.empty') }));
    const toArr = (x) => [].concat(x);
    function renderFeed() {
      const stick = nearBottom();
      const prevTop = wrap.scrollTop;
      const nodes = [];
      nodeOf.clear(); pendNode.clear();
      if (!msgs.length && !pending.length) nodes.push(emptyNode());
      for (const g of L.groupFeed(msgs)) {
        const ns = g.type === 'tools' ? [toolsNode(g)] : toArr(msgNode(g.msg));
        if (g.type === 'tools') g.items.forEach((m) => nodeOf.set(m.id, ns)); else nodeOf.set(g.msg.id, ns);
        nodes.push(...ns);
      }
      for (const p of pending) { const n = humanNode(p, true); pendNode.set(p.id, n); nodes.push(n); }
      clear(feed).append(...nodes);
      pin();
      if (stick) toBottom(); else wrap.scrollTop = prevTop;
    }
    const firstPending = () => { for (const n of pendNode.values()) return n; return null; };
    const dropEmpty = () => { const e = feed.querySelector('.newchat-empty'); if (e) e.remove(); };
    function addPendingNode(p) {
      dropEmpty();
      const n = humanNode(p, true); pendNode.set(p.id, n); feed.append(n);
    }
    function dropPendingNode(id) {
      const n = pendNode.get(id); if (n) n.remove(); pendNode.delete(id);
      if (!msgs.length && !pendNode.size && !feed.firstChild) feed.append(emptyNode());
    }
    // one new message at the end of the feed (or a changed one in place)
    function addMsgNode(m) {
      dropEmpty();
      const g = L.groupFeed(msgs), last = g[g.length - 1];
      const before = firstPending();
      const put = (ns) => ns.forEach((n) => (before ? before.before(n) : feed.append(n)));
      if (m.tool && last && last.type === 'tools') {                   // the run grew: replace its folded row
        const old = nodeOf.get(last.items[0].id), fresh = [toolsNode(last)];
        if (old && old[0].isConnected) old[0].replaceWith(fresh[0]); else put(fresh);
        last.items.forEach((x) => nodeOf.set(x.id, fresh));
      } else {
        const ns = m.tool ? [toolsNode(last)] : toArr(msgNode(m));
        nodeOf.set(m.id, ns); put(ns);
      }
    }
    // the card of the running or waiting task stays at the top while the history scrolls (G24.2)
    function pin() {
      const mine = st.chatTasks(chatId).filter((x) => ['wait', 'run'].includes(L.kindOf(x)) && L.info(x.status).live);
      mine.sort((a, b) => (L.kindOf(a) === 'wait' ? 0 : 1) - (L.kindOf(b) === 'wait' ? 0 : 1) || Date.parse(b.created_at) - Date.parse(a.created_at));
      for (const [id, c] of cards) c.setPinned(mine[0] && mine[0].id === id);
    }

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
      projectId: () => selProject, activeTask,
      addPending(p) { pending.push(p); addPendingNode(p); toBottom(); },
      dropPending(id) { pending = pending.filter((p) => p.id !== id); dropPendingNode(id); },
      sent: async (newId) => {
        if (isNew && newId) {
          NS.storage.del(L.draftKey(envId, null));
          await st.loadChats(envId);
          if (alive.v) NS.nav('env', { env: envId, chat: newId });
        }
      },
    } });

    root.append(h('section', { class: 'convo' }, header, wrap, composer.el));
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
      }
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
          const c = cards.get(payload.id);
          if (c) c.update(payload);               // only this card is redrawn
          else if (payload.chat_id === chatId) {   // the task arrived after its "Это задача" message: put the card in front of that line
            const m = msgs.find((x) => x.kind === 'task' && x.task_id === payload.id), first = m && (nodeOf.get(m.id) || [])[0], nc = m && cardFor(payload.id);
            if (first && nc) first.before(nc.el);
          }
          pin();
        } else if (evt === 'chatmsg') {
          if (payload.chat_id !== chatId) return;
          const gone = pending.filter((p) => p.id === payload.id || (L.whoOf(payload).kind === 'human' && p.text === payload.text));
          pending = pending.filter((p) => !gone.includes(p));
          gone.forEach((p) => dropPendingNode(p.id));
          const known = msgs.some((x) => x.id === payload.id);
          msgs = L.mergeMsgs(msgs, [payload]);
          const stick = nearBottom();
          const isLast = msgs[msgs.length - 1].id === payload.id;
          if (!known && isLast) addMsgNode(payload);
          else if (known && !payload.tool && nodeOf.get(payload.id)) {   // e.g. delivered_at arrived: swap this message's node
            const old = nodeOf.get(payload.id), fresh = toArr(msgNode(payload));
            old[0].before(...fresh); old.filter((n) => !fresh.includes(n)).forEach((n) => n.remove()); nodeOf.set(payload.id, fresh);
          } else renderFeed();
          if (stick) toBottom();
        } else if (evt === 'limits') { if (payload === envId) { limits.refresh(); composer.drawHint(); } }
        else if (evt === 'catalog' || evt === 'settings') { drawHeader(); composer.refresh(); }
        else if (evt === 'project') { drawHeader(); }
        else if (evt === 'focus-input') composer.focus();
      },
      destroy() { alive.v = false; closeChatMenu(); NS.taskCard.closeMenu(); clearInterval(tick); document.removeEventListener('keydown', onKey); limits.destroy(); composer.destroy(); },
    };
  }

  NS.registry.register({ id: 'chat', layout: 'chats', mount });
})();
