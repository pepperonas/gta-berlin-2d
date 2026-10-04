//! Umgebungsklang an der Kamera (Port der in der Simulation vorhandenen Teile von `ambience.js`): Stadtrauschen,
//! Verkehr, Vögel in Grün und Bäumen bei Tag, Wasser am Ufer, Regen, Wind und Böen, Dämpfung im Auto und durch
//! Schnee, Martinshörner und Nachtleben (Stimmengewirr vor Bars und Leuten am Lebensplatz, gedämpfte Clubmusik).
//! Dazu das Rumpeln der Bahnen (echte Fahrplanzüge in der Nähe, ohne Fahrplan ein fester Takt an der Hochbahn) und
//! die gedämpfte Halle im U-Bahnhof.
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
    /// nächstes Martinshorn: Lautstärke 0…1 und ob gerade der hohe Ton
    pub siren: f64,
    pub siren_high: bool,
    /// Stimmengewirr (Bars, Leute am Lebensplatz), gedämpfte Musik, Richtung der lautesten Quelle, Anteil Feed
    pub bar: f64,
    pub music: f64,
    pub bar_pan: f64,
    pub night_feed: f64,
    /// Rumpeln vorbeifahrender Bahnen (tiefes Rauschen)
    pub rumble: f64,
    /// im U-Bahnhof (gedämpfte Halle)
    pub station: bool,
}

/// Hochbahn-Hörweite, Takt und Dauer eines Zuges ohne Fahrplan (`ambience.js AMB`)
pub const HOCHBAHN_HEAR: f64 = 450.;
pub const TRAIN_EVERY: f64 = 150.;
pub const TRAIN_LEN: f64 = 14.;

/// Rumpeln echter Fahrplanzüge um (x, y): Straßenbahn bis 250 px und halb so laut, S-/U-Bahn bis 450 px (auch im
/// Tunnel unter der Straße). Busse rumpeln nicht.
pub fn train_rumble(w: &World, x: f64, y: f64) -> f64 {
    use crate::transit::{Mode, point_on_shape, position_at};
    let Some(tr) = w.transit.as_deref() else {
        return 0.;
    };
    let mut r = 0f64;
    for (&id, track) in &w.transit_state.tracked {
        let Some(p) = tr.patterns.get(id) else {
            continue;
        };
        if p.mode == Mode::Bus {
            continue;
        }
        let (radius, k) = if p.mode == Mode::Tram {
            (250., 0.5)
        } else {
            (HOCHBAHN_HEAR, 1.)
        };
        for v in track.veh.iter().filter(|v| !v.gone) {
            let pos = position_at(p, v.tau);
            let (qx, qy, _) = point_on_shape(tr.shape_of(p), pos.s);
            let d = (qx - x).hypot(qy - y);
            if d < radius {
                r = r.max((1. - d / radius) * k);
            }
        }
    }
    r
}

/// Ohne Fahrplan: fester Takt an der Hochbahn (zwei Züge je 150 s, je 14 s), nur in Hörweite einer Hochbahn.
pub fn fixed_rumble(time: f64, hochbahn: f64) -> f64 {
    let phase = time.rem_euclid(TRAIN_EVERY);
    let train = phase < TRAIN_LEN || (phase - TRAIN_EVERY / 2.).abs() < TRAIN_LEN / 2.;
    if hochbahn < HOCHBAHN_HEAR && train {
        1. - hochbahn / HOCHBAHN_HEAR
    } else {
        0.
    }
}

/// so weit hört man ein Martinshorn (px)
pub const SIREN_HEAR: f64 = 3000.;

fn wrap(m: f64) -> f64 {
    ((m % 1440.) + 1440.) % 1440.
}

/// Vogelgesang über den Tag: Morgenchor 4:30–8:00, tagsüber leiser, Abendgesang, nachts still.
/// Hörweite der Kirchenglocke (px)
pub const BELLS_HEAR: f64 = 1800.;

