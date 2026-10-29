//! Token use of a terminal agent from the CLI's own session files: a fake HOME with fixture lines.
use nightshift_core::sessions::key_for;
use nightshift_core::termusage::{usage_in, Usage};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// 2020-01-01T10:00:00Z: the files are written "now", so their change time is always later than this.
const SINCE: u64 = 1_577_872_800_000;

fn lines(path: &Path, rows: &[&str]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, rows.join("\n") + "\n").unwrap();
}

fn claude_row(id: &str, ts: &str, cwd: &str, usage: &str, extra: &str) -> String {
    format!(r#"{{"type":"assistant","timestamp":"{ts}","cwd":"{cwd}","message":{{"id":"{id}","usage":{usage}}}{extra}}}"#)
}

fn old(path: &Path) {
    let f = fs::File::options().write(true).open(path).unwrap();
    f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_500_000_000)).unwrap();
}

struct Home {
    _d: tempfile::TempDir,
    home: PathBuf,
    cwd: PathBuf,
}

fn home() -> Home {
    let d = tempfile::tempdir().unwrap();
    let home = d.path().canonicalize().unwrap();
    let cwd = home.join("work").join("my proj");
    fs::create_dir_all(&cwd).unwrap();
    Home { _d: d, home, cwd }
}

impl Home {
    fn claude(&self) -> PathBuf {
        self.home.join(".claude")
    }
    fn session(&self, name: &str) -> PathBuf {
        self.claude().join("projects").join(key_for(&self.cwd)).join(name)
    }
    fn get(&self, env: &str, since: u64) -> Usage {
        usage_in(Some(&self.claude()), Some(&self.home.join(".codex")), &self.cwd, env, since)
    }
}

#[test]
fn claude_usage_sums_messages_once_and_counts_cache_as_input() {
    let h = home();
    let cwd = h.cwd.to_str().unwrap();
    let t = |s: &str| format!("2020-01-01T10:{s}Z");
    let streaming = claude_row("m1", &t("00:01"), cwd, r#"{"input_tokens":10,"cache_read_input_tokens":100,"cache_creation_input_tokens":5,"output_tokens":1}"#, "");
    let final_ = claude_row("m1", &t("00:02"), cwd, r#"{"input_tokens":10,"cache_read_input_tokens":100,"cache_creation_input_tokens":5,"output_tokens":20}"#, "");
    let priced = claude_row("m2", &t("01:00"), cwd, r#"{"input_tokens":3,"output_tokens":7}"#, r#","costUSD":0.5"#);
    let before = claude_row("m0", "2019-12-31T10:00:00Z", cwd, r#"{"input_tokens":1000,"output_tokens":1000}"#, "");
    let elsewhere = claude_row("m9", &t("02:00"), "/some/other/place", r#"{"input_tokens":1000,"output_tokens":1000}"#, "");
    let user = r#"{"type":"user","timestamp":"2020-01-01T10:00:05Z","message":{"content":"hi"}}"#;
    lines(&h.session("s1.jsonl"), &[&streaming, user, &final_, &priced, &before, &elsewhere, r#"{"type":"assistant","time"#, "not json"]);
    // a resumed session repeats the old messages; a file untouched since `since` is not read at all
    lines(&h.session("s2.jsonl"), &[&final_]);
    lines(&h.session("stale.jsonl"), &[&claude_row("m5", &t("03:00"), cwd, r#"{"input_tokens":1000,"output_tokens":1000}"#, "")]);
    old(&h.session("stale.jsonl"));
    assert_eq!(h.get("claude", SINCE), Usage { tokens_in: Some(118), tokens_out: Some(27), usd: Some(0.5) });
    // later start: only the second message
    assert_eq!(h.get("claude", SINCE + 30_000), Usage { tokens_in: Some(3), tokens_out: Some(7), usd: Some(0.5) });
    // after everything: no data, not zeros
    assert_eq!(h.get("claude", SINCE + 3_600_000), Usage::default());
}

#[test]
fn claude_usage_is_not_priced_by_us_and_is_empty_without_files_or_for_other_envs() {
    let h = home();
    assert_eq!(h.get("claude", 0), Usage::default(), "no folder of sessions");
    let cwd = h.cwd.to_str().unwrap();
    lines(&h.session("s.jsonl"), &[&claude_row("a", "2020-01-01T10:00:01Z", cwd, r#"{"input_tokens":4,"output_tokens":2}"#, "")]);
    assert_eq!(h.get("claude", 0), Usage { tokens_in: Some(4), tokens_out: Some(2), usd: None }, "no cost field, no price");
    for env in ["gemini", "opencode", "kimi", "copilot", "crush", "amp", "goose", "droid", "aider", "grok", "custom:x", ""] {
        assert_eq!(h.get(env, 0), Usage::default(), "{env}");
    }
    // another folder with the same folder-name key is not this one's
    let twin = h.home.join("work").join("my-proj");
    assert_eq!(key_for(&twin), key_for(&h.cwd));
    assert_eq!(usage_in(Some(&h.claude()), None, &twin, "claude", 0), Usage::default());
}

#[test]
fn codex_usage_takes_the_last_totals_of_the_sessions_in_the_folder() {
    let h = home();
    let day = h.home.join(".codex/sessions/2020/01/01");
    let cwd = h.cwd.to_str().unwrap();
    let meta = |c: &str| format!(r#"{{"timestamp":"2020-01-01T09:00:00Z","type":"session_meta","payload":{{"id":"x","cwd":"{c}"}}}}"#);
    let tc = |ts: &str, i: u64, o: u64| format!(r#"{{"timestamp":"{ts}","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":{i},"cached_input_tokens":1,"output_tokens":{o},"total_tokens":{}}}}}}}}}"#, i + o);
    let none = r#"{"timestamp":"2020-01-01T10:00:30Z","type":"event_msg","payload":{"type":"token_count","info":null}}"#;
    lines(&day.join("rollout-2020-01-01T09-00-00-a.jsonl"), &[&meta(cwd), &tc("2020-01-01T09:59:00Z", 100, 10), &tc("2020-01-01T10:01:00Z", 400, 50), none, &tc("2020-01-01T10:02:00Z", 900, 120), "{broken"]);
    lines(&day.join("rollout-2020-01-01T11-00-00-b.jsonl"), &[&meta(cwd), &tc("2020-01-01T11:00:00Z", 30, 5)]);
    lines(&day.join("rollout-2020-01-01T12-00-00-other.jsonl"), &[&meta("/elsewhere"), &tc("2020-01-01T12:00:00Z", 5000, 5000)]);
    lines(&day.join("rollout-2020-01-01T13-00-00-nocwd.jsonl"), &[&tc("2020-01-01T13:00:00Z", 7000, 7000)]);
    lines(&day.join("notes.jsonl"), &[&meta(cwd), &tc("2020-01-01T13:00:00Z", 7000, 7000)]);
    lines(&day.join("rollout-2019-old.jsonl"), &[&meta(cwd), &tc("2019-01-01T13:00:00Z", 7000, 7000)]);
    old(&day.join("rollout-2019-old.jsonl"));
    // the first session counts from where `since` cut it: 900-100 and 120-10, plus the second one whole
    assert_eq!(h.get("codex", SINCE), Usage { tokens_in: Some(830), tokens_out: Some(115), usd: None });
    assert_eq!(h.get("codex", 0), Usage { tokens_in: Some(7930), tokens_out: Some(7125), usd: None }, "from the start: the last totals, the old file too");
    assert_eq!(h.get("codex", SINCE + 3 * 3_600_000), Usage::default());
}
