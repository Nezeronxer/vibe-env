//! Terminal agents' desk: their own copy, the list of changes, one file's diff and revert, the hand-over to the base.
//! Temporary repositories only (the harness of the engine tests).
mod common;
use common::*;
use nightshift_core::engine::Engine;
use nightshift_core::git::desk::{Change, Ctx};
use nightshift_core::model::Project;
use nightshift_core::notify::RecordingNotifier;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct Rig {
    env: Env,
    e: Arc<Engine>,
    p: Project,
}

fn rig() -> Rig {
    let env = Env::new();
    // the machine's own commit hooks (they may write files into a repository) have no place in these tests
    g(&env.repo, &["config", "core.hooksPath", "/dev/null"]);
    let p = env.project(|_| {});
    let e = env.engine(env.store(&p, |_| {}), vec![], Arc::new(RecordingNotifier::default()), None);
    Rig { env, e, p }
}

impl Rig {
    fn term(&self, name: &str) -> (PathBuf, String) {
        self.e.agent_worktree(&self.p.id, name).unwrap()
    }
    fn files(&self, cwd: &Path) -> Vec<(String, String, u32, u32)> {
        self.e.agent_changes(cwd).unwrap().files.into_iter().map(|c| (c.path, c.status, c.add, c.del)).collect()
    }
}

fn row(p: &str, s: &str, a: u32, d: u32) -> (String, String, u32, u32) {
    (p.into(), s.into(), a, d)
}

#[test]
fn a_worktree_is_on_a_term_branch_in_the_task_copies_folder_with_the_env_files_kept_out() {
    let r = rig();
    fs::write(r.env.repo.join(".env"), "KEY=1\n").unwrap();
    let (path, branch) = r.term("Fix the login page");
    assert!(branch.starts_with("ns/term-") && branch.ends_with("-fix-the-login-page"), "{branch}");
    assert_eq!(branch.split('-').nth(1).map(str::len), Some(8), "short id: {branch}");
    assert!(path.canonicalize().unwrap().starts_with(r.env.d.path().join("wts").canonicalize().unwrap()), "{path:?}");
    assert_eq!(g(&path, &["symbolic-ref", "--short", "HEAD"]), branch);
    assert_eq!(fs::read_to_string(path.join(".env")).unwrap(), "KEY=1\n", "copied like for a task");
    let c = r.e.agent_changes(&path).unwrap();
    assert!(c.repo && c.own_worktree && c.files.is_empty(), "the copied file is no change: {c:?}");
    assert_eq!((c.branch.as_deref(), c.base.as_deref()), (Some(branch.as_str()), Some("main")));
    // two agents with one name get two copies
    assert_ne!(r.term("Fix the login page").1, branch);
    // a name with no latin letters still makes a branch
    assert!(r.term("Починить").1.ends_with("-task"));
    assert_eq!(r.e.agent_worktree("nope", "x").unwrap_err(), "project_not_found");
}

