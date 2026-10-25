//! GUI apps start with a minimal PATH; agents, gh and node live elsewhere.
use std::process::Command;

/// Markers of a Claude Code session the app was started from (`open` in its terminal): an agent that inherits them
/// counts as that session's child, saves no transcript (so no `--continue`) and gets its messaging token.
const PARENT_SESSION: [&str; 8] = [
    "CLAUDECODE",
    "CLAUDE_PID",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_MESSAGING_SOCKET",
];

pub fn fix_path() {
    for k in PARENT_SESSION {
        std::env::remove_var(k);
    }
    if !cfg!(unix) {
        return;
    }
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let out = Command::new(shell).args(["-ilc", "printf '\\nNSPATH=%s\\n' \"$PATH\""]).stdin(std::process::Stdio::null()).output();
        let _ = tx.send(out);
    });
    let from_shell = rx
        .recv_timeout(std::time::Duration::from_secs(4))
        .ok()
        .and_then(|r| r.ok())
        .and_then(|o| String::from_utf8_lossy(&o.stdout).lines().find_map(|l| l.strip_prefix("NSPATH=").map(str::to_string)))
        .unwrap_or_default();
    let home = std::env::var("HOME").unwrap_or_default();
    let extra = ["/opt/homebrew/bin", "/usr/local/bin"].map(String::from).into_iter().chain([format!("{home}/.local/bin"), format!("{home}/.cargo/bin")]);
    let cur = std::env::var("PATH").unwrap_or_default();
    let mut seen = vec![];
    for p in from_shell.split(':').map(String::from).chain(extra).chain(cur.split(':').map(String::from)) {
        if !p.is_empty() && !seen.contains(&p) {
            seen.push(p);
        }
    }
    std::env::set_var("PATH", seen.join(":"));
}
