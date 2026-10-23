// Limits of the environments for the dev-mock, as the real core reports them: only Claude Code has windows;
// every other environment says "no data" (shown as "Limits: no data"). A pure builder so ui/selftest.js can check it.
(function (root) {
  function build(opts) {
    const o = opts || {}, now = o.now || Date.now(), exhausted = !!o.exhausted;
    const iso = (ms) => new Date(ms).toISOString();
    const nextAt = (h, m, addDays) => { const d = new Date(now); d.setHours(h, m, 0, 0); if (d.getTime() <= now || addDays) d.setDate(d.getDate() + (addDays || 1)); return iso(d.getTime()); };
    const W = (name, pct, resetsAt, extra) => Object.assign({ name, model: null, used_pct: pct, remaining: null, resets_at: resetsAt, exhausted: false, source: 'cli', seen_at: iso(now - 10 * 60000) }, extra);
    return {
      claude: { known: [W('five_hour', exhausted ? 100 : 54, nextAt(19, 0), { exhausted }), W('seven_day', 16, nextAt(10, 0, 5)), W('seven_day_opus', 31, nextAt(10, 0, 5), { model: 'opus' }), W('seven_day_sonnet', 22, nextAt(10, 0, 5), { model: 'sonnet' }), W('day', 7, nextAt(0, 0))] },
      codex: { no_data: 'Codex не сообщает лимиты в неинтерактивном режиме' },
      gemini: { no_data: 'Gemini не сообщает лимиты' },
      opencode: { no_data: 'after_first_run' },
    };
  }
  root.NSMockLimits = { build };
})(typeof window !== 'undefined' ? window : globalThis);
