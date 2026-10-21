// Screen registry. A screen is a module that registers itself here:
//   NS.registry.register({ id, layout: 'chats' | 'full' | 'bare', mount(root, ctx) -> { update?(evt, payload), destroy?() } | void,
//                          rail?: { zone: 'foot', icon: 'i-fact', labelKey, order }, placeholder?: true })
// 'chats' = left rail + chat list of the env + screen; 'full' = left rail + screen over the full width (factory, settings);
// 'bare' = no rail (first run). ctx = { params, store, api, t, nav, toast, setLang, logic } (see shell.js).
// A placeholder entry (placeholder: true) is a slot for task 15: registering the same id without the flag replaces it,
// nobody has to edit this file or shell.js.
(function () {
  const NS = (window.NS = window.NS || {});
  const screens = new Map();
  function register(def) {
    const old = screens.get(def.id);
    if (old && !old.placeholder && def.placeholder) return;   // a real screen is never replaced by a placeholder
    screens.set(def.id, def);
  }
  NS.registry = { register, get: (id) => screens.get(id), list: () => [...screens.values()] };
})();
