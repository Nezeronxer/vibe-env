//! The convoy (Поручение, G08.1): one phrase, the Coordinator turns it into tasks with dependencies.
//! The split runs in a task of its own (so a question of the Coordinator is an ordinary `NEEDS_HUMAN`
//! with an answer and a call) that goes away once the real tasks exist. A task that depends on others
//! does not start until each of them has landed.
use super::flow::{Flow, Res};
use super::steps::Structured;
use super::*;
use crate::prompt::Vendor;
use crate::swarm::{parse, prompts};

/// The task that holds the Coordinator's run: it has a convoy and no coding step.
pub(super) fn is_plan(t: &Task) -> bool {
    t.convoy_id.is_some() && !t.steps.iter().any(|(s, _)| *s == Step::Code)
}

/// Landed on main (or waiting only for the person's push).
fn landed(t: &Task) -> bool {
    matches!(t.status, Status::Done | Status::DoneNotPushed) || (t.status == Status::AwaitingOutward && t.outcome.commit.is_some())
}

impl Engine {
    /// "Поручить" (G08.1): a chat «Поручение: <фраза>» opens at once, the Coordinator splits the phrase
    /// into at most 12 tasks; their cards appear in the chat and the tasks start by their dependencies.
    pub fn submit_convoy(&self, project_id: &str, env: &str, text: &str) -> Result<String, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("empty_prompt".into());
        }
        if text.chars().count() > MAX_PROMPT_CHARS {
            return Err("prompt_too_long".into());
        }
        let p = self.project(&project_id.to_string()).ok_or("project_not_found")?;
        if self.adapter(env).is_none() {
            return Err(format!("agent_not_found: {env}"));
        }
        let now = self.now();
        let chat_id = uuid::Uuid::new_v4().to_string();
        let phrase: String = util::title_of(text).chars().take(60).collect();
        {
            let mut st = lk(&self.store);
            st.upsert_chat(&Chat {
                id: chat_id.clone(),
                env: env.to_string(),
                project_id: p.id.clone(),
                title: format!("Поручение: {phrase}"),
                session: None,
                convoy: true,
                created_at: now,
                updated_at: now,
                archived: false,
                queue: vec![],
                wait: None,
                answering: None,
                terminal: None,
            })
            .map_err(|e| e.to_string())?;
            st.append_chat_msg(&ChatMsg {
                id: uuid::Uuid::new_v4().to_string(),
                chat_id: chat_id.clone(),
                at: now,
                who: Who::Human,
                text: text.to_string(),
                task_id: None,
                delivered_at: None,
                kind: None,
                tool: None,
                model: None,
                reply_to: None,
                action: None,
                attachments: vec![],
            })
            .map_err(|e| e.to_string())?;
        }
        let new = NewTask { project_id: p.id.clone(), prompt: text.to_string(), agent: Some(env.to_string()), chat_id: Some(chat_id.clone()), ..NewTask::default() };
        // the holder of the split has one step and no coding: that is how it is told from the tasks it makes
        self.submit_full(new, None, Some(uuid::Uuid::new_v4().to_string()), vec![], Some(vec![(Step::Plan, StepState::Pending)]), None)?;
        Ok(chat_id)
    }

    /// The Coordinator's run: the tasks, their cards in the chat, and the holder of the split is gone.
    pub(super) fn convoy_plan(&self, t: &mut Task, p: &Project, cancel: &AtomicBool) -> Res {
        t.started_at.get_or_insert(self.now());
        self.set(t, Status::Working)?;
        self.set_step(t, Step::Plan, StepState::Running)?;
        self.stage(t, "convoy: splitting the request");
        let hint = format!("Project: {}\nChecks: {}", p.name, p.check_cmd.as_deref().unwrap_or("none"));
        let request = prompts::convoy_request(&t.prompt, &hint, None);
        let base = |_: Vendor, _: bool| request.clone();
        let tasks = match self.structured(t, p, RoleKind::Coordinator, Status::Working, prompts::convoy_schema(), &base, &parse::parse_convoy, cancel)? {
            Structured::Stopped => return Ok(Flow::Stop),
            Structured::Unusable(why) => return Err(format!("convoy_split_failed: {why}")),
            Structured::Parsed(x) => x,
        };
        let convoy = t.convoy_id.clone().ok_or("no_convoy")?;
        let chat = t.chat_id.clone();
        let ids: Vec<(String, TaskId)> = tasks.iter().map(|x| (x.id.clone(), uuid::Uuid::new_v4().to_string())).collect();
        let real = |local: &str| ids.iter().find(|(l, _)| l == local).map(|(_, r)| r.clone());
        for (x, (_, tid)) in tasks.iter().zip(&ids) {
            let deps: Vec<TaskId> = x.depends_on.iter().filter_map(|d| real(d)).collect();
            let new = NewTask { project_id: p.id.clone(), prompt: x.text.clone(), agent: Some(t.agent.clone()), chat_id: chat.clone(), ..NewTask::default() };
            // the task is made with the Coordinator's title: once it is in the store the scheduler may start it, and a
            // later read-change-write of it here would put back a stale copy over the running task
            let title: String = x.title.chars().take(80).collect();
            self.submit_full(new, Some(tid.clone()), Some(convoy.clone()), deps, None, Some(title))?;
            // the card names the task by the Coordinator's title
            if let Some(c) = &chat {
                let st = lk(&self.store);
                let _ = st.append_chat_msg(&ChatMsg {
                    id: uuid::Uuid::new_v4().to_string(),
                    chat_id: c.clone(),
                    at: self.now(),
                    who: Who::System,
                    text: x.title.clone(),
                    task_id: Some(tid.clone()),
                    delivered_at: None,
                    kind: Some(ChatKind::Task),
                    tool: None,
                    model: None,
                    reply_to: None,
                    action: None,
                    attachments: vec![],
                });
            }
        }
        if let Some(c) = &chat {
            let mut st = lk(&self.store);
            if let Some(mut chat) = st.chat(c) {
                chat.updated_at = self.now();
                let _ = st.upsert_chat(&chat);
            }
        }
        // the holder of the split has done its job
        self.clear_pending(&t.id);
        self.clear_state(&t.id);
        lk(&self.store).remove_task(&t.id).map_err(|e| e.to_string())?;
        Ok(Flow::Stop)
    }

    /// A task waits for the tasks it depends on to land on main (G08.1). What it waits for is on its card.
    pub(super) fn dependencies_landed(&self, t: &Task, tasks: &[Task]) -> bool {
        // a task that is gone counts as done: nothing is left to wait for
        let blocking = t.depends_on.iter().filter_map(|d| tasks.iter().find(|o| o.id == *d)).find(|o| !landed(o));
        let Some(dep) = blocking else {
            return true;
        };
        let broken = matches!(dep.status, Status::Failed | Status::Cancelled | Status::Reverted);
        let reason = format!("{}: {}", if broken { "dependency_blocked" } else { "waiting_dependency" }, dep.title);
        if t.outcome.error.as_deref() != Some(&reason) {
            if let Ok(x) = self.cas(&t.id, "changed", |x| x.status == Status::Queued, |x| x.outcome.error = Some(reason.clone())) {
                let _ = x;
                self.log(&t.id, LogKind::Stage, &reason);
            }
        }
        false
    }
}
