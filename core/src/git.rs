//! All git / gh operations. System binaries via `Command`, no libgit2.
//! Safety rules: never `push --force`, never `reset --hard`, only `ns/*` branches are deleted,
//! the human's working copy is only touched by `merge --ff-only` on a clean base branch.
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub mod desk;

#[derive(Debug, Clone, PartialEq)]
pub struct RepoInfo {
    pub is_repo: bool,
    pub base_branch: Option<String>,
    pub current_branch: Option<String>,
    /// Tracked files modified (staged or not). Untracked files do not count.
    pub dirty: bool,
    pub origin_url: Option<String>,
    /// https link of origin, credentials stripped (None for local paths).
    pub web_url: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct FileStat {
    pub path: String,
    pub insertions: u32,
    pub deletions: u32,
}
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GitError {
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("advance blocked: {0}")]
    AdvanceBlocked(String),
    /// Base got new commits after the rebase (or while advancing it): rebase again.
    #[error("base moved: {0}")]
    BaseMoved(String),
    #[error("push failed: {0}")]
    Push(String),
    #[error("gh: {0}")]
    Gh(String),
    #[error("git: {0}")]
    Other(String),
}

const NS_PREFIX: &str = "ns/";
const MARKER_NS: &str = "refs/ns-merged/";

static SIGNED: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

