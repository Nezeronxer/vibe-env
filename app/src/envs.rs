//! The list of envs for the edge of the window: installed, signed in, working, how many wait (R14i.2, G17.2, G17.3).
use crate::hub::{lk, Hub, Res};
use crate::term::{self, Launch};
use nightshift_core::engine::is_active;
use nightshift_core::kits::Scope;
use nightshift_core::model::{Status, Task};
use serde_json::{json, Value};
use std::path::PathBuf;

/// What the last check found out about one env. Checked at start, when the window comes back and every 5 minutes.
#[derive(Clone, Debug, PartialEq)]
pub struct EnvInfo {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub error: Option<String>,
    pub install_hint: Option<String>,
    pub kit_dir: String,
    pub custom: bool,
    pub configured: bool,
    pub sandbox: Value,
    pub path: Option<String>,
    /// What `--version` printed; empty when it cannot be read (no CLI, an own command).
    pub version: String,
}

/// Where the CLI of an env is, as its adapter knows it (the settings' path, else the PATH).
pub fn cli_of(hub: &Hub, env: &str) -> Option<PathBuf> {
    hub.adapter(env)?.cli_path()
}

impl Hub {
    /// Asks every adapter again. True when something changed since the last check.
    pub fn refresh_envs(&self) -> bool {
        let settings = self.settings();
        let adapters = lk(&self.adapters).clone();
        let list: Vec<EnvInfo> = adapters
            .iter()
            .map(|a| {
                let id = a.id().to_string();
                let r = a.available();
                let r_ok = r.is_ok();
                let custom = settings.custom_agents.iter().any(|c| c.id == id);
                let configured = !custom || settings.custom_agents.iter().any(|c| c.id == id && !c.run.trim().is_empty());
                let sb = a.sandbox_info();
                EnvInfo {
                    name: a.name().to_string(),
                    installed: r_ok,
                    error: r.err(),
                    install_hint: a.install_hint(),
                    kit_dir: self.kits.inventory(&id, &Scope::Global).dir,
                    custom,
                    configured,
                    sandbox: json!({ "sandboxed": sb.sandboxed, "push_blocked": sb.push_blocked, "network_open": sb.network_open,
                        "writes_outside_copy": sb.writes_outside_copy, "note": sb.note }),
                    path: a.cli_path().map(|p| p.to_string_lossy().into_owned()),
                    version: if r_ok { a.version().unwrap_or_default() } else { String::new() },
                    id,
                }
            })
            .collect();
        let mut cur = lk(&self.envs);
        let changed = *cur != list;
        *cur = list;
        changed
    }

    /// The window is back, or 5 minutes passed (G17.3): every env is asked again, and the engine drops what it remembered
    /// of their availability, so the tasks that wait for an env or its login try it at once. Nothing is moved to another env.
    pub fn recheck_envs(&self) -> bool {
        let list = lk(&self.adapters).clone();
        self.engine.set_adapters(list);
        self.refresh_envs()
    }

    pub fn list_envs(&self) -> Vec<Value> {
        if lk(&self.envs).is_empty() {
            self.refresh_envs();
        }
        let tasks = lk(self.engine.store()).tasks();
        let envs = lk(&self.envs).clone();
        envs.iter().map(|e| env_json(e, &tasks)).collect()
    }
}

impl Hub {
    /// "Установить": the install command opens in the person's terminal in front of them, not in the background. A plain
    /// `npm i -g ...` runs as a list of words; a line with a pipe or quotes is only shown there, to be run by hand (spec 33).
    pub fn install_env(&self, env: &str, launch: &dyn Fn(&Launch) -> Res<()>) -> Res<Value> {
        let hint = self.adapter(env).ok_or_else(|| format!("agent_not_found: {env}"))?.install_hint().ok_or("no_install_hint")?;
        let words = term::hint_words(&hint);
        let shown = hint.split(" (").next().unwrap_or(&hint).trim().to_string();
        launch(&Launch { cwd: None, argv: words.clone().unwrap_or_default(), banner: Some(shown), env: vec![] })?;
        Ok(json!({ "ran": words.is_some() }))
    }

