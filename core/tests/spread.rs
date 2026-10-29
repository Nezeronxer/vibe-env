//! Spread: skills and MCP servers of every installed environment go into the others, on a temporary home and data
//! folder. Nothing here reads or writes the real `~` or the real keyring.

use nightshift_core::kits::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

const GH: &str = "ghp_abcdefghijklmnopqrstuvwxyz0123456789AB";

#[derive(Clone, Default)]
struct FakeKeyring(Arc<Mutex<BTreeMap<String, String>>>);
impl SecretStore for FakeKeyring {
    fn set(&self, service: &str, user: &str, value: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(format!("{service}|{user}"), value.into());
        Ok(())
    }
    fn get(&self, service: &str, user: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().get(&format!("{service}|{user}")).cloned())
    }
}

struct Env {
    _d: TempDir,
    home: PathBuf,
    data: PathBuf,
}

fn env() -> Env {
    let d = tempfile::tempdir().unwrap();
    let (home, data) = (d.path().join("home"), d.path().join("data"));
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&data).unwrap();
    Env { home, data, _d: d }
}

fn put(p: &Path, s: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, s).unwrap();
}

/// Every file under `dir` (symlinks as their target text), relative path → bytes.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, d: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            let md = fs::symlink_metadata(&p).unwrap();
            let rel = p.strip_prefix(root).unwrap().display().to_string();
            if md.file_type().is_symlink() {
                out.insert(rel, fs::read_link(&p).unwrap().display().to_string().into_bytes());
            } else if md.is_dir() {
                out.insert(format!("{rel}/"), vec![]);
                walk(root, &p, out);
            } else {
                out.insert(rel, fs::read(&p).unwrap());
            }
        }
    }
    let mut m = BTreeMap::new();
    walk(dir, dir, &mut m);
    m
}

fn json_of(p: &Path) -> Value {
    serde_json::from_slice(&fs::read(p).unwrap()).unwrap()
}

/// Claude Code: two skills (one runs a script and has a link out of its folder), a stdio server with a secret and a
/// plain variable, an http server with a token by reference. opencode and Qwen: other things, one name taken.
fn homes(e: &Env) {
    let c = e.home.join(".claude");
    put(&c.join("skills/alpha/SKILL.md"), "---\nname: alpha\n---\nAlpha from Claude.\n");
    put(&c.join("skills/beta/SKILL.md"), "---\nname: beta\n---\nRun run.sh\n");
    put(&c.join("skills/beta/run.sh"), "#!/bin/sh\necho beta\n");
    std::os::unix::fs::symlink("../../../.claude.json", c.join("skills/beta/leak.json")).unwrap();
    put(
        &e.home.join(".claude.json"),
        &serde_json::to_string_pretty(&json!({
            "numStartups": 7,
            "mcpServers": {
                "gh": { "type": "stdio", "command": "npx", "args": ["-y", "@mcp/github"], "env": { "GITHUB_TOKEN": GH, "LOG_LEVEL": "debug" } },
                "docs": { "type": "http", "url": "https://docs.example.com/mcp", "headers": { "Authorization": "Bearer ${DOCS_TOKEN}" } },
                "copilot": { "type": "http", "url": "https://api.example.com/mcp/", "headersHelper": "gh-token-helper" }
            }
        }))
        .unwrap(),
    );
    let o = e.home.join(".config/opencode");
    put(&o.join("skills/gamma/SKILL.md"), "---\nname: gamma\n---\nGamma from opencode.\n");
    put(&o.join("skills/alpha/SKILL.md"), "---\nname: alpha\n---\nopencode's own alpha.\n");
    put(
        &o.join("opencode.json"),
        "{\n  \"$schema\": \"https://opencode.ai/config.json\",\n  \"mcp\": {\n    \"fs\": { \"type\": \"local\", \"command\": [\"node\", \"/srv/fs.js\"], \"enabled\": true },\n    \"docs\": { \"type\": \"remote\", \"url\": \"https://other.example.com/mcp\", \"enabled\": true }\n  }\n}\n",
    );
    put(&e.home.join(".qwen/settings.json"), "{\n  \"theme\": \"Dracula\",\n  \"mcpServers\": { \"web\": { \"httpUrl\": \"https://web.example.com/mcp\" } }\n}\n");
}

fn kits(e: &Env, ring: &FakeKeyring) -> Kits {
    Kits::new(&e.home, &e.data).with_secret_store(Box::new(ring.clone()))
}

