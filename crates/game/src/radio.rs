//! Autoradio im Spiel: welcher Sender in welchem Auto läuft (Klang und Streaming: `berlin_audio::radio`).
//!
//! Jedes Auto merkt sich seinen Sender. Ein Auto, in dem man noch nicht saß, läuft auf einem Sender aus seiner
//! Nummer (manchmal ist das Radio aus) – so klingt jeder geklaute Wagen anders. Durchgeschaltet wird reihum
//! AUS → 1 … 12 → AUS. Radio gibt es nur in geschlossenen Autos (nicht auf Fahrrad, Roller, Motorrad).
use berlin_sim::math::hash01;
use std::collections::HashMap;

/// So lange zeigt das HUD Sender und Genre nach einem Wechsel bzw. dem Einsteigen (s)
pub const SHOW_S: f64 = 3.;
/// Anteil der Autos, deren Radio beim Einsteigen aus ist
const OFF_SHARE: f64 = 0.12;

#[derive(Debug, Default)]
pub struct RadioCtl {
    per_car: HashMap<u32, Option<usize>>,
    car: Option<u32>,
    /// wann zuletzt umgeschaltet bzw. eingestiegen (für die Einblendung)
    pub shown_at: f64,
}

/// Startsender eines Autos (fest je Nummer, ohne Welt-Zufall): `None` = Radio aus.
pub fn default_for(car: u32, n: usize) -> Option<usize> {
    if n == 0 {
        return None;
    }
    let h = hash01(car as f64 * 3.17 + 0.71);
    if h < OFF_SHARE {
        None
    } else {
        let u = (h - OFF_SHARE) / (1. - OFF_SHARE);
        Some(((u * n as f64) as usize).min(n - 1))
    }
}

/// Nächster (`dir` = 1) bzw. vorheriger (−1) Eintrag der Runde AUS, 0 … n−1.
pub fn cycle(cur: Option<usize>, dir: i32, n: usize) -> Option<usize> {
    let len = n as i32 + 1;
    // Position in der Runde: 0 = AUS, k + 1 = Sender k
    let pos = cur.map_or(0, |k| k as i32 + 1);
    let next = (pos + dir).rem_euclid(len);
    (next > 0).then(|| (next - 1) as usize)
}

impl RadioCtl {
    /// Ein Schritt: `car` = Auto, in dem man sitzt (mit Radio), `next`/`prev` = Tasten. Liefert den gewünschten
    /// Sender (None = aus oder kein Auto).
    pub fn step(
        &mut self,
        car: Option<u32>,
        next: bool,
        prev: bool,
        n: usize,
        t: f64,
    ) -> Option<usize> {
        let Some(id) = car else {
            self.car = None;
            return None;
        };
        if self.car != Some(id) {
            self.car = Some(id);
            self.shown_at = t;
        }
        let cur = *self.per_car.entry(id).or_insert_with(|| default_for(id, n));
        let dir = next as i32 - prev as i32;
        if dir != 0 {
            let new = cycle(cur, dir, n);
            self.per_car.insert(id, new);
            self.shown_at = t;
            return new;
        }
        cur
    }
    /// Sender des aktuellen Autos direkt setzen (Befehlszeile). `false` ohne Auto.
    pub fn set(&mut self, station: Option<usize>, t: f64) -> bool {
        let Some(id) = self.car else { return false };
        self.per_car.insert(id, station);
        self.shown_at = t;
        true
    }
    /// Sender des aktuellen Autos (None = aus oder kein Auto).
    pub fn current(&self) -> Option<usize> {
        self.car
            .and_then(|id| self.per_car.get(&id).copied().flatten())
    }
    pub fn in_car(&self) -> bool {
        self.car.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_through_off_and_all_stations() {
        let n = 12;
        assert_eq!(cycle(None, 1, n), Some(0));
        assert_eq!(cycle(Some(0), 1, n), Some(1));
        assert_eq!(cycle(Some(11), 1, n), None, "nach dem letzten kommt AUS");
        assert_eq!(cycle(None, -1, n), Some(11));
        assert_eq!(cycle(Some(0), -1, n), None);
        // einmal rundherum: 13 Schritte zurück zum Start
        let mut c = Some(4);
        for _ in 0..13 {
            c = cycle(c, 1, n);
        }
        assert_eq!(c, Some(4));
    }

    #[test]
    fn each_car_keeps_its_station_and_starts_on_its_own() {
        let n = 12;
        let mut r = RadioCtl::default();
        // Startsender aus der Nummer; über viele Autos sind alle Sender und AUS vertreten
        let mut seen = vec![false; n + 1];
        for id in 0..2000 {
            seen[default_for(id, n).unwrap_or(n)] = true;
        }
        assert!(seen.iter().all(|&s| s));
        let a = r.step(Some(7), false, false, n, 1.);
        assert_eq!(a, default_for(7, n));
        assert_eq!(r.shown_at, 1., "Einsteigen zeigt den Sender");
        let a2 = r.step(Some(7), true, false, n, 2.);
        assert_eq!(a2, cycle(a, 1, n));
        assert_eq!(r.shown_at, 2.);
        // aussteigen: aus; anderes Auto: dessen Sender; zurück: der gemerkte
        assert_eq!(r.step(None, false, false, n, 3.), None);
        assert!(!r.in_car());
        r.step(Some(8), false, false, n, 4.);
        assert_eq!(r.step(Some(7), false, false, n, 5.), a2);
        assert!(r.set(Some(3), 6.));
        assert_eq!(r.current(), Some(3));
    }
}
