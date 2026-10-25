//! v6 (G82, G85): what an env has besides the model - skills, commands, MCP servers, mods - for the menu "/" of the
//! chat. Read only from the places in spec v6, table «Среды для «/» и «В терминале»»; nothing is run, nothing is
//! written. Names and one-line descriptions only: at most `MAX_ITEMS` per section, `MAX_READ` bytes of a file.
//! Places checked on this machine (claude 2.1.309, qwen 0.26.0, opencode 1.19.4) are marked so; the rest are from
//! the vendor's docs.
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Items of one section the menu gets at most.
pub const MAX_ITEMS: usize = 300;
/// Bytes of one file read for its description (frontmatter or the first lines).
const MAX_READ: usize = 8 * 1024;
/// Bytes of one config file read (MCP lists, plugin lists).
const MAX_CONF: u64 = 4 * 1024 * 1024;

/// One thing of the env: `name` is what follows "/" (a skill or command) or the server's / mod's name.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct InvItem {
    pub name: String,
    pub description: String,
    /// `global` (the person's folder), `project`, `plugin:<name>`, `extension:<name>`.
    pub source: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Inventory {
    pub skills: Vec<InvItem>,
    pub commands: Vec<InvItem>,
    pub mcp: Vec<InvItem>,
    /// Claude Code mods (`CLAUDE_CODE_PLUGIN_DIRS`), Gemini / Qwen extensions, opencode plugins: they draw in the CLI.
    pub mods: Vec<InvItem>,
}

/// The person's home folder; None when `HOME` is not set.
pub fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from)
}

/// The inventory of `env` (an adapter id) for the project in `project`, with the person's folder `home`.
pub fn scan(env: &str, home: &Path, project: &Path) -> Inventory {
    let mut inv = match env {
        "claude" => claude(home, project),
        "codex" => codex(home, project),
        "gemini" => gemini_like(home, project, ".gemini", "gemini-extension.json"),
        "qwen" => gemini_like(home, project, ".qwen", "qwen-extension.json"),
        "opencode" => opencode(home, project),
        "cursor" => cursor(home, project),
        _ => Inventory::default(),
    };
    for v in [&mut inv.skills, &mut inv.commands, &mut inv.mcp, &mut inv.mods] {
        tidy(v);
    }
    inv
}

/// Sorted by name, the first of a name kept (project before global is the order the scanners add them in), capped.
fn tidy(v: &mut Vec<InvItem>) {
    let mut seen = std::collections::HashSet::new();
    v.retain(|i| !i.name.is_empty() && seen.insert(i.name.clone()));
    v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    v.truncate(MAX_ITEMS);
}

// ---------------- the envs ----------------

