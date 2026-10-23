//! Catalog (G40-G42): sources on recorded answers through `FakeFetch`, installs through `Kits` into a temp home.
use nightshift_core::kits::{Choices, ExportDest, Group, HeldKind, HeldRef, KitSource, Kits, Scope, SecretStore, Selection};
use nightshift_core::market::{Kind, Market, MarketError, Origin, Source, MAX_ITEM};
use nightshift_core::net::{Fetch, FakeFetch, FetchOut, NetError};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn fx(name: &str) -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/market").join(name)).unwrap()
}

#[derive(Default)]
struct Ring(Mutex<BTreeMap<String, String>>);
struct RingRef(Arc<Ring>);
impl SecretStore for RingRef {
    fn set(&self, service: &str, user: &str, value: &str) -> Result<(), String> {
        self.0 .0.lock().unwrap().insert(format!("{service}|{user}"), value.into());
        Ok(())
    }
    fn get(&self, service: &str, user: &str) -> Result<Option<String>, String> {
        Ok(self.0 .0.lock().unwrap().get(&format!("{service}|{user}")).cloned())
    }
}

/// Lets a test change answers after the fetcher went into the market.
struct Shared(Arc<FakeFetch>);
impl Fetch for Shared {
    fn get(&self, url: &str, max: usize, etag: Option<&str>) -> Result<FetchOut, NetError> {
        self.0.get(url, max, etag)
    }
}

struct T {
    _tmp: tempfile::TempDir,
    home: PathBuf,
    kits: Arc<Kits>,
    ring: Arc<Ring>,
    fetch: Arc<FakeFetch>,
    m: Market,
}

fn setup(fetch: FakeFetch) -> T {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let ring = Arc::new(Ring::default());
    let kits = Arc::new(Kits::new(&home, tmp.path().join("data")).with_secret_store(Box::new(RingRef(ring.clone()))));
    let fetch = Arc::new(fetch);
    let m = Market::new(Box::new(Shared(fetch.clone())), tmp.path().join("cache")).with_kits(kits.clone());
    T { home, kits, ring, fetch, m, _tmp: tmp }
}

/// Only the source `id` is on (plus own sources given).
fn only(m: &Market, id: &str, own: Vec<Source>) {
    let mut all: Vec<Source> = m.sources().unwrap().into_iter().filter(|s| s.builtin).map(|s| Source { enabled: s.id == id, ..s }).collect();
    all.extend(own);
    m.set_sources(all).unwrap();
}

fn zip_of(files: &[(&str, &[u8])], comment: Option<&str>) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let o = zip::write::SimpleFileOptions::default();
        for (n, b) in files {
            z.start_file(*n, o).unwrap();
            z.write_all(b).unwrap();
        }
        if let Some(c) = comment {
            z.set_comment(c.to_string()).unwrap();
        }
        z.finish().unwrap();
    }
    buf.into_inner()
}

const SHA: &str = "dbd4588f9e1033efb41dad4bef2f7947c8993d44";
const SKILL_MD: &str = "---\nname: pdf\ndescription: PDF things\nlicense: MIT\n---\nUse pdftotext.\n";

fn skills_zip() -> Vec<u8> {
    zip_of(
        &[
            ("skills-HEAD/README.md", b"repo"),
            ("skills-HEAD/skills/pdf/SKILL.md", SKILL_MD.as_bytes()),
            ("skills-HEAD/skills/pdf/reference.md", b"ref\n"),
            ("skills-HEAD/skills/docx/SKILL.md", b"---\nname: docx\n---\n"),
        ],
        Some(SHA),
    )
}

// ---------------- sources ----------------

#[test]
fn skills_sh_search_reads_the_recorded_answer() {
    let t = setup(FakeFetch::new().with("https://skills.sh/api/search?q=pdf&limit=20", fx("skillssh-search.json")));
    only(&t.m, "skills.sh", vec![]);
    let p = t.m.search(Kind::Skill, "pdf", None).unwrap();
    assert_eq!(p.items.len(), 3);
    let pdf = &p.items[0];
    assert_eq!(pdf.id, "skills.sh:anthropics/skills/pdf");
    assert_eq!(pdf.installs, Some(215206));
    assert_eq!(pdf.origin, Origin::Github { repo: "anthropics/skills".into(), path: String::new(), git_ref: None, skill: Some("pdf".into()) });
    assert!(p.cursor.is_none() && p.offline.is_empty());
}

