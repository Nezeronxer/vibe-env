//! core::git v4: publishing to GitHub (a fake `gh` via NIGHTSHIFT_GH, nothing leaves the machine), the history page
//! with its graph, reverting a commit from the history.
use nightshift_core::git::*;
use std::fs;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// Test repos are made with `init --template=`: a hook from the user's `init.templatedir` (e.g. one that writes
/// files after each commit) would otherwise change them behind the test's back.
fn g(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").current_dir(dir).args(args).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

/// The fake gh writes its arguments, one per line, to `../gh.args` (gh runs in the repo) and behaves as
/// `../gh.mode` says: empty = fine, `taken` = the name exists, `nologin` = not logged in.
fn fake_gh() {
    static ONCE: OnceLock<tempfile::TempDir> = OnceLock::new();
    let d = ONCE.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("gh");
        let script = r#"#!/bin/sh
for a in "$@"; do printf '%s\n' "$a"; done >> ../gh.args
mode=$(cat ../gh.mode 2>/dev/null)
case "$1 $2" in
"api user") [ "$mode" = nologin ] && { echo 'HTTP 401: Bad credentials' >&2; exit 1; }; echo octocat;;
"repo create") [ "$mode" = taken ] && { echo 'GraphQL: Name already exists on this account (createRepository)' >&2; exit 1; }
  echo "https://github.com/octocat/$3";;
*) echo bad >&2; exit 1;;
esac
"#;
        fs::write(&p, script).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("NIGHTSHIFT_GH", &p);
        d
    });
    assert!(d.path().exists());
}

fn repo(d: &Path) -> PathBuf {
    let r = d.join("r");
    fs::create_dir(&r).unwrap();
    g(&r, &["init", "-q", "--template=", "-b", "main"]);
    g(&r, &["config", "user.email", "t@e.com"]);
    g(&r, &["config", "user.name", "T"]);
    fs::write(r.join("a.txt"), "a").unwrap();
    g(&r, &["add", "-A"]);
    g(&r, &["commit", "-q", "-m", "init"]);
    r
}

fn gh_args(d: &Path) -> Vec<String> {
    fs::read_to_string(d.join("gh.args")).unwrap_or_default().lines().map(str::to_string).collect()
}

#[test]
fn publish_calls_gh_repo_create_with_the_right_arguments_and_returns_the_url() {
    fake_gh();
    let d = tempfile::tempdir().unwrap();
    let r = repo(d.path());
    let url = publish(&r, "my-app", true, "-a tool").unwrap();
    assert_eq!(url, "https://github.com/octocat/my-app");
    let args = gh_args(d.path());
    let create = args.iter().position(|a| a == "create").unwrap();
    assert_eq!(
        args[create - 1..].to_vec(),
        vec!["repo", "create", "my-app", "--private", "--description=-a tool", "--source", r.to_str().unwrap(), "--push"]
    );
    fs::remove_file(d.path().join("gh.args")).unwrap();
    publish(&r, "my-app", false, "").unwrap();
    assert!(gh_args(d.path()).contains(&"--public".to_string()));
}

#[test]
fn a_key_anywhere_in_the_history_refuses_publishing_before_gh_is_called() {
    fake_gh();
    let d = tempfile::tempdir().unwrap();
    let r = repo(d.path());
    fs::write(r.join(".env"), "X=1").unwrap();
    g(&r, &["add", "-A"]);
    g(&r, &["commit", "-q", "-m", "oops"]);
    g(&r, &["rm", "-q", ".env"]);
    g(&r, &["commit", "-q", "-m", "removed again"]);
    assert_eq!(publish(&r, "my-app", true, "d"), Err(PublishError::ProtectedInHistory { paths: vec![".env".into()] }));
    assert!(gh_args(d.path()).is_empty(), "gh was not called");
    let j = serde_json::to_value(PublishError::ProtectedInHistory { paths: vec![".env".into()] }).unwrap();
    assert_eq!(j, serde_json::json!({"kind": "protected_in_history", "paths": [".env"]}));
}

#[test]
fn a_key_added_only_in_a_merge_commit_refuses_publishing_before_gh_is_called() {
    fake_gh();
    let d = tempfile::tempdir().unwrap();
    let r = repo(d.path());
    g(&r, &["checkout", "-q", "-b", "side"]);
    fs::write(r.join("s.txt"), "s").unwrap();
    g(&r, &["add", "-A"]);
    g(&r, &["commit", "-q", "-m", "side"]);
    g(&r, &["checkout", "-q", "main"]);
    g(&r, &["merge", "-q", "--no-ff", "--no-commit", "side"]);
    fs::write(r.join(".env"), "X=1").unwrap();
    g(&r, &["add", ".env"]);
    g(&r, &["commit", "-q", "-m", "merge side"]);
    assert_eq!(publish(&r, "my-app", true, "d"), Err(PublishError::ProtectedInHistory { paths: vec![".env".into()] }));
    assert!(gh_args(d.path()).is_empty(), "gh was not called");
}

