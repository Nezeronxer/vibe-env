//! Seam 4: `Kits` on a temporary home folder and a temporary data folder. Nothing here reads
//! or writes the real `~`, the real keyring or the network.

use nightshift_core::kits::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

// Secret values planted in the "home" - none of them may reach a kit.
const GH: &str = "ghp_abcdefghijklmnopqrstuvwxyz0123456789AB";
const ANT: &str = "sk-ant-api03-Zx9Qw8Er7Ty6Ui5Op4As3Df2Gh1Jk0LzXcVb";
const SLACK: &str = "xoxb-123456789012-abcdefABCDEF";
const PASS: &str = "hunter2-very-long-pass";
const DB_PASSWORD: &str = "pg-s3cr3t-value-77";
const DB_URL_PASS: &str = "Url-Pa55word";
const ARG_KEY: &str = "zz9-arg-key-value";
const OAUTH: &str = "oauth-account-uuid-4242";

/// In-memory keyring; `fail` = no keyring on this machine.
#[derive(Clone, Default)]
struct FakeKeyring {
    vals: Arc<Mutex<BTreeMap<String, String>>>,
    fail: bool,
}
impl SecretStore for FakeKeyring {
    fn set(&self, service: &str, user: &str, value: &str) -> Result<(), String> {
        if self.fail {
            return Err("no keyring".into());
        }
        self.vals.lock().unwrap().insert(format!("{service}|{user}"), value.into());
        Ok(())
    }
    fn get(&self, service: &str, user: &str) -> Result<Option<String>, String> {
        if self.fail {
            return Err("no keyring".into());
        }
        Ok(self.vals.lock().unwrap().get(&format!("{service}|{user}")).cloned())
    }
}

struct Env {
    _d: TempDir,
    home: PathBuf,
    data: PathBuf,
    out: PathBuf,
}

fn env() -> Env {
    let d = tempfile::tempdir().unwrap();
    let home = d.path().join("home");
    let data = d.path().join("data");
    let out = d.path().join("out");
    for p in [&home, &data, &out] {
        fs::create_dir_all(p).unwrap();
    }
    Env { home, data, out, _d: d }
}

fn kits(e: &Env) -> Kits {
    Kits::new(&e.home, &e.data).with_secret_store(Box::new(FakeKeyring::default()))
}

fn put(p: &Path, s: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, s).unwrap();
}

/// Every file under `dir` (relative path → bytes).
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, d: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            let md = fs::symlink_metadata(&p).unwrap();
            if md.is_dir() {
                walk(root, &p, out);
            } else {
                out.insert(p.strip_prefix(root).unwrap().display().to_string(), fs::read(&p).unwrap_or_default());
            }
        }
    }
    let mut m = BTreeMap::new();
    walk(dir, dir, &mut m);
    m
}

fn all_text(dir: &Path) -> String {
    tree(dir).values().map(|b| String::from_utf8_lossy(b).to_string()).collect::<Vec<_>>().join("\n")
}

/// A Claude Code home with a secret in every place a secret can hide.
fn claude_home(e: &Env) {
    let c = e.home.join(".claude");
    put(&c.join("CLAUDE.md"), &format!("# Rules\nUse token {GH} for the API.\nNotes live in {}/notes and /Users/alice/work.\n", e.home.display()));
    put(
        &c.join("settings.json"),
        &serde_json::json!({
            "model": "opus",
            "env": { "ANTHROPIC_API_KEY": ANT, "DB_PASSWORD": DB_PASSWORD, "EDITOR": "vim" },
            "permissions": { "allow": ["Bash(ls)"] },
            "hooks": { "PreToolUse": [ { "matcher": "Bash", "hooks": [ { "type": "command", "command": format!("bash {}/.claude/hooks/guard.sh", e.home.display()) } ] } ] }
        })
        .to_string(),
    );
    put(&c.join("hooks/guard.sh"), &format!("#!/bin/sh\nexport SLACK_TOKEN={SLACK}\necho checked\n"));
    put(&c.join("skills/notes/SKILL.md"), &format!("---\nname: notes\n---\nLogin with password = {PASS}\n"));
    put(&c.join("skills/deploy/SKILL.md"), "---\nname: deploy\n---\nRun run.sh\n");
    put(&c.join("skills/deploy/run.sh"), "#!/bin/sh\necho deploy\n");
    // user-scope MCP of Claude Code lives in ~/.claude.json next to the CLI's own state
    put(
        &e.home.join(".claude.json"),
        &serde_json::json!({
            "oauthAccount": { "accountUuid": OAUTH },
            "projects": { "/x": { "history": ["private"] } },
            "mcpServers": {
                "db": { "command": "npx", "args": ["-y", "pg-mcp", "--api-key", ARG_KEY], "env": { "DATABASE_URL": format!("postgres://app:{DB_URL_PASS}@db.local/main") } }
            }
        })
        .to_string(),
    );
    put(&c.join("skills/notes/.env"), "OPENAI_API_KEY=sk-should-never-leave-1234567890\n");
    // a link from a skill into the key store of the same home
    put(&e.home.join(".ssh/id_work"), "ssh-key-material-that-must-stay\n");
    std::os::unix::fs::symlink(e.home.join(".ssh/id_work"), c.join("skills/notes/ref.txt")).unwrap();
}

fn all_groups() -> Vec<Group> {
    Group::ALL.to_vec()
}

