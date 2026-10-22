// Syntax highlight (story 226): NS.highlight(code, lang) -> <code> with token spans. Tokens come from NSLive.tokenize (pure, selftested);
// text lands through text nodes only. Unknown language = plain monospace text.
(function () {
  const NS = window.NS;
  const V = window.NSLive;
  const CLS = { kw: 't-kw', str: 't-str', com: 't-com', num: 't-num' };

  function fill(el, code, lang) {
    for (const tk of V.tokenize(code, lang)) {
      if (!tk.k) el.append(document.createTextNode(tk.s));
      else { const sp = document.createElement('span'); sp.className = CLS[tk.k]; sp.textContent = tk.s; el.append(sp); }
    }
    return el;
  }
  function highlight(code, lang) {
    const el = document.createElement('code');
    el.className = 'hl' + (lang ? ' lang-' + lang : '');
    return fill(el, code, lang);
  }

  Object.assign(NS, { highlight, highlightInto: fill });
})();
