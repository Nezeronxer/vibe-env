// Clawd (stories 285-288, G46-G47): the pixel night watchman in the corner right of the tabs (registry point `tabsCorner`).
// Shape - the official Claude Code icon: a 7-pixel body, arms out, slit eyes, four legs, #D97757 (after ~/.claude/mods/clawd-run).
// Three parts, the first two pure (selftest runs them without a window):
//   machine() - what he is doing, from the events of the app (task status, task-action / chat-action, chat-delta, frozen);
//   anim(state) - the frame as a pure function of the time in the state: poses and particles are computed from t, never
//                 integrated frame by frame, so 30, 60 or 144 fps draw the same picture at the same moment;
//   loop() - one requestAnimationFrame, stopped while the window is hidden.
(function (root) {
  // ---------------- the state machine ----------------
  const PRIORITY = ['call', 'sad', 'type', 'run', 'think', 'cheer', 'sleep', 'idle'];
  const STATES = ['idle', 'think', 'type', 'run', 'call', 'cheer', 'sad', 'sleep'];
  const HOLD_MS = 1200;        // a finished Edit / Bash stays on screen this long: a 50 ms edit would otherwise never show
  const CHEER_MS = 2000;       // story 285: "радуется (задача влита - прыжок, 2 с)"
  const SAD_MS = 30000;
  const THINK_MS = 3000;       // a streamed chat answer counts as thinking this long after its last piece
  const ACTIVE = new Set(['preparing', 'working', 'checking', 'reviewing', 'merging']);
  const CALL = new Set(['needs_human', 'awaiting_outward', 'merge_blocked']);
  const TYPE = new Set(['edit', 'write', 'multiedit', 'notebookedit']);
  const base = (p) => String(p || '').split('/').pop();
  function kindOf(a) {
    const inp = (a && a.input) || {};
    const k = String(inp.kind || (typeof inp === 'object' ? Object.keys(inp)[0] : inp) || a.name || '').toLowerCase();
    const n = String((a && a.name) || '').toLowerCase();
    if (TYPE.has(k) || TYPE.has(n)) return 'type';
    if (k === 'bash' || n === 'bash') return 'run';
    return 'other';
  }
  const night = (t) => t.status === 'queued' && (t.start === 'night' || (t.start && typeof t.start === 'object' && 'night' in t.start));

  function machine() {
    const tasks = new Map();     // id -> task (last seen)
    const acts = new Map();      // source (task or chat id) -> Map(action id -> {kind, path, cmd, env, until})
    const deltas = new Map();    // chat id -> {until, env}
    let cheer = null, sad = null, frozen = false, paused = false;
    return {
      task(t, now) {
        const prev = tasks.get(t.id);
        if (prev && prev.status !== t.status) {
          if (t.status === 'done') cheer = { title: t.title, env: t.agent, until: now + CHEER_MS };
          if (t.status === 'failed') sad = { title: t.title, env: t.agent, until: now + SAD_MS };
        }
        tasks.set(t.id, t);
        if (!ACTIVE.has(t.status)) acts.delete(t.id);
      },
      forget(id) { tasks.delete(id); acts.delete(id); },
      action(src, a, now, env) {
        if (!a || !a.id) return;
        const list = acts.get(src) || new Map();
        acts.set(src, list);
        const inp = a.input || {};
        list.set(a.id, { kind: kindOf(a), path: base(inp.path), cmd: String(inp.command || '').slice(0, 28), env: env || (tasks.get(src) || {}).agent, until: a.done ? now + HOLD_MS : Infinity });
      },
      delta(src, now, env) { deltas.set(src, { until: now + THINK_MS, env }); },
      frozen(v) { frozen = !!v; },
      paused(v) { paused = !!v; },
      state(now) {
        const all = [...tasks.values()];
        const found = {};
        const c = all.find((t) => CALL.has(t.status));
        if (c) found.call = { title: c.title, env: c.agent };
        if (sad && now < sad.until) found.sad = sad;
        for (const [src, list] of acts) {
          for (const [id, x] of list) {
            if (x.until <= now) { list.delete(id); continue; }
            if (x.kind === 'other') { found.think = found.think || { env: x.env }; continue; }
            if (!found[x.kind]) found[x.kind] = x;
          }
          if (!list.size) acts.delete(src);
        }
        const busy = all.find((t) => ACTIVE.has(t.status));
        if (busy) found.think = found.think || { env: busy.agent };
        for (const [src, d] of deltas) { if (d.until <= now) deltas.delete(src); else found.think = found.think || { env: d.env }; }
        if (cheer && now < cheer.until) found.cheer = cheer;
        if (frozen || paused) found.sleep = { why: 'pause' };
        else if (all.some(night)) found.sleep = { why: 'night' };
        const name = PRIORITY.find((p) => found[p]) || 'idle';
        return { name, detail: found[name] || {} };
      },
    };
  }

  // ---------------- the picture ----------------
  const W = 28, H = 14, X0 = 8, FEET = 13;
  const BODY = '#d97757', BODY_HI = '#eb8a69', BODY_SLEEP = '#9c5a44', LID = '#a8543c', BLUSH = '#f28bb3', CAP = '#6f9cf5', CAP_TRIM = '#e8d5b0';
  // theme colours are resolved at draw time from the CSS tokens (@name -> var(--name))
  const CONFETTI = ['@human', '@ok', '@accent', '@bad', '@ansi-5'];
  const GLYPH = {
    '!': ['.#.', '.#.', '.#.', '...', '.#.'], z: ['####', '..#.', '.#..', '####'],
    '{': ['.##', '.#.', '#..', '.#.', '.##'], '}': ['##.', '.#.', '..#', '.#.', '##.'], ';': ['...', '.#.', '...', '.#.', '#..'],
    '<': ['..#', '.#.', '#..', '.#.', '..#'], '>': ['#..', '.#.', '..#', '.#.', '#..'], '/': ['..#', '..#', '.#.', '#..', '#..'],
    '=': ['...', '###', '...', '###', '...'], '+': ['...', '.#.', '###', '.#.', '...'],
  };
  const CODE = ['{', '}', ';', '<', '>', '/', '=', '+'];
  const hash = (n, s) => { const x = Math.sin(n * 12.9898 + (s || 0) * 78.233) * 43758.5453; return x - Math.floor(x); };
  const hex = (n) => n.toString(16).padStart(2, '0');
  function mix(a, b, k) {
    const p = (c, i) => parseInt(c.slice(1 + i * 2, 3 + i * 2), 16);
    return '#' + [0, 1, 2].map((i) => hex(Math.round(p(a, i) + (p(b, i) - p(a, i)) * k))).join('');
  }
  const LEGS = { stand: [2, 3, 5, 6], runA: [1, 2, 5, 6], runB: [2, 3, 6, 7], sit: null, tuck: null };

  // Clawd in a pose: x = left edge of the body box, feet = row of the legs (or of the bottom when sitting / in the air)
  function hero(dot, x, feet, p) {
    const legs = LEGS[p.legs];
    const bodyH = p.shape === 'squash' ? 3 : p.shape === 'stretch' ? 5 : 4;
    const top = feet - (legs ? 1 : 0) - bodyH + 1;
    const l = p.shape === 'squash' ? 0 : 1, r = p.shape === 'squash' ? 8 : 7;
    const eyeRow = top + (p.eyes === 'up' ? 0 : p.eyes === 'down' ? 2 : 1);
    const cols = p.eyes === 'right' ? [3, 6] : p.eyes === 'left' ? [2, 5] : [2, 6];
    for (let y = top; y < top + bodyH; y++)
      for (let c = l; c <= r; c++) {
        const eye = y === eyeRow && cols.includes(c);
        if (!eye) dot(x + c, y, p.color);
        else if (p.eyes === 'shut') dot(x + c, y, LID);
      }
    if (p.blush && p.shape !== 'squash') { dot(x + 1, top + 2, BLUSH); dot(x + 7, top + 2, BLUSH); }
    const arm = top + bodyH - 2;
    const draw = (pose, side) => {
      const c = side < 0 ? l - 1 : r + 1;
      const pts = pose === 'up' ? [[c, arm - 1], [c + side, arm - 2]]
        : pose === 'down' ? [[c, arm + 1], [c, arm + 2]]
        : pose === 'reach' ? [[c, arm], [c + side, arm]]
        : pose === 'tap' ? [[c, arm], [c + side, arm + 1]]
        : pose === 'chin' ? [[c, arm], [c, arm - 1]]
        : [[c, arm], [c, arm + 1]];
      for (const [px, py] of pts) dot(x + px, py, p.color);
    };
    draw(p.armL, -1);
    draw(p.armR, 1);
    if (legs) for (const c of legs) dot(x + c, feet, p.color);
    if (p.cap) {
      for (let c = l + 1; c < r; c++) dot(x + c, top - 1, CAP_TRIM);
      for (let c = l + 2; c < r - 2; c++) dot(x + c, top - 2, CAP);
      dot(x + l, top - 1, CAP); dot(x + l - 1, top, CAP_TRIM);
    }
    return { top, arm };
  }
  function glyph(dot, ch, x, y, color) {
    const g = GLYPH[ch];
    g.forEach((row, j) => [...row].forEach((c, i) => { if (c === '#') dot(x + i, y + j, color); }));
  }
  // particles of an emitter, all computed from t: spawned every `every` s, live `life` s; spec(k) -> {x, y, vx, vy, g, ...}
  function emit(t, every, life, spec, out) {
    for (let k = Math.max(0, Math.ceil((t - life) / every)); k * every <= t; k++) {
      const age = t - k * every;
      if (age < 0 || age >= life) continue;
      const s = spec(k);
      out(s, Math.round(s.x + s.vx * age), Math.round(s.y - (s.vy * age - 0.5 * (s.g || 0) * age * age)), age);
    }
  }

  // the frame of a state at t seconds in it -> [[x, y, colour]], whole pixels inside the canvas
  function frame(state, t) {
    const px = [];
    const dot = (x, y, c) => { if (x >= 0 && y >= 0 && x < W && y < H) px.push([x, y, c]); };
    const cyc = (period) => Math.floor(t / period);
    const breath = mix(BODY, BODY_HI, (Math.sin((2 * Math.PI * t) / 3) + 1) / 4);
    const pose = { shape: 'normal', eyes: 'front', armL: 'out', armR: 'out', legs: 'stand', color: BODY };
    let x = X0, feet = FEET;
    // blinks: one in each 4.5 s slot, at a slot-dependent moment -> 3…6 s apart (story 285)
    const slot = cyc(4.5), bt = slot * 4.5 + 1.5 * hash(slot, 3);
    const blink = t >= bt && t < bt + 0.14;
    if (state === 'idle') {
      pose.color = t > 0.001 ? breath : BODY;
      const c = t % 12;
      pose.eyes = c < 3 ? 'front' : c < 4.2 ? 'left' : c < 7 ? 'front' : c < 8.2 ? 'right' : 'front';
      if (c > 10.4 && c < 11.2) pose.armL = pose.armR = 'up';           // a stretch now and then
      if (blink) pose.eyes = 'shut';
      hero(dot, x, feet, pose);
    } else if (state === 'think') {
      x += Math.round(Math.sin((2 * Math.PI * t) / 1.6));                 // sways from foot to foot
      pose.eyes = 'up'; pose.armR = 'chin';
      if (blink) pose.eyes = 'shut';
      const { top } = hero(dot, x, feet, pose);
      const n = cyc(0.4) % 4;
      for (let i = 0; i < n; i++) dot(x + 10 + i * 2, top - 2 - i, '@ink2');
    } else if (state === 'type') {
      pose.eyes = cyc(1.1) % 3 === 2 ? 'front' : 'right';
      const beat = cyc(0.12) % 2;
      pose.armR = beat ? 'tap' : 'reach'; pose.armL = beat ? 'out' : 'up';
      hero(dot, x, feet, pose);
      for (let c = 9; c <= 13; c++) dot(x + c, feet - 1, '@ink3');        // the keyboard of a laptop
      for (let y = feet - 5; y <= feet - 2; y++) dot(x + 13, y, '@ink3');  // its screen, side on
      dot(x + 12, feet - 3 - (cyc(0.3) % 2), '@ok');                      // the glow of the code on it
      emit(t, 0.32, 1.1, (k) => ({ x: x + 11, y: feet - 7, vx: (hash(k, 1) - 0.3) * 5, vy: 4 + hash(k, 2) * 3, g: 9, ch: CODE[Math.floor(hash(k, 4) * CODE.length)] }),
        (s, gx, gy) => glyph(dot, s.ch, gx, gy, '@ok'));
    } else if (state === 'run') {
      const step = cyc(0.09) % 2;
      pose.legs = step ? 'runA' : 'runB'; pose.eyes = 'right';
      pose.armL = step ? 'up' : 'out'; pose.armR = step ? 'out' : 'up';
      hero(dot, x, feet - step, pose);
      emit(t, 0.12, 0.36, (k) => ({ x: x + 1, y: feet, vx: -7 - hash(k, 1) * 4, vy: 2 + hash(k, 2) * 2, g: 14 }), (s, gx, gy) => dot(gx, gy, '@ink3'));
      emit(t, 0.08, 0.24, (k) => ({ x: x - 1, y: feet - 2 - Math.floor(hash(k, 5) * 4), vx: -16, vy: 0 }), (s, gx, gy) => { dot(gx, gy, '@ink3'); dot(gx + 1, gy, '@ink3'); });
    } else if (state === 'call') {
      pose.blush = true;
      pose.armR = cyc(0.22) % 2 ? 'up' : 'reach';
      if (blink) pose.eyes = 'shut';
      const hop = cyc(1.2) !== cyc(1.2 - 0.18) ? 1 : 0;                    // a little hop every 1.2 s
      const { top } = hero(dot, x, feet - hop, pose);
      glyph(dot, '!', x + 3, top - 7 + (cyc(0.25) % 2), '@human');
    } else if (state === 'cheer') {
      // 1 s cycle: crouch, a ballistic jump (v 16 px/s, g 48 px/s²), landing squash; confetti at each take-off
      const c = t % 1, V = 16, G = 48, up = 0.12, air = (2 * V) / G;
      pose.armL = pose.armR = 'up'; pose.eyes = 'up'; pose.blush = true;
      let lift = 0;
      if (c < up) { pose.shape = 'squash'; pose.armL = pose.armR = 'out'; pose.eyes = 'front'; }
      else if (c < up + air) { const a = c - up; lift = Math.round(V * a - 0.5 * G * a * a); pose.legs = 'tuck'; if (a < air / 2) pose.shape = 'stretch'; }
      else if (c < up + air + 0.1) { pose.shape = 'squash'; pose.armL = pose.armR = 'out'; }
      hero(dot, x, feet - lift, pose);
      emit(t, 1, 0.95, () => ({}), (s, gx, gy, age) => {
        const k = cyc(1);
        for (let i = 0; i < 9; i++) {
          const vx = (hash(k * 9 + i, 1) - 0.5) * 18, vy = 6 + hash(k * 9 + i, 2) * 8, a = age;
          dot(Math.round(x + 4 + vx * a), Math.round(feet - 6 - (vy * a - 10 * a * a)), CONFETTI[i % CONFETTI.length]);
        }
      });
    } else if (state === 'sad') {
      pose.legs = 'sit'; pose.eyes = 'down'; pose.color = mix(BODY, BODY_SLEEP, 0.45);
      pose.armL = pose.armR = cyc(2.6) % 4 === 3 ? 'reach' : 'out';
      const { top } = hero(dot, x, feet, pose);
      emit(t, 2.4, 0.9, () => ({ x: x + 2, y: top + 3, vx: 0, vy: 0, g: 9 }), (s, gx, gy) => { if (gy > s.y) dot(gx, gy, '@ansi-6'); });
    } else if (state === 'sleep') {
      pose.eyes = 'shut'; pose.color = mix(BODY_SLEEP, BODY, (Math.sin((2 * Math.PI * t) / 4) + 1) / 8); pose.cap = true; pose.legs = 'sit';
      pose.armL = pose.armR = cyc(2) % 2 ? 'out' : 'down';
      const { top } = hero(dot, x, feet, pose);
      emit(t, 1.1, 2.6, () => ({ x: x + 9, y: top - 4, vx: 1.6, vy: 1.4 }), (s, gx, gy) => glyph(dot, 'z', gx, gy, '@ink3'));
    }
    return px;
  }
  // a still pose per state (prefers-reduced-motion, "Статично"): the moment that reads best
  const STILL = { idle: 0.5, think: 0.85, type: 0.36, run: 0.05, call: 0.3, cheer: 0.3, sad: 1.2, sleep: 1.4 };
  function anim(state) {
    let t = 0;
    const q = () => Math.round(t * 1000) / 1000;      // whole ms: summed float steps land on the same pixels
    return {
      step(dt) { t += Math.min(Math.max(dt, 0), 0.1); },
      time: () => q(),
      pixels: () => frame(state, q()),
      still: () => frame(state, STILL[state] || 0),
    };
  }

  // ---------------- the loop ----------------
  // o = {raf, caf, hidden() -> bool, onVisibility(fn), frame(ts)}; one rAF at a time, none while hidden
  function loop(o) {
    let id = 0, running = false;
    const tick = (ts) => { id = 0; if (!running || o.hidden()) return; o.frame(ts); id = o.raf(tick); };
    o.onVisibility(() => {
      if (o.hidden()) { if (id) o.caf(id); id = 0; }
      else if (running && !id) id = o.raf(tick);
    });
    return {
      start() { if (running) return; running = true; if (!o.hidden()) id = o.raf(tick); },
      stop() { running = false; if (id) o.caf(id); id = 0; },
      running: () => running,
    };
  }

  root.NSClawd = { PRIORITY, STATES, HOLD_MS, CHEER_MS, W, H, machine, anim, frame, loop, kindOf };

  // ---------------- the corner of the tabs (only in a window) ----------------
  if (typeof document === 'undefined' || !root.NS || !root.NS.registry) return;
  const NS = root.NS;
  const t = (k, p) => NS.t(k, p);
  const PX = 2;                                   // css px per Clawd pixel in the corner
  const fpsOn = new URLSearchParams(location.search).get('fps') === '1';
  const reduceQ = root.matchMedia ? root.matchMedia('(prefers-reduced-motion: reduce)') : null;
  const envName = (id) => { const e = id && NS.store.env(id); return e ? e.name : id || 'Claude Code'; };
  const lookMode = () => { const T = root.NSTheme; return (T && T.current && T.current.look && T.current.look.clawd) || 'animate'; };

  function words(st) {
    const d = st.detail || {}, env = envName(d.env);
    switch (st.name) {
      case 'type': return d.path ? t('clawd.s.type', { path: d.path, env }) : t('clawd.s.type0', { env });
      case 'run': return d.cmd ? t('clawd.s.run', { cmd: d.cmd, env }) : t('clawd.s.run0', { env });
      case 'think': return t('clawd.s.think', { env });
      case 'call': return t('clawd.s.call', { title: d.title || '' });
      case 'cheer': return t('clawd.s.cheer', { title: d.title || '' });
      case 'sad': return t('clawd.s.sad', { title: d.title || '' });
      case 'sleep': return t(d.why === 'night' ? 'clawd.s.night' : 'clawd.s.sleep');
      default: return t('clawd.s.idle');
    }
  }

  // integer device pixels per Clawd pixel: crisp at any devicePixelRatio
  function painter(canvas, px) {
    const g = canvas.getContext('2d');
    let scale = 1, colors = new Map();
    const fit = () => {
      const dpr = root.devicePixelRatio || 1;
      scale = Math.max(1, Math.round(px * dpr));
      canvas.width = W * scale; canvas.height = H * scale;
      NS.setVars(canvas, { '--cw': (W * scale) / dpr + 'px', '--ch': (H * scale) / dpr + 'px' });
      g.imageSmoothingEnabled = false;
      colors = new Map();
    };
    const color = (c) => {
      if (c[0] !== '@') return c;
      if (!colors.has(c)) colors.set(c, getComputedStyle(canvas).getPropertyValue('--' + c.slice(1)).trim() || '#888');
      return colors.get(c);
    };
    fit();
    return {
      fit, retheme: () => { colors = new Map(); },
      draw(pixels) {
        g.clearRect(0, 0, canvas.width, canvas.height);
        let last = '';
        for (const [x, y, c] of pixels) { const col = color(c); if (col !== last) { g.fillStyle = col; last = col; } g.fillRect(x * scale, y * scale, scale, scale); }
      },
    };
  }

  function mount(el) {
    const { h, clear } = NS;
    const S = NS.store.s;
    const m = machine();
    const now = () => Date.now();
    for (const tk of S.tasks.values()) m.task(tk, now());
    m.frozen(S.frozen); m.paused(S.settings && S.settings.paused);

    const canvas = h('canvas', { class: 'clawd-cv', 'aria-hidden': 'true' });
    const btn = h('button', { type: 'button', class: 'clawd', 'aria-label': t('clawd.open'), 'aria-haspopup': 'dialog', 'aria-expanded': 'false' }, canvas);
    const fpsEl = fpsOn ? h('output', { class: 'clawd-fps', 'aria-live': 'off' }) : null;
    el.classList.add('clawd-corner');
    el.append(btn, fpsEl || '');
    const paint = painter(canvas, PX);

    let st = m.state(now()), a = anim(st.name), last = 0, mode = '', card = null;
    const refresh = () => {
      const next = m.state(now());
      if (next.name !== st.name) { st = next; a = anim(st.name); if (mode !== 'animate') paint.draw(a.still()); }
      else st = next;
      if (card) card.update();
    };
    let stateTimer = 0, fpsN = 0, fpsT = 0, cost = 0;
    const lp = loop({
      raf: (f) => root.requestAnimationFrame(f), caf: (id) => root.cancelAnimationFrame(id), hidden: () => document.hidden,
      onVisibility: (f) => document.addEventListener('visibilitychange', f),
      frame(ts) {
        const t0 = performance.now();
        a.step(last ? (ts - last) / 1000 : 0); last = ts;
        if (ts - stateTimer > 100) { stateTimer = ts; refresh(); }    // timed changes (hold, 2 s cheer) without an event
        paint.draw(a.pixels());
        if (fpsEl) {
          cost += performance.now() - t0; fpsN++;
          if (ts - fpsT >= 500) { fpsEl.textContent = t('clawd.fps', { fps: Math.round((fpsN * 1000) / (ts - fpsT)), ms: (cost / fpsN).toFixed(2) }); root.__clawdFps = (fpsN * 1000) / (ts - fpsT); root.__clawdMs = cost / fpsN; fpsN = 0; cost = 0; fpsT = ts; }
        }
      },
    });
    document.addEventListener('visibilitychange', () => { last = 0; });
    let stillTimer = null;
    function apply() {
      const want = lookMode();
      const next = want === 'hide' ? 'hide' : want === 'static' || (reduceQ && reduceQ.matches) ? 'static' : 'animate';
      if (next === mode) return;
      mode = next;
      el.hidden = mode === 'hide';
      el.classList.toggle('still', mode === 'static');
      clearInterval(stillTimer); stillTimer = null;
      if (mode === 'animate') { last = 0; lp.start(); return; }
      lp.stop();
      if (mode === 'static') { paint.draw(a.still()); stillTimer = setInterval(() => { if (!document.hidden) refresh(); }, 500); }
      if (mode === 'hide' && card) card.close();
    }
    const offs = [];
    const on = (name, fn) => offs.push(NS.store.on(name, (p) => { fn(p); refresh(); }));
    on('task', (tk) => tk && m.task(tk, now()));
    on('taskremoved', (tk) => tk && m.forget(tk.id));
    on('taskaction', (pl) => pl && m.action(pl.id, pl.action, now()));
    on('chataction', (pl) => { if (!pl) return; const c = NS.store.findChat(pl.id); m.action(pl.id, pl.action, now(), c && c.env); });
    on('chatdelta', (pl) => { if (!pl) return; const c = NS.store.findChat(pl.id); m.delta(pl.id, now(), c && c.env); });
    on('frozen', (v) => m.frozen(v));
    on('settings', () => { m.paused(S.settings && S.settings.paused); apply(); });
    if (root.NSTheme && root.NSTheme.onTheme) root.NSTheme.onTheme(() => { paint.retheme(); apply(); if (mode === 'static') paint.draw(a.still()); });
    if (reduceQ && reduceQ.addEventListener) reduceQ.addEventListener('change', apply);
    root.addEventListener('resize', () => { paint.fit(); if (mode === 'static') paint.draw(a.still()); });

    // click: the card - name, what he does in words, his story (stories 287-288)
    function openCard() {
      const big = h('canvas', { class: 'clawd-big', 'aria-hidden': 'true' });
      const now1 = h('p', { class: 'clawd-now' });
      const box = h('div', { class: 'clawd-card', role: 'dialog', 'aria-label': t('clawd.name') },
        h('div', { class: 'clawd-head' }, big, h('div', {}, h('h2', { text: t('clawd.name') }), h('small', { text: t('clawd.role') }))),
        now1,
        h('h3', { text: t('clawd.story_h') }),
        h('p', { class: 'clawd-story', text: t('clawd.story') }),
        h('button', { type: 'button', class: 'btn sm clawd-x', text: t('clawd.close'), onclick: () => close() }));
      const bp = painter(big, 5);
      let shown = '';
      const update = () => { if (shown !== st.name) { shown = st.name; bp.draw(frame(st.name, STILL[st.name] || 0)); } now1.textContent = words(st); };
      const onDoc = (e) => { if (!box.contains(e.target) && !btn.contains(e.target)) close(); };
      const onKey = (e) => { if (e.key === 'Escape') { close(); btn.focus(); } };
      function close() { box.remove(); document.removeEventListener('pointerdown', onDoc, true); document.removeEventListener('keydown', onKey); btn.setAttribute('aria-expanded', 'false'); card = null; }
      el.append(box);
      update();
      document.addEventListener('pointerdown', onDoc, true);
      document.addEventListener('keydown', onKey);
      btn.setAttribute('aria-expanded', 'true');
      box.querySelector('.clawd-x').focus();
      card = { update, close };
    }
    btn.addEventListener('click', () => (card ? card.close() : openCard()));
    btn.title = words(st);
    offs.push(NS.store.on('task', () => { btn.title = words(st); }));
    apply();
    return {
      update() {},
      destroy() { lp.stop(); clearInterval(stillTimer); offs.forEach((f) => f && f()); if (card) card.close(); clear(el); },
    };
  }
  NS.registry.extend('tabsCorner', { id: 'clawd', mount });
})(typeof window !== 'undefined' ? window : globalThis);
