use nightshift_core::git::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn g(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").current_dir(dir).args(args).output().unwrap();
    assert!(o.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

fn new_repo() -> (TempDir, PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("repo");
    fs::create_dir(&p).unwrap();
    g(&p, &["init", "-q", "-b", "main"]);
    g(&p, &["config", "user.email", "t@example.com"]);
    g(&p, &["config", "user.name", "T"]);
    fs::write(p.join("a.txt"), "one\n").unwrap();
    g(&p, &["add", "-A"]);
    g(&p, &["commit", "-q", "-m", "init"]);
    (d, p)
}

fn wt_for(d: &TempDir, repo: &Path, id: &str, branch: &str) -> PathBuf {
    let wt = create_worktree(repo, &d.path().join("wts"), id, branch, "main").unwrap();
    g(&wt, &["config", "user.email", "t@example.com"]);
    g(&wt, &["config", "user.name", "T"]);
    wt
}

fn count(repo: &Path, rev: &str) -> u32 {
    g(repo, &["rev-list", "--count", rev]).parse().unwrap()
}

fn task_with_change(d: &TempDir, repo: &Path, id: &str, branch: &str, file: &str, body: &str) -> PathBuf {
    let wt = wt_for(d, repo, id, branch);
    fs::write(wt.join(file), body).unwrap();
    assert!(commit_all(&wt, "wip", true).unwrap().is_some());
    wt
}

fn commit_in(dir: &Path, file: &str, body: &str, msg: &str) {
    fs::write(dir.join(file), body).unwrap();
    g(dir, &["add", "-A"]);
    g(dir, &["commit", "-q", "-m", msg]);
}

#[test]
fn worktree_is_created_and_removed_with_its_branch() {
    let (d, repo) = new_repo();
    let wt = wt_for(&d, &repo, "t1", "ns/abc12345-fix");
    assert!(wt.join("a.txt").exists());
    assert_eq!(g(&wt, &["symbolic-ref", "--short", "HEAD"]), "ns/abc12345-fix");
    remove_worktree(&repo, &wt, true).unwrap();
    assert!(!wt.exists());
    assert_eq!(g(&repo, &["branch", "--list", "ns/*"]), "");
}

#[test]
fn remove_worktree_can_keep_the_branch() {
    let (d, repo) = new_repo();
    let wt = wt_for(&d, &repo, "t1", "ns/abc12345-keep");
    remove_worktree(&repo, &wt, false).unwrap();
    assert!(g(&repo, &["branch", "--list", "ns/*"]).contains("ns/abc12345-keep"));
}

#[test]
fn commit_all_returns_none_when_nothing_changed() {
    let (d, repo) = new_repo();
    let wt = wt_for(&d, &repo, "t1", "ns/aaaa1111-x");
    assert_eq!(commit_all(&wt, "nothing", true).unwrap(), None);
    fs::write(wt.join("new.txt"), "x").unwrap();
    let sha = commit_all(&wt, "add new", true).unwrap().expect("sha");
    assert_eq!(sha, g(&wt, &["rev-parse", "HEAD"]));
    assert_eq!(commit_all(&wt, "again", true).unwrap(), None);
}

#[test]
fn rebase_conflict_is_reported_and_can_be_resolved() {
    let (d, repo) = new_repo();
    let wt = wt_for(&d, &repo, "t1", "ns/aaaa1111-x");
    fs::write(wt.join("a.txt"), "task side\n").unwrap();
    commit_all(&wt, "task edit", true).unwrap();
    commit_in(&repo, "a.txt", "main side\n", "main edit");

    let err = rebase_onto(&wt, "main").unwrap_err();
    assert!(matches!(err, GitError::Conflict(_)), "got {err:?}");

    fs::write(wt.join("a.txt"), "resolved\n").unwrap();
    rebase_continue(&wt, true).unwrap();
    assert_eq!(fs::read_to_string(wt.join("a.txt")).unwrap(), "resolved\n");
    assert_eq!(count(&wt, "main..HEAD"), 1);
}

#[test]
fn clean_rebase_moves_branch_onto_new_base() {
    let (d, repo) = new_repo();
    let wt = task_with_change(&d, &repo, "t1", "ns/aaaa1111-x", "t.txt", "t\n");
    commit_in(&repo, "m.txt", "m\n", "main moves");
    rebase_onto(&wt, "main").unwrap();
    assert!(wt.join("m.txt").exists() && wt.join("t.txt").exists());
}

#[test]
fn diff_and_diffstat_describe_the_task_changes() {
    let (d, repo) = new_repo();
    let wt = wt_for(&d, &repo, "t1", "ns/aaaa1111-x");
    fs::write(wt.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    fs::write(wt.join("b.txt"), "x\n").unwrap();
    commit_all(&wt, "c", true).unwrap();
    let mut st = diffstat(&wt, "main");
    st.sort_by(|a, b| a.path.cmp(&b.path));
    // counted by hand: a.txt gets two new lines, b.txt one
    assert_eq!(st, vec![
        FileStat { path: "a.txt".into(), insertions: 2, deletions: 0 },
        FileStat { path: "b.txt".into(), insertions: 1, deletions: 0 },
    ]);
    assert!(diff(&wt, "main", 100_000).contains("+three"));
    assert!(diff(&wt, "main", 20).contains("[diff truncated]"));
}

#[test]
fn squash_gives_exactly_one_commit_with_the_task_message() {
    let (d, repo) = new_repo();
    let wt = wt_for(&d, &repo, "t1", "ns/aaaa1111-x");
    for i in 0..3 {
        fs::write(wt.join(format!("f{i}.txt")), "x").unwrap();
        commit_all(&wt, &format!("step {i}"), true).unwrap();
    }
    assert_eq!(count(&repo, "main"), 1);
    let sha = squash_to_base(&repo, &wt, "main", "Add three files").unwrap();
    assert_eq!(count(&repo, "main"), 2);
    assert_eq!(g(&repo, &["rev-parse", "main"]), sha);
    assert_eq!(g(&repo, &["log", "-1", "--format=%s", "main"]), "Add three files");
    assert_eq!(g(&repo, &["ls-tree", "-r", "--name-only", "main"]), "a.txt\nf0.txt\nf1.txt\nf2.txt");
}

#[test]
fn advance_rule1_human_on_clean_main_gets_fast_forward() {
    let (d, repo) = new_repo();
    let wt = task_with_change(&d, &repo, "t1", "ns/aaaa1111-x", "t.txt", "task\n");
    squash_to_base(&repo, &wt, "main", "Task").unwrap();
    assert_eq!(fs::read_to_string(repo.join("t.txt")).unwrap(), "task\n");
    assert_eq!(g(&repo, &["status", "--porcelain"]), "");
}

#[test]
fn advance_rule2_dirty_main_blocks_and_keeps_humans_edit() {
    let (d, repo) = new_repo();
    let wt = task_with_change(&d, &repo, "t1", "ns/aaaa1111-x", "t.txt", "task\n");
    fs::write(repo.join("a.txt"), "human work in progress\n").unwrap();
    let before = g(&repo, &["rev-parse", "main"]);
    let err = squash_to_base(&repo, &wt, "main", "Task").unwrap_err();
    assert!(matches!(err, GitError::AdvanceBlocked(_)), "got {err:?}");
    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "human work in progress\n");
    assert_eq!(g(&repo, &["rev-parse", "main"]), before);
    assert!(!repo.join("t.txt").exists());
    // after the human commits, rebase + squash go through
    g(&repo, &["commit", "-q", "-am", "human commit"]);
    rebase_onto(&wt, "main").unwrap();
    squash_to_base(&repo, &wt, "main", "Task").unwrap();
    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "human work in progress\n");
}

#[test]
fn advance_rule3_human_on_other_branch_main_moves_and_files_stay() {
    let (d, repo) = new_repo();
    g(&repo, &["checkout", "-q", "-b", "feature"]);
    fs::write(repo.join("a.txt"), "feature edit\n").unwrap();
    fs::write(repo.join("scratch.txt"), "uncommitted\n").unwrap();
    let head_before = g(&repo, &["rev-parse", "HEAD"]);
    let wt = task_with_change(&d, &repo, "t1", "ns/aaaa1111-x", "t.txt", "task\n");
    let sha = squash_to_base(&repo, &wt, "main", "Task").unwrap();
    assert_eq!(g(&repo, &["rev-parse", "main"]), sha);
    assert_eq!(g(&repo, &["symbolic-ref", "--short", "HEAD"]), "feature");
    assert_eq!(g(&repo, &["rev-parse", "HEAD"]), head_before);
    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "feature edit\n");
    assert_eq!(fs::read_to_string(repo.join("scratch.txt")).unwrap(), "uncommitted\n");
    assert!(!repo.join("t.txt").exists());
}

