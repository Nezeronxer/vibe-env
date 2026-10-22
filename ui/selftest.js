// Seams of the UI: logic.js (pure) and the i18n dictionary. Expected values are hand-computed or taken from the spec text
// (stories 96-101, 105-107, 114-115, 120-127, 171, G15.4, G05), never from running the code under test.
const out = []; let bad = 0;
function eq(name, got, want) { const ok = JSON.stringify(got) === JSON.stringify(want); if (!ok) bad++; out.push((ok ? 'PASS ' : 'FAIL ') + name + (ok ? '' : ` got=${JSON.stringify(got)} want=${JSON.stringify(want)}`)); }
const L = NSLogic;
NSI18n.setLang('ru');
const t = (k, p) => NSI18n.t(k, p);

// local-time constructors: the checks do not depend on the machine's time zone
const at = (y, mo, d, h, mi) => new Date(y, mo - 1, d, h, mi).toISOString();
const NOW = new Date(2026, 9, 21, 14, 20).getTime();             // Wed 21 Oct 2026, 14:20
const day = 864e5, hour = 36e5;
const task = (id, status, extra) => Object.assign({ id, status, agent: 'claude', chat_id: 'c1', created_at: new Date(NOW - 3 * day).toISOString(), finished_at: null }, extra);

// --- chat state: waits for you > failed > working > queued > done > plain talk (story 114) ---
const all = [task('d', 'done'), task('q', 'queued'), task('w', 'working'), task('f', 'failed'), task('n', 'needs_human')];
eq('chat priority: needs_human wins', L.chatState(all).task.id, 'n');
eq('chat priority: then failed', L.chatState(all.slice(0, 4)).task.id, 'f');
eq('chat priority: then working', L.chatState(all.slice(0, 3)).task.id, 'w');
eq('chat priority: then queued', L.chatState(all.slice(0, 2)).task.id, 'q');
eq('chat priority: done, then plain talk', [L.chatState([task('d', 'done')]).kind, L.chatState([]).kind], ['done', 'talk']);
eq('awaiting_outward counts as waiting for you', L.chatState([task('o', 'awaiting_outward'), task('w', 'working')]).kind, 'wait');
eq('of two waiting tasks the older one leads', L.chatState([task('new', 'needs_human', { created_at: new Date(NOW - day).toISOString() }), task('old', 'needs_human')]).task.id, 'old');
eq('every status of the machine has a kind', ['queued', 'preparing', 'working', 'checking', 'reviewing', 'merging', 'needs_human', 'paused_limit', 'awaiting_outward', 'merge_blocked',
  'handed_off', 'done', 'done_not_pushed', 'failed', 'cancelled', 'reverted'].filter((s) => !L.STATUS[s]), []);
eq('a status is always a word: every label key exists in ru and en',
  Object.keys(L.STATUS).map((s) => L.labelKey({ status: s })).filter((k) => !(k in NSI18n.dicts.ru) || !(k in NSI18n.dicts.en)), []);
eq('status words from the spec', ['needs_human', 'working', 'queued', 'done', 'failed'].map((s) => t(L.labelKey({ status: s }))), ['Ждёт тебя', 'Работает', 'В очереди', 'Готово', 'Не вышло']);

// --- grouping of chats by project, archive after 7 days (stories 114-115) ---
const chat = (id, project, daysIdle, extra) => Object.assign({ id, env: 'claude', project_id: project, title: id, updated_at: new Date(NOW - daysIdle * day).toISOString(), archived: false }, extra);
const projects = [{ id: 'p1', name: 'billing-api' }, { id: 'p2', name: 'web-site' }];
const chats = [chat('c1', 'p1', 0), chat('c2', 'p2', 1), chat('c3', 'p1', 8), chat('c4', 'p1', 9), chat('c5', 'p2', 30, { archived: true })];
const tasks = [task('t1', 'working', { chat_id: 'c4' }), task('t2', 'needs_human', { chat_id: 'c2' })];
const g = L.groupChats(chats, tasks, projects, NOW);
eq('groups: the project with a waiting chat goes first, empty groups are absent', g.groups.map((x) => x.name), ['web-site', 'billing-api']);
eq('idle for 8 days -> archive; idle but still working -> stays; flag -> archive', [g.archive.map((i) => i.chat.id).sort(), g.groups[1].items.map((i) => i.chat.id)], [['c3', 'c5'], ['c4', 'c1']]);
eq('7-day boundary: 6.9 days idle stays in the list', L.chatArchived(chat('x', 'p1', 6.9), [], NOW), false);
eq('no chats -> no groups', L.groupChats([], [], projects, NOW), { groups: [], archive: [] });

// --- counters: "9+" on the icon, snoozed failures leave the count (G15.4, G15.2) ---
eq('badge text', [0, 1, 9, 10, 30].map(L.badgeText), ['', '1', '9', '9+', '9+']);
const ts = [task('a', 'needs_human'), task('b', 'awaiting_outward'), task('c', 'failed'), task('d', 'working'), task('e', 'needs_human', { agent: 'codex' }), task('f', 'queued')];
eq('env counts: calls and running, only this env', L.envCounts(ts, 'claude'), { calls: 3, running: 1 });
eq('a postponed failure is not a call', L.envCounts(ts, 'claude', new Set(['c'])), { calls: 2, running: 1 });

// --- limits (stories 96-101) ---
const win = (name, pct, resets, extra) => Object.assign({ name, model: null, used_pct: pct, remaining: null, resets_at: resets, exhausted: false, source: 'cli', seen_at: at(2026, 10, 21, 14, 10) }, extra);
eq('limit line, same day (spec: "5 ч · 54% · сброс в 19:00")', L.formatLimit(win('five_hour', 54, at(2026, 10, 21, 19, 0)), NOW, t), '5 ч · 54% · сброс в 19:00');
eq('limit line, another day (spec: "Неделя · 16% · пн 10:00")', L.formatLimit(win('seven_day', 16, at(2026, 10, 26, 10, 0)), NOW, t), 'Неделя · 16% · пн 10:00');
eq('exhausted line (spec: "исчерпан · сброс в 19:00")', L.formatLimit(win('five_hour', 100, at(2026, 10, 21, 19, 0), { exhausted: true }), NOW, t), '5 ч · исчерпан · сброс в 19:00');
eq('window reset since last seen (spec: "окно сброшено в 19:00, новых данных нет")', L.formatLimit(win('five_hour', 54, at(2026, 10, 21, 13, 0), { seen_at: at(2026, 10, 21, 12, 0) }), NOW, t), '5 ч · 54% · окно сброшено в 13:00, новых данных нет');
const five = [win('a', 10), win('b', 54), win('c', 85), win('d', 30), win('e', 16)];
const picked = L.pickWindows(five, 2);
eq('two most used windows, the rest only as a count ("ещё 3")', [picked.shown.map((w) => w.name), picked.rest.length, t('lim.more', { n: picked.rest.length })], [['c', 'b'], 3, 'ещё 3']);
eq('an exhausted window outranks any percentage', L.pickWindows([win('a', 99), win('b', 5, null, { exhausted: true })], 1).shown[0].name, 'b');
eq('level: 79 ok, 80 warning, exhausted, unknown', [79, 80].map((p) => L.limitLevel(win('x', p))).concat(L.limitLevel(win('x', 100, null, { exhausted: true })), L.limitLevel(win('x', null))), ['ok', 'warn', 'exhausted', 'unknown']);
eq('the warning threshold is a setting', L.limitLevel(win('x', 70), 60), 'warn');
eq('freshness: 5 h fresh, 7 h grey, reset window', [win('x', 1, null, { seen_at: new Date(NOW - 5 * hour).toISOString() }), win('x', 1, null, { seen_at: new Date(NOW - 7 * hour).toISOString() }),
  win('x', 1, at(2026, 10, 21, 13, 0), { seen_at: at(2026, 10, 21, 12, 0) })].map((w) => L.limitFreshness(w, NOW)), ['fresh', 'stale', 'reset']);
