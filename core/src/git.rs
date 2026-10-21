//! All git / gh operations. System binaries via `Command`, no libgit2.
//! Safety rules: never `push --force`, never `reset --hard`, only `ns/*` branches are deleted,
//! the human's working copy is only touched by `merge --ff-only` on a clean base branch.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
    let branch = if wt.exists() { try_out(wt, &["symbolic-ref", "--short", "-q", "HEAD"]) } else { None };
    if wt.exists() {
        ok(repo, &["worktree", "remove", "--force", &wt.to_string_lossy()])?;
    }
    let _ = run(repo, &["worktree", "prune"]);
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

/// `-- :(literal)p ...` for the hidden paths; None when there are none.
fn hidden_specs(wt: &Path) -> Option<Vec<String>> {
    let hidden = hidden_list(wt).and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
    let specs: Vec<String> = hidden.lines().filter(|l| !l.is_empty()).map(|l| format!(":(literal){l}")).collect();
    (!specs.is_empty()).then_some(specs)
}

fn run_with(wt: &Path, head: &[&str], specs: &[String]) -> Result<Out, GitError> {
    let mut args: Vec<&str> = head.to_vec();
    args.push("--");
    args.extend(specs.iter().map(String::as_str));
    run(wt, &args)
}

/// `git add -A`, then the hidden paths are put back as HEAD has them (out of the index if HEAD
/// lacks them). Naming ignored paths here is fine, unlike in an `add` pathspec.
fn add_all(wt: &Path) -> Result<(), GitError> {
    ok(wt, &["add", "-A"])?;
    if let Some(specs) = hidden_specs(wt) {
        let o = run_with(wt, &["reset", "-q"], &specs)?;
        if !o.ok {
            return Err(GitError::Other(format!("git reset hidden paths: {}", o.stderr)));
        }
    }
    Ok(())
}

/// Hidden paths that a commit of the branch since it forked from `base` added or changed.
pub fn hidden_committed(wt: &Path, base: &str) -> Vec<String> {
    let Some(specs) = hidden_specs(wt) else { return vec![] };
    let range = format!("{}..HEAD", merge_base(wt, base));
    let out = run_with(wt, &["log", "--format=", "--name-only", &range], &specs).map(|o| o.stdout).unwrap_or_default();
    let mut v: Vec<String> = out.lines().filter(|l| !l.is_empty()).map(str::to_string).collect();
    v.sort();
    v.dedup();
    v
}

/// Someone (the agent) committed a hidden path: fold the branch back into the index
/// (`reset --soft` to the fork point, files stay) so the next `commit_all` leaves it out and
/// no commit of the branch keeps it. Ok(true) when it did that.
pub fn uncommit_hidden(wt: &Path, base: &str) -> Result<bool, GitError> {
    if hidden_committed(wt, base).is_empty() {
        return Ok(false);
    }
    ok(wt, &["reset", "-q", "--soft", &merge_base(wt, base)])?;
    Ok(true)
}

pub fn commit_all(wt: &Path, msg: &str) -> Result<Option<String>, GitError> {
    add_all(wt)?;
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

pub fn rebase_continue(wt: &Path) -> Result<(), GitError> {
    add_all(wt)?;
    let o = run(wt, &["rebase", "--continue"])?;
    if o.ok {
        Ok(())
    } else if rebase_in_progress(wt) {
        Err(GitError::Conflict(format!("{}\n{}", o.stdout, o.stderr).trim().to_string()))
    } else {
        Err(GitError::Other(format!("git rebase --continue: {}", o.stderr)))
    }
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
        let o = run(&dir, &["merge", "--ff-only", "-q", new])?;
        if !o.ok {
            return Err(if tip_is_old() {
                GitError::AdvanceBlocked(format!("merge --ff-only refused: {}", o.stderr))
            } else {
                moved()
            });
        }
    } else {
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
    if o.code == Some(1) {
        return Err(GitError::Conflict(format!("{}\n{}", o.stdout, o.stderr).trim().to_string()));
    }
    if !o.ok {
        return Err(GitError::Other(format!("git merge-tree: {}", o.stderr)));
    }
    let tree = o.stdout.lines().next().unwrap_or("").to_string();
    let subject = ok(repo, &["log", "-1", "--format=%s", sha])?;
    let msg = format!("Revert \"{subject}\"\n\nThis reverts commit {sha}.");
    let new = ok(repo, &["commit-tree", &tree, "-p", &old, "-m", &msg])?;
    advance(repo, base, &old, &new)?;
    Ok(new)
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

/// "open" | "merged" | "closed"
pub fn pr_state(repo: &Path, url: &str) -> Result<String, GitError> {
    Ok(gh(repo, &["pr", "view", url, "--json", "state", "-q", ".state"])?.to_lowercase())
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