#[test]
fn export_replaces_secrets_and_home_paths_and_keeps_names() {
    let e = env();
    claude_home(&e);
    let k = kits(&e);

    let inv = k.inventory("claude", &Scope::Global);
    let g = |g: Group| inv.groups.iter().find(|x| x.group == g).unwrap().clone();
    assert_eq!(g(Group::Skills).count, 2);
    assert!(g(Group::Skills).active);
    assert!(!g(Group::Agents).active, "empty group is inactive");
    assert_eq!(g(Group::Mcp).items, vec!["db".to_string()], "user-scope MCP from ~/.claude.json");
    assert!(inv.dangerous.contains(&"env".to_string()) && inv.dangerous.contains(&"permissions".to_string()), "{:?}", inv.dangerous);
    assert!(inv.found);
    assert_eq!(inv.dir, "~/.claude");

    let dest = e.out.join("kit");
    let rep = k
        .export("claude", &Selection { name: "my env".into(), groups: all_groups(), scope: Scope::Global }, ExportDest::Folder(dest.clone()))
        .unwrap();

    let text = all_text(&dest);
    for secret in [GH, ANT, SLACK, PASS, DB_PASSWORD, "sk-should-never-leave", "ssh-key-material", DB_URL_PASS, ARG_KEY, OAUTH, "private"] {
        assert!(!text.contains(secret), "secret value leaked into the kit");
    }
    assert!(!text.contains(&e.home.display().to_string()), "home path leaked");
    assert!(!text.contains("/Users/alice"), "foreign home path leaked");
    assert!(text.contains("~/notes") && text.contains("~/work"));
    assert!(text.contains("bash ~/.claude/hooks/guard.sh"));
    assert!(!dest.join("skills/notes/.env").exists());

    let kj: serde_json::Value = serde_json::from_slice(&fs::read(dest.join("kit.json")).unwrap()).unwrap();
    assert_eq!(kj["env"], "claude");
    assert_eq!(kj["kind"], "nightshift-kit");
    let names: Vec<String> = kj["env_vars"].as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap().to_string()).collect();
    for n in ["ANTHROPIC_API_KEY", "DB_PASSWORD", "GITHUB_TOKEN", "SLACK_TOKEN"] {
        assert!(names.contains(&n.to_string()), "{n} missing from kit.json: {names:?}");
    }
    let mcp = fs::read_to_string(dest.join(".mcp.json")).unwrap();
    assert!(mcp.contains("postgres://${POSTGRES_USERINFO}@db.local/main"), "{mcp}");
    assert!(mcp.contains("\"--api-key\",\n") && mcp.contains("${API_KEY}"), "{mcp}");
    assert!(rep.dangerous.contains(&"env".to_string()));
    let settings = fs::read_to_string(dest.join("settings.json")).unwrap();
    assert!(settings.contains("\"ANTHROPIC_API_KEY\": \"${ANTHROPIC_API_KEY}\""));
    assert!(settings.contains("\"EDITOR\": \"vim\""), "non-secret values stay");
    // every listed file has its sha256
    assert!(kj["files"].as_array().unwrap().iter().all(|f| f["sha256"].as_str().unwrap().len() == 64));
    assert!(rep.secrets_replaced >= 5);
}

fn git(dir: &Path, args: &[&str]) -> String {
    let o = std::process::Command::new("git").current_dir(dir).args(args).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

fn sel(groups: &[Group]) -> Selection {
    Selection { name: "my env".into(), groups: groups.to_vec(), scope: Scope::Global }
}

#[test]
fn export_to_zip_and_to_a_local_git_repository() {
    let e = env();
    claude_home(&e);
    let k = kits(&e);

    let zip = e.out.join("kit.zip");
    k.export("claude", &sel(&[Group::Rules, Group::Skills]), ExportDest::Zip(zip.clone())).unwrap();
    let raw = fs::read(&zip).unwrap();
    assert!(raw.starts_with(b"PK"), "a zip file");
    // the zip is a kit the import side accepts
    let plan = k.preview("claude", KitSource::Zip(zip), &Scope::Global).unwrap();
    assert_eq!(plan.kit_name, "my env");
    assert!(plan.entries.iter().any(|p| p.key == "rules:CLAUDE.md"));

    let repo = e.out.join("repo");
    let rep = k.export("claude", &sel(&[Group::Rules, Group::Skills]), ExportDest::Git(repo.clone())).unwrap();
    assert_eq!(git(&repo, &["rev-list", "--count", "HEAD"]), "1");
    assert_eq!(rep.commit.as_deref(), Some(git(&repo, &["rev-parse", "HEAD"]).as_str()));
    assert_eq!(git(&repo, &["status", "--porcelain"]), "", "everything committed");
    assert_eq!(git(&repo, &["remote"]), "", "nothing is sent anywhere: no remote");
    assert!(git(&repo, &["ls-files"]).lines().any(|l| l == "kit.json"));

    // a destination that is not empty is refused
    assert!(matches!(
        k.export("claude", &sel(&[Group::Rules]), ExportDest::Folder(repo)),
        Err(KitError::DestNotEmpty(_))
    ));
}

/// Export the planted Claude home into a kit folder; a second, clean home receives it.
fn kit_from(e: &Env, groups: &[Group]) -> PathBuf {
    let src = env();
    claude_home(&src);
    let dest = e.out.join(format!("kit-{}", uuid_like()));
    kits(&src).export("claude", &sel(groups), ExportDest::Folder(dest.clone())).unwrap();
    dest
}

fn uuid_like() -> String {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    N.fetch_add(1, Ordering::SeqCst).to_string()
}

fn entry<'a>(p: &'a ImportPlan, key: &str) -> &'a PlanEntry {
    p.entries.iter().find(|x| x.key == key).unwrap_or_else(|| panic!("no {key} in {:?}", p.entries.iter().map(|x| &x.key).collect::<Vec<_>>()))
}

#[test]
fn preview_writes_nothing_and_shows_add_replace_same() {
    let e = env();
    let kit = kit_from(&e, &[Group::Rules, Group::Skills, Group::Settings]);
    let c = e.home.join(".claude");
    put(&c.join("CLAUDE.md"), "# My own rules\n");
    put(&c.join("skills/deploy/SKILL.md"), "---\nname: deploy\n---\nRun run.sh\n");
    put(&c.join("skills/deploy/run.sh"), "#!/bin/sh\necho deploy\n");
    let project = e.out.join("proj");
    fs::create_dir_all(&project).unwrap();

    let before = (tree(&e.home), tree(&e.data), tree(&project));
    let k = kits(&e);
    let plan = k.preview("claude", KitSource::Folder(kit.clone()), &Scope::Global).unwrap();
    let pplan = k.preview("claude", KitSource::Folder(kit), &Scope::Project(project.clone())).unwrap();
    assert_eq!(before, (tree(&e.home), tree(&e.data), tree(&project)), "preview changed the disk");

    assert_eq!(plan.target_dir, "~/.claude");
    let rules = entry(&plan, "rules:CLAUDE.md");
    assert_eq!(rules.change, Change::Replace);
    let d = rules.diff.as_deref().unwrap();
    assert!(d.contains("- # My own rules") && d.contains("+ # Rules"), "{d}");
    assert_eq!(entry(&plan, "skill:notes").change, Change::Add);
    assert_eq!(entry(&plan, "skill:deploy").change, Change::Same);
    assert_eq!(entry(&plan, "setting:model").change, Change::Add);

    // only into the project: everything is new there
    assert_eq!(pplan.target_dir, project.display().to_string());
    assert_eq!(entry(&pplan, "rules:CLAUDE.md").change, Change::Add);
    assert!(entry(&pplan, "rules:CLAUDE.md").target.contains("proj/.claude/CLAUDE.md"));
}

fn codex_home(e: &Env) {
    let c = e.home.join(".codex");
    put(&c.join("AGENTS.md"), "Be brief.\n");
    put(
        &c.join("config.toml"),
        &format!(
            "model = \"gpt-5\"\n\n[mcp_servers.github]\ncommand = \"npx\"\nargs = [\"-y\", \"@modelcontextprotocol/server-github\"]\n\n[mcp_servers.github.env]\nGITHUB_PERSONAL_ACCESS_TOKEN = \"{GH}\"\n\n[tui]\ntheme = \"dark\"\n"
        ),
    );
    put(&c.join("skills/x/SKILL.md"), "not a confirmed place\n");
}

