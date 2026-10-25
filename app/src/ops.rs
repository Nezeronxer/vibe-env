//! Commands that act on tasks and chats: thin over the engine, plus what only the app knows -
//! the terminal (spec 28), the digest's shape for the window, the notes' answers.
use crate::envs::cli_of;
use crate::hub::{lk, Hub, Res};
use crate::term::{self, Launch};
use nightshift_core::model::{ChatMsg, EnvLimits, ModelChoice, OutwardChoice};
use serde_json::{json, Value};

impl Hub {
    pub fn note(&self, task_id: &str, to: Option<String>, text: &str) -> Res<Value> {
        use nightshift_core::engine::NoteResult;
        Ok(match self.engine.note(task_id, to, text)? {
            NoteResult::Queued => json!({ "result": "queued" }),
            NoteResult::Answered => json!({ "result": "answered" }),
            NoteResult::NotDelivered(why) => json!({ "result": "not_delivered", "why": why }),
        })
    }

    pub fn answer_outward(&self, task_id: &str, choice: OutwardChoice) -> Res<()> {
        self.engine.answer_outward(task_id, choice)
    }

    pub fn resume_failed(&self, task_id: &str, hint: Option<String>, model: Option<ModelChoice>) -> Res<()> {
        self.engine.resume_failed(task_id, hint.filter(|h| !h.trim().is_empty()), model)
    }

    pub fn freeze(&self, on: bool) {
        self.engine.freeze(on);
    }

    /// "Продолжить в терминале": the task goes to the person, the terminal opens in its copy with the env's own
    /// command to continue the session. The session id is checked before the engine is touched (A07).
    pub fn hand_off(&self, task_id: &str, ready: &dyn Fn() -> Res<()>, launch: &dyn Fn(&Launch) -> Res<()>) -> Res<Value> {
        let t = lk(self.engine.store()).task(task_id).ok_or("task_not_found")?;
        if t.session.as_deref().is_some_and(|s| !term::valid_session_id(s)) {
            return Err("bad_session_id".into());
        }
        // no terminal to open: the task stays where it was, and its run goes on
        ready()?;
        let h = self.engine.hand_off(task_id)?;
        let l = match &h.command {
            Some(c) => term::from_command(c),
            // the session cannot be continued: a new one of the env's CLI in the same copy
            None => Launch { cwd: Some(h.worktree.clone()), argv: cli_of(self, &h.env).map(|p| vec![p.to_string_lossy().into_owned()]).unwrap_or_default(), banner: None, env: vec![] },
        };
        launch(&Launch { cwd: Some(h.worktree.clone()), ..l })?;
        Ok(json!({ "resumed": h.command.is_some(), "worktree": h.worktree }))
    }

    pub fn take_back(&self, task_id: &str) -> Res<()> {
        self.engine.take_back(task_id)
    }

    /// The chat in the terminal (A07.2): the chat's own read-only copy; the feed gets the warning that edits made there are lost.
    pub fn open_chat_terminal(&self, chat_id: &str, launch: &dyn Fn(&Launch) -> Res<()>) -> Res<()> {
        let tm = self.chats.terminal(chat_id)?;
        if tm.session.as_deref().is_some_and(|s| !term::valid_session_id(s)) {
            return Err("bad_session_id".into());
        }
        let l = match &tm.command {
            Some(c) => term::from_command(c),
            None => Launch { cwd: None, argv: cli_of(self, &tm.env).map(|p| vec![p.to_string_lossy().into_owned()]).unwrap_or_default(), banner: None, env: vec![] },
        };
        launch(&Launch { cwd: Some(tm.copy.clone()), ..l })?;
        // the feed line goes through the chats (same order, same event as every other line of the app)
        self.chats.terminal_opened(chat_id)
    }

