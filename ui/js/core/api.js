// The only door to the core. UI calls Tauri commands listed in interfaces.md (v1 list + the `app` row of "Границы v2");
// arguments are camelCase (Tauri rule). Anything else throws here, so a typo cannot reach the backend.
(function () {
  const NS = (window.NS = window.NS || {});
  const COMMANDS = new Set([
    // v1
    'list_projects', 'add_project', 'create_project', 'update_project', 'remove_project', 'init_repo', 'repo_info', 'list_tasks', 'get_log', 'submit_task',
    'answer', 'approve_outward', 'cancel', 'retry', 'remove_task', 'undo', 'push_base', 'pull_base', 'pause_all', 'list_agents',
    'get_settings', 'set_settings', 'github_info', 'delete_merged_branches', 'pr_state', 'open_path', 'open_editor', 'open_terminal', 'get_diffstat',
    // v2 (app)
    'list_envs', 'get_catalog', 'refresh_catalog', 'get_limits', 'list_chats', 'get_chat', 'send_chat', 'retry_chat', 'side_ask', 'remove_chat', 'note',
    'answer_outward', 'resume_failed', 'pick_variant', 'stop_swarm', 'nudge', 'switch_model', 'freeze', 'hand_off', 'take_back', 'start_now',
    'not_a_task', 'submit_convoy', 'summary_since', 'diff_file', 'check_custom', 'install_env', 'login_env',
    'kits_inventory', 'kits_export', 'kits_preview', 'kits_install', 'kits_enable', 'kits_snapshots', 'kits_rollback', 'kits_check_update', 'set_secret',
    // not in "Границы v2" by name: A07.2 "Продолжить в терминале" for a chat (the spec gives the behaviour, not the command) - see the report of task 15
    'open_chat_terminal',
    // data that cannot be read at start (G16.4): ask, show the folder, start clean after a confirmation
    'data_state', 'open_data_folder', 'start_fresh',
    // v3 ("Границы v3"): the task log with actions, page by page
    'get_task_log',
    // v4 ("Границы v4"): terminals, windows, catalog, CLI updates, GitHub, config file
    'term_open', 'term_write', 'term_resize', 'term_close', 'term_attach', 'term_detach', 'term_ack', 'term_list',
    'window_detach', 'window_tab', 'window_return',
    'market_search', 'market_item', 'market_plan', 'market_apply', 'market_installed', 'market_remove', 'market_sources', 'market_set_sources',
    'task_snapshots', 'restore_snapshot', 'todo_add', 'todo_set', 'todo_remove', 'take_to_main', 'give_to_agent',
    'cli_status', 'cli_update', 'updates_history', 'gh_publish', 'git_log_page', 'git_revert_commit', 'config_open', 'config_errors',
  ]);
  const EVENTS = new Set(['task-changed', 'task-log', 'task-removed', 'project-changed', 'chat-msg', 'limits-changed', 'catalog-changed', 'frozen',
    'focus-input', 'open-task', 'tauri://drag-enter', 'tauri://drag-leave', 'tauri://drag-drop',
    // app -> window notices named in task 13 without a fixed name: settings file replaced (G16.4), system notifications blocked (R15i.7),
    // tray host missing (R10.2), kit install rolled back at start (G25.9). Names chosen by task 15, see its report.
    'settings-replaced', 'notify-blocked', 'tray-state', 'kits-recovered',
    // v3: actions of agents with output and diff, the answer while it is written
    'task-action', 'chat-action', 'chat-delta',
    // v4
    'term-exit', 'cli-updated', 'new-model', 'config-changed', 'windows-changed', 'window-return', 'terms-restored']);

  const tauri = () => window.__TAURI__;
  function call(cmd, args) {
    if (!COMMANDS.has(cmd)) return Promise.reject(new Error('unknown command ' + cmd));
    return tauri().core.invoke(cmd, args || {});
  }
  async function listen(name, fn) {
    if (!EVENTS.has(name)) throw new Error('unknown event ' + name);
    if (!tauri() || !tauri().event) return () => {};
    return tauri().event.listen(name, (e) => fn(e.payload));
  }
  const errText = (e) => (typeof e === 'string' ? e : (e && e.message) || JSON.stringify(e));

  NS.api = { call, listen, errText, COMMANDS, EVENTS };
})();
