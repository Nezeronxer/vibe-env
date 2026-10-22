// The v4 fakes of the backend, only outside Tauri (as mock-loader.js does for dev-mock.js).
if (window.NSLogic.mockWanted(window)) {
  for (const f of ['mock-term', 'mock-market', 'mock-clawd']) document.write('<script src="js/v4/' + f + '.js"><\/script>');
}
