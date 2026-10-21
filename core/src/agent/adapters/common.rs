//! Helpers shared by all adapters: PATH lookup, limit detection, shell quoting, fallbacks.
use crate::agent::AgentEvent;
use chrono::{DateTime, Duration, FixedOffset, Local, NaiveTime, TimeZone};
use regex::Regex;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Resolve `bin` in PATH plus the usual user install dirs (a GUI app starts with a thin PATH).
pub fn find_bin(bin: &str) -> Option<PathBuf> {
    if bin.contains('/') {
        return std::path::Path::new(bin).is_file().then(|| PathBuf::from(bin));
    }
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if let Some(h) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.extend([h.join(".local/bin"), h.join(".npm-global/bin"), h.join(".cargo/bin")]);
    }
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    dirs.into_iter().map(|d| d.join(bin)).find(|p| p.is_file())
}

pub fn check_installed(bin: &str, hint: &str) -> Result<(), String> {
    find_bin(bin).map(|_| ()).ok_or_else(|| format!("not installed: {hint}"))
}

/// Single-quote for `sh`.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Worker rules go to engines without a system-prompt flag as a prompt prefix.
pub fn with_system(system: &str, prompt: &str) -> String {
    if system.trim().is_empty() {
        prompt.to_string()
    } else {
        format!("{system}\n\n{prompt}")
    }
}

fn re(cell: &'static OnceLock<Regex>, p: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(p).unwrap())
}

/// Is this error text a subscription/rate limit message? Some(reset time if the text has one).
pub fn detect_limit(text: &str) -> Option<Option<DateTime<Local>>> {
    detect_limit_at(text, Local::now())
}

pub fn detect_limit_at(text: &str, now: DateTime<Local>) -> Option<Option<DateTime<Local>>> {
    static PHRASE: OnceLock<Regex> = OnceLock::new();
    let phrase = re(
        &PHRASE,
        r"(?i)usage limit|limit reached|reached your .{0,20}limit|hit your .{0,20}limit|rate.?limit(?:ed| exceeded| reached| hit)|(?:hit|exceeded|reached) .{0,12}rate.?limit|quota (?:exceeded|exhausted)|too many requests|resets? (?:at|in)\b|try again (?:at|in)\b",
    );
    if !phrase.is_match(text) {
        return None;
    }
    Some(reset_time(text, now))
}

fn reset_time(text: &str, now: DateTime<Local>) -> Option<DateTime<Local>> {
    static EPOCH: OnceLock<Regex> = OnceLock::new();
    static ISO: OnceLock<Regex> = OnceLock::new();
    static REL: OnceLock<Regex> = OnceLock::new();
    static UNIT: OnceLock<Regex> = OnceLock::new();
    static CLOCK: OnceLock<Regex> = OnceLock::new();
    // "usage limit reached|1760000000" (Claude Code legacy form)
    if let Some(c) = re(&EPOCH, r"\|(\d{10})\b").captures(text) {
        return Local.timestamp_opt(c[1].parse().ok()?, 0).single();
    }
    if let Some(m) = re(&ISO, r"\d{4}-\d{2}-\d{2}T[\d:.]+(?:Z|[+-]\d{2}:\d{2})").find(text) {
        if let Ok(t) = DateTime::parse_from_rfc3339(m.as_str()) {
            return Some(t.with_timezone(&Local));
        }
    }
    // "in 2 hours 30 minutes", "in 45 min"
    if let Some(c) = re(&REL, r"(?i)\bin\s+((?:\d+\s*(?:days?|hours?|hrs?|h|minutes?|mins?|m)\b[\s,]*(?:and\s+)?)+)").captures(text) {
        let mut secs = 0i64;
        for u in re(&UNIT, r"(?i)(\d+)\s*(days?|hours?|hrs?|h|minutes?|mins?|m)\b").captures_iter(&c[1]) {
            let n: i64 = u[1].parse().ok()?;
            secs += n * match u[2].to_lowercase().chars().next()? {
                'd' => 86400,
                'h' => 3600,
                _ => 60,
            };
        }
        return Some(now + Duration::seconds(secs));
    }
    // "resets 3pm (Europe/Moscow)", "resets at 14:00", "try again at 3:45 PM"
    let c = re(
        &CLOCK,
        r"(?i)(?:resets?|try again)\s*(?:at\s+)?(\d{1,2})(?::(\d{2}))?\s*(am|pm)?\b(?:\s*\(([A-Za-z_]+(?:/[A-Za-z0-9_+-]+)+)\))?",
    )
    .captures(text)?;
    if c.get(2).is_none() && c.get(3).is_none() {
        return None; // bare number such as "resets 3 days"
    }
    let mut h: u32 = c[1].parse().ok()?;
    let mi: u32 = c.get(2).map_or(Some(0), |m| m.as_str().parse().ok())?;
    match c.get(3).map(|m| m.as_str().to_lowercase()) {
        Some(ap) if (1..=12).contains(&h) => h = h % 12 + if ap == "pm" { 12 } else { 0 },
        Some(_) => return None,
        None => {}
    }
    let at = NaiveTime::from_hms_opt(h, mi, 0)?;
    // shortcut: the zone's UTC offset is read once via `date` (no chrono-tz), so a DST switch between now and the reset
    // is off by an hour; unknown zone name -> local time. Replace with chrono-tz if it joins the dependencies.
    let off = c.get(4).and_then(|z| zone_offset(z.as_str())).unwrap_or_else(|| *now.offset());
    let local_now = now.with_timezone(&off);
    let mut t = off.from_local_datetime(&local_now.date_naive().and_time(at)).earliest()?;
    if t <= local_now {
        t += Duration::days(1);
    }
    Some(t.with_timezone(&Local))
}

