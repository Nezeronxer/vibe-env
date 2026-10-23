//! Practices from the videos (research-youtube.md, stories 305-311): the hand-off note, "Сжать", ports and a
//! database name per copy, raw output of a failed check kept in the copy.
use super::*;
use crate::prompt;

/// First `PORT` of the copies (Cole Medin's setup starts at 4000); copy N gets `PORT_BASE + 10·N`.
pub(super) const PORT_BASE: u16 = 4000;
/// The app's own folder in a copy: never committed (`prepare` hides it), the agent can read it.
pub(super) const APP_DIR: &str = ".nightshift";
const HANDOFF_FILE: &str = "handoff.md";

/// Where the raw output of a failed check is kept in the copy.
pub(super) fn raw_output_path(wt: &std::path::Path, check: &str) -> PathBuf {
    let name: String = check.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    wt.join(APP_DIR).join("out").join(format!("{name}.log"))
}

impl Engine {
    /// Story 309: the copy gets the lowest port block no other unfinished task of the project holds, and a
    /// database name of its own. Decided under the store's lock, so two tasks preparing at once differ.
    pub(super) fn alloc_ports(&self, t: &mut Task, p: &Project) -> Result<(), String> {
        if !p.isolate_ports {
            (t.port, t.db_name) = (None, None);
            return Ok(());
        }
        let mut st = lk(&self.store);
        let taken: Vec<u16> = st
            .tasks()
            .into_iter()
            .filter(|x| x.project_id == p.id && x.id != t.id && !matches!(x.status, Status::Done | Status::DoneNotPushed | Status::Failed | Status::Cancelled | Status::Reverted))
            .filter_map(|x| x.port)
            .collect();
        let port = (0u16..).map(|n| PORT_BASE.saturating_add(n.saturating_mul(10))).find(|p| !taken.contains(p)).unwrap_or(PORT_BASE);
        let clean = |s: &str| s.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect::<String>();
        t.port = Some(port);
        t.db_name = Some(format!("{}_{}", clean(&p.name), clean(&t.id.chars().take(8).collect::<String>())));
        st.upsert_task(t).map_err(|e| e.to_string())
    }

    /// The variables of the copy (ports and database) for the agent, the setup and the checks.
    pub(super) fn copy_env(t: &Task) -> Vec<(String, String)> {
        let mut v = vec![];
        if let Some(port) = t.port {
            v.push(("PORT".to_string(), port.to_string()));
            v.push(("NIGHTSHIFT_PORT_BASE".to_string(), port.to_string()));
        }
        if let Some(db) = &t.db_name {
            v.push(("NIGHTSHIFT_DB_NAME".to_string(), db.clone()));
        }
        v
    }

    /// Story 307: before another model (or a fresh session) takes over, a note of where the work stands goes
    /// into the copy and onto the task; the next run of `role` starts with it. Written by the app from what it
    /// knows: on a used-up limit the old model cannot write anything.
    pub(super) fn write_handoff(&self, t: &mut Task, base: &str, role: RoleKind, reason: &str) -> Result<(), String> {
        let files: Vec<String> = t.worktree.as_ref().map(|w| git::diffstat(w, base).into_iter().map(|f| f.path).collect()).unwrap_or_default();
        let last = lk(&self.store)
            .read_log(&t.id, 200)
            .into_iter()
            .rev()
            .find(|e| matches!(e.kind, LogKind::Agent) && e.role == Some(role))
            .map(|e| e.text.trim().chars().take(600).collect::<String>());
        let plan = self.state(&t.id).brief.map(|b| b.plan).filter(|p| !p.is_empty());
        let pending = self.pending(&t.id).prompt.filter(|p| p != prompt::CONTINUE);
        let or_none = |s: Option<String>| s.unwrap_or_else(|| "none recorded".into());
        let note = format!(
            "HANDOFF NOTE ({reason}): you take over this task from another session; this is where it stands.\n\
             Goal: {}\nDone so far: {}\nDecisions: {}\nNext: {}\nFiles changed: {}\n",
            t.title,
            or_none(last),
            or_none(plan.map(|p| p.join("; "))),
            pending.unwrap_or_else(|| "finish the goal; check the files below first".into()),
            if files.is_empty() { "none yet".into() } else { files.join(", ") },
        );
        if let Some(wt) = &t.worktree {
            let f = wt.join(APP_DIR).join(HANDOFF_FILE);
            let _ = std::fs::create_dir_all(f.parent().unwrap_or(wt));
            std::fs::write(&f, &note).map_err(|e| format!("handoff_not_written: {e}"))?;
        }
        t.handoff = Some(note);
        self.update_state(&t.id, |s| s.handoff_for = Some(role))?;
        self.log(&t.id, LogKind::Stage, &format!("handoff: {reason}"));
        Ok(())
    }

    /// The note opens the prompt of the role it was written for, once.
    pub(super) fn with_handoff(&self, t: &Task, role: RoleKind, text: String) -> Result<String, String> {
        if self.state(&t.id).handoff_for != Some(role) {
            return Ok(text);
        }
        self.update_state(&t.id, |s| s.handoff_for = None)?;
        Ok(match &t.handoff {
            Some(n) => format!("{n}\n{text}"),
            None => text,
        })
    }

    /// "Сжать" (story 305): the worker's session is compacted before its next run, keeping `keep` (default:
    /// code changes and decisions); in an env without a compact command a fresh session starts with the hand-off note.
    pub fn compact(&self, id: &str, keep: Option<&str>) -> Result<(), String> {
        let t = self.task(id).ok_or("task_not_found")?;
        if t.session.is_none() || matches!(t.status, Status::Done | Status::DoneNotPushed | Status::Failed | Status::Cancelled | Status::Reverted) {
            return Err("nothing_to_compact".into());
        }
        let keep = keep.map(str::trim).filter(|k| !k.is_empty()).unwrap_or("code changes and decisions").to_string();
        self.update_state(id, |s| s.compact = Some(keep))?;
        self.log(id, LogKind::Human, "compact_queued");
        Ok(())
    }
}