/// `-c` args giving only the missing half of the signature (name and/or email) a Nightshift
/// fallback; a configured value is never overridden. Dirs (repo / worktree roots) that
/// already have both are remembered; incomplete ones are re-checked so later config is seen.
fn identity_args(dir: &Path) -> Vec<String> {
    if SIGNED.lock().unwrap().iter().any(|d| d == dir) {
        return vec![];
    }
    let out = Command::new("git")
        .current_dir(dir)
        .args(["config", "--get-regexp", r"^user\.(name|email)$"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let has = |k: &str| {
        out.lines().any(|l| l.split_once(' ').is_some_and(|(key, v)| key == k && !v.trim().is_empty()))
    };
    let mut args = vec![];
    if !has("user.name") {
        args.extend(["-c".to_string(), "user.name=Nightshift".to_string()]);
    }
    if !has("user.email") {
        args.extend(["-c".to_string(), "user.email=nightshift@localhost".to_string()]);
    }
    if args.is_empty() {
        SIGNED.lock().unwrap().push(dir.to_path_buf());
    }
    args
}

fn git_cmd(dir: &Path) -> Command {
    let mut c = Command::new("git");
    c.current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_EDITOR", "true")
        .env("LC_ALL", "C")
        // a repository's own config must not run a program on a read (fsmonitor of an unpacked or agent-written .git)
        .args(["-c", "core.fsmonitor=false"])
        .args(identity_args(dir));
    c
}

struct Out {
    ok: bool,
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run(dir: &Path, args: &[&str]) -> Result<Out, GitError> {
    let o: Output = git_cmd(dir)
        .args(args)
        .output()
        .map_err(|e| GitError::Other(format!("git {}: {e}", args.join(" "))))?;
    Ok(Out {
        ok: o.status.success(),
        code: o.status.code(),
        stdout: String::from_utf8_lossy(&o.stdout).trim_end().to_string(),
        stderr: String::from_utf8_lossy(&o.stderr).trim().to_string(),
    })
}

/// Run and require success; error text carries the command and its stderr.
fn ok(dir: &Path, args: &[&str]) -> Result<String, GitError> {
    let o = run(dir, args)?;
    if o.ok {
        Ok(o.stdout)
    } else {
        Err(GitError::Other(format!("git {}: {}", args.join(" "), o.stderr)))
    }
}

fn try_out(dir: &Path, args: &[&str]) -> Option<String> {
    run(dir, args).ok().filter(|o| o.ok && !o.stdout.is_empty()).map(|o| o.stdout)
}

fn branch_exists(repo: &Path, b: &str) -> bool {
    run(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{b}")]).map(|o| o.ok).unwrap_or(false)
}

fn has_commit(repo: &Path) -> bool {
    run(repo, &["rev-parse", "--verify", "--quiet", "HEAD"]).map(|o| o.ok).unwrap_or(false)
}

pub fn repo_info(repo: &Path) -> RepoInfo {
    let mut info = RepoInfo {
        is_repo: false, base_branch: None, current_branch: None, dirty: false,
        origin_url: None, web_url: None, ahead: 0, behind: 0,
    };
    if !repo.is_dir() {
        return info;
    }
    info.is_repo = try_out(repo, &["rev-parse", "--is-inside-work-tree"]).as_deref() == Some("true");
    if !info.is_repo {
        return info;
    }
    info.current_branch = try_out(repo, &["symbolic-ref", "--short", "-q", "HEAD"]);
    info.origin_url = try_out(repo, &["remote", "get-url", "origin"]);
    info.web_url = info.origin_url.as_deref().and_then(web_url);
    info.base_branch = try_out(repo, &["symbolic-ref", "--short", "-q", "refs/remotes/origin/HEAD"])
        .and_then(|s| s.strip_prefix("origin/").map(str::to_string))
        .filter(|b| branch_exists(repo, b))
        .or_else(|| ["main", "master"].iter().find(|b| branch_exists(repo, b)).map(|b| b.to_string()))
        .or_else(|| info.current_branch.clone());
    info.dirty = is_dirty(repo);
    if let (Some(b), Some(_)) = (&info.base_branch, &info.origin_url) {
        let spec = format!("{b}...origin/{b}");
        if let Some(s) = try_out(repo, &["rev-list", "--left-right", "--count", &spec]) {
            let mut it = s.split_whitespace().filter_map(|x| x.parse::<u32>().ok());
            info.ahead = it.next().unwrap_or(0);
            info.behind = it.next().unwrap_or(0);
        }
    }
    info
}

fn web_url(u: &str) -> Option<String> {
    let u = u.trim();
    let (host, path) = if let Some((scheme, rest)) = u.split_once("://") {
        let (auth, path) = rest.split_once('/')?;
        let host = auth.rsplit('@').next()?;
        // an ssh/git port is not a web port
        let host = if scheme.starts_with("http") { host } else { host.split(':').next()? };
        (host, path)
    } else {
        let (left, path) = u.split_once(':')?;
        (left.split_once('@')?.1, path)
    };
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some(format!("https://{host}/{path}"))
}

fn is_dirty(repo: &Path) -> bool {
    run(repo, &["status", "--porcelain", "--untracked-files=no"]).map(|o| !o.stdout.is_empty()).unwrap_or(false)
}

/// `git init` + initial commit. Existing files are committed except `.env*`.
pub fn init_repo(path: &Path) -> Result<(), GitError> {
    std::fs::create_dir_all(path).map_err(|e| GitError::Other(e.to_string()))?;
    if !repo_info(path).is_repo {
        ok(path, &["init", "-b", "main"])?;
    }
    if has_commit(path) {
        return Ok(());
    }
    ok(path, &["add", "-A", "--", ".", ":(exclude,glob)**/.env*"])?;
    ok(path, &["commit", "--allow-empty", "-q", "-m", "Initial commit"])?;
    Ok(())
}

pub fn create_worktree(repo: &Path, worktrees_dir: &Path, task_id: &str, branch: &str, base: &str) -> Result<PathBuf, GitError> {
    if !branch.starts_with(NS_PREFIX) {
        return Err(GitError::Other(format!("branch must start with {NS_PREFIX}: {branch}")));
    }
    std::fs::create_dir_all(worktrees_dir).map_err(|e| GitError::Other(e.to_string()))?;
    let wt = worktrees_dir.join(task_id);
    let wts = wt.to_string_lossy().to_string();
    if branch_exists(repo, branch) {
        ok(repo, &["worktree", "add", &wts, branch])?;
    } else {
        ok(repo, &["worktree", "add", "-b", branch, &wts, base])?;
    }
    Ok(wt)
}

/// Removes the worktree (it is the app's own, so `--force` is fine). The branch is deleted only
/// when `delete_branch` is set and it is an `ns/*` branch.
pub fn remove_worktree(repo: &Path, wt: &Path, delete_branch: bool) -> Result<(), GitError> {
    remove_wt(repo, wt, delete_branch, true)
}

/// `remove_worktree` without `--force`, for a copy the person asked to remove: the files the app copied in (`.env*`,
/// listed in `ns-hidden`) go first, then git itself refuses a copy that still holds modified or untracked work - a
/// file written after the app's own check (a process of the agent that outlived its tab) is not deleted with it.
pub fn remove_worktree_careful(repo: &Path, wt: &Path, delete_branch: bool) -> Result<(), GitError> {
    if wt.exists() {
        if let Some(list) = hidden_list(wt) {
            let root = wt.canonicalize().map_err(|e| GitError::Other(format!("{}: {e}", wt.display())))?;
            for p in std::fs::read_to_string(&list).unwrap_or_default().lines().filter(|l| !l.is_empty()) {
                // only an untracked copy the app made, inside the copy
                let full = wt.join(p);
                let tracked = run(wt, &["ls-files", "--error-unmatch", "--", &literal(p)]).map(|o| o.ok).unwrap_or(true);
                if safe_rel(p).is_ok() && inside(&root, &full, p).is_ok() && !tracked && full.is_file() {
                    let _ = std::fs::remove_file(&full);
                }
            }
        }
    }
    remove_wt(repo, wt, delete_branch, false)
}

fn remove_wt(repo: &Path, wt: &Path, delete_branch: bool, force: bool) -> Result<(), GitError> {
    let branch = if wt.exists() { try_out(wt, &["symbolic-ref", "--short", "-q", "HEAD"]) } else { None };
    if wt.exists() {
        let path = wt.to_string_lossy();
        let args: Vec<&str> = if force { vec!["worktree", "remove", "--force", &path] } else { vec!["worktree", "remove", &path] };
        let o = run(repo, &args)?;
        if !o.ok {
            return Err(if o.stderr.contains("modified or untracked") { GitError::AdvanceBlocked("uncommitted changes in the copy".into()) } else { GitError::Other(format!("git worktree remove: {}", o.stderr)) });
        }
    }
    let _ = run(repo, &["worktree", "prune"]);
    // the snapshots of a task go with its copy (the copy's folder is named by the task id)
    if let Some(id) = wt.file_name().and_then(|n| n.to_str()) {
        drop_snapshots(repo, id);
    }
    if delete_branch {
        if let Some(b) = branch.filter(|b| b.starts_with(NS_PREFIX)) {
            ok(repo, &["branch", "-D", &b])?;
        }
    }
    Ok(())
}

/// File in the worktree's private git dir listing untracked paths `add -A` must skip.
fn hidden_list(wt: &Path) -> Option<PathBuf> {
    let gd = PathBuf::from(try_out(wt, &["rev-parse", "--git-dir"])?);
    Some(if gd.is_absolute() { gd } else { wt.join(gd) }.join("ns-hidden"))
}

/// Keeps files copied into the worktree (`.env*` etc.) out of every commit the app makes there,
/// whatever `.gitignore` says now or later, without touching the human's repo (its
/// `info/exclude` is shared by all worktrees).
pub fn hide_untracked(wt: &Path, paths: &[String]) -> Result<(), GitError> {
    let f = hidden_list(wt).ok_or_else(|| GitError::Other(format!("not a worktree: {}", wt.display())))?;
    let mut all = std::fs::read_to_string(&f).unwrap_or_default();
    for p in paths {
        all.push_str(p);
        all.push('\n');
    }
    std::fs::write(&f, all).map_err(|e| GitError::Other(format!("{}: {e}", f.display())))
}

/// `:(literal)p ...` pathspecs of the hidden paths; empty when there are none. A list that does not
/// exist (`NotFound`) means none are hidden; a list that exists but cannot be read is an error, and
/// whoever commits stops: committing without knowing what to leave out could publish a `.env` (R11.9).
fn hidden_specs(wt: &Path) -> Result<Vec<String>, GitError> {
    let Some(f) = hidden_list(wt) else { return Ok(vec![]) };
    let hidden = match std::fs::read_to_string(&f) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(GitError::Other(format!("hidden_list_unreadable: {}: {e}", f.display()))),
    };
    Ok(hidden.lines().filter(|l| !l.is_empty()).map(|l| format!(":(literal){l}")).collect())
}

fn run_with(wt: &Path, head: &[&str], specs: &[String]) -> Result<Out, GitError> {
    let mut args: Vec<&str> = head.to_vec();
    args.push("--");
    args.extend(specs.iter().map(String::as_str));
    run(wt, &args)
}

/// `git add -A`, then the hidden paths are put back as HEAD has them (out of the index if HEAD
/// lacks them), and with `protect` ("Защищённые пути", from the settings: nothing in the copy turns it on or off)
/// so are new key and `.env` files (`protected_staged`). Naming ignored paths here is fine, unlike in an `add` pathspec.
fn add_all(wt: &Path, protect: bool) -> Result<(), GitError> {
    let mut specs = hidden_specs(wt)?; // before anything is staged: an unreadable list stops the commit
    ok(wt, &["add", "-A"])?;
    let keys = if protect { protected_staged(wt)? } else { vec![] };
    specs.extend(keys.iter().map(|p| format!(":(literal){p}")));
    if !specs.is_empty() {
        let o = run_with(wt, &["reset", "-q"], &specs)?;
        if !o.ok {
            return Err(GitError::Other(format!("git reset hidden paths: {}", o.stderr)));
        }
    }
    Ok(())
}

/// A key, a `.env` or a credentials file (`kits::is_secret_file`); templates people commit on purpose
/// (`.env.example`, `.env.sample`, `.env.template`) are not.
fn secret_path(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    crate::kits::is_secret_file(rel) && ![".env.example", ".env.sample", ".env.template"].contains(&name)
}

fn nul_list(s: &str) -> Vec<String> {
    let mut v: Vec<String> = s.split('\0').map(|x| x.trim_matches('\n')).filter(|x| !x.is_empty() && secret_path(x)).map(str::to_string).collect();
    v.sort();
    v.dedup();
    v
}

/// Secret-like files that `add -A` just staged as new. Files git already tracks stay staged: put back out of the index
/// they would leave the copy dirty and stop the rebase; `protected_committed` refuses the merge for them instead.
fn protected_staged(wt: &Path) -> Result<Vec<String>, GitError> {
    Ok(nul_list(&ok(wt, &["diff", "--cached", "--name-only", "--no-renames", "--diff-filter=A", "-z"])?))
}

/// New secret-like files in `wt` that the next protected commit leaves out (hidden copied files are not listed, they
/// never go anyway): for the feed.
pub fn protected_pending(wt: &Path) -> Vec<String> {
    let hidden: Vec<String> = hidden_specs(wt).unwrap_or_default().iter().map(|s| s.trim_start_matches(":(literal)").to_string()).collect();
    let out = run(wt, &["ls-files", "--others", "--exclude-standard", "-z"]).map(|o| o.stdout).unwrap_or_default();
    nul_list(&out).into_iter().filter(|p| !hidden.contains(p)).collect()
}

/// Secret-like files a commit of the branch since it forked from `base` added or changed (a deletion is no leak).
pub fn protected_committed(wt: &Path, base: &str) -> Result<Vec<String>, GitError> {
    let range = format!("{}..HEAD", merge_base(wt, base));
    Ok(nul_list(&ok(wt, &["log", "--format=", "--name-only", "--no-renames", "--diff-filter=d", "-z", &range])?))
}

/// Hidden paths that a commit of the branch since it forked from `base` added or changed. A list that
/// cannot be read is an error (see `hidden_specs`).
pub fn try_hidden_committed(wt: &Path, base: &str) -> Result<Vec<String>, GitError> {
    let specs = hidden_specs(wt)?;
    if specs.is_empty() {
        return Ok(vec![]);
    }
    let range = format!("{}..HEAD", merge_base(wt, base));
    let out = run_with(wt, &["log", "--format=", "--name-only", &range], &specs).map(|o| o.stdout).unwrap_or_default();
    let mut v: Vec<String> = out.lines().filter(|l| !l.is_empty()).map(str::to_string).collect();
    v.sort();
    v.dedup();
    Ok(v)
}

/// `try_hidden_committed` that reads an unreadable list as "none": for callers that cannot fail and
/// have already been through `commit_all`, which does not tolerate one.
pub fn hidden_committed(wt: &Path, base: &str) -> Vec<String> {
    try_hidden_committed(wt, base).unwrap_or_default()
}

/// Someone (the agent) committed a hidden path or a protected one (`protected_committed`): fold the branch back into the index
/// (`reset --soft` to the fork point, files stay) so the next `commit_all` leaves it out and
/// no commit of the branch keeps it. Ok(true) when it did that.
pub fn uncommit_hidden(wt: &Path, base: &str, protect: bool) -> Result<bool, GitError> {
    if try_hidden_committed(wt, base)?.is_empty() && (!protect || protected_committed(wt, base)?.is_empty()) {
        return Ok(false);
    }
    ok(wt, &["reset", "-q", "--soft", &merge_base(wt, base)])?;
    Ok(true)
}

pub fn commit_all(wt: &Path, msg: &str, protect: bool) -> Result<Option<String>, GitError> {
    add_all(wt, protect)?;
    if run(wt, &["diff", "--cached", "--quiet"])?.ok {
        return Ok(None);
    }
    ok(wt, &["commit", "-q", "-m", msg])?;
    Ok(Some(ok(wt, &["rev-parse", "HEAD"])?))
}

/// A rebase stopped in `wt` (conflict waiting to be resolved).
pub fn rebase_in_progress(wt: &Path) -> bool {
    ["rebase-merge", "rebase-apply"].iter().any(|d| {
        try_out(wt, &["rev-parse", "--git-path", d])
            .map(|p| {
                let p = PathBuf::from(p);
                if p.is_absolute() { p } else { wt.join(p) }.exists()
            })
            .unwrap_or(false)
    })
}

pub fn rebase_onto(wt: &Path, base: &str) -> Result<(), GitError> {
    let o = run(wt, &["rebase", base])?;
    if o.ok {
        Ok(())
    } else if rebase_in_progress(wt) {
        Err(GitError::Conflict(format!("{}\n{}", o.stdout, o.stderr).trim().to_string()))
    } else {
        Err(GitError::Other(format!("git rebase {base}: {}", o.stderr)))
    }
}

pub fn rebase_continue(wt: &Path, protect: bool) -> Result<(), GitError> {
    add_all(wt, protect)?;
    let o = run(wt, &["rebase", "--continue"])?;
    if o.ok {
        Ok(())
    } else if rebase_in_progress(wt) {
        Err(GitError::Conflict(format!("{}\n{}", o.stdout, o.stderr).trim().to_string()))
    } else {
        Err(GitError::Other(format!("git rebase --continue: {}", o.stderr)))
    }
}

/// The messages of the commits made in `wt` since it forked from `base`, oldest first, joined by a blank line
/// (a fold of them by `uncommit_hidden` keeps what the agent wrote, not the task text).
pub fn own_messages(wt: &Path, base: &str) -> String {
    let range = format!("{}..HEAD", merge_base(wt, base));
    try_out(wt, &["log", "--reverse", "--format=%B%x00", &range])
        .map(|o| o.split('\0').map(str::trim).filter(|m| !m.is_empty()).collect::<Vec<_>>().join("\n\n"))
        .unwrap_or_default()
}

fn merge_base(wt: &Path, base: &str) -> String {
    try_out(wt, &["merge-base", base, "HEAD"]).unwrap_or_else(|| base.to_string())
}

/// Changes of the task relative to where it forked from `base`, truncated to `max_bytes`.
pub fn diff(wt: &Path, base: &str, max_bytes: usize) -> String {
    let mb = merge_base(wt, base);
    let mut s = run(wt, &["diff", &mb]).map(|o| o.stdout).unwrap_or_default();
    if s.len() > max_bytes {
        let mut cut = max_bytes;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
        s.push_str("\n... [diff truncated]");
    }
    s
}

pub fn diffstat(wt: &Path, base: &str) -> Vec<FileStat> {
    let mb = merge_base(wt, base);
    let out = run(wt, &["diff", "--numstat", &mb]).map(|o| o.stdout).unwrap_or_default();
    out.lines()
        .filter_map(|l| {
            let mut p = l.splitn(3, '\t');
            let (a, d, path) = (p.next()?, p.next()?, p.next()?);
            Some(FileStat { path: path.to_string(), insertions: a.parse().unwrap_or(0), deletions: d.parse().unwrap_or(0) })
        })
        .collect()
}

/// Directory where `base` is checked out (main copy or a linked worktree), if any.
fn checkout_of(repo: &Path, base: &str) -> Option<PathBuf> {
    let out = run(repo, &["worktree", "list", "--porcelain"]).ok()?.stdout;
    let want = format!("branch refs/heads/{base}");
    out.split("\n\n").find(|blk| blk.lines().any(|l| l == want)).and_then(|blk| {
        blk.lines().find_map(|l| l.strip_prefix("worktree ")).map(PathBuf::from)
    })
}

/// Move `base` from `old` to `new` (fast-forward only) without ever clobbering the human's copy.
/// 1. `base` is checked out (main copy or linked worktree) and clean -> `merge --ff-only` there
/// 2. checked out and dirty -> AdvanceBlocked (nothing is touched)
/// 3. checked out nowhere   -> `update-ref` guarded by the old value
fn advance(repo: &Path, base: &str, old: &str, new: &str) -> Result<(), GitError> {
    let r = format!("refs/heads/{base}");
    let moved = || GitError::BaseMoved(format!("{base} moved while advancing it"));
    let tip_is_old = || try_out(repo, &["rev-parse", &r]).as_deref() == Some(old);
    if !tip_is_old() {
        return Err(moved());
    }
    if let Some(dir) = checkout_of(repo, base) {
        if is_dirty(&dir) {
            return Err(GitError::AdvanceBlocked(format!("{base} has uncommitted changes")));
        }
        // a file the merge adds where the person keeps an untracked or ignored one of their own (a local.json the agent
        // un-ignored): `merge --ff-only` overwrites ignored files without a word, so it is refused here, nothing touched
        let added = run(&dir, &["diff", "-z", "--name-only", "--no-renames", "--diff-filter=A", old, new]).map(|o| o.stdout).unwrap_or_default();
        let taken: Vec<&str> = added.split('\0').filter(|p| !p.is_empty() && dir.join(p).symlink_metadata().is_ok()).collect();
        if !taken.is_empty() {
            return Err(GitError::AdvanceBlocked(format!("untracked files would be overwritten: {}", taken.join(", "))));
        }
        let o = run(&dir, &["merge", "--ff-only", "-q", new])?;
        if !o.ok {
            return Err(if tip_is_old() {
                GitError::AdvanceBlocked(format!("merge --ff-only refused: {}", o.stderr))
            } else {
                moved()
            });
        }
    } else {
        // a rebase of the base going on in the main copy (HEAD detached, so `base` looks checked out nowhere): moving the
        // ref under it left the person's rebase unable to finish
        if rebase_in_progress(repo) {
            return Err(GitError::AdvanceBlocked(format!("a rebase is in progress in {}", repo.display())));
        }
        let o = run(repo, &["update-ref", &r, new, old])?;
        if !o.ok {
            return Err(if tip_is_old() { GitError::Other(format!("update-ref {r}: {}", o.stderr)) } else { moved() });
        }
    }
    Ok(())
}

/// One commit on `base` carrying the tree of the (already rebased) worktree branch.
pub fn squash_to_base(repo: &Path, wt: &Path, base: &str, msg: &str) -> Result<String, GitError> {
    let old = ok(repo, &["rev-parse", &format!("refs/heads/{base}")])?;
    if !run(wt, &["merge-base", "--is-ancestor", &old, "HEAD"])?.ok {
        return Err(GitError::BaseMoved(format!("{base} moved since rebase")));
    }
    let tree = ok(wt, &["rev-parse", "HEAD^{tree}"])?;
    let new = ok(repo, &["commit-tree", &tree, "-p", &old, "-m", msg])?;
    // Squashed branches are not ancestors of base; remember the merge so cleanup can tell.
    // Written before advancing: a crash right after the advance still leaves the marker
    // (`merged_commit` only trusts it once it is part of base).
    if let Some(b) = try_out(wt, &["symbolic-ref", "--short", "-q", "HEAD"]) {
        let _ = run(repo, &["update-ref", &format!("{MARKER_NS}{b}"), &new]);
    }
    advance(repo, base, &old, &new)?;
    Ok(new)
}

/// Forgets that `branch` landed (retry starts new work on the same branch name).
pub fn forget_merged(repo: &Path, branch: &str) {
    let _ = run(repo, &["update-ref", "-d", &format!("{MARKER_NS}{branch}")]);
}

/// The squash commit of `branch` if it already landed on `base` (see `squash_to_base`).
pub fn merged_commit(repo: &Path, base: &str, branch: &str) -> Option<String> {
    let m = try_out(repo, &["rev-parse", "--verify", "--quiet", &format!("{MARKER_NS}{branch}")])?;
    run(repo, &["merge-base", "--is-ancestor", &m, base]).ok().filter(|o| o.ok).map(|_| m)
}

/// New commit on `base` that undoes `sha`, built from trees only (the working copy is not used).
pub fn revert(repo: &Path, base: &str, sha: &str) -> Result<String, GitError> {
    let old = ok(repo, &["rev-parse", &format!("refs/heads/{base}")])?;
    let parent = ok(repo, &["rev-parse", &format!("{sha}^")])?;
    let mb = format!("--merge-base={sha}");
    let o = run(repo, &["merge-tree", "--write-tree", &mb, &old, &parent])?;
    let tree = if o.code == Some(1) {
        return Err(GitError::Conflict(format!("{}\n{}", o.stdout, o.stderr).trim().to_string()));
    } else if o.ok {
        o.stdout.lines().next().unwrap_or("").to_string()
    } else if o.stderr.contains("unknown option") || o.stderr.contains("usage: git merge-tree") {
        // git before 2.40 has no --merge-base (2.38-2.39: «unknown option», Debian 12) and before 2.38 no --write-tree
        // at all (the old usage line, Ubuntu 22.04): the same three-way merge in a scratch index
        revert_tree_compat(repo, sha, &old, &parent)?
    } else {
        return Err(GitError::Other(format!("git merge-tree: {}", o.stderr)));
    };
    let subject = ok(repo, &["log", "-1", "--format=%s", sha])?;
    let msg = format!("Revert \"{subject}\"\n\nThis reverts commit {sha}.");
    let new = ok(repo, &["commit-tree", &tree, "-p", &old, "-m", &msg])?;
    advance(repo, base, &old, &new)?;
    Ok(new)
}

/// The tree of `ours` with `base`→`theirs` applied, for a git without `merge-tree --merge-base`: a three-way
/// `read-tree` in a scratch index, then a line merge of each regular file both sides changed (a submodule, a symlink
/// or a file deleted on one side is a conflict, as merge-tree says). Files and the real index stay.
#[doc(hidden)]
pub fn revert_tree_compat(repo: &Path, base: &str, ours: &str, theirs: &str) -> Result<String, GitError> {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    // one name per call: two reverts in one repository at once (history and a task) must not share the scratch
    let tag = format!("{}-{}", std::process::id(), SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    let gd = hidden_list(repo).ok_or_else(|| GitError::Other("not a repository".into()))?;
    let ix = TmpIndex(gd.with_file_name(format!("ns-revert-index-{tag}")));
    let tmp = ScratchDir(gd.with_file_name(format!("ns-revert-files-{tag}")));
    std::fs::create_dir_all(&tmp.0).map_err(|e| GitError::Other(format!("revert scratch: {e}")))?;
    ix.run(repo, &["read-tree", "-i", "-m", "--aggressive", base, ours, theirs])?;
    // "<mode> <oid> <stage>\t<path>" per stage of each path both sides changed
    let mut paths: Vec<(String, [Option<(String, String)>; 3])> = vec![];
    for e in ix.run(repo, &["ls-files", "-u", "-z"])?.split('\0').filter(|e| !e.is_empty()) {
        let (meta, path) = e.split_once('\t').ok_or_else(|| GitError::Other(format!("ls-files -u: {e}")))?;
        let f: Vec<&str> = meta.split(' ').collect();
        let stage: usize = f.get(2).and_then(|x| x.parse().ok()).filter(|x| (1..=3).contains(x)).ok_or_else(|| GitError::Other(format!("ls-files -u: {e}")))?;
        if paths.last().map(|(p, _)| p != path).unwrap_or(true) {
            paths.push((path.to_string(), [None, None, None]));
        }
        paths.last_mut().unwrap().1[stage - 1] = Some((f[0].to_string(), f[1].to_string()));
    }
    let regular = |m: &str| m == "100644" || m == "100755";
    for (path, st) in &paths {
        let [Some(b), Some(o), Some(t)] = st else { return Err(GitError::Conflict(format!("CONFLICT (modify/delete): {path}"))) };
        if ![b, o, t].iter().all(|(m, _)| regular(m)) {
            return Err(GitError::Conflict(format!("CONFLICT (submodule or symlink): {path}")));
        }
        let mut files = vec![];
        for (i, (_, oid)) in [b, o, t].iter().enumerate() {
            let out = git_cmd(repo).args(["cat-file", "blob", oid]).output().map_err(|e| GitError::Other(format!("git cat-file: {e}")))?;
            if !out.status.success() {
                return Err(GitError::Other(format!("git cat-file blob {oid}: {}", String::from_utf8_lossy(&out.stderr).trim())));
            }
            let f = tmp.0.join(format!("{i}"));
            std::fs::write(&f, &out.stdout).map_err(|e| GitError::Other(format!("revert scratch: {e}")))?;
            files.push(f);
        }
        let m = git_cmd(repo).arg("merge-file").arg("-p").arg(&files[1]).arg(&files[0]).arg(&files[2]).output()
            .map_err(|e| GitError::Other(format!("git merge-file: {e}")))?;
        if !m.status.success() {
            return Err(GitError::Conflict(format!("CONFLICT (content): {path}")));
        }
        std::fs::write(&files[1], &m.stdout).map_err(|e| GitError::Other(format!("revert scratch: {e}")))?;
        let oid = ok(repo, &["hash-object", "-w", &files[1].to_string_lossy()])?;
        let mode = if o.0 == b.0 { &t.0 } else { &o.0 };
        ix.run(repo, &["update-index", "--force-remove", "--", path])?;
        ix.run(repo, &["update-index", "--add", "--cacheinfo", &format!("{mode},{oid},{path}")])?;
    }
    ix.run(repo, &["write-tree"])
}

/// The scratch files of `revert_tree_compat`, gone on every way out (a conflict left copies of the files in .git).
struct ScratchDir(PathBuf);
impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Plain push of `branch` to origin. There is deliberately no force variant.
pub fn push(repo: &Path, branch: &str) -> Result<(), GitError> {
    let o = run(repo, &["push", "origin", branch])?;
    if o.ok {
        Ok(())
    } else {
        Err(GitError::Push(o.stderr))
    }
}

/// Fetch origin and fast-forward local `base` to `origin/base` when possible.
pub fn fetch_ff(repo: &Path, base: &str) -> Result<(), GitError> {
    ok(repo, &["fetch", "origin"])?;
    let remote = format!("refs/remotes/origin/{base}");
    let new = ok(repo, &["rev-parse", &remote])?;
    let old = ok(repo, &["rev-parse", &format!("refs/heads/{base}")])?;
    if old == new {
        return Ok(());
    }
    if !run(repo, &["merge-base", "--is-ancestor", &old, &new])?.ok {
        return Err(GitError::Other(format!("{base} has diverged from origin/{base}")));
    }
    advance(repo, base, &old, &new)
}

fn gh_bin() -> String {
    std::env::var("NIGHTSHIFT_GH").unwrap_or_else(|_| "gh".to_string())
}

fn gh(repo: &Path, args: &[&str]) -> Result<String, GitError> {
    let o = Command::new(gh_bin())
        .current_dir(repo)
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .output()
        .map_err(|_| GitError::Gh("gh_not_installed: install GitHub CLI (https://cli.github.com)".into()))?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        Err(GitError::Gh(String::from_utf8_lossy(&o.stderr).trim().to_string()))
    }
}

/// Pushes the branch (plain push), then `gh pr create`. Returns the PR url.
pub fn open_pr(repo: &Path, branch: &str, base: &str, title: &str, body: &str) -> Result<String, GitError> {
    push(repo, branch)?;
    let out = gh(repo, &["pr", "create", "--head", branch, "--base", base, "--title", title, "--body", body])?;
    out.lines()
        .rev()
        .find(|l| l.starts_with("http"))
        .map(str::to_string)
        .ok_or_else(|| GitError::Gh(format!("no PR url in gh output: {out}")))
}

/// Ok(login) when gh is installed and authenticated, Err(hint) otherwise.
pub fn gh_status(repo: &Path) -> Result<String, GitError> {
    match gh(repo, &["api", "user", "-q", ".login"]) {
        Ok(u) if !u.is_empty() => Ok(u),
        Ok(_) => Err(GitError::Gh("gh_not_logged_in: run `gh auth login`".into())),
        Err(GitError::Gh(m)) if m.starts_with("gh_not_installed") => Err(GitError::Gh(m)),
        Err(GitError::Gh(m)) => Err(GitError::Gh(format!("gh_not_logged_in: run `gh auth login` ({m})"))),
        Err(e) => Err(e),
    }
}

/// v8: the open pull requests of the repo for the GitHub panel (`gh pr list`), as gh gives them: number, title, head and
/// base branch, draft, url, author, updatedAt, reviewDecision, statusCheckRollup. Read-only.
pub fn pr_list(repo: &Path) -> Result<serde_json::Value, GitError> {
    let fields = "number,title,headRefName,baseRefName,isDraft,url,author,updatedAt,reviewDecision,statusCheckRollup";
    let out = gh(repo, &["pr", "list", "--state", "open", "--limit", "30", "--json", fields])?;
    serde_json::from_str(&out).map_err(|e| GitError::Gh(format!("bad gh output: {e}")))
}

/// "open" | "merged" | "closed"
pub fn pr_state(repo: &Path, url: &str) -> Result<String, GitError> {
    Ok(gh(repo, &["pr", "view", url, "--json", "state", "-q", ".state"])?.to_lowercase())
}

/// Why `publish` refused or failed. JSON `{kind, ...}` (kind snake_case) for the window, which translates it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PublishError {
    #[error("gh_missing")]
    GhMissing,
    #[error("gh_not_logged_in")]
    GhNotLoggedIn,
    #[error("name_taken")]
    NameTaken,
    /// Not a GitHub repository name (letters, digits, `.`, `_`, `-`; up to 100).
    #[error("bad_name")]
    BadName,
    /// A key or `.env` some commit of what would be pushed added (rule F1): nothing is sent.
    #[error("protected_in_history: {}", paths.join(", "))]
    ProtectedInHistory { paths: Vec<String> },
    #[error("publish_failed: {detail}")]
    Failed { detail: String },
}

fn repo_name_ok(name: &str) -> bool {
    (1..=100).contains(&name.len()) && !name.starts_with(['-', '.']) && name.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

/// Secret-like files any commit reachable from HEAD added or changed: what `gh repo create --push` would send.
/// `-m`: a merge commit's own files too (against each parent), plain `log` shows none for merges.
pub fn protected_in_history(repo: &Path) -> Result<Vec<String>, GitError> {
    if !has_commit(repo) {
        return Ok(vec![]);
    }
    Ok(nul_list(&ok(repo, &["log", "-m", "--format=", "--name-only", "--no-renames", "--diff-filter=d", "-z", "HEAD"])?))
}

/// Creates the GitHub repository and pushes the current branch: `gh repo create <name> --private|--public
/// --description --source <path> --push`. The history is checked for protected files first (F1); with one
/// there, gh is never called. Returns the repository URL.
/// Checked bound = what `gh --push` sends: the current branch, every commit reachable from HEAD.
pub fn publish(path: &Path, name: &str, private: bool, desc: &str) -> Result<String, PublishError> {
    if !repo_name_ok(name) {
        return Err(PublishError::BadName);
    }
    let paths = protected_in_history(path).map_err(|e| PublishError::Failed { detail: e.to_string() })?;
    if !paths.is_empty() {
        return Err(PublishError::ProtectedInHistory { paths });
    }
    if !has_commit(path) {
        return Err(PublishError::Failed { detail: "no_commits".into() });
    }
    match gh_status(path) {
        Ok(_) => {}
        Err(GitError::Gh(m)) if m.starts_with("gh_not_installed") => return Err(PublishError::GhMissing),
        Err(_) => return Err(PublishError::GhNotLoggedIn),
    }
    let vis = if private { "--private" } else { "--public" };
    let desc = format!("--description={desc}"); // `=`: a description starting with `-` stays a value
    let src = path.to_string_lossy();
    match gh(path, &["repo", "create", name, vis, &desc, "--source", &src, "--push"]) {
        Ok(out) => out
            .lines()
            .rev()
            .map(str::trim)
            .find(|l| l.starts_with("https://"))
            .map(str::to_string)
            .ok_or(PublishError::Failed { detail: format!("no repository url in gh output: {out}") }),
        Err(GitError::Gh(m)) if m.contains("already exists") => Err(PublishError::NameTaken),
        Err(e) => Err(PublishError::Failed { detail: e.to_string() }),
    }
}

/// One row of the history page. `time` = author time, Unix seconds. `task` = the id from the
/// `Nightshift-Task:` trailer of a task's commit, else the 8-char prefix of an `ns/<id>-…` branch on it.
/// `lane` = column of the node; `edges` = lines from this row (lane here) to the next row (lane there).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CommitRow {
    pub sha: String,
    pub parents: Vec<String>,
    pub author: String,
    pub time: i64,
    pub subject: String,
    pub refs: Vec<String>,
    pub task: Option<String>,
    pub lane: u16,
    pub edges: Vec<(u16, u16)>,
}

/// Trailer the merge of a task writes, so the history knows the commit.
pub const TASK_TRAILER: &str = "Nightshift-Task";

/// Lanes open after a row: the commit each one waits for and the lane of the row above it comes from.
#[derive(Clone, Default, Debug)]
struct Lanes {
    slots: Vec<Option<String>>,
    from: Vec<u16>,
}

impl Lanes {
    /// Puts the next commit in a lane. Returns its lane and the edges of the row above into this one.
    fn place(&mut self, sha: &str, parents: &[String]) -> (u16, Vec<(u16, u16)>) {
        let first_free = |s: &Vec<Option<String>>| s.iter().position(Option::is_none).unwrap_or(s.len());
        let node = self.slots.iter().position(|s| s.as_deref() == Some(sha)).unwrap_or_else(|| first_free(&self.slots));
        let edges = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|s| (self.from[i], if s == sha { node as u16 } else { i as u16 })))
            .collect();
        for (i, s) in self.slots.iter_mut().enumerate() {
            if s.as_deref() == Some(sha) {
                *s = None;
            }
            self.from[i] = i as u16;
        }
        if let Some(p0) = parents.first() {
            self.put(node, p0, node);
        }
        for p in parents.iter().skip(1) {
            let k = first_free(&self.slots);
            self.put(k, p, node);
        }
        while self.slots.last().is_some_and(Option::is_none) {
            self.slots.pop();
            self.from.pop();
        }
        (node as u16, edges)
    }
    fn put(&mut self, k: usize, sha: &str, from: usize) {
        if k >= self.slots.len() {
            self.slots.resize(k + 1, None);
            self.from.resize(k + 1, 0);
        }
        self.slots[k] = Some(sha.to_string());
        self.from[k] = from as u16;
    }
}

