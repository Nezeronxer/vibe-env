// Loads the fake backend only outside Tauri. Tauri 2 always defines __TAURI_INTERNALS__ (even without withGlobalTauri).
// document.write keeps the load order (mock, then the app) and is a same-origin script, so the strict CSP allows it.
if (window.NSLogic.mockWanted(window)) document.write('<script src="dev-mock.js"><\/script>');