    /// "Пока тебя не было" in the shape the window reads (`ui/logic.js: summaryRows`): rows per list, `spent`, and the
    /// limit windows that are exhausted or past the warning share.
    pub fn summary_since(&self, at: &str) -> Res<Value> {
        let at = serde_json::from_value(Value::String(at.to_string())).map_err(|_| format!("bad_time: {at}"))?;
        let s = self.engine.summary_since(at);
        let warn = f32::from(self.settings().limits.warn_pct);
        let row = |x: &nightshift_core::engine::SummaryItem| {
            json!({ "task_id": x.task_id, "project": x.project, "env": x.env, "title": x.title, "text": x.text, "reason": x.text, "commit": x.commit })
        };
        let limits: Vec<Value> = s
            .limits
            .iter()
            .filter_map(|(env, l)| match l {
                EnvLimits::Known(ws) => Some(ws.iter().filter(|w| w.exhausted || w.used_pct.is_some_and(|p| p >= warn)).map(move |w| json!({ "env": env, "window": w }))),
                EnvLimits::NoData(_) => None,
            })
            .flatten()
            .collect();
        Ok(json!({
            "since": s.since, "done": s.done.iter().map(row).collect::<Vec<_>>(), "failed": s.failed.iter().map(row).collect::<Vec<_>>(),
            "waiting": s.waiting.iter().map(row).collect::<Vec<_>>(),
            "spent": if s.spent_usd > 0.0 { json!([{ "usd": s.spent_usd }]) } else { json!([]) },
            "limits": limits,
        }))
    }
}

impl Hub {
    pub fn get_chat(&self, id: &str, before: Option<&str>, limit: usize) -> Res<Vec<ChatMsg>> {
        let before = match before {
            Some(b) => Some(serde_json::from_value(Value::String(b.to_string())).map_err(|_| format!("bad_time: {b}"))?),
            None => None,
        };
        Ok(self.chats.messages(id, before, limit))
    }

    /// "Это был вопрос" on a task card of a chat.
    pub fn not_a_task(&self, task_id: &str) -> Res<()> {
        let t = lk(self.engine.store()).task(task_id).ok_or("task_not_found")?;
        let chat = t.chat_id.ok_or("task_not_in_chat")?;
        self.chats.not_a_task(&chat, task_id)
    }

    /// The models of the env as last seen (the snapshot, else the built-in hint); no CLI is run.
    pub fn get_catalog(&self, env: &str) -> Res<Value> {
        let a = self.adapter(env).ok_or_else(|| format!("agent_not_found: {env}"))?;
        let st = lk(self.engine.store());
        Ok(json!(nightshift_core::models::catalog(env, &*a, &st)))
    }

    /// Asks the env's CLI for its list again; `catalog-changed` follows.
    pub fn refresh_catalog(&self, env: &str) -> Res<Value> {
        Ok(json!(self.engine.refresh_catalog(env)?))
    }

    pub fn get_limits(&self, env: &str) -> Value {
        json!(self.engine.limits(env))
    }
}

impl Hub {
    /// "Повторить push" in a PR project: send the task's branch again and open the PR.
    pub fn retry(&self, task_id: &str) -> Res<()> {
        self.engine.retry(task_id)
    }

    /// "Повторить push" in a Push project: send the project's main.
    pub fn push_base(&self, project_id: &str) -> Res<()> {
        self.project_ready(project_id)?;
        self.engine.push_base(project_id)
    }
}

#[cfg(test)]
mod tests {
    use crate::testkit::*;
    use nightshift_core::agent::{FakeAdapter, FakeStep};
    use nightshift_core::model::{Finish, OutwardChoice, RoleKind, Status};
    use std::path::Path;

    const APPROVE: &str = r#"{"verdict":"approve","issues":[]}"#;

    fn worker_and_reviewer() -> FakeAdapter {
        FakeAdapter::new(vec![])
            .with_role(RoleKind::Worker, vec![vec![FakeStep::Write("f.txt".into(), "f".into()), FakeStep::Done("f".into())]])
            .with_role(RoleKind::Reviewer, vec![vec![FakeStep::Done(APPROVE.into())]])
    }

    fn on_origin(bare: &Path, branch: &str) -> bool {
        std::process::Command::new("git").current_dir(bare).args(["rev-parse", "--verify", "-q", &format!("refs/heads/{branch}")]).status().unwrap().success()
    }

    fn asking(s: &mut nightshift_core::model::Settings) {
        s.envs.insert("fake".into(), nightshift_core::model::EnvSettings { ask_push: true, ask_pr: true, ..Default::default() });
    }

