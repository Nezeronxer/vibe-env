// Strings of F15: the app preview with "Указать элемент" and the proof screenshots of "Проверить как пользователь". Loaded after i18n-shell.js.
(function (root) {
  const ru = {
    'pv.kind': 'Превью', 'pv.cmd': 'Превью приложения', 'pv.title': 'Превью приложения', 'pv.of': 'задача «{title}»',
    'pv.url': 'Адрес', 'pv.url_ph': 'http://localhost:3000', 'pv.open': 'Открыть', 'pv.reload': 'Обновить', 'pv.close': 'Закрыть окно',
    'pv.width': 'Ширина', 'pv.w_phone': 'Телефон', 'pv.w_tablet': 'Планшет', 'pv.w_desk': 'Десктоп',
    'pv.pick': 'Указать элемент', 'pv.pick_d': 'Кликни по элементу в окне превью: ссылка на него встанет в поле чата задачи',
    'pv.note': 'Страница открывается отдельным окном и только с этого компьютера: http://localhost или 127.0.0.1.',
    'pv.st_closed': 'Окно превью не открыто', 'pv.st_open': 'Открыто: {url}', 'pv.st_pick': 'Кликни по элементу в окне превью. Esc - отмена',
    'pv.st_picked': 'Вставлено в поле чата: {sel}', 'pv.st_nochat': 'Открой чат задачи: описание элемента некуда вставить',
    'pv.port': 'порт задачи {port}',
    'pv.pick_text': 'Элемент на странице {url}: `{sel}`{text}, {w}×{h} px', 'pv.pick_q': ' «{text}»',
    'pv.proof': 'Проверять как пользователь', 'pv.proof_d': 'После зелёных проверок запустить приложение командой проекта и приложить к отчёту 3 скриншота: страница, после клика по основной кнопке, ширина телефона',
    'pv.proof_run': 'Команда запуска: {cmd}', 'pv.proof_norun': 'В проекте нет команды запуска (Настройки → Проекты) - шаг будет пропущен',
    'pv.proof_url': 'Адрес приложения: {url}', 'pv.proof_nourl': 'Адрес не задан: открой превью по адресу, и он запомнится',
    'pv.shots': 'Скриншоты: как это выглядит у пользователя', 'pv.shot': 'Скриншот {n} из {m}', 'pv.shot_names': 'Страница|После клика|Телефон',
    'pv.big_close': 'Закрыть', 'pv.loading': 'Загружаю…',
    'pv.chk': 'Как пользователь', 'pv.chk_ok': 'Снято скриншотов: {n}', 'pv.chk_skip': 'Пропущено: {why}',
    'pv.det': 'Превью', 'pv.det_d': 'Открыть запущенное приложение задачи рядом с чатом',
    'err.not_local_url': 'Только адреса этого компьютера: http://localhost или http://127.0.0.1', 'err.preview_closed': 'Окно превью закрыто',
    'err.no_capture_tool': 'нет node с Playwright и Chrome на этом компьютере', 'err.no_run_cmd': 'в проекте нет команды запуска', 'err.no_app_url': 'не задан адрес приложения',
    'err.app_not_started': 'приложение не ответило на своём адресе', 'err.port_busy': 'порт уже занят другим приложением', 'err.capture_failed': 'снимок не получился',
    'err.shot_not_found': 'Скриншот не найден',
  };
  const en = {
    'pv.kind': 'Preview', 'pv.cmd': 'App preview', 'pv.title': 'App preview', 'pv.of': 'task “{title}”',
    'pv.url': 'Address', 'pv.url_ph': 'http://localhost:3000', 'pv.open': 'Open', 'pv.reload': 'Reload', 'pv.close': 'Close window',
    'pv.width': 'Width', 'pv.w_phone': 'Phone', 'pv.w_tablet': 'Tablet', 'pv.w_desk': 'Desktop',
    'pv.pick': 'Point at element', 'pv.pick_d': 'Click an element in the preview window: a reference to it goes into the task chat field',
    'pv.note': 'The page opens in a window of its own and only from this computer: http://localhost or 127.0.0.1.',
    'pv.st_closed': 'The preview window is not open', 'pv.st_open': 'Open: {url}', 'pv.st_pick': 'Click an element in the preview window. Esc cancels',
    'pv.st_picked': 'Put into the chat field: {sel}', 'pv.st_nochat': 'Open the task chat: there is no field to put the element into',
    'pv.port': 'task port {port}',
    'pv.pick_text': 'Element on the page {url}: `{sel}`{text}, {w}×{h} px', 'pv.pick_q': ' “{text}”',
    'pv.proof': 'Check as a user', 'pv.proof_d': 'After green checks start the app with the project command and attach 3 screenshots to the report: the page, after a click on the main button, phone width',
    'pv.proof_run': 'Start command: {cmd}', 'pv.proof_norun': 'The project has no start command (Settings → Projects) - the step will be skipped',
    'pv.proof_url': 'App address: {url}', 'pv.proof_nourl': 'No address yet: open the preview at an address and it is remembered',
    'pv.shots': 'Screenshots: what the user sees', 'pv.shot': 'Screenshot {n} of {m}', 'pv.shot_names': 'Page|After the click|Phone',
    'pv.big_close': 'Close', 'pv.loading': 'Loading…',
    'pv.chk': 'As a user', 'pv.chk_ok': 'Screenshots taken: {n}', 'pv.chk_skip': 'Skipped: {why}',
    'pv.det': 'Preview', 'pv.det_d': 'Open the running app of the task next to the chat',
    'err.not_local_url': 'Only addresses of this computer: http://localhost or http://127.0.0.1', 'err.preview_closed': 'The preview window is closed',
    'err.no_capture_tool': 'no node with Playwright and Chrome on this computer', 'err.no_run_cmd': 'the project has no start command', 'err.no_app_url': 'no app address',
    'err.app_not_started': 'the app did not answer on its address', 'err.port_busy': 'the port is taken by another app', 'err.capture_failed': 'the screenshot failed',
    'err.shot_not_found': 'Screenshot not found',
  };
  Object.assign(root.NSI18n.dicts.ru, ru);
  Object.assign(root.NSI18n.dicts.en, en);
})(typeof window !== 'undefined' ? window : globalThis);
