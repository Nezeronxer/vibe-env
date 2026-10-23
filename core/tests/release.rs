//! "Вышла версия X" (G69): the latest release from `release_feed`, without the network in the test.
use chrono::{Duration, Local};
use nightshift_core::model::Settings;
use nightshift_core::net::release::{newer, parse_release, Releases, NO_FEED};
use nightshift_core::net::{FakeFetch, FetchOut, NetError};

const FEED: &str = "https://api.github.com/repos/someone/nightshift/releases/latest";

fn answer(tag: &str) -> String {
    serde_json::json!({ "tag_name": tag, "html_url": format!("https://github.com/someone/nightshift/releases/tag/{tag}"), "body": "notes" }).to_string()
}

#[test]
fn without_a_feed_the_check_is_off_fetches_nothing_and_says_why() {
    assert_eq!(Settings::default().release_feed, None, "no address until the repository is published");
    let d = tempfile::tempdir().unwrap();
    let r = Releases::new(d.path());
    let f = FakeFetch::new();
    let v = r.check(&f, None, "0.1.0", Local::now());
    assert!(f.calls().is_empty(), "nothing goes to the network");
    assert!(!v.feed && !v.show);
    assert_eq!(v.error.as_deref(), Some(NO_FEED));
    assert_eq!(v.checked_at, None);
}

#[test]
fn a_newer_release_shows_the_bar_once_closed_it_stays_closed_until_the_next_version() {
    let d = tempfile::tempdir().unwrap();
    let r = Releases::new(d.path());
    let now = Local::now();
    let f = FakeFetch::new().with(FEED, answer("v0.2.0"));
    let v = r.check(&f, Some(FEED), "0.1.0", now);
    assert_eq!(f.calls().len(), 1);
    assert_eq!((v.latest.as_deref(), v.show, v.error.as_deref()), (Some("0.2.0"), true, None));
    assert_eq!(v.url.as_deref(), Some("https://github.com/someone/nightshift/releases/tag/v0.2.0"));
    assert!(!r.due(now + Duration::hours(23)), "once a day");
    assert!(r.due(now + Duration::hours(24)));

    r.dismiss("0.2.0");
    assert!(!r.check(&f, Some(FEED), "0.1.0", now).show, "closed: not back for the same version");
    assert!(!Releases::new(d.path()).view("0.1.0", Some(FEED)).show, "kept after a restart");
    f.set(FEED, Ok(FetchOut { status: 200, body: answer("0.3.0").into(), etag: None }));
    assert!(r.check(&f, Some(FEED), "0.1.0", now).show, "a newer version shows again");
    assert!(!r.check(&f, Some(FEED), "0.3.0", now).show, "the version that runs is not news");
}

#[test]
fn a_failure_is_kept_quietly_with_its_time_and_what_was_known_stays() {
    let d = tempfile::tempdir().unwrap();
    let r = Releases::new(d.path());
    let t0 = Local::now();
    let f = FakeFetch::new().with(FEED, answer("0.2.0"));
    r.check(&f, Some(FEED), "0.1.0", t0);
    f.set(FEED, Err(NetError::Failed("could not resolve host".into())));
    let t1 = t0 + Duration::hours(25);
    let v = r.check(&f, Some(FEED), "0.1.0", t1);
    assert_eq!(v.error.as_deref(), Some("no_connection"));
    assert_eq!(v.checked_at, Some(t1.to_rfc3339()));
    assert_eq!(v.latest.as_deref(), Some("0.2.0"), "what was known before is kept");
    f.set(FEED, Ok(FetchOut { status: 404, body: b"{}".to_vec(), etag: None }));
    assert_eq!(r.check(&f, Some(FEED), "0.1.0", t1).error.as_deref(), Some("http_404"));
}

#[test]
fn an_answer_that_is_not_a_release_is_refused() {
    assert!(parse_release(b"not json").is_err());
    assert!(parse_release(br#"{"tag_name":"v1.0.0","html_url":"http://x/1"}"#).is_err(), "https only");
    assert!(parse_release(br#"{"tag_name":"--evil","html_url":"https://x/1"}"#).is_err());
    assert_eq!(parse_release(br#"{"tag_name":"v1.2.10","html_url":"https://x/1"}"#).unwrap().0, "1.2.10");
    assert!(newer("0.10.0", "0.9.3") && !newer("0.1.0", "0.1.0") && !newer("0.1.0-rc1", "0.1.0"));
}
