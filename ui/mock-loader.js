// Loads the fake backend only outside Tauri. Tauri 2 always defines __TAURI_INTERNALS__ (even without withGlobalTauri).
// document.write keeps load order (mock, then app.js) and is a same-origin script, so a strict CSP allows it.
if (!window.__TAURI_INTERNALS__ && !window.__TAURI__) document.write('<script src="dev-mock.js"><\/script>');
