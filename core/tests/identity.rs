//! Own test binary: it blanks the global git config for the whole process.
use nightshift_core::git::*;
use std::path::Path;
use std::process::Command;
use std::sync::Once;

fn no_global_identity() {
    static O: Once = Once::new();
    O.call_once(|| {
        std::env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
        std::env::set_var("GIT_CONFIG_SYSTEM", "/dev/null");
    });
}

fn g(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").current_dir(dir).args(args).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

#[test]
fn fallback_signature_when_nothing_is_configured() {
    no_global_identity();
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("r");
    init_repo(&p).unwrap();
    assert_eq!(g(&p, &["log", "-1", "--format=%an <%ae>"]), "Nightshift <nightshift@localhost>");
}

#[test]
fn repos_own_signature_is_never_overridden() {
    no_global_identity();
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("r");
    init_repo(&p).unwrap();
    g(&p, &["config", "user.name", "Local Person"]);
    g(&p, &["config", "user.email", "local@example.com"]);
    let wt = create_worktree(&p, &d.path().join("w"), "t1", "ns/aaaa1111-x", "main").unwrap();
    std::fs::write(wt.join("f"), "x").unwrap();
    commit_all(&wt, "c", true).unwrap().unwrap();
    assert_eq!(g(&wt, &["log", "-1", "--format=%an <%ae>"]), "Local Person <local@example.com>");
}
