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

const total = out.length;
if (typeof document !== 'undefined') {
  document.getElementById('out').textContent = out.join('\n') + `\n\n${bad ? 'FAILED ' + bad : 'ALL PASSED'}: ${total - bad} of ${total}`;
  document.title = bad ? 'FAILED' : 'PASSED';
} else console.log(out.join('\n') + `\n${bad ? 'FAILED ' + bad : 'ALL PASSED'}: ${total - bad} of ${total}`);
