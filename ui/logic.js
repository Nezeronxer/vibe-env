// Pure helpers: grouping, summary parts, duration formatting. No DOM.
(function (root) {
  const DAY = 24 * 3600 * 1000;
  const GROUPS = ['needs', 'working', 'queued', 'done', 'failed', 'archive'];

  // One source of truth: status -> display group, CSS class, live timer.
  // merge_blocked is a postponed merge (waits for a clean main), not a question: "working" group and style.
  const STATUS = {
    queued:           { group: 'queued',  cls: 'queued',  live: false },
    preparing:        { group: 'working', cls: 'working', live: true },
    working:          { group: 'working', cls: 'working', live: true },
    checking:         { group: 'working', cls: 'working', live: true },
    reviewing:        { group: 'working', cls: 'working', live: true },
    merging:          { group: 'working', cls: 'working', live: true },
    needs_human:      { group: 'needs',   cls: 'needs',   live: true },
    awaiting_outward: { group: 'needs',   cls: 'needs',   live: true },
    paused_limit:     { group: 'working', cls: 'working', live: false },
    merge_blocked:    { group: 'working', cls: 'working', live: false },
    done:             { group: 'done',    cls: 'done',    live: false },
    done_not_pushed:  { group: 'done',    cls: 'failed',  live: false },
    failed:           { group: 'failed',  cls: 'failed',  live: false },
    cancelled:        { group: 'archive', cls: 'queued',  live: false },
    reverted:         { group: 'archive', cls: 'queued',  live: false },
  };
  const info = (status) => STATUS[status] || STATUS.working;

  function groupOf(task, now) {
    const g = info(task.status).group;
    if (g === 'done') {
      const fin = task.finished_at ? Date.parse(task.finished_at) : now;
      return now - fin > 7 * DAY ? 'archive' : 'done';
    }
    return g;
  }

  const ts = (v) => (v ? Date.parse(v) : 0);
  const SORT = {
    needs: (a, b) => ts(a.created_at) - ts(b.created_at),
    working: (a, b) => ts(a.started_at || a.created_at) - ts(b.started_at || b.created_at),
    queued: (a, b) => ts(a.created_at) - ts(b.created_at),
    done: (a, b) => ts(b.finished_at || b.created_at) - ts(a.finished_at || a.created_at),
    failed: (a, b) => ts(b.finished_at || b.created_at) - ts(a.finished_at || a.created_at),
    archive: (a, b) => ts(b.finished_at || b.created_at) - ts(a.finished_at || a.created_at),
  };

  // Returns only non-empty groups, in display order.
  function groupTasks(tasks, now) {
    const out = {};
    for (const t of tasks) (out[groupOf(t, now)] = out[groupOf(t, now)] || []).push(t);
    const res = [];
    for (const g of GROUPS) if (out[g] && out[g].length) res.push({ id: g, tasks: out[g].sort(SORT[g]) });
    return res;
  }

  // 185000 -> "3:05", 4329000 -> "1:12:09"
  function formatElapsed(ms) {
    const s = Math.max(0, Math.floor(ms / 1000));
    const p = (n) => String(n).padStart(2, '0');
    const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = s % 60;
    return h ? `${h}:${p(m)}:${p(sec)}` : `${m}:${p(sec)}`;
  }

  // forms = [one, few, many] for ru; [one, other] for en
  function plural(n, forms, lang) {
    if (lang === 'ru') {
      const a = Math.abs(n) % 100, b = a % 10;
      if (a > 10 && a < 20) return forms[2];
      if (b > 1 && b < 5) return forms[1];
      if (b === 1) return forms[0];
      return forms[2];
    }
    return n === 1 ? forms[0] : forms[forms.length - 1];
  }

  function countNeeds(tasks) {
    return tasks.filter((t) => t.status === 'needs_human' || t.status === 'awaiting_outward').length;
  }

  const api = { GROUPS, STATUS, info, groupOf, groupTasks, formatElapsed, plural, countNeeds };
  if (typeof module !== 'undefined') module.exports = api;
  root.NSLogic = api;
})(typeof window !== 'undefined' ? window : globalThis);
