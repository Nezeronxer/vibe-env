//! The catalogs: built-in list, own sources, and how each API is asked and read (research-catalogs.md §1-3, §5.1).
//! Every answer is untrusted data: read into plain fields, a wrong shape is `BadAnswer`, never a panic.
use super::{Api, Item, Kind, Market, MarketError, Origin, Result, Source, MAX_INDEX, PAGE};
use chrono::{DateTime, Local};
use serde_json::Value;
use std::path::Path;

fn src(id: &str, name: &str, kind: Kind, api: Api, url: &str, enabled: bool, note: Option<&str>) -> Source {
    Source { id: id.into(), kind, url: url.into(), enabled, api, name: name.into(), builtin: true, note: note.map(String::from) }
}

/// Checked live without a key on 2026-10-23 (research §5.1), except skillsmp (did not answer from here).
pub(super) fn builtins() -> Vec<Source> {
    vec![
        src("skills.sh", "skills.sh", Kind::Skill, Api::SkillsSh, "https://skills.sh/api/search", true, None),
        src("anthropic-skills", "anthropics/skills", Kind::Skill, Api::Marketplace, "https://raw.githubusercontent.com/anthropics/skills/main/.claude-plugin/marketplace.json", true, None),
        src("claude-plugins-official", "Claude plugins (official)", Kind::Skill, Api::Marketplace, "https://raw.githubusercontent.com/anthropics/claude-plugins-official/main/.claude-plugin/marketplace.json", true, None),
        src("claude-plugins-community", "Claude plugins (community)", Kind::Skill, Api::Marketplace, "https://raw.githubusercontent.com/anthropics/claude-plugins-community/main/.claude-plugin/marketplace.json", true, None),
        src("skillsmp", "SkillsMP", Kind::Skill, Api::SkillsMp, "https://skillsmp.com/api/v1/skills/search", false, Some("fallback")),
        src("mcp-registry", "MCP Registry", Kind::Mcp, Api::McpRegistry, "https://registry.modelcontextprotocol.io", true, None),
        src("github-mcp", "GitHub MCP Registry", Kind::Mcp, Api::McpRegistry, "https://api.mcp.github.com", true, None),
        src("smithery", "Smithery", Kind::Mcp, Api::Smithery, "https://registry.smithery.ai/servers", false, Some("terms_not_checked")),
        src("claude-code-playground", "anthropics/claude-code-playground", Kind::Mod, Api::GithubDir, "https://api.github.com/repos/anthropics/claude-code-playground/contents/claude-code/mods", true, None),
    ]
}

pub(super) fn is_builtin(id: &str) -> bool {
    builtins().iter().any(|b| b.id == id)
}

fn ok_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
}

/// `owner/repo` with GitHub's characters only.
pub(super) fn ok_repo(r: &str) -> bool {
    let mut p = r.split('/');
    let ok = |s: &str| !s.is_empty() && s != "." && s != ".." && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    matches!((p.next(), p.next(), p.next()), (Some(a), Some(b), None) if ok(a) && ok(b))
}

/// An own source: HTTPS address of a catalog, or a GitHub repository (`owner/repo`, `https://github.com/owner/repo`)
/// whose `.claude-plugin/marketplace.json` is read.
pub(super) fn own(mut s: Source) -> Result<Source> {
    let bad = |w: &str| MarketError::BadSource(w.into());
    if !ok_id(&s.id) {
        return Err(bad("id"));
    }
    s.builtin = false;
    let u = s.url.trim().trim_end_matches('/').trim_end_matches(".git").to_string();
    let repo = u.strip_prefix("https://github.com/").unwrap_or(&u);
    s.url = if ok_repo(repo) && (u.starts_with("https://github.com/") || !u.contains("://")) {
        s.api = Api::Marketplace;
        format!("https://raw.githubusercontent.com/{repo}/HEAD/.claude-plugin/marketplace.json")
    } else if u.starts_with("https://") && !u.contains(['\n', '\r', ' ', '\0']) {
        u
    } else {
        return Err(bad("url"));
    };
    if !matches!(s.api, Api::Marketplace | Api::McpRegistry | Api::GithubDir) {
        return Err(bad("api"));
    }
    if s.name.trim().is_empty() {
        s.name = s.id.clone();
    }
    Ok(s)
}

/// Percent-encoding of a query value.
pub(super) fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