/// Same order for every page and every replay.
const LOG_ORDER: [&str; 5] = ["--topo-order", "--branches", "--tags", "--remotes", "HEAD"];

type LogCache = std::sync::Mutex<std::collections::HashMap<PathBuf, (String, usize, Lanes)>>;
/// Per repository: (refs fingerprint, rows already placed, lanes after them), so the next page goes on from there.
static LOG_CACHE: std::sync::OnceLock<LogCache> = std::sync::OnceLock::new();

/// A page of the history (newest first) with the graph. Pages are computed one after another: the next
/// page continues the lanes of the previous one while the refs stay the same; any other `skip` replays the
/// first `skip` commits (their shas and parents only). An empty repository gives an empty page.
pub fn log_page(path: &Path, skip: usize, limit: usize) -> Result<Vec<CommitRow>, GitError> {
    if !has_commit(path) || limit == 0 {
        return Ok(vec![]);
    }
    let fp = format!("{}\n{}", ok(path, &["rev-parse", "HEAD"])?, ok(path, &["for-each-ref", "--format=%(objectname) %(refname)", "refs/heads", "refs/tags", "refs/remotes"])?);
    let key = path.to_path_buf();
    let cache = LOG_CACHE.get_or_init(Default::default);
    let cached = cache.lock().unwrap().get(&key).filter(|(f, n, _)| *f == fp && *n == skip).map(|c| c.2.clone());
    let mut lanes = match cached {
        Some(l) => l,
        None => {
            let mut l = Lanes::default();
            if skip > 0 {
                let mut args = vec!["log", "--format=%H %P"];
                args.extend(LOG_ORDER);
                let n = format!("-n{skip}");
                args.push(&n);
                for line in ok(path, &args)?.lines() {
                    let mut it = line.split(' ').map(str::to_string);
                    let sha = it.next().unwrap_or_default();
                    l.place(&sha, &it.collect::<Vec<_>>());
                }
            }
            l
        }
    };
    let (skip_arg, n_arg) = (format!("--skip={skip}"), format!("-n{}", limit + 1));
    let mut args = vec![
        "log",
        "--decorate-refs=refs/heads",
        "--decorate-refs=refs/tags",
        "--decorate-refs=refs/remotes",
        // NUL between fields and records (`-z`): git refuses NUL in a message, so a subject cannot forge a row
        "-z",
        "--format=%H%x00%P%x00%an%x00%at%x00%s%x00%D%x00%(trailers:key=Nightshift-Task,valueonly,separator=%x2C)",
    ];
    args.extend(LOG_ORDER);
    args.extend([skip_arg.as_str(), n_arg.as_str()]);
    let out = ok(path, &args)?;
    let mut rows: Vec<CommitRow> = vec![];
    let tokens: Vec<&str> = out.strip_suffix('\0').unwrap_or(&out).split('\0').collect();
    for f in tokens.chunks(7) {
        if !is_log_record(f) {
            break; // out of step: nothing after it can be trusted
        }
        let get = |i: usize| f[i].trim();
        let sha = get(0).to_string();
        let parents: Vec<String> = get(1).split_whitespace().map(str::to_string).collect();
        if rows.len() == limit {
            // one commit past the page: only for the edges of its last row
            let (_, e) = lanes.clone().place(&sha, &parents);
            if let Some(last) = rows.last_mut() {
                last.edges = e;
            }
            break;
        }
        let (lane, e) = lanes.place(&sha, &parents);
        if let Some(last) = rows.last_mut() {
            last.edges = e;
        }
        let refs: Vec<String> = get(5).split(", ").map(|r| r.trim_start_matches("HEAD -> ").to_string()).filter(|r| !r.is_empty() && r != "HEAD").collect();
        let task = Some(get(6).split(',').next().unwrap_or("").trim().to_string()).filter(|s| !s.is_empty()).or_else(|| {
            refs.iter().find_map(|r| r.rsplit("ns/").next().filter(|_| r.contains("ns/")).map(|x| x.chars().take(8).collect()))
        });
        rows.push(CommitRow { sha, parents, author: get(2).to_string(), time: get(3).parse().unwrap_or(0), subject: get(4).to_string(), refs, task, lane, edges: vec![] });
    }
    cache.lock().unwrap().insert(key, (fp, skip + rows.len(), lanes));
    Ok(rows)
}

