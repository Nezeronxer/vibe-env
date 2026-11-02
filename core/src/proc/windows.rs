//! portable-pty quotes C argv, which is insufficient for cmd/bat. Keep the cmd command line opaque to it.
use std::io;
use std::path::{Path, PathBuf};
use base64::{engine::general_purpose::STANDARD, Engine as _};

pub struct WindowsTerminal {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
}

const PAYLOAD: &str = "NIGHTSHIFT_TERMINAL_LAUNCH";
const RUN: &str = r#"$ErrorActionPreference='Stop'; $launch=ConvertFrom-Json $env:NIGHTSHIFT_TERMINAL_LAUNCH; Remove-Item Env:NIGHTSHIFT_TERMINAL_LAUNCH; try{[Console]::Title=$launch.title}catch{}; if($launch.banner){Write-Host $launch.banner}; if($launch.program){$info=New-Object Diagnostics.ProcessStartInfo; $info.FileName=$launch.program; $info.Arguments=$launch.arguments; $info.UseShellExecute=$false; $child=[Diagnostics.Process]::Start($info); $child.WaitForExit(); if(-not $launch.stay_open){exit $child.ExitCode}}"#;

fn invalid() -> io::Error { io::Error::new(io::ErrorKind::InvalidInput, "invalid Windows terminal argument") }

/// Same batch escaping as Rust std's append_bat_arg/make_bat_command_line (MIT/Apache-2.0).
/// https://github.com/rust-lang/rust/blob/1.99.1/library/std/src/sys/args/windows.rs
fn quote(arg: &str, batch: bool) -> io::Result<String> {
    if arg.contains('\0') || (batch && arg.contains(['\r', '\n'])) { return Err(invalid()); }
    let mut out = String::from("\"");
    let mut slashes = 0;
    for c in arg.chars() {
        if c == '\\' { slashes += 1; } else {
            if c == '"' {
                out.extend(std::iter::repeat_n('\\', slashes));
                out.push(if batch { '"' } else { '\\' });
            } else if batch && c == '%' {
                out.push_str("%%cd:~,");
            }
            slashes = 0;
        }
        out.push(c);
    }
    out.extend(std::iter::repeat_n('\\', slashes));
    out.push('"');
    Ok(out)
}

fn resolved(program: &str) -> PathBuf {
    super::which(program).unwrap_or_else(|| PathBuf::from(program))
}

fn batch(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
}

pub fn batch_terminal(argv: &[String]) -> io::Result<Option<WindowsTerminal>> {
    if argv.first().is_some_and(|p| batch(&resolved(p))) {
        windows_terminal(argv, None, false).map(Some)
    } else { Ok(None) }
}

/// A fixed encoded launcher; data travels in the environment, so long prompts do not inflate the command line.
/// No profiles, shell expansion of data, temporary scripts, or data in the PowerShell source.
pub fn windows_terminal(argv: &[String], banner: Option<&str>, stay_open: bool) -> io::Result<WindowsTerminal> {
    let system = PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into())).join("System32");
    let (program, arguments) = if let Some(first) = argv.first() {
        let p = resolved(first);
        if batch(&p) {
            let path = p.to_string_lossy();
            if path.contains(['"', '\0', '\r', '\n', '%']) || path.ends_with('\\') { return Err(invalid()); }
            let mut line = format!("/e:ON /v:OFF /d /c \"\"{path}\"");
            for arg in &argv[1..] { line.push(' '); line.push_str(&quote(arg, true)?); }
            line.push('"');
            // cmd.exe's own command-line limit, independent of CreateProcessW's larger limit.
            if line.encode_utf16().count() > 8191 { return Err(io::Error::new(io::ErrorKind::InvalidInput, "Windows batch command exceeds 8191 characters")); }
            (system.join("cmd.exe").to_string_lossy().into_owned(), line)
        } else {
            let args = argv[1..].iter().map(|a| quote(a, false)).collect::<io::Result<Vec<_>>>()?.join(" ");
            (p.to_string_lossy().into_owned(), args)
        }
    } else { (String::new(), String::new()) };
    let title = argv.first().and_then(|p| Path::new(p).file_stem()).and_then(|s| s.to_str()).unwrap_or("Nightshift");
    let data = serde_json::json!({"program":program, "arguments":arguments, "banner":banner, "stay_open":stay_open, "title":title}).to_string();
    if data.encode_utf16().count() > 32760 { return Err(invalid()); }
    let encoded = STANDARD.encode(RUN.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>());
    let mut args = vec![system.join("WindowsPowerShell/v1.0/powershell.exe").to_string_lossy().into_owned(), "-NoLogo".into(), "-NoProfile".into()];
    if stay_open { args.push("-NoExit".into()); } else { args.push("-NonInteractive".into()); }
    args.extend(["-EncodedCommand".into(), encoded]);
    Ok(WindowsTerminal { argv: args, env: vec![(PAYLOAD.into(), data)] })
}
