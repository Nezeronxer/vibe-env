// Strings of v6 (G67, G68): issues of GitHub in «Предлагаю», the question «Закрыть issue #N?», sessions of Claude Code from a terminal.
(function (root) {
  const ru = {
    // issues
    'iss.tag': 'issue #{n}', 'iss.open': 'Открыть на GitHub', 'iss.in': '{project}',
    'iss.note': 'issues недоступны: {why}', 'iss.why.gh_not_installed': 'не найден gh', 'iss.why.gh_not_logged_in': 'gh не вошёл в GitHub', 'iss.why.gh_error': 'gh не ответил',
    'iss.ask_h': 'Нужен ты: закрыть issue?', 'iss.ask_q': 'Закрыть issue #{n}?', 'iss.ask_d': 'Задача по нему слита. Закрою с комментарием, где лежит правка. Пока не нажмёшь «Закрыть», issue остаётся открытым.',
    'iss.close': 'Закрыть', 'iss.keep': 'Оставить', 'iss.closed': 'Issue #{n} закрыт', 'iss.kept': 'Issue #{n} оставлен открытым',
    // sessions
    'ses.title': 'Сессии из терминала', 'ses.cmd': 'Сессии из терминала: прошлые разговоры с Claude Code', 'ses.open': 'Сессии из терминала',
    'ses.lead': 'Разговоры с Claude Code, начатые в терминале в папке проекта. Приложение их только читает. «Продолжить» откроет сессию обычной задачей в отдельной git-копии.',
    'ses.project': 'Проект', 'ses.none': 'Сессий из терминала для этого проекта нет.', 'ses.no_project': 'Сначала добавь проект.',
    'ses.messages': '{n} сообщ.', 'ses.messages_more': 'больше {n} сообщ.', 'ses.empty_prompt': '(без первой реплики)', 'ses.branch': 'ветка {name}',
    'ses.unreadable': 'не читается', 'ses.continue': 'Продолжить', 'ses.started': 'Задача поставлена: сессия продолжается в отдельной копии',
    'ses.more': 'Показать ещё', 'ses.count': 'Всего сессий: {n}', 'ses.loading': 'Читаю…',
    'err.session_not_found': 'Такой сессии у проекта нет: {detail}', 'err.session_unreadable': 'Файл сессии не читается: {detail}', 'err.bad_session_id': 'Неверный номер сессии: {detail}',
    'err.session_copy_failed': 'Не вышло скопировать сессию в git-копию: {detail}', 'err.no_question': 'Этот вопрос уже закрыт', 'err.no_home': 'Не нашёл домашнюю папку',
  };
  const en = {
    'iss.tag': 'issue #{n}', 'iss.open': 'Open on GitHub', 'iss.in': '{project}',
    'iss.note': 'issues unavailable: {why}', 'iss.why.gh_not_installed': 'gh not found', 'iss.why.gh_not_logged_in': 'gh is not signed in to GitHub', 'iss.why.gh_error': 'gh did not answer',
    'iss.ask_h': 'You are needed: close the issue?', 'iss.ask_q': 'Close issue #{n}?', 'iss.ask_d': 'The task for it is merged. I will close it with a comment that says where the change is. Until you press "Close" the issue stays open.',
    'iss.close': 'Close', 'iss.keep': 'Keep open', 'iss.closed': 'Issue #{n} closed', 'iss.kept': 'Issue #{n} left open',
    'ses.title': 'Sessions from the terminal', 'ses.cmd': 'Sessions from the terminal: earlier talks with Claude Code', 'ses.open': 'Sessions from the terminal',
    'ses.lead': 'Talks with Claude Code started in a terminal in the project folder. The app only reads them. "Continue" opens a session as an ordinary task in a separate git copy.',
    'ses.project': 'Project', 'ses.none': 'There are no sessions from the terminal for this project.', 'ses.no_project': 'Add a project first.',
    'ses.messages': '{n} msgs', 'ses.messages_more': 'over {n} msgs', 'ses.empty_prompt': '(no first message)', 'ses.branch': 'branch {name}',
    'ses.unreadable': 'unreadable', 'ses.continue': 'Continue', 'ses.started': 'Task queued: the session goes on in a separate copy',
    'ses.more': 'Show more', 'ses.count': 'Sessions in all: {n}', 'ses.loading': 'Reading…',
    'err.session_not_found': 'The project has no such session: {detail}', 'err.session_unreadable': 'The session file cannot be read: {detail}', 'err.bad_session_id': 'Bad session id: {detail}',
    'err.session_copy_failed': 'Could not copy the session into the git copy: {detail}', 'err.no_question': 'That question is already closed', 'err.no_home': 'Could not find the home folder',
  };
  Object.assign(root.NSI18n.dicts.ru, ru);
  Object.assign(root.NSI18n.dicts.en, en);
})(typeof window !== 'undefined' ? window : globalThis);