#[test]
fn marketplace_lists_skills_of_a_plugin_and_pages_a_big_index() {
    let url = "https://raw.githubusercontent.com/anthropics/skills/main/.claude-plugin/marketplace.json";
    let t = setup(FakeFetch::new().with(url, fx("mp-anthropic-skills.json")));
    only(&t.m, "anthropic-skills", vec![]);
    let p = t.m.search(Kind::Skill, "pdf", None).unwrap();
    let pdf = p.items.iter().find(|i| i.name == "pdf").expect("skill listed by the plugin");
    assert_eq!(pdf.id, "anthropic-skills:document-skills/pdf");
    assert_eq!(pdf.origin, Origin::Github { repo: "anthropics/skills".into(), path: "skills/pdf".into(), git_ref: Some("main".into()), skill: None });

    // the official index: plugins with url / relative sources
    let off = "https://raw.githubusercontent.com/anthropics/claude-plugins-official/main/.claude-plugin/marketplace.json";
    t.fetch.set(off, Ok(FetchOut { status: 200, body: fx("mp-official.json"), etag: None }));
    only(&t.m, "claude-plugins-official", vec![]);
    let p = t.m.search(Kind::Skill, "", None).unwrap();
    assert_eq!(p.items.len(), 4);
    let sdk = p.items.iter().find(|i| i.name == "agent-sdk-dev").unwrap();
    assert!(matches!(&sdk.origin, Origin::Github { repo, path, .. } if repo == "anthropics/claude-plugins-official" && path == "plugins/agent-sdk-dev"));
    let adlc = p.items.iter().find(|i| i.name == "agentforce-adlc").unwrap();
    assert!(matches!(&adlc.origin, Origin::Github { repo, git_ref: Some(r), .. } if repo == "SalesforceAIResearch/agentforce-adlc" && r.len() == 40), "{:?}", adlc.origin);

    // community index, and portions of 20 from a 45-plugin own marketplace
    let com = "https://raw.githubusercontent.com/anthropics/claude-plugins-community/main/.claude-plugin/marketplace.json";
    t.fetch.set(com, Ok(FetchOut { status: 200, body: fx("mp-community.json"), etag: None }));
    only(&t.m, "claude-plugins-community", vec![]);
    assert_eq!(t.m.search(Kind::Skill, "", None).unwrap().items.len(), 4);

    let plugins: Vec<serde_json::Value> = (0..45).map(|i| serde_json::json!({"name": format!("p{i}"), "source": format!("./p{i}")})).collect();
    let big = "https://raw.githubusercontent.com/me/kit/HEAD/.claude-plugin/marketplace.json";
    t.fetch.set(big, Ok(FetchOut { status: 200, body: serde_json::to_vec(&serde_json::json!({"name": "mine", "plugins": plugins})).unwrap(), etag: None }));
    let own = Source { id: "mine".into(), kind: Kind::Skill, url: "https://github.com/me/kit".into(), enabled: true, api: Default::default(), name: String::new(), builtin: false, note: None };
    only(&t.m, "-", vec![own]);
    assert_eq!(t.m.sources().unwrap().iter().find(|s| s.id == "mine").unwrap().url, big, "owner/repo → its marketplace.json");
    let mut names = vec![];
    let mut cur = None;
    for _ in 0..5 {
        let p = t.m.search(Kind::Skill, "", cur.as_deref()).unwrap();
        names.extend(p.items.into_iter().map(|i| i.name));
        cur = p.cursor;
        if cur.is_none() {
            break;
        }
    }
    assert_eq!(names.len(), 45);
    assert_eq!(names[20], "p20");
}

