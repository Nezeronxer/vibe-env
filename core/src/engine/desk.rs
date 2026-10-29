//! Terminal agents (PTY + the env's CLI in a folder): their own copy of a project, the list of what they changed
//! and the hand-over of their branch to the base. A copy is made and removed like a task's, a merge goes through
//! the same queue lock and the same `advance` as the engine's own. Errors are short English keys.
use super::*;
use crate::git::desk::{AgentChanges, Ctx};
use std::path::Path;

/// Branches of terminal agents: `ns/term-<short id>-<slug>`. Only these are touched here.
const TERM_BRANCH: &str = "ns/term-";
const GITATTRIBUTES: &str = ".gitattributes";

fn gf(e: git::GitError) -> String {
    format!("git_failed: {e}")
}

impl Engine {
    /// A separate copy of the project for a terminal agent: branch `ns/term-<short id>-<slug of name>` from the
    /// project's base, in the folder of the task copies, with the project's `copy_files` (.env ...) put in and kept out
    /// of every commit. Err: `project_not_found`, `not_a_repo`, `worktree_failed: <why>`.
    pub fn agent_worktree(&self, project_id: &str, name: &str) -> Result<(PathBuf, String), String> {
        let p = self.project(&project_id.to_string()).ok_or("project_not_found")?;
        if !git::repo_info(&p.path).is_repo {
            return Err("not_a_repo".into());
        }
        let short = uuid::Uuid::new_v4().simple().to_string()[..8].to_string();
        let branch = format!("{TERM_BRANCH}{short}-{}", util::slug(name));
        let wt = self.add_wt(&p.path, &self.worktrees_dir, &format!("term-{short}"), &branch, &p.base_branch).map_err(|e| format!("worktree_failed: {e}"))?;
        let mut copied = util::copy_files(&p.path, &wt, &p.copy_files);
        copied.push(practices::APP_DIR.to_string());
        // a `.gitattributes` the base lacks comes from the person's own CLI hooks (graphify writes one into every folder
        // a session starts in): it is the machine's, not the agent's work, so it is never listed or merged
        if !wt.join(GITATTRIBUTES).exists() {
            copied.push(GITATTRIBUTES.to_string());
        }
        if let Err(e) = git::hide_untracked(&wt, &copied) {
            let _ = self.rm_wt(&p.path, &wt, true);
            return Err(format!("worktree_failed: {e}"));
        }
        Ok((wt, branch))
    }

    /// Removes a copy made by `agent_worktree`. Err: `not_a_repo`, `not_own_worktree` (not the root of a linked work tree
    /// under the app's copies on an `ns/term-*` branch), `uncommitted_changes` (without `force`). Ok(true) = the branch
    /// went too; it stays (Ok(false)) while it holds commits the base lacks, unless `force`.
    pub fn agent_worktree_remove(&self, path: &Path, force: bool) -> Result<bool, String> {
        let ctx = Ctx::open(path).ok_or("not_a_repo")?;
        if ctx.root != ctx.cwd || !self.is_own(&ctx) {
            return Err("not_own_worktree".into());
        }
        let repo = ctx.repo_dir().ok_or("not_own_worktree")?;
        if !force && !ctx.changes()?.0.is_empty() {
            return Err("uncommitted_changes".into());
        }
        let branch = ctx.branch().ok_or("not_own_worktree")?;
        let delete = force || self.base_of(&ctx).is_some_and(|b| git::desk::branch_merged(&repo, &b, &branch));
        if force {
            self.rm_wt(&repo, &ctx.root, delete).map_err(gf)?;
        } else {
            // without --force: git refuses work written after the check above instead of deleting it with the copy
            self.with_git(&repo, || git::remove_worktree_careful(&repo, &ctx.root, delete)).map_err(|e| match e {
                git::GitError::AdvanceBlocked(_) => "uncommitted_changes".to_string(),
                e => gf(e),
            })?;
        }
        Ok(delete)
    }

    /// The folder is the root of a copy this engine made for a terminal agent.
    fn is_own(&self, ctx: &Ctx) -> bool {
        let Ok(wts) = self.worktrees_dir.canonicalize() else { return false };
        ctx.root.starts_with(&wts) && ctx.root.join(".git").is_file() && ctx.branch().is_some_and(|b| b.starts_with(TERM_BRANCH))
    }

    fn project_at(&self, repo: &Path) -> Option<Project> {
        lk(&self.store).projects().into_iter().find(|p| p.path.canonicalize().ok().as_deref() == Some(repo))
    }

