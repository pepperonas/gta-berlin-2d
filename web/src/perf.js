// Dynamische Auflösung (rein, testbar): hält die Bildrate, wenn die Grafikkarte der Engpass ist – das misst die
// Zeichenzeit in JavaScript nicht (render.js nextQuality), wohl aber der Abstand der Bilder (requestAnimationFrame).
// Stufen der internen Auflösung; über RES.slowMs (Median) eine Stufe herunter, unter RES.fastMs wieder hinauf.
export const RES = { steps: [1, 0.85, 0.7], slowMs: 21, fastMs: 15, window: 120, maxGapMs: 250 }; // Pausen (Tab im Hintergrund) dauern Sekunden

// Bildabstände sammeln (Lücken über maxGapMs – Tab im Hintergrund, Laden – zählen nicht). Liefert die neue Stufe
// (Index in RES.steps), sobald ein Fenster voll ist, sonst die alte. state: { gaps: [] }
export function stepResolution(state, level, gapMs) {
  if (!(gapMs > 0) || gapMs > RES.maxGapMs) return level;
  state.gaps.push(gapMs);
  if (state.gaps.length < RES.window) return level;
  const g = state.gaps.sort((a, b) => a - b), median = g[g.length >> 1];
  state.gaps.length = 0; state.median = median;
  if (median > RES.slowMs && level < RES.steps.length - 1) return level + 1;
  if (median < RES.fastMs && level > 0) return level - 1;
  return level;
}