fn json(src: &str, b: &[u8]) -> Result<Value> {
    serde_json::from_slice(b).map_err(|_| MarketError::BadAnswer(src.into()))
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(|x| x.chars().filter(|c| !c.is_control() || *c == '\n').take(4000).collect())
}

fn text(v: &Value, k: &str) -> String {
    s(v, k).unwrap_or_default()
}

type Found = (Vec<Item>, Option<String>, Option<DateTime<Local>>);

pub(super) fn search(m: &Market, src: &Source, q: &str, at: Option<&str>) -> Result<Found> {
    match src.api {
        Api::SkillsSh => skills_sh(m, src, q),
        Api::Marketplace => marketplace(m, src, q, at),
        Api::SkillsMp => skillsmp(m, src, q, at),
        Api::McpRegistry => registry(m, src, q, at),
        Api::Smithery => smithery(m, src, q, at),
        Api::GithubDir => github_dir(m, src, q),
    }
}

fn skills_sh(m: &Market, src: &Source, q: &str) -> Result<Found> {
    if q.is_empty() {
        return Ok((vec![], None, None)); // the endpoint searches; browsing is the marketplaces' job
    }
    let (b, stale) = m.cached_get(&format!("{}?q={}&limit={PAGE}", src.url, enc(q)), MAX_INDEX)?;
    let v = json(&src.id, &b)?;
    let list = v.get("skills").and_then(Value::as_array).ok_or_else(|| MarketError::BadAnswer(src.id.clone()))?;
    let items = list
        .iter()
        .filter_map(|x| {
            let id = s(x, "id")?;
            let repo = s(x, "source")?;
            let skill = s(x, "skillId").or_else(|| s(x, "name"))?;
            ok_repo(&repo).then(|| Item {
                id: format!("{}:{id}", src.id),
                kind: Kind::Skill,
                name: s(x, "name").unwrap_or_else(|| skill.clone()),
                source: src.id.clone(),
                description: String::new(),
                author: repo.split('/').next().map(String::from),
                license: None,
                updated: None,
                installs: x.get("installs").and_then(Value::as_u64),
                stars: None,
                version: None,
                homepage: Some(format!("https://skills.sh/{id}")),
                origin: Origin::Github { repo, path: String::new(), git_ref: None, skill: Some(skill) },
            })
        })
        .collect();
    Ok((items, None, stale))
}

/// `https://raw.githubusercontent.com/<o>/<r>/<ref>/…` → (`o/r`, ref).
fn raw_repo(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("https://raw.githubusercontent.com/")?;
    let mut p = rest.splitn(4, '/');
    let (o, r, rf) = (p.next()?, p.next()?, p.next()?);
    let repo = format!("{o}/{r}");
    ok_repo(&repo).then(|| (repo, rf.to_string()))
}

/// `https://github.com/o/r(.git)` → `o/r`.
fn github_repo(url: &str) -> Option<String> {
    let r = url.strip_prefix("https://github.com/")?.trim_end_matches('/').trim_end_matches(".git");
    ok_repo(r).then(|| r.to_string())
}

fn clean_path(p: &str) -> String {
    p.trim_start_matches("./").trim_matches('/').to_string()
}

fn join(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.into(),
        (_, true) => a.into(),
        _ => format!("{a}/{b}"),
    }
}

/// Where a marketplace plugin's files are (`source` of `marketplace.json`).
fn plugin_origin(base: Option<&(String, String)>, source: &Value) -> Origin {
    let sha = |v: &Value| s(v, "sha").or_else(|| s(v, "ref"));
    match source {
        Value::String(p) if p.starts_with("./") || p == "." => match base {
            Some((repo, rf)) => Origin::Github { repo: repo.clone(), path: clean_path(p), git_ref: Some(rf.clone()), skill: None },
            None => Origin::Unsupported { why: "relative_source".into() },
        },
        Value::Object(_) => {
            let kind = text(source, "source");
            match kind.as_str() {
                "github" => match s(source, "repo").filter(|r| ok_repo(r)) {
                    Some(repo) => Origin::Github { repo, path: String::new(), git_ref: sha(source), skill: None },
                    None => Origin::Unsupported { why: "github".into() },
                },
                "url" | "git-subdir" => {
                    let url = text(source, "url");
                    let path = clean_path(&text(source, "path"));
                    match github_repo(&url) {
                        Some(repo) => Origin::Github { repo, path, git_ref: sha(source), skill: None },
                        None if url.starts_with("https://") => Origin::Git { url, path },
                        None => Origin::Unsupported { why: "url".into() },
                    }
                }
                other => Origin::Unsupported { why: if other.is_empty() { "source".into() } else { other.chars().take(40).collect() } },
            }
        }
        _ => Origin::Unsupported { why: "source".into() },
    }
}