eq('limits view reads the contract tags: known / no_data / nothing yet', [L.limitsView({ known: [1] }).kind, L.limitsView({ no_data: 'why' }).why, L.limitsView({ no_data: 'why' }).kind, L.limitsView(null).kind], ['known', 'why', 'nodata', 'none']);
eq('consequence line: fallback model, else waiting; none when nothing is exhausted', [L.exhaustedLine([win('x', 100, null, { exhausted: true })], { fallback: 'Sonnet 5.5' }).key, L.exhaustedLine([win('x', 100, null, { exhausted: true })], {}).key, L.exhaustedLine([win('x', 50)], {})], ['lim.cons.fallback', 'lim.cons.wait', null]);
eq('swarm hint: ×3 at 15% left (spec: "рой ×3 при остатке 15%")', t('swarm.hint', L.swarmHint([win('x', 85)], 3)), 'рой ×3 при остатке 15% - лимит кончится раньше');
eq('no swarm hint while the limit is comfortable', L.swarmHint([win('x', 40)], 3), null);

// --- model versions ---
const sonnet = { id: 'sonnet', label: 'Sonnet', vendor: 'anthropic', versions: ['5.5', '4.6', '4.5'], default: true };
const cat = { families: [{ id: 'opus', label: 'Opus', vendor: 'anthropic', versions: ['5.5'], default: false }, sonnet, { id: 'haiku', label: 'Haiku', vendor: 'anthropic', versions: ['5.5'], default: false }] };
eq('version row (spec: «последняя (5.5)» · «5.5» · «4.6»)', L.versionRow(sonnet, null, t).items.map((i) => i.text), ['последняя (5.5)', '5.5', '4.6', '4.5']);
eq('only the chosen version is on; pinned = lock', L.versionRow(sonnet, '4.6', t).items.map((i) => [i.on, i.pinned]), [[false, false], [false, false], [true, true], [false, false]]);
eq('"есть новее" only for an older pinned version', [L.versionRow(sonnet, '4.6', t).newer, L.versionRow(sonnet, '5.5', t).newer, L.versionRow(sonnet, null, t).newer], ['5.5', null, null]);
eq('"есть новее: 5.1" text', t('model.newer', { v: '5.1' }), 'есть новее: 5.1');
eq('model label: default family, latest version, pinned, unknown id, cli default', [L.modelLabel(null, cat), L.modelLabel({ family: 'sonnet', version: null }, cat), L.modelLabel({ family: 'sonnet', version: '4.6' }, cat), L.modelLabel({ family: 'my-model' }, cat), L.modelLabel({ family: 'default' }, cat)],
  ['Sonnet 5.5', 'Sonnet 5.5', 'Sonnet 4.6', 'my-model', '']);
eq('"Взять модель сильнее" = first family of the list that differs from the current (spec 171)', [L.strongerModel(cat, { family: 'sonnet' }), L.strongerModel(cat, { family: 'opus' }), L.strongerModel({ families: [] }, { family: 'x' })],
  [{ family: 'opus', version: null }, { family: 'sonnet', version: null }, null]);
const many = Array.from({ length: 9 }, (_, i) => ({ id: 'm' + i, label: 'Model ' + i, vendor: i < 5 ? 'anthropic' : 'openai' }));
eq('more than 8 models -> grouped by vendor with search', [L.needsGrouping(many), L.needsGrouping(many.slice(0, 8)), L.groupByVendor(many).map((x) => [x.vendor, x.families.length]), L.filterFamilies(many, 'model 7').map((f) => f.id)], [true, false, [['anthropic', 5], ['openai', 4]], ['m7']]);

// --- task: step strip, actions, push (stories 120, 125, 127, G01.7-8) ---
const full = [['plan', 'done'], ['spec', { skipped: 'в одну фразу' }], ['critique', 'done'], ['tests_first', 'done'], ['code', 'running'], ['gates', 'pending'], ['review', 'pending'], ['merge', 'pending']];
eq('strip with Quality: 7 steps, plan done despite skipped spec, code running', L.stepBar(task('s', 'working', { steps: full })).map((s) => s.key + ':' + s.state), ['plan:done', 'critique:done', 'tests:done', 'code:run', 'gates:pending', 'review:pending', 'merge:pending']);
eq('Quality off -> "Код · Ворота · Ревью · Влив"', L.stepBar(task('s', 'working', { quality: { on: false }, steps: full })).map((s) => s.key), ['code', 'gates', 'review', 'merge']);
eq('a skipped step is grey and keeps the reason for the tooltip', L.stepBar(task('s', 'working', { steps: [['plan', { skipped: 'в одну фразу' }], ['code', 'running']] })).filter((s) => s.state === 'skip'), [{ key: 'plan', reason: 'в одну фразу', state: 'skip' }]);
eq('a running step is "wait" while the task waits for you', L.stepBar(task('s', 'needs_human', { steps: full })).find((s) => s.key === 'code').state, 'wait');
eq('a failed step shows as fail', L.stepBar(task('s', 'failed', { steps: [['code', 'done'], ['gates', { failed: 'tests' }]] })).find((s) => s.key === 'gates').state, 'fail');
eq('"Повторить push": PR project -> retry, Push project -> push_base', [L.retryPushCommand({ finish: 'pr' }), L.retryPushCommand({ finish: 'push' })], ['retry', 'push_base']);
eq('permission buttons by project mode (spec: Push и PR / Только push / Не надо; Запушить / Не надо)', [L.outwardChoices({ finish: 'pr' }).map((c) => t(c[1])), L.outwardChoices({ finish: 'push' }).map((c) => t(c[1]))],
  [['Push и PR', 'Только push', 'Не надо'], ['Запушить', 'Не надо']]);
