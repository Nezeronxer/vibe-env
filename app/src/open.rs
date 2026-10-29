//! What the window may open: `http(s)` links, and files or folders inside a registered project or the app's copies.
//! Everything else (`file://`, `javascript:`, scripts, bundles, paths outside) is refused.
use crate::hub::{lk, Hub, Res};
use std::path::{Path, PathBuf};
use std::process::Command;


/// Files that may be opened: plain text, code, images, PDF. Everything else (no extension,
/// .terminal, .webloc, .command, .app, scripts, ...) is refused; folders are only revealed.
const OPENABLE: &[&str] = &[
    "md", "txt", "json", "toml", "yaml", "yml", "rs", "ts", "tsx", "go", "java", "c", "h", "cpp", "css", "log",
    "diff", "patch", "csv", "ini", "lock", "png", "jpg", "jpeg", "gif", "webp", "pdf", "sql",
];

/// Types a browser runs as a page (scripts in them execute): shown in the file manager, never opened.
/// Scripts too: Windows runs .js with Windows Script Host on a double click, and an installed Python or Ruby runs
/// .py / .rb on Windows and (Python Launcher) on macOS.
const REVEAL_ONLY: &[&str] = &["html", "htm", "xhtml", "xht", "shtml", "svg", "svgz", "xml", "mht", "mhtml", "js", "jsx", "mjs", "cjs", "py", "pyw", "rb", "php"];

#[derive(Debug, PartialEq)]
pub enum Target {
    Url(String),
    File(PathBuf),
    /// shown in the file manager, never launched
    Dir(PathBuf),
    /// a file a browser would run: its place is shown, it is not opened
    Reveal(PathBuf),
}

/// open_path: https, or a plain file / folder inside a registered project or the worktrees dir.
/// The returned path is the canonical one that passed the check; open exactly that.
pub fn check_openable(target: &str, roots: &[PathBuf]) -> Res<Target> {
    if target.starts_with("https://") {
        // the opener (explorer on Windows) reads quotes and spaces itself: such a link is not a link
        if target.chars().any(|c| c.is_whitespace() || c.is_control() || c == '"') {
            return Err(format!("cannot open: {target}"));
        }
        return Ok(Target::Url(target.to_string()));
    }
    let bad = || format!("cannot open: {target}");
    let p = nightshift_core::proc::canon(target).map_err(|_| bad())?;
    if !roots.iter().filter_map(|r| nightshift_core::proc::canon(r).ok()).any(|r| p.starts_with(r)) {
        return Err(bad());
    }
    if p.is_dir() {
        return Ok(Target::Dir(p));
    }
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    #[cfg(unix)]
    let executable = {
        use std::os::unix::fs::PermissionsExt;
        p.metadata().map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(true)
    };
    #[cfg(not(unix))] // Windows runs by extension, and OPENABLE has no program extensions
    let executable = false;
    if p.is_file() && REVEAL_ONLY.contains(&ext.as_str()) {
        return Ok(Target::Reveal(p));
    }
    if p.is_file() && OPENABLE.contains(&ext.as_str()) && !executable {
        Ok(Target::File(p))
    } else {
        Err(bad())
    }
}

/// open_terminal / open_editor: an existing local folder inside the same roots (canonical path returned).
pub fn check_dir(target: &str, roots: &[PathBuf]) -> Res<PathBuf> {
    match check_openable(target, roots)? {
        Target::Dir(p) => Ok(p),
        _ => Err(format!("not a folder: {target}")),
    }
}

/// Starts `c` and lets a thread wait for it, so that when it ends it does not stay a zombie until the app ends (`open`,
/// `xdg-open` and a closed terminal are never waited for otherwise). Returns the pid.
pub fn spawn_reaped(c: &mut Command) -> std::io::Result<u32> {
    let mut child = c.spawn()?;
    let pid = child.id();
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(pid)
}

