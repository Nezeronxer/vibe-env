//! Task 20: CLI versions, install method, update with rollback, new models radar (stories 274-277, 299).
use nightshift_core::agent::adapters::{Claude, Gemini, OpenCode, Qwen};
use nightshift_core::agent::{Adapter, FakeAdapter};
use nightshift_core::model::ModelFamily;
use nightshift_core::models;
use nightshift_core::net::{FakeFetch, FetchOut};
use nightshift_core::store::Store;
use nightshift_core::updates::{self, Method, Updates, MODELS_DEV};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/updates").join(name)).unwrap()
}

fn write_exec(p: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
}

const QWEN_LATEST: &str = "https://registry.npmjs.org/@qwen-code/qwen-code/latest";

/// A fake npm prefix: `bin/npm` installs by writing the version into `state`; `@latest` writes `broken`
/// when the file `break` exists. `bin/qwen` -> `lib/node_modules/@qwen-code/qwen-code/cli.sh` prints the state.
fn fake_npm_qwen(root: &Path, installed: &str) -> PathBuf {
    let prefix = root.join("prefix");
    let state = root.join("state");
    std::fs::write(&state, installed).unwrap();
    let cli = prefix.join("lib/node_modules/@qwen-code/qwen-code/cli.sh");
    write_exec(
        &cli,
        &format!(
            "#!/bin/sh\nv=$(cat '{s}')\n[ \"$v\" = broken ] && exit 1\ncase \"$1\" in --version) echo \"$v\";; --help) echo usage;; esac\n",
            s = state.display()
        ),
    );
    write_exec(
        &prefix.join("bin/npm"),
        &format!(
            "#!/bin/sh\necho \"$@\" >> '{log}'\nv=${{3##*@}}\nif [ \"$v\" = latest ]; then if [ -f '{brk}' ]; then v=broken; else v=0.26.0; fi; fi\necho \"$v\" > '{s}'\n",
            log = root.join("npm.log").display(),
            brk = root.join("break").display(),
            s = state.display()
        ),
    );
    std::os::unix::fs::symlink(&cli, prefix.join("bin/qwen")).unwrap();
    prefix.join("bin/qwen")
}

fn qwen_updates(root: &Path, bin: &Path) -> Updates {
    let fetch = FakeFetch::new().with(QWEN_LATEST, fixture("npm_qwen_latest.json"));
    Updates::new(Box::new(fetch), vec![Arc::new(Qwen::with_bin(bin.to_str().unwrap()))]).with_dir(root.join("data"))
}

// ---------- criterion 1: version, install method, update command, never sudo ----------

#[test]
fn versions_are_read_from_real_cli_output() {
    assert_eq!(updates::parse_version(&fixture("version_claude.txt")).as_deref(), Some("2.1.309"));
    assert_eq!(updates::parse_version(&fixture("version_opencode.txt")).as_deref(), Some("1.18.34"));
    assert_eq!(updates::parse_version(&fixture("version_qwen.txt")).as_deref(), Some("0.22.0"));
    assert_eq!(updates::parse_version("usage: qwen [options]"), None);
}

#[test]
fn install_method_comes_from_the_real_path() {
    let p = |s: &str| PathBuf::from(s);
    assert_eq!(updates::install_method("codex", &p("/opt/homebrew/lib/node_modules/@openai/codex/bin/codex.js")), Method::Npm);
    assert_eq!(updates::install_method("claude", &p("/opt/homebrew/Caskroom/claude-code/2.1.300/claude")), Method::Brew);
    assert_eq!(updates::install_method("gemini", &p("/opt/homebrew/Cellar/gemini-cli/0.65.0/bin/gemini")), Method::Brew);
    assert_eq!(updates::install_method("claude", &p("/Users/me/.local/share/claude/versions/2.1.309")), Method::Native);
    assert_eq!(updates::install_method("qwen", &p("/Users/me/.local/lib/qwen-code/bin/qwen")), Method::Native);
    assert_eq!(updates::install_method("opencode", &p("/Users/me/.opencode/bin/opencode")), Method::Native);
    // Gemini has no update command of its own: outside npm and brew nothing is known
    assert_eq!(updates::install_method("gemini", &p("/usr/local/bin/gemini")), Method::Unknown);
}

