//! F14.1 - scheduled tasks: when a preset fires next, whether a schedule is due (one catch-up after sleep, never a pile),
//! and whether the env has room left. Pure: the app owns the timer, the storage and `submit_task`.
use crate::model::{EnvLimits, ModelChoice, QualityPlan};
use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, TimeZone, Weekday};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    Daily,
    Weekdays,
    /// Once a week on `Schedule.weekday`.
    Weekly,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Schedule {
    pub id: String,
    pub title: String,
    pub prompt: String,
    pub project_id: String,
    #[serde(default)]
    pub env: Option<String>,
    #[serde(default)]
    pub model: Option<ModelChoice>,
    /// None = the settings' "Quality".
    #[serde(default)]
    pub quality: Option<QualityPlan>,
    pub preset: Preset,
    /// "HH:MM", local time.
    pub at: String,
    /// 0 = Monday ... 6 = Sunday; read by `Weekly` only.
    #[serde(default)]
    pub weekday: u8,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "Local::now")]
    pub created_at: DateTime<Local>,
    #[serde(default)]
    pub last_run: Option<DateTime<Local>>,
    #[serde(default)]
    pub last_task: Option<String>,
    /// Why the last due moment did not start a task ("limit", "paused", "folder_not_found: ...").
    #[serde(default)]
    pub skipped: Option<String>,
}

/// An env with less than this share of its window left does not get a scheduled task.
pub const MIN_LEFT_PCT: f32 = 10.0;