/// Claude Code (checked on 2.1.309): skills `<dir>/skills/<name>/SKILL.md`, commands `<dir>/commands/**/*.md`
/// (`a/b.md` = `/a:b`), the same of each enabled plugin (`plugins/installed_plugins.json` + `enabledPlugins` of
/// settings.json) as `plugin:name`; MCP `mcpServers` of `~/.claude.json` (all projects and this one) and `.mcp.json`;
/// mods = the folders of `env.CLAUDE_CODE_PLUGIN_DIRS` of settings.json.
fn claude(home: &Path, project: &Path) -> Inventory {
    let dir = home.join(".claude");
    let pdir = project.join(".claude");
    let mut inv = Inventory::default();
    skills_in(&pdir.join("skills"), "project", "", &mut inv.skills);
    skills_in(&dir.join("skills"), "global", "", &mut inv.skills);
    commands_in(&pdir.join("commands"), "project", "", &["md"], &mut inv.commands);
    commands_in(&dir.join("commands"), "global", "", &["md"], &mut inv.commands);
    let settings = read_json(&dir.join("settings.json"));
    let enabled = &settings["enabledPlugins"];
    let installed = read_json(&dir.join("plugins").join("installed_plugins.json"));
    if let Some(map) = installed["plugins"].as_object() {
        for (key, list) in map {
            if enabled[key.as_str()] != Value::Bool(true) {
                continue;
            }
            let name = key.split('@').next().unwrap_or(key);
            let Some(path) = list.as_array().and_then(|l| l.iter().find_map(|x| x["installPath"].as_str())) else { continue };
            let root = PathBuf::from(path);
            let src = format!("plugin:{name}");
            skills_in(&root.join("skills"), &src, &format!("{name}:"), &mut inv.skills);
            commands_in(&root.join("commands"), &src, &format!("{name}:"), &["md"], &mut inv.commands);
        }
    }
    let user = read_json(&home.join(".claude.json"));
    servers(&user["mcpServers"], "global", &mut inv.mcp);
    servers(&user["projects"][project.to_string_lossy().as_ref()]["mcpServers"], "project", &mut inv.mcp);
    servers(&read_json(&project.join(".mcp.json"))["mcpServers"], "project", &mut inv.mcp);
    if let Some(dirs) = settings["env"]["CLAUDE_CODE_PLUGIN_DIRS"].as_str() {
        for d in dirs.split(':').map(str::trim).filter(|d| !d.is_empty()) {
            let p = match d.strip_prefix("~/") {
                Some(rest) => home.join(rest),
                None => PathBuf::from(d),
            };
            let Some(name) = p.file_name().map(|n| n.to_string_lossy().into_owned()) else { continue };
            let manifest = read_json(&p.join(".claude-plugin").join("plugin.json"));
            inv.mods.push(InvItem { name, description: line_of(manifest["description"].as_str().unwrap_or("")), source: "global".into() });
        }
    }
    inv
}

/// Codex (docs): skills `~/.agents/skills/<name>/SKILL.md` and the project's `.agents/skills`; custom prompts
/// `~/.codex/prompts/*.md`, called as `/prompts:<name>`; MCP tables `[mcp_servers.<name>]` of `~/.codex/config.toml`.
fn codex(home: &Path, project: &Path) -> Inventory {
    let mut inv = Inventory::default();
    skills_in(&project.join(".agents").join("skills"), "project", "", &mut inv.skills);
    skills_in(&home.join(".agents").join("skills"), "global", "", &mut inv.skills);
    commands_in(&home.join(".codex").join("prompts"), "global", "prompts:", &["md"], &mut inv.commands);
    let toml = read_text(&home.join(".codex").join("config.toml"), MAX_CONF as usize);
    for line in toml.lines() {
        let l = line.trim();
        let Some(rest) = l.strip_prefix("[mcp_servers.").and_then(|r| r.strip_suffix(']')) else { continue };
        let name = rest.trim_matches('"');
        if !name.is_empty() && !name.contains('.') {
            inv.mcp.push(InvItem { name: name.to_string(), description: String::new(), source: "global".into() });
        }
    }
    inv
}

/// Gemini CLI (docs) and Qwen Code (checked on 0.26.0: `~/.qwen/skills`, `~/.qwen/extensions`, `settings.json`):
/// skills `<dir>/skills/<name>/SKILL.md`; commands `<dir>/commands/**/*.toml|md` (`a/b.toml` = `/a:b`) of the person
/// and of the project; MCP `mcpServers` of `settings.json` (both); extensions `~/<dir>/extensions/<name>/`.
fn gemini_like(home: &Path, project: &Path, dir: &str, manifest: &str) -> Inventory {
    let mut inv = Inventory::default();
    let (g, p) = (home.join(dir), project.join(dir));
    skills_in(&p.join("skills"), "project", "", &mut inv.skills);
    skills_in(&g.join("skills"), "global", "", &mut inv.skills);
    commands_in(&p.join("commands"), "project", "", &["toml", "md"], &mut inv.commands);
    commands_in(&g.join("commands"), "global", "", &["toml", "md"], &mut inv.commands);
    servers(&read_json(&p.join("settings.json"))["mcpServers"], "project", &mut inv.mcp);
    servers(&read_json(&g.join("settings.json"))["mcpServers"], "global", &mut inv.mcp);
    for (name, path) in dirs_in(&g.join("extensions")) {
        let m = read_json(&path.join(manifest));
        let m = if m.is_null() { read_json(&path.join("gemini-extension.json")) } else { m };
        inv.mods.push(InvItem { name, description: line_of(m["description"].as_str().unwrap_or("")), source: "extension".into() });
    }
    inv
}