eq('actions fit the status', [L.taskActions(task('a', 'failed')).includes('retry'), L.taskActions(task('a', 'done')).includes('cancel'), L.taskActions(task('a', 'done_not_pushed'))[0], L.taskActions(task('a', 'handed_off'))[0],
  L.taskActions(task('a', 'queued', { start: 'night' }))[0]], [true, false, 'retry_push', 'take_back', 'start_now']);
eq('long lists show a cap and "ещё N" (200 files, 2000 lines)', [L.capList(Array(250).fill(0), 200).more, L.capList(Array(10).fill(0), 200).more, L.capList(Array(250).fill(0), 200).shown.length], [50, 0, 200]);
eq('role keys', ['worker', 'Planner', 'TestAuthor', 'test_author', 'chat'].map(L.roleKey), ['exec', 'plan', 'test', 'test', null]);
eq('duration', [L.formatDuration(14 * 60000, 'ru'), L.formatDuration(65 * 60000, 'ru'), L.formatDuration(5000, 'ru')], ['14 мин', '1 ч 05 мин', 'меньше минуты']);

// --- feed: paging by 200, folding of actions (stories 115, 123) ---
const m = (id, min, extra) => Object.assign({ id, at: new Date(NOW + min * 60000).toISOString(), who: 'system', text: '' }, extra);
eq('next page starts before the oldest loaded message', L.nextBefore([m('b', 5), m('a', 1), m('c', 9)]), new Date(NOW + 60000).toISOString());
eq('page size: full page -> maybe more; short page -> that is all', [L.hasMore(200, 200), L.hasMore(37, 200), L.MSG_LIMIT], [true, false, 200]);
eq('merge: no duplicates by id, oldest first', L.mergeMsgs([m('b', 5), m('a', 1)], [m('c', 3), m('b', 5)]).map((x) => x.id), ['a', 'c', 'b']);
const tool = (id, min, name, result, ok) => m(id, min, { tool: { name, arg: 'x', result, ok } });
const folded = L.groupFeed([m('h', 0, { who: 'human' }), tool('1', 1, 'edit', '+64', true), tool('2', 2, 'edit', '+41', true), tool('3', 3, 'run', '7 passed, 2 failed', false), m('r', 4, { who: { role: 'worker' } })]);
eq('actions fold into one row: "3 действия: edit, edit, run -> 7 passed, 2 failed"', [folded.map((x) => x.type), folded[1].count, folded[1].names, folded[1].result, folded[1].bad, L.plural(3, ['действие', 'действия', 'действий'], 'ru')], [['msg', 'tools', 'msg'], 3, ['edit', 'edit', 'run'], '7 passed, 2 failed', true, 'действия']);
eq('who: serde spellings', [L.whoOf({ who: 'Human' }), L.whoOf({ who: { Role: 'Worker' } }), L.whoOf({ who: { role: 'test_author' } })], [{ kind: 'human', role: null }, { kind: 'role', role: 'worker' }, { kind: 'role', role: 'testauthor' }]);

// --- sending (G05): empty, 100 000 / 100 001 characters, one id per attempt ---
eq('send: empty is refused, 100 000 passes, 100 001 does not', [L.validateSend('  \n').reason, L.validateSend('a'.repeat(100000)).ok, L.validateSend('a'.repeat(100001)).reason], ['empty', true, 'too_long']);
eq('send: characters, not UTF-16 units (one emoji = one character)', L.validateSend('😀'.repeat(100000)).ok, true);
let n = 0; const gate = L.createSendGate(() => 'id' + (++n));
const first = gate.begin('c1', 'hello');
eq('double press while the call runs creates no second message', gate.begin('c1', 'hello'), null);
gate.fail();
eq('retry of the same text after a failure reuses the id (the core ignores a repeat)', gate.begin('c1', 'hello'), first);
gate.fail();
eq('changed text gets a new id', gate.begin('c1', 'hello!') !== first, true);
gate.done();
eq('after success the next message gets a new id', gate.begin('c1', 'hello') !== first, true);
eq('draft key is per chat', [L.draftKey('claude', 'c1'), L.draftKey('claude', null), L.draftKey('codex', 'c1')], ['ns.draft.claude.c1', 'ns.draft.claude.new', 'ns.draft.codex.c1']);

// --- envs and the mock gate (R14i.2, G17.2-3) ---
eq('env states', [{ id: 'claude', installed: true }, { id: 'cursor', installed: false }, { id: 'codex', installed: true, needs_login: true }, { id: 'gemini', installed: true, error: 'cli broken' }, { id: 'custom:x', custom: true, configured: false }]
  .map((e) => L.normalizeEnv(e).state), ['ok', 'missing', 'login', 'unavailable', 'unset']);
eq('v1 list_agents row still reads', L.normalizeEnv({ id: 'claude', name: 'Claude Code', available: false, hint: 'npm i -g x' }), { id: 'claude', name: 'Claude Code', custom: false, state: 'missing', version: '', hint: 'npm i -g x', reason: '', kitDir: '' });
eq('dev mock loads only outside Tauri', [L.mockWanted({}), L.mockWanted({ __TAURI__: {} }), L.mockWanted({ __TAURI_INTERNALS__: {} })], [true, false, false]);

// --- i18n and helpers kept from the first UI ---
eq('elapsed mm:ss and h:mm:ss', [L.formatElapsed(185000), L.formatElapsed(4329000), L.formatElapsed(-5000)], ['3:05', '1:12:09', '0:00']);
const F = ['файл', 'файла', 'файлов'];
eq('ru plural 1/2/5/11/21/22', [1, 2, 5, 11, 21, 22].map((k) => L.plural(k, F, 'ru')), ['файл', 'файла', 'файлов', 'файлов', 'файл', 'файла']);
eq('en plural', [1, 2].map((k) => L.plural(k, ['file', 'files'], 'en')), ['file', 'files']);
const ru = Object.keys(NSI18n.dicts.ru), en = Object.keys(NSI18n.dicts.en);
eq('i18n: ru and en have the same keys', [ru.filter((k) => !en.includes(k)), en.filter((k) => !ru.includes(k))], [[], []]);
NSI18n.setLang('en'); eq('i18n en param + language switch', NSI18n.t('lim.at', { time: '19:00' }), 'resets at 19:00');
NSI18n.setLang('ru'); eq('i18n ru param', NSI18n.t('lim.at', { time: '19:00' }), 'сброс в 19:00');

// ===== task 15: swarm, factory, settings, kits, "while you were away" =====
// --- swarm size: 2..5, the -/+ buttons stop at the edges (G11.7, story 159) ---
eq('swarm size is clamped to 2..5 (1 -> 2, 6 -> 5, 3 stays, garbage -> default 3)', [1, 6, 3, 0, 99].map((v) => L.clampSwarmN(v)).concat(L.clampSwarmN('x'), L.clampSwarmN(2.7)), [2, 5, 3, 2, 5, 3, 3]);
eq('the stepper cannot go below 2 or above 5', [L.stepSwarmN(2, -1), L.stepSwarmN(5, 1), L.stepSwarmN(3, 1), L.stepSwarmN(3, -1)], [2, 5, 4, 2]);

