//! Opening the data at start: a settings block that cannot be read is replaced by the defaults with a copy beside it (G16.4);
//! a state file that cannot be read at all is never replaced on its own: the person is asked (the core's `StoreError::Unreadable`).
use nightshift_core::store::{Store, StoreError};
use std::path::{Path, PathBuf};

pub enum Opened {
    Ready {
        store: Store,
        /// Where the copy of the unreadable settings went (the defaults are in use), for the window's strip.
        replaced: Option<PathBuf>,
    },
    /// The file is untouched; a copy of it is at `copy`.
    Unreadable { copy: String },
}

pub fn open_store(dir: &Path) -> Result<Opened, String> {
    match Store::open(dir) {
        Ok(store) => {
            let replaced = store.settings_recovered().map(Path::to_path_buf);
            Ok(Opened::Ready { store, replaced })
        }
        Err(StoreError::Unreadable { copy }) => Ok(Opened::Unreadable { copy }),
        Err(e) => Err(e.to_string()),
    }
}

/// The confirmation of "start clean" lives here: showing the screen issues a one-time token, only that token starts it.
#[derive(Default)]
pub struct FreshGate(std::sync::Mutex<Option<String>>);

impl FreshGate {
    /// A new token (the one before it stops working).
    pub fn issue(&self) -> String {
        use std::hash::{BuildHasher, Hasher};
        // RandomState is seeded by the system per instance: unguessable enough for a one-shot local token
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        let t = format!("{:016x}{:016x}", h.finish(), std::collections::hash_map::RandomState::new().build_hasher().finish());
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(t.clone());
        t
    }

    /// Starts clean when `token` is the one issued last; it is used up by this. Anything else: `confirm_needed`, nothing moves.
    pub fn start_fresh(&self, dir: &Path, token: &str) -> Result<PathBuf, String> {
        let mut g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if g.as_deref() != Some(token) || token.is_empty() {
            return Err("confirm_needed".into());
        }
        let r = start_fresh(dir);
        if r.is_ok() {
            *g = None;
        }
        r
    }
}

/// "Начать с чистого листа": the unreadable file is moved aside (the copy stays where it is too), so the next open starts empty.
pub fn start_fresh(dir: &Path) -> Result<PathBuf, String> {
    let from = dir.join("state.json");
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    // a name nobody has: an earlier file put aside is never written over, however close in time
    let mut n = 0u32;
    let to = loop {
        let c = dir.join(format!("state.json.unreadable-{stamp}-{n}"));
        if !c.exists() {
            break c;
        }
        n += 1;
    };
    std::fs::rename(&from, &to).map_err(|e| e.to_string())?;
    Ok(to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::Scratch;

    #[test]
    fn an_unreadable_state_file_is_not_replaced_on_its_own_and_a_fresh_start_keeps_everything_it_moved() {
        let s = Scratch::new("startup");
        std::fs::write(s.0.join("state.json"), "{ this is not json").unwrap();
        let Ok(Opened::Unreadable { copy }) = open_store(&s.0) else { panic!("expected Unreadable") };
        assert_eq!(std::fs::read_to_string(&copy).unwrap(), "{ this is not json", "the copy has the bytes");
        assert_eq!(std::fs::read_to_string(s.0.join("state.json")).unwrap(), "{ this is not json", "the file itself is untouched");

        let gate = FreshGate::default();
        assert_eq!(gate.start_fresh(&s.0, "guess").unwrap_err(), "confirm_needed");
        assert_eq!(gate.start_fresh(&s.0, "").unwrap_err(), "confirm_needed");
        let first = gate.issue();
        let second = gate.issue();
        assert_ne!(first, second);
        assert_eq!(gate.start_fresh(&s.0, &first).unwrap_err(), "confirm_needed", "an older token is dead");
        assert!(s.0.join("state.json").exists(), "without the right token nothing moves");
        let moved = gate.start_fresh(&s.0, &second).unwrap();
        std::fs::write(s.0.join("state.json"), "again").unwrap();
        assert_eq!(gate.start_fresh(&s.0, &second).unwrap_err(), "confirm_needed", "a token works once");
        assert!(s.0.join("state.json").exists());
        std::fs::remove_file(s.0.join("state.json")).unwrap();
        assert_eq!(std::fs::read_to_string(moved).unwrap(), "{ this is not json");
        assert!(std::path::Path::new(&copy).exists(), "the copy stays");
        let Ok(Opened::Ready { store, replaced }) = open_store(&s.0) else { panic!("expected a fresh store") };
        assert!(store.projects().is_empty() && replaced.is_none());
    }

    #[test]
    fn two_files_put_aside_in_the_same_instant_keep_their_own_names() {
        let s = Scratch::new("startup3");
        let mut names = std::collections::HashSet::new();
        for i in 0..5 {
            std::fs::write(s.0.join("state.json"), format!("v{i}")).unwrap();
            let moved = start_fresh(&s.0).unwrap();
            assert!(names.insert(moved.clone()));
            assert_eq!(std::fs::read_to_string(moved).unwrap(), format!("v{i}"));
        }
    }

    #[test]
    fn broken_settings_start_with_the_defaults_a_copy_and_the_projects_kept() {
        let s = Scratch::new("startup2");
        let proj = serde_json::json!({ "id": "p1", "name": "demo", "path": "/x", "base_branch": "main", "check_cmd": null, "setup_cmd": null,
            "copy_files": [], "finish": "local", "ask_outward": true, "agent": "claude", "reviewer": null, "fallback_agent": null });
        std::fs::write(s.0.join("state.json"), serde_json::json!({ "projects": [proj], "settings": { "max_parallel": "many" } }).to_string()).unwrap();
        let Ok(Opened::Ready { store, replaced }) = open_store(&s.0) else { panic!("expected Ready") };
        assert_eq!(store.projects().len(), 1);
        assert_eq!(store.settings().max_parallel, 3, "the default");
        let copy = replaced.expect("a copy of the old settings");
        assert!(copy.exists() && std::fs::read_to_string(copy).unwrap().contains("many"));
    }
}
