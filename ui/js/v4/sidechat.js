// F12 #9: the side question. A narrow panel on the right of the chat (⌘; or the button in the header): a short read-only run of the env
// with the task's context. The answers live here only; nothing goes to the feed of the task and the executor is not woken.
(function () {
  const NS = window.NS;
  const { h, clear, richText } = NS;
  const t = (k, p) => NS.t(k, p);
  const talks = new Map();              // chat id -> [{q, a, bad}]: kept while the window lives, never sent anywhere

  function create(chatId) {
    const list = h('div', { class: 'sc-list', 'aria-live': 'polite' });
    const ta = h('textarea', { class: 'sc-in', rows: 2, placeholder: t('chatx.side_ph'), 'aria-label': t('chatx.side_ph'), spellcheck: false });
    const go = h('button', { type: 'button', class: 'btn sm primary sc-go', text: t('chatx.side_send') });
    const x = h('button', { type: 'button', class: 'btn sm sc-x', 'aria-label': t('chatx.side_close'), title: t('chatx.side_close'), text: '×' });
    const el = h('aside', { class: 'sidechat', role: 'complementary', 'aria-label': t('chatx.side'), hidden: true },
      h('header', { class: 'sc-h' }, h('b', { text: t('chatx.side') }), x), h('p', { class: 'sc-note', text: t('chatx.side_note') }), list, h('div', { class: 'sc-f' }, ta, go));
    let busy = false;
    const turns = () => talks.get(chatId) || talks.set(chatId, []).get(chatId);
    function draw() {
      clear(list);
      for (const m of turns()) {
        list.append(h('div', { class: 'sc-q' }, h('small', { text: t('chatx.side_you') }), h('div', {}, m.q)));
        list.append(h('div', { class: 'sc-a' + (m.bad ? ' bad' : '') }, m.pending ? h('span', { class: 'sc-wait', text: t('chatx.side_busy') }) : m.bad ? h('span', { text: t('chatx.side_fail', { why: NS.errHuman(m.a) }) }) : richText(m.a)));
      }
      list.scrollTop = list.scrollHeight;
    }
    async function ask() {
      const q = ta.value.trim();
      if (!q || busy) return;
      busy = true; go.disabled = true; ta.value = '';
      const turn = { q, a: '', pending: true }; turns().push(turn); draw();
      try { turn.a = await NS.api.call('side_ask', { id: chatId, text: q }); }
      catch (e) { turn.a = NS.api.errText(e); turn.bad = true; }
      turn.pending = false; busy = false; go.disabled = false; draw();
    }
    const close = () => { el.hidden = true; };
    const open = () => { el.hidden = false; draw(); ta.focus(); };
    go.onclick = ask; x.onclick = close;
    ta.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) { e.preventDefault(); ask(); }
      else if (e.key === 'Escape') { e.stopPropagation(); close(); }
    });
    return { el, open, close, toggle: () => (el.hidden ? open() : close()), isOpen: () => !el.hidden };
  }

  NS.sidechat = { create };
})();