// --- cost: shown only where the environment reports it (story 161, spec 30) ---
eq('cost format: $0.92, $8.10, $5.00, nothing when the env does not report, tiny amounts', [0.92, 8.1, 5, null, undefined, 0.004, 1234.5].map(L.formatCost), ['$0.92', '$8.10', '$5.00', '', '', '<$0.01', '$1234.50']);
eq('spent of cap (spec: "$8.10 из $20", "$1.84 из $5")', [L.formatBudget(8.1, 20), L.formatBudget(1.84, 5), L.formatBudget(7, 7.5)], ['$8.10 из $20', '$1.84 из $5', '$7.00 из $7.50']);

// --- reviewer's auto-pick timer: "Нет ответа 10 минут - беру A сам" (story 153) ---
eq('auto-pick countdown rounds up to whole minutes and vanishes once due', [L.autopickLeft(new Date(NOW + 6 * 60000).toISOString(), NOW), L.autopickLeft(new Date(NOW + 90000).toISOString(), NOW),
  L.autopickLeft(new Date(NOW - 1).toISOString(), NOW), L.autopickLeft(null, NOW)], [6, 2, null, null]);

// --- which variants are shown first, which is "Лучший", which may be taken (stories 153, 158, 160) ---
const V = (id, state, score, passed, total) => ({ id, state, score, gate: passed == null ? null : { passed, total } });
const rankIds = (r) => r.map((x) => x.id);
const mix = [V('C', 'failed', 4.2, 12, 14), V('B', 'passed', 7.1, 14, 14), V('A', 'passed', 8.6, 14, 14)];
eq('variants: passed by score first; the best passed one is flagged; failed ones cannot be taken', [rankIds(L.rankVariants(mix, null)), L.rankVariants(mix, null).map((x) => x.best), L.rankVariants(mix, null).map((x) => x.canTake)],
  [['A', 'B', 'C'], [true, false, false], [true, true, false]]);
eq('the reviewer advice wins over the score', L.rankVariants(mix, { variant: 'B' }).map((x) => x.best), [false, true, false]);
eq('running ones sit between passed and failed', rankIds(L.rankVariants([V('C', 'failed', 4.2, 12, 14), V('A', 'running', null, null, null), V('B', 'passed', 7.1, 14, 14)], null)), ['B', 'A', 'C']);
const allBad = L.rankVariants([V('A', 'failed', 3, 9, 14), V('B', 'failed', 4, 12, 14), V('C', 'failed', 5, 12, 14)], null);
eq('all failed -> the best attempt is the one with the most green tests (B and C both 12: first letter wins), nothing can be taken', [allBad.filter((x) => x.bestFailed).map((x) => x.id), allBad.some((x) => x.best), allBad.some((x) => x.canTake)],
  [['B'], false, false]);

// --- council: majority decides, a tie goes to the reviewer (story 155) ---
const vote = (voter, to) => ({ voter, for: to });
const t21 = L.tallyVotes([vote('A', 'B'), vote('B', 'B'), vote('C', 'A')]);
eq('council tally 2:1', [t21.counts.A, t21.counts.B, t21.total, t21.winner, t21.tie], [1, 2, 3, 'B', []]);
eq('council tie of three goes to the reviewer', L.tallyVotes([vote('A', 'A'), vote('B', 'B'), vote('C', 'C')]), { counts: { A: 1, B: 1, C: 1 }, total: 3, winner: null, tie: ['A', 'B', 'C'] });
eq('council tie of two, and no votes yet', [L.tallyVotes([vote('A', 'A'), vote('B', 'B')]).tie, L.tallyVotes([]).winner, L.tallyVotes([]).total], [['A', 'B'], null, 0]);

// --- factory: placing a bar on the time axis (the window is the last hour with a stretch ahead, story 168) ---
eq('a bar is placed in percent of the window and clipped at its edges; outside the window it is dropped', [L.placeBar(20, 50, 0, 100), L.placeBar(-10, 10, 0, 100), L.placeBar(90, 130, 0, 100), L.placeBar(120, 130, 0, 100), L.placeBar(-20, -5, 0, 100)],
  [{ left: 20, width: 30 }, { left: 0, width: 10 }, { left: 90, width: 10 }, null, null]);

// --- factory: the merge queue is a tube into main, one at a time (story 166) ---
const mt = (id, status, minAgo, extra) => task(id, status, Object.assign({ started_at: new Date(NOW - minAgo * 60000).toISOString() }, extra));
const q = L.mergeQueue([mt('w', 'working', 30), mt('rv', 'reviewing', 20), mt('m2', 'merging', 5), mt('m1', 'merging', 9), mt('bl', 'merge_blocked', 12), mt('d', 'done', 90)]);
eq('merge queue: the one merging longest is at the head, then the waiting ones, then blocked, then those still in review (pale)', [q.map((x) => x.id), q.map((x) => x.kind)], [['m1', 'm2', 'bl', 'rv'], ['head', 'wait', 'blocked', 'ghost']]);
eq('an empty queue is empty (the screen says "пусто")', L.mergeQueue([mt('w', 'working', 30), mt('d', 'done', 90)]), []);

// --- factory: the strip of roles is aggregated, not one row per agent (story 137, G12.2) ---
const run = (role, model, state, extra) => Object.assign({ role, model, state }, extra);
const live = [
  mt('a', 'working', 30, { runs: [run('planner', 'Opus 5.5', 'done'), run('coordinator', 'Opus 5.5', 'running'), run('worker', 'Opus 5.5', 'running'), run('reviewer', 'Opus 5.5', 'pending')] }),
  mt('b', 'working', 20, { runs: [run('worker', 'Sonnet 5.5', 'running')] }),
  mt('c', 'working', 10, { runs: [run('worker', 'Sonnet 5.5', 'running')] }),
  mt('e', 'merging', 40, { runs: [run('worker', 'Sonnet 5.5', 'done'), run('integrator', 'Sonnet 5.5', 'running')] }),
  mt('f', 'done', 90, { runs: [run('worker', 'Haiku 5.5', 'running')] })];
const strip = L.rolesStrip(live);
eq('strip: keys in the fixed order', strip.map((x) => x.key), ['coord', 'exec', 'rev', 'watch', 'queue']);
eq('strip: three executors at work, their models listed once each (a finished task does not count)', [strip[1].count, strip[1].state, strip[1].models], [3, 'work', ['Opus 5.5', 'Sonnet 5.5']]);
eq('strip: coordinator works; reviewer has nothing running -> idle with its model; watcher keeps watch; the queue counts one', [strip[0].state, strip[2].state, strip[2].models, strip[3].state, strip[4].count], ['work', 'idle', ['Opus 5.5'], 'watch', 1]);
eq('strip with no live tasks is empty (the screen says "Агентов нет")', L.rolesStrip([mt('d', 'done', 90)]), []);

