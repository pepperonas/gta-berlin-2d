//! Tagesrhythmus der Stadt (Port von `rhythm.js`, rein rechnerisch): wie viel Verkehr und wie viele Menschen zu welcher
//! Uhrzeit an welchem Wochentag unterwegs sind, und wie belebt ein Ort ist (gezählte Kfz je Werktag, Einwohnerdichte,
//! Lokale in der Nähe). `world.rs` stellt daraus die Zielbevölkerung um die Kamera ein.
use crate::city::City;
use crate::collision::Rect;

pub const DAYS: [&str; 7] = ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"];

// Stützstellen [Minute, Anteil] – stetig interpoliert; alle Kurven treffen sich um Mitternacht im selben Wert.
const TRAFFIC_WORKDAY: [(f64, f64); 11] = [
    (0., 0.26),
    (240., 0.12),
    (360., 0.55),
    (450., 1.),
    (570., 0.72),
    (720., 0.78),
    (960., 0.95),
    (1050., 1.),
    (1170., 0.7),
    (1320., 0.4),
    (1440., 0.26),
];
const TRAFFIC_WEEKEND: [(f64, f64); 8] = [
    (0., 0.26),
    (300., 0.14),
    (480., 0.35),
    (660., 0.7),
    (900., 0.8),
    (1080., 0.75),
    (1260., 0.5),
    (1440., 0.26),
];
const PEOPLE_WORKDAY: [(f64, f64); 9] = [
    (0., 0.28),
    (240., 0.08),
    (390., 0.35),
    (480., 0.7),
    (720., 0.95),
    (1020., 1.),
    (1200., 0.75),
    (1320., 0.45),
    (1440., 0.28),
];
const PEOPLE_WEEKEND: [(f64, f64); 8] = [
    (0., 0.28),
    (300., 0.1),
    (540., 0.35),
    (720., 0.85),
    (900., 1.),
    (1140., 0.95),
    (1320., 0.7),
    (1440., 0.28),
];

fn curve(tab: &[(f64, f64)], m: f64) -> f64 {
    for w in tab.windows(2) {
        let ((a, va), (b, vb)) = (w[0], w[1]);
        if m <= b {
            let d = if b - a == 0. { 1. } else { b - a };
            return va + (vb - va) * (m - a) / d;
        }
    }
    tab[tab.len() - 1].1
}
/// Minute in 0…1440.
pub fn wrap(m: f64) -> f64 {
    m.rem_euclid(1440.)
}
pub fn is_weekend(day: u32) -> bool {
    day == 5 || day == 6
}
pub fn day_name(day: u32) -> &'static str {
    DAYS[(day % 7) as usize]
}
pub fn traffic_level(minutes: f64, day: u32) -> f64 {
    let t: &[(f64, f64)] = if is_weekend(day) {
        &TRAFFIC_WEEKEND
    } else {
        &TRAFFIC_WORKDAY
    };
    curve(t, wrap(minutes))
}
pub fn people_level(minutes: f64, day: u32) -> f64 {
    let t: &[(f64, f64)] = if is_weekend(day) {
        &PEOPLE_WEEKEND
    } else {
        &PEOPLE_WORKDAY
    };
    curve(t, wrap(minutes))
}

/// Nachtleben: 0 tagsüber, abends ansteigend, Freitag-/Samstagnacht voll (die Nacht zählt bis 6 Uhr zum Vortag).
pub fn nightlife(minutes: f64, day: u32) -> f64 {
    let m = wrap(minutes);
    let night = if m < 360. { (day + 6) % 7 } else { day };
    let party = match night {
        4 | 5 => 1.,
        3 | 6 => 0.55,
        _ => 0.35,
    };
    let t = if m < 360. {
        if m < 240. { 1. } else { 1. - (m - 240.) / 120. }
    } else if m >= 1260. {
        1.
    } else if m >= 1110. {
        (m - 1110.) / 150.
    } else {
        0.
    };
    party * t
}

