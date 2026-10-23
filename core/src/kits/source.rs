//! Reading a kit from a folder, a zip or a repository into memory, with every check done
//! before anything is written anywhere: paths, symlinks, sizes, hashes.

use super::{KitError, KitSource, Manifest};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) const MAX_FILE: u64 = 10 * 1024 * 1024;
pub(crate) const MAX_TOTAL: u64 = 50 * 1024 * 1024;
pub(crate) const MAX_FILES: usize = 5000;
const MAX_MANIFEST: u64 = 2 * 1024 * 1024;

pub(crate) struct LoadedKit {
    pub manifest: Manifest,
    pub files: BTreeMap<String, Vec<u8>>,
    pub commit: Option<String>,
    /// sha256 of kit.json: install checks it still loads the same kit as the preview did.
    pub sha: String,
}

pub(crate) fn sha256_hex(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

/// A relative path inside a kit: `/`-separated, no `..`, `.`, empty parts, backslashes,
/// drive letters, control characters or `.git`.
pub(crate) fn safe_rel(p: &str) -> Result<String, KitError> {
    let bad = || KitError::UnsafePath(p.chars().take(200).collect());
    if p.is_empty() || p.len() > 1024 || p.starts_with('/') || p.contains('\\') || p.contains(':') {
        return Err(bad());
    }
    let p = p.strip_suffix('/').unwrap_or(p);
    for c in p.split('/') {
        if c.is_empty() || c == "." || c == ".." || c == ".git" || c.chars().any(|ch| ch.is_control()) {
            return Err(bad());
        }
    }
    Ok(p.to_string())
}

pub(crate) fn load(src: &KitSource, scratch: &Path) -> Result<LoadedKit, KitError> {
    match src {
        KitSource::Folder(p) => load_dir(p, None),
        KitSource::Zip(p) => load_zip(p),
        KitSource::Repo(url) => load_repo(url, scratch),
    }
}

fn io(e: impl std::fmt::Display) -> KitError {
    KitError::Io(e.to_string())
}

fn walk(root: &Path, dir: &Path, depth: usize, out: &mut Vec<(String, u64)>, total: &mut u64) -> Result<(), KitError> {
    if depth > 32 {
        return Err(KitError::UnsafePath(dir.display().to_string()));
    }
    for e in std::fs::read_dir(dir).map_err(io)? {
        let e = e.map_err(io)?;
        let p = e.path();
        let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
        if depth == 0 && rel == ".git" {
            continue;
        }
        let md = std::fs::symlink_metadata(&p).map_err(io)?;
        if md.file_type().is_symlink() {
            return Err(KitError::Symlink(rel));
        }
        if md.is_dir() {
            walk(root, &p, depth + 1, out, total)?;
        } else if md.is_file() {
            if md.len() > MAX_FILE {
                return Err(KitError::FileTooLarge { path: rel, size: md.len() });
            }
            *total += md.len();
            if *total > MAX_TOTAL {
                return Err(KitError::KitTooLarge { size: *total });
            }
            out.push((rel, md.len()));
            if out.len() > MAX_FILES {
                return Err(KitError::TooManyFiles(out.len()));
            }
        } else {
            return Err(KitError::UnsafePath(rel));
        }
    }
    Ok(())
}

fn load_dir(root: &Path, commit: Option<String>) -> Result<LoadedKit, KitError> {
    let md = std::fs::symlink_metadata(root).map_err(|_| KitError::NotFound(root.display().to_string()))?;
    if md.file_type().is_symlink() {
        return Err(KitError::Symlink(root.display().to_string()));
    }
    if !md.is_dir() {
        return Err(KitError::NotAKit("not_a_folder".into()));
    }
    let mut all = vec![];
    let mut total = 0;
    walk(root, root, 0, &mut all, &mut total)?;
    let present: BTreeMap<String, u64> = all.into_iter().collect();
    let mut files = BTreeMap::new();
    let mj = present.get("kit.json").ok_or_else(|| KitError::NotAKit("no_kit_json".into()))?;
    if *mj > MAX_MANIFEST {
        return Err(KitError::NotAKit("kit_json_too_large".into()));
    }
    let raw = std::fs::read(root.join("kit.json")).map_err(io)?;
    let manifest = Manifest::parse(&raw)?;
    for f in &manifest.files {
        if !present.contains_key(&f.path) {
            return Err(KitError::Corrupt(format!("missing: {}", f.path)));
        }
        let b = std::fs::read(root.join(&f.path)).map_err(io)?;
        files.insert(f.path.clone(), b);
    }
    finish(manifest, files, commit, &raw)
}

fn load_zip(path: &Path) -> Result<LoadedKit, KitError> {
    let md = std::fs::metadata(path).map_err(|_| KitError::NotFound(path.display().to_string()))?;
    if md.len() > MAX_TOTAL + 8 * 1024 * 1024 {
        return Err(KitError::KitTooLarge { size: md.len() });
    }
    let f = std::fs::File::open(path).map_err(io)?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| KitError::BadZip(e.to_string()))?;
    if z.len() > MAX_FILES {
        return Err(KitError::TooManyFiles(z.len()));
    }
    // 1) check every entry before reading any content
    let mut names = vec![];
    let mut total = 0u64;
    for i in 0..z.len() {
        let e = z.by_index(i).map_err(|e| KitError::BadZip(e.to_string()))?;
        let raw = e.name().map_err(|e| KitError::BadZip(e.to_string()))?.to_string();
        if raw.starts_with('/') || raw.contains('\\') || raw.split('/').any(|c| c == "..") || e.enclosed_name().is_none() {
            return Err(KitError::UnsafePath(raw));
        }
        if e.is_symlink() {
            return Err(KitError::Symlink(raw));
        }
        if e.is_dir() {
            safe_rel(&raw)?;
            continue;
        }
        let rel = safe_rel(&raw)?;
        if e.size() > MAX_FILE {
            return Err(KitError::FileTooLarge { path: rel, size: e.size() });
        }
        total += e.size();
        if total > MAX_TOTAL {
            return Err(KitError::KitTooLarge { size: total });
        }
        names.push((i, rel));
    }
    // kit.json at the root, or inside one top folder (a zipped kit folder)
    let prefix = if names.iter().any(|(_, n)| n == "kit.json") {
        String::new()
    } else {
        let tops: std::collections::BTreeSet<&str> = names.iter().map(|(_, n)| n.split('/').next().unwrap_or("")).collect();
        let only = tops.iter().next().copied().unwrap_or("");
        if tops.len() == 1 && names.iter().any(|(_, n)| *n == format!("{only}/kit.json")) {
            format!("{only}/")
        } else {
            return Err(KitError::NotAKit("no_kit_json".into()));
        }
    };
    // 2) read content, never trusting declared sizes
    let mut content: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut read_total = 0u64;
    for (i, n) in &names {
        let Some(rel) = n.strip_prefix(&prefix) else { continue };
        let mut e = z.by_index(*i).map_err(|e| KitError::BadZip(e.to_string()))?;
        let mut buf = vec![];
        (&mut e).take(MAX_FILE + 1).read_to_end(&mut buf).map_err(|e| KitError::BadZip(e.to_string()))?;
        if buf.len() as u64 > MAX_FILE {
            return Err(KitError::FileTooLarge { path: rel.to_string(), size: buf.len() as u64 });
        }
        read_total += buf.len() as u64;
        if read_total > MAX_TOTAL {
            return Err(KitError::KitTooLarge { size: read_total });
        }
        content.insert(rel.to_string(), buf);
    }
    let raw = content.remove("kit.json").ok_or_else(|| KitError::NotAKit("no_kit_json".into()))?;
    let manifest = Manifest::parse(&raw)?;
    let mut files = BTreeMap::new();
    for f in &manifest.files {
        let b = content.remove(&f.path).ok_or_else(|| KitError::Corrupt(format!("missing: {}", f.path)))?;
        files.insert(f.path.clone(), b);
    }
    finish(manifest, files, None, &raw)
}

