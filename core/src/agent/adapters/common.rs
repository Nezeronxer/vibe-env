//! Helpers shared by all adapters: PATH lookup, limit detection, shell quoting, fallbacks.
use crate::agent::AgentEvent;
use crate::model::ModelFamily;
use crate::models::version_cmp;
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
        // `.kimi-code/bin` and `.amp/bin`: where the Kimi Code and Amp install scripts put the binary
        dirs.extend([h.join(".local/bin"), h.join(".npm-global/bin"), h.join(".cargo/bin"), h.join(".kimi-code/bin"), h.join(".amp/bin")]);
    }
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    dirs.into_iter().map(|d| d.join(bin)).find(|p| p.is_file())
}

pub fn check_installed(bin: &str, hint: &str) -> Result<(), String> {
    find_bin(bin).map(|_| ()).ok_or_else(|| format!("not installed: {hint}"))
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
    // `date` says +0000 for a zone it does not know (a typo, no tzdata in a minimal system): the reset time went 3 h
    // off without a word; only a zone the database has counts, anything else is local time as documented above
    let known = std::env::var_os("TZDIR").map(std::path::PathBuf::from).into_iter().chain(["/usr/share/zoneinfo".into(), "/var/db/timezone/zoneinfo".into()])
        .any(|d: std::path::PathBuf| !zone.contains("..") && d.join(zone).is_file());
    if !known {
        return None;
    }
    let out = crate::proc::cmd("date").arg("+%z").env("TZ", zone).output().ok()?;
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

/// Phrases that only a CLI's own login / model complaint contains. Searched in the error event and
/// in the log / stderr tail. Real samples: Qwen (`No auth type is selected`), opencode
/// (`ProviderModelNotFoundError`), Claude (`issue with the selected model`); the rest is unverified.
/// From sources, not runs: Crush `no providers configured` (internal/cmd/run.go), Aider `no API keys were provided`
/// (aider/onboarding.py), Kimi Code `OAuth login expired` (docs changelog), goose `No api key passed in` (config-files.md).
fn strong_auth() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"(?i)not logged in|please (?:run )?[`'/]*(?:\w+ )?log ?in|log ?in required|login required|no auth type is selected|authentication (?:required|failed)|api key (?:not found|is missing|required|not set)|set an auth method|no providers configured|no api keys were provided|login expired|no api key passed in")
}
fn strong_model() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"(?i)ProviderModelNotFound|(?:issue|problem) with the selected model|(?:cannot|can't) use this model|model_not_found")
}
/// Common words that a tool's output may contain for other reasons ("unauthorized" in a test log):
/// looked for only in the CLI's own error message, never in the tail of the run's output.
fn wide_auth() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"(?i)unauthorized|not authenticated|invalid api key|incorrect api key|(?:missing|no) api key|credentials? (?:not found|missing|expired)")
}
fn wide_model() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"(?i)model not found|unknown model|invalid model|unsupported model|model .{0,80}(?:does not exist|not (?:found|available|supported|exist)|is not supported|may not exist|unavailable)|requested entity was not found")
}

/// The sentence around the first match in `context`, bounded by quotes, `|` and line ends
/// (log lines like `error="ProviderModelNotFoundError: ..." cause=...` give just the quoted value).
fn snippet(re: &Regex, context: &str) -> Option<String> {
    let m = re.find(context)?;
    let stop = |c: char| matches!(c, '"' | '\n' | '|');
    let start = context[..m.start()].rfind(stop).map_or(0, |i| i + 1);
    let end = context[m.end()..].find(stop).map_or(context.len(), |i| m.end() + i);
    Some(context[start..end].trim().to_string())
}

/// The event for a failed run: not logged in, model refused, limit, or a plain failed Done.
/// `msg` is the error text; `context` (stderr / log tail) is searched too, because a CLI may keep
/// the real reason out of its error event (opencode prints only "Unexpected server error").
pub fn classify_failure(msg: &str, context: &str) -> AgentEvent {
    let both = format!("{msg}\n{context}");
    let found = |strong: &Regex, wide: &Regex| -> Option<String> {
        if strong.is_match(msg) || wide.is_match(msg) {
            Some(msg.trim().to_string())
        } else {
            snippet(strong, context)
        }
    };
    if let Some(m) = found(strong_auth(), wide_auth()) {
        return AgentEvent::AuthRequired(m);
    }
    if let Some(m) = found(strong_model(), wide_model()) {
        return AgentEvent::ModelRejected(m);
    }
    match detect_limit(&both) {
        Some(resets_at) => AgentEvent::Limit { resets_at },
        None => AgentEvent::Done { session: None, text: msg.trim().to_string(), is_error: true },
    }
}

/// Session ids go into a terminal command: letters, digits and `._:-` only, 1..=128 chars,
/// no leading dash (it would read as a flag).
pub fn valid_session_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && !s.starts_with('-')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
}

/// Runs `bin args` and returns stdout; kills it after `timeout`. Err = text for the person.
pub fn capture(bin: &str, args: &[&str], timeout: std::time::Duration) -> Result<String, String> {
    use std::io::Read;
    use std::process::Stdio;
    let path = find_bin(bin).ok_or_else(|| format!("not installed: {bin}"))?;
    let mut child = crate::proc::cmd(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{bin}: {e}"))?;
    let (mut out, mut err) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
    let reader = std::thread::spawn(move || {
        let (mut o, mut e) = (String::new(), String::new());
        let _ = out.read_to_string(&mut o);
        let _ = err.read_to_string(&mut e);
        (o, e)
    });
    let start = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(std::time::Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{bin} {}: timed out", args.join(" ")));
            }
            Err(e) => return Err(format!("{bin}: {e}")),
        }
    };
    let (o, e) = reader.join().unwrap_or_default();
    if status.success() {
        Ok(o)
    } else {
        Err(format!("{bin} {}: {}", args.join(" "), tail(&e)))
    }
}

