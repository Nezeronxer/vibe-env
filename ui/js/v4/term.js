// v4 terminal pane (stories 250-265, 298-302): pane kind 'term' of the registry - a PTY session of core::term in xterm 6.
// Output comes raw (ArrayBuffer) through a Tauri channel, an ack every ~100 KB (flow control); the session lives in the core,
// the pane only attaches: a closed window, a moved tab or a hidden page detach, the buffer comes back on attach.
// Also: env CLIs as terminals (298), ⌘⇧T, ⌘F, ⌘-click on links and path:line, copy on select, ⌘⌥I input to every pane,
// "Спросить агента" on a selection, the restart strip after `cli-updated`, windows (⌘N, ⌘⇧W, out and back), the session list
// in the palette (attach), the tmux prefix, the notice after a long command. Pure part (NSTerm) runs in selftest.
(function (root) {
  // ---------- pure ----------
  // tmux prefix mode (302): state 'idle' | 'armed'; key names come from keyName()
  const MODS = ['shift', 'ctrl', 'alt', 'meta'];
  function prefixStep(state, key, prefix) {
    if (!prefix) return { state: 'idle', eat: false, action: null };
    if (state !== 'armed') return key === prefix ? { state: 'armed', eat: true, action: null } : { state: 'idle', eat: false, action: null };
    if (MODS.includes(key)) return { state: 'armed', eat: true, action: null };
    const A = { '%': { type: 'split', dir: 'h' }, '"': { type: 'split', dir: 'v' }, left: { type: 'focus', dir: 'left' }, right: { type: 'focus', dir: 'right' },
      up: { type: 'focus', dir: 'up' }, down: { type: 'focus', dir: 'down' }, z: { type: 'zoom' }, c: { type: 'new' }, d: { type: 'detach' } };
    let action = key === prefix ? { type: 'send-prefix' } : A[key] || (/^\d$/.test(key) ? { type: 'goto', n: +key } : null);
    return { state: 'idle', eat: true, action };
  }
  // a KeyboardEvent -> 'ctrl+b' | '%' | 'left' | 'shift' …; letters by their key position, so a Russian layout gives the same name
  function keyName(e) {
    const k = e.key || '';
    const modOnly = { Shift: 'shift', Control: 'ctrl', Alt: 'alt', Meta: 'meta' }[k];
    if (modOnly) return modOnly;
    const arrow = /^Arrow(Up|Down|Left|Right)$/.exec(k);
    if (arrow) return arrow[1].toLowerCase();
    const letter = /^Key([A-Z])$/.exec(e.code || ''), digit = /^Digit(\d)$/.exec(e.code || '');
    const base = letter ? letter[1].toLowerCase() : digit && !e.shiftKey ? digit[1] : k === 'Escape' ? 'escape' : k.length === 1 ? k.toLowerCase() : k.toLowerCase();
    const mods = (e.ctrlKey ? 'ctrl+' : '') + (e.altKey ? 'alt+' : '') + (e.metaKey ? 'meta+' : '');
    return mods && (letter || digit) ? mods + base : mods && k.length !== 1 ? mods + base : base;
  }

  // path:line[:col] and file:// in a line of output (264); a match needs a line number or file://, so 12:30 and host:port stay text
  const REF = /(?<![\w:/.~\-])(file:\/\/)?((?:~|\.{1,2})?\/?[\w.@+\-]+(?:\/[\w.@+\-]+)*)(?::(\d+)(?::(\d+))?)?/g;
  function fileRefs(text) {
    const out = [];
    for (const m of String(text).matchAll(REF)) {
      const url = !!m[1], path = m[2], line = m[3] ? +m[3] : null, col = m[4] ? +m[4] : null;
      if (url ? path[0] !== '/' : !line || !(path.includes('/') || /\.\w+$/.test(path))) continue;
      out.push({ path, line, col, start: m.index, end: m.index + m[0].length });
    }
    return out;
  }
  const resolvePath = (p, cwd) => (p[0] === '/' || p[0] === '~' || !cwd ? p : cwd.replace(/\/+$/, '') + '/' + p.replace(/^\.\//, ''));

  // flow control: tell the core what xterm has written, once per `every` bytes
  function acker(every, send) {
    let written = 0, acked = 0;
    return { wrote(n) { written += n; if (written - acked >= every) { send(written - acked); acked = written; } } };
  }
  // the shell waits at its prompt: the text left of the cursor ends with a prompt sign and no full-screen program is on
  const looksIdle = (left, alt) => !alt && /[$%#>❯➜»λ]\s*$/.test(String(left || ''));

  // the CLI of an env in a terminal (251, 298): its binary, the model flag that env takes, resume where the CLI can (299)
  const MODEL_FLAG = { claude: '--model', cursor: '--model', codex: '-m', gemini: '-m', opencode: '-m', qwen: '-m' };
  const RESUME = { claude: ['--continue'], codex: ['resume', '--last'] };
  function cliArgv(env, model, resume) {
    const argv = [env.path || env.id];
    if (resume && RESUME[env.id]) argv.push(...RESUME[env.id]);
    if (model && MODEL_FLAG[env.id]) argv.push(MODEL_FLAG[env.id], model);
    return argv;
  }
  // a model choice -> the CLI argument, as core::models::resolve does: no flag for "Как в CLI", a pinned version's exact id, else latest
  function modelArg(choice, catalog, envId) {
    if (!choice || choice.family === 'default') return null;
    const f = ((catalog && catalog.families) || []).find((x) => x.id === choice.family);
    if (!f) return null;
    if (f.vendor === 'custom') return f.id;
    const exact = f.exact || {};
    if (choice.version && exact[choice.version]) return exact[choice.version];
    return envId !== 'claude' && f.versions && exact[f.versions[0]] ? exact[f.versions[0]] : f.id;
  }

  root.NSTerm = { prefixStep, keyName, fileRefs, resolvePath, acker, looksIdle, cliArgv, modelArg };
  if (typeof document === 'undefined' || !root.NS || !root.NS.h) return;

  // ---------- the window ----------
  const NS = root.NS;
  const { h, ic, envIcon, clear } = NS;
  const t = (k, p) => NS.t(k, p);
  const S = () => NS.store.s;
  const TS = () => root.NSTermSettings;
  const cfg = () => TS().fromSettings(S().settings);
  const P = () => root.NSPanes;
  const tauri = () => root.__TAURI__;
  const call = (cmd, args) => NS.api.call(cmd, args);
  const errText = (e) => NS.errHuman ? NS.errHuman(NS.api.errText(e)) : NS.api.errText(e);
  const toast = (m, bad) => NS.shell && NS.shell.toast(m, bad);
  const mac = () => /Mac/i.test(navigator.platform || '') || /[?&]platform=mac/.test(location.search);
  const modKey = (e) => (mac() ? e.metaKey : e.ctrlKey);
  const keyHint = (accel) => (mac() ? accel : accel.replace('Shift+Cmd', 'Ctrl+Shift').replace('Alt+Cmd', 'Ctrl+Alt').replace('Cmd', 'Ctrl'));
  const keyText = (accel) => window.NSLogic.keyLabel(keyHint(accel), mac() ? 'mac' : 'linux').join('');
  const ACK_EVERY = 100 * 1024;
  const query = new URLSearchParams(location.search);
  const WIN = query.get('window');                         // an extra window shows the one tab it was given

  const live = new Map();          // pane id -> session view
  const broadcast = new Set();     // tab ids with "ввод во все панели" on
  let restored = [];               // TermSaved of the last run (terms-restored)
  let adopt = null;                // fields for the next pane 'term' made by create() (attach, env CLI, a returned tab)

  const project = () => {
    const p = NS.store.project(NS.projects.current());
    return p || null;
  };
  const baseName = (p) => String(p || '').replace(/\/+$/, '').split('/').pop() || '~';
  const envOk = (e) => e && e.state !== 'missing';
  // F8: "Терминал · Claude Code" - the name of the tab and of the header says what runs in it
  const termName = (pane) => (pane.env && NS.store.env(pane.env) ? NS.store.env(pane.env).name : pane.title || '');
  const termTitle = (pane) => pane.ptitle || (termName(pane) ? t('term.tab', { name: termName(pane) }) : t('term.kind'));

  // ---------- one session in one pane ----------
  function View(el, pane, opts) {
    opts = opts || {};
    const L = cfg();
    const env = pane.env ? NS.store.env(pane.env) : null;
    const title = h('span', { class: 'pane-ttl', text: termTitle(pane) });
    const sub = h('small', { class: 't-cwd', text: pane.cwd ? String(pane.cwd).replace(/^\/(Users|home)\/[^/]+/, '~') : '' });
    const head = h('header', { class: 'pane-h t-head' }, h('span', { class: 't-glyph', 'aria-hidden': 'true', text: '>_' }), title, sub);   // F8: a terminal always shows >_, a chat shows the env
    const strips = h('div', { class: 't-strips' });
    const host = h('div', { class: 't-host' });
    const find = h('div', { class: 't-find', hidden: true });
    const dead = h('div', { class: 't-dead', role: 'status', hidden: true });
    const box = h('div', { class: 't-pane' }, head, strips, h('div', { class: 't-body' }, host, find, dead));
    el.append(box);

    const th = TS().themeOf(L);
    const theme = th ? TS().xterm(th) : (root.NSTheme.current ? root.NSTheme.current.xterm : root.NSTheme.xterm('dark', {}));
    const font = root.NSTheme.current ? root.NSTheme.current.font : root.NSTheme.xtermFont({});
    NS.setVars(box, { '--t-bg': theme.background });   // the header and the padding around the output take the ground of the terminal
    const term = new root.Terminal({ allowProposedApi: true, theme, fontFamily: L.font ? '"' + L.font + '", ' + font.fontFamily : font.fontFamily, fontSize: L.size || font.fontSize,
      lineHeight: L.line_height, cursorStyle: L.cursor, cursorBlink: L.blink, scrollback: L.scrollback, macOptionIsMeta: false, rightClickSelectsWord: false });
    const fit = new root.FitAddon.FitAddon();
    const search = new root.SearchAddon.SearchAddon();
    term.loadAddon(fit); term.loadAddon(search);
    term.loadAddon(new root.Unicode11Addon.Unicode11Addon());
    term.unicode.activeVersion = '11';
    term.loadAddon(new root.WebLinksAddon.WebLinksAddon((e, uri) => { if (modKey(e)) openLink(uri); }));
    term.open(host);
    let webgl = null;
    // WebGL draws fast; ligatures need the DOM renderer (the browser shapes the text); a lost context falls back to DOM
    function renderer(lig) {
      host.classList.toggle('lig', lig);
      if (lig && webgl) { webgl.dispose(); webgl = null; }
      if (!lig && !webgl) try { webgl = new root.WebglAddon.WebglAddon(); webgl.onContextLoss(() => { webgl.dispose(); webgl = null; }); term.loadAddon(webgl); } catch (e) { webgl = null; }
    }
    renderer(L.ligatures);
    setPad(L.padding);
    function setPad(px) { NS.setVars(host, { '--t-pad': px + 'px' }); }

    // ⌘-click on path:line[:col] opens the editor, on a file:// URL the file
    term.registerLinkProvider({ provideLinks(y, cb) {
      const line = term.buffer.active.getLine(y - 1);
      const text = line ? line.translateToString(true) : '';
      cb(fileRefs(text).map((r) => ({ text: text.slice(r.start, r.end), range: { start: { x: r.start + 1, y }, end: { x: r.end, y } },
        decorations: { underline: true, pointerCursor: true }, activate: (e) => { if (modKey(e)) openRef(r, pane.cwd); } })));
    } });
    // OSC 52 (a program writes the clipboard) only when the person turned it on (254)
    term.parser.registerOscHandler(52, (data) => {
      if (!cfg().osc52) return true;
      const b64 = String(data).split(';').pop();
      try { navigator.clipboard.writeText(new TextDecoder().decode(Uint8Array.from(atob(b64), (c) => c.charCodeAt(0)))).catch(() => {}); } catch (e) { /* not base64: ignored */ }
      return true;
    });
    // OSC 9 / 777: a program asks for attention -> the tab shows "ждёт"
    const ask = () => { if (!focused()) setState('wait'); return false; };
    term.parser.registerOscHandler(9, ask);
    term.parser.registerOscHandler(777, ask);
    term.onBell(() => { if (!focused()) setState('wait'); });

    // ⌘ keys belong to the app, not the shell; Linux copy / paste is Ctrl+Shift+C / V
    term.attachCustomKeyEventHandler((e) => {
      if (e.type !== 'keydown') return true;
      if (mac() && e.metaKey) {
        if ((e.code === 'KeyC') && term.hasSelection()) { copy(term.getSelection()); return false; }
        return false;
      }
      if (!mac() && e.ctrlKey && e.shiftKey && (e.code === 'KeyC' || e.code === 'KeyV')) {
        if (e.code === 'KeyC') copy(term.getSelection());
        else if (navigator.clipboard) navigator.clipboard.readText().then((s) => term.paste(s)).catch(() => {});
        return false;
      }
      return true;
    });
    term.onSelectionChange(() => { if (cfg().copy_select && term.hasSelection()) copy(term.getSelection()); });

    // ---------- the session ----------
    let id = pane.termId || null, running = false, attached = false, exitCode = null, state = 'talk', lastOut = 0, actTimer = 0;
    let cmdStart = 0, cmdText = '';
    const spec = () => ({ cwd: pane.cwd || null, argv: pane.argv || [], env: [], cols: term.cols, rows: term.rows, title: pane.title || '' });
    const note = (s, cls) => term.write('\r\n\x1b[' + (cls === 'bad' ? '31' : '2') + 'm' + s + '\x1b[0m\r\n');
    function setState(s) { if (state === s) return; state = s; head.dataset.state = s; markTabs(); }
    function activity() {
      lastOut = Date.now();
      if (state !== 'run') setState('run');
      clearTimeout(actTimer);
      actTimer = setTimeout(settle, 1500);
    }
    // output went quiet: the env CLI is free again; a shell back at its prompt ends the command (the long-command notice)
    function settle() {
      if (!running) return;
      setState('talk');
      if (cmdStart && idle()) {
        const secs = (Date.now() - cmdStart) / 1000, lim = cfg().notify_secs;
        if (lim && secs >= lim && !seen()) notify(t('term.done_cmd', { cmd: cmdText || t('term.kind') }));
        cmdStart = 0;
      }
    }
    function idle() {
      if (pane.argv && pane.argv.length) return false;            // an env CLI or a program: always ask
      const b = term.buffer.active, line = b.getLine(b.baseY + b.cursorY);
      return looksIdle(line ? line.translateToString(false, 0, b.cursorX) : '', b.type === 'alternate');
    }
    // one attach at a time: a show of the page (visibilitychange) while the first attach is on its way must not wipe the
    // screen and attach again - the buffer of the first one would land on a reset terminal or twice (live run v4, 260)
    let attaching = null;
    function attach() {
      if (attaching) return attaching;
      attaching = attachNow().finally(() => { attaching = null; });
      return attaching;
    }
    async function attachNow() {
      const T = tauri();
      if (!T || !T.core.Channel) return;
      const ch = new T.core.Channel();
      const a = acker(ACK_EVERY, (n) => call('term_ack', { id, bytes: n }).catch(() => {}));
      const me = id;
      ch.onmessage = (m) => {
        if (me !== id) return;
        if (m instanceof ArrayBuffer || ArrayBuffer.isView(m)) {
          const u = m instanceof ArrayBuffer ? new Uint8Array(m) : m;
          if (!u.length) return;
          activity();
          term.write(u, () => a.wrote(u.length));
          return;
        }
        const o = typeof m === 'string' ? JSON.parse(m) : m;
        if (o && 'exit' in o) exited(o.exit);
      };
      await call('term_attach', { id, channel: ch });
      attached = true;
      if (exitCode === null) running = true;
      await call('term_resize', { id, cols: term.cols, rows: term.rows }).catch(() => {});
    }
    async function start(fresh) {
      if (pane.env && !envOk(env)) { note(t('term.cli_missing', { env: env ? env.name : pane.env }), 'bad'); exitCode = -1; setState('fail'); return; }
      try {
        if (!id || fresh) {
          id = await call('term_open', { spec: spec() });
          pane.termId = id;
          refreshSessions();
        }
        exitCode = null; running = true; dead.hidden = true;
        await attach();
        setState('talk');
      } catch (e) {
        const msg = NS.api.errText(e);
        if (/term_not_found/.test(msg) && !fresh) return restore();
        running = false; exitCode = -1;
        note(t('term.pty_failed', { why: errText(e) }), 'bad');
        setState('fail');
      }
    }
    // the session is gone (the app was restarted): the text of the last run, then Enter starts it again (253)
    function restore() {
      const i = restored.findIndex((x) => x.cwd === (pane.cwd || x.cwd) && (!pane.title || x.title === pane.title));
      const saved = i >= 0 ? restored.splice(i, 1)[0] : null;
      id = null; running = false; exitCode = 0;
      if (saved && saved.text) term.write(String(saved.text).replace(/\r?\n/g, '\r\n'));
      if (saved) { note('── ' + t('term.restored') + ' ──'); note(t('term.again')); setState('done'); drawDead(0); }
      else start(true);
    }
    // F8: the process is gone - a plain card in the middle of the output with the two things to do
    function drawDead(code) {
      const name = pane.env && NS.store.env(pane.env) ? NS.store.env(pane.env).name : t('term.kind');
      clear(dead).append(h('div', { class: 't-dead-card' }, h('b', { text: t('term.dead_t', { name }) }), h('p', { text: t('term.dead_d', { code: code == null || code < 0 ? t('term.signal') : code }) }),
        h('div', { class: 't-dead-b' }, h('button', { type: 'button', class: 'btn primary sm t-dead-run', text: t('term.dead_run'), onclick: () => { restart(); term.focus(); } }),
          WIN ? null : h('button', { type: 'button', class: 'btn sm t-dead-x', text: t('term.dead_close'), onclick: () => { if (NS.tabs && NS.tabs.closePane) NS.tabs.closePane(pane.id); } }))));
      dead.hidden = false;
    }
    function exited(code) {
      if (!running && exitCode !== null) return;
      running = false; attached = false; exitCode = code == null ? -1 : code;
      drawDead(code);
      note(t('term.exited', { code: code == null ? t('term.signal') : code, close: keyText('Cmd+W') }));
      setState(code === 0 ? 'done' : 'fail');
      const lim = cfg().notify_secs;
      if (lim && cmdStart && (Date.now() - cmdStart) / 1000 >= lim && !seen()) notify(t('term.done_code', { cmd: cmdText || pane.title || '', code: exitCode }));
      cmdStart = 0;
      refreshSessions();
    }
    async function restart(argv) {
      const old = id;
      dead.hidden = true;
      if (old) { await call('term_close', { id: old }).catch(() => {}); }
      if (argv) pane.argv = argv;
      id = null;
      note('── ' + t('term.restarting') + ' ──');
      await start(true);
    }

    function input(d) {
      if (!running) { if (d === '\r' && exitCode !== null) restart(); return; }
      if (d === '\r' && !pane.argv?.length && idle()) {
        const b = term.buffer.active, line = b.getLine(b.baseY + b.cursorY);
        const text = line ? line.translateToString(true) : '';
        cmdText = text.replace(/^.*?[$%#>❯➜»λ]\s/, '').trim().slice(0, 60);
        cmdStart = cmdText ? Date.now() : 0;
      }
      const tab = tabOf(pane.id);
      if (tab && broadcast.has(tab)) { for (const v of live.values()) if (v.running() && tabOf(v.pane.id) === tab) v.write(d); return; }
      write(d);
    }
    const write = (d) => { if (id && running) call('term_write', { id, data: d }).catch(() => {}); };
    term.onData(input);
    term.onBinary((d) => write(d));
    term.onResize(({ cols, rows }) => { if (id && running) call('term_resize', { id, cols, rows }).catch(() => {}); });
    term.onTitleChange((s) => { pane.ptitle = s.slice(0, 60); title.textContent = termTitle(pane); });
    let fitT = 0;
    const ro = new ResizeObserver(() => { clearTimeout(fitT); fitT = setTimeout(() => { if (host.offsetWidth > 0 && host.offsetHeight > 0) try { fit.fit(); } catch (e) { /* hidden */ } }, 50); });
    ro.observe(host);
    try { fit.fit(); } catch (e) { /* not laid out yet */ }

    // ---------- search ⌘F ----------
    const fin = h('input', root.NSPalette.quiet({ type: 'text', class: 't-find-in', placeholder: t('term.find'), 'aria-label': t('term.find') }));
    const fbtn = (label, glyph, fn) => h('button', { type: 'button', class: 'pt', 'aria-label': label, title: label, text: glyph, onclick: fn });
    find.append(fin, fbtn(t('term.find_prev'), '↑', () => search.findPrevious(fin.value)), fbtn(t('term.find_next'), '↓', () => search.findNext(fin.value)), fbtn(t('term.find_close'), '×', closeFind));
    fin.addEventListener('input', () => search.findNext(fin.value, { incremental: true }));
    fin.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') { e.preventDefault(); if (e.shiftKey) search.findPrevious(fin.value); else search.findNext(fin.value); }
      else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); closeFind(); }
    });
    function openFind() { find.hidden = false; const s = term.getSelection(); if (s && !s.includes('\n')) fin.value = s; fin.focus(); fin.select(); }
    function closeFind() { find.hidden = true; search.clearDecorations(); term.clearSelection(); term.focus(); }

    // ---------- strips: input to every pane, CLI updated ----------
    let updateTo = null;
    function drawStrips() {
      clear(strips);
      const tab = tabOf(pane.id);
      if (tab && broadcast.has(tab)) strips.append(h('div', { class: 't-strip bcast', role: 'status' }, h('span', { text: t('term.bcast_on') }), h('kbd', { text: keyText('Alt+Cmd+I') })));
      if (updateTo) {
        strips.append(h('div', { class: 't-strip upd', role: 'status' }, h('span', { text: t('term.cli_updated', { env: env ? env.name : pane.env, v: updateTo }) }),
          h('button', { type: 'button', class: 'btn sm', text: t('term.restart_tab'), onclick: () => { updateTo = null; drawStrips(); restart(env ? cliArgv(env, modelArg(NS.store.envSettings(env.id).default_model, S().catalogs.get(env.id), env.id), true) : null); } }),
          h('button', { type: 'button', class: 'pt', 'aria-label': t('term.later'), title: t('term.later'), text: '×', onclick: () => { updateTo = null; drawStrips(); } })));
      }
      if (fit) setTimeout(() => { try { fit.fit(); } catch (e) { /* hidden */ } }, 0);
    }

    // ---------- the context menu: copy, ask the agent, delegate, paste, find, take out ----------
    let menu = null;
    const closeMenu = () => { if (menu) { menu.remove(); menu = null; document.removeEventListener('mousedown', onDoc, true); } };
    const onDoc = (e) => { if (menu && !menu.contains(e.target)) closeMenu(); };
    host.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      closeMenu();
      const sel = term.getSelection();
      const item = (label, fn, hint) => h('button', { type: 'button', role: 'menuitem', onclick: () => { closeMenu(); fn(); } }, h('span', { text: label }), hint ? h('kbd', { text: hint }) : null);
      menu = h('div', { class: 'menu t-menu', role: 'menu' }, sel ? [
        item(t('term.copy'), () => copy(sel), keyText('Cmd+C')),
        item(t('term.ask_agent'), () => toAgent(sel, false)),
        item(t('term.delegate'), () => toAgent(sel, true)),
      ] : [
        item(t('term.paste'), () => navigator.clipboard && navigator.clipboard.readText().then((s) => term.paste(s)).catch(() => {}), keyText('Cmd+V')),
        item(t('term.find'), openFind, keyText(mac() ? 'Cmd+F' : 'Shift+Cmd+F')),
        WIN ? null : item(t('term.detach'), () => detachToWindow(view)),
      ]);
      const r = box.getBoundingClientRect();
      NS.setVars(menu, { '--x': Math.min(e.clientX - r.left, r.width - 230) + 'px', '--y': Math.min(e.clientY - r.top, r.height - 140) + 'px' });
      box.append(menu);
      setTimeout(() => document.addEventListener('mousedown', onDoc, true), 0);
    });

    function focused() { return document.hasFocus() && !document.hidden && (box.contains(document.activeElement) || (P() && NS.tabs && NS.tabs.active && NS.tabs.active.id === pane.id)); }
    function seen() { return document.hasFocus() && !document.hidden && box.offsetParent !== null; }
    box.addEventListener('focusin', () => { if (state === 'wait') setState(running ? 'talk' : 'done'); });
    // a click on a pane out of focus redraws the layout (tabs.js moves the pane element), which blurs xterm: take the focus back
    box.addEventListener('pointerup', (e) => { if (!e.target.closest('input, button, .menu')) requestAnimationFrame(() => { if (!box.contains(document.activeElement)) term.focus(); }); });

    const view = {
      pane, term, box,
      running: () => running, write, id: () => id, state: () => state, env: () => env,
      focus: () => term.focus(), openFind, drawStrips, restart, fit: () => { try { fit.fit(); } catch (e) { /* hidden */ } },
      cliUpdated(to) { if (running) { updateTo = to; drawStrips(); } },
      applyLook() {
        const L2 = cfg(), th2 = TS().themeOf(L2), f2 = root.NSTheme.current ? root.NSTheme.current.font : font;
        term.options.theme = th2 ? TS().xterm(th2) : root.NSTheme.current.xterm;
        NS.setVars(box, { '--t-bg': term.options.theme.background });
        Object.assign(term.options, { fontFamily: L2.font ? '"' + L2.font + '", ' + f2.fontFamily : f2.fontFamily, fontSize: L2.size || f2.fontSize, lineHeight: L2.line_height,
          cursorStyle: L2.cursor, cursorBlink: L2.blink, scrollback: L2.scrollback });
        renderer(L2.ligatures); setPad(L2.padding);
        view.fit();
      },
      async detach() { if (attaching) await attaching.catch(() => {}); if (id && attached) { attached = false; await call('term_detach', { id }).catch(() => {}); } },
      async reattach() { if (!id || exitCode !== null || attached || attaching) return; term.reset(); await attach().catch(() => {}); },
      text: () => { const b = term.buffer.active, out = []; for (let i = 0; i < b.length; i++) { const l = b.getLine(i); if (l) out.push(l.translateToString(true)); } return out.join('\n'); },
      async close() { const x = id; id = null; running = false; if (x) await call('term_close', { id: x }).catch(() => {}); refreshSessions(); },
      idle, what: () => cmdText || pane.ptitle || (env ? env.name : pane.title) || t('term.kind'),
      dispose() { ro.disconnect(); closeMenu(); clearTimeout(actTimer); if (webgl) webgl.dispose(); term.dispose(); },
    };
    head.dataset.state = state;
    drawStrips();
    if (!opts.noStart) start(false);
    return view;
  }

  // ---------- helpers shared by the panes ----------
  function copy(s) { if (s && navigator.clipboard) navigator.clipboard.writeText(s).catch(() => {}); }
  function notify(text) {
    // shortcut: the window has no notification permission (capabilities); a toast until app gets a `notify` command
    try { if (root.Notification && root.Notification.permission === 'granted') { new root.Notification('Nightshift', { body: text }); return; } } catch (e) { /* no web notifications */ }
    toast(text);
  }
  function openLink(uri) {
    const u = String(uri);
    if (/^https?:\/\//i.test(u)) return call('open_path', { path: u }).catch((e) => toast(errText(e), true));
    if (/^file:\/\//i.test(u)) return call('open_path', { path: decodeURIComponent(u.replace(/^file:\/\//i, '')) }).catch((e) => toast(errText(e), true));
  }
  // path:line -> `code -g path:line:col` in a short-lived session (argv, no shell: nothing in the path is executed); no line -> open_path
  async function openRef(ref, cwd) {
    const abs = resolvePath(ref.path, cwd || (project() || {}).path);
    if (!ref.line) return call('open_path', { path: abs }).catch((e) => toast(errText(e), true));
    try {
      const id = await call('term_open', { spec: { cwd: null, argv: ['code', '-g', abs + ':' + ref.line + (ref.col ? ':' + ref.col : '')], env: [], cols: 80, rows: 24, title: 'editor' } });
      // shortcut: the result is read from term_list after 3 s; an `open_editor(path, line)` in app would make this direct
      setTimeout(async () => {
        const info = ((await call('term_list').catch(() => [])) || []).find((x) => x.id === id);
        if (info && !info.running && info.exit !== 0) { toast(t('term.no_editor')); call('open_path', { path: abs }).catch(() => {}); }
        call('term_close', { id }).catch(() => {});
      }, 3000);
    } catch (e) { toast(errText(e), true); }
  }
  // the selection to an agent session of this env (262): a new pane at the right, the text as a code block in its input
  function toAgent(text, delegate) {
    if (!NS.tabs) return;
    const f = NS.tabs.active;
    const env = (f && f.env) || S().lastEnv || ((S().envs.find((e) => e.state === 'ok') || {}).id);
    if (!env) return toast(t('term.no_env'), true);
    const body = (delegate ? t('term.delegate_text') : t('term.ask_text')) + '\n\n```\n' + text.replace(/\s+$/, '') + '\n```\n';
    NS.storage.set(window.NSLogic.draftKey(env, null), body);
    NS.tabs.split('h');
  }
  const tabOf = (paneId) => { const st = NS.tabs && NS.tabs.state; const tb = st && st.tabs.find((x) => P().findPane(x.root, paneId)); return tb ? tb.id : null; };
  const activeView = () => { const f = NS.tabs && NS.tabs.active; return f && f.kind === 'term' ? live.get(f.id) : WIN ? live.get('win') : null; };

  // the dot of a tab whose focused pane is a terminal says what the terminal does (298: работает / ждёт / свободна)
  const observed = new WeakSet();
  let marking = false;
  function markTabs() {
    if (marking || !NS.tabs) return;
    marking = true;
    const st = NS.tabs.state;
    for (const el of document.querySelectorAll('.tab[data-tab]')) {
      const tb = st.tabs.find((x) => x.id === el.dataset.tab), p = tb && P().findPane(tb.root, tb.focus), v = p && p.kind === 'term' ? live.get(p.id) : null;
      const dot = el.querySelector('.tdot'), icon = el.querySelector('.tab-ic');
      if (!v || !dot) continue;
      if (icon && !p.env && !icon.querySelector('.t-glyph')) icon.replaceChildren(h('span', { class: 't-glyph', 'aria-hidden': 'true', text: '>_' }));
      const s = { run: 'run', wait: 'wait', talk: 'talk', done: 'done', fail: 'fail' }[v.state()];
      if (!dot.classList.contains(s)) dot.className = 'tdot ' + s;
      const sr = el.querySelector('.sr-only');
      if (sr && sr.textContent !== t('term.st_' + s)) sr.textContent = t('term.st_' + s);
    }
    const strip = document.querySelector('.tabstrip');
    if (strip && !observed.has(strip)) { observed.add(strip); new MutationObserver(() => markTabs()).observe(strip, { childList: true, subtree: true }); }
    marking = false;
  }

  // ---------- the pane kind ----------
  function create() {
    const a = adopt; adopt = null;
    if (a) return a;
    const p = project();
    return { cwd: p ? p.path : null, title: p ? p.name : '' };
  }
  NS.registry.extend('paneKind', { kind: 'term', labelKey: 'term.kind', order: 2, create,
    sig: (pane) => pane.id,
    title: (pane) => termTitle(pane),
    // ⌘W / × on a pane with a running process asks first and the layout waits for the answer (253, live run v4)
    async beforeClose(pane) {
      const v = live.get(pane.id);
      if (!v || pane.moved || !v.id() || !v.running() || v.idle()) { closing.add(pane.id); return true; }
      const ok = await NS.projects.confirm(t('term.close_q', { what: v.what() }), t('term.close_text'), t('term.close_ok'));
      if (ok) closing.add(pane.id); else v.focus();
      return ok;
    },
    mount(el, pane) {
      if (WIN) return null;                // the hidden workspace of an extra window never takes the main window's sessions
      let v = live.get(pane.id);
      if (v) { v.dispose(); live.delete(pane.id); }
      v = View(el, pane);
      live.set(pane.id, v);
      setTimeout(markTabs, 0);
      return {
        focus: () => v.focus(),
        update(evt) { if (evt === 'focus-input') v.focus(); if (evt === 'focus') v.fit(); },
        destroy() {
          live.delete(pane.id);
          const still = NS.tabs && P().allPanes(NS.tabs.state).some((x) => x.pane.id === pane.id);
          const moved = pane.moved;
          v.dispose();
          if (still || moved) { closing.delete(pane.id); v.detach(); return; }
          if (closing.delete(pane.id)) { v.close(); return; }
          closeAsk(v, pane);        // removed without the question (not by ⌘W / ×): ask now, "Отмена" brings it back
        },
      };
    },
  });
  const closing = new Set();        // pane ids whose close was answered (or needed no question)
  // the pane was closed: an idle shell or an ended process goes quietly; anything else asks first (253); "Отмена" brings it back
  async function closeAsk(v, pane) {
    const id = v.id();
    if (!id) return;
    if (!v.running() || v.idle()) { v.close(); return; }
    await v.detach();
    const what = v.what();
    if (await NS.projects.confirm(t('term.close_q', { what }), t('term.close_text'), t('term.close_ok'))) { v.close(); return; }
    adopt = Object.assign({}, pane, { id: undefined, termId: id });
    delete adopt.id;
    NS.tabs.newTab('term');
  }

  // ---------- opening ----------
  function openTerm(fields, where) {
    adopt = fields || null;
    if (!NS.tabs) return;
    if (NS.shell && NS.shell.current && NS.shell.current.def.id !== 'tabs' && NS.shell.back) NS.shell.back();
    if (where === 'h' || where === 'v') NS.tabs.split(where, 'term'); else NS.tabs.newTab('term');
    adopt = null;
  }
  // a tab back from its own window: to its old place (beside the same neighbour, or its own tab at the old index)
  function returnTerm(s) {
    const fields = { termId: s.termId, env: s.env || null, argv: s.argv || [], cwd: s.cwd || null, title: s.title || '' };
    if (!NS.tabs || !NS.tabs.putBack || !s.place) return openTerm(fields);
    if (NS.shell && NS.shell.current && NS.shell.current.def.id !== 'tabs' && NS.shell.back) NS.shell.back();
    adopt = fields;
    NS.tabs.putBack('term', s.place);
    adopt = null;
  }
  function envTerm(envId, where, reuse) {
    const env = NS.store.env(envId);
    if (!env) return;
    // the bar button never leaves the click without an answer: a terminal of this env already open is brought to the front (F8)
    if (reuse && NS.tabs && NS.tabs.focusPane) {
      const have = P().allPanes(NS.tabs.state).find((x) => x.pane.kind === 'term' && x.pane.env === env.id && !x.pane.moved);
      if (have) { if (NS.shell && NS.shell.current && NS.shell.current.def.id !== 'tabs' && NS.shell.back) NS.shell.back(); NS.tabs.focusPane(have.pane.id); return; }
    }
    if (!envOk(env)) { toast(t('term.cli_missing', { env: env.name }), true); return; }
    const p = project();
    const model = modelArg(NS.store.envSettings(env.id).default_model, S().catalogs.get(env.id), env.id);
    openTerm({ env: env.id, argv: cliArgv(env, model), cwd: p ? p.path : null, title: env.name }, where);
  }
  // "Продолжить в терминале" for the chat in focus: the built-in tab, or the system terminal with the setting "Внешний терминал"
  async function continueChat() {
    const f = NS.tabs && NS.tabs.active;
    if (!f || f.kind !== 'agent' || f.chat === 'new') return toast(t('term.no_chat'), true);
    if (cfg().external) return call('open_chat_terminal', { id: f.chat }).catch((e) => toast(errText(e), true));
    // v6 G83: the core gives the words that continue the session in the chat's copy and opens the session (NS.chatShell)
    if (NS.chatShell) return NS.chatShell.openTerminal(f.chat);
  }

  // ---------- windows (260, 301) ----------
  async function detachToWindow(v) {
    if (!v) return toast(t('term.no_term'), true);
    const pane = v.pane;
    if (!v.id()) return;
    await v.detach();
    // the place in the layout rides along in the tabSpec, so "Вернуть" puts the pane back where it stood (live run v4, 260)
    const place = NS.tabs && NS.tabs.placeOf ? NS.tabs.placeOf(pane.id) : null;
    const tabSpec = { kind: 'term', termId: v.id(), env: pane.env || null, argv: pane.argv || [], cwd: pane.cwd || null, title: pane.title || '', place };
    try {
      await call('window_detach', { tabSpec });
      pane.moved = true;
      const tb = NS.tabs.state.tabs.find((x) => P().findPane(x.root, pane.id));
      if (tb && P().count(tb.root) === 1) NS.tabs.closeTab(tb.id);
      else {
        // closePane acts on the active tab: focus the pane first the way a click does (tabs.js listens for pointerdown on it)
        const el = v.box.closest('.pane');
        if (el) el.dispatchEvent(new Event('pointerdown', { bubbles: true }));
        NS.tabs.closePane(pane.id);
      }
    } catch (e) { v.reattach(); toast(errText(e), true); }
  }
  async function newWindow() {
    const p = project();
    try {
      const id = await call('term_open', { spec: { cwd: p ? p.path : null, argv: [], env: [], cols: 100, rows: 30, title: p ? p.name : '' } });
      await call('window_detach', { tabSpec: { kind: 'term', termId: id, cwd: p ? p.path : null, title: p ? p.name : '', argv: [] } });
    } catch (e) { toast(errText(e), true); }
  }
  // "Отключить окно" ⌘⇧W: sessions keep running in the core; the window goes
  async function offWindow() {
    for (const v of live.values()) await v.detach();
    try {
      const w = tauri() && tauri().window && tauri().window.getCurrentWindow();
      if (!w) throw new Error('no window api');
      await (WIN ? w.close() : w.hide());
    } catch (e) {
      // shortcut: the capabilities give the window no close/hide; app must add `window_off` (or core:window:allow-hide/close)
      for (const v of live.values()) v.reattach();
      toast(t('term.need_app'), true);
    }
  }

  // ---------- the session list in the palette (301): every running session not shown in this window ----------
  const listed = new Set();
  let listT = 0;
  function refreshSessions() {
    clearTimeout(listT);
    listT = setTimeout(async () => {
      if (!tauri()) return;
      let list = [];
      try { list = (await call('term_list')) || []; } catch (e) { return; }
      const here = new Set([...live.values()].map((v) => v.id()).filter(Boolean));
      const now = new Set();
      for (const x of list) {
        if (!x.running || here.has(x.id) || x.title === 'editor') continue;
        now.add(x.id);
        NS.registry.extend('command', { id: 'term.attach.' + x.id, group: 'cmd.g.term', order: 95, label: t('term.attach', { title: x.title || baseName(x.cwd), cwd: x.cwd || '~' }),
          run: () => openTerm({ termId: x.id, cwd: x.cwd || null, title: x.title || '', env: x.env_id || null, argv: [] }) });
      }
      for (const id of listed) if (!now.has(id)) NS.registry.extend('command', { id: 'term.attach.' + id, label: '', when: () => false, run() {} });
      listed.clear(); now.forEach((x) => listed.add(x));
    }, 150);
  }

  // ---------- keys: ⌘⇧T, ⌘F, ⌘⌥I, ⌘N, ⌘⇧W and the tmux prefix ----------
  let pstate = 'idle';
  function toggleBroadcast() {
    const tb = NS.tabs && NS.tabs.activeTab;
    if (!tb) return;
    if (broadcast.has(tb.id)) broadcast.delete(tb.id); else broadcast.add(tb.id);
    for (const v of live.values()) v.drawStrips();
  }
  function runPrefix(a, v) {
    if (a.type === 'send-prefix') { if (v) v.write('\x02'); return; }
    if (!NS.tabs) return;
    if (a.type === 'split') NS.tabs.split(a.dir, 'term');
    else if (a.type === 'focus') NS.tabs.focusDir(a.dir);
    else if (a.type === 'zoom') NS.tabs.zoom();
    else if (a.type === 'new') NS.tabs.newTab('term');
    else if (a.type === 'detach') offWindow();
    else if (a.type === 'goto') root.dispatchEvent(new KeyboardEvent('keydown', { code: 'Digit' + Math.min(9, a.n + 1), key: String(Math.min(9, a.n + 1)), metaKey: mac(), ctrlKey: !mac(), bubbles: true }));
  }
  root.addEventListener('keydown', (e) => {
    if (e.defaultPrevented || e.isComposing || document.querySelector('dialog[open]') || (NS.palette && NS.palette.isOpen)) return;
    if (e.target && e.target.closest && e.target.closest('.keyfield')) return;
    const v = activeView(), inTerm = !!(e.target && e.target.classList && e.target.classList.contains('xterm-helper-textarea'));
    const pre = cfg().prefix;
    if (pre && v && (inTerm || pstate === 'armed')) {
      const r = prefixStep(pstate, keyName(e), pre);
      pstate = r.state;
      if (r.eat) { e.preventDefault(); e.stopPropagation(); if (r.action) runPrefix(r.action, v); return; }
    }
    const mod = modKey(e), code = e.code || '';
    let hit = true;
    if (mod && e.shiftKey && !e.altKey && code === 'KeyT') openTerm();
    else if (mod && !e.altKey && code === 'KeyF' && (mac() ? !e.shiftKey : e.shiftKey) && v) v.openFind();
    else if (mod && e.altKey && !e.shiftKey && code === 'KeyI') toggleBroadcast();
    else if (mod && !e.altKey && !e.shiftKey && code === 'KeyN' && !WIN) newWindow();
    else if (mod && e.shiftKey && !e.altKey && code === 'KeyW') offWindow();
    else hit = false;
    if (hit) { e.preventDefault(); e.stopPropagation(); }
  }, true);

  // a hidden page stops confirming writes (WebKit slows its timers) and flow control would pause the processes: detach, attach on show
  document.addEventListener('visibilitychange', () => {
    for (const v of live.values()) { if (document.hidden) v.detach(); else v.reattach(); }
    if (!document.hidden) refreshSessions();
  });

  // ---------- palette commands, the bar, the pane header ----------
  const cmd = (c) => NS.registry.extend('command', c);
  cmd({ id: 'term.new', labelKey: 'cmd.term_new', group: 'cmd.g.term', keys: () => keyHint('Shift+Cmd+T'), order: 12, run: () => openTerm() });
  cmd({ id: 'term.split_r', labelKey: 'cmd.term_right', group: 'cmd.g.term', order: 13, run: () => openTerm(null, 'h') });
  cmd({ id: 'term.split_d', labelKey: 'cmd.term_down', group: 'cmd.g.term', order: 14, run: () => openTerm(null, 'v') });
  cmd({ id: 'term.find', labelKey: 'cmd.term_find', group: 'cmd.g.term', keys: () => keyHint(mac() ? 'Cmd+F' : 'Shift+Cmd+F'), order: 15, when: () => !!activeView(), run: () => setTimeout(() => activeView() && activeView().openFind(), 30) });
  cmd({ id: 'term.broadcast', labelKey: 'cmd.term_bcast', group: 'cmd.g.term', keys: () => keyHint('Alt+Cmd+I'), order: 16, run: toggleBroadcast });
  cmd({ id: 'term.detach', labelKey: 'cmd.term_detach', group: 'cmd.g.term', order: 17, when: () => !!activeView() && !WIN, run: () => detachToWindow(activeView()) });
  cmd({ id: 'term.window', labelKey: 'cmd.term_window', group: 'cmd.g.term', keys: () => keyHint('Cmd+N'), order: 18, when: () => !WIN, run: newWindow });
  cmd({ id: 'term.off', labelKey: 'cmd.term_off', group: 'cmd.g.term', keys: () => keyHint('Shift+Cmd+W'), order: 19, run: offWindow });
  cmd({ id: 'term.continue', labelKey: 'cmd.term_continue', group: 'cmd.g.term', order: 20, when: () => { const f = NS.tabs && NS.tabs.active; return !!f && f.kind === 'agent' && f.chat !== 'new'; }, run: continueChat });
  cmd({ id: 'term.settings', labelKey: 'cmd.term_settings', group: 'cmd.g.term', order: 63, run: () => NS.nav('settings', { env: S().lastEnv, section: 'terminal' }) });
  function envCommands() {
    S().envs.forEach((e, i) => cmd({ id: 'term.env.' + e.id, group: 'cmd.g.term', order: 41 + i / 100, get label() { return t('cmd.term_env', { env: e.name }); }, when: () => envOk(e), run: () => envTerm(e.id) }));
  }
  NS.store.on('envs', envCommands);
  root.addEventListener('ns-ready', () => { envCommands(); refreshSessions(); markTabs(); });
  root.addEventListener('focus', refreshSessions);

  // the bar: "<Среда> в терминале" for the env in focus (251)
  NS.registry.extend('bar', { id: 'term', side: 'right', order: 15, render(el) {
    const draw = () => {
      const env = NS.store.env(S().route.env || S().lastEnv);
      clear(el);
      if (!env || !envOk(env)) return;
      el.append(h('button', { type: 'button', class: 'bm t-barbtn', title: t('term.bar_btn_d', { env: env.name }) + ' · ' + keyText('Shift+Cmd+T') + ' ' + t('term.shell'), 'aria-label': t('term.bar_btn_d', { env: env.name }),
        onclick: () => envTerm(env.id, null, true) }, h('span', { class: 't-glyph', 'aria-hidden': 'true', text: '>_' }), h('span', { text: t('term.bar_btn') })));
    };
    draw();
    return { update: (evt) => { if (['route', 'focus', 'envs', 'settings'].includes(evt)) draw(); } };
  } });
  // "Вынести в окно" on the header of a terminal pane
  NS.registry.extend('paneHeader', { id: 'term-detach', kinds: ['term'], order: 50, render(el, pane) {
    if (WIN) return;
    el.append(h('button', { type: 'button', class: 'pt', 'aria-label': t('term.detach'), title: t('term.detach'), onclick: (e) => { e.stopPropagation(); detachToWindow(live.get(pane.id)); } },
      h('span', { class: 'pg pg-w', 'aria-hidden': 'true' })));
  } });

  // ---------- events ----------
  NS.api.listen('terms-restored', (list) => { restored = Array.isArray(list) ? list.slice() : []; }).catch(() => {});
  NS.api.listen('cli-updated', (pl) => { for (const v of live.values()) if (pl && v.pane.env === pl.env) v.cliUpdated(pl.to); }).catch(() => {});
  NS.api.listen('term-exit', () => refreshSessions()).catch(() => {});
  NS.api.listen('windows-changed', () => refreshSessions()).catch(() => {});
  NS.api.listen('window-return', (pl) => { const s = pl && pl.tabSpec; if (s && s.kind === 'term' && s.termId && !WIN) returnTerm(s); }).catch(() => {});
  // the global key ⌃` (265) shows the window; app then asks for a terminal in focus (event `focus-term`, see the report of task 23)
  if (tauri() && tauri().event) tauri().event.listen('focus-term', () => { const v = activeView(); if (v) v.focus(); else openTerm(); }).catch(() => {});
  // a new look (theme, font, terminal settings) reaches every open terminal
  if (root.NSTheme) root.NSTheme.onTheme(() => { for (const v of live.values()) v.applyLook(); });

  // ---------- an extra window: the one tab it was given, "Вернуть в главное окно" ----------
  if (WIN && tauri()) {
    const app = document.getElementById('app');
    if (app) app.hidden = true;
    document.body.classList.add('t-window');
    const holder = h('div', { class: 't-win pane focus' });
    document.body.append(holder);
    // the language and the look come with the settings (ns-ready): words made before it were English (live run v4, 260),
    // and a terminal made before it was rebuilt by the look that came later
    const ready = new Promise((res) => { root.addEventListener('ns-ready', res, { once: true }); setTimeout(res, 5000); });
    ready.then(() => call('window_tab', { id: WIN })).then((spec) => {
      const back = h('button', { type: 'button', class: 'btn sm', text: t('term.return') });
      const pane = { id: 'win', kind: 'term', termId: spec.termId, env: spec.env || null, argv: spec.argv || [], cwd: spec.cwd || null, title: spec.title || '' };
      const v = View(holder, pane);
      live.set('win', v);
      holder.querySelector('.t-head').append(back);
      back.onclick = async () => { await v.detach(); call('window_return', { tabSpec: Object.assign({}, spec, { termId: v.id() || spec.termId }) }).catch((e) => toast(errText(e), true)); };
      document.title = (pane.title || t('term.kind')) + ' - Nightshift';
      v.focus();
    }).catch((e) => holder.append(h('p', { class: 'hint', text: errText(e) })));
  }

  NS.term = { open: openTerm, envTerm, detachToWindow, newWindow, offWindow, refreshSessions, views: live, toggleBroadcast };
}(typeof window !== 'undefined' ? window : globalThis));