/// opencode (checked on 1.19.4: `~/.config/opencode/{commands,skills,plugins}`, `opencode.json`): commands
/// `command/` or `commands/*.md` of `~/.config/opencode` and of the project's `.opencode`; skills `skills/<name>/SKILL.md`;
/// MCP key `mcp` of `opencode.json` (the person's and the project root's); plugins = files of `plugin(s)/` and the
/// `plugin` list of the config.
fn opencode(home: &Path, project: &Path) -> Inventory {
    let mut inv = Inventory::default();
    let (g, p) = (home.join(".config").join("opencode"), project.join(".opencode"));
    for (d, src) in [(&p, "project"), (&g, "global")] {
        for sub in ["command", "commands"] {
            commands_in(&d.join(sub), src, "", &["md"], &mut inv.commands);
        }
        skills_in(&d.join("skills"), src, "", &mut inv.skills);
        for sub in ["plugin", "plugins"] {
            for f in files_in(&d.join(sub), &["ts", "js", "mjs"]) {
                let name = f.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                inv.mods.push(InvItem { name, description: String::new(), source: src.into() });
            }
        }
    }
    for (conf, src) in [(project.join("opencode.json"), "project"), (g.join("opencode.json"), "global")] {
        let c = read_json(&conf);
        servers(&c["mcp"], src, &mut inv.mcp);
        for name in c["plugin"].as_array().into_iter().flatten().filter_map(|x| x.as_str()) {
            inv.mods.push(InvItem { name: line_of(name), description: String::new(), source: src.into() });
        }
    }
    inv
}

/// Cursor (docs): rules `.cursor/rules/*.mdc|md` of the project (the table's «скиллы / команды»); MCP `mcpServers` of
/// `~/.cursor/mcp.json` and the project's `.cursor/mcp.json`.
fn cursor(home: &Path, project: &Path) -> Inventory {
    let mut inv = Inventory::default();
    commands_in(&project.join(".cursor").join("rules"), "project", "", &["mdc", "md"], &mut inv.skills);
    servers(&read_json(&project.join(".cursor").join("mcp.json"))["mcpServers"], "project", &mut inv.mcp);
    servers(&read_json(&home.join(".cursor").join("mcp.json"))["mcpServers"], "global", &mut inv.mcp);
    inv
}

// ---------------- reading ----------------

/// Folders of `dir` (a symlinked skill folder counts: the person links skills in), by name.
fn dirs_in(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<(String, PathBuf)> = rd
        .filter_map(|e| e.ok())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
        .filter(|(n, p)| !n.starts_with('.') && p.is_dir())
        .take(MAX_ITEMS * 2)
        .collect();
    out.sort();
    out
}

/// Files of `dir` with one of `exts`, not deeper.
fn files_in(dir: &Path, exts: &[&str]) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| exts.iter().any(|e| x == *e)))
        .take(MAX_ITEMS * 2)
        .collect();
    out.sort();
    out
}

/// `<dir>/<name>/SKILL.md` -> a skill `prefix + name` with the `description` of its frontmatter.
fn skills_in(dir: &Path, source: &str, prefix: &str, out: &mut Vec<InvItem>) {
    for (name, path) in dirs_in(dir) {
        let file = path.join("SKILL.md");
        if !file.is_file() {
            continue;
        }
        let text = read_text(&file, MAX_READ);
        let name = front(&text, "name").filter(|n| is_name(n)).unwrap_or(name);
        out.push(InvItem { name: format!("{prefix}{name}"), description: describe(&text), source: source.into() });
    }
}