pub fn opener(target: &Target) -> Res<()> {
    if cfg!(windows) {
        // explorer opens a link in the browser and a file in its program; /select, shows a file in its folder
        let mut c = nightshift_core::proc::cmd("explorer");
        match target {
            Target::Url(u) => c.arg(u),
            Target::File(p) | Target::Dir(p) => c.arg(p),
            Target::Reveal(p) => c.arg(format!("/select,{}", p.display())),
        };
        return spawn_reaped(&mut c).map(|_| ()).map_err(|e| format!("explorer: {e}"));
    }
    let bin = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let mut c = nightshift_core::proc::cmd(bin);
    match target {
        Target::Url(u) => c.arg(u),
        Target::File(p) => c.arg(p),
        Target::Dir(p) if cfg!(target_os = "macos") => c.arg("-R").arg(p), // reveal, never launch a bundle
        Target::Dir(p) => c.arg(p),
        Target::Reveal(p) if cfg!(target_os = "macos") => c.arg("-R").arg(p),
        // no "select this file" on a plain xdg-open: its folder is shown
        Target::Reveal(p) => c.arg(p.parent().unwrap_or(p)),
    };
    spawn_reaped(&mut c).map(|_| ()).map_err(|e| format!("{bin}: {e}"))
}

impl Hub {
    /// Where the window may open things: the projects and the app's copies of tasks and chats.
    pub fn open_roots(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = lk(self.engine.store()).projects().into_iter().map(|p| p.path).collect();
        v.push(self.dir.join("worktrees"));
        v
    }

    pub fn open_path(&self, path: &str) -> Res<()> {
        opener(&check_openable(path, &self.open_roots())?)
    }

    /// `$VISUAL` / `$EDITOR` when it is not a terminal editor, else `code`, else VS Code on macOS, else the folder is shown.
    pub fn open_editor(&self, path: &str) -> Res<()> {
        let dir = check_dir(path, &self.open_roots())?;
        let path = dir.to_string_lossy().to_string();
        let spawn = |bin: String, args: Vec<String>| spawn_reaped(nightshift_core::proc::cmd(bin).args(args).arg(&path)).map(|_| ()).map_err(|e| e.to_string());
        if let Some(mut parts) = gui_editor() {
            let bin = parts.remove(0);
            return spawn(bin, parts);
        }
        if exists_in_path("code") {
            return spawn("code".into(), vec![]);
        }
        if cfg!(target_os = "macos") && nightshift_core::proc::cmd("open").args(["-a", "Visual Studio Code", &path]).status().map(|s| s.success()).unwrap_or(false) {
            return Ok(());
        }
        opener(&Target::Dir(dir))
    }

    /// The folder (inside the same roots) for a terminal.
    pub fn terminal_folder(&self, path: &str) -> Res<PathBuf> {
        check_dir(path, &self.open_roots())
    }
}

pub fn exists_in_path(bin: &str) -> bool {
    nightshift_core::proc::which(bin).is_some()
}

/// `$VISUAL` / `$EDITOR` only when it is not a terminal editor.
pub fn gui_editor() -> Option<Vec<String>> {
    const TUI: [&str; 11] = ["vi", "vim", "nvim", "nano", "emacs", "micro", "hx", "helix", "pico", "ed", "joe"];
    ["VISUAL", "EDITOR"].iter().filter_map(|k| std::env::var(k).ok()).find_map(|v| {
        let parts: Vec<String> = v.split_whitespace().map(String::from).collect();
        let name = Path::new(parts.first()?).file_name()?.to_string_lossy().to_string();
        (!TUI.contains(&name.as_str()) && !parts.contains(&"-nw".to_string())).then_some(parts)
    })
}