/// UTC offset of an IANA zone now, from the system tz database (`TZ=<zone> date +%z`).
fn zone_offset(zone: &str) -> Option<FixedOffset> {
    let out = std::process::Command::new("date").arg("+%z").env("TZ", zone).output().ok()?;
    let s = String::from_utf8(out.stdout).ok()?;
    let s = s.trim();
    if !out.status.success() || s.len() != 5 {
        return None;
    }
    let secs = s[1..3].parse::<i32>().ok()? * 3600 + s[3..5].parse::<i32>().ok()? * 60;
    FixedOffset::east_opt(if s.starts_with('-') { -secs } else { secs })
}

/// Last non-JSON lines of a failure log, one line.
pub fn tail(s: &str) -> String {
    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('{')).collect();
    let t = lines[lines.len().saturating_sub(5)..].join(" | ");
    if t.is_empty() {
        "engine failed".into()
    } else {
        t
    }
}

/// Done/Limit for a run that failed: a limit message becomes `Limit`.
pub fn failure_event(msg: &str) -> AgentEvent {
    match detect_limit(msg) {
        Some(resets_at) => AgentEvent::Limit { resets_at },
        None => AgentEvent::Done { session: None, text: msg.trim().to_string(), is_error: true },
    }
}

/// One-line summary of tool input: first known key, flattened and cut.
pub fn summarize(input: &Value) -> String {
    let v = ["file_path", "path", "command", "pattern", "url", "query", "description", "filePath"]
        .iter()
        .find_map(|k| input.get(k).and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| input.to_string());
    let one: String = v.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > 120 {
        format!("{}…", one.chars().take(120).collect::<String>())
    } else {
        one
    }
}

/// What a JSON line meant, for engines whose stream has no final text (Done is built at EOF).
#[derive(Default)]
pub struct Scan {
    pub events: Vec<AgentEvent>,
    pub session: Option<String>,
    /// Assistant text chunk; the final answer = chunks since the last tool call.
    pub text: Option<String>,
    pub error: Option<String>,
    /// The turn completed fine: forgets earlier non-fatal `error` events.
    pub ok: bool,
}

pub fn json_line(line: &str) -> Option<Value> {
    serde_json::from_str(line.trim()).ok().filter(Value::is_object)
}

/// EOF handler for engines scanned line by line: fold all stdout lines into one Done / Limit.
pub fn synth_done(scan: impl Fn(&str) -> Scan, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
    let (mut session, mut text, mut error) = (None, String::new(), None);
    for l in all_text.lines() {
        let s = scan(l);
        if s.session.is_some() {
            session = s.session;
        }
        if s.events.iter().any(|e| matches!(e, AgentEvent::Tool { .. })) {
            text.clear();
        }
        if let Some(t) = s.text {
            text.push_str(&t);
        }
        if s.ok {
            error = None;
        }
        if s.error.is_some() {
            error = s.error;
        }
    }
    if error.is_some() || !exit_ok {
        let msg = error.unwrap_or_else(|| tail(all_text));
        return vec![match failure_event(&msg) {
            AgentEvent::Done { text, is_error, .. } => AgentEvent::Done { session, text, is_error },
            limit => limit,
        }];
    }
    vec![AgentEvent::Done { session, text: text.trim().to_string(), is_error: false }]
}

/// EOF handler for engines that report their own result event: only covers a crash without one.
pub fn finish_without_result(exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
    if all_text.lines().filter_map(json_line).any(|j| j["type"] == "result") {
        return vec![];
    }
    let msg = if exit_ok { "engine ended without a result".to_string() } else { tail(all_text) };
    vec![failure_event(&msg)]
}