#[test]
fn skillsmp_and_smithery_page_by_number() {
    let t = setup(
        FakeFetch::new()
            .with("https://skillsmp.com/api/v1/skills/search?q=pdf&limit=20&page=1", fx("skillsmp-search.synthetic.json"))
            .with("https://registry.smithery.ai/servers?q=github&pageSize=10&page=1", fx("smithery-search.synthetic.json")),
    );
    let mut all: Vec<Source> = t.m.sources().unwrap();
    for s in all.iter_mut() {
        s.enabled = s.id == "skillsmp" || s.id == "smithery";
    }
    t.m.set_sources(all).unwrap();
    let p = t.m.search(Kind::Skill, "pdf", None).unwrap();
    assert_eq!(p.items[0].origin, Origin::Github { repo: "anthropics/skills".into(), path: "skills/pdf".into(), git_ref: Some("main".into()), skill: None });
    assert_eq!(p.items[0].stars, Some(120000));
    assert_eq!(p.cursor.as_deref(), Some(r#"{"skillsmp":"2"}"#));
    let p = t.m.search(Kind::Mcp, "github", None).unwrap();
    assert_eq!(p.items[0].installs, Some(4200));
    assert_eq!(p.cursor.as_deref(), Some(r#"{"smithery":"2"}"#));
}

#[test]
fn mcp_registry_pages_by_next_cursor_and_github_registry_gives_stars() {
    let base = "https://registry.modelcontextprotocol.io/v0.1/servers?version=latest&limit=10&search=github";
    let t = setup(
        FakeFetch::new()
            .with(base, fx("mcp-registry-p1.json"))
            .with(&format!("{base}&cursor=ai.smithery%2Fsmithery-ai-github%3A1.0.0"), fx("mcp-registry-p2.json"))
            .with("https://api.mcp.github.com/v0.1/servers?version=latest&limit=10&search=github", fx("mcp-github.json")),
    );
    only(&t.m, "mcp-registry", vec![]);
    let p1 = t.m.search(Kind::Mcp, "github", None).unwrap();
    assert_eq!(p1.items.len(), 3);
    assert_eq!(p1.items[0].id, "mcp-registry:ai.smithery/Hint-Services-obsidian-github-mcp");
    let p2 = t.m.search(Kind::Mcp, "github", p1.cursor.as_deref()).unwrap();
    assert_eq!(p2.items[0].id, "mcp-registry:com.a2awire/data-github-changelog-platform-change-announcement");
    assert!(p2.cursor.is_some());

    only(&t.m, "github-mcp", vec![]);
    let g = t.m.search(Kind::Mcp, "github", None).unwrap();
    let md = &g.items[0];
    assert_eq!((md.stars, md.license.as_deref()), (Some(191659), Some("MIT License")));
    assert_eq!(md.updated.as_deref(), Some("2026-01-21T09:35:10Z"));
}

#[test]
fn mods_come_from_the_playground_folder() {
    let url = "https://api.github.com/repos/anthropics/claude-code-playground/contents/claude-code/mods";
    let t = setup(FakeFetch::new().with(url, fx("mods-playground.json")));
    let p = t.m.search(Kind::Mod, "", None).unwrap();
    let names: Vec<&str> = p.items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["blast-radius", "replay-theater", "token-weather"]);
    assert_eq!(p.items[0].origin, Origin::Github { repo: "anthropics/claude-code-playground".into(), path: "claude-code/mods/blast-radius".into(), git_ref: None, skill: None });
}

#[test]
fn broken_json_from_any_source_is_an_error_not_a_panic() {
    let cases: &[(&str, Kind, &str)] = &[
        ("skills.sh", Kind::Skill, "https://skills.sh/api/search?q=x&limit=20"),
        ("anthropic-skills", Kind::Skill, "https://raw.githubusercontent.com/anthropics/skills/main/.claude-plugin/marketplace.json"),
        ("skillsmp", Kind::Skill, "https://skillsmp.com/api/v1/skills/search?q=x&limit=20&page=1"),
        ("mcp-registry", Kind::Mcp, "https://registry.modelcontextprotocol.io/v0.1/servers?version=latest&limit=10&search=x"),
        ("smithery", Kind::Mcp, "https://registry.smithery.ai/servers?q=x&pageSize=10&page=1"),
        ("claude-code-playground", Kind::Mod, "https://api.github.com/repos/anthropics/claude-code-playground/contents/claude-code/mods"),
    ];
    for (id, kind, url) in cases {
        for body in [&b"{\"skills\": [tru"[..], b"<html>rate limited</html>", b"{\"servers\": 7, \"plugins\": 7, \"data\": 7}"] {
            let t = setup(FakeFetch::new().with(url, body.to_vec()));
            only(&t.m, id, vec![]);
            let e = t.m.search(*kind, "x", None).expect_err(id);
            assert_eq!(e, MarketError::BadAnswer(id.to_string()), "{id}");
        }
    }
}

#[test]
fn offline_source_shows_its_saved_list_with_the_date_and_fresh_copy_is_not_asked_again() {
    let url = "https://skills.sh/api/search?q=pdf&limit=20";
    let t = setup(FakeFetch::new().answer(url, Ok(FetchOut { status: 200, body: fx("skillssh-search.json"), etag: Some("\"v1\"".into()) })));
    only(&t.m, "skills.sh", vec![]);
    t.m.search(Kind::Skill, "pdf", None).unwrap();
    t.m.search(Kind::Skill, "pdf", None).unwrap();
    assert_eq!(t.fetch.calls().len(), 1, "a copy younger than 24 h is used as is");

    // older than the limit: asked again with its ETag; 304 keeps the copy
    let m = Market::new(Box::new(Shared(t.fetch.clone())), t._tmp.path().join("cache")).with_ttl(Duration::ZERO);
    assert_eq!(m.search(Kind::Skill, "pdf", None).unwrap().items.len(), 3);
    assert_eq!(t.fetch.calls().last().unwrap().1.as_deref(), Some("\"v1\""));

    t.fetch.set(url, Err(NetError::Failed("Could not resolve host".into())));
    let p = m.search(Kind::Skill, "pdf", None).unwrap();
    assert_eq!(p.items.len(), 3, "saved list");
    assert_eq!(p.offline.len(), 1);
    assert_eq!(p.offline[0].reason, "no_connection");
    assert!(p.offline[0].cached_at.is_some());

    // never fetched and offline: the error itself
    let e = m.search(Kind::Skill, "docx", None).unwrap_err();
    assert_eq!(e.key(), "no_connection");
}

// ---------------- install ----------------

fn search_pdf(t: &T) -> nightshift_core::market::Item {
    only(&t.m, "skills.sh", vec![]);
    t.m.search(Kind::Skill, "pdf", None).unwrap().items.remove(0)
}

#[test]
fn skill_goes_where_each_environment_reads_it_and_remove_puts_back_what_was_there() {
    let t = setup(FakeFetch::new().with("https://skills.sh/api/search?q=pdf&limit=20", fx("skillssh-search.json")).with("https://codeload.github.com/anthropics/skills/zip/HEAD", skills_zip()));
    let item = search_pdf(&t);
    // research §1.4: Claude ~/.claude/skills, Codex ~/.agents/skills, Gemini ~/.gemini/skills
    for (env, dir) in [("claude", ".claude/skills"), ("codex", ".agents/skills"), ("gemini", ".gemini/skills")] {
        let plan = t.m.install(&item, env, Scope::Global).unwrap();
        assert!(!plan.installed && plan.executable.is_empty(), "{env}");
        assert_eq!(plan.kit.commit.as_deref(), Some(SHA), "the commit shown is the commit installed");
        assert_eq!(plan.texts[0].text, SKILL_MD, "SKILL.md shown in full");
        let snap = t.m.apply(&plan, vec![]).unwrap();
        let f = t.home.join(dir).join("pdf/SKILL.md");
        assert_eq!(std::fs::read_to_string(&f).unwrap(), SKILL_MD, "{env}");
        assert!(t.home.join(dir).join("pdf/reference.md").exists());
        assert!(!t.home.join(dir).join("docx").exists(), "only the chosen skill");

        let again = t.m.install(&item, env, Scope::Global).unwrap();
        assert!(again.installed, "{env}: «установлено»");
        assert!(again.writes.is_empty());
        assert_eq!(t.m.installed(env).unwrap()[0].version, SHA);

        t.m.remove(env, &item.id).unwrap();
        assert!(!f.exists(), "{env}: removed");
        assert!(t.m.installed(env).unwrap().is_empty());

        // rollback of the snapshot does the same
        let plan = t.m.install(&item, env, Scope::Global).unwrap();
        let snap2 = t.m.apply(&plan, vec![]).unwrap();
        assert_ne!(snap.id, snap2.id);
        t.kits.rollback(&snap2.id).unwrap();
        assert!(!f.exists(), "{env}: rolled back");
    }

    // a skill that was there before comes back as it was
    let mine = t.home.join(".claude/skills/pdf/SKILL.md");
    std::fs::create_dir_all(mine.parent().unwrap()).unwrap();
    std::fs::write(&mine, "my own pdf skill\n").unwrap();
    let plan = t.m.install(&item, "claude", Scope::Global).unwrap();
    assert!(plan.kit.entries[0].diff.is_some(), "~ заменится, разница видна");
    t.m.apply(&plan, vec![]).unwrap();
    assert_eq!(std::fs::read_to_string(&mine).unwrap(), SKILL_MD);
    t.m.remove("claude", &item.id).unwrap();
    assert_eq!(std::fs::read_to_string(&mine).unwrap(), "my own pdf skill\n");

    // no confirmed skills folder: refused
    assert!(matches!(t.m.install(&item, "cursor", Scope::Global), Err(MarketError::EnvUnsupported { .. })));
}

fn listing(home: &Path) -> Vec<String> {
    let mut out = vec![];
    fn walk(d: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                walk(&e.path(), out);
            } else {
                out.push(e.path().display().to_string());
            }
        }
    }
    walk(home, &mut out);
    out
}

#[test]
fn bad_archives_are_refused_and_nothing_is_written() {
    let url = "https://codeload.github.com/anthropics/skills/zip/HEAD";
    let big_zero = vec![0u8; 101 * 1024 * 1024];
    let symlink = {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("skills-HEAD/skills/pdf/SKILL.md", o).unwrap();
            z.write_all(SKILL_MD.as_bytes()).unwrap();
            z.add_symlink("skills-HEAD/skills/pdf/key", "/Users/x/.ssh/id_rsa", o).unwrap();
            z.finish().unwrap();
        }
        buf.into_inner()
    };
    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        ("dotdot", zip_of(&[("skills-HEAD/skills/pdf/SKILL.md", SKILL_MD.as_bytes()), ("skills-HEAD/skills/pdf/../../../evil.sh", b"rm -rf ~")], None), "unsafe_path"),
        ("absolute", zip_of(&[("skills-HEAD/skills/pdf/SKILL.md", SKILL_MD.as_bytes()), ("/etc/evil", b"x")], None), "unsafe_path"),
        ("symlink", symlink, "symlink"),
        ("over 100 MB unpacked", zip_of(&[("skills-HEAD/skills/pdf/SKILL.md", SKILL_MD.as_bytes()), ("skills-HEAD/skills/pdf/blob.bin", &big_zero)], None), "kit_too_large"),
        ("over 20 MB download", vec![7u8; MAX_ITEM + 1], "too_large"),
        ("not a zip", b"<html>404</html>".to_vec(), "bad_zip"),
    ];
    for (what, body, key) in cases {
        let t = setup(FakeFetch::new().with("https://skills.sh/api/search?q=pdf&limit=20", fx("skillssh-search.json")).with(url, body));
        let item = search_pdf(&t);
        let e = t.m.install(&item, "claude", Scope::Global).expect_err(what);
        assert_eq!(e.key(), key, "{what}: {e:?}");
        assert!(listing(&t.home).is_empty(), "{what}: nothing written");
    }
}

