//! Stadtleben (Port von `life.js`): Menschen, die nicht nur gehen, sondern an echten Orten etwas tun – abhängig von
//! Ort, Uhrzeit und Wochentag.
//!   Haltestellen: Wartende · Bars/Kneipen: Rauchergruppen · Clubs (Fr/Sa-Nacht): Schlange · Späti: Leute mit Flasche
//!   Cafés: Gäste an Tischen · Imbiss/Restaurant: Plaudernde · Läden/Kultur: Schaufenstergucker · U-Bahnhof: Straßenmusik
//!   Bänke (OSM): Sitzende · Grünflächen: Gruppen auf Decken
//! `life_spots` ist rein rechnerisch und deterministisch (gleicher Ort + gleiche Stunde = gleiche Szene); `world.rs`
//! setzt daraus echte Passanten (Zustand `Hang`), außer Sicht erzeugt und außer Sicht abgebaut. Erschreckt fliehen
//! sie wie alle anderen und gehen danach normal weiter.
use crate::city::{City, Poi, PolyKind, point_in_rings};
use crate::collision::Rect;
use crate::math::hash01;
use crate::rhythm::{is_weekend, nightlife, people_level, wrap};
use std::collections::HashMap;

pub const RADIUS: f64 = 1500.;
pub const VIEW_HALF_X: f64 = 760.;
pub const VIEW_HALF_Y: f64 = 470.;
pub const DESPAWN: f64 = 1900.;
pub const EVERY: f64 = 0.5;
pub const MAX_HANGERS: usize = 70;
/// Flächenarten (citycodes.js AREA_KIND) und Stadtmöbel (FURN_KIND)
pub const AREA_PLAZA: u8 = 1;
pub const AREA_GRASS: u8 = 4;
pub const FURN_BENCH: u8 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Act {
    Wait,
    Smoke,
    Queue,
    Drink,
    Sit,
    Chat,
    Browse,
    Music,
    Lie,
}
impl Act {
    pub fn name(self) -> &'static str {
        match self {
            Act::Wait => "wait",
            Act::Smoke => "smoke",
            Act::Queue => "queue",
            Act::Drink => "drink",
            Act::Sit => "sit",
            Act::Chat => "chat",
            Act::Browse => "browse",
            Act::Music => "music",
            Act::Lie => "lie",
        }
    }
}

/// Ein Platz für eine Person: Schlüssel, Ort, Blick, Tätigkeit, Gruppe (Mitte), Bank.
#[derive(Debug, Clone, PartialEq)]
pub struct Hang {
    pub key: String,
    pub x: f64,
    pub y: f64,
    pub face: f64,
    pub act: Act,
    pub group: String,
    pub gx: f64,
    pub gy: f64,
    pub bench: bool,
}

/// Stelle vor einem Laden auf dem Gehweg und Blick zur Straße.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Front {
    pub x: f64,
    pub y: f64,
    pub ux: f64,
    pub uy: f64,
    pub face: f64,
}

/// `h(...n)` aus life.js: Ort-Hash über gerundete Zahlen.
pub fn h(seed: f64, n: &[f64]) -> f64 {
    hash01(n.iter().fold(seed, |a, &b| a * 31. + (b + 0.5).floor()))
}
fn hl(n: &[f64]) -> f64 {
    h(7., n)
}
fn in_hours(m: f64, from: f64, to: f64) -> bool {
    if from <= to {
        m >= from && m < to
    } else {
        m >= from || m < to
    }
}

/// Was an einem POI gerade los ist: (Tätigkeit, Anzahl) oder `None`.
pub fn activity_for(q: &Poi, minutes: f64, day: u32) -> Option<(Act, usize)> {
    let m = wrap(minutes);
    let hr = (m / 60.).floor();
    let r = hl(&[q.x, q.y, hr, day as f64]);
    let (people, night) = (people_level(m, day), nightlife(m, day));
    let n = |v: f64| v.floor().max(0.) as usize;
    match q.cat {
        "bus" => in_hours(m, 300., 60.).then(|| (Act::Wait, n(people * 3. + r))),
        "ubahn" | "sbahn" => {
            if in_hours(m, 600., 1200.) && r < 0.3 {
                return Some((Act::Music, 1));
            }
            in_hours(m, 300., 60.).then(|| (Act::Wait, n(people * 2. + r)))
        }
        "drink" => {
            if q.kind == "nightclub" {
                return (in_hours(m, 1380., 360.) && night > 0.5)
                    .then(|| (Act::Queue, 6 + n(r * 10.)));
            }
            in_hours(m, 1080., 180.).then(|| (Act::Smoke, n(1. + night * 3. + r * 2.)))
        }
        "supermarket" | "shop" => {
            if q.kind == "convenience" || q.kind == "kiosk" {
                return in_hours(m, 1020., 180.)
                    .then(|| (Act::Drink, n(night * 3. + people + r * 1.5)));
            }
            (in_hours(m, 540., 1200.) && r < 0.35).then_some((Act::Browse, 1))
        }
        "cafe" => in_hours(m, 480., 1140.).then(|| (Act::Sit, n(people * 3. + r))),
        "food" => in_hours(m, 660., 1380.).then(|| (Act::Chat, n(people * 2. + r))),
        "culture" | "mall" => {
            (in_hours(m, 600., 1200.) && r < 0.5).then(|| (Act::Browse, 1 + n(r * 2.)))
        }
        _ => None,
    }
}