#[test]
fn changes_list_modified_added_deleted_renamed_and_untracked_files_with_line_counts() {
    let r = rig();
    let repo = &r.env.repo;
    fs::write(repo.join("b.txt"), "x\n").unwrap();
    fs::write(repo.join("c.txt"), "gone\n").unwrap();
    fs::write(repo.join(".gitignore"), "*.log\n").unwrap();
    g(repo, &["add", "-A"]);
    g(repo, &["commit", "-q", "-m", "more"]);
    fs::write(repo.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    fs::remove_file(repo.join("c.txt")).unwrap();
    g(repo, &["mv", "b.txt", "b2.txt"]);
    fs::write(repo.join("staged.txt"), "s\n").unwrap();
    g(repo, &["add", "staged.txt"]);
    fs::write(repo.join("new file.txt"), "1\n2\n3").unwrap();
    fs::write(repo.join("Ünï код.txt"), "u\n").unwrap();
    fs::write(repo.join("bin.dat"), [0u8, 1, 2, 0]).unwrap();
    fs::write(repo.join("noise.log"), "ignored\n").unwrap();
    let c = r.e.agent_changes(repo).unwrap();
    assert!(c.repo && !c.own_worktree && !c.truncated);
    assert_eq!((c.branch.as_deref(), c.base.as_deref()), (Some("main"), Some("main")));
    let got: Vec<_> = c.files.iter().map(|c: &Change| (c.path.as_str(), c.status.as_str(), c.add, c.del)).collect();
    assert_eq!(
        got,
        vec![
            ("a.txt", "M", 2, 0),
            ("b2.txt", "R", 0, 0),
            ("bin.dat", "?", 0, 0),
            ("c.txt", "D", 0, 1),
            ("new file.txt", "?", 3, 0),
            ("staged.txt", "A", 1, 0),
            ("Ünï код.txt", "?", 1, 0),
        ],
        "the .gitignore is committed above, noise.log is ignored"
    );
}

#[test]
fn changes_of_a_subfolder_are_relative_to_it_and_a_plain_folder_is_no_repo() {
    let r = rig();
    let repo = &r.env.repo;
    fs::create_dir(repo.join("sub")).unwrap();
    fs::write(repo.join("sub/s.txt"), "s\n").unwrap();
    g(repo, &["add", "-A"]);
    g(repo, &["commit", "-q", "-m", "sub"]);
    fs::write(repo.join("sub/s.txt"), "s\nt\n").unwrap();
    fs::write(repo.join("sub/n.txt"), "n\n").unwrap();
    fs::write(repo.join("a.txt"), "changed\n").unwrap();
    assert_eq!(r.files(&repo.join("sub")), vec![row("n.txt", "?", 1, 0), row("s.txt", "M", 1, 0)]);
    assert!(r.e.agent_file_diff_text(&repo.join("sub"), "s.txt").contains("+t"));
    let plain = r.env.d.path().join("plain");
    fs::create_dir(&plain).unwrap();
    let c = r.e.agent_changes(&plain).unwrap();
    assert!(!c.repo && c.files.is_empty() && c.branch.is_none());
    assert!(!r.e.agent_changes(&r.env.d.path().join("missing")).unwrap().repo);
}

trait DiffText {
    fn agent_file_diff_text(&self, cwd: &Path, file: &str) -> String;
}
impl DiffText for Arc<Engine> {
    fn agent_file_diff_text(&self, cwd: &Path, file: &str) -> String {
        Ctx::open(cwd).unwrap().file_diff(file).unwrap().0
    }
}

#[test]
fn a_file_diff_is_unified_for_tracked_and_untracked_files_cut_and_scrubbed() {
    let r = rig();
    let repo = &r.env.repo;
    fs::write(repo.join("a.txt"), "one\ntwo\n").unwrap();
    let ctx = Ctx::open(repo).unwrap();
    let (d, cut) = ctx.file_diff("a.txt").unwrap();
    assert!(!cut && d.contains("--- a/a.txt") && d.contains("+two"), "{d}");
    fs::write(repo.join("n.txt"), format!("API_TOKEN=ghp_{}\nplain\n", "a".repeat(36))).unwrap();
    let (d, _) = ctx.file_diff("n.txt").unwrap();
    assert!(d.contains("new file mode") && d.contains("+plain"), "{d}");
    assert!(!d.contains("ghp_aaaa"), "scrubbed: {d}");
    fs::write(repo.join("big.txt"), "0123456789abcdef\n".repeat(40_000)).unwrap();
    let (d, cut) = ctx.file_diff("big.txt").unwrap();
    assert!(cut && d.len() <= 262_144 + 64, "{} bytes", d.len());
    fs::write(repo.join("bin.dat"), [0u8, 1, 2, 0]).unwrap();
    assert!(ctx.file_diff("bin.dat").unwrap().0.contains("Binary files"));
}

#[test]
fn paths_that_leave_the_folder_are_refused_by_diff_and_revert() {
    let r = rig();
    let repo = &r.env.repo;
    let outside = r.env.d.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("x.txt"), "precious\n").unwrap();
    fs::write(r.env.d.path().join("o.txt"), "precious\n").unwrap();
    symlink(&outside, repo.join("link")).unwrap();
    fs::create_dir(repo.join("sub")).unwrap();
    let ctx = Ctx::open(repo).unwrap();
    let sub = Ctx::open(&repo.join("sub")).unwrap();
    for bad in ["../o.txt", "/etc/passwd", "link/x.txt", "sub/../../o.txt", "", "a\0b"] {
        assert_eq!(ctx.file_diff(bad).unwrap_err(), "bad_path", "{bad:?}");
        assert_eq!(ctx.revert_file(bad).unwrap_err(), "bad_path", "{bad:?}");
    }
    assert_eq!(sub.revert_file("../a.txt").unwrap_err(), "bad_path", "above the folder asked about");
    assert_eq!(fs::read_to_string(outside.join("x.txt")).unwrap(), "precious\n");
    assert_eq!(fs::read_to_string(r.env.d.path().join("o.txt")).unwrap(), "precious\n");
    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "one\n");
}

