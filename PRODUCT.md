# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

(Tauri 2 desktop app for macOS and Linux; the UI is a vanilla JS/CSS web view with a strict CSP, system fonts only, no network assets.)

## Users
A solo developer (the owner, Russian-speaking) who uses agent CLIs (Claude Code first; Codex, Gemini, opencode, Qwen, Cursor) every day. Situation: hands a coding task to agents, leaves, comes back when called. Job: describe the work in a chat, watch progress at a glance, answer the agent's questions, accept or undo the result.

## Product Purpose
Nightshift hands coding work to the user's agent CLIs and stays quiet until it needs a human decision. Success: the user writes one message, the agents plan, write tests first, code in a separate git worktree, pass project checks and an independent review, merge, and only call the person when a decision, a failure, or a push needs them.

## Positioning
Not a remote control for parallel agents where you watch diffs and merge yourself: the full loop (plan, critic, tests first, code, checks, review, merge) runs before a human is called. Everything happens inside the user's own CLIs, subscriptions and limits.

## Operating Context
Daily desktop use, long sessions, often dark macOS. Main surface is a chat per environment where a message is a question and "Сделай…" becomes a task in the same feed. Around it: tabs and split panes, a terminal multiplexer (real shell, Claude Code TUI), the Factory (all agents and the merge queue), Swarm (several agents on one task), Settings with a hand-editable config file, a command palette (⌘K), a pixel mascot Clawd.

## Capabilities and Constraints
- Terms (CONTEXT.md): Среда, Задача, Роль, Запуск, worktree, Проверки, Качество, Рой, Фабрика. Use them, avoid listed synonyms.
- Russian UI first, English second; short hyphen `-`, never em dash.
- No new dependencies or network fonts; colours through tokens (`ui/css/tokens.css`) and themes (`ui/js/v4/theme.js`); dev-mock renders the UI without Tauri for screenshots.
- Terminal colours come from the theme (xterm palette); user config overrides defaults.

## Brand Commitments
- Name "Nightshift" (working name). Mascot Clawd (pixel crab-like character, orange). Environment vendor marks in `ui/icons/`.
- User request 23.10.2026, binding: "Сделай приложение красивым. Градиенты какие-то добавь, кремы ... приложение сейчас очень скудное" - the current look is too sparse; gradients and warm/cream tones are explicitly welcome.
- User request 23.10.2026, binding: main view is a minimal but beautiful chat (references: VibeDeck, Vibe Kanban, Vibecode, Pinterest picks in `.autopilot/vibe-env/refs/v5-chat/`).

## Evidence on Hand
- Screenshots: `docs/screenshot-main-*.png`, `.autopilot/vibe-env/screens-v2/` (f7/f8 after).
- References: `.autopilot/vibe-env/refs/` (Dribbble agent dashboards), `refs/v5-chat/` (chat apps, Pinterest).
- All data on screenshots is dev-mock, synthetic; no real users, testimonials or metrics exist.

## Product Principles
1. Quiet until needed: the interface shows state calmly and raises its voice only for "Ждёт тебя".
2. Chat first: a newcomer must see where to write and what the agent is doing right now.
3. Everything reachable without memorising keys: visible entries for Settings, Factory, terminal.
4. The user's own tools and config win over our defaults.

## Accessibility & Inclusion
Text contrast at least 4.5:1 (body 7:1 targeted on dark), keyboard reachable controls, `prefers-reduced-motion` respected.
