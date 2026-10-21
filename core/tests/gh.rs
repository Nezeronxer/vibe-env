//! gh is replaced by a tiny script via NIGHTSHIFT_GH, so the tests do not need a login.
use nightshift_core::git::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

fn g(dir: &Path, args: &[&str]) {
    let o = Command::new("git").current_dir(dir).args(args).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

fn fake_gh() {
    static ONCE: OnceLock<tempfile::TempDir> = OnceLock::new();
    let d = ONCE.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("gh");
        fs::write(&p, "#!/bin/sh\ncase \"$1 $2\" in\n\"api user\") echo octocat;;\n\"pr view\") echo MERGED;;\n\"pr create\") echo 'Creating pull request'; echo https://github.com/o/r/pull/7;;\n*) echo bad >&2; exit 1;;\nesac\n").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("NIGHTSHIFT_GH", &p);
        d
    });
    assert!(d.path().exists());
}

#[test]
fn gh_status_pr_state_and_open_pr_use_gh_output() {
    fake_gh();
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("r");
    let bare = d.path().join("o.git");
    fs::create_dir(&repo).unwrap();
    g(&repo, &["init", "-q", "-b", "main"]);
    g(&repo, &["config", "user.email", "t@e.com"]);
    g(&repo, &["config", "user.name", "T"]);
    fs::write(repo.join("a"), "a").unwrap();
    g(&repo, &["add", "-A"]);
    g(&repo, &["commit", "-q", "-m", "i"]);
    g(d.path(), &["init", "-q", "--bare", bare.to_str().unwrap()]);
    g(&repo, &["remote", "add", "origin", bare.to_str().unwrap()]);
    g(&repo, &["branch", "ns/abcd1234-x"]);

    assert_eq!(gh_status(&repo).unwrap(), "octocat");
    assert_eq!(pr_state(&repo, "https://github.com/o/r/pull/7").unwrap(), "merged");
    let url = open_pr(&repo, "ns/abcd1234-x", "main", "Title", "Body").unwrap();
    assert_eq!(url, "https://github.com/o/r/pull/7");
    // open_pr pushed the branch first
    let heads = Command::new("git").current_dir(&bare).args(["branch", "--list", "ns/*"]).output().unwrap();
    assert!(String::from_utf8_lossy(&heads.stdout).contains("ns/abcd1234-x"));
}
