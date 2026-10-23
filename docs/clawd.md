# Clawd

The pixel night watchman in the corner right of the tabs (stories 285-288). Code: `ui/js/v4/clawd.js`, strings `ui/js/v4/i18n-clawd.js`, styles `ui/css/v4-clawd.css`, checks in `ui/selftest.js` (block "v4 Clawd").

## States

| State | When | What you see |
|---|---|---|
| `call` | a task waits for you (`needs_human`, `awaiting_outward`, `merge_blocked`) | waves, blushes, hops, an amber "!" |
| `sad` | a task went to `failed` (shown 30 s) | sits, looks down, a tear now and then |
| `type` | an agent runs Edit / Write / MultiEdit (`task-action`, `chat-action`) | taps at a laptop, code glyphs fly out |
| `run` | an agent runs Bash | runs on the spot, dust behind |
| `think` | a task is active with no open action, or a chat answer is streaming (`chat-delta`, 3 s after the last piece) | sways, hand at the chin, thinking dots |
| `cheer` | a task went to `done` (2 s) | crouch, jump, confetti |
| `sleep` | the app is frozen or paused, or only a night queue waits | cap on, eyes shut, "z" |
| `idle` | nothing else | breathes, blinks every 3-6 s, looks around, stretches |

Several at once - the first in this order wins: call > sad > type > run > think > cheer > sleep > idle. A finished Edit or Bash stays on screen 1.2 s, so a 50 ms edit still shows. A "Нужен ты" wakes him even when frozen.

## How it is built

- **`machine()`** - the state from app events only: `task(t, now)`, `action(src, a, now, env)`, `delta(src, now, env)`, `frozen(v)`, `paused(v)`, `state(now) -> {name, detail}`. It never reads the clock itself, so tests pass time in.
- **`frame(state, t)`** - the picture as a pure function of the seconds spent in the state: poses, jumps and particles are computed from `t` (ballistic formulas), never integrated frame by frame. 30, 60 or 144 fps draw the same pixels at the same moment. Canvas 28×14 Clawd pixels, the body after the official icon (#D97757, arms out, slit eyes, four legs).
- **`anim(state)`** - the clock of one state: `step(dt)` (a long frame is clamped to 0.1 s), `pixels()`, `still()` (the pose for "Still" and `prefers-reduced-motion`).
- **`loop()`** - one `requestAnimationFrame` at a time; `visibilitychange` to hidden cancels it, showing the window starts it again. A hidden or minimised window draws nothing.
- **Painting** - whole device pixels per Clawd pixel (`round(2 × devicePixelRatio)`), `imageSmoothingEnabled = false`, colours of particles from the theme tokens.

## Settings

- Settings → Basics → Clawd: animated / still / hidden (`settings.ui.tabs.look.clawd`); the palette has the same three commands.
- `prefers-reduced-motion: reduce` - still, the pose still changes with the state.
- `?fps=1` - a frame counter next to him: frames per second and the cost of a frame in ms.

## The card

A click opens a card: his name, what he is doing now in words ("печатает cache.py в Claude Code") and his story (ru/en, the text of the spec, section "Clawd - история"). Esc or a click outside closes it.
