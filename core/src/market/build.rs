//! From a catalog item to a kit in memory: download (one archive, ≤ 20 MB), unpack with the limits (≤ 100 MB, no
//! `..`, absolute paths or symlinks), pick the skill / mod folder, or turn `server.json` into the environment's MCP
//! config (research §2.5-2.6) with variables by name only.
use super::sources::{enc, ok_repo, registry_item};
use super::{Item, ItemDetail, Kind, Market, MarketError, Origin, Result, VarInfo, MAX_INDEX, MAX_ITEM, MAX_UNPACKED};
use crate::kits::{env_name_allowed, read_tree, safe_rel, scratch_dir, with_clone, EnvVarNeed, KitError, KitSource, ScriptText};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::io::Read;

pub(super) struct Built {
    pub files: BTreeMap<String, Vec<u8>>,
    pub source: KitSource,
    pub commit: Option<String>,
    pub env_vars: Vec<EnvVarNeed>,
    pub runs: Option<String>,
    pub needs: Vec<String>,
    pub warnings: Vec<String>,
    pub texts: Vec<ScriptText>,
    pub version: Option<String>,
}

const MAX_ENTRIES: usize = 20_000;

type Files = BTreeMap<String, Vec<u8>>;
/// (type `streamable-http` | `sse`, url, headers)
type Remote = (String, String, Vec<(String, String)>);

/// Unpack a zip in memory. Every name is checked (absolute, `..`, backslash → refused); only entries `keep` wants are
/// read, and of those a symlink is refused and the bytes are counted against `MAX_UNPACKED` while reading (declared
/// sizes are not trusted). One common top folder (GitHub archives: `repo-<ref>/`) is cut off. The zip comment of a
/// GitHub archive is the commit.
pub(super) fn unzip(bytes: &[u8], keep: &dyn Fn(&str) -> bool) -> Result<(Files, Option<String>)> {
    let bad = |e: zip::result::ZipError| MarketError::Kit(KitError::BadZip(e.to_string()));
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(bad)?;
    if z.len() > MAX_ENTRIES {
        return Err(KitError::TooManyFiles(z.len()).into());
    }
    let comment = String::from_utf8_lossy(z.comment()).trim().to_string();
    let mut names = vec![];
    for i in 0..z.len() {
        let e = z.by_index_raw(i).map_err(bad)?;
        let raw = e.name().map_err(bad)?.to_string();
        if raw.starts_with('/') || raw.contains('\\') || raw.split('/').any(|c| c == "..") || e.enclosed_name().is_none() {
            return Err(KitError::UnsafePath(raw.chars().take(200).collect()).into());
        }
        names.push((i, raw.trim_end_matches('/').to_string(), e.is_dir()));
    }
    let top = names.first().map(|(_, n, _)| n.split('/').next().unwrap_or("").to_string()).unwrap_or_default();
    let strip = !top.is_empty() && names.iter().all(|(_, n, d)| n == &top && *d || n.starts_with(&format!("{top}/")));
    let mut out = BTreeMap::new();
    let mut total = 0u64;
    for (i, n, dir) in names {
        let rel = if strip { n.strip_prefix(&top).unwrap_or(&n).trim_start_matches('/').to_string() } else { n };
        if dir || rel.is_empty() || !keep(&rel) {
            continue;
        }
        let mut e = z.by_index(i).map_err(bad)?;
        if e.is_symlink() {
            return Err(KitError::Symlink(rel).into());
        }
        let rel = safe_rel(&rel)?;
        let mut buf = vec![];
        (&mut e).take(MAX_UNPACKED - total + 1).read_to_end(&mut buf).map_err(|e| MarketError::Kit(KitError::BadZip(e.to_string())))?;
        total += buf.len() as u64;
        if total > MAX_UNPACKED {
            return Err(KitError::KitTooLarge { size: total }.into());
        }
        out.insert(rel, buf);
    }
    let commit = (comment.len() == 40 && comment.chars().all(|c| c.is_ascii_hexdigit())).then_some(comment);
    Ok((out, commit))
}

fn ok_ref(r: &str) -> bool {
    !r.is_empty() && r.len() <= 200 && !r.contains("..") && r.chars().all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c))
}

fn is_sha(r: &str) -> bool {
    r.len() == 40 && r.chars().all(|c| c.is_ascii_hexdigit())
}