/// One built-in family; `versions` are `(version, exact CLI identifier)`, newest first. The description is
/// the interface's dictionary key `model.desc.<id>` (story 240); `source` says where it comes from, for the hint.
pub fn builtin_family(id: &str, label: &str, vendor: &str, source: &str, versions: &[(&str, &str)]) -> ModelFamily {
    ModelFamily {
        id: id.into(),
        label: label.into(),
        vendor: vendor.into(),
        blurb: Some(format!("model.desc.{id}")),
        source_note: Some(source.into()),
        versions: versions.iter().map(|(v, _)| v.to_string()).collect(),
        default: false,
        exact: versions.iter().map(|(v, e)| (v.to_string(), e.to_string())).collect(),
    }
}

/// A model id as a CLI prints it: no spaces, no leading dash.
pub fn is_model_id(s: &str) -> bool {
    !s.is_empty() && !s.starts_with('-') && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '/' | '@' | '+' | '-'))
}

/// The run's model for `-m`/`--model`, only when it is a model id: a catalog id like `--yolo` would be read as a flag.
pub fn cli_model(run: &crate::agent::Run) -> Option<&str> {
    run.model.as_deref().filter(|m| is_model_id(m))
}

pub fn strip_ansi(s: &str) -> String {
    static ANSI: OnceLock<Regex> = OnceLock::new();
    re(&ANSI, r"\x1b\[[0-9;]*[A-Za-z]").replace_all(s, "").into_owned()
}

/// Folds `(id, label, blurb, is_default)` rows of a CLI model list into families: `name-1.2`
/// becomes family `name` with version `1.2` (newest first), other ids stay a family of their own.
/// `ModelFamily.exact` keeps each version's id exactly as the CLI printed it.
pub fn group_models(rows: &[(String, Option<String>, Option<String>, bool)], vendor: impl Fn(&str) -> String) -> Vec<ModelFamily> {
    static VER: OnceLock<Regex> = OnceLock::new();
    let ver = re(&VER, r"^(.+?)-(\d+(?:\.\d+)*)$");
    let mut out: Vec<ModelFamily> = vec![];
    for (id, label, blurb, default) in rows {
        let (family, version) = match ver.captures(id) {
            Some(c) => (c[1].to_string(), Some(c[2].to_string())),
            None => (id.clone(), None),
        };
        let at = match out.iter().position(|f| f.id == family) {
            Some(i) => i,
            None => {
                out.push(ModelFamily {
                    id: family.clone(),
                    label: label.clone().unwrap_or_else(|| family.clone()),
                    vendor: vendor(&family),
                    blurb: blurb.clone(),
                    versions: vec![],
                    default: false,
                    exact: Default::default(),
                    source_note: None,
                });
                out.len() - 1
            }
        };
        let f = &mut out[at];
        f.default |= *default;
        if let Some(v) = version {
            if !f.versions.contains(&v) {
                f.versions.push(v.clone());
            }
            f.exact.insert(v, id.clone()); // the CLI's own id, as listed
        }
    }
    for f in &mut out {
        f.versions.sort_by(|a, b| version_cmp(b, a));
    }
    out
}

/// Summary of tool input: the first known key, else the whole input. Neither flattened nor cut here:
/// `actions::finish_action` scrubs first and makes it one short line after, so a cut never splits a secret.
pub fn summarize(input: &Value) -> String {
    ["file_path", "path", "command", "pattern", "url", "query", "description", "filePath"]
        .iter()
        .find_map(|k| input.get(k).and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| input.to_string())
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
        if s.events.iter().any(|e| matches!(e, AgentEvent::Tool { .. } | AgentEvent::Action(_))) {
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
        return vec![match classify_failure(&msg, &tail(all_text)) {
            AgentEvent::Done { text, is_error, .. } => AgentEvent::Done { session, text, is_error },
            other => other,
        }];
    }
    vec![AgentEvent::Done { session, text: text.trim().to_string(), is_error: false }]
}

/// A CLI without a JSON mode: each stdout line is a piece of text (colours stripped).
pub fn text_line(line: &str) -> Vec<AgentEvent> {
    let l = strip_ansi(line);
    if l.trim().is_empty() {
        vec![]
    } else {
        vec![AgentEvent::Text(l)]
    }
}

/// EOF of a plain-text CLI: the whole output is the answer; a failure is read from its last lines (with stderr).
pub fn text_done(exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
    let all = strip_ansi(all_text);
    if exit_ok {
        return vec![AgentEvent::Done { session: None, text: all.trim().to_string(), is_error: false }];
    }
    vec![classify_failure(&tail(&all), "")]
}

/// EOF handler for engines that report their own result event: only covers a crash without one.
pub fn finish_without_result(exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
    if all_text.lines().filter_map(json_line).any(|j| j["type"] == "result") {
        return vec![];
    }
    let msg = if exit_ok { "engine ended without a result".to_string() } else { tail(all_text) };
    vec![classify_failure(&msg, "")]
}

/// `<bin> --version`, the first line, at most 3 seconds; None when it cannot be read.
pub fn cli_version(bin: &str) -> Option<String> {
    let out = capture(bin, &["--version"], std::time::Duration::from_secs(3)).ok()?;
    out.lines().map(str::trim).find(|l| !l.is_empty()).map(|l| l.chars().take(60).collect())
}