    #[test]
    fn the_answer_to_the_outward_question_decides_and_a_refused_push_of_a_push_project_is_repeated_by_push_base() {
        let r = Rig::build(worker_and_reviewer(), false, asking, |p| p.finish = Finish::Push);
        let bare = r.with_origin();
        let id = r.hub.submit_task(&r.project.id, "Add f", None).unwrap();
        r.wait_task(&id, "the question", |t| t.status == Status::AwaitingOutward);
        let before = g(&bare, &["rev-parse", "main"]);

        r.hub.answer_outward(&id, OutwardChoice::No).unwrap();
        r.wait_task(&id, "done not pushed", |t| t.status == Status::DoneNotPushed);
        assert_eq!(g(&bare, &["rev-parse", "main"]), before, "nothing left the machine");

        r.hub.push_base(&r.project.id).unwrap();
        r.wait_task(&id, "done", |t| t.status == Status::Done);
        assert_eq!(g(&bare, &["rev-parse", "main"]), g(&r.repo, &["rev-parse", "main"]));
    }

    #[test]
    fn a_pr_task_that_was_not_pushed_is_brought_to_done_by_retry_with_the_branch_on_origin_and_one_pr() {
        let gh = fake_gh();
        let r = Rig::build(worker_and_reviewer(), false, asking, |p| p.finish = Finish::Pr);
        let bare = r.with_origin();
        let id = r.hub.submit_task(&r.project.id, "Add f", None).unwrap();
        r.wait_task(&id, "the question", |t| t.status == Status::AwaitingOutward);
        r.hub.answer_outward(&id, OutwardChoice::No).unwrap();
        let t = r.wait_task(&id, "done not pushed", |t| t.status == Status::DoneNotPushed);
        assert!(!on_origin(&bare, &t.branch));

        r.hub.retry(&id).unwrap();
        let t = r.wait_task(&id, "done", |t| t.status == Status::Done);
        assert_eq!(t.outcome.pr_url.as_deref(), Some("https://github.com/o/r/pull/9"));
        assert!(on_origin(&bare, &t.branch));
        assert_eq!(gh_calls_for(&gh, &t.branch), 1);
    }

    /// A task that failed its one check round: the check wants `ok.txt`, the first worker run only writes `f.txt`.
    fn failing_rig(second_run: Vec<FakeStep>, resumable: bool) -> (Rig, String) {
        let fake = FakeAdapter::new(vec![])
            .with_role(RoleKind::Worker, vec![vec![FakeStep::Write("f.txt".into(), "f".into()), FakeStep::Done("f".into())], second_run])
            .with_role(RoleKind::Reviewer, vec![vec![FakeStep::Done(APPROVE.into())]]);
        let r = Rig::build(fake, resumable, |s| s.attempts = 1, |p| p.check_cmd = Some("test -f ok.txt".into()));
        let id = r.hub.submit_task(&r.project.id, "Add f", None).unwrap();
        r.wait_task(&id, "failed", |t| t.status == Status::Failed);
        (r, id)
    }

    #[test]
    fn a_failed_task_continues_in_its_copy_with_the_hint_and_the_hint_reaches_the_next_run() {
        let (r, id) = failing_rig(vec![FakeStep::Write("ok.txt".into(), "ok".into()), FakeStep::Done("fixed".into())], false);
        r.hub.resume_failed(&id, Some("  create ok.txt  ".into()), None).unwrap();
        r.wait_task(&id, "done", |t| t.status == Status::Done);
        let prompts: Vec<String> = r.fake.calls().iter().filter(|c| c.role == RoleKind::Worker).map(|c| c.prompt.clone()).collect();
        assert_eq!(prompts.len(), 2);
        assert!(prompts[1].contains("create ok.txt"), "{}", prompts[1]);
        assert_eq!(r.hub.resume_failed(&id, None, None).unwrap_err(), "not_failed");
    }