fn weekday_of(n: u8) -> Weekday {
    Weekday::try_from(n % 7).unwrap_or(Weekday::Mon)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn parse_at(at: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(at, "%H:%M").ok()
}

/// The first firing strictly after `after`; None for a bad time. A local time that does not exist (spring-forward gap)
/// is skipped for that day.
pub fn next_after(preset: Preset, at: &str, weekday: u8, after: DateTime<Local>) -> Option<DateTime<Local>> {
    let time = parse_at(at)?;
    let first = after.date_naive();
    for i in 0..=14 {
        let day = first + Duration::days(i);
        let ok = match preset {
            Preset::Daily => true,
            Preset::Weekdays => !matches!(day.weekday(), Weekday::Sat | Weekday::Sun),
            Preset::Weekly => day.weekday() == weekday_of(weekday),
        };
        if !ok {
            continue;
        }
        if let Some(t) = Local.from_local_datetime(&day.and_time(time)).earliest().filter(|t| *t > after) {
            return Some(t);
        }
    }
    None
}

impl Schedule {
    pub fn next(&self, after: DateTime<Local>) -> Option<DateTime<Local>> {
        next_after(self.preset, &self.at, self.weekday, after)
    }

    /// Due = a firing passed since the last run (or since creation). However many were missed while the Mac slept,
    /// that is one run: the caller then sets `last_run = now`.
    pub fn due(&self, now: DateTime<Local>) -> bool {
        self.enabled && self.next(self.last_run.unwrap_or(self.created_at)).is_some_and(|t| t <= now)
    }
}

/// False when a window of the env is spent or has under `MIN_LEFT_PCT` left. An env that reports nothing is not blocked.
pub fn limit_ok(l: &EnvLimits) -> bool {
    match l {
        EnvLimits::NoData(_) => true,
        EnvLimits::Known(w) => w.iter().all(|w| !w.exhausted && w.used_pct.is_none_or(|u| 100.0 - u >= MIN_LEFT_PCT)),
    }
}

/// The four ready-made tasks. `(id, prompt)`; the window shows the names from its dictionary.
pub const TEMPLATES: [(&str, &str); 4] = [
    ("deps", "Update the project's dependencies to their latest compatible versions, one ecosystem at a time. Run the tests after each step and keep only the updates that pass. Do not change the public behaviour."),
    ("security", "Audit the project for security problems: secrets in the tree, injection into commands and queries, unsafe file paths, outdated dependencies with known advisories. Fix what is clearly safe to fix and list the rest with file and line."),
    ("todo", "Find the TODO, FIXME, XXX and HACK comments in the tracked files and resolve the ones that are small and clear. Leave a note with file and line for each one you skipped."),
    ("dead_code", "Find code that nothing uses (unused functions, exports, files, dependencies). Remove what is certainly dead, run the tests, and list the doubtful cases instead of deleting them."),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LimitWindow;

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, h, min, 0).earliest().unwrap()
    }
    fn sched(preset: Preset, time: &str, weekday: u8, created: DateTime<Local>) -> Schedule {
        Schedule { id: "s".into(), title: "t".into(), prompt: "p".into(), project_id: "p1".into(), env: None, model: None, quality: None, preset, at: time.into(), weekday, enabled: true, created_at: created, last_run: None, last_task: None, skipped: None }
    }

    // 2026-10-23 is a Friday (checked against the calendar, not against the code)
    #[test]
    fn daily_before_and_after_the_time_and_across_midnight() {
        assert_eq!(next_after(Preset::Daily, "03:00", 0, at(2026, 10, 23, 1, 0)), Some(at(2026, 10, 23, 3, 0)));
        assert_eq!(next_after(Preset::Daily, "03:00", 0, at(2026, 10, 23, 3, 0)), Some(at(2026, 10, 24, 3, 0)));
        assert_eq!(next_after(Preset::Daily, "00:30", 0, at(2026, 10, 23, 23, 59)), Some(at(2026, 10, 24, 0, 30)));
        assert_eq!(next_after(Preset::Daily, "00:30", 0, at(2026, 12, 31, 23, 59)), Some(at(2027, 1, 1, 0, 30)));
    }

    #[test]
    fn weekdays_skip_the_weekend() {
        // Friday evening -> Monday morning
        assert_eq!(next_after(Preset::Weekdays, "09:00", 0, at(2026, 10, 23, 18, 0)), Some(at(2026, 10, 26, 9, 0)));
        // Saturday and Sunday -> Monday
        assert_eq!(next_after(Preset::Weekdays, "09:00", 0, at(2026, 10, 24, 12, 0)), Some(at(2026, 10, 26, 9, 0)));
        assert_eq!(next_after(Preset::Weekdays, "09:00", 0, at(2026, 10, 25, 12, 0)), Some(at(2026, 10, 26, 9, 0)));
        // Friday morning stays on Friday
        assert_eq!(next_after(Preset::Weekdays, "09:00", 0, at(2026, 10, 23, 8, 0)), Some(at(2026, 10, 23, 9, 0)));
    }

    #[test]
    fn weekly_waits_for_its_day() {
        // Wednesday (2) from Friday -> next Wednesday 28 -> 2026-10-28
        assert_eq!(next_after(Preset::Weekly, "07:15", 2, at(2026, 10, 23, 8, 0)), Some(at(2026, 10, 28, 7, 15)));
        // the same day, time already passed -> a week later
        assert_eq!(next_after(Preset::Weekly, "07:15", 4, at(2026, 10, 23, 8, 0)), Some(at(2026, 10, 30, 7, 15)));
        // the same day, time still ahead
        assert_eq!(next_after(Preset::Weekly, "07:15", 4, at(2026, 10, 23, 7, 0)), Some(at(2026, 10, 23, 7, 15)));
        // Sunday (6) over a month end
        assert_eq!(next_after(Preset::Weekly, "07:15", 6, at(2026, 10, 30, 8, 0)), Some(at(2026, 11, 1, 7, 15)));
    }

    #[test]
    fn a_bad_time_never_fires() {
        assert_eq!(next_after(Preset::Daily, "25:00", 0, at(2026, 10, 23, 8, 0)), None);
        assert_eq!(next_after(Preset::Daily, "soon", 0, at(2026, 10, 23, 8, 0)), None);
    }

    #[test]
    fn sleeping_through_many_firings_gives_one_catch_up() {
        let mut s = sched(Preset::Daily, "03:00", 0, at(2026, 10, 20, 12, 0));
        s.last_run = Some(at(2026, 10, 20, 12, 0));
        // the Mac slept from the 20th to the 23rd 10:00: three firings were missed, due once
        let wake = at(2026, 10, 23, 10, 0);
        assert!(s.due(wake));
        // the caller records the run; the next tick is quiet until the 24th 03:00
        s.last_run = Some(wake);
        assert!(!s.due(at(2026, 10, 23, 10, 0) + Duration::seconds(30)));
        assert!(!s.due(at(2026, 10, 24, 2, 59)));
        assert!(s.due(at(2026, 10, 24, 3, 0)));
    }

    #[test]
    fn a_new_schedule_waits_for_its_first_time_and_off_never_fires() {
        let mut s = sched(Preset::Daily, "03:00", 0, at(2026, 10, 23, 12, 0));
        assert!(!s.due(at(2026, 10, 23, 23, 0)), "created after today's time: first run is tomorrow");
        assert!(s.due(at(2026, 10, 24, 3, 0)));
        s.enabled = false;
        assert!(!s.due(at(2026, 10, 30, 3, 0)));
    }

    fn win(used: Option<f32>, exhausted: bool) -> LimitWindow {
        LimitWindow { name: "5h".into(), model: None, used_pct: used, remaining: None, resets_at: None, exhausted, source: "t".into(), seen_at: Local::now() }
    }

    #[test]
    fn limits_under_ten_percent_left_block() {
        assert!(limit_ok(&EnvLimits::NoData("after_first_run".into())));
        assert!(limit_ok(&EnvLimits::Known(vec![win(Some(89.0), false), win(None, false)])));
        assert!(!limit_ok(&EnvLimits::Known(vec![win(Some(91.0), false)])));
        assert!(!limit_ok(&EnvLimits::Known(vec![win(Some(10.0), false), win(None, true)])));
    }
}
