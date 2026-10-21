//! Presence and sleep (spec 27): "the person is here / away" for the engine, and a hold on the system's sleep while agents
//! work. Platform code that the core does not have by design; the core only gets `set_presence(away)`.
use crate::hub::{lk, Hub};
use nightshift_core::engine::is_active;
use nightshift_core::model::Task;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

/// Away = the screen is locked, or no input for `away_min` minutes. Unknown idle time (a Linux desktop that does not
/// say) means only the lock counts; the night window then decides alone.
pub fn decide(idle_secs: Option<u64>, locked: bool, away_min: u32) -> bool {
    locked || idle_secs.is_some_and(|s| s >= u64::from(away_min) * 60)
}

/// `"HIDIdleTime" = 90130875` (nanoseconds) from `ioreg -c IOHIDSystem -d 4 -r`.
pub fn parse_hid_idle(ioreg: &str) -> Option<u64> {
    let line = ioreg.lines().find(|l| l.contains("\"HIDIdleTime\""))?;
    line.rsplit('=').next()?.trim().parse::<u64>().ok().map(|ns| ns / 1_000_000_000)
}

/// `"IOConsoleLocked" = Yes` from `ioreg -n Root -d1`.
pub fn parse_console_locked(ioreg: &str) -> bool {
    ioreg.lines().any(|l| l.contains("\"IOConsoleLocked\"") && l.trim_end().ends_with("Yes"))
}

