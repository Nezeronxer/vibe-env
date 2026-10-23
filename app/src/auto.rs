//! F14 automation: scheduled tasks, suggested tasks, the CI watchman. The logic is in `core::{schedule, suggest, ci}`;
//! this is the storage (`<data dir>/automation.json`), the timer and the commands. Nothing starts by itself: a schedule
//! fires only when the person switched it on, the watchman only when its setting is on, a suggestion only on "Сделать".
use crate::hub::{e2s, lk, Hub, Res};
use crate::App;
use chrono::{DateTime, Duration as Span, Local};
use nightshift_core::ci::{self, CiConfig, Gh, PrRef, RealGh, Step, Watch};
use nightshift_core::engine::NewTask;
use nightshift_core::model::{Status, Task};
use nightshift_core::schedule::{self, Schedule};
use nightshift_core::suggest::{self, Suggestion};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

const TICK: Duration = Duration::from_secs(30);
/// The watchman looks every other tick (~60 s).
const CI_EVERY: u64 = 2;
/// A nightly pass of suggestions runs when the last one is older than this.
const NIGHTLY_HOURS: i64 = 20;
/// Pull requests of tasks older than this are not watched.
const WATCH_DAYS: i64 = 30;

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Data {
    schedules: Vec<Schedule>,
    /// Fingerprints of cards the person refused or started, per project.
    dismissed: BTreeMap<String, Vec<String>>,
    cards: BTreeMap<String, Vec<Suggestion>>,
    suggest_nightly: bool,
    last_scan: Option<DateTime<Local>>,
    ci: CiConfig,
    watches: BTreeMap<String, Watch>,
    /// Tasks whose pull request is merged or closed: not asked again.
    closed: BTreeSet<String>,
}

pub struct Auto {
    path: PathBuf,
    d: Mutex<Data>,
    gh: Box<dyn Gh>,
    ci_view: Mutex<Vec<Value>>,
    ci_hint: Mutex<Option<String>>,
}

fn terminal(s: &Status) -> bool {
    matches!(s, Status::Done | Status::DoneNotPushed | Status::Failed | Status::Cancelled | Status::Reverted)
}

impl Auto {
    pub fn open(dir: &std::path::Path, gh: Box<dyn Gh>) -> Auto {
        let path = dir.join("automation.json");
        let d = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        Auto { path, d: Mutex::new(d), gh, ci_view: Mutex::new(vec![]), ci_hint: Mutex::new(None) }
    }

    fn save(&self, d: &Data) {
        let tmp = self.path.with_extension("json.tmp");
        if let Ok(b) = serde_json::to_vec_pretty(d) {
            if std::fs::write(&tmp, b).is_ok() {
                let _ = std::fs::rename(&tmp, &self.path);
            }
        }
    }

    /// Everything the screen shows.
    pub fn get(&self, hub: &Hub, now: DateTime<Local>) -> Value {
        let d = lk(&self.d);
        let schedules: Vec<Value> = d
            .schedules
            .iter()
            .map(|s| {
                let mut v = json!(s);
                v["next"] = json!(if s.enabled { s.next(now) } else { None });
                v
            })
            .collect();
        let projects: BTreeSet<String> = lk(hub.engine.store()).projects().into_iter().map(|p| p.id).collect();
        let cards: BTreeMap<&String, Vec<&Suggestion>> = d.cards.iter().filter(|(p, _)| projects.contains(*p)).map(|(p, c)| (p, c.iter().filter(|c| !d.dismissed.get(p).is_some_and(|x| x.contains(&c.fp))).collect())).collect();
        let count: usize = cards.values().map(Vec::len).sum();
        json!({
            "schedules": schedules,
            "templates": schedule::TEMPLATES.iter().map(|(id, prompt)| json!({ "id": id, "prompt": prompt })).collect::<Vec<_>>(),
            "suggest": { "nightly": d.suggest_nightly, "count": count, "cards": cards, "last_scan": d.last_scan },
            "ci": { "config": d.ci, "hint": *lk(&self.ci_hint), "items": *lk(&self.ci_view) },
        })
    }

    // ---------- schedules ----------