// --- factory lanes: bars from the runs of a task, dashed "впереди" for the steps that are still to come (stories 162, 168) ---
const mAgo = (m) => new Date(NOW + m * 60000).toISOString();     // minutes relative to now
const relSegs = (t) => L.laneSegments(t, NOW).map((x) => [x.role, x.state, Math.round((x.from - NOW) / 60000), Math.round((x.to - NOW) / 60000)]);
const stepsRun = [['plan', 'done'], ['spec', { skipped: 'x' }], ['critique', 'done'], ['tests_first', 'done'], ['code', 'running'], ['gates', 'pending'], ['review', 'pending'], ['merge', 'pending']];
const laneTask = mt('lt', 'working', 20, { steps: stepsRun, runs: [run('planner', 'Opus 5.5', 'done', { started_at: mAgo(-20), finished_at: mAgo(-15) }), run('worker', 'Opus 5.5', 'running', { started_at: mAgo(-15) })] });
eq('lane: a finished run is a done bar, the running one grows to now, the steps ahead are projected 5 min each, dashed', relSegs(laneTask),
  [['plan', 'done', -20, -15], ['exec', 'run', -15, 0], ['gate', 'proj', 0, 5], ['rev', 'proj', 5, 10], ['integ', 'proj', 10, 15]]);
eq('lane: while the task waits for you its running bar is "wait"', relSegs(Object.assign({}, laneTask, { status: 'needs_human' })).filter((x) => x[0] === 'exec').map((x) => x[1]), ['wait']);
eq('lane: a failed run is "fail"; a finished task has nothing ahead', [relSegs(mt('x', 'failed', 30, { runs: [run('worker', 'm', 'failed', { started_at: mAgo(-10), finished_at: mAgo(-2) })] })), relSegs(mt('y', 'done', 30, { runs: [run('worker', 'm', 'done', { started_at: mAgo(-10), finished_at: mAgo(-2) })], steps: stepsRun }))],
  [[['exec', 'fail', -10, -2]], [['exec', 'done', -10, -2]]]);

eq('lane: a coordinator run is not a bar of the task (it belongs to the convoy or the swarm)', relSegs(mt('co', 'working', 30, { steps: stepsRun, runs: [run('coordinator', 'Opus 5.5', 'running', { started_at: mAgo(-5) }), run('worker', 'Opus 5.5', 'running', { started_at: mAgo(-15) })] })).map((x) => x[0]),
  ['exec', 'gate', 'rev', 'integ']);
// --- factory rows: at most 20 tasks on the screen, the rest "ещё N"; the last hour, scroll back for the past (story 168) ---
const kinds50 = ['needs_human', 'failed', 'working', 'queued', 'done'];
const fifty = Array.from({ length: 50 }, (_, i) => mt('t' + i, kinds50[i % 5], i % 5 === 4 ? 10 : 30 + i % 7, i % 5 === 4 ? { finished_at: mAgo(-5) } : {}));
const fr = L.factoryRows(fifty, NOW, {});
eq('50 tasks: 20 on the screen, "ещё 30"', [fr.rows.length, fr.more], [20, 30]);
eq('rows are ordered: waiting for you first (10 of them), then failed (10) - so the 20 on screen are exactly those', [fr.rows.filter((r) => r.kind === 'wait').length, fr.rows.filter((r) => r.kind === 'fail').length, fr.rows[0].kind], [10, 10, 'wait']);
eq('5 tasks: all shown, nothing more', [L.factoryRows(fifty.slice(0, 5), NOW, {}).rows.length, L.factoryRows(fifty.slice(0, 5), NOW, {}).more], [5, 0]);
const old = mt('old', 'done', 200, { finished_at: mAgo(-180) });
eq('a task that finished 3 hours ago is not in the last hour but appears after scrolling back', [L.factoryRows([old], NOW, {}).rows.length, L.factoryRows([old], NOW, { offsetMin: 180 }).rows.length], [0, 1]);

// --- factory header: "Работают X из N", "Сегодня $ из $" ---
const stat = [mt('s1', 'working', 30, { cost_usd: 2, runs: [run('worker', 'a', 'running'), run('worker', 'b', 'running')] }), mt('s2', 'working', 20), mt('s3', 'queued', 5),
  mt('s4', 'done', 100, { cost_usd: 1.84, finished_at: mAgo(-60) }), mt('s5', 'done', 60 * 30, { cost_usd: 3, finished_at: mAgo(-60 * 29) })];
const fs = L.factoryStats(stat, NOW);
eq('stats: 2 running swarm agents + 1 working task = 3 at work; spent today = 2 + 1.84 (yesterday\'s 3 is not counted); the env reports cost', [fs.working, fs.spent, fs.reportsCost], [3, 3.84, true]);
eq('stats: an env that reports no cost at all', [L.factoryStats([mt('s2', 'working', 20)], NOW).spent, L.factoryStats([mt('s2', 'working', 20)], NOW).reportsCost], [null, false]);

// --- the factory feed: last 500 events, "Все / Проблемы" with a counter, nudge only where it makes sense (stories 164, 168) ---
const ev = (id, min, kind, extra) => Object.assign({ task_id: id, at: mAgo(min), kind, text: kind + min }, extra);
const events = [ev('a', -5, 'stage'), ev('a', -4, 'tool'), ev('a', -3, 'error'), ev('a', -2, 'check', { role: 'verifier' }), ev('a', -1, 'agent', { role: 'watcher', text: 'тишина 15 мин, толкнул' })];
const fe = L.feedEntries(events, [mt('a', 'working', 30)], NOW);
eq('feed: newest first, tool noise is dropped', fe.map((e) => e.text), ['тишина 15 мин, толкнул', 'check-2', 'error-3', 'stage-5']);
eq('feed: an error is a problem; the watcher is a problem with "Подтолкнуть" while the task lives; a plain check is neither', [fe[0].level, fe[0].acts, fe[1].level, fe[2].level], ['bad', true, null, 'bad']);
eq('feed: no nudge on a finished task', L.feedEntries([ev('a', -1, 'agent', { role: 'watcher' })], [mt('a', 'done', 30)], NOW)[0].acts, false);
eq('feed: a task that waits for you adds a top line with its question (amber, counted as a problem)', L.feedEntries([], [mt('q', 'needs_human', 5, { question: { text: 'Оставить ?page=?', options: [] } })], NOW).map((e) => [e.level, e.text, e.taskId]), [['warn', 'Оставить ?page=?', 'q']]);
const fz = L.filterFeed(L.feedEntries(events, [mt('a', 'working', 30)], NOW), 'problems');
eq('feed filter "Проблемы": only problems, and the counter matches', [fz.entries.map((e) => e.level), fz.count, L.filterFeed(fe, 'all').entries.length], [['bad', 'bad'], 2, 4]);
const manyEv = Array.from({ length: 600 }, (_, i) => ev('a', -i, 'stage', { text: 'e' + i }));
const capped = L.feedEntries(manyEv, [mt('a', 'working', 30)], NOW);
eq('feed keeps the last 500 events', [capped.length, capped[0].text, capped[499].text], [500, 'e0', 'e499']);

