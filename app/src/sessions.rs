//! G68: sessions of Claude Code from a terminal, seen from a project. The files are only read; "Продолжить" makes an ordinary
//! task in a git copy that resumes the session (the copy of the session file is made by the engine, before the first run).
use crate::hub::{Hub, Res};
use crate::App;
use nightshift_core::engine::NewTask;
use nightshift_core::sessions::{self, PAGE};
use serde_json::{json, Value};
use std::path::Path;
use tauri::State;

/// The words of the new task come from the first line of the old session, cut short.
const TITLE_CHARS: usize = 120;

/// One page of the project's sessions: `{items, total, page, page_size, more}`. `root` = the `projects` folder of Claude.
pub fn page_json(root: &Path, project_path: &Path, page: usize) -> Value {
    let total = sessions::count_in(root, project_path);
    let items = sessions::list_in(root, project_path, page);
    json!({ "items": items, "total": total, "page": page, "page_size": PAGE, "more": (page + 1) * PAGE < total })
}

/// "Продолжить": the session `id` of the project as a task (its id). Only a session that is in the project's own list.
pub fn continue_session(hub: &Hub, project_id: &str, id: &str) -> Res<String> {
    let p = hub.project_ready(project_id)?;
    let root = sessions::claude_dir().ok_or("no_home")?.join("projects");
    let head = sessions::find_in(&root, &p.path, id).ok_or_else(|| format!("session_not_found: {id}"))?;
    if head.unreadable {
        return Err(format!("session_unreadable: {id}"));
    }
    let said: String = head.first_prompt.chars().take(TITLE_CHARS).collect();
    let prompt = if said.is_empty() { "Продолжить сессию из терминала".to_string() } else { format!("Продолжить сессию из терминала: {said}") };
    hub.engine.submit_resumed(NewTask { project_id: p.id.clone(), prompt, ..NewTask::default() }, id)
}

#[tauri::command]
pub async fn sessions_list(s: State<'_, App>, project_id: String, page: Option<usize>) -> Res<Value> {
    let p = s.hub.project(&project_id)?;
    let root = sessions::claude_dir().map(|d| d.join("projects"));
    tauri::async_runtime::spawn_blocking(move || root.map(|r| page_json(&r, &p.path, page.unwrap_or(0))).unwrap_or_else(|| page_json(Path::new(""), &p.path, 0)))
        .await
        .map_err(crate::hub::e2s)
}

#[tauri::command]
pub async fn session_continue(s: State<'_, App>, project_id: String, id: String) -> Res<String> {
    let hub = s.hub.clone();
    tauri::async_runtime::spawn_blocking(move || continue_session(&hub, &project_id, &id)).await.map_err(crate::hub::e2s)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::*;
    use nightshift_core::agent::FakeAdapter;
    use nightshift_core::model::Status;

    fn session(root: &Path, project: &Path, id: &str, text: &str) {
        let dir = root.join(sessions::key_for(project));
        std::fs::create_dir_all(&dir).unwrap();
        let line = serde_json::json!({ "type": "user", "timestamp": "2026-10-20T10:00:00Z", "message": { "role": "user", "content": text } });
        std::fs::write(dir.join(format!("{id}.jsonl")), format!("{line}\n")).unwrap();
    }

    #[test]
    fn a_page_says_how_many_there_are_and_whether_more_follow() {
        let s = Scratch::new("sess-page");
        let (root, project) = (s.0.join("projects"), s.0.join("p"));
        std::fs::create_dir(&project).unwrap();
        assert_eq!(page_json(&root, &project, 0), json!({ "items": [], "total": 0, "page": 0, "page_size": 50, "more": false }));
        for n in 0..51 {
            session(&root, &project, &format!("s{n:02}"), "x");
        }
        let (a, b) = (page_json(&root, &project, 0), page_json(&root, &project, 1));
        assert_eq!((a["items"].as_array().unwrap().len(), a["more"].clone(), a["total"].clone()), (50, json!(true), json!(51)));
        assert_eq!((b["items"].as_array().unwrap().len(), b["more"].clone()), (1, json!(false)));
    }

    #[test]
    fn continuing_makes_a_task_that_resumes_the_session_and_only_for_a_session_of_the_project() {
        let rig = Rig::new(FakeAdapter::new(vec![]));
        let cfg = rig.s.0.join("claude");
        std::env::set_var("CLAUDE_CONFIG_DIR", &cfg);
        session(&cfg.join("projects"), &rig.repo, "abc-1", "Add the cart page");
        let id = continue_session(&rig.hub, &rig.project.id, "abc-1").unwrap();
        let t = rig.task(&id);
        assert_eq!(t.resume_session.as_deref(), Some("abc-1"));
        assert_eq!(t.prompt, "Продолжить сессию из терминала: Add the cart page");
        rig.wait_task(&id, "task finished", |t| t.status != Status::Queued && t.worktree.is_some());
        assert!(continue_session(&rig.hub, &rig.project.id, "other").unwrap_err().starts_with("session_not_found"));
        assert!(continue_session(&rig.hub, &rig.project.id, "../abc-1").unwrap_err().starts_with("session_not_found"));
        assert!(continue_session(&rig.hub, "nope", "abc-1").unwrap_err().starts_with("project_not_found"));
    }
}