    pub fn save_schedule(&self, hub: &Hub, mut s: Schedule, now: DateTime<Local>) -> Res<String> {
        s.title = s.title.trim().to_string();
        s.prompt = s.prompt.trim().to_string();
        if s.title.is_empty() {
            return Err("empty_title".into());
        }
        if s.prompt.is_empty() {
            return Err("empty_prompt".into());
        }
        if schedule::parse_at(&s.at).is_none() {
            return Err(format!("bad_time: {}", s.at));
        }
        hub.project(&s.project_id)?;
        let mut d = lk(&self.d);
        if let Some(old) = d.schedules.iter_mut().find(|x| !s.id.is_empty() && x.id == s.id) {
            // what the timer owns stays as it was
            (s.created_at, s.last_run, s.last_task, s.skipped) = (old.created_at, old.last_run, old.last_task.clone(), None);
            *old = s.clone();
        } else {
            s.id = schedule::new_id();
            s.created_at = now;
            (s.last_run, s.last_task, s.skipped) = (None, None, None);
            d.schedules.push(s.clone());
        }
        self.save(&d);
        Ok(s.id)
    }

    pub fn remove_schedule(&self, id: &str) {
        let mut d = lk(&self.d);
        d.schedules.retain(|s| s.id != id);
        self.save(&d);
    }

    pub fn toggle_schedule(&self, id: &str, enabled: bool, now: DateTime<Local>) -> Res<()> {
        let mut d = lk(&self.d);
        let s = d.schedules.iter_mut().find(|s| s.id == id).ok_or("schedule_not_found")?;
        // switching on starts the clock now: an old off period is not a missed run
        if enabled && !s.enabled {
            s.created_at = now;
            s.last_run = None;
        }
        s.enabled = enabled;
        s.skipped = None;
        self.save(&d);
        Ok(())
    }

    /// Starts what is due: one task per due schedule, however many firings were missed. true = something changed.
    pub fn tick_schedules(&self, hub: &Hub, now: DateTime<Local>) -> bool {
        if hub.settings().paused {
            return false;
        }
        let due: Vec<Schedule> = lk(&self.d).schedules.iter().filter(|s| s.due(now)).cloned().collect();
        let mut changed = false;
        for s in due {
            let outcome = self.fire(hub, &s);
            let mut d = lk(&self.d);
            let Some(x) = d.schedules.iter_mut().find(|x| x.id == s.id) else { continue };
            match outcome {
                Ok(task) => (x.last_run, x.last_task, x.skipped) = (Some(now), Some(task), None),
                // a reason to wait: tried again at the next tick, the missed moment is not lost
                Err(why) if x.skipped.as_deref() != Some(&why) => x.skipped = Some(why),
                Err(_) => continue,
            }
            self.save(&d);
            changed = true;
        }
        changed
    }

    fn fire(&self, hub: &Hub, s: &Schedule) -> Result<String, String> {
        let p = hub.project_ready(&s.project_id)?;
        let env = s.env.clone().unwrap_or_else(|| p.agent.clone());
        if !schedule::limit_ok(&hub.engine.limits(&env)) {
            return Err("limit".into());
        }
        hub.engine.submit(NewTask { project_id: s.project_id.clone(), prompt: format!("{}\n\n{}", s.title, s.prompt), agent: Some(env), quality: s.quality.clone(), model: s.model.clone(), ..Default::default() })
    }

    // ---------- suggestions ----------

    pub fn scan(&self, hub: &Hub, project_id: &str, now: DateTime<Local>) -> Res<usize> {
        let p = hub.project_ready(project_id)?;
        let dismissed = lk(&self.d).dismissed.get(project_id).cloned().unwrap_or_default();
        let found = suggest::scan(&p.path, &dismissed);
        let n = found.len();
        let mut d = lk(&self.d);
        d.cards.insert(project_id.into(), found);
        d.last_scan = Some(now);
        self.save(&d);
        Ok(n)
    }

    pub fn scan_all(&self, hub: &Hub, now: DateTime<Local>) {
        for p in lk(hub.engine.store()).projects() {
            let _ = self.scan(hub, &p.id, now);
        }
    }