#[test]
fn every_env_gets_what_the_others_have_in_its_own_format_once_and_the_undo_puts_back_every_byte() {
    let e = env();
    homes(&e);
    let ring = FakeKeyring::default();
    let k = kits(&e, &ring);
    let before = tree(&e.home);

    let r = k.spread(&["claude", "opencode", "qwen", "cursor"]);
    let added: Vec<String> = r.added.iter().map(|a| format!("{}:{}:{}<{}", a.env, a.kind, a.name, a.from)).collect();
    for want in [
        "claude:skill:gamma<opencode",
        "claude:mcp:fs<opencode",
        "claude:mcp:web<qwen",
        "opencode:skill:beta<claude",
        "opencode:mcp:gh<claude",
        "opencode:mcp:web<qwen",
        "qwen:mcp:gh<claude",
        "qwen:mcp:docs<claude",
        "qwen:mcp:fs<opencode",
    ] {
        assert!(added.contains(&want.to_string()), "{want} missing from {added:?}");
    }
    assert_eq!(added.len(), 9, "{added:?}");
    let skipped: Vec<String> = r.skipped.iter().map(|s| format!("{}:{}:{}={}", s.env, s.kind, s.name, s.reason)).collect();
    for want in ["opencode:skill:alpha=conflict", "opencode:mcp:docs=conflict", "qwen:skill:*=layout_unverified", "opencode:skill:beta/leak.json=symlink_outside", "claude:mcp:copilot=unsupported_field"] {
        assert!(skipped.contains(&want.to_string()), "{want} missing from {skipped:?}");
    }
    assert_eq!(r.snapshots.len(), 3, "one snapshot per environment written into");

    // Claude Code: the servers are on (in ~/.claude.json, the CLI's own keys kept), the skill is there
    let cj = json_of(&e.home.join(".claude.json"));
    assert_eq!(cj["numStartups"], 7);
    assert_eq!(cj["mcpServers"]["fs"], json!({ "command": "node", "args": ["/srv/fs.js"] }));
    assert_eq!(cj["mcpServers"]["web"], json!({ "type": "http", "url": "https://web.example.com/mcp" }));
    assert_eq!(cj["mcpServers"]["gh"]["env"]["GITHUB_TOKEN"], GH, "the source keeps its own value");
    assert_eq!(fs::read_to_string(e.home.join(".claude/skills/gamma/SKILL.md")).unwrap(), "---\nname: gamma\n---\nGamma from opencode.\n");

    // opencode: its own format, the secret by reference, the taken names untouched
    let oc = json_of(&e.home.join(".config/opencode/opencode.json"));
    assert_eq!(
        oc["mcp"]["gh"],
        json!({ "type": "local", "command": ["npx", "-y", "@mcp/github"], "enabled": true, "environment": { "GITHUB_TOKEN": "{env:GITHUB_TOKEN}", "LOG_LEVEL": "debug" } })
    );
    assert_eq!(oc["mcp"]["web"], json!({ "type": "remote", "url": "https://web.example.com/mcp", "enabled": true }));
    assert_eq!(oc["mcp"]["docs"]["url"], "https://other.example.com/mcp", "a taken name is not written over");
    assert_eq!(oc["$schema"], "https://opencode.ai/config.json");
    let ob = e.home.join(".config/opencode/skills/beta");
    assert_eq!(fs::read_to_string(ob.join("run.sh")).unwrap(), "#!/bin/sh\necho beta\n", "the skill with a script goes in turned on");
    assert!(!ob.join("leak.json").exists(), "nothing past the skill's folder");
    assert_eq!(fs::read_to_string(e.home.join(".config/opencode/skills/alpha/SKILL.md")).unwrap(), "---\nname: alpha\n---\nopencode's own alpha.\n");

    // Qwen: its format; no skills folder of its own
    let q = json_of(&e.home.join(".qwen/settings.json"));
    assert_eq!(q["theme"], "Dracula");
    assert_eq!(q["mcpServers"]["gh"], json!({ "command": "npx", "args": ["-y", "@mcp/github"], "env": { "GITHUB_TOKEN": "${GITHUB_TOKEN}", "LOG_LEVEL": "debug" } }));
    assert_eq!(q["mcpServers"]["docs"], json!({ "httpUrl": "https://docs.example.com/mcp", "headers": { "Authorization": "Bearer ${DOCS_TOKEN}" } }));
    assert_eq!(q["mcpServers"]["fs"], json!({ "command": "node", "args": ["/srv/fs.js"] }));

    // the secret: in the keyring of each target env and in the runs of that env, never in a file it wrote
    for env in ["opencode", "qwen"] {
        assert!(k.env_vars(env).contains(&("GITHUB_TOKEN".to_string(), GH.to_string())), "{env}");
    }
    assert!(k.env_vars("claude").is_empty());
    let leaked: Vec<String> = tree(&e.home).into_iter().filter(|(p, b)| p != ".claude.json" && String::from_utf8_lossy(b).contains(GH)).map(|(p, _)| p).collect();
    assert!(leaked.is_empty(), "{leaked:?}");
    assert!(!String::from_utf8_lossy(&fs::read(e.data.join("kits/spread.json")).unwrap()).contains(GH));
    assert_eq!(r.secrets, vec!["opencode/GITHUB_TOKEN", "qwen/GITHUB_TOKEN"]);

    // a second call: nothing new, nothing written, no snapshot
    let after = tree(&e.home);
    let data_before = tree(&e.data);
    let r2 = k.spread(&["claude", "opencode", "qwen"]);
    assert!(r2.added.is_empty() && r2.snapshots.is_empty() && r2.secrets.is_empty(), "{r2:?}");
    assert!(!r2.skipped.iter().any(|s| !["conflict", "layout_unverified", "unsupported_field"].contains(&s.reason.as_str())), "{:?}", r2.skipped);
    assert_eq!(tree(&e.home), after);
    assert_eq!(tree(&e.data).keys().collect::<Vec<_>>(), data_before.keys().collect::<Vec<_>>(), "no new snapshot");
    assert_eq!(k.spread_last().unwrap().snapshots, r.snapshots, "the last spread that changed something stays the one to undo");

    // undo: every file back byte for byte, the new ones gone
    let kept = k.spread_undo().unwrap();
    assert!(kept.kept_changed.is_empty() && kept.kept_files.is_empty(), "{kept:?}");
    assert_eq!(tree(&e.home), before);
    assert!(k.spread_last().unwrap().undone);
}