/// Stelle vor dem Laden (zur nächsten Straße hin), ohne Cache.
pub fn front_of(city: &mut City, x: f64, y: f64) -> Option<Front> {
    let s = city.scale;
    let e = city.nearest_edge(x, y, 40. * s, |o| o.cls <= 9)?;
    let w = city.edges.get(&e.edge)?.w;
    let (dx, dy) = (x - e.x, y - e.y);
    let d = dx.hypot(dy);
    let d = if d > 0. { d } else { 1. };
    let off = d.min(w / 2. + 2.2 * s);
    let (fx, fy) = (e.x + dx / d * off, e.y + dy / d * off);
    if city.in_building(fx, fy).is_some() || city.on_road(fx, fy, 0., None).is_some() {
        return None;
    }
    Some(Front {
        x: fx,
        y: fy,
        ux: e.ux,
        uy: e.uy,
        face: (-dy).atan2(-dx),
    })
}

/// Gecachte Vorplätze und Bankrichtungen (je Ort, gerundet).
#[derive(Debug, Default, Clone)]
pub struct LifeCache {
    fronts: HashMap<(i64, i64), Option<Front>>,
    benches: HashMap<(i64, i64), f64>,
}
impl LifeCache {
    pub fn front(&mut self, city: &mut City, x: f64, y: f64) -> Option<Front> {
        if self.fronts.len() > 20_000 {
            self.fronts.clear();
        }
        *self
            .fronts
            .entry((x.round() as i64, y.round() as i64))
            .or_insert_with(|| front_of(city, x, y))
    }
    /// Bank: Ausrichtung parallel zur nächsten Straße (OSM kennt sie meist nicht).
    pub fn bench_angle(&mut self, city: &mut City, x: f64, y: f64) -> f64 {
        if self.benches.len() > 20_000 {
            self.benches.clear();
        }
        *self
            .benches
            .entry((x.round() as i64, y.round() as i64))
            .or_insert_with(|| {
                let r = 25. * city.scale;
                city.nearest_edge(x, y, r, |_| true)
                    .map(|e| e.uy.atan2(e.ux))
                    .unwrap_or_else(|| (hash01(x * 7. + y) * 628.).floor() / 100.)
            })
    }
}

/// Anordnung einer Gruppe: Schlange entlang des Gehwegs, Reihe (Warten/Gucken/Musik), sonst im Kreis.
fn arrange(spot: Front, n: usize, act: Act, seed: f64, out: &mut Vec<Hang>, base: &str) {
    let tau = std::f64::consts::TAU;
    for i in 0..n {
        let fi = i as f64;
        let (x, y, face) = match act {
            Act::Queue => (
                spot.x + spot.ux * fi * 9.,
                spot.y + spot.uy * fi * 9.,
                (-spot.uy).atan2(-spot.ux),
            ),
            Act::Wait | Act::Browse | Act::Music => {
                let k = (fi - (n as f64 - 1.) / 2.) * 12.;
                let face = if act == Act::Browse {
                    spot.face + std::f64::consts::PI
                } else {
                    spot.face
                };
                (spot.x + spot.ux * k, spot.y + spot.uy * k, face)
            }
            _ => {
                let a = seed * (628. / 100.) + fi / (n.max(1) as f64) * tau;
                let r = if n == 1 { 0. } else { 8. + n as f64 };
                (
                    spot.x + a.cos() * r,
                    spot.y + a.sin() * r,
                    a + std::f64::consts::PI,
                )
            }
        };
        out.push(Hang {
            key: format!("{base}:{i}"),
            x,
            y,
            face,
            act,
            group: base.into(),
            gx: spot.x,
            gy: spot.y,
            bench: false,
        });
    }
}

