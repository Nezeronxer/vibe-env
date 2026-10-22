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

// ================= v3: live feed, markdown, diff, tabs, keys, first run (stories 225-242) =================
{   // own block: names of the v3 checks do not clash with the ones above
const V = NSLive;
// --- markdown (story 228): bold, italic, headings, lists, `code`, fenced blocks, only http(s) links ---
eq('md: bold and italic inside a paragraph', V.parseMarkdown('**жирный** и *курсив*'),
  [{ type: 'p', inline: [{ t: 'b', c: [{ t: 'text', s: 'жирный' }] }, { t: 'text', s: ' и ' }, { t: 'i', c: [{ t: 'text', s: 'курсив' }] }] }]);
eq('md: a fenced block keeps its language and text as is, ** inside is not bold', V.parseMarkdown('До:\n```py\nx = 2 ** 3\n```'),
  [{ type: 'p', inline: [{ t: 'text', s: 'До:' }] }, { type: 'code', lang: 'py', text: 'x = 2 ** 3' }]);
eq('md: "- " and "1." lists, heading levels # to ###', V.parseMarkdown('## План\n- один\n- `два`\n\n1. первый\n2. второй').map((b) => [b.type, b.level || b.start || null, (b.items || []).length]),
  [['h', 2, 0], ['ul', null, 2], ['ol', 1, 2]]);
eq('md: inline code is one node, its stars stay', V.parseInline('см. `a**b`'), [{ t: 'text', s: 'см. ' }, { t: 'code', s: 'a**b' }]);
eq('md: a javascript: link is not a link, only its text stays', V.parseInline('[жми](javascript:alert(1))').filter((n) => n.t === 'a'), []);
eq('md: https link and a bare https address become links, trailing dot is not part of it', V.parseInline('[доки](https://a.dev/x) и https://b.dev/y.').filter((n) => n.t === 'a').map((n) => n.href),
  ['https://a.dev/x', 'https://b.dev/y']);
eq('md: safeHref accepts only http and https', ['https://x.io', 'http://x.io', 'javascript:alert(1)', 'file:///etc/passwd', ' data:text/html,x'].map(V.safeHref), ['https://x.io', 'http://x.io', null, null, null]);
eq('md: a table stays as text', V.parseMarkdown('| a | b |\n|---|---|\n| 1 | 2 |').map((b) => b.type), ['table']);

// --- highlight (story 226): by extension, own light tokens ---
eq('hl: language from the file extension', ['src/a.rs', 'x.tsx', 'b.py', 'Cargo.toml', 'run.sh', 'Makefile', 'README.md'].map(V.langOf), ['rs', 'js', 'py', 'toml', 'sh', '', 'md']);
eq('hl: js keyword, string, comment', V.tokenize('let x = "a" // c', 'js'), [{ k: 'kw', s: 'let' }, { k: '', s: ' x = ' }, { k: 'str', s: '"a"' }, { k: '', s: ' ' }, { k: 'com', s: '// c' }]);
eq('hl: a keyword inside a longer word is not a keyword', V.tokenize('outlet', 'js'), [{ k: '', s: 'outlet' }]);
eq('hl: unknown language is plain text', V.tokenize('fn x', ''), [{ k: '', s: 'fn x' }]);

// --- unified diff -> rows with line numbers (story 226) ---
eq('diff: hunk header sets the numbers, - takes the old one, + the new one', V.parseDiff('--- a/x.py\n+++ b/x.py\n@@ -10,3 +10,4 @@ def f\n ctx\n-old\n+new1\n+new2\n ctx2'),
  [{ kind: 'hunk', old: null, new: null, text: '@@ -10,3 +10,4 @@ def f' }, { kind: 'ctx', old: 10, new: 10, text: 'ctx' }, { kind: 'del', old: 11, new: null, text: 'old' },
    { kind: 'add', old: null, new: 11, text: 'new1' }, { kind: 'add', old: null, new: 12, text: 'new2' }, { kind: 'ctx', old: 12, new: 13, text: 'ctx2' }]);
eq('diff: counts of added and removed lines', V.diffStat(V.parseDiff('@@ -1 +1,2 @@\n-a\n+b\n+c')), { add: 2, del: 1 });

// --- folding long output (stories 225, 229) ---
const lines10 = Array.from({ length: 10 }, (_, i) => 'l' + (i + 1)).join('\n') + '\n';
eq('fold: 10 lines of output -> 8 shown, 2 hidden, nothing cut', (({ lines, hidden, cut }) => [lines.length, lines[7], hidden, cut])(V.foldLines(lines10, 8, 2000)), [8, 'l8', 2, 0]);
const lines2500 = Array.from({ length: 2500 }, (_, i) => String(i)).join('\n');
eq('fold: opened 2500 lines -> 2000 shown, 500 cut ("full - in the task log")', (({ lines, hidden, cut }) => [lines.length, hidden, cut])(V.foldLines(lines2500, 2000, 2000)), [2000, 0, 500]);
eq('fold: empty output -> nothing', V.foldLines('', 8, 2000), { lines: [], hidden: 0, cut: 0, total: 0 });

// --- gluing reads of one role (story 229) ---
const A = (key, role, kind, extra) => ({ key, role, action: { id: key, name: { read: 'Read', search: 'Grep', edit: 'Edit' }[kind], input: Object.assign({ kind }, extra), done: true, ok: true } });
const glued = V.groupActions([A('1', 'exec', 'read', { path: 'a' }), A('2', 'exec', 'read', { path: 'b' }), A('3', 'exec', 'search', { pattern: 'x' }), A('4', 'exec', 'edit', { path: 'c' }), A('5', 'exec', 'read', { path: 'd' }), A('6', 'rev', 'read', { path: 'e' })]);
eq('glue: three reads/searches in a row of one role -> one row; a lone read stays a line; another role breaks the run', glued.map((g) => (g.type === 'group' ? 'G' + g.items.length : g.item.key)), ['G3', '4', '5', '6']);
eq('glue: the row counts files and searches', V.readSummary(glued[0].items), { files: 2, searches: 1 });

// --- ToolAction shapes: serde kind tag in any case, the legacy ChatMsg.tool -> a line too ---
eq('action input: {kind:"Read"} and {"Read":{..}} read the same', [V.inputOf({ input: { kind: 'Read', path: 'a' } }), V.inputOf({ input: { Read: { path: 'a' } } })], [{ kind: 'read', path: 'a' }, { kind: 'read', path: 'a' }]);
eq('legacy tool row -> an action: run -> bash with its result as output', V.toolToAction({ name: 'run', arg: 'pytest', result: '9 failed', ok: false }, 'm1'),
  { id: 'm1', name: 'Bash', input: { kind: 'bash', command: 'pytest' }, output: '9 failed', ok: false, done: true });

// --- the live line: verb by the last action (story 227), the plan window (story 242), typing (story 227) ---
const act = (kind, done) => ({ input: { kind }, done });
eq('verb: during Bash - running, after Edit - writing code, after Read - thinking, while Read - reading, streaming text - answering, nothing - thinking',
  [V.actionVerb(act('bash', false)), V.actionVerb(act('edit', true)), V.actionVerb(act('read', true)), V.actionVerb(act('read', false)), V.actionVerb(null, true), V.actionVerb(null)],
  ['live.running', 'live.writing', 'live.thinking', 'live.reading', 'live.answering', 'live.thinking']);

// --- F3: live feed without doubles, live gluing, verb by role, paragraphs of a result card ---
const acts1 = new Map();
V.upsertAction(acts1, 't1', { id: 'r1', name: 'Read', input: { kind: 'read', path: 'a' }, done: false, ok: false }, 10, 'exec');
V.upsertAction(acts1, 'chat:c1', { id: 'r1', name: 'Read', input: { kind: 'read', path: 'a' }, done: false, ok: false }, 11, 'chat');
V.upsertAction(acts1, 't1', { id: 'r1', name: 'Read', input: { kind: 'read', path: 'a' }, output: 'x\ny', done: true, ok: true }, 20, 'exec');
V.upsertAction(acts1, 'chat:c1', { id: 'r1', name: 'Read', input: { kind: 'read', path: 'a' }, output: 'x\ny', done: true, ok: true }, 21, 'chat');
eq('merge: start and result of one action, from task-action and its chat-action mirror -> one row of the task, done, with output',
  [...acts1.values()].map((e) => [e.key, e.taskId, e.role, e.at, e.action.done, e.action.output]), [['t1:r1', 't1', 'exec', 10, true, 'x\ny']]);
const acts2 = new Map();
V.upsertAction(acts2, 'chat:c1', { id: 'e1', name: 'Edit', input: { kind: 'edit', path: 'a', diff: '' }, done: false, ok: false }, 5, 'chat');
V.upsertAction(acts2, 't1', { id: 'e1', name: 'Edit', input: { kind: 'edit', path: 'a', diff: '' }, output: 'ok', done: true, ok: true }, 9, 'exec');
V.upsertAction(acts2, 't1', { id: 'e1', name: 'Edit', input: { kind: 'edit', path: 'a', diff: '' }, done: false, ok: false }, 12, 'exec');
V.upsertAction(acts2, 'chat:c1', { id: 'q1', name: 'Grep', input: { kind: 'search', pattern: 'x' }, done: true, ok: true }, 13, 'chat');
eq('merge: the chat mirror first, the task copy later -> moved under the task; a late start never undoes a result; a chat-only action stays the chat\'s',
  [...acts2.values()].map((e) => [e.key, e.taskId, e.role, e.at, e.action.done]), [['t1:e1', 't1', 'exec', 5, true], ['chat:c1:q1', null, 'chat', 13, true]]);
const Ra = (key, done) => ({ key, role: 'exec', action: { id: key, name: 'Read', input: { kind: 'read', path: key }, done, ok: done } });
eq('glue live: a read that has only started (done=false, ok=false) joins the run, the row grows in place', V.groupActions([Ra('1', true), Ra('2', false)]).map((g) => (g.type === 'group' ? 'G' + g.items.length : g.item.key)), ['G2']);
eq('verb by role: reviewer and critic check or read, "writing code" only after an Edit/Write of the executor',
  [V.actionVerb(act('edit', true), false, { role: 'rev', lastRole: 'exec' }), V.actionVerb(null, false, { role: 'crit' }), V.actionVerb(act('read', false), false, { role: 'rev', lastRole: 'rev' }),
    V.actionVerb(act('bash', false), false, { role: 'rev', lastRole: 'rev' }), V.actionVerb(act('read', true), false, { role: 'rev', lastRole: 'rev' }), V.actionVerb(act('write', true), false, { role: 'exec', lastRole: 'exec' })],
  ['live.reviewing', 'live.reviewing', 'live.reading', 'live.running', 'live.reviewing', 'live.writing']);
const sumMd = V.parseMarkdown('Тест проверяет один случай.\n\n**Что ждёт меня:** ничего.\n**Что изменено:** `calc.py`.');
eq('md: a blank line splits paragraphs, a single newline before a bold line stays a line break',
  sumMd.map((b) => [b.type, b.inline.map((n) => n.t)]), [['p', ['text']], ['p', ['b', 'text', 'br', 'b', 'text', 'code', 'text']]]);
eq('service tool: StructuredOutput (our roles\' answer by schema) is never a feed line - as an action, a v2 tool row or a log line; real tools are',
  [V.isServiceTool({ name: 'StructuredOutput' }), V.isServiceTool({ name: 'StructuredOutput', arg: '{"constraints":[]}' }), V.isServiceTool({ kind: 'tool', text: 'StructuredOutput: {"constraints":[1]}' }),
    V.isServiceTool({ name: 'Read' }), V.isServiceTool({ kind: 'tool', text: 'Bash: ls' }), V.isServiceTool(null)], [true, true, true, false, false, false]);
const kev = (code, key, extra) => Object.assign({ code, key, metaKey: true, ctrlKey: false, altKey: false, shiftKey: false }, extra);
eq('tab keys match the physical key: Russian layout (key "е"/"в"/"ц"), upper case, and an event without `code` (key or keyCode only)',
  [V.tabKeyAction(kev('KeyT', 'е'), 'mac'), V.tabKeyAction(kev('KeyD', 'D'), 'mac'), V.tabKeyAction(kev('', 'е'), 'mac'), V.tabKeyAction(kev('', 'Ц'), 'mac'), V.tabKeyAction(kev('', 'в'), 'mac'),
    V.tabKeyAction(kev('', 'T'), 'mac'), V.tabKeyAction(kev('', '', { keyCode: 68 }), 'mac'), V.tabKeyAction(kev('', '1'), 'mac'), V.tabKeyAction(kev('', 'е', { metaKey: false, ctrlKey: true }), 'linux')].map((a) => a && a.type),
  ['new', 'split', 'new', 'close', 'split', 'new', 'split', 'goto', 'new']);
const plan10 = Array.from({ length: 10 }, (_, i) => ({ text: 'p' + i, state: i < 5 ? 'done' : i === 5 ? 'active' : 'pending' }));
eq('plan: more than 7 items -> the current one ±2, the rest counted', (({ items, before, after }) => [items.map((x) => x.text), before, after])(V.planView(plan10)), [['p3', 'p4', 'p5', 'p6', 'p7'], 3, 2]);
eq('plan: 7 or fewer items are all shown', V.planView(plan10.slice(0, 4)).items.length, 4);
const tp3 = V.typePlan('один два три'), tp200 = V.typePlan(Array.from({ length: 200 }, () => 'w').join(' '));
eq('typing: 3 words -> one word per 60 ms; 200 words -> 30 ms ticks, all within 1.5 s; the pieces add up to the text',
  [tp3.ms, tp3.chunks.length, tp3.chunks.join(''), tp200.ms, tp200.chunks.length * tp200.ms <= 1500, tp200.chunks.join('').split(' ').length], [60, 3, 'один два три', 30, true, 200]);

// --- tabs: open, split, close, save and restore (stories 232-235) ---
const tab = (id, chat, env) => ({ id, env: env || 'claude', chat });
let ts0 = V.tabsOpen(V.tabsOpen(V.tabsEmpty(), tab('a', 'c1')), tab('b', 'c2'));
eq('tabs: a new tab goes after the active one and becomes active and shown', [ts0.tabs.map((x) => x.id), ts0.active, ts0.panels], [['a', 'b'], 'b', ['b']]);
const tsSplit = V.tabsSplit(ts0, 'b');
eq('tabs: ⌘D on the shown tab puts the previous one on the left, the current stays on the right and active', [tsSplit.panels, tsSplit.active], [['a', 'b'], 'b']);
eq('tabs: ⌘⇧D takes the panel out of the split, the tab stays in the strip', (({ tabs, panels, active }) => [tabs.length, panels, active])(V.tabsUnsplit(tsSplit, 'b')), [2, ['a'], 'a']);
const ts4 = ['c', 'd', 'e'].reduce((s, id) => V.tabsSplit(V.tabsOpen(s, tab(id, 'x' + id)), id), tsSplit);
eq('tabs: no more than 4 panels', ts4.panels.length, 4);
const tsClosed = V.tabsClose(tsSplit, 'b');
eq('tabs: closing a shown tab leaves its neighbour shown and active', [tsClosed.tabs.map((x) => x.id), tsClosed.panels, tsClosed.active], [['a'], ['a'], 'a']);
eq('tabs: activating a hidden tab in a split replaces the focused panel', V.tabsActivate(V.tabsOpen(tsSplit, tab('c', 'c3')), 'a').panels, ['a', 'c']);
eq('tabs: moving changes the order', V.tabsMove(ts0, 'b', 0).tabs.map((x) => x.id), ['b', 'a']);
const saved = V.tabsSerialize({ tabs: [tab('a', 'c1'), tab('b', 'gone'), tab('c', 'new', 'codex')], active: 'b', panels: ['a', 'b'], prev: null });
const restored = V.tabsRestore(saved, (envId, chatId) => chatId === 'c1');
eq('tabs restore: a deleted chat does not come back, an empty new chat does, active and split are repaired',
  [restored.tabs.map((x) => x.id), restored.active, restored.panels], [['a', 'c'], 'a', ['a']]);
eq('tabs restore: broken or empty data -> nothing', [V.tabsRestore('{oops', () => true), V.tabsRestore(null, () => true)], [null, null]);

// --- keys of tabs (story 232, 236): the window handler and the busy check ---
const K = (code, mods) => Object.assign({ code, key: code }, mods);
eq('tab keys mac: ⌘T new, ⌘3 third, ⌘9 last, ⌥⌘3 is the env (not a tab), ⌘⇧] next, ⌘⌥← panel left, ⌘W close, ⌘D split, ⌘⇧D unsplit',
  [K('KeyT', { metaKey: true }), K('Digit3', { metaKey: true }), K('Digit9', { metaKey: true }), K('Digit3', { metaKey: true, altKey: true }), K('BracketRight', { metaKey: true, shiftKey: true }),
    K('ArrowLeft', { metaKey: true, altKey: true }), K('KeyW', { metaKey: true }), K('KeyD', { metaKey: true }), K('KeyD', { metaKey: true, shiftKey: true })].map((e) => V.tabKeyAction(e, 'mac', null)),
  [{ type: 'new' }, { type: 'goto', n: 3 }, { type: 'last' }, null, { type: 'next' }, { type: 'panel', dir: -1 }, { type: 'close' }, { type: 'split' }, { type: 'unsplit' }]);
eq('tab keys linux: Ctrl+Tab next, Ctrl+Shift+Tab previous, Ctrl+1 first', [K('Tab', { ctrlKey: true }), K('Tab', { ctrlKey: true, shiftKey: true }), K('Digit1', { ctrlKey: true })].map((e) => V.tabKeyAction(e, 'linux', null)),
  [{ type: 'next' }, { type: 'prev' }, { type: 'goto', n: 1 }]);
eq('keys: the defaults of v3 do not collide', V.keyConflicts(Object.assign({ new_chat: 'Alt+Cmd+N', send: 'Enter', freeze: 'Alt+Cmd+.', switch_env: 'Alt+Cmd+{n}' }, V.tabKeyDefaults('mac')), 'mac'), []);
eq('keys: the old ⌘1…7 for envs collides with the tab numbers; two rows with one key collide', V.keyConflicts({ switch_env: 'Cmd+{n}', tab_new: 'Cmd+T', tab_close: 'Cmd+T' }, 'mac'),
  [['switch_env', 'tab_digits'], ['tab_new', 'tab_close']]);
eq('keys: env switching moves from ⌘1…7 to ⌥⌘1…7 (Ctrl+Alt on Linux)', [V.migrateSwitchEnv('Cmd+{n}', 'mac'), V.migrateSwitchEnv('Ctrl+{n}', 'linux'), V.migrateSwitchEnv('Alt+Cmd+{n}', 'mac')], ['Alt+Cmd+{n}', 'Ctrl+Alt+{n}', 'Alt+Cmd+{n}']);

// --- queue line (story 234), first run env pick (story 237), model words (story 240) ---
const qt = (id, status, created, extra) => Object.assign({ id, status, agent: 'claude', title: id, created_at: new Date(NOW - created * 60000).toISOString(), started_at: status === 'working' ? new Date(NOW - created * 60000).toISOString() : null }, extra);
const qtasks = [qt('r1', 'working', 30), qt('r2', 'reviewing', 20), qt('q1', 'queued', 10), qt('q2', 'queued', 5), qt('o', 'queued', 1, { agent: 'codex' })];
eq('queue: over the env limit -> "2nd, frees after" the oldest running task', V.queueInfo(qtasks, 'q2', 2), { pos: 2, after: 'r1' });
eq('queue: under the limit -> no queue line', V.queueInfo(qtasks, 'q2', 3), null);
const envs6 = (states) => ['cursor', 'qwen', 'gemini', 'opencode', 'codex', 'claude'].map((id, i) => ({ id, name: id, state: states[i] }));
eq('first run: the env is picked in the order Claude Code, Codex, opencode, Gemini, Qwen, Cursor - the first signed in',
  [V.pickEnv(envs6(['ok', 'ok', 'ok', 'ok', 'missing', 'missing'])), V.pickEnv(envs6(['missing', 'missing', 'missing', 'login', 'missing', 'missing'])), V.pickEnv(envs6(['missing', 'missing', 'missing', 'missing', 'missing', 'missing']))].map((x) => [x.why, x.env && x.env.id]),
  [['ok', 'opencode'], ['login', 'opencode'], ['none', null]]);
const has = (k) => k in NSI18n.dicts.ru;
eq('model words: the core key model.desc.<id> is translated, the source note goes to the tooltip only',
  [V.modelDesc({ id: 'opus', blurb: 'model.desc.opus' }, t, has).text, V.modelDesc({ id: 'zz', blurb: 'Fast. (Google docs, 2026-10-21; не сверено с CLI)' }, t, has)],
  ['Долгая агентная работа с кодом и знаниями.', { text: 'Fast.', note: 'Google docs, 2026-10-21; не сверено с CLI' }]);
eq('limit reasons from the core are words, not keys', t('lim.why.after_first_run'), 'данные появятся после первого запуска');
eq('v3 words exist in ru and en', Object.keys(NSI18n.dicts.ru).filter((k) => /^(live|act3|md|tabs|onb3|model\.desc|lim\.why|keys3)\./.test(k)).filter((k) => !(k in NSI18n.dicts.en)), []);

// bugs of the Playwright walk (ticket 17, criteria 10 and 12)
const tEn = (k, p) => { NSI18n.setLang('en'); const r = NSI18n.t(k, p); NSI18n.setLang('ru'); return r; };
eq('budget in en reads "of", in ru "из" (bug 10)', [L.formatBudget(14.89, 40, tEn), L.formatBudget(14.89, 40, t)], ['$14.89 of $40', '$14.89 из $40']);
eq('the "day" limit window is a word in ru and en, not the raw name (bug 12)', [L.windowName({ name: 'day' }, t), L.windowName({ name: 'day' }, tEn)], ['День', 'Day']);
}
const total = out.length;
if (typeof document !== 'undefined') {
  document.getElementById('out').textContent = out.join('\n') + `\n\n${bad ? 'FAILED ' + bad : 'ALL PASSED'}: ${total - bad} of ${total}`;
  document.title = bad ? 'FAILED' : 'PASSED';
} else console.log(out.join('\n') + `\n${bad ? 'FAILED ' + bad : 'ALL PASSED'}: ${total - bad} of ${total}`);
