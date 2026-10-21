//! The Watcher (G08.2): a run that prints nothing for `stall_min` minutes is stopped and continued in
//! the same session; the Watcher's own run (read-only, its model) tells what the log shows. Five more
//! minutes of silence restart the role in a new session and count as an attempt. In idle nothing
//! runs: the timer lives only inside a run.
use super::roles::{is_read_only, RoleCall, RoleEnd};
use super::*;
use crate::prompt::Vendor;

/// Minutes of silence after a nudge before the role is restarted.
pub(super) const RESTART_MIN: i64 = 5;
pub(super) const NUDGE: &str = "Continue where you stopped. If you are stuck, say what is blocking you.";

impl Engine {
    /// The Watcher's short diagnosis from the tail of the log, into the feed. True = cancelled meanwhile.
    pub(super) fn diagnose(&self, t: &mut Task, p: &Project, stage: Status, cancel: &AtomicBool) -> Result<bool, String> {
        let request = self.diagnosis_request(&t.id);
        debug_assert!(is_read_only(RoleKind::Watcher));
        let base = |_: Vendor, _: bool| request.clone();
        let call = RoleCall { role: RoleKind::Watcher, resume: None, prompt: &base, schema: None, stage, shown: None };
        match self.run_role(t, p, call, cancel)? {
            RoleEnd::Done { text, .. } => {
                self.log_diagnosis(&t.id, &text);
                Ok(false)
            }
            RoleEnd::Cancelled => Ok(true),
            // no diagnosis (the run failed): the nudge goes on without it
            _ => Ok(false),
        }
    }

    /// "Подтолкнуть": the run in progress is stopped; a worker continues its session with the nudge,
    /// any other role starts its step again. Not needed - the Watcher does the same by timer.
    pub fn nudge(&self, id: &str) -> Result<(), String> {
        let _h = self.hold(id, true)?;
        let mut t = self.task(id).ok_or("task_not_found")?;
        if !matches!(t.status, Status::Working | Status::Checking | Status::Reviewing) {
            return Err("not_nudgeable".into());
        }
        for r in t.runs.iter_mut().filter(|r| matches!(r.state, StepState::Running)) {
            r.state = StepState::Failed("nudged".into());
        }
        if t.status == Status::Working && t.session.is_some() {
            self.set_pending(id, &Pending { prompt: Some(NUDGE.into()), stage: Some(Status::Working), ..Default::default() })?;
        }
        self.save(&t)?;
        self.log(id, LogKind::Human, "nudge");
        Ok(())
    }
}
