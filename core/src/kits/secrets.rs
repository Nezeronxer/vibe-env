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
    /// Forget a value (the spread's undo). A store that cannot forget keeps it; the name leaves the index either way.
    fn delete(&self, _service: &str, _user: &str) -> Result<(), String> {
        Ok(())
    }
}

/// macOS Keychain / Secret Service, through the `keyring` crate.
pub struct SystemKeyring;

impl SecretStore for SystemKeyring {
    fn set(&self, service: &str, user: &str, value: &str) -> Result<(), String> {
        keyring::Entry::new(service, user).and_then(|e| e.set_password(value)).map_err(|e| e.to_string())
    }
    fn delete(&self, service: &str, user: &str) -> Result<(), String> {
        match keyring::Entry::new(service, user).and_then(|e| e.delete_credential()) {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
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

/// Families of variables a kit may never set (the spec names none; these are classes, not single names). Each one
/// either makes a program the run starts load or run other code, or sends the run and its keys somewhere else:
/// - where programs and their start files are found: PATH, HOME, SHELL, ZDOTDIR, ENV, IFS, PS4, PROMPT_COMMAND,
///   SHELLOPTS, BASHOPTS, `BASH_*`, `ZSH*`, `XDG_*`, `*_HOME`, `*_CONFIG_DIR`;
/// - the dynamic loader: `LD_*`, `DYLD_*`;
/// - git and ssh (config by variables, ssh / askpass commands, exec and template folders): `GIT_*`, `SSH_*`, `*ASKPASS`;
/// - interpreters' search paths and options: `PYTHON*`, `PERL*`, `RUBY*`, `NODE_*`, `NPM_CONFIG_*`, `JAVA_*`, `_JAVA_*`,
///   CLASSPATH, `*_PATH`, `*_OPTIONS`, `*_OPTS`, `*LIB`;
/// - programs a CLI starts by name: EDITOR, VISUAL, PAGER, BROWSER;
/// - endpoints, proxies and TLS: `*_BASE_URL`, `*_API_BASE`, `*_PROXY`, `SSL_*`, `*_CA_CERTS`, `*_CA_BUNDLE`, `*_TLS_*`.
///
/// Names are compared in upper case (`http_proxy` is read too).
const FORBIDDEN_NAMES: [&str; 15] = ["PATH", "HOME", "SHELL", "ZDOTDIR", "ENV", "IFS", "PS4", "PROMPT_COMMAND", "SHELLOPTS", "BASHOPTS", "CLASSPATH", "EDITOR", "VISUAL", "PAGER", "BROWSER"];
const FORBIDDEN_PREFIXES: [&str; 16] = ["BASH_", "ZSH", "XDG_", "LD_", "DYLD_", "GIT_", "SSH_", "PYTHON", "PERL", "RUBY", "NODE_", "NPM_CONFIG_", "JAVA_", "_JAVA_", "SSL_", "CURL_"];
const FORBIDDEN_SUFFIXES: [&str; 12] = ["_HOME", "_CONFIG_DIR", "ASKPASS", "_PATH", "_OPTIONS", "_OPTS", "LIB", "_BASE_URL", "_API_BASE", "_PROXY", "_CA_CERTS", "_CA_BUNDLE"];

/// The only names a kit may set: ones that look like a secret, the same signs the export looks for (§26). A list of
/// what is safe, not of what is dangerous: CLI variables like `CLAUDE_ENV_FILE`, `OPENCODE_CONFIG` or
/// `CODE_ASSIST_ENDPOINT` run a file or move the key elsewhere and no list of classes names all of them.
const SECRET_SUFFIXES: [&str; 4] = ["_KEY", "_TOKEN", "_SECRET", "_PASSWORD"];

/// Whether a kit may set the variable `name` at all: a secret-like name (`SECRET_SUFFIXES`) outside every forbidden
/// class (`FORBIDDEN_NAMES`).
pub fn env_name_allowed(name: &str) -> bool {
    let n = name.to_ascii_uppercase();
    valid_name(name)
        && SECRET_SUFFIXES.iter().any(|x| n.ends_with(x))
        && !FORBIDDEN_NAMES.contains(&n.as_str())
        && !FORBIDDEN_PREFIXES.iter().any(|p| n.starts_with(p))
        && !FORBIDDEN_SUFFIXES.iter().any(|x| n.ends_with(x))
        && !n.contains("_TLS_")
}

/// Whether `value` keeps the shape of the variable `name` in a run or a terminal (the name itself: `env_name_allowed`;
/// the terminal also carries variables the app set, like the CLI homes). HOME, TMPDIR, the CLI homes and the XDG
/// folders an absolute path, PATH and XDG_*_DIRS a list of them, LANG / LC_ALL a locale name.
/// No value may hold a NUL (the process would not start at all) or a line break (it reads as one more variable).
pub fn env_value_ok(name: &str, value: &str) -> bool {
    if value.contains(['\0', '\n', '\r']) {
        return false;
    }
    let path = |p: &str| std::path::Path::new(p).is_absolute() && !p.contains(['=', ':']);
    match name {
        "PATH" => value.split(':').all(path),
        "HOME" | "TMPDIR" | "CODEX_HOME" | "CLAUDE_CONFIG_DIR" | "XDG_RUNTIME_DIR" | "KIMI_CODE_HOME" | "COPILOT_HOME" | "GROK_HOME"
        | "GOOSE_PATH_ROOT" => path(value),
        // XDG_DATA_DIRS / XDG_CONFIG_DIRS are lists like PATH; XDG_*_HOME one folder
        n if n.starts_with("XDG_") && n.ends_with("_DIRS") => value.split(':').all(path),
        n if n.starts_with("XDG_") && n.ends_with("_HOME") => path(value),
        "LANG" | "LC_ALL" => !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '@' | '-')),
        _ => true,
    }
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

    /// The value goes from the keyring and the fallback file, the name from the index: runs of `env` stop getting it.
    pub fn forget(&self, store: &dyn SecretStore, env: &str, name: &str) -> Result<(), KitError> {
        let user = format!("{env}/{name}");
        let _ = store.delete(SERVICE, &user);
        let mut m = self.file_map();
        if m.remove(&user).is_some() {
            write_private(&self.file_path(), &serde_json::to_vec_pretty(&m).unwrap())?;
        }
        let mut all = self.names();
        if let Some(v) = all.get_mut(env) {
            v.retain(|x| x != name);
        }
        super::snap::write_atomic(&self.names_path(), &serde_json::to_vec_pretty(&all).unwrap(), None)
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
