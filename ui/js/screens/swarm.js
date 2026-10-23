// Swarm of one task (G11). The screen opens on the shared chat of the swarm: it is the main surface, like the chat of an env (story 156) - it is
// the chat screen itself, with a "Кому" field (Всем / A / B / C / Ревьюеру). The race and the variants are a panel beside it that collapses.
// Data: task.swarm (mode, n), task.runs (RoleRun.variant) for the lanes, task.swarm_view for scores, advice, parts and votes (see the report).
// Only agents of this env appear (G17): the swarm never leaves its env.
(function () {
  const NS = window.NS;
  const { h, ic, roleIcon, envIcon, clear, setVars } = NS;
  const L = window.NSLogic;
  const t = (k, p) => NS.t(k, p);
  const fail = (e) => NS.shell.toast(NS.errHuman(NS.api.errText(e)), true);
  const PANEL = 'ns.swarmpanel';
  const LETTERS = 'ABCDE';

  function mount(root, ctx) {
    const st = NS.store, S = st.s;
    const envId = ctx.params.env, env = st.env(envId);
    let task = S.tasks.get(ctx.params.task) || L.swarmTaskOf(st.chatTasks(ctx.params.chat));
    if (!task) return NS.registry.get('chat').mount(root, ctx);          // nothing swarm-like here: an ordinary chat
    const chatId = ctx.params.chat || task.chat_id;
    const alive = { v: true };
    let collapsed = NS.storage.get(PANEL) === '0', tab = 'v', diffOpen = null, picking = false;
    const diffs = new Map();

    // ---------- the chat (main) with "Кому" ----------
    const viewOf = () => task.swarm_view || {};
    const letters = () => { const v = (viewOf().variants || []).map((x) => x.id); return v.length ? v : LETTERS.split('').slice(0, L.clampSwarmN((task.swarm || {}).n)); };
    const to = h('select', { 'aria-label': t('swarm.to') });
    function drawTo() {
      const keep = to.value;
      clear(to).append(h('option', { value: 'all', text: t('swarm.to_all') }), ...letters().map((l) => h('option', { value: l, text: l })), h('option', { value: 'reviewer', text: t('swarm.to_reviewer') }));
      if ([...to.options].some((o) => o.value === keep)) to.value = keep;
    }
    drawTo();
    const ext = { noteTo: () => (to.value === 'all' ? null : to.value),
      decorate(composer) { const row = composer.el.querySelector('.comp-row'); if (row) row.prepend(h('label', { class: 'sw-to' }, t('swarm.to'), to)); } };
    const chatCol = h('div', { class: 'sw-chatcol' });
    const chat = NS.registry.get('chat').mount(chatCol, Object.assign({}, ctx, { params: { env: envId, chat: chatId, ext } })) || {};

    // ---------- the panel ----------
    const panel = h('aside', { class: 'sw-panel', 'aria-label': t('swarm.panel') });
    const wrap = h('div', { class: 'sw-wrap' }, chatCol, panel);
    root.append(wrap);

    const mode = () => L.variant((task.swarm || {}).mode)[0] || 'race';
    const modeName = () => t('swarm.' + mode());
    const stopped = () => !!viewOf().stopped;
    const ended = () => ['done', 'fail'].includes(L.kindOf(task)) || stopped();
    const titleText = () => t('swarm.title', { env: env.name, mode: modeName(), n: (task.swarm || {}).n || letters().length });

    const toggle = () => { collapsed = !collapsed; NS.storage.set(PANEL, collapsed ? '0' : '1'); draw(); };

    async function pick(v, btn) {
      if (picking) return;
      picking = true; btn.disabled = true;
      try { await NS.api.call('pick_variant', { taskId: task.id, variant: v }); btn.textContent = t('swarm.taking', { v }); }
      catch (e) { fail(e); btn.disabled = false; picking = false; }
    }
    async function stop(btn) {
      if (!(await NS.projects.confirm(t('swarm.stop'), t('swarm.stop_text'), t('swarm.stop')))) return;
      btn.disabled = true;
      try { await NS.api.call('stop_swarm', { taskId: task.id }); NS.shell.toast(t('swarm.stopped')); } catch (e) { fail(e); btn.disabled = false; }
    }

    // the advice of the reviewer with the auto-pick timer (story 153)
    function verdict(ranked) {
      const v = viewOf(), a = v.advice;
      if (!a || mode() !== 'race') return null;
      const best = ranked.find((x) => x.id === a.variant);
      const left = L.autopickLeft(a.autopick_at, Date.now());
      const picked = v.picked;
      const mins = (S.settings || {}).autopick_min || 10;
      const body = h('p', {}, h('b', { text: t('swarm.advice', { model: a.model || '', v: a.variant }) + ' ' }), a.text || '',
        picked ? h('span', { class: 'tick', text: t('swarm.picked', { v: picked }) }) : !ended() && a.autopick_at ? h('span', { class: 'tick', text: t('swarm.autopick', { n: mins, v: a.variant }) + (left ? ' · ' + t('swarm.left', { n: left }) : '') }) : null);
      const btn = h('button', { type: 'button', class: 'btn primary', text: t('swarm.take', { v: a.variant }), disabled: !best || !best.canTake || !!picked || picking, onclick: (e) => pick(a.variant, e.currentTarget) });
      return h('div', { class: 'verdict' }, roleIcon('rev'), body, btn);
    }

    // a lane on the env's limit waits alone until its reset (G64.1): "ждёт лимит до HH:MM"
    const pausedText = (x) => { const d = new Date(x.resume_at); return t('swarm.paused', { time: isNaN(d) ? '?' : String(d.getHours()).padStart(2, '0') + ':' + String(d.getMinutes()).padStart(2, '0') }); };
    const pausedOf = (id) => [].concat(viewOf().variants || [], viewOf().parts || []).find((x) => x.id === id && x.state === 'paused');
    // lanes: shared preparation, one lane per variant, "Выбор", the mark "сейчас"
    function lanesBox() {
      const now = Date.now(), { lanes, t0, t1 } = L.swarmLanes(task, now);
      const known = new Set(lanes.map((l) => l.key));
      for (const v of viewOf().variants || []) if (!known.has(v.id)) lanes.splice(Math.max(0, lanes.findIndex((l) => l.kind === 'choice')), 0, { key: v.id, kind: 'variant', variant: v.id, model: v.model || '', segs: [], queued: true });
      const span = Math.max(t1 - t0, 60000), step = Math.max(1, Math.round(span / 60000 / 6));
      const axis = h('div', { class: 'axis' });
      for (let m = 0; m * 60000 <= span; m += step) axis.append(setVars(h('span', { text: t('swarm.min', { n: m }) }), { '--l': (m * 60000 * 100 / span) + '%' }));
      const rows = lanes.map((l) => {
        const lab = l.kind === 'prep' ? [t('swarm.prep'), t('swarm.prep_d')] : l.kind === 'choice' ? [t('swarm.choice'), t('swarm.choice_d', { model: l.model })] : [`${l.variant} · ${l.model}`, l.queued ? t('swarm.queued') : t(mode() === 'council' ? 'swarm.proposes' : 'swarm.worktree')];
        const rk = l.kind === 'variant' ? 'exec' : l.kind === 'choice' ? 'rev' : null;
        const track = h('div', { class: 'track' }, l.segs.map((s) => {
          const b = L.placeBar(s.from, s.to, t0, t1);
          const text = s.role === 'gate' ? (s.state === 'ok' ? '✓' : s.state === 'fail' ? '✕' : '…')
            : s.role === 'coord' ? t(mode() === 'council' ? 'swarm.bar_council' : 'swarm.bar_split') : t('role.' + s.role).toLowerCase().slice(0, 14);
          return setVars(h('div', { class: 'seg-b r-' + s.role + (s.state === 'proj' ? ' proj' : s.state === 'fail' ? ' fail' : ''), title: t('role.' + s.role) + (s.model ? ' · ' + s.model : ''), text }), { '--l': b.left + '%', '--w': b.width + '%' });
        }));
        const paused = l.kind === 'variant' ? pausedOf(l.variant) : null;
        if (paused) lab[1] = pausedText(paused);
        else if (l.queued) track.append(h('span', { class: 'hint', text: t('swarm.queued') }));
        return h('div', { class: 'lane' + (rk ? ' r-' + rk : '') + (paused ? ' paused' : ''), title: paused ? t('swarm.paused_d') : null }, h('div', { class: 'lane-n' }, rk ? roleIcon(rk) : null, h('div', {}, h('b', { text: lab[0] }), h('small', { text: lab[1] }))), track);
      });
      const nowPct = L.placeBar(now, now + 1, t0, t1);
      const box = h('div', { class: 'lanes' }, h('div', { class: 'lane-axis' }, h('div'), axis), rows,
        nowPct ? h('div', { class: 'lane-now-wrap' }, setVars(h('div', { class: 'now' }, h('span', { text: t('swarm.now') })), { '--l': nowPct.left + '%' })) : null);
      const k = { race: 'swarm.heat', crew: 'swarm.heat_crew', council: 'swarm.heat_council' }[mode()] || 'swarm.heat';
      return h('section', { class: 'sw-race' }, h('h3', {}, t(k), h('span', { class: 'hint', text: t(k + '_d', { env: env.name }) })), box);
    }

    // a variant: score, tests, "Почему такой балл", "Взять", "Дифф", the cost where the env reports it
    const critBar = (c) => { const v = Math.max(0, Math.min(10, c.value)); return h('div', { class: 'crit' }, h('span', { text: t('crit.' + c.key) }), setVars(h('span', { class: 'cbar ' + (v >= 7 ? 'good' : v < 5 ? 'bad' : '') }, h('i')), { '--w': v * 10 + '%' }), h('b', { text: String(c.value) })); };
    function statusChip(v) {
      if (v.bestFailed) return h('span', { class: 'st st-run' }, ic('i-run'), t('swarm.best_try'));
      if (v.best) return h('span', { class: 'st st-ok' }, ic('i-check'), t('swarm.best'));
      if (v.state === 'passed') return h('span', { class: 'st st-ok' }, ic('i-check'), t('swarm.passed'));
      if (v.state === 'failed') return h('span', { class: 'st st-bad' }, ic('i-fail'), t('swarm.failed'));
      if (v.state === 'queued') return h('span', { class: 'st st-run' }, ic('i-dash'), t('swarm.queued'));
      if (v.state === 'paused') return h('span', { class: 'st st-wait', title: t('swarm.paused_d') }, ic('i-dash'), pausedText(v));
      return h('span', { class: 'st st-run' }, ic('i-run'), t('swarm.running'));
    }
    async function loadDiff(v, path) {
      const key = v + '|' + path;
      if (diffs.has(key)) return;
      try { diffs.set(key, (await NS.api.call('diff_file', { taskId: task.id, path, variant: v })) || { lines: [] }); } catch (e) { diffs.set(key, { lines: [{ kind: 'ctx', text: NS.errHuman(NS.api.errText(e)) }] }); }
    }
    function diffPane(v, data) {
      const sel = diffOpen && diffOpen.v === v.id ? diffOpen.path : null;
      const files = data.files || [];
      const list = h('ul', { class: 'flist' });
      for (const f of files) {
        const on = sel === f.path;
        list.append(h('li', { class: on ? 'on' : '', tabIndex: 0, role: 'button', 'aria-expanded': String(on),
          onclick: async () => { diffOpen = { v: v.id, path: on ? null : f.path }; if (!on) await loadDiff(v.id, f.path); if (alive.v) draw(); },
          onkeydown: (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.currentTarget.click(); } } },
        ic('i-chev-r', 'tw'), h('span', { text: f.path }), h('small', {}, h('span', { class: 'plus', text: '+' + (f.insertions || 0) }), ' ', h('span', { class: 'minus', text: '−' + (f.deletions || 0) }))));
        if (on) list.append(h('li', { class: 'fdiff' }, diffs.has(v.id + '|' + f.path) ? NS.taskCard.diffBox(diffs.get(v.id + '|' + f.path), task, f.path) : h('p', { class: 'hint', text: t('diff.loading') })));
      }
      return h('div', { class: 'var-diff' }, files.length ? list : h('p', { class: 'hint', text: t('det.no_changes') }));
    }
    function variantCard(v) {
      const st2 = v.stats || {};
      const more = [st2.ins != null ? `+${st2.ins} −${st2.del || 0}` : null, st2.minutes != null ? t('swarm.mins', { n: st2.minutes }) : null, L.formatCost(v.cost_usd) || null].filter(Boolean).join(' · ');
      const card = h('article', { class: 'var' + (v.best ? ' best' : ''), dataset: { variant: v.id } },
        h('div', { class: 'var-h' }, h('span', { class: 'var-l', text: v.id }), envIcon(env.custom ? 'custom' : env.id), h('b', { text: v.model || '' }), statusChip(v)),
        v.summary ? h('div', { class: 'var-sum' }, NS.renderMarkdown(v.summary)) : null,
        v.state === 'failed' && v.why_failed ? h('p', { class: 'var-why', text: NS.errHuman(v.why_failed) }) : null,
        v.score != null || v.gate ? h('div', { class: 'var-nums' },
          v.score != null ? h('div', {}, h('b', { text: Number(v.score).toFixed(1) }), h('small', { text: t('swarm.score') })) : null,
          v.gate ? h('div', {}, h('b', { text: t('swarm.of', { a: v.gate.passed, b: v.gate.total }) }), h('small', { text: t('swarm.tests') })) : null) : null,
        (v.criteria || []).length ? h('details', {}, h('summary', { text: t('swarm.why') }), (v.criteria || []).map(critBar), more ? h('small', { text: more }) : null) : null,
        h('div', { class: 'var-f' }, v.canTake ? h('button', { type: 'button', class: 'btn' + (v.best ? ' primary' : ''), text: t('swarm.take', { v: v.id }), disabled: !!viewOf().picked || picking, onclick: (e) => pick(v.id, e.currentTarget) }) : null,
          (v.files || []).length ? h('button', { type: 'button', class: 'btn', 'aria-expanded': String(!!diffOpen && diffOpen.v === v.id), text: t('swarm.diff'), onclick: () => { diffOpen = diffOpen && diffOpen.v === v.id ? null : { v: v.id, path: null }; draw(); } }) : null));
      return card;
    }
    function variantsGrid() {
      const view = viewOf(), ranked = L.rankVariants(view.variants || [], view.advice);
      const grid = h('div', { class: 'variants' });
      for (const v of ranked) {
        grid.append(variantCard(v));
        if (diffOpen && diffOpen.v === v.id) grid.append(diffPane(v, v));
      }
      const failedAll = ranked.length && ranked.every((x) => x.state === 'failed');
      const bf = ranked.find((x) => x.bestFailed);
      return h('div', {}, failedAll && bf ? h('div', { class: 'verdict' }, ic('i-fail'), h('p', {}, h('b', { text: t('swarm.all_failed') + ' ' }), t('swarm.best_try_d', { v: bf.id, a: bf.gate ? bf.gate.passed : 0, b: bf.gate ? bf.gate.total : 0 }))) : null, grid);
    }
    // Бригада: parts with disjoint files, each done by a worker in its own copy
    function partsGrid() {
      const parts = viewOf().parts || [];
      return h('div', { class: 'variants' }, parts.map((p) => h('article', { class: 'part' }, h('h4', {}, p.title, h('span', { class: 'st ' + (p.state === 'done' ? 'st-ok' : p.state === 'failed' ? 'st-bad' : p.state === 'paused' ? 'st-wait' : 'st-run') },
        ic(p.state === 'done' ? 'i-check' : p.state === 'failed' ? 'i-fail' : p.state === 'paused' ? 'i-dash' : 'i-run'), p.state === 'paused' ? pausedText(p) : t('swarm.part_' + (p.state || 'running')))),
      h('small', { text: [p.worker, p.model].filter(Boolean).join(' · ') }), h('ul', {}, (p.files || []).map((f) => h('li', { text: f }))))));
    }
    // Совет: proposals and the vote; the majority decides, a tie goes to the reviewer (story 155)
    function councilBox() {
      const c = viewOf().council || {}, tally = L.tallyVotes(c.votes || []);
      const win = tally.winner || c.winner;
      if (tab === 'd') {
        return h('div', {}, h('div', { class: 'tally' }, h('b', { text: tally.winner ? t('swarm.winner', { v: tally.winner, n: tally.counts[tally.winner], total: tally.total }) : tally.tie.length ? t('swarm.tie', { v: tally.tie.join(', ') }) : t('swarm.no_votes') }),
          ...Object.keys(tally.counts).sort().map((k) => h('span', { class: 'hint', text: `${k}: ${tally.counts[k]}` }))),
        h('ul', { class: 'votes' }, (c.votes || []).map((v) => h('li', {}, h('b', { text: v.voter }), t('swarm.votes_for'), h('b', { text: v.for }), v.why ? h('span', { class: 'hint', text: ' - ' + v.why }) : null))));
      }
      return h('div', { class: 'variants' }, (c.proposals || []).map((p) => h('article', { class: 'prop' + (win === p.id ? ' win' : '') }, h('h4', {}, h('span', { class: 'var-l', text: p.id }), p.model || '',
        win === p.id ? h('span', { class: 'st st-ok' }, ic('i-check'), t('swarm.won')) : null, h('small', { text: t('swarm.n_votes', { n: tally.counts[p.id] || 0 }) })), h('p', { class: 'var-sum', text: p.text || '' }))));
    }

    function draw() {
      wrap.classList.toggle('collapsed', collapsed);
      panel.classList.toggle('collapsed', collapsed);
      clear(panel);
      const toggler = h('button', { type: 'button', class: 'btn sm', 'aria-expanded': String(!collapsed), title: collapsed ? t('swarm.expand') : t('swarm.collapse'), 'aria-label': collapsed ? t('swarm.expand') : t('swarm.collapse'), onclick: toggle }, ic('i-chev-r'));
      if (collapsed) {
        panel.append(toggler, h('span', { class: 'sw-mini', text: titleText() }));
        return;
      }
      const stopBtn = h('button', { type: 'button', class: 'btn', text: t('swarm.stop'), disabled: ended(), onclick: (e) => stop(e.currentTarget) });
      panel.append(h('div', { class: 'sw-ph' }, h('div', {}, h('span', { class: 'st ' + (stopped() ? 'st-bad' : ended() ? 'st-ok' : 'st-run') }, ic('i-swarm'), titleText() + (stopped() ? ' · ' + t('swarm.stopped_tag') : '')), h('h2', { text: task.title })),
        h('div', { class: 'acts' }, stopBtn, toggler)));
      const view = viewOf(), ranked = L.rankVariants(view.variants || [], view.advice);
      const v = verdict(ranked);
      if (v) panel.append(v);
      panel.append(lanesBox());
      if (mode() === 'council') {
        const c = view.council || {};
        panel.append(h('div', { class: 'sw-tabs', role: 'tablist' }, h('button', { type: 'button', role: 'tab', class: tab === 'v' ? 'on' : '', 'aria-selected': String(tab === 'v'), text: t('swarm.proposals'), onclick: () => { tab = 'v'; draw(); } }),
          h('button', { type: 'button', role: 'tab', class: tab === 'd' ? 'on' : '', 'aria-selected': String(tab === 'd'), onclick: () => { tab = 'd'; draw(); } }, t('swarm.discussion'), h('small', { text: String((c.proposals || []).length + (c.votes || []).length) }))), councilBox());
      } else if (mode() === 'crew') panel.append(h('div', { class: 'sw-tabs' }, h('button', { type: 'button', class: 'on', text: t('swarm.parts') })), partsGrid());
      else panel.append(h('div', { class: 'sw-tabs' }, h('button', { type: 'button', class: 'on', text: t('swarm.variants') })), variantsGrid());
    }
    draw();
    const tick = setInterval(() => alive.v && !collapsed && draw(), 30000);

    return {
      update(evt, payload) {
        if (chat.update) chat.update(evt, payload);
        if (evt === 'task' && payload && payload.id === task.id) { task = payload; picking = false; drawTo(); draw(); }
      },
      destroy() { alive.v = false; clearInterval(tick); if (chat.destroy) chat.destroy(); },
    };
  }

  NS.registry.register({ id: 'swarm', layout: 'chats', mount });
})();
