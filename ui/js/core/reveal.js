// v6 (G81): the answer prints smoothly. One requestAnimationFrame loop for every text being revealed: each frame a node
// gets the next few characters of its target text, more when it lags behind (the buffer grows), never more than MAX.
// The node is written to, not rebuilt: one Text node inside it grows by appendData.
(function () {
  const NS = (window.NS = window.NS || {});
  const MAX = 40;                           // characters a frame at most (60 frames a second: up to 2400 a second)
  const LAG = 6;                            // a backlog of N characters is caught up in about LAG frames
  const items = new Set();
  let raf = 0;
  const reduced = () => !!(window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches);
  // how many characters this frame for a backlog: 1 when it barely lags, faster when it lags more, MAX at most
  const step = (backlog) => (backlog <= 0 ? 0 : Math.min(MAX, Math.max(1, Math.ceil(backlog / LAG))));

  function frame() {
    raf = 0;
    const all = reduced();
    for (const it of [...items]) {
      if (it.stopped) { items.delete(it); continue; }
      if (!it.node.isConnected) { if (++it.away > 600) items.delete(it); continue; }     // closed for ~10 s: forget it
      it.away = 0;
      const target = String(it.getTarget() || ''), tn = it.text;
      let shown = tn.length;
      if (target.slice(0, shown) !== tn.data) {                    // the text was replaced (the final message differs): keep what agrees
        let i = 0; while (i < shown && target[i] === tn.data[i]) i++;
        tn.deleteData(i, shown - i); shown = i;
      }
      const n = all ? target.length - shown : step(target.length - shown);
      if (n > 0) { tn.appendData(target.slice(shown, shown + n)); it.frames.push(n); if (it.frames.length > 240) it.frames.shift(); if (it.onGrow) it.onGrow(); }
      else if (it.isFinal()) { items.delete(it); if (it.onDone) it.onDone(); }
    }
    if (items.size) raf = requestAnimationFrame(frame);
  }
  const wake = () => { if (!raf && items.size) raf = requestAnimationFrame(frame); };

  // node: the element the text goes into (a Text node is put first in it); getTarget(): the whole text known now.
  // opts: isFinal() - no more text will come (then onDone() once all is shown), onGrow() - after each frame that added text.
  function attach(node, getTarget, opts) {
    const o = opts || {};
    const text = document.createTextNode('');
    node.insertBefore(text, node.firstChild);
    const it = { node, text, getTarget, isFinal: o.isFinal || (() => false), onDone: o.onDone, onGrow: o.onGrow, frames: [], away: 0, stopped: false };
    items.add(it);
    wake();
    return {
      text,
      frames: it.frames,                         // characters added in each of the last frames (the selftest reads it)
      poke: wake,                                // more text arrived
      shown: () => text.length,
      stop() { it.stopped = true; items.delete(it); },
    };
  }

  NS.reveal = { attach, step, MAX };
})();
