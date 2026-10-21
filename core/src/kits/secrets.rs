//! Secret values for kits: the system keyring (service `nightshift`, user `<env>/<NAME>`),
//! a 0600 file when there is no keyring. Only names are kept anywhere else.

use super::KitError;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SERVICE: &str = "nightshift";

/// Where secret values live. Swappable so tests never touch the real keyring.
pub trait SecretStore: Send + Sync {
    fn set(&self, service: &str, user: &str, value: &str) -> Result<(), String>;
    fn get(&self, service: &str, user: &str) -> Result<Option<String>, String>;
}

/// macOS Keychain / Secret Service, through the `keyring` crate.
pub struct SystemKeyring;

impl SecretStore for SystemKeyring {
    fn set(&self, service: &str, user: &str, value: &str) -> Result<(), String> {
        keyring::Entry::new(service, user).and_then(|e| e.set_password(value)).map_err(|e| e.to_string())
    }
    fn get(&self, service: &str, user: &str) -> Result<Option<String>, String> {
        match keyring::Entry::new(service, user).and_then(|e| e.get_password()) {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Where a secret ended up.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum SecretSaved {
    Keyring,
    /// No keyring: a file readable only by the user; the UI shows `warning` (a key).
    File { path: PathBuf, warning: String },
}

pub(crate) fn valid_name(n: &str) -> bool {
    let mut c = n.chars();
    matches!(c.next(), Some(ch) if ch.is_ascii_alphabetic() || ch == '_')
        && n.len() <= 128
        && n.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

pub(crate) struct Secrets {
    pub dir: PathBuf,
}

impl Secrets {
    fn names_path(&self) -> PathBuf {
        self.dir.join("secret-names.json")
    }
    pub fn file_path(&self) -> PathBuf {
        self.dir.join("secrets.json")
    }

    /// env → names set through `set_secret` (names only, no values).
    pub fn names(&self) -> BTreeMap<String, Vec<String>> {
        std::fs::read(self.names_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn add_name(&self, env: &str, name: &str) -> Result<(), KitError> {
        let mut all = self.names();
        let v = all.entry(env.to_string()).or_default();
        if !v.iter().any(|x| x == name) {
            v.push(name.to_string());
        }
        super::snap::write_atomic(&self.names_path(), &serde_json::to_vec_pretty(&all).unwrap(), None)
    }

    fn file_map(&self) -> BTreeMap<String, String> {
        std::fs::read(self.file_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn set(&self, store: &dyn SecretStore, env: &str, name: &str, value: &str) -> Result<SecretSaved, KitError> {
        let user = format!("{env}/{name}");
        let saved = match store.set(SERVICE, &user, value) {
            Ok(()) => {
                // a value left in the fallback file from an earlier run goes away
                let mut m = self.file_map();
                if m.remove(&user).is_some() {
                    write_private(&self.file_path(), &serde_json::to_vec_pretty(&m).unwrap())?;
                }
                SecretSaved::Keyring
            }
            Err(_) => {
                let mut m = self.file_map();
                m.insert(user, value.to_string());
                write_private(&self.file_path(), &serde_json::to_vec_pretty(&m).unwrap())?;
                SecretSaved::File { path: self.file_path(), warning: "kit_secret_no_keyring".into() }
            }
        };
        self.add_name(env, name)?;
        Ok(saved)
    }

    pub fn get(&self, store: &dyn SecretStore, env: &str, name: &str) -> Option<String> {
        let user = format!("{env}/{name}");
        if let Ok(Some(v)) = store.get(SERVICE, &user) {
            return Some(v);
        }
        self.file_map().get(&user).cloned()
    }
}

fn write_private(p: &Path, b: &[u8]) -> Result<(), KitError> {
    super::snap::write_atomic(p, b, Some(0o600))
}