/// A `log_page` record: 7 fields, a 40/64-hex sha, hex parents split by single spaces, a number for the time.
fn is_log_record(f: &[&str]) -> bool {
    let hex = |s: &str| matches!(s.len(), 40 | 64) && s.bytes().all(|b| b.is_ascii_hexdigit());
    f.len() == 7 && hex(f[0]) && (f[1].is_empty() || f[1].split(' ').all(hex)) && !f[3].is_empty() && f[3].bytes().all(|b| b.is_ascii_digit())
}

/// Undoes `sha` with a new commit on the current branch (`revert` rules: trees only, the working copy is
/// moved only when clean). A merge is undone against its first parent. Returns the new commit.
pub fn revert_commit(path: &Path, sha: &str) -> Result<String, GitError> {
    let branch = repo_info(path).current_branch.ok_or_else(|| GitError::Other("detached_head".into()))?;
    revert(path, &branch, sha)
}

/// The commit is on a remote-tracking branch: undoing it locally and pushing is an outward action (D01).
pub fn is_pushed(path: &Path, sha: &str) -> bool {
    try_out(path, &["branch", "-r", "--contains", sha]).is_some()
}

fn default_base(repo: &Path) -> Option<String> {
    repo_info(repo).base_branch
}

fn is_merged(repo: &Path, base: &str, branch: &str) -> bool {
    let tip = match try_out(repo, &["rev-parse", &format!("refs/heads/{branch}")]) {
        Some(t) => t,
        None => return false,
    };
    let anc = |a: &str| run(repo, &["merge-base", "--is-ancestor", a, base]).map(|o| o.ok).unwrap_or(false);
    if anc(&tip) {
        return true;
    }
    if let Some(m) = try_out(repo, &["rev-parse", "--verify", "--quiet", &format!("{MARKER_NS}{branch}")]) {
        if anc(&m) {
            return true;
        }
    }
    // every commit has a patch-equivalent on base (rebase-merge on GitHub, single-commit squash)
    run(repo, &["cherry", base, &tip])
        .map(|o| o.ok && !o.stdout.lines().any(|l| l.starts_with('+')))
        .unwrap_or(false)
}