#[test]
fn revert_restores_tracked_files_removes_untracked_ones_and_undoes_a_rename() {
    let r = rig();
    let repo = &r.env.repo;
    fs::write(repo.join("b.txt"), "b\n").unwrap();
    g(repo, &["add", "-A"]);
    g(repo, &["commit", "-q", "-m", "b"]);
    let ctx = Ctx::open(repo).unwrap();
    // modified
    fs::write(repo.join("a.txt"), "changed\n").unwrap();
    ctx.revert_file("a.txt").unwrap();
    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "one\n");
    // deleted
    fs::remove_file(repo.join("a.txt")).unwrap();
    ctx.revert_file("a.txt").unwrap();
    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "one\n");
    // staged and edited
    fs::write(repo.join("a.txt"), "staged\n").unwrap();
    g(repo, &["add", "a.txt"]);
    fs::write(repo.join("a.txt"), "staged and edited\n").unwrap();
    ctx.revert_file("a.txt").unwrap();
    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "one\n");
    assert_eq!(g(repo, &["status", "--porcelain"]), "");
    // untracked, in a folder; added to the index only
    fs::create_dir(repo.join("d")).unwrap();
    fs::write(repo.join("d/new.txt"), "n\n").unwrap();
    ctx.revert_file("d/new.txt").unwrap();
    assert!(!repo.join("d/new.txt").exists());
    fs::write(repo.join("added.txt"), "n\n").unwrap();
    g(repo, &["add", "added.txt"]);
    ctx.revert_file("added.txt").unwrap();
    assert!(!repo.join("added.txt").exists());
    assert_eq!(g(repo, &["status", "--porcelain"]), "", "nothing is left to show (an empty folder is no change)");
    // a rename: both names come back as they were
    g(repo, &["mv", "b.txt", "b2.txt"]);
    ctx.revert_file("b2.txt").unwrap();
    assert!(repo.join("b.txt").exists() && !repo.join("b2.txt").exists());
    assert_eq!(g(repo, &["status", "--porcelain", "--untracked-files=no"]), "");
}

#[test]
fn revert_leaves_copied_env_files_of_an_agents_copy_alone() {
    let r = rig();
    fs::write(r.env.repo.join(".env"), "KEY=1\n").unwrap();
    let (path, _) = r.term("env");
    let ctx = Ctx::open(&path).unwrap();
    assert_eq!(ctx.revert_file(".env").unwrap_err(), "bad_path");
    assert_eq!(ctx.file_diff(".env").unwrap_err(), "bad_path");
    assert!(path.join(".env").exists());
}

#[test]
fn merge_commits_the_copy_and_fast_forwards_the_base_without_keys_or_ai_marks() {
    let r = rig();
    fs::write(r.env.repo.join(".env"), "KEY=1\n").unwrap();
    let (path, branch) = r.term("feature");
    fs::write(path.join("f.txt"), "feature\n").unwrap();
    fs::write(path.join("k.pem"), "secret\n").unwrap();
    let before = r.env.main_count();
    let (merged, sha) = r.e.agent_merge(&path, "Add feature").unwrap();
    assert!(merged);
    assert_eq!(sha, g(&r.env.repo, &["rev-parse", "main"]));
    assert_eq!(r.env.main_count(), before + 1);
    assert_eq!(r.env.main_file("f.txt"), "feature");
    assert!(!g(&r.env.repo, &["ls-tree", "-r", "--name-only", "main"]).contains("k.pem"), "a new key stays out");
    assert!(!g(&r.env.repo, &["ls-tree", "-r", "--name-only", "main"]).contains(".env"));
    assert_eq!(g(&r.env.repo, &["log", "-1", "--format=%an|%ae|%B"]).trim(), "T|t@example.com|Add feature");
    assert_eq!(g(&r.env.repo, &["status", "--porcelain", "--untracked-files=no"]), "", "the main copy is clean and has the file");
    assert!(r.env.repo.join("f.txt").exists());
    assert_eq!(g(&path, &["symbolic-ref", "--short", "HEAD"]), branch, "the agent goes on in its copy");
    // nothing new: no second commit
    let (merged, sha2) = r.e.agent_merge(&path, "again").unwrap();
    assert!(!merged && sha2 == sha);
    assert_eq!(r.env.main_count(), before + 1);
    // an empty message gets a plain one
    fs::write(path.join("g.txt"), "g\n").unwrap();
    r.e.agent_merge(&path, "  ").unwrap();
    assert_eq!(g(&r.env.repo, &["log", "-1", "--format=%s"]), "Terminal agent changes");
}