    /// "Сделать": the card's own text goes to `submit_task` (the window sends only the fingerprint).
    pub fn do_suggestion(&self, hub: &Hub, project_id: &str, fp: &str) -> Res<String> {
        let card = lk(&self.d).cards.get(project_id).and_then(|c| c.iter().find(|c| c.fp == fp).cloned()).ok_or("suggestion_not_found")?;
        let id = hub.submit_task(project_id, &card.prompt, None)?;
        self.dismiss(project_id, fp);
        Ok(id)
    }

    pub fn dismiss(&self, project_id: &str, fp: &str) {
        let mut d = lk(&self.d);
        let list = d.dismissed.entry(project_id.into()).or_default();
        if !list.iter().any(|x| x == fp) {
            list.push(fp.into());
        }
        if let Some(c) = d.cards.get_mut(project_id) {
            c.retain(|c| c.fp != fp);
        }
        self.save(&d);
    }

    pub fn set_nightly(&self, on: bool) {
        let mut d = lk(&self.d);
        d.suggest_nightly = on;
        self.save(&d);
    }

    fn nightly_due(&self, now: DateTime<Local>) -> bool {
        let d = lk(&self.d);
        d.suggest_nightly && d.last_scan.is_none_or(|t| now - t >= Span::hours(NIGHTLY_HOURS))
    }

    // ---------- the CI watchman ----------

    pub fn set_ci(&self, cfg: CiConfig) -> Res<()> {
        if !(1..=10).contains(&cfg.max_attempts) {
            return Err("out_of_range: ci.max_attempts 1..10".into());
        }
        let mut d = lk(&self.d);
        // the confirmation is the person's and stays once given; it cannot be set without "merge when green"
        d.ci = CiConfig { merge_confirmed: d.ci.merge_confirmed || cfg.merge_confirmed && cfg.merge_when_green, ..cfg };
        self.save(&d);
        Ok(())
    }

    /// One look at every open pull request of the tasks. true = what the window shows changed.
    pub fn poll_ci(&self, hub: &Hub, now: DateTime<Local>) -> bool {
        let cfg = lk(&self.d).ci.clone();
        let (mut view, mut hint) = (vec![], None);
        if cfg.enabled {
            let tasks = lk(hub.engine.store()).tasks();
            let cutoff = now - Span::days(WATCH_DAYS);
            let skip = lk(&self.d).closed.clone();
            for t in tasks.iter().filter(|t| t.outcome.pr_url.is_some() && !skip.contains(&t.id) && t.created_at >= cutoff) {
                if let Some(item) = self.look(hub, t, &cfg, &tasks, &mut hint) {
                    view.push(item);
                }
            }
        }
        let changed = *lk(&self.ci_view) != view || *lk(&self.ci_hint) != hint;
        *lk(&self.ci_view) = view;
        *lk(&self.ci_hint) = hint;
        changed
    }

