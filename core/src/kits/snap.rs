//! Snapshots and the install journal. A snapshot keeps the old bytes of every file an install
//! (or a later "enable") changes, the list of files it created, and the kit itself for items
//! that stay off until the human turns them on.

use super::source;
use super::units::{self, ConfOp};
use super::{HeldItem, KitError, KitSource, Scope, SnapshotId};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

fn io(e: impl std::fmt::Display) -> KitError {
    KitError::Io(e.to_string())
}

/// Write via a temp file in the same folder + rename, so a reader never sees half a file.
/// `mode` is set on the temp file before any byte is written.
pub(crate) fn write_atomic(p: &Path, bytes: &[u8], mode: Option<u32>) -> Result<(), KitError> {
    let dir = p.parent().ok_or_else(|| KitError::Io(format!("no parent: {}", p.display())))?;
    std::fs::create_dir_all(dir).map_err(io)?;
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.ns-tmp-{}", uuid::Uuid::new_v4().simple()));
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(mode.unwrap_or(0o644));
    }
    let res = (|| {
        let mut f = o.open(&tmp).map_err(io)?;
        f.write_all(bytes).map_err(io)?;
        f.sync_all().map_err(io)?;
        #[cfg(unix)]
        if let Some(m) = mode {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(m)).map_err(io)?;
        }
        std::fs::rename(&tmp, p).map_err(io)
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

pub(crate) fn mode_of(p: &Path) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p).ok().map(|m| m.permissions().mode() & 0o7777)
    }
    #[cfg(not(unix))]
    {
        let _ = p;
        None
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotState {
    /// Journal open: an install is (or was, if the app died) writing files.
    Installing,
    Done,
    RolledBack,
    /// Found unfinished at start and rolled back by `recover()`.
    Interrupted,
    /// Written, but the CLI saved its own copy of a shared config over the kit's keys right
    /// after (it was running): the human closes the CLI and installs again, or rolls back.
    NotApplied,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct Entry {
    pub path: PathBuf,
    pub existed: bool,
    pub backup: Option<String>,
    pub mode: Option<u32>,
    /// Shared config merged by key: old value of each touched key (None = absent). Rollback
    /// puts back only these, keeping whatever the CLI wrote to other keys since.
    #[serde(default)]
    pub conf: Vec<(ConfOp, Option<String>)>,
    /// sha256 of the bytes the last write put there: a file new to the install is removed on
    /// rollback only while it still holds exactly these.
    #[serde(default)]
    pub written: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct Meta {
    pub id: SnapshotId,
    pub env: String,
    pub kit_name: String,
    pub source: KitSource,
    pub commit: Option<String>,
    pub scope: Scope,
    pub at: DateTime<Local>,
    pub state: SnapshotState,
    pub entries: Vec<Entry>,
    pub created_dirs: Vec<PathBuf>,
    pub held: Vec<HeldItem>,
    pub project_files: Vec<String>,
}

/// What a rollback left in place because it changed after the install.
#[derive(Default, Debug)]
pub(crate) struct Kept {
    /// (config file, key)
    pub keys: Vec<(PathBuf, String)>,
    /// Files new to the install that the human wrote into since: kept, minus the kit's keys.
    pub files: Vec<PathBuf>,
}

pub(crate) const KEEP: usize = 20;
const KEEP_DAYS: i64 = 30;

pub(crate) struct Snaps {
    pub root: PathBuf,
}

impl Snaps {
    pub fn dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    /// Snapshots hold old copies of the user's own files: the folders are 0700.
    pub fn private_dir(&self, id: &str) -> Result<(), KitError> {
        for d in [self.root.parent().unwrap_or(&self.root).to_path_buf(), self.root.clone(), self.dir(id)] {
            std::fs::create_dir_all(&d).map_err(io)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700)).map_err(io)?;
            }
        }
        Ok(())
    }

    /// Keep the newest `KEEP` snapshots of `env` that are younger than `KEEP_DAYS`; never the
    /// one just made, never an install still open.
    pub fn prune(&self, env: &str, keep_id: &str) {
        let cutoff = chrono::Local::now() - chrono::Duration::days(KEEP_DAYS);
        let mine: Vec<Meta> = self.all().into_iter().filter(|m| m.env == env).collect();
        for (i, m) in mine.iter().enumerate() {
            if m.id != keep_id && m.state != SnapshotState::Installing && (i >= KEEP || m.at < cutoff) {
                let _ = std::fs::remove_dir_all(self.dir(&m.id));
            }
        }
    }

    pub fn save(&self, m: &Meta) -> Result<(), KitError> {
        self.private_dir(&m.id)?;
        write_atomic(&self.dir(&m.id).join("meta.json"), &serde_json::to_vec_pretty(m).map_err(io)?, Some(0o600))
    }

    pub fn load(&self, id: &str) -> Result<Meta, KitError> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(KitError::NotFound(id.to_string()));
        }
        let b = std::fs::read(self.dir(id).join("meta.json")).map_err(|_| KitError::NotFound(id.to_string()))?;
        serde_json::from_slice(&b).map_err(|e| KitError::Corrupt(e.to_string()))
    }

    pub fn all(&self) -> Vec<Meta> {
        let mut v: Vec<Meta> = std::fs::read_dir(&self.root)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| self.load(&e.file_name().to_string_lossy()).ok())
            .collect();
        v.sort_by_key(|m| std::cmp::Reverse(m.at));
        v
    }

    /// Folders left without a journal (the app died before the journal was written,
    /// so nothing outside the data folder was touched yet).
    pub fn sweep_orphans(&self) {
        for e in std::fs::read_dir(&self.root).into_iter().flatten().flatten() {
            if e.path().is_dir() && !e.path().join("meta.json").exists() {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }

    /// Keep the old bytes of `path` (once per snapshot) before it changes.
    pub fn remember(&self, m: &mut Meta, path: &Path, ops: &[ConfOp]) -> Result<(), KitError> {
        if !ops.is_empty() {
            let cur = std::fs::read(path).ok();
            let olds: Vec<(ConfOp, Option<String>)> = ops.iter().map(|o| (o.clone(), units::conf_old(cur.as_deref(), o))).collect();
            if let Some(e) = m.entries.iter_mut().find(|e| e.path == path) {
                e.conf.extend(olds);
                return Ok(());
            }
            self.remember(m, path, &[])?;
            if let Some(e) = m.entries.iter_mut().find(|e| e.path == path) {
                e.conf = olds;
            }
            return Ok(());
        }
        if m.entries.iter().any(|e| e.path == path) {
            return Ok(());
        }
        let existed = std::fs::symlink_metadata(path).is_ok();
        let backup = if existed {
            let n = format!("orig/{}", m.entries.len());
            let bytes = std::fs::read(path).map_err(io)?;
            write_atomic(&self.dir(&m.id).join(&n), &bytes, Some(0o600))?;
            Some(n)
        } else {
            None
        };
        // folders the write will create, outermost first
        if !existed {
            let mut missing = vec![];
            let mut cur = path.parent();
            while let Some(d) = cur {
                if d.exists() {
                    break;
                }
                missing.push(d.to_path_buf());
                cur = d.parent();
            }
            for d in missing.into_iter().rev() {
                if !m.created_dirs.contains(&d) {
                    m.created_dirs.push(d);
                }
            }
        }
        m.entries.push(Entry { path: path.to_path_buf(), existed, backup, mode: mode_of(path), conf: vec![], written: None });
        Ok(())
    }

    /// Put every remembered file back as it was and remove what was added. A config key whose
    /// live value is no longer what the install wrote (the human or the CLI changed it since)
    /// stays as it is and comes back in `Kept`. `careful` = a rollback the human asked for; an
    /// install cut short (nobody edited anything in between) removes what it added outright.
    pub fn restore(&self, m: &Meta, careful: bool) -> Result<Kept, KitError> {
        let mut kept = Kept::default();
        for e in m.entries.iter().rev() {
            // the read and the write of a shared config under the app's one lock (see kits::CONFIG_LOCK)
            let _w = (!e.conf.is_empty()).then(|| super::CONFIG_LOCK.lock().unwrap_or_else(|x| x.into_inner()));
            let cur = std::fs::read(&e.path).ok();
            // a file new to the install that no longer holds what the install wrote
            let touched = careful && e.backup.is_none() && e.written.as_ref().is_some_and(|h| cur.as_deref().is_some_and(|c| source::sha256_hex(c) != *h));
            if touched {
                kept.files.push(e.path.clone());
            }
            if !e.conf.is_empty() {
                let Some(mut undone) = cur else {
                    // the file is gone since: nothing of the kit is left in it, the old one goes back
                    if let Some(b) = &e.backup {
                        write_atomic(&e.path, &std::fs::read(self.dir(&m.id).join(b)).map_err(io)?, e.mode)?;
                    }
                    continue;
                };
                let mut any_kept = false;
                for (op, old) in e.conf.iter().rev() {
                    if units::conf_state(Some(&undone), op).1 {
                        undone = units::undo_conf(Some(&undone), &[(op.clone(), old.clone())])?;
                    } else if units::conf_old(Some(&undone), op) != *old {
                        any_kept = true;
                        kept.keys.push((e.path.clone(), op.label()));
                    }
                }
                let orig = match &e.backup {
                    Some(b) => Some(std::fs::read(self.dir(&m.id).join(b)).map_err(io)?),
                    None => None,
                };
                match orig {
                    // nobody else touched the file: the exact old bytes go back
                    Some(o) if units::same_conf(&undone, &o) => write_atomic(&e.path, &o, e.mode)?,
                    None if !touched && !any_kept && units::empty_conf(&undone) => {
                        if std::fs::symlink_metadata(&e.path).is_ok() {
                            std::fs::remove_file(&e.path).map_err(io)?;
                        }
                    }
                    _ => write_atomic(&e.path, &undone, e.mode)?,
                }
                continue;
            }
            match &e.backup {
                Some(b) => {
                    let bytes = std::fs::read(self.dir(&m.id).join(b)).map_err(io)?;
                    write_atomic(&e.path, &bytes, e.mode)?;
                }
                None if touched => {}
                None => {
                    if std::fs::symlink_metadata(&e.path).is_ok() {
                        std::fs::remove_file(&e.path).map_err(io)?;
                    }
                }
            }
        }
        // temp files of a write that was cut short
        for e in &m.entries {
            let (Some(dir), Some(name)) = (e.path.parent(), e.path.file_name()) else { continue };
            let prefix = format!(".{}.ns-tmp-", name.to_string_lossy());
            for f in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                if f.file_name().to_string_lossy().starts_with(&prefix) {
                    let _ = std::fs::remove_file(f.path());
                }
            }
        }
        for d in m.created_dirs.iter().rev() {
            // only if empty: never removes anything the human put there since
            let _ = std::fs::remove_dir(d);
        }
        Ok(kept)
    }
}