/// Files under `root` (relative to it).
fn under(tree: &BTreeMap<String, Vec<u8>>, root: &str) -> BTreeMap<String, Vec<u8>> {
    if root.is_empty() {
        return tree.clone();
    }
    let p = format!("{root}/");
    tree.iter().filter_map(|(k, v)| k.strip_prefix(&p).map(|r| (r.to_string(), v.clone()))).collect()
}

/// `key: value` of a SKILL.md frontmatter.
fn front(md: &[u8], key: &str) -> Option<String> {
    let s = String::from_utf8_lossy(md);
    let rest = s.trim_start_matches('\u{feff}').strip_prefix("---")?;
    let fm = &rest[..rest.find("\n---")?];
    fm.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        (k.trim() == key).then(|| v.trim().trim_matches(['"', '\'']).chars().take(1000).collect())
    })
}

fn folder_name(n: &str) -> Result<String> {
    let n: String = n.trim().chars().take(64).collect();
    if n.is_empty() || n.starts_with('.') || !n.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) {
        return Err(KitError::BadName(n).into());
    }
    Ok(n)
}

/// The files of a git-like origin: (repo files, commit, folder inside, kit source for the snapshot).
/// `zip` keeps a GitHub archive between two reads of the same item (one download).
fn tree_of(m: &Market, origin: &Origin, keep: &dyn Fn(&str) -> bool, zip: &mut Option<Vec<u8>>) -> Result<(Files, Option<String>, String, KitSource)> {
    match origin {
        Origin::Github { repo, path, git_ref, .. } => {
            let rf = git_ref.clone().unwrap_or_else(|| "HEAD".into());
            if !ok_repo(repo) || !ok_ref(&rf) {
                return Err(MarketError::BadSource(repo.clone()));
            }
            let root = if path.is_empty() { String::new() } else { safe_rel(path)? };
            if zip.is_none() {
                *zip = Some(m.get(&format!("https://codeload.github.com/{repo}/zip/{rf}"), MAX_ITEM)?);
            }
            let (files, commit) = unzip(zip.as_deref().unwrap_or_default(), keep)?;
            Ok((files, commit.or_else(|| is_sha(&rf).then(|| rf.clone())), root, KitSource::Repo(format!("https://github.com/{repo}"))))
        }
        Origin::Git { url, path } => {
            let root = if path.is_empty() { String::new() } else { safe_rel(path)? };
            let (files, commit) = with_clone(url, &scratch_dir(), |d, c| Ok((read_tree(d)?, c)))?;
            Ok((files, commit, root, KitSource::Repo(url.clone())))
        }
        Origin::Folder { path } => Ok((read_tree(path)?, None, String::new(), KitSource::Folder(path.clone()))),
        Origin::Zip { path } => {
            let size = std::fs::metadata(path).map_err(|_| KitError::NotFound(path.display().to_string()))?.len();
            if size > MAX_ITEM as u64 {
                return Err(KitError::KitTooLarge { size }.into());
            }
            let b = std::fs::read(path).map_err(|e| MarketError::Io(e.to_string()))?;
            let (files, _) = unzip(&b, keep)?;
            Ok((files, None, String::new(), KitSource::Zip(path.clone())))
        }
        Origin::Unsupported { why } => Err(MarketError::Unsupported(why.clone())),
        _ => Err(MarketError::Unsupported("origin".into())),
    }
}

/// Kit folder for skills of `env` (research §1.4).
fn skills_dir(env: &str) -> Result<&'static str> {
    match env {
        "claude" | "opencode" => Ok("skills"),
        "codex" | "gemini" => Ok("catalog-skills"),
        _ => Err(MarketError::EnvUnsupported { kind: Kind::Skill, env: env.into() }),
    }
}

pub(super) fn build(m: &Market, item: &Item, env: &str) -> Result<Built> {
    match item.kind {
        Kind::Mcp => mcp(m, item, env),
        Kind::Skill | Kind::Mod => files_item(m, item, env),
    }
}

fn is_mod(files: &BTreeMap<String, Vec<u8>>) -> bool {
    files.contains_key(".claude-plugin/plugin.json")
        && files.get("hooks/hooks.json").and_then(|b| serde_json::from_slice::<Value>(b).ok()).is_some_and(|v| v.get("modules").is_some())
}

