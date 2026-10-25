// Screen registry and the v4 extension points. A screen is a module that registers itself here:
//   NS.registry.register({ id, layout: 'chats' | 'full' | 'bare', mount(root, ctx) -> { update?(evt, payload), destroy?() } | void,
//                          palette?: { labelKey, keys?, order }, placeholder?: true })
// 'chats' = the workspace (tabs, panes, bar); 'full' = one screen over the workspace area (factory, settings), Esc goes back;
// 'bare' = no chrome (first run). ctx = { params, store, api, t, nav, toast, setLang, logic } (see shell.js).
// A placeholder entry (placeholder: true) is a slot: registering the same id without the flag replaces it.
//
// v4 extension points (tasks 23-25 add to them without editing the shell): NS.registry.extend(point, def), NS.registry.points(point).
// A def with the id of an existing one replaces it. Every list comes back sorted by `order` (default 100).
//   command   - palette command  { id, labelKey | label, group?, keys?: 'Cmd+K'-style accel, order?, when?(ctx) -> bool, run(ctx) }
//   settings  - settings section { id, level: 'basic' | 'advanced', labelKey, order?, build(env, fx) -> Node[] }  (settings.js SEC rules)
//   bar       - bar module       { id, side: 'left' | 'right', order?, render(el, ctx) -> { update?(evt, payload), destroy?() } | void }
//   paneHeader- pane header slot { id, kinds?: ['agent' | 'term' | …], order?, render(el, pane, ctx) -> { update?, destroy? } | void }
//   details   - block in task "Детали" { id, order?, render(task, ctx) -> Node | null }
//   paneKind  - pane kind        { kind, labelKey, icon?, mount(el, pane, ctx) -> { update?, focus?, destroy?, title?() } , create?(ctx) -> pane fields }
//   tabsCorner- the corner right of the tabs (Clawd) { id, mount(el, ctx) -> { update?, destroy? } | void }  (the last registered wins)
//   paletteSource - v7 rows of ⌘K made on open { id, items?(ctx) -> command rows, search?(query) -> rows under the ranked ones }
(function (root) {
  const NS = (root.NS = root.NS || {});
  const screens = new Map();
  function register(def) {
    const old = screens.get(def.id);
    if (old && !old.placeholder && def.placeholder) return;   // a real screen is never replaced by a placeholder
    screens.set(def.id, def);
  }
  const POINTS = ['command', 'settings', 'bar', 'paneHeader', 'details', 'paneKind', 'tabsCorner', 'paletteSource'];
  const ext = new Map(POINTS.map((p) => [p, []]));
  const keyOf = (point, def) => (point === 'paneKind' ? def.kind : def.id);
  const listeners = [];
  function extend(point, def) {
    if (!ext.has(point)) throw new Error('unknown extension point ' + point);
    const k = keyOf(point, def);
    if (!k) throw new Error(point + ' needs ' + (point === 'paneKind' ? 'kind' : 'id'));
    const list = ext.get(point).filter((d) => keyOf(point, d) !== k);
    list.push(def);
    ext.set(point, list);
    for (const fn of listeners) fn(point, def);
  }
  const points = (point) => {
    if (!ext.has(point)) throw new Error('unknown extension point ' + point);
    return ext.get(point).slice().sort((a, b) => (a.order == null ? 100 : a.order) - (b.order == null ? 100 : b.order));
  };
  // the shell redraws the bar / palette when a module or command arrives after the first paint
  const onExtend = (fn) => { listeners.push(fn); return () => listeners.splice(listeners.indexOf(fn), 1); };
  NS.registry = { register, get: (id) => screens.get(id), list: () => [...screens.values()], extend, points, onExtend, POINTS };
})(typeof window !== 'undefined' ? window : globalThis);