#[test]
fn update_commands_are_native_or_the_same_manager_and_never_sudo() {
    let bin = Path::new("/Users/me/.local/bin/claude");
    assert_eq!(updates::update_cmd("claude", Method::Native, bin).unwrap(), vec!["/Users/me/.local/bin/claude", "update"]);
    assert_eq!(updates::update_cmd("opencode", Method::Native, Path::new("/o/opencode")).unwrap(), vec!["/o/opencode", "upgrade"]);
    assert_eq!(updates::update_cmd("qwen", Method::Native, Path::new("/q/qwen")).unwrap(), vec!["/q/qwen", "update"]);
    assert_eq!(updates::update_cmd("cursor", Method::Native, Path::new("/c/agent")).unwrap(), vec!["/c/agent", "update"]);
    let npm = updates::update_cmd("codex", Method::Npm, Path::new("/nowhere/lib/node_modules/@openai/codex/bin/codex.js")).unwrap();
    assert_eq!(npm[1..], ["install", "-g", "@openai/codex@latest"]);
    let brew = updates::update_cmd("claude", Method::Brew, Path::new("/opt/homebrew/Caskroom/claude-code/2.1.300/claude")).unwrap();
    assert!(brew.ends_with(&["upgrade".into(), "--cask".into(), "claude-code".into()]), "{brew:?}");
    assert!(brew.contains(&"HOMEBREW_NO_AUTO_UPDATE=1".to_string()), "{brew:?}");
    assert_eq!(updates::update_cmd("gemini", Method::Unknown, Path::new("/usr/local/bin/gemini")), None);
    for env in ["claude", "codex", "gemini", "opencode", "qwen", "cursor"] {
        for m in [Method::Native, Method::Npm, Method::Brew, Method::Unknown] {
            for p in ["/x/lib/node_modules/pkg/cli.js", "/opt/homebrew/Cellar/f/1/bin/f", "/x/bin/cli"] {
                if let Some(cmd) = updates::update_cmd(env, m, Path::new(p)) {
                    assert!(!cmd.iter().any(|a| a.contains("sudo")), "{env} {m:?}: {cmd:?}");
                }
            }
        }
    }
}

#[test]
fn check_reads_installed_latest_and_the_command() {
    let t = tempfile::tempdir().unwrap();
    let bin = fake_npm_qwen(t.path(), "0.22.0");
    let st = qwen_updates(t.path(), &bin).check("qwen").unwrap();
    assert_eq!(st.installed.as_deref(), Some("0.22.0"));
    assert_eq!(st.latest.as_deref(), Some("0.26.0")); // from the saved npm answer
    assert_eq!(st.method, Method::Npm);
    assert!(st.cmd[0].ends_with("prefix/bin/npm"), "{:?}", st.cmd);
    assert_eq!(st.cmd[1..], ["install", "-g", "@qwen-code/qwen-code@latest"]);
    assert!(!st.major);
}

// ---------- criterion 2: waits for running launches, rolls back when --version fails ----------

#[test]
fn update_waits_while_the_env_has_running_launches() {
    let t = tempfile::tempdir().unwrap();
    let bin = fake_npm_qwen(t.path(), "0.22.0");
    let busy = Arc::new(AtomicBool::new(true));
    let b = busy.clone();
    let u = qwen_updates(t.path(), &bin).with_busy(Arc::new(move |env: &str| env == "qwen" && b.load(Ordering::SeqCst)));
    let b2 = busy.clone();
    let log = t.path().join("npm.log");
    let freed = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(700));
        let ran_while_busy = log.exists();
        b2.store(false, Ordering::SeqCst);
        ran_while_busy
    });
    let start = Instant::now();
    let r = u.update("qwen").unwrap();
    assert!(!freed.join().unwrap(), "the installer ran while a launch was going");
    assert!(start.elapsed() >= Duration::from_millis(700));
    assert!(r.ok, "{r:?}");
    assert_eq!((r.from.as_deref(), r.to.as_deref()), (Some("0.22.0"), Some("0.26.0")));
    assert_eq!(u.history("qwen").len(), 1);
}