#[test]
fn merge_refuses_a_dirty_base_and_changes_nothing_there() {
    let r = rig();
    let (path, _) = r.term("x");
    fs::write(path.join("f.txt"), "f\n").unwrap();
    fs::write(r.env.repo.join("a.txt"), "dirty\n").unwrap();
    assert_eq!(r.e.agent_merge(&path, "m").unwrap_err(), "base_dirty");
    assert_eq!(fs::read_to_string(r.env.repo.join("a.txt")).unwrap(), "dirty\n", "the person's edit is untouched");
    assert_eq!(r.env.main_count(), 1);
    // the changes are safe on the agent's branch, and a retry after the person committed is fine
    assert_eq!(g(&path, &["status", "--porcelain"]), "");
    g(&r.env.repo, &["checkout", "-q", "--", "a.txt"]);
    assert!(r.e.agent_merge(&path, "m").unwrap().0);
}

#[test]
fn merge_rebases_onto_a_base_that_moved_and_undoes_a_conflicting_rebase() {
    let r = rig();
    let (path, branch) = r.term("x");
    fs::write(path.join("h.txt"), "h\n").unwrap();
    // the base moves on while the agent works, in another file: the branch is rebased, then goes in by fast-forward
    fs::write(r.env.repo.join("other.txt"), "o\n").unwrap();
    g(&r.env.repo, &["add", "-A"]);
    g(&r.env.repo, &["commit", "-q", "-m", "other"]);
    let moved = g(&r.env.repo, &["rev-parse", "main"]);
    let (merged, sha) = r.e.agent_merge(&path, "Add h").unwrap();
    assert!(merged);
    assert_eq!(g(&r.env.repo, &["rev-parse", "main"]), sha);
    assert_eq!(g(&r.env.repo, &["rev-parse", "main~1"]), moved, "on top of the base, no merge commit");
    assert_eq!((r.env.main_file("h.txt"), r.env.main_file("other.txt")), ("h".into(), "o".into()));
    assert_eq!(g(&path, &["rev-parse", "HEAD"]), sha);
    assert_eq!(g(&path, &["symbolic-ref", "--short", "HEAD"]), branch);
    // both sides change one line: the rebase is undone, the agent's commit stays on its branch, the base is untouched
    fs::write(path.join("a.txt"), "agent\n").unwrap();
    fs::write(r.env.repo.join("a.txt"), "person\n").unwrap();
    g(&r.env.repo, &["commit", "-q", "-am", "person"]);
    let tip = g(&r.env.repo, &["rev-parse", "main"]);
    assert_eq!(r.e.agent_merge(&path, "Agent a").unwrap_err(), "merge_conflict: a.txt");
    assert_eq!(g(&r.env.repo, &["rev-parse", "main"]), tip);
    assert_eq!(g(&path, &["log", "-1", "--format=%s"]), "Agent a");
    assert_eq!(g(&path, &["status", "--porcelain"]), "", "no rebase left half-way");
    assert!(!nightshift_core::git::rebase_in_progress(&path));
}

