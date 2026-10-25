// Strings of v6 task 29: the chat as a shell of the CLI - the menu "/" (what the env has), "В терминале" for every env
// and back, images in the input field. Loaded after i18n-chat6.js.
(function (root) {
  const ru = {
    'slash.title': 'Что есть в среде', 'slash.skills': 'Скиллы', 'slash.commands': 'Команды', 'slash.mcp': 'MCP', 'slash.mods': 'Моды',
    'slash.mods_d': 'рисуют в терминале - открыть', 'slash.mcp_d': 'модель зовёт их сама, вставлять не нужно',
    'slash.empty': 'В среде нет скиллов - открыть Каталог', 'slash.none': 'Ничего не нашлось по «{q}»', 'slash.loading': 'Смотрю, что есть в среде…',
    'slash.more': 'и ещё {n}', 'slash.keys': '↑↓ выбрать · Enter вставить · Esc закрыть', 'slash.btn': 'Скиллы и команды среды',
    'slash.src.global': 'твоё', 'slash.src.project': 'проект', 'slash.src.extension': 'расширение', 'slash.src.plugin': 'плагин {name}',
    'shell.term': 'В терминале', 'shell.term_on': 'в терминале', 'shell.term_on_d': 'Этот разговор сейчас открыт в терминале',
    'shell.term_warn_t': 'Разговор идёт в терминале', 'shell.term_warn': 'Этот чат сейчас открыт в терминале. Отправить отсюда всё равно? Ответ уйдёт в ту же сессию, терминал о нём не узнает.',
    'shell.term_send': 'Отправить', 'shell.new_term': 'Новый разговор в терминале', 'shell.new_term_q': 'Открыть в терминале новый разговор этой среды? Старый останется в чате.',
    'shell.by_docs': 'Команда продолжения для {env} взята из документации, на этой машине не проверена',
    'shell.no_resume': 'откроется новый разговор',
    'img.attach': 'Приложить картинку', 'img.remove': 'Убрать картинку', 'img.alt': 'Картинка {n}', 'img.drop': 'Отпусти - приложу картинку',
    'img.chip': 'картинка', 'img.limit': 'До 5 картинок по 10 МБ', 'img.task': 'Картинки уходят в разговор с моделью, не в задачу: дождись конца задачи',
    'err.no_session': 'у этого чата ещё нет разговора в CLI: модель не ответила ни разу',
    'err.cannot_resume': 'эта среда не умеет продолжать разговор в терминале', 'err.session_not_found': 'CLI не нашёл разговор этого чата',
    'err.opened_in_terminal': 'Чат открыт в терминале. Правки файлов там не попадут в проект: копия чата только для чтения',
    'err.continued_in_terminal': 'Разговор продолжался в терминале, история там',
    'err.back_from_terminal': 'Чат вернулся из терминала. Сообщений из терминала дочитано: {detail}',
    'err.not_an_image': 'Это не картинка: подходят PNG, JPEG, GIF и WebP', 'err.image_too_big': 'Картинка больше 10 МБ',
    'err.too_many_images': 'Не больше 5 картинок в одном сообщении', 'err.images_not_supported': 'Эта среда не принимает картинки',
    'err.attachment_not_found': 'Картинка пропала: приложи её ещё раз', 'err.file_not_found': 'Файл не найден', 'err.attach_failed': 'Не удалось сохранить картинку: {detail}',
    'err.bad_base64': 'Картинка пришла повреждённой',
  };
  const en = {
    'slash.title': 'What the env has', 'slash.skills': 'Skills', 'slash.commands': 'Commands', 'slash.mcp': 'MCP', 'slash.mods': 'Mods',
    'slash.mods_d': 'they draw in the terminal - open it', 'slash.mcp_d': 'the model calls them itself, nothing to insert',
    'slash.empty': 'The env has no skills - open the Catalog', 'slash.none': 'Nothing found for «{q}»', 'slash.loading': 'Looking at what the env has…',
    'slash.more': 'and {n} more', 'slash.keys': '↑↓ choose · Enter insert · Esc close', 'slash.btn': 'Skills and commands of the env',
    'slash.src.global': 'yours', 'slash.src.project': 'project', 'slash.src.extension': 'extension', 'slash.src.plugin': 'plugin {name}',
    'shell.term': 'In the terminal', 'shell.term_on': 'in the terminal', 'shell.term_on_d': 'This conversation is open in a terminal now',
    'shell.term_warn_t': 'The conversation goes on in the terminal', 'shell.term_warn': 'This chat is open in a terminal now. Send from here anyway? The answer goes to the same session; the terminal will not know of it.',
    'shell.term_send': 'Send', 'shell.new_term': 'New conversation in the terminal', 'shell.new_term_q': 'Open a new conversation of this env in the terminal? The old one stays in the chat.',
    'shell.by_docs': 'The command that continues a {env} session comes from its docs and was not checked on this machine',
    'shell.no_resume': 'a new conversation opens',
    'img.attach': 'Attach an image', 'img.remove': 'Remove the image', 'img.alt': 'Image {n}', 'img.drop': 'Drop it - I will attach the image',
    'img.chip': 'image', 'img.limit': 'Up to 5 images of 10 MB', 'img.task': 'Images go to the conversation with the model, not to a task: wait for the task to end',
    'err.no_session': 'this chat has no CLI conversation yet: the model has not answered once',
    'err.cannot_resume': 'this env cannot continue a conversation in a terminal', 'err.session_not_found': 'the CLI did not find the conversation of this chat',
    'err.opened_in_terminal': 'The chat is open in a terminal. File edits made there do not reach the project: the chat copy is read-only',
    'err.continued_in_terminal': 'The conversation went on in the terminal; its history is there',
    'err.back_from_terminal': 'The chat is back from the terminal. Messages read from it: {detail}',
    'err.not_an_image': 'Not an image: PNG, JPEG, GIF and WebP work', 'err.image_too_big': 'The image is over 10 MB',
    'err.too_many_images': 'At most 5 images in one message', 'err.images_not_supported': 'This env does not take images',
    'err.attachment_not_found': 'The image is gone: attach it again', 'err.file_not_found': 'The file is not found', 'err.attach_failed': 'Could not keep the image: {detail}',
    'err.bad_base64': 'The image came broken',
  };
  Object.assign(root.NSI18n.dicts.ru, ru);
  Object.assign(root.NSI18n.dicts.en, en);
})(typeof window !== 'undefined' ? window : globalThis);