    /// "Войти": the CLI of the env opens in the terminal and asks for the login its own way; the tasks of the env go on
    /// by themselves after the window comes back (the engine tries again every minute).
    pub fn login_env(&self, env: &str, launch: &dyn Fn(&Launch) -> Res<()>) -> Res<()> {
        let a = self.adapter(env).ok_or_else(|| format!("agent_not_found: {env}"))?;
        if self.settings().custom_agents.iter().any(|c| c.id == env) {
            return Err("login_not_supported".into());
        }
        let cli = cli_of(self, env).ok_or_else(|| format!("not_installed: {}", a.install_hint().unwrap_or_default()))?;
        launch(&Launch { cwd: None, argv: vec![cli.to_string_lossy().into_owned()], banner: None, env: vec![] })
    }
}

/// Waits for the person: a question, a permission, a merge that needs help.
pub fn waits_for_human(t: &Task) -> bool {
    matches!(t.status, Status::NeedsHuman | Status::AwaitingOutward | Status::MergeBlocked)
}

/// "9+" on the env's icon (G15.4).
pub fn badge(n: usize) -> String {
    match n {
        0 => String::new(),
        1..=9 => n.to_string(),
        _ => "9+".into(),
    }
}

/// How the window installs an env in its own terminal: the line to show, the words to run when it is plain words
/// (`term::hint_words`), else `argv: null` and `piped: true` (a pipe, quotes, `;`, `$`: the app never runs it itself).
pub fn install_json(hint: Option<&str>) -> Value {
    let Some(hint) = hint else { return Value::Null };
    let argv = term::hint_words(hint);
    json!({ "line": hint.split(" (").next().unwrap_or(hint).trim(), "piped": argv.is_none(), "argv": argv })
}

