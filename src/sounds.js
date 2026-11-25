'use strict';
// The island's sounds, made on the fly (no sound files): a ding when the AI is done, a pling for a question,
// a low tone for an error. Four packs. Used by the island and by Settings (to hear a pack before choosing it).
(function () {
  let ctx = null;
  const NOTES = {
    done: [784, 1175],          // up: done
    question: [988, 740, 988],  // pling-plong-pling: your turn
    error: [440, 311]           // down: something went wrong
  };
  const PACKS = {
    // classic: clean sine tones
    classic: { wave: 'sine', gap: 0.12, len: 0.5, gain: 0.16 },
    // 8-bit: square waves, short and snappy like an old game
    '8bit': { wave: 'square', gap: 0.09, len: 0.16, gain: 0.06, step: true },
    // soft: rounder and lower, slower
    soft: { wave: 'triangle', gap: 0.18, len: 0.75, gain: 0.14, shift: true },
    // bells: a tone with overtones and a long ring
    bells: { wave: 'sine', gap: 0.16, len: 1.6, gain: 0.11, bell: true }
  };

  function tone(f, t, p, vol) {
    const o = ctx.createOscillator(), g = ctx.createGain();
    o.type = p.wave;
    o.frequency.value = p.shift ? f * 0.75 : f; // soft: a bit lower
    const peak = p.gain * vol;
    g.gain.setValueAtTime(0, t);
    g.gain.linearRampToValueAtTime(peak, t + (p.step ? 0.004 : 0.015));
    if (p.step) { g.gain.setValueAtTime(peak, t + p.len * 0.8); g.gain.linearRampToValueAtTime(0, t + p.len); }
    else g.gain.exponentialRampToValueAtTime(0.0008, t + p.len);
    o.connect(g).connect(ctx.destination); o.start(t); o.stop(t + p.len + 0.05);
    if (p.bell) for (const [mult, part] of [[2.76, 0.35], [5.4, 0.12]]) {
      const o2 = ctx.createOscillator(), g2 = ctx.createGain();
      o2.frequency.value = f * mult;
      g2.gain.setValueAtTime(0, t); g2.gain.linearRampToValueAtTime(peak * part, t + 0.01); g2.gain.exponentialRampToValueAtTime(0.0005, t + p.len * 0.5);
      o2.connect(g2).connect(ctx.destination); o2.start(t); o2.stop(t + p.len);
    }
  }

  /** playChime('done' | 'question' | 'error', 'classic' | '8bit' | 'soft' | 'bells', volume 0–100) */
  window.playChime = function (kind, pack, volume) {
    try {
      const vol = Math.max(0, Math.min(100, volume ?? 70)) / 100;
      if (vol <= 0) return;
      ctx ||= new AudioContext();
      if (ctx.state === 'suspended') ctx.resume();
      const p = PACKS[pack] || PACKS.classic;
      (NOTES[kind] || NOTES.done).forEach((f, i) => tone(f, ctx.currentTime + 0.02 + i * p.gap, p, vol));
    } catch {}
  };
})();
