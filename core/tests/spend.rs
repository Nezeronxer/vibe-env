//! The spending history (G70): every run is a row of its day, project and env; an env that says nothing of
//! money is "no data", never 0; old days fold into months.
mod common;
use chrono::NaiveDate;
use common::*;
use nightshift_core::agent::FakeStep;
use nightshift_core::model::*;
use nightshift_core::notify::RecordingNotifier;
use nightshift_core::store::{SpendRow, Store, SPEND_KEEP_DAYS};
use std::sync::Arc;

fn day(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

fn row(d: &str, project: &str, env: &str, usd: Option<f64>) -> SpendRow {
    SpendRow { day: day(d), month: false, project: Some(project.into()), env: env.into(), chat: false, usd, tokens_in: None, tokens_out: None }
}

#[test]
fn what_a_task_costs_is_a_row_of_its_day_project_and_env() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = fake(vec![vec![write("feature.txt", "hello\n"), FakeStep::Cost(0.25), done("Added feature.txt")]]);
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add a feature file");
    wait_status(&e, &id, Status::Done);

    let rows = e.spend_history(None, None);
    let mine: Vec<&SpendRow> = rows.iter().filter(|r| r.env == "fake").collect();
    assert_eq!(mine.len(), 1, "{rows:?}");
    assert_eq!(mine[0].project.as_deref(), Some(p.id.as_str()));
    assert_eq!(mine[0].day, chrono::Local::now().date_naive());
    assert!(!mine[0].chat);
    assert!((mine[0].usd.unwrap() - 0.25).abs() < 1e-9);
    // the history is a file of its own next to the day's spending
    assert!(env.d.path().join("data").join("spend_history.json").exists());
}

#[test]
fn an_env_that_says_nothing_of_money_is_no_data_not_zero() {
    let env = Env::new();
    let p = env.project(|_| {});
    let w = Arc::new(Knobs::new(fake(vec![vec![write("feature.txt", "hello\n"), done("Added feature.txt")]])).no_cost());
    let e = env.engine(env.store(&p, |_| {}), vec![w, reviewer(1)], Arc::new(RecordingNotifier::default()), None);
    let id = submit(&e, &p, "Add a feature file");
    wait_status(&e, &id, Status::Done);
    let rows = e.spend_history(None, None);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!((rows[0].project.as_deref(), rows[0].env.as_str(), rows[0].usd), (Some(p.id.as_str()), "fake", None));
}

#[test]
fn runs_of_the_same_day_project_and_env_add_up_and_money_after_no_data_is_money() {
    let d = tempfile::tempdir().unwrap();
    let st = Store::open(d.path()).unwrap();
    let today = day("2026-10-23");
    st.add_spend(row("2026-10-23", "p1", "claude", None), today).unwrap();
    st.add_spend(row("2026-10-23", "p1", "claude", Some(0.5)), today).unwrap();
    st.add_spend(row("2026-10-23", "p1", "claude", Some(0.25)), today).unwrap();
    st.add_spend(row("2026-10-23", "p2", "claude", Some(1.0)), today).unwrap();
    let mut chat = row("2026-10-23", "p1", "claude", Some(0.1));
    chat.chat = true;
    st.add_spend(chat, today).unwrap();

    let all = st.spend_history(None, None);
    assert_eq!(all.len(), 3, "{all:?}");
    let p1 = all.iter().find(|r| r.project.as_deref() == Some("p1") && !r.chat).unwrap();
    assert_eq!(p1.usd, Some(0.75));
    assert!(all.iter().any(|r| r.chat && r.usd == Some(0.1)), "the chat is a row of its own");
    assert!(Store::open(d.path()).unwrap().spend_history(Some(today), Some(today)).len() == 3, "kept on disk");
}

#[test]
fn days_older_than_four_hundred_fold_into_their_month() {
    let d = tempfile::tempdir().unwrap();
    let st = Store::open(d.path()).unwrap();
    let today = day("2026-10-23");
    assert_eq!(SPEND_KEEP_DAYS, 400);
    // 2025-09-10 and 2025-09-12 are more than 400 days before today; 2025-09-20 is 398 days before
    st.add_spend(row("2025-09-10", "p1", "claude", Some(1.0)), today).unwrap();
    st.add_spend(row("2025-09-12", "p1", "claude", Some(2.0)), today).unwrap();
    st.add_spend(row("2025-09-20", "p1", "claude", Some(4.0)), today).unwrap();
    st.add_spend(row("2026-10-23", "p1", "claude", Some(8.0)), today).unwrap();

    let all = st.spend_history(None, None);
    let month: Vec<&SpendRow> = all.iter().filter(|r| r.month).collect();
    assert_eq!(month.len(), 1, "{all:?}");
    assert_eq!(month[0].day, day("2025-09-01"));
    assert_eq!(month[0].usd, Some(3.0));
    assert!(all.iter().any(|r| !r.month && r.day == day("2025-09-20") && r.usd == Some(4.0)));
    assert_eq!(st.spend_history(Some(day("2026-01-01")), None).len(), 1);
}