fn env_json(e: &EnvInfo, tasks: &[Task]) -> Value {
    let mine = || tasks.iter().filter(|t| t.agent == e.id);
    let waiting = mine().filter(|t| waits_for_human(t)).count();
    // the last known sign: a task of this env is waiting for a login (the engine asks once per env)
    let needs_login = mine().any(|t| t.status == Status::NeedsHuman && t.question.as_ref().is_some_and(|q| q.text.starts_with("auth_required")));
    json!({
        "id": e.id, "name": e.name, "installed": e.installed, "needs_login": needs_login, "error": e.error,
        "version": e.version, "install_hint": e.install_hint, "kit_dir": e.kit_dir, "custom": e.custom, "configured": e.configured,
        "sandbox": e.sandbox, "path": e.path, "install": install_json(e.install_hint.as_deref()),
        "waiting": waiting, "badge": badge(waiting), "working": mine().any(|t| is_active(&t.status)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::*;
    use nightshift_core::agent;
    use nightshift_core::model::{CustomAgent, Question, QuestionKind, Task};
    use nightshift_core::notify::NullNotifier;
    use nightshift_core::store::Store;
    use std::sync::Arc;

    fn task(env: &str, status: Status, question: Option<&str>) -> Task {
        Task {
            id: uuid_like(env, &status),
            agent: env.into(),
            status,
            question: question.map(|q| Question { text: q.into(), options: vec![], kind: QuestionKind::Agent }),
            ..Task::default()
        }
    }
    fn uuid_like(env: &str, s: &Status) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        format!("{env}-{s:?}-{}", N.fetch_add(1, Ordering::Relaxed))
    }

    #[test]
    fn the_list_has_the_builtin_envs_and_the_own_command_with_what_waits_and_what_works() {
        let s = Scratch::new("envs");
        let mut store = Store::open(&s.0.join("data")).unwrap();
        let mut st = store.settings();
        st.custom_agents = vec![
            CustomAgent { id: "custom:mine".into(), name: "Mine".into(), run: "echo {prompt}".into(), resume: None },
            CustomAgent { id: "custom:empty".into(), name: "Empty".into(), run: " ".into(), resume: None },
        ];
        store.set_settings(st.clone()).unwrap();
        for t in [
            task("claude", Status::NeedsHuman, Some("auth_required: Claude Code")),
            task("claude", Status::AwaitingOutward, None),
            task("claude", Status::Working, None),
            task("codex", Status::NeedsHuman, Some("which file?")),
            task("codex", Status::Done, None),
        ] {
            store.upsert_task(&t).unwrap();
        }
        let kits = nightshift_core::kits::Kits::new(s.0.join("home"), s.0.join("kits")).with_secret_store(Box::new(MemKeyring::default()));
        let hub = Hub::new(store, agent::adapters(&st), Arc::new(NullNotifier), kits, s.0.join("data"), Arc::new(chrono::Local::now));

        let envs = hub.list_envs();
        let ids: Vec<&str> = envs.iter().map(|e| e["id"].as_str().unwrap()).collect();
        assert_eq!(
            ids,
            ["claude", "codex", "gemini", "opencode", "cursor", "qwen", "kimi", "copilot", "crush", "amp", "goose", "droid", "aider", "grok", "custom:mine", "custom:empty"]
        );
        let by = |id: &str| envs.iter().find(|e| e["id"] == id).unwrap().clone();
        let claude = by("claude");
        assert_eq!((claude["waiting"].as_u64(), claude["working"].as_bool(), claude["needs_login"].as_bool()), (Some(2), Some(true), Some(true)));
        assert_eq!(claude["badge"], "2");
        let codex = by("codex");
        assert_eq!((codex["waiting"].as_u64(), codex["working"].as_bool(), codex["needs_login"].as_bool()), (Some(1), Some(false), Some(false)));
        assert!(claude["install_hint"].as_str().unwrap().contains("claude-code"), "the hint of an env comes from its adapter");
        assert_eq!((by("custom:mine")["custom"].as_bool(), by("custom:mine")["configured"].as_bool()), (Some(true), Some(true)));
        assert_eq!(by("custom:empty")["configured"].as_bool(), Some(false));
        assert!(!by("gemini")["install_hint"].is_null());
        // the sandbox of an env is shown as the adapter says: Claude runs inside one
        assert_eq!(claude["sandbox"]["sandboxed"], true);
    }

    #[test]
    fn install_opens_the_terminal_with_the_command_as_words_or_only_shows_a_line_that_needs_a_shell_and_login_opens_the_cli() {
        use std::cell::RefCell;
        let s = Scratch::new("envs-launch");
        let store = Store::open(&s.0.join("data")).unwrap();
        let kits = nightshift_core::kits::Kits::new(s.0.join("home"), s.0.join("kits")).with_secret_store(Box::new(MemKeyring::default()));
        let hub = Hub::new(store, agent::adapters(&Default::default()), Arc::new(NullNotifier), kits, s.0.join("data"), Arc::new(chrono::Local::now));
        let seen = RefCell::new(vec![]);
        let launch = |l: &Launch| {
            seen.borrow_mut().push(l.clone());
            Ok(())
        };
        assert_eq!(hub.install_env("codex", &launch).unwrap()["ran"], true);
        assert_eq!(hub.install_env("cursor", &launch).unwrap()["ran"], false, "a pipe is never run by the app");
        let seen_now = seen.borrow().clone();
        assert_eq!(seen_now[0], Launch { cwd: None, argv: ["npm", "i", "-g", "@openai/codex"].map(String::from).to_vec(), banner: Some("npm i -g @openai/codex".into()), env: vec![] });
        assert!(seen_now[1].argv.is_empty() && seen_now[1].banner.as_deref().unwrap().contains("curl https://cursor.com/install"));
        assert!(hub.install_env("nope", &launch).is_err());

        // login: the CLI where it is found; an env without a CLI on this machine says so
        let bin = s.0.join("claude-bin");
        std::fs::write(&bin, "#!/bin/sh\n").unwrap();
        let mut st = hub.settings();
        st.envs.entry("claude".into()).or_default().cli_path = Some(bin.clone());
        hub.engine.store().lock().unwrap().set_settings(st).unwrap();
        hub.reload_adapters();
        hub.login_env("claude", &launch).unwrap();
        assert_eq!(seen.borrow().last().unwrap().argv, [bin.to_string_lossy().to_string()]);
        let mut st = hub.settings();
        st.envs.entry("qwen".into()).or_default().cli_path = Some(s.0.join("not-there"));
        hub.engine.store().lock().unwrap().set_settings(st).unwrap();
        hub.reload_adapters();
        assert!(hub.login_env("qwen", &launch).unwrap_err().starts_with("not_installed"));
        assert_eq!(hub.login_env("custom:x", &launch).unwrap_err(), "agent_not_found: custom:x");
    }

    #[test]
    fn an_env_that_came_back_is_seen_after_the_recheck_and_the_one_that_left_is_marked_missing() {
        let s = Scratch::new("envs-recheck");
        let store = Store::open(&s.0.join("data")).unwrap();
        let kits = nightshift_core::kits::Kits::new(s.0.join("home"), s.0.join("kits")).with_secret_store(Box::new(MemKeyring::default()));
        let bin = s.0.join("qwen-bin");
        let mut st = store.settings();
        st.envs.entry("qwen".into()).or_default().cli_path = Some(bin.clone());
        let mut store = store;
        store.set_settings(st.clone()).unwrap();
        let hub = Hub::new(store, agent::adapters(&st), Arc::new(NullNotifier), kits, s.0.join("data"), Arc::new(chrono::Local::now));
        let qwen = |h: &Hub| h.list_envs().into_iter().find(|e| e["id"] == "qwen").unwrap();
        assert_eq!(qwen(&hub)["installed"], false);
        assert!(qwen(&hub)["error"].as_str().unwrap().starts_with("not installed"));
        std::fs::write(&bin, "#!/bin/sh\necho 'qwen 1.2.3'\n").unwrap();
        nightshift_core::proc::cmd("chmod").arg("755").arg(&bin).status().unwrap();
        assert_eq!(qwen(&hub)["installed"], false, "the list is the last check until the next one");
        assert_eq!(qwen(&hub)["version"], "");
        assert!(hub.recheck_envs(), "something changed");
        assert_eq!(qwen(&hub)["installed"], true);
        assert_eq!(qwen(&hub)["version"], "qwen 1.2.3", "what the CLI prints for --version");
        std::fs::remove_file(&bin).unwrap();
        assert!(hub.recheck_envs());
        assert_eq!(qwen(&hub)["installed"], false);
        assert!(!hub.recheck_envs(), "nothing changed since");
    }

    #[test]
    fn install_is_the_line_and_its_words_or_null_words_for_a_pipe() {
        assert_eq!(
            install_json(Some("npm i -g @openai/codex (не сверено)")),
            json!({ "line": "npm i -g @openai/codex", "argv": ["npm", "i", "-g", "@openai/codex"], "piped": false })
        );
        assert_eq!(
            install_json(Some("curl -fsSL https://x.ai/cli/install.sh | bash (не сверено)")),
            json!({ "line": "curl -fsSL https://x.ai/cli/install.sh | bash", "argv": null, "piped": true })
        );
        for bad in ["a; b", "a $(id)", "echo 'x'", "a && b"] {
            assert_eq!(install_json(Some(bad))["piped"], true, "{bad}");
        }
        assert!(install_json(None).is_null());
        // in the list: every built-in env has one, the own command has none
        let s = Scratch::new("envs-install");
        let store = Store::open(&s.0.join("data")).unwrap();
        let kits = nightshift_core::kits::Kits::new(s.0.join("home"), s.0.join("kits")).with_secret_store(Box::new(MemKeyring::default()));
        let mut st = store.settings();
        st.custom_agents = vec![CustomAgent { id: "custom:mine".into(), name: "Mine".into(), run: "echo {prompt}".into(), resume: None }];
        let mut store = store;
        store.set_settings(st.clone()).unwrap();
        let hub = Hub::new(store, agent::adapters(&st), Arc::new(NullNotifier), kits, s.0.join("data"), Arc::new(chrono::Local::now));
        for e in hub.list_envs() {
            let i = &e["install"];
            if e["custom"] == true {
                assert!(i.is_null(), "{e}");
                continue;
            }
            assert!(i["line"].as_str().is_some_and(|l| !l.is_empty() && !l.contains("(не сверено)")), "{e}");
            assert_eq!(i["piped"], i["argv"].is_null(), "{e}");
            assert!(!i["line"].as_str().unwrap().contains("sudo"), "{e}");
        }
    }

    #[test]
    fn the_badge_stops_at_nine_plus() {
        assert_eq!([badge(0), badge(1), badge(9), badge(10), badge(30)], ["", "1", "9", "9+", "9+"]);
    }
}
