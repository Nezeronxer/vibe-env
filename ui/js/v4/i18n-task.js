// Strings of F13: rollback to a turn, the "До мержа" list, "Забрать к себе" / "Отдать агенту".
(function (root) {
  const ru = {
    'snap.rollback': 'Вернуть код сюда', 'snap.rollback_d': 'Вернуть код задачи к этому месту. Текущее состояние сохранится, откат можно отменить.', 'snap.sure': 'Точно вернуть?',
    'snap.title': 'Ходы', 'snap.hint': 'Снимок кода перед каждым запуском роли. Откат обратим: перед ним состояние тоже сохраняется.',
    'snap.row': 'Ход {n} · {role} · {time}', 'snap.none': 'Снимков пока нет', 'snap.done': 'Код возвращён к ходу {n}. Состояние до отката - ход {saved}.',
    'todo.title': 'До мержа', 'todo.hint': 'Пока в списке есть неотмеченное, задача не сливается. Список видит исполнитель.', 'todo.empty': 'Пунктов нет - ничто не держит мерж.',
    'todo.add': 'Что ещё сделать до мержа', 'todo.add_btn': 'Добавить', 'todo.remove': 'Убрать пункт', 'todo.by_reviewer': 'ревьюер', 'todo.open': '{n} открыто',
    'take.take': 'Забрать к себе', 'take.give': 'Отдать агенту', 'take.take_d': 'Переключить папку проекта на ветку задачи', 'take.give_d': 'Вернуть папку на прежнюю ветку, задача снова в своей копии',
    'take.taken': 'Папка проекта стоит на ветке задачи{prev}. Работай в ней как обычно, потом нажми «Отдать агенту».', 'take.taken_prev': ' (была {prev})',
    'take.done_take': 'Папка проекта теперь на ветке задачи', 'take.done_give': 'Папка проекта вернулась на прежнюю ветку',
    'need.todos_open': 'Мерж ждёт', 'need.todos_cont': 'Проверить ещё раз',
    'err.todos_open': 'В списке «До мержа» не отмечено пунктов: {detail}. Отметь их в карточке задачи, и мерж пойдёт сам.',
    'err.main_dirty': 'В папке проекта есть незакоммиченные правки. Закоммить или убери их, потом повтори - ничего не потеряно.',
    'err.main_detached': 'Папка проекта не на ветке (detached HEAD). Встань на ветку и повтори.',
    'err.main_busy': 'В папке проекта идёт слияние или перебазирование. Закончи его и повтори.',
    'err.taken_to_main': 'Задача у тебя в папке проекта. Сначала нажми «Отдать агенту».',
    'err.already_taken': 'Задача уже в папке проекта.', 'err.not_takeable': 'Забрать можно задачу, которая сейчас не работает: «Не вышло» или «Ждёт тебя».', 'err.not_taken': 'Задача не в папке проекта.',
    'err.rebase_in_progress': 'Задача сейчас перебазируется. Дождись конца.', 'err.already_merged': 'Задача уже слита, откатывать нечего.', 'err.empty_todo': 'Пустой пункт.',
    'err.no_snapshot': 'Такого снимка нет.',
  };
  const en = {
    'snap.rollback': 'Restore code here', 'snap.rollback_d': 'Put the task code back to this point. The current state is saved, the rollback can be undone.', 'snap.sure': 'Really restore?',
    'snap.title': 'Turns', 'snap.hint': 'A snapshot of the code before each role runs. Reversible: the state before a rollback is saved too.',
    'snap.row': 'Turn {n} · {role} · {time}', 'snap.none': 'No snapshots yet', 'snap.done': 'Code restored to turn {n}. The state before is turn {saved}.',
    'todo.title': 'Before merge', 'todo.hint': 'While anything is unchecked the task is not merged. The executor sees the list.', 'todo.empty': 'No items - nothing holds the merge.',
    'todo.add': 'What else must be done before the merge', 'todo.add_btn': 'Add', 'todo.remove': 'Remove item', 'todo.by_reviewer': 'reviewer', 'todo.open': '{n} open',
    'take.take': 'Take to my folder', 'take.give': 'Give back to the agent', 'take.take_d': 'Switch the project folder to the task branch', 'take.give_d': 'Put the folder back on its branch; the task returns to its own copy',
    'take.taken': 'The project folder is on the task branch{prev}. Work in it as usual, then press "Give back to the agent".', 'take.taken_prev': ' (was {prev})',
    'take.done_take': 'The project folder is on the task branch now', 'take.done_give': 'The project folder is back on its branch',
    'need.todos_open': 'Merge is waiting', 'need.todos_cont': 'Check again',
    'err.todos_open': 'The "Before merge" list has unchecked items: {detail}. Tick them in the task card and the merge goes on by itself.',
    'err.main_dirty': 'The project folder has uncommitted changes. Commit or drop them, then retry - nothing was lost.',
    'err.main_detached': 'The project folder is not on a branch (detached HEAD). Check out a branch and retry.',
    'err.main_busy': 'A merge or rebase is in progress in the project folder. Finish it and retry.',
    'err.taken_to_main': 'The task is in your project folder. Press "Give back to the agent" first.',
    'err.already_taken': 'The task is already in the project folder.', 'err.not_takeable': 'Only a task that is not running can be taken: "Failed" or "Waiting for you".', 'err.not_taken': 'The task is not in the project folder.',
    'err.rebase_in_progress': 'A rebase is running in the task copy. Wait for it to end.', 'err.already_merged': 'The task is already merged, nothing to roll back.', 'err.empty_todo': 'Empty item.',
    'err.no_snapshot': 'No such snapshot.',
  };
  Object.assign(root.NSI18n.dicts.ru, ru);
  Object.assign(root.NSI18n.dicts.en, en);
})(typeof window !== 'undefined' ? window : globalThis);
