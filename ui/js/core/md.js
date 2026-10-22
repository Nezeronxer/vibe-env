// Markdown of agent text (story 228): NS.renderMarkdown(text) -> Node. Parsing is NSLive.parseMarkdown (pure, selftested);
// nodes are built with h(), never innerHTML. Links: only http(s), opened in the external browser through the core (open_path).
// NS.richText (used by every feed and card) now goes through here, so ** and backticks never reach the screen raw.
(function () {
  const NS = window.NS;
  const V = window.NSLive;
  const { h } = NS;

  function inlineNodes(list) {
    return list.map((n) => {
      if (n.t === 'text') return n.s;
      if (n.t === 'br') return h('br');
      if (n.t === 'code') return h('code', { text: n.s });
      if (n.t === 'b') return h('strong', {}, inlineNodes(n.c));
      if (n.t === 'i') return h('em', {}, inlineNodes(n.c));
      if (n.t === 'a') {
        const href = V.safeHref(n.href);
        if (!href) return inlineNodes(n.c);
        return h('a', { href, class: 'md-a', title: NS.t('act3.link') + ': ' + href, rel: 'noopener noreferrer',
          onclick: (e) => { e.preventDefault(); NS.api.call('open_path', { path: href }).catch((er) => NS.shell.toast(NS.errHuman(NS.api.errText(er)), true)); },
          // a middle click or a drag would open the page inside the app window: only the click above opens a link
          onauxclick: (e) => e.preventDefault(), onmousedown: (e) => { if (e.button === 1) e.preventDefault(); }, ondragstart: (e) => e.preventDefault() }, inlineNodes(n.c));
      }
      return '';
    });
  }

  function renderMarkdown(text) {
    const box = h('div', { class: 'rt md' });
    for (const b of V.parseMarkdown(text)) {
      if (b.type === 'p') box.append(h('p', {}, inlineNodes(b.inline)));
      else if (b.type === 'h') box.append(h('h' + (b.level + 2), { class: 'md-h' }, inlineNodes(b.inline)));     // h3-h5: smaller than the screen title
      else if (b.type === 'ul') box.append(h('ul', {}, b.items.map((it) => h('li', {}, inlineNodes(it)))));
      else if (b.type === 'ol') box.append(h('ol', { start: b.start }, b.items.map((it) => h('li', {}, inlineNodes(it)))));
      else if (b.type === 'code') box.append(h('pre', { class: 'md-code' }, NS.highlight(b.text, V.fenceLang(b.lang))));
      else if (b.type === 'table') box.append(h('pre', { class: 'md-table', text: b.text }));
      else if (b.type === 'quote') box.append(h('blockquote', {}, inlineNodes(b.inline)));
      else if (b.type === 'hr') box.append(h('hr'));
    }
    return box;
  }

  NS.renderMarkdown = renderMarkdown;
  NS.richText = renderMarkdown;
})();
