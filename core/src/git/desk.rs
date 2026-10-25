//! Terminal agents (task "agent = PTY terminal"): what a folder changed against HEAD, one file's diff, undoing one
//! file, and moving the base branch forward by the agent's own branch. Everything is by `cwd`, any folder of a git
//! repository (the app's own copies and the person's folders alike). Errors are short English keys for the window.
use super::*;
use serde::Serialize;

const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
/// The list of changes is cut here (`truncated`): a project that does not ignore `node_modules` must not flood the window.
const MAX_FILES: usize = 2000;
/// An untracked file bigger than this is listed without a line count.
const COUNT_MAX_BYTES: u64 = 1 << 20;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Change {
    /// Relative to the folder asked about.
    pub path: String,
    /// "M" | "A" | "D" | "R" | "?" (untracked).
    pub status: String,
    pub add: u32,
    pub del: u32,
}

/// The answer of `agent_changes`.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct AgentChanges {
    pub repo: bool,
    pub branch: Option<String>,
    pub base: Option<String>,
    pub own_worktree: bool,
    pub files: Vec<Change>,
    pub truncated: bool,
}

fn gf(e: GitError) -> String {
    format!("git_failed: {e}")
}

/// A folder inside a work tree: the real folder, the root of the work tree and the way from one to the other.
pub struct Ctx {
    pub cwd: PathBuf,
    pub root: PathBuf,
    /// "" or "sub/dir/" (cwd below root).
    pub prefix: String,
    /// What changes are counted from: HEAD, or for an agent's own copy where its branch left the base (`from`), so the
    /// list shows the commits the agent made itself too - all that a merge would take.
    since: String,
}

impl Ctx {
    /// None = not a folder or not inside a work tree.
    pub fn open(cwd: &Path) -> Option<Ctx> {
        let cwd = cwd.canonicalize().ok().filter(|c| c.is_dir())?;
        let o = run(&cwd, &["rev-parse", "--show-toplevel", "--show-prefix"]).ok().filter(|o| o.ok)?;
        let mut l = o.stdout.lines();
        let root = PathBuf::from(l.next()?).canonicalize().ok()?;
        let since = if has_commit(&root) { "HEAD" } else { EMPTY_TREE }.to_string();
        Some(Ctx { cwd, root, prefix: l.next().unwrap_or("").to_string(), since })
    }

    /// Counts changes from the commit where HEAD left `base` (unchanged when there is none).
    pub fn from(mut self, base: &str) -> Ctx {
        if let Some(mb) = try_out(&self.root, &["merge-base", base, "HEAD"]).filter(|_| self.since == "HEAD") {
            self.since = mb;
        }
        self
    }

    pub fn branch(&self) -> Option<String> {
        try_out(&self.root, &["symbolic-ref", "--short", "-q", "HEAD"])
    }