#[cfg(test)]
mod tests {
    #[cfg(unix)]
    #[test]
    fn a_process_started_for_the_person_is_waited_for_and_does_not_stay_a_zombie() {
        let state = |pid: u32| String::from_utf8_lossy(&nightshift_core::proc::cmd("ps").args(["-o", "stat=", "-p", &pid.to_string()]).output().unwrap().stdout).trim().to_string();
        let wait_gone = |pid: u32| (0..50).any(|_| { std::thread::sleep(std::time::Duration::from_millis(40)); !state(pid).starts_with('Z') });
        // the cause: a child that is dropped without `wait` stays a zombie after it ended
        let lost = nightshift_core::proc::cmd("true").spawn().unwrap();
        let lost_pid = lost.id();
        drop(lost);
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(state(lost_pid).starts_with('Z'), "the reproduction: {:?}", state(lost_pid));
        // the helper reaps it
        let pid = spawn_reaped(&mut nightshift_core::proc::cmd("true")).unwrap();
        assert!(wait_gone(pid), "still {:?}", state(pid));
        assert!(spawn_reaped(&mut nightshift_core::proc::cmd("/nonexistent/program")).is_err());
    }

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ns-app-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        nightshift_core::proc::canon(&d).unwrap()
    }

    #[test]
    fn open_allows_web_links_and_listed_file_types_inside_projects_only() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch("open");
        let outside = scratch("open-outside");
        for f in ["notes.txt", "page.html", "pic.svg", "data.xml", "evil.js", "evil.py", "run.sh", "tool", "tool.txt", "a.terminal", "a.webloc", "a.fileloc", "a.inetloc", "noext"] {
            std::fs::write(root.join(f), "x").unwrap();
        }
        std::fs::set_permissions(root.join("tool.txt"), std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::create_dir(root.join("Thing.app")).unwrap();
        std::fs::write(outside.join("notes.txt"), "x").unwrap();
        std::os::unix::fs::symlink(outside.join("notes.txt"), root.join("link.txt")).unwrap();
        let roots = vec![root.clone()];
        let p = |n: &str| root.join(n).to_string_lossy().to_string();

        assert_eq!(check_openable("https://github.com/o/r", &roots), Ok(Target::Url("https://github.com/o/r".into())));
        assert_eq!(check_openable(&p("notes.txt"), &roots), Ok(Target::File(root.join("notes.txt"))));
        // what a browser would run as a page is only shown in the file manager
        // and a script a double click would run (.js on Windows, .py with Python Launcher)
        for n in ["page.html", "pic.svg", "data.xml", "evil.js", "evil.py"] {
            assert_eq!(check_openable(&p(n), &roots), Ok(Target::Reveal(root.join(n))), "{n}");
        }
        assert_eq!(check_openable(&root.to_string_lossy(), &roots), Ok(Target::Dir(root.clone())));
        // a folder named like a bundle is only revealed, not launched
        assert_eq!(check_openable(&p("Thing.app"), &roots), Ok(Target::Dir(root.join("Thing.app"))));
        for bad in [p("run.sh"), p("tool"), p("tool.txt"), p("a.terminal"), p("a.webloc"), p("a.fileloc"), p("a.inetloc"), p("noext"),
                    p("link.txt"), outside.join("notes.txt").to_string_lossy().to_string(),
                    "/".into(), "file:///etc/passwd".into(), "javascript:alert(1)".into(), "http://evil.example/".into(),
                    "https://a.com/\",/select,C:\\x.js".into(), "https://a.com/ b".into(), "-a Calculator".into(), p("missing")] {
            assert!(check_openable(&bad, &roots).is_err(), "{bad}");
        }
    }

    #[test]
    fn terminal_and_editor_take_only_existing_folders() {
        let root = scratch("dir");
        std::fs::write(root.join("a.txt"), "x").unwrap();
        let roots = vec![root.clone()];
        assert_eq!(check_dir(&root.to_string_lossy(), &roots), Ok(root.clone()));
        assert!(check_dir(&root.join("a.txt").to_string_lossy(), &roots).is_err());
        assert!(check_dir("https://example.com", &roots).is_err());
    }

}
