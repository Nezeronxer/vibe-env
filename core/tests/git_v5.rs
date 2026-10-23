//! core::git v5: snapshots of a task's copy (turn rollback) and the main-folder handoff.
use nightshift_core::git::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn g(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").current_dir(dir).args(args).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

/// A repo with one commit and a task copy `wts/<id>` on `ns/t1-x`.
fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("r");
    fs::create_dir(&r).unwrap();
    g(&r, &["init", "-q", "--template=", "-b", "main"]);
    g(&r, &["config", "user.email", "t@e.com"]);
    g(&r, &["config", "user.name", "T"]);
    fs::write(r.join("a.txt"), "a\n").unwrap();
    g(&r, &["add", "-A"]);
    g(&r, &["commit", "-q", "-m", "init"]);
    let wt = create_worktree(&r, &d.path().join("wts"), "t1", "ns/t1-x", "main").unwrap();
    (d, r, wt)
}

#[test]
fn snapshot_restores_uncommitted_and_new_files_and_hides_its_ref() {
    let (_d, r, wt) = setup();
    fs::write(wt.join("a.txt"), "edited\n").unwrap(); // uncommitted edit
    fs::write(wt.join("new.txt"), "new\n").unwrap(); // untracked
    let n = snapshot(&wt, "t1", "worker").unwrap();
    assert_eq!(n, 1);
    // later: the agent commits, changes more, adds another file
    g(&wt, &["add", "-A"]);
    g(&wt, &["commit", "-q", "-m", "work"]);
    fs::write(wt.join("a.txt"), "worse\n").unwrap();
    fs::write(wt.join("later.txt"), "later\n").unwrap();
    let before_head = g(&wt, &["rev-parse", "HEAD"]);

    let saved = restore_snapshot(&wt, "t1", 1).unwrap();
    assert_eq!(fs::read_to_string(wt.join("a.txt")).unwrap(), "edited\n");
    assert_eq!(fs::read_to_string(wt.join("new.txt")).unwrap(), "new\n");
    assert!(!wt.join("later.txt").exists());
    assert_ne!(g(&wt, &["rev-parse", "HEAD"]), before_head, "the branch went back too");
    assert_eq!(g(&wt, &["rev-parse", "HEAD"]), g(&r, &["rev-parse", "main"]));
    // the ref is no branch and no tag
    assert!(!g(&r, &["branch", "--all"]).contains("nightshift"));
    assert!(g(&r, &["for-each-ref", "refs/nightshift/"]).contains("refs/nightshift/t1/1"));
    assert!(!g(&r, &["log", "--branches", "--oneline"]).contains("snapshot"));

    // the rollback is reversible: the state it left is a snapshot
    restore_snapshot(&wt, "t1", saved).unwrap();
    assert_eq!(fs::read_to_string(wt.join("a.txt")).unwrap(), "worse\n");
    assert_eq!(fs::read_to_string(wt.join("later.txt")).unwrap(), "later\n");
    assert_eq!(g(&wt, &["rev-parse", "HEAD"]), before_head);
}

#[test]
fn snapshot_leaves_copied_env_files_alone_and_out_of_the_object_store() {
    let (_d, r, wt) = setup();
    fs::write(wt.join(".env"), "SECRET=1\n").unwrap();
    hide_untracked(&wt, &[".env".to_string()]).unwrap();
    fs::write(wt.join("k.txt"), "1\n").unwrap();
    let n = snapshot(&wt, "t1", "worker").unwrap();
    fs::write(wt.join("k.txt"), "2\n").unwrap();
    restore_snapshot(&wt, "t1", n).unwrap();
    assert_eq!(fs::read_to_string(wt.join(".env")).unwrap(), "SECRET=1\n", "the copied file is still there");
    let sha = snapshots(&r, "t1")[0].sha.clone();
    assert!(!g(&r, &["ls-tree", "-r", "--name-only", &sha]).contains(".env"));
}

#[test]
fn snapshots_are_numbered_listed_and_dropped_with_the_copy() {
    let (_d, r, wt) = setup();
    assert_eq!(snapshot(&wt, "t1", "worker").unwrap(), 1);
    assert_eq!(snapshot(&wt, "t1", "worker").unwrap(), 2);
    let l = snapshots(&r, "t1");
    assert_eq!(l.iter().map(|s| s.n).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(l[0].label, "worker");
    assert!(restore_snapshot(&wt, "t1", 9).is_err());
    assert!(snapshot(&wt, "../x", "w").is_err());
    remove_worktree(&r, &wt, true).unwrap();
    assert!(snapshots(&r, "t1").is_empty());
}

#[test]
fn take_refuses_a_dirty_folder_and_switches_a_clean_one_and_give_returns() {
    let (_d, r, wt) = setup();
    fs::write(wt.join("w.txt"), "wip\n").unwrap(); // the copy's uncommitted work
    // dirty main: refused, nothing changes
    fs::write(r.join("a.txt"), "mine\n").unwrap();
    let e = take_to_main(&r, &wt, "ns/t1-x").unwrap_err();
    assert!(e.to_string().contains("main_dirty"), "{e}");
    assert_eq!(g(&r, &["branch", "--show-current"]), "main");
    assert_eq!(fs::read_to_string(r.join("a.txt")).unwrap(), "mine\n");
    assert_eq!(g(&wt, &["branch", "--show-current"]), "ns/t1-x");

    g(&r, &["checkout", "-q", "--", "a.txt"]);
    assert_eq!(take_to_main(&r, &wt, "ns/t1-x").unwrap(), "main");
    assert_eq!(g(&r, &["branch", "--show-current"]), "ns/t1-x");
    assert_eq!(g(&wt, &["branch", "--show-current"]), "", "the copy is detached");
    assert_eq!(fs::read_to_string(wt.join("w.txt")).unwrap(), "wip\n", "its files are kept");

    give_to_agent(&r, &wt, "ns/t1-x", "main", "main").unwrap();
    assert_eq!(g(&r, &["branch", "--show-current"]), "main");
    assert_eq!(g(&wt, &["branch", "--show-current"]), "ns/t1-x");
    assert_eq!(fs::read_to_string(wt.join("w.txt")).unwrap(), "wip\n");
}

#[test]
fn give_refuses_when_the_person_left_edits_on_the_task_branch() {
    let (_d, r, wt) = setup();
    take_to_main(&r, &wt, "ns/t1-x").unwrap();
    fs::write(r.join("a.txt"), "edited in main\n").unwrap();
    let e = give_to_agent(&r, &wt, "ns/t1-x", "main", "main").unwrap_err();
    assert!(e.to_string().contains("main_dirty"), "{e}");
    assert_eq!(g(&r, &["branch", "--show-current"]), "ns/t1-x");
    assert_eq!(fs::read_to_string(r.join("a.txt")).unwrap(), "edited in main\n");
}
