export const clamp = (v, lo, hi) => (v < lo ? lo : v > hi ? hi : v);
export const lerp = (a, b, t) => a + (b - a) * t;
export const dist = (ax, ay, bx, by) => Math.hypot(bx - ax, by - ay);
export const sign = (v) => (v > 0 ? 1 : v < 0 ? -1 : 0);
export function wrapAngle(a) {
  while (a > Math.PI) a -= 2 * Math.PI;
  while (a < -Math.PI) a += 2 * Math.PI;
  return a;
}
// Frame-unabhängige Annäherung (exponentielles Glätten).
export const damp = (a, b, rate, dt) => lerp(a, b, 1 - Math.exp(-rate * dt));
