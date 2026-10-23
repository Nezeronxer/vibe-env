//! "Стартовый налог" (story 306): what a session of an env pays before the first word. Rule files (CLAUDE.md,
//! AGENTS.md ...) go into the context whole; skills and subagents only by their name and description (the body loads
//! when called - research-youtube, приём 2); MCP servers by the schemas of their tools, which only the running server
//! knows, so they are counted, not estimated. Tokens are estimated as bytes / 4.
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Rule files longer than this get a warning: every session pays for each line.
pub const LONG_RULES_LINES: usize = 200;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct RuleFile {
    pub path: PathBuf,
    pub lines: usize,
    pub tokens: u64,
    /// Longer than `LONG_RULES_LINES`.
    pub long: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct StartupTax {
    pub rules: Vec<RuleFile>,
    pub skills: u32,
    pub skill_tokens: u64,
    pub agents: u32,
    pub agent_tokens: u64,
    /// Names of the MCP servers the env starts; their tool schemas are not in any file.
    pub mcp_servers: Vec<String>,
    /// Rules + skill and agent descriptions (MCP not included).
    pub total_tokens: u64,
}

fn tokens(bytes: usize) -> u64 {
    bytes.div_ceil(4) as u64
}

/// MCP config files: (file, JSON key or "toml" for `[mcp_servers.<name>]` tables).
type McpFiles = &'static [(&'static str, &'static str)];

/// Where an env keeps rules, skills, subagents and MCP configs: (home-relative, project-relative) for each.
struct Layout {
    rules: (&'static [&'static str], &'static [&'static str]),
    skills: (&'static [&'static str], &'static [&'static str]),
    agents: (&'static [&'static str], &'static [&'static str]),
    mcp: (McpFiles, McpFiles),
}

fn layout(env: &str) -> Layout {
    match env {
        "claude" => Layout {
            rules: (&[".claude/CLAUDE.md"], &["CLAUDE.md", ".claude/CLAUDE.md", "CLAUDE.local.md"]),
            skills: (&[".claude/skills"], &[".claude/skills"]),
            agents: (&[".claude/agents"], &[".claude/agents"]),
            mcp: (&[(".claude.json", "mcpServers")], &[(".mcp.json", "mcpServers")]),
        },
        "codex" => Layout {
            rules: (&[".codex/AGENTS.md"], &["AGENTS.md"]),
            skills: (&[".agents/skills"], &[]),
            agents: (&[], &[]),
            mcp: (&[(".codex/config.toml", "toml")], &[]),
        },
        "gemini" => Layout {
            rules: (&[".gemini/GEMINI.md"], &["GEMINI.md"]),
            skills: (&[".gemini/skills"], &[]),
            agents: (&[".gemini/agents"], &[".gemini/agents"]),
            mcp: (&[(".gemini/settings.json", "mcpServers")], &[(".gemini/settings.json", "mcpServers")]),
        },
        "qwen" => Layout {
            rules: (&[".qwen/QWEN.md"], &["QWEN.md"]),
            skills: (&[], &[]),
            agents: (&[".qwen/agents"], &[".qwen/agents"]),
            mcp: (&[(".qwen/settings.json", "mcpServers")], &[(".qwen/settings.json", "mcpServers")]),
        },
        "opencode" => Layout {
            rules: (&[".config/opencode/AGENTS.md"], &["AGENTS.md"]),
            skills: (&[], &[]),
            agents: (&[], &[]),
            mcp: (&[(".config/opencode/opencode.json", "mcp")], &[("opencode.json", "mcp")]),
        },
        // Cursor and own commands: only the shared AGENTS.md is known
        _ => Layout { rules: (&[], &["AGENTS.md"]), skills: (&[], &[]), agents: (&[], &[]), mcp: (&[], &[]) },
    }
}

fn paths<'a>(home: &'a Path, project: Option<&'a Path>, (h, p): (&'a [&'a str], &'a [&'a str])) -> impl Iterator<Item = PathBuf> + 'a {
    h.iter().map(move |r| home.join(r)).chain(project.into_iter().flat_map(move |d| p.iter().map(move |r| d.join(r))))
}

/// `name:` and `description:` of a Markdown front matter: what the CLI shows the model before a call.
fn front_matter(text: &str) -> usize {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return 0;
    }
    lines.take_while(|l| l.trim() != "---").filter(|l| l.starts_with("name:") || l.starts_with("description:")).map(str::len).sum()
}

/// Skill dirs hold `<name>/SKILL.md`; agent dirs hold `<name>.md`.
fn descriptions(dir: &Path, skill: bool) -> (u32, u64) {
    let Ok(rd) = std::fs::read_dir(dir) else { return (0, 0) };
    let mut n = 0;
    let mut t = 0;
    for e in rd.flatten() {
        let f = if skill { e.path().join("SKILL.md") } else { e.path() };
        if !skill && f.extension().is_none_or(|x| x != "md") {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&f) {
            n += 1;
            t += tokens(front_matter(&text));
        }
    }
    (n, t)
}

fn mcp_names(file: &Path, key: &str) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(file) else { return vec![] };
    if key == "toml" {
        return text.lines().filter_map(|l| l.trim().strip_prefix("[mcp_servers.")?.strip_suffix(']').map(|n| n.trim_matches('"').to_string())).collect();
    }
    serde_json::from_str::<serde_json::Value>(&text).ok().and_then(|j| j[key].as_object().map(|o| o.keys().cloned().collect())).unwrap_or_default()
}

/// The startup tax of `env` for a session in `project` (None = the global files only), from the files under `home`.
pub fn startup_tax(env: &str, home: &Path, project: Option<&Path>) -> StartupTax {
    let l = layout(env);
    let mut tax = StartupTax::default();
    for f in paths(home, project, l.rules) {
        if let Ok(text) = std::fs::read_to_string(&f) {
            let lines = text.lines().count();
            tax.rules.push(RuleFile { path: f, lines, tokens: tokens(text.len()), long: lines > LONG_RULES_LINES });
        }
    }
    for d in paths(home, project, l.skills) {
        let (n, t) = descriptions(&d, true);
        (tax.skills, tax.skill_tokens) = (tax.skills + n, tax.skill_tokens + t);
    }
    for d in paths(home, project, l.agents) {
        let (n, t) = descriptions(&d, false);
        (tax.agents, tax.agent_tokens) = (tax.agents + n, tax.agent_tokens + t);
    }
    let (h, p) = l.mcp;
    let files = h.iter().map(|(f, k)| (home.join(f), *k)).chain(project.into_iter().flat_map(|d| p.iter().map(move |(f, k)| (d.join(f), *k))));
    for (f, k) in files {
        for n in mcp_names(&f, k) {
            if !tax.mcp_servers.contains(&n) {
                tax.mcp_servers.push(n);
            }
        }
    }
    tax.total_tokens = tax.rules.iter().map(|r| r.tokens).sum::<u64>() + tax.skill_tokens + tax.agent_tokens;
    tax
}
