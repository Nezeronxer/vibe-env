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

  // v8: no model in the name ("Неделя Opus" read badly): the bar names the agent once, the tooltip has the rest
  function windowName(win, t) {
    const n = String(win.name || '').toLowerCase();
    if (/five|5\s*h|5\s*ч/.test(n)) return t('lim.w5h');
    if (/seven|7\s*d|week|недел/.test(n)) return t('lim.wweek');
    if (/^(day|daily|one_day|1d|24h)$/.test(n)) return t('lim.wday');
    if (/^(month|monthly|30d)$/.test(n)) return t('lim.wmonth');
    return win.name || '';
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
  // bars and numbers show what is left (просьба 28.10): a full bar that drains; null = unknown
  const leftPct = (w) => (w.exhausted ? 0 : w.used_pct == null ? null : Math.max(0, Math.min(100, Math.round(100 - w.used_pct))));
  // "5 ч · осталось 46% · сброс в 19:00", exhausted: "5 ч · исчерпан · сброс в 19:00"
  function formatLimit(w, now, t) {
    const parts = [windowName(w, t)];
    if (w.exhausted) parts.push(t('lim.exhausted'));
    else if (w.used_pct != null) parts.push(t('lim.left_pct', { n: leftPct(w) }));
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
    const env = { id: raw.id, name: raw.name || raw.id, custom, state, version: raw.version || '', hint: raw.install_hint || raw.hint || '',
      reason, kitDir: raw.kit_dir || '' };
    if (raw.sandbox) env.sandbox = raw.sandbox;      // SandboxInfo from the adapter (rights screen, R17i.3)
    if (raw.path) env.path = raw.path;               // where the CLI was found
    if (raw.install) env.install = raw.install;      // v8: {line, argv|null, piped} - the agents column installs by it
    if (Array.isArray(raw.efforts) && raw.efforts.length) env.efforts = raw.efforts;   // Adapter::efforts: the thinking levels of the CLI
    return env;
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

  // ================= task 15: swarm, factory, settings, kits, "while you were away" =================
  // ---------- swarm ----------
  const SWARM_MIN = 2, SWARM_MAX = 5, SWARM_DEFAULT = 3;       // story 152, 159
  function clampSwarmN(v) {
    const n = Math.round(Number(v));
    return Number.isFinite(n) ? Math.max(SWARM_MIN, Math.min(SWARM_MAX, n)) : SWARM_DEFAULT;
  }
  const stepSwarmN = (n, delta) => clampSwarmN(clampSwarmN(n) + delta);

  // cost only where the environment reports it (spec 30): null -> '' and the screen says "среда не сообщает"
  function formatCost(usd) {
    if (usd == null || !Number.isFinite(Number(usd))) return '';
    const v = Number(usd);
    return v > 0 && v < 0.01 ? '<$0.01' : '$' + v.toFixed(2);
  }
  const formatBudget = (spent, cap, t) => `${formatCost(spent)} ${cap == null ? '' : (t ? t('fmt.of') : 'из') + ' $' + (Number.isInteger(Number(cap)) ? Number(cap) : Number(cap).toFixed(2))}`.trim();
  // whole minutes left until the reviewer's choice is taken automatically; null when there is no timer or it is due
  function autopickLeft(atIso, now) {
    if (!atIso) return null;
    const ms = ts(atIso) - now;
    return ms > 0 ? Math.ceil(ms / MIN) : null;
  }
  // variants for display: passed by score, then still running / waiting, then failed by green tests; "Лучший" = reviewer's advice,
  // else the best score among passed; when nothing passed the best attempt is the one with the most green tests (story 158)
  function rankVariants(variants, advice) {
    const green = (v) => (v.gate ? v.gate.passed : -1);
    const group = (v) => (v.state === 'passed' ? 0 : v.state === 'failed' ? 2 : 1);
    const sorted = variants.slice().sort((a, b) => group(a) - group(b)
      || (group(a) === 0 ? (b.score || 0) - (a.score || 0) : group(a) === 2 ? green(b) - green(a) : 0) || String(a.id).localeCompare(String(b.id)));
    const passed = sorted.filter((v) => v.state === 'passed');
    const bestId = advice && advice.variant ? advice.variant : passed.length ? passed[0].id : null;
    const failed = sorted.filter((v) => v.state === 'failed');
    const bestFailedId = !passed.length && failed.length && failed.every((v) => v.gate) ? failed[0].id : null;
    return sorted.map((v) => Object.assign({}, v, { best: v.id === bestId, bestFailed: v.id === bestFailedId, canTake: v.state === 'passed' }));
  }
  // council votes [{voter, for}] -> counts, the winner by majority, a tie (the reviewer decides, story 155)
  function tallyVotes(votes) {
    const counts = {};
    for (const v of votes) counts[v.for] = (counts[v.for] || 0) + 1;
    const top = Math.max(0, ...Object.values(counts));
    const leaders = Object.keys(counts).filter((k) => counts[k] === top).sort();
    return { counts, total: votes.length, winner: leaders.length === 1 ? leaders[0] : null, tie: leaders.length > 1 ? leaders : [] };
  }

  // ---------- factory ----------
  // a bar [from, to] on an axis [t0, t1] -> {left, width} in percent, clipped to the axis; null when it lies outside
  function placeBar(from, to, t0, t1) {
    if (to <= t0 || from >= t1) return null;
    const l = Math.max(from, t0), r = Math.min(to, t1), span = t1 - t0;
    return { left: ((l - t0) * 100) / span, width: ((r - l) * 100) / span };
  }
  const startMs = (t) => ts(t.started_at) || ts(t.created_at);
  // the pipe into main: who merges now, who waits, who is blocked, who is still in review (pale)
  function mergeQueue(tasks) {
    const by = (status) => tasks.filter((t) => t.status === status).sort((a, b) => startMs(a) - startMs(b));
    const merging = by('merging');
    return [].concat(merging.map((t, i) => ({ id: t.id, task: t, kind: i === 0 ? 'head' : 'wait' })),
      by('merge_blocked').map((t) => ({ id: t.id, task: t, kind: 'blocked' })), by('reviewing').map((t) => ({ id: t.id, task: t, kind: 'ghost' })));
  }
  const isActive = (t) => ['wait', 'run'].includes(kindOf(t)) && t.status !== 'handed_off';
  const STRIP_ROLE = { coordinator: 'coord', council: 'coord', worker: 'exec', reviewer: 'rev' };
  // aggregated roles of one env: Координатор · Исполнители ×N · Ревьюер · Надзиратель · Очередь (G12.2)
  function rolesStrip(tasks) {
    const act = tasks.filter(isActive);
    if (!act.length) return [];
    const agg = { coord: [], exec: [], rev: [] };
    let watcherModel = null;
    for (const t of act) {
      const runs = t.runs && t.runs.length ? t.runs : t.status === 'working' ? [{ role: 'worker', model: null, state: 'running' }] : [];
      for (const r of runs) {
        const key = STRIP_ROLE[norm(r.role)];
        if (key) agg[key].push(r);
        else if (norm(r.role) === 'watcher') watcherModel = r.model || watcherModel;
      }
    }
    const out = [];
    for (const key of ['coord', 'exec', 'rev']) {
      const all = agg[key];
      if (!all.length) continue;
      const running = all.filter((r) => variant(r.state)[0] === 'running');
      const pool = running.length ? running : all;
      out.push({ key, count: running.length, state: running.length ? 'work' : 'idle', models: [...new Set(pool.map((r) => r.model).filter(Boolean))] });
    }
    out.push({ key: 'watch', count: 0, state: 'watch', models: watcherModel ? [watcherModel] : [] });
    const queue = mergeQueue(tasks).length;
    if (queue) out.push({ key: 'queue', count: queue, state: 'work', models: [] });
    return out;
  }

  // bars of one task: runs as they happened, then the steps still to come as dashed "впереди" (PROJ_MIN each)
  const STEP_ROLE = { plan: 'plan', critique: 'crit', tests: 'test', code: 'exec', gates: 'gate', review: 'rev', merge: 'integ' };
  const PROJ_MIN = 5;
  function laneSegments(task, now) {
    const segs = [], waiting = kindOf(task) === 'wait';
    for (const r of task.runs || []) {
      const key = roleKey(r.role), st = variant(r.state)[0];
      if (!key || ['coord', 'watch'].includes(key) || !r.started_at || st === 'skipped' || st === 'pending') continue;     // a coordinator or the watcher is not a step of the task
      const from = ts(r.started_at), to = r.finished_at ? ts(r.finished_at) : now;
      segs.push({ role: key, state: st === 'running' ? (waiting ? 'wait' : 'run') : st === 'failed' ? 'fail' : 'done', from, to: Math.max(to, from), model: r.model || '', variant: r.variant || null });
    }
    if (isActive(task)) {
      let at = now;
      for (const s of stepBar(task)) if (s.state === 'pending') { segs.push({ role: STEP_ROLE[s.key], state: 'proj', from: at, to: at + PROJ_MIN * MIN }); at += PROJ_MIN * MIN; }
    }
    return segs;
  }
  // tasks of one env as lanes: the last hour with a stretch ahead (offsetMin scrolls back), most urgent first, at most `max` rows (story 168)
  function factoryRows(tasks, now, opts) {
    const o = Object.assign({ windowMin: 60, aheadMin: 20, offsetMin: 0, max: 20 }, opts);
    const t0 = now - (o.windowMin - o.aheadMin) * MIN - o.offsetMin * MIN, t1 = t0 + o.windowMin * MIN;
    const rank = (t) => KINDS.indexOf(kindOf(t));
    const shown = tasks.filter((t) => {
      if (o.offsetMin === 0 && ['wait', 'fail', 'run', 'queue'].includes(kindOf(t))) return true;
      const from = startMs(t), to = t.finished_at ? ts(t.finished_at) : now;
      return to >= t0 && from <= t1;
    }).sort((a, b) => rank(a) - rank(b) || startMs(b) - startMs(a));
    const { shown: rows, more } = capList(shown, o.max);
    return { t0, t1, now, more, rows: rows.map((t) => ({ id: t.id, task: t, kind: kindOf(t),
      segs: laneSegments(t, now).map((s) => Object.assign({}, s, placeBar(s.from, s.to, t0, t1))).filter((s) => s.left != null) })) };
  }
  // "Работают X из N" counts agent runs at work; "Сегодня $" sums what the env itself reported today
  function factoryStats(tasks, now) {
    let working = 0, spent = 0, reportsCost = false;
    const today = new Date(now);
    for (const t of tasks) {
      if (t.cost_usd != null) {
        reportsCost = true;
        if (sameDay(new Date(startMs(t)), today)) spent += t.cost_usd;
      }
      if (kindOf(t) !== 'run' || !info(t.status).live) continue;
      const running = (t.runs || []).filter((r) => variant(r.state)[0] === 'running').length;
      working += t.runs && t.runs.length ? running : 1;
    }
    return { working, spent: reportsCost ? Math.round(spent * 100) / 100 : null, reportsCost };
  }
  // feed of the factory: LogEvent {task_id, at, kind, text, role?} -> entries, newest first, at most 500; tool noise is not shown
  const FEED_MAX = 500;
  const FEED_ICON = { check: 'gate', review: 'rev', git: 'queue', stage: 'coord', agent: 'exec', error: 'exec', human: 'coord' };
  function feedEntries(events, tasks, now) {
    const byId = new Map(tasks.map((t) => [t.id, t]));
    const out = events.filter((e) => norm(e.kind) !== 'tool').map((e) => {
      const rk = roleKey(e.role), task = byId.get(e.task_id);
      const level = norm(e.kind) === 'error' || rk === 'watch' ? 'bad' : null;
      return { ms: ts(e.at), at: e.at, taskId: e.task_id, role: rk || FEED_ICON[norm(e.kind)] || 'coord', text: e.text, model: e.model || '', level,
        acts: rk === 'watch' && !!task && isActive(task) };
    });
    for (const t of tasks) if (kindOf(t) === 'wait' && t.question && t.question.text) out.push({ ms: now, at: new Date(now).toISOString(), taskId: t.id, role: 'exec', text: t.question.text, model: '', level: 'warn', acts: false, need: true });
    return out.sort((a, b) => b.ms - a.ms).slice(0, FEED_MAX);
  }
  function filterFeed(entries, mode) {
    const problems = entries.filter((e) => e.level);
    return { entries: mode === 'problems' ? problems : entries, count: problems.length };
  }
  // a convoy (G08.1): overall progress "3 из 5" and one cell per task
  function convoyProgress(tasks) {
    const cell = (t) => (['done', 'done_not_pushed'].includes(t.status) ? 'd' : kindOf(t) === 'fail' ? 'f' : kindOf(t) === 'wait' ? 'w' : kindOf(t) === 'run' ? 'r' : '');
    const parts = tasks.map(cell);
    return { done: parts.filter((p) => p === 'd').length, total: tasks.length, parts };
  }

  // ---------- "Пока тебя не было" ----------
  const SUMMARY_MAX = 10;
  // Summary {done, failed, waiting, spent, limits} (summary_since) -> rows by importance, "ещё N" past `max`; no events = no banner
  function summaryRows(summary, max) {
    const s = summary || {}, cap = max == null ? SUMMARY_MAX : max;
    const all = [].concat((s.waiting || []).map((x) => Object.assign({ kind: 'wait' }, x)), (s.failed || []).map((x) => Object.assign({ kind: 'fail' }, x)),
      (s.done || []).map((x) => Object.assign({ kind: 'done' }, x)), (s.limits || []).map((x) => Object.assign({ kind: 'limit' }, x)));
    const { shown, more } = capList(all, cap);
    const money = (s.spent || []).filter((x) => x.usd != null);
    return { rows: shown, more, empty: !all.length, spent: money.length ? Math.round(money.reduce((a, x) => a + x.usd, 0) * 100) / 100 : null };
  }
  // away >= afterMin minutes: from the moment the window lost focus; a stopped heartbeat while focused = the computer slept (A06.1)
  const SLEEP_GAP = 3 * MIN;
  function awayDecision(p) {
    let since = null;
    if (p.leftAt != null) since = p.leftAt;
    else if (p.lastAlive != null && p.now - p.lastAlive > SLEEP_GAP) since = p.lastAlive;
    if (since == null) return { show: false, since: null };
    return { show: p.now - since >= p.afterMin * MIN, since };
  }
  // the chat of a swarm: the live swarm task wins, else the newest one
  function swarmTaskOf(tasks) {
    const sw = tasks.filter((t) => t.swarm);
    if (!sw.length) return null;
    const live = sw.filter((t) => ['wait', 'run', 'queue'].includes(kindOf(t)));
    return (live.length ? live : sw).slice().sort((a, b) => ts(b.created_at) - ts(a.created_at))[0];
  }

  // ---------- settings ----------
  // story 194: the current env first, then "Для всех сред", then "Проекты"
  const SETTINGS_SECTIONS = [['env', 'models'], ['env', 'kits'], ['env', 'perms'], ['env', 'run'], ['all', 'quality'], ['all', 'swarm'], ['all', 'limits'], ['all', 'notify'], ['all', 'look'], ['all', 'keys'], ['projects', 'projects']]
    .map(([group, id]) => ({ group, id }));
  // -> {ok, value} | {ok: false, why: 'nan' | 'range'}; a decimal comma is accepted
  function validateNumber(raw, rule) {
    const s = String(raw == null ? '' : raw).trim();
    if (!/^-?\d+([.,]\d+)?$/.test(s) || (rule.int && /[.,]/.test(s))) return { ok: false, why: 'nan' };
    const v = Number(s.replace(',', '.'));
    if ((rule.min != null && v < rule.min) || (rule.max != null && v > rule.max)) return { ok: false, why: 'range' };
    return { ok: true, value: v };
  }
  function validateTime(raw) {
    const m = /^(\d{1,2}):(\d{2})$/.exec(String(raw == null ? '' : raw).trim());
    if (!m || +m[1] > 23 || +m[2] > 59) return { ok: false, why: 'format' };
    return { ok: true, value: pad2(+m[1]) + ':' + m[2] };
  }
  // extra CLI arguments: split like a shell would but never run through one; an unclosed quote is an error
  function parseArgs(str) {
    const args = [];
    let cur = '', has = false, quote = null;
    const s = String(str);
    for (let i = 0; i < s.length; i++) {
      const c = s[i];
      if (quote) {
        if (c === quote) quote = null;
        else if (c === '\\' && quote === '"' && i + 1 < s.length) cur += s[++i];
        else cur += c;
      } else if (c === '"' || c === "'") { quote = c; has = true; }
      else if (c === '\\' && i + 1 < s.length) { cur += s[++i]; has = true; }
      else if (/\s/.test(c)) { if (has || cur) args.push(cur); cur = ''; has = false; }
      else { cur += c; has = true; }
    }
    if (quote) return { error: 'quotes' };
    if (has || cur) args.push(cur);
    return { args };
  }
  const formatArgs = (args) => args.map((a) => (a === '' || /[\s"'\\]/.test(a) ? '"' + a.replace(/(["\\])/g, '\\$1') + '"' : a)).join(' ');

  // hotkeys
  const CODE_KEY = { Period: '.', Comma: ',', Slash: '/', Backquote: '`', Space: 'Space', Enter: 'Enter', Minus: '-', Equal: '=', Semicolon: ';', Quote: "'", BracketLeft: '[', BracketRight: ']', Backslash: '\\' };
  function accelFromEvent(ev, platform) {
    const c = ev.code || '';
    let key = null;
    if (/^Key[A-Z]$/.test(c)) key = c[3];
    else if (/^Digit\d$/.test(c)) key = c[5];
    else if (/^F\d{1,2}$/.test(c)) key = c;
    else if (CODE_KEY[c]) key = CODE_KEY[c];
    if (!key) return null;
    const mods = [ev.ctrlKey && 'Ctrl', ev.altKey && 'Alt', ev.shiftKey && 'Shift', ev.metaKey && (platform === 'mac' ? 'Cmd' : 'Super')].filter(Boolean);
    return mods.concat(key).join('+');
  }
  const RESERVED = ['cmd+q', 'cmd+w', 'cmd+tab', 'cmd+space', 'alt+f4', 'ctrl+alt+delete', 'super+l'];
  function validateAccel(accel, opts) {
    const o = opts || {};
    const parts = String(accel || '').split('+');
    const key = parts.pop(), mods = parts;
    const okKey = /^([A-Za-z0-9]|F\d{1,2}|Enter|Space|[.,/`\-=;'\[\]\\])$/.test(key || '');
    const okMods = mods.every((m) => ['ctrl', 'alt', 'shift', 'cmd', 'super'].includes(m.toLowerCase()));
    const real = mods.some((m) => ['ctrl', 'alt', 'cmd', 'super'].includes(m.toLowerCase()));
    if (!okKey || !okMods || (o.needModifier && !real)) return { ok: false, why: 'format' };
    const norm2 = String(accel).toLowerCase();
    if (RESERVED.includes(norm2) || (o.taken || []).some((x) => String(x).toLowerCase() === norm2)) return { ok: false, why: 'busy' };
    return { ok: true };
  }
  const KEY_CAP = { Alt: '⌥', Cmd: '⌘', Ctrl: '⌃', Shift: '⇧', Enter: '↵' };
  function keyLabel(accel, platform) {
    return String(accel).split('+').map((p) => (p === 'Enter' ? '↵' : platform === 'mac' && KEY_CAP[p] ? KEY_CAP[p] : p));
  }

  // models per role: select value "family|version" ("" = inherit)
  const choiceKey = (c) => `${c.family}|${c.version || ''}`;
  function choiceFromKey(k) {
    if (!k) return null;
    const i = k.indexOf('|');
    return { family: k.slice(0, i), version: k.slice(i + 1) || null };
  }
  function roleOptions(catalog, t, inheritKey, current) {
    const fams = ((catalog && catalog.families) || []).filter((f) => f.id !== 'default');
    const out = [{ value: '', text: t(inheritKey) }];
    for (const f of fams) {
      out.push({ value: f.id + '|', text: t('set.latest_short', { label: f.label }) });
      for (const v of f.versions || []) out.push({ value: `${f.id}|${v}`, text: `${f.label} ${v}` });
    }
    if (current && current.family !== 'default' && !fams.some((f) => f.id === current.family)) out.push({ value: choiceKey(current), text: current.family });
    return out;
  }
  // the newest version of the family when an older one is pinned
  function pinNewer(catalog, choice) {
    if (!choice || !choice.version) return null;
    const fam = ((catalog && catalog.families) || []).find((f) => f.id === choice.family);
    return fam ? versionRow(fam, choice.version, () => '').newer : null;
  }
  // "Сбросить раздел": the keys to DROP from the settings (and from the env's block); the core fills dropped keys with its defaults
  // (Settings and EnvSettings are #[serde(default)]), so the window keeps no copy of the defaults. null = nothing to reset
  const RESET = { models: [[], ['default_model', 'roles', 'fallback', 'custom_models']], perms: [[], ['ask_push', 'ask_pr', 'protected_paths']], run: [[], ['cli_path', 'extra_args']],
    quality: [['quality', 'review_rounds'], []], swarm: [['max_parallel', 'swarm_default', 'attempts', 'stall_min', 'autopick_min'], []], limits: [['limits'], []],
    notify: [['notify'], []], look: [['look', 'language'], []], keys: [['keys'], []] };
  const resetPatch = (section) => (RESET[section] ? { settings: RESET[section][0], env: RESET[section][1] } : null);

  // ---------- kits ----------
  const KIT_GROUPS = ['skills', 'rules', 'hooks', 'mcp', 'agents', 'settings'];
  const CHANGE = { add: ['+', 0], replace: ['~', 1], same: ['=', 2] };
  // ImportPlan.entries [{group, label, change, diff}] -> lines "+ Скиллы: a, b" / "~ ..." / "= ...": added, replaced (with its diff), already there
  function groupChanges(entries) {
    const lines = new Map();
    for (const x of entries) {
      const k = x.change + '|' + x.group;
      if (!lines.has(k)) lines.set(k, { sign: CHANGE[x.change][0], cls: x.change, group: x.group, names: [], diff: null, order: CHANGE[x.change][1] * 10 + Math.max(0, KIT_GROUPS.indexOf(x.group)) });
      const l = lines.get(k);
      l.names.push(x.label);
      if (x.diff) l.diff = (l.diff || []).concat(String(x.diff).split('\n'));
    }
    return [...lines.values()].sort((a, b) => a.order - b.order);
  }
  // "Установить" needs a chosen place and at least one thing to add or replace; until then nothing touches the disk
  const canInstall = (plan, scope) => !!scope && !!plan && (plan.entries || []).some((x) => x.change === 'add' || x.change === 'replace');
  // entries that run code (hooks, MCP servers, scripted skills, running settings): shown whole, installed off
  const heldEntries = (plan) => ((plan && plan.entries) || []).filter((x) => x.held);
  // the keys the person ticked, in the order of the plan; unknown keys are ignored. They are switched on after the install (kits_enable)
  const enableKeys = (plan, ticked) => { const on = ticked instanceof Set ? ticked : new Set(ticked || []); return heldEntries(plan).map((x) => x.key).filter((k) => on.has(k)); };
  // KitError (serde tag "kind") as an object or its JSON text -> {key, detail, ...}; old "key: detail" text and anything else is kept whole
  const KIT_REFUSALS = ['not_a_kit', 'bad_zip', 'unsafe_path', 'symlink', 'file_too_large', 'kit_too_large', 'too_many_files', 'corrupt', 'wrong_env', 'clone_failed', 'unknown_env', 'dest_not_empty',
    'not_found', 'changed', 'bad_name', 'bad_value', 'interrupted', 'io', 'not_applied'];
  function kitRefusal(raw) {
    let e = raw;
    if (typeof e === 'string') { try { e = JSON.parse(e); } catch (x) { e = null; } }
    if (!e || typeof e !== 'object' || !KIT_REFUSALS.includes(e.kind)) return { key: 'unknown', detail: typeof raw === 'string' ? raw : JSON.stringify(raw) };
    const str = Object.keys(e).filter((k) => k !== 'kind' && typeof e[k] === 'string').map((k) => e[k]);
    if (e.kind === 'wrong_env') return { key: 'wrong_env', detail: e.kit_env_name || e.kit_env, env: e.kit_env, target: e.target_env_name || e.target_env };
    if (e.kind === 'not_applied') return { key: 'not_applied', detail: e.cli || '', keys: e.keys || [] };
    // a kit.json that is there but is not a kit of this app is not «no kit.json» (the detail says which)
    if (e.kind === 'not_a_kit' && /^format_\d+$/.test(str[0] || '')) return { key: 'kit_format', detail: str[0].slice(7) };
    if (e.kind === 'not_a_kit' && ['bad_kit_json', 'bad_kind', 'kit_json_too_large'].includes(str[0])) return { key: 'bad_kit_json', detail: str[0] };
    return { key: e.kind, detail: e.path || str[0] || (e.size != null ? String(e.size) : '') };
  }
  // GroupInfo {group, available, active, note, count}: off when the layout is not verified or nothing is there
  const kitGroupOff = (g) => (g.note === 'layout_unverified' || g.available === false ? 'unverified' : g.active === false || !g.count ? 'empty' : null);
  const exportSelection = (groups, ticked) => groups.filter((g) => ticked.has(g.group) && !kitGroupOff(g)).map((g) => g.group);

  // "Откуда" of an import in the core form KitSource: link / ssh -> {repo}, *.zip -> {zip}, else {folder}
  function kitSourceOf(raw) {
    const v = String(raw == null ? '' : raw).trim();
    if (!v) return null;
    if (/^(https?:\/\/|ssh:\/\/|git@)/i.test(v)) return { repo: v };
    return /\.zip$/i.test(v) ? { zip: v } : { folder: v };
  }

  // lanes of a swarm built from the runs of its task (RoleRun.variant): shared preparation, a lane per variant, then the choice
  const PREP = ['plan', 'crit', 'test', 'coord'];
  function swarmLanes(task, now) {
    const runs = (task && task.runs) || [];
    const bar = (r, role, state) => ({ role, state, from: ts(r.started_at), to: r.finished_at ? ts(r.finished_at) : now, model: r.model || '' });
    const stateOf = (r) => variant(r.state)[0];
    const live = !!task && isActive(task);
    const lanes = [];
    const prep = runs.filter((r) => !r.variant && PREP.includes(roleKey(r.role)) && r.started_at);
    if (prep.length) lanes.push({ key: 'prep', kind: 'prep', segs: prep.map((r) => bar(r, roleKey(r.role), stateOf(r) === 'running' ? 'run' : stateOf(r) === 'failed' ? 'fail' : 'done')) });
    const letters = [...new Set(runs.filter((r) => r.variant).map((r) => r.variant))].sort();
    for (const v of letters) {
      const mine = runs.filter((r) => r.variant === v && r.started_at);
      const worker = mine.find((r) => roleKey(r.role) === 'exec');
      const segs = mine.map((r) => {
        const k = roleKey(r.role), s = stateOf(r);
        if (k === 'gate') return bar(r, 'gate', s === 'failed' ? 'fail' : s === 'running' ? 'run' : 'ok');
        return bar(r, k || 'exec', s === 'running' ? 'run' : s === 'failed' ? 'fail' : 'done');
      });
      lanes.push({ key: v, kind: 'variant', variant: v, model: (worker || mine[0] || {}).model || '', segs });
    }
    const pick = runs.filter((r) => !r.variant && ['rev', 'integ'].includes(roleKey(r.role)) && r.started_at);
    const segs = pick.map((r) => bar(r, roleKey(r.role), stateOf(r) === 'running' ? 'run' : stateOf(r) === 'failed' ? 'fail' : 'done'));
    if (live && !pick.some((r) => roleKey(r.role) === 'rev')) {            // the comparison and the merge are still ahead
      const at = Math.max(now, ...lanes.flatMap((l) => l.segs.map((x) => x.to)));
      segs.push({ role: 'rev', state: 'proj', from: at, to: at + 2 * MIN, model: '' }, { role: 'integ', state: 'proj', from: at + 2 * MIN, to: at + 4 * MIN, model: '' });
    }
    if (segs.length) lanes.push({ key: 'choice', kind: 'choice', model: (pick.find((r) => roleKey(r.role) === 'rev') || {}).model || '', segs });
    const all = lanes.flatMap((l) => l.segs);
    const t0 = all.length ? Math.min(...all.map((x) => x.from)) : now, t1 = all.length ? Math.max(...all.map((x) => x.to)) : now + MIN;
    return { lanes, t0, t1: t1 > t0 ? t1 : t0 + MIN };
  }

  const api = { MAX_TASK_CHARS, WARN_AT, STALE_MS, ARCHIVE_MS, MSG_LIMIT, KINDS, KIND_LABEL, KIND_ICON, STATUS, variant, info, kindOf, labelKey, iconOf,
    roleKey, chatState, chatArchived, groupChats, badgeText, envCounts, stepBar, retryPushCommand, outwardChoices, taskActions, strongerModel, capList,
    formatDuration, formatElapsed, plural, windowName, formatReset, formatLimit, leftPct, limitLevel, limitFreshness, pickWindows, limitsView, exhaustedLine,
    swarmHint, defaultFamily, modelLabel, versionRow, groupByVendor, filterFamilies, needsGrouping, normalizeEnv, nextBefore, hasMore, mergeMsgs,
    whoOf, groupFeed, mockWanted, validateSend, createSendGate, draftKey,
    SWARM_MIN, SWARM_MAX, clampSwarmN, stepSwarmN, formatCost, formatBudget, autopickLeft, rankVariants, tallyVotes,
    placeBar, mergeQueue, rolesStrip, laneSegments, factoryRows, factoryStats, feedEntries, filterFeed, convoyProgress, FEED_MAX,
    summaryRows, awayDecision, swarmTaskOf,
    SETTINGS_SECTIONS, validateNumber, validateTime, parseArgs, formatArgs, accelFromEvent, validateAccel, keyLabel,
    choiceKey, choiceFromKey, roleOptions, pinNewer, resetPatch,
    KIT_GROUPS, groupChanges, canInstall, heldEntries, enableKeys, kitRefusal, kitGroupOff, exportSelection, kitSourceOf, swarmLanes };
  if (typeof module !== 'undefined') module.exports = api;
  root.NSLogic = api;
})(typeof window !== 'undefined' ? window : globalThis);
