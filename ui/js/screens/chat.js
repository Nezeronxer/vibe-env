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
    let msgs = [], more = false, loading = false, pending = [], queue = [];       // queue: v6 G62, the chat's queue as the core keeps it on disk
    let think = null;                                   // v6 G80: {since} from the frame a message is sent until the first text, action or error
    const cards = new Map();                            // task id -> card controller (kept across re-renders: open state lives there)
    const openGroups = new Set();
    let selProject = (chatObj && chatObj.project_id) || NS.projects.current();     // F11: no silent first project; the last explicit choice is offered
    const alive = { v: true };
    const ext = ctx.params.ext || {};
    const tabId = ctx.params.tab || null;              // the tab this chat lives in (tabs.js): a new chat id goes back to that tab
    const go = (chat) => (tabId && NS.tabs ? NS.tabs.setChat(tabId, envId, chat) : NS.nav('env', { env: envId, chat }));
    const V = window.NSLive;                  // the swarm screen reuses this chat as its main surface (story 156): extra composer control, note target

    // ---------- header ----------
    const title = h('h1', { class: 'pane-ttl' }), sub = h('span', { class: 'pane-sub' });
    const limits = NS.limits.limitsBar(envId);
    const menuBtn = h('button', { type: 'button', class: 'btn sm menu-btn', 'aria-haspopup': 'menu', 'aria-label': t('chat.menu'), text: '⋯' });
    // v4: this header is the pane's title row (tabs.js adds the registry slots and the pane tools); env, model and limits are in the bar
    const lamp = h('span', { class: 'pane-lamp', role: 'img', hidden: true });     // the tally lamp: red = an agent of this chat is on air, amber = it calls you
    // F12 #9: the side question: a narrow panel on the right, ⌘; or this button
    const side = !isNew && NS.sidechat ? NS.sidechat.create(chatId) : null;
    const sideBtn = side ? h('button', { type: 'button', class: 'btn sm side-btn', 'aria-label': t('chatx.side'), title: t('chatx.side_d'), text: t('chatx.side'), onclick: () => side.toggle() }) : null;
    // v6 G83: the chat is open in a terminal (the same session in the real CLI): a quiet mark in the header, sending asks first
    let inTerm = !!(chatObj && chatObj.in_terminal);
    const termMark = h('span', { class: 'term6', title: t('shell.term_on_d'), hidden: !inTerm }, h('span', { class: 't-glyph', 'aria-hidden': 'true', text: '>_' }), h('span', { text: t('shell.term_on') }));
    const setTerm = (on) => { inTerm = !!on; termMark.hidden = !inTerm; };
    // one terminal per chat: a second CLI on the same session would lose one of the two talks (bring the open one forward instead)
    const openTerm = () => {
      if (inTerm) { const have = NS.tabs && window.NSPanes.allPanes(NS.tabs.state).find((x) => x.pane.kind === 'term' && x.pane.chat === chatId); if (have) { NS.tabs.focusPane(have.pane.id); return Promise.resolve(); } NS.shell.toast(t('shell.term_on_d')); return Promise.resolve(); }
      return NS.chatShell.openTerminal(chatId).then((r) => { if (r && alive.v) setTerm(true); });
    };
    // v10: «Чат | Терминал» - the same talk in the real CLI, in this pane (tabs.js chatView); in a pane only
    const viewSw = tabId && NS.tabs && NS.tabs.viewSwitch ? NS.tabs.viewSwitch({ id: tabId, chat: isNew ? 'new' : chatId, term: null }) : null;
    const header = h('header', { class: 'convo-h pane-h' }, h('span', { class: 'pane-ic' }, envIcon(env.custom ? 'custom' : envId)), lamp,
      title, sub, termMark, viewSw, sideBtn, !isNew ? menuBtn : null);
    // F11: a new chat starts with the project button above the field (the menu: recent, open a folder, create)
    const pj = isNew ? NS.projects.picker(() => selProject, (p) => { selProject = p.id; drawHeader(); renderFeed(); composer.focus(); }) : null;
    function drawLamp() {
      const tks = isNew ? [] : st.chatTasks(chatId);
      const calling = tks.find((x) => L.kindOf(x) === 'wait'), onAir = tks.find((x) => L.kindOf(x) === 'run' && L.info(x.status).live), tk = calling || onAir;
      lamp.hidden = !tk;
      if (!tk) return;
      lamp.className = 'pane-lamp' + (calling ? ' calling' : '');
      lamp.setAttribute('aria-label', t(L.labelKey(tk)));
      lamp.title = t(L.labelKey(tk));
    }
    function drawHeader() {
      drawLamp();
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
        // v10: «В терминале» is the «Терминал» switch in the head now (one way, not two); outside a pane the item stays
        viewSw ? null : h('button', { type: 'button', role: 'menuitem', text: t('shell.term'), onclick: () => { closeChatMenu(); openTerm(); } }),
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
    const streams = new Map();       // msg id -> {text, at, final, msg}: the answer printed smoothly (chat-delta, or a whole live message)
    const thoughts = new Map();      // v6 G80.1: msg id -> {text, at, el, open}: what the model thinks before that answer
    const reveals = new Set();       // reveal handles of this feed (NS.reveal), stopped with the screen
    const open = new Set();          // expanded outputs, diffs, glued reads
    const cache = new Map();         // node key -> {sig, nodes}
    const olderLog = new Map();      // task id -> "before" for "показать раньше" (the log keeps the last 500 events)
    let loaded = false;
    const reduced = () => !!(window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches);
    const live = NS.actions.liveBar();
    // G81.1: the feed follows the text only while the person is at its bottom; scrolling up stops it
    let stuck = true;
    // G84: the words of the spinner: the env's own (Claude Code `spinnerVerbs`) instead of or after the app's
    let verbs = t('think.verbs').split('|');
    NS.api.call('ui_hints', { env: envId, projectId: (chatObj && chatObj.project_id) || selProject || null }).then((hint) => {
      const own = hint && Array.isArray(hint.spinner_verbs) ? hint.spinner_verbs.filter((x) => typeof x === 'string' && x.trim()) : [];
      if (own.length) verbs = hint.spinner_append ? verbs.concat(own) : own;
    }).catch(() => {});
    const ts = (v) => (typeof v === 'number' ? v : Date.parse(v) || 0);
    const LOG_LIMIT = ((S.settings && S.settings.keep && S.settings.keep.log_events) || 500);

    function activeTask() {
      const mine = st.chatTasks(chatId).filter((x) => x.status !== 'handed_off' && ['wait', 'run', 'queue'].includes(L.kindOf(x)));
      return mine.find((x) => x.status === 'needs_human' && x.question) || mine[0] || null;
    }
    // the executor is on its way (not waiting for an answer): a new message waits its turn in the core's queue (G62)
    const busy = () => { const a = activeTask(); return !!a && L.kindOf(a) !== 'wait'; };
    function setQueue(list) {
      queue = (list || []).map((q) => ({ id: q.id, text: q.text, at: q.at, attachments: q.attachments || [] }));
      const ids = new Set(queue.map((q) => q.id));
      const was = pending.length;
      pending = pending.filter((p) => !ids.has(p.id));                 // the core queued what was sent while it answered
      if (think && was && !pending.length) think = null;
      schedule(); composer.drawHint();
    }
    async function enqueueCore(text, first, files) {
      const q = { id: qid(), text, at: new Date().toISOString(), attachments: files || [] };
      if (first) queue.unshift(q); else queue.push(q);
      renderFeed(); toBottom();
      const args = { id: chatId, msgId: q.id, text, first: !!first };
      if (files && files.length) args.attachments = files;
      try { await NS.api.call('chat_queue_add', args); }
      catch (e) { queue = queue.filter((x) => x.id !== q.id); renderFeed(); NS.shell.toast(t('chat6.queue_fail', { why: NS.errHuman(NS.api.errText(e)) }), true); }
    }
    const dropQueued = (q) => NS.api.call('chat_queue_drop', { id: chatId, msgId: q.id }).catch((e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true));
    const qid = () => (crypto.randomUUID ? crypto.randomUUID() : 'q-' + Date.now() + Math.random().toString(16).slice(2));
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
      // F9: my message is a grey bubble on the right; the time and delivery sit under it and show on hover
      return h('div', { class: 'me' + (isPending ? ' sending' : '') }, attNode(m), h('div', { class: 'me-b' }, h('span', { class: 'sr-only', text: t('feed.you') }), richText(m.text)),
        h('small', { class: 'me-m' }, NS.hhmm(m.at), isPending ? ' · ' + t('msg.sending') : note ? ' · ' + (m.delivered_at ? t('msg.delivered', { time: NS.hhmm(m.delivered_at) }) : t('msg.will_pass')) + to : '',
        note && NS.rollbackBtn ? [' ', NS.rollbackBtn(m.task_id, NS.snapAfter(m.at))] : null));   // F13: "Вернуть код сюда" (only when the task has a snapshot after this message)
    }
    // v6 G65: the images that went with a message of the person: a quiet row of chips (the file name only)
    function attNode(m) {
      const list = (m && m.attachments) || [];
      if (!list.length) return null;
      return h('div', { class: 'me-att6' }, list.map((p) => h('span', { class: 'att6-c', title: String(p) }, h('span', { 'aria-hidden': 'true', text: '▣' }), h('span', { text: t('img.chip') + ' ' + String(p).split('/').pop() }))));
    }
    // F12 #8: a message that waits for the agent: grey, can be edited or removed until it is sent (v6: in the core's queue)
    function queuedNode(q) {
      const edit = h('button', { type: 'button', class: 'q-act', text: t('chatx.edit'), onclick: () => {
        queue = queue.filter((x) => x.id !== q.id); renderFeed(); dropQueued(q);
        composer.insert(q.text + (composer.ta.value ? '\n' + composer.ta.value : ''));
        (q.attachments || []).forEach((p) => composer.addPath(p));          // the images go back with the text
      } });
      const drop = h('button', { type: 'button', class: 'q-act', 'aria-label': t('chatx.drop'), title: t('chatx.drop'), text: '×', onclick: () => { queue = queue.filter((x) => x.id !== q.id); renderFeed(); composer.drawHint(); dropQueued(q); } });
      return h('div', { class: 'me queued' }, attNode(q), h('div', { class: 'me-b' }, h('span', { class: 'sr-only', text: t('feed.you') }), richText(q.text)),
        h('small', { class: 'me-m' }, t('chatx.queued') + ' · ' + t('chatx.queued_d'), edit, drop));
    }
    // G62: a line of the app `key: details` in words; the details may be a key themselves (`note_not_passed: task_cancelled`)
    function sysHuman(text) {
      const m = /^([a-z_]+):\s*([\s\S]*)$/.exec(String(text || '').trim());
      if (m && window.NSI18n.has('err.' + m[1])) return t('err.' + m[1], { detail: NS.errHuman(m[2]) });
      return NS.errHuman(text);
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
      } else if (m.text === 'stopped' || m.text.startsWith('not_answered: stopped')) {      // F12: the person stopped the run: a calm line, not a failure
        out.push(h('div', { class: 'sysline calm' }, ic('i-warn'), h('span', { text: t('feed.stopped') }),
          h('button', { type: 'button', class: 'linkb', text: t('feed.retry'), onclick: () => NS.api.call('retry_chat', { id: chatId, msgId: m.reply_to || m.id }).catch((e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true)) })));
      } else if (m.kind === 'error') {
        const retry = () => NS.api.call('retry_chat', { id: chatId, msgId: m.reply_to || m.id }).catch((e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true));
        const proj = NS.projects.gitUnseen(m.text) && NS.store.project((NS.store.findChat(chatId) || {}).project_id);
        out.push(h('div', { class: 'sysline bad' }, ic('i-fail'), h('span', { text: t('feed.no_answer', { why: sysHuman(String(m.text).replace(/^not_answered:\s*/, '')) }) }),
          proj ? h('button', { type: 'button', class: 'linkb', text: t('git.trust_btn'), onclick: async () => { if (await NS.projects.trustGit(proj.path)) retry(); } }) : null,
          h('button', { type: 'button', class: 'linkb', text: t('feed.retry'), onclick: retry })));
      } else out.push(h('div', { class: 'sysline' + (m.kind === 'warn' ? ' bad' : '') }, ic(m.kind === 'warn' ? 'i-warn' : 'i-check'), h('span', { text: sysHuman(m.text) })));
      return out;
    }
    // the body of an agent message: markdown, or a word-by-word print of a live message (story 227)
    // F12 #12: "Выполнить" on a shell block: the command goes into a terminal and waits for Enter, it never runs by itself
    const SHELL = /^(bash|sh|zsh|shell|console|terminal)$/;
    function shellOf(pre) {
      const lang = pre.dataset.lang || '', lines = pre.textContent.replace(/\n$/, '').split('\n');
      if (SHELL.test(lang)) return lines.map((x) => x.replace(/^\$ /, '')).join('\n').trim();
      if (!lang && lines.some((x) => x.startsWith('$ '))) return lines.filter((x) => x.startsWith('$ ')).map((x) => x.slice(2)).join('\n').trim();
      return '';
    }
    function runnable(node) {
      for (const pre of node.querySelectorAll('pre.md-code')) {
        const cmd = shellOf(pre);
        if (!cmd) continue;
        const btn = h('button', { type: 'button', class: 'runcmd', text: t('chatx.run'), title: t('chatx.run_d'), onclick: () => runInTerminal(cmd) });
        const wrapEl = h('div', { class: 'codewrap' }); pre.replaceWith(wrapEl); wrapEl.append(pre, btn);
      }
      return node;
    }
    async function runInTerminal(cmd) {
      if (!NS.term || !NS.term.open) return;
      const pj = st.project(chatObj ? chatObj.project_id : selProject), before = new Set(NS.term.views.keys());
      NS.term.open({ cwd: pj ? pj.path : null, title: t('chatx.run_title') });
      let v = null;
      for (let i = 0; i < 40 && !v; i++) {
        await new Promise((r) => setTimeout(r, 100));
        const k = [...NS.term.views.keys()].find((x) => !before.has(x)), w = k && NS.term.views.get(k);
        if (w && w.id() && w.running()) v = w;
      }
      if (!v) return;
      // several lines go as one paste (bracketed), so the shell does not run the first line at once
      v.write(cmd.includes('\n') ? '\x1b[200~' + cmd + '\x1b[201~' : cmd);
      NS.shell.toast(t('chatx.run_toast'));
    }
    function bodyOf(m) {
      const body = h('div', { class: 'msg-b' });
      body.append(runnable(richText(m.text)));
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
        return h('div', { class: 'msg ai' }, h('div', { class: 'msg-h' }, h('span', { class: 'av' }, envIcon(envId)), h('b', { text: env.name }), mlabel ? h('small', { text: mlabel }) : null, h('time', { text: NS.hhmm(m.at) })), bodyOf(m));
      }
      return h('div', { class: 'msg' }, h('div', { class: 'msg-h' }, roleIcon(rk), h('span', { class: 'rname r-' + rk, text: t('role.' + rk) + (m.variant ? ' ' + m.variant : '') }), model ? h('small', { text: model }) : null, h('time', { text: NS.hhmm(m.at) })),
        bodyOf(m));
    }
    // the answer while it prints (G81): one node, plain text that grows frame by frame (NS.reveal), markdown once all of it
    // is shown. Markers of markdown are left out while it prints, so no raw ** or backticks flash by (story 228).
    const plainOf = (text) => String(text || '').replace(/[*`]/g, '');
    function streamNode(id, sx) {
      const mlabel = (sx.msg && sx.msg.model) || NS.models.label(envId) || '';
      const host = h('span', { class: 'rv' });
      const el = h('div', { class: 'msg ai streaming', dataset: { stream: id } }, h('div', { class: 'msg-h' }, h('span', { class: 'av' }, envIcon(envId)), h('b', { text: env.name }), mlabel ? h('small', { text: mlabel }) : null),
        h('div', { class: 'msg-b' }, host, h('span', { class: 'caret', 'aria-hidden': 'true' })));
      const r = NS.reveal.attach(host, () => plainOf(sx.text), { isFinal: () => !!sx.final, onGrow: () => { if (stuck) toBottom(); }, onDone: () => finishStream(id) });
      sx.reveal = r; reveals.add(r);
      return el;
    }
    // all of the answer is shown: the stored message (markdown) takes the place of the printed text
    function finishStream(id) {
      const sx = streams.get(id);
      if (!sx) return;
      streams.delete(id); reveals.delete(sx.reveal);
      if (sx.msg) msgs = L.mergeMsgs(msgs, [sx.msg]);
      if (alive.v) { renderFeed(); if (stuck) toBottom(); }
    }
    // G80.1: the thoughts of the model: one grey folded line «Думает …», the whole text on a click; the node is kept and written to
    function thoughtNode(id, th) {
      if (th.el) return th.el;
      const label = h('b', { text: t('think.thoughts') }), prev = h('span', { class: 'th-p' }), body = h('div', { class: 'th-b', hidden: true });
      const btn = h('button', { type: 'button', class: 'th-h', 'aria-expanded': 'false', title: t('think.open') }, label, prev, ic('i-chev', 'th-chev'));
      th.el = h('div', { class: 'thought', dataset: { thought: id } }, btn, body);
      th.draw = () => {
        label.textContent = streams.has(id) || msgs.some((x) => x.id === id) ? t('think.thought') : t('think.thoughts');
        prev.textContent = th.text.replace(/\s+/g, ' ').trim().slice(-160);
        btn.setAttribute('aria-expanded', String(!!th.open)); th.el.classList.toggle('on', !!th.open); body.hidden = !th.open;
        if (th.open) body.textContent = th.text;
      };
      btn.onclick = () => { th.open = !th.open; th.draw(); };
      th.draw();
      return th.el;
    }
    const emptyNode = () => h('div', { class: 'newchat-empty' }, h('h2', { text: t('feed.empty_title', { env: env.name }) }), pj ? pj.el : null, st.project(selProject) ? null : h('p', { text: t('feed.empty') }));
    // examples as three quiet cards under the field; a click puts the text into the field, it is not sent (story 239)
    const EX = [['onb3.ex1', 'onb3.ex1_t', 'f9.ex1_d'], ['onb3.ex2', 'onb3.ex2_t', 'f9.ex2_d'], ['onb3.ex3', 'onb3.ex3_t', 'f9.ex3_d']];
    const blankEx = h('div', { class: 'examples', role: 'group', 'aria-label': t('onb3.ex_title'), hidden: true }, EX.map(([label, text, d]) => h('button', { type: 'button', class: 'ex',
      onclick: () => composer.insert(t(text)) }, h('b', { text: t(label) }), h('span', { text: t(d) }))));

    // ---------- steps of the agent (F9): one quiet line per action, the details open on a click ----------
    // The line says what happened ("Изменил test_calc.py +1 −0"); the old action block (diff, output, parameters) lives inside and is built on the first open.
    const SVGNS = 'http://www.w3.org/2000/svg';
    const GLYPHS = {
      read: ['M4 1.75h5l3 3v9.5H4z', 'M9 1.75v3h3'], edit: ['M10.5 2.5l3 3-8 8H2.5v-3z', 'M9 4l3 3'], write: ['M4 1.75h5l3 3v9.5H4z', 'M9 1.75v3h3', 'M8 7.5v4', 'M6 9.5h4'],
      bash: ['M3 4.5l3.5 3.5L3 11.5', 'M8.5 12h4.5'], search: ['M7 2.5a4.5 4.5 0 1 0 0 9a4.5 4.5 0 1 0 0-9z', 'M10.3 10.3l3.2 3.2'],
      web: ['M8 2a6 6 0 1 0 0 12a6 6 0 1 0 0-12z', 'M2 8h12', 'M8 2c2.2 2 2.2 10 0 12', 'M8 2c-2.2 2-2.2 10 0 12'], other: ['M9 1.75L3.75 9H8l-1 5.25L12.25 7H8z'] };
    function glyph(kind) {
      const svg = document.createElementNS(SVGNS, 'svg');
      svg.setAttribute('viewBox', '0 0 16 16'); svg.setAttribute('class', 'stp-g'); svg.setAttribute('aria-hidden', 'true');
      for (const d of GLYPHS[kind] || GLYPHS.other) { const pth = document.createElementNS(SVGNS, 'path'); pth.setAttribute('d', d); svg.append(pth); }
      return svg;
    }
    const baseName = (x) => String(x || '').split('/').filter(Boolean).pop() || String(x || '');
    const tailPath = (x) => String(x || '').split('/').filter(Boolean).slice(-2).join('/');
    const firstLine = (x, n) => String(x == null ? '' : x).split('\n')[0].slice(0, n);
    const KINDS = ['read', 'edit', 'write', 'bash', 'search', 'web'];
    // what the line shows for an action: kind, verb, argument, "+1 −0", the result of a command ("7 из 9")
    function stepInfo(action, live) {
      const inp = V.inputOf(action), kind = KINDS.includes(inp.kind) ? inp.kind : 'other';
      const running = !action.done && live !== false, bad = action.done && action.ok === false;
      const out = action.output == null ? '' : String(action.output);
      let arg = '', title = '', stat = null, res = '';
      if (kind === 'read' || kind === 'edit' || kind === 'write') { arg = baseName(inp.path); title = inp.path || ''; }
      else if (kind === 'bash') { arg = firstLine(inp.command, 140); title = inp.command || ''; }
      else { arg = firstLine(NS.actions.argOf(inp, action) || action.name || '', 140); title = arg; }
      if (kind === 'edit' && inp.diff) { const d = V.diffStat(V.parseDiff(inp.diff)); stat = [d.add, d.del]; }
      else if (kind === 'write') stat = [inp.lines != null ? Number(inp.lines) : String(inp.head || '').split('\n').length, null];
      if (kind === 'bash' && action.done) {
        const ps = /(\d+)\s+passed/.exec(out), fl = /(\d+)\s+failed/.exec(out);
        if (ps || fl) { const a = ps ? +ps[1] : 0, b = a + (fl ? +fl[1] : 0); res = t('st.of', { a, b }); }
      }
      if (bad && !res) res = t('st.error');
      return { kind, running, bad, arg, title, stat, res, inp, verb: t('st.' + kind + (running ? '_r' : '')) };
    }
    const fold = new Map();         // step / group key -> the user's choice (true = open); no entry = the default
    const stepKeys = (e) => 'f:' + String(e.action.id || e.key);
    function stepNode(e, o) {
      const a = e.action, info = stepInfo(a, o.live), key = stepKeys(e);
      const dflt = info.bad && !!o.last;                       // a failed command stays open only while it is the last event
      const isOpen = () => (fold.has(key) ? fold.get(key) : dflt);
      const body = h('div', { class: 'stp-b', hidden: true });
      const stat = info.stat ? h('span', { class: 'stp-s' }, h('span', { class: 'plus', text: '+' + info.stat[0] }), info.stat[1] != null ? [' ', h('span', { class: 'minus', text: '−' + info.stat[1] })] : null) : null;
      const btn = h('button', { type: 'button', class: 'stp-h', 'aria-expanded': 'false', title: info.title || '' }, glyph(info.kind), h('span', { class: 'stp-v', text: info.verb }),
        info.arg ? h('span', { class: 'stp-a', text: info.arg }) : null, stat, info.res ? h('span', { class: 'stp-r' + (info.bad ? ' bad' : ''), text: '· ' + info.res }) : null, ic('i-chev', 'stp-chev'));
      const el = h('div', { class: 'stp k-' + info.kind + ' s-' + (info.running ? 'run' : info.bad ? 'bad' : 'ok'), dataset: { act: String(a.id || '') } }, btn, body);
      const draw = () => {
        const v = isOpen();
        btn.setAttribute('aria-expanded', String(v)); body.hidden = !v; el.classList.toggle('on', v);
        if (v && !body.firstChild) NS.append(body, [info.kind === 'bash' && info.inp.command ? h('div', { class: 'stp-cmd' }, h('span', { 'aria-hidden': 'true', text: '$ ' }), info.inp.command) : null, NS.actions.actionNode(a, { open, live: o.live })]);
      };
      btn.onclick = () => { fold.set(key, !isOpen()); draw(); };
      draw();
      return el;
    }
    // two or more actions of one role in a row: one group "3 действия" that folds, the lines inside go in a ladder
    function groupNode(entries, o) {
      const infos = entries.map((e) => stepInfo(e.action, o.live)), key = 'g:' + entries[0].key;
      const running = infos.some((x) => x.running), lastBad = infos[infos.length - 1].bad;
      const dflt = !!o.last && (running || lastBad);
      const isOpen = () => (fold.has(key) ? fold.get(key) : dflt);
      const n = entries.length, count = t('st.group_' + L.plural(n, ['1', '2', '5'], NS.lang()), { n });
      const tail = infos.slice(0, 3).map((x) => [x.verb, x.arg].filter(Boolean).join(' ')).join(' · ');
      const list = h('div', { class: 'agrp-b', hidden: true });
      const btn = h('button', { type: 'button', class: 'stp-h agrp-h', 'aria-expanded': 'false' }, h('span', { class: 'agrp-dot' + (running ? ' run' : lastBad ? ' bad' : ''), 'aria-hidden': 'true' }),
        h('span', { class: 'stp-v', text: count }), h('span', { class: 'stp-a agrp-t', text: tail }), ic('i-chev', 'stp-chev'));
      const el = h('div', { class: 'agrp s-' + (running ? 'run' : lastBad ? 'bad' : 'ok') }, btn, list);
      const draw = () => {
        const v = isOpen();
        btn.setAttribute('aria-expanded', String(v)); list.hidden = !v; el.classList.toggle('on', v);
        if (v && !list.firstChild) NS.append(list, entries.map((e, i) => stepNode(e, { live: o.live, last: o.last && i === entries.length - 1 })));
      };
      btn.onclick = () => { fold.set(key, !isOpen()); draw(); };
      draw();
      return el;
    }
    // actions of one role in a row, in order
    const bySpeaker = (run) => { const out = []; for (const e of run) { const l = out[out.length - 1]; if (l && l[0].role === e.role) l.push(e); else out.push([e]); } return out; }
    const toArr = (x) => [].concat(x);
    const actOpts = (e) => ({ open, live: e.taskId ? isLiveTask(S.tasks.get(e.taskId)) : !!(streams.size || think || [...acts.values()].some((x) => !x.taskId && !x.action.done)) });

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
      for (const [id, th] of thoughts) items.push({ at: th.at, type: 'thought', id, th });
      return items.sort((a, b) => a.at - b.at || (a.type === 'thought' ? -1 : b.type === 'thought' ? 1 : 0));
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
      const blank = !items.length && !pending.length && !queue.length;
      if (blank) put('empty', String(selProject), () => [emptyNode()]);
      convoEl.classList.toggle('blank', blank);                    // an empty chat: the question, the field and the examples in the middle (pin-25)
      blankEx.hidden = !blank;
      let run = [];
      const flush = (isLast) => {
        const segs = bySpeaker(run);
        for (const [si, seg] of segs.entries()) {
          const lastSeg = !!isLast && si === segs.length - 1;
          const sigOf = (e) => e.key + (e.action.done ? '1' : '0') + (e.action.ok === false ? 'x' : '');
          if (seg.length > 1) { const o = actOpts(seg[0]); put('g:' + seg[0].key, seg.map(sigOf).join() + o.live + lastSeg, () => [groupNode(seg, Object.assign({ last: lastSeg }, o))]); }
          else { const e = seg[0], o = actOpts(e); put('a:' + e.key, JSON.stringify(e.action) + o.live + lastSeg, () => [stepNode(e, Object.assign({ last: lastSeg }, o))]); }
        }
        run = [];
      };
      for (const it of items) {
        if (it.type === 'act') { run.push(it.e); continue; }
        flush();
        if (it.type === 'msg') put('m:' + it.m.id, JSON.stringify(it.m) + (it.m.task_id && cards.has(it.m.task_id) ? '+' : ''), () => toArr(msgNode(it.m)));
        else if (it.type === 'thought') put('th:' + it.id, 'th', () => [thoughtNode(it.id, it.th)]);
        else put('s:' + it.id, 's', () => [streamNode(it.id, it.s)]);       // made once: the text grows inside it
      }
      flush(!pending.length);
      for (const p of pending) put('p:' + p.id, p.text, () => [humanNode(p, true)]);
      for (const q of queue) put('q:' + q.id, q.text, () => [queuedNode(q)]);
      for (const k of [...cache.keys()]) if (!used.has(k)) cache.delete(k);
      for (const th of thoughts.values()) if (th.draw) th.draw();
      nodes.push(live.el);
      const same = nodes.length === feed.childNodes.length && nodes.every((n, i) => feed.childNodes[i] === n);
      if (!same) feed.replaceChildren(...nodes);
      drawLive();
      pin();
      if (stick) toBottom(); else wrap.scrollTop = prevTop;
    }
    // the live line: verb by the last action of the running task (or of the chat answer), timer of the step, role and model, the plan
    // F12 #11: the next step as a grey line, from rules by the state (no model call)
    let lastHint;
    function nextHint() {
      if (isNew || busy() || !loaded || queue.length || think) return null;
      const talk = msgs.filter((m) => !m.action && !m.tool && m.kind !== 'error' && m.text);
      const lastTalk = talk[talk.length - 1], lastTask = st.chatTasks(chatId).slice().sort((a, b) => ts(b.created_at) - ts(a.created_at))[0];
      if (lastTalk && L.whoOf(lastTalk).kind === 'human' && !(lastTask && ts(lastTask.finished_at) > ts(lastTalk.at))) return null;      // the agent has not answered yet
      if (lastTask) {
        if (lastTask.status === 'done_not_pushed') return t('chatx.h_push');
        if (lastTask.status === 'done') return /тест|test/i.test(lastTask.title) ? t('chatx.h_mobile') : t('chatx.h_tests', { title: firstLine(lastTask.title, 40) });
        if (lastTask.status === 'failed') return t('chatx.h_why');
        return null;
      }
      return lastTalk && L.whoOf(lastTalk).kind !== 'human' ? t('chatx.h_do') : null;
    }
    function drawLive() {
      const hintNow = nextHint();
      if (hintNow !== lastHint) { lastHint = hintNow; composer.suggest(hintNow); }
      const mine = st.chatTasks(chatId);
      const tk = mine.filter(isLiveTask).sort((a, b) => ts(b.started_at || b.created_at) - ts(a.started_at || a.created_at))[0];
      const lastOf = (pred) => { let best = null; for (const e of acts.values()) if (pred(e) && V.inputOf(e.action).kind !== 'plan' && (!best || e.at >= best.at)) best = e; return best; };
      const planOf = (pred) => { let best = null; for (const e of acts.values()) if (pred(e) && V.inputOf(e.action).kind === 'plan' && (!best || e.at >= best.at)) best = e; return best ? V.inputOf(best.action).items || [] : null; };
      const toast = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
      if (tk) {
        const run = runningRun(tk), rk = run && L.roleKey(run.role);
        const last = lastOf((e) => e.taskId === tk.id);
        const streaming = streams.size > 0;
        const stop = () => NS.api.call('cancel', { id: tk.id }).catch(toast);
        composer.setWork({ who: rk ? t('role.' + rk) : env.name, what: workText(last, streaming), stop });
        const since = ts(run && run.started_at) || ts(tk.started_at), who = [rk ? t('role.' + rk) : null, run && run.model].filter(Boolean).join(' · ');
        // G80: the run of the task has said nothing yet: the same "thinking" line as after a message
        if (!last && !streaming) return live.set({ think: { verbs, stop }, since, who, plan: planOf((e) => e.taskId === tk.id) });
        return live.set({ verbKey: V.actionVerb(last && last.action, streaming && (!last || last.action.done), { role: rk, lastRole: last && last.role }), since,
          who, plan: planOf((e) => e.taskId === tk.id) });
      }
      const queued = mine.find((x) => x.status === 'queued');
      const q = queued && V.queueInfo([...S.tasks.values()], queued.id, (S.settings && S.settings.max_parallel) || 3);
      if (q) { composer.setWork(null); return live.set({ queue: q.after ? t('live.queue', q) : t('live.queue_short', q) }); }
      const chatLast = lastOf((e) => !e.taskId);
      const unfinished = [...streams.values()].some((x) => !x.final);
      const chatBusy = unfinished || (chatLast && !chatLast.action.done);
      const stopChat = isNew ? null : () => NS.api.call('chat_stop', { id: chatId }).catch(toast);
      if (chatBusy) {
        const sx = [...streams.values()][0];
        composer.setWork({ who: env.name, what: workText(chatLast, unfinished), stop: stopChat });
        return live.set({ verbKey: V.actionVerb(chatLast && !chatLast.action.done ? chatLast.action : null, unfinished), since: (think && think.since) || (sx && sx.at) || (chatLast && chatLast.at) || Date.now(),
          who: [env.name, NS.models.label(envId)].filter(Boolean).join(' · '), plan: planOf((e) => !e.taskId) });
      }
      // G80: sent, nothing back yet: "thinking" right where the answer will be
      if (think) {
        composer.setWork({ who: env.name, what: t('work.think'), stop: stopChat });
        return live.set({ think: { verbs, stop: stopChat }, since: think.since, who: [env.name, NS.models.label(envId)].filter(Boolean).join(' · ') });
      }
      composer.setWork(null);
      live.set(null);
    }
    // the plate over the field: what the agent is doing right now ("правит cursor.py")
    function workText(last, streaming) {
      if (streaming && (!last || last.action.done)) return t('work.answer');
      if (!last) return t('work.think');
      const info = stepInfo(last.action, true);
      if (last.action.done && !['edit', 'write'].includes(info.kind)) return t('work.think');
      return KINDS.includes(info.kind) ? t('work.' + info.kind, { a: info.kind === 'bash' || info.kind === 'search' || info.kind === 'web' ? firstLine(info.arg, 40) : tailPath(info.title) }) : t('work.think');
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
    wrap.addEventListener('scroll', () => { stuck = nearBottom(); if (wrap.scrollTop < 60) loadOlder(); });

    const convoEl = h('section', { class: 'convo' });
    const composer = NS.composer.create({ envId, chat: chatId, hooks: {
      projectId: () => selProject, activeTask, busy,
      chatId: () => (isNew ? null : chatId), inTerminal: () => inTerm, openTerminal: isNew ? null : openTerm,
      enqueue(text, files) { return enqueueCore(text, false, files); },
      async now(text, files) {                        // "Сразу": stop the current run; the message is first in the queue and goes as soon as the run is over
        const tk = activeTask();
        await enqueueCore(text, true, files);
        if (!tk) return;
        try { await NS.api.call('cancel', { id: tk.id }); } catch (e) { NS.shell.toast(NS.errHuman(NS.api.errText(e)), true); }
      },
      needProject: () => { if (!isNew || st.project(selProject)) return false; NS.shell.toast(t('pj.need')); pj.open(); return true; }, noteTo: ext.noteTo,
      // G80: a message to the model (not a note to a task): "thinking" shows in the same frame as the bubble
      addPending(p) { pending.push(p); if (!activeTask()) think = { since: Date.now() }; stuck = true; renderFeed(); toBottom(); },
      dropPending(id) { pending = pending.filter((p) => p.id !== id); if (!pending.length) think = null; schedule(); },
      sent: async (newId) => {
        if (isNew && newId) {
          NS.storage.del(L.draftKey(envId, null));
          await st.loadChats(envId);
          if (alive.v) go(newId);
        }
      },
    } });

    const convoy = chatObj && chatObj.convoy && NS.factory ? NS.factory.convoyBar(chatId) : null;       // "Поручение": overall progress "3 из 5" (story 163)
    NS.append(convoEl, [header, convoy ? convoy.el : null, wrap, composer.el, blankEx, side ? side.el : null]);
    root.append(convoEl);
    if (ext.decorate) ext.decorate(composer);
    drawHeader();
    renderFeed();

    // ---------- data ----------
    (async () => {
      if (!isNew) {
        try {
          const batch = (await NS.api.call('get_chat', { id: chatId, before: null, limit: L.MSG_LIMIT })) || [];
          more = L.hasMore(batch.length, L.MSG_LIMIT);
          msgs = L.mergeMsgs([], batch);
          // opened while the model has not answered the last message yet (a new chat just made, a chat opened again): "thinking"
          const talk = msgs.filter((m) => !m.action && !m.tool), lastTalk = talk[talk.length - 1];
          if (lastTalk && L.whoOf(lastTalk).kind === 'human' && !lastTalk.task_id && !msgs.some((m) => m.reply_to === lastTalk.id) && Date.now() - ts(lastTalk.at) < 30 * 60000 && !think) think = { since: ts(lastTalk.at) };
          if (alive.v) { renderFeed(); toBottom(); }
        } catch (e) { if (alive.v) NS.shell.toast(NS.errHuman(NS.api.errText(e)), true); }
        try { const qs = await NS.api.call('chat_queue', { id: chatId }); if (alive.v && qs) setQueue(qs.queue); } catch (e) { /* an older core: no queue on disk */ }
        await Promise.all(st.chatTasks(chatId).map((tk) => loadTaskLog(tk.id)));
        if (alive.v) toBottom();
      }
      loaded = true;
    })();
    st.loadLimits(envId).then(() => alive.v && (limits.refresh(), composer.drawHint()));
    st.loadCatalog(envId).then(() => { if (!alive.v) return; drawHeader(); composer.refresh(); cards.forEach((c, id) => S.tasks.get(id) && c.update(S.tasks.get(id))); });
    const tick = setInterval(() => cards.forEach((c) => c.tick()), 30000);
    // Esc closes pop-ups first (composer.js), then an open task card
    const onKey = (e) => {
      if (side && (e.metaKey || e.ctrlKey) && e.code === 'Semicolon' && (convoEl.contains(document.activeElement) || (NS.tabs && NS.tabs.active && NS.tabs.active.chat === chatId))) { e.preventDefault(); side.toggle(); return; }
      if (e.key === 'Escape' && !e.defaultPrevented) for (const c of cards.values()) if (c.open) { c.close(); break; }
    };
    document.addEventListener('keydown', onKey);
    // v6: the thoughts of the model (G80.1) and the queue of the chat as the core keeps it (G62) come straight from the core
    const offs = isNew ? [] : [
      NS.api.listen('chat-thinking', (pl) => {
        if (!alive.v || !pl || pl.id !== chatId || !pl.msgId) return;
        const th = thoughts.get(pl.msgId) || { text: '', at: Date.now(), open: false };
        th.text += String(pl.text || '');
        if (!thoughts.has(pl.msgId)) { thoughts.set(pl.msgId, th); schedule(); } else if (th.draw) th.draw();
        if (stuck) toBottom();
      }),
      NS.api.listen('chat-queue', (pl) => { if (alive.v && pl && pl.id === chatId) setQueue(pl.queue); }),
    ];

    return {
      update(evt, payload) {
        if (evt === 'task') {
          if (convoy) convoy.update();
          const c = cards.get(payload.id);
          if (c) c.update(payload);               // only this card is redrawn
          drawLamp();
          if (payload.chat_id === chatId) schedule();   // a card that arrived after its "Это задача" line, the live line, action spinners
          pin();
        } else if (evt === 'chatmsg') {
          if (payload.chat_id !== chatId) return;
          if (payload.who === 'system' && /^(opened_in_terminal|continued_in_terminal|back_from_terminal)\b/.test(payload.text || '')) setTerm(/^opened/.test(payload.text));
          pending = pending.filter((p) => !(p.id === payload.id || (L.whoOf(payload).kind === 'human' && p.text === payload.text)));
          queue = queue.filter((q) => q.id !== payload.id);                           // a queued message went out
          const known = msgs.some((x) => x.id === payload.id);
          const kind = L.whoOf(payload).kind;
          const agent = ['model', 'role'].includes(kind) && !payload.tool && payload.text;
          if (kind !== 'human' && (agent || kind === 'system' || payload.action)) think = null;       // G80: an answer, an action or a line ends "thinking"
          // a stop or a failure in the middle of an answer comes as one system line (reply_to set): nothing of this answer goes on -
          // the streams that never got their final message end as they are, the chat's unfinished actions end as failed
          if (kind === 'system' && payload.reply_to) {
            for (const [sid, sx] of streams) if (!sx.final) { streams.delete(sid); reveals.delete(sx.reveal); if (sx.text) msgs = L.mergeMsgs(msgs, [{ id: sid, chat_id: chatId, who: 'model', text: sx.text, at: new Date(sx.at || Date.now()).toISOString() }]); }
            for (const e of acts.values()) if (!e.taskId && e.action && !e.action.done) e.action = Object.assign({}, e.action, { done: true, ok: false });
          }
          const sx = streams.get(payload.id);
          if (sx) {                                     // G81: the streamed answer is complete: it prints to its end, then markdown replaces it
            Object.assign(sx, { text: payload.text, final: true, msg: payload });
            if (reduced() || !sx.reveal) finishStream(payload.id);
            schedule(); return;
          }
          if (loaded && !known && agent && !reduced()) {      // no partial messages from this CLI: the whole message prints the same way
            streams.set(payload.id, { text: payload.text, at: ts(payload.at), final: true, msg: payload });
            schedule(); return;
          }
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
          think = null;
          schedule();
        } else if (evt === 'chatdelta') {
          if (payload.id !== chatId || msgs.some((x) => x.id === payload.msgId)) return;
          const sx = streams.get(payload.msgId) || { text: '', at: Date.now() };
          if (sx.final) return;
          sx.text += String(payload.text || '');
          streams.set(payload.msgId, sx);
          think = null;
          schedule();
        } else if (evt === 'limits') { if (payload === envId) { limits.refresh(); composer.drawHint(); } }
        else if (evt === 'catalog' || evt === 'settings') { drawHeader(); composer.refresh(); }
        else if (evt === 'project' || evt === 'chats') { drawHeader(); }
        else if (evt === 'focus-input') composer.focus();
      },
      destroy() { alive.v = false; if (pj && pj.destroy) pj.destroy(); for (const r of reveals) r.stop(); offs.forEach((f) => f.then((u) => u && u()).catch(() => {})); live.destroy(); if (convoy) convoy.destroy(); closeChatMenu(); NS.taskCard.closeMenu(); clearInterval(tick); document.removeEventListener('keydown', onKey); limits.destroy(); composer.destroy(); },
    };
  }

  NS.registry.register({ id: 'chat', layout: 'chats', mount });
})();
