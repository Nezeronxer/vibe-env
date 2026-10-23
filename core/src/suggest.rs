//! F14.2 - suggested tasks: a quick pass over the tracked files of a project for TODO / FIXME / XXX / HACK comments,
//! no model involved. Cards carry a fingerprint that survives line shifts, so "do not suggest this" sticks.
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Suggestion {
    /// `file::marker text`, lowercase and without the line number.
    pub fp: String,
    pub file: String,
    pub line: u32,
    pub marker: String,
    pub text: String,
    /// The task text for `submit_task`.
    pub prompt: String,
}

const MARKERS: [&str; 4] = ["TODO", "FIXME", "XXX", "HACK"];
const MAX_CARDS: usize = 50;
const MAX_TEXT: usize = 200;

/// Paths a pass never reads: other people's code, generated files, locks, secrets.
fn skip_path(p: &str) -> bool {
    let low = p.to_lowercase();
    let name = low.rsplit('/').next().unwrap_or(&low);
    ["vendor/", "node_modules/", "dist/", "build/", "target/", ".git/", "third_party/"].iter().any(|d| low.starts_with(d) || low.contains(&format!("/{d}")))
        || name.contains(".min.")
        || name.ends_with(".lock")
        || name.ends_with("-lock.json")
        || name.ends_with(".map")
        || name == ".env"
        || name.starts_with(".env.")
        || name.ends_with(".pem")
        || name.ends_with(".key")
}

/// `git grep -n` output (`file:line:text`) -> cards, minus the ones in `dismissed`. One card per fingerprint, at most `MAX_CARDS`.
pub fn parse_grep(out: &str, dismissed: &[String]) -> Vec<Suggestion> {
    let mut cards: Vec<Suggestion> = vec![];
    for l in out.lines() {
        let mut it = l.splitn(3, ':');
        let (Some(file), Some(no), Some(raw)) = (it.next(), it.next(), it.next()) else { continue };
        let Ok(line) = no.parse::<u32>() else { continue };
        if skip_path(file) {
            continue;
        }
        let Some((at, marker)) = MARKERS.iter().filter_map(|m| find_word(raw, m).map(|i| (i, *m))).min_by_key(|(i, _)| *i) else { continue };
        let mut rest = &raw[at + marker.len()..];
        // `TODO(maya): ...` - the owner is not part of the task
        if let Some(r) = rest.strip_prefix('(').and_then(|r| r.split_once(')')).filter(|(who, _)| who.len() <= 40) {
            rest = r.1;
        }
        let text: String = rest.trim_start_matches(|c: char| c == ':' || c == '(' || c == ')' || c == '-' || c.is_whitespace()).trim_end_matches(|c: char| c == '*' || c == '/' || c == '>' || c == '-' || c.is_whitespace()).chars().take(MAX_TEXT).collect();
        let fp = format!("{file}::{} {}", marker.to_lowercase(), text.to_lowercase());
        if dismissed.contains(&fp) || cards.iter().any(|c| c.fp == fp) {
            continue;
        }
        let prompt = if text.is_empty() {
            format!("Resolve the {marker} comment in `{file}` (line {line}). Keep the change small and run the project's tests.")
        } else {
            format!("Resolve the {marker} in `{file}` (line {line}): {text}\nKeep the change small and run the project's tests.")
        };
        cards.push(Suggestion { fp, file: file.into(), line, marker: marker.into(), text, prompt });
        if cards.len() == MAX_CARDS {
            break;
        }
    }
    cards
}

/// Byte offset of `word` as a whole word (no letter or digit on either side).
fn find_word(s: &str, word: &str) -> Option<usize> {
    let alnum = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    s.match_indices(word).map(|(i, _)| i).find(|&i| !alnum(s[..i].chars().next_back()) && !alnum(s[i + word.len()..].chars().next()))
}

/// The pass: `git grep` in the tracked files of `repo` (nothing else is read, nothing is changed).
pub fn scan(repo: &Path, dismissed: &[String]) -> Vec<Suggestion> {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo).args(["grep", "-n", "-I", "--no-color", "-w", "-E", &format!("({})", MARKERS.join("|"))]).stdin(Stdio::null()).stderr(Stdio::null());
    cmd.output().ok().map(|o| parse_grep(&String::from_utf8_lossy(&o.stdout), dismissed)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUT: &str = "src/a.rs:12:    // TODO: handle the empty case\n\
src/a.rs:40:    let x = 1; // FIXME(maya) wrong rounding */\n\
src/b.py:3:# HACK - works only on mac\n\
ui/vendor/xterm/x.js:1:// TODO: not ours\n\
Cargo.lock:9:XXX\n\
.env:1:TODO=secret\n\
src/c.rs:7:fn todo_list() {}\n\
src/c.rs:8:// TODOS are fine\n\
src/d.rs:5:// XXX\n\
broken line without colons\n";

    #[test]
    fn grep_lines_become_cards_with_clean_text() {
        let c = parse_grep(OUT, &[]);
        let got: Vec<(&str, u32, &str, &str)> = c.iter().map(|c| (c.file.as_str(), c.line, c.marker.as_str(), c.text.as_str())).collect();
        assert_eq!(got, vec![
            ("src/a.rs", 12, "TODO", "handle the empty case"),
            ("src/a.rs", 40, "FIXME", "wrong rounding"),
            ("src/b.py", 3, "HACK", "works only on mac"),
            ("src/d.rs", 5, "XXX", ""),
        ]);
        assert!(c[0].prompt.contains("`src/a.rs` (line 12): handle the empty case"));
        assert!(c[3].prompt.starts_with("Resolve the XXX comment in `src/d.rs` (line 5)"));
    }

    #[test]
    fn dismissed_fingerprints_stay_hidden_even_when_the_line_moves() {
        let first = parse_grep(OUT, &[]);
        let dismissed = vec![first[0].fp.clone()];
        let moved = OUT.replace("src/a.rs:12:", "src/a.rs:99:");
        let c = parse_grep(&moved, &dismissed);
        assert_eq!(c.len(), 3);
        assert!(c.iter().all(|c| c.fp != first[0].fp));
    }

    #[test]
    fn the_same_comment_twice_is_one_card_and_the_list_is_capped() {
        let two = "a.rs:1:// TODO: x\na.rs:9:// TODO: x\n";
        assert_eq!(parse_grep(two, &[]).len(), 1);
        let many: String = (0..80).map(|i| format!("f.rs:{}:// TODO: item {i}\n", i + 1)).collect();
        assert_eq!(parse_grep(&many, &[]).len(), MAX_CARDS);
    }

    #[test]
    fn scan_reads_tracked_files_only() {
        let d = std::env::temp_dir().join(format!("ns-suggest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let g = |a: &[&str]| assert!(Command::new("git").current_dir(&d).args(a).output().unwrap().status.success());
        g(&["init", "-q"]);
        std::fs::write(d.join("a.txt"), "x\n// TODO: tracked\n").unwrap();
        std::fs::write(d.join("untracked.txt"), "// TODO: not in git\n").unwrap();
        g(&["add", "a.txt"]);
        let c = scan(&d, &[]);
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].file.as_str(), c[0].line, c[0].text.as_str()), ("a.txt", 2, "tracked"));
    }
}