/// Örtlicher Verkehr: mittlere Kfz je Werktag der Straßen im Umkreis (nach Länge gewichtet), 1 ≈ 8 000 Kfz/Tag.
pub fn local_traffic(city: &mut City, x: f64, y: f64, r: f64) -> f64 {
    let (mut sum, mut len) = (0., 0.);
    for h in city.edge_segs.query(&Rect::around(x, y, r)) {
        let s = *city.edge_segs.get(h);
        let Some(e) = s.edge.and_then(|id| city.edges.get(&id)) else {
            continue;
        };
        if e.cls > 8 {
            continue;
        }
        let l = (s.bx - s.ax).hypot(s.by - s.ay);
        sum += e.dtv * l;
        len += l;
    }
    if len == 0. {
        return 0.5;
    }
    (sum / len / 8000.).sqrt().clamp(0.3, 1.8)
}

/// Einwohner je Hektar im Umkreis (Mittel der bewohnten Zellen).
pub fn density_near(city: &City, x: f64, y: f64, r: f64, step: f64) -> f64 {
    let (mut sum, mut n) = (0., 0);
    let mut dx = -r;
    while dx <= r {
        let mut dy = -r;
        while dy <= r {
            let v = city.density_at(x + dx, y + dy);
            if v > 0. {
                sum += v;
                n += 1;
            }
            dy += step;
        }
        dx += step;
    }
    if n > 0 { sum / n as f64 } else { 0. }
}

/// Örtliche Belebung zu Fuß: Wohndichte (1 ≈ 200 EW/ha) plus Geschäfte und Lokale im Umkreis.
pub fn local_people(city: &mut City, x: f64, y: f64) -> f64 {
    let dens = density_near(city, x, y, 1500., 640.) / 200.;
    let shops = city
        .pois
        .query(&Rect::around(x, y, 2500.))
        .into_iter()
        .filter(|&h| city.pois.get(h).cat != "bus")
        .count() as f64;
    (0.2 + dens * 0.45 + (shops / 60.).min(1.) * 0.6).clamp(0.2, 1.3)
}

/// Bars, Clubs und Spätis im Umkreis (für das Nachtleben).
pub fn night_spots(city: &mut City, x: f64, y: f64, r: f64) -> usize {
    city.pois
        .query(&Rect::around(x, y, r))
        .into_iter()
        .filter(|&h| {
            let q = city.pois.get(h);
            q.cat == "drink" || q.kind == "convenience" || q.kind == "nightclub"
        })
        .count()
}

/// Zielbevölkerung um die Kamera aus den Grundwerten (Autos, Passanten).
pub fn population_targets(
    city: &mut City,
    x: f64,
    y: f64,
    minutes: f64,
    day: u32,
    base: (usize, usize),
) -> (usize, usize) {
    let (bc, bp) = (base.0 as f64, base.1 as f64);
    let cars = bc * traffic_level(minutes, day) * local_traffic(city, x, y, 3000.);
    let night = nightlife(minutes, day) * (night_spots(city, x, y, 2500.) as f64 / 12.).min(1.);
    let peds = bp * (people_level(minutes, day) * local_people(city, x, y) + night * 0.9);
    (
        cars.max(3.).min(bc * 1.7).round() as usize,
        peds.max(4.).min(bp * 1.6).round() as usize,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curves_meet_at_midnight_and_peak_in_rush_hour() {
        for d in 0..7 {
            assert!((traffic_level(0., d) - 0.26).abs() < 1e-9);
            assert!((people_level(1440., d) - 0.28).abs() < 1e-9);
        }
        assert_eq!(traffic_level(450., 1), 1.);
        assert!(traffic_level(450., 6) < 0.5, "Sonntagmorgen ruhig");
        assert!(people_level(240., 2) < 0.1);
        assert!(
            (traffic_level(405., 0) - 0.775).abs() < 1e-9,
            "linear zwischen den Stützstellen"
        );
    }
    #[test]
    fn nightlife_belongs_to_the_evening_before() {
        assert_eq!(nightlife(720., 4), 0., "mittags nichts");
        assert_eq!(nightlife(1320., 4), 1., "Freitagnacht");
        assert_eq!(nightlife(120., 5), 1., "Samstag 2 Uhr zählt zum Freitag");
        assert!((nightlife(120., 1) - 0.35).abs() < 1e-9);
        assert!(
            (nightlife(300., 5) - 0.5).abs() < 1e-9,
            "klingt bis 6 Uhr aus"
        );
        assert_eq!(day_name(11), "Fr");
    }
}
