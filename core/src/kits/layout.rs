//! Where each environment keeps its rules, skills, hooks, MCP servers, subagents and settings
//! (spec §26). A group marked "not confirmed" there has no place here and stays inactive.

use super::Group;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ConfFmt {
    Json,
    Toml,
}

/// How the hooks key of the JSON config is shaped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum HookShape {
    /// `{"Event": [entry, ...]}` - Claude Code, Gemini, Qwen.
    EventMap,
    /// `["plugin", ...]` - opencode plugins.
    List,
}

#[derive(Debug)]
pub(crate) struct McpFile {
    /// Relative to the home folder.
    pub global: &'static str,
    /// Relative to the project root.
    pub project: &'static str,
}

/// Name of the MCP file inside a kit.
pub(crate) const KIT_MCP: &str = ".mcp.json";

#[derive(Debug)]
pub(crate) struct Layout {
    pub id: &'static str,
    pub name: &'static str,
    /// Relative to the home folder; empty = not confirmed.
    pub home_dir: &'static str,
    /// Relative to the project root.
    pub project_dir: &'static str,
    /// Rule files at the root of the environment folder.
    pub rules: &'static [&'static str],
    /// Rule files live at the project root (not inside `project_dir`) in project scope.
    pub rules_at_project_root: bool,
    /// Rules kept as a key of the config file (opencode `instructions`).
    pub rules_key: Option<&'static str>,
    pub skills: Option<&'static str>,
    pub agents: Option<&'static str>,
    pub config: Option<(&'static str, ConfFmt)>,
    pub config_at_project_root: bool,
    /// JSON: hooks key in the config; TOML: table prefix (`hooks`).
    pub hooks_key: Option<(&'static str, HookShape)>,
    /// A separate hooks file (Codex `hooks.json`), taken whole.
    pub hooks_file: Option<&'static str>,
    /// JSON: `mcpServers`; TOML: table prefix `mcp_servers`.
    pub mcp_key: Option<&'static str>,
    /// MCP kept in its own file outside the environment folder (Claude Code: `~/.claude.json`
    /// user scope, `.mcp.json` at the project root). The kit carries it as `.mcp.json`.
    pub mcp_file: Option<McpFile>,
    pub settings: bool,
    /// Top-level settings keys (TOML: keys and table names) a kit sets without a tick: model, theme, interface. Any
    /// other key may move the provider, its address or its key, or run something, and is held like a hook.
    pub safe_settings: &'static [&'static str],
    pub unverified: &'static [Group],
}

const ALL: &[Group] = &[Group::Skills, Group::Rules, Group::Hooks, Group::Mcp, Group::Agents, Group::Settings];