#[test]
fn publish_errors_come_as_keys() {
    fake_gh();
    let d = tempfile::tempdir().unwrap();
    let r = repo(d.path());
    fs::write(d.path().join("gh.mode"), "taken").unwrap();
    assert_eq!(publish(&r, "my-app", true, ""), Err(PublishError::NameTaken));
    fs::write(d.path().join("gh.mode"), "nologin").unwrap();
    assert_eq!(publish(&r, "my-app", true, ""), Err(PublishError::GhNotLoggedIn));
    assert_eq!(publish(&r, "--help", true, ""), Err(PublishError::BadName));
    assert_eq!(publish(&r, "a b", true, ""), Err(PublishError::BadName));
}

/// Commits made fast through `git fast-import`: `main` with `n` rounds; every 5th round a two-commit
/// branch forks off main and is merged back; every 7th round a branch forks and stays open.
fn big_repo(d: &Path, n: usize) -> PathBuf {
    let r = d.join("big");
    fs::create_dir(&r).unwrap();
    g(&r, &["init", "-q", "--template=", "-b", "main"]);
    let mut s = String::new();
    let mut mark = 0;
    let mut t = 1_700_000_000u64;
    let mut commit = |s: &mut String, branch: &str, from: Option<usize>, merge: Option<usize>| -> usize {
        mark += 1;
        t += 60;
        s.push_str(&format!("commit refs/heads/{branch}\nmark :{mark}\ncommitter T <t@e.com> {t} +0000\ndata 3\nc{}\n", mark % 10));
        if let Some(f) = from {
            s.push_str(&format!("from :{f}\n"));
        }
        if let Some(m) = merge {
            s.push_str(&format!("merge :{m}\n"));
        }
        s.push_str(&format!("M 644 inline f{mark}\ndata 1\nx\n\n"));
        mark
    };
    let mut main = commit(&mut s, "main", None, None);
    for i in 0..n {
        main = commit(&mut s, "main", Some(main), None);
        if i % 5 == 0 {
            let b = format!("feat{i}");
            let a = commit(&mut s, &b, Some(main), None);
            let b2 = commit(&mut s, &b, Some(a), None);
            main = commit(&mut s, "main", Some(main), None);
            main = commit(&mut s, "main", Some(main), Some(b2));
        }
        if i % 7 == 0 {
            commit(&mut s, &format!("open{i}"), Some(main), None);
        }
    }
    let mut c = Command::new("git").current_dir(&r).args(["fast-import", "--quiet"]).stdin(Stdio::piped()).spawn().unwrap();
    c.stdin.take().unwrap().write_all(s.as_bytes()).unwrap();
    assert!(c.wait().unwrap().success());
    g(&r, &["checkout", "-q", "main"]);
    r
}

#[test]
fn two_pages_of_200_are_the_same_as_one_page_of_400() {
    let d = tempfile::tempdir().unwrap();
    let r = big_repo(d.path(), 250);
    assert!(g(&r, &["rev-list", "--count", "--all"]).parse::<usize>().unwrap() > 400);
    let whole = log_page(&r, 0, 400).unwrap();
    assert_eq!(whole.len(), 400);
    // after the whole page the cache stands at 400: the page at 200 is replayed from the start
    let replayed = log_page(&r, 200, 200).unwrap();
    assert_eq!(replayed, whole[200..]);
    // the usual way: page after page, the second goes on from the first
    let mut paged = log_page(&r, 0, 200).unwrap();
    paged.extend(log_page(&r, 200, 200).unwrap());
    assert_eq!(paged, whole);
    assert!(whole.iter().any(|c| c.lane > 0), "branches get lanes of their own");
}

