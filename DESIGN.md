---
name: Nightshift
description: Night radio studio for agent CLIs - warm felt, brass hairlines, cream meter faces, one amber live accent.
colors:
  felt-canvas: "#1f160e"
  felt-ground: "#0f0b08"
  felt-panel: "#130e09"
  felt-raised: "#261c13"
  felt-raised-2: "#32261a"
  brass-line: "#392d1f"
  brass-line-strong: "#5a4830"
  brass: "#b8935a"
  ink: "#f3eadb"
  ink-2: "#d6c9b0"
  ink-3: "#ad9e82"
  amber: "#f2a33a"
  amber-on: "#231703"
  human-amber: "#f6b445"
  human-bg: "#2d2110"
  cream: "#efe3c8"
  cream-deep: "#d6c49b"
  cream-ink: "#2d2114"
  needle: "#c4620d"
  on-air: "#ff5d48"
  bad: "#f26a55"
  done: "#86cb98"
  walnut: "#38230f"
  walnut-deep: "#27180a"
  day-ground: "#faf3e3"
  day-canvas: "#e4d7bb"
  day-panel: "#f5ecd8"
  day-ink: "#2b1f14"
  day-accent-text: "#8f5200"
  day-on-air: "#c4311f"
  day-done: "#17693f"
typography:
  chat:
    fontFamily: "system-ui, -apple-system, 'Segoe UI', 'Noto Sans', 'Helvetica Neue', sans-serif"
    fontSize: "14.5px"
    fontWeight: 400
    lineHeight: 1.55
  body:
    fontFamily: "system-ui, -apple-system, 'Segoe UI', 'Noto Sans', 'Helvetica Neue', sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.45
  heading:
    fontFamily: "system-ui, -apple-system, 'Segoe UI', 'Noto Sans', 'Helvetica Neue', sans-serif"
    fontSize: "22px"
    fontWeight: 650
    letterSpacing: "-0.015em"
  label:
    fontFamily: "system-ui, -apple-system, 'Segoe UI', 'Noto Sans', 'Helvetica Neue', sans-serif"
    fontSize: "12px"
    fontWeight: 400
  code:
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, 'DejaVu Sans Mono', monospace"
    fontSize: "12.5px"
    fontWeight: 400
rounded:
  sm: "8px"
  pane: "12px"
  bubble: "14px"
  field: "16px"
spacing:
  gap: "6px"
  feed-width: "760px"
components:
  button:
    backgroundColor: "{colors.felt-raised}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
    height: "30px"
    padding: "0 12px"
  button-primary:
    backgroundColor: "{colors.cream}"
    textColor: "{colors.cream-ink}"
    rounded: "{rounded.sm}"
    height: "30px"
    padding: "0 12px"
  button-sm:
    height: "24px"
    padding: "0 8px"
  composer:
    backgroundColor: "{colors.felt-raised}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
    padding: "10px"
  tab:
    textColor: "{colors.ink-3}"
    rounded: "{rounded.sm}"
    height: "28px"
  tab-active:
    textColor: "{colors.ink}"
    backgroundColor: "{colors.felt-panel}"
---

# Design System: Nightshift

## Overview

**Creative North Star: "Ночная радиостудия"**

The app is a night radio studio: agents are on air, the person is the producer. The ground is warm brown-black felt (never blue-black) lit by a soft amber lamp from the top-left, with a faint grain and a brass reflection from the bottom-right. Panels are deeper felt with 1px warm hairlines, the side list is walnut, meters are aged cream with dark ink. The light theme is the daytime studio: cream ground, walnut ink, same amber.

The interface stays quiet and raises its voice only for "Ждёт тебя". State is told by lamps (tally) and meters (VU), not by badges or colour fills. Controls stay standard (buttons, toggles, tabs, inputs); the world lives in material, light and the two signature devices.

**Key Characteristics:**
- Warm felt ground with lamp-glow radial gradients and grain, both themes built from the same tokens.
- Brass hairlines (1px) instead of shadows to separate panes.
- Amber is the single live accent; red, green and cream each have one narrow job.
- Tally lamps and VU arcs are the signature; cancelled work is stamped, never removed.
- System sans for everything; system mono only for code, diffs and the terminal.

