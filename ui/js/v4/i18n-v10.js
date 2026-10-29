// Strings of v10 (Agentation notes 29.10.2026): the settings menu like macOS - ten items with coloured tiles, a search, tabs inside an item.
(function (root) {
  const ru = {
    'nav10.basic': 'Основное', 'nav10.look': 'Внешний вид', 'nav10.terminal': 'Терминал', 'nav10.agents': 'Агенты', 'nav10.quality': 'Качество и роли',
    'nav10.swarm': 'Рой и фабрика', 'nav10.limits': 'Лимиты и экономия', 'nav10.notify': 'Уведомления', 'nav10.keys': 'Клавиши', 'nav10.projects': 'Проекты',
    'nav10.advanced': 'Расширенное', 'nav10.search': 'Поиск', 'nav10.none': 'Ничего не нашлось. Enter - поиск по всем ключам', 'nav10.env': 'Агент',
    'nav10.part.envs4': 'Обновления',
    'view10.label': 'Вид сессии', 'view10.chat': 'Чат', 'view10.term': 'Терминал', 'view10.chat_d': 'Разговор чатом',
    'view10.term_d': 'Тот же разговор в самом CLI. «Чат» закроет его и вернёт историю сюда', 'view10.term_wait': 'Терминал появится после первого сообщения',
    'nav10.kw.basic': 'язык вид окна clawd подписи language', 'nav10.kw.look': 'тема цвет акцент шрифт прозрачность размытие скругление theme color font',
    'nav10.kw.terminal': 'курсор история прокрутка шрифт терминала cursor scrollback', 'nav10.kw.agents': 'модель cli конфиги mcp скиллы права песочница запуск вход версия обновление model',
    'nav10.kw.quality': 'проверки тесты ревью роли исполнитель ревьюер review tests roles', 'nav10.kw.swarm': 'рой фабрика параллельно очередь swarm factory',
    'nav10.kw.limits': 'лимиты бюджет деньги токены экономия budget tokens', 'nav10.kw.notify': 'уведомления звук тихие часы notifications sound',
    'nav10.kw.keys': 'горячие клавиши сочетания shortcuts hotkeys', 'nav10.kw.projects': 'проект папка github команды project folder',
    'nav10.kw.advanced': 'все ключи json расширенное advanced',
  };
  const en = {
    'nav10.basic': 'General', 'nav10.look': 'Appearance', 'nav10.terminal': 'Terminal', 'nav10.agents': 'Agents', 'nav10.quality': 'Quality and roles',
    'nav10.swarm': 'Swarm and factory', 'nav10.limits': 'Limits and savings', 'nav10.notify': 'Notifications', 'nav10.keys': 'Keys', 'nav10.projects': 'Projects',
    'nav10.advanced': 'Advanced', 'nav10.search': 'Search', 'nav10.none': 'Nothing found. Enter searches every key', 'nav10.env': 'Agent',
    'nav10.part.envs4': 'Updates',
    'view10.label': 'Session view', 'view10.chat': 'Chat', 'view10.term': 'Terminal', 'view10.chat_d': 'The talk as a chat',
    'view10.term_d': 'The same talk in the CLI itself. «Chat» ends it and brings the history back here', 'view10.term_wait': 'The terminal comes after the first message',
    'nav10.kw.basic': 'language window clawd labels', 'nav10.kw.look': 'theme color accent font transparency blur radius',
    'nav10.kw.terminal': 'cursor scrollback terminal font', 'nav10.kw.agents': 'model cli configs mcp skills permissions sandbox launch login version update',
    'nav10.kw.quality': 'checks tests review roles worker reviewer', 'nav10.kw.swarm': 'swarm factory parallel queue',
    'nav10.kw.limits': 'limits budget money tokens savings', 'nav10.kw.notify': 'notifications sound quiet hours',
    'nav10.kw.keys': 'shortcuts hotkeys keys', 'nav10.kw.projects': 'project folder github commands',
    'nav10.kw.advanced': 'every key json advanced',
  };
  Object.assign(root.NSI18n.dicts.ru, ru);
  Object.assign(root.NSI18n.dicts.en, en);
})(typeof window !== 'undefined' ? window : globalThis);
