//! v3 (story 226): the edits of a run that its CLI did not show as text. Before a writing run the copy's
//! state is noted; after it every file that changed and has no diff in the run's own actions becomes an
//! `Edit` action of its own, with the diff of what the file was before the run and what it is now.
use crate::agent::actions::{finish_action, unified_diff};
use crate::agent::{ActionInput, Scrub, ToolAction};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::process::{Command, Stdio};

/// More changed files than this in one run are not shown one by one.
const MAX_FILES: usize = 200;
/// A file larger than this is not read for a diff.
const MAX_FILE: u64 = 4 * 1024 * 1024;

/// The copy before a run: its commit and the files that already differed from it.
pub(super) struct Before {
    head: String,
    dirty: HashMap<String, Option<String>>,
}

fn git(dir: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let o = Command::new("git").current_dir(dir).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    o.status.success().then_some(o.stdout)
}

/// Paths that differ from HEAD in the work tree, untracked ones too (`git status -z`).
fn dirty_paths(dir: &Path) -> Vec<String> {
    let Some(out) = git(dir, &["status", "--porcelain=v1", "-z", "--untracked-files=all"]) else { return vec![] };
    let text = String::from_utf8_lossy(&out);
    let mut parts = text.split('\0');
    let mut v = vec![];
    while let Some(p) = parts.next() {
        if p.len() < 4 {
            continue;
        }
        if p.starts_with('R') || p.starts_with('C') {
            parts.next(); // the old name of a rename
        }
        v.push(p[3..].to_string());
    }
    v
}

fn read_text(path: &Path) -> Option<String> {
    let m = std::fs::metadata(path).ok()?;
    if !m.is_file() || m.len() > MAX_FILE {
        return None;
    }
    String::from_utf8(std::fs::read(path).ok()?).ok()
}

/// None when `dir` is not a git work tree (nothing is shown then).
pub(super) fn before(dir: &Path) -> Option<Before> {
    let head = String::from_utf8(git(dir, &["rev-parse", "HEAD"])?).ok()?.trim().to_string();
    let dirty = dirty_paths(dir).into_iter().map(|p| (p.clone(), read_text(&dir.join(&p)))).collect();
    Some(Before { head, dirty })
}

/// A path of an action relative to the copy (CLIs give absolute paths).
pub(super) fn rel_path(dir: &Path, p: &str) -> String {
    let path = Path::new(p);
    for base in [dir.to_path_buf(), dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf())] {
        if let Ok(r) = path.strip_prefix(&base) {
            return r.to_string_lossy().into_owned();
        }
    }
    if let Ok(c) = path.canonicalize() {
        if let Ok(r) = c.strip_prefix(dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf())) {
            return r.to_string_lossy().into_owned();
        }
    }
    p.trim_start_matches("./").to_string()
}

/// The `Edit` actions of the files the run changed, leaving out `shown` (relative paths that already have a
/// diff or a new-file view in the run's actions). Scrubbed and cut like every action.
pub(super) fn after(dir: &Path, b: &Before, shown: &BTreeSet<String>, scrub: &Scrub) -> Vec<ToolAction> {
    let mut paths: BTreeSet<String> = dirty_paths(dir).into_iter().collect();
    paths.extend(b.dirty.keys().cloned());
    if let Some(out) = git(dir, &["diff", "--name-only", "-z", &b.head, "HEAD"]) {
        paths.extend(String::from_utf8_lossy(&out).split('\0').filter(|p| !p.is_empty()).map(str::to_string));
    }
    let mut v = vec![];
    for p in paths.into_iter().filter(|p| !shown.contains(p) && !crate::kits::is_secret_file(p)).take(MAX_FILES) {
        let old = match b.dirty.get(&p) {
            Some(c) => c.clone(),
            None => git(dir, &["show", &format!("{}:{p}", b.head)]).and_then(|o| String::from_utf8(o).ok()),
        };
        let new = read_text(&dir.join(&p));
        if old == new {
            continue;
        }
        let diff = unified_diff(&p, old.as_deref(), new.as_deref());
        if diff.is_empty() {
            continue;
        }
        let mut a = ToolAction {
            id: format!("diff-{}", uuid::Uuid::new_v4().simple()),
            name: "Edit".into(),
            input: ActionInput::Edit { path: p, diff },
            output: None,
            ok: true,
            done: true,
        };
        finish_action(&mut a, scrub);
        v.push(a);
    }
    v
}