fn files_item(m: &Market, item: &Item, env: &str) -> Result<Built> {
    if item.kind == Kind::Mod && env != "claude" {
        return Err(MarketError::EnvUnsupported { kind: Kind::Mod, env: env.into() });
    }
    // skills.sh names a skill, not its folder: find it among the SKILL.md files first
    let mut origin = item.origin.clone();
    let mut zip = None;
    if let Origin::Github { path, skill: Some(want), .. } = &mut origin {
        if path.is_empty() {
            let (mds, _, _, _) = tree_of(m, &item.origin, &|r| r == "SKILL.md" || r.ends_with("/SKILL.md"), &mut zip)?;
            let found = mds
                .iter()
                .filter(|(k, b)| {
                    let dir = k.trim_end_matches("SKILL.md").trim_end_matches('/');
                    dir.rsplit('/').next() == Some(want.as_str()) || front(b, "name").as_deref() == Some(want.as_str())
                })
                .map(|(k, _)| k.trim_end_matches("SKILL.md").trim_end_matches('/').to_string())
                .min_by_key(|d| (d.len(), d.clone()))
                .ok_or_else(|| MarketError::NotFound(format!("skill {want}")))?;
            *path = found;
        }
    }
    let root_of = |o: &Origin| match o {
        Origin::Github { path, .. } | Origin::Git { path, .. } => path.clone(),
        _ => String::new(),
    };
    let root = root_of(&origin);
    let pre = if root.is_empty() { String::new() } else { format!("{root}/") };
    let (tree, commit, root, source) = tree_of(m, &origin, &|r| pre.is_empty() || r.starts_with(&pre), &mut zip)?;
    let sub = under(&tree, &root);
    if sub.is_empty() {
        return Err(MarketError::NotFound(if root.is_empty() { item.name.clone() } else { root }));
    }
    let leaf = if root.is_empty() { item.name.clone() } else { root.rsplit('/').next().unwrap_or(&root).to_string() };
    let mut files = BTreeMap::new();
    let mut texts = vec![];
    let mut warnings = vec![];
    if item.kind == Kind::Mod || is_mod(&sub) {
        if env != "claude" {
            return Err(MarketError::EnvUnsupported { kind: Kind::Mod, env: env.into() });
        }
        let pj = sub.get(".claude-plugin/plugin.json").ok_or_else(|| MarketError::Unsupported("not_a_mod".into()))?;
        let name = folder_name(&serde_json::from_slice::<Value>(pj).ok().and_then(|v| v.get("name").and_then(Value::as_str).map(String::from)).unwrap_or(leaf))?;
        for (k, v) in sub.into_iter().filter(|(k, _)| !k.starts_with(".claude-plugin/types/")) {
            files.insert(format!("mods/{name}/{k}"), v);
        }
        warnings.push("runs_code".into());
    } else {
        let dir = skills_dir(env)?;
        // one skill at the root, or a plugin's `skills/<name>/SKILL.md` folders
        let mut skills: Vec<(String, String)> = vec![];
        if let Some(md) = sub.get("SKILL.md") {
            skills.push((folder_name(&front(md, "name").unwrap_or(leaf))?, String::new()));
        } else {
            for k in sub.keys().filter(|k| k.starts_with("skills/") && k.ends_with("/SKILL.md") && k.matches('/').count() == 2) {
                let d = k.trim_end_matches("/SKILL.md").to_string();
                skills.push((folder_name(d.rsplit('/').next().unwrap_or(&d))?, format!("{d}/")));
            }
        }
        if skills.is_empty() {
            return Err(MarketError::Unsupported("nothing_to_install".into()));
        }
        for (name, from) in skills {
            for (k, v) in sub.iter().filter(|(k, _)| k.starts_with(&from)) {
                let rel = &k[from.len()..];
                if rel == "SKILL.md" {
                    texts.push(ScriptText { path: format!("{name}/SKILL.md"), text: String::from_utf8_lossy(v).to_string() });
                }
                files.insert(format!("{dir}/{name}/{rel}"), v.clone());
            }
        }
    }
    Ok(Built { files, source, version: commit.clone(), commit, env_vars: vec![], runs: None, needs: vec![], warnings, texts })
}

// ---------------- MCP ----------------

/// A server as the environment needs it, before it is written in that environment's format.
struct Spec {
    local: Option<(String, Vec<String>)>,
    remote: Option<Remote>,
    /// Variables passed to a local server's process.
    pass: Vec<String>,
    vars: Vec<VarInfo>,
    needs: Vec<String>,
    warnings: Vec<String>,
}