fn marketplace(m: &Market, src: &Source, q: &str, at: Option<&str>) -> Result<Found> {
    let (b, stale) = m.cached_get(&src.url, MAX_INDEX)?;
    let v = json(&src.id, &b)?;
    let plugins = v.get("plugins").and_then(Value::as_array).ok_or_else(|| MarketError::BadAnswer(src.id.clone()))?;
    let base = raw_repo(&src.url);
    let owner = v.get("owner").and_then(|o| s(o, "name"));
    let q = q.to_lowercase();
    let mut all = vec![];
    for p in plugins {
        let Some(name) = s(p, "name") else { continue };
        let origin = plugin_origin(base.as_ref(), p.get("source").unwrap_or(&Value::Null));
        let tags: Vec<String> = p.get("tags").and_then(Value::as_array).map(|a| a.iter().filter_map(|t| t.as_str().map(String::from)).collect()).unwrap_or_default();
        let hay = format!("{name} {} {} {}", text(p, "description"), text(p, "category"), tags.join(" ")).to_lowercase();
        let item = Item {
            id: format!("{}:{name}", src.id),
            kind: src.kind,
            name: name.clone(),
            source: src.id.clone(),
            description: text(p, "description"),
            author: p.get("author").and_then(|a| s(a, "name")).or_else(|| owner.clone()),
            license: s(p, "license"),
            updated: None,
            installs: None,
            stars: None,
            version: s(p, "version"),
            homepage: s(p, "homepage"),
            origin: origin.clone(),
        };
        let skills: Vec<String> = p.get("skills").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(clean_path)).collect()).unwrap_or_default();
        match (&origin, src.kind, skills.is_empty()) {
            // a plugin that lists its skills: each skill is one item
            (Origin::Github { repo, path, git_ref, .. }, Kind::Skill, false) => {
                for sp in skills {
                    let sname = sp.rsplit('/').next().unwrap_or(&sp).to_string();
                    if !q.is_empty() && !format!("{sname} {hay}").contains(&q) {
                        continue;
                    }
                    all.push(Item {
                        id: format!("{}:{name}/{sname}", src.id),
                        name: sname,
                        origin: Origin::Github { repo: repo.clone(), path: join(path, &sp), git_ref: git_ref.clone(), skill: None },
                        ..item.clone()
                    });
                }
            }
            _ if q.is_empty() || hay.contains(&q) => all.push(item),
            _ => {}
        }
    }
    let start: usize = at.and_then(|a| a.parse().ok()).unwrap_or(0);
    let next = (start + PAGE < all.len()).then(|| (start + PAGE).to_string());
    Ok((all.into_iter().skip(start).take(PAGE).collect(), next, stale))
}

/// `https://github.com/o/r/tree/<ref>/<path>` (or `/blob/<ref>/<path>/SKILL.md`) → origin.
fn github_tree(url: &str, skill: Option<String>) -> Option<Origin> {
    let rest = url.strip_prefix("https://github.com/")?.trim_end_matches('/');
    let parts: Vec<&str> = rest.split('/').collect();
    let repo = format!("{}/{}", parts.first()?, parts.get(1)?.trim_end_matches(".git"));
    if !ok_repo(&repo) {
        return None;
    }
    let (git_ref, path) = match parts.get(2) {
        Some(&"tree") | Some(&"blob") => {
            let mut p = parts[4.min(parts.len())..].join("/");
            if p.ends_with("SKILL.md") {
                p = p.trim_end_matches("SKILL.md").trim_end_matches('/').into();
            }
            (parts.get(3).map(|r| r.to_string()), p)
        }
        None => (None, String::new()),
        _ => return None,
    };
    Some(Origin::Github { repo, path, git_ref, skill })
}