fn copy_dir(from: &Path, to: &Path) {
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let p = to.join(e.file_name());
        if e.path().is_dir() {
            std::fs::create_dir_all(&p).unwrap();
            copy_dir(&e.path(), &p);
        } else {
            std::fs::write(&p, std::fs::read(e.path()).unwrap()).unwrap();
        }
    }
}

fn clawd() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/market/clawd-run")
}

#[test]
fn clawd_run_mod_moves_with_a_kit_marked_executable_and_off_until_ticked() {
    let src = setup(FakeFetch::new());
    let modir = src.home.join(".claude/mods/clawd-run");
    std::fs::create_dir_all(&modir).unwrap();
    copy_dir(&clawd(), &modir);
    // written by Claude Code itself: never exported
    std::fs::create_dir_all(modir.join(".claude-plugin/types")).unwrap();
    std::fs::write(modir.join(".claude-plugin/types/index.d.ts"), "gen").unwrap();

    let inv = src.kits.inventory("claude", &Scope::Global);
    let mods = inv.groups.iter().find(|g| g.group == Group::Mods).unwrap();
    assert_eq!(mods.items, vec!["clawd-run".to_string()]);
    assert!(src.kits.inventory("codex", &Scope::Global).groups.iter().any(|g| g.group == Group::Mods && !g.available), "mods: Claude Code only");

    let out = src._tmp.path().join("kit");
    let rep = src.kits.export("claude", &Selection { name: "mine".into(), groups: vec![Group::Mods], scope: Scope::Global }, ExportDest::Folder(out.clone())).unwrap();
    assert!(rep.files.contains(&"mods/clawd-run/hooks/register.tsx".to_string()));
    assert!(!rep.files.iter().any(|f| f.contains(".claude-plugin/types")), "{:?}", rep.files);

    let dst = setup(FakeFetch::new());
    let plan = dst.kits.preview("claude", KitSource::Folder(out), &Scope::Global).unwrap();
    let e = plan.entries.iter().find(|e| e.key == "mod:clawd-run").unwrap();
    let held = e.held.as_ref().expect("a mod is held");
    assert_eq!(held.kind, HeldKind::Mod);
    assert!(held.scripts.iter().any(|s| s.path == "mods/clawd-run/hooks/register.tsx"), "register.tsx is code: {:?}", held.scripts.iter().map(|s| &s.path).collect::<Vec<_>>());
    let snap = dst.kits.install(&plan, &Choices::default()).unwrap();
    let reg = dst.home.join(".claude/mods/clawd-run/hooks/register.tsx");
    assert!(!reg.exists(), "off until ticked");
    dst.kits.enable_item("claude", &HeldRef { snapshot: snap.clone(), key: "mod:clawd-run".into() }).unwrap();
    assert!(reg.exists());
    dst.kits.rollback(&snap).unwrap();
    assert!(!reg.exists(), "snapshot takes it back");
}