#[test]
fn a_kit_goes_only_into_its_own_environment_and_unverified_groups_are_off() {
    let e = env();
    codex_home(&e);
    let k = kits(&e);

    let inv = k.inventory("codex", &Scope::Global);
    for g in [Group::Skills, Group::Agents] {
        let gi = inv.groups.iter().find(|x| x.group == g).unwrap();
        assert!(!gi.available && !gi.active && gi.note.as_deref() == Some("layout_unverified"), "{g:?}");
    }
    let mcp = inv.groups.iter().find(|x| x.group == Group::Mcp).unwrap();
    assert_eq!(mcp.items, vec!["github".to_string()]);
    let cursor = k.inventory("cursor", &Scope::Global);
    assert!(cursor.groups.iter().all(|g| !g.available && !g.active && g.note.as_deref() == Some("layout_unverified")));

    let kit = e.out.join("codex-kit");
    k.export("codex", &sel(&all_groups()), ExportDest::Folder(kit.clone())).unwrap();
    let text = all_text(&kit);
    assert!(!text.contains(GH));
    assert!(text.contains("GITHUB_PERSONAL_ACCESS_TOKEN = \"${GITHUB_PERSONAL_ACCESS_TOKEN}\""), "{text}");
    assert!(!kit.join("skills").exists(), "unconfirmed group is not exported");

    let before = (tree(&e.home), tree(&e.data));
    match k.preview("claude", KitSource::Folder(kit), &Scope::Global) {
        Err(KitError::WrongEnv { kit_env, kit_env_name, target_env, target_env_name }) => {
            assert_eq!((kit_env.as_str(), kit_env_name.as_str()), ("codex", "Codex"), "data for «Открыть в Codex»");
            assert_eq!((target_env.as_str(), target_env_name.as_str()), ("claude", "Claude Code"));
        }
        other => panic!("expected WrongEnv, got {other:?}"),
    }
    assert_eq!(before, (tree(&e.home), tree(&e.data)));
}

fn zip_with(path: &Path, entries: &[(&str, &[u8])], symlink: Option<(&str, &str)>) {
    use std::io::Write;
    let f = fs::File::create(path).unwrap();
    let mut z = zip::ZipWriter::new(f);
    let opt = zip::write::SimpleFileOptions::default();
    for (n, b) in entries {
        z.start_file(*n, opt).unwrap();
        z.write_all(b).unwrap();
    }
    if let Some((n, target)) = symlink {
        z.add_symlink(n, target, opt).unwrap();
    }
    z.finish().unwrap();
}

/// kit.json for a one-file Claude kit carrying `path`.
fn manifest_for(path: &str, body: &[u8]) -> Vec<u8> {
    use sha2::Digest;
    let sha: String = sha2::Sha256::digest(body).iter().map(|b| format!("{b:02x}")).collect();
    serde_json::to_vec(&serde_json::json!({
        "format": 1, "kind": "nightshift-kit", "name": "bad", "env": "claude", "app_version": "0", "created_at": "",
        "groups": ["rules"], "files": [{"path": path, "sha256": sha, "size": body.len()}], "env_vars": []
    }))
    .unwrap()
}

#[test]
fn bad_kits_are_refused_with_a_reason_and_nothing_written() {
    let e = env();
    let k = kits(&e);
    let snapshot = || (tree(&e.home), tree(&e.data), tree(e.out.parent().unwrap()));
    let before = snapshot();
    let pv = |src: KitSource| k.preview("claude", src, &Scope::Global).unwrap_err();

    // not a kit
    let plain = e.out.join("plain");
    put(&plain.join("README.md"), "hello\n");
    assert!(matches!(pv(KitSource::Folder(plain.clone())), KitError::NotAKit(_)));
    // broken zip
    let broken = e.out.join("broken.zip");
    fs::write(&broken, b"PK\x03\x04 definitely not a zip").unwrap();
    assert!(matches!(pv(KitSource::Zip(broken.clone())), KitError::BadZip(_)));
    // zip slip: a path out of the folder, and an absolute one
    let body = b"evil\n";
    for bad in ["../../evil.txt", "/tmp/evil.txt", "skills/../../evil.txt"] {
        let z = e.out.join(format!("slip{}.zip", uuid_like()));
        zip_with(&z, &[("kit.json", &manifest_for(bad, body)), (bad, body)], None);
        assert!(matches!(pv(KitSource::Zip(z)), KitError::UnsafePath(_)), "{bad}");
    }
    // a kit.json listing a path out of the folder (folder kit)
    let slipdir = e.out.join("slipdir");
    put(&slipdir.join("kit.json"), &String::from_utf8(manifest_for("../outside.md", body)).unwrap());
    assert!(matches!(pv(KitSource::Folder(slipdir)), KitError::UnsafePath(_)));
    // symlinks, in a folder and in a zip
    let linked = e.out.join("linked");
    put(&linked.join("kit.json"), &String::from_utf8(manifest_for("CLAUDE.md", body)).unwrap());
    std::os::unix::fs::symlink("/etc/hosts", linked.join("CLAUDE.md")).unwrap();
    assert!(matches!(pv(KitSource::Folder(linked)), KitError::Symlink(_)));
    let zlink = e.out.join("link.zip");
    zip_with(&zlink, &[("kit.json", &manifest_for("CLAUDE.md", body))], Some(("CLAUDE.md", "/etc/hosts")));
    assert!(matches!(pv(KitSource::Zip(zlink)), KitError::Symlink(_)));
    // a file over 10 MB
    let big = e.out.join("big");
    let blob = vec![b'a'; 10 * 1024 * 1024 + 1];
    fs::create_dir_all(&big).unwrap();
    fs::write(big.join("CLAUDE.md"), &blob).unwrap();
    fs::write(big.join("kit.json"), manifest_for("CLAUDE.md", &blob)).unwrap();
    assert!(matches!(pv(KitSource::Folder(big)), KitError::FileTooLarge { .. }));
    // a kit over 50 MB (each file under 10 MB)
    let huge = e.out.join("huge");
    fs::create_dir_all(&huge).unwrap();
    let part = vec![b'b'; 9 * 1024 * 1024];
    for i in 0..6 {
        fs::write(huge.join(format!("part{i}.bin")), &part).unwrap();
    }
    fs::write(huge.join("kit.json"), manifest_for("part0.bin", &part)).unwrap();
    assert!(matches!(pv(KitSource::Folder(huge)), KitError::KitTooLarge { .. }));
    // a zip entry over 10 MB
    let bomb = e.out.join("bomb.zip");
    let zeros = vec![0u8; 11 * 1024 * 1024];
    zip_with(&bomb, &[("kit.json", &manifest_for("CLAUDE.md", &zeros)), ("CLAUDE.md", &zeros)], None);
    assert!(matches!(pv(KitSource::Zip(bomb)), KitError::FileTooLarge { .. }));
    // a damaged kit: hash does not match
    let damaged = e.out.join("damaged");
    put(&damaged.join("kit.json"), &String::from_utf8(manifest_for("CLAUDE.md", b"original\n")).unwrap());
    put(&damaged.join("CLAUDE.md"), "tampered\n");
    assert!(matches!(pv(KitSource::Folder(damaged)), KitError::Corrupt(_)));
    // clone: a scheme that runs a command, and a link that does not clone (a folder that is no repository)
    let pwned = e.out.join("PWNED");
    assert!(matches!(pv(KitSource::Repo(format!("ext::sh -c touch% {}", pwned.display()))), KitError::CloneFailed(_)));
    assert!(!pwned.exists());
    assert!(matches!(pv(KitSource::Repo(plain.display().to_string())), KitError::CloneFailed(_)));

    // the only files that exist are the ones this test made in `out`; home and data untouched
    let after = snapshot();
    assert_eq!(before.0, after.0);
    assert_eq!(before.1, after.1);
    assert!(!e.out.parent().unwrap().join("evil.txt").exists() && !Path::new("/tmp/evil.txt").exists());
}

