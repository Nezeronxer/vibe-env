//! The app's notifier (spec 12): quiet hours, one sound per call, calls within a minute folded into one
//! notification, and the line for the window when the system refuses notifications. The core knows none of it.
use chrono::{DateTime, Local, Timelike};
use nightshift_core::model::NotifySettings;
use nightshift_core::notify::{Notice, NoticeKind, Notifier};
use std::collections::HashSet;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

/// One system notification as it goes out. Showing the same `id` again replaces it where the system allows.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub id: i32,
    pub title: String,
    pub body: String,
    /// The sound the person chose; None = silent.
    pub sound: Option<String>,
    /// The task a click opens (the first of a folded group).
    pub task_id: String,
    /// How many tasks the notification speaks for.
    pub count: usize,
}

pub trait Sink: Send + Sync {
    fn show(&self, s: &Shown) -> Result<(), String>;
}

/// Calls closer than this to the first of a group fold into it.
const FOLD_SECS: i64 = 60;

struct Group {
    id: i32,
    start: DateTime<Local>,
    tasks: Vec<(String, String)>,
}

#[derive(Default)]
struct State {
    group: Option<Group>,
    /// Calls that have already sounded.
    sounded: HashSet<(String, String)>,
    blocked: bool,
}

pub struct AppNotifier {
    sink: Arc<dyn Sink>,
    settings: Arc<dyn Fn() -> NotifySettings + Send + Sync>,
    clock: Arc<dyn Fn() -> DateTime<Local> + Send + Sync>,
    ru: Arc<dyn Fn() -> bool + Send + Sync>,
    /// Gets the line for the window: how to switch notifications on. Called once per refusal streak.
    on_blocked: Arc<dyn Fn(String) + Send + Sync>,
    ids: AtomicI32,
    st: Mutex<State>,
}

/// `from` <= now < `to`, over midnight when `from` > `to`.
pub fn in_quiet(now: DateTime<Local>, from: &str, to: &str) -> bool {
    let min = |s: &str| s.split_once(':').and_then(|(h, m)| Some(h.parse::<u32>().ok()? * 60 + m.parse::<u32>().ok()?));
    let (Some(f), Some(t)) = (min(from), min(to)) else { return false };
    let n = now.hour() * 60 + now.minute();
    match f.cmp(&t) {
        std::cmp::Ordering::Equal => false,
        std::cmp::Ordering::Less => f <= n && n < t,
        std::cmp::Ordering::Greater => n >= f || n < t,
    }
}

fn plural_ru(n: usize) -> &'static str {
    match (n % 10, n % 100) {
        (_, 11..=14) => "задач",
        (1, _) => "задача",
        (2..=4, _) => "задачи",
        _ => "задач",
    }
}

/// How to switch system notifications on, for the strip in the window (R15i.7).
pub fn how_to_enable(ru: bool) -> String {
    match (cfg!(target_os = "macos"), ru) {
        (true, true) => "Системные настройки → Уведомления → Nightshift → разрешить".into(),
        (true, false) => "System Settings → Notifications → Nightshift → allow".into(),
        (false, true) => "запусти службу уведомлений (например dunst, mako или службу своего окружения)".into(),
        (false, false) => "start a notification service (for example dunst, mako or your desktop's own)".into(),
    }
}

/// The sound name the system plays for the person's choice.
pub(crate) fn sound_name(choice: &str) -> Option<String> {
    let mac = cfg!(target_os = "macos");
    match choice {
        "none" => None,
        "bell" => Some(if mac { "Glass" } else { "bell" }.into()),
        _ => Some(if mac { "Tink" } else { "message-new-instant" }.into()),
    }
}

impl AppNotifier {
    pub fn new(
        sink: Arc<dyn Sink>,
        settings: Arc<dyn Fn() -> NotifySettings + Send + Sync>,
        clock: Arc<dyn Fn() -> DateTime<Local> + Send + Sync>,
        ru: Arc<dyn Fn() -> bool + Send + Sync>,
        on_blocked: Arc<dyn Fn(String) + Send + Sync>,
    ) -> AppNotifier {
        AppNotifier { sink, settings, clock, ru, on_blocked, ids: AtomicI32::new(1), st: Mutex::new(State::default()) }
    }

    /// The task left the state of waiting: if it calls again, that is a new call and may sound.
    pub fn forget(&self, task_id: &str) {
        self.st.lock().unwrap_or_else(|e| e.into_inner()).sounded.retain(|(t, _)| t != task_id);
    }