#[test]
fn a_mod_from_a_folder_installs_off_and_only_into_claude_code() {
    let t = setup(FakeFetch::new());
    let item = t.m.from_link(Kind::Mod, &clawd().display().to_string()).unwrap();
    assert!(matches!(t.m.install(&item, "codex", Scope::Global), Err(MarketError::EnvUnsupported { kind: Kind::Mod, .. })));
    let plan = t.m.install(&item, "claude", Scope::Global).unwrap();
    assert_eq!(plan.executable, vec!["mod:clawd-run".to_string()]);
    assert!(plan.warnings.contains(&"runs_code".to_string()), "«мод исполняет код на твоём компьютере»");
    assert!(plan.writes.is_empty());
    t.m.apply(&plan, vec![]).unwrap();
    let reg = t.home.join(".claude/mods/clawd-run/hooks/register.tsx");
    assert!(!reg.exists(), "installed off");
    let plan = t.m.install(&item, "claude", Scope::Global).unwrap();
    t.m.apply(&plan, vec!["mod:clawd-run".into()]).unwrap();
    assert!(reg.exists(), "on by the tick");
    t.m.remove("claude", &item.id).unwrap();
    assert!(!reg.exists());
}

fn registry_page(server: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"servers": [{"server": server, "_meta": {"io.modelcontextprotocol.registry/official": {"status": "active", "isLatest": true, "updatedAt": "2026-10-01T00:00:00Z"}}}], "metadata": {"count": 1}})).unwrap()
}