    #[test]
    fn freeze_holds_new_runs_until_it_is_lifted() {
        let r = Rig::new(FakeAdapter::new(vec![]).with_role(RoleKind::Reviewer, vec![vec![FakeStep::Done(APPROVE.into())]]));
        r.hub.freeze(true);
        assert!(r.hub.engine.is_frozen());
        let id = r.hub.submit_task(&r.project.id, "Add f", None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(400));
        assert_eq!(r.task(&id).status, Status::Queued, "nothing starts under a freeze");
        r.hub.freeze(false);
        r.wait_task(&id, "it runs after the freeze", |t| t.status != Status::Queued);
    }

    #[test]
    fn the_terminal_gets_the_command_as_a_list_in_the_copy_and_a_forged_session_id_is_refused_before_anything_changes() {
        use std::cell::RefCell;
        let (r, id) = failing_rig(vec![FakeStep::Done("x".into())], true);
        let wt = r.task(&id).worktree.unwrap();
        let seen = RefCell::new(vec![]);
        let launch = |l: &crate::term::Launch| {
            seen.borrow_mut().push(l.clone());
            Ok(())
        };
        for forged in ["x; touch pwned", "$(id)", "a b", "-r"] {
            let mut t = r.task(&id);
            t.session = Some(forged.into());
            r.hub.engine.store().lock().unwrap().upsert_task(&t).unwrap();
            assert_eq!(r.hub.hand_off(&id, &|| Ok(()), &launch).unwrap_err(), "bad_session_id", "{forged}");
            assert_eq!(r.task(&id).status, Status::Failed, "the task stays where it was");
        }
        assert!(seen.borrow().is_empty());

        let mut t = r.task(&id);
        t.session = Some("abc-123".into());
        r.hub.engine.store().lock().unwrap().upsert_task(&t).unwrap();
        // no terminal to open: nothing is changed, the task is still the engine's
        assert_eq!(r.hub.hand_off(&id, &|| Err("no_terminal: set $TERMINAL".into()), &launch).unwrap_err(), "no_terminal: set $TERMINAL");
        assert_eq!(r.task(&id).status, Status::Failed);
        assert!(seen.borrow().is_empty());
        let out = r.hub.hand_off(&id, &|| Ok(()), &launch).unwrap();
        assert_eq!(out["resumed"], true);
        assert_eq!(r.task(&id).status, Status::HandedOff);
        assert_eq!(*seen.borrow(), vec![crate::term::Launch { cwd: Some(wt.clone()), argv: ["/opt/bin/claude", "--resume", "abc-123"].map(String::from).to_vec(), banner: None, env: vec![] }]);

        std::fs::write(wt.join("mine.txt"), "by hand").unwrap();
        r.hub.take_back(&id).unwrap();
        assert_ne!(r.task(&id).status, Status::HandedOff);
        assert_eq!(r.hub.take_back(&id).unwrap_err(), "not_handed_off");
    }

    #[test]
    fn the_digest_has_what_finished_and_the_limit_windows_that_matter_in_the_shape_the_window_reads() {
        use nightshift_core::model::{EnvLimits, LimitWindow};
        let r = Rig::new(worker_and_reviewer());
        let id = r.hub.submit_task(&r.project.id, "Add f", None).unwrap();
        let t = r.wait_task(&id, "done", |t| t.status == Status::Done);
        let w = |name: &str, pct: f32| LimitWindow { name: name.into(), model: None, used_pct: Some(pct), remaining: None, resets_at: None, exhausted: false, source: "rate_limit_event".into(), seen_at: at(12, 0) };
        r.hub.engine.store().lock().unwrap().set_limits("fake", EnvLimits::Known(vec![w("five_hour", 95.0), w("seven_day", 16.0)])).unwrap();

        let s = r.hub.summary_since(&at(11, 0).to_rfc3339()).unwrap();
        let done = s["done"].as_array().unwrap();
        assert_eq!(done.len(), 1);
        assert_eq!((done[0]["title"].as_str(), done[0]["env"].as_str(), done[0]["commit"].as_str()), (Some("Add f"), Some("fake"), t.outcome.commit.as_deref()));
        assert!(s["failed"].as_array().unwrap().is_empty() && s["waiting"].as_array().unwrap().is_empty());
        let limits = s["limits"].as_array().unwrap();
        assert_eq!(limits.len(), 1, "only the window past the warning share (80%)");
        assert_eq!((limits[0]["env"].as_str(), limits[0]["window"]["name"].as_str()), (Some("fake"), Some("five_hour")));

        let later = r.hub.summary_since(&at(13, 0).to_rfc3339()).unwrap();
        assert!(later["done"].as_array().unwrap().is_empty(), "work before the window was left is not news");
        assert!(r.hub.summary_since("yesterday").unwrap_err().starts_with("bad_time"));
    }