## Colors

Warm, low-chroma neutrals with one amber voice. Values are dark theme; the light theme swaps grounds to cream and ink to walnut (day-* tokens) and keeps amber. Source of truth: `ui/js/v4/theme.js`, emitted to `ui/css/tokens.css`.

### Primary
- **Amber Needle** (`amber`): the one live accent: focus ring (outline 2px at 80%), composer focus border, caret, accent-color of native controls, send button, VU glow. In light theme text-sized accent switches to `day-accent-text` for contrast; the lamp amber itself stays `amber`.
- **Studio Phone Amber** (`human-amber`, on `human-bg` with a `#6e5120` line): the "Ждёт тебя" state only; lamp pulses (lamp-ring, 1.5s).

### Secondary
- **ON AIR Red** (`on-air`, light `day-on-air`): running/in-air tally lamp only. `bad` (`#f26a55`) is the error text/border colour, never a decoration.
- **Signal Green** (`done`, light `day-done`): done only.

### Neutral
- **Felt ground / canvas / panel / raised** (`felt-ground`, `felt-canvas`, `felt-panel`, `felt-raised`, `felt-raised-2`): window, pane and card steps, darkest to lightest.
- **Brass hairlines** (`brass-line`, `brass-line-strong`, `brass`): 1px borders; `brass` appears on hover borders and the scrollbar thumb hover.
- **Ink ramp** (`ink`, `ink-2`, `ink-3`): primary, secondary, tertiary text.
- **Walnut** (`walnut`, `walnut-deep`): side-list gradient with grain.
- **Cream meter face** (`cream`, `cream-deep`, `cream-ink`): VU faces and the primary button; always paired with dark ink.
- **Needle** (`needle`): VU arc and needle stroke on cream faces.

Role colours (plan, critic, tests, executor, gate, review, integrator, coordinator, watcher, queue) are defined per theme in `theme.js` and used only for role marks and names, always with shape plus label.

### Terminal palette
xterm colours come from the same table (`ansi-0..15`, warm coal background off the panel, cream text). Floors set in `theme.js`: text at least 7:1 (about 14:1 on dark), ANSI 1-6 at least 4.5:1 (6.7:1 on dark). The user's own terminal config overrides these defaults.

### Named Rules
**The One Voice Rule.** Amber is the only live accent, focus border included. A second accent hue is a defect.
**The Tally Rule.** Red means on air/running, amber means calling you, green means done. Nothing else wears these colours.
**The Warm Ground Rule.** Grounds are brown-black or cream, never grey-blue.

## Typography

**Display / Body Font:** system sans stack (`system-ui`, -apple-system, Segoe UI, Noto Sans, Helvetica Neue). No network fonts (strict CSP).
**Mono Font:** system mono stack (`ui-monospace`, SFMono, Menlo, Consolas, DejaVu Sans Mono), only for code, diffs and the terminal.

**Character:** plain and legible; hierarchy comes from size, weight 600-650 and ink step, not from a display face.

### Hierarchy
- **Heading** (650, 22px settings page, 26px onboarding at 700; tracking -0.015em to -0.03em): page titles. Empty-chat title 30px/1.2 at 600.
- **Title** (650, 17-18px): panel headings.
- **Chat** (400, 14.5px, 1.55): messages in the 760px feed; message column about 70ch.
- **Body** (400, 13px, 1.45): chrome and lists.
- **Label** (400-600, 12-12.5px; 11.5px for time, tabular numerals): message headers, field labels, hints in `ink-3`.
- **Code** (400, 12.5px mono): inline code on `felt-raised-2`, diffs, terminal.

### Named Rules
**The Mono Is For Code Rule.** Mono is reserved for code, diff and terminal. Chrome and labels stay sans.

## Layout