fn read_json(p: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(p).unwrap()).unwrap()
}

#[test]
fn hooks_mcp_and_scripted_skills_are_installed_off_and_turned_on_from_the_snapshot() {
    let e = env();
    let kit = kit_from(&e, &all_groups());
    let k = kits(&e);
    let plan = k.preview("claude", KitSource::Folder(kit.clone()), &Scope::Global).unwrap();

    // the hook is shown whole: event, command and the text of its script
    let hook = plan.entries.iter().find(|x| x.group == Group::Hooks).unwrap();
    let held = hook.held.as_ref().expect("hook is held");
    assert_eq!(held.kind, HeldKind::Hook);
    assert_eq!(held.event.as_deref(), Some("PreToolUse"));
    assert!(held.text.contains(&format!("bash {}/.claude/hooks/guard.sh", e.home.display())), "{}", held.text);
    assert_eq!(held.scripts.len(), 1);
    assert_eq!(held.scripts[0].path, "hooks/guard.sh");
    assert!(held.scripts[0].text.contains("echo checked"));
    let deploy = entry(&plan, "skill:deploy");
    assert_eq!(deploy.held.as_ref().unwrap().kind, HeldKind::ScriptedSkill, "«содержит скрипты»");
    assert!(entry(&plan, "skill:notes").held.is_none());
    assert_eq!(entry(&plan, "setting:permissions").held.as_ref().unwrap().kind, HeldKind::Command, "permissions change what runs");

    put(&e.home.join(".claude.json"), "{\"projects\": {\"/p\": {}}, \"theme\": \"dark\"}");
    let id = k.install(&plan, &Choices::default()).unwrap();
    assert!(read_json(&e.home.join(".claude.json")).get("mcpServers").is_none(), "held MCP is not in ~/.claude.json");
    k.enable_item("claude", &HeldRef { snapshot: id.clone(), key: "mcp:db".into() }).unwrap();
    let cj = read_json(&e.home.join(".claude.json"));
    assert_eq!(cj["mcpServers"]["db"]["command"], "npx");
    assert!(cj["projects"]["/p"].is_object() && cj["theme"] == "dark", "the CLI's own keys stay");
    let c = e.home.join(".claude");
    let settings = read_json(&c.join("settings.json"));
    assert!(settings.get("hooks").is_none(), "held hook is not in the CLI files");
    assert!(settings.get("env").is_none(), "env runs into every process: held as well");
    assert_eq!(settings["model"], "opus");
    assert!(!c.join("hooks/guard.sh").exists());
    assert!(!c.join("skills/deploy").exists(), "scripted skill is off");
    assert!(c.join("skills/notes/SKILL.md").exists());

    let snaps = k.snapshots("claude");
    let s = snaps.iter().find(|s| s.id == id).unwrap();
    let hook_key = s.held.iter().find(|h| h.kind == HeldKind::Hook).unwrap().key.clone();
    assert!(s.held.iter().all(|h| !h.enabled || h.key == "mcp:db"));

    // turned on later: from the snapshot, the kit folder is gone by now
    fs::remove_dir_all(&kit).unwrap();
    k.enable_item("claude", &HeldRef { snapshot: id.clone(), key: hook_key.clone() }).unwrap();
    let settings = read_json(&c.join("settings.json"));
    let cmd = settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"].as_str().unwrap().to_string();
    assert_eq!(cmd, format!("bash {}/.claude/hooks/guard.sh", e.home.display()));
    assert!(fs::read_to_string(c.join("hooks/guard.sh")).unwrap().contains("echo checked"));
    k.enable_item("claude", &HeldRef { snapshot: id.clone(), key: "skill:deploy".into() }).unwrap();
    assert!(c.join("skills/deploy/run.sh").exists());
    let s = k.snapshots("claude").into_iter().find(|s| s.id == id).unwrap();
    assert!(s.held.iter().find(|h| h.key == hook_key).unwrap().enabled);
    assert!(!s.held.iter().find(|h| h.key == "setting:env").unwrap().enabled);
}

#[test]
fn mcp_server_with_a_command_is_held_until_enabled() {
    let src = env();
    put(
        &src.home.join(".gemini/settings.json"),
        &serde_json::json!({
            "theme": "dark",
            "mcpServers": { "github": { "command": "npx", "args": ["-y", "server-github"], "env": { "GITHUB_TOKEN": GH } } }
        })
        .to_string(),
    );
    let e = env();
    let kit = e.out.join("gemini-kit");
    kits(&src).export("gemini", &sel(&all_groups()), ExportDest::Folder(kit.clone())).unwrap();
    let k = kits(&e);
    let plan = k.preview("gemini", KitSource::Folder(kit), &Scope::Global).unwrap();
    let m = entry(&plan, "mcp:github");
    let held = m.held.as_ref().unwrap();
    assert_eq!(held.kind, HeldKind::Mcp);
    assert!(held.text.contains("npx") && held.text.contains("${GITHUB_TOKEN}"));
    assert_eq!(plan.env_vars.iter().map(|v| (v.name.as_str(), v.missing)).collect::<Vec<_>>(), vec![("GITHUB_TOKEN", true)]);

    let id = k.install(&plan, &Choices::default()).unwrap();
    let p = e.home.join(".gemini/settings.json");
    assert!(read_json(&p).get("mcpServers").is_none());
    assert_eq!(read_json(&p)["theme"], "dark");
    k.enable_item("gemini", &HeldRef { snapshot: id, key: "mcp:github".into() }).unwrap();
    assert_eq!(read_json(&p)["mcpServers"]["github"]["command"], "npx");
    assert!(!fs::read_to_string(&p).unwrap().contains(GH));
}