fn skillsmp(m: &Market, src: &Source, q: &str, at: Option<&str>) -> Result<Found> {
    if q.is_empty() {
        return Ok((vec![], None, None));
    }
    let page: u64 = at.and_then(|a| a.parse().ok()).unwrap_or(1);
    let (b, stale) = m.cached_get(&format!("{}?q={}&limit={PAGE}&page={page}", src.url, enc(q)), MAX_INDEX)?;
    let v = json(&src.id, &b)?;
    let data = v.get("data").ok_or_else(|| MarketError::BadAnswer(src.id.clone()))?;
    let list = data.get("skills").and_then(Value::as_array).ok_or_else(|| MarketError::BadAnswer(src.id.clone()))?;
    let items = list
        .iter()
        .filter_map(|x| {
            let name = s(x, "name")?;
            let origin = github_tree(&text(x, "githubUrl"), None)?;
            Some(Item {
                id: format!("{}:{}", src.id, s(x, "id").unwrap_or_else(|| name.clone())),
                kind: Kind::Skill,
                name,
                source: src.id.clone(),
                description: text(x, "description"),
                author: s(x, "author"),
                license: None,
                updated: s(x, "updatedAt").or_else(|| x.get("updatedAt").and_then(Value::as_u64).map(|n| n.to_string())),
                installs: None,
                stars: x.get("stars").and_then(Value::as_u64),
                version: None,
                homepage: s(x, "skillUrl"),
                origin,
            })
        })
        .collect();
    let pg = data.get("pagination").or_else(|| v.get("pagination"));
    let more = pg.is_some_and(|p| {
        p.get("hasNext").and_then(Value::as_bool).unwrap_or(false) || p.get("totalPages").and_then(Value::as_u64).is_some_and(|t| page < t)
    });
    Ok((items, more.then(|| (page + 1).to_string()), stale))
}

pub(super) fn server_meta<'a>(entry: &'a Value, server: &'a Value) -> (Option<&'a Value>, Option<&'a Value>) {
    let official = entry.get("_meta").and_then(|m| m.get("io.modelcontextprotocol.registry/official"));
    let gh = server
        .get("_meta")
        .and_then(|m| m.get("io.modelcontextprotocol.registry/publisher-provided"))
        .and_then(|p| p.get("github"));
    (official, gh)
}

pub(super) fn registry_item(src_id: &str, entry: &Value) -> Option<Item> {
    let server = entry.get("server").unwrap_or(entry);
    let name = s(server, "name")?;
    let (official, gh) = server_meta(entry, server);
    if official.and_then(|o| s(o, "status")).as_deref() == Some("deleted") {
        return None;
    }
    let repo = server.get("repository").and_then(|r| s(r, "url"));
    // the copy kept in the item: the GitHub readme is a big untrusted text nobody installs from
    let mut keep = server.clone();
    if let Some(m) = keep.get_mut("_meta").and_then(Value::as_object_mut) {
        m.remove("io.modelcontextprotocol.registry/publisher-provided");
    }
    Some(Item {
        id: format!("{src_id}:{name}"),
        kind: Kind::Mcp,
        name: s(server, "title").or_else(|| gh.and_then(|g| s(g, "displayName"))).unwrap_or_else(|| name.clone()),
        source: src_id.into(),
        description: text(server, "description"),
        author: gh.and_then(|g| s(g, "nameWithOwner")).and_then(|n| n.split('/').next().map(String::from)).or_else(|| name.split('/').next().map(String::from)),
        license: gh.and_then(|g| s(g, "license")),
        updated: official.and_then(|o| s(o, "updatedAt")),
        installs: None,
        stars: gh.and_then(|g| g.get("stargazerCount")).and_then(Value::as_u64),
        version: s(server, "version"),
        homepage: s(server, "websiteUrl").or(repo),
        origin: Origin::Mcp { server: keep },
    })
}

fn registry(m: &Market, src: &Source, q: &str, at: Option<&str>) -> Result<Found> {
    // research §2.1: pages over ~10 hang on this registry
    let mut url = format!("{}/v0.1/servers?version=latest&limit=10", src.url.trim_end_matches('/'));
    if !q.is_empty() {
        url.push_str(&format!("&search={}", enc(q)));
    }
    if let Some(c) = at {
        url.push_str(&format!("&cursor={}", enc(c)));
    }
    let (b, stale) = m.cached_get(&url, MAX_INDEX)?;
    let v = json(&src.id, &b)?;
    let list = v.get("servers").and_then(Value::as_array).ok_or_else(|| MarketError::BadAnswer(src.id.clone()))?;
    let items: Vec<Item> = list.iter().filter_map(|e| registry_item(&src.id, e)).collect();
    let next = v.get("metadata").and_then(|md| s(md, "nextCursor")).filter(|c| !c.is_empty() && !list.is_empty());
    Ok((items, next, stale))
}

