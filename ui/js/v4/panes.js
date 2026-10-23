// v4 layout (stories 246, 258-259, G37): every tab holds a tree of panes like a tiling WM.
//   tree = { split: { dir: 'h' | 'v', ratio, a, b } } | { pane: { id, kind: 'agent' | 'term' | …, ...fields } }
//   'h' = side by side (split right, ⌘D), 'v' = stacked (split down, ⌘⇧D); ratio = share of `a`, 0.1…0.9.
//   workspace = { v: 2, tabs: [{ id, root: tree, focus: pane id, zoom: bool }], active: tab id }
// Pure functions only (selftest runs them in node); tabs.js draws them. An agent pane: { id, kind: 'agent', env, chat }.
(function (root) {
  const MAX_PANES = 6, MIN_RATIO = 0.1;
  const isPane = (n) => !!(n && n.pane);
  const clampRatio = (r) => Math.max(MIN_RATIO, Math.min(1 - MIN_RATIO, Number(r) || 0.5));

  // ---------- one tree ----------
  const leaves = (n) => (!n ? [] : isPane(n) ? [n.pane] : leaves(n.split.a).concat(leaves(n.split.b)));
  const count = (n) => leaves(n).length;
  const findPane = (n, id) => leaves(n).find((p) => p.id === id) || null;
  function mapPane(n, id, fn) {
    if (!n) return n;
    if (isPane(n)) return n.pane.id === id ? fn(n) : n;
    const a = mapPane(n.split.a, id, fn), b = mapPane(n.split.b, id, fn);
    return a === n.split.a && b === n.split.b ? n : { split: Object.assign({}, n.split, { a, b }) };
  }
  // the pane `id` becomes a split: itself on the left/top, `fresh` on the right/bottom; refused at MAX_PANES
  function splitPane(n, id, dir, fresh) {
    if (count(n) >= MAX_PANES || !findPane(n, id) || !fresh || findPane(n, fresh.id)) return n;
    return mapPane(n, id, (leaf) => ({ split: { dir: dir === 'v' ? 'v' : 'h', ratio: 0.5, a: leaf, b: { pane: fresh } } }));
  }
  // the sibling of the removed pane takes its parent's place; null when the last pane goes
  function removePane(n, id) {
    if (!n) return null;
    if (isPane(n)) return n.pane.id === id ? null : n;
    const a = removePane(n.split.a, id), b = removePane(n.split.b, id);
    if (!a) return b;
    if (!b) return a;
    return a === n.split.a && b === n.split.b ? n : { split: Object.assign({}, n.split, { a, b }) };
  }
  // a split is addressed by its path from the root: '' the root, 'a' / 'b' its children, 'ab' …
  function nodeAt(n, path) { let x = n; for (const c of path) { if (!x || isPane(x)) return null; x = x.split[c]; } return x; }
  function setRatio(n, path, ratio) {
    const at = nodeAt(n, path);
    if (!at || isPane(at)) return n;
    const go = (x, rest) => {
      if (!rest.length) return { split: Object.assign({}, x.split, { ratio: clampRatio(ratio) }) };
      return { split: Object.assign({}, x.split, { [rest[0]]: go(x.split[rest[0]], rest.slice(1)) }) };
    };
    return go(n, path.split(''));
  }
  // rectangles of the panes in a unit square (gaps are the drawing's business)
  function rects(n, box, out) {
    box = box || { x: 0, y: 0, w: 1, h: 1 }; out = out || {};
    if (!n) return out;
    if (isPane(n)) { out[n.pane.id] = box; return out; }
    const r = clampRatio(n.split.ratio);
    if (n.split.dir === 'h') { rects(n.split.a, { x: box.x, y: box.y, w: box.w * r, h: box.h }, out); rects(n.split.b, { x: box.x + box.w * r, y: box.y, w: box.w * (1 - r), h: box.h }, out); }
    else { rects(n.split.a, { x: box.x, y: box.y, w: box.w, h: box.h * r }, out); rects(n.split.b, { x: box.x, y: box.y + box.h * r, w: box.w, h: box.h * (1 - r) }, out); }
    return out;
  }
  // the pane next to `id` in a direction ('left' | 'right' | 'up' | 'down'): it must touch that edge and overlap across it;
  // of several, the one whose middle is closest to the middle of `id`
  function neighbor(n, id, dir) {
    const R = rects(n), me = R[id], E = 1e-9;
    if (!me) return null;
    const horiz = dir === 'left' || dir === 'right';
    let best = null, bestD = Infinity;
    for (const [pid, r] of Object.entries(R)) {
      if (pid === id) continue;
      const touches = dir === 'left' ? Math.abs(r.x + r.w - me.x) < E : dir === 'right' ? Math.abs(me.x + me.w - r.x) < E
        : dir === 'up' ? Math.abs(r.y + r.h - me.y) < E : Math.abs(me.y + me.h - r.y) < E;
      if (!touches) continue;
      const lo = horiz ? Math.max(r.y, me.y) : Math.max(r.x, me.x), hi = horiz ? Math.min(r.y + r.h, me.y + me.h) : Math.min(r.x + r.w, me.x + me.w);
      if (hi - lo <= E) continue;
      const d = horiz ? Math.abs((r.y + r.h / 2) - (me.y + me.h / 2)) : Math.abs((r.x + r.w / 2) - (me.x + me.w / 2));
      if (d < bestD) { best = pid; bestD = d; }
    }
    return best;
  }

  // ---------- the workspace (tabs of trees) ----------
  const wsEmpty = () => ({ v: 2, tabs: [], active: null });
  const tabOf = (st, id) => st.tabs.find((x) => x.id === (id === undefined ? st.active : id)) || null;
  const focusedPane = (st) => { const tb = tabOf(st); return tb ? findPane(tb.root, tb.focus) : null; };
  const putTab = (st, tab) => Object.assign({}, st, { tabs: st.tabs.map((x) => (x.id === tab.id ? tab : x)) });
  const wsActivate = (st, id) => (tabOf(st, id) && st.active !== id ? Object.assign({}, st, { active: id }) : st);
  // a new tab with one pane goes right after the active one
  function wsOpen(st, id, pane) {
    const i = st.tabs.findIndex((x) => x.id === st.active), tabs = st.tabs.slice();
    tabs.splice(i < 0 ? tabs.length : i + 1, 0, { id, root: { pane }, focus: pane.id, zoom: false });
    return Object.assign({}, st, { tabs, active: id });
  }
  function wsClose(st, id) {
    const i = st.tabs.findIndex((x) => x.id === id);
    if (i < 0) return st;
    const tabs = st.tabs.filter((x) => x.id !== id);
    const active = st.active !== id ? st.active : tabs.length ? tabs[Math.min(i, tabs.length - 1)].id : null;
    return Object.assign({}, st, { tabs, active });
  }
  function wsMove(st, id, to) {
    const tab = tabOf(st, id);
    if (!tab) return st;
    const tabs = st.tabs.filter((x) => x.id !== id);
    tabs.splice(Math.max(0, Math.min(to, tabs.length)), 0, tab);
    return Object.assign({}, st, { tabs });
  }
  // split the focused pane of the active tab; the new pane takes the focus, a zoom ends
  function wsSplit(st, dir, fresh) {
    const tb = tabOf(st);
    if (!tb) return st;
    const root2 = splitPane(tb.root, tb.focus, dir, fresh);
    if (root2 === tb.root) return st;
    return putTab(st, Object.assign({}, tb, { root: root2, focus: fresh.id, zoom: false }));
  }
  // ⌘W: the focused pane goes; the last pane of a tab closes the tab
  function wsClosePane(st, paneId) {
    const tb = tabOf(st);
    if (!tb) return st;
    const id = paneId || tb.focus;
    const next = removePane(tb.root, id);
    if (!next) return wsClose(st, tb.id);
    if (next === tb.root) return st;
    let focus = tb.focus;
    if (focus === id) {        // the focus goes to the nearest remaining pane (left, up, right, down), else the first one
      focus = ['left', 'up', 'right', 'down'].map((d) => neighbor(tb.root, id, d)).find(Boolean) || leaves(next)[0].id;
    }
    return putTab(st, Object.assign({}, tb, { root: next, focus, zoom: false }));
  }
  function wsFocus(st, dir) {
    const tb = tabOf(st);
    if (!tb || tb.zoom) return st;
    const to = neighbor(tb.root, tb.focus, dir);
    return to ? putTab(st, Object.assign({}, tb, { focus: to })) : st;
  }
  function wsFocusPane(st, paneId) {
    const tb = st.tabs.find((x) => findPane(x.root, paneId));
    if (!tb) return st;
    if (tb.focus === paneId && st.active === tb.id) return st;
    return Object.assign(putTab(st, Object.assign({}, tb, { focus: paneId })), { active: tb.id });
  }
  const wsZoom = (st) => { const tb = tabOf(st); return tb && count(tb.root) > 1 ? putTab(st, Object.assign({}, tb, { zoom: !tb.zoom })) : st; };
  function wsSetPane(st, paneId, patch) {
    const tb = st.tabs.find((x) => findPane(x.root, paneId));
    if (!tb) return st;
    return putTab(st, Object.assign({}, tb, { root: mapPane(tb.root, paneId, (leaf) => ({ pane: Object.assign({}, leaf.pane, patch, { id: paneId }) })) }));
  }
  function wsRatio(st, path, ratio) { const tb = tabOf(st); return tb ? putTab(st, Object.assign({}, tb, { root: setRatio(tb.root, path, ratio) })) : st; }
  // every pane of every tab: [{tab, pane}]
  const allPanes = (st) => st.tabs.flatMap((tb) => leaves(tb.root).map((pane) => ({ tab: tb.id, pane })));

  // ---------- saving: settings.ui.tabs ----------
  const wsSerialize = (st) => JSON.stringify({ v: 2, tabs: st.tabs.map((x) => ({ id: x.id, root: x.root, focus: x.focus, zoom: !!x.zoom })), active: st.active });
  // a tree from disk: unknown shapes go, a pane that `keep(pane)` refuses goes (its sibling takes the place)
  function cleanTree(n, keep) {
    if (!n || typeof n !== 'object') return null;
    if (n.pane) { const p = n.pane; return p && typeof p.id === 'string' && typeof p.kind === 'string' && keep(p) ? { pane: Object.assign({}, p) } : null; }
    if (n.split) {
      const a = cleanTree(n.split.a, keep), b = cleanTree(n.split.b, keep);
      if (!a || !b) return a || b;
      return { split: { dir: n.split.dir === 'v' ? 'v' : 'h', ratio: clampRatio(n.split.ratio), a, b } };
    }
    return null;
  }
  // keep(pane) says whether a pane can come back (an agent pane whose chat was deleted cannot); a v1 save (tabs of chats)
  // becomes one tab per chat; more than MAX_PANES panes in a tree are cut from the end
  function wsRestore(raw, keep) {
    let o;
    try { o = typeof raw === 'string' ? JSON.parse(raw) : raw; } catch (e) { return null; }
    if (!o || !Array.isArray(o.tabs)) return null;
    keep = keep || (() => true);
    let tabs;
    if (o.v === 2) {
      tabs = o.tabs.map((x) => {
        if (!x || typeof x.id !== 'string') return null;
        let r = cleanTree(x.root, keep);
        while (r && count(r) > MAX_PANES) r = removePane(r, leaves(r)[leaves(r).length - 1].id);
        if (!r) return null;
        const ids = leaves(r).map((p) => p.id);
        return { id: x.id, root: r, focus: ids.includes(x.focus) ? x.focus : ids[0], zoom: !!x.zoom && ids.length > 1 };
      }).filter(Boolean);
    } else {
      tabs = o.tabs.filter((x) => x && x.id && x.env && x.chat).map((x) => ({ pane: { id: 'p-' + x.id, kind: 'agent', env: x.env, chat: x.chat }, id: x.id }))
        .filter((x) => keep(x.pane)).map((x) => ({ id: x.id, root: { pane: x.pane }, focus: x.pane.id, zoom: false }));
    }
    if (!tabs.length) return null;
    const ids = new Set();
    tabs = tabs.filter((x) => (ids.has(x.id) ? false : ids.add(x.id)));
    return { v: 2, tabs, active: tabs.some((x) => x.id === o.active) ? o.active : tabs[0].id };
  }

  root.NSPanes = { MAX_PANES, leaves, count, findPane, mapPane, splitPane, removePane, setRatio, rects, neighbor, nodeAt,
    wsEmpty, tabOf, focusedPane, wsOpen, wsClose, wsMove, wsActivate, wsSplit, wsClosePane, wsFocus, wsFocusPane, wsZoom, wsSetPane, wsRatio, allPanes, wsSerialize, wsRestore };
})(typeof window !== 'undefined' ? window : globalThis);
