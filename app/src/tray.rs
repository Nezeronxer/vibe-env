//! What the tray, the window's close button and the keys decide from the state, as plain functions (R07.3, R10.1, R10.2, R08.2).
use nightshift_core::engine::is_active;
use nightshift_core::model::Task;
use std::process::{Command, Stdio};

use crate::envs::waits_for_human;

/// `2 ▶ 1 ?`: only the non-zero parts, empty when idle. From 30 calls up it is just `30` (story 174): the window has the rest.
pub fn label(running: usize, waiting: usize) -> String {
    if waiting >= 30 {
        return "30".into();
    }
    let mut parts = vec![];
    if running > 0 {
        parts.push(format!("{running} ▶"));
    }
    if waiting > 0 {
        parts.push(format!("{waiting} ?"));
    }
    parts.join(" ")
}

/// (working now, waiting for the person).
pub fn counts(tasks: &[Task]) -> (usize, usize) {
    (tasks.iter().filter(|t| is_active(&t.status)).count(), tasks.iter().filter(|t| waits_for_human(t)).count())
}

/// The text of the confirmation when the person quits with work going on (R08.2). None = nothing is going.
pub fn quit_prompt(running: usize, ru: bool) -> Option<String> {
    if running == 0 {
        return None;
    }
    Some(if ru {
        let (verb, noun) = match (running % 10, running % 100) {
            (_, 11..=14) => ("Идут", "задач"),
            (1, _) => ("Идёт", "задача"),
            (2..=4, _) => ("Идут", "задачи"),
            _ => ("Идут", "задач"),
        };
        format!("{verb} {running} {noun}. Выйти? Продолжат при следующем запуске")
    } else {
        format!("{running} {} running. Quit? They will continue on the next start", if running == 1 { "task is" } else { "tasks are" })
    })
}

pub fn is_wayland(session_type: Option<&str>, wayland_display: Option<&str>) -> bool {
    session_type.is_some_and(|s| s.eq_ignore_ascii_case("wayland")) || wayland_display.is_some_and(|d| !d.is_empty())
}

/// A global key cannot be taken on Wayland: the person binds `nightshift --new-chat` in the compositor (R10.1).
pub fn hotkey_available() -> bool {
    !cfg!(target_os = "linux") || !is_wayland(std::env::var("XDG_SESSION_TYPE").ok().as_deref(), std::env::var("WAYLAND_DISPLAY").ok().as_deref())
}

pub fn wants_new_chat(args: &[String]) -> bool {
    args.iter().any(|a| a == "--new-chat")
}

/// `busctl get-property ... IsStatusNotifierHostRegistered` prints `b true`.
pub fn parse_host_registered(busctl: &str) -> Option<bool> {
    match busctl.trim() {
        "b true" => Some(true),
        "b false" => Some(false),
        _ => None,
    }
}

/// Is there something on this desktop that shows tray icons? macOS always; Linux asks the StatusNotifier watcher.
/// None = cannot tell (no `busctl`, no watcher on the bus).
pub fn tray_host() -> Option<bool> {
    if !cfg!(target_os = "linux") {
        return Some(true);
    }
    let o = Command::new("busctl")
        .args(["--user", "get-property", "org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher", "org.kde.StatusNotifierWatcher", "IsStatusNotifierHostRegistered"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !o.status.success() {
        return Some(false); // no watcher on the bus: nobody shows the icon
    }
    parse_host_registered(&String::from_utf8_lossy(&o.stdout))
}

#[derive(Debug, PartialEq)]
pub enum OnClose {
    /// Leave only the tray icon.
    Hide,
    /// No icon to come back by: keep the window in the taskbar (R10.2).
    Minimize,
}

pub fn on_close(host: Option<bool>) -> OnClose {
    if host == Some(true) {
        OnClose::Hide
    } else {
        OnClose::Minimize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightshift_core::model::{Question, QuestionKind, Status};

    #[test]
    fn the_counter_has_only_the_non_zero_parts_and_thirty_calls_show_thirty() {
        assert_eq!([label(2, 1), label(0, 3), label(1, 0), label(0, 0)], ["2 ▶ 1 ?", "3 ?", "1 ▶", ""]);
        assert_eq!([label(4, 29), label(4, 30), label(0, 31)], ["4 ▶ 29 ?", "30", "30"]);
    }

    #[test]
    fn what_works_and_what_waits_are_counted_from_the_statuses() {
        let t = |status| Task { status, question: Some(Question { text: "q".into(), options: vec![], kind: QuestionKind::Agent }), ..Task::default() };
        let tasks = [Status::Working, Status::Checking, Status::Merging, Status::NeedsHuman, Status::AwaitingOutward, Status::MergeBlocked, Status::Queued, Status::Done, Status::Failed].map(t);
        assert_eq!(counts(&tasks), (3, 3));
    }

    #[test]
    fn quitting_with_work_asks_in_the_persons_words_and_without_work_does_not_ask() {
        assert_eq!(quit_prompt(0, true), None);
        assert_eq!(quit_prompt(3, true).unwrap(), "Идут 3 задачи. Выйти? Продолжат при следующем запуске");
        assert_eq!(quit_prompt(1, true).unwrap(), "Идёт 1 задача. Выйти? Продолжат при следующем запуске");
        assert_eq!(quit_prompt(5, true).unwrap(), "Идут 5 задач. Выйти? Продолжат при следующем запуске");
        assert_eq!(quit_prompt(12, true).unwrap(), "Идут 12 задач. Выйти? Продолжат при следующем запуске");
        assert_eq!(quit_prompt(2, false).unwrap(), "2 tasks are running. Quit? They will continue on the next start");
    }

    #[test]
    fn wayland_is_told_by_the_session_and_the_second_start_with_the_flag_is_a_request_for_the_window() {
        assert!(is_wayland(Some("wayland"), None) && is_wayland(None, Some("wayland-0")) && is_wayland(Some("Wayland"), Some("")));
        assert!(!is_wayland(Some("x11"), None) && !is_wayland(None, Some("")) && !is_wayland(None, None));
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(wants_new_chat(&a(&["nightshift", "--new-chat"])));
        assert!(!wants_new_chat(&a(&["nightshift", "--new-chats", "new-chat"])));
    }

    #[test]
    fn closing_the_window_hides_it_only_where_an_icon_will_bring_it_back() {
        assert_eq!(parse_host_registered("b true\n"), Some(true));
        assert_eq!(parse_host_registered("b false"), Some(false));
        assert_eq!(parse_host_registered("Failed to get property"), None);
        assert_eq!([on_close(Some(true)), on_close(Some(false)), on_close(None)], [OnClose::Hide, OnClose::Minimize, OnClose::Minimize]);
    }
}
