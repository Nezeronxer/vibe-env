// Pure helpers for the UI: statuses, chat grouping, counters, limits, model versions, task steps, feed paging, send rules.
// No DOM, no Tauri, no i18n import: anything human-readable takes a `t(key, params)` or `lang` argument, so it runs in selftest.html.
(function (root) {
  const MIN = 60000, HOUR = 3600000, DAY = 24 * HOUR;
  const MAX_TASK_CHARS = 100000;   // longer text is not sent (G05)
  const WARN_AT = 80;              // % used: bar gets louder (G26.3)
  const STALE_MS = 6 * HOUR;       // limit data older than this is grey (G26.5)
  const ARCHIVE_MS = 7 * DAY;      // chat without movement goes to the archive (G20.11)
  const ts = (v) => (v ? Date.parse(v) : 0);
  const norm = (s) => String(s == null ? '' : s).toLowerCase().replace(/_/g, '');

  // serde enums arrive as "name" or {"name": payload}; accept snake_case and PascalCase alike.
  function variant(v) {
    if (v == null) return ['', null];
    if (typeof v === 'string') return [norm(v), null];
    const k = Object.keys(v)[0];
    return k == null ? ['', null] : [norm(k), v[k]];
  }

  // ---------- statuses ----------
  // kind order = priority of a chat's line: waits for you > failed > working > queued > done > plain talk
  const KINDS = ['wait', 'fail', 'run', 'queue', 'done', 'talk'];
  const STATUS = {
    queued:           { kind: 'queue' },
    preparing:        { kind: 'run', live: true },
    working:          { kind: 'run', live: true },
    checking:         { kind: 'run', live: true },
    reviewing:        { kind: 'run', live: true },
    merging:          { kind: 'run', live: true },
    needs_human:      { kind: 'wait', live: true },
    awaiting_outward: { kind: 'wait', live: true },
    paused_limit:     { kind: 'run', label: 'st.paused_limit' },
    merge_blocked:    { kind: 'run', label: 'st.merge_blocked' },
    handed_off:       { kind: 'run', label: 'st.handed_off' },
    done:             { kind: 'done' },
    done_not_pushed:  { kind: 'done', label: 'st.done_not_pushed', warn: true },
    failed:           { kind: 'fail' },
    cancelled:        { kind: 'done', label: 'st.cancelled', icon: 'dash' },
    reverted:         { kind: 'done', label: 'st.reverted', icon: 'dash' },
  };
  const KIND_LABEL = { wait: 'st.wait', fail: 'st.fail', run: 'st.run', queue: 'st.queue', done: 'st.done', talk: 'st.talk' };
  const KIND_ICON = { wait: 'ask', fail: 'fail', run: 'run', queue: 'dash', done: 'check', talk: 'chat' };
  const info = (status) => STATUS[status] || STATUS.working;
  const kindOf = (task) => info(task.status).kind;
  const labelKey = (task) => info(task.status).label || KIND_LABEL[kindOf(task)];
  const iconOf = (task) => info(task.status).icon || KIND_ICON[kindOf(task)];

  // the one task that decides a chat's line: best kind; among waiting - the oldest, otherwise the freshest
  function chatState(tasks) {
    let best = null;
    for (const t of tasks) {
      const k = kindOf(t);
      if (!best) { best = { kind: k, task: t }; continue; }
      const d = KINDS.indexOf(k) - KINDS.indexOf(best.kind);
      if (d < 0) best = { kind: k, task: t };
      else if (d === 0) {
        const older = ts(t.created_at) < ts(best.task.created_at);
        if (k === 'wait' ? older : ts(t.created_at) > ts(best.task.created_at)) best = { kind: k, task: t };
      }
    }
    return best || { kind: 'talk', task: null };
  }

  function chatArchived(chat, tasks, now) {
    if (chat.archived) return true;
    if (tasks.some((t) => ['wait', 'fail', 'run', 'queue'].includes(kindOf(t)))) return false;
    return now - ts(chat.updated_at || chat.created_at) > ARCHIVE_MS;
  }

  // chats of one env -> groups by project (most urgent first), archive apart. Empty groups never appear.
  function groupChats(chats, tasks, projects, now) {
    const byChat = new Map();
    for (const t of tasks) if (t.chat_id) (byChat.get(t.chat_id) || byChat.set(t.chat_id, []).get(t.chat_id)).push(t);
    const name = (id) => (projects.find((p) => p.id === id) || {}).name || id;
    const groups = new Map(), archive = [];
    for (const c of chats) {
      const mine = byChat.get(c.id) || [];
      const item = Object.assign({ chat: c, tasks: mine }, chatState(mine));
      if (chatArchived(c, mine, now)) { archive.push(item); continue; }
      (groups.get(c.project_id) || groups.set(c.project_id, []).get(c.project_id)).push(item);
    }
    const rank = (i) => KINDS.indexOf(i.kind);
    const byUrgency = (a, b) => rank(a) - rank(b) || ts(b.chat.updated_at) - ts(a.chat.updated_at);
    const out = [...groups].map(([projectId, items]) => ({ projectId, name: name(projectId), items: items.sort(byUrgency) }));
    out.sort((a, b) => Math.min(...a.items.map(rank)) - Math.min(...b.items.map(rank)) || a.name.localeCompare(b.name));
    return { groups: out, archive: archive.sort(byUrgency) };
  }

  // badge text for the call counter: 30 calls -> "9+" on the icon (G15.4); nothing when zero
  const badgeText = (n) => (n > 9 ? '9+' : n > 0 ? String(n) : '');
  // a call is a question, a permission or a failure; snoozed failures ("Отложить") leave the count (G15.2)
  function envCounts(tasks, envId, snoozed) {
    let calls = 0, running = 0;
    for (const t of tasks) {
      if (t.agent !== envId) continue;
      const k = kindOf(t);
      if (k === 'wait' || (k === 'fail' && !(snoozed && snoozed.has(t.id)))) calls++;
      else if (k === 'run' && info(t.status).live) running++;
    }
    return { calls, running };
  }

  // RoleKind (any case) -> role icon/colour key; null for roles without a mark
  const ROLE_KEY = { planner: 'plan', critic: 'crit', testauthor: 'test', worker: 'exec', reviewer: 'rev', verifier: 'gate', integrator: 'integ',
    coordinator: 'coord', watcher: 'watch', council: 'coord' };
  const roleKey = (role) => ROLE_KEY[norm(role)] || null;

  // ---------- tasks: steps, actions, push ----------
  const BAR = [['plan', ['plan', 'spec']], ['critique', ['critique']], ['tests', ['testsfirst']], ['code', ['code']],
    ['gates', ['gates', 'e2e']], ['review', ['review']], ['merge', ['merge']]];
  const FULL_ONLY = ['plan', 'critique', 'tests'];     // dropped when "Качество" is off
  const STATUS_STEP = { working: 'code', checking: 'gates', reviewing: 'review', merging: 'merge', needs_human: 'code', awaiting_outward: 'merge' };

  function stepState(raw) {
    const [name, payload] = variant(raw);
    const s = { pending: 'pending', running: 'running', done: 'done', skipped: 'skipped', failed: 'failed' }[name] || 'pending';
    return { state: s, reason: typeof payload === 'string' ? payload : null };
  }
  // -> [{key, state: done|run|wait|fail|skip|pending, reason}] for the strip "План · Критик · Тесты · Код · Ворота · Ревью · Влив"
  function stepBar(task) {
    const quality = !task.quality || task.quality.on !== false;
    const keys = BAR.filter(([k]) => quality || !FULL_ONLY.includes(k));
    const have = new Map();
    for (const pair of task.steps || []) have.set(norm(pair[0]), stepState(pair[1]));
    const waiting = kindOf(task) === 'wait';
    const derived = !task.steps;
    const cur = STATUS_STEP[task.status];
    const out = keys.map(([key, members]) => {
      let st = members.map((m) => have.get(m)).filter(Boolean);
      if (derived) {
        const order = keys.map((x) => x[0]), i = order.indexOf(key), c = order.indexOf(cur);
        const s = task.status === 'done' || task.status === 'done_not_pushed' ? 'done' : c < 0 ? 'pending' : i < c ? 'done' : i === c ? 'running' : 'pending';
        st = [{ state: s, reason: null }];
      }
      const any = (s) => st.find((x) => x.state === s);
      let state = 'pending', reason = null;
      if (any('failed')) { state = 'failed'; reason = any('failed').reason; }
      else if (any('running')) state = 'running';
      else if (st.length && st.every((x) => x.state === 'skipped')) { state = 'skipped'; reason = st[0].reason; }
      else if (st.length && st.filter((x) => x.state !== 'skipped').every((x) => x.state === 'done')) state = 'done';
      return { key, state, reason };
    });
    if (task.status === 'done') for (const s of out) if (s.state !== 'skipped') s.state = 'done';
    if (task.status === 'failed' && !out.some((s) => s.state === 'failed')) {
      const first = out.find((s) => s.state === 'running' || s.state === 'pending'); if (first) first.state = 'failed';
    }
    return out.map((s) => ({ key: s.key, reason: s.reason,
      state: { done: 'done', running: waiting ? 'wait' : 'run', failed: 'fail', skipped: 'skip', pending: 'pending' }[s.state] }));
  }

  // "Повторить push" (G01.7): PR project retries the PR, Push project pushes main again
  const retryPushCommand = (project) => (project && project.finish === 'pr' ? 'retry' : 'push_base');
  // buttons for an outward permission (G01.8): value = OutwardChoice
  function outwardChoices(project) {
    if (project && project.finish === 'pr') return [['push_and_pr', 'out.push_pr'], ['push_only', 'out.push_only'], ['no', 'out.no']];
    return [['push', 'out.push'], ['no', 'out.no']];
  }
  // only the actions that fit the status (G24.7); ids are i18n keys act.<id>
  const ACTIONS = {
    queued: ['cancel'], preparing: ['cancel', 'terminal', 'folder', 'editor'], working: ['cancel', 'terminal', 'folder', 'editor'],
    checking: ['cancel', 'terminal', 'folder', 'editor'], reviewing: ['cancel', 'terminal', 'folder', 'editor'], merging: ['cancel', 'folder'],
    needs_human: ['cancel', 'terminal', 'folder', 'editor'], awaiting_outward: ['cancel', 'folder', 'editor'],
    paused_limit: ['cancel', 'folder', 'editor'], merge_blocked: ['cancel', 'folder', 'editor'],
    handed_off: ['take_back', 'folder', 'editor'], failed: ['retry', 'terminal', 'folder', 'editor', 'remove'],
    done: ['undo', 'folder', 'remove'], done_not_pushed: ['retry_push', 'undo', 'folder', 'remove'],
    cancelled: ['retry', 'remove'], reverted: ['retry', 'remove'],
  };
  function taskActions(task) {
    const list = (ACTIONS[task.status] || []).slice();
    const start = task.start && variant(task.start)[0] === 'night';
    if (task.status === 'queued' && start) list.unshift('start_now');
    return list;
  }

  // "Взять модель сильнее" (G15.1): the first family of the env list that differs from the current one
  function strongerModel(catalog, current) {
    const fams = (catalog && catalog.families) || [];
    const cur = current && current.family;
    const f = fams.find((x) => x.id !== cur);
    return f ? { family: f.id, version: null } : null;
  }

  // first N items, the rest only as a count ("ещё 37")
  function capList(items, max) {
    const shown = items.slice(0, max);
    return { shown, more: items.length - shown.length };
  }

  function formatDuration(ms, lang) {
    const m = Math.floor(Math.max(0, ms) / MIN), h = Math.floor(m / 60), r = String(m % 60).padStart(2, '0');
    if (lang === 'ru') return m < 1 ? 'меньше минуты' : h ? `${h} ч ${r} мин` : `${m} мин`;
    return m < 1 ? 'under a minute' : h ? `${h} h ${r} min` : `${m} min`;
  }
  // 185000 -> "3:05", 4329000 -> "1:12:09"
  function formatElapsed(ms) {
    const s = Math.max(0, Math.floor(ms / 1000)), p = (n) => String(n).padStart(2, '0');
    const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = s % 60;
    return h ? `${h}:${p(m)}:${p(sec)}` : `${m}:${p(sec)}`;
  }
  // forms = [one, few, many] for ru; [one, other] for en
  function plural(n, forms, lang) {
    if (lang === 'ru') {
      const a = Math.abs(n) % 100, b = a % 10;
      if (a > 10 && a < 20) return forms[2];
      if (b > 1 && b < 5) return forms[1];
      if (b === 1) return forms[0];
      return forms[2];
    }
    return n === 1 ? forms[0] : forms[forms.length - 1];
  }

  // ---------- limits (G26) ----------
  const pad2 = (n) => String(n).padStart(2, '0');
  const hhmm = (d) => `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
  const sameDay = (a, b) => a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();

  function windowName(win, t) {
    const n = String(win.name || '').toLowerCase();
    const who = win.model ? ' ' + win.model.charAt(0).toUpperCase() + win.model.slice(1) : '';   // a per-model window: "Неделя Opus"
    if (/five|5\s*h|5\s*ч/.test(n)) return t('lim.w5h') + who;
    if (/seven|7\s*d|week|недел/.test(n)) return t('lim.wweek') + who;
    return (win.name || '') + who;
  }
  // same day -> "сброс в 19:00"; another day -> "пн 10:00" (as in the spec)
  function formatReset(iso, now, t) {
    if (!iso) return '';
    const d = new Date(iso), n = new Date(now);
    return sameDay(d, n) ? t('lim.at', { time: hhmm(d) }) : `${t('wd.' + d.getDay())} ${hhmm(d)}`;
  }
  const usageOf = (w) => (w.exhausted ? 101 : w.used_pct == null ? -1 : w.used_pct);
  // 'exhausted' | 'warn' (>= 80%) | 'ok' | 'unknown'
  function limitLevel(w, warnAt) {
    if (w.exhausted || (w.used_pct != null && w.used_pct >= 100)) return 'exhausted';
    if (w.used_pct == null) return 'unknown';
    return w.used_pct >= (warnAt == null ? WARN_AT : warnAt) ? 'warn' : 'ok';
  }
  // 'reset' = the window has reset since we last saw it; 'stale' = older than 6 h; else 'fresh'
  function limitFreshness(w, now) {
    if (w.resets_at && ts(w.resets_at) <= now && ts(w.seen_at) < ts(w.resets_at)) return 'reset';
    return now - ts(w.seen_at) > STALE_MS ? 'stale' : 'fresh';
  }
  // "5 ч · 54% · сброс в 19:00", exhausted: "5 ч · исчерпан · сброс в 19:00"
  function formatLimit(w, now, t) {
    const parts = [windowName(w, t)];
    if (w.exhausted) parts.push(t('lim.exhausted'));
    else if (w.used_pct != null) parts.push(Math.round(w.used_pct) + '%');
    else if (w.remaining) parts.push(t('lim.left', { n: w.remaining }));
    if (w.resets_at) {
      if (limitFreshness(w, now) === 'reset') parts.push(t('lim.reset_gone', { time: hhmm(new Date(w.resets_at)) }));
      else parts.push(formatReset(w.resets_at, now, t));
    }
    return parts.filter(Boolean).join(' · ');
  }
  // two most used windows on the header, the rest only as a count (G26.7)
  function pickWindows(wins, n) {
    const sorted = wins.map((w, i) => [w, i]).sort((a, b) => usageOf(b[0]) - usageOf(a[0]) || a[1] - b[1]).map((x) => x[0]);
    const k = n == null ? 2 : n;
    return { shown: sorted.slice(0, k), rest: sorted.slice(k) };
  }
  // EnvLimits {known:[...]} | {no_data:"why"} | null (nothing yet) -> {kind, windows|why}
  function limitsView(raw) {
    if (!raw) return { kind: 'none', windows: [] };
    if (Array.isArray(raw.known)) return { kind: 'known', windows: raw.known };
    if ('no_data' in raw) return { kind: 'nodata', windows: [], why: typeof raw.no_data === 'string' ? raw.no_data : '' };
    return { kind: 'none', windows: [] };
  }
  // the one consequence line under an exhausted window (G26.4); null when nothing is exhausted
  function exhaustedLine(wins, ctx) {
    if (!wins.some((w) => limitLevel(w) === 'exhausted')) return null;
    if (ctx && ctx.fallback) return { key: 'lim.cons.fallback', params: { model: ctx.fallback } };
    if (ctx && ctx.nightOnly) return { key: 'lim.cons.night' };
    return { key: 'lim.cons.wait' };
  }
  // hint at the "Рой" switch: only when some window is in the warning zone (G26.3)
  function swarmHint(wins, n) {
    const hot = wins.filter((w) => ['warn', 'exhausted'].includes(limitLevel(w)) && w.used_pct != null);
    if (!hot.length) return null;
    const worst = Math.max(...hot.map((w) => w.used_pct));
    return { n, left: Math.max(0, Math.round(100 - worst)) };
  }

  // ---------- models and versions (G18) ----------
  const defaultFamily = (catalog) => ((catalog && catalog.families) || []).find((f) => f.default) || ((catalog && catalog.families) || [])[0] || null;
  // choice {family, version|null} -> "Sonnet 5.5"; no version = latest of the family; unknown family = its id (custom model)
  function modelLabel(choice, catalog) {
    const c = choice || (defaultFamily(catalog) ? { family: defaultFamily(catalog).id, version: null } : null);
    if (!c) return null;
    if (c.family === 'default') return '';   // "Как в CLI по умолчанию" (core family id "default", vendor "cli"): no model flag at all
    const f = ((catalog && catalog.families) || []).find((x) => x.id === c.family);
    const v = c.version || (f && f.versions && f.versions[0]);
    return `${f ? f.label : c.family}${v ? ' ' + v : ''}`;
  }
  // "последняя (5.5)" · "5.5" · "4.6"; `newer` = "есть новее: 5.1" when an older version is pinned
  function versionRow(fam, pinned, t) {
    const vs = (fam && fam.versions) || [];
    if (!vs.length) return { items: [], newer: null };
    const items = [{ value: null, text: t('model.latest', { v: vs[0] }), on: !pinned, pinned: false }]
      .concat(vs.map((v) => ({ value: v, text: v, on: pinned === v, pinned: pinned === v })));
    const idx = pinned ? vs.indexOf(pinned) : -1;
    return { items, newer: idx > 0 ? vs[0] : null };
  }
  function groupByVendor(families) {
    const m = new Map();
    for (const f of families) (m.get(f.vendor) || m.set(f.vendor, []).get(f.vendor)).push(f);
    return [...m].map(([vendor, families]) => ({ vendor, families }));
  }
  const filterFamilies = (families, q) => {
    const s = String(q || '').trim().toLowerCase();
    return s ? families.filter((f) => `${f.label} ${f.id} ${f.vendor}`.toLowerCase().includes(s)) : families;
  };
  const GROUP_FROM = 8;   // more than 8 models: groups by vendor and search (G18)
  const needsGrouping = (families) => families.length > GROUP_FROM;

  // the dev mock only exists outside Tauri (Tauri 2 always defines __TAURI_INTERNALS__)
  const mockWanted = (win) => !win.__TAURI_INTERNALS__ && !win.__TAURI__;

  // ---------- envs ----------
  // list_envs row -> a flat shape the screens read; accepts the v1 list_agents row too
  function normalizeEnv(raw) {
    const installed = raw.installed != null ? !!raw.installed : raw.available != null ? !!raw.available : true;
    const custom = !!raw.custom || String(raw.id).startsWith('custom:');
    const reason = raw.error || raw.reason || '';
    let state = 'ok';
    if (custom && !(raw.configured != null ? raw.configured : raw.run)) state = 'unset';
    else if (!installed && !custom) state = 'missing';
    else if (raw.needs_login || raw.login_required || raw.signed_out) state = 'login';
    else if (reason || !installed) state = 'unavailable';
    return { id: raw.id, name: raw.name || raw.id, custom, state, version: raw.version || '', hint: raw.install_hint || raw.hint || '',
      reason, kitDir: raw.kit_dir || '' };
  }

  // ---------- feed ----------
  const MSG_LIMIT = 200;
  const nextBefore = (msgs) => (msgs.length ? msgs.reduce((m, x) => (ts(x.at) < ts(m) ? x.at : m), msgs[0].at) : null);
  const hasMore = (batchLen, limit) => batchLen >= (limit || MSG_LIMIT);
  function mergeMsgs(have, incoming) {
    const seen = new Map();
    for (const m of have.concat(incoming)) if (!seen.has(m.id)) seen.set(m.id, m);
    return [...seen.values()].sort((a, b) => ts(a.at) - ts(b.at));
  }
  // who: "human" | "model" | "system" | {"role": "worker"} -> {kind, role}
  function whoOf(msg) {
    const [name, payload] = variant(msg.who);
    return { kind: name || 'system', role: name === 'role' ? norm(payload) : null };
  }
  // consecutive tool actions fold into one row: "3 действия: edit, edit, run -> 7 passed, 2 failed"
  function groupFeed(msgs) {
    const out = [];
    let cur = null;
    for (const m of msgs) {
      if (m.tool) {
        if (!cur) { cur = { type: 'tools', items: [] }; out.push(cur); }
        cur.items.push(m);
      } else { cur = null; out.push({ type: 'msg', msg: m }); }
    }
    for (const g of out) if (g.type === 'tools') {
      const last = g.items[g.items.length - 1].tool;
      g.count = g.items.length; g.names = g.items.map((x) => x.tool.name);
      g.result = last.result || ''; g.bad = last.ok === false;
    }
    return out;
  }

  // ---------- sending (G05) ----------
  function validateSend(text) {
    if (!String(text).trim()) return { ok: false, reason: 'empty' };
    if (Array.from(text).length > MAX_TASK_CHARS) return { ok: false, reason: 'too_long' };
    return { ok: true };
  }
  // one id per attempt: a double press is ignored while the call is in flight; a retry of the same text after a failure reuses the id
  function createSendGate(genId) {
    let inflight = false, failed = null;
    return {
      begin(key, text) {
        if (inflight) return null;
        inflight = true;
        const id = failed && failed.key === key && failed.text === text ? failed.id : genId();
        failed = { key, text, id };
        return id;
      },
      done() { inflight = false; failed = null; },
      fail() { inflight = false; },
    };
  }
  const draftKey = (env, chatId) => `ns.draft.${env}.${chatId || 'new'}`;

  const api = { MAX_TASK_CHARS, WARN_AT, STALE_MS, ARCHIVE_MS, MSG_LIMIT, KINDS, KIND_LABEL, KIND_ICON, STATUS, variant, info, kindOf, labelKey, iconOf,
    roleKey, chatState, chatArchived, groupChats, badgeText, envCounts, stepBar, retryPushCommand, outwardChoices, taskActions, strongerModel, capList,
    formatDuration, formatElapsed, plural, windowName, formatReset, formatLimit, limitLevel, limitFreshness, pickWindows, limitsView, exhaustedLine,
    swarmHint, defaultFamily, modelLabel, versionRow, groupByVendor, filterFamilies, needsGrouping, normalizeEnv, nextBefore, hasMore, mergeMsgs,
    whoOf, groupFeed, mockWanted, validateSend, createSendGate, draftKey };
  if (typeof module !== 'undefined') module.exports = api;
  root.NSLogic = api;
})(typeof window !== 'undefined' ? window : globalThis);