    fn deliver(&self, st: &mut State, s: &Shown) {
        match self.sink.show(s) {
            Ok(()) => st.blocked = false,
            Err(_) if !st.blocked => {
                st.blocked = true;
                (self.on_blocked)(how_to_enable((self.ru)()));
            }
            Err(_) => {}
        }
    }
}

impl Notifier for AppNotifier {
    fn send(&self, n: &Notice) {
        let cfg = (self.settings)();
        let now = (self.clock)();
        let ru = (self.ru)();
        let mut st = self.st.lock().unwrap_or_else(|e| e.into_inner());
        let title = format!("{} · {}", n.project, n.title);
        let call = matches!(n.kind, NoticeKind::Question | NoticeKind::Failed | NoticeKind::Outward);
        if !call {
            // "Готово" is off until switched on, and then it is silent; a limit notice is silent too
            if n.kind == NoticeKind::Done && !cfg.on_done {
                return;
            }
            let id = self.ids.fetch_add(1, Ordering::Relaxed);
            let shown = Shown { id, title, body: n.body.clone(), sound: None, task_id: n.task_id.clone(), count: 1 };
            self.deliver(&mut st, &shown);
            return;
        }
        let quiet = in_quiet(now, &cfg.quiet_from, &cfg.quiet_to);
        let key = (n.task_id.clone(), format!("{:?}", n.kind));
        let first_time = st.sounded.insert(key);
        let sound = if quiet || !first_time { None } else { sound_name(&cfg.sound) };
        // inside a minute of the first call of a group: the same notification, now for more tasks
        if let Some(g) = st.group.as_mut().filter(|g| (now - g.start).num_seconds() < FOLD_SECS) {
            if !g.tasks.iter().any(|(t, _)| *t == n.task_id) {
                g.tasks.push((n.task_id.clone(), n.title.clone()));
            }
            let count = g.tasks.len();
            if count >= 2 {
                let head = if ru { format!("{count} {} ждут тебя", plural_ru(count)) } else { format!("{count} tasks are waiting for you") };
                let body = g.tasks.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join("\n");
                let shown = Shown { id: g.id, title: head, body, sound: None, task_id: g.tasks[0].0.clone(), count };
                self.deliver(&mut st, &shown);
                return;
            }
        }
        let id = self.ids.fetch_add(1, Ordering::Relaxed);
        st.group = Some(Group { id, start: now, tasks: vec![(n.task_id.clone(), n.title.clone())] });
        let shown = Shown { id, title, body: n.body.clone(), sound, task_id: n.task_id.clone(), count: 1 };
        self.deliver(&mut st, &shown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[derive(Default)]
    struct Rec {
        shown: Mutex<Vec<Shown>>,
        refuse: Mutex<bool>,
    }
    impl Sink for Rec {
        fn show(&self, s: &Shown) -> Result<(), String> {
            if *self.refuse.lock().unwrap() {
                return Err("no service".into());
            }
            self.shown.lock().unwrap().push(s.clone());
            Ok(())
        }
    }

    struct Rig {
        n: AppNotifier,
        sink: Arc<Rec>,
        now: Arc<Mutex<DateTime<Local>>>,
        blocked: Arc<Mutex<Vec<String>>>,
    }

    fn at(h: u32, m: u32, s: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 21, h, m, s).unwrap()
    }

    fn rig(on_done: bool, h: u32) -> Rig {
        let sink = Arc::new(Rec::default());
        let now = Arc::new(Mutex::new(at(h, 0, 0)));
        let blocked = Arc::new(Mutex::new(vec![]));
        let (nn, bb) = (now.clone(), blocked.clone());
        let cfg = NotifySettings { on_done, ..NotifySettings::default() };
        let n = AppNotifier::new(sink.clone(), Arc::new(move || cfg.clone()), Arc::new(move || *nn.lock().unwrap()), Arc::new(|| true), Arc::new(move |s| bb.lock().unwrap().push(s)));
        Rig { n, sink, now, blocked }
    }

    fn call(id: &str, kind: NoticeKind) -> Notice {
        Notice { kind, task_id: id.into(), project: "demo".into(), title: format!("Task {id}"), body: "what now?".into(), options: vec![] }
    }

    impl Rig {
        fn shown(&self) -> Vec<Shown> {
            self.sink.shown.lock().unwrap().clone()
        }
        fn move_to(&self, h: u32, m: u32, s: u32) {
            *self.now.lock().unwrap() = at(h, m, s);
        }
    }

    #[test]
    fn quiet_hours_cover_midnight_and_end_at_the_morning_minute() {
        let q = |h, m| in_quiet(at(h, m, 0), "23:00", "08:00");
        assert_eq!([q(22, 59), q(23, 0), q(23, 30), q(3, 0), q(7, 59), q(8, 0), q(12, 0)], [false, true, true, true, true, false, false]);
        assert!(!in_quiet(at(12, 0, 0), "10:00", "10:00"), "an empty window is no window");
        assert!(in_quiet(at(12, 0, 0), "10:00", "14:00"));
    }

    #[test]
    fn a_call_sounds_by_day_and_is_silent_in_quiet_hours_while_still_shown() {
        let day = rig(false, 12);
        day.n.send(&call("a", NoticeKind::Question));
        assert!(day.shown()[0].sound.is_some());
        let night = rig(false, 23);
        night.n.send(&call("a", NoticeKind::Failed));
        assert_eq!(night.shown().len(), 1, "it arrives and waits in the notification centre");
        assert_eq!(night.shown()[0].sound, None);
    }

    #[test]
    fn three_calls_in_a_minute_make_one_notification_for_three_tasks_and_a_click_opens_the_first() {
        let r = rig(false, 12);
        r.n.send(&call("a", NoticeKind::Question));
        r.move_to(12, 0, 20);
        r.n.send(&call("b", NoticeKind::Failed));
        r.move_to(12, 0, 50);
        r.n.send(&call("c", NoticeKind::Outward));
        let all = r.shown();
        let ids: HashSet<i32> = all.iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), 1, "one notification, replaced as more tasks join");
        let last = all.last().unwrap();
        assert_eq!((last.title.as_str(), last.count, last.task_id.as_str()), ("3 задачи ждут тебя", 3, "a"));
        assert_eq!(all.iter().filter(|s| s.sound.is_some()).count(), 1, "one sound for the whole group");
        // a minute later it is a new notification
        r.move_to(12, 1, 30);
        r.n.send(&call("d", NoticeKind::Question));
        let d = r.shown().pop().unwrap();
        assert_ne!(d.id, last.id);
        assert!(d.sound.is_some());
    }

    #[test]
    fn the_same_unanswered_call_does_not_sound_twice_and_a_new_call_after_the_answer_does() {
        let r = rig(false, 12);
        r.n.send(&call("a", NoticeKind::Question));
        r.move_to(13, 0, 0);
        r.n.send(&call("a", NoticeKind::Question));
        assert_eq!(r.shown().iter().filter(|s| s.sound.is_some()).count(), 1);
        assert_eq!(r.shown().last().unwrap().count, 1, "not counted twice");
        r.n.forget("a");
        r.move_to(14, 0, 0);
        r.n.send(&call("a", NoticeKind::Question));
        assert_eq!(r.shown().iter().filter(|s| s.sound.is_some()).count(), 2);
    }

    #[test]
    fn done_is_off_by_default_and_silent_when_switched_on() {
        let off = rig(false, 12);
        off.n.send(&call("a", NoticeKind::Done));
        assert!(off.shown().is_empty());
        let on = rig(true, 12);
        on.n.send(&call("a", NoticeKind::Done));
        assert_eq!((on.shown().len(), on.shown()[0].sound.clone()), (1, None));
    }

    #[test]
    fn a_refusal_of_the_system_gives_the_window_its_line_once_and_again_after_it_worked() {
        let r = rig(false, 12);
        *r.sink.refuse.lock().unwrap() = true;
        r.n.send(&call("a", NoticeKind::Question));
        r.move_to(12, 5, 0);
        r.n.send(&call("b", NoticeKind::Question));
        assert_eq!(r.blocked.lock().unwrap().len(), 1);
        assert!(r.blocked.lock().unwrap()[0].to_lowercase().contains("уведомлени"), "{:?}", r.blocked);
        *r.sink.refuse.lock().unwrap() = false;
        r.move_to(12, 10, 0);
        r.n.send(&call("c", NoticeKind::Question));
        *r.sink.refuse.lock().unwrap() = true;
        r.move_to(12, 15, 0);
        r.n.send(&call("d", NoticeKind::Question));
        assert_eq!(r.blocked.lock().unwrap().len(), 2);
    }

    #[test]
    fn the_russian_count_has_the_right_form() {
        assert_eq!([2, 3, 5, 11, 21, 22, 25].map(plural_ru), ["задачи", "задачи", "задач", "задач", "задача", "задачи", "задач"]);
    }
}