#[test]
fn codex_reads_and_gets_servers_in_toml_and_skills_in_its_catalog_folder() {
    let e = env();
    put(&e.home.join(".claude/skills/alpha/SKILL.md"), "---\nname: alpha\n---\nA.\n");
    put(
        &e.home.join(".claude.json"),
        &json!({ "mcpServers": {
            "gh": { "command": "npx", "args": ["-y", "@mcp/github"], "env": { "GITHUB_TOKEN": GH } },
            "docs": { "type": "http", "url": "https://docs.example.com/mcp", "headers": { "Authorization": "Bearer ${DOCS_TOKEN}" } },
            "inline": { "command": "srv", "args": ["--token", GH] },
            "off": { "command": "x", "disabled": true }
        } })
        .to_string(),
    );
    put(&e.home.join(".codex/config.toml"), "model = \"o3\"\n\n[mcp_servers.tools]\ncommand = \"uvx\"\nargs = [\n  \"mcp-tools\",\n]\nenv = { TOOLS_MODE = \"fast\" }\n");
    let ring = FakeKeyring::default();
    let k = kits(&e, &ring);
    let before = tree(&e.home);
    let r = k.spread(&["claude", "codex"]);
    let added: Vec<String> = r.added.iter().map(|a| format!("{}:{}", a.env, a.name)).collect();
    assert_eq!(added, vec!["claude:tools", "codex:alpha", "codex:docs", "codex:gh"], "{:?}", r.skipped);
    assert!(r.skipped.iter().any(|s| s.name == "inline" && s.reason == "secret_inline"), "a secret in an argument is not copied: {:?}", r.skipped);
    assert!(!r.skipped.iter().any(|s| s.name == "off"), "a server that is off is not spread");

    let cj = json_of(&e.home.join(".claude.json"));
    assert_eq!(cj["mcpServers"]["tools"], json!({ "command": "uvx", "args": ["mcp-tools"], "env": { "TOOLS_MODE": "fast" } }));
    let toml = fs::read_to_string(e.home.join(".codex/config.toml")).unwrap();
    assert!(toml.starts_with("model = \"o3\""), "{toml}");
    assert!(toml.contains("[mcp_servers.gh]\ncommand = \"npx\"\nargs = [\"-y\", \"@mcp/github\"]\nenv_vars = [\"GITHUB_TOKEN\"]\n"), "{toml}");
    assert!(toml.contains("[mcp_servers.docs]\nurl = \"https://docs.example.com/mcp\"\nbearer_token_env_var = \"DOCS_TOKEN\"\n"), "{toml}");
    assert!(!toml.contains(GH));
    assert!(e.home.join(".agents/skills/alpha/SKILL.md").is_file());
    assert_eq!(k.env_vars("codex"), vec![("GITHUB_TOKEN".to_string(), GH.to_string())]);

    // what was written reads back as the same servers: nothing to do the second time
    let r2 = k.spread(&["claude", "codex"]);
    assert!(r2.added.is_empty() && r2.snapshots.is_empty(), "{r2:?}");
    k.spread_undo().unwrap();
    assert_eq!(tree(&e.home), before);
}