Three zones: a 240px walnut side list, a 760px centred feed (`--feed-w`) on lit felt, and a status bar below. The composer sits at the feed bottom, up to 772px wide, 16px from the bottom. Panes are 12px-rounded felt cards with a 6px gap. Tabs and split panes sit above the feed. Breakpoints: 1199px (Factory grid collapses to one column), 1099px, 899px (feed padding 14px, composer inset 28px, settings single column), 820px, 599px (status bar drops secondary meters). Verified at 1440 and 960 wide in both themes.

## Elevation & Depth

Hybrid, mostly tonal: depth comes from felt steps, gradient sheen on cards (`cream` at 7% over `raised`), inset 1px highlights and brass hairlines. Shadows are soft and warm-dark, used for floating layers and the primary button only.

- **Float** (`0 14px 34px rgba(0,0,0,.5), 0 2px 6px rgba(0,0,0,.35)`; light uses warm brown at .22/.12): menus, dialogs.
- **Small** (`0 3px 10px rgba(0,0,0,.4)`): primary button.
- **Lamp glow** (`0 0 9px 1px` of the lamp colour at 60%): tally lamps only.

**The Glow Is Light Rule.** Glow belongs to lamps and meters. A card never glows.

## Shapes

Gently rounded, never pill-shaped chrome. Controls 8px (`--radius-sm`), panes 12px, chat bubbles 14px, the composer field up to 16px. Lamps are 10px circles with a radial highlight; failed lamps are struck through, stamped (cancelled) lamps are dashed rings. Borders are 1px; the stamp is a 1.5px rotated outline.

## Components

### Buttons
- **Shape:** 8px radius, 30px tall (24px `sm`), 0 12px padding, weight 500.
- **Default:** felt card gradient, `brass-line-strong` border; hover raises to `felt-raised-2` with brass border; active scales .97.
- **Primary:** cream gradient with dark ink, weight 600, small shadow and inset highlight. Hover dims brightness .94.
- **Danger:** transparent with `bad` border and text; hover fills `bad-bg`. Disabled at .45 opacity.

### Composer
- A felt card with 1px `brass-line-strong` border, 10px padding; border turns amber on focus-within (the inner textarea has no outline). Textarea 14px.

### Tabs
- 28px, 8px radius, `ink-3` text; hover tints with 7% cream; active is `ink` on a faint cream-over-panel gradient with an inset 1px hairline.

### Tally lamp (signature)
- 10px dot beside each chat row and on pane headers. States: on-air (red, glow), calling (amber, pulsing), done (green), failed (struck through), stamped (dashed). The pulse stops under `prefers-reduced-motion`.

### VU meter (signature)
- 48x28 cream face with brass-tinted stroke, ticks in `cream-ink`, a red zone arc, a `needle` arc and needle; needle eases 0.6s. Used for limits in the status bar.

### Stage strip
- The task stages (План to Merge) read as one signal line with lit stations; running station glows red, waiting station amber.

## Do's and Don'ts

### Do:
- **Do** take every colour from `tokens.css` variables; add colours in `theme.js`, which also feeds xterm.
- **Do** keep text at least 4.5:1 and body targeting 7:1 on dark; pair role colours with a shape and a label.
- **Do** respect `prefers-reduced-motion`: lamps and pulses go static.
- **Do** let the user's terminal config override terminal defaults.
- **Do** use brass 1px hairlines to separate panes.

### Don't:
- **Don't** add a second accent or tint a focus ring any colour but amber.
- **Don't** use red for anything but on-air/running (and `bad` for errors), or green for anything but done.
- **Don't** use mono for chrome or labels.
- **Don't** use blue-black or neutral grey grounds, or a flat grey chat with one blue accent.
- **Don't** load network fonts or assets.

## Not canonized (carried defects, not rules)

- Tab and menu glyph icons set in 11px bold mono (`.t-glyph`) are a stopgap; new surfaces use SVG icons from the sprite.
- The uppercase 10.5px letter-spaced `.st-stamp` is a one-off stamp for cancelled work; it is not a pattern for labels or kickers.
- `--accent`, `--amber` and `--human` share near-identical hues under three names; treat them as one voice and do not add more.