    fn look(&self, hub: &Hub, t: &Task, cfg: &CiConfig, tasks: &[Task], hint: &mut Option<String>) -> Option<Value> {
        let url = t.outcome.pr_url.clone()?;
        let p = hub.project_ready(&t.project_id).ok()?;
        let watch = lk(&self.d).watches.get(&t.id).cloned().unwrap_or_default();
        // the PR's own task and the fix tasks: while one of them lives or waits for the person, the watchman keeps out
        let busy = std::iter::once(&t.id).chain(watch.fix_tasks.iter()).any(|id| tasks.iter().find(|x| &x.id == id).is_some_and(|x| !terminal(&x.status)));
        let item = |state: &str, note: Option<&str>, w: &Watch| json!({ "task_id": t.id, "title": t.title, "url": url, "state": state, "attempts": w.attempts, "max": cfg.max_attempts, "note": note });
        let pr = PrRef { url: &url, branch: &t.branch, title: &t.title };
        let (state, step) = match ci::check_pr(self.gh.as_ref(), &p.path, &pr, cfg, &watch, busy) {
            Ok((ci, step)) => (ci, step),
            Err(e) => {
                // no gh or not logged in: the feature says so and stays off for this pass
                *hint = Some(if e.starts_with("gh_not_installed") { "gh_not_installed" } else if e.to_lowercase().contains("auth") || e.contains("login") { "gh_not_logged_in" } else { "gh_error" }.into());
                return Some(item("error", None, &watch));
            }
        };
        let mut w = watch;
        let note = match step {
            Step::Idle => None,
            Step::Wait => Some("waiting"),
            Step::GiveUp => Some("gave_up"),
            Step::AskMerge => Some("ask_merge"),
            Step::Merge => {
                w.merge_asked = true;
                Some(if ci::merge_when_green(self.gh.as_ref(), &p.path, &url).is_ok() { "merge_set" } else { "merge_failed" })
            }
            Step::Fix { prompt, sig } => match hub.submit_task(&t.project_id, &prompt, Some(t.agent.clone())) {
                Ok(id) => {
                    (w.attempts, w.last_sig) = (w.attempts + 1, Some(sig));
                    w.fix_tasks.push(id);
                    Some("fix_started")
                }
                Err(_) => Some("fix_failed"),
            },
        };
        let mut d = lk(&self.d);
        if state == ci::Ci::Closed {
            d.closed.insert(t.id.clone());
            d.watches.remove(&t.id);
            self.save(&d);
            return None;
        }
        let out = item(state.name(), note, &w);
        if d.watches.get(&t.id) != Some(&w) {
            d.watches.insert(t.id.clone(), w);
            self.save(&d);
        }
        Some(out)
    }
}

static AUTO: OnceLock<Arc<Auto>> = OnceLock::new();

fn auto(hub: &Hub) -> &'static Arc<Auto> {
    AUTO.get_or_init(|| Arc::new(Auto::open(&hub.dir, Box::new(RealGh))))
}

/// The timer: schedules every 30 s, the watchman every other tick, the nightly pass of suggestions.
pub fn start(app: &AppHandle, hub: Arc<Hub>) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("automation".into()).spawn(move || {
        let a = auto(&hub).clone();
        for tick in 0u64.. {
            let now = Local::now();
            let mut changed = a.tick_schedules(&hub, now);
            if tick % CI_EVERY == 0 {
                changed |= a.poll_ci(&hub, now);
            }
            if a.nightly_due(now) {
                a.scan_all(&hub, now);
                changed = true;
            }
            if changed {
                let _ = app.emit("auto-changed", ());
            }
            std::thread::sleep(TICK);
        }
    });
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(e2s)
}

#[tauri::command]
pub async fn auto_get(s: State<'_, App>) -> Res<Value> {
    Ok(auto(&s.hub).get(&s.hub, Local::now()))
}

#[tauri::command]
pub async fn auto_schedule_save(s: State<'_, App>, schedule: Schedule) -> Res<String> {
    auto(&s.hub).save_schedule(&s.hub, schedule, Local::now())
}

#[tauri::command]
pub async fn auto_schedule_remove(s: State<'_, App>, id: String) -> Res<()> {
    auto(&s.hub).remove_schedule(&id);
    Ok(())
}

#[tauri::command]
pub async fn auto_schedule_toggle(s: State<'_, App>, id: String, enabled: bool) -> Res<()> {
    auto(&s.hub).toggle_schedule(&id, enabled, Local::now())
}

#[tauri::command]
pub async fn auto_suggest_scan(s: State<'_, App>, project_id: String) -> Res<usize> {
    let hub = s.hub.clone();
    blocking(move || auto(&hub).scan(&hub, &project_id, Local::now())).await?
}

#[tauri::command]
pub async fn auto_suggest_do(s: State<'_, App>, project_id: String, fp: String) -> Res<String> {
    auto(&s.hub).do_suggestion(&s.hub, &project_id, &fp)
}

#[tauri::command]
pub async fn auto_suggest_dismiss(s: State<'_, App>, project_id: String, fp: String) -> Res<()> {
    auto(&s.hub).dismiss(&project_id, &fp);
    Ok(())
}

#[tauri::command]
pub async fn auto_suggest_nightly(s: State<'_, App>, on: bool) -> Res<()> {
    auto(&s.hub).set_nightly(on);
    Ok(())
}

