//! Umgebungsklang an der Kamera (Port der in der Simulation vorhandenen Teile von `ambience.js`): Stadtrauschen,
//! Verkehr, Vögel in Grün und Bäumen bei Tag, Wasser am Ufer, Regen, Wind und Böen, Dämpfung im Auto und durch
//! Schnee. Nachtleben, Hochbahn und Martinshörner folgen mit Nahverkehr und Einsatzfahrzeugen.
use crate::city::{CircleKind, PolyKind, Solid, bounds_of};
use crate::collision::Rect;
use crate::world::World;
use berlin_map_loader::citycodes::area_kind;

pub const HEAR: f64 = 1200.;

/// Mischung (alle Werte 0…1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Mix {
    pub hum: f64,
    pub traffic: f64,
    pub birds: f64,
    pub water: f64,
    pub rain: f64,
    pub wind: f64,
    /// Böenstärke 0…1 (hebt das Windband und lässt Kanten pfeifen)
    pub gust: f64,
    /// Dämpfung von außen (Karosserie, Schneedecke)
    pub muffle: f64,
    pub in_car: bool,
}

fn wrap(m: f64) -> f64 {
    ((m % 1440.) + 1440.) % 1440.
}

/// Vogelgesang über den Tag: Morgenchor 4:30–8:00, tagsüber leiser, Abendgesang, nachts still.
pub fn bird_level(minutes: f64) -> f64 {
    let m = wrap(minutes);
    if !(270. ..=1290.).contains(&m) {
        0.
    } else if m < 330. {
        (m - 270.) / 60.
    } else if m < 480. {
        1.
    } else if m < 600. {
        1. - (m - 480.) / 120. * 0.6
    } else if m < 1110. {
        0.4
    } else if m < 1200. {
        0.4 + (m - 1110.) / 90. * 0.3
    } else {
        0.7 * (1. - (m - 1200.) / 90.)
    }
}

pub fn ambience_at(w: &mut World) -> Mix {
    let (cx, cy) = (w.camera.x, w.camera.y);
    let box_ = Rect::around(cx, cy, HEAR);
    let (mut green, mut water) = (0f64, 0f64);
    for h in w.city.polys.query(&box_) {
        let p = w.city.polys.get(h);
        let b = bounds_of(&p.rings[0]);
        match p.kind {
            PolyKind::Area {
                kind:
                    area_kind::GRASS | area_kind::WOOD | area_kind::CEMETERY | area_kind::ALLOTMENTS,
                ..
            } => {
                green += (b.w * b.h / 4e6).min(1.);
            }
            PolyKind::Water
                if (b.x + b.w / 2. - cx).abs() < b.w / 2. + 400.
                    && (b.y + b.h / 2. - cy).abs() < b.h / 2. + 400. =>
            {
                water = 1.
            }
            _ => {}
        }
    }
    for h in w.city.solids.query(&box_) {
        if matches!(
            w.city.solids.get(h),
            Solid::Circle {
                kind: CircleKind::Tree,
                ..
            }
        ) {
            green += 0.012;
        }
    }
    let mut traffic = 0.;
    for c in &w.cars {
        if c.driver.is_none() || c.wrecked {
            continue;
        }
        let d = (c.x - cx).hypot(c.y - cy);
        if d < HEAR {
            traffic += c.speed() / 250.
                * (1. - d / HEAR)
                * if matches!(c.kind, "truck" | "garbage") {
                    2.
                } else {
                    1.
                };
        }
    }
    let in_car = w
        .player_car()
        .is_some_and(|c| !crate::carmodels::is_open_kind(c.kind));
    let night = wrap(w.clock) < 360. || wrap(w.clock) > 1260.;
    let sky = w.sky.p;
    let hush = 1. - 0.45 * w.weather.snow.clamp(0., 1.) - 0.2 * sky.snow.clamp(0., 1.);
    let gust = crate::weather::gust_at(sky.storm, w.time);
    Mix {
        hum: if night { 0.35 } else { 0.6 } * hush,
        traffic: (traffic / 2.).clamp(0., 1.) * hush,
        // bei Regen, Sturm und Schnee schweigen die Vögel
        birds: (green.min(1.)
            * bird_level(w.clock)
            * (1. - (sky.rain + sky.storm + sky.snow).min(1.)))
        .clamp(0., 1.),
        water: water * (0.4 + if night { 0.2 } else { 0. }),
        rain: sky.rain.min(1.6),
        wind: (sky.storm * gust * 0.8 + 0.15 * sky.snow * sky.storm).clamp(0., 1.),
        gust: if sky.storm > 0. {
            ((gust - 0.2) / 1.6).clamp(0., 1.)
        } else {
            0.
        },
        muffle: ((if in_car { 0.65 } else { 0. }) + 0.35 * w.weather.snow.clamp(0., 1.))
            .clamp(0., 1.),
        in_car,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn birds_follow_the_day() {
        assert_eq!(bird_level(3. * 60.), 0.);
        assert_eq!(bird_level(6. * 60.), 1.);
        assert_eq!(bird_level(12. * 60.), 0.4);
        assert!((bird_level(20. * 60.) - 0.7).abs() < 1e-12);
        assert_eq!(bird_level(1290.), 0.);
        assert_eq!(bird_level(-720.), 0.4);
    }
}
