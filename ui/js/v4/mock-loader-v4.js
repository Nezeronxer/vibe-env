// The v4 fakes of the backend, only outside Tauri (as mock-loader.js does for dev-mock.js).
// dev-mock.js has already put a fake __TAURI__ in place by now, so its __MOCK__ flag decides
if (window.__MOCK__ || window.NSLogic.mockWanted(window)) {
  for (const f of ['mock-term', 'mock-market', 'mock-clawd', 'mock-auto', 'mock-sessions', 'mock-preview']) document.write('<script src="js/v4/' + f + '.js"><\/script>');
}