#[tauri::command]
pub async fn auto_ci_set(s: State<'_, App>, config: CiConfig) -> Res<()> {
    auto(&s.hub).set_ci(config)
}

/// "Проверить сейчас".
#[tauri::command]
pub async fn auto_ci_poll(s: State<'_, App>) -> Res<()> {
    let hub = s.hub.clone();
    blocking(move || auto(&hub).poll_ci(&hub, Local::now())).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::*;
    use chrono::TimeZone;
    use nightshift_core::agent::FakeAdapter;
    use nightshift_core::model::{EnvLimits, LimitWindow};
    use nightshift_core::schedule::Preset;
    use std::path::Path;

    /// `gh` from a table: the answer to `pr view` per PR url, every call recorded.
    struct StubGh {
        view: Arc<Mutex<String>>,
        calls: Arc<Mutex<Vec<String>>>,
    }
    impl Gh for StubGh {
        fn run(&self, _: &Path, args: &[&str]) -> Result<String, String> {
            self.calls.lock().unwrap().push(args.join(" "));
            match args {
                ["pr", "view", ..] => Ok(self.view.lock().unwrap().clone()),
                ["run", "view", ..] => Ok("FAIL: test_x".into()),
                _ => Ok(String::new()),
            }
        }
    }

    fn auto_with(rig: &Rig, view: &str) -> (Auto, Arc<Mutex<Vec<String>>>) {
        let (a, calls, _) = auto_view(rig, view);
        (a, calls)
    }
    fn auto_view(rig: &Rig, view: &str) -> (Auto, Arc<Mutex<Vec<String>>>, Arc<Mutex<String>>) {
        let (calls, view) = (Arc::new(Mutex::new(vec![])), Arc::new(Mutex::new(view.to_string())));
        (Auto::open(&rig.s.0, Box::new(StubGh { view: view.clone(), calls: calls.clone() })), calls, view)
    }
    fn day(d: u32, h: u32, m: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, d, h, m, 0).unwrap()
    }
    fn sched(rig: &Rig) -> Schedule {
        Schedule { id: String::new(), title: "Аудит".into(), prompt: "Audit it".into(), project_id: rig.project.id.clone(), env: None, model: None, quality: None, preset: Preset::Daily, at: "03:00".into(), weekday: 0, enabled: true, created_at: day(20, 12, 0), last_run: None, last_task: None, skipped: None }
    }
    fn tasks(rig: &Rig) -> Vec<Task> {
        lk(rig.hub.engine.store()).tasks()
    }

    #[test]
    fn a_schedule_fires_once_after_sleep_through_the_normal_task_path() {
        let rig = Rig::new(FakeAdapter::new(vec![]));
        let (a, _) = auto_with(&rig, "{}");
        let id = a.save_schedule(&rig.hub, sched(&rig), day(20, 12, 0)).unwrap();
        assert!(!a.tick_schedules(&rig.hub, day(20, 23, 0)), "before its time nothing starts");
        assert_eq!(tasks(&rig).len(), 0);
        // three nights slept through: one task, titled with the schedule's name
        assert!(a.tick_schedules(&rig.hub, day(23, 10, 0)));
        assert!(!a.tick_schedules(&rig.hub, day(23, 10, 1)), "not a pile");
        let t = tasks(&rig);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].title, "Аудит");
        assert!(t[0].prompt.contains("Audit it"));
        let s = lk(&a.d).schedules.iter().find(|s| s.id == id).cloned().unwrap();
        assert_eq!(s.last_task.as_deref(), Some(t[0].id.as_str()));
    }

    #[test]
    fn a_disabled_schedule_never_starts_and_turning_on_does_not_replay_the_past() {
        let rig = Rig::new(FakeAdapter::new(vec![]));
        let (a, _) = auto_with(&rig, "{}");
        let mut s = sched(&rig);
        s.enabled = false;
        let id = a.save_schedule(&rig.hub, s, day(20, 12, 0)).unwrap();
        assert!(!a.tick_schedules(&rig.hub, day(25, 10, 0)));
        a.toggle_schedule(&id, true, day(25, 10, 0)).unwrap();
        assert!(!a.tick_schedules(&rig.hub, day(25, 10, 1)), "the days it was off are not missed runs");
        assert_eq!(tasks(&rig).len(), 0);
        assert!(a.tick_schedules(&rig.hub, day(26, 3, 0)));
        assert_eq!(tasks(&rig).len(), 1);
    }

    #[test]
    fn nothing_starts_while_everything_is_paused() {
        let rig = Rig::with(FakeAdapter::new(vec![]), |s| s.paused = true);
        let (a, _) = auto_with(&rig, "{}");
        a.save_schedule(&rig.hub, sched(&rig), day(20, 12, 0)).unwrap();
        assert!(!a.tick_schedules(&rig.hub, day(23, 10, 0)));
        assert_eq!(tasks(&rig).len(), 0);
    }

    #[test]
    fn low_limits_hold_the_run_and_it_goes_when_they_recover() {
        let rig = Rig::new(FakeAdapter::new(vec![]));
        let (a, _) = auto_with(&rig, "{}");
        let w = |used: f32| EnvLimits::Known(vec![LimitWindow { name: "5h".into(), model: None, used_pct: Some(used), remaining: None, resets_at: None, exhausted: false, source: "t".into(), seen_at: day(23, 1, 0) }]);
        lk(rig.hub.engine.store()).set_limits("fake", w(95.0)).unwrap();
        a.save_schedule(&rig.hub, sched(&rig), day(20, 12, 0)).unwrap();
        assert!(a.tick_schedules(&rig.hub, day(21, 4, 0)), "the reason is recorded");
        assert_eq!(tasks(&rig).len(), 0);
        assert_eq!(lk(&a.d).schedules[0].skipped.as_deref(), Some("limit"));
        assert!(!a.tick_schedules(&rig.hub, day(21, 4, 1)), "same reason: quiet");
        lk(rig.hub.engine.store()).set_limits("fake", w(40.0)).unwrap();
        assert!(a.tick_schedules(&rig.hub, day(21, 5, 0)));
        assert_eq!(tasks(&rig).len(), 1);
        assert_eq!(lk(&a.d).schedules[0].skipped, None);
    }

    #[test]
    fn bad_schedules_are_refused() {
        let rig = Rig::new(FakeAdapter::new(vec![]));
        let (a, _) = auto_with(&rig, "{}");
        let mut s = sched(&rig);
        s.at = "7pm".into();
        assert_eq!(a.save_schedule(&rig.hub, s, day(20, 12, 0)).unwrap_err(), "bad_time: 7pm");
        let mut s = sched(&rig);
        s.project_id = "nope".into();
        assert!(a.save_schedule(&rig.hub, s, day(20, 12, 0)).unwrap_err().starts_with("project_not_found"));
    }

    #[test]
    fn suggestions_scan_dismiss_and_do() {
        let rig = Rig::new(FakeAdapter::new(vec![]));
        std::fs::write(rig.repo.join("a.txt"), "one\n// TODO: first\n// FIXME: second\n").unwrap();
        g(&rig.repo, &["add", "-A"]);
        g(&rig.repo, &["commit", "-q", "-m", "todos"]);
        let (a, _) = auto_with(&rig, "{}");
        assert_eq!(a.scan(&rig.hub, &rig.project.id, day(23, 1, 0)).unwrap(), 2);
        let v = a.get(&rig.hub, day(23, 1, 0));
        assert_eq!(v["suggest"]["count"], 2);
        let cards = v["suggest"]["cards"][&rig.project.id].as_array().unwrap().clone();
        assert_eq!(cards[0]["file"], "a.txt");
        a.dismiss(&rig.project.id, cards[0]["fp"].as_str().unwrap());
        assert_eq!(a.get(&rig.hub, day(23, 1, 0))["suggest"]["count"], 1);
        assert_eq!(a.scan(&rig.hub, &rig.project.id, day(23, 2, 0)).unwrap(), 1, "a refused one does not come back");
        let fp = cards[1]["fp"].as_str().unwrap();
        let id = a.do_suggestion(&rig.hub, &rig.project.id, fp).unwrap();
        assert!(rig.task(&id).prompt.contains("a.txt"));
        assert_eq!(a.get(&rig.hub, day(23, 2, 0))["suggest"]["count"], 0);
        assert_eq!(a.do_suggestion(&rig.hub, &rig.project.id, fp).unwrap_err(), "suggestion_not_found");
        // the state outlives a restart
        let b = Auto::open(&rig.s.0, Box::new(StubGh { view: Arc::default(), calls: Arc::default() }));
        assert_eq!(lk(&b.d).dismissed[&rig.project.id].len(), 2);
    }

    const RED: &str = r#"{"state":"OPEN","mergeable":"MERGEABLE","statusCheckRollup":[{"name":"test","status":"COMPLETED","conclusion":"FAILURE","detailsUrl":"https://github.com/o/r/actions/runs/5/job/1"}]}"#;

    fn with_pr(rig: &Rig) -> String {
        let id = rig.hub.submit_task(&rig.project.id, "Add f", None).unwrap();
        let t = rig.wait_task(&id, "task finished", |t| terminal(&t.status));
        let mut t = t;
        t.outcome.pr_url = Some("https://github.com/o/r/pull/9".into());
        t.created_at = day(23, 0, 0);
        lk(rig.hub.engine.store()).upsert_task(&t).unwrap();
        id
    }

    #[test]
    fn the_watchman_is_off_until_the_person_turns_it_on() {
        let rig = Rig::new(FakeAdapter::new(vec![]));
        with_pr(&rig);
        let (a, calls) = auto_with(&rig, RED);
        assert!(!a.poll_ci(&rig.hub, day(23, 12, 0)));
        assert!(calls.lock().unwrap().is_empty(), "no gh call while the setting is off");
        assert_eq!(tasks(&rig).len(), 1);
    }

    #[test]
    fn red_ci_makes_one_fix_task_then_waits_for_it_then_gives_up_at_the_limit() {
        let rig = Rig::new(FakeAdapter::new(vec![]));
        let pr_task = with_pr(&rig);
        let (a, _, view) = auto_view(&rig, RED);
        a.set_ci(CiConfig { enabled: true, max_attempts: 1, ..Default::default() }).unwrap();
        assert!(a.poll_ci(&rig.hub, day(23, 12, 0)));
        let fixes: Vec<Task> = tasks(&rig).into_iter().filter(|t| t.id != pr_task).collect();
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].title.starts_with("Починить CI:") && fixes[0].prompt.contains("FAIL: test_x"));
        assert_eq!(lk(&a.ci_view)[0]["note"], "fix_started");
        // the same failure again: the fix is not made twice
        a.poll_ci(&rig.hub, day(23, 12, 1));
        assert_eq!(tasks(&rig).len(), 2);
        // a new failing run after the fix, attempts are spent
        rig.wait_task(&fixes[0].id, "fix finished", |t| terminal(&t.status));
        *view.lock().unwrap() = RED.replace("runs/5", "runs/6");
        a.poll_ci(&rig.hub, day(23, 12, 2));
        assert_eq!(tasks(&rig).len(), 2);
        assert_eq!(lk(&a.ci_view)[0]["note"], "gave_up");
    }

    #[test]
    fn a_missing_gh_is_a_hint_and_a_merged_pr_is_forgotten() {
        let rig = Rig::new(FakeAdapter::new(vec![]));
        with_pr(&rig);
        struct NoGh;
        impl Gh for NoGh {
            fn run(&self, _: &Path, _: &[&str]) -> Result<String, String> {
                Err("gh_not_installed".into())
            }
        }
        let a = Auto::open(&rig.s.0, Box::new(NoGh));
        a.set_ci(CiConfig { enabled: true, ..Default::default() }).unwrap();
        assert!(a.poll_ci(&rig.hub, day(23, 12, 0)));
        assert_eq!(*lk(&a.ci_hint), Some("gh_not_installed".into()));
        let (b, _) = auto_with(&rig, r#"{"state":"MERGED","statusCheckRollup":[]}"#);
        b.set_ci(CiConfig { enabled: true, ..Default::default() }).unwrap();
        b.poll_ci(&rig.hub, day(23, 12, 0));
        assert!(lk(&b.ci_view).is_empty() && lk(&b.d).closed.len() == 1);
    }
}