/// What the audit of 29.10.2026 reproduced, each fixed: a linked folder in a skill walked it again (two links hung the
/// start), a Codex server in an inline table got a second definition (the file stopped parsing), DEL went into TOML
/// raw, a password with `{word}` inside passed as a variable reference, and the undo kept the secrets it had put in.
#[test]
fn the_audit_cases_are_closed() {
    let e = env();
    let c = e.home.join(".claude");
    put(&c.join("skills/loop/SKILL.md"), "---\nname: loop\n---\nL.\n");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(".", c.join("skills/loop/a")).unwrap();
        std::os::unix::fs::symlink(".", c.join("skills/loop/b")).unwrap();
    }
    put(
        &e.home.join(".claude.json"),
        &json!({ "mcpServers": {
            "gh": { "command": "npx", "args": ["-y", "x"] },
            "del": { "command": "srv", "args": ["a\u{7f}b"] },
            "db": { "command": "db-mcp", "env": { "DB_PASSWORD": "Hunter2{Secret}Pass9" } }
        } })
        .to_string(),
    );
    put(&e.home.join(".config/opencode/opencode.json"), "{}\n");
    let codex_toml = "[mcp_servers]\ngh = { command = \"npx\", args = [\"-y\", \"x\"] }\n";
    put(&e.home.join(".codex/config.toml"), codex_toml);
    let ring = FakeKeyring::default();
    let k = kits(&e, &ring);
    let t = std::time::Instant::now();
    let r = k.spread(&["claude", "opencode", "codex"]);
    assert!(t.elapsed().as_secs() < 5, "a linked folder is not walked again: {:?}", t.elapsed());
    let oc = e.home.join(".config/opencode/skills/loop");
    assert!(oc.join("SKILL.md").is_file() && !oc.join("a").exists() && !oc.join("b").exists());
    assert!(r.skipped.iter().any(|s| s.name == "loop/a" && s.reason == "symlink_dir"), "{:?}", r.skipped);
    // Codex with a server it cannot read is left byte for byte
    assert_eq!(fs::read_to_string(e.home.join(".codex/config.toml")).unwrap(), codex_toml);
    assert!(r.skipped.iter().any(|s| s.env == "codex" && s.kind == "mcp" && s.reason == "unparsed"), "{:?}", r.skipped);
    // the password went to the keyring, the file holds only a reference
    let o = json_of(&e.home.join(".config/opencode/opencode.json"));
    assert_eq!(o["mcp"]["db"]["environment"]["DB_PASSWORD"], "{env:DB_PASSWORD}", "{o}");
    assert!(!fs::read_to_string(e.home.join(".config/opencode/opencode.json")).unwrap().contains("Hunter2"));
    assert_eq!(k.env_vars("opencode"), vec![("DB_PASSWORD".to_string(), "Hunter2{Secret}Pass9".to_string())]);
    // the undo takes the secret back from the runs of opencode
    k.spread_undo().unwrap();
    assert!(k.env_vars("opencode").is_empty(), "{:?}", k.env_vars("opencode"));

    // DEL in an argument is escaped in TOML (a Codex that reads its servers)
    let e2 = env();
    put(&e2.home.join(".claude.json"), &json!({ "mcpServers": { "del": { "command": "srv", "args": ["a\u{7f}b"] } } }).to_string());
    put(&e2.home.join(".codex/config.toml"), "model = \"o3\"\n");
    let ring2 = FakeKeyring::default();
    let r2 = kits(&e2, &ring2).spread(&["claude", "codex"]);
    let toml = fs::read_to_string(e2.home.join(".codex/config.toml")).unwrap();
    assert!(r2.added.iter().any(|a| a.name == "del") && toml.contains("a\\u007Fb") && !toml.contains('\u{7f}'), "{toml}");
}