/// Kirchenglocke (`ambience.js bellStrikes`): hat die Uhr seit dem letzten Schritt eine volle Stunde überschritten und
/// steht eine Kirche in Hörweite, schlägt sie die Stunde (1–12), sonst 0.
pub fn bell_strikes(prev_clock: f64, clock: f64, church_near: bool) -> u32 {
    let wrap = |m: f64| m.rem_euclid(1440.);
    let (a, b) = (wrap(prev_clock), wrap(clock));
    let crossed = b < a || (b / 60.).floor() != (a / 60.).floor();
    if !crossed || !church_near {
        return 0;
    }
    match ((b / 60.).floor() as u32) % 12 {
        0 => 12,
        h => h,
    }
}

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
    // im U-Bahnhof: gedämpftes Grundrauschen der Halle, Züge rumpeln laut, von oben kommt kaum etwas an
    if w.in_tunnel_station() {
        return Mix {
            hum: 0.5,
            muffle: 0.9,
            // ferne Züge grollen durch die Wände; der Zug am Bahnsteig selbst klingt über `railsound`
            rumble: (train_rumble(w, cx, cy) * 0.6).clamp(0., 1.),
            station: true,
            ..Mix::default()
        };
    }
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
    // im Auto, im Bus, in der Bahn: draußen gedämpft (die eigene Bahn klingt über `railsound`)
    let riding = w.player.ride.as_ref();
    if riding.is_some_and(|r| r.underground) {
        // im Tunnel: von oben kommt nichts an, andere Züge hört man durch die Röhre
        return Mix {
            hum: 0.15,
            muffle: 0.95,
            in_car: true,
            ..Mix::default()
        };
    }
    let in_car = riding.is_some()
        || w.player_car()
            .is_some_and(|c| !crate::carmodels::is_open_kind(c.kind));
    let night = wrap(w.clock) < 360. || wrap(w.clock) > 1260.;
    let sky = w.sky.p;
    let hush = 1. - 0.45 * w.weather.snow.clamp(0., 1.) - 0.2 * sky.snow.clamp(0., 1.);
    let gust = crate::weather::gust_at(sky.storm, w.time);
    // Martinshorn: das nächste Einsatzfahrzeug mit Sondersignal (bis 300 m hörbar)
    let (mut siren, mut siren_high) = (0f64, false);
    let mut nearest = f64::INFINITY;
    for c in &w.cars {
        if !c.siren {
            continue;
        }
        let d = (c.x - cx).hypot(c.y - cy);
        if d < SIREN_HEAR && d < nearest {
            nearest = d;
            siren = (1. - d / SIREN_HEAR).powi(2);
            siren_high = crate::services::siren_high(w.time + c.id as f64 * 0.37);
        }
    }
    // Leute am Lebensplatz (trinken, rauchen, anstehen, plaudern, sitzen) in der Nähe
    let mut bar = 0.;
    for p in &w.peds {
        if p.state != crate::pedestrians::PedState::Hang {
            continue;
        }
        use crate::life::Act;
        if p.hang.as_ref().is_some_and(|h| {
            matches!(
                h.act,
                Act::Drink | Act::Smoke | Act::Queue | Act::Chat | Act::Sit
            )
        }) {
            let d = (p.x - cx).hypot(p.y - cy);
            if d < 400. {
                bar += 0.12 * (1. - d / 400.);
            }
        }
    }
    let nl = w.nightlife_at(cx, cy);
    // Bahnen: mit Fahrplan das Rumpeln echter Züge, sonst ein fester Takt an der Hochbahn
    let rumble = if w.transit.is_some() {
        train_rumble(w, cx, cy)
    } else {
        let mut hochbahn = f64::INFINITY;
        for h in w.city.rails.query(&Rect::around(cx, cy, HOCHBAHN_HEAR)) {
            let l = w.city.rails.get(h);
            if !l.bridge {
                continue;
            }
            for s in l.pts.windows(2) {
                let d2 = crate::city::seg_dist2(cx, cy, s[0].0, s[0].1, s[1].0, s[1].1);
                hochbahn = hochbahn.min(d2.sqrt());
            }
        }
        fixed_rumble(w.time, hochbahn)
    };
    Mix {
        rumble: rumble.clamp(0., 1.),
        station: false,
        bar: bar.max(nl.crowd).clamp(0., 1.),
        music: nl.music * hush,
        bar_pan: nl.pan,
        night_feed: nl.feed,
        hum: if night { 0.35 } else { 0.6 } * hush,
        traffic: (traffic / 2.).clamp(0., 1.) * hush,
        // bei Regen, Sturm und Schnee schweigen die Vögel
        birds: (green.min(1.)
            * bird_level(w.clock)
            * (1. - (sky.rain + sky.storm + sky.snow).min(1.)))
        .clamp(0., 1.),
        siren,
        siren_high,
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

    #[test]
    fn bells_strike_the_hour_near_a_church() {
        assert_eq!(bell_strikes(14. * 60. + 59.5, 15. * 60. + 0.2, true), 3);
        assert_eq!(bell_strikes(23. * 60. + 59.9, 0.1, true), 12, "Mitternacht");
        assert_eq!(bell_strikes(12. * 60. - 0.1, 12. * 60. + 0.1, true), 12);
        assert_eq!(
            bell_strikes(15. * 60. + 1., 15. * 60. + 2., true),
            0,
            "keine volle Stunde"
        );
        assert_eq!(
            bell_strikes(14. * 60. + 59.5, 15. * 60. + 0.2, false),
            0,
            "keine Kirche"
        );
    }

    #[test]
    fn fixed_rumble_follows_the_beat_and_distance() {
        assert!(fixed_rumble(5., 100.) > 0.7, "Zug unterwegs, nah");
        assert_eq!(fixed_rumble(40., 100.), 0., "zwischen den Zügen");
        assert!(
            fixed_rumble(75., 100.) > 0.7,
            "zweiter Zug zur Hälfte des Takts"
        );
        assert_eq!(fixed_rumble(5., 500.), 0., "zu weit weg");
    }
}
