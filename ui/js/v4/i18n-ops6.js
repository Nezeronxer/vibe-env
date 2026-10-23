// Strings of v6 (task 28: G64, G69-G71): a lane of a swarm waiting for its limit, «Расходы», «О программе», the error report,
// the bar «Вышла версия X».
(function (root) {
  const ru = {
    // a lane of the swarm (G64.1)
    'swarm.paused': 'ждёт лимит до {time}', 'swarm.part_paused': 'ждёт лимит до {time}', 'swarm.paused_d': 'Лимит среды кончился у этой полосы. Остальные работают, она продолжит сама после сброса.',
    // spending (G70)
    'spend.cmd': 'Расходы: деньги и токены по дням', 'spend.title': 'Расходы',
    'spend.lead': 'Что потратили задачи и чаты: по дням и проектам. Хранится 400 дней, старше - сворачивается в месяцы. Среды, которые не сообщают стоимость, показаны как «нет данных».',
    'spend.empty': 'Расходов пока нет', 'spend.empty_d': 'Здесь появится строка, когда задача или чат что-нибудь потратят.',
    'spend.day': 'День', 'spend.project': 'Проект', 'spend.env': 'Среда', 'spend.usd': 'Деньги', 'spend.tin': 'Токены вход', 'spend.tout': 'Токены выход',
    'spend.chat': 'чат', 'spend.no_data': 'нет данных', 'spend.unknown_project': 'проект не указан', 'spend.removed_project': 'удалённый проект',
    'spend.total': 'Итого', 'spend.month': '{month} (за месяц)', 'spend.range': 'Период', 'spend.r30': '30 дней', 'spend.r90': '90 дней', 'spend.rall': 'Всё время',
    'spend.part_money': 'часть сред без данных о деньгах',
    // about (G69, G71)
    'about.cmd': 'О программе: версия, обновления, отчёт об ошибке', 'about.title': 'О программе',
    'about.version': 'Версия {v}', 'about.lead': 'Nightshift: агенты доводят задачи сами, тебя зовут только за решением.',
    'about.upd_h': 'Обновления', 'about.check': 'Проверить', 'about.checking': 'Проверяю…',
    'about.no_feed': 'Проверка выключена: адрес обновлений не задан.', 'about.no_feed_d': 'Адрес появится, когда репозиторий опубликуют (настройка release_feed).',
    'about.never': 'Ещё не проверяли.', 'about.checked': 'Последняя проверка: {when}.', 'about.latest': 'Последняя версия: {v}.', 'about.uptodate': 'Это последняя версия.',
    'about.newer': 'Вышла версия {v}.', 'about.open_release': 'Открыть релиз', 'about.err': 'Не вышло проверить: {why}. Повторю завтра.',
    'about.auto': 'Сама проверяет раз в сутки.',
    'about.diag_h': 'Отчёт об ошибке', 'about.diag_d': 'Соберу zip только с техническим: версии, ОС, CLI, настройки без секретов, состояния задач без текстов, хвост лога. Тексты чатов, промпты, дифы и ключи туда не попадают. Ничего никуда не отправляется.',
    'about.diag_go': 'Собрать отчёт об ошибке', 'about.diag_pick': 'В другую папку…', 'about.diag_busy': 'Собираю…', 'about.diag_done': 'Отчёт готов: {path}',
    'about.diag_where': 'Ляжет в «Загрузки».',
    'diag.cmd': 'Собрать отчёт об ошибке',
    // the bar (G69)
    'appupd.bar': 'Вышла версия {v}', 'appupd.open': 'Открыть релиз', 'appupd.later': 'Скрыть',
    'appupd.why.no_connection': 'нет сети', 'appupd.why.timeout': 'адрес не ответил вовремя', 'appupd.why.bad_answer': 'адрес ответил не релизом', 'appupd.why.not_https': 'адрес не https',
    'appupd.why.curl_missing': 'не найден curl', 'appupd.why.too_large': 'ответ слишком большой', 'appupd.why.http': 'адрес ответил {code}',
    'err.bad_dir': 'Такой папки нет', 'err.bad_day': 'Неверная дата', 'err.bad_version': 'Неверная версия',
  };
  const en = {
    'swarm.paused': 'waits for the limit until {time}', 'swarm.part_paused': 'waits for the limit until {time}', 'swarm.paused_d': 'The env ran out of its limit in this lane. The others keep working; it goes on by itself after the reset.',
    'spend.cmd': 'Spending: money and tokens by day', 'spend.title': 'Spending',
    'spend.lead': 'What tasks and chats spent, by day and project. 400 days are kept, older ones fold into months. Envs that do not report cost show "no data".',
    'spend.empty': 'No spending yet', 'spend.empty_d': 'A row appears here once a task or a chat spends something.',
    'spend.day': 'Day', 'spend.project': 'Project', 'spend.env': 'Env', 'spend.usd': 'Money', 'spend.tin': 'Tokens in', 'spend.tout': 'Tokens out',
    'spend.chat': 'chat', 'spend.no_data': 'no data', 'spend.unknown_project': 'project not named', 'spend.removed_project': 'removed project',
    'spend.total': 'Total', 'spend.month': '{month} (whole month)', 'spend.range': 'Period', 'spend.r30': '30 days', 'spend.r90': '90 days', 'spend.rall': 'All time',
    'spend.part_money': 'some envs have no money data',
    'about.cmd': 'About: version, updates, error report', 'about.title': 'About',
    'about.version': 'Version {v}', 'about.lead': 'Nightshift: agents finish tasks themselves and call you only for a decision.',
    'about.upd_h': 'Updates', 'about.check': 'Check', 'about.checking': 'Checking…',
    'about.no_feed': 'The check is off: no update address is set.', 'about.no_feed_d': 'The address appears once the repository is published (the release_feed setting).',
    'about.never': 'Not checked yet.', 'about.checked': 'Last check: {when}.', 'about.latest': 'Latest version: {v}.', 'about.uptodate': 'This is the latest version.',
    'about.newer': 'Version {v} is out.', 'about.open_release': 'Open the release', 'about.err': 'Could not check: {why}. I will try again tomorrow.',
    'about.auto': 'Checks by itself once a day.',
    'about.diag_h': 'Error report', 'about.diag_d': 'I will make a zip with technical facts only: versions, OS, CLIs, settings without secrets, task states without texts, the end of the log. Chat texts, prompts, diffs and keys do not go in. Nothing is sent anywhere.',
    'about.diag_go': 'Make an error report', 'about.diag_pick': 'To another folder…', 'about.diag_busy': 'Making…', 'about.diag_done': 'The report is ready: {path}',
    'about.diag_where': 'It goes to Downloads.',
    'diag.cmd': 'Make an error report',
    'appupd.bar': 'Version {v} is out', 'appupd.open': 'Open the release', 'appupd.later': 'Hide',
    'appupd.why.no_connection': 'no network', 'appupd.why.timeout': 'the address did not answer in time', 'appupd.why.bad_answer': 'the address did not answer with a release', 'appupd.why.not_https': 'the address is not https',
    'appupd.why.curl_missing': 'curl not found', 'appupd.why.too_large': 'the answer is too big', 'appupd.why.http': 'the address answered {code}',
    'err.bad_dir': 'There is no such folder', 'err.bad_day': 'Bad date', 'err.bad_version': 'Bad version',
  };
  Object.assign(root.NSI18n.dicts.ru, ru);
  Object.assign(root.NSI18n.dicts.en, en);
})(typeof window !== 'undefined' ? window : globalThis);
