const out = []; let bad = 0;
function eq(name, got, want) { const ok = JSON.stringify(got) === JSON.stringify(want); if (!ok) bad++; out.push((ok ? 'PASS ' : 'FAIL ') + name + (ok ? '' : ` got=${JSON.stringify(got)} want=${JSON.stringify(want)}`)); }
const NOW = Date.parse('2026-10-21T12:00:00Z'), day = 864e5;
const task = (id, status, finDaysAgo) => ({ id, status, created_at: new Date(NOW - 3 * day).toISOString(), started_at: null,
  finished_at: finDaysAgo == null ? null : new Date(NOW - finDaysAgo * day).toISOString() });

// spec story 10/13 + "Рост": group order, needs first, empty groups hidden, done older than 7 days -> archive
const groups = NSLogic.groupTasks([task('a', 'queued'), task('b', 'done', 8), task('c', 'awaiting_outward'), task('d', 'done', 1),
  task('e', 'failed', 1), task('f', 'cancelled', 1), task('g', 'checking'), task('h', 'needs_human')], NOW);
eq('group order, empty hidden', groups.map((g) => g.id), ['needs', 'working', 'queued', 'done', 'failed', 'archive']);
eq('needs holds both question kinds', groups[0].tasks.map((t) => t.id).sort(), ['c', 'h']);
eq('archive = old done + cancelled', groups[5].tasks.map((t) => t.id).sort(), ['b', 'f']);
eq('7-day boundary stays in Done', NSLogic.groupOf(task('x', 'done', 6.9), NOW), 'done');
// merge_blocked = postponed merge (spec story 22), not a question: working group + working style, no live timer
eq('merge_blocked is in progress, not Needs', [NSLogic.groupOf(task('m', 'merge_blocked'), NOW), NSLogic.info('merge_blocked').cls], ['working', 'working']);
eq('question statuses are needs/needs', ['needs_human', 'awaiting_outward'].map((s) => [NSLogic.groupOf(task('q', s), NOW), NSLogic.info(s).cls]), [['needs', 'needs'], ['needs', 'needs']]);
eq('every spec status is mapped', ['queued','preparing','working','checking','reviewing','merging','needs_human','paused_limit','awaiting_outward','merge_blocked','done','done_not_pushed','failed','cancelled','reverted'].filter((s) => !NSLogic.STATUS[s]), []);
eq('no tasks -> no groups', NSLogic.groupTasks([], NOW), []);
eq('counter counts only waiting-for-human', NSLogic.countNeeds([task('a', 'needs_human'), task('b', 'awaiting_outward'), task('c', 'working')]), 2);

// hand-computed: 3*60+5 = 185 s; 1*3600+12*60+9 = 4329 s
eq('elapsed mm:ss', NSLogic.formatElapsed(185000), '3:05');
eq('elapsed h:mm:ss', NSLogic.formatElapsed(4329000), '1:12:09');
eq('elapsed never negative', NSLogic.formatElapsed(-5000), '0:00');

const F = ['файл', 'файла', 'файлов'];
eq('ru plural 1/2/5/11/21/22', [1, 2, 5, 11, 21, 22].map((n) => NSLogic.plural(n, F, 'ru')), ['файл', 'файла', 'файлов', 'файлов', 'файл', 'файла']);
eq('en plural', [1, 2].map((n) => NSLogic.plural(n, ['file', 'files'], 'en')), ['file', 'files']);

// criterion: every string exists in ru and en
const ru = Object.keys(NSI18n.dicts.ru), en = Object.keys(NSI18n.dicts.en);
eq('i18n: ru and en have the same keys', [ru.filter((k) => !en.includes(k)), en.filter((k) => !ru.includes(k))], [[], []]);
NSI18n.setLang('ru'); eq('i18n ru param', NSI18n.t('gh.as', { user: 'x' }), 'gh: вошёл как x');
NSI18n.setLang('en'); eq('i18n en param', NSI18n.t('gh.as', { user: 'x' }), 'gh: signed in as x');

document.getElementById('out').textContent = out.join('\n') + `\n\n${bad ? 'FAILED ' + bad : 'ALL PASSED'}`;
document.title = bad ? 'FAILED' : 'PASSED';
