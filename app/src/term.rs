//! The person's own terminal (spec 28): the env's command goes in as a list of arguments, never as one
//! string for a shell to parse. macOS Terminal cannot take arguments, so it gets a one-line script file
//! in which every word is single-quoted; Linux terminals take the list after `-e` / `--`.
use std::path::{Path, PathBuf};
use std::process::Command;

pub type Res<T> = Result<T, String>;

/// What to run in the terminal: `argv` empty = just a shell (with `banner` printed first, if any).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Launch {
    pub cwd: Option<PathBuf>,
    pub argv: Vec<String>,
    /// Printed before the command runs, so the person sees what is about to happen (install, login).
    pub banner: Option<String>,
    /// Variables the adapter put on its command; `env K=V ...` carries them into the terminal.
    pub env: Vec<(String, String)>,
}

/// Session ids go into a terminal command; the rule is the adapters' own, checked again here before anything is opened.
pub use nightshift_core::agent::adapters::common::valid_session_id;

/// The only variables that go from an adapter's command into the terminal: where things are and the language, plus the
/// folders a CLI keeps its sessions in. Anything else (API keys, tokens, kit secrets from `Run.extra_env`) is dropped: it would
/// land in the process arguments and, on macOS, in a file on disk. The person's own shell has their login anyway.
pub const CARRIED_ENV: [&str; 10] = ["PATH", "HOME", "LANG", "LC_ALL", "TMPDIR", "CODEX_HOME", "CLAUDE_CONFIG_DIR", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"];

fn carried(k: &str) -> bool {
    CARRIED_ENV.contains(&k)
}

/// The program, the arguments and the folder of a `Command` an adapter built.
pub fn from_command(c: &Command) -> Launch {
    let mut argv = vec![c.get_program().to_string_lossy().into_owned()];
    argv.extend(c.get_args().map(|a| a.to_string_lossy().into_owned()));
    let env = c.get_envs().filter_map(|(k, v)| Some((k.to_string_lossy().into_owned(), v?.to_string_lossy().into_owned()))).filter(|(k, _)| carried(k)).collect();
    Launch { cwd: c.get_current_dir().map(Path::to_path_buf), argv, banner: None, env }
}

/// Text of the script `sh -c` runs when there is a banner. Fixed text: the words reach it as `$1`, `$@`.
const WITH_BANNER: &str = r#"printf '%s\n\n' "$1"; shift; "$@"; code=$?; printf '\n[exit %s] press Enter to close ' "$code"; read -r _"#;
const SHELL_AFTER_BANNER: &str = r#"printf '%s\n\n' "$1"; exec "${SHELL:-sh}""#;

/// The list of words that finally runs in the terminal.
pub fn final_argv(l: &Launch) -> Vec<String> {
    // `env K=V prog args`: the variables travel as words, no shell reads them
    let vars: Vec<&(String, String)> = l.env.iter().filter(|(k, _)| carried(k)).collect();
    let argv: Vec<String> = if vars.is_empty() || l.argv.is_empty() {
        l.argv.clone()
    } else {
        std::iter::once("env".to_string()).chain(vars.iter().map(|(k, v)| format!("{k}={v}"))).chain(l.argv.iter().cloned()).collect()
    };
    match (&l.banner, l.argv.is_empty()) {
        (None, false) => argv,
        (Some(b), false) => ["sh", "-c", WITH_BANNER, "sh", b.as_str()].iter().map(|s| s.to_string()).chain(argv).collect(),
        (Some(b), true) => ["sh", "-c", SHELL_AFTER_BANNER, "sh", b.as_str()].iter().map(|s| s.to_string()).collect(),
        (None, true) => vec![],
    }
}

pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// The script macOS Terminal opens: `cd` and `exec`, every word quoted.
pub fn mac_script(cwd: Option<&Path>, argv: &[String]) -> String {
    let mut s = String::from("#!/bin/sh\n");
    if let Some(c) = cwd {
        s += &format!("cd {} || exit 1\n", sh_quote(&c.to_string_lossy()));
    }
    if argv.is_empty() {
        s += "exec \"${SHELL:-/bin/sh}\"\n";
    } else {
        s += &format!("exec {}\n", argv.iter().map(|w| sh_quote(w)).collect::<Vec<_>>().join(" "));
    }
    s
}

/// The terminals tried on Linux after `$TERMINAL`.
pub const LINUX_TERMINALS: [&str; 6] = ["x-terminal-emulator", "kitty", "gnome-terminal", "konsole", "xterm", "alacritty"];

/// The program and arguments that open `term` on Linux with `argv` (the working folder of kitty, gnome-terminal and
/// konsole is a flag; the others take it from the process).
pub fn linux_invocation(term: &str, cwd: Option<&Path>, argv: &[String]) -> (String, Vec<String>) {
    let dir = cwd.map(|c| c.to_string_lossy().into_owned());
    let mut a: Vec<String> = vec![];
    match term.rsplit('/').next().unwrap_or(term) {
        "kitty" => {
            if let Some(d) = &dir {
                a.extend(["--directory".to_string(), d.clone()]);
            }
        }
        "gnome-terminal" => {
            if let Some(d) = &dir {
                a.push(format!("--working-directory={d}"));
            }
            if !argv.is_empty() {
                a.push("--".into());
            }
        }
        "konsole" => {
            if let Some(d) = &dir {
                a.extend(["--workdir".to_string(), d.clone()]);
            }
            if !argv.is_empty() {
                a.push("-e".into());
            }
        }
        _ => {
            if !argv.is_empty() {
                a.push("-e".into());
            }
        }
    }
    a.extend(argv.iter().cloned());
    (term.to_string(), a)
}

/// Opens the terminal. `scratch` is a private folder of the app for the macOS script.
pub fn open(l: &Launch, scratch: &Path) -> Res<()> {
    let argv = final_argv(l);
    if argv.iter().any(|w| w.contains('\0')) {
        return Err("bad_argument".into());
    }
    // just a terminal in a folder: macOS opens one natively, no script needed
    if cfg!(target_os = "macos") && argv.is_empty() {
        if let Some(dir) = &l.cwd {
            return Command::new("open").arg("-a").arg("Terminal").arg(dir).spawn().map(|_| ()).map_err(|e| format!("open: {e}"));
        }
    }
    if cfg!(target_os = "macos") {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(scratch).map_err(|e| e.to_string())?;
        std::fs::set_permissions(scratch, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        // scripts of earlier terminals are not kept
        for e in std::fs::read_dir(scratch).into_iter().flatten().flatten() {
            let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|d| d.as_secs() > 3600);
            if old && e.file_name().to_string_lossy().starts_with("terminal-") {
                let _ = std::fs::remove_file(e.path());
            }
        }
        let file = scratch.join(format!("terminal-{}-{}.command", std::process::id(), now_nanos()));
        std::fs::write(&file, mac_script(l.cwd.as_deref(), &argv)).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        return Command::new("open").arg("-a").arg("Terminal").arg(&file).spawn().map(|_| ()).map_err(|e| format!("open: {e}"));
    }
    let env_term = std::env::var("TERMINAL").ok().filter(|t| !t.is_empty());
    for t in env_term.iter().map(String::as_str).chain(LINUX_TERMINALS) {
        let (prog, args) = linux_invocation(t, l.cwd.as_deref(), &argv);
        let mut c = Command::new(prog);
        c.args(args);
        if let Some(d) = &l.cwd {
            c.current_dir(d);
        }
        if c.spawn().is_ok() {
            return Ok(());
        }
    }
    Err("no_terminal: set $TERMINAL".into())
}

fn now_nanos() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
}