fn smithery(m: &Market, src: &Source, q: &str, at: Option<&str>) -> Result<Found> {
    let page: u64 = at.and_then(|a| a.parse().ok()).unwrap_or(1);
    let (b, stale) = m.cached_get(&format!("{}?q={}&pageSize=10&page={page}", src.url, enc(q)), MAX_INDEX)?;
    let v = json(&src.id, &b)?;
    let list = v.get("servers").and_then(Value::as_array).ok_or_else(|| MarketError::BadAnswer(src.id.clone()))?;
    let items = list
        .iter()
        .filter_map(|x| {
            let qn = s(x, "qualifiedName")?;
            Some(Item {
                id: format!("{}:{qn}", src.id),
                kind: Kind::Mcp,
                name: s(x, "displayName").unwrap_or_else(|| qn.clone()),
                source: src.id.clone(),
                description: text(x, "description"),
                author: s(x, "namespace"),
                license: None,
                updated: s(x, "createdAt"),
                installs: x.get("useCount").and_then(Value::as_u64),
                stars: None,
                version: None,
                homepage: s(x, "homepage"),
                origin: Origin::Smithery { name: qn },
            })
        })
        .collect();
    let total = v.get("pagination").and_then(|p| p.get("totalPages")).and_then(Value::as_u64).unwrap_or(0);
    Ok((items, (page < total).then(|| (page + 1).to_string()), stale))
}

fn github_dir(m: &Market, src: &Source, q: &str) -> Result<Found> {
    let rest = src.url.strip_prefix("https://api.github.com/repos/").ok_or_else(|| MarketError::BadSource(src.id.clone()))?;
    let mut p = rest.splitn(4, '/');
    let repo = format!("{}/{}", p.next().unwrap_or(""), p.next().unwrap_or(""));
    if !ok_repo(&repo) || p.next() != Some("contents") {
        return Err(MarketError::BadSource(src.id.clone()));
    }
    let (b, stale) = m.cached_get(&src.url, MAX_INDEX)?;
    let v = json(&src.id, &b)?;
    let list = v.as_array().ok_or_else(|| MarketError::BadAnswer(src.id.clone()))?;
    let q = q.to_lowercase();
    let items = list
        .iter()
        .filter(|x| s(x, "type").as_deref() == Some("dir"))
        .filter_map(|x| {
            let name = s(x, "name").filter(|n| !n.starts_with('.'))?;
            let path = s(x, "path")?;
            (q.is_empty() || name.to_lowercase().contains(&q)).then(|| Item {
                id: format!("{}:{name}", src.id),
                kind: src.kind,
                name,
                source: src.id.clone(),
                description: String::new(),
                author: repo.split('/').next().map(String::from),
                license: None,
                updated: None,
                installs: None,
                stars: None,
                version: None,
                homepage: s(x, "html_url"),
                origin: Origin::Github { repo: repo.clone(), path, git_ref: None, skill: None },
            })
        })
        .collect();
    Ok((items, None, stale))
}

/// See `Market::from_link`.
pub(super) fn from_link(kind: Kind, link: &str) -> Result<Item> {
    let l = link.trim();
    let bad = || MarketError::BadSource("link".into());
    if l.is_empty() || l.contains(['\n', '\r', '\0']) || l.starts_with('-') {
        return Err(bad());
    }
    let origin = if let Some(o) = github_tree(l, None) {
        o
    } else if l.starts_with("https://") || l.starts_with("ssh://") || l.starts_with("git@") {
        Origin::Git { url: l.into(), path: String::new() }
    } else {
        let p = Path::new(l);
        if !p.is_absolute() {
            return Err(bad());
        }
        if l.to_ascii_lowercase().ends_with(".zip") {
            Origin::Zip { path: p.into() }
        } else {
            Origin::Folder { path: p.into() }
        }
    };
    let name = l.trim_end_matches('/').trim_end_matches(".git").trim_end_matches(".zip").rsplit(['/', ':']).next().unwrap_or("item").to_string();
    Ok(Item {
        id: format!("link:{l}"),
        kind,
        name,
        source: "link".into(),
        description: String::new(),
        author: None,
        license: None,
        updated: None,
        installs: None,
        stars: None,
        version: None,
        homepage: l.starts_with("https://").then(|| l.to_string()),
        origin,
    })
}