#[test]
fn a_cli_broken_by_the_update_is_rolled_back_with_the_same_manager() {
    let t = tempfile::tempdir().unwrap();
    let bin = fake_npm_qwen(t.path(), "0.22.0");
    std::fs::write(t.path().join("break"), "").unwrap();
    let u = qwen_updates(t.path(), &bin);
    let r = u.update("qwen").unwrap();
    assert!(!r.ok);
    assert!(r.rolled_back, "{r:?}");
    let calls = std::fs::read_to_string(t.path().join("npm.log")).unwrap();
    assert_eq!(calls.lines().collect::<Vec<_>>(), ["install -g @qwen-code/qwen-code@latest", "install -g @qwen-code/qwen-code@0.22.0"]);
    assert_eq!(std::fs::read_to_string(t.path().join("state")).unwrap().trim(), "0.22.0");
    // the history survives a new Updates over the same folder
    let again = qwen_updates(t.path(), &bin);
    assert_eq!(again.history("qwen").len(), 1);
    assert!(again.history("qwen")[0].rolled_back);
    assert!(again.interrupted().is_empty());
}

#[test]
fn a_major_jump_is_only_reported() {
    let t = tempfile::tempdir().unwrap();
    let bin = fake_npm_qwen(t.path(), "0.22.0");
    let fetch = FakeFetch::new().with(QWEN_LATEST, r#"{"name":"@qwen-code/qwen-code","version":"1.0.0"}"#);
    let u = Updates::new(Box::new(fetch), vec![Arc::new(Qwen::with_bin(bin.to_str().unwrap()))]).with_dir(t.path().join("data"));
    assert!(u.check("qwen").unwrap().major);
    assert_eq!(u.update("qwen").unwrap_err().key(), "major_jump");
    assert!(!t.path().join("npm.log").exists());
}

// ---------- criterion 3, 4: new models ----------

fn store(dir: &Path) -> Mutex<Store> {
    Mutex::new(Store::open(dir).unwrap())
}

fn md(etag: &str, file: &str) -> Result<FetchOut, nightshift_core::net::NetError> {
    Ok(FetchOut { status: 200, body: fixture(file).into_bytes(), etag: Some(etag.into()) })
}

fn fam(id: &str, versions: &[(&str, &str)]) -> ModelFamily {
    nightshift_core::agent::adapters::common::builtin_family(id, id, "openai", "test", versions)
}

#[test]
fn a_new_model_is_announced_once() {
    let t = tempfile::tempdir().unwrap();
    let bin = t.path().join("bin/cli");
    write_exec(&bin, "#!/bin/sh\necho 1.0.0\n");
    let b = bin.to_str().unwrap();
    let adapters: Vec<Arc<dyn Adapter>> = vec![Arc::new(Claude::with_bin(b)), Arc::new(Gemini::with_bin(b))];
    let fetch = Arc::new(FakeFetch::new().answer(MODELS_DEV, md("\"v1\"", "modelsdev_v1.json")));
    let u = Updates::new(Box::new(ArcFetch(fetch.clone())), adapters.clone()).with_dir(t.path().join("data"));
    let st = store(&t.path().join("store"));
    assert!(u.models_radar(&st, false).is_empty(), "the first look only remembers what exists");

    fetch.set(MODELS_DEV, md("\"v2\"", "modelsdev_v2.json"));
    let mut news = u.models_radar(&st, false);
    news.sort_by(|a, b| a.id.cmp(&b.id));
    let got: Vec<_> = news.iter().map(|n| (n.env.as_str(), n.id.as_str(), n.name.as_str(), n.auto)).collect();
    assert_eq!(got, [("claude", "claude-opus-6", "Claude Opus 6", true), ("gemini", "gemini-4-flash", "Gemini 4 Flash", false)]);
    assert_eq!(fetch.calls().last().unwrap().1.as_deref(), Some("\"v1\""), "If-None-Match is sent");

    // 304 (same ETag) and the same ids from a fresh process: nothing new
    assert!(u.models_radar(&st, false).is_empty());
    let u2 = Updates::new(Box::new(ArcFetch(fetch.clone())), adapters.clone()).with_dir(t.path().join("data"));
    fetch.set(MODELS_DEV, md("\"v3\"", "modelsdev_v2.json"));
    assert!(u2.models_radar(&st, false).is_empty());

    // "latest" of Claude moves to Opus 6 by itself; Gemini 4 waits for a button, latest stays 3.8
    let cat = models::catalog("claude", adapters[0].as_ref(), &st.lock().unwrap());
    let opus = cat.families.iter().find(|f| f.id == "opus").unwrap();
    assert_eq!(opus.versions[0], "6");
    assert_eq!(opus.exact["6"], "claude-opus-6");
    let cat = models::catalog("gemini", adapters[1].as_ref(), &st.lock().unwrap());
    let flash = cat.families.iter().find(|f| f.id == "flash").unwrap();
    assert_eq!(flash.versions[0], "3.8");
    assert_eq!(flash.exact["4"], "gemini-4-flash");
    // Gemini 4 Argon is a family of its own, from models.dev; tts and "-latest" copies are not models to offer
    assert!(cat.families.iter().any(|f| f.id == "gemini-4-argon"));
    assert!(!cat.families.iter().any(|f| f.exact.values().any(|e| e.contains("tts") || e.ends_with("latest"))));
}

#[test]
fn codex_offers_only_what_its_cli_lists() {
    let t = tempfile::tempdir().unwrap();
    let st = store(&t.path().join("store"));
    let data = t.path().join("data");
    let fetch = Arc::new(FakeFetch::new().answer(MODELS_DEV, md("\"v1\"", "modelsdev_v1.json")));
    let codex = |fams: Vec<ModelFamily>| -> Vec<Arc<dyn Adapter>> { vec![Arc::new(FakeAdapter::new(vec![]).with_id("codex").with_models(fams))] };
    let old = vec![fam("gpt-sol", &[("6.1", "gpt-6.1-sol")])];
    let u = Updates::new(Box::new(ArcFetch(fetch.clone())), codex(old.clone())).with_dir(data.clone());
    assert!(u.models_radar(&st, true).is_empty());
    // models.dev now has gpt-6.2-sol, the CLI does not: not offered
    fetch.set(MODELS_DEV, md("\"v2\"", "modelsdev_v2.json"));
    assert!(u.models_radar(&st, true).is_empty());
    let cat = models::catalog("codex", codex(old.clone())[0].as_ref(), &st.lock().unwrap());
    assert!(!cat.families.iter().any(|f| f.exact.values().any(|e| e == "gpt-6.2-sol")));
    // the CLI lists it: one event, the family's latest moves
    let new = vec![fam("gpt-sol", &[("6.2", "gpt-6.2-sol"), ("6.1", "gpt-6.1-sol")])];
    let u = Updates::new(Box::new(ArcFetch(fetch.clone())), codex(new.clone())).with_dir(data.clone());
    let news = u.models_radar(&st, true);
    assert_eq!(news.iter().map(|n| (n.env.as_str(), n.id.as_str(), n.auto)).collect::<Vec<_>>(), [("codex", "gpt-6.2-sol", true)]);
    assert!(u.models_radar(&st, true).is_empty());
}

/// `Box<dyn Fetch>` over a shared FakeFetch, so the test can change answers after handing it over.
struct ArcFetch(Arc<FakeFetch>);
impl nightshift_core::net::Fetch for ArcFetch {
    fn get(&self, url: &str, max: usize, etag: Option<&str>) -> Result<FetchOut, nightshift_core::net::NetError> {
        self.0.get(url, max, etag)
    }
}

// ---------- criterion 6: live, read-only ----------

#[test]
#[ignore]
fn live_check_of_the_installed_clis() {
    let adapters: Vec<Arc<dyn Adapter>> = vec![Arc::new(Claude::new()), Arc::new(OpenCode::new()), Arc::new(Qwen::new())];
    let u = Updates::new(Box::new(nightshift_core::net::CurlFetch), adapters);
    for env in ["claude", "opencode", "qwen"] {
        let st = u.check(env);
        eprintln!("LIVE {env}: {st:?}");
        let st = st.unwrap();
        assert!(st.installed.is_some() && st.latest.is_some(), "{env}: {st:?}");
        assert!(!st.cmd.iter().any(|a| a.contains("sudo")));
    }
}
