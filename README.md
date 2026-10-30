# Nightshift

[Русская версия](README.ru.md)

**A desktop app that hands coding work to your agent CLIs and stays quiet until it needs a human decision.**

> Nightshift is a working name (see [Renaming](#renaming)). Repository: https://github.com/Nezeronxer/vibe-env

![Main screen: chat with Claude Code, a task waiting for you, limit bars on top](docs/screenshot-main-dark.png)

The screenshot shows the main screen with dev-mock data, not a real project. A light theme is in
[`docs/screenshot-main-light.png`](docs/screenshot-main-light.png).

Most tools in this space are a remote control for several agents running in parallel: you watch the diffs and
merge. Nightshift is the opposite: you describe the work, the agents do it in a separate git copy, check it
with tests and an independent reviewer, merge it, and **call you only when something needs you**.

## What it looks like

Frames from a live run of the real app on a local project `bukva` (a small program that draws a letter), except "Swarm"
and "From GitHub…", which use demo data.

**An agent finished a task in its own copy of the project.** On the left the agent feed with previews and states, on the
right its terminal: 37 tests pass, your folder untouched.

![An agent finished a task in its own copy](docs/screenshots/01-agent-in-project.png)

**A new agent in one click.** Environment, model, project and where to work - right in the folder or in its own copy on
its own branch.

![The "New agent" popover](docs/screenshots/02-new-agent.png)

**Changes and merge.** Every changed file with its diff, undo any file with a button, "Merge into main" without
rewriting history.

![The Changes panel and the merge question](docs/screenshots/03-changes-merge.png)

**GitHub panel.** The project's branches with three agent merges and open pull requests next to the chat.

![Chat and the GitHub panel with branches](docs/screenshots/04-github-panel.png)

**A project from GitHub.** Sign in once through GitHub CLI, pick one of your repositories or paste any public one - it
becomes a project in `~/Projects`.

![The "Project from GitHub" dialog](docs/screenshots/05-from-github.png)

**Swarm.** Several agents on one task: a race of three variants, the reviewer picks the best, scores and tests shown.

![Swarm: a race of three variants](docs/screenshots/06-swarm.png)

**Catalog of skills, MCP servers and mods.** Description and license from the skill itself, installed into the agent you choose.

![Catalog: the pdf skill card](docs/screenshots/07-catalog.png)

## Quick start

1. Install at least one agent CLI (Claude Code, Codex, opencode, Qwen Code, Gemini…). The left column shows the
   installed ones; the rest sit under "Add" and install from there in the built-in terminal.
2. A project: the column head → "Create project…" (a name is enough: a folder in `~/Projects`, git and a first commit)
   or "Open folder…" or "From GitHub…" (sign in once with GitHub CLI, pick one of your repositories or paste
   `owner/name` or a link to any public one: it is cloned into `~/Projects` and becomes the project).
3. "New agent": environment, model, thinking level (Claude Code and Codex: "Low" to "Max"; also in the chat's model menu and ⌘K), project and where to work - "In its own copy" (its own `ns/term-*` branch, your
   files untouched) or right in the folder. Write the task → "Run".
4. The agent works in the terminal of its CLI. Done or asking - the card says "waits for you", a system notification
   fires, ⌘J jumps to it. "Chat | Terminal" in the session head shows the same talk as a chat or in the CLI itself.
5. "Changes" - what the agent changed (diff, undo a file) and "Merge into main".
6. Skills and MCP servers set up in one CLI show up in the others at start (Settings → General → "Skills and MCP in
   every agent"; "What was skipped and why" and the undo are there too).

## How the loop works

1. **Ask.** In the chat of an environment you ask the model a question, or write "Do ..." and it becomes a task in the same feed.
2. **Plan.** A planner writes the plan, a critic attacks it (roles are separate runs of the same environment, each with its own model).
3. **Tests first.** A test author writes failing tests that define "done".
4. **Code.** The executor works in its own git copy (`ns/*` branch) until the tests pass.
5. **Gates and review.** Your project's checks run, then an independent reviewer reads the diff. Up to 3 review rounds.
6. **Merge.** One squashed commit goes into `main` (or a pull request, per project setting). A failed merge check is retried after a fresh rebase; a conflict goes to a dedicated integrator run.

You are called (system notification with a sound, a mark on the environment icon) in three cases: the agent needs
a decision only you can make, the task failed after all attempts, or it asks permission to push. Any finished
task can be undone with one button; one key freezes all agents. The "Quality" switch adds extra prompt-improving
and verification steps; "Swarm" puts several agents on one task (race, split into parts, or council; 2 to 5).

## How it differs

From a survey of 25 comparable products (2026-10-21, notes in the repository's research files):

- Most of them show diffs and wait for you. We did not find one on the desktop that chains tests, self-check,
  independent review and auto-merge before calling a human.
- A git worktree is not a sandbox. Nightshift turns the sandbox on by default where the CLI offers one
  (Claude Code) and tells you plainly which environments run without technical limits.
- Continuing after a usage-limit reset exists in Claude Code Desktop; the products we looked at do not switch
  to another model on a limit. Nightshift falls back to another model **of the same environment**.
- Local-first: data is plain files, no telemetry, no server of our own (as in Backlog.md and Paseo).
- Linux is a first-class target, which is a weak spot for most of them.

Ideas we took from others: git worktree per task (everyone), several CLIs behind adapters (Claude Squad,
Superset, Orca), `.worktreeinclude`-style copying of `.env` (Claude Code Desktop), one-command tasks from chat
(Emdash, Terragon), merge queue with a fresh agent for conflicts (Gas Town), limit reset time on screen (Orca).

## Supported environments

"Verified" means the flags and output formats were checked against the real CLI on a development machine.
"Documentation" means they were taken from the vendor's docs only and have not been run.

| Environment | Status | Sandbox and push block |
|---|---|---|
| Claude Code | verified | sandbox on, push / remote / sudo denied by flags |
| opencode | verified | none enforced by the app |
| Qwen Code | verified | none enforced by the app |
| Codex | documentation | none enforced by the app |
| Gemini CLI | documentation | none enforced by the app |
| Cursor | documentation (the binary name `agent` is not confirmed) | none enforced by the app |
| Your own command | n/a | none enforced by the app |

Environments without technical limits are held to their working copy by instructions only; the app warns
you once before the first task. Environments never mix: a reviewer, a fallback model and every swarm member
belong to the environment of the task.

Not confirmed even for the verified ones: usage limits are shown only for Claude Code (5-hour and weekly windows,
from the CLI's own events); for the others the app says "no data" rather than guessing. Read-only flags of
opencode and Qwen Code are unconfirmed, so read-only roles run in a throw-away copy. Model lists for Claude Code,
Gemini CLI and Qwen Code are a built-in hint list (from vendor docs, not checked against the CLIs) plus "custom model".

## Limits

- Per task and per day cost budgets (defaults: 5 and 40 USD, configurable). The task budget is passed to Claude Code as `--max-budget-usd`.
- On a usage limit the task waits for the reset or continues on the fallback model of the same environment.
- A night window setting (default 23:00-08:00) and a "While you were away" summary of what happened while you were gone.
- Parallel tasks: 3 by default.

## Moving your setup ("Kits")

Install the app and your own agent CLIs and they work with their own folders as before. To hand your setup
to another machine or person, export a **kit** (skills, rules, hooks, MCP servers, subagents) as a folder, a zip
or a git repository, and **Import** it on the other side.

- A kit installs only into its own environment (a Claude Code kit does not go into Codex).
- Secrets do not travel: you enter them yourself; they are stored in the system keyring.
- Hooks, MCP servers, skills with scripts and settings that run programs are installed **switched off**; you enable them one by one.
- A snapshot is taken before every install, so one click rolls it back.
- Cursor kit groups are unavailable until its layout is verified (skills of Codex, Gemini and Qwen, and Codex subagents, are flagged "layout not verified" too).

## Security

Details and the audit map: [`docs/security.md`](docs/security.md).

- Work happens in a separate git copy; the app never force-pushes, never `reset --hard`s your branches, deletes only `ns/*` branches, and updates your working copy only by fast-forward on a clean base branch.
- `.env*`, keys and `~/.ssh` are never committed.
- Text from tasks, files, diffs and foreign kits is passed to agents as marked data, not instructions.
- The app has no telemetry and no endpoint of its own. Outbound traffic: your agent CLIs, git/gh, and a kit download from a link you gave.
- Windows allow only the minimum Tauri capabilities and a CSP without external sources.
- Environments other than Claude Code run without technical restrictions (see above).

## Meet Clawd

Clawd was born as a pixel on the Claude Code splash screen - nine dots wide, five tall. By day he sat still while people typed. At night, when everyone went to bed, he noticed the terminals stayed on: agents kept working and nobody watched over them. So Clawd took the night shift. He runs between tabs, taps along with whoever is writing code, jumps when a task lands in main, and waves when it cannot go on without you. They say he has never slept on duty - only when you pause everything yourself.

He lives in the corner right of the tabs and does what the agents do: types during an edit, runs during a command, waves when a task needs you, jumps for two seconds when one lands in main. Click him to see what he is doing in words. Settings → Basics → Clawd: animated, still or hidden; `?fps=1` shows his frame counter. How he works: [docs/clawd.md](docs/clawd.md).

## Add your own environment

- **A custom command** (no code): add a custom environment in Settings. The command template gets the placeholders `{prompt}`, `{cwd}` and `{session}`; they reach the command as safe positional arguments, never pasted into a shell string. Output is text, exit code 0 means success; a limit message is looked for in the last 5 lines.
- **An adapter** (Rust): implement the `Adapter` trait in `core/src/agent/` next to the existing ones (Claude, Codex, Gemini, opencode, Cursor, Qwen) and add it to `adapters()`. Mark any flag you could not run yourself as unverified. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Install

Windows 10/11 x64, macOS (Apple silicon and Intel) and Linux x86_64. Downloads: [https://github.com/Nezeronxer/vibe-env/releases/latest](https://github.com/Nezeronxer/vibe-env/releases/latest).

**Windows:** download `Nightshift_<version>_x64-setup.exe` (or the `.msi`) and run it - it installs for your user,
no admin rights. The build is not signed yet, so SmartScreen may say "Windows protected your PC": "More info" →
"Run anyway". Needs git ([git-scm.com](https://git-scm.com/download/win)) and at least one agent CLI; for
"From GitHub…" also GitHub CLI (`winget install --id GitHub.cli`). The terminal inside the app is PowerShell.

One line (the script downloads the latest release, checks its SHA-256 against `SHA256SUMS` from the same release and installs it). The checksum protects against a damaged download, **not** against a tampered release: both files come from the same place. Trust comes from the repository and its release page:

```sh
curl -fsSL https://raw.githubusercontent.com/Nezeronxer/vibe-env/main/scripts/install.sh | sh
```

macOS and Linux. Prefer to read the script first:
[`scripts/install.sh`](scripts/install.sh). It also sets the install target by distribution: `.deb` for the
Debian family and `.rpm` for Fedora/openSUSE (it prints the `sudo` command, it does not run sudo), the AppImage
in `~/.local/bin` plus a `.desktop` entry for everything else.

**By hand:** download from the Releases page (https://github.com/Nezeronxer/vibe-env/releases): `-setup.exe` / `.msi` (Windows), `.dmg` (macOS: from 1.0.1 one `_universal.dmg` for any Mac; in 1.0.0 an M1-M5 Mac takes `_aarch64.dmg`, an Intel one `_x64.dmg`), `.AppImage` (any Linux),
`.deb`, `.rpm`. Compare the file with `SHA256SUMS` from the same release (this catches corruption, not a replaced release).

**macOS and the missing signature.** The macOS build is not signed or notarized yet (that needs a paid Apple Developer
account). If macOS refuses to open the app, remove the quarantine flag:

```sh
xattr -dr com.apple.quarantine /Applications/Nightshift.app
```

**Linux runtime libraries.** `.deb` and `.rpm` declare them. For the AppImage on a minimal system install
WebKitGTK 4.1 and an appindicator library:

| Distribution | Packages |
|---|---|
| Debian, Ubuntu | `libwebkit2gtk-4.1-0 libayatana-appindicator3-1` |
| Fedora | `webkit2gtk4.1 libappindicator-gtk3` |
| Arch | `webkit2gtk-4.1 libayatana-appindicator` (a PKGBUILD is in [`app/installer/aur/`](app/installer/aur/PKGBUILD)) |

You also need git, and the agent CLIs you want to use (Nightshift runs them, it does not ship them).

## Build from source

Requirements: stable Rust (cargo 1.96 or newer), git, the Tauri CLI (`cargo install tauri-cli --version "^2"`).
On Debian/Ubuntu also: `build-essential pkg-config libssl-dev libxdo-dev librsvg2-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev`.

```sh
cargo test --workspace           # tests
cargo build -p nightshift-app    # build the window app
cd app && cargo tauri dev        # run in development
cd app && cargo tauri build      # installers in target/release/bundle/
```

`ui/` is plain HTML, CSS and JavaScript with no bundler. Quick UI checks without a window: `node scripts/ui-selftest.js`.
CI: `.github/workflows/ci.yml` (tests on macOS and Ubuntu, the app build on Windows), `.github/workflows/release.yml` (installers on a `v*` tag, draft release).
To sign macOS builds later, set the repository secrets named in `release.yml`.

## Not done yet, and out of scope

Not finished:

- Linux packages (`.deb`, `.rpm`, AppImage) are built only in CI; they were not run on Linux hardware by the author. The macOS `.dmg` was built locally.
- Chat reply queue and "waiting for limit" live in memory: after a restart use Retry. There is no model-fallback chain for a rejected model in chat.
- In a swarm a usage limit pauses the whole task, not one lane. Best-of-N for risky tasks is not built.
- Protection against sending the same chat message twice lives in memory: after a restart repeating it creates a second chat.
- On macOS a blocked-notifications warning rarely arrives.
- Codex, Gemini CLI and Cursor have never been run against this app (see the table above).

Out of scope on purpose: mobile and web clients; containers (the CLI sandbox, worktree and throw-away copies
cover the main need); a built-in editor or terminal (the diff is read-only, "continue in terminal" opens one);
starting tasks from the command line; translating kits between environments; our own limit estimates for
environments that report none; Flatpak (the AppImage covers other distributions); macOS signing (needs your Apple account).

## Publishing checklist

Do not publish the repository until every box is ticked; if one cannot be, publication waits and the reason is written down.

- [x] Security audit closed (`docs/security.md`, "Audit report" is filled in; every finding fixed or explained)
- [x] `gitleaks` over the whole history is clean (29.10.2026: 13 hits, all test fixtures and UI words; trufflehog: no live secret)
- [x] `README.md`, `README.ru.md` and `LICENSE` are in place
- [ ] CI builds (`ci.yml` green; a test tag produced the draft release)
- [x] Placeholders filled: `<OWNER>`, `<REPO>`, `<REPO_URL>`, `<SECURITY_CONTACT>`, `<MAINTAINER ...>` in the PKGBUILD
- [ ] Icon terms re-checked ([`docs/icons-sources.md`](docs/icons-sources.md))

## Renaming

The name is an assumption. Change `productName` and the window title in `app/tauri.conf.json`, the name inside the
dictionaries `ui/i18n.js` and `ui/i18n-v2.js`, and `APP_NAME` in `scripts/install.sh`. The name is also used
in the docs and a few identifiers (`nightshift-core`, `nightshift-app`).

## Trademarks and licenses

Trademarks belong to their owners and are used only to identify compatible products.
Товарные знаки принадлежат владельцам и используются только для обозначения совместимых продуктов.
Sources and terms of every logo: [`docs/icons-sources.md`](docs/icons-sources.md).

Released under the [MIT License](LICENSE). Contributing: [CONTRIBUTING.md](CONTRIBUTING.md).
