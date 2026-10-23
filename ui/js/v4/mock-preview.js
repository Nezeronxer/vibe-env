// Fakes of F15 for the browser without Tauri (loaded by mock-loader-v4.js): the preview window commands, a pick that
// "happens" a moment after «Указать элемент», and proof screenshots on the done task t-es (project web-site).
(function () {
  const T = window.__TAURI__;
  if (!T || !window.__MOCK__) return;
  const subs = {};
  const listen0 = T.event.listen;
  T.event.listen = (name, fn) => { (subs[name] = subs[name] || []).push(fn); return listen0(name, fn); };
  const emit = (name, payload) => (subs[name] || []).forEach((fn) => setTimeout(() => fn({ payload }), 0));
  const invoke0 = T.core.invoke;
  // the project and the task the screenshots belong to, as the core would have them after a run
  invoke0('list_projects').then((list) => { const p = list.find((x) => x.id === 'p2'); if (p) Object.assign(p, { proof: true, run_cmd: 'npm run dev', app_url: 'http://localhost:4010/' }); });
  invoke0('list_tasks').then((list) => { const tk = list.find((x) => x.id === 't-es'); if (tk) tk.outcome.proof = { shots: ['1.png', '2.png', '3.png'], skipped: null }; });

  // a page drawn as an SVG stands in for a PNG (img-src allows data:)
  const page = (n) => {
    const phone = n === 3, w = phone ? 390 : 1280, hgt = phone ? 844 : 800, clicked = n === 2;
    const bw = phone ? 300 : 360;
    const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="' + w + '" height="' + hgt + '" viewBox="0 0 ' + w + ' ' + hgt + '">' +
      '<rect width="100%" height="100%" fill="#fbf7ef"/><rect width="100%" height="56" fill="#1d3b5a"/>' +
      '<text x="24" y="36" font-family="Helvetica, Arial" font-size="20" font-weight="700" fill="#ffffff">web-site</text>' +
      (phone ? '' : '<text x="' + (w - 300) + '" y="35" font-family="Helvetica, Arial" font-size="15" fill="#dce6f0">Inicio   Precios   Contacto</text>') +
      '<text x="' + (phone ? 24 : 96) + '" y="' + (phone ? 150 : 210) + '" font-family="Helvetica, Arial" font-size="' + (phone ? 30 : 46) + '" font-weight="700" fill="#1d2733">Facturas sin papeleo</text>' +
      '<text x="' + (phone ? 24 : 96) + '" y="' + (phone ? 190 : 262) + '" font-family="Helvetica, Arial" font-size="' + (phone ? 15 : 20) + '" fill="#4b5866">Envía, cobra y archiva en un solo lugar.</text>' +
      '<rect x="' + (phone ? 24 : 96) + '" y="' + (phone ? 230 : 310) + '" width="' + (phone ? 220 : 240) + '" height="52" rx="8" fill="' + (clicked ? '#1b7f4c' : '#e0702b') + '"/>' +
      '<text x="' + ((phone ? 24 : 96) + 22) + '" y="' + (phone ? 263 : 343) + '" font-family="Helvetica, Arial" font-size="18" font-weight="700" fill="#ffffff">' + (clicked ? '¡Listo! Revisa tu correo' : 'Empezar gratis') + '</text>' +
      [0, 1, 2].map((i) => '<rect x="' + (phone ? 24 : 96 + i * (bw + 24)) + '" y="' + (phone ? 330 + i * 150 : 440) + '" width="' + (phone ? w - 48 : bw) + '" height="' + (phone ? 130 : 200) + '" rx="10" fill="#ffffff" stroke="#e2d9c8"/>' +
        '<text x="' + ((phone ? 24 : 96 + i * (bw + 24)) + 20) + '" y="' + ((phone ? 330 + i * 150 : 440) + 40) + '" font-family="Helvetica, Arial" font-size="17" font-weight="700" fill="#1d2733">' + ['Facturas', 'Cobros', 'Archivo'][i] + '</text>').join('') +
      '</svg>';
    return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
  };
  const LOCAL = /^http:\/\/(localhost|127\.0\.0\.1)(:\d{1,5})?([/?#][^\s\\]*)?$/i;
  let open = null;
  const cmds = {
    preview_open: ({ url }) => {
      const u = String(url || '').trim();
      if (!LOCAL.test(u)) throw 'not_local_url';
      open = /^http:\/\/[^/?#]+$/i.test(u) ? u + '/' : u;
      return open;
    },
    preview_reload: () => { if (!open) throw 'preview_closed'; return null; },
    preview_width: () => { if (!open) throw 'preview_closed'; return null; },
    preview_close: () => { open = null; return null; },
    preview_pick: () => {
      if (!open) throw 'preview_closed';
      setTimeout(() => emit('preview-pick', { selector: 'main > section.hero > a.btn-primary', text: 'Empezar gratis', w: 240, h: 52 }), 150);
      return null;
    },
    proof_shot: ({ taskId, file }) => {
      if (taskId !== 't-es' || !/^[1-3]\.png$/.test(file)) throw 'shot_not_found';
      return page(parseInt(file, 10));
    },
  };
  const invoke1 = T.core.invoke;
  // the chat of t-es had no lines: the request and the task card, so the card with the shots shows in its chat
  const at = (min) => new Date(Date.now() - min * 60000).toISOString();
  const ES = [{ id: 'es-1', chat_id: 'es', at: at(332), who: 'human', text: 'Сделай: переводы главной на испанский', task_id: null, delivered_at: null },
    { id: 'es-2', chat_id: 'es', at: at(331), who: 'system', text: 'Это задача.', kind: 'task', task_id: 't-es', delivered_at: null }];
  cmds.get_chat = (a) => invoke1('get_chat', a).then((l) => (a.id === 'es' && !l.length ? ES.filter((m) => !a.before || Date.parse(m.at) < Date.parse(a.before)) : l));
  T.core.invoke = (cmd, args) => {
    if (!cmds[cmd]) return invoke1(cmd, args);
    try { return Promise.resolve(cmds[cmd](args || {})); } catch (e) { return Promise.reject(e); }
  };
})();
