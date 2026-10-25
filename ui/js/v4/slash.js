// v6 task 29: the chat as a shell of the CLI.
//  - NSSlash (pure, selftested): the "/word" under the caret, the sections of the menu for a query, the text after a choice.
//  - NS.slash.attach(...): the menu "/" over the input field - Скиллы, Команды, MCP, Моды of the env (`chat_shell`, cached for
//    the session), search by the typed name, ↑↓ Enter Tab Esc; a choice puts `/name ` into the field. Empty: «В среде нет
//    скиллов - открыть Каталог». Mods draw in the CLI: their section opens the chat in the terminal.
//  - NS.chatShell.openTerminal(chatId): "В терминале" - the same session in the real CLI in a terminal tab; without a session or a
//    CLI that can continue it, an explicit choice «Новый разговор в терминале».
(function (root) {
  const SECTIONS = ['skills', 'commands', 'mcp', 'mods'];
  const SHOW = 40;                                  // items of one section drawn at most (the rest: «и ещё N»)

  // the "/word" that ends right at the caret, at the start of the field or after a space; null = no menu
  function token(text, caret) {
    const before = String(text || '').slice(0, caret == null ? String(text || '').length : caret);
    const m = /(^|\s)\/([^\s/]{0,80})$/.exec(before);
    return m ? { start: before.length - m[2].length - 1, end: before.length, query: m[2] } : null;
  }
  // sections with the items that match the query (name first, then the description), the name matches on top
  function filter(inv, query) {
    const q = String(query || '').toLowerCase();
    return SECTIONS.map((key) => {
      const all = (inv && Array.isArray(inv[key]) ? inv[key] : []).filter((x) => x && x.name);
      const hit = all.map((x) => {
        const n = x.name.toLowerCase(), d = String(x.description || '').toLowerCase();
        const rank = !q ? 1 : n.startsWith(q) ? 0 : n.includes(q) ? 1 : d.includes(q) ? 2 : -1;
        return { x, rank };
      }).filter((r) => r.rank >= 0).sort((a, b) => a.rank - b.rank);
      return { key, total: all.length, items: hit.map((r) => r.x), insertable: key === 'skills' || key === 'commands' };
    });
  }
  // the field after a choice: the "/word" is replaced by "/name " and the caret stands after it
  function insert(text, tok, name) {
    const s = String(text || ''), put = '/' + name + ' ';
    const after = s.slice(tok.end).replace(/^ /, '');
    return { text: s.slice(0, tok.start) + put + after, caret: tok.start + put.length };
  }
  root.NSSlash = { token, filter, insert, SECTIONS, SHOW };

  const NS = root.NS;
  if (!NS || !root.document) return;
  const { h, clear } = NS;
  const t = (k, p) => NS.t(k, p);

  // ---------- what the env has and can do: one ask per env and project for the session ----------
  const cache = new Map();
  function shell(env, projectId) {
    const key = env + '|' + (projectId || '');
    if (!cache.has(key)) cache.set(key, NS.api.call('chat_shell', { env, projectId: projectId || null }).catch(() => { cache.delete(key); return null; }));
    return cache.get(key);
  }
  const srcText = (s) => {
    const m = /^plugin:(.+)$/.exec(String(s || ''));
    if (m) return t('slash.src.plugin', { name: m[1] });
    return window.NSI18n.has('slash.src.' + s) ? t('slash.src.' + s) : String(s || '');
  };

  // ---------- the menu "/" ----------
  // opts: {ta, form, envId, projectId(), onTerminal()|null}; returns {key(e) -> handled, update(), close(), isOpen()}
  function attach(opts) {
    const { ta, form, envId } = opts;
    const pop = h('div', { class: 'pop slash6', role: 'listbox', 'aria-label': t('slash.title'), hidden: true });
    form.append(pop);
    let tok = null, inv = null, loading = false, active = 0, opts2 = [];
    const isOpen = () => !pop.hidden;
    function close() { pop.hidden = true; tok = null; ta.removeAttribute('aria-activedescendant'); }
    function choose(item) {
      if (!tok) return;
      const r = insert(ta.value, tok, item.name);
      ta.value = r.text;
      ta.dispatchEvent(new Event('input'));
      ta.focus(); ta.setSelectionRange(r.caret, r.caret);
      close();
    }
    function draw() {
      clear(pop);
      const head = h('div', { class: 'sl-h' }, h('b', { text: t('slash.title') }), tok && tok.query ? h('span', { class: 'sl-q', text: '/' + tok.query }) : null, h('small', { text: t('slash.keys') }));
      pop.append(head);
      opts2 = [];
      if (loading && !inv) { pop.append(h('p', { class: 'sl-empty', text: t('slash.loading') })); return; }
      const secs = filter(inv || {}, tok ? tok.query : '');
      const none = secs.every((s) => !s.total);
      if (none) {
        pop.append(h('p', { class: 'sl-empty' }, h('button', { type: 'button', class: 'linkb', text: t('slash.empty'), onclick: () => { close(); NS.nav('catalog', { env: envId, kind: 'skill' }); } })));
        return;
      }
      if (secs.every((s) => !s.items.length)) { pop.append(h('p', { class: 'sl-empty', text: t('slash.none', { q: tok ? tok.query : '' }) })); return; }
      for (const s of secs) {
        if (!s.items.length) continue;
        const sub = s.key === 'mods' && opts.onTerminal ? h('button', { type: 'button', class: 'linkb sl-term', text: t('slash.mods_d'), onclick: () => { close(); opts.onTerminal(); } })
          : s.key === 'mods' ? h('small', { text: t('slash.mods_d') }) : s.key === 'mcp' ? h('small', { text: t('slash.mcp_d') }) : null;
        const list = h('ul', { class: 'sl-list' });
        for (const x of s.items.slice(0, SHOW)) {
          const body = [h('b', { text: (s.insertable ? '/' : '') + x.name }), x.description ? h('span', { class: 'sl-d', text: x.description }) : null, x.source ? h('small', { class: 'sl-s', text: srcText(x.source) }) : null];
          if (s.insertable) {
            const i = opts2.length;
            const li = h('li', { role: 'option', id: 'sl-' + envId + '-' + i, class: 'sl-o', 'aria-selected': 'false', onmousedown: (e) => e.preventDefault(), onclick: () => choose(x) }, body);
            opts2.push({ li, x });
            list.append(li);
          } else list.append(h('li', { class: 'sl-i' }, body));
        }
        if (s.items.length > SHOW) list.append(h('li', { class: 'sl-more', text: t('slash.more', { n: s.items.length - SHOW }) }));
        pop.append(h('section', { class: 'sl-sec sl-' + s.key }, h('h4', {}, h('span', { text: t('slash.' + s.key) }), h('i', { text: String(s.items.length) }), sub), list));
      }
      mark();
    }
    function mark() {
      active = Math.max(0, Math.min(active, opts2.length - 1));
      opts2.forEach((o, i) => o.li.setAttribute('aria-selected', String(i === active)));
      const cur = opts2[active];
      if (cur) { ta.setAttribute('aria-activedescendant', cur.li.id); cur.li.scrollIntoView({ block: 'nearest' }); }
    }
    let invFor;                                   // the project the inventory was read for: another project reads again
    async function load() {
      const pid = opts.projectId() || null;
      if ((inv && invFor === pid) || loading) return;
      loading = true;
      const r = await shell(envId, pid);
      loading = false; invFor = pid;
      inv = (r && r.inventory) || { skills: [], commands: [], mcp: [], mods: [] };
      if (isOpen()) draw();
    }
    // after every change of the field: open on "/" at the start or after a space, follow the typed name
    function update() {
      const next = token(ta.value, ta.selectionStart);
      if (!next) { if (isOpen()) close(); return; }
      const fresh = !tok || tok.start !== next.start;
      tok = next;
      if (fresh) active = 0;
      pop.hidden = false;
      draw();
      load();
    }
    function key(e) {
      if (!isOpen()) return false;
      if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); close(); return true; }
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { if (!opts2.length) return false; e.preventDefault(); active = (active + (e.key === 'ArrowDown' ? 1 : -1) + opts2.length) % opts2.length; mark(); return true; }
      if ((e.key === 'Enter' && !e.shiftKey && !e.isComposing) || (e.key === 'Tab' && !e.shiftKey)) {
        const cur = opts2[active];
        if (!cur) { if (e.key === 'Tab') return false; close(); return false; }
        e.preventDefault(); choose(cur.x); return true;
      }
      return false;
    }
    ta.addEventListener('input', update);
    ta.addEventListener('click', update);
    ta.addEventListener('blur', () => setTimeout(() => { if (!form.contains(document.activeElement)) close(); }, 120));
    return { key, update, close, isOpen, el: pop };
  }

  // ---------- "В терминале" ----------
  const ASK_NEW = ['no_session', 'cannot_resume', 'session_not_found', 'bad_session_id'];
  async function openTerminal(chatId, o) {
    o = o || {};
    const call = NS.api.call, toast = (m, bad) => NS.shell.toast(m, bad);
    try {
      const r = await call('chat_terminal', { id: chatId, fresh: !!o.fresh, cols: 100, rows: 30 });
      if (NS.term && NS.term.open) NS.term.open({ termId: r.termId, env: r.env, argv: r.argv || [], cwd: r.cwd, title: r.title, chat: chatId }, o.where || 'h');
      const env = NS.store.env(r.env);
      if (r.resumed && !r.checked) toast(t('shell.by_docs', { env: env ? env.name : r.env }));
      return r;
    } catch (e) {
      const why = NS.api.errText(e), k = (/^[a-z_]+/.exec(why) || [''])[0];
      if (!o.fresh && ASK_NEW.includes(k)) {
        const sure = await NS.projects.confirm(t('shell.new_term'), NS.errHuman(why).replace(/^./, (c) => c.toUpperCase()) + '. ' + t('shell.new_term_q'), t('shell.new_term'));
        return sure ? openTerminal(chatId, Object.assign({}, o, { fresh: true })) : null;
      }
      toast(NS.errHuman(why), true);
      return null;
    }
  }

  NS.slash = { attach, shell };
  NS.chatShell = { openTerminal, shell };
})(typeof window !== 'undefined' ? window : globalThis);
