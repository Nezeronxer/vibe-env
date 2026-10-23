---
version: 1
slug: "ui-index-html"
primary_target: "ui/index.html"
related_targets: []
---

# Surface: Nightshift main window (chat + shell)

Mode: Operate. Scope: the whole app shell - chat (main view), tabs, side list, composer, status bar, terminal colours, Factory/Settings inherit the same world. Code-led.

Audience/job: a solo developer hands work to agent CLIs at night and comes back when called. Task: write one message, see at a glance what each agent is doing, answer "Ждёт тебя", accept or undo. Constraints: tokens in `ui/css/tokens.css`, themes in `ui/js/v4/theme.js`, system fonts only, strict CSP, no new deps, structure of F7-F9 stays (layout, controls, behaviour). User pinned: gradients and warm/cream tones welcome; current look "скудное".

## Direction contract

THESIS: the app is a night radio studio where agents are on air and you are the producer; it refuses the category's flat grey chat with one blue accent.

OWN-WORLD: dark: warm charcoal acoustic-felt ground (brown-black, not blue-black) with a soft lamp-glow radial gradient from the top-left and a faint felt grain; panels are deeper felt with 1px warm-brass hairlines; meter faces in aged cream (#efe3c8-ish) with dark ink; amber needle (#f2a33a-ish) is the one live accent; ON AIR red only for "в эфире"/running tally; signal green for "готово". Walnut brown for the side list ground. Light theme = daytime studio: cream meter-face ground, walnut ink, same amber. Type: system sans for UI and messages, system mono only for code/diffs/terminal. Controls are standard: buttons, toggles, tabs, inputs.

STORY: newcomer sees where to write (the lit composer, like a mic channel), which agents are on air (tally lamps), and who is calling (studio phone lamp blinking on "Ждёт тебя"); they answer, then leave.

FIRST VIEWPORT: left 240px walnut list with tally lamps per chat (red = on air, amber blink = calls you, green = done); centre 760px feed on felt with lamp glow; composer at the bottom as a cream-rimmed channel strip with amber send; limits in the status bar drawn as small VU meters (cream face, amber needle arc), Clawd in the corner unchanged.

FORM: Night radio studio, position 6 of my ordered list (1 dusk-to-dawn sky, 2 darkroom, 3 sleeper train, 4 hi-fi, 5 observatory, 6 night radio studio, 7 diner); seed:c17642db1175ba60. Challenger raises: from ticket wallet - cancelled/undone tasks are stamped, never vanish; from transit diagram (competitive) - the task stage strip (План…Merge) reads as one continuous signal line with lit stations. Declined: Ikeda, Metro, Crouwel (no warmth, contradict pinned brief).

SIGNATURE: tally lamps + VU meters - a running agent's chat row and pane header carry a softly glowing red ON AIR lamp; "Ждёт тебя" is an amber lamp that pulses like a studio phone; limits are VU arcs. Motion respects prefers-reduced-motion (lamps static).

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
