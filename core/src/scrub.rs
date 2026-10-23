//! Keeps secrets and the home folder out of what leaves the app's hands (an exported kit, the error
//! report, the app log): values under secret-looking names and token-looking strings become `${NAME}`,
//! home paths become `~`. Moved out of `kits` (G71.1); kits uses it as before.

use regex::Regex;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

/// gitleaks-style rules: (variable name used for the replacement, pattern).
fn token_rules() -> &'static [(&'static str, Regex)] {
    static R: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    R.get_or_init(|| {
        let r = |s: &str| Regex::new(s).unwrap();
        vec![
            ("PRIVATE_KEY", r(r"-----BEGIN[A-Z ]*PRIVATE KEY( BLOCK)?-----[\s\S]*?-----END[A-Z ]*PRIVATE KEY( BLOCK)?-----")),
            ("ANTHROPIC_API_KEY", r(r"sk-ant-[A-Za-z0-9_\-]{20,}")),
            ("OPENAI_API_KEY", r(r"sk-(?:proj-|svcacct-|admin-)?[A-Za-z0-9_\-]{20,}")),
            ("GITHUB_TOKEN", r(r"\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{22,})")),
            ("GITLAB_TOKEN", r(r"\bglpat-[A-Za-z0-9_\-]{20,}")),
            ("AWS_ACCESS_KEY_ID", r(r"\b(?:AKIA|ASIA|AGPA|AIDA|AROA)[0-9A-Z]{16}\b")),
            ("SLACK_TOKEN", r(r"\bxox[abposr]-[A-Za-z0-9\-]{10,}")),
            ("GOOGLE_API_KEY", r(r"\bAIza[0-9A-Za-z_\-]{35}")),
            ("STRIPE_KEY", r(r"\b(?:sk|rk|pk)_(?:live|test)_[0-9A-Za-z]{16,}")),
            ("NPM_TOKEN", r(r"\bnpm_[A-Za-z0-9]{36}")),
            ("HF_TOKEN", r(r"\bhf_[A-Za-z0-9]{30,}")),
            ("TELEGRAM_BOT_TOKEN", r(r"\b\d{8,10}:AA[A-Za-z0-9_\-]{30,}")),
            ("JWT", r(r"\beyJ[A-Za-z0-9_\-]{8,}\.eyJ[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}")),
        ]
    })
}

/// `NAME=value` / `name: value` in free text, for names that clearly hold a secret.
fn assign_rule() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r#"(?i)\b([A-Za-z0-9_]*(?:token|secret|passw(?:or)?d|api_?key|access_?key|private_?key|auth_?key|client_?key)[A-Za-z0-9_]*)(["']?\s*[:=]\s*)(["']?)([^\s"'$<{][^\s"']{7,})"#,
        )
        .unwrap()
    })
}

/// Bearer credentials in headers / commands.
/// `scheme://user:pass@host` - the userinfo part.
fn userinfo_rule() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"\b([A-Za-z][A-Za-z0-9+.\-]*)://([^/\s:@'"`$]+:[^@\s/'"`]+)@"#).unwrap())
}

/// A command-line flag whose next argument is a secret: `--token`, `--api-key`, `-password`.
fn secret_flag() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)^--?([A-Za-z0-9_\-]*(?:token|key|secret|passw(?:or)?d)[A-Za-z0-9_\-]*)$").unwrap())
}

fn bearer_rule() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)\b(bearer|basic)(\s+)([A-Za-z0-9._~+/=\-]{16,})").unwrap())
}

/// Key names (config keys, env names) whose string value is a secret (spec §26: *TOKEN*,
/// *KEY*, *SECRET*, *PASSWORD*; plus authorization / credential headers).
pub(crate) fn is_secret_name(k: &str) -> bool {
    let u = k.to_ascii_uppercase();
    ["TOKEN", "KEY", "SECRET", "PASSWORD", "PASSWD", "AUTHORIZATION", "CREDENTIAL"]
        .iter()
        .any(|w| u.contains(w))
}

fn env_ref() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}").unwrap())
}

