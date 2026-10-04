//! Ladenlicht und Leuchtreklame (Port von `render.js shopGlowPoint`/`neonSigns` und `wetfx.js` neonText/neonColor/
//! neonOn/drawNeon): Läden, Lokale, Hotels und Bahnhöfe werfen nachts warmes Licht aus dem Schaufenster auf den
//! Gehweg; Kneipen, Clubs, Spätis, Imbisse und Hotels tragen dazu einen leuchtenden Schriftzug, jede achte Röhre
//! flackert. Alles aus Ort-Hashes, nie aus dem Welt-Zufall. Nur Darstellung.
use berlin_engine::camera::Camera;
use berlin_engine::hud::{Align, Hud};
use berlin_sim::city::{City, Poi};
use berlin_sim::math::hash01;
use glam::Vec2;
use std::collections::HashMap;

/// Kategorien mit Schaufensterlicht (`render.js SHOP_GLOW`)
pub const SHOP_GLOW: [&str; 9] = [
    "mall",
    "supermarket",
    "shop",
    "food",
    "drink",
    "cafe",
    "hotel",
    "ubahn",
    "sbahn",
];
pub const MAX_SIGNS: usize = 40;
const COLORS: [u32; 7] = [
    0xff3cac, 0x3cf0ff, 0x57ff6b, 0xffae3c, 0xb76bff, 0xff4b4b, 0xfff04a,
];

/// `wetfx.js h(...n)`: Hash über gerundete Zahlen.
pub fn h(ns: &[f64]) -> f64 {
    hash01(ns.iter().fold(5., |a, b| a * 31. + b.round()))
}

fn short(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
        .chars()
        .take(14)
        .collect()
}

/// Schriftzug einer Leuchtreklame oder `None` (nur Nachtleben, Spätis, Imbisse, Hotels).
pub fn neon_text(q: &Poi) -> Option<String> {
    let name = q.name.trim();
    let low = name.to_lowercase();
    if q.cat == "drink" {
        return Some(if q.kind == "nightclub" {
            if name.is_empty() {
                "CLUB".into()
            } else {
                short(name)
            }
        } else if !name.is_empty() && h(&[q.x, q.y, 1.]) < 0.6 {
            short(name)
        } else if q.kind == "pub" {
            "KNEIPE".into()
        } else {
            "BAR".into()
        });
    }
    if q.kind == "convenience" || q.kind == "kiosk" {
        return Some("SPÄTI".into());
    }
    if q.cat == "food" {
        if low.contains("döner") || low.contains("doner") || low.contains("kebab") {
            return Some("DÖNER".into());
        }
        if low.contains("pizz") {
            return Some("PIZZA".into());
        }
        if q.kind == "fast_food" {
            return Some(if h(&[q.x, q.y, 2.]) < 0.5 || name.is_empty() {
                "IMBISS".into()
            } else {
                short(name)
            });
        }
        return (h(&[q.x, q.y, 3.]) < 0.35 && !name.is_empty()).then(|| short(name));
    }
    (q.cat == "hotel").then(|| "HOTEL".into())
}

pub fn neon_color(q: &Poi) -> [f32; 3] {
    let c = COLORS[(h(&[q.x, q.y, 4.]) * COLORS.len() as f64) as usize % COLORS.len()];
    [
        ((c >> 16) & 255) as f32 / 255.,
        ((c >> 8) & 255) as f32 / 255.,
        (c & 255) as f32 / 255.,
    ]
}

/// Flackernde Röhren (etwa jede achte), sonst an.
pub fn neon_on(q: &Poi, t: f64) -> bool {
    h(&[q.x, q.y, 5.]) > 0.12 || h(&[q.x, q.y, (t * 9.).floor()]) > 0.3
}

#[derive(Debug, Clone)]
pub struct Sign {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub color: [f32; 3],
    pub on: bool,
}