#[test]
fn rollback_puts_back_old_bytes_and_removes_what_was_added() {
    let e = env();
    let kit = kit_from(&e, &all_groups());
    let c = e.home.join(".claude");
    put(&c.join("CLAUDE.md"), "# Mine\r\nkeep exact bytes\n\n");
    put(&c.join("settings.json"), "{\n    \"theme\": \"light\"\n}");
    let before = tree(&e.home);

    let k = kits(&e);
    let plan = k.preview("claude", KitSource::Folder(kit.clone()), &Scope::Global).unwrap();
    let id = k.install(&plan, &Choices::default()).unwrap();
    assert!(fs::read_to_string(c.join("CLAUDE.md")).unwrap().starts_with("# Rules"));
    assert!(c.join("skills/notes/SKILL.md").exists());
    let hook = k.snapshots("claude")[0].held.iter().find(|h| h.kind == HeldKind::Hook).unwrap().key.clone();
    k.enable_item("claude", &HeldRef { snapshot: id.clone(), key: hook }).unwrap();
    assert!(c.join("hooks/guard.sh").exists());

    let list = k.snapshots("claude");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].source, KitSource::Folder(kit));
    assert_eq!(list[0].kit_name, "my env");
    assert!((chrono::Local::now() - list[0].at).num_seconds() < 60);
    assert!(k.snapshots("codex").is_empty());

    k.rollback(&id).unwrap();
    assert_eq!(tree(&e.home), before, "home is byte for byte as before the install");
    assert!(!c.join("skills").exists() && !c.join("hooks").exists(), "added folders removed");
    assert_eq!(k.snapshots("claude")[0].state, SnapshotState::RolledBack);
}