fn run(bin: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(bin).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok().filter(|o| o.status.success())?;
    Some(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// (seconds without input if the system says, screen locked).
pub fn probe(away_min: u32) -> (Option<u64>, bool) {
    if cfg!(target_os = "macos") {
        let idle = run("ioreg", &["-c", "IOHIDSystem", "-d", "4", "-r"]).and_then(|t| parse_hid_idle(&t));
        let locked = run("ioreg", &["-n", "Root", "-d1"]).is_some_and(|t| parse_console_locked(&t));
        return (idle, locked);
    }
    // Linux: X11 idle time if `xprintidle` exists, else the session's own idle and lock hints from logind
    let session = std::env::var("XDG_SESSION_ID").unwrap_or_else(|_| "auto".into());
    let hints = run("loginctl", &["show-session", &session, "-p", "LockedHint", "-p", "IdleHint"]).unwrap_or_default();
    let locked = hints.lines().any(|l| l.trim() == "LockedHint=yes");
    let idle = run("xprintidle", &[]).and_then(|t| t.trim().parse::<u64>().ok()).map(|ms| ms / 1000).or_else(|| hints.lines().any(|l| l.trim() == "IdleHint=yes").then(|| u64::from(away_min) * 60));
    (idle, locked)
}

/// Something is working: the system must not fall asleep (A08.2). Off when the person turned "Не давать уснуть" off.
pub fn want_awake(tasks: &[Task], keep_awake: bool) -> bool {
    keep_awake && tasks.iter().any(|t| is_active(&t.status))
}

/// The program that holds the sleep and ends by itself when the app (`pid`) is gone.
/// macOS: `caffeinate -i` is a power assertion against idle sleep; Linux: `systemd-inhibit` around a waiting loop.
pub fn inhibit_command(mac: bool, pid: u32) -> (String, Vec<String>) {
    let pid = pid.to_string();
    if mac {
        ("caffeinate".into(), vec!["-i".into(), "-w".into(), pid])
    } else {
        let wait = r#"while kill -0 "$1" 2>/dev/null; do sleep 5; done"#;
        ("systemd-inhibit".into(), ["--what=sleep", "--who=Nightshift", "--why=Agents are working", "--mode=block", "sh", "-c", wait, "sh", pid.as_str()].map(String::from).to_vec())
    }
}

type Spawn = Box<dyn Fn() -> Option<Child> + Send + Sync>;

/// The hold itself: started when work begins, ended when it stops or the app exits.
pub struct KeepAwake {
    child: Mutex<Option<Child>>,
    spawn: Spawn,
}

impl KeepAwake {
    pub fn system() -> KeepAwake {
        KeepAwake::with(Box::new(|| {
            let (bin, args) = inhibit_command(cfg!(target_os = "macos"), std::process::id());
            Command::new(bin).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().ok()
        }))
    }

    pub fn with(spawn: Spawn) -> KeepAwake {
        KeepAwake { child: Mutex::new(None), spawn }
    }

    /// True while the hold is on.
    pub fn update(&self, want: bool) -> bool {
        let mut c = lk(&self.child);
        if c.as_mut().is_some_and(|ch| ch.try_wait().ok().flatten().is_some()) {
            *c = None; // it ended by itself: start again if still wanted
        }
        match (want, c.is_some()) {
            (true, false) => *c = (self.spawn)(),
            (false, true) => {
                if let Some(mut ch) = c.take() {
                    let _ = ch.kill();
                    let _ = ch.wait();
                }
            }
            _ => {}
        }
        c.is_some()
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        self.update(false);
    }
}

impl Hub {
    /// Whether the person is away is the app's call; the engine starts night tasks while they are.
    pub fn set_presence(&self, away: bool) {
        self.engine.set_presence(away);
    }

    pub fn keep_awake_wanted(&self) -> bool {
        let tasks = lk(self.engine.store()).tasks();
        want_awake(&tasks, self.settings().keep_awake)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::*;
    use nightshift_core::agent::{FakeAdapter, FakeStep};
    use nightshift_core::model::{RoleKind, StartWhen, Status};

    #[test]
    fn away_after_the_idle_minutes_or_a_locked_screen_and_never_on_an_unknown_idle_time() {
        assert!(!decide(Some(19 * 60 + 59), false, 20));
        assert!(decide(Some(20 * 60), false, 20));
        assert!(decide(Some(5), true, 20), "a locked screen is away at once");
        assert!(!decide(None, false, 20));
    }

    #[test]
    fn the_idle_time_and_the_lock_are_read_from_what_ioreg_prints() {
        assert_eq!(parse_hid_idle("  | {\n  |   \"HIDIdleTime\" = 125000000000\n  | }"), Some(125));
        assert_eq!(parse_hid_idle("nothing here"), None);
        assert!(parse_console_locked("  \"IOConsoleLocked\" = Yes\n"));
        assert!(!parse_console_locked("  \"IOConsoleLocked\" = No\n"));
        assert!(!parse_console_locked(""));
    }

    #[test]
    fn the_hold_on_sleep_command_waits_for_the_app_and_takes_the_pid_as_a_word() {
        assert_eq!(inhibit_command(true, 42), ("caffeinate".to_string(), ["-i", "-w", "42"].map(String::from).to_vec()));
        let (bin, args) = inhibit_command(false, 42);
        assert_eq!(bin, "systemd-inhibit");
        assert_eq!(args.last().unwrap(), "42");
        assert!(args.contains(&"--what=sleep".to_string()) && args.contains(&"--mode=block".to_string()));
        assert!(!args[6].contains("42"), "the script text does not hold the number");
    }

    #[test]
    fn the_hold_is_on_while_agents_work_and_off_when_they_stop_or_the_setting_is_off() {
        let alive = |k: &KeepAwake| lk(&k.child).as_mut().is_some_and(|c| c.try_wait().unwrap().is_none());
        let k = KeepAwake::with(Box::new(|| Command::new("sleep").arg("30").spawn().ok()));
        assert!(k.update(true) && alive(&k));
        let pid = lk(&k.child).as_ref().unwrap().id();
        assert!(k.update(true));
        assert_eq!(lk(&k.child).as_ref().unwrap().id(), pid, "one hold, not one per tick");
        assert!(!k.update(false));
        assert!(lk(&k.child).is_none());

        let t = |status| Task { status, ..Task::default() };
        assert!(want_awake(&[t(Status::Done), t(Status::Working)], true));
        assert!(!want_awake(&[t(Status::Working)], false), "the person switched it off");
        assert!(!want_awake(&[t(Status::Done), t(Status::NeedsHuman), t(Status::Queued)], true), "waiting is not working");
    }

    #[test]
    fn a_night_task_waits_in_the_day_and_starts_when_the_person_is_reported_away() {
        let fake = FakeAdapter::new(vec![])
            .with_role(RoleKind::Worker, vec![vec![FakeStep::Write("f.txt".into(), "f".into()), FakeStep::Done("f".into())]])
            .with_role(RoleKind::Reviewer, vec![vec![FakeStep::Done(r#"{"verdict":"approve","issues":[]}"#.into())]]);
        let r = Rig::new(fake);
        let id = r.hub.engine.submit(nightshift_core::engine::NewTask { project_id: r.project.id.clone(), prompt: "Add f".into(), start: Some(StartWhen::Night), ..Default::default() }).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(400));
        assert_eq!(r.task(&id).status, Status::Queued, "12:00, the person is here");
        r.hub.set_presence(true);
        r.wait_task(&id, "it ran", |t| t.status == Status::Done);
    }
}
