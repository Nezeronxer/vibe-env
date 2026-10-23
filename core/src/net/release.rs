//! "Вышла версия X" (G69): the latest release of the app itself, read from `Settings.release_feed` (the GitHub API
//! `releases/latest` of the published repository) through `Fetch`, once a day and on the button in "О программе".
//! Without a feed nothing is fetched. A failure is quiet: it is kept with the time for "О программе".
use super::{Fetch, NetError};
use chrono::{DateTime, Duration, Local};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How often the app looks for a new release by itself.
pub const CHECK_EVERY_HOURS: i64 = 24;
/// The answer of `releases/latest` is a small JSON page.
const MAX_BYTES: usize = 512 * 1024;
/// The error when no feed is set: the window says "адрес обновлений не задан".
pub const NO_FEED: &str = "no_feed";

/// What is kept between starts (`<data>/updates/app_release.json`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ReleaseState {
    pub checked_at: Option<DateTime<Local>>,
    pub latest: Option<String>,
    pub url: Option<String>,
    /// The key of the last failure (`no_connection`, `timeout`, `bad_answer`...); None = the last check went through.
    pub error: Option<String>,
    /// The version whose bar the person closed: it does not come back until a newer one.
    pub dismissed: Option<String>,
}

/// What the window gets (`app_update_check`).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct UpdateCheck {
    pub current: String,
    pub latest: Option<String>,
    pub url: Option<String>,
    /// rfc3339 of the last check; None = never.
    pub checked_at: Option<String>,
    pub error: Option<String>,
    /// A feed is set (else the check is off).
    pub feed: bool,
    /// The bar is shown: a newer version, not closed by the person.
    pub show: bool,
}

/// `tag_name` (a leading `v` dropped) and `html_url` (https only) of a GitHub release.
pub fn parse_release(body: &[u8]) -> Result<(String, String), String> {
    let v: serde_json::Value = serde_json::from_slice(body).map_err(|_| "bad_answer".to_string())?;
    let tag = v["tag_name"].as_str().map(|t| t.trim().trim_start_matches(['v', 'V']).to_string()).filter(|t| is_version(t));
    let url = v["html_url"].as_str().filter(|u| u.starts_with("https://") && !u.contains(['\n', '\r', ' ', '"', '<'])).map(String::from);
    tag.zip(url).ok_or_else(|| "bad_answer".to_string())
}

fn is_version(t: &str) -> bool {
    !t.is_empty() && t.len() <= 40 && t.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+')) && t.starts_with(|c: char| c.is_ascii_digit())
}

/// `a` is newer than `b` (numbers by dots; a suffix after `-` is ignored).
pub fn newer(a: &str, b: &str) -> bool {
    let core = |s: &str| s.split(['-', '+']).next().unwrap_or("").to_string();
    crate::models::version_cmp(&core(a), &core(b)) == std::cmp::Ordering::Greater
}

pub struct Releases {
    path: PathBuf,
}

impl Releases {
    pub fn new(data_dir: &Path) -> Releases {
        Releases { path: data_dir.join("updates").join("app_release.json") }
    }

    pub fn state(&self) -> ReleaseState {
        std::fs::read(&self.path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn save(&self, s: &ReleaseState) {
        if let Some(d) = self.path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(b) = serde_json::to_vec_pretty(s) {
            let tmp = self.path.with_extension("json.tmp");
            if std::fs::write(&tmp, b).is_ok() {
                let _ = std::fs::rename(&tmp, &self.path);
            }
        }
    }

    /// What the window shows now, without the network.
    pub fn view(&self, current: &str, feed: Option<&str>) -> UpdateCheck {
        let s = self.state();
        let feed_set = feed.is_some_and(|f| !f.trim().is_empty());
        let show = feed_set && s.latest.as_deref().is_some_and(|l| newer(l, current) && s.dismissed.as_deref() != Some(l));
        UpdateCheck {
            current: current.to_string(),
            latest: s.latest.clone(),
            url: s.url.clone(),
            checked_at: s.checked_at.map(|d| d.to_rfc3339()),
            error: if feed_set { s.error.clone() } else { Some(NO_FEED.into()) },
            feed: feed_set,
            show,
        }
    }

    /// A day has passed since the last check (or there was none): the app checks by itself.
    pub fn due(&self, now: DateTime<Local>) -> bool {
        self.state().checked_at.is_none_or(|t| now - t >= Duration::hours(CHECK_EVERY_HOURS))
    }

    /// Asks the feed for the latest release. No feed = nothing is fetched. A failure keeps what was known before
    /// and says what went wrong, with the time.
    pub fn check(&self, f: &dyn Fetch, feed: Option<&str>, current: &str, now: DateTime<Local>) -> UpdateCheck {
        let Some(feed) = feed.map(str::trim).filter(|x| !x.is_empty()) else { return self.view(current, None) };
        let mut s = self.state();
        s.checked_at = Some(now);
        match f.get(feed, MAX_BYTES, None) {
            Ok(out) if out.status == 200 => match parse_release(&out.body) {
                Ok((tag, url)) => {
                    s.latest = Some(tag);
                    s.url = Some(url);
                    s.error = None;
                }
                Err(e) => s.error = Some(e),
            },
            Ok(out) => s.error = Some(format!("http_{}", out.status)),
            Err(e) => s.error = Some(NetError::key(&e).to_string()),
        }
        self.save(&s);
        self.view(current, Some(feed))
    }

    /// The person closed the bar of `version`: it stays closed until a newer one comes.
    pub fn dismiss(&self, version: &str) {
        let mut s = self.state();
        s.dismissed = Some(version.to_string());
        self.save(&s);
    }
}
