//! Kleine Rechenhilfen (Port von `math.js`) und deterministischer Zufall (`rng.js`).
use std::f64::consts::PI;

pub fn wrap_angle(mut a: f64) -> f64 {
    while a > PI {
        a -= 2. * PI;
    }
    while a < -PI {
        a += 2. * PI;
    }
    a
}
pub fn sign(v: f64) -> f64 {
    if v > 0. {
        1.
    } else if v < 0. {
        -1.
    } else {
        0.
    }
}
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
/// Frame-unabhängige Annäherung (exponentielles Glätten).
pub fn damp(a: f64, b: f64, rate: f64, dt: f64) -> f64 {
    lerp(a, b, 1. - (-rate * dt).exp())
}
pub fn smoothstep01(x: f64) -> f64 {
    let t = x.clamp(0., 1.);
    t * t * (3. - 2. * t)
}

/// mulberry32 – bitgleich zur JS-Fassung (`rng.js`): gleiche Folge bei gleichem Seed.
#[derive(Debug, Clone)]
pub struct Rng(u32);
impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed)
    }
    pub fn state(&self) -> u32 {
        self.0
    }
    /// Gleichverteilt in [0, 1).
    pub fn float(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        (t ^ (t >> 14)) as f64 / 4294967296.
    }
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.float()
    }
    /// Ganzzahl in [lo, hi] (beide eingeschlossen).
    pub fn int(&mut self, lo: i64, hi: i64) -> i64 {
        (lo as f64 + (hi - lo + 1) as f64 * self.float()).floor() as i64
    }
    pub fn index(&mut self, len: usize) -> usize {
        ((self.float() * len as f64).floor() as usize).min(len.saturating_sub(1))
    }
}

/// `hash01` aus `map.js` für eine beliebige (auch nicht ganzzahlige) Zahl: wie JS über ToUint32.
pub fn hash01(n: f64) -> f64 {
    let mut t = to_uint32(n + 1831565813.); // (n + 0x6d2b79f5) >>> 0
    t = (t ^ (t >> 15)).wrapping_mul(t | 1);
    t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
    (t ^ (t >> 14)) as f64 / 4294967296.
}
/// ECMAScript ToUint32 (abschneiden, modulo 2³²).
pub fn to_uint32(n: f64) -> u32 {
    if !n.is_finite() {
        return 0;
    }
    let t = n.trunc().rem_euclid(4294967296.);
    t as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mulberry32_matches_js_reference() {
        // Werte aus Node: const r = mulberry32(1996); [r(), r(), r()]
        let mut r = Rng::new(1996);
        let v: Vec<f64> = (0..3).map(|_| r.float()).collect();
        assert_eq!(
            v,
            [0.13257614197209477, 0.3102250755764544, 0.917528766207397]
        );
        assert_eq!(hash01(237964.5 * 7919. + 3.25), 0.6421540749724954);
        assert_eq!(hash01(-5.), 0.48384718922898173);
        assert_eq!(hash01(42.), berlin_map_loader::citycodes::hash01(42));
    }
    #[test]
    fn wrap_and_uint() {
        assert!((wrap_angle(3. * PI) - PI).abs() < 1e-12);
        assert!((wrap_angle(-3.5 * PI) - 0.5 * PI).abs() < 1e-12);
        assert_eq!(to_uint32(-1.), u32::MAX);
        assert_eq!(to_uint32(4294967297.9), 1);
    }
}