// --- a convoy ("Поручение") shows overall progress "3 из 5" (story 163) ---
const cv = L.convoyProgress([task('1', 'done'), task('2', 'done'), task('3', 'done'), task('4', 'working'), task('5', 'needs_human')]);
eq('convoy progress', [cv.done, cv.total, cv.parts, t('convoy.progress', { done: cv.done, total: cv.total })], [3, 5, ['d', 'd', 'd', 'r', 'w'], '3 из 5']);

// --- "Пока тебя не было": rows by importance, more than 10 -> "ещё N", nothing happened -> no summary (stories 175-176) ---
const sd = (i) => ({ task_id: 'd' + i, env: 'claude', title: 'Готово ' + i, commit: 'abc' + i });
const bigSummary = { done: Array.from({ length: 14 }, (_, i) => sd(i)), failed: [{ task_id: 'f1', env: 'codex', title: 'Упала', reason: 'tests' }], waiting: [{ task_id: 'w1', env: 'claude', title: 'Ждёт' }],
  spent: [{ env: 'claude', usd: 8.1 }, { env: 'codex', usd: 3.2 }], limits: [{ env: 'claude', window: win('five_hour', 100, null, { exhausted: true }) }] };
const sr = L.summaryRows(bigSummary, 10);
eq('summary: 17 event rows -> 10 shown, "ещё 7"; what needs you comes first, then failures, then the finished', [sr.rows.length, sr.more, sr.rows[0].kind, sr.rows[1].kind, sr.rows[2].kind, t('sum.more', { n: sr.more })], [10, 7, 'wait', 'fail', 'done', 'ещё 7']);
eq('summary: the money line is the sum over envs that reported (8.10 + 3.20)', sr.spent, 11.3);
eq('summary: ten rows exactly -> no "ещё"', L.summaryRows({ done: Array.from({ length: 10 }, (_, i) => sd(i)) }, 10).more, 0);
eq('summary: nothing happened (money alone is not an event) -> empty, no banner', [L.summaryRows({}, 10).empty, L.summaryRows({ spent: [{ env: 'claude', usd: 1 }] }, 10).empty, L.summaryRows(null, 10).empty, sr.empty], [true, true, true, false]);

// --- when the banner appears: absence of 30+ minutes, counted from losing focus or from sleep (stories 175-176) ---
const am = (m) => NOW + m * 60000;
eq('away: left 31 min ago -> show, since the moment of leaving; 29 min -> no', [L.awayDecision({ leftAt: am(-31), lastAlive: am(-31), now: NOW, afterMin: 30 }), L.awayDecision({ leftAt: am(-29), lastAlive: am(-29), now: NOW, afterMin: 30 }).show],
  [{ show: true, since: am(-31) }, false]);
eq('away: the computer slept while the window had focus (the heartbeat stopped an hour ago) -> counts from the last heartbeat', L.awayDecision({ leftAt: null, lastAlive: am(-60), now: NOW, afterMin: 30 }), { show: true, since: am(-60) });
eq('away: a fresh heartbeat and focus -> nothing', L.awayDecision({ leftAt: null, lastAlive: am(-0.5), now: NOW, afterMin: 30 }).show, false);
eq('away: the threshold is a setting', L.awayDecision({ leftAt: am(-12), lastAlive: am(-12), now: NOW, afterMin: 10 }).show, true);

// --- a chat of a swarm opens the swarm screen (story 156) ---
const swt = (id, status, extra) => task(id, status, Object.assign({ swarm: { mode: 'race', n: 3 } }, extra));
eq('swarm chat: the running swarm task wins over an older finished one; plain tasks are ignored; none -> null', [L.swarmTaskOf([swt('old', 'done'), swt('run', 'working'), task('plain', 'working')]).id, L.swarmTaskOf([task('plain', 'working')]), L.swarmTaskOf([])], ['run', null, null]);

// --- settings: eleven sections in the order of story 194 ---
eq('settings: current env first (Модели, Наборы, Права и песочница, Запуск и вход), then "Для всех сред", then "Проекты" - 11 in all', [L.SETTINGS_SECTIONS.length, L.SETTINGS_SECTIONS.map((x) => x.group + ':' + x.id)],
  [11, ['env:models', 'env:kits', 'env:perms', 'env:run', 'all:quality', 'all:swarm', 'all:limits', 'all:notify', 'all:look', 'all:keys', 'projects:projects']]);

// --- settings: a wrong value gives a hint and is not applied (story 196) ---
const intRule = { min: 1, max: 10, int: true };
eq('number: in range is applied (spaces are fine)', [L.validateNumber('5', intRule), L.validateNumber(' 4 ', intRule)], [{ ok: true, value: 5 }, { ok: true, value: 4 }]);
eq('number: out of range, not a number, empty, a fraction where an integer is needed', ['0', '11', 'abc', '', '2.5'].map((v) => L.validateNumber(v, intRule).why), ['range', 'range', 'nan', 'nan', 'nan']);
eq('number: money accepts a decimal point or comma', [L.validateNumber('7,5', { min: 0.5, max: 1000 }).value, L.validateNumber('7.5', { min: 0.5, max: 1000 }).value, L.validateNumber('0', { min: 0.5, max: 1000 }).why], [7.5, 7.5, 'range']);
eq('time: 23:00 is fine, 8:05 is normalised, 24:00 / 12:60 / text are not', [L.validateTime('23:00'), L.validateTime('8:05'), L.validateTime('24:00').why, L.validateTime('12:60').why, L.validateTime('ночью').why], [{ ok: true, value: '23:00' }, { ok: true, value: '08:05' }, 'format', 'format', 'format']);
eq('extra arguments are split without a shell: quotes group, a backslash escapes', [L.parseArgs('--model "a b" -x'), L.parseArgs("--name 'it is'"), L.parseArgs('a\\ b'), L.parseArgs('a"b c"d'), L.parseArgs('  ')],
  [{ args: ['--model', 'a b', '-x'] }, { args: ['--name', 'it is'] }, { args: ['a b'] }, { args: ['ab cd'] }, { args: [] }]);
eq('extra arguments: a quote that is never closed is an error (the field keeps what was typed)', [L.parseArgs('--x "oops'), L.parseArgs("--y 'oops")], [{ error: 'quotes' }, { error: 'quotes' }]);
eq('extra arguments shown back with quotes where needed', L.formatArgs(['--model', 'a b', '-x']), '--model "a b" -x');