#[test]
fn merge_is_for_the_apps_term_branches_only() {
    let r = rig();
    // the main copy, a branch the person made elsewhere, a work tree on a term-like branch outside the app's folder
    assert_eq!(r.e.agent_merge(&r.env.repo, "m").unwrap_err(), "not_own_worktree");
    let theirs = r.env.d.path().join("theirs");
    g(&r.env.repo, &["worktree", "add", "-q", "-b", "feature", theirs.to_str().unwrap()]);
    fs::write(theirs.join("t.txt"), "t\n").unwrap();
    assert_eq!(r.e.agent_merge(&theirs, "m").unwrap_err(), "not_own_worktree");
    let fake = r.env.d.path().join("fake");
    g(&r.env.repo, &["worktree", "add", "-q", "-b", "ns/term-deadbeef-x", fake.to_str().unwrap()]);
    fs::write(fake.join("t.txt"), "t\n").unwrap();
    assert_eq!(r.e.agent_merge(&fake, "m").unwrap_err(), "not_own_worktree");
    // a task's own copy (ns/<id>) is not a terminal agent's
    let (path, _) = r.term("ok");
    g(&path, &["checkout", "-q", "--detach"]);
    assert_eq!(r.e.agent_merge(&path, "m").unwrap_err(), "not_own_worktree");
    assert_eq!(r.env.main_count(), 1);
    assert_eq!(r.e.agent_merge(&r.env.d.path().join("nope"), "m").unwrap_err(), "not_a_repo");
    assert!(!r.e.agent_changes(&theirs).unwrap().own_worktree);
}

#[test]
fn remove_needs_force_for_uncommitted_work_and_touches_only_term_copies() {
    let r = rig();
    let (path, branch) = r.term("rm");
    fs::write(path.join("f.txt"), "f\n").unwrap();
    assert_eq!(r.e.agent_worktree_remove(&path, false).unwrap_err(), "uncommitted_changes");
    assert!(path.exists());
    // a clean copy goes with its branch
    let (clean, clean_branch) = r.term("clean");
    assert_eq!(r.e.agent_worktree_remove(&clean, false), Ok(true));
    assert!(!clean.exists());
    assert!(g(&r.env.repo, &["branch", "--list", &clean_branch]).is_empty());
    // committed but not merged: the copy goes, the branch (the work) stays
    fs::remove_file(path.join("f.txt")).unwrap();
    fs::write(path.join("g.txt"), "g\n").unwrap();
    g(&path, &["add", "-A"]);
    g(&path, &["commit", "-q", "-m", "wip"]);
    assert_eq!(r.e.agent_worktree_remove(&path, false), Ok(false));
    assert!(!path.exists() && !g(&r.env.repo, &["branch", "--list", &branch]).is_empty());
    // force drops uncommitted work and the branch
    let (forced, forced_branch) = r.term("forced");
    fs::write(forced.join("f.txt"), "f\n").unwrap();
    assert_eq!(r.e.agent_worktree_remove(&forced, true), Ok(true));
    assert!(g(&r.env.repo, &["branch", "--list", &forced_branch]).is_empty());
    // never the main copy, a person's work tree, a sub-folder
    assert_eq!(r.e.agent_worktree_remove(&r.env.repo, true).unwrap_err(), "not_own_worktree");
    let theirs = r.env.d.path().join("theirs");
    g(&r.env.repo, &["worktree", "add", "-q", "-b", "ns/term-aaaaaaaa-x", theirs.to_str().unwrap()]);
    assert_eq!(r.e.agent_worktree_remove(&theirs, true).unwrap_err(), "not_own_worktree");
    let (own, _) = r.term("sub");
    fs::create_dir(own.join("sub")).unwrap();
    assert_eq!(r.e.agent_worktree_remove(&own.join("sub"), true).unwrap_err(), "not_own_worktree");
    assert!(own.exists() && theirs.exists() && r.env.repo.join("a.txt").exists());
    assert_eq!(g(&r.env.repo, &["branch", "--list", "main"]).trim_start_matches(['*', ' ']), "main");
}

#[test]
fn a_repository_with_no_commit_yet_lists_its_files() {
    let r = rig();
    let fresh = r.env.d.path().join("fresh");
    fs::create_dir(&fresh).unwrap();
    g(&fresh, &["init", "-q", "-b", "main"]);
    g(&fresh, &["config", "core.hooksPath", "/dev/null"]);
    fs::write(fresh.join("s.txt"), "s\nt\n").unwrap();
    g(&fresh, &["add", "s.txt"]);
    fs::write(fresh.join("u.txt"), "u\n").unwrap();
    assert_eq!(r.files(&fresh), vec![row("s.txt", "A", 2, 0), row("u.txt", "?", 1, 0)]);
    let ctx = Ctx::open(&fresh).unwrap();
    assert!(ctx.file_diff("s.txt").unwrap().0.contains("+t"));
    ctx.revert_file("s.txt").unwrap();
    ctx.revert_file("u.txt").unwrap();
    assert_eq!(r.files(&fresh), vec![]);
}