fn home_rule() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"/(?:Users|home)/[A-Za-z0-9._\-]+(/|$|[\s'`;:,)\]}]|\x22)").unwrap())
}

/// `apiKey` / `api-key` → `API_KEY`.
pub(crate) fn var_name(key: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for c in key.chars() {
        if c.is_ascii_uppercase() && prev_lower {
            out.push('_');
        }
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
        prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("SECRET_{out}")
    } else {
        out
    }
}

#[derive(Default)]
pub(crate) struct Scrubber {
    home: String,
    /// variable name → who needs it
    pub vars: BTreeMap<String, BTreeSet<String>>,
    /// secret value → variable name
    values: BTreeMap<String, String>,
    pub replaced: usize,
}

impl Scrubber {
    pub fn new(home: &str) -> Self {
        Scrubber { home: home.trim_end_matches('/').to_string(), ..Default::default() }
    }

    fn need(&mut self, name: &str, used_by: &str) {
        self.vars.entry(name.to_string()).or_default().insert(used_by.to_string());
    }

    /// Name for `value`; the same value keeps its name, another value under a taken name gets `_2`.
    fn name_for(&mut self, base: &str, value: &str) -> String {
        if let Some(n) = self.values.get(value) {
            return n.clone();
        }
        let mut name = base.to_string();
        let mut i = 2;
        while self.values.values().any(|n| n == &name) {
            name = format!("{base}_{i}");
            i += 1;
        }
        self.values.insert(value.to_string(), name.clone());
        self.replaced += 1;
        name
    }

    /// Records `${NAME}` references that a config already uses.
    pub fn note_refs(&mut self, s: &str, used_by: &str) {
        let names: Vec<String> = env_ref().captures_iter(s).map(|c| c[1].to_string()).collect();
        for n in names {
            self.need(&n, used_by);
        }
    }

    /// Structured pass over a JSON config value.
    pub fn json(&mut self, v: &mut Value, used_by: &str) {
        match v {
            Value::Object(m) => {
                for (k, val) in m.iter_mut() {
                    if let Value::String(s) = val {
                        if is_secret_name(k) && !s.is_empty() && !s.starts_with('$') {
                            let name = self.name_for(&var_name(k), s);
                            self.need(&name, used_by);
                            *s = format!("${{{name}}}");
                            continue;
                        }
                    }
                    self.json(val, used_by);
                }
            }
            Value::Array(a) => {
                let mut flag: Option<String> = None;
                for x in a.iter_mut() {
                    if let (Some(f), Value::String(s)) = (&flag, &mut *x) {
                        if !s.is_empty() && !s.starts_with(['$', '-']) {
                            let name = self.name_for(&var_name(f), s);
                            self.need(&name, used_by);
                            *s = format!("${{{name}}}");
                            flag = None;
                            continue;
                        }
                    }
                    flag = x.as_str().and_then(|s| secret_flag().captures(s)).map(|c| c[1].to_string());
                    self.json(x, used_by);
                }
            }
            Value::String(s) => {
                *s = self.text(s, used_by);
            }
            _ => {}
        }
    }

