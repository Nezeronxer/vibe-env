// Temporary terminal tab of task 18 (criteria 1, 3, 4) until task 23 builds the real one: `?term=1` opens a shell over
// the whole window, `?window=<id>` shows the tab a window was given. `&bench=<file>` runs the speed check and writes
// its numbers to <file> through the shell. Does nothing without these parameters or outside Tauri.
(function () {
  const q = new URLSearchParams(location.search);
  const T = window.__TAURI__;
  if (!T || !(q.get('term') || q.get('window'))) return;
  const { invoke, Channel } = T.core;
  const csp = [];
  document.addEventListener('securitypolicyviolation', (e) => csp.push(e.violatedDirective + ' ' + (e.blockedURI || '')));

  const app = document.getElementById('app');
  if (app) app.hidden = true;
  const bar = document.createElement('div');
  const box = document.createElement('div');
  Object.assign(bar.style, { position: 'fixed', top: '0', right: '0', zIndex: '2', display: 'flex', gap: '4px', padding: '4px' });
  Object.assign(box.style, { position: 'fixed', inset: '0', background: '#111' });
  document.body.append(box, bar);

  const term = new Terminal({ scrollback: 10000, fontFamily: 'Menlo, monospace', fontSize: 13, allowProposedApi: true });
  const fit = new FitAddon.FitAddon();
  term.loadAddon(fit);
  term.loadAddon(new Unicode11Addon.Unicode11Addon());
  term.unicode.activeVersion = '11';
  term.loadAddon(new WebLinksAddon.WebLinksAddon());
  term.open(box);
  let webgl = false;
  try { term.loadAddon(new WebglAddon.WebglAddon()); webgl = true; } catch (_) { /* DOM renderer stays */ }
  fit.fit();

  let id = null, written = 0, acked = 0, onBytes = null;
  const ACK_EVERY = 100 * 1024;
  async function attach(tid) {
    id = tid;
    written = acked = 0;
    const ch = new Channel();
    ch.onmessage = (m) => {
      if (m instanceof ArrayBuffer) {
        const u = new Uint8Array(m);
        if (onBytes) onBytes(u);
        term.write(u, () => {
          written += u.length;
          if (written - acked >= ACK_EVERY) { invoke('term_ack', { id: tid, bytes: written - acked }); acked = written; }
        });
      } else if (m && 'exit' in m) term.write('\r\n[exit ' + m.exit + ']\r\n');
    };
    await invoke('term_attach', { id: tid, channel: ch });
    await invoke('term_resize', { id: tid, cols: term.cols, rows: term.rows });
  }
  // a hidden page stops parsing (WebKit throttles its timers) and the acks with it: detach, take the buffer again when shown
  document.addEventListener('visibilitychange', () => {
    if (!id || q.get('bench')) return;
    if (document.hidden) invoke('term_detach', { id });
    else { term.reset(); attach(id); }
  });
  term.onData((d) => id && invoke('term_write', { id, data: d }));
  term.onResize(({ cols, rows }) => id && invoke('term_resize', { id, cols, rows }));
  let t = 0;
  new ResizeObserver(() => { clearTimeout(t); t = setTimeout(() => fit.fit(), 50); }).observe(box);

  const button = (label, fn) => { const b = document.createElement('button'); b.textContent = label; b.onclick = fn; bar.append(b); };
  const winId = q.get('window');
  if (winId) {
    button('Return', () => invoke('window_return', { tabSpec: { kind: 'term', termId: id } }));
    invoke('window_tab', { id: winId }).then((tab) => attach(tab.termId));
  } else {
    button('Detach', async () => { await invoke('window_detach', { tabSpec: { kind: 'term', termId: id } }); term.write('\r\n[moved to a window]\r\n'); });
    T.event.listen('window-return', (e) => attach(e.payload.tabSpec.termId));
    invoke('term_open', { spec: { cols: term.cols, rows: term.rows } }).then(attach).then(() => q.get('bench') && setTimeout(bench, 1500));
  }

  // ---------- speed check (criterion 1) ----------
  const dec = new TextDecoder();
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const send = (s) => invoke('term_write', { id, data: s });
  // runs `cmd`, collects the output until the marker line, returns {text, ms, renderedMs}
  async function run(cmd, n, keep = true) {
    let text = '';
    const t0 = performance.now();
    let got = 0;
    const done = new Promise((res) => { onBytes = (u) => { got += u.length; text = keep ? text + dec.decode(u, { stream: true }) : (text + dec.decode(u, { stream: true })).slice(-64); if (text.includes('__END' + n + '__\r\n')) res(); }; });
    await send(cmd + '; echo __END$((' + n + '+0))__\r');
    await done;
    const ms = performance.now() - t0;
    onBytes = null;
    await new Promise((r) => term.write('', r)); // xterm has parsed everything before this point
    return { text, ms: Math.round(ms), renderedMs: Math.round(performance.now() - t0), bytes: got };
  }
  const seqOk = (text, n) => {
    const nums = text.split('\r\n').filter((l) => /^\d+$/.test(l)).map(Number);
    return nums.length === n && nums.every((v, i) => v === i + 1);
  };
  async function echoMs(times) {
    const out = [];
    for (let i = 0; i < times; i++) {
      const p = new Promise((res) => { onBytes = () => res(); });
      const t0 = performance.now();
      await send('x');
      await p;
      out.push(performance.now() - t0);
      onBytes = null;
      await send('\x7f');
      await sleep(30);
    }
    out.sort((a, b) => a - b);
    return { median: +out[out.length >> 1].toFixed(1), max: +out[out.length - 1].toFixed(1) };
  }
  async function bench() {
    const r = { webgl, cols: term.cols, rows: term.rows };
    // a second session only writes the progress, so a stuck stage is visible from outside
    const logId = await invoke('term_open', { spec: { cols: 80, rows: 24, argv: ['/bin/sh'] } });
    const log = (m) => invoke('term_write', { id: logId, data: "echo '" + document.visibilityState + ' ' + String(m).replace(/'/g, '').slice(0, 300) + "' >> '" + q.get('bench') + ".log'\n" });
    log('r.echoIdle '+Math.round(performance.now()));
    r.echoIdle = await echoMs(20);
    // Ctrl-C while `yes` floods the window: until the prompt is back
    log('await send '+Math.round(performance.now()));
    await send('command yes\r');
    let flood = 0, last = 0, after = 0, t0 = 0;
    onBytes = (u) => { last = performance.now(); if (t0) after += u.length; else flood += u.length; };
    await sleep(1500);
    // the typed-ahead line would be flushed with the interrupt, so the end is the last byte before 300 ms of quiet
    t0 = performance.now();
    log('ctrl-c '+Math.round(performance.now()));
    await send('\x03');
    log('sent '+Math.round(performance.now())+' flood '+flood+' written '+written+' acked '+acked);
    while (performance.now() - Math.max(last, t0) < 300) await sleep(20);
    onBytes = null;
    log('quiet '+Math.round(performance.now())+' after '+after+' written '+written+' acked '+acked);
    r.ctrlC = { floodBytes: flood, bytesAfter: after, lastByteMs: Math.round(Math.max(last, t0) - t0) };
    log('const y = await '+Math.round(performance.now()));
    const y = await run('yes | head -n 200000', 1);
    r.yes = { ms: y.ms, renderedMs: y.renderedMs, lines: y.text.split('\r\n').filter((l) => l === 'y').length };
    log('const s = await '+Math.round(performance.now()));
    const s = await run('seq 1 200000', 2);
    r.seq = { ms: s.ms, renderedMs: s.renderedMs, ok: seqOk(s.text, 200000), bytes: s.bytes };
    log('await run '+Math.round(performance.now()));
    await run('seq 1 999999 > "$TMPDIR/ns-bench.txt"', 3);
    log('const c = await '+Math.round(performance.now()));
    const c = await run('command cat "$TMPDIR/ns-bench.txt"', 4); // `command`: cat is often an alias (bat)
    r.cat = { ms: c.ms, renderedMs: c.renderedMs, ok: seqOk(c.text, 999999), bytes: c.bytes };
    log('r.echoAfter '+Math.round(performance.now()));
    r.echoAfter = await echoMs(10);
    // memory: the same flood again and again, read from outside (`ps`) between the rounds
    for (let i = 0; i < 5; i++) { log('round ' + i); await run('command yes | head -n 400000', 10 + i, false);  }
    r.csp = csp;
    await send("printf '%s\\n' '" + JSON.stringify(r) + "' > '" + q.get('bench') + "'; rm -f \"$TMPDIR/ns-bench.txt\"\r");
  }
})();
