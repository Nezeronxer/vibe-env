// Strings of v6 task 27: "думает" right after sending (G80), the thoughts of the model, the queue on disk (G62) and the
// system lines of the chat in words, not raw keys. Loaded after i18n-chatx.js.
(function (root) {
  const ru = {
    // the spinner's own words (G84: Claude Code `spinnerVerbs` replaces or adds to them), | between the words
    'think.verbs': 'Думаю|Читаю код|Прикидываю|Взвешиваю|Сверяюсь|Собираю мысли|Ищу ответ',
    'think.long': 'Долго думает', 'think.stop': 'Стоп', 'think.thoughts': 'Думает', 'think.thought': 'Мысли', 'think.open': 'Показать мысли',
    'chat6.queue_fail': 'Не удалось поставить в очередь: {why}',
    // the lines of the app in the feed: `key: details` from core::chat::keys
    'err.will_answer_after': 'Лимит исчерпан. Отвечу после {detail}', 'err.note_queued': 'Передам задаче «{detail}», когда агент закончит шаг',
    'err.note_delivered': 'Агент получил в {detail}', 'err.answer_passed': 'Ответ передан задаче «{detail}»', 'err.note_not_passed': 'Задаче не передано: {detail}. Отвечаю как на обычное сообщение',
    'err.back_to_question': 'Это был вопрос: задача отменена, отвечаю на него', 'err.new_session_started': 'CLI потерял прошлый разговор: начат новый, с последними 20 сообщениями',
    'err.task_not_created': 'Задача не создана: {detail}', 'err.this_is_task': 'Это задача: {detail}',
    'err.task_already_merged': 'задача уже влита', 'err.task_cancelled': 'задача отменена',
    // reasons after "Не ответил: " (core::chat::reason)
    'err.interrupted': 'приложение закрылось посреди ответа. Сам не повторяю, чтобы не тратить лимит дважды',
    'err.cli_error': 'CLI ответил ошибкой: {detail}', 'err.run_failed': 'CLI не запустился: {detail}', 'err.agent_no_result': 'CLI закончил без ответа',
    'err.auth_required': 'нужен вход в CLI: {detail}', 'err.env_unavailable': 'среда недоступна: {detail}', 'err.agent_not_found': 'нет такой среды: {detail}',
    'err.project_not_found': 'проект не найден', 'err.copy_failed': 'не получилось подготовить копию проекта: {detail}', 'err.model_rejected': 'CLI не принял модель: {detail}',
    'err.empty_message': 'Пустое сообщение', 'err.empty_message_id': 'Сообщение без id', 'err.queued_not_found': 'Сообщение уже ушло из очереди',
  };
  const en = {
    'think.verbs': 'Thinking|Reading the code|Weighing|Pondering|Checking|Gathering thoughts|Looking for the answer',
    'think.long': 'Thinking for long', 'think.stop': 'Stop', 'think.thoughts': 'Thinking', 'think.thought': 'Thoughts', 'think.open': 'Show the thoughts',
    'chat6.queue_fail': 'Could not queue it: {why}',
    'err.will_answer_after': 'The limit is used up. I will answer after {detail}', 'err.note_queued': 'Goes to the task «{detail}» when the agent ends its step',
    'err.note_delivered': 'The agent got it at {detail}', 'err.answer_passed': 'The answer went to the task «{detail}»', 'err.note_not_passed': 'Not passed to the task: {detail}. Answering it as a plain message',
    'err.back_to_question': 'It was a question: the task is cancelled, answering it', 'err.new_session_started': 'The CLI lost the earlier conversation: a new one started with the last 20 messages',
    'err.task_not_created': 'The task was not created: {detail}', 'err.this_is_task': 'This is a task: {detail}',
    'err.task_already_merged': 'the task is merged already', 'err.task_cancelled': 'the task is cancelled',
    'err.interrupted': 'the app closed in the middle of the answer. Not repeated by itself, so the limit is not spent twice',
    'err.cli_error': 'the CLI ended with an error: {detail}', 'err.run_failed': 'the CLI did not start: {detail}', 'err.agent_no_result': 'the CLI ended without an answer',
    'err.auth_required': 'log in to the CLI: {detail}', 'err.env_unavailable': 'the env is not available: {detail}', 'err.agent_not_found': 'no such env: {detail}',
    'err.project_not_found': 'the project is not found', 'err.copy_failed': 'could not prepare the copy of the project: {detail}', 'err.model_rejected': 'the CLI refused the model: {detail}',
    'err.empty_message': 'The message is empty', 'err.empty_message_id': 'The message has no id', 'err.queued_not_found': 'The message has left the queue already',
  };
  Object.assign(root.NSI18n.dicts.ru, ru);
  Object.assign(root.NSI18n.dicts.en, en);
})(typeof window !== 'undefined' ? window : globalThis);