/// Parkbelegung: Grünflächen füllen sich nachmittags, am Wochenende mehr.
pub fn park_level(minutes: f64, day: u32) -> f64 {
    let m = wrap(minutes);
    let t = if !(600. ..=1260.).contains(&m) {
        0.
    } else if m < 780. {
        (m - 600.) / 180.
    } else if m < 1140. {
        1.
    } else {
        1. - (m - 1140.) / 120.
    };
    t * if is_weekend(day) { 1. } else { 0.55 }
}

fn bbox(rings: &[Vec<(f64, f64)>]) -> Option<Rect> {
    let r = rings.first()?;
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in r {
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    (x0 <= x1).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// Freier Punkt (weder Haus noch Fahrbahn) in einer Fläche, aus dem Ort-Hash.
fn free_spot_in(
    city: &mut City,
    b: &Rect,
    rings: &[Vec<(f64, f64)>],
    g: f64,
    tries: usize,
) -> Option<(f64, f64)> {
    for t in 0..tries {
        let x = b.x + hl(&[b.x, g, t as f64, 1.]) * b.w;
        let y = b.y + hl(&[b.y, g, t as f64, 2.]) * b.h;
        if point_in_rings(x, y, rings)
            && city.in_building(x, y).is_none()
            && city.on_road(x, y, 0., None).is_none()
        {
            return Some((x, y));
        }
    }
    None
}

/// Alle gewünschten Plätze im Umkreis (deterministisch).
pub fn life_spots(
    city: &mut City,
    cache: &mut LifeCache,
    cx: f64,
    cy: f64,
    minutes: f64,
    day: u32,
    radius: f64,
) -> Vec<Hang> {
    let mut out = Vec::new();
    let bx = Rect::around(cx, cy, radius);
    let hr = (wrap(minutes) / 60.).floor();
    let pois: Vec<Poi> = city
        .pois
        .query(&bx)
        .into_iter()
        .map(|h| city.pois.get(h).clone())
        .collect();
    for q in &pois {
        let Some((act, n)) = activity_for(q, minutes, day) else {
            continue;
        };
        if n == 0 {
            continue;
        }
        if let Some(f) = cache.front(city, q.x, q.y) {
            let cap = if act == Act::Queue { 16 } else { 6 };
            arrange(
                f,
                n.min(cap),
                act,
                hl(&[q.x, q.y]),
                &mut out,
                &format!("p{},{}", q.x, q.y),
            );
        }
    }
    // Der Späti aus dem Auftrag: tagsüber ein, zwei Leute davor, abends Stammgäste mit Flasche
    let sp = city.places.giver;
    if (sp.x - cx).abs() < radius && (sp.y - cy).abs() < radius {
        let m = wrap(minutes);
        let night = nightlife(m, day);
        let evening = in_hours(m, 1020., 240.);
        let n = if evening {
            2 + (night * 3.).round() as usize
        } else if in_hours(m, 480., 1020.) {
            1
        } else {
            0
        };
        if n > 0
            && let Some(f) = cache.front(city, sp.x, sp.y)
        {
            let f = Front {
                x: f.x + f.ux * 30.,
                y: f.y + f.uy * 30.,
                ..f
            };
            let act = if evening { Act::Drink } else { Act::Chat };
            arrange(f, n, act, 0.3, &mut out, "spaeti");
        }
    }
    let occ = if in_hours(minutes, 480., 1200.) {
        0.35
    } else if in_hours(minutes, 1200., 1380.) {
        0.2
    } else {
        0.04
    };
    let benches: Vec<(f64, f64)> = city
        .furn
        .query(&bx)
        .into_iter()
        .map(|h| city.furn.get(h))
        .filter(|f| f.kind == FURN_BENCH)
        .map(|f| (f.x, f.y))
        .collect();
    for (x, y) in benches {
        if hl(&[x, y, hr, day as f64]) >= occ {
            continue;
        }
        let a = cache.bench_angle(city, x, y);
        let n = if hl(&[y, x, hr]) < 0.4 { 2 } else { 1 };
        let g = format!("b{x},{y}");
        for i in 0..n {
            let u = if n == 1 { 0. } else { (i as f64 - 0.5) * 10. };
            out.push(Hang {
                key: format!("{g}:{i}"),
                x: x + a.cos() * u,
                y: y + a.sin() * u,
                face: a + std::f64::consts::FRAC_PI_2,
                act: Act::Sit,
                group: g.clone(),
                gx: x,
                gy: y,
                bench: true,
            });
        }
    }
    let park = park_level(minutes, day);
    if park > 0. {
        let lawns: Vec<Vec<Vec<(f64, f64)>>> = city
            .polys
            .query(&bx)
            .into_iter()
            .map(|h| city.polys.get(h))
            .filter(|p| {
                matches!(
                    p.kind,
                    PolyKind::Area {
                        kind: AREA_GRASS,
                        ..
                    }
                )
            })
            .map(|p| p.rings.clone())
            .collect();
        for rings in lawns {
            let Some(b) = bbox(&rings) else { continue };
            if b.w * b.h < 400_000. {
                continue; // unter ~4 000 m² Hüllfläche: Beet, Mittelstreifen
            }
            let groups = (5f64)
                .min(((b.w * b.h / 1_500_000.).floor() + 1.) * park + hl(&[b.x, b.y, hr]) * 0.9)
                .floor() as usize;
            for g in 0..groups {
                let Some((x, y)) = free_spot_in(city, &b, &rings, g as f64, 12) else {
                    continue;
                };
                let n = 2 + (hl(&[b.x, b.y, g as f64, day as f64]) * 3.).floor() as usize;
                let spot = Front {
                    x,
                    y,
                    ux: 1.,
                    uy: 0.,
                    face: 0.,
                };
                let base = format!("a{},{}:{g}", b.x.round(), b.y.round());
                arrange(spot, n, Act::Lie, hl(&[g as f64, b.x]), &mut out, &base);
            }
        }
    }
    out
}

/// Bbox einer Fläche (für Tiere).
pub fn area_bbox(rings: &[Vec<(f64, f64)>]) -> Option<Rect> {
    bbox(rings)
}
/// Freier Punkt in einer Fläche (für Tiere).
pub fn free_point(
    city: &mut City,
    b: &Rect,
    rings: &[Vec<(f64, f64)>],
    tries: usize,
) -> Option<(f64, f64)> {
    for t in 0..tries {
        let x = b.x + h(11., &[b.x, t as f64, 1.]) * b.w;
        let y = b.y + h(11., &[b.y, t as f64, 2.]) * b.h;
        if point_in_rings(x, y, rings)
            && city.in_building(x, y).is_none()
            && city.on_road(x, y, 0., None).is_none()
        {
            return Some((x, y));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn poi(cat: &'static str, kind: &str) -> Poi {
        Poi {
            x: 1000.,
            y: 2000.,
            cat,
            name: String::new(),
            kind: kind.into(),
        }
    }
    #[test]
    fn activities_follow_the_clock() {
        let club = poi("drink", "nightclub");
        assert_eq!(activity_for(&club, 1400., 4).map(|a| a.0), Some(Act::Queue));
        assert_eq!(
            activity_for(&club, 1400., 1),
            None,
            "Montagnacht keine Schlange"
        );
        assert_eq!(activity_for(&club, 720., 4), None);
        assert!(
            activity_for(&poi("bus", ""), 180., 2).is_none(),
            "nachts um 3 wartet niemand"
        );
        assert_eq!(
            activity_for(&poi("cafe", ""), 600., 2).map(|a| a.0),
            Some(Act::Sit)
        );
        assert_eq!(activity_for(&poi("cafe", ""), 1300., 2), None);
        let spaeti = poi("shop", "convenience");
        assert_eq!(
            activity_for(&spaeti, 1320., 4).map(|a| a.0),
            Some(Act::Drink)
        );
        // deterministisch: gleiche Stunde, gleiche Szene
        assert_eq!(
            activity_for(&poi("food", ""), 700., 3),
            activity_for(&poi("food", ""), 715., 3)
        );
    }
    #[test]
    fn groups_are_arranged() {
        let f = Front {
            x: 0.,
            y: 0.,
            ux: 1.,
            uy: 0.,
            face: 0.,
        };
        let mut out = Vec::new();
        arrange(f, 4, Act::Queue, 0.2, &mut out, "q");
        assert_eq!(out.len(), 4);
        assert_eq!(out[3].x, 27.);
        assert!(out.iter().all(|s| s.y == 0. && s.group == "q"));
        out.clear();
        arrange(f, 3, Act::Chat, 0.2, &mut out, "c");
        for s in &out {
            assert!((s.x.hypot(s.y) - 11.).abs() < 1e-9, "Kreis um die Mitte");
        }
        assert_eq!(park_level(900., 6), 1.);
        assert!((park_level(900., 1) - 0.55).abs() < 1e-9);
        assert_eq!(park_level(300., 6), 0.);
    }
}
