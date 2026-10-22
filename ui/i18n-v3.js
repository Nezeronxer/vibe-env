// Strings of v3 (live feed, tabs, first run in one step, model words), merged into the dictionaries of i18n.js.
// Loaded right after i18n-v2.js; selftest checks that ru and en keep the same keys.
(function (root) {
  const ru = {
    // the live line under the feed (story 227)
    'live.thinking': 'Думает…', 'live.writing': 'Пишет код…', 'live.running': 'Запускает команду…', 'live.reading': 'Читает…', 'live.web': 'Ищет в сети…',
    'live.planning': 'Составляет план…', 'live.answering': 'Пишет ответ…', 'live.queue': 'В очереди: {pos}-я, освободится после «{after}»', 'live.queue_short': 'В очереди: {pos}-я',
    'live.plan': 'План', 'live.plan_more': 'ещё {n}', 'live.label': 'Ход работы',
    // action lines (stories 225, 226, 229)
    'act3.read_n': 'Прочитал {n} {w}', 'act3.file_1': 'файл', 'act3.file_2': 'файла', 'act3.file_5': 'файлов', 'act3.search_n': 'искал {n} {w}', 'act3.times_1': 'раз', 'act3.times_2': 'раза', 'act3.times_5': 'раз',
    'act3.more_lines': 'ещё {n} {w}', 'act3.line_1': 'строка', 'act3.line_2': 'строки', 'act3.line_5': 'строк', 'act3.show_all': 'показать всё', 'act3.collapse': 'свернуть',
    'act3.cut': 'вывод обрезан, полный - в логе задачи', 'act3.open_log': 'Открыть лог', 'act3.error': 'Ошибка', 'act3.hits': '{n} {w}', 'act3.hit_1': 'совпадение', 'act3.hit_2': 'совпадения', 'act3.hit_5': 'совпадений',
    'act3.no_output': '(нет вывода)', 'act3.new_file': 'новый файл, {n} {w}', 'act3.running': 'выполняется…', 'act3.log_title': 'Вывод команды', 'act3.close': 'Закрыть', 'act3.older': 'показать раньше',
    'act3.no_diff': 'правка без текста диффа', 'act3.link': 'Откроется в браузере',
    // tabs and panels (stories 232-236)
    'tabs.label': 'Вкладки', 'tabs.new': 'Новая вкладка', 'tabs.new_chat': 'Новый чат', 'tabs.close': 'Закрыть вкладку', 'tabs.close_others': 'Закрыть остальные', 'tabs.split': 'Разделить',
    'tabs.unsplit': 'Убрать из раздела', 'tabs.narrow': 'Окно слишком узкое для двух панелей', 'tabs.more': 'ещё {n}', 'tabs.menu': 'Меню вкладки', 'tabs.max': 'Больше 4 панелей не помещается',
    'tabs.st_run': 'работает', 'tabs.st_wait': 'ждёт тебя', 'tabs.st_done': 'готово', 'tabs.st_fail': 'не вышло', 'tabs.st_queue': 'в очереди', 'tabs.st_talk': 'разговор',
    // first run in one step (stories 237-239)
    'onb3.title': 'Начнём с папки проекта', 'onb3.lead': 'Агенты работают в отдельных ветках этой папки и зовут тебя, только когда без тебя никак.',
    'onb3.drop': 'Выбери или перетащи папку проекта', 'onb3.drop_d': 'Нужен git. Работаем в ветках ns/*, твой main не трогаем.', 'onb3.pick': 'Выбрать папку…',
    'onb3.line': 'Среда: {env} · модель как в CLI · поменять можно в строке ввода',
    'onb3.none_title': 'Не нашёл ни одной агентной среды', 'onb3.none_d': 'Установи одну из них и вернись в окно - она появится сама.', 'onb3.install': 'Установить', 'onb3.recheck': 'Проверить снова',
    'onb3.login_title': 'Войди в {env}', 'onb3.login_d': 'Среда найдена, но вход не выполнен. Откроется терминал с CLI - войди её способом.', 'onb3.login_btn': 'Войти',
    'onb3.folder': 'Папка: {path}', 'onb3.send_off': 'Сначала установи среду или войди в неё', 'onb3.ph': 'Опиши задачу…',
    'onb3.ex_title': 'С чего начать', 'onb3.ex_d': 'Опиши задачу обычными словами. Пример встанет в поле - отправишь сам.',
    'onb3.ex1': 'Сделай…', 'onb3.ex2': 'Найди и почини…', 'onb3.ex3': 'Объясни, как устроен…', 'onb3.ex1_t': 'Сделай ', 'onb3.ex2_t': 'Найди и почини ', 'onb3.ex3_t': 'Объясни, как устроен ',
    // hotkeys of v3 (story 236)
    'keys3.tab_new': 'Новая вкладка', 'keys3.tab_close': 'Закрыть вкладку', 'keys3.tab_close_d': 'Работа агента не останавливается, чат остаётся в списке.', 'keys3.tab_split': 'Разделить панели',
    'keys3.tab_split_d': 'Текущая вкладка встаёт рядом, до 4 панелей.', 'keys3.tab_unsplit': 'Убрать панель из раздела', 'keys3.fixed': 'Вкладки по номеру, соседние вкладки и панели',
    'keys3.fixed_mac': '⌘1…9 · ⌘⇧] / ⌘⇧[ · ⌃Tab · ⌘⌥← / →', 'keys3.fixed_pc': 'Ctrl+1…9 · Ctrl+Tab / Ctrl+Shift+Tab · Ctrl+Alt+← / →', 'keys3.conflict': 'Совпадает с «{other}» - одна из клавиш не сработает',
    'keys3.tab_digits': 'вкладки по номеру',
    // model words (story 240): the core sends model.desc.<id>
    'model.desc.fable': 'Самая сильная: трудные рассуждения и долгая агентная работа.', 'model.desc.opus': 'Долгая агентная работа с кодом и знаниями.',
    'model.desc.sonnet': 'Скорость и ум в балансе. Подходит почти всегда.', 'model.desc.haiku': 'Быстро и дёшево: много мелких задач.',
    'model.desc.pro': 'Сильнее, длинный контекст.', 'model.desc.flash': 'Быстро и дёшево.', 'model.desc.flash-lite': 'Быстрее и дешевле всех.',
    'model.desc.qwen3-max-2026-01-23': 'Самая большая модель, с рассуждением.', 'model.desc.qwen-plus': 'Продвинутая модель с рассуждением (3.6 - только с подпиской Pro).',
    'model.desc.qwen3-coder-plus': 'Заточена под код.', 'model.desc.qwen3-coder-next': 'Экспериментальная модель для кода.',
    'model.desc.gpt-codex': 'Заточена под код. Хороший выбор по умолчанию.', 'model.desc.gpt': 'Глубже рассуждает, медленнее.', 'model.desc.gpt-mini': 'Быстро и дёшево.',
    'model.desc.gemini-pro': 'Через твой ключ Google.', 'model.desc.gemini-flash': 'Быстро и дёшево.', 'model.desc.qwen-coder': 'Открытая модель под код.',
    'model.desc.llama': 'Ollama на этом компьютере, бесплатно.', 'model.desc.deepseek': 'Через твой ключ DeepSeek.', 'model.source': 'Источник: {note}',
    // limit reasons sent by the core as keys (story 240)
    'lim.why.after_first_run': 'данные появятся после первого запуска', 'lim.why.not_reported': 'среда не сообщает лимиты', 'lim.why.no_auth': 'данные появятся после входа в среду',
    'lim.why.unsupported': 'эта среда лимиты не показывает',
    'fmt.of': 'из', 'lim.wday': 'День', 'lim.wmonth': 'Месяц',
  };
  const en = {
    'live.thinking': 'Thinking…', 'live.writing': 'Writing code…', 'live.running': 'Running a command…', 'live.reading': 'Reading…', 'live.web': 'Searching the web…',
    'live.planning': 'Making a plan…', 'live.answering': 'Writing the answer…', 'live.queue': 'Queued: #{pos}, frees after "{after}"', 'live.queue_short': 'Queued: #{pos}',
    'live.plan': 'Plan', 'live.plan_more': '{n} more', 'live.label': 'Progress',
    'act3.read_n': 'Read {n} {w}', 'act3.file_1': 'file', 'act3.file_2': 'files', 'act3.file_5': 'files', 'act3.search_n': 'searched {n} {w}', 'act3.times_1': 'time', 'act3.times_2': 'times', 'act3.times_5': 'times',
    'act3.more_lines': '{n} more {w}', 'act3.line_1': 'line', 'act3.line_2': 'lines', 'act3.line_5': 'lines', 'act3.show_all': 'show all', 'act3.collapse': 'collapse',
    'act3.cut': 'output cut, the full one is in the task log', 'act3.open_log': 'Open the log', 'act3.error': 'Error', 'act3.hits': '{n} {w}', 'act3.hit_1': 'match', 'act3.hit_2': 'matches', 'act3.hit_5': 'matches',
    'act3.no_output': '(no output)', 'act3.new_file': 'new file, {n} {w}', 'act3.running': 'running…', 'act3.log_title': 'Command output', 'act3.close': 'Close', 'act3.older': 'show earlier',
    'act3.no_diff': 'an edit without diff text', 'act3.link': 'Opens in the browser',
    'tabs.label': 'Tabs', 'tabs.new': 'New tab', 'tabs.new_chat': 'New chat', 'tabs.close': 'Close tab', 'tabs.close_others': 'Close other tabs', 'tabs.split': 'Split',
    'tabs.unsplit': 'Take out of the split', 'tabs.narrow': 'The window is too narrow for two panels', 'tabs.more': '{n} more', 'tabs.menu': 'Tab menu', 'tabs.max': 'No room for more than 4 panels',
    'tabs.st_run': 'working', 'tabs.st_wait': 'waits for you', 'tabs.st_done': 'done', 'tabs.st_fail': 'failed', 'tabs.st_queue': 'queued', 'tabs.st_talk': 'talk',
    'onb3.title': 'Start with a project folder', 'onb3.lead': 'Agents work in separate branches of this folder and call you only when they cannot go on without you.',
    'onb3.drop': 'Choose or drop the project folder', 'onb3.drop_d': 'Needs git. We work in ns/* branches, your main stays untouched.', 'onb3.pick': 'Choose a folder…',
    'onb3.line': 'Environment: {env} · model as in the CLI · change it in the input field',
    'onb3.none_title': 'No agent environment found', 'onb3.none_d': 'Install one of them and come back to the window - it appears by itself.', 'onb3.install': 'Install', 'onb3.recheck': 'Check again',
    'onb3.login_title': 'Sign in to {env}', 'onb3.login_d': 'The environment is found but not signed in. A terminal with the CLI opens - sign in its own way.', 'onb3.login_btn': 'Sign in',
    'onb3.folder': 'Folder: {path}', 'onb3.send_off': 'Install an environment or sign in first', 'onb3.ph': 'Describe a task…',
    'onb3.ex_title': 'Where to start', 'onb3.ex_d': 'Describe the task in plain words. An example goes into the field - you send it yourself.',
    'onb3.ex1': 'Make…', 'onb3.ex2': 'Find and fix…', 'onb3.ex3': 'Explain how … works', 'onb3.ex1_t': 'Make ', 'onb3.ex2_t': 'Find and fix ', 'onb3.ex3_t': 'Explain how this works: ',
    'keys3.tab_new': 'New tab', 'keys3.tab_close': 'Close tab', 'keys3.tab_close_d': 'The agent keeps working, the chat stays in the list.', 'keys3.tab_split': 'Split panels',
    'keys3.tab_split_d': 'The current tab goes next to it, up to 4 panels.', 'keys3.tab_unsplit': 'Take the panel out of the split', 'keys3.fixed': 'Tabs by number, neighbour tabs and panels',
    'keys3.fixed_mac': '⌘1…9 · ⌘⇧] / ⌘⇧[ · ⌃Tab · ⌘⌥← / →', 'keys3.fixed_pc': 'Ctrl+1…9 · Ctrl+Tab / Ctrl+Shift+Tab · Ctrl+Alt+← / →', 'keys3.conflict': 'Same as "{other}" - one of the keys will not work',
    'keys3.tab_digits': 'tabs by number',
    'model.desc.fable': 'The strongest: demanding reasoning and long agentic work.', 'model.desc.opus': 'Long-running agentic coding and knowledge work.',
    'model.desc.sonnet': 'Speed and intelligence in balance. Fits almost always.', 'model.desc.haiku': 'Fast and cheap: many small tasks.',
    'model.desc.pro': 'Stronger, long context.', 'model.desc.flash': 'Fast and cheap.', 'model.desc.flash-lite': 'Fastest and cheapest.',
    'model.desc.qwen3-max-2026-01-23': 'The largest model, with thinking.', 'model.desc.qwen-plus': 'Advanced model with thinking (3.6: Pro subscribers only).',
    'model.desc.qwen3-coder-plus': 'Optimized for coding.', 'model.desc.qwen3-coder-next': 'Experimental coding model.',
    'model.desc.gpt-codex': 'Made for code. A good default.', 'model.desc.gpt': 'Reasons deeper, slower.', 'model.desc.gpt-mini': 'Fast and cheap.',
    'model.desc.gemini-pro': 'Through your Google key.', 'model.desc.gemini-flash': 'Fast and cheap.', 'model.desc.qwen-coder': 'An open model for code.',
    'model.desc.llama': 'Ollama on this computer, free.', 'model.desc.deepseek': 'Through your DeepSeek key.', 'model.source': 'Source: {note}',
    'lim.why.after_first_run': 'data will appear after the first run', 'lim.why.not_reported': 'the environment does not report limits', 'lim.why.no_auth': 'data will appear after signing in',
    'lim.why.unsupported': 'this environment does not show limits',
    'fmt.of': 'of', 'lim.wday': 'Day', 'lim.wmonth': 'Month',
  };
  Object.assign(root.NSI18n.dicts.ru, ru);
  Object.assign(root.NSI18n.dicts.en, en);
})(typeof window !== 'undefined' ? window : globalThis);