fn var_name(x: &str) -> String {
    x.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect()
}

fn add_var(vars: &mut Vec<VarInfo>, v: VarInfo) {
    if !env_name_allowed(&v.name) {
        return;
    }
    match vars.iter_mut().find(|x| x.name == v.name) {
        Some(x) => {
            x.secret |= v.secret;
            x.required |= v.required;
        }
        None => vars.push(v),
    }
}

/// `{placeholder}` → the environment's reference to a variable (the value is never in the file).
fn subst(s: &str, secret: bool, vars: &mut Vec<VarInfo>, var_ref: &dyn Fn(&str) -> String) -> String {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\{([A-Za-z_][A-Za-z0-9_\-]*)\}").unwrap());
    re.replace_all(s, |c: &regex::Captures| {
        let n = var_name(&c[1]);
        add_var(vars, VarInfo { name: n.clone(), secret, required: true, description: String::new() });
        var_ref(&n)
    })
    .to_string()
}

fn plain(s: &str) -> Option<String> {
    (!s.is_empty() && s.len() <= 300 && !s.chars().any(|c| c.is_control() || c.is_whitespace())).then(|| s.to_string())
}

fn args_of(list: Option<&Value>, vars: &mut Vec<VarInfo>, var_ref: &dyn Fn(&str) -> String) -> Vec<String> {
    let mut out = vec![];
    for a in list.and_then(Value::as_array).into_iter().flatten() {
        let val = a.get("value").and_then(Value::as_str).or_else(|| a.get("default").and_then(Value::as_str));
        let secret = a.get("isSecret").and_then(Value::as_bool).unwrap_or(false);
        if a.get("type").and_then(Value::as_str) == Some("named") {
            if let Some(n) = a.get("name").and_then(Value::as_str) {
                out.push(n.to_string());
            }
        }
        if let Some(v) = val {
            out.push(subst(v, secret, vars, var_ref));
        }
    }
    out
}

fn spec(server: &Value, var_ref: &dyn Fn(&str) -> String) -> Result<Spec> {
    let mut vars = vec![];
    let packages = server.get("packages").and_then(Value::as_array).cloned().unwrap_or_default();
    for p in &packages {
        let transport = p.get("transport").and_then(|t| t.get("type")).and_then(Value::as_str).unwrap_or("stdio");
        let Some(id) = p.get("identifier").and_then(Value::as_str).and_then(plain) else { continue };
        let ver = p.get("version").and_then(Value::as_str).and_then(plain).filter(|v| v != "latest");
        let (cmd, mut args, pkg, need) = match p.get("registryType").and_then(Value::as_str) {
            Some("npm") => ("npx", vec!["-y".to_string()], ver.as_ref().map(|v| format!("{id}@{v}")), "node"),
            Some("pypi") => ("uvx", vec![], ver.as_ref().map(|v| format!("{id}=={v}")), "uv"),
            Some("oci") => ("docker", vec!["run".into(), "-i".into(), "--rm".into()], ver.as_ref().map(|v| format!("{id}:{v}")), "docker"),
            _ => continue,
        };
        if transport != "stdio" {
            continue;
        }
        let mut warnings = vec!["runs_code".to_string()];
        if ver.is_none() {
            warnings.push("unpinned".into()); // research rule 8: shown, never silently `@latest`
        }
        let mut pass = vec![];
        for e in p.get("environmentVariables").and_then(Value::as_array).into_iter().flatten() {
            let Some(n) = e.get("name").and_then(Value::as_str) else { continue };
            if !env_name_allowed(n) {
                continue;
            }
            pass.push(n.to_string());
            add_var(
                &mut vars,
                VarInfo {
                    name: n.into(),
                    secret: e.get("isSecret").and_then(Value::as_bool).unwrap_or(false),
                    required: e.get("isRequired").and_then(Value::as_bool).unwrap_or(false),
                    description: e.get("description").and_then(Value::as_str).unwrap_or("").chars().take(500).collect(),
                },
            );
        }
        if cmd == "docker" {
            for n in &pass {
                args.extend(["-e".to_string(), n.clone()]);
            }
            warnings.push("needs_docker".into());
        }
        args.extend(args_of(p.get("runtimeArguments"), &mut vars, var_ref));
        args.push(pkg.unwrap_or_else(|| id.clone()));
        args.extend(args_of(p.get("packageArguments"), &mut vars, var_ref));
        return Ok(Spec { local: Some((cmd.into(), args)), remote: None, pass, vars, needs: vec![need.into()], warnings });
    }
    for r in server.get("remotes").and_then(Value::as_array).into_iter().flatten() {
        let t = r.get("type").and_then(Value::as_str).unwrap_or("");
        let Some(url) = r.get("url").and_then(Value::as_str).filter(|u| u.starts_with("https://") && plain(u).is_some()) else { continue };
        if !matches!(t, "streamable-http" | "sse") {
            continue;
        }
        let url = subst(url, false, &mut vars, var_ref);
        let mut headers = vec![];
        for h in r.get("headers").and_then(Value::as_array).into_iter().flatten() {
            let (Some(n), Some(v)) = (h.get("name").and_then(Value::as_str), h.get("value").and_then(Value::as_str)) else { continue };
            let secret = h.get("isSecret").and_then(Value::as_bool).unwrap_or(false);
            headers.push((n.to_string(), v.to_string()));
            subst(v, secret, &mut vars, var_ref); // collect the variables (the value itself is rendered per environment)
        }
        return Ok(Spec { local: None, remote: Some((t.into(), url, headers)), pass: vec![], vars, needs: vec![], warnings: vec!["remote".into()] });
    }
    Err(MarketError::Unsupported("no_package".into()))
}

