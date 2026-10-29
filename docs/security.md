# Security map

This is the map of trust boundaries that the pre-publication security audit works through. It states
what the design intends and where to look. The audit results are in the last section.

Reporting a vulnerability: privately through GitHub, https://github.com/Nezeronxer/vibe-env/security/advisories/new (not in a public issue).

## What agents can and cannot do

| Environment | Technical limits | Everything else |
|---|---|---|
| Claude Code | sandbox on, `git push` / remote changes / `sudo` denied by flags (`Adapter::guarded()`); network is closed unless the CLI is older than 2.1.186 or its version cannot be read (`network_note`) | rules in the role prompt |
| Codex, Gemini CLI, Cursor, opencode, Qwen Code, custom command | none enforced by the app: the agent is confined to its working copy **by instructions only**, can use the network and read your home folder | the app shows a one-time "runs without a sandbox" notice before the first task in such an environment |

- Work always happens in a separate git copy (`ns/*` branch, worktree) outside your working folder.
- Read-only roles (reviewer, verifier, planner, critic) run with the CLI's read-only flag where it is
  confirmed, otherwise in a throw-away copy.
- The app never runs `git push --force`, never `git reset --hard` outside its own copies, deletes only
  branches `ns/*`, and touches your working copy only with `merge --ff-only` on a clean base branch.
- Protected paths (`.env*`, keys, `~/.ssh`) are never committed (checked before every commit).
  `.env` is not made unreadable to the agent, because tests in the copy need it.
- The Claude adapter can open the sandbox network (`WebFetch(domain:*)`) so npm/pip work; the risk is
  that files of the working copy can leave through the network. This is a separate audit item.

## Audit table

Each row is one area of the audit. "Where" lists code or config the checker starts from.

| Area | What is checked | Where |
|---|---|---|
| `/security-review` | the whole diff, v1 and v2 | whole repository |
| Secrets | gitleaks over the tree **and the whole git history**; no values in `state.json`, logs, kits, prompts; keyring or a 0600 file for secrets | `core/src/store.rs`, `core/src/kits/` |
| Command injection | no `sh -c` with data pasted in; custom command takes positional parameters (`{prompt}`, `{cwd}`, `{session}` become `"$1" "$2" "$3"`); extra arguments are a list; the terminal command is built as an argument list; session id checked by format; kit link: only allowed schemes, no `ext::`, no `--upload-pack` | `core/src/agent/`, `app/src/term.rs`, `core/src/kits/` |
| Prompt injection | task text, files, diff, output and foreign kits are wrapped in marked "data" blocks; signals (`NEEDS_HUMAN`, `DEFERRED`, `NS_TASK`) are accepted only from the end of a role's own answer; skills from imported kits are labelled and their scripts are off | `core/src/prompt/`, `core/src/engine/` |
| Rights and boundaries | read-only roles do not write; the chat copy does not touch your folder; `copy_files` and kits do not follow symlinks out; writes only into CLI folders, the project and the app data dir; git rules above | `core/src/git.rs`, `core/src/chat/`, `core/src/kits/` |
| Kits | zip slip, symlinks, size limits, broken archives; hooks, MCP servers and skill scripts are off until you enable them one by one; a kit of another environment is refused; rollback by snapshot restores everything; snapshots are 0700 | `core/src/kits/` |
| Tauri | minimal capabilities (`core:event:default`, `dialog:allow-open`); CSP `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost` with no external sources; no remote URLs in the window; `open_path` opens only `http(s)` links and plain files or folders inside registered projects or the app's copies; HTML/SVG/XML from an agent is only shown in the file manager; vendor SVG icons are bundled and contain no scripts, external links or `foreignObject` | `app/tauri.conf.json`, `app/capabilities/`, `app/src/open.rs`, `ui/icons/` |
| Network | the code has no endpoint of its own and no telemetry; outbound traffic is only: the processes of your environments, git/gh, and a kit clone from a link you typed | whole repository |
| Dependencies | `cargo audit` / `cargo deny`; new crates (`zip`, `keyring`); the removed `reqwest` | `Cargo.lock` |
| Denial of service | huge agent output and huge diffs must not exhaust memory (line-by-line reading, tails, caps) | `core/src/agent/`, `core/src/git.rs` |

## What leaves the machine, and what does not