    /// TOML lines `key = "value"` under a secret-looking key, then the text rules.
    pub fn toml(&mut self, s: &str, used_by: &str) -> String {
        static KV: OnceLock<Regex> = OnceLock::new();
        let kv = KV.get_or_init(|| Regex::new(r#"^(\s*)("?[A-Za-z0-9_.\-]+"?)(\s*=\s*)(["'])(.*)(["'])(\s*(?:#.*)?)$"#).unwrap());
        let mut out = String::new();
        for line in s.split_inclusive('\n') {
            let (body, nl) = match line.strip_suffix('\n') {
                Some(b) => (b, "\n"),
                None => (line, ""),
            };
            if let Some(c) = kv.captures(body) {
                let key = c[2].trim_matches('"');
                let val = &c[5];
                if is_secret_name(key) && !val.is_empty() && !val.starts_with('$') && c[4] == c[6] {
                    let name = self.name_for(&var_name(key), val);
                    self.need(&name, used_by);
                    out.push_str(&format!("{}{}{}{}${{{name}}}{}{}{nl}", &c[1], &c[2], &c[3], &c[4], &c[6], &c[7]));
                    continue;
                }
            }
            out.push_str(&self.text(body, used_by));
            out.push_str(nl);
        }
        out
    }

    /// Text rules: token shapes, `NAME=value`, bearer headers, home paths.
    pub fn text(&mut self, s: &str, used_by: &str) -> String {
        let mut t = s.to_string();
        for (name, re) in token_rules() {
            let found: Vec<String> = re.find_iter(&t).map(|m| m.as_str().to_string()).collect();
            for v in found {
                let n = self.name_for(name, &v);
                self.need(&n, used_by);
                t = t.replace(&v, &format!("${{{n}}}"));
            }
        }
        let caps: Vec<(String, String)> =
            assign_rule().captures_iter(&t).map(|c| (c[1].to_string(), c[4].to_string())).collect();
        for (key, v) in caps {
            // plain words ("token: string") are documentation, not secrets
            let looks_secret = v.chars().any(|c| c.is_ascii_digit()) || v.len() >= 20;
            if !looks_secret {
                continue;
            }
            let n = self.name_for(&var_name(&key), &v);
            self.need(&n, used_by);
            t = t.replace(&v, &format!("${{{n}}}"));
        }
        let caps: Vec<(String, String)> = userinfo_rule().captures_iter(&t).map(|c| (c[1].to_string(), c[2].to_string())).collect();
        for (scheme, v) in caps {
            let n = self.name_for(&format!("{}_USERINFO", var_name(&scheme)), &v);
            self.need(&n, used_by);
            t = t.replace(&format!("://{v}@"), &format!("://${{{n}}}@"));
        }
        let caps: Vec<String> = bearer_rule().captures_iter(&t).map(|c| c[3].to_string()).collect();
        for v in caps {
            if v.starts_with('$') {
                continue;
            }
            let n = self.name_for("AUTH_TOKEN", &v);
            self.need(&n, used_by);
            t = t.replace(&v, &format!("${{{n}}}"));
        }
        self.home_paths(&t)
    }

    /// `/Users/<name>/…` and `/home/<name>/…` → `~/…`.
    pub fn home_paths(&self, s: &str) -> String {
        let mut t = s.to_string();
        if self.home.len() > 1 {
            t = t.replace(&format!("{}/", self.home), "~/");
            if t.ends_with(&self.home) {
                let cut = t.len() - self.home.len();
                t.replace_range(cut.., "~");
            }
        }
        home_rule().replace_all(&t, "~$1").to_string()
    }

    /// Last pass over finished text: any already-known secret value left anywhere goes too.
    pub fn final_pass(&self, s: &str) -> String {
        let mut t = s.to_string();
        for (v, n) in &self.values {
            if v.len() >= 6 {
                t = t.replace(v.as_str(), &format!("${{{n}}}"));
            }
        }
        t
    }

    /// Bytes that cannot be scrubbed (not UTF-8) are only allowed when nothing secret shows.
    pub fn binary_is_clean(&self, b: &[u8]) -> bool {
        let t = String::from_utf8_lossy(b);
        !token_rules().iter().any(|(_, r)| r.is_match(&t))
            && !self.values.keys().any(|v| v.len() >= 6 && t.contains(v.as_str()))
    }
}

/// One pass of the text rules over `s` (tokens, `NAME=value`, userinfo, bearer, home paths), for text
/// that leaves the app: the error report, the app log.
pub fn clean(s: &str, home: &str) -> String {
    let mut sc = Scrubber::new(home);
    let t = sc.text(s, "report");
    sc.final_pass(&t)
}

/// Names a kit file must never carry, whatever its group.
pub(crate) fn is_secret_file(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel).to_ascii_lowercase();
    name.starts_with(".env")
        || name.ends_with(".pem")
        || name.ends_with(".key")
        || name.ends_with(".p12")
        || name.ends_with(".pfx")
        || name.starts_with("id_rsa")
        || name.starts_with("id_ed25519")
        || name.starts_with("id_ecdsa")
        || name == ".netrc"
        || name.starts_with("credentials")
        || name.starts_with(".credentials")
}
