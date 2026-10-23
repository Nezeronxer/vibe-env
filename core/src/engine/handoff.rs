//! F13: rollback to a turn (snapshots of the copy), the "До мержа" list that stops the merge, and
//! "Забрать к себе" / "Отдать агенту" (the project folder on the task's branch and back).
use super::*;

/// `special` of the question that parks a task whose "До мержа" list still has open items.
pub(super) const SPECIAL_TODOS: &str = "todos_open";
const TODOS_QUESTION: &str = "todos_open";
const TODO_MAX: usize = 200;

pub(super) fn open_todos(t: &Task) -> usize {
    t.todos.iter().filter(|x| !x.done).count()
}

/// The "До мержа" list as the executor reads it; None when nothing is open.
pub fn todos_block(todos: &[Todo]) -> Option<String> {
    let open: Vec<String> = todos.iter().filter(|x| !x.done).map(|x| format!("- {}", x.text)).collect();
    (!open.is_empty()).then(|| format!("Before the merge the list \"До мержа\" must be empty. Open items (do them; the person or the reviewer ticks them off):\n{}", open.join("\n")))
}

/// Git's text for the person: the refusals of this file are keys (`main_dirty` ...), not git sentences.
fn herr(e: git::GitError) -> String {
    let s = flow::gerr(e);
    for pre in ["advance blocked: ", "git: "] {
        if let Some(r) = s.strip_prefix(pre) {
            if ["main_", "rebase_in_progress", "no_snapshot", "not_a_repo"].iter().any(|k| r.starts_with(k)) {
                return r.to_string();
            }
        }
    }
    s
}

impl Engine {
    fn not_taken(&self, id: &str) -> Result<(), String> {
        if self.task(id).is_some_and(|t| t.taken_from.is_some()) {
            return Err("taken_to_main".into());
        }
        Ok(())
    }
    /// The commands that would start work in the copy are refused while the project folder has its branch.
    pub(super) fn guard_taken(&self, id: &str) -> Result<(), String> {
        self.not_taken(id)
    }

    // ---- snapshots (#5) ----

    /// Before a run of a role that writes: the copy as it is is saved (never blocks the run).
    pub(super) fn snap_before_run(&self, t: &Task, wt: &std::path::Path, role: RoleKind) {
        match self.with_git(wt, || git::snapshot(wt, &t.id, &format!("{role:?}").to_lowercase())) {
            Ok(n) => self.log(&t.id, LogKind::Git, &format!("snapshot {n}")),
            Err(e) => self.log(&t.id, LogKind::Git, &format!("snapshot failed: {e}")),
        }
    }

    pub fn snapshots(&self, id: &str) -> Result<Vec<git::Snapshot>, String> {
        let t = self.task(id).ok_or("task_not_found")?;
        let Some(wt) = t.worktree.filter(|w| w.exists()) else { return Ok(vec![]) };
        Ok(git::snapshots(&wt, id))
    }

    /// "Вернуть код сюда": the copy goes back to snapshot `n`; the state it had is saved as a snapshot first,
    /// so this can be undone. Returns that new snapshot's number. Not while the task runs, not in the main folder.
    pub fn restore_snapshot(&self, id: &str, n: u32) -> Result<u32, String> {
        let _h = self.hold(id, false)?;
        self.not_taken(id)?;
        let t = self.task(id).ok_or("task_not_found")?;
        let wt = t.worktree.clone().filter(|w| w.exists()).ok_or("no_worktree")?;
        if t.outcome.commit.is_some() {
            return Err("already_merged".into());
        }
        let saved = self.with_git(&wt, || git::restore_snapshot(&wt, id, n)).map_err(herr)?;
        self.log(id, LogKind::Human, &format!("rolled back to snapshot {n}; the state before is snapshot {saved}"));
        self.emit(EngineEvent::TaskChanged(self.task(id).ok_or("task_not_found")?));
        Ok(saved)
    }

    // ---- the "До мержа" list (#6) ----

    pub fn todo_add(&self, id: &str, text: &str, by: &str) -> Result<(), String> {
        let text: String = text.trim().chars().take(TODO_MAX).collect();
        if text.is_empty() {
            return Err("empty_todo".into());
        }
        let by = if by == "reviewer" { "reviewer" } else { "human" }.to_string();
        let item = Todo { id: uuid::Uuid::new_v4().simple().to_string()[..8].to_string(), text, done: false, by };
        self.cas(id, "task_not_found", |t| !matches!(t.status, Status::Done | Status::Cancelled | Status::Reverted), |t| t.todos.push(item))?;
        Ok(())
    }

    pub fn todo_set(&self, id: &str, todo_id: &str, done: bool) -> Result<(), String> {
        let t = self.cas(id, "task_not_found", |t| t.todos.iter().any(|x| x.id == todo_id), |t| {
            if let Some(x) = t.todos.iter_mut().find(|x| x.id == todo_id) {
                x.done = done;
            }
        })?;
        self.resume_if_todos_closed(&t);
        Ok(())
    }

    pub fn todo_remove(&self, id: &str, todo_id: &str) -> Result<(), String> {
        let t = self.cas(id, "task_not_found", |t| t.todos.iter().any(|x| x.id == todo_id), |t| t.todos.retain(|x| x.id != todo_id))?;
        self.resume_if_todos_closed(&t);
        Ok(())
    }

