// DOM helpers. Text is always set through textContent, never innerHTML; the CSP forbids style="..." and inline handlers,
// so dynamic sizes go through setVars (element.style.setProperty is allowed).
(function () {
  const NS = (window.NS = window.NS || {});
  const SVGNS = 'http://www.w3.org/2000/svg';
  const TEXT_ATTRS = new Set(['spellcheck', 'autocorrect', 'autocapitalize', 'autocomplete']);

  function h(tag, attrs, ...kids) {
    const el = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs || {})) {
      // enumerated attributes: `spellcheck: false` must reach the DOM as "false", and WebKit's `autocorrect` property is
      // a boolean that turns the string "off" into true - these go through setAttribute as text
      if (TEXT_ATTRS.has(k) && v != null) { el.setAttribute(k, String(v)); continue; }
      if (v == null || v === false) continue;
      if (k === 'class') el.className = v;
      else if (k === 'text') el.textContent = v;
      else if (k === 'style') throw new Error('inline style is not allowed (CSP): use NS.setVars');
      else if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
      else if (k === 'dataset') Object.assign(el.dataset, v);
      else if (k in el && !k.includes('-') && k !== 'list') el[k] = v;
      else el.setAttribute(k, v === true ? '' : v);
    }
    append(el, kids);
    return el;
  }
  function append(el, kids) {
    for (const c of kids.flat(Infinity)) if (c != null && c !== false) el.append(c.nodeType ? c : document.createTextNode(String(c)));
    return el;
  }
  const clear = (el) => { el.replaceChildren(); return el; };
  const setVars = (el, vars) => { for (const [k, v] of Object.entries(vars)) el.style.setProperty(k, v); return el; };

  // sprite symbol from index.html (<use href="#i-name">)
  function ic(name, cls) {
    const s = document.createElementNS(SVGNS, 'svg');
    s.setAttribute('class', 'ic' + (cls ? ' ' + cls : ''));
    s.setAttribute('aria-hidden', 'true');
    const u = document.createElementNS(SVGNS, 'use');
    u.setAttribute('href', '#' + name);
    s.append(u);
    return s;
  }

  // role mark: shape + colour + (always) a text label next to it
  function roleIcon(key) {
    const s = ic('i-' + key, 'rl r-' + key); s.setAttribute('class', 'rl r-' + key); return s;
  }

  // original vendor marks (icons/SOURCES.md); [file, mono]: a mono mark is black and is inverted in the dark theme (CSS img.mono)
  // crush: no official SVG mark found (icons/SOURCES.md), so the neutral mark
  const ENV_ICON = { claude: ['claude-code', 0], codex: ['openai', 1], gemini: ['gemini', 0], opencode: ['opencode', 1], cursor: ['cursor', 1], qwen: ['qwen', 1],
    kimi: ['kimi', 1], copilot: ['copilot', 1], amp: ['amp', 0], goose: ['goose', 1], droid: ['factory', 1], aider: ['aider', 1], grok: ['grok', 1] };
  const VENDOR_ICON = { anthropic: ['claude', 0], claude: ['claude', 0], openai: ['openai', 1], google: ['gemini', 0], gemini: ['gemini', 0],
    cursor: ['cursor', 1], opencode: ['opencode', 1], qwen: ['qwen', 1], alibaba: ['qwen', 1] };
  function logo(entry, cls) {
    if (!entry) return ic('e-custom', cls);   // unknown vendor, local model, "Своя модель", custom env: neutral app mark (G21.1)
    const img = h('img', { class: 'ic logo' + (entry[1] ? ' mono' : '') + (cls ? ' ' + cls : ''), src: 'icons/' + entry[0] + '.svg', alt: '' });
    img.setAttribute('aria-hidden', 'true');
    return img;
  }
  const envIcon = (envId, cls) => logo(ENV_ICON[envId], cls);
  const vendorIcon = (vendor, cls) => logo(VENDOR_ICON[String(vendor || '').toLowerCase()], cls);

  // minimal text blocks: paragraphs, "- " lists, `code`. Everything lands through text nodes.
  function richText(text) {
    const box = h('div', { class: 'rt' });
    for (const block of String(text).split(/\n{2,}/)) {
      const lines = block.split('\n');
      if (lines.every((l) => /^\s*[-*] /.test(l))) box.append(h('ul', {}, lines.map((l) => h('li', {}, inline(l.replace(/^\s*[-*] /, ''))))));
      else box.append(h('p', {}, inline(block)));
    }
    return box;
  }
  function inline(s) {
    return s.split(/(`[^`]+`)/).map((part) => (/^`[^`]+`$/.test(part) ? h('code', { text: part.slice(1, -1) }) : part));
  }

  Object.assign(NS, { h, append, clear, setVars, ic, roleIcon, envIcon, vendorIcon, richText });
})();