#[test]
fn squash_after_main_moved_asks_for_a_new_rebase() {
    let (d, repo) = new_repo();
    let wt = task_with_change(&d, &repo, "t1", "ns/aaaa1111-x", "t.txt", "task\n");
    commit_in(&repo, "m.txt", "m", "other");
    assert!(matches!(squash_to_base(&repo, &wt, "main", "Task"), Err(GitError::BaseMoved(_))));
}

#[test]
fn revert_undoes_the_squash_commit() {
    let (d, repo) = new_repo();
    let wt = task_with_change(&d, &repo, "t1", "ns/aaaa1111-x", "t.txt", "task\n");
    let sha = squash_to_base(&repo, &wt, "main", "Add t").unwrap();
    assert!(repo.join("t.txt").exists());
    let rv = revert(&repo, "main", &sha).unwrap();
    assert_eq!(g(&repo, &["rev-parse", "main"]), rv);
    assert!(!repo.join("t.txt").exists());
    assert!(repo.join("a.txt").exists());
    assert_eq!(count(&repo, "main"), 3);
    assert!(g(&repo, &["log", "-1", "--format=%s", "main"]).starts_with("Revert"));
}

#[test]
fn revert_works_while_human_is_on_another_branch() {
    let (d, repo) = new_repo();
    let wt = task_with_change(&d, &repo, "t1", "ns/aaaa1111-x", "t.txt", "task\n");
    let sha = squash_to_base(&repo, &wt, "main", "Add t").unwrap();
    g(&repo, &["checkout", "-q", "-b", "other"]);
    revert(&repo, "main", &sha).unwrap();
    assert_eq!(g(&repo, &["ls-tree", "-r", "--name-only", "main"]), "a.txt");
}