    #[test]
    fn a_convoy_opens_its_chat_at_once_and_makes_the_tasks_of_the_phrase() {
        let tasks = serde_json::json!({ "tasks": [{"id": "t1", "title": "Task t1", "text": "Do t1", "depends_on": []}] }).to_string();
        let fake = FakeAdapter::new(vec![])
            .with_role(RoleKind::Coordinator, vec![vec![FakeStep::Done(tasks)]])
            .with_role(RoleKind::Worker, vec![vec![FakeStep::Write("f.txt".into(), "f".into()), FakeStep::Done("f".into())]])
            .with_role(RoleKind::Reviewer, vec![vec![FakeStep::Done(APPROVE.into())]]);
        let r = Rig::new(fake);
        let chat = r.hub.submit_convoy(&r.project.id, "fake", "Prepare the release").unwrap();
        let c = r.hub.engine.store().lock().unwrap().chat(&chat).unwrap();
        assert!(c.convoy);
        until("the task is done", || r.hub.engine.store().lock().unwrap().tasks().iter().any(|t| t.chat_id.as_deref() == Some(chat.as_str()) && t.status == Status::Done));
        assert!(r.hub.submit_convoy("no-such-project", "fake", "x").is_err());

        // the card of the task was written without an event: the window gets it once
        let mut cards = vec![];
        r.hub.deliver_cards(&mut |m| cards.push(m.clone()));
        assert_eq!(cards.len(), 1);
        assert_eq!((cards[0].chat_id.as_str(), cards[0].kind), (chat.as_str(), Some(nightshift_core::model::ChatKind::Task)));
        assert!(cards[0].task_id.is_some());
        r.hub.deliver_cards(&mut |_| panic!("delivered twice"));
    }

    #[test]
    fn the_chat_terminal_opens_in_the_chats_copy_and_the_warning_goes_through_the_chats_with_its_own_id_each_time() {
        use nightshift_core::chat::{ChatEvent, ChatTarget};
        use nightshift_core::model::{ChatKind, Who};
        let r = Rig::new(FakeAdapter::new(vec![]).with_role(RoleKind::Chat, vec![vec![FakeStep::Done("hi".into())]]));
        let chat = r.hub.send_chat(ChatTarget::New { env: "fake".into(), project_id: r.project.id.clone() }, "m1", "hello", &[]).unwrap();
        until("answered", || r.hub.chats.messages(&chat, None, 50).iter().any(|m| m.who == Who::Model));
        let events = r.hub.chats.subscribe();
        let seen = std::cell::RefCell::new(vec![]);
        let launch = |l: &crate::term::Launch| {
            seen.borrow_mut().push(l.cwd.clone());
            Ok(())
        };
        r.hub.open_chat_terminal(&chat, &launch).unwrap();
        r.hub.open_chat_terminal(&chat, &launch).unwrap();
        assert!(seen.borrow()[0].as_ref().unwrap().starts_with(r.s.0.join("data/worktrees/.chats")), "the copy of the chat, not the project");
        let warns: Vec<_> = r.hub.chats.messages(&chat, None, 50).into_iter().filter(|m| m.kind == Some(ChatKind::Warn)).collect();
        assert_eq!(warns.len(), 2);
        assert_ne!(warns[0].id, warns[1].id, "two calls in one millisecond keep both");
        assert_eq!(warns[0].text, "terminal_edits_lost");
        let got: Vec<_> = events.try_iter().filter(|e| matches!(e, ChatEvent::Msg(m) if m.kind == Some(ChatKind::Warn))).collect();
        assert_eq!(got.len(), 2, "the window hears of both from the chats' own event");
    }
}