/// An install hint like `npm i -g @openai/codex (не сверено)` as words, when it is plain words: letters, digits and
/// `@ / . _ : = + ~ -`. Anything with a pipe, quotes, `;`, `&`, `$` or a redirect is not run by the app (None).
pub fn hint_words(hint: &str) -> Option<Vec<String>> {
    let cmd = hint.split(" (").next().unwrap_or(hint).trim();
    let words: Vec<String> = cmd.split_whitespace().map(String::from).collect();
    let plain = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '/' | '.' | '_' | ':' | '=' | '+' | '~' | '-'));
    (!words.is_empty() && words.iter().all(|w| plain(w))).then_some(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_session_ids_are_taken() {
        for ok in ["abc-123", "3f2a9c1e-77aa-4d11-8e5b-0a1b2c3d4e5f", "ses_1.2:3"] {
            assert!(valid_session_id(ok), "{ok}");
        }
        for bad in ["", "-r", "a b", "a;b", "$(touch x)", "`id`", "a|b", "a&b", "a\nb", "a'b", "a\"b", "../x", &"a".repeat(129)] {
            assert!(!valid_session_id(bad), "{bad:?}");
        }
    }

    #[test]
    fn linux_terminals_get_the_words_as_a_list_and_the_folder_as_their_own_flag() {
        let argv: Vec<String> = ["claude", "--resume", "a b;$(x)"].map(String::from).to_vec();
        let d = Path::new("/tmp/copy 1");
        let (p, a) = linux_invocation("gnome-terminal", Some(d), &argv);
        assert_eq!((p.as_str(), a), ("gnome-terminal", ["--working-directory=/tmp/copy 1", "--", "claude", "--resume", "a b;$(x)"].map(String::from).to_vec()));
        assert_eq!(linux_invocation("kitty", Some(d), &argv).1, ["--directory", "/tmp/copy 1", "claude", "--resume", "a b;$(x)"].map(String::from).to_vec());
        assert_eq!(linux_invocation("/usr/bin/konsole", Some(d), &argv).1, ["--workdir", "/tmp/copy 1", "-e", "claude", "--resume", "a b;$(x)"].map(String::from).to_vec());
        assert_eq!(linux_invocation("xterm", None, &argv).1, ["-e", "claude", "--resume", "a b;$(x)"].map(String::from).to_vec());
    }

    #[test]
    fn the_mac_script_quotes_every_word_so_nothing_is_parsed() {
        let argv: Vec<String> = ["/opt/bin/claude", "--resume", "it's $(id); x"].map(String::from).to_vec();
        let s = mac_script(Some(Path::new("/tmp/my copy")), &argv);
        assert_eq!(s, "#!/bin/sh\ncd '/tmp/my copy' || exit 1\nexec '/opt/bin/claude' '--resume' 'it'\\''s $(id); x'\n");
    }

    #[test]
    fn a_banner_runs_through_a_fixed_script_and_the_words_stay_positional() {
        let l = Launch { cwd: None, argv: vec!["npm".into(), "i".into(), "-g".into(), "x y".into()], banner: Some("npm i -g x y".into()), env: vec![] };
        let a = final_argv(&l);
        assert_eq!(a[..5], ["sh", "-c", WITH_BANNER, "sh", "npm i -g x y"].map(String::from));
        assert_eq!(a[5..], ["npm", "i", "-g", "x y"].map(String::from));
        assert!(!WITH_BANNER.contains("npm"), "the script text has none of the words");
    }

    #[test]
    fn the_variables_of_the_adapters_command_go_into_the_terminal_as_env_words_but_only_the_allowed_ones() {
        assert_eq!(CARRIED_ENV, ["PATH", "HOME", "LANG", "LC_ALL", "TMPDIR", "CODEX_HOME", "CLAUDE_CONFIG_DIR", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"], "the list is fixed");
        let mut c = Command::new("/opt/bin/codex");
        c.current_dir("/tmp").arg("resume").env("CODEX_HOME", "/h o m e").env_remove("GONE").env("ANTHROPIC_API_KEY", "sk-secret-123").env("FAKE_TOKEN", "tok-456");
        let l = from_command(&c);
        assert_eq!(l.env, vec![("CODEX_HOME".to_string(), "/h o m e".to_string())], "a removed variable is not carried");
        assert_eq!(final_argv(&l), ["env", "CODEX_HOME=/h o m e", "/opt/bin/codex", "resume"].map(String::from).to_vec());
        // a secret is in neither the words of the terminal command nor the script file, even if it reached the Launch by hand
        let mut forged = l.clone();
        forged.env.push(("API_TOKEN".into(), "sk-secret-123".into()));
        let words = final_argv(&forged);
        let script = mac_script(None, &words);
        for text in [words.join(" "), script, linux_invocation("xterm", None, &words).1.join(" ")] {
            assert!(!text.contains("secret-123") && !text.contains("tok-456"), "{text}");
        }
        assert_eq!(final_argv(&from_command(&Command::new("x"))), ["x"].map(String::from).to_vec(), "none: the command as it is");
    }

    #[test]
    fn install_hints_run_only_when_they_are_plain_words() {
        assert_eq!(hint_words("npm i -g @openai/codex (не сверено)"), Some(["npm", "i", "-g", "@openai/codex"].map(String::from).to_vec()));
        assert_eq!(hint_words("curl https://cursor.com/install -fsS | bash (не сверено)"), None);
        assert_eq!(hint_words("npm i -g x; rm -rf ~"), None);
        assert_eq!(hint_words("echo $(id)"), None);
        assert_eq!(hint_words(""), None);
    }
}
