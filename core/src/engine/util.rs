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

/// What `sh` keeps of a command's output: a runaway check must not fill the memory. The end stays (the failure is there).
const OUT_MAX: usize = 8 << 20;
const OUT_CUT: &str = "[... the start of the output was cut ...]\n";

fn char_start(s: &str, mut i: usize) -> usize {
    while !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Runs `sh -c cmd` in `cwd` with stderr merged into stdout. None = cancelled (process group killed). Only the last
/// `OUT_MAX` bytes of the output are kept.
pub fn sh(cwd: &Path, cmd: &str, envs: &[(String, String)], cancel: &AtomicBool) -> Option<(bool, String)> {
    let mut c = Command::new("sh");
    c.arg("-c").arg(format!("exec 2>&1\n{cmd}")).current_dir(cwd).envs(envs.iter().map(|(k, v)| (k, v)));
    let mut out = String::new();
    let mut cut = false;
    match crate::agent::run_lines(c, cancel, &mut |l| {
        // a line of its own bigger than half the room: its end is what is kept
        let l = &l[char_start(&l, l.len().saturating_sub(OUT_MAX / 2))..];
        out.push_str(l);
        out.push('\n');
        if out.len() > OUT_MAX {
            // the older half goes, from a line start
            let from = char_start(&out, out.len() / 2);
            let from = out[from..].find('\n').map_or(from, |n| from + n + 1);
            out.drain(..from);
            cut = true;
        }
    }) {
        Ok(Some((ok, _))) => {
            if cut {
                out.insert_str(0, OUT_CUT);
            }
            Some((ok, out))
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_runaway_output_keeps_its_end_and_not_the_whole() {
        let c = AtomicBool::new(false);
        // 40 MB in lines, then the sign of the end; and 120 MB without one newline
        let (ok, out) = sh(Path::new("/tmp"), "yes 0123456789abcdef0123456789abcdef | head -c 40000000; echo; echo THE-END", &[], &c).unwrap();
        assert!(ok && out.len() <= OUT_MAX + OUT_CUT.len(), "{} bytes", out.len());
        assert!(out.starts_with(OUT_CUT) && out.ends_with("THE-END\n"));
        let (_, out) = sh(Path::new("/tmp"), "head -c 120000000 /dev/zero | tr '\\0' a; echo; echo AFTER", &[], &c).unwrap();
        assert!(out.len() <= OUT_MAX + OUT_CUT.len(), "{} bytes", out.len());
        assert!(out.ends_with("AFTER\n"));
        // a small output is as it was
        assert_eq!(sh(Path::new("/tmp"), "echo one; echo two", &[], &c).unwrap(), (true, "one\ntwo\n".to_string()));
    }
}