/// Repo by hand: R; feature F1 (from R); main M1 (from R); M = merge of F1 into main; a tag on M1.
#[test]
fn lanes_and_edges_of_a_branch_and_its_merge() {
    let d = tempfile::tempdir().unwrap();
    let r = repo(d.path());
    let root = g(&r, &["rev-parse", "HEAD"]);
    g(&r, &["checkout", "-q", "-b", "feature"]);
    fs::write(r.join("f.txt"), "f").unwrap();
    g(&r, &["add", "-A"]);
    g(&r, &["commit", "-q", "-m", "F1", "--date=2020-01-01T00:00:02"]);
    let f1 = g(&r, &["rev-parse", "HEAD"]);
    g(&r, &["checkout", "-q", "main"]);
    fs::write(r.join("m.txt"), "m").unwrap();
    g(&r, &["add", "-A"]);
    g(&r, &["commit", "-q", "-m", "M1"]);
    let m1 = g(&r, &["rev-parse", "HEAD"]);
    g(&r, &["tag", "v1"]);
    g(&r, &["merge", "-q", "--no-ff", "-m", "Merge feature", "feature"]);
    let m = g(&r, &["rev-parse", "HEAD"]);

    let rows = log_page(&r, 0, 50).unwrap();
    let order: Vec<&str> = rows.iter().map(|c| c.sha.as_str()).collect();
    // git's topo order: the merge, the second parent's line, the first parent's line, the root
    assert_eq!(order, vec![m.as_str(), f1.as_str(), m1.as_str(), root.as_str()]);
    let lanes: Vec<u16> = rows.iter().map(|c| c.lane).collect();
    // M in lane 0 opens lane 1 for F1; M1 waits in lane 0; both lines join at R in lane 0
    assert_eq!(lanes, vec![0, 1, 0, 0]);
    assert_eq!(rows[0].edges, vec![(0, 0), (0, 1)], "the merge goes to its two parents");
    assert_eq!(rows[1].edges, vec![(0, 0), (1, 1)], "F1 waits for R in lane 1 while M1 comes in lane 0");
    assert_eq!(rows[2].edges, vec![(0, 0), (1, 0)], "both lines reach R in lane 0");
    assert_eq!(rows[3].edges, vec![], "the root has nothing below");
    assert_eq!(rows[0].parents, vec![m1.clone(), f1.clone()]);
    assert!(rows[0].refs.contains(&"main".to_string()), "{:?}", rows[0].refs);
    assert!(rows[1].refs.contains(&"feature".to_string()));
    assert!(rows[2].refs.contains(&"tag: v1".to_string()), "{:?}", rows[2].refs);
    assert_eq!(rows[0].author, "T");
    assert_eq!(rows[0].subject, "Merge feature");
}

#[test]
fn task_commits_are_known_by_trailer_or_ns_branch_and_an_empty_repo_has_no_history() {
    let d = tempfile::tempdir().unwrap();
    let r = repo(d.path());
    fs::write(r.join("b.txt"), "b").unwrap();
    g(&r, &["add", "-A"]);
    g(&r, &["commit", "-q", "-m", "Do it\n\nBody.\n\nNightshift-Task: 1234abcd-0000-4000-8000-000000000000"]);
    g(&r, &["branch", "ns/9876fedc-some-task", "HEAD~1"]);
    let rows = log_page(&r, 0, 10).unwrap();
    assert_eq!(rows[0].task.as_deref(), Some("1234abcd-0000-4000-8000-000000000000"));
    assert_eq!(rows[1].task.as_deref(), Some("9876fedc"));

    let empty = d.path().join("e");
    fs::create_dir(&empty).unwrap();
    g(&empty, &["init", "-q", "--template="]);
    assert_eq!(log_page(&empty, 0, 200).unwrap(), vec![]);
}

#[test]
fn revert_commit_undoes_any_commit_of_the_current_branch_locally() {
    let d = tempfile::tempdir().unwrap();
    let r = repo(d.path());
    fs::write(r.join("a.txt"), "changed").unwrap();
    g(&r, &["commit", "-q", "-am", "Change a"]);
    let sha = g(&r, &["rev-parse", "HEAD"]);
    fs::write(r.join("c.txt"), "c").unwrap();
    g(&r, &["add", "-A"]);
    g(&r, &["commit", "-q", "-m", "Add c"]);

    let new = revert_commit(&r, &sha).unwrap();
    assert_eq!(g(&r, &["rev-parse", "main"]), new);
    assert_eq!(fs::read_to_string(r.join("a.txt")).unwrap(), "a", "the clean working copy follows");
    assert!(r.join("c.txt").exists(), "the later commit stays");
    assert!(g(&r, &["log", "-1", "--format=%B"]).contains(&sha));
    assert!(!is_pushed(&r, &sha), "no remote");
}

/// Audit v4: 0x1E/0x1F in a subject must not forge a history row (a revert on it would undo the attacker's pick).
#[test]
fn separator_bytes_in_a_subject_do_not_forge_a_history_row() {
    let d = tempfile::tempdir().unwrap();
    let r = repo(d.path());
    let init = g(&r, &["rev-parse", "HEAD"]);
    fs::write(r.join("b.txt"), "b").unwrap();
    g(&r, &["add", "-A"]);
    let evil = format!("x\x1e{init}\x1f\x1fA\x1f1\x1fforged\x1f\x1f\x1e");
    g(&r, &["commit", "-q", "-m", &evil]);
    let head = g(&r, &["rev-parse", "HEAD"]);
    let rows = log_page(&r, 0, 50).unwrap();
    let shas: Vec<&str> = rows.iter().map(|c| c.sha.as_str()).collect();
    assert_eq!(shas, [head.as_str(), init.as_str()], "one row per real commit: {rows:?}");
    assert_eq!(rows[0].parents, std::slice::from_ref(&init));
    assert!(!rows.iter().any(|c| c.subject == "forged"), "{rows:?}");
}
