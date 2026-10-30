//! `cmd(program)` - `Command::new` for every child the app starts. On Windows a GUI app's console children
//! open a black window each (CREATE_NO_WINDOW stops it), and `Command` finds only `.exe` on PATH, while npm
//! installs CLIs as `.cmd` shims (claude.cmd, codex.cmd): the program is looked up with PATHEXT first.
use std::ffi::OsStr;
use std::process::Command;

pub fn cmd<S: AsRef<OsStr>>(program: S) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut c = Command::new(resolve(program.as_ref()));
        c.creation_flags(CREATE_NO_WINDOW);
        c
    }
    #[cfg(not(windows))]
    Command::new(program)
}

/// `canonicalize` without the `\\?\` prefix Windows adds (git and Node CLIs stumble on it); the same as std elsewhere.
pub fn canon<P: AsRef<std::path::Path>>(p: P) -> std::io::Result<std::path::PathBuf> {
    dunce::canonicalize(p)
}

/// `sh -c <line>`; on Windows `cmd /C <line>`, the line given to cmd exactly as written.
pub fn shell(line: &str) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut c = cmd("cmd");
        c.arg("/C").raw_arg(line);
        c
    }
    #[cfg(not(windows))]
    {
        let mut c = cmd("sh");
        c.arg("-c").arg(line);
        c
    }
}

/// A bare name without an extension -> its file on PATH (with a PATHEXT extension); else as given.
#[cfg(windows)]
fn resolve(program: &OsStr) -> std::ffi::OsString {
    let p = std::path::Path::new(program);
    if p.extension().is_some() || p.components().count() != 1 {
        return program.to_os_string();
    }
    which(&program.to_string_lossy()).map(|f| f.into_os_string()).unwrap_or_else(|| program.to_os_string())
}

/// The file `name` runs from PATH: `<dir>/<name>`; on Windows `<dir>\<name><ext>` for the PATHEXT extensions only
/// (npm puts an sh script `claude` next to `claude.cmd`, and Windows cannot run the script).
pub fn which(name: &str) -> Option<std::path::PathBuf> {
    let exts: Vec<String> = if cfg!(windows) {
        let pe = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        pe.split(';').filter(|e| !e.is_empty()).map(|e| e.to_ascii_lowercase()).collect()
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .filter(|d| !d.as_os_str().is_empty())
        .flat_map(|d| exts.iter().map(move |e| d.join(format!("{name}{e}"))))
        .find(|f| f.is_file())
}

#[cfg(all(test, unix))]
mod tests {
    #[test]
    fn shell_runs_a_line_and_which_finds_a_program_on_path() {
        let out = super::shell("echo a; echo b >&2").output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "a\n");
        assert!(super::which("sh").is_some_and(|p| p.is_absolute()));
        assert!(super::which("ns-no-such-program-here").is_none());
    }
}