fn with_origin(d: &TempDir, repo: &Path) -> PathBuf {
    let bare = d.path().join("origin.git");
    g(d.path(), &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()]);
    g(repo, &["remote", "add", "origin", bare.to_str().unwrap()]);
    g(repo, &["push", "-q", "-u", "origin", "main"]);
    bare
}

fn other_clone_pushes(d: &TempDir, bare: &Path) {
    let other = d.path().join("other");
    g(d.path(), &["clone", "-q", bare.to_str().unwrap(), other.to_str().unwrap()]);
    g(&other, &["config", "user.email", "o@example.com"]);
    g(&other, &["config", "user.name", "O"]);
    commit_in(&other, "o.txt", "o", "theirs");
    g(&other, &["push", "-q", "origin", "main"]);
}

#[test]
fn push_to_local_bare_origin_works() {
    let (d, repo) = new_repo();
    let bare = with_origin(&d, &repo);
    commit_in(&repo, "n.txt", "n", "n");
    push(&repo, "main").unwrap();
    assert_eq!(g(&bare, &["rev-parse", "main"]), g(&repo, &["rev-parse", "main"]));
}

#[test]
fn rejected_push_is_a_push_error_and_remote_is_untouched() {
    let (d, repo) = new_repo();
    let bare = with_origin(&d, &repo);
    other_clone_pushes(&d, &bare);
    let theirs = g(&bare, &["rev-parse", "main"]);
    commit_in(&repo, "n.txt", "n", "mine");
    let err = push(&repo, "main").unwrap_err();
    assert!(matches!(err, GitError::Push(_)), "got {err:?}");
    assert_eq!(g(&bare, &["rev-parse", "main"]), theirs);
}

#[test]
fn repo_info_reports_ahead_behind_dirty_and_origin() {
    let (d, repo) = new_repo();
    let bare = with_origin(&d, &repo);
    let i = repo_info(&repo);
    assert!(i.is_repo);
    assert_eq!(i.base_branch.as_deref(), Some("main"));
    assert_eq!(i.current_branch.as_deref(), Some("main"));
    assert_eq!(i.origin_url.as_deref(), Some(bare.to_str().unwrap()));
    assert_eq!((i.ahead, i.behind, i.dirty), (0, 0, false));

    commit_in(&repo, "n.txt", "n", "n");
    fs::write(repo.join("a.txt"), "edited\n").unwrap();
    let i = repo_info(&repo);
    assert_eq!((i.ahead, i.behind, i.dirty), (1, 0, true));
}

#[test]
fn repo_info_on_non_repo() {
    let d = tempfile::tempdir().unwrap();
    let i = repo_info(d.path());
    assert!(!i.is_repo);
    assert!(i.base_branch.is_none());
}

#[test]
fn fetch_ff_brings_in_remote_commits() {
    let (d, repo) = new_repo();
    let bare = with_origin(&d, &repo);
    other_clone_pushes(&d, &bare);
    fetch_ff(&repo, "main").unwrap();
    assert!(repo.join("o.txt").exists());
    assert_eq!(repo_info(&repo).behind, 0);
}

#[test]
fn only_merged_ns_branches_are_deleted() {
    let (d, repo) = new_repo();
    let wt1 = task_with_change(&d, &repo, "t1", "ns/11111111-done", "one.txt", "1\n");
    squash_to_base(&repo, &wt1, "main", "Task one").unwrap();
    remove_worktree(&repo, &wt1, false).unwrap();
    let wt2 = task_with_change(&d, &repo, "t2", "ns/22222222-open", "two.txt", "2\n");
    remove_worktree(&repo, &wt2, false).unwrap();
    g(&repo, &["branch", "feature-merged"]);

    let mut l = list_ns_branches(&repo);
    l.sort();
    assert_eq!(l, vec![("ns/11111111-done".to_string(), true), ("ns/22222222-open".to_string(), false)]);

    assert_eq!(delete_merged_ns_branches(&repo).unwrap(), 1);
    let left = g(&repo, &["branch", "--format=%(refname:short)"]);
    let mut left: Vec<&str> = left.lines().collect();
    left.sort();
    assert_eq!(left, vec!["feature-merged", "main", "ns/22222222-open"]);
}

#[test]
fn init_repo_makes_a_repo_with_one_commit_and_skips_env_files() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("proj");
    fs::create_dir(&p).unwrap();
    fs::write(p.join("code.rs"), "fn main(){}").unwrap();
    fs::write(p.join(".env"), "SECRET=1").unwrap();
    fs::create_dir_all(p.join("sub/deep")).unwrap();
    fs::write(p.join("sub/.env"), "SECRET=2").unwrap();
    fs::write(p.join("sub/deep/.env.local"), "SECRET=3").unwrap();
    fs::write(p.join("sub/keep.txt"), "k").unwrap();
    init_repo(&p).unwrap();
    assert!(repo_info(&p).is_repo);
    assert_eq!(count(&p, "HEAD"), 1);
    assert_eq!(g(&p, &["ls-tree", "-r", "--name-only", "HEAD"]), "code.rs\nsub/keep.txt");
    init_repo(&p).unwrap();
    assert_eq!(count(&p, "HEAD"), 1);
}