/// A key the config files and TOML headers take as is.
fn server_key(name: &str) -> String {
    let last = name.rsplit('/').next().unwrap_or(name);
    let k: String = last.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).take(64).collect();
    if k.trim_matches('-').is_empty() {
        "server".into()
    } else {
        k
    }
}

fn toml_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// `{VAR}` alone, or `Bearer {VAR}`.
fn header_var(v: &str) -> Option<(bool, String)> {
    let re = Regex::new(r"^(Bearer )?\{([A-Za-z_][A-Za-z0-9_\-]*)\}$").ok()?;
    let c = re.captures(v)?;
    Some((c.get(1).is_some(), var_name(&c[2])))
}

/// (kit file, its bytes) for `env`.
fn render(env: &str, key: &str, server: &Value) -> Result<(String, Vec<u8>, Spec)> {
    let unsupported = || MarketError::EnvUnsupported { kind: Kind::Mcp, env: env.into() };
    let var_ref: Box<dyn Fn(&str) -> String> = match env {
        "claude" | "gemini" | "qwen" => Box::new(|v: &str| format!("${{{v}}}")),
        "opencode" => Box::new(|v: &str| format!("{{env:{v}}}")),
        "codex" => Box::new(|v: &str| format!("\u{0}{v}")), // Codex does not expand variables in args: marked, refused below
        _ => return Err(unsupported()),
    };
    let sp = spec(server, var_ref.as_ref())?;
    let headers_json = |hs: &[(String, String)]| -> Map<String, Value> {
        let mut vars = vec![];
        hs.iter().map(|(n, v)| (n.clone(), Value::String(subst(v, true, &mut vars, var_ref.as_ref())))).collect()
    };
    let env_map = |f: &dyn Fn(&str) -> String| -> Map<String, Value> { sp.pass.iter().map(|n| (n.clone(), Value::String(f(n)))).collect() };
    let cfg = |v: Value, wrap: &str, file: &str| -> Result<(String, Vec<u8>)> {
        let mut s = serde_json::to_string_pretty(&json!({ wrap: { key: v } })).map_err(|e| MarketError::Io(e.to_string()))?;
        s.push('\n');
        Ok((file.into(), s.into_bytes()))
    };
    let (file, bytes) = match (env, &sp.local, &sp.remote) {
        ("claude" | "gemini" | "qwen", Some((cmd, args)), _) => {
            let mut v = json!({ "command": cmd, "args": args });
            if !sp.pass.is_empty() {
                v["env"] = Value::Object(env_map(var_ref.as_ref()));
            }
            cfg(v, "mcpServers", if env == "claude" { ".mcp.json" } else { "settings.json" })?
        }
        ("claude", None, Some((t, url, hs))) => {
            let mut v = json!({ "type": if t == "sse" { "sse" } else { "http" }, "url": url });
            if !hs.is_empty() {
                v["headers"] = Value::Object(headers_json(hs));
            }
            cfg(v, "mcpServers", ".mcp.json")?
        }
        ("gemini" | "qwen", None, Some((t, url, hs))) => {
            // never `trust: true` (research rule 10)
            let mut v = if t == "sse" { json!({ "url": url }) } else { json!({ "httpUrl": url }) };
            if !hs.is_empty() {
                v["headers"] = Value::Object(headers_json(hs));
            }
            cfg(v, "mcpServers", "settings.json")?
        }
        ("opencode", Some((cmd, args)), _) => {
            let mut command = vec![cmd.clone()];
            command.extend(args.iter().cloned());
            let mut v = json!({ "type": "local", "command": command, "enabled": true });
            if !sp.pass.is_empty() {
                v["environment"] = Value::Object(env_map(var_ref.as_ref()));
            }
            cfg(v, "mcp", "opencode.json")?
        }
        ("opencode", None, Some((_, url, hs))) => {
            let mut v = json!({ "type": "remote", "url": url, "enabled": true });
            if !hs.is_empty() {
                v["headers"] = Value::Object(headers_json(hs));
            }
            cfg(v, "mcp", "opencode.json")?
        }
        ("codex", Some((cmd, args)), _) => {
            if args.iter().any(|a| a.contains('\u{0}')) {
                return Err(MarketError::Unsupported("codex_arg_variable".into()));
            }
            let mut t = format!("[mcp_servers.{key}]\ncommand = {}\nargs = [{}]\n", toml_str(cmd), args.iter().map(|a| toml_str(a)).collect::<Vec<_>>().join(", "));
            if !sp.pass.is_empty() {
                t.push_str(&format!("env_vars = [{}]\n", sp.pass.iter().map(|a| toml_str(a)).collect::<Vec<_>>().join(", ")));
            }
            ("config.toml".to_string(), t.into_bytes())
        }
        ("codex", None, Some((_, url, hs))) => {
            if url.contains('\u{0}') {
                return Err(MarketError::Unsupported("codex_url_variable".into()));
            }
            let mut t = format!("[mcp_servers.{key}]\nurl = {}\n", toml_str(url));
            let (mut env_h, mut static_h) = (vec![], vec![]);
            for (n, v) in hs {
                match header_var(v) {
                    Some((true, var)) if n.eq_ignore_ascii_case("authorization") => t.push_str(&format!("bearer_token_env_var = {}\n", toml_str(&var))),
                    Some((false, var)) => env_h.push(format!("{} = {}", toml_str(n), toml_str(&var))),
                    None if !v.contains('{') => static_h.push(format!("{} = {}", toml_str(n), toml_str(v))),
                    _ => return Err(MarketError::Unsupported("header_form".into())),
                }
            }
            if !static_h.is_empty() {
                t.push_str(&format!("http_headers = {{ {} }}\n", static_h.join(", ")));
            }
            if !env_h.is_empty() {
                t.push_str(&format!("env_http_headers = {{ {} }}\n", env_h.join(", ")));
            }
            ("config.toml".to_string(), t.into_bytes())
        }
        _ => return Err(unsupported()),
    };
    Ok((file, bytes, sp))
}

