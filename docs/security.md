# Security map

This is the map of trust boundaries that the pre-publication security audit works through. It states
what the design intends and where to look. **It is not an audit result.** The audit has not been run
yet; its report goes into the last section.

Reporting a vulnerability: `<SECURITY_CONTACT>` (placeholder, set before publishing).

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

<!-- Filled in after the audit. One row per finding: id, area, severity, status
(fixed / not confirmed / deferred with a reason), how to re-check. -->

Status: **not run yet.**

| Id | Area | Finding | Status | Re-check command |
|---|---|---|---|---|
| - | - | - | - | - |