#[test]
fn detect_commands_by_project_type() {
    type Case<'a> = (&'a str, Vec<(&'a str, &'a str)>, Option<&'a str>, Option<&'a str>);
    let cases: Vec<Case> = vec![
        ("npm with lock", vec![("package.json", r#"{"scripts":{"test":"jest"}}"#), ("package-lock.json", "{}")], Some("npm test"), Some("npm ci")),
        ("npm no lock", vec![("package.json", r#"{"scripts":{"test":"jest"}}"#)], Some("npm test"), Some("npm install")),
        ("npm without test script", vec![("package.json", r#"{"scripts":{"build":"x"}}"#)], None, None),
        ("npm placeholder test", vec![("package.json", r#"{"scripts":{"test":"echo \"Error: no test specified\" && exit 1"}}"#)], None, None),
        ("cargo", vec![("Cargo.toml", "[package]")], Some("cargo test"), None),
        ("python", vec![("pyproject.toml", "[project]")], Some("pytest"), None),
        ("go", vec![("go.mod", "module x")], Some("go test ./..."), None),
        ("make with test", vec![("Makefile", "build:\n\tcc x\n\ntest:\n\t./run\n")], Some("make test"), None),
        ("make without test", vec![("Makefile", "build:\n\tcc x\n")], None, None),
        ("empty", vec![], None, None),
    ];
    for (name, files, check, setup) in cases {
        let d = tempfile::tempdir().unwrap();
        for (f, c) in files {
            fs::write(d.path().join(f), c).unwrap();
        }
        let (c, s) = detect_commands(d.path());
        assert_eq!((c.as_deref(), s.as_deref()), (check, setup), "case: {name}");
    }
}

#[test]
fn web_url_from_ssh_and_https_origins() {
    let (_d, repo) = new_repo();
    for (origin, want) in [
        ("git@github.com:o/r.git", Some("https://github.com/o/r")),
        ("https://github.com/o/r.git", Some("https://github.com/o/r")),
        ("https://user:tok@github.com/o/r", Some("https://github.com/o/r")),
        ("ssh://git@example.com:2222/o/r.git", Some("https://example.com/o/r")),
        ("/some/local/bare.git", None),
    ] {
        let _ = Command::new("git").current_dir(&repo).args(["remote", "remove", "origin"]).output();
        g(&repo, &["remote", "add", "origin", origin]);
        assert_eq!(repo_info(&repo).web_url.as_deref(), want, "origin {origin}");
    }
}

#[test]
fn base_checked_out_in_linked_worktree_follows_the_same_rules() {
    let (d, repo) = new_repo();
    g(&repo, &["checkout", "-q", "-b", "other"]);
    let human_wt = d.path().join("human-main");
    g(&repo, &["worktree", "add", "-q", human_wt.to_str().unwrap(), "main"]);
    let wt = task_with_change(&d, &repo, "t1", "ns/aaaa1111-x", "t.txt", "task\n");

    // dirty linked copy: blocked, edit untouched
    fs::write(human_wt.join("a.txt"), "wip\n").unwrap();
    let before = g(&repo, &["rev-parse", "main"]);
    assert!(matches!(squash_to_base(&repo, &wt, "main", "Task"), Err(GitError::AdvanceBlocked(_))));
    assert_eq!(fs::read_to_string(human_wt.join("a.txt")).unwrap(), "wip\n");
    assert_eq!(g(&repo, &["rev-parse", "main"]), before);

    // clean linked copy: fast-forwarded in place
    g(&human_wt, &["checkout", "-q", "--", "a.txt"]);
    squash_to_base(&repo, &wt, "main", "Task").unwrap();
    assert_eq!(fs::read_to_string(human_wt.join("t.txt")).unwrap(), "task\n");
}

#[test]
fn paths_with_spaces_and_cyrillic_work() {
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("мой проект");
    fs::create_dir(&repo).unwrap();
    g(&repo, &["init", "-q", "-b", "main"]);
    g(&repo, &["config", "user.email", "t@example.com"]);
    g(&repo, &["config", "user.name", "T"]);
    commit_in(&repo, "a.txt", "one\n", "init");
    let wt = create_worktree(&repo, &d.path().join("рабочие копии"), "t1", "ns/aaaa1111-x", "main").unwrap();
    fs::write(wt.join("файл.txt"), "x").unwrap();
    commit_all(&wt, "add", true).unwrap().unwrap();
    squash_to_base(&repo, &wt, "main", "Задача").unwrap();
    assert_eq!(fs::read_to_string(repo.join("файл.txt")).unwrap(), "x");
    assert_eq!(g(&repo, &["status", "--porcelain"]), "");
    assert_eq!(g(&repo, &["log", "-1", "--format=%s", "main"]), "Задача");
    remove_worktree(&repo, &wt, true).unwrap();
    assert!(!wt.exists());
}

#[test]
fn two_worktrees_and_humans_uncommitted_file_stay_independent() {
    let (d, repo) = new_repo();
    fs::write(repo.join("human-notes.txt"), "mine\n").unwrap(); // untracked, in the main copy
    let w1 = task_with_change(&d, &repo, "t1", "ns/11111111-a", "one.txt", "1\n");
    let w2 = task_with_change(&d, &repo, "t2", "ns/22222222-b", "two.txt", "2\n");
    assert!(!w1.join("two.txt").exists() && !w2.join("one.txt").exists());
    assert!(!w1.join("human-notes.txt").exists() && !w2.join("human-notes.txt").exists());
    assert_eq!(count(&repo, "main"), 1);

    squash_to_base(&repo, &w1, "main", "First").unwrap();
    rebase_onto(&w2, "main").unwrap();
    squash_to_base(&repo, &w2, "main", "Second").unwrap();
    assert_eq!(count(&repo, "main"), 3);
    assert_eq!(g(&repo, &["ls-tree", "-r", "--name-only", "main"]), "a.txt\none.txt\ntwo.txt");
    assert_eq!(fs::read_to_string(repo.join("human-notes.txt")).unwrap(), "mine\n");
}

#[test]
fn revert_that_cannot_apply_cleanly_is_a_conflict() {
    let (d, repo) = new_repo();
    let wt = wt_for(&d, &repo, "t1", "ns/aaaa1111-x");
    fs::write(wt.join("a.txt"), "task\n").unwrap();
    commit_all(&wt, "c", true).unwrap();
    let sha = squash_to_base(&repo, &wt, "main", "Edit a").unwrap();
    commit_in(&repo, "a.txt", "later human edit\n", "later");
    assert!(matches!(revert(&repo, "main", &sha), Err(GitError::Conflict(_))));
}

#[test]
fn readonly_copy_is_clean_after_refresh_and_the_humans_folder_is_untouched() {
    let (d, repo) = new_repo();
    let copy = create_readonly_copy(&repo, &d.path().join("chat-1"), "main").unwrap();
    assert_eq!(fs::read_to_string(copy.join("a.txt")).unwrap(), "one\n");
    assert_eq!(g(&copy, &["branch", "--show-current"]), "", "no branch");
    let branches_before = g(&repo, &["branch", "--list"]);

    // the model scribbles in the copy: edits, new and ignored files
    fs::write(copy.join("a.txt"), "changed\n").unwrap();
    fs::write(copy.join("new.txt"), "x").unwrap();
    // main moves on in the human's repo
    commit_in(&repo, "b.txt", "two\n", "second");

    refresh_readonly_copy(&copy, "main").unwrap();
    assert_eq!(g(&copy, &["status", "--porcelain", "--ignored"]), "");
    assert_eq!(fs::read_to_string(copy.join("a.txt")).unwrap(), "one\n");
    assert_eq!(fs::read_to_string(copy.join("b.txt")).unwrap(), "two\n", "copy follows the fresh base");
    assert!(!copy.join("new.txt").exists());
    // the human's folder: same branches, nothing from the copy in it
    assert_eq!(g(&repo, &["branch", "--list"]), branches_before);
    assert!(!repo.join("new.txt").exists());
    assert_eq!(g(&repo, &["status", "--porcelain", "--untracked-files=no"]), "");
}

#[test]
fn refresh_refuses_a_folder_that_is_not_an_app_copy() {
    let (d, repo) = new_repo();
    fs::write(repo.join("precious.txt"), "mine").unwrap();
    fs::write(repo.join("a.txt"), "my edit\n").unwrap();
    assert!(refresh_readonly_copy(&repo, "main").is_err());
    // a task worktree sits on a branch: not a read-only copy either
    let wt = wt_for(&d, &repo, "t9", "ns/aaaa1111-x");
    fs::write(wt.join("w.txt"), "work").unwrap();
    assert!(refresh_readonly_copy(&wt, "main").is_err());
    assert_eq!(fs::read_to_string(repo.join("precious.txt")).unwrap(), "mine");
    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "my edit\n");
    assert!(wt.join("w.txt").exists());
}

#[test]
fn restore_paths_returns_files_to_the_state_of_from() {
    let (d, repo) = new_repo();
    let wt = wt_for(&d, &repo, "t2", "ns/bbbb2222-x");
    let from = g(&wt, &["rev-parse", "HEAD"]);
    // an edit, a new file and a deletion that the part should not have made
    fs::write(wt.join("a.txt"), "edited\n").unwrap();
    fs::write(wt.join("extra.txt"), "new").unwrap();
    fs::create_dir(wt.join("dir")).unwrap();
    fs::write(wt.join("dir/inner.txt"), "new").unwrap();
    fs::write(wt.join("keep.txt"), "mine").unwrap();
    g(&wt, &["add", "-A"]);
    g(&wt, &["commit", "-q", "-m", "part"]);

    restore_paths(&wt, &from, &["a.txt".into(), "extra.txt".into(), "dir/inner.txt".into()]).unwrap();
    assert_eq!(fs::read_to_string(wt.join("a.txt")).unwrap(), "one\n");
    assert!(!wt.join("extra.txt").exists() && !wt.join("dir/inner.txt").exists());
    assert_eq!(fs::read_to_string(wt.join("keep.txt")).unwrap(), "mine", "other files stay");
    // after committing, the only difference left from `from` is the file that was not restored
    commit_all(&wt, "fix", true).unwrap();
    assert_eq!(changed_paths(&wt, &from, "HEAD"), vec!["keep.txt".to_string()]);
    assert!(restore_paths(&wt, &from, &["../outside".into()]).is_err());
    assert!(restore_paths(&wt, &from, &["/etc/passwd".into()]).is_err());
}

#[test]
fn ff_branch_moves_forward_only() {
    let (d, repo) = new_repo();
    let wt = task_with_change(&d, &repo, "t3", "ns/cccc3333-x", "c.txt", "c\n");
    let tip = g(&wt, &["rev-parse", "HEAD"]);
    // a part branch that is ahead of the task branch
    g(&repo, &["branch", "ns/cccc3333-part", &tip]);
    let part = wt_for(&d, &repo, "t3p", "ns/cccc3333-part");
    commit_in(&part, "p.txt", "p\n", "part work");
    let part_tip = g(&part, &["rev-parse", "HEAD"]);

    ff_branch(&repo, "ns/cccc3333-x", "ns/cccc3333-part").unwrap();
    assert_eq!(g(&repo, &["rev-parse", "ns/cccc3333-x"]), part_tip);
    assert_eq!(fs::read_to_string(wt.join("p.txt")).unwrap(), "p\n", "the checked-out task copy follows");

    // backwards, and sideways, are refused and change nothing
    assert!(ff_branch(&repo, "ns/cccc3333-x", &tip).is_err());
    commit_in(&repo, "side.txt", "s", "side"); // main moves away from the task branch
    assert!(ff_branch(&repo, "ns/cccc3333-x", "main").is_err());
    assert_eq!(g(&repo, &["rev-parse", "ns/cccc3333-x"]), part_tip);
    // the human's branches never move
    assert!(ff_branch(&repo, "main", "ns/cccc3333-x").is_err());
}

#[test]
fn diff_file_cuts_at_max_lines_and_flags_binary_files() {
    let (d, repo) = new_repo();
    let wt = wt_for(&d, &repo, "t4", "ns/dddd4444-x");
    let body: String = (0..50).map(|i| format!("line {i}\n")).collect();
    fs::write(wt.join("big.txt"), &body).unwrap();
    fs::write(wt.join("pic.bin"), [0u8, 159, 146, 150, 0, 1, 2]).unwrap();
    commit_all(&wt, "add", true).unwrap();

    let full = diff_file(&wt, "main", "big.txt", 1000);
    assert!(!full.truncated && !full.binary);
    assert!(full.text.contains("+line 49"));
    let cut = diff_file(&wt, "main", "big.txt", 10);
    assert!(cut.truncated);
    assert_eq!(cut.text.lines().count(), 10);

    let bin = diff_file(&wt, "main", "pic.bin", 100);
    assert!(bin.binary && bin.text.is_empty());
    assert_eq!(bin.size, 7);
    // a path that is not inside the copy gives nothing
    assert_eq!(diff_file(&wt, "main", "../a.txt", 100).text, "");
}

#[test]
fn clone_shallow_takes_a_local_folder_and_refuses_dangerous_sources() {
    let (d, repo) = new_repo();
    commit_in(&repo, "b.txt", "two\n", "second");
    commit_in(&repo, "c.txt", "three\n", "third");
    let dest = d.path().join("clone");
    clone_shallow(repo.to_str().unwrap(), &dest).unwrap();
    assert_eq!(fs::read_to_string(dest.join("c.txt")).unwrap(), "three\n");
    assert_eq!(count(&dest, "HEAD"), 1, "shallow: one commit of three");

    let marker = d.path().join("PWNED");
    let evil = [
        format!("ext::sh -c 'touch {}'", marker.display()),
        "--upload-pack=touch PWNED".to_string(),
        "http://example.com/r.git".to_string(),
        "file:///etc".to_string(),
        "ftp://example.com/r".to_string(),
        "relative/path".to_string(),
        "/no/such/folder".to_string(),
        "".to_string(),
    ];
    for (i, url) in evil.iter().enumerate() {
        let dest = d.path().join(format!("evil{i}"));
        assert!(clone_shallow(url, &dest).is_err(), "{url}");
        assert!(!dest.exists(), "{url}");
    }
    assert!(!marker.exists());
}

#[test]
fn refresh_leaves_the_humans_own_detached_worktree_alone() {
    let (d, repo) = new_repo();
    // a worktree the human made themselves on a detached HEAD: no mark from create_readonly_copy
    let mine = d.path().join("mine");
    g(&repo, &["worktree", "add", "--detach", mine.to_str().unwrap(), "main"]);
    fs::write(mine.join("a.txt"), "my uncommitted edit\n").unwrap();
    fs::write(mine.join("notes.txt"), "mine").unwrap();
    assert!(refresh_readonly_copy(&mine, "main").is_err());
    assert_eq!(fs::read_to_string(mine.join("a.txt")).unwrap(), "my uncommitted edit\n");
    assert!(mine.join("notes.txt").exists());
}

#[cfg(unix)]
#[test]
fn restore_paths_refuses_a_folder_swapped_for_a_link_to_the_outside() {
    let (d, repo) = new_repo();
    let wt = wt_for(&d, &repo, "t7", "ns/eeee5555-x");
    fs::create_dir(wt.join("dir")).unwrap();
    fs::write(wt.join("dir/f.txt"), "in").unwrap();
    commit_all(&wt, "dir", true).unwrap();
    let from = g(&wt, &["rev-parse", "HEAD"]);
    fs::write(wt.join("dir/new.txt"), "n").unwrap();
    // the part replaced the folder by a link to somewhere else
    let outside = d.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("new.txt"), "precious").unwrap();
    fs::remove_dir_all(wt.join("dir")).unwrap();
    std::os::unix::fs::symlink(&outside, wt.join("dir")).unwrap();
    let r = restore_paths(&wt, &from, &["dir/new.txt".into(), "a.txt".into()]);
    assert!(r.is_err());
    assert_eq!(fs::read_to_string(outside.join("new.txt")).unwrap(), "precious");
}

#[test]
fn diff_file_also_stops_at_a_byte_limit() {
    let (d, repo) = new_repo();
    let wt = wt_for(&d, &repo, "t8", "ns/ffff6666-x");
    fs::write(wt.join("min.js"), "x".repeat(2_000_000)).unwrap(); // one huge line
    commit_all(&wt, "min", true).unwrap();
    let df = diff_file(&wt, "main", "min.js", 10_000);
    assert!(df.truncated);
    assert!(df.text.len() <= 256 * 1024, "{}", df.text.len());
}

fn tree_of(dir: &Path, rev: &str) -> Vec<String> {
    g(dir, &["ls-tree", "-r", "--name-only", rev]).lines().map(str::to_string).collect()
}

/// R17i.2: a key or a `.env` the agent wrote never goes into a commit of the app; `.env.example` and code still do.
#[test]
fn protected_paths_stay_out_of_every_commit_and_the_branch() {
    let (d, repo) = new_repo();
    let wt = wt_for(&d, &repo, "t1", "ns/abc12345-keys");
    fs::create_dir_all(wt.join("fixtures")).unwrap();
    for f in [".env", ".env.local", "fixtures/id_ed25519", "fixtures/server.pem", ".netrc", "code.rs", ".env.example"] {
        fs::write(wt.join(f), "x\n").unwrap();
    }
    let mut pending = protected_pending(&wt);
    pending.sort();
    assert_eq!(pending, [".env", ".env.local", ".netrc", "fixtures/id_ed25519", "fixtures/server.pem"]);
    assert!(commit_all(&wt, "work", true).unwrap().is_some());
    let mut files = tree_of(&wt, "HEAD");
    files.sort();
    assert_eq!(files, [".env.example", "a.txt", "code.rs"]);
    assert!(wt.join(".env").exists(), "the file stays in the copy, only out of git");

    // the agent commits a key itself: the branch says so, and folding it back leaves it out of the next commit
    fs::write(wt.join("id_rsa"), "k\n").unwrap();
    g(&wt, &["add", "-f", "id_rsa"]);
    g(&wt, &["commit", "-q", "-m", "oops"]);
    assert_eq!(protected_committed(&wt, "main").unwrap(), ["id_rsa"]);
    assert!(uncommit_hidden(&wt, "main", true).unwrap());
    commit_all(&wt, "again", true).unwrap();
    assert!(!tree_of(&wt, "HEAD").contains(&"id_rsa".to_string()));
    assert!(protected_committed(&wt, "main").unwrap().is_empty());
}

#[test]
fn protected_paths_off_commits_them_as_any_file() {
    let (d, repo) = new_repo();
    let wt = wt_for(&d, &repo, "t1", "ns/abc12345-off");
    fs::write(wt.join(".env"), "x\n").unwrap();
    commit_all(&wt, "work", false).unwrap();
    assert!(tree_of(&wt, "HEAD").contains(&".env".to_string()));
}

/// Review of F1: a key already tracked on base and changed by the branch is no new file, but it still leaves with the
/// branch: the merge check sees it. Deleting one is no leak.
#[test]
fn a_changed_tracked_key_is_seen_by_the_merge_check_and_a_deleted_one_is_not() {
    let (d, repo) = new_repo();
    commit_in(&repo, "server.key", "old\n", "tracked key");
    commit_in(&repo, ".env", "A=1\n", "tracked env");
    let wt = wt_for(&d, &repo, "t1", "ns/abc12345-tracked");
    fs::write(wt.join("server.key"), "stolen\n").unwrap();
    fs::remove_file(wt.join(".env")).unwrap();
    commit_all(&wt, "work", true).unwrap();
    assert_eq!(protected_committed(&wt, "main").unwrap(), ["server.key"]);
}

// ---- revert on a git without `merge-tree --merge-base` (before 2.40): the scratch-index path, checked against merge-tree ----

fn scratch_left(repo: &Path) -> Vec<String> {
    fs::read_dir(repo.join(".git")).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with("ns-revert-")).collect()
}

#[test]
fn old_git_revert_gives_the_tree_merge_tree_gives_and_merges_lines_of_one_file() {
    let (_d, repo) = new_repo();
    commit_in(&repo, "a.txt", "one\ntwo\nthree\nfour\nfive\n", "lines");
    commit_in(&repo, "a.txt", "one\nTWO\nthree\nfour\nfive\n", "edit two");
    let sha = g(&repo, &["rev-parse", "HEAD"]);
    commit_in(&repo, "a.txt", "one\nTWO\nthree\nfour\nFIVE\n", "later edit five");
    fs::write(repo.join("b.txt"), "b\n").unwrap();
    g(&repo, &["add", "-A"]);
    g(&repo, &["commit", "-q", "-m", "add b"]);
    let (main, parent) = (g(&repo, &["rev-parse", "main"]), g(&repo, &["rev-parse", &format!("{sha}^")]));
    let got = revert_tree_compat(&repo, &sha, &main, &parent).unwrap();
    // the reference only where this git has it (2.40+); on an older one the content check below is the test
    let want = Command::new("git").current_dir(&repo).args(["merge-tree", "--write-tree", &format!("--merge-base={sha}"), &main, &parent]).output().unwrap();
    if want.status.success() {
        assert_eq!(got, String::from_utf8_lossy(&want.stdout).lines().next().unwrap());
    }
    assert_eq!(g(&repo, &["show", &format!("{got}:a.txt")]), "one\ntwo\nthree\nfour\nFIVE");
    assert!(scratch_left(&repo).is_empty(), "{:?}", scratch_left(&repo));
}

#[test]
fn old_git_revert_of_a_line_changed_again_is_a_conflict_and_leaves_no_scratch() {
    let (_d, repo) = new_repo();
    commit_in(&repo, "a.txt", "task\n", "edit a");
    let sha = g(&repo, &["rev-parse", "HEAD"]);
    commit_in(&repo, "a.txt", "later human edit\n", "later");
    let (main, parent) = (g(&repo, &["rev-parse", "main"]), g(&repo, &["rev-parse", &format!("{sha}^")]));
    assert!(matches!(revert_tree_compat(&repo, &sha, &main, &parent), Err(GitError::Conflict(_))));
    assert!(scratch_left(&repo).is_empty(), "{:?}", scratch_left(&repo));
}

#[test]
fn old_git_revert_of_a_submodule_changed_twice_is_a_conflict_not_an_empty_blob() {
    let (_d, repo) = new_repo();
    let gitlink = |oid: &str, msg: &str| {
        g(&repo, &["update-index", "--add", "--cacheinfo", &format!("160000,{oid},sub")]);
        g(&repo, &["commit", "-q", "-m", msg]);
    };
    let (a, b, c) = ("1111111111111111111111111111111111111111", "2222222222222222222222222222222222222222", "3333333333333333333333333333333333333333");
    gitlink(a, "sub at A");
    gitlink(b, "sub to B");
    let sha = g(&repo, &["rev-parse", "HEAD"]);
    gitlink(c, "sub to C");
    let (main, parent) = (g(&repo, &["rev-parse", "main"]), g(&repo, &["rev-parse", &format!("{sha}^")]));
    let r = revert_tree_compat(&repo, &sha, &main, &parent);
    assert!(matches!(r, Err(GitError::Conflict(_))), "{r:?}");
    assert!(scratch_left(&repo).is_empty());
#[test]
fn a_github_repository_is_named_owner_slash_name_from_a_name_or_a_link_and_flags_never_pass() {
    use nightshift_core::git::gh_full_name as f;
    assert_eq!(f("Nezeronxer/vibe-env").as_deref(), Some("Nezeronxer/vibe-env"));
    assert_eq!(f(" https://github.com/tauri-apps/tauri.git ").as_deref(), Some("tauri-apps/tauri"));
    assert_eq!(f("https://github.com/a/b/tree/main/src").as_deref(), Some("a/b"));
    assert_eq!(f("git@github.com:a/b_c.d.git").as_deref(), Some("a/b_c.d"));
    for bad in ["", "a", "a/b/c", "-o/x", "a/--upload-pack=x", "a/.git", "../a", "a b/c", "https://gitlab.com/a/b", "a/b;rm"] {
        assert_eq!(f(bad), None, "{bad}");
    }
}

#[test]
fn cloning_refuses_a_bad_name_and_an_existing_folder_before_gh_runs() {
    let d = std::env::temp_dir().join(format!("ns-ghclone-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let e = nightshift_core::git::gh_clone("-x/y", &d.join("y")).unwrap_err().to_string();
    assert!(e.contains("bad_repo_name"), "{e}");
    let e = nightshift_core::git::gh_clone("a/b", &d).unwrap_err().to_string();
    assert!(e.contains("folder_exists"), "{e}");
    let _ = std::fs::remove_dir(&d);
}

#[test]
fn the_base_does_not_move_under_a_rebase_the_person_has_going() {
    let (_d, repo) = new_repo();
    // a global git template may install hooks that write files into every repo (graphify): they block the checkouts here
    g(&repo, &["config", "core.hooksPath", "/dev/null"]);
    let _ = fs::remove_dir_all(repo.join("graphify-out"));
    commit_in(&repo, "a.txt", "task\n", "edit a");
    let sha = g(&repo, &["rev-parse", "HEAD"]);
    g(&repo, &["checkout", "-q", "-b", "topic", &format!("{sha}^")]);
    commit_in(&repo, "a.txt", "topic\n", "topic edit");
    // the person rebases main onto topic and stops on the conflict: HEAD is detached, main is checked out nowhere
    g(&repo, &["checkout", "-q", "main"]);
    let o = Command::new("git").current_dir(&repo).args(["rebase", "topic"]).output().unwrap();
    assert!(rebase_in_progress(&repo), "{}\n{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    let before = g(&repo, &["rev-parse", "refs/heads/main"]);
    assert!(revert(&repo, "main", &sha).is_err());
    assert_eq!(g(&repo, &["rev-parse", "refs/heads/main"]), before, "main stays where the rebase expects it");
    let _ = Command::new("git").current_dir(&repo).args(["rebase", "--abort"]).output();
}