fn smithery_server(m: &Market, name: &str) -> Result<Value> {
    let base = m.sources()?.into_iter().find(|s| s.id == "smithery").map(|s| s.url).unwrap_or_default();
    let (b, _) = m.cached_get(&format!("{}/{}", base.trim_end_matches('/'), enc(name)), MAX_INDEX)?;
    let v: Value = serde_json::from_slice(&b).map_err(|_| MarketError::BadAnswer("smithery".into()))?;
    let url = v.get("deploymentUrl").and_then(Value::as_str).ok_or_else(|| MarketError::Unsupported("not_hosted".into()))?;
    let url = if url.ends_with("/mcp") { url.to_string() } else { format!("{}/mcp", url.trim_end_matches('/')) };
    // hosted Smithery servers want the human's own Smithery key (research §2.3): asked for by name like any secret
    Ok(json!({ "name": name, "remotes": [{ "type": "streamable-http", "url": url,
        "headers": [{ "name": "Authorization", "value": "Bearer {smithery_api_key}", "isSecret": true }] }] }))
}

fn mcp_server(m: &Market, item: &Item) -> Result<Value> {
    match &item.origin {
        Origin::Mcp { server } => Ok(server.clone()),
        Origin::Smithery { name } => smithery_server(m, name),
        _ => Err(MarketError::Unsupported("origin".into())),
    }
}