pub fn list_ns_branches(repo: &Path) -> Vec<(String, bool)> {
    let Some(base) = default_base(repo) else { return vec![] };
    let refs = format!("refs/heads/{NS_PREFIX}");
    let out = run(repo, &["for-each-ref", "--format=%(refname:short)", &refs]).map(|o| o.stdout).unwrap_or_default();
    out.lines()
        .filter(|b| b.starts_with(NS_PREFIX))
        .map(|b| (b.to_string(), is_merged(repo, &base, b)))
        .collect()
}

/// Deletes merged `ns/*` branches only (never other branches, never unmerged ones).
pub fn delete_merged_ns_branches(repo: &Path) -> Result<u32, GitError> {
    let mut n = 0;
    for (b, merged) in list_ns_branches(repo) {
        if !merged || !b.starts_with(NS_PREFIX) {
            continue;
        }
        // a branch still checked out in a worktree is in use; git refuses and we skip it
        if run(repo, &["branch", "-D", &b])?.ok {
            let _ = run(repo, &["update-ref", "-d", &format!("{MARKER_NS}{b}")]);
            n += 1;
        }
    }
    Ok(n)
}

pub fn detect_commands(repo: &Path) -> (Option<String>, Option<String>) {
    if let Ok(txt) = std::fs::read_to_string(repo.join("package.json")) {
        let has_test = serde_json::from_str::<serde_json::Value>(&txt)
            .ok()
            .and_then(|v| v["scripts"]["test"].as_str().map(str::to_string))
            .map(|s| !s.contains("no test specified"))
            .unwrap_or(false);
        if has_test {
            let setup = if repo.join("package-lock.json").exists() { "npm ci" } else { "npm install" };
            return (Some("npm test".into()), Some(setup.into()));
        }
    }
    if repo.join("Cargo.toml").exists() {
        return (Some("cargo test".into()), None);
    }
    if ["pyproject.toml", "pytest.ini", "setup.py"].iter().any(|f| repo.join(f).exists()) {
        return (Some("pytest".into()), None);
    }
    if repo.join("go.mod").exists() {
        return (Some("go test ./...".into()), None);
    }
    if let Ok(mk) = std::fs::read_to_string(repo.join("Makefile")) {
        if mk.lines().any(|l| l.starts_with("test:") || l.starts_with("test :")) {
            return (Some("make test".into()), None);
        }
    }
    (None, None)
}

