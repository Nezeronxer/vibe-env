---
name: Nightshift
description: Agents as terminals - graphite, hairlines, one live accent that is the colour of the agent in focus; a column of agent cards that glow while they work and breathe when they wait.
colors:
  canvas: "#0b0c0f"
  ground: "#08090b"
  panel: "#111216"
  raised: "#17181d"
  raised-2: "#1e2026"
  line: "#24262d"
  line-strong: "#343741"
  ink: "#eceef3"
  ink-2: "#b9bdc8"
  ink-3: "#8a8f9c"
  accent-default: "#8e95ff"
  human: "#f5b14c"
  human-bg: "#2a2112"
  ok: "#5fd38d"
  bad: "#f2706a"
  terminal: "#0d0e11"
  env-claude: "#e8865f"
  env-codex: "#4cc38a"
  env-gemini: "#5b9cff"
  env-qwen: "#9b8cff"
  env-opencode: "#f0b35a"
  env-cursor: "#d6d9e0"
  day-canvas: "#eceef2"
  day-panel: "#ffffff"
  day-ink: "#15161a"
  day-accent-default: "#4f56d8"
  day-env-claude: "#b8532c"
  day-env-codex: "#0f7a4a"
typography:
  body:
    fontFamily: "system-ui, -apple-system, 'Segoe UI', 'Noto Sans', 'Helvetica Neue', sans-serif"
    fontSize: "13px"
    lineHeight: 1.45
  card-title:
    fontFamily: "system-ui, -apple-system, sans-serif"
    fontSize: "13px"
    fontWeight: 500
  group-label:
    fontFamily: "system-ui, -apple-system, sans-serif"
    fontSize: "10.5px"
    fontWeight: 600
    letterSpacing: "0.07em"
    textTransform: uppercase
  preview:
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, 'DejaVu Sans Mono', monospace"
    fontSize: "11px"
    lineHeight: 1.5
rounded:
  sm: "8px"
  card: "10px"
  pane: "12px"
  popover: "14px"
spacing:
  gap: "6px"
  rail-width: "272px"
motion:
  ease: "cubic-bezier(.2, .7, .2, 1)"
  spring: "cubic-bezier(.34, 1.4, .64, 1)"
  quick: "140ms"
  move: "240ms"
---

# Nightshift - v7 «агенты как терминалы»

Replaces the v5 «Ночная радиостудия» (felt, brass, cream meter faces). Source of every value: the theme table in
`ui/js/v4/theme.js`; `ui/css/tokens.css` carries the same values for the first paint (selftest checks they never drift).
No screen keeps a colour of its own.

## Direction
Dark premium in the manner of Warp and Linear: near-black graphite with a hair of blue, flat surfaces, 1px hairlines
instead of heavy shadows, one live accent. The accent (`--accent`, `--amber`) is the env colour of the agent in focus -
Claude terracotta, Codex green, Gemini blue, Qwen violet, opencode amber, Cursor silver - and flows to the brand mark, the
highlight bar of the rail, focus rings and primary buttons when focus moves. The light theme is the same layout on near-white
with darker env colours (every env colour reads >= 4.5:1 on the panel, selftested).

## Layout
- Left: the agents column (272px) - brand + Clawd, «Новый агент», cards grouped by project (uppercase label, a coloured
  square per project), the foot with «⌘K Поиск и команды», «Автоматика» (bolt + count) and the gear. «Ждут: N ⌘J» appears
  above them only when someone waits.
- Right: the panes of the active agent (split up to 4, a card dropped on the panes opens beside), then the status bar.
- The chat list (⌘B) is hidden by default; a chat is one click away (header «Чат», «Новый агент → Чат с агентом», ⌘K).

## Agent card
Grid: env icon · title (task or the CLI's own title) · state dot · ×; two lines of the terminal's last output (mono, frames
and spinners stripped); meta: env name, branch chip for an own copy, tokens from the CLI's own files, ⌘N on hover.
One highlight element glides under the card in focus (transform + height on the spring), with a 3px bar in the env colour.

## States
- run: a light runs round the card and its pane (conic gradient masked to a 1px rim, `@property --a7`), speed follows the
  output rate (4s idle print, 2.6s, 1.6s when it pours); a soft glow of the env colour.
- wait (an env CLI went quiet after real work while out of sight): warm `--human` tint and a breathing rim; a sound (two
  soft sine notes) and a system notification unless the window has focus; ⌘J jumps to the next one.
- done / fail: the dot only; a failed card's preview turns faintly red.

## Motion
Everything at or under 240ms: the highlight glides, a switched agent's pane comes in with 6px and fade, popovers pop from
.97, the changes panel slides 14px. `prefers-reduced-motion`: no aura, no breathing, no glide.

## Don't
Grain, lamps, brass, cream faces (v5); a second accent next to the env colour; colours outside tokens.css; innerHTML for
terminal text or diffs (text nodes only).