    /// The base branch of the project the folder belongs to, else what git itself calls the base.
    fn base_of(&self, ctx: &Ctx) -> Option<String> {
        ctx.repo_dir().and_then(|r| self.project_at(&r)).map(|p| p.base_branch).or_else(|| git::repo_info(&ctx.root).base_branch)
    }

    /// The folder for the changes panel: an agent's own copy counts from where it left the base, so the panel lists
    /// everything a merge would take, the agent's own commits too; any other folder counts from HEAD.
    fn panel(&self, cwd: &Path) -> Option<(Ctx, bool)> {
        let ctx = Ctx::open(cwd)?;
        let own = self.is_own(&ctx);
        let base = own.then(|| self.base_of(&ctx)).flatten();
        Some(match base {
            Some(b) => (ctx.from(&b), own),
            None => (ctx, own),
        })
    }

    /// `agent_changes`: `repo: false` for a folder that is no git work tree.
    pub fn agent_changes(&self, cwd: &Path) -> Result<AgentChanges, String> {
        let Some((ctx, own)) = self.panel(cwd) else { return Ok(AgentChanges::default()) };
        let (files, truncated) = ctx.changes()?;
        Ok(AgentChanges { repo: true, branch: ctx.branch(), base: self.base_of(&ctx), own_worktree: own, files, truncated })
    }

    /// `(text, truncated)` of one file of the panel. Err: `not_a_repo`, `bad_path`, `git_failed: <why>`.
    pub fn agent_file_diff(&self, cwd: &Path, file: &str) -> Result<(String, bool), String> {
        self.panel(cwd).ok_or("not_a_repo")?.0.file_diff(file)
    }

    /// Puts one file of the panel back. Err: `not_a_repo`, `bad_path`, `git_failed: <why>`.
    pub fn agent_revert_file(&self, cwd: &Path, file: &str) -> Result<(), String> {
        self.panel(cwd).ok_or("not_a_repo")?.0.revert_file(file)
    }

    /// Commits everything of a terminal agent's copy on its branch (the repository's own user; keys and copied files
    /// stay out) and moves the project's base forward to it, fast-forward only, one merge at a time per project.
    /// Ok((merged, sha)): `sha` is the base's new tip, or its tip when there was nothing to merge. Err: `not_a_repo`,
    /// `not_own_worktree`, `project_not_found`, `rebase_in_progress`, `protected_file_committed: <paths>`,
    /// `base_dirty` (the main copy has uncommitted changes), `merge_conflict: <files>` (rebase onto a moved base failed and was undone),
    /// `not_fast_forward` (the base moved again meanwhile), `base_missing`.
    pub fn agent_merge(&self, cwd: &Path, message: &str) -> Result<(bool, String), String> {
        let ctx = Ctx::open(cwd).ok_or("not_a_repo")?;
        if !self.is_own(&ctx) {
            return Err("not_own_worktree".into());
        }
        let repo = ctx.repo_dir().ok_or("not_own_worktree")?;
        let p = self.project_at(&repo).ok_or("project_not_found")?;
        let (wt, base) = (&ctx.root, p.base_branch.as_str());
        if git::rebase_in_progress(wt) {
            return Err("rebase_in_progress".into());
        }
        let msg = Some(message.trim()).filter(|m| !m.is_empty()).unwrap_or("Terminal agent changes");
        self.with_git(wt, || {
            // a key or a copied file the agent itself committed is folded back out first, as for a task; the fold keeps
            // the agent's own commit messages (before, the base got the task text cut at 40 characters)
            let own = git::own_messages(wt, base);
            let folded = git::uncommit_hidden(wt, base, true).map_err(gf)?;
            git::commit_all(wt, if folded && !own.is_empty() { &own } else { msg }, true).map_err(gf)?;
            let keys = git::protected_committed(wt, base).map_err(gf)?;
            if keys.is_empty() { Ok(()) } else { Err(format!("protected_file_committed: {}", keys.join(", "))) }
        })?;
        let lock = self.merge_lock(&p.id);
        let _one = lock.lock().unwrap_or_else(|e| e.into_inner());
        match self.with_git(&repo, || git::desk::fast_forward_base(&repo, base, wt)) {
            // the base moved on while the agent worked: its branch is the app's own, so it is rebased onto the base (a
            // conflict is undone and named) and then goes in by fast-forward as before
            Err(e) if e == "not_fast_forward" => {
                self.with_git(wt, || git::desk::rebase_onto_base(wt, base))?;
                self.with_git(&repo, || git::desk::fast_forward_base(&repo, base, wt))
            }
            r => r,
        }
    }
}
