// Gemeinsame Art Direction. Die Simulation verwendet keine dieser Werte.
export const ART = Object.freeze({
  sidewalk: '#a6a297', asphalt: '#393e43', curb: '#c9c5b8', water: '#315b61',
  grass: '#6d8650', grassLight: '#7d945a', grassDark: '#5b7547',
  forest: '#435f40', stone: '#bab5a7', warm: '255,217,157', cool: '81,121,153',
  shadow: '#162b3e', vignette: 0.17,
});
export const FX_BUDGET = Object.freeze({ high: { particles: 360, rings: 64, emission: 1 }, low: { particles: 96, rings: 16, emission: 0.35 } });
export function filmMood(light) {
  const dusk = Math.max(0, 1 - Math.abs((light.elevation ?? 1) - 0.15) / 0.35) * (1 - light.dark * 0.7);
  return { warmth: 0.018 + dusk * 0.045, cool: 0.015 + light.dark * 0.035, bloom: Math.max(0, light.dark - 0.3) * 0.32 };
}
