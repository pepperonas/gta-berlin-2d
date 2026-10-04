//! Fahrzeugarten jenseits des Pkw (Port von `fleet.js`): wann und wo sie unterwegs sind und ihr Arbeitsrhythmus –
//! Paketwagen halten in zweiter Reihe, das Müllauto der BSR fährt morgens von Tonne zu Tonne. Maße und Leistung
//! stehen in `carmodels::KINDS`; Blaulichtfahrzeuge entstehen über Einsätze (`services.rs`).

/// Samstag (5) und Sonntag (6), Tage ab Montag = 0.
pub fn is_weekend(day: u32) -> bool {
    day == 5 || day == 6
}

fn in_hours(m: f64, a: f64, b: f64) -> bool {
    m >= a && m < b
}

/// Fahrzeugart für einen neuen Verkehrsteilnehmer auf einer Straße der Klasse `cls`; `r` ∈ [0,1).
pub fn pick_kind(minutes: f64, day: u32, cls: u8, r: f64) -> &'static str {
    let m = minutes.rem_euclid(1440.);
    let (we, sun) = (is_weekend(day), day == 6);
    let mut p = 0.;
    let truck = if in_hours(m, 300., 1260.) {
        if cls <= 4 {
            if we { 0.03 } else { 0.1 }
        } else if cls <= 6 {
            if we { 0.01 } else { 0.04 }
        } else {
            0.
        }
    } else if cls <= 3 {
        0.02
    } else {
        0.
    };
    p += truck;
    if p > r {
        return "truck";
    }
    let delivery = if !sun && in_hours(m, 480., 1170.) && cls >= 4 {
        if we { 0.04 } else { 0.08 }
    } else {
        0.
    };
    p += delivery;
    if p > r {
        return "delivery";
    }
    let garbage = if !we && in_hours(m, 360., 720.) && (5..=8).contains(&cls) {
        0.05
    } else {
        0.
    };
    p += garbage;
    if p > r {
        return "garbage";
    }
    // Zweiräder: tagsüber, am Wochenende mehr (Ausflug), nachts kaum
    let moto = if in_hours(m, 420., 1260.) {
        if we { 0.05 } else { 0.03 }
    } else {
        0.005
    };
    p += moto * 0.55;
    if p > r {
        return "motorcycle";
    }
    p += moto * 0.45;
    if p > r {
        return "scooter";
    }
    "car"
}

/// Abstand bis zum nächsten Arbeitshalt (px Fahrstrecke).
pub fn next_stop_after(kind: &str, r: f64) -> f64 {
    match kind {
        "delivery" => 1500. + r * 3500., // alle 150–500 m ein Paket
        "garbage" => 350. + r * 450.,    // alle 35–80 m Tonnen
        _ => f64::INFINITY,
    }
}
/// Dauer eines Arbeitshalts (s).
pub fn stop_duration(kind: &str, r: f64) -> f64 {
    match kind {
        "delivery" => 12. + r * 16.,
        "garbage" => 6. + r * 4.,
        _ => 0.,
    }
}
/// Darf hier gehalten werden? Nicht auf Hauptstraßen (Lieferwagen) und nicht nahe der Kreuzung.
pub fn may_stop_on(kind: &str, cls: u8, to_end: f64, from_start: f64) -> bool {
    if to_end < 250. || from_start < 120. {
        return false;
    }
    match kind {
        "garbage" => (5..=8).contains(&cls),
        "delivery" => cls >= 4,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kinds_follow_time_day_and_street() {
        // Montag 10 Uhr, Hauptstraße: erst Lkw (10 %), dann Lieferwagen (8 %), dann Zweiräder
        assert_eq!(pick_kind(600., 0, 4, 0.05), "truck");
        assert_eq!(pick_kind(600., 0, 4, 0.15), "delivery");
        assert_eq!(pick_kind(600., 0, 4, 0.185), "motorcycle");
        assert_eq!(pick_kind(600., 0, 4, 0.5), "car");
        // Müllauto nur werktags morgens in Wohnstraßen
        assert_eq!(pick_kind(420., 1, 7, 0.03), "garbage");
        assert_ne!(pick_kind(420., 5, 7, 0.03), "garbage");
        assert_ne!(pick_kind(900., 1, 7, 0.03), "garbage");
        // sonntags keine Pakete, nachts kaum Zweiräder
        assert_ne!(pick_kind(600., 6, 5, 0.02), "delivery");
        assert_eq!(pick_kind(120., 0, 7, 0.01), "car");
        // Anteile über viele Würfe: Pkw bleiben die große Mehrheit
        let n = (0..1000)
            .filter(|&i| pick_kind(600., 2, 5, i as f64 / 1000.) == "car")
            .count();
        assert!(
            (780..820).contains(&n),
            "{n}: 4 % Lkw, 8 % Pakete, 5 % Müll, 3 % Zweiräder"
        );
    }
    #[test]
    fn work_stops() {
        assert!((1500. ..=5000.).contains(&next_stop_after("delivery", 0.5)));
        assert!(next_stop_after("car", 0.5).is_infinite());
        assert!(may_stop_on("garbage", 6, 300., 200.));
        assert!(
            !may_stop_on("garbage", 3, 300., 200.),
            "nicht auf der Hauptstraße"
        );
        assert!(
            !may_stop_on("delivery", 5, 200., 200.),
            "nicht vor der Kreuzung"
        );
        assert_eq!(stop_duration("garbage", 0.), 6.);
    }
}