// ---- v2: read-only copies, per-file diff, restore, fast-forward, shallow clone ----

/// One file of the task's change, for the diff view.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FileDiff {
    pub path: String,
    /// Unified diff text, at most `max_lines` lines; empty for a binary file or when there is no change.
    pub text: String,
    /// The diff was cut at `max_lines`.
    pub truncated: bool,
    /// Git treats the file as binary: no text, only the size.
    pub binary: bool,
    /// Size of the file in the working copy in bytes (0 when it is gone).
    pub size: u64,
}

/// A path inside a copy: relative, no `..`, no NUL. Anything else never reaches git.
fn safe_rel(p: &str) -> Result<(), GitError> {
    let bad = p.is_empty()
        || p.contains('\0')
        || Path::new(p).is_absolute()
        || Path::new(p).components().any(|c| matches!(c, std::path::Component::ParentDir));
    if bad {
        return Err(GitError::Other(format!("bad path: {p}")));
    }
    Ok(())
}

/// The real location of `full`'s folder (symlinks resolved, nearest existing ancestor for new paths) is
/// inside `root`; a folder swapped for a link to the outside is refused.
fn inside(root: &Path, full: &Path, shown: &str) -> Result<(), GitError> {
    let mut dir = full.parent();
    while let Some(d) = dir {
        if let Ok(real) = d.canonicalize() {
            return if real.starts_with(root) { Ok(()) } else { Err(GitError::Other(format!("path leaves the copy: {shown}"))) };
        }
        dir = d.parent();
    }
    Err(GitError::Other(format!("path leaves the copy: {shown}")))
}

fn literal(p: &str) -> String {
    format!(":(literal){p}")
}

const READONLY_MARK: &str = "ns-readonly-copy";

fn git_dir_of(wt: &Path) -> Option<PathBuf> {
    let gd = PathBuf::from(try_out(wt, &["rev-parse", "--git-dir"])?);
    Some(if gd.is_absolute() { gd } else { wt.join(gd) })
}

/// A copy made by `create_readonly_copy`: it carries the mark in its private git dir, is a linked
/// worktree, and sits on a detached HEAD. A worktree the human made (even a detached one) has no mark.
fn is_app_copy(wt: &Path) -> bool {
    let (Some(gd), Some(common)) = (git_dir_of(wt), try_out(wt, &["rev-parse", "--path-format=absolute", "--git-common-dir"])) else {
        return false;
    };
    let linked = try_out(wt, &["rev-parse", "--absolute-git-dir"]).is_some_and(|a| a != common);
    linked && gd.join(READONLY_MARK).is_file() && try_out(wt, &["symbolic-ref", "-q", "HEAD"]).is_none()
}

/// Copy of `base` for roles that only read (chat, planner): a worktree at `dir` without a branch,
/// so nothing of it can ever reach a branch or the human's folder. `dir` must not exist yet.
pub fn create_readonly_copy(repo: &Path, dir: &Path, base: &str) -> Result<PathBuf, GitError> {
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| GitError::Other(e.to_string()))?;
    }
    ok(repo, &["worktree", "add", "--detach", &dir.to_string_lossy(), base])?;
    let gd = git_dir_of(dir).ok_or_else(|| GitError::Other(format!("not a worktree: {}", dir.display())))?;
    std::fs::write(gd.join(READONLY_MARK), "created by Nightshift\n").map_err(|e| GitError::Other(e.to_string()))?;
    Ok(dir.to_path_buf())
}