#[test]
fn the_panel_of_an_agents_copy_lists_its_own_commits_and_diffs_and_reverts_them() {
    let r = rig();
    let (path, _) = r.term("sneaky");
    fs::write(path.join("gone.txt"), "keep me\n").unwrap();
    g(&path, &["add", "gone.txt"]);
    g(&path, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "add"]);
    r.e.agent_merge(&path, "base has gone.txt").unwrap();
    // the agent deletes a file and commits that itself, then leaves one harmless edit
    g(&path, &["rm", "-q", "gone.txt"]);
    g(&path, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "sneaky"]);
    fs::write(path.join("readme.md"), "hi\n").unwrap();
    assert_eq!(r.files(&path), vec![row("gone.txt", "D", 0, 1), row("readme.md", "?", 1, 0)], "all that a merge takes");
    assert!(r.e.agent_file_diff(&path, "gone.txt").unwrap().0.contains("-keep me"));
    r.e.agent_revert_file(&path, "gone.txt").unwrap();
    assert_eq!(fs::read_to_string(path.join("gone.txt")).unwrap(), "keep me\n", "put back as the base has it");
    assert_eq!(r.files(&path), vec![row("readme.md", "?", 1, 0)]);
    // a folder that is not an agent's copy still counts from HEAD
    assert!(r.files(&r.env.repo).is_empty());
}

#[test]
fn a_hidden_file_is_refused_in_any_case_of_its_name() {
    let r = rig();
    fs::write(r.env.repo.join(".env"), "KEY=1\n").unwrap();
    let (path, _) = r.term("env");
    assert_eq!(r.e.agent_revert_file(&path, ".ENV").unwrap_err(), "bad_path");
    assert_eq!(r.e.agent_file_diff(&path, ".Env").unwrap_err(), "bad_path");
    assert!(path.join(".env").exists());
}

#[test]
fn reading_the_changes_never_runs_the_fsmonitor_of_the_repository() {
    let r = rig();
    let (path, _) = r.term("fsm");
    let mark = r.env.d.path().join("fsm-ran");
    let script = r.env.d.path().join("fsm.sh");
    fs::write(&script, format!("#!/bin/sh\ntouch '{}'\n", mark.display())).unwrap();
    fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    g(&path, &["config", "core.fsmonitor", &script.to_string_lossy()]);
    let _ = r.e.agent_changes(&path).unwrap();
    let _ = r.e.agent_file_diff(&path, "x.txt");
    assert!(!mark.exists(), "the fsmonitor of the repository ran");
}

#[test]
fn a_gitattributes_the_base_lacks_is_kept_out_of_the_panel_and_the_merge() {
    let r = rig();
    let (path, _) = r.term("hooks");
    fs::write(path.join(".gitattributes"), "graphify-out/graph.json merge=graphify\n").unwrap();
    fs::write(path.join("f.txt"), "f\n").unwrap();
    assert_eq!(r.files(&path), vec![row("f.txt", "?", 1, 0)]);
    r.e.agent_merge(&path, "f").unwrap();
    assert!(!g(&r.env.repo, &["ls-tree", "-r", "--name-only", "main"]).contains(".gitattributes"));
    // the agent committed the stray file itself: it is folded out and the agent's message goes in, not the task text
    let (c, _) = r.term("commit");
    fs::write(c.join(".gitattributes"), "graphify-out/graph.json merge=graphify\n").unwrap();
    fs::write(c.join("h.txt"), "h\n").unwrap();
    g(&c, &["add", "-A"]);
    g(&c, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "h.txt: the agent's own words"]);
    r.e.agent_merge(&c, "Task text cut at fort").unwrap();
    assert_eq!(g(&r.env.repo, &["log", "-1", "--format=%s"]).trim(), "h.txt: the agent's own words");
    assert!(!g(&r.env.repo, &["ls-tree", "-r", "--name-only", "main"]).contains(".gitattributes"));
    // a project that keeps its own .gitattributes still sees the agent's edits of it
    fs::write(r.env.repo.join(".gitattributes"), "*.png binary\n").unwrap();
    g(&r.env.repo, &["add", ".gitattributes"]);
    g(&r.env.repo, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "attrs"]);
    let (own, _) = r.term("attrs");
    fs::write(own.join(".gitattributes"), "*.png binary\n*.jpg binary\n").unwrap();
    assert_eq!(r.files(&own), vec![row(".gitattributes", "M", 1, 0)]);
}