const SEARCH: &str = "https://registry.modelcontextprotocol.io/v0.1/servers?version=latest&limit=10&search=acme";

fn mcp_item(t: &T) -> nightshift_core::market::Item {
    only(&t.m, "mcp-registry", vec![]);
    t.m.search(Kind::Mcp, "acme", None).unwrap().items.remove(0)
}

#[test]
fn mcp_with_a_secret_variable_writes_only_its_name_and_the_value_goes_to_the_keyring() {
    let server = serde_json::json!({
        "name": "io.github.acme/acme-mcp", "description": "Acme", "version": "1.2.3",
        "packages": [{"registryType": "npm", "identifier": "@acme/mcp", "version": "1.2.3", "runtimeHint": "npx", "transport": {"type": "stdio"},
            "environmentVariables": [{"name": "API_KEY", "description": "Acme key", "isRequired": true, "isSecret": true}]}]
    });
    let t = setup(FakeFetch::new().with(SEARCH, registry_page(server)));
    let item = mcp_item(&t);
    let d = t.m.item(&item.id).unwrap();
    assert_eq!(d.runs.as_deref(), Some("npx -y @acme/mcp@1.2.3"), "version pinned");
    assert!(d.env_vars[0].secret && d.env_vars[0].name == "API_KEY");

    let plan = t.m.install(&item, "claude", Scope::Global).unwrap();
    assert_eq!(plan.executable, vec!["mcp:acme-mcp".to_string()], "held until ticked");
    assert!(plan.warnings.contains(&"runs_code".to_string()), "local server: «запускает код»");
    let need = &plan.kit.env_vars[0];
    assert_eq!((need.name.as_str(), need.missing), ("API_KEY", true));
    const VALUE: &str = "acme-live-0123456789abcdef";
    t.kits.set_secret("claude", "API_KEY", VALUE).unwrap();
    assert_eq!(t.ring.0.lock().unwrap().get("nightshift|claude/API_KEY").map(String::as_str), Some(VALUE));
    t.m.apply(&plan, vec!["mcp:acme-mcp".into()]).unwrap();
    let conf = std::fs::read_to_string(t.home.join(".claude.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&conf).unwrap();
    assert_eq!(v["mcpServers"]["acme-mcp"], serde_json::json!({"command": "npx", "args": ["-y", "@acme/mcp@1.2.3"], "env": {"API_KEY": "${API_KEY}"}}));
    assert!(!conf.contains(VALUE));
    assert_eq!(t.kits.env_vars("claude"), vec![("API_KEY".to_string(), VALUE.to_string())], "runs of the environment get it");

    // the same server in the other formats (research §2.5)
    let plan = t.m.install(&item, "codex", Scope::Global).unwrap();
    t.m.apply(&plan, vec!["mcp:acme-mcp".into()]).unwrap();
    let toml = std::fs::read_to_string(t.home.join(".codex/config.toml")).unwrap();
    assert!(toml.contains("[mcp_servers.acme-mcp]") && toml.contains(r#"env_vars = ["API_KEY"]"#) && toml.contains(r#"args = ["-y", "@acme/mcp@1.2.3"]"#), "{toml}");
    let plan = t.m.install(&item, "gemini", Scope::Global).unwrap();
    t.m.apply(&plan, vec!["mcp:acme-mcp".into()]).unwrap();
    let g: serde_json::Value = serde_json::from_slice(&std::fs::read(t.home.join(".gemini/settings.json")).unwrap()).unwrap();
    assert_eq!(g["mcpServers"]["acme-mcp"]["env"]["API_KEY"], "${API_KEY}");
    assert!(g["mcpServers"]["acme-mcp"].get("trust").is_none());
    let plan = t.m.install(&item, "opencode", Scope::Global).unwrap();
    t.m.apply(&plan, vec!["mcp:acme-mcp".into()]).unwrap();
    let o: serde_json::Value = serde_json::from_slice(&std::fs::read(t.home.join(".config/opencode/opencode.json")).unwrap()).unwrap();
    assert_eq!(o["mcp"]["acme-mcp"], serde_json::json!({"type": "local", "command": ["npx", "-y", "@acme/mcp@1.2.3"], "environment": {"API_KEY": "{env:API_KEY}"}, "enabled": true}));

    // not ticked: nothing in the config
    let t2 = setup(FakeFetch::new().with(SEARCH, registry_page(serde_json::json!({"name": "io.github.acme/acme-mcp", "version": "1", "packages": [{"registryType": "npm", "identifier": "@acme/mcp", "version": "1", "transport": {"type": "stdio"}}]}))));
    let it2 = mcp_item(&t2);
    let p2 = t2.m.install(&it2, "claude", Scope::Global).unwrap();
    t2.m.apply(&p2, vec![]).unwrap();
    assert!(!t2.home.join(".claude.json").exists());
}

#[test]
fn remote_mcp_shows_its_address_and_its_token_by_name() {
    let server = serde_json::json!({
        "name": "io.github.netdata/mcp-server", "version": "2.12.1",
        "remotes": [{"type": "streamable-http", "url": "https://app.netdata.cloud/api/v1/mcp",
            "headers": [{"name": "Authorization", "value": "Bearer {NETDATA_CLOUD_API_TOKEN}", "isSecret": true, "isRequired": true}]}]
    });
    let t = setup(FakeFetch::new().with(SEARCH, registry_page(server)));
    let item = mcp_item(&t);
    let plan = t.m.install(&item, "claude", Scope::Global).unwrap();
    assert_eq!(plan.runs.as_deref(), Some("https://app.netdata.cloud/api/v1/mcp"), "address visible before install");
    assert!(!plan.warnings.contains(&"runs_code".to_string()) && plan.warnings.contains(&"remote".to_string()));
    assert_eq!(plan.kit.env_vars[0].name, "NETDATA_CLOUD_API_TOKEN");
    t.m.apply(&plan, vec!["mcp:mcp-server".into()]).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(t.home.join(".claude.json")).unwrap()).unwrap();
    assert_eq!(v["mcpServers"]["mcp-server"], serde_json::json!({"type": "http", "url": "https://app.netdata.cloud/api/v1/mcp", "headers": {"Authorization": "Bearer ${NETDATA_CLOUD_API_TOKEN}"}}));
    let plan = t.m.install(&item, "codex", Scope::Global).unwrap();
    t.m.apply(&plan, vec!["mcp:mcp-server".into()]).unwrap();
    let toml = std::fs::read_to_string(t.home.join(".codex/config.toml")).unwrap();
    assert!(toml.contains(r#"bearer_token_env_var = "NETDATA_CLOUD_API_TOKEN""#), "{toml}");
}

#[test]
fn net_refuses_plain_http_and_answers_over_the_limit_with_too_large() {
    let f = FakeFetch::new().with("https://x.test/a", vec![1u8; 10]);
    assert_eq!(f.get("http://x.test/a", 100, None), Err(NetError::NotHttps("http://x.test/a".into())));
    assert_eq!(f.get("https://x.test/a", 5, None), Err(NetError::TooLarge(5)));
    assert!(matches!(nightshift_core::net::CurlFetch.get("file:///etc/passwd", 100, None), Err(NetError::NotHttps(_))));
}

/// Criterion 6: one live request to each enabled built-in source. `cargo test -p nightshift-core --test market -- --ignored --nocapture`
#[test]
#[ignore]
fn live_each_enabled_source_answers() {
    let tmp = tempfile::tempdir().unwrap();
    let m = Market::new(Box::new(nightshift_core::net::CurlFetch), tmp.path().join("cache"));
    for s in m.sources().unwrap().into_iter().filter(|s| s.enabled) {
        let mut all = m.sources().unwrap();
        for x in all.iter_mut() {
            x.enabled = x.id == s.id;
        }
        m.set_sources(all).unwrap();
        let q = match s.kind {
            Kind::Skill => "pdf",
            Kind::Mcp => "github",
            Kind::Mod => "",
        };
        let t0 = std::time::Instant::now();
        match m.search(s.kind, q, None) {
            Ok(p) => println!("LIVE {:<26} ok   {:>3} items, more: {:<5} {:>5} ms, first: {}", s.id, p.items.len(), p.cursor.is_some(), t0.elapsed().as_millis(), p.items.first().map(|i| i.id.as_str()).unwrap_or("-")),
            Err(e) => println!("LIVE {:<26} FAIL {} ({} ms)", s.id, e, t0.elapsed().as_millis()),
        }
    }
}