fn mcp(m: &Market, item: &Item, env: &str) -> Result<Built> {
    let server = mcp_server(m, item)?;
    let name = server.get("name").and_then(Value::as_str).unwrap_or(&item.name).to_string();
    let key = server_key(&name);
    let (file, bytes, sp) = render(env, &key, &server)?;
    let runs = match (&sp.local, &sp.remote) {
        (Some((c, a)), _) => Some(std::iter::once(c.clone()).chain(a.iter().cloned()).collect::<Vec<_>>().join(" ")),
        (_, Some((_, u, _))) => Some(u.clone()),
        _ => None,
    };
    let env_vars = sp.vars.iter().map(|v| EnvVarNeed { name: v.name.clone(), used_by: vec![format!("mcp:{key}")], missing: false }).collect();
    let mut files = BTreeMap::new();
    files.insert(file, bytes);
    Ok(Built {
        files,
        source: KitSource::Repo(item.homepage.clone().unwrap_or_else(|| format!("mcp:{name}"))),
        commit: None,
        env_vars,
        runs,
        needs: sp.needs,
        warnings: sp.warnings,
        texts: vec![],
        version: server.get("version").and_then(Value::as_str).map(String::from),
    })
}

// ---------------- detail and updates ----------------

pub(super) fn detail(m: &Market, mut item: Item) -> Result<ItemDetail> {
    let mut d = ItemDetail { item: item.clone(), readme: None, env_vars: vec![], runs: None, needs: vec![] };
    match (&item.origin, item.kind) {
        (Origin::Github { repo, path, git_ref, skill }, Kind::Skill) => {
            let rf = git_ref.clone().unwrap_or_else(|| "HEAD".into());
            let tries: Vec<String> = match (path.is_empty(), skill) {
                (false, _) => vec![format!("{path}/SKILL.md")],
                (true, Some(s)) => vec![format!("skills/{s}/SKILL.md"), format!("{s}/SKILL.md"), "SKILL.md".into()],
                (true, None) => vec!["SKILL.md".into()],
            };
            if ok_repo(repo) && ok_ref(&rf) {
                for t in tries.iter().filter(|t| safe_rel(t).is_ok()) {
                    if let Ok((b, _)) = m.cached_get(&format!("https://raw.githubusercontent.com/{repo}/{rf}/{t}"), 1024 * 1024) {
                        if item.description.is_empty() {
                            item.description = front(&b, "description").unwrap_or_default();
                        }
                        if item.license.is_none() {
                            item.license = front(&b, "license");
                        }
                        d.readme = Some(String::from_utf8_lossy(&b).to_string());
                        break;
                    }
                }
            }
        }
        (_, Kind::Mcp) => {
            let server = mcp_server(m, &item)?;
            let sp = spec(&server, &|v: &str| format!("${{{v}}}"))?;
            d.runs = match (&sp.local, &sp.remote) {
                (Some((c, a)), _) => Some(std::iter::once(c.clone()).chain(a.iter().cloned()).collect::<Vec<_>>().join(" ")),
                (_, Some((_, u, _))) => Some(u.clone()),
                _ => None,
            };
            d.env_vars = sp.vars;
            d.needs = sp.needs;
        }
        _ => {}
    }
    d.item = item;
    Ok(d)
}

/// The newest version the source knows: an MCP registry's latest version, or the GitHub HEAD commit.
pub(super) fn latest(m: &Market, item: &Item) -> Option<String> {
    match &item.origin {
        Origin::Mcp { server } => {
            let name = server.get("name")?.as_str()?;
            let base = m.sources().ok()?.into_iter().find(|s| s.id == item.source)?.url;
            let (b, _) = m.cached_get(&format!("{}/v0.1/servers/{}/versions/latest", base.trim_end_matches('/'), enc(name)), MAX_INDEX).ok()?;
            let v: Value = serde_json::from_slice(&b).ok()?;
            registry_item(&item.source, &v)?.version
        }
        Origin::Github { repo, .. } if ok_repo(repo) => {
            let (b, _) = m.cached_get(&format!("https://api.github.com/repos/{repo}/commits/HEAD"), MAX_INDEX).ok()?;
            serde_json::from_slice::<Value>(&b).ok()?.get("sha")?.as_str().filter(|s| is_sha(s)).map(String::from)
        }
        _ => None,
    }
}