/// Brings a copy made by `create_readonly_copy` to the current tip of `base` and throws away
/// everything the last run left in it (edits, untracked and ignored files). Refuses anything that
/// was not made by `create_readonly_copy` (no mark in its git dir): `reset --hard` and `clean` must never touch the human's folder.
pub fn refresh_readonly_copy(wt: &Path, base: &str) -> Result<(), GitError> {
    if !is_app_copy(wt) {
        return Err(GitError::Other(format!("not a read-only copy of the app: {}", wt.display())));
    }
    let sha = ok(wt, &["rev-parse", "--verify", &format!("refs/heads/{base}^{{commit}}")])?;
    ok(wt, &["checkout", "-q", "--detach", "-f", &sha])?;
    ok(wt, &["reset", "-q", "--hard", &sha])?;
    ok(wt, &["clean", "-q", "-f", "-d", "-x"])?;
    Ok(())
}

/// Byte limit of `diff_file` next to its line limit: a one-line minified file must not fill memory.
const DIFF_FILE_MAX_BYTES: usize = 256 * 1024;

/// The change of one file against where the task forked from `base`, cut to `max_lines` lines and
/// 256 KiB, whichever comes first (`truncated` then). git's output is read as a stream and git is
/// stopped at the cut, so a huge diff is never held whole.
pub fn diff_file(wt: &Path, base: &str, path: &str, max_lines: usize) -> FileDiff {
    let mut d = FileDiff { path: path.to_string(), ..FileDiff::default() };
    if safe_rel(path).is_err() {
        return d;
    }
    d.size = std::fs::metadata(wt.join(path)).map(|m| m.len()).unwrap_or(0);
    let mb = merge_base(wt, base);
    let spec = literal(path);
    let stat = run(wt, &["diff", "--numstat", &mb, "--", &spec]).map(|o| o.stdout).unwrap_or_default();
    if stat.starts_with("-\t-\t") {
        d.binary = true;
        return d;
    }
    let (text, truncated) = stream_diff(wt, &["diff", &mb, "--", &spec], max_lines);
    d.text = text;
    d.truncated = truncated;
    d
}

/// Output of a git diff command read as a stream and cut at `max_lines` lines or the byte limit.
fn stream_diff(dir: &Path, args: &[&str], max_lines: usize) -> (String, bool) {
    use std::io::BufRead;
    let Ok(mut child) = git_cmd(dir).args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn() else {
        return (String::new(), false);
    };
    let mut reader = std::io::BufReader::new(child.stdout.take().expect("piped"));
    let (mut text, mut lines, mut buf, mut truncated) = (String::new(), 0usize, Vec::new(), false);
    loop {
        buf.clear();
        // a single line is read at most up to the byte limit too
        let n = (&mut reader).take((DIFF_FILE_MAX_BYTES + 1) as u64).read_until(b'\n', &mut buf).unwrap_or(0);
        if n == 0 {
            break;
        }
        if lines >= max_lines || text.len() + buf.len() > DIFF_FILE_MAX_BYTES {
            truncated = true;
            break;
        }
        text.push_str(&String::from_utf8_lossy(&buf));
        lines += 1;
    }
    if truncated {
        let _ = child.kill();
    }
    let _ = child.wait();
    (text.trim_end_matches('\n').to_string(), truncated)
}

/// The first commit(s) of the repository: two folders are the same repository when these are equal. None = not a repository.
pub fn root_commits(repo: &Path) -> Option<String> {
    let o = run(repo, &["rev-list", "--max-parents=0", "HEAD"]).ok()?;
    let mut v: Vec<&str> = o.stdout.lines().collect();
    v.sort();
    Some(v.join(","))
}

/// The files a commit changed (a squash commit on base: a merged task), with their line counts.
pub fn commit_stat(repo: &Path, sha: &str) -> Vec<FileStat> {
    if !sha.chars().all(|c| c.is_ascii_hexdigit()) || sha.is_empty() {
        return vec![];
    }
    let out = run(repo, &["show", "--numstat", "--format=", "--no-renames", sha]).map(|o| o.stdout).unwrap_or_default();
    out.lines()
        .filter_map(|l| {
            let mut p = l.splitn(3, '\t');
            let (a, d, path) = (p.next()?, p.next()?, p.next()?);
            Some(FileStat { path: path.to_string(), insertions: a.parse().unwrap_or(0), deletions: d.parse().unwrap_or(0) })
        })
        .collect()
}

/// The diff one commit made to one file, like `diff_file` but for a commit that is already on base.
pub fn commit_file_diff(repo: &Path, sha: &str, path: &str, max_lines: usize) -> FileDiff {
    let mut d = FileDiff { path: path.to_string(), ..FileDiff::default() };
    if safe_rel(path).is_err() || sha.is_empty() || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return d;
    }
    let spec = literal(path);
    d.size = run(repo, &["cat-file", "-s", &format!("{sha}:{path}")]).ok().and_then(|o| o.stdout.trim().parse().ok()).unwrap_or(0);
    let stat = run(repo, &["show", "--numstat", "--format=", "--no-renames", sha, "--", &spec]).map(|o| o.stdout).unwrap_or_default();
    if stat.starts_with("-\t-\t") {
        d.binary = true;
        return d;
    }
    let (text, truncated) = stream_diff(repo, &["show", "--format=", "--no-renames", sha, "--", &spec], max_lines);
    d.text = text;
    d.truncated = truncated;
    d
}

/// Paths that differ between two commits (renames count as delete + add), sorted by git.
pub fn changed_paths(wt: &Path, from: &str, to: &str) -> Vec<String> {
    let out = run(wt, &["diff", "-z", "--name-only", "--no-renames", from, to]).map(|o| o.stdout).unwrap_or_default();
    out.split('\0').filter(|p| !p.is_empty()).map(str::to_string).collect()
}

/// Puts `paths` back to how they are in commit `from`, in the index and on disk; a path that does
/// not exist in `from` is deleted. For a part of a crew that touched files it does not own.
pub fn restore_paths(wt: &Path, from: &str, paths: &[String]) -> Result<(), GitError> {
    // every path is checked first, so a bad one changes nothing
    let root = wt.canonicalize().map_err(|e| GitError::Other(format!("{}: {e}", wt.display())))?;
    for p in paths {
        safe_rel(p)?;
        inside(&root, &wt.join(p), p)?;
    }
    for p in paths {
        let spec = literal(p);
        let existed = run(wt, &["cat-file", "-e", &format!("{from}:{p}")])?.ok;
        if existed {
            ok(wt, &["restore", "--source", from, "--staged", "--worktree", "--", &spec])?;
        } else {
            let _ = run(wt, &["rm", "-q", "-r", "-f", "--cached", "--ignore-unmatch", "--", &spec])?;
            let full = wt.join(p);
            if full.is_dir() {
                let _ = std::fs::remove_dir_all(&full);
            } else if full.symlink_metadata().is_ok() {
                std::fs::remove_file(&full).map_err(|e| GitError::Other(format!("{p}: {e}")))?;
            }
        }
    }
    Ok(())
}

/// Moves the app's branch `ns/*` forward to `to` (a commit or ref). Only forward: if `to` is not
/// a descendant of the branch, nothing happens. A dirty checkout of the branch blocks it.
pub fn ff_branch(repo: &Path, branch: &str, to: &str) -> Result<(), GitError> {
    if !branch.starts_with(NS_PREFIX) {
        return Err(GitError::Other(format!("only {NS_PREFIX}* branches move: {branch}")));
    }
    let old = ok(repo, &["rev-parse", "--verify", &format!("refs/heads/{branch}")])?;
    let new = ok(repo, &["rev-parse", "--verify", &format!("{to}^{{commit}}")])?;
    if old == new {
        return Ok(());
    }
    if !run(repo, &["merge-base", "--is-ancestor", &old, &new])?.ok {
        return Err(GitError::Other(format!("not a fast-forward: {branch} -> {to}")));
    }
    advance(repo, branch, &old, &new)
}

/// Only these remotes are cloned: `https://`, `ssh://`, `git@host:path`, or an existing local folder.
/// `ext::` (runs a command), `--upload-pack`-style option text, `http://`, `file://` and the rest are refused.
fn clone_source(url: &str) -> Result<String, GitError> {
    let u = url.trim();
    let bad = |why: &str| GitError::Other(format!("clone refused ({why}): {u}"));
    if u.is_empty() || u.starts_with('-') || u.contains(['\0', '\n', '\r']) || u.contains("::") || u.contains("--upload-pack") {
        return Err(bad("unsafe text"));
    }
    if u.starts_with("https://") || u.starts_with("ssh://") || u.starts_with("git@") {
        return Ok(u.to_string());
    }
    if u.contains("://") {
        return Err(bad("scheme not allowed"));
    }
    let local = Path::new(u);
    if local.is_absolute() && local.is_dir() {
        // file:// so that --depth is honoured (a plain path makes a full local clone)
        let canon = local.canonicalize().map_err(|e| GitError::Other(e.to_string()))?;
        return Ok(format!("file://{}", canon.display()));
    }
    Err(bad("not a remote and not an existing local folder"))
}