// --- hotkeys: captured from the keyboard, a taken combination is refused (stories 196, 185) ---
eq('a key press becomes an accelerator in the order Ctrl, Alt, Shift, Cmd; a lone modifier is nothing yet', [L.accelFromEvent({ code: 'KeyN', key: 'ñ', altKey: true, metaKey: true }, 'mac'), L.accelFromEvent({ code: 'Period', key: '.', altKey: true, ctrlKey: true }, 'linux'),
  L.accelFromEvent({ code: 'ControlLeft', key: 'Control', ctrlKey: true }, 'linux'), L.accelFromEvent({ code: 'Enter', key: 'Enter' }, 'mac'), L.accelFromEvent({ code: 'Digit1', key: '1', ctrlKey: true }, 'linux')],
  ['Alt+Cmd+N', 'Ctrl+Alt+.', null, 'Enter', 'Ctrl+1']);
eq('accelerator check: fine / taken by another action / no modifier / reserved by the OS / broken', [L.validateAccel('Alt+Cmd+N', { taken: ['Alt+Cmd+.'], needModifier: true }).ok, L.validateAccel('alt+cmd+.', { taken: ['Alt+Cmd+.'], needModifier: true }).why, L.validateAccel('N', { needModifier: true }).why,
  L.validateAccel('Cmd+Q', { needModifier: true }).why, L.validateAccel('Alt+Cmd+', { needModifier: true }).why, L.validateAccel('Enter', { needModifier: false }).ok], [true, 'busy', 'format', 'busy', 'format', true]);
eq('key caps: mac symbols, Linux words, Enter as the return sign', [L.keyLabel('Alt+Cmd+N', 'mac'), L.keyLabel('Ctrl+Alt+N', 'linux'), L.keyLabel('Enter', 'mac')], [['⌥', '⌘', 'N'], ['Ctrl', 'Alt', 'N'], ['↵']]);

// --- settings: models per role, pinned versions, "есть новее" (story 194, spec 18) ---
eq('role choice <-> select value round trip; empty value = inherit', [L.choiceKey({ family: 'opus', version: null }), L.choiceKey({ family: 'sonnet', version: '4.6' }), L.choiceFromKey('sonnet|4.6'), L.choiceFromKey('opus|'), L.choiceFromKey('')],
  ['opus|', 'sonnet|4.6', { family: 'sonnet', version: '4.6' }, { family: 'opus', version: null }, null]);
eq('role options: inherit, then every family as "последняя" and by version; a custom model that is chosen is added', [L.roleOptions(cat, t, 'set.inherit', null).map((o) => o.text), L.roleOptions(cat, t, 'set.inherit', { family: 'my-model', version: null }).slice(-1)[0]],
  [['как у исполнителя', 'Opus · последняя', 'Opus 5.5', 'Sonnet · последняя', 'Sonnet 5.5', 'Sonnet 4.6', 'Sonnet 4.5', 'Haiku · последняя', 'Haiku 5.5'], { value: 'my-model|', text: 'my-model' }]);
eq('"есть новее" in settings: an older pinned version shows the newest, the latest or unpinned shows nothing', [L.pinNewer(cat, { family: 'sonnet', version: '4.6' }), L.pinNewer(cat, { family: 'sonnet', version: '5.5' }), L.pinNewer(cat, { family: 'sonnet', version: null }), L.pinNewer(cat, null)], ['5.5', null, null, null]);

// --- settings: "Сбросить раздел" drops exactly the keys of that section; the core fills them with its defaults (story 199) ---
eq('reset: which keys go per section (Лимиты, Рой и фабрика, Права, Модели)', [L.resetPatch('limits'), L.resetPatch('swarm'), L.resetPatch('perms'), L.resetPatch('models')],
  [{ settings: ['limits'], env: [] }, { settings: ['max_parallel', 'swarm_default', 'attempts', 'stall_min', 'autopick_min'], env: [] }, { settings: [], env: ['ask_push', 'ask_pr', 'protected_paths'] },
    { settings: [], env: ['default_model', 'roles', 'fallback', 'custom_models'] }]);
eq('Наборы and Проекты have nothing to reset', [L.resetPatch('kits'), L.resetPatch('projects')], [null, null]);

// --- kits: "Что изменится" tells + (added), ~ (replaced, old goes to a snapshot) and = (already there) apart (story 206); forms of core::kits ---
const ent = (change, group, label, extra) => Object.assign({ key: group + ':' + label, change, group, label }, extra);
const held = (kind, key, extra) => ent('add', 'hooks', key, { key, held: Object.assign({ kind, event: null, text: 'x', scripts: [] }, extra) });
const plan = { entries: [ent('same', 'skills', 'code-review'), ent('add', 'skills', 'api-docs'), ent('replace', 'rules', 'CLAUDE.md', { diff: '-a\n+b' }), ent('add', 'mcp', 'sentry'), ent('add', 'skills', 'changelog'), ent('same', 'skills', 'release-notes'),
  held('hook', 'h1'), held('hook', 'h2'), held('scripted_skill', 's1'), held('mcp', 'm1')] };
const gc = L.groupChanges(plan.entries.slice(0, 6));
eq('changes: added first, then replaced, then already-there; names of one group on one line', gc.map((x) => [x.sign, x.group, x.names]),
  [['+', 'skills', ['api-docs', 'changelog']], ['+', 'mcp', ['sentry']], ['~', 'rules', ['CLAUDE.md']], ['=', 'skills', ['code-review', 'release-notes']]]);
eq('changes: a replaced line carries its diff as lines for "разница", the others do not', gc.map((x) => x.diff), [null, null, ['-a', '+b'], null]);

// --- kits: nothing is installed without an explicit choice, hooks stay off until each is ticked (stories 206, 208) ---
eq('"Установить" waits for a choice of place; and for something to change', [L.canInstall(plan, null), L.canInstall(plan, 'global'), L.canInstall(plan, { project: '/p' }), L.canInstall({ entries: [ent('same', 'skills', 'x')] }, 'global'), L.canInstall({ entries: [] }, 'global')],
  [false, true, true, false, false]);
eq('held entries are the ones that run code', L.heldEntries(plan).map((x) => x.key), ['h1', 'h2', 's1', 'm1']);
eq('only the keys the person ticked are switched on (plan order); unknown keys are ignored; nothing ticked -> nothing on', [L.enableKeys(plan, new Set(['m1', 'h2', 'zzz'])), L.enableKeys(plan, new Set())], [['h2', 'm1'], []]);