/// Schaufenster-Punkt je POI: am Gehweg vor dem Laden (zur Fahrbahn hin), zwischengespeichert.
#[derive(Default)]
pub struct Neon {
    glow: HashMap<(i64, i64), Option<(f64, f64)>>,
    pub signs: Vec<Sign>,
    /// Deckkraft der Reklame (setzt mit der Dämmerung ein)
    pub alpha: f32,
}
impl Neon {
    pub fn glow_point(&mut self, city: &mut City, q: &Poi) -> Option<(f64, f64)> {
        let key = (q.x.round() as i64, q.y.round() as i64);
        if let Some(&g) = self.glow.get(&key) {
            return g;
        }
        let s = city.scale;
        let g = city
            .nearest_edge(q.x, q.y, 40. * s, |e| e.cls <= 8 && !e.bridge)
            .and_then(|ne| {
                let w = city.edges.get(&ne.edge).map_or(0., |e| e.w);
                let (dx, dy) = (q.x - ne.x, q.y - ne.y);
                let d = dx.hypot(dy);
                if d <= 1. {
                    return None;
                }
                let k = (w / 2. + 2.2 * s) / d;
                (k < 1.).then_some((ne.x + dx * k, ne.y + dy * k))
            });
        if self.glow.len() > 20000 {
            self.glow.clear();
        }
        self.glow.insert(key, g);
        g
    }
}

/// Leuchtschriften im Bildraum: farbiger Schein (versetzte Kopien), darüber weiße Röhren; aus = matt in der Farbe.
pub fn draw(neon: &Neon, camera: &Camera, viewport: Vec2, h: &mut Hud) {
    if neon.alpha <= 0.01 {
        return;
    }
    let k = neon.alpha;
    for s in &neon.signs {
        let p = camera.world_to_screen(Vec2::new(s.x as f32, s.y as f32), 0., viewport) / h.scale;
        let q =
            camera.world_to_screen(Vec2::new(s.x as f32 + 9., s.y as f32), 0., viewport) / h.scale;
        let size = (q - p).length().clamp(6., 22.);

        let [r, g, b] = s.color;
        if s.on {
            let o = size * 0.12;
            for (dx, dy) in [(-o, 0.), (o, 0.), (0., -o), (0., o)] {
                h.text(
                    &s.text,
                    p.x + dx,
                    p.y + dy,
                    size,
                    [r, g, b, 0.45 * k],
                    Align::Center,
                    false,
                );
            }
            h.text(
                &s.text,
                p.x,
                p.y,
                size,
                [1., 1., 1., k],
                Align::Center,
                false,
            );
        } else {
            h.text(
                &s.text,
                p.x,
                p.y,
                size,
                [r, g, b, 0.35 * k],
                Align::Center,
                false,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn poi(cat: &'static str, kind: &str, name: &str) -> Poi {
        Poi {
            x: 1234.,
            y: 5678.,
            cat,
            name: name.into(),
            kind: kind.into(),
        }
    }

    #[test]
    fn texts_follow_the_kind() {
        assert_eq!(
            neon_text(&poi("shop", "convenience", "Späti 24")).as_deref(),
            Some("SPÄTI")
        );
        let imbiss = neon_text(&poi("food", "fast_food", "Curry 36")).unwrap();
        assert!(imbiss == "IMBISS" || imbiss == "CURRY 36", "{imbiss}");
        assert_eq!(
            neon_text(&poi("food", "restaurant", "Döner König")).as_deref(),
            Some("DÖNER")
        );
        assert_eq!(
            neon_text(&poi("food", "restaurant", "Pizzeria Roma")).as_deref(),
            Some("PIZZA")
        );
        assert_eq!(
            neon_text(&poi("drink", "nightclub", "")).as_deref(),
            Some("CLUB")
        );
        assert_eq!(
            neon_text(&poi("drink", "nightclub", "Berghain  Panorama Bar")).as_deref(),
            Some("BERGHAIN PANOR")
        );
        assert_eq!(
            neon_text(&poi("hotel", "hotel", "x")).as_deref(),
            Some("HOTEL")
        );
        assert!(neon_text(&poi("shop", "bakery", "Bäcker")).is_none());
        let bar = neon_text(&poi("drink", "pub", "Zur Quelle")).unwrap();
        assert!(bar == "ZUR QUELLE" || bar == "KNEIPE", "{bar}");
    }

    #[test]
    fn colours_and_flicker_are_stable() {
        let q = poi("drink", "bar", "A");
        assert_eq!(neon_color(&q), neon_color(&q));
        assert!(COLORS.iter().any(|&c| {
            let n = neon_color(&q);
            ((c >> 16) & 255) as f32 / 255. == n[0]
        }));
        // jede Röhre ist die meiste Zeit an
        let on = (0..900).filter(|i| neon_on(&q, *i as f64 / 9.)).count();
        assert!(on > 600, "{on}");
        assert!((h(&[1., 2., 3.]) - h(&[1., 2., 3.])).abs() < 1e-15);
    }
}