    /// The folder of the repository the work tree belongs to (the main copy for a linked work tree).
    pub fn repo_dir(&self) -> Option<PathBuf> {
        let common = try_out(&self.root, &["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
        let common = PathBuf::from(common).canonicalize().ok()?;
        (common.file_name()? == ".git").then(|| common.parent().map(Path::to_path_buf))?
    }

    /// Copied local files and the app's own folder: never listed, shown, reverted or committed.
    fn hidden(&self) -> Vec<String> {
        hidden_specs(&self.root).unwrap_or_default().iter().map(|s| s.trim_start_matches(":(literal)").to_string()).collect()
    }

    /// `file` (relative to the folder) as a path from the root of the work tree, after the checks every path passes:
    /// relative, no `..`, its real folder inside the asked folder, not a hidden file.
    fn rel(&self, file: &str) -> Result<String, String> {
        safe_rel(file).map_err(|_| "bad_path".to_string())?;
        inside(&self.cwd, &self.cwd.join(file), file).map_err(|_| "bad_path".to_string())?;
        let parts: Vec<String> = Path::new(file).components().filter_map(|c| match c {
            std::path::Component::Normal(n) => Some(n.to_string_lossy().into_owned()),
            _ => None,
        }).collect();
        let rel = format!("{}{}", self.prefix, parts.join("/"));
        if rel.is_empty() || is_hidden(&rel, &self.hidden()) {
            return Err("bad_path".into());
        }
        Ok(rel)
    }

    /// The working tree against `since` (HEAD unless `from`), untracked files included (ignored ones not), sorted by path.
    /// The bool says the list was cut at `MAX_FILES`.
    pub fn changes(&self) -> Result<(Vec<Change>, bool), String> {
        let st = run(&self.cwd, &["status", "--porcelain=v1", "-z", "-uall", "--", "."]).map_err(gf)?;
        if !st.ok {
            return Err(format!("git_failed: {}", st.stderr));
        }
        let stats = self.numstat()?;
        let hidden = self.hidden();
        let mut files = vec![];
        let mut truncated = false;
        // an agent's copy: tracked files come from the diff against `since` below, the status gives the untracked ones
        let forked = self.since != "HEAD" && self.since != EMPTY_TREE;
        let mut it = st.stdout.split('\0');
        while let Some(rec) = it.next() {
            let mut ch = rec.chars();
            let (Some(x), Some(y), Some(' ')) = (ch.next(), ch.next(), ch.next()) else { continue };
            let full = ch.as_str();
            if x == 'R' || x == 'C' || y == 'R' || y == 'C' {
                it.next(); // the old name
            }
            if is_hidden(full, &hidden) || (forked && (x, y) != ('?', '?')) {
                continue;
            }
            let Some(path) = full.strip_prefix(self.prefix.as_str()) else { continue };
            if files.len() >= MAX_FILES {
                truncated = true;
                break;
            }
            let status = code(x, y);
            let (add, del) = if status == "?" { count_lines(&self.cwd.join(path)) } else { stats.get(path).copied().unwrap_or((0, 0)) };
            files.push(Change { path: path.to_string(), status: status.to_string(), add, del });
        }
        if forked && !truncated {
            // the working tree against where the branch left the base: the agent's own commits and its edits on top
            let o = run(&self.cwd, &["diff", "--name-status", "-z", "-M", "--relative", "--no-ext-diff", &self.since, "--", "."]).map_err(gf)?;
            if !o.ok {
                return Err(format!("git_failed: {}", o.stderr));
            }
            let mut it = o.stdout.split('\0');
            while let Some(st) = it.next() {
                let Some(x) = st.chars().next() else { continue };
                if x == 'R' || x == 'C' {
                    it.next();
                }
                let Some(path) = it.next() else { break };
                if is_hidden(&format!("{}{path}", self.prefix), &hidden) {
                    continue;
                }
                if files.len() >= MAX_FILES {
                    truncated = true;
                    break;
                }
                let (add, del) = stats.get(path).copied().unwrap_or((0, 0));
                files.push(Change { path: path.to_string(), status: code(x, ' ').to_string(), add, del });
            }
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok((files, truncated))
    }

    /// Added / deleted lines of the tracked files against `since`, by path below the asked folder.
    fn numstat(&self) -> Result<std::collections::HashMap<String, (u32, u32)>, String> {
        let o = run(&self.cwd, &["diff", "--numstat", "-z", "-M", "--relative", "--no-ext-diff", "--no-textconv", &self.since, "--", "."]).map_err(gf)?;
        if !o.ok {
            return Err(format!("git_failed: {}", o.stderr));
        }
        let mut m = std::collections::HashMap::new();
        let mut it = o.stdout.split('\0');
        while let Some(t) = it.next() {
            let mut f = t.splitn(3, '\t');
            let (Some(a), Some(d), Some(p)) = (f.next(), f.next(), f.next()) else { continue };
            // a rename: the path field is empty and the old and the new name follow
            let path = if p.is_empty() {
                it.next();
                it.next().unwrap_or("")
            } else {
                p
            };
            m.insert(path.to_string(), (a.parse().unwrap_or(0), d.parse().unwrap_or(0)));
        }
        Ok(m)
    }

    /// Unified diff of one file against `since` (an untracked file as a new one), scrubbed, cut at the byte limit of
    /// `diff_file` (`true` then). A binary file gives git's one-line "Binary files differ".
    pub fn file_diff(&self, file: &str) -> Result<(String, bool), String> {
        self.rel(file)?;
        let spec = literal(file);
        let untracked = !run(&self.cwd, &["ls-files", "--others", "--exclude-standard", "--", &spec]).map_err(gf)?.stdout.is_empty();
        let (text, cut) = if untracked {
            stream_diff(&self.cwd, &["diff", "--no-index", "--no-color", "--no-ext-diff", "--no-textconv", "--", "/dev/null", file], usize::MAX)
        } else {
            stream_diff(&self.cwd, &["diff", "--no-color", "--no-ext-diff", "--no-textconv", &self.since, "--", &spec], usize::MAX)
        };
        let home = std::env::var("HOME").unwrap_or_default();
        Ok((crate::scrub::clean(&text, &home), cut))
    }

    /// Puts the file back as `since` has it (index and disk); a file it lacks is removed. A staged rename is undone
    /// with its old name too.
    pub fn revert_file(&self, file: &str) -> Result<(), String> {
        let rel = self.rel(file)?;
        let mut paths = vec![rel.clone()];
        if has_commit(&self.root) {
            if let Some(old) = self.rename_origin(&rel) {
                paths.push(old);
            }
        }
        restore_paths(&self.root, &self.since, &paths).map_err(gf)
    }

    /// The old name of `rel` when the index renames it (`git mv`).
    fn rename_origin(&self, rel: &str) -> Option<String> {
        let o = run(&self.root, &["diff", "--cached", "--name-status", "-z", "-M", "--no-ext-diff", "HEAD"]).ok().filter(|o| o.ok)?;
        let mut it = o.stdout.split('\0');
        while let Some(s) = it.next() {
            if s.starts_with('R') || s.starts_with('C') {
                let (old, new) = (it.next()?, it.next()?);
                if new == rel && s.starts_with('R') {
                    return Some(old.to_string());
                }
            } else {
                it.next();
            }
        }
        None
    }
}

/// Without case: on the case-insensitive disk of a Mac `.ENV` is the hidden `.env`.
fn is_hidden(rel: &str, hidden: &[String]) -> bool {
    let rel = rel.to_lowercase();
    hidden.iter().map(|h| h.to_lowercase()).any(|h| rel == h || rel.strip_prefix(h.as_str()).is_some_and(|r| r.starts_with('/')))
}

/// `git status` letters of one entry -> the one letter the window shows.
fn code(x: char, y: char) -> &'static str {
    match (x, y) {
        ('?', '?') => "?",
        _ if x == 'U' || y == 'U' || (x, y) == ('A', 'A') || (x, y) == ('D', 'D') => "M",
        _ if x == 'D' || y == 'D' => "D",
        _ if x == 'R' => "R",
        _ if x == 'A' || x == 'C' || y == 'A' => "A",
        _ => "M",
    }
}

/// Lines of a new text file; 0 for a binary one, a bigger-than-`COUNT_MAX_BYTES` one or one that cannot be read.
fn count_lines(p: &Path) -> (u32, u32) {
    let Ok(meta) = std::fs::symlink_metadata(p) else { return (0, 0) };
    if !meta.is_file() || meta.len() > COUNT_MAX_BYTES {
        return (0, 0);
    }
    let Ok(b) = std::fs::read(p) else { return (0, 0) };
    if b.iter().take(8000).any(|c| *c == 0) {
        return (0, 0);
    }
    let n = b.iter().filter(|c| **c == b'\n').count() + usize::from(b.last().is_some_and(|c| *c != b'\n'));
    (n as u32, 0)
}

/// Moves `base` forward to the tip of the work tree `wt`, fast-forward only, by the rules of the engine (`advance`: the
/// checkout of `base` must be clean, it is touched by `merge --ff-only` alone). Ok((true, new tip)); Ok((false, tip of
/// `base`)) when the branch has nothing `base` lacks. Err keys: `base_missing`, `not_fast_forward`, `base_dirty`.
pub fn fast_forward_base(repo: &Path, base: &str, wt: &Path) -> Result<(bool, String), String> {
    let old = try_out(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{base}^{{commit}}")]).ok_or("base_missing")?;
    let new = ok(wt, &["rev-parse", "--verify", "HEAD^{commit}"]).map_err(gf)?;
    let is_anc = |a: &str, b: &str| run(wt, &["merge-base", "--is-ancestor", a, b]).map(|o| o.ok);
    if is_anc(&new, &old).map_err(gf)? {
        return Ok((false, old));
    }
    if !is_anc(&old, &new).map_err(gf)? {
        return Err("not_fast_forward".into());
    }
    match advance(repo, base, &old, &new) {
        Ok(()) => Ok((true, new)),
        Err(GitError::AdvanceBlocked(m)) if m.ends_with("uncommitted changes") => Err("base_dirty".into()),
        Err(GitError::AdvanceBlocked(_)) | Err(GitError::BaseMoved(_)) => Err("not_fast_forward".into()),
        Err(e) => Err(gf(e)),
    }
}

/// Rebases the work tree's branch (an app branch, so history may be rewritten) onto `base`. A conflict is undone with
/// `rebase --abort` and comes back as `merge_conflict: <files>`.
pub fn rebase_onto_base(wt: &Path, base: &str) -> Result<(), String> {
    match rebase_onto(wt, base) {
        Ok(()) => Ok(()),
        Err(GitError::Conflict(_)) => {
            let files = run(wt, &["diff", "--name-only", "--diff-filter=U", "-z"]).map(|o| o.stdout).unwrap_or_default();
            let files: Vec<&str> = files.split('\0').filter(|f| !f.is_empty()).collect();
            let _ = run(wt, &["rebase", "--abort"]);
            Err(format!("merge_conflict: {}", files.join(", ")))
        }
        Err(e) => {
            if rebase_in_progress(wt) {
                let _ = run(wt, &["rebase", "--abort"]);
            }
            Err(gf(e))
        }
    }
}

/// The branch is fully in `base` (nothing would be lost by deleting it).
pub fn branch_merged(repo: &Path, base: &str, branch: &str) -> bool {
    is_merged(repo, base, branch)
}
