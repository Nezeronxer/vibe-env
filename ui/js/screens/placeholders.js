// Slots for task 15. Each id below is a screen that task 15 fills: it registers a module with the same id (without
// `placeholder: true`) and the slot is replaced; shell.js, the rail and the other screens stay untouched.
//   factory  - rail foot, all envs at once      settings - rail foot, incl. "Внешний вид" (language switch: ctx.setLang)
//   swarm    - swarm of one task                kits     - kits (export / import / snapshots)
//   import   - kit import flow                  summary  - "Пока тебя не было"
(function () {
  const NS = window.NS;
  const { h } = NS;
  const slot = (id, rail) => NS.registry.register({ id, layout: 'full', placeholder: true, rail,
    mount(root) { root.append(h('div', { class: 'placeholder' }, h('h2', { text: NS.t('ph.' + id) }), h('p', { text: NS.t('ph.soon') }))); } });
  slot('factory', { zone: 'foot', icon: 'i-fact', labelKey: 'rail.factory', order: 1 });
  slot('settings', { zone: 'foot', icon: 'i-gear', labelKey: 'rail.settings', order: 2 });
  for (const id of ['swarm', 'kits', 'import', 'summary']) slot(id, null);
  NS.registry.register({ id: 'placeholder', layout: 'full', placeholder: true, mount(root) { root.append(h('div', { class: 'placeholder' }, h('p', { text: NS.t('ph.soon') }))); } });
})();
