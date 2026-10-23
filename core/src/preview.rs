//! F15: the app preview and "Проверить как пользователь". Only `http://localhost` / `http://127.0.0.1` pages are
//! ever opened, shot or given the "Указать элемент" script; the check is here, so the window and the engine share it.
use serde::Serialize;
use std::path::{Path, PathBuf};

/// The error key of an address that is not the local machine.
pub const NOT_LOCAL: &str = "not_local_url";
/// The error key of a machine without node + playwright + a Chromium.
pub const NO_TOOL: &str = "no_capture_tool";

/// `s` as a normalized `http://localhost[:port]/...` or `http://127.0.0.1[:port]/...`, or `not_local_url`.
/// An exact host match: no user info (`localhost@evil.com`), no look-alike (`localhost.evil.com`), http only.
pub fn local_url(s: &str) -> Result<String, &'static str> {
    let s = s.trim();
    if s.len() > 2048 || s.chars().any(|c| c.is_control() || c.is_whitespace() || c == '\\') {
        return Err(NOT_LOCAL);
    }
    let rest = s.get(..7).filter(|p| p.eq_ignore_ascii_case("http://")).map(|_| &s[7..]).ok_or(NOT_LOCAL)?;
    let (auth, tail) = rest.split_at(rest.find(['/', '?', '#']).unwrap_or(rest.len()));
    let (host, port) = match auth.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (auth, None),
    };
    let host = host.to_ascii_lowercase();
    if host != "localhost" && host != "127.0.0.1" {
        return Err(NOT_LOCAL);
    }
    let port = match port {
        None => String::new(),
        Some(p) if !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()) && p.parse::<u32>().is_ok_and(|n| (1..=65535).contains(&n)) => format!(":{p}"),
        Some(_) => return Err(NOT_LOCAL),
    };
    let tail = if tail.starts_with('/') { tail.to_string() } else { format!("/{tail}") };
    Ok(format!("http://{host}{port}{tail}"))
}

/// (host, port) of an address `local_url` gave.
pub fn host_port(url: &str) -> (String, u16) {
    let auth = url.trim_start_matches("http://").split(['/', '?', '#']).next().unwrap_or("");
    match auth.split_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().unwrap_or(80)),
        None => (auth.to_string(), 80),
    }
}

/// What "Указать элемент" sends back from the preview page (data of the page: capped, control characters gone).
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Pick {
    pub selector: String,
    pub text: String,
    pub w: u32,
    pub h: u32,
}

/// A pick from the JSON the page script wrote; None when it is not one.
pub fn parse_pick(json: &str) -> Option<Pick> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let text = |k: &str, max: usize| -> Option<String> {
        let s = v.get(k)?.as_str()?;
        Some(s.split(|c: char| c.is_whitespace() || c.is_control()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ").chars().take(max).collect())
    };
    let num = |k: &str| v.get(k).and_then(|x| x.as_f64()).map_or(0, |n| n.clamp(0.0, 100_000.0).round() as u32);
    let selector = text("selector", 300).filter(|s| !s.is_empty())?;
    Some(Pick { selector, text: text("text", 160).unwrap_or_default(), w: num("w"), h: num("h") })
}

/// Takes the screenshots of the proof step. Bound by the app; tests put a fake in.
pub trait Capture: Send + Sync {
    /// Ok when the tool is there; Err(`no_capture_tool`) otherwise. `cwd` is the task's copy (its node_modules count).
    fn available(&self, cwd: &Path) -> Result<(), String>;
    /// Screenshots of `url` written into the empty folder `out`; the files, in order.
    fn shoot(&self, url: &str, cwd: &Path, out: &Path) -> Result<Vec<PathBuf>, String>;
}

/// `node` + `playwright-core` / `playwright` (the copy's own, `NS_PLAYWRIGHT`, or the global npm folder) + Chrome
/// (`NS_CHROME`, the installed Google Chrome, or Playwright's own Chromium).
pub struct NodeCapture {
    pub node: PathBuf,
}

impl Default for NodeCapture {
    fn default() -> Self {
        NodeCapture { node: "node".into() }
    }
}

/// The capture script: `check` only launches the browser; `shoot <url> <out>` writes 1.png (the page), 2.png (after a
/// click on the main button, when there is one that does not look destructive) and 3.png (390 wide). Exit 3 = no tool.
const SCRIPT: &str = r#"
setTimeout(() => process.exit(4), 120000).unref();
const [mode, url, out] = process.argv.slice(1);
(async () => {
  let pw = null;
  for (const m of [process.env.NS_PLAYWRIGHT, 'playwright-core', 'playwright'].filter(Boolean)) { try { pw = require(m); break; } catch (e) {} }
  if (!pw || !pw.chromium) process.exit(3);
  let b = null;
  for (const o of [process.env.NS_CHROME && { executablePath: process.env.NS_CHROME }, { channel: 'chrome' }, {}].filter(Boolean)) { try { b = await pw.chromium.launch(o); break; } catch (e) {} }
  if (!b) process.exit(3);
  if (mode !== 'shoot') { await b.close(); process.exit(0); }
  const p = await b.newPage({ viewport: { width: 1280, height: 800 } });
  const open = async () => { await p.goto(url, { waitUntil: 'load', timeout: 30000 }); await p.waitForTimeout(800); };
  await open();
  await p.screenshot({ path: out + '/1.png' });
  const btn = p.locator('main button:visible, button:visible, [role=button]:visible, input[type=submit]:visible')
    .filter({ hasNotText: /delete|remove|clear|reset|log ?out|sign ?out|удал|очист|сброс|выйти|выход/i }).first();
  if (await btn.count()) {
    try { await btn.click({ timeout: 3000 }); await p.waitForTimeout(800); await p.screenshot({ path: out + '/2.png' }); } catch (e) {}
  }
  await p.setViewportSize({ width: 390, height: 844 });
  await open();
  await p.screenshot({ path: out + '/3.png' });
  await b.close();
})().catch((e) => { console.error(String(e && e.message || e).split('\n')[0]); process.exit(1); });
"#;

impl NodeCapture {
    fn run(&self, cwd: &Path, args: &[&str]) -> Result<(), String> {
        let mut c = std::process::Command::new(&self.node);
        c.arg("-e").arg(SCRIPT).args(args).current_dir(cwd).stdin(std::process::Stdio::null());
        // a global `npm i -g playwright` is not on node's own search path
        if std::env::var_os("NODE_PATH").is_none() {
            if let Some(root) = std::process::Command::new("npm").args(["root", "-g"]).output().ok().filter(|o| o.status.success()) {
                c.env("NODE_PATH", String::from_utf8_lossy(&root.stdout).trim());
            }
        }
        let o = c.output().map_err(|_| NO_TOOL.to_string())?;
        match o.status.code() {
            Some(0) => Ok(()),
            Some(3) => Err(NO_TOOL.into()),
            _ => Err(format!("capture_failed: {}", String::from_utf8_lossy(&o.stderr).trim().chars().take(200).collect::<String>())),
        }
    }
}

impl Capture for NodeCapture {
    fn available(&self, cwd: &Path) -> Result<(), String> {
        self.run(cwd, &["check"])
    }
    fn shoot(&self, url: &str, cwd: &Path, out: &Path) -> Result<Vec<PathBuf>, String> {
        let url = local_url(url)?;
        self.run(cwd, &["shoot", &url, &out.to_string_lossy()])?;
        Ok(["1.png", "2.png", "3.png"].iter().map(|f| out.join(f)).filter(|f| f.is_file()).collect())
    }
}
