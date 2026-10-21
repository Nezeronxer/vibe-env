// Nightshift UI. Plain JS, no build. Talks to the core only through Tauri commands listed in API below.
(function () {
  'use strict';
  const TAURI = window.__TAURI__; // needs withGlobalTauri; dev-mock provides it outside Tauri
  const L = window.NSLogic;

  // ---- the only commands the UI may call (see interfaces.md) ----
  const API = ['list_projects', 'add_project', 'update_project', 'remove_project', 'init_repo', 'repo_info',
    'list_tasks', 'get_log', 'submit_task', 'answer', 'approve_outward', 'cancel', 'retry', 'remove_task', 'undo',
    'push_base', 'pull_base', 'pause_all', 'list_agents', 'get_settings', 'set_settings', 'github_info',
    'delete_merged_branches', 'pr_state', 'get_diffstat', 'open_path', 'open_editor', 'open_terminal'];
  function call(cmd, args) {
    if (!API.includes(cmd)) throw new Error('unknown command ' + cmd);
    return TAURI.core.invoke(cmd, args || {});
  }

  // ---- state ----
  const S = {
    projects: [], tasks: new Map(), settings: null, agents: [], sel: null, open: null,
    repo: {}, gh: {}, logs: new Map(), diff: {}, prState: {}, branchesOpen: false, showArchive: false,
    answerDraft: {}, busy: new Set(),
  };
  const rowCache = new Map(); // taskId -> {el, sig}
  const $ = (id) => document.getElementById(id);
  const IS_MAC = /Mac|iPhone|iPad/.test(navigator.platform || '');
  const MOD = IS_MAC ? '⌘' : 'Ctrl';

  // ---- tiny DOM helper (text is always set via textContent, never innerHTML) ----
  function h(tag, attrs, ...kids) {
    const el = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs || {})) {
      if (v == null || v === false) continue;
      if (k === 'class') el.className = v;
      else if (k === 'text') el.textContent = v;
      else if (k === 'style') throw new Error('inline style is not allowed (CSP)');
      else if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
      else if (k in el && !k.includes('-')) el[k] = v;
      else el.setAttribute(k, v === true ? '' : v);
    }
    for (const c of kids.flat(Infinity)) {
      if (c == null || c === false) continue;
      el.append(c.nodeType ? c : document.createTextNode(String(c)));
    }
    return el;
  }
  const t = (k, p) => window.NSI18n.t(k, p);
  const lang = () => window.NSI18n.lang();
  const pl = (n, key) => L.plural(n, t(key).split('|'), lang());

  function errText(e) { return typeof e === 'string' ? e : (e && e.message) || JSON.stringify(e); }
  function errHuman(raw) {
    if (!raw) return '';
    const key = 'err.' + String(raw).trim();
    const tr = window.NSI18n.has(key) ? t(key) : null;
    return tr || raw;
  }
  let toastTimer = 0;
  function toast(msg, bad) {
    const el = $('toast');
    el.textContent = msg; el.className = 'toast' + (bad ? ' bad' : ''); el.hidden = false;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => { el.hidden = true; }, bad ? 7000 : 3500);
  }
  // run an action; disable duplicates by key; show error text on failure
  async function act(key, fn, okMsg) {
    if (key && S.busy.has(key)) return false;
    if (key) S.busy.add(key);
    try { await fn(); if (okMsg) toast(okMsg); return true; }
    catch (e) { toast(errHuman(errText(e)), true); return false; }
    finally { if (key) S.busy.delete(key); }
  }

  // ---- lookups ----
  const project = (id) => S.projects.find((p) => p.id === id);
  const agent = (id) => S.agents.find((a) => a.id === id);
  const agentName = (id) => (agent(id) ? agent(id).name : id);
  const tasksArr = () => [...S.tasks.values()];
  const NEED = (s) => s === 'needs_human' || s === 'awaiting_outward';
  const statusClass = (s) => L.info(s).cls;
  const fmtTime = (iso, withSec) => iso ? new Date(iso).toLocaleTimeString(lang() === 'ru' ? 'ru-RU' : 'en-GB',
    withSec ? { hour: '2-digit', minute: '2-digit', second: '2-digit' } : { hour: '2-digit', minute: '2-digit' }) : '';
  const fmtDate = (iso) => iso ? new Date(iso).toLocaleString(lang() === 'ru' ? 'ru-RU' : 'en-GB',
    { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }) : '';
  const liveSince = (task) => L.info(task.status).live && task.started_at;
  function elapsedOf(task) {
    if (!task.started_at) return '';
    const end = !L.info(task.status).live && task.finished_at ? Date.parse(task.finished_at) : Date.now();
    return L.formatElapsed(end - Date.parse(task.started_at));
  }

  // ---- one-line outcome summary: summary · files · tests · review · commit ----
  function outcomeParts(task) {
    const o = task.outcome || {};
    const parts = [];
    if (o.summary) parts.push(o.summary);
    if (o.files_changed != null) parts.push(`${o.files_changed} ${pl(o.files_changed, 'u.files')}`);
    parts.push(o.tests ? (o.tests.passed ? t('tests.ok') : t('tests.red')) : t('tests.none'));
    if (o.review) parts.push(o.review.approved ? t('review.ok') : t('review.changes'));
    if (o.commit) parts.push(String(o.commit).slice(0, 7));
    return parts;
  }

  // ---- drafts (localStorage may be unavailable) ----
  function draftGet(pid) { try { return localStorage.getItem('ns.draft.' + pid) || ''; } catch (e) { return ''; } }
  function draftSet(pid, v) { try { if (v) localStorage.setItem('ns.draft.' + pid, v); else localStorage.removeItem('ns.draft.' + pid); } catch (e) { /* storage blocked: draft just isn't kept */ } }

  // ================= data loading =================
  async function loadAll() {
    const [projects, tasks, settings, agents] = await Promise.all([
      call('list_projects'), call('list_tasks'), call('get_settings'), call('list_agents')]);
    S.projects = projects; S.settings = settings; S.agents = agents;
    S.tasks = new Map(tasks.map((x) => [x.id, x]));
    window.NSI18n.setLang(settings.language);
    if (!S.sel || !project(S.sel)) S.sel = (settings.default_project && project(settings.default_project)) ? settings.default_project : (projects[0] && projects[0].id) || null;
  }
  async function loadProjectInfo(pid) {
    if (!pid) return;
    try { S.repo[pid] = await call('repo_info', { id: pid }); } catch (e) { S.repo[pid] = { error: errText(e) }; }
    try { S.gh[pid] = await call('github_info', { projectId: pid }); } catch (e) { S.gh[pid] = { ghHint: errText(e), branches: [] }; }
    if (S.sel === pid) renderMain();
  }

  // ================= sidebar =================
  function projStatus(pid) {
    const ts = tasksArr().filter((x) => x.project_id === pid);
    const needs = L.countNeeds(ts);
    const working = ts.filter((x) => L.groupOf(x, Date.now()) === 'working').length;
    const failed = ts.filter((x) => x.status === 'failed').length;
    return { needs, working, failed };
  }
  function renderSide() {
    const side = $('side');
    const all = tasksArr();
    const needs = L.countNeeds(all);
    const running = all.filter((x) => L.groupOf(x, Date.now()) === 'working').length;
    const paused = S.settings && S.settings.paused;
    side.replaceChildren(
      h('div', { class: 'side-head' },
        h('div', { class: 'brand', text: 'Nightshift' }),
        h('div', { class: 'summary', 'aria-live': 'polite', text: `${running} ${t('sum.running')} · ${needs} ${t('sum.waiting')}` }),
        h('button', { class: 'btn block', type: 'button', 'aria-pressed': paused ? 'true' : 'false', onclick: togglePause, text: paused ? t('pause.resume') : t('pause.pause') })),
      h('ul', { class: 'projects' }, S.projects.map((p) => {
        const st = projStatus(p.id);
        const cls = st.needs ? 'needs' : st.working ? 'working' : st.failed ? 'failed' : '';
        const label = st.needs ? t('proj.needs') : st.working ? t('proj.working') : st.failed ? t('proj.failed') : t('proj.idle');
        return h('li', {}, h('button', {
          class: 'proj', type: 'button', 'aria-current': p.id === S.sel ? 'true' : 'false',
          onclick: () => selectProject(p.id),
        }, h('span', { class: 'dot ' + cls, title: label, role: 'img', 'aria-label': label }),
          h('span', { class: 'name', text: p.name }),
          st.needs ? h('span', { class: 'badge', text: `${st.needs} ${t('badge.waiting')}` }) : null));
      })),
      h('div', { class: 'side-foot' },
        h('button', { class: 'btn block', type: 'button', onclick: () => openAddDialog(), text: '+ ' + t('proj.add') }),
        h('div', { class: 'dropzone-mini', text: t('drop.mini') }),
        h('button', { class: 'btn block', type: 'button', onclick: openGeneralSettings, text: t('settings.general') })));
    updateTitle(needs);
  }
  function updateTitle(needs) {
    document.title = needs ? `(${needs}) Nightshift` : 'Nightshift';
  }
  async function togglePause() {
    const next = !S.settings.paused;
    await act('pause', async () => { await call('pause_all', { paused: next }); S.settings.paused = next; });
    renderSide(); renderMain();
  }
  function selectProject(pid) {
    S.sel = pid; S.branchesOpen = false;
    renderSide(); renderMain();
    loadProjectInfo(pid);
  }

  // ================= main =================
  function renderMain() {
    const main = $('main');
    const keepScroll = main.scrollTop;
    const p = project(S.sel);
    if (!p) { renderEmpty(main); return; }
    // composer textarea persists across list updates: keep node if already in DOM for this project
    let comp = main.querySelector('.composer-wrap');
    const same = comp && comp.dataset.pid === p.id && comp.dataset.agentok === String(agentOk(p)) && comp.dataset.lang === lang();
    const inner = h('div', { class: 'main-inner' });
    if (S.settings.paused) inner.append(h('div', { class: 'banner warn', role: 'status' },
      h('span', { class: 'grow', text: t('pause.banner') }),
      h('button', { class: 'btn sm', type: 'button', onclick: togglePause, text: t('pause.resume') })));
    inner.append(projectHeader(p));
    const rinfo = S.repo[p.id];
    if (rinfo && rinfo.is_repo === false) inner.append(initBanner(p));
    inner.append(same ? comp : composerBlock(p));
    const groupsHost = h('div', { id: 'groups', class: 'groups' });
    inner.append(groupsHost);
    main.replaceChildren(inner);
    renderList();
    main.scrollTop = keepScroll;
  }
  function renderEmpty(main) {
    main.replaceChildren(h('div', { class: 'main-inner' }, h('div', { class: 'empty' },
      h('h1', { text: t('empty.title') }),
      h('p', { class: 'hint', text: t('empty.sub') }),
      h('div', { class: 'dropzone' },
        h('strong', { text: t('empty.drop') }),
        h('span', { class: 'hint', text: t('empty.or') }),
        h('button', { class: 'btn primary', type: 'button', onclick: pickFolder, text: t('proj.pick') })),
      h('button', { class: 'link', type: 'button', onclick: () => openAddDialog(), text: t('empty.paste') }),
      noAgentNote())));
  }
  function noAgentNote() {
    const def = S.agents.find((a) => a.id === 'claude') || S.agents[0];
    if (!def || def.available) return null;
    return h('div', { class: 'card-note left' },
      h('h2', { text: t('agent.install', { name: def.name }) }),
      h('div', { class: 'code', text: def.hint || '' }));
  }
  function agentOk(p) { const a = agent(p.agent); return a ? a.available : true; }

  function projectHeader(p) {
    const info = S.repo[p.id] || {};
    const gh = S.gh[p.id];
    const hasOrigin = !!info.origin_url;
    const ahead = info.ahead || 0, behind = info.behind || 0;
    const originLabel = (info.web_url || info.origin_url || '').replace(/^https?:\/\//, '').replace(/\.git$/, '');
    const line = h('div', { class: 'meta-line' },
      h('span', { class: 'path code', title: t('open.folder'), role: 'link', tabindex: '0', onclick: () => doOpen('open_path', p.path), onkeydown: (e) => { if (e.key === 'Enter') doOpen('open_path', p.path); }, text: String(p.path) }),
      hasOrigin
        ? h('span', {}, 'origin: ', info.web_url
          ? h('button', { class: 'link', type: 'button', title: t('gh.open'), onclick: () => doOpen('open_path', info.web_url), text: originLabel })
          : originLabel, ` · ${info.current_branch && info.current_branch !== p.base_branch ? info.current_branch : p.base_branch} ↑${ahead} ↓${behind}`)
        : (info.error ? h('span', { class: 'err', text: info.error }) : h('span', { text: info.is_repo === false ? '' : t('origin.none') })),
      info.dirty ? h('span', { text: t('repo.dirty') }) : null,
      gh ? h('span', { text: gh.ghUser ? t('gh.as', { user: gh.ghUser }) : (gh.ghHint ? 'gh: ' + gh.ghHint : '') }) : null);
    const br = (gh && gh.branches) || [];
    const merged = br.filter((b) => b.merged).length;
    const top = h('div', { class: 'phead-top' }, h('h1', { text: p.name }),
      br.length ? h('button', { class: 'btn sm', type: 'button', 'aria-expanded': S.branchesOpen ? 'true' : 'false', onclick: () => { S.branchesOpen = !S.branchesOpen; renderMain(); }, text: t('gh.branches', { n: br.length, m: merged }) }) : null,
      h('button', { class: 'btn sm', type: 'button', disabled: !hasOrigin, onclick: () => act('push:' + p.id, async () => { await call('push_base', { projectId: p.id }); await loadProjectInfo(p.id); }, t('toast.pushed')), text: t('proj.push') }),
      h('button', { class: 'btn sm', type: 'button', disabled: !hasOrigin, onclick: () => act('pull:' + p.id, async () => { await call('pull_base', { projectId: p.id }); await loadProjectInfo(p.id); }, t('toast.pulled')), text: t('proj.pull') }),
      h('button', { class: 'btn sm', type: 'button', onclick: () => openProjectSettings(p.id), text: t('settings.project') }));
    const box = h('div', { class: 'phead' }, top, line);
    if (S.branchesOpen && br.length) {
      box.append(h('div', { class: 'branches' },
        h('ul', {}, br.map((b) => h('li', {}, h('span', { class: 'code', text: b.name }), ' ', h('span', { class: 'muted', text: b.merged ? t('gh.merged') : t('gh.unmerged') })))),
        merged ? h('div', {}, h('button', { class: 'btn sm', type: 'button', onclick: () => act('delbr', async () => { await call('delete_merged_branches', { projectId: p.id }); await loadProjectInfo(p.id); }, t('toast.branches')), text: t('gh.delmerged', { m: merged }) })) : null));
    }
    return box;
  }
  function initBanner(p) {
    return h('div', { class: 'banner warn', role: 'alert' },
      h('span', { class: 'grow', text: t('init.ask') }),
      h('button', { class: 'btn sm primary', type: 'button', onclick: () => act('init:' + p.id, async () => { await call('init_repo', { path: String(p.path) }); await loadProjectInfo(p.id); }, t('toast.inited')), text: t('init.do') }));
  }

  // ---- composer ----
  function composerBlock(p) {
    const wrap = h('div', { class: 'composer-wrap', 'data-pid': p.id, 'data-agentok': String(agentOk(p)), 'data-lang': lang() });
    if (!agentOk(p)) {
      const a = agent(p.agent);
      wrap.append(h('div', { class: 'card-note' },
        h('h2', { text: t('agent.install', { name: a ? a.name : p.agent }) }),
        h('div', { class: 'code', text: (a && a.hint) || '' }),
        h('div', { class: 'btns' },
          h('button', { class: 'btn', type: 'button', onclick: async () => { await act('agents', async () => { S.agents = await call('list_agents'); }); renderMain(); }, text: t('agent.recheck') }),
          h('button', { class: 'btn', type: 'button', onclick: () => openProjectSettings(p.id), text: t('agent.choose') }))));
      return wrap;
    }
    const ta = h('textarea', { id: 'composer-ta', rows: 3, placeholder: t('composer.ph'), 'aria-label': t('composer.label'), value: draftGet(p.id) });
    const sel = h('select', { id: 'composer-agent', 'aria-label': t('composer.engine') });
    fillAgentSelect(sel, p.agent);
    const err = h('div', { class: 'err', role: 'alert', hidden: true });
    const btn = h('button', { class: 'btn primary', type: 'button', disabled: !ta.value.trim(), onclick: submit, 'aria-keyshortcuts': 'Control+Enter Meta+Enter' },
      t('composer.send'), ' ', h('span', { class: 'kbd', text: MOD + '↵' }));
    ta.addEventListener('input', () => { btn.disabled = !ta.value.trim(); draftSet(p.id, ta.value); err.hidden = true; });
    ta.addEventListener('keydown', (e) => { if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') { e.preventDefault(); submit(); } });
    async function submit() {
      const text = ta.value.trim();
      if (!text) return;
      btn.disabled = true;
      const ok = await act('submit', async () => {
        await call('submit_task', { projectId: p.id, prompt: text, agent: sel.value === p.agent ? null : sel.value });
      });
      if (ok) { ta.value = ''; draftSet(p.id, ''); err.hidden = true; ta.focus(); }
      else { err.textContent = t('composer.fail'); err.hidden = false; }
      btn.disabled = !ta.value.trim();
    }
    const hasTasks = tasksArr().some((x) => x.project_id === p.id);
    wrap.append(h('div', { class: 'composer' }, ta,
      h('div', { class: 'composer-row' },
        h('span', { class: 'grow', text: hasTasks ? t('composer.hint.short') : t('composer.hint') }),
        sel, btn), err));
    return wrap;
  }
  function fillAgentSelect(sel, current, opts) {
    opts = opts || {};
    if (opts.none) sel.append(h('option', { value: '', text: opts.none }));
    for (const a of S.agents) {
      sel.append(h('option', { value: a.id, disabled: !a.available && a.id !== current, text: a.available ? a.name : `${a.name} — ${t('agent.missing')}` }));
    }
    sel.value = current == null ? '' : current;
  }

  // ================= task list =================
  function visibleTasks() {
    return tasksArr().filter((x) => NEED(x.status) || x.project_id === S.sel);
  }
  function renderList() {
    const host = $('groups');
    if (!host) return;
    const groups = L.groupTasks(visibleTasks(), Date.now());
    const wantIds = new Set();
    const hasTasks = groups.length > 0;
    const secs = [];
    for (const g of groups) {
      const rows = g.tasks.map((task) => { wantIds.add(task.id); return getRow(task); });
      secs.push(groupSection(g.id, rows));
    }
    for (const [id, c] of rowCache) if (!wantIds.has(id)) { c.el.remove(); rowCache.delete(id); }
    const p = project(S.sel);
    host.replaceChildren(...secs, ...((!hasTasks && p) ? [h('p', { class: 'hint', text: t('list.empty') })] : []));
    for (const [id, c] of rowCache) c.el.classList.toggle('sel', id === S.open);
    tick();
  }
  const secCache = {};
  function groupSection(gid, rows) {
    let sec = secCache[gid];
    if (!sec) {
      const body = h('div', { class: 'rows' });
      const count = h('span', { class: 'count' });
      const title = h('h2');
      if (gid === 'archive') {
        sec = h('details', { class: 'group archive' }, h('summary', { class: 'gh' }, title, count), body);
        sec.addEventListener('toggle', () => { S.showArchive = sec.open; });
      } else {
        sec = h('section', { class: 'group ' + gid, 'aria-label': gid }, h('div', { class: 'gh' }, title, count), body);
      }
      sec._body = body; sec._count = count; sec._title = title;
      secCache[gid] = sec;
    }
    sec._title.textContent = t('group.' + gid);
    sec._count.textContent = String(rows.length);
    if (gid === 'archive') sec.open = S.showArchive;
    // reconcile children in order without rebuilding unchanged rows
    const body = sec._body;
    rows.forEach((r, i) => { if (body.children[i] !== r) body.insertBefore(r, body.children[i] || null); });
    while (body.children.length > rows.length) body.lastElementChild.remove();
    return sec;
  }
  function rowSig(task) {
    const p = project(task.project_id);
    return JSON.stringify([task.status, task.title, task.question, task.attempts, task.review_rounds, task.resume_at, task.outcome, task.agent,
      p && p.finish, task.project_id === S.sel, lang(), task.finished_at, S.settings && S.settings.max_parallel]);
  }
  function getRow(task) {
    const sig = rowSig(task);
    const c = rowCache.get(task.id);
    if (c && c.sig === sig) return c.el;
    const el = buildRow(task);
    if (c) c.el.replaceWith(el);
    rowCache.set(task.id, { el, sig });
    return el;
  }
  function statusLabel(task) { return t('status.' + task.status); }
  function stBadge(task) {
    return h('span', { class: 'st ' + statusClass(task.status) }, h('span', { class: 'dot', 'aria-hidden': 'true' }), statusLabel(task));
  }
  const line = (detail, actions) => h('div', { class: 'row-line' }, detail, actions);
  function buildRow(task) {
    const p = project(task.project_id);
    const top = h('div', { class: 'row-top' }, stBadge(task),
      h('button', { class: 'row-title', type: 'button', title: task.title, onclick: (e) => { e.stopPropagation(); openCard(task.id); }, text: task.title }),
      (p && task.project_id !== S.sel) ? h('span', { class: 'ptag', text: p.name }) : null,
      liveSince(task) ? h('span', { class: 'tm', 'data-since': task.started_at, text: elapsedOf(task) }) : (task.finished_at ? h('span', { class: 'tm', title: fmtDate(task.finished_at), text: fmtTime(task.finished_at) }) : null));
    const row = h('div', { class: 'row', 'data-id': task.id, onclick: (e) => { if (!e.target.closest('button,input,a,select,textarea')) openCard(task.id); } }, top);
    const q = task.question;
    switch (task.status) {
      case 'needs_human':
        row.append(h('div', { class: 'row-detail q', text: q ? q.text : '' }), answerControls(task));
        break;
      case 'awaiting_outward':
        row.append(h('div', { class: 'row-detail q', text: q ? q.text : t('outward.q') }), outwardControls(task, p));
        break;
      case 'queued':
        row.append(line(h('div', { class: 'row-detail', text: t('detail.queued', { n: S.settings ? S.settings.max_parallel : 2 }) }), rowButtons(task, p)));
        break;
      case 'paused_limit':
        row.append(line(h('div', { class: 'row-detail', text: task.resume_at ? t('detail.limit', { time: fmtTime(task.resume_at) }) : t('detail.limit0') }), rowButtons(task, p)));
        break;
      case 'merge_blocked':
        row.append(line(h('div', { class: 'row-detail', text: t('detail.blocked') }), rowButtons(task, p)));
        break;
      case 'done': case 'done_not_pushed': case 'reverted':
        row.append(line(h('div', { class: 'row-detail', text: outcomeParts(task).join(' \u00b7 ') }), rowButtons(task, p)));
        if (task.status === 'done_not_pushed') row.append(h('div', { class: 'row-detail bad', text: errHuman(task.outcome.error) || t('detail.notpushed') }));
        break;
      case 'failed': case 'cancelled':
        row.append(line(h('div', { class: 'row-detail' + (task.outcome && task.outcome.error ? ' bad' : ''), text: errHuman(task.outcome && task.outcome.error) || '' }), rowButtons(task, p)));
        break;
      default: {
        const stage = t('stage.' + task.status);
        const extra = [];
        if (task.attempts) extra.push(t('detail.attempt', { n: task.attempts + 1 }));
        if (task.review_rounds) extra.push(t('detail.round', { n: task.review_rounds + 1 }));
        row.append(line(h('div', { class: 'row-detail', text: [stage, ...extra, agentName(task.agent)].join(' \u00b7 ') }), rowButtons(task, p)));
      }
    }
    return row;
  }
  function confirmButton(label, confirmLabel, fn, cls) {
    let armed = false, timer = 0;
    const b = h('button', { class: 'btn sm ' + (cls || ''), type: 'button', text: label });
    b.addEventListener('click', (e) => {
      e.stopPropagation();
      if (!armed) {
        armed = true; b.textContent = confirmLabel;
        timer = setTimeout(() => { armed = false; b.textContent = label; }, 3500);
        return;
      }
      clearTimeout(timer); armed = false; b.textContent = label; fn();
    });
    b.addEventListener('blur', () => { clearTimeout(timer); armed = false; b.textContent = label; });
    return b;
  }
  function rowButtons(task, p) {
    const btns = [];
    const id = task.id;
    const go = (cmd, extra) => act(cmd + id, () => call(cmd, Object.assign({ id }, extra || {})));
    switch (task.status) {
      case 'failed':
        btns.push(h('button', { class: 'btn sm', type: 'button', onclick: () => go('retry'), text: t('act.retry') }));
        btns.push(confirmButton(t('act.remove'), t('act.confirm'), () => removeTask(id), 'danger'));
        break;
      case 'done':
        btns.push(confirmButton(t('act.undo'), t('act.confirm'), () => go('undo')));
        break;
      case 'done_not_pushed':
        btns.push(h('button', { class: 'btn sm', type: 'button', onclick: () => act('push' + id, () => call('push_base', { projectId: task.project_id }), t('toast.pushed')), text: t('act.retrypush') }));
        btns.push(confirmButton(t('act.undo'), t('act.confirm'), () => go('undo')));
        break;
      case 'cancelled': case 'reverted':
        btns.push(confirmButton(t('act.remove'), t('act.confirm'), () => removeTask(id), 'danger'));
        break;
      default:
        btns.push(confirmButton(t('act.cancel'), t('act.confirm'), () => go('cancel')));
    }
    if (task.outcome && task.outcome.pr_url) btns.push(h('button', { class: 'link', type: 'button', onclick: () => doOpen('open_path', task.outcome.pr_url), text: t('act.pr') }));
    return h('div', { class: 'row-actions' }, btns);
  }
  function answerControls(task) {
    const q = task.question || { options: [] };
    const input = h('input', { type: 'text', 'aria-label': t('answer.label'), placeholder: t('answer.ph'), value: S.answerDraft[task.id] || '' });
    input.addEventListener('input', () => { S.answerDraft[task.id] = input.value; });
    const send = (text) => {
      if (!text.trim()) return;
      return act('answer' + task.id, async () => { await call('answer', { id: task.id, text }); delete S.answerDraft[task.id]; });
    };
    input.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); send(input.value); } });
    const okBtn = h('button', { class: 'btn sm primary', type: 'button', text: t('answer.send'), onclick: () => send(input.value) });
    return h('div', { class: 'row-actions' },
      (q.options || []).map((o) => h('button', { class: 'btn sm', type: 'button', text: o, onclick: () => send(o) })), input, okBtn);
  }
  function outwardControls(task, p) {
    const mode = p ? p.finish : 'push';
    const yes = h('button', { class: 'btn sm primary', type: 'button', text: mode === 'pr' ? t('outward.pr') : t('outward.push'), onclick: () => act('out' + task.id, () => call('approve_outward', { id: task.id, yes: true })) });
    const no = h('button', { class: 'btn sm', type: 'button', text: t('outward.no'), onclick: () => act('out' + task.id, () => call('approve_outward', { id: task.id, yes: false })) });
    return h('div', { class: 'row-actions' }, yes, no);
  }
  async function removeTask(id) {
    const ok = await act('rm' + id, () => call('remove_task', { id }));
    if (!ok) return;
    dropTask(id);
  }
  function dropTask(id) {
    S.tasks.delete(id); S.logs.delete(id); delete S.diff[id]; delete S.answerDraft[id];
    if (S.open === id) closeCard();
    renderSide(); renderList();
  }
  async function doOpen(cmd, path) { await act(null, () => call(cmd, { path: String(path) })); }

  // live timers without re-render
  function tick() {
    for (const el of document.querySelectorAll('[data-since]')) {
      el.textContent = L.formatElapsed(Date.now() - Date.parse(el.dataset.since));
    }
  }
  setInterval(tick, 1000);
  // re-group once a minute (done -> archive after 7 days)
  setInterval(() => renderList(), 60000);

  // ================= task card =================
  const CARD = { loading: null, top: null, logBox: null, bottom: null, logList: null, id: null, lastFocus: null };
  async function openCard(id) {
    const task = S.tasks.get(id);
    if (!task) return;
    CARD.lastFocus = document.activeElement;
    S.open = id; CARD.id = id;
    const card = $('card');
    CARD.top = h('div', { class: 'card-top' });
    CARD.bottom = h('div', { class: 'card-bottom' });
    CARD.logList = h('div', { class: 'log', role: 'log', 'aria-label': t('card.log'), tabindex: '0' });
    CARD.logBox = h('div', { class: 'sec' }, h('h3', { text: t('card.log') }), CARD.logList);
    card.replaceChildren(h('div', { class: 'card-scroll' }, CARD.top, CARD.logBox, CARD.bottom));
    card.hidden = false; document.body.classList.add('card-open');
    renderCardParts();
    for (const [rid, c] of rowCache) c.el.classList.toggle('sel', rid === id);
    const closeBtn = card.querySelector('[data-close]');
    if (closeBtn) closeBtn.focus();
    if (task.outcome && task.outcome.pr_url) {
      call('pr_state', { taskId: id }).then((s) => { S.prState[id] = s; if (CARD.id === id) renderCardParts(); }, () => { S.prState[id] = null; });
    }
    // get_diffstat may not exist yet in the app: rejection just means "show totals only"
    call('get_diffstat', { taskId: id }).then((d) => { S.diff[id] = Array.isArray(d) ? d : null; if (CARD.id === id) renderCardParts(); }, () => { S.diff[id] = null; });
    // Events that arrive while get_log is in flight are buffered, then merged after the snapshot
    // (events already present in the snapshot are dropped by count, so nothing is lost or doubled).
    const load = { id, pending: [] };
    CARD.loading = load;
    CARD.logList.textContent = t('card.loading');
    let evs;
    try { evs = await call('get_log', { id, limit: 500 }); } catch (e) { evs = [{ at: new Date().toISOString(), kind: 'error', text: errText(e) }]; }
    if (CARD.loading !== load) return; // card was closed or another one opened meanwhile
    CARD.loading = null;
    const seen = new Map();
    for (const ev of evs) { const k = logKey(ev); seen.set(k, (seen.get(k) || 0) + 1); }
    const merged = evs.slice();
    for (const ev of load.pending) {
      const k = logKey(ev), n = seen.get(k) || 0;
      if (n > 0) seen.set(k, n - 1); else merged.push(ev);
    }
    const tail = merged.slice(-500);
    S.logs.set(id, tail);
    CARD.logList.replaceChildren(...tail.map(logLine));
    CARD.logList.scrollTop = CARD.logList.scrollHeight;
    renderCardParts();
  }
  const logKey = (ev) => `${ev.at}|${ev.kind}|${ev.text}`;
  function closeCard() {
    if (!S.open) return;
    const prev = CARD.lastFocus;
    S.open = null; CARD.id = null; CARD.loading = null;
    $('card').hidden = true; document.body.classList.remove('card-open');
    for (const c of rowCache.values()) c.el.classList.remove('sel');
    if (prev && document.contains(prev)) prev.focus();
  }
  const KIND = { agent: 'kind.agent', tool: 'kind.tool', check: 'kind.check', review: 'kind.review', git: 'kind.git', error: 'kind.error', human: 'kind.human', stage: 'kind.stage' };
  function logLine(ev) {
    const text = ev.kind === 'stage' && window.NSI18n.has('stage.' + ev.text) ? t('stage.' + ev.text) : ev.text;
    return h('div', { class: 'ln' }, h('span', { class: 'at', text: fmtTime(ev.at, true) }),
      h('span', { class: 'k ' + ev.kind, text: t(KIND[ev.kind] || 'kind.agent') }), h('span', { class: 'tx', text }));
  }
  function appendLog(id, ev) {
    if (CARD.id !== id || !CARD.logList) return; // closed cards reload the log from get_log on open
    if (CARD.loading && CARD.loading.id === id) { CARD.loading.pending.push(ev); return; }
    const arr = S.logs.get(id) || [];
    arr.push(ev); if (arr.length > 500) arr.shift();
    S.logs.set(id, arr);
    const box = CARD.logList;
    const atBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 24;
    box.append(logLine(ev));
    while (box.children.length > 500) box.firstElementChild.remove();
    if (atBottom) box.scrollTop = box.scrollHeight;
    if (ev.kind === 'stage') renderCardParts();
  }
  function renderCardParts() {
    const task = S.tasks.get(CARD.id);
    if (!task || !CARD.top) return;
    const p = project(task.project_id);
    const o = task.outcome || {};
    const top = [];
    top.push(h('div', { class: 'card-head' },
      h('h2', { text: task.title }),
      h('button', { class: 'btn sm', type: 'button', 'data-close': '', onclick: closeCard, 'aria-label': t('card.close'), 'aria-keyshortcuts': 'Escape', text: t('card.close') })));
    top.push(h('div', {}, stBadge(task)));
    // question
    if (task.status === 'needs_human') top.push(h('div', { class: 'qbox' }, h('div', { text: task.question ? task.question.text : '' }), answerControls(task)));
    if (task.status === 'awaiting_outward') top.push(h('div', { class: 'qbox' }, h('div', { text: task.question ? task.question.text : t('outward.q') }), outwardControls(task, p)));
    if (task.status === 'paused_limit') top.push(h('div', { class: 'row-detail', text: task.resume_at ? t('detail.limit', { time: fmtTime(task.resume_at) }) : t('detail.limit0') }));
    if (task.status === 'merge_blocked') top.push(h('div', { class: 'row-detail', text: t('detail.blocked') }));
    if ((task.status === 'failed' || task.status === 'done_not_pushed') && o.error) top.push(h('div', { class: 'row-detail bad', role: 'alert', text: errHuman(o.error) }));
    // meta
    const dl = h('dl', { class: 'kv' });
    const kv = (k, v) => { if (v != null && v !== '') dl.append(h('dt', { text: k }), h('dd', {}, v)); };
    kv(t('card.project'), p ? p.name : task.project_id);
    kv(t('card.branch'), h('span', { class: 'code', text: task.branch }));
    kv(t('card.engine'), agentName(task.agent));
    kv(t('card.started'), fmtDate(task.started_at));
    if (task.started_at) kv(t('card.time'), elapsedOf(task));
    kv(t('card.attempts'), `${task.attempts} · ${t('card.rounds')}: ${task.review_rounds}`);
    if (o.commit) kv(t('card.commit'), h('span', { class: 'code', text: String(o.commit).slice(0, 7) }));
    if (o.pr_url) {
      const st = S.prState[task.id];
      kv(t('card.pr'), h('span', {}, h('button', { class: 'link', type: 'button', onclick: () => doOpen('open_path', o.pr_url), text: o.pr_url.replace(/^https?:\/\//, '') }), st ? ` (${t('pr.' + st)})` : ''));
    }
    top.push(dl);
    if (o.summary || ['done', 'done_not_pushed', 'reverted'].includes(task.status)) top.push(h('div', { class: 'sec' }, h('h3', { text: t('card.result') }), h('div', { text: outcomeParts(task).join(' · ') })));
    top.push(h('div', { class: 'btns' }, cardButtons(task, p)));
    // stages
    const stages = (S.logs.get(task.id) || []).filter((e) => e.kind === 'stage');
    if (stages.length) {
      top.push(h('div', { class: 'sec' }, h('h3', { text: t('card.stages') }),
        h('ol', { class: 'stages' }, stages.map((e, i) => {
          const next = stages[i + 1];
          const dur = next ? L.formatElapsed(Date.parse(next.at) - Date.parse(e.at)) : (liveSince(task) ? L.formatElapsed(Date.now() - Date.parse(e.at)) : '');
          return h('li', { class: i === stages.length - 1 && liveSince(task) ? 'cur' : '' },
            h('span', { class: 'when', text: fmtTime(e.at, true) }),
            h('span', { text: window.NSI18n.has('stage.' + e.text) ? t('stage.' + e.text) : e.text }),
            dur ? h('span', { class: 'muted', text: dur }) : null);
        }))));
    }
    CARD.top.replaceChildren(...top);

    // bottom: changes, tests, review
    const bot = [];
    const files = S.diff[task.id];
    if (files && files.length) bot.push(h('div', { class: 'sec diffstat' }, h('h3', { text: t('card.changes') }),
      h('div', {}, `${files.length} ${pl(files.length, 'u.files')} `, h('span', { class: 'ins', text: '+' + files.reduce((n, f) => n + f.insertions, 0) }), ' ',
        h('span', { class: 'del', text: '\u2212' + files.reduce((n, f) => n + f.deletions, 0) })),
      h('ul', { class: 'files' }, files.map((f) => h('li', {}, h('span', { class: 'fp code', text: f.path }), h('span', { class: 'ins', text: '+' + f.insertions }), h('span', { class: 'del', text: '\u2212' + f.deletions }))))));
    else if (o.files_changed) bot.push(h('div', { class: 'sec diffstat' }, h('h3', { text: t('card.changes') }),
      h('div', {}, `${o.files_changed} ${pl(o.files_changed, 'u.files')} `, h('span', { class: 'ins', text: '+' + o.insertions }), ' ', h('span', { class: 'del', text: '−' + o.deletions }))));
    if (o.tests) bot.push(h('div', { class: 'sec' }, h('h3', { text: t('card.tests') }),
      h('div', { class: o.tests.passed ? 'pass' : 'fail', text: o.tests.passed ? t('tests.ok') : t('tests.red') }),
      o.tests.tail ? h('pre', { class: 'tail', tabindex: '0', text: o.tests.tail }) : null));
    else if (['done', 'done_not_pushed', 'reverted', 'merging', 'reviewing'].includes(task.status)) bot.push(h('div', { class: 'sec' }, h('h3', { text: t('card.tests') }), h('div', { class: 'row-detail', text: t('tests.nonehint') })));
    if (o.review) bot.push(h('div', { class: 'sec' }, h('h3', { text: t('card.review') }),
      h('div', { class: o.review.approved ? 'pass' : 'fail', text: o.review.approved ? t('review.ok') : t('review.changes') }),
      o.review.issues && o.review.issues.length ? h('ul', { class: 'plain' }, o.review.issues.map((i) => h('li', { text: i }))) : null));
    CARD.bottom.replaceChildren(...bot);
  }
  function cardButtons(task, p) {
    const id = task.id;
    const go = (cmd, extra) => act(cmd + id, () => call(cmd, Object.assign({ id }, extra || {})));
    const b = [];
    const dir = task.worktree || (p && p.path);
    switch (task.status) {
      case 'failed': b.push(h('button', { class: 'btn', type: 'button', onclick: () => go('retry'), text: t('act.retry') })); break;
      case 'done': b.push(confirmButton(t('act.undo'), t('act.confirm'), () => go('undo'))); break;
      case 'done_not_pushed':
        b.push(h('button', { class: 'btn', type: 'button', onclick: () => act('push' + id, () => call('push_base', { projectId: task.project_id }), t('toast.pushed')), text: t('act.retrypush') }));
        b.push(confirmButton(t('act.undo'), t('act.confirm'), () => go('undo'))); break;
      case 'queued': case 'preparing': case 'working': case 'checking': case 'reviewing': case 'merging':
      case 'needs_human': case 'awaiting_outward': case 'paused_limit': case 'merge_blocked':
        b.push(confirmButton(t('act.cancel'), t('act.confirm'), () => go('cancel'))); break;
      default: break;
    }
    if (dir) {
      b.push(h('button', { class: 'btn', type: 'button', onclick: () => doOpen('open_path', dir), text: t('open.folder') }));
      b.push(h('button', { class: 'btn', type: 'button', onclick: () => doOpen('open_editor', dir), text: t('open.editor') }));
      b.push(h('button', { class: 'btn', type: 'button', onclick: () => doOpen('open_terminal', dir), text: t('open.terminal') }));
    }
    if (['failed', 'cancelled', 'reverted'].includes(task.status)) b.push(confirmButton(t('act.remove'), t('act.confirm'), () => removeTask(id), 'danger'));
    return b;
  }

  // ================= dialogs =================
  function showDialog(title, body, foot) {
    const dlg = $('dlg');
    const closeBtn = h('button', { class: 'btn sm', type: 'button', onclick: () => dlg.close(), text: t('dlg.close') });
    dlg.replaceChildren(h('div', { class: 'dlg' }, h('div', { class: 'dlg-head' }, h('h2', { text: title }), closeBtn),
      h('div', { class: 'dlg-body' }, body), foot ? h('div', { class: 'dlg-foot' }, foot) : null));
    if (!dlg.open) dlg.showModal();
    return dlg;
  }
  const fld = (label, control, hint) => h('label', { class: 'fld' }, h('span', { class: 'lbl', text: label }), control, hint ? h('small', { text: hint }) : null);

  // ---- add project ----
  async function pickFolder() {
    const dialogApi = TAURI && TAURI.dialog;
    if (dialogApi && dialogApi.open) {
      try {
        const path = await dialogApi.open({ directory: true, multiple: false });
        if (path) return addProjectByPath(Array.isArray(path) ? path[0] : path);
        return;
      } catch (e) { toast(errText(e), true); }
    }
    openAddDialog();
  }
  function openAddDialog(prefill, askInit) {
    const input = h('input', { type: 'text', id: 'add-path', placeholder: '/path/to/project', value: prefill || '', 'aria-label': t('add.path') });
    const msg = h('div', { class: 'err', role: 'alert', hidden: !askInit });
    const initBtn = h('button', { class: 'btn primary', type: 'button', hidden: !askInit, text: t('init.do'), onclick: async () => {
      const ok = await act('initnew', async () => { await call('init_repo', { path: input.value.trim() }); });
      if (ok) addProjectByPath(input.value.trim(), true);
    } });
    if (askInit) msg.textContent = t('init.ask');
    const addBtn = h('button', { class: 'btn primary', type: 'button', text: t('proj.add'), onclick: () => addProjectByPath(input.value.trim(), false, { msg, initBtn, addBtn }) });
    input.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); addBtn.click(); } });
    const pick = (TAURI && TAURI.dialog && TAURI.dialog.open) ? h('button', { class: 'btn', type: 'button', onclick: () => { $('dlg').close(); pickFolder(); }, text: t('proj.pick') }) : null;
    showDialog(t('proj.add'), [fld(t('add.path'), input, t('add.hint')), pick, msg, initBtn ? h('div', { class: 'btns' }, initBtn) : null], [addBtn]);
    input.focus();
  }
  function isNotGit(e) { return errText(e).includes('not a git repo'); }
  async function addProjectByPath(path, afterInit, ui) {
    if (!path) return;
    try {
      const p = await call('add_project', { path });
      $('dlg').open && $('dlg').close();
      S.projects = await call('list_projects');
      S.sel = (p && p.id) || (S.projects[S.projects.length - 1] || {}).id;
      renderSide(); renderMain(); loadProjectInfo(S.sel);
      const a = agent(project(S.sel) && project(S.sel).agent);
      if (a && !a.available) toast(t('agent.install', { name: a.name }), true);
    } catch (e) {
      if (!afterInit && isNotGit(e)) { openAddDialog(path, true); return; }
      toast(errHuman(errText(e)), true);
    }
  }
  // folder drop (Tauri delivers real paths through its own event)
  function onDropPaths(paths) {
    document.body.classList.remove('dragging');
    if (paths && paths[0]) addProjectByPath(paths[0]);
  }
  window.addEventListener('dragover', (e) => { e.preventDefault(); document.body.classList.add('dragging'); });
  window.addEventListener('dragleave', (e) => { if (!e.relatedTarget) document.body.classList.remove('dragging'); });
  window.addEventListener('drop', (e) => {
    e.preventDefault(); document.body.classList.remove('dragging');
    const f = e.dataTransfer && e.dataTransfer.files && e.dataTransfer.files[0];
    if (f && f.path) addProjectByPath(f.path); // plain browsers expose no path: Tauri event handles it
  });

  // ---- project settings ----
  function openProjectSettings(pid) {
    const p = project(pid); if (!p) return;
    const name = h('input', { type: 'text', value: p.name });
    const check = h('input', { type: 'text', value: p.check_cmd || '', placeholder: t('ps.check.ph') });
    const setup = h('input', { type: 'text', value: p.setup_cmd || '', placeholder: t('ps.setup.ph') });
    const files = h('textarea', { rows: 3, value: (p.copy_files || []).join('\n') });
    const radios = ['local', 'push', 'pr'].map((v) => h('label', {}, h('input', { type: 'radio', name: 'finish', value: v, checked: p.finish === v }), h('span', {}, h('strong', { text: t('finish.' + v) }), ' — ', t('finish.' + v + '.d'))));
    const ask = h('input', { type: 'checkbox', checked: !!p.ask_outward });
    const ag = h('select'); fillAgentSelect(ag, p.agent);
    const rv = h('select'); fillAgentSelect(rv, p.reviewer || '', { none: t('ps.reviewer.same') });
    const fb = h('select'); fillAgentSelect(fb, p.fallback_agent || '', { none: t('ps.fallback.none') });
    const base = h('input', { type: 'text', value: p.base_branch, disabled: true });
    const save = h('button', { class: 'btn primary', type: 'button', text: t('dlg.save'), onclick: async () => {
      const upd = Object.assign({}, p, {
        name: name.value.trim() || p.name, check_cmd: check.value.trim() || null, setup_cmd: setup.value.trim() || null,
        copy_files: files.value.split('\n').map((s) => s.trim()).filter(Boolean),
        finish: (radios.map((r) => r.querySelector('input')).find((i) => i.checked) || {}).value || 'local',
        ask_outward: ask.checked, agent: ag.value, reviewer: rv.value || null, fallback_agent: fb.value || null,
      });
      const ok = await act('psave', async () => { await call('update_project', { project: upd }); S.projects = await call('list_projects'); });
      if (ok) { $('dlg').close(); renderSide(); renderMain(); toast(t('toast.saved')); }
    } });
    const remove = confirmButton(t('ps.remove'), t('act.confirm'), async () => {
      const ok = await act('prm', async () => { await call('remove_project', { id: p.id }); S.projects = await call('list_projects'); });
      if (ok) { $('dlg').close(); S.sel = S.projects[0] && S.projects[0].id; renderSide(); renderMain(); loadProjectInfo(S.sel); }
    }, 'danger');
    remove.classList.remove('sm');
    showDialog(`${t('settings.project')}: ${p.name}`, [
      fld(t('ps.name'), name),
      h('div', { class: 'two' }, fld(t('ps.check'), check, t('ps.check.d')), fld(t('ps.setup'), setup, t('ps.setup.d'))),
      fld(t('ps.copy'), files, t('ps.copy.d')),
      h('fieldset', { class: 'fld bare' }, h('legend', { class: 'lbl', text: t('ps.finish') }), h('div', { class: 'radios' }, radios)),
      h('label', { class: 'chk' }, ask, h('span', {}, h('strong', { text: t('ps.ask') }), ' — ', t('ps.ask.d'))),
      h('hr', { class: 'sep' }),
      h('div', { class: 'two' }, fld(t('ps.agent'), ag), fld(t('ps.reviewer'), rv, t('ps.reviewer.d'))),
      fld(t('ps.fallback'), fb, t('ps.fallback.d')),
      fld(t('ps.base'), base),
    ], [remove, save]);
  }

  // ---- general settings ----
  function openGeneralSettings() {
    const s = JSON.parse(JSON.stringify(S.settings));
    const par = h('input', { type: 'number', min: 1, max: 16, value: s.max_parallel });
    const lg = h('select', {}, ['auto', 'ru', 'en'].map((v) => h('option', { value: v, selected: s.language === v, text: t('lang.' + v) })));
    const hot = h('input', { type: 'text', value: s.hotkey });
    const list = h('div', { class: 'agents' });
    const rows = [];
    function addRow(a) {
      const id = h('input', { type: 'text', value: a.id, placeholder: 'my-agent' });
      const nm = h('input', { type: 'text', value: a.name, placeholder: t('ca.name') });
      const run = h('input', { type: 'text', value: a.run, placeholder: 'my-cli --prompt {prompt}' });
      const res = h('input', { type: 'text', value: a.resume || '', placeholder: 'my-cli --resume {session} {prompt}' });
      const box = h('div', { class: 'agentbox' },
        h('div', { class: 'two' }, fld('id', id), fld(t('ca.name'), nm)),
        fld(t('ca.run'), run, t('ca.tpl')), fld(t('ca.resume'), res),
        h('div', { class: 'row-detail bad', text: t('ca.warn') }),
        h('div', {}, h('button', { class: 'btn sm danger', type: 'button', text: t('ca.remove'), onclick: () => { box.remove(); rows.splice(rows.findIndex((r) => r.box === box), 1); } })));
      rows.push({ box, id, nm, run, res }); list.append(box);
    }
    (s.custom_agents || []).forEach(addRow);
    const save = h('button', { class: 'btn primary', type: 'button', text: t('dlg.save'), onclick: async () => {
      s.max_parallel = Math.max(1, parseInt(par.value, 10) || 2);
      s.language = lg.value; s.hotkey = hot.value.trim() || s.hotkey;
      s.custom_agents = rows.filter((r) => r.id.value.trim() && r.run.value.trim()).map((r) => ({ id: r.id.value.trim(), name: r.nm.value.trim() || r.id.value.trim(), run: r.run.value.trim(), resume: r.res.value.trim() || null }));
      const ok = await act('gsave', async () => { await call('set_settings', { settings: s }); S.settings = s; S.agents = await call('list_agents'); });
      if (ok) { window.NSI18n.setLang(s.language); $('dlg').close(); applyLang(); toast(t('toast.saved')); }
    } });
    showDialog(t('settings.general'), [
      h('div', { class: 'two' }, fld(t('gs.parallel'), par, t('gs.parallel.d')), fld(t('gs.lang'), lg)),
      fld(t('gs.hotkey'), hot, t('gs.hotkey.d')),
      h('hr', { class: 'sep' }),
      h('h3', { text: t('ca.title') }), h('p', { class: 'hint tight', text: t('ca.d') }),
      list,
      h('div', {}, h('button', { class: 'btn', type: 'button', text: '+ ' + t('ca.add'), onclick: () => addRow({ id: '', name: '', run: '', resume: '' }) })),
    ], [save]);
  }
  function applyLang() {
    document.documentElement.lang = lang();
    for (const c of rowCache.values()) c.el.remove();
    rowCache.clear();
    renderSide(); renderMain();
    if (S.open) { const id = S.open; S.open = null; openCard(id); }
  }

  // ================= events =================
  function onTaskChanged(task) {
    const prev = S.tasks.get(task.id);
    S.tasks.set(task.id, task);
    renderSide(); renderList();
    if (S.open === task.id) renderCardParts();
    if (prev && prev.status !== task.status && S.sel === task.project_id && ['done', 'done_not_pushed', 'merging'].includes(task.status)) loadProjectInfo(task.project_id);
  }
  async function listen(name, fn) {
    if (!TAURI || !TAURI.event) return;
    try { await TAURI.event.listen(name, (e) => fn(e.payload)); } catch (e) { toast(errText(e), true); }
  }

  document.addEventListener('keydown', (e) => {
    const typing = /^(INPUT|TEXTAREA|SELECT)$/.test((e.target.tagName || '')) || e.target.isContentEditable;
    if (e.key === 'Escape' && !$('dlg').open && S.open) { e.preventDefault(); closeCard(); }
    else if (e.key === '/' && !typing && !e.metaKey && !e.ctrlKey && !e.altKey && !$('dlg').open) {
      const ta = $('composer-ta'); if (ta) { e.preventDefault(); ta.focus(); }
    }
  });

  // ================= boot =================
  async function boot() {
    try { await loadAll(); }
    catch (e) { $('main').replaceChildren(h('div', { class: 'main-inner' }, h('p', { class: 'err', text: errText(e) }))); return; }
    document.documentElement.lang = lang();
    renderSide(); renderMain();
    loadProjectInfo(S.sel);
    listen('task-changed', onTaskChanged);
    listen('task-log', (pl_) => appendLog(pl_.id, pl_.event));
    listen('task-removed', (id) => dropTask(id));
    listen('project-changed', async (id) => {
      try { S.projects = await call('list_projects'); } catch (e) { toast(errText(e), true); }
      renderSide(); if (id === S.sel) loadProjectInfo(id); else renderMain();
    });
    listen('tauri://drag-enter', () => document.body.classList.add('dragging'));
    listen('tauri://drag-leave', () => document.body.classList.remove('dragging'));
    listen('tauri://drag-drop', (pl_) => onDropPaths(pl_ && pl_.paths));
    window.NS = { openCard, openProjectSettings, openGeneralSettings, selectProject, openAddDialog, S };
    window.dispatchEvent(new Event('ns-ready'));
  }
  boot();
})();