const CLONE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
/// The clone folder (history pack + checkout) may hold about two kits' worth of bytes.
const MAX_CLONE_DIR: u64 = 2 * MAX_TOTAL + 8 * 1024 * 1024;

/// Same rules as `git::clone_shallow`: `https://`, `ssh://`, `git@host:path` or an existing
/// absolute folder; `ext::`, option-looking text and other schemes are refused.
fn clone_url(url: &str) -> Result<String, KitError> {
    let u = url.trim();
    let bad = |why: &str| KitError::CloneFailed(format!("clone refused ({why})"));
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
        let canon = local.canonicalize().map_err(io)?;
        return Ok(format!("file://{}", canon.display()));
    }
    Err(bad("not a remote and not an existing local folder"))
}

fn dir_size(p: &Path) -> u64 {
    let mut n = 0;
    for e in std::fs::read_dir(p).into_iter().flatten().flatten() {
        match std::fs::symlink_metadata(e.path()) {
            Ok(m) if m.is_dir() => n += dir_size(&e.path()),
            Ok(m) => n += m.len(),
            Err(_) => {}
        }
    }
    n
}

/// Run git under a time limit, killing its whole process group when the limit or the size of
/// `watch` is exceeded. Stdout goes to `out_file` when given.
fn git_limited(dir: &Path, args: &[&str], watch: &Path, err_file: &Path, out_file: Option<&Path>) -> Result<(), KitError> {
    use std::os::unix::process::CommandExt;
    let err = std::fs::File::create(err_file).map_err(io)?;
    let out = match out_file {
        Some(p) => std::process::Stdio::from(std::fs::File::create(p).map_err(io)?),
        None => std::process::Stdio::null(),
    };
    let mut child = Command::new("git")
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ALLOW_PROTOCOL", "https:ssh:file")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("LC_ALL", "C")
        .args(["-c", "core.sshCommand=ssh -o BatchMode=yes", "-c", "protocol.ext.allow=never", "-c", "core.hooksPath=/dev/null"])
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(out)
        .stderr(err)
        .process_group(0)
        .spawn()
        .map_err(|e| KitError::CloneFailed(e.to_string()))?;
    let start = std::time::Instant::now();
    let kill = |child: &mut std::process::Child| {
        let _ = Command::new("kill").args(["-KILL", &format!("-{}", child.id())]).status();
        let _ = child.kill();
        let _ = child.wait();
    };
    loop {
        if let Some(st) = child.try_wait().map_err(io)? {
            if st.success() {
                return Ok(());
            }
            let msg = std::fs::read_to_string(err_file).unwrap_or_default();
            return Err(KitError::CloneFailed(msg.trim().chars().take(500).collect()));
        }
        if start.elapsed() > CLONE_TIMEOUT {
            kill(&mut child);
            return Err(KitError::CloneFailed("timeout".into()));
        }
        let size = dir_size(watch);
        if size > MAX_CLONE_DIR {
            kill(&mut child);
            return Err(KitError::KitTooLarge { size });
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn load_repo(url: &str, scratch: &Path) -> Result<LoadedKit, KitError> {
    with_clone(url, scratch, load_dir)
}

/// Every file of a folder with the kit limits (no symlinks, sizes, count), `.git` left out. For the catalog.
pub(crate) fn read_tree(root: &Path) -> Result<BTreeMap<String, Vec<u8>>, KitError> {
    let md = std::fs::symlink_metadata(root).map_err(|_| KitError::NotFound(root.display().to_string()))?;
    if md.file_type().is_symlink() {
        return Err(KitError::Symlink(root.display().to_string()));
    }
    if !md.is_dir() {
        return Err(KitError::NotFound(root.display().to_string()));
    }
    let mut all = vec![];
    walk(root, root, 0, &mut all, &mut 0)?;
    all.into_iter().map(|(rel, _)| Ok((safe_rel(&rel)?, std::fs::read(root.join(&rel)).map_err(io)?))).collect()
}

/// Clone `url` with the kit limits into a temp folder, hand the checkout and its commit to `f`, remove the folder.
pub(crate) fn with_clone<T>(url: &str, scratch: &Path, f: impl FnOnce(&Path, Option<String>) -> Result<T, KitError>) -> Result<T, KitError> {
    let src = clone_url(url)?;
    let tmp = scratch.join(format!("nightshift-kit-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).map_err(io)?;
    let dest = tmp.join("repo");
    let err = tmp.join("git-stderr");
    let res = (|| {
        // no checkout yet: sizes, count and symlinks are checked from the tree first
        let d = dest.to_string_lossy().to_string();
        git_limited(&tmp, &["clone", "--depth", "1", "--no-tags", "--single-branch", "--no-checkout", "--filter=blob:limit=10m", "-q", "--", &src, &d], &tmp, &err, None)?;
        // same limits as the clone; with lazy fetch off a blob the filter left out shows as "BAD"
        let listing = tmp.join("ls-tree");
        git_limited(&dest, &["ls-tree", "-r", "-l", "-z", "HEAD"], &tmp, &err, Some(&listing))
            .map_err(|e| if matches!(e, KitError::CloneFailed(_)) { KitError::CloneFailed("empty repository".into()) } else { e })?;
        let listing = std::fs::read(&listing).map_err(io)?;
        let mut total = 0u64;
        let mut count = 0usize;
        for rec in listing.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            let rec = String::from_utf8_lossy(rec);
            let (meta, path) = rec.split_once('\t').unwrap_or((&rec, ""));
            let f: Vec<&str> = meta.split_whitespace().collect();
            let mode = f.first().copied().unwrap_or("");
            if mode == "120000" {
                return Err(KitError::Symlink(path.to_string()));
            }
            if mode == "160000" {
                continue; // submodule: never fetched
            }
            // no size = the blob is not here: `--filter=blob:limit=10m` left it out as too large
            let size = match f.get(3).and_then(|s| s.parse::<u64>().ok()) {
                Some(n) if n <= MAX_FILE => n,
                n => return Err(KitError::FileTooLarge { path: path.to_string(), size: n.unwrap_or(MAX_FILE) }),
            };
            total += size;
            count += 1;
            if total > MAX_TOTAL {
                return Err(KitError::KitTooLarge { size: total });
            }
            if count > MAX_FILES {
                return Err(KitError::TooManyFiles(count));
            }
        }
        git_limited(&dest, &["checkout", "-q", "HEAD", "--", "."], &tmp, &err, None)?;
        let commit = Command::new("git")
            .current_dir(&dest)
            .env("LC_ALL", "C")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        f(&dest, commit)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    res
}

fn finish(manifest: Manifest, files: BTreeMap<String, Vec<u8>>, commit: Option<String>, raw: &[u8]) -> Result<LoadedKit, KitError> {
    for f in &manifest.files {
        let b = &files[&f.path];
        if sha256_hex(b) != f.sha256.to_ascii_lowercase() {
            return Err(KitError::Corrupt(format!("sha256: {}", f.path)));
        }
    }
    Ok(LoadedKit { manifest, files, commit, sha: sha256_hex(raw) })
}

/// Temp folder for clones: outside the home and data folders, removed after loading.
pub(crate) fn scratch_dir() -> PathBuf {
    std::env::temp_dir()
}