pub(crate) static LAYOUTS: &[Layout] = &[
    Layout {
        id: "claude",
        name: "Claude Code",
        home_dir: ".claude",
        project_dir: ".claude",
        rules: &["CLAUDE.md"],
        rules_at_project_root: false,
        rules_key: None,
        skills: Some("skills"),
        agents: Some("agents"),
        config: Some(("settings.json", ConfFmt::Json)),
        config_at_project_root: false,
        hooks_key: Some(("hooks", HookShape::EventMap)),
        hooks_file: None,
        // checked on claude 2.1.306: `claude mcp add -s user` → `mcpServers` in ~/.claude.json,
        // `-s project` → `.mcp.json`; local scope (projects[path].mcpServers) is not moved
        mcp_key: Some("mcpServers"),
        mcp_file: Some(McpFile { global: ".claude.json", project: ".mcp.json" }),
        settings: true,
        safe_settings: &["model", "theme", "outputStyle", "alwaysThinkingEnabled", "includeCoAuthoredBy", "cleanupPeriodDays", "spinnerTipsEnabled"],
        unverified: &[],
    },
    Layout {
        id: "codex",
        name: "Codex",
        home_dir: ".codex",
        project_dir: ".codex",
        rules: &["AGENTS.md", "AGENTS.override.md"],
        rules_at_project_root: true,
        rules_key: None,
        skills: None,
        agents: None, // `multi_agent` feature, files not confirmed
        config: Some(("config.toml", ConfFmt::Toml)),
        config_at_project_root: false,
        hooks_key: Some(("hooks", HookShape::EventMap)),
        hooks_file: Some("hooks.json"),
        mcp_key: Some("mcp_servers"),
        mcp_file: None,
        settings: true,
        safe_settings: &["model", "model_reasoning_effort", "model_reasoning_summary", "model_verbosity", "hide_agent_reasoning", "show_raw_agent_reasoning", "tui"],
        unverified: &[Group::Skills, Group::Agents],
    },
    Layout {
        id: "gemini",
        name: "Gemini",
        home_dir: ".gemini",
        project_dir: ".gemini",
        rules: &["GEMINI.md"],
        rules_at_project_root: true,
        rules_key: None,
        skills: None,
        agents: Some("agents"),
        config: Some(("settings.json", ConfFmt::Json)),
        config_at_project_root: false,
        hooks_key: Some(("hooks", HookShape::EventMap)),
        hooks_file: None,
        mcp_key: Some("mcpServers"),
        mcp_file: None,
        settings: true,
        safe_settings: &["theme", "ui"],
        unverified: &[Group::Skills],
    },
    Layout {
        id: "opencode",
        name: "opencode",
        home_dir: ".config/opencode",
        project_dir: ".opencode",
        rules: &[],
        rules_at_project_root: false,
        rules_key: Some("instructions"),
        skills: Some("skills"),
        agents: Some("agents"),
        config: Some(("opencode.json", ConfFmt::Json)),
        config_at_project_root: true,
        hooks_key: Some(("plugin", HookShape::List)),
        hooks_file: None,
        // checked on opencode 1.18.34 (`opencode debug config`): servers under key `mcp`
        mcp_key: Some("mcp"),
        mcp_file: None,
        settings: true,
        safe_settings: &["$schema", "theme", "model", "small_model", "keybinds", "tui", "username"],
        unverified: &[],
    },
    Layout {
        id: "cursor",
        name: "Cursor",
        home_dir: "",
        project_dir: "",
        rules: &[],
        rules_at_project_root: false,
        rules_key: None,
        skills: None,
        agents: None,
        config: None,
        config_at_project_root: false,
        hooks_key: None,
        hooks_file: None,
        mcp_key: None,
        mcp_file: None,
        settings: false,
        safe_settings: &[],
        unverified: ALL,
    },
    Layout {
        id: "qwen",
        name: "Qwen Code",
        home_dir: ".qwen",
        project_dir: ".qwen",
        rules: &["QWEN.md"],
        rules_at_project_root: true,
        rules_key: None,
        skills: None,
        agents: Some("agents"),
        config: Some(("settings.json", ConfFmt::Json)),
        config_at_project_root: false,
        hooks_key: Some(("hooks", HookShape::EventMap)),
        hooks_file: None,
        mcp_key: Some("mcpServers"),
        mcp_file: None,
        settings: true,
        safe_settings: &["theme", "ui"],
        unverified: &[Group::Skills],
    },
];

pub(crate) fn layout(env: &str) -> Option<&'static Layout> {
    LAYOUTS.iter().find(|l| l.id == env)
}

impl Layout {
    pub fn available(&self, g: Group) -> bool {
        !self.unverified.contains(&g)
            && match g {
                Group::Skills => self.skills.is_some(),
                Group::Rules => !self.rules.is_empty() || self.rules_key.is_some(),
                Group::Hooks => self.hooks_key.is_some() || self.hooks_file.is_some(),
                Group::Mcp => self.mcp_key.is_some(),
                Group::Agents => self.agents.is_some(),
                Group::Settings => self.settings && self.config.is_some(),
            }
    }

    /// Path of a file of the environment layout (`rel`, as in a kit) in project scope,
    /// relative to the project root.
    pub fn project_rel(&self, rel: &str) -> String {
        if rel == KIT_MCP {
            if let Some(m) = &self.mcp_file {
                return m.project.to_string();
            }
        }
        let at_root = (self.rules_at_project_root && self.rules.contains(&rel))
            || (self.config_at_project_root && self.config.is_some_and(|(c, _)| c == rel));
        if at_root {
            rel.to_string()
        } else {
            format!("{}/{}", self.project_dir, rel)
        }
    }
}