// --- kits: a clear refusal for a bad kit (story 212) and for another env's kit (story 210); KitError arrives as {kind, ...} ---
eq('refusal: kind and detail; the wrong-env one names both envs; JSON text works too; unknown text is kept whole', [L.kitRefusal({ kind: 'wrong_env', kit_env: 'codex', kit_env_name: 'Codex', target_env: 'claude', target_env_name: 'Claude Code' }),
  L.kitRefusal('{"kind":"bad_zip"}'), L.kitRefusal({ kind: 'file_too_large', path: 'hooks/big.bin', size: 11 }), L.kitRefusal({ kind: 'not_applied', snapshot: 's', cli: 'claude', keys: ['hooks'] }), L.kitRefusal('что-то странное')],
  [{ key: 'wrong_env', detail: 'Codex', env: 'codex', target: 'Claude Code' }, { key: 'bad_zip', detail: '' }, { key: 'file_too_large', detail: 'hooks/big.bin' }, { key: 'not_applied', detail: 'claude', keys: ['hooks'] }, { key: 'unknown', detail: 'что-то странное' }]);

// --- kits: export boxes - an empty group is off, a group whose layout is not verified is off with its own label (stories 205, 204) ---
const gs = [{ group: 'skills', available: true, active: true, count: 42 }, { group: 'rules', available: true, active: true, count: 2 }, { group: 'hooks', available: true, active: false, count: 0 },
  { group: 'mcp', available: false, active: false, note: 'layout_unverified', count: 6 }];
eq('export group state', gs.map((g) => L.kitGroupOff(g)), [null, null, 'empty', 'unverified']);
eq('export selection: ticked and available only', L.exportSelection(gs, new Set(['skills', 'hooks', 'mcp'])), ['skills']);

// --- kits: what the person typed in "Откуда" says whether it is a repository, a zip or a folder (story 206) ---
eq('kit source in the core form: link or ssh -> repo, .zip -> zip, anything else -> folder (spaces around are ignored)', ['https://github.com/someone/my-setup', 'git@github.com:someone/my-setup.git', 'ssh://git@host/x.git', ' /Users/x/codex-setup.zip ', '/Users/x/kit', '~/kit'].map(L.kitSourceOf),
  [{ repo: 'https://github.com/someone/my-setup' }, { repo: 'git@github.com:someone/my-setup.git' }, { repo: 'ssh://git@host/x.git' }, { zip: '/Users/x/codex-setup.zip' }, { folder: '/Users/x/kit' }, { folder: '~/kit' }]);
eq('kit source: nothing typed is nothing', L.kitSourceOf('   '), null);

// --- the race of a swarm: shared preparation, a lane per variant with its own model, "Выбор" (story 160) ---
const SW0 = NOW - 20 * 60000;                                   // the swarm started 20 minutes ago
const sm = (m) => new Date(SW0 + m * 60000).toISOString();
const rr = (role, variantId, model, state, a, b) => ({ role, variant: variantId, model, state, started_at: sm(a), finished_at: b == null ? null : sm(b) });
const swarmRuns = [rr('planner', null, 'Opus 5.5', 'done', 0, 2), rr('critic', null, 'Sonnet 5.5', 'done', 2, 4), rr('test_author', null, 'Sonnet 5.5', 'done', 4, 7),
  rr('worker', 'A', 'Opus 5.5', 'done', 7, 16), rr('verifier', 'A', 'pytest', 'done', 16, 17), rr('worker', 'B', 'Sonnet 5.5', 'done', 7, 14), rr('verifier', 'B', 'pytest', 'done', 14, 15),
  rr('worker', 'C', 'Sonnet 5.5', 'done', 7, 18), rr('verifier', 'C', 'pytest', 'failed', 18, 19)];
const sl = L.swarmLanes(task('sw', 'working', { swarm: { mode: 'race', n: 3 }, runs: swarmRuns }), NOW);
const relM = (ms) => Math.round((ms - SW0) / 60000);
eq('race lanes: preparation, one lane per variant, the choice - in this order', sl.lanes.map((l) => l.key), ['prep', 'A', 'B', 'C', 'choice']);
eq('race lanes: each variant lane carries its own model; the executor bar is followed by the gate mark (ok / fail)', [sl.lanes[1].model, sl.lanes[2].model, sl.lanes[1].segs.map((x) => [x.role, x.state, relM(x.from), relM(x.to)]), sl.lanes[3].segs.map((x) => x.state)],
  ['Opus 5.5', 'Sonnet 5.5', [['exec', 'done', 7, 16], ['gate', 'ok', 16, 17]], ['done', 'fail']]);
eq('race lanes: the shared preparation is plan, critic, tests', sl.lanes[0].segs.map((x) => [x.role, relM(x.from), relM(x.to)]), [['plan', 0, 2], ['crit', 2, 4], ['test', 4, 7]]);
eq('race lanes: the reviewer has not started - the comparison and the merge are dashed, 2 minutes each, from now', sl.lanes[4].segs.map((x) => [x.role, x.state, relM(x.from), relM(x.to)]), [['rev', 'proj', 20, 22], ['integ', 'proj', 22, 24]]);
eq('race lanes: the axis runs from the first bar to the last (projected) one', [relM(sl.t0), relM(sl.t1)], [0, 24]);
const councilRuns = [rr('council', 'A', 'Opus 5.5', 'done', 0, 5), rr('council', 'B', 'Sonnet 5.5', 'done', 0, 4)];
eq('council lanes carry the model of the member even without an executor run', L.swarmLanes(task('cn', 'working', { swarm: { mode: 'council', n: 2 }, runs: councilRuns }), NOW).lanes.filter((l) => l.kind === 'variant').map((l) => [l.key, l.model]), [['A', 'Opus 5.5'], ['B', 'Sonnet 5.5']]);

// --- every section, kit group, criterion and refusal has words in both languages (stories 194, 205, 212) ---
const need = [].concat(L.SETTINGS_SECTIONS.map((x) => 'set.nav.' + x.id), L.KIT_GROUPS.map((g) => 'kit.g.' + g), ['correctness', 'coverage', 'simplicity', 'risk'].map((k) => 'crit.' + k),
  ['not_a_kit', 'bad_zip', 'unsafe_path', 'symlink', 'file_too_large', 'kit_too_large', 'too_many_files', 'corrupt', 'wrong_env', 'clone_failed', 'unknown_env', 'dest_not_empty', 'not_found', 'changed', 'bad_name', 'bad_value', 'interrupted', 'io', 'not_applied'].map((k) => 'kit.err.' + k));
eq('labels exist in ru and en', need.filter((k) => !(k in NSI18n.dicts.ru) || !(k in NSI18n.dicts.en)), []);
eq('the wrong-env refusal reads as in the spec (story 210)', t('kit.err.wrong_env', { detail: 'Codex', env: 'Claude Code' }), 'Это набор Codex. В Claude Code он не ставится: у каждой среды свои скиллы и правила.');

const total = out.length;
if (typeof document !== 'undefined') {
  document.getElementById('out').textContent = out.join('\n') + `\n\n${bad ? 'FAILED ' + bad : 'ALL PASSED'}: ${total - bad} of ${total}`;
  document.title = bad ? 'FAILED' : 'PASSED';
} else console.log(out.join('\n') + `\n${bad ? 'FAILED ' + bad : 'ALL PASSED'}: ${total - bad} of ${total}`);
