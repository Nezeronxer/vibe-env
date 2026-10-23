//! Prompts, text parsing and small helpers used by the engine.
use chrono::{DateTime, Local, NaiveTime, Timelike};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;

pub fn title_of(prompt: &str) -> String {
    let first = prompt.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    first.chars().take(80).collect()
}

pub fn slug(title: &str) -> String {
    let mut s = String::new();
    for c in title.chars().map(|c| c.to_ascii_lowercase()) {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') {
            s.push('-');
        }
    }
    let s: String = s.trim_matches('-').chars().take(30).collect();
    let s = s.trim_end_matches('-').to_string();
    if s.is_empty() { "task".into() } else { s }
}

pub fn tail(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// Runs `sh -c cmd` in `cwd` with stderr merged into stdout. None = cancelled (process group killed).
pub fn sh(cwd: &Path, cmd: &str, envs: &[(String, String)], cancel: &AtomicBool) -> Option<(bool, String)> {
    let mut c = Command::new("sh");
    c.arg("-c").arg(format!("exec 2>&1\n{cmd}")).current_dir(cwd).envs(envs.iter().map(|(k, v)| (k, v)));
    let mut out = String::new();
    match crate::agent::run_lines(c, cancel, &mut |l| {
        out.push_str(&l);
        out.push('\n');
    }) {
        Ok(Some((ok, _))) => Some((ok, out)),
        Ok(None) => None,
        Err(e) => Some((false, format!("{cmd}: {e}"))),
    }
}

/// gitignore-like glob: no `/` = match the file name at any depth, otherwise the relative path.
fn glob(p: &str) -> Option<(bool, Regex)> {
    let p = p.trim();
    if p.is_empty() || p.starts_with('#') {
        return None; // shortcut: `!negation` is not supported; add it if .worktreeinclude files need it
    }
    let anchored = p.trim_end_matches('/').contains('/');
    let dir = p.ends_with('/');
    let p = p.trim_start_matches('/').trim_end_matches('/');
    let mut re = String::from("^");
    let mut it = p.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '*' if it.peek() == Some(&'*') => {
                it.next();
                re.push_str(".*");
            }
            '*' => re.push_str("[^/]*"),
            '?' => re.push_str("[^/]"),
            c => re.push_str(&regex::escape(&c.to_string())),
        }
    }
    re.push_str(if dir { "/.*$" } else { "$" });
    Regex::new(&re).ok().map(|r| (anchored || dir, r))
}

/// Copies files outside git that the task needs (`copy_files` + `.worktreeinclude`) from the
/// human's copy into the worktree, where it lacks them. Returns the copied relative paths;
/// the caller hides them from commits.
pub fn copy_files(repo: &Path, wt: &Path, patterns: &[String]) -> Vec<String> {
    let mut pats: Vec<String> = patterns.to_vec();
    if let Ok(s) = std::fs::read_to_string(repo.join(".worktreeinclude")) {
        pats.extend(s.lines().map(str::to_string));
    }
    let globs: Vec<(bool, Regex)> = pats.iter().filter_map(|p| glob(p)).collect();
    if globs.is_empty() {
        return vec![];
    }
    let mut copied = vec![];
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(repo.join(&rel)) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let r = rel.join(&name);
            let Ok(ft) = e.file_type() else { continue };
            // shortcut: heavy dirs are skipped outright; make it configurable if a project needs files there
            if ft.is_dir() {
                if ![".git", "node_modules", "target", ".venv"].contains(&name.as_str()) {
                    stack.push(r);
                }
                continue;
            }
            let rs = r.to_string_lossy().to_string();
            let hit = globs.iter().any(|(anch, re)| re.is_match(if *anch { &rs } else { &name }));
            if hit && !wt.join(&r).exists() {
                if let Some(parent) = wt.join(&r).parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if std::fs::copy(repo.join(&r), wt.join(&r)).is_ok() {
                    copied.push(rs);
                }
            }
        }
    }
    copied
}

/// "HH:MM" -> minutes since midnight.
fn minutes(hhmm: &str) -> Option<u32> {
    NaiveTime::parse_from_str(hhmm.trim(), "%H:%M").ok().map(|t| t.hour() * 60 + t.minute())
}

/// `now` is inside the daily window `from..to` ("23:00".."08:00" wraps midnight). An unreadable
/// bound or an empty window (`from == to`) means never.
pub fn in_window(now: DateTime<Local>, from: &str, to: &str) -> bool {
    let (Some(f), Some(t)) = (minutes(from), minutes(to)) else { return false };
    let m = now.hour() * 60 + now.minute();
    match f.cmp(&t) {
        std::cmp::Ordering::Equal => false,
        std::cmp::Ordering::Less => m >= f && m < t,
        std::cmp::Ordering::Greater => m >= f || m < t,
    }
}