/// Command files of `dir` and one level of folders below it (`a/b.md` = `a:b`).
fn commands_in(dir: &Path, source: &str, prefix: &str, exts: &[&str], out: &mut Vec<InvItem>) {
    let mut add = |f: &Path, ns: &str| {
        let Some(stem) = f.file_stem().map(|s| s.to_string_lossy().into_owned()) else { return };
        let text = read_text(f, MAX_READ);
        out.push(InvItem { name: format!("{prefix}{ns}{stem}"), description: describe(&text), source: source.into() });
    };
    for f in files_in(dir, exts) {
        add(&f, "");
    }
    for (sub, path) in dirs_in(dir) {
        for f in files_in(&path, exts) {
            add(&f, &format!("{sub}:"));
        }
    }
}

/// The keys of an MCP map (`{"name": {...}}`).
fn servers(map: &Value, source: &str, out: &mut Vec<InvItem>) {
    for (name, v) in map.as_object().into_iter().flatten() {
        let what = v["command"].as_str().or_else(|| v["url"].as_str()).or_else(|| v["type"].as_str()).unwrap_or("");
        // the command's program name only: arguments and URLs can hold tokens
        let what = what.rsplit('/').next().unwrap_or("").split(['?', ' ']).next().unwrap_or("");
        out.push(InvItem { name: line_of(name), description: line_of(what), source: source.into() });
    }
}

fn read_text(f: &Path, max: usize) -> String {
    use std::io::Read;
    let Ok(file) = std::fs::File::open(f) else { return String::new() };
    let mut buf = Vec::new();
    let _ = file.take(max as u64).read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

fn read_json(f: &Path) -> Value {
    if std::fs::metadata(f).map_or(true, |m| m.len() > MAX_CONF) {
        return Value::Null;
    }
    std::fs::read(f).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null)
}

/// A value of the YAML frontmatter (`---` ... `---`) or of a TOML line (`key = "..."`), one line, quotes off.
fn front(text: &str, key: &str) -> Option<String> {
    let mut lines = text.lines();
    let first = lines.next()?.trim();
    let yaml = first == "---";
    let rest: Box<dyn Iterator<Item = &str>> = if yaml { Box::new(lines.take_while(|l| l.trim() != "---")) } else { Box::new(text.lines()) };
    for l in rest {
        let l = l.trim();
        let Some(v) = l.strip_prefix(key).map(str::trim_start) else { continue };
        let v = v.strip_prefix(':').or_else(|| if yaml { None } else { v.strip_prefix('=') });
        let Some(v) = v.map(str::trim) else { continue };
        let v = v.trim_matches(|c| c == '"' || c == '\'');
        if !v.is_empty() && v != "|" && v != ">" {
            return Some(line_of(v));
        }
    }
    None
}

/// The description: the frontmatter's `description`, else the first line of text that is not a heading mark or fence.
fn describe(text: &str) -> String {
    if let Some(d) = front(text, "description") {
        return d;
    }
    let body = if text.trim_start().starts_with("---") { text.splitn(3, "---").nth(2).unwrap_or("") } else { text };
    body.lines().map(|l| l.trim().trim_start_matches('#').trim()).find(|l| !l.is_empty() && !l.starts_with("```")).map(line_of).unwrap_or_default()
}

/// One line of at most 200 characters, control characters out.
fn line_of(s: &str) -> String {
    let one: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let one = one.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > 200 {
        one.chars().take(199).collect::<String>() + "…"
    } else {
        one
    }
}

/// A name that can follow "/" in the field: letters, digits, `-`, `_`, `.`, `:`.
fn is_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 100 && s.chars().all(|c| c.is_alphanumeric() || "-_.:".contains(c))
}