Leaves: the prompts and files your agent CLIs send to their own vendors (that is the CLI's behaviour,
not the app's); git/gh traffic to your remotes; a kit download from a URL you gave.

Does not leave: task and chat history, settings, kit contents and secrets (kept in app data and the
system keyring), usage statistics (none are collected). The model list shipped with the app and all
vendor icons are inside the app, not fetched.

## Where data lives

App data directory (Tauri `app_data_dir()`): `state.json`, `catalog.json`, `limits.json`, chat logs,
kit snapshots (mode 0700). Secrets entered for kits go to the system keyring; if there is none, to a
file with mode 0600 (the app says so). Git copies live in a worktrees folder next to the app data.

## Audit report

Status: **run.** Last audit: 23.10.2026 (v4). Each audit was a set of independent checkers, and every finding
was re-checked by a fresh agent before it counted. Threat model: the attacker is the author of a foreign kit or
catalog item, a models.dev entry, or the contents of a repository an agent works on; the user and the machine
are trusted.

### v1-v3 (22.10-23.10.2026)

| Id | Area | Finding | Status | Re-check command |
|---|---|---|---|---|
| F1.1 | Kits | kit env variables were filtered by a deny list, so `CLAUDE_ENV_FILE`, `CLAUDE_CODE_SHELL_PREFIX`, `OPENCODE_CONFIG(_CONTENT)`, `CODE_ASSIST_ENDPOINT` passed | fixed: allow list of secret-like names only (`*_API_KEY`, `*_KEY`, `*_TOKEN`, `*_SECRET`, `*_PASSWORD`) | `cargo test -p nightshift-core --test kits` |
| F1.2 | Kits | kit settings that change a provider address or auth (opencode `baseURL`/`apiKey`, Codex `model_provider`, `[model_providers.*]`, `*_base_url`, `profiles`) were written silently | fixed: per-env allow list of safe top-level keys, the rest is held until ticked | `cargo test -p nightshift-core --test kits` |
| F1.3 | Kits | skills and subagents with `hooks`/`mcpServers`/`permissionMode`/`allowed-tools`/`tools` in frontmatter or a `` !`cmd` `` line installed without a tick | fixed: held as `ScriptedSkill` and shown whole | `cargo test -p nightshift-core --test kits` |
| F1.4 | Rights and boundaries | `Settings.protected_paths` was never read: an agent-made `.env` or key could be committed, merged and pushed | fixed: protected paths unstaged after `git add -A`, checked over `base..HEAD` before merge, push and PR; Claude gets `Read(~/.ssh/**)` deny | `cargo test -p nightshift-core --test git --test engine_e2e` |
| F2.1 | Secrets | a `.env` symlink in the task copy was not read, so its values were not scrubbed from the feed and log | fixed: symlinked files read through `metadata` | `cargo test -p nightshift-core --test adapters` |
| F2.2 | Secrets | the feed cut 20 lines / 120 characters before scrubbing, so a multi-line secret (PEM) leaked in part | fixed: scrub first, then cut | `cargo test -p nightshift-core --test adapters` |
| F2.3 | Secrets | `.env` search went depth-first and one deep folder used up the limit, so `server/.env` was not read | fixed: breadth-first search | `cargo test -p nightshift-core --test adapters` |
| R1 | Command injection | branch name from `origin/HEAD` used as a git option | not confirmed: never reaches a dangerous call | - |
| R2 | Command injection | `session_id` passed to `--resume` unchecked | not confirmed: the attacker does not control it (and it is checked by format) | - |
| D1 | Dependencies | `glib` 0.18.5 (GHSA-wrw7-89jp-8q8g, medium) through the Tauri GTK stack (Linux) | deferred: the app passes it no attacker data; updates with Tauri | `cargo audit` |

gitleaks for v1-v3: no record of a run is kept in the repository or the task notes; v4 below covers the whole history.

### v4 (23.10.2026): terminals, catalogs, CLI updates, publishing

| Id | Area | Finding | Status | Re-check command |
|---|---|---|---|---|
| F4.1 | Command injection | model ids from models.dev were not checked: an id like `--yolo` became a family and then the `-m` value of Qwen/Gemini, read as a flag (medium) | fixed: catalog ids must pass `is_model_id`; every adapter passes `-m`/`--model` only for a valid id | `cargo test -p nightshift-core --test updates --test adapters` |
| F4.2 | Kits | a catalog skill hid `hooks`/`allowed-tools` with YAML quoting or escapes (`"hoo\x6bs":`) and installed without a tick (medium) | fixed, closed by default: a top-level frontmatter key that is not a plain identifier (quotes, `\`, `?`, `&`, `*`, `!`, `{`) holds the skill; keys compared case-blind with `-`/`_` ignored | `cargo test -p nightshift-core --test kits` |
| F4.3 | Secrets | `terms.json` kept up to 2 MB of terminal output per session, mode 0644, never removed (low) | fixed: written 0600, removed right after it is loaded at start | `cargo test -p nightshift-app` |
| F4.4 | Rights and boundaries | 0x1E/0x1F bytes in a commit subject forged history rows, so "Revert" on a forged row reverted a commit the attacker chose (low) | fixed: `log_page` reads NUL-separated records and stops at a record that is not 7 fields with a hex sha, hex parents and a numeric time (see D2) | `cargo test -p nightshift-core --test git_v4` |
| D2 | Rights and boundaries | deviation from F4.4: a bad record cuts the history list short instead of being skipped | kept on purpose: after a bad record the fields are shifted, so skipping it would let it forge the rows next to it; a bad record only appears for objects made with `hash-object --literally` | `cargo test -p nightshift-core --test git_v4` |

No v4 finding was refuted. gitleaks v4 (tree and whole history): only false positives, `api_message_id` in a test
fixture and the minified xterm bundle.

gitleaks clean (`gitleaks git .` and `gitleaks dir .`: no leaks found), allowlist in `.gitleaks.toml`: it extends the default rules and allows only checked false positives, by path (the `claude_stream_partial.jsonl` fixture with its `msg_...` ids, the minified `ui/vendor/xterm/*.js`, and `target/` plus `graphify-out/`, which git ignores) and by regex (the fake `tok-123456789` of the kits test and the `keys3.*` i18n keys); no rule is switched off.