#[test]
fn a_failed_write_rolls_back_at_once() {
    let e = env();
    let kit = kit_from(&e, &[Group::Rules, Group::Skills]);
    let c = e.home.join(".claude");
    put(&c.join("CLAUDE.md"), "# Mine\n");
    fs::create_dir_all(c.join("skills")).unwrap();
    let before = tree(&e.home);
    let k = kits(&e);
    let plan = k.preview("claude", KitSource::Folder(kit), &Scope::Global).unwrap();
    // the disk "fails" on the skills folder: not writable
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(c.join("skills"), fs::Permissions::from_mode(0o500)).unwrap();
    let res = k.install(&plan, &Choices::default());
    fs::set_permissions(c.join("skills"), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(res.is_err());
    assert_eq!(tree(&e.home), before, "no half kit left");
}

#[test]
fn an_install_cut_short_is_rolled_back_by_recover_at_next_start() {
    let e = env();
    let kit = kit_from(&e, &[Group::Rules, Group::Skills, Group::Settings]);
    let c = e.home.join(".claude");
    put(&c.join("CLAUDE.md"), "# Mine\n");
    let before = tree(&e.home);

    let dying = kits(&e).crash_after_writes(1);
    let plan = dying.preview("claude", KitSource::Folder(kit), &Scope::Global).unwrap();
    assert!(dying.install(&plan, &Choices::default()).is_err());
    assert_ne!(tree(&e.home), before, "the crash left a half-installed kit");
    drop(dying);

    let k = kits(&e); // next start
    let rec = k.recover();
    assert_eq!(rec.len(), 1);
    assert_eq!(rec[0].kit_name, "my env");
    assert_eq!(rec[0].message, "kit_install_interrupted_rolled_back", "UI: «установка my env прервана - откатил»");
    assert_eq!(tree(&e.home), before, "no half of the kit remains");
    assert_eq!(k.snapshots("claude")[0].state, SnapshotState::Interrupted);
    assert!(k.recover().is_empty(), "reported once");
}

#[test]
fn only_into_the_project_files_land_there_uncommitted_and_their_paths_come_back() {
    let e = env();
    let kit = kit_from(&e, &[Group::Rules, Group::Skills, Group::Settings]);
    let project = e.out.join("proj");
    fs::create_dir_all(&project).unwrap();
    git(&project, &["init", "-q", "-b", "main"]);
    put(&project.join("main.rs"), "fn main() {}\n");
    git(&project, &["add", "-A"]);
    git(&project, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-q", "-m", "init"]);
    let home_before = tree(&e.home);

    let k = kits(&e);
    let plan = k.preview("claude", KitSource::Folder(kit), &Scope::Project(project.clone())).unwrap();
    assert!(plan.uncommitted_in_project, "«файлы набора в проекте не закоммичены»");
    for p in [".claude/CLAUDE.md", ".claude/skills/notes/SKILL.md", ".claude/settings.json"] {
        assert!(plan.project_files.contains(&p.to_string()), "{p} not in {:?}", plan.project_files);
    }
    assert!(!plan.project_files.iter().any(|p| p.contains("deploy")), "held scripted skill is not written");
    let id = k.install(&plan, &Choices::default()).unwrap();

    assert!(project.join(".claude/CLAUDE.md").exists());
    assert_eq!(tree(&e.home), home_before, "the home is not touched");
    assert_eq!(git(&project, &["ls-files"]), "main.rs", "nothing added to the git index");
    assert!(git(&project, &["status", "--porcelain"]).contains("?? .claude/"));
    let s = k.snapshots("claude").into_iter().find(|s| s.id == id).unwrap();
    assert_eq!(s.project_files, plan.project_files);
}

#[test]
fn secrets_go_to_the_keyring_or_a_0600_file_and_come_back_as_run_env() {
    let e = env();
    let ring = FakeKeyring::default();
    let k = Kits::new(&e.home, &e.data).with_secret_store(Box::new(ring.clone()));
    assert_eq!(k.set_secret("gemini", "GITHUB_TOKEN", GH).unwrap(), SecretSaved::Keyring);
    assert_eq!(ring.vals.lock().unwrap().get("nightshift|gemini/GITHUB_TOKEN").map(|s| s.as_str()), Some(GH), "service nightshift, user <env>/<name>");
    assert_eq!(k.env_vars("gemini"), vec![("GITHUB_TOKEN".to_string(), GH.to_string())]);
    assert!(k.env_vars("claude").is_empty(), "only for this environment's runs");
    assert!(!all_text(&e.data).contains(GH), "no value in the data folder");
    assert!(all_text(&e.data).contains("GITHUB_TOKEN"), "the name is remembered");
    assert!(matches!(k.set_secret("gemini", "BAD NAME", "x"), Err(KitError::BadName(_))));

    // the next preview no longer asks for it
    let src = env();
    put(&src.home.join(".gemini/settings.json"), &serde_json::json!({"mcpServers": {"gh": {"command": "x", "env": {"GITHUB_TOKEN": GH}}}}).to_string());
    let kit = e.out.join("g");
    kits(&src).export("gemini", &sel(&[Group::Mcp]), ExportDest::Folder(kit.clone())).unwrap();
    let plan = k.preview("gemini", KitSource::Folder(kit), &Scope::Global).unwrap();
    assert_eq!(plan.env_vars[0].name, "GITHUB_TOKEN");
    assert!(!plan.env_vars[0].missing);

    // no keyring: a file only the user can read, and a warning for the UI
    let e2 = env();
    let k2 = Kits::new(&e2.home, &e2.data).with_secret_store(Box::new(FakeKeyring { fail: true, ..Default::default() }));
    match k2.set_secret("claude", "API_TOKEN", "tok-123456789").unwrap() {
        SecretSaved::File { path, warning } => {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
            assert!(!warning.is_empty());
            assert!(path.starts_with(&e2.data));
        }
        other => panic!("expected file fallback, got {other:?}"),
    }
    assert_eq!(k2.env_vars("claude"), vec![("API_TOKEN".to_string(), "tok-123456789".to_string())]);
    let files_with_value: Vec<String> =
        tree(&e2.data).into_iter().filter(|(_, b)| String::from_utf8_lossy(b).contains("tok-123456789")).map(|(p, _)| p).collect();
    assert_eq!(files_with_value, vec!["kits/secrets.json".to_string()], "the value is only in the 0600 file");
}

fn copy_tree(from: &Path, to: &Path) {
    for (rel, b) in tree(from) {
        put(&to.join(&rel), &String::from_utf8(b).unwrap());
    }
}

#[test]
fn check_update_of_a_repository_kit_shows_changes_installs_nothing_and_holds_changed_hooks() {
    let src = env();
    claude_home(&src);
    let e = env();
    let repo = e.out.join("kit-repo");
    kits(&src).export("claude", &sel(&all_groups()), ExportDest::Git(repo.clone())).unwrap();
    let first = git(&repo, &["rev-parse", "HEAD"]);

    let k = kits(&e);
    let plan = k.preview("claude", KitSource::Repo(repo.display().to_string()), &Scope::Global).unwrap();
    assert_eq!(plan.commit.as_deref(), Some(first.as_str()), "the kit remembers source and commit");
    let id = k.install(&plan, &Choices::default()).unwrap();
    let hook = k.snapshots("claude")[0].held.iter().find(|h| h.kind == HeldKind::Hook).unwrap().key.clone();
    k.enable_item("claude", &HeldRef { snapshot: id.clone(), key: hook.clone() }).unwrap();

    // the author changes the hook script and the rules, and commits
    put(&src.home.join(".claude/hooks/guard.sh"), "#!/bin/sh\necho checked v2\n");
    put(&src.home.join(".claude/CLAUDE.md"), "# Rules v2\n");
    let v2 = e.out.join("v2");
    kits(&src).export("claude", &sel(&all_groups()), ExportDest::Folder(v2.clone())).unwrap();
    copy_tree(&v2, &repo);
    git(&repo, &["add", "-A"]);
    git(&repo, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-q", "-m", "v2"]);
    let second = git(&repo, &["rev-parse", "HEAD"]);

    let installed = k.snapshots("claude").into_iter().find(|s| s.id == id).unwrap().installed();
    let before = (tree(&e.home), tree(&e.data));
    let up = k.check_update(&installed).unwrap();
    assert_eq!(before, (tree(&e.home), tree(&e.data)), "check_update installs nothing");
    assert_eq!(up.update_from.as_deref(), Some(first.as_str()));
    assert_eq!(up.commit.as_deref(), Some(second.as_str()));
    let rules = entry(&up, "rules:CLAUDE.md");
    assert_eq!(rules.change, Change::Replace);
    assert!(rules.diff.as_deref().unwrap().contains("+ # Rules v2"));
    let h = entry(&up, &hook);
    assert_eq!(h.change, Change::Replace);
    let held = h.held.as_ref().expect("a changed hook comes back off");
    assert!(held.scripts[0].text.contains("echo checked v2"));
    assert_eq!(entry(&up, "skill:notes").change, Change::Same);
}

#[test]
fn install_does_not_follow_a_symlink_out_of_the_home() {
    let e = env();
    let kit = kit_from(&e, &[Group::Rules, Group::Skills]);
    let outside = e.out.join("elsewhere");
    fs::create_dir_all(&outside).unwrap();
    fs::create_dir_all(e.home.join(".claude")).unwrap();
    std::os::unix::fs::symlink(&outside, e.home.join(".claude/skills")).unwrap();
    let before = tree(&e.home);
    let k = kits(&e);
    let plan = k.preview("claude", KitSource::Folder(kit), &Scope::Global).unwrap();
    assert!(matches!(k.install(&plan, &Choices::default()), Err(KitError::UnsafePath(_))));
    assert!(tree(&outside).is_empty(), "nothing written outside the home");
    assert_eq!(tree(&e.home), before);
}

#[test]
fn codex_config_toml_mcp_server_is_held_and_merged_on_enable() {
    let src = env();
    codex_home(&src);
    let e = env();
    let kit = e.out.join("codex-kit");
    kits(&src).export("codex", &sel(&all_groups()), ExportDest::Folder(kit.clone())).unwrap();
    let cfg = e.home.join(".codex/config.toml");
    put(&cfg, "# mine\nmodel = \"o3\"\napproval_policy = \"never\"\n\n[tui]\ntheme = \"light\"\n");

    let k = kits(&e);
    let plan = k.preview("codex", KitSource::Folder(kit), &Scope::Global).unwrap();
    assert_eq!(entry(&plan, "setting:model").change, Change::Replace);
    let m = entry(&plan, "mcp:github");
    assert!(m.held.as_ref().unwrap().text.contains("[mcp_servers.github.env]"));
    let id = k.install(&plan, &Choices::default()).unwrap();
    let t = fs::read_to_string(&cfg).unwrap();
    assert!(t.contains("model = \"gpt-5\"") && t.contains("theme = \"dark\"") && t.contains("approval_policy = \"never\"") && t.contains("# mine"), "{t}");
    assert!(!t.contains("mcp_servers"), "held server is not in config.toml: {t}");

    k.enable_item("codex", &HeldRef { snapshot: id.clone(), key: "mcp:github".into() }).unwrap();
    let t = fs::read_to_string(&cfg).unwrap();
    assert!(t.contains("[mcp_servers.github]") && t.contains("GITHUB_PERSONAL_ACCESS_TOKEN = \"${GITHUB_PERSONAL_ACCESS_TOKEN}\""), "{t}");
    k.rollback(&id).unwrap();
    assert_eq!(fs::read_to_string(&cfg).unwrap(), "# mine\nmodel = \"o3\"\napproval_policy = \"never\"\n\n[tui]\ntheme = \"light\"\n");
}

#[test]
fn install_refuses_a_link_into_a_key_store_even_inside_the_home() {
    let e = env();
    let kit = kit_from(&e, &[Group::Rules, Group::Skills]);
    put(&e.home.join(".ssh/config"), "Host x\n");
    fs::create_dir_all(e.home.join(".claude")).unwrap();
    std::os::unix::fs::symlink(e.home.join(".ssh"), e.home.join(".claude/skills")).unwrap();
    let before = tree(&e.home.join(".ssh"));
    let k = kits(&e);
    let plan = k.preview("claude", KitSource::Folder(kit), &Scope::Global).unwrap();
    assert!(matches!(k.install(&plan, &Choices::default()), Err(KitError::UnsafePath(_))));
    assert_eq!(tree(&e.home.join(".ssh")), before);
}

#[test]
fn snapshots_are_private_and_only_the_newest_twenty_are_kept() {
    use std::os::unix::fs::PermissionsExt;
    let e = env();
    let k = kits(&e);
    let mut ids = vec![];
    for i in 0..22 {
        let src = env();
        put(&src.home.join(".claude/CLAUDE.md"), &format!("# v{i}\n"));
        let kit = e.out.join(format!("k{i}"));
        kits(&src).export("claude", &sel(&[Group::Rules]), ExportDest::Folder(kit.clone())).unwrap();
        let plan = k.preview("claude", KitSource::Folder(kit), &Scope::Global).unwrap();
        ids.push(k.install(&plan, &Choices::default()).unwrap());
    }
    let left: Vec<String> = k.snapshots("claude").into_iter().map(|s| s.id).collect();
    assert_eq!(left.len(), 20);
    assert!(left.contains(ids.last().unwrap()) && !left.contains(&ids[0]) && !left.contains(&ids[1]), "the oldest go");
    for d in [e.data.join("kits"), e.data.join("kits/snapshots"), e.data.join("kits/snapshots").join(&left[0])] {
        assert_eq!(fs::metadata(&d).unwrap().permissions().mode() & 0o777, 0o700, "{}", d.display());
    }
}

#[test]
fn codex_inline_hooks_and_mcp_are_reported_not_moved() {
    let e = env();
    put(&e.home.join(".codex/config.toml"), "model = \"o3\"\nmcp_servers = { a = { command = \"x\" } }\nhooks = { }\n");
    let rep = kits(&e).export("codex", &sel(&all_groups()), ExportDest::Folder(e.out.join("k"))).unwrap();
    let inline: Vec<&str> = rep.skipped.iter().filter(|s| s.reason == "inline_form").map(|s| s.path.as_str()).collect();
    assert_eq!(inline, vec!["config.toml#mcp_servers", "config.toml#hooks"]);
    assert!(!fs::read_to_string(e.out.join("k/config.toml")).unwrap().contains("mcp_servers"));
}

#[test]
fn a_repository_with_a_big_file_or_a_symlink_is_refused_before_checkout() {
    let e = env();
    for (i, bad) in ["big", "link"].iter().enumerate() {
        let repo = e.out.join(format!("r{i}"));
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        // the clone filter really applies: the big blob stays on the source side
        git(&repo, &["config", "uploadpack.allowFilter", "true"]);
        put(&repo.join("kit.json"), "{}");
        if *bad == "big" {
            fs::write(repo.join("CLAUDE.md"), vec![b'a'; 10 * 1024 * 1024 + 1]).unwrap();
        } else {
            std::os::unix::fs::symlink("/etc/hosts", repo.join("CLAUDE.md")).unwrap();
        }
        git(&repo, &["add", "-A"]);
        git(&repo, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-q", "-m", "x"]);
        let before = (tree(&e.home), tree(&e.data));
        let err = kits(&e).preview("claude", KitSource::Repo(repo.display().to_string()), &Scope::Global).unwrap_err();
        match *bad {
            // refused for the missing blob: its real size (10 MiB + 1) was never fetched
            "big" => assert!(matches!(err, KitError::FileTooLarge { ref path, size } if path == "CLAUDE.md" && size != 10 * 1024 * 1024 + 1), "{err:?}"),
            _ => assert!(matches!(err, KitError::Symlink(_)), "{err:?}"),
        }
        assert_eq!(before, (tree(&e.home), tree(&e.data)));
    }
}

#[test]
fn rollback_of_shared_configs_returns_only_the_kits_keys_and_keeps_what_the_cli_wrote_since() {
    let e = env();
    let kit = kit_from(&e, &all_groups());
    let cj = e.home.join(".claude.json");
    put(&cj, &serde_json::json!({ "projects": { "/a": { "n": 1 } }, "mcpServers": { "mine": { "command": "my-server" } } }).to_string());
    put(&e.home.join(".claude/settings.json"), "{\"theme\": \"light\"}");
    let k = kits(&e);
    let plan = k.preview("claude", KitSource::Folder(kit), &Scope::Global).unwrap();
    let id = k.install(&plan, &Choices::default()).unwrap();
    k.enable_item("claude", &HeldRef { snapshot: id.clone(), key: "mcp:db".into() }).unwrap();
    assert_eq!(read_json(&cj)["mcpServers"]["db"]["command"], "npx");

    // the CLI keeps living: new key, changed projects, its own settings change
    let mut live = read_json(&cj);
    live["numStartups"] = 7.into();
    live["projects"]["/b"] = serde_json::json!({ "n": 2 });
    fs::write(&cj, serde_json::to_string(&live).unwrap()).unwrap();
    let sp = e.home.join(".claude/settings.json");
    let mut st = read_json(&sp);
    st["theme"] = "dark".into();
    fs::write(&sp, st.to_string()).unwrap();

    k.rollback(&id).unwrap();
    let after = read_json(&cj);
    assert_eq!(after["mcpServers"], serde_json::json!({ "mine": { "command": "my-server" } }), "MCP as before the install");
    assert_eq!(after["numStartups"], 7, "key the CLI added is kept");
    assert_eq!(after["projects"]["/b"]["n"], 2, "projects changed by the CLI are kept");
    let st = read_json(&sp);
    assert_eq!(st["theme"], "dark", "the CLI's own settings change is kept");
    assert!(st.get("model").is_none(), "the kit's key is gone");
}

#[test]
fn a_repository_blob_left_out_by_the_clone_filter_is_refused_not_fetched_later() {
    // `--filter=blob:limit=10m` leaves out blobs of 10 MiB and more (git docs: "at least n
    // bytes"); a file of exactly 10 MiB passes the folder check but must not be lazily fetched.
    let src = env();
    put(&src.home.join(".claude/CLAUDE.md"), "# rules\n");
    let e = env();
    let repo = e.out.join("kit-repo");
    kits(&src).export("claude", &sel(&[Group::Rules]), ExportDest::Git(repo.clone())).unwrap();
    git(&repo, &["config", "uploadpack.allowFilter", "true"]);
    fs::write(repo.join("extra.bin"), vec![b'a'; 10 * 1024 * 1024]).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-q", "-m", "big"]);
    let before = (tree(&e.home), tree(&e.data));
    let err = kits(&e).preview("claude", KitSource::Repo(repo.display().to_string()), &Scope::Global).unwrap_err();
    assert!(matches!(&err, KitError::FileTooLarge { path, .. } if path == "extra.bin"), "{err:?}");
    assert_eq!(before, (tree(&e.home), tree(&e.data)));
}

#[test]
fn rollback_keeps_a_kit_key_the_human_changed_after_install_and_reports_it() {
    let e = env();
    let kit = kit_from(&e, &[Group::Settings]);
    let sp = e.home.join(".claude/settings.json");
    put(&sp, "{\"theme\": \"light\", \"model\": \"haiku\"}");
    let k = kits(&e);
    let plan = k.preview("claude", KitSource::Folder(kit), &Scope::Global).unwrap();
    let id = k.install(&plan, &Choices::default()).unwrap();
    assert_eq!(read_json(&sp)["model"], "opus");

    // the human (or the CLI's /model) picks another model after the install
    let mut st = read_json(&sp);
    st["model"] = "sonnet".into();
    fs::write(&sp, st.to_string()).unwrap();

    let rep = k.rollback(&id).unwrap();
    let st = read_json(&sp);
    assert_eq!(st["model"], "sonnet", "the human's value stays");
    assert_eq!(st["theme"], "light");
    assert_eq!(rep.kept_changed, vec!["~/.claude/settings.json: model".to_string()]);
}

#[test]
fn rollback_keeps_a_file_new_to_the_install_once_the_human_wrote_into_it() {
    let src = env();
    codex_home(&src);
    let e = env();
    let kit = e.out.join("codex-kit");
    kits(&src).export("codex", &sel(&[Group::Settings, Group::Rules]), ExportDest::Folder(kit.clone())).unwrap();
    let k = kits(&e);
    let plan = k.preview("codex", KitSource::Folder(kit), &Scope::Global).unwrap();
    let id = k.install(&plan, &Choices::default()).unwrap();
    let cfg = e.home.join(".codex/config.toml");
    let rules = e.home.join(".codex/AGENTS.md");
    assert!(fs::read_to_string(&cfg).unwrap().contains("model = \"gpt-5\""));

    // the human notes something in both files the install created
    let t = fs::read_to_string(&cfg).unwrap();
    fs::write(&cfg, format!("# my notes: try o3 next week\n{t}")).unwrap();
    fs::write(&rules, "Be brief.\nAnd answer in Russian.\n").unwrap();

    let rep = k.rollback(&id).unwrap();
    let t = fs::read_to_string(&cfg).expect("config.toml stays: the human wrote into it");
    assert!(t.contains("# my notes: try o3 next week"), "{t}");
    assert!(!t.contains("gpt-5") && !t.contains("theme"), "the kit's keys are gone: {t}");
    assert_eq!(fs::read_to_string(&rules).unwrap(), "Be brief.\nAnd answer in Russian.\n");
    let mut kept = rep.kept_files.clone();
    kept.sort();
    assert_eq!(kept, vec!["~/.codex/AGENTS.md".to_string(), "~/.codex/config.toml".to_string()]);
}

#[test]
fn a_config_the_running_cli_overwrote_right_after_the_write_is_reported_not_applied() {
    let e = env();
    let kit = kit_from(&e, &[Group::Settings]);
    let sp = e.home.join(".claude/settings.json");
    put(&sp, "{\"theme\": \"light\"}");
    // a running Claude Code saves its in-memory settings over the file just written
    let k = kits(&e).after_config_write(|p: &Path| {
        if p.ends_with(".claude/settings.json") {
            fs::write(p, "{\"theme\": \"light\"}").unwrap();
        }
    });
    let plan = k.preview("claude", KitSource::Folder(kit), &Scope::Global).unwrap();
    let err = k.install(&plan, &Choices::default()).unwrap_err();
    let KitError::NotApplied { snapshot, cli, keys } = &err else { panic!("{err:?}") };
    assert_eq!(cli, "Claude Code");
    assert_eq!(keys, &vec!["~/.claude/settings.json: model".to_string()]);
    assert!(err.to_string().contains("close Claude Code and retry"), "{err}");
    let s = k.snapshots("claude").into_iter().find(|s| &s.id == snapshot).unwrap();
    assert_eq!(s.state, SnapshotState::NotApplied, "not Done");
    assert_eq!(read_json(&sp), serde_json::json!({ "theme": "light" }));
}

#[test]
fn every_kit_error_serializes_with_its_kind_and_fields() {
    use nightshift_core::kits::KitError::*;
    let s = |x: nightshift_core::kits::KitError| serde_json::to_value(x).unwrap();
    let cases = vec![
        (s(NotAKit("a".into())), "not_a_kit", "detail"),
        (s(BadZip("a".into())), "bad_zip", "detail"),
        (s(UnsafePath("a".into())), "unsafe_path", "detail"),
        (s(Symlink("a".into())), "symlink", "detail"),
        (s(FileTooLarge { path: "p".into(), size: 3 }), "file_too_large", "path"),
        (s(KitTooLarge { size: 3 }), "kit_too_large", "size"),
        (s(TooManyFiles(9)), "too_many_files", "size"),
        (s(Corrupt("a".into())), "corrupt", "detail"),
        (s(WrongEnv { kit_env: "codex".into(), kit_env_name: "Codex".into(), target_env: "claude".into(), target_env_name: "Claude".into() }), "wrong_env", "kit_env_name"),
        (s(CloneFailed("a".into())), "clone_failed", "detail"),
        (s(UnknownEnv("a".into())), "unknown_env", "detail"),
        (s(DestNotEmpty("a".into())), "dest_not_empty", "detail"),
        (s(NotFound("a".into())), "not_found", "detail"),
        (s(Changed), "changed", "kind"),
        (s(BadName("a b".into())), "bad_name", "detail"),
        (s(Interrupted), "interrupted", "kind"),
        (s(NotApplied { snapshot: "s".into(), cli: "claude".into(), keys: vec!["k".into()] }), "not_applied", "cli"),
        (s(Io("a".into())), "io", "detail"),
    ];
    for (v, kind, field) in cases {
        assert_eq!(v["kind"], kind);
        assert!(v.get(field).is_some(), "{kind} has {field}: {v}");
    }
    assert_eq!(s(BadName("a b".into()))["detail"], "a b");
}
