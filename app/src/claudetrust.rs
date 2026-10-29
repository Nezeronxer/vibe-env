//! Claude Code asks «Is this a project you trust?» in every folder it has not seen, with «No, exit» selected: a terminal
//! agent started in its own fresh copy stood on that question unseen. Nightshift answers it only where the human
//! already did: a copy inherits the trust of its project (or of a folder above it), and a project Nightshift has just
//! made (an empty folder with one commit) is trusted. Anything else is left to Claude's own question.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// One read-modify-write at a time - shared with the kit installs and the spread, which write the same file.
use nightshift_core::kits::CONFIG_LOCK as ONE;

/// The file itself: a `~/.claude.json` that is a link (dotfiles) is written through, not replaced by a plain file.
fn config(home: &Path) -> PathBuf {
    let p = home.join(".claude.json");
    nightshift_core::proc::canon(&p).unwrap_or(p)
}

fn key(p: &Path) -> String {
    nightshift_core::proc::canon(p).unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().into_owned()
}

fn accepted(cfg: &Value, dir: &str) -> bool {
    cfg.pointer("/projects").and_then(|ps| ps.get(dir)).and_then(|p| p.get("hasTrustDialogAccepted")).and_then(Value::as_bool) == Some(true)
}

/// The folder or one above it is trusted in Claude's config.
fn trusted(cfg: &Value, dir: &Path) -> bool {
    Path::new(&key(dir)).ancestors().any(|a| accepted(cfg, &a.to_string_lossy()))
}

/// Marks `dir` trusted. Read-modify-write of the whole file, then an atomic rename: Claude itself rewrites it the same way.
/// No config, or one that does not parse, is left alone (Claude will ask).
fn mark(home: &Path, cfg: &mut Value, dir: &Path) -> Result<(), String> {
    let Some(ps) = cfg.as_object_mut().map(|o| o.entry("projects").or_insert_with(|| json!({}))) else { return Ok(()) };
    let Some(ps) = ps.as_object_mut() else { return Ok(()) };
    let e = ps.entry(key(dir)).or_insert_with(|| json!({}));
    let Some(e) = e.as_object_mut() else { return Ok(()) };
    e.insert("hasTrustDialogAccepted".into(), Value::Bool(true));
    let path = config(home);
    static N: AtomicU64 = AtomicU64::new(0);
    let tmp = path.with_extension(format!("json.ns-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    // in the file's own indent and key order (it is Claude's file, rewritten on every new copy of an agent)
    let bytes = nightshift_core::kits::json_in_style(std::fs::read(&path).ok().as_deref(), cfg).map_err(|e| e.to_string())?.into_bytes();
    // made private from the start: the file holds tokens, and the home folder is readable by the group on a Mac
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    let w = o.open(&tmp).and_then(|mut f| std::io::Write::write_all(&mut f, &bytes).and_then(|_| f.sync_all()));
    if let Err(e) = w.and_then(|_| std::fs::rename(&tmp, &path)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    Ok(())
}

fn read(home: &Path) -> Option<Value> {
    std::fs::read(config(home)).ok().and_then(|b| serde_json::from_slice(&b).ok()).filter(Value::is_object)
}

/// A copy of `project` at `copy`: trusted when the project is. -> whether it was marked.
pub fn inherit(home: &Path, project: &Path, copy: &Path) -> bool {
    let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(mut cfg) = read(home) else { return false };
    if !trusted(&cfg, project) || trusted(&cfg, copy) {
        return false;
    }
    mark(home, &mut cfg, copy).is_ok()
}

/// A project Nightshift has just created: an empty folder the human named. -> whether it was marked.
pub fn created(home: &Path, dir: &Path) -> bool {
    let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(mut cfg) = read(home) else { return false };
    if trusted(&cfg, dir) {
        return false;
    }
    mark(home, &mut cfg, dir).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::Scratch;

    #[test]
    fn a_copy_inherits_the_trust_of_its_project_and_nothing_else_is_answered() {
        let s = Scratch::new("trust");
        let (home, proj, copy, other) = (s.0.join("home"), s.0.join("proj"), s.0.join("wt/one"), s.0.join("other"));
        for d in [&home, &proj, &copy, &other] {
            std::fs::create_dir_all(d).unwrap();
        }
        // no config: nothing is written
        assert!(!inherit(&home, &proj, &copy) && !config(&home).exists());
        let raw = json!({ "numStartups": 7, "mcpServers": { "x": { "command": "y" } }, "projects": { key(&proj): { "hasTrustDialogAccepted": true, "allowedTools": ["a"] } } });
        std::fs::write(config(&home), serde_json::to_vec(&raw).unwrap()).unwrap();
        assert!(inherit(&home, &proj, &copy));
        let got = read(&home).unwrap();
        assert!(accepted(&got, &key(&copy)));
        assert_eq!(got["numStartups"], 7, "the rest of the file stays");
        assert_eq!(got["mcpServers"]["x"]["command"], "y");
        assert_eq!(got["projects"][key(&proj)]["allowedTools"][0], "a");
        assert!(!inherit(&home, &proj, &copy), "a second time writes nothing");
        // a project the human never trusted gives its copy nothing
        assert!(!inherit(&home, &other, &s.0.join("wt")));
        assert!(!accepted(&read(&home).unwrap(), &key(&s.0.join("wt"))));
        // a folder under a trusted one counts as trusted
        let sub = proj.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        assert!(inherit(&home, &sub, &other));
        // a made project is trusted; a config that does not parse is left as it is
        let made = s.0.join("made");
        std::fs::create_dir_all(&made).unwrap();
        assert!(created(&home, &made) && accepted(&read(&home).unwrap(), &key(&made)));
        // eight copies at once: every answer lands (one writer at a time)
        let made: Vec<PathBuf> = (0..8).map(|i| { let d = s.0.join(format!("par{i}")); std::fs::create_dir_all(&d).unwrap(); d }).collect();
        std::thread::scope(|sc| for d in &made { let h = &home; sc.spawn(move || assert!(created(h, d))); });
        let got = read(&home).unwrap();
        assert!(made.iter().all(|d| accepted(&got, &key(d))));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(config(&home)).unwrap().permissions().mode() & 0o777, 0o600);
            // a link to a dotfiles copy stays a link and the copy gets the answer
            let real = s.0.join("dotfiles.json");
            std::fs::rename(home.join(".claude.json"), &real).unwrap();
            std::os::unix::fs::symlink(&real, home.join(".claude.json")).unwrap();
            let linked = s.0.join("linked");
            std::fs::create_dir_all(&linked).unwrap();
            assert!(created(&home, &linked));
            assert!(home.join(".claude.json").symlink_metadata().unwrap().file_type().is_symlink());
            assert!(accepted(&serde_json::from_slice(&std::fs::read(&real).unwrap()).unwrap(), &key(&linked)));
            std::fs::remove_file(home.join(".claude.json")).unwrap();
        }
        std::fs::write(home.join(".claude.json"), b"{ not json").unwrap();
        assert!(!created(&home, &s.0.join("x")));
        assert_eq!(std::fs::read(home.join(".claude.json")).unwrap(), b"{ not json");
    }
}
