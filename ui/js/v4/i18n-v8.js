// Strings of v8 (Agentation notes 28.10.2026): the agents column, the session tabs above the terminal, the limits bar, install in a tab.
(function (root) {
  const ru = {
    'env8.rail': 'Агенты', 'env8.all': 'Все', 'env8.all_d': 'Сессии всех агентов', 'env8.pick_d': '{env}: сессии и новый терминал этого агента',
    'env8.missing': 'Не установлен', 'env8.get': 'Добавить', 'env8.install': 'Установить', 'env8.install_d': 'Команда из официального источника, запустится во встроенном терминале',
    'env8.piped_d': 'Команда со скриптом из сети: она уже набрана в терминале, проверь и нажми Enter', 'env8.settings': 'Указать путь',
    'env8.installing': 'Установка {env}', 'env8.installed': '{env} установлен', 'env8.no_hint': 'Для {env} нет команды установки', 'env8.wait_n': 'Ждут тебя: {n}', 'env8.run_n': 'Работают: {n}',
    'tabs8.label': 'Сессии', 'tabs8.new': 'Новая сессия', 'tabs8.new_d': 'Новая сессия {env}', 'tabs8.empty': 'У {env} пока нет сессий', 'tabs8.start': 'Начать сессию',
    'proj8.none': 'Выбрать проект',
    'md8.title': 'Режимы работы', 'md8.settings': 'Настройки режимов', 'md8.swarm': 'Рой', 'md8.swarm_n': 'Рой ×{n}', 'md8.swarm_d': 'Несколько агентов на одну задачу из чата',
    'md8.mode': 'Как работает рой', 'md8.agents': 'Агентов', 'md8.quality': 'Качество', 'md8.quality_d': 'Спека, план, тесты и ревьюер перед merge',
    'md8.q_spec': 'Спека', 'md8.q_plan': 'План', 'md8.q_tests_first': 'Сначала тесты', 'md8.q_e2e': 'E2E-проверка',
    'md8.factory_d': 'Агенты берут задачи из очереди; выключить - заморозить всё', 'md8.open_factory': 'Открыть фабрику', 'md8.frozen': 'Фабрика заморожена',
    'gh8.toggle': 'Панель GitHub: ветки и pull requests', 'gh8.refresh': 'Обновить', 'gh8.tree': 'Ветки', 'gh8.prs': 'Pull requests', 'gh8.no_project': 'Выбери проект сверху - здесь будут его ветки и PR',
    'gh8.loading': 'Загружаю…', 'gh8.no_commits': 'Коммитов пока нет', 'gh8.all_history': 'Вся история', 'gh8.no_prs': 'Открытых PR нет', 'gh8.no_gh': 'Нужен GitHub CLI', 'gh8.no_login': 'gh не вошёл в GitHub',
    'gh8.err': 'GitHub не ответил', 'gh8.login_btn': 'Войти в терминале', 'gh8.draft': 'черновик', 'gh8.approved': 'одобрен', 'gh8.changes': 'нужны правки',
    'gh8.ck_ok': 'Проверки прошли', 'gh8.ck_fail': 'Проверки упали', 'gh8.ck_run': 'Проверки идут', 'gh8.ck_none': 'Проверок нет',
    'am8.idle': 'свободен', 'am8.run': 'работает', 'am8.wait': 'ждёт тебя',
    'lim8.of': 'Лимиты {env}', 'lim8.none': 'появятся после первого запуска',
  };
  const en = {
    'env8.rail': 'Agents', 'env8.all': 'All', 'env8.all_d': 'Sessions of every agent', 'env8.pick_d': '{env}: its sessions and a new terminal',
    'env8.missing': 'Not installed', 'env8.get': 'Add', 'env8.install': 'Install', 'env8.install_d': 'The command from the official source, run in the built-in terminal',
    'env8.piped_d': 'A command with a script from the web: it is typed in the terminal, check it and press Enter', 'env8.settings': 'Set the path',
    'env8.installing': 'Installing {env}', 'env8.installed': '{env} installed', 'env8.no_hint': 'No install command for {env}', 'env8.wait_n': 'Waiting for you: {n}', 'env8.run_n': 'Working: {n}',
    'tabs8.label': 'Sessions', 'tabs8.new': 'New session', 'tabs8.new_d': 'New {env} session', 'tabs8.empty': '{env} has no sessions yet', 'tabs8.start': 'Start a session',
    'proj8.none': 'Pick a project',
    'md8.title': 'Work modes', 'md8.settings': 'Mode settings', 'md8.swarm': 'Swarm', 'md8.swarm_n': 'Swarm ×{n}', 'md8.swarm_d': 'Several agents on one chat task',
    'md8.mode': 'How the swarm works', 'md8.agents': 'Agents', 'md8.quality': 'Quality', 'md8.quality_d': 'Spec, plan, tests and a reviewer before merge',
    'md8.q_spec': 'Spec', 'md8.q_plan': 'Plan', 'md8.q_tests_first': 'Tests first', 'md8.q_e2e': 'E2E check',
    'md8.factory_d': 'Agents take tasks from the queue; off freezes everything', 'md8.open_factory': 'Open the factory', 'md8.frozen': 'The factory is frozen',
    'gh8.toggle': 'GitHub panel: branches and pull requests', 'gh8.refresh': 'Refresh', 'gh8.tree': 'Branches', 'gh8.prs': 'Pull requests', 'gh8.no_project': 'Pick a project above - its branches and PRs show here',
    'gh8.loading': 'Loading…', 'gh8.no_commits': 'No commits yet', 'gh8.all_history': 'Full history', 'gh8.no_prs': 'No open PRs', 'gh8.no_gh': 'GitHub CLI is needed', 'gh8.no_login': 'gh is not signed in to GitHub',
    'gh8.err': 'GitHub did not answer', 'gh8.login_btn': 'Sign in in a terminal', 'gh8.draft': 'draft', 'gh8.approved': 'approved', 'gh8.changes': 'changes requested',
    'gh8.ck_ok': 'Checks passed', 'gh8.ck_fail': 'Checks failed', 'gh8.ck_run': 'Checks running', 'gh8.ck_none': 'No checks',
    'am8.idle': 'free', 'am8.run': 'working', 'am8.wait': 'waiting for you',
    'lim8.of': '{env} limits', 'lim8.none': 'will appear after the first run',
  };
  Object.assign(root.NSI18n.dicts.ru, ru);
  Object.assign(root.NSI18n.dicts.en, en);
})(typeof window !== 'undefined' ? window : globalThis);