    /// The task waits only for this list and it is closed now: the merge goes on by itself.
    fn resume_if_todos_closed(&self, t: &Task) {
        if open_todos(t) > 0 || self.state(&t.id).special.as_deref() != Some(SPECIAL_TODOS) {
            return;
        }
        let stage = self.pending(&t.id).stage.unwrap_or(Status::Merging);
        let ok = self.cas(&t.id, "not_waiting", |x| x.status == Status::NeedsHuman, |x| {
            x.question = None;
            x.status = stage;
            x.resume_at = None;
        });
        if ok.is_ok() {
            let _ = self.update_state(&t.id, |s| s.special = None);
            self.clear_pending(&t.id);
            self.log(&t.id, LogKind::Stage, "todos closed, merge goes on");
        }
    }

    pub(super) fn open_todos_now(&self, id: &str) -> usize {
        self.task(id).map_or(0, |t| open_todos(&t))
    }

    /// What a verdict does to the list, in one step on the stored task: findings become items, an approval closes them.
    pub(super) fn review_todos(&self, id: &str, approved: bool, issues: &[String]) -> Result<Vec<Todo>, String> {
        let issues: Vec<String> = issues.iter().filter(|i| i.as_str() != crate::prompt::BAD_FORMAT).map(|i| i.trim().chars().take(200).collect()).filter(|i: &String| !i.is_empty()).collect();
        let t = self.cas(id, "task_not_found", |_| true, |t| {
            if approved {
                t.todos.iter_mut().filter(|x| x.by == "reviewer").for_each(|x| x.done = true);
                return;
            }
            for text in issues {
                match t.todos.iter_mut().find(|x| x.by == "reviewer" && x.text == text) {
                    Some(x) => x.done = false,
                    None => t.todos.push(Todo { id: uuid::Uuid::new_v4().simple().to_string()[..8].to_string(), text, done: false, by: "reviewer".into() }),
                }
            }
        })?;
        Ok(t.todos)
    }

    /// The merge step stops here while an item is open: the task waits for the person with the reason.
    pub(super) fn park_on_todos(&self, t: &mut Task, p: &Project) -> flow::Res {
        let n = self.open_todos_now(&t.id);
        let text = format!("{TODOS_QUESTION}: {n}");
        let options = vec!["Continue".to_string()];
        self.set_step(t, Step::Merge, StepState::Pending)?;
        t.question = Some(Question { text: text.clone(), options: options.clone(), kind: QuestionKind::Agent });
        t.status = Status::NeedsHuman;
        let mut pend = self.pending(&t.id);
        pend.stage = Some(Status::Merging);
        self.set_pending(&t.id, &pend)?;
        self.update_state(&t.id, |s| s.special = Some(SPECIAL_TODOS.to_string()))?;
        self.save(t)?;
        self.log(&t.id, LogKind::Stage, &text);
        self.notify(NoticeKind::Question, t, p, text, options);
        // the last item may have been ticked while this was being saved
        if let Some(now) = self.task(&t.id) {
            self.resume_if_todos_closed(&now);
        }
        Ok(flow::Flow::Stop)
    }

    // ---- "Забрать к себе" / "Отдать агенту" (#4) ----

    /// The project folder goes to the task's branch (clean folder only). The copy keeps its files.
    pub fn take_to_main(&self, id: &str) -> Result<(), String> {
        let _h = self.hold(id, false)?;
        let mut t = self.task(id).ok_or("task_not_found")?;
        if t.taken_from.is_some() {
            return Err("already_taken".into());
        }
        // statuses nothing starts by itself: a waiting task would be resumed into a detached copy
        if !matches!(t.status, Status::Failed | Status::NeedsHuman) {
            return Err("not_takeable".into());
        }
        let p = self.project(&t.project_id).ok_or("project_not_found")?;
        let wt = t.worktree.clone().filter(|w| w.exists()).ok_or("no_worktree")?;
        if git::rebase_in_progress(&wt) {
            return Err("rebase_in_progress".into());
        }
        // the work in the copy is saved as a snapshot first; the branch itself is not touched
        let _ = self.with_git(&wt, || git::snapshot(&wt, id, "take_to_main"));
        let prev = self.with_git(&p.path, || git::take_to_main(&p.path, &wt, &t.branch)).map_err(herr)?;
        t.taken_from = Some(prev.clone());
        self.save(&t)?;
        self.log(id, LogKind::Git, &format!("project folder: {prev} -> {}", t.branch));
        self.emit(EngineEvent::ProjectChanged(p.id.clone()));
        Ok(())
    }

    /// The folder goes back to the branch it was on, the task gets its branch back in its copy.
    pub fn give_to_agent(&self, id: &str) -> Result<(), String> {
        let _h = self.hold(id, false)?;
        let mut t = self.task(id).ok_or("task_not_found")?;
        let prev = t.taken_from.clone().ok_or("not_taken")?;
        let p = self.project(&t.project_id).ok_or("project_not_found")?;
        let wt = t.worktree.clone().filter(|w| w.exists()).ok_or("no_worktree")?;
        self.with_git(&p.path, || git::give_to_agent(&p.path, &wt, &t.branch, &prev, &p.base_branch)).map_err(herr)?;
        t.taken_from = None;
        self.save(&t)?;
        self.log(id, LogKind::Git, &format!("project folder back on {prev}"));
        self.emit(EngineEvent::ProjectChanged(p.id.clone()));
        Ok(())
    }
}
