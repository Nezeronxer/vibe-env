// Echo terminal for the browser without Tauri (task 23): term_* commands of core::term, a raw channel (ArrayBuffer), window_*.
// A tiny fake shell: echo, ls, seq N, sleep-free `exit N`, an env CLI banner. ?term=claude opens Claude Code in a terminal at start;
// NSMockTerm.emit('cli-updated', {env:'claude', from:'2.1.4', to:'2.1.5'}) imitates an event of app.
(function () {
  const T = window.__TAURI__;
  if (!T || !window.__MOCK__) return;
  const enc = new TextEncoder();
  const sessions = new Map();
  let n = 0;
  class Channel { constructor() { this.onmessage = null; } }
  T.core.Channel = Channel;
  const subs = {};
  const listen0 = T.event.listen;
  T.event.listen = (name, fn) => { (subs[name] = subs[name] || []).push(fn); return listen0(name, fn); };
  const emit = (name, payload) => (subs[name] || []).forEach((fn) => fn({ payload }));

  const C = { dim: '\x1b[2m', red: '\x1b[31m', green: '\x1b[32m', yellow: '\x1b[33m', blue: '\x1b[34m', mag: '\x1b[35m', cyan: '\x1b[36m', bold: '\x1b[1m', off: '\x1b[0m' };
  const base = (p) => String(p || '~').replace(/\/+$/, '').split('/').pop();
  const prompt = (s) => (s.cli ? C.mag + '> ' + C.off : C.blue + base(s.cwd) + C.off + ' ' + C.mag + 'main' + C.off + ' % ');
  function out(s, text) {
    s.buf += text;
    if (s.buf.length > 200000) s.buf = s.buf.slice(-200000);
    if (s.ch && s.ch.onmessage) { const b = enc.encode(text).buffer; setTimeout(() => s.ch && s.ch.onmessage && s.ch.onmessage(b), 0); }
  }
  function banner(s) {
    const bin = base(s.argv[0]);
    const model = s.argv.includes('--model') ? s.argv[s.argv.indexOf('--model') + 1] : s.argv.includes('-m') ? s.argv[s.argv.indexOf('-m') + 1] : 'default';
    out(s, C.bold + '✻ ' + bin + C.off + C.dim + ' 2.1.4 · ' + model + ' · ' + s.cwd + C.off + '\r\n' + (s.argv.includes('--continue') ? C.dim + '  resumed the last conversation\r\n' + C.off : '')
      + C.dim + '  Type a request, /help for commands' + C.off + '\r\n\r\n' + prompt(s));
  }
  const LS = ['Cargo.toml', 'README.md', C.blue + 'src' + C.off, C.blue + 'tests' + C.off, C.green + 'run.sh' + C.off, 'invoices.sql'];
  function run(s, line) {
    const [cmd, ...args] = line.trim().split(/\s+/);
    if (s.cli) {
      if (!cmd) return out(s, prompt(s));
      return out(s, C.dim + '· Read src/invoices/routes.py' + C.off + '\r\n' + 'Нашёл: страница считается от 1, а курсор - от 0. Исправить?\r\n\r\n' + prompt(s));
    }
    if (!cmd) return out(s, prompt(s));
    if (cmd === 'ls') return out(s, LS.join('  ') + '\r\n' + prompt(s));
    if (cmd === 'echo') return out(s, args.join(' ') + '\r\n' + prompt(s));
    if (cmd === 'pwd') return out(s, s.cwd + '\r\n' + prompt(s));
    if (cmd === 'clear') return out(s, '\x1b[2J\x1b[H' + prompt(s));
    if (cmd === 'seq') { const k = Math.min(100000, +args[0] || 10); let x = ''; for (let i = 1; i <= k; i++) x += i + '\r\n'; return out(s, x + prompt(s)); }
    if (cmd === 'cargo') return out(s, C.green + '   Compiling' + C.off + ' invoices v0.3.1\r\n' + 'error[E0308]: mismatched types\r\n  --> src/routes.rs:42:17\r\n' + C.yellow + 'warning' + C.off + ': unused variable `cursor` at src/page.rs:7\r\n' + prompt(s));
    if (cmd === 'exit') return finish(s, +args[0] || 0);
    if (cmd === 'claude' || cmd === 'codex') { s.cli = true; s.argv = [cmd]; return banner(s); }
    return out(s, 'zsh: command not found: ' + cmd + '\r\n' + prompt(s));
  }
  function finish(s, code) {
    s.running = false; s.exit = code;
    if (s.ch && s.ch.onmessage) setTimeout(() => s.ch && s.ch.onmessage({ exit: code }), 0);
    emit('term-exit', { id: s.id, code });
  }
  const cmds = {
    term_open: ({ spec }) => {
      if (spec.argv && spec.argv[0] === 'code') return 'ed' + (++n);
      const id = 't' + (++n);
      const s = { id, cwd: spec.cwd || '/Users/dev', argv: spec.argv || [], title: spec.title || '', running: true, exit: null, buf: '', line: '', ch: null, cli: !!(spec.argv && spec.argv.length) };
      sessions.set(id, s);
      if (s.cli) banner(s); else out(s, C.dim + 'Last login: Thu Oct 23 09:14 on ttys004' + C.off + '\r\n' + prompt(s));
      return id;
    },
    term_attach: ({ id, channel }) => {
      const s = sessions.get(id);
      if (!s) throw 'term_not_found';
      s.ch = channel;
      const b = enc.encode(s.buf).buffer;
      setTimeout(() => { channel.onmessage && channel.onmessage(b); if (!s.running) channel.onmessage && channel.onmessage({ exit: s.exit }); }, 0);
      return null;
    },
    term_write: ({ id, data }) => {
      const s = sessions.get(id);
      if (!s || !s.running) return null;
      for (const ch of data) {
        if (ch === '\r') { out(s, '\r\n'); const l = s.line; s.line = ''; run(s, l); }
        else if (ch === '\x7f') { if (s.line) { s.line = s.line.slice(0, -1); out(s, '\b \b'); } }
        else if (ch === '\x03') { s.line = ''; out(s, '^C\r\n' + prompt(s)); }
        else if (ch >= ' ') { s.line += ch; out(s, ch); }
      }
      return null;
    },
    term_resize: () => null, term_ack: () => null,
    term_detach: ({ id }) => { const s = sessions.get(id); if (s) s.ch = null; return null; },
    term_close: ({ id }) => { sessions.delete(id); return null; },
    term_list: () => [...sessions.values()].map((s) => ({ id: s.id, title: s.title, cwd: s.cwd, pid: 1000 + +s.id.slice(1), running: s.running, exit: s.exit, env_id: null, cli_version: null })),
    window_detach: () => { emit('windows-changed', null); return 'w' + (++n); },
    window_tab: () => { throw 'window_not_found'; },
    window_return: () => null,
  };
  const invoke0 = T.core.invoke;
  T.core.invoke = (cmd, args) => (cmds[cmd] ? new Promise((res, rej) => { try { res(cmds[cmd](args || {})); } catch (e) { rej(e); } }) : invoke0(cmd, args));
  window.NSMockTerm = { emit, sessions };

  // ?term=1 | ?term=<env>: a terminal tab at start; ?termsplit=<env>: a shell with the env CLI split on the right
  const q = new URLSearchParams(location.search);
  window.addEventListener('ns-ready', () => setTimeout(() => {
    const NS = window.NS;
    if (!NS.term) return;
    const what = q.get('term'), sp = q.get('termsplit');
    if (what) { if (what === '1') NS.term.open(); else NS.term.envTerm(what); }
    if (sp) setTimeout(() => NS.term.envTerm(sp, 'h'), 300);
    if (q.get('cliupd')) setTimeout(() => emit('cli-updated', { env: q.get('cliupd'), from: '2.1.4', to: '2.1.5' }), 900);
  }, 300));
})();