/// Shallow (`--depth 1`) clone of `url` into `dest`, which the caller makes a fresh temp folder.
pub fn clone_shallow(url: &str, dest: &Path) -> Result<(), GitError> {
    let src = clone_source(url)?;
    let parent = dest.parent().unwrap_or(Path::new("."));
    let o = Command::new("git")
        .current_dir(parent)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .env("GIT_ALLOW_PROTOCOL", "https:ssh:file") // belt and braces next to the check above
        .args(["clone", "--depth", "1", "--no-tags", "-q", "--", &src])
        .arg(dest)
        .output()
        .map_err(|e| GitError::Other(format!("git clone: {e}")))?;
    if o.status.success() {
        Ok(())
    } else {
        Err(GitError::Other(format!("git clone: {}", String::from_utf8_lossy(&o.stderr).trim())))
    }
}

// ---- v5: snapshots of a task's copy (turn rollback), main-folder handoff ----

const SNAP_PREFIX: &str = "refs/nightshift/";

/// One saved state of a task's copy.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Snapshot {
    pub n: u32,
    pub sha: String,
    /// RFC 3339 time of the snapshot; `label` is what the caller put in the message (a role).
    pub at: String,
    pub label: String,
}

fn snap_task_ok(task: &str) -> Result<(), GitError> {
    if task.is_empty() || !task.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(GitError::Other(format!("bad task id: {task}")));
    }
    Ok(())
}

/// A scratch index next to the copy's own, so the real index and the files stay as they are.
struct TmpIndex(PathBuf);
impl TmpIndex {
    fn new(wt: &Path) -> Result<TmpIndex, GitError> {
        let gd = hidden_list(wt).ok_or_else(|| GitError::Other(format!("not a worktree: {}", wt.display())))?;
        Ok(TmpIndex(gd.with_file_name(format!("ns-snap-index-{}", std::process::id()))))
    }
    fn run(&self, wt: &Path, args: &[&str]) -> Result<String, GitError> {
        let o = git_cmd(wt).env("GIT_INDEX_FILE", &self.0).args(args).output().map_err(|e| GitError::Other(format!("git {}: {e}", args.join(" "))))?;
        if o.status.success() {
            Ok(String::from_utf8_lossy(&o.stdout).trim_end().to_string())
        } else {
            Err(GitError::Other(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim())))
        }
    }
    /// The whole tree as it is now (tracked, untracked, uncommitted), without the copied local files and keys.
    fn fill(&self, wt: &Path) -> Result<(), GitError> {
        let hidden = hidden_specs(wt)?;
        if has_commit(wt) {
            self.run(wt, &["read-tree", "HEAD"])?;
        }
        self.run(wt, &["add", "-A"])?;
        let mut out: Vec<String> = hidden.iter().map(|h| h.trim_start_matches(":(literal)").to_string()).collect();
        let listed = self.run(wt, &["ls-files", "-z"])?;
        out.extend(listed.split('\0').filter(|p| !p.is_empty() && secret_path(p)).map(str::to_string));
        for p in out {
            let _ = self.run(wt, &["rm", "-q", "-r", "-f", "--cached", "--ignore-unmatch", "--", &literal(&p)]);
        }
        Ok(())
    }
}
impl Drop for TmpIndex {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Saves the copy as it is now (committed and not) under `refs/nightshift/<task>/<n>`: a commit that is
/// on no branch, so history and `git branch` do not show it. Returns the new number.
pub fn snapshot(wt: &Path, task: &str, label: &str) -> Result<u32, GitError> {
    snap_task_ok(task)?;
    let n = snapshots(wt, task).iter().map(|s| s.n).max().map_or(1, |m| m + 1);
    let idx = TmpIndex::new(wt)?;
    idx.fill(wt)?;
    let tree = idx.run(wt, &["write-tree"])?;
    let msg = format!("nightshift snapshot {n}: {label}");
    let sha = if has_commit(wt) {
        ok(wt, &["commit-tree", &tree, "-p", "HEAD", "-m", &msg])?
    } else {
        ok(wt, &["commit-tree", &tree, "-m", &msg])?
    };
    ok(wt, &["update-ref", &format!("{SNAP_PREFIX}{task}/{n}"), &sha])?;
    Ok(n)
}

pub fn snapshots(repo: &Path, task: &str) -> Vec<Snapshot> {
    if snap_task_ok(task).is_err() {
        return vec![];
    }
    let fmt = "%(refname)\t%(objectname)\t%(committerdate:iso-strict)\t%(subject)";
    let out = run(repo, &["for-each-ref", &format!("--format={fmt}"), &format!("{SNAP_PREFIX}{task}/")]).map(|o| o.stdout).unwrap_or_default();
    let mut v: Vec<Snapshot> = out
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.splitn(4, '\t').collect();
            let n = f.first()?.rsplit('/').next()?.parse().ok()?;
            let label = f.get(3)?.split_once(": ").map_or("", |x| x.1).to_string();
            Some(Snapshot { n, sha: f.get(1)?.to_string(), at: f.get(2)?.to_string(), label })
        })
        .collect();
    v.sort_by_key(|s| s.n);
    v
}

pub fn drop_snapshots(repo: &Path, task: &str) {
    for s in snapshots(repo, task) {
        let _ = run(repo, &["update-ref", "-d", &format!("{SNAP_PREFIX}{task}/{}", s.n)]);
    }
}

/// Puts the copy back to snapshot `n`: the branch, the files, the files that were added since. The state it
/// leaves is saved as a snapshot first (returned), so the rollback itself can be rolled back.
/// The copied local files (`.env`) are not touched. Refused in the middle of a rebase.
pub fn restore_snapshot(wt: &Path, task: &str, n: u32) -> Result<u32, GitError> {
    snap_task_ok(task)?;
    if rebase_in_progress(wt) {
        return Err(GitError::Other("rebase_in_progress".into()));
    }
    let snap = snapshots(wt, task).into_iter().find(|s| s.n == n).ok_or_else(|| GitError::Other(format!("no_snapshot: {n}")))?;
    let saved = snapshot(wt, task, "before_restore")?;
    let idx = TmpIndex::new(wt)?;
    idx.fill(wt)?;
    // files of the current state that the snapshot lacks are removed, the others are put as the snapshot has them
    idx.run(wt, &["read-tree", "--reset", "-u", &format!("{}^{{tree}}", snap.sha)])?;
    // the branch goes to where it was; the files stay (as uncommitted changes against it)
    if let Ok(parent) = ok(wt, &["rev-parse", "--verify", "--quiet", &format!("{}^", snap.sha)]) {
        ok(wt, &["reset", "-q", "--mixed", &parent])?;
    }
    Ok(saved)
}

/// The base of the main-folder handoff: the folder must be on a named branch, clean, not in the middle of something.
fn main_ready(repo: &Path) -> Result<String, GitError> {
    let info = repo_info(repo);
    if !info.is_repo {
        return Err(GitError::Other("not_a_repo".into()));
    }
    if rebase_in_progress(repo) || try_out(repo, &["rev-parse", "--verify", "--quiet", "MERGE_HEAD"]).is_some() {
        return Err(GitError::Other("main_busy".into()));
    }
    if info.dirty {
        return Err(GitError::AdvanceBlocked("main_dirty".into()));
    }
    info.current_branch.ok_or_else(|| GitError::Other("main_detached".into()))
}

/// "Забрать к себе": the project folder goes to the task's branch. A branch lives in one place only, so the
/// task's copy lets go of it (detached at the same commit, files untouched) and keeps its files. Returns the
/// branch the folder was on. Nothing is forced: a dirty folder is refused, and a failed switch puts the copy back.
pub fn take_to_main(repo: &Path, wt: &Path, branch: &str) -> Result<String, GitError> {
    if !branch.starts_with(NS_PREFIX) {
        return Err(GitError::Other(format!("only {NS_PREFIX}* branches are taken: {branch}")));
    }
    let prev = main_ready(repo)?;
    ok(wt, &["checkout", "-q", "--detach"])?;
    if let Err(e) = ok(repo, &["checkout", "-q", branch]) {
        let _ = run(wt, &["checkout", "-q", branch]);
        return Err(e);
    }
    Ok(prev)
}

/// "Отдать агенту": the folder returns to `prev` (the base branch if that is gone), the copy takes its branch back.
pub fn give_to_agent(repo: &Path, wt: &Path, branch: &str, prev: &str, fallback: &str) -> Result<(), GitError> {
    let on = repo_info(repo).current_branch;
    if on.as_deref() == Some(branch) {
        let back = if branch_exists(repo, prev) { prev } else { fallback };
        main_ready(repo)?;
        ok(repo, &["checkout", "-q", back])?;
    }
    // if this fails the folder is already back and the copy stays as it is (detached, files intact): try again
    ok(wt, &["checkout", "-q", branch])?;
    Ok(())
}
