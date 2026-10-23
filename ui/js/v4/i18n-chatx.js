// Strings of F12: the message queue and "Сразу", the side question, the next-step hint, "Выполнить" on shell blocks, "Остановлено". Loaded after i18n-shell.js.
(function (root) {
  const ru = {
    'chatx.queued': 'в очереди', 'chatx.queued_d': 'уйдёт, когда агент закончит', 'chatx.edit': 'Править', 'chatx.drop': 'Убрать из очереди',
    'chatx.now': 'Сразу', 'chatx.now_d': 'Остановить текущий запуск и передать это сообщение первым',
    'chatx.run': 'Выполнить', 'chatx.run_d': 'Вставить в терминал. Enter нажмёшь сам', 'chatx.run_toast': 'Команда в терминале. Проверь её и нажми Enter.', 'chatx.run_title': 'Команда',
    'chatx.side': 'Спросить', 'chatx.side_d': 'Боковой вопрос (⌘;): ответ не попадёт в историю задачи', 'chatx.side_note': 'Отдельный короткий вопрос. Только чтение: в историю он не пишется и агента не будит.',
    'chatx.side_ph': 'Спросить о задаче…', 'chatx.side_send': 'Спросить', 'chatx.side_busy': 'Читаю код…', 'chatx.side_close': 'Закрыть', 'chatx.side_you': 'Вы', 'chatx.side_fail': 'Не получилось: {why}',
    'chatx.hint_on': 'Подсказывать следующий шаг', 'chatx.hint_on_d': 'Серая реплика в пустом поле. Tab принимает, любая клавиша прячет',
    'chatx.h_push': 'Запушить?', 'chatx.h_mobile': 'Проверь на мобильном', 'chatx.h_tests': 'Напиши тесты на «{title}»', 'chatx.h_do': 'Сделай это', 'chatx.h_why': 'Почему не вышло? Попробуй ещё раз',
    'feed.stopped': 'Остановлено',
    'err.empty_or_too_long': 'Вопрос пустой или слишком длинный', 'err.limit_used_up': 'Лимит исчерпан, спросите позже', 'err.chat_not_found': 'Чат не найден', 'err.stopped': 'Остановлено',
  };
  const en = {
    'chatx.queued': 'queued', 'chatx.queued_d': 'goes when the agent is done', 'chatx.edit': 'Edit', 'chatx.drop': 'Remove from the queue',
    'chatx.now': 'Now', 'chatx.now_d': 'Stop the current run and pass this message first',
    'chatx.run': 'Run', 'chatx.run_d': 'Paste into a terminal. You press Enter yourself', 'chatx.run_toast': 'The command is in the terminal. Check it and press Enter.', 'chatx.run_title': 'Command',
    'chatx.side': 'Ask', 'chatx.side_d': 'Side question (⌘;): the answer stays out of the task history', 'chatx.side_note': 'A separate short question. Read-only: it is not written to the history and does not wake the agent.',
    'chatx.side_ph': 'Ask about the task…', 'chatx.side_send': 'Ask', 'chatx.side_busy': 'Reading the code…', 'chatx.side_close': 'Close', 'chatx.side_you': 'You', 'chatx.side_fail': 'Failed: {why}',
    'chatx.hint_on': 'Suggest the next step', 'chatx.hint_on_d': 'A grey line in the empty field. Tab accepts, any key hides it',
    'chatx.h_push': 'Push it?', 'chatx.h_mobile': 'Check it on mobile', 'chatx.h_tests': 'Write tests for "{title}"', 'chatx.h_do': 'Do this', 'chatx.h_why': 'Why did it fail? Try again',
    'feed.stopped': 'Stopped',
    'err.empty_or_too_long': 'The question is empty or too long', 'err.limit_used_up': 'The limit is used up, ask later', 'err.chat_not_found': 'Chat not found', 'err.stopped': 'Stopped',
  };
  Object.assign(root.NSI18n.dicts.ru, ru);
  Object.assign(root.NSI18n.dicts.en, en);
})(typeof window !== 'undefined' ? window : globalThis);