// ---- review 30.10.2026: what the panel and the merge must not destroy ----

#[test]
fn revert_refuses_a_nested_git_repository_and_keeps_it() {
    let r = rig();
    let n = r.env.repo.join("vendor/lib");
    fs::create_dir_all(&n).unwrap();
    g(&n, &["init", "-q"]);
    fs::write(n.join("history.txt"), "committed work\n").unwrap();
    g(&n, &["add", "-A"]);
    g(&n, &["-c", "user.name=x", "-c", "user.email=x@x", "commit", "-q", "-m", "c"]);
    let c = r.e.agent_changes(&r.env.repo).unwrap();
    let f = c.files.iter().find(|f| f.path.contains("vendor")).expect("the panel lists it").path.clone();
    assert!(r.e.agent_revert_file(&r.env.repo, &f).unwrap_err().starts_with("nested_repo"));
    assert!(n.join(".git").exists() && n.join("history.txt").exists(), "the clone stays with its history");
}

#[test]
fn revert_of_a_rename_does_not_overwrite_a_new_file_at_the_old_name() {
    let r = rig();
    let repo = &r.env.repo;
    fs::write(repo.join("b.txt"), "old b\n").unwrap();
    g(repo, &["add", "-A"]);
    g(repo, &["commit", "-q", "-m", "b"]);
    g(repo, &["mv", "b.txt", "b2.txt"]);
    fs::write(repo.join("b.txt"), "NEW WORK at old name\n").unwrap();
    assert!(r.e.agent_revert_file(repo, "b2.txt").unwrap_err().starts_with("path_taken"));
    assert_eq!(fs::read_to_string(repo.join("b.txt")).unwrap(), "NEW WORK at old name\n");
}

#[test]
fn merge_refuses_to_overwrite_an_ignored_file_of_the_person() {
    let r = rig();
    let repo = &r.env.repo;
    fs::write(repo.join(".gitignore"), "local.json\n").unwrap();
    g(repo, &["add", "-A"]);
    g(repo, &["commit", "-q", "-m", "gi"]);
    let (path, _) = r.term("cfg");
    fs::write(repo.join("local.json"), "{\"my\":\"settings\"}\n").unwrap();
    fs::write(path.join(".gitignore"), "").unwrap();
    fs::write(path.join("local.json"), "{\"agent\":1}\n").unwrap();
    let err = r.e.agent_merge(&path, "m").unwrap_err();
    assert!(err.starts_with("base_untracked") && err.contains("local.json"), "{err}");
    assert_eq!(fs::read_to_string(repo.join("local.json")).unwrap(), "{\"my\":\"settings\"}\n");
}

#[test]
fn a_copy_is_removed_without_force_and_git_keeps_work_written_after_the_check() {
    let r = rig();
    fs::write(r.env.repo.join(".env"), "KEY=1\n").unwrap();
    // the app's own copy of .env does not block an ordinary «Удалить копию»
    let (path, _) = r.term("clean");
    assert!(path.join(".env").exists(), "the copy got the project's .env");
    r.e.agent_worktree_remove(&path, false).unwrap();
    assert!(!path.exists());
    // a file that appears after the check (a process of the agent still writing): git refuses, the file stays
    let (path, _) = r.term("late");
    fs::write(path.join("late.txt"), "written after the check\n").unwrap();
    let repo = r.env.repo.clone();
    assert!(matches!(nightshift_core::git::remove_worktree_careful(&repo, &path, false), Err(nightshift_core::git::GitError::AdvanceBlocked(_))));
    assert_eq!(fs::read_to_string(path.join("late.txt")).unwrap(), "written after the check\n");
}
