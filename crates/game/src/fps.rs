//! Bildratenmessung für die Befehlszeile `fps` (`main.js fpsMeter` + `hud.drawFps`): gleitendes Fenster der letzten
//! Bildabstände (eine Sekunde), dazu die mittlere Arbeitszeit je Bild (Simulation, Aufbau, Zeichnen bis zur Abgabe).
//! Rein, ohne Uhr: die Engine misst und reicht die Zeiten durch.
use std::collections::VecDeque;

/// Fensterlänge in Sekunden: lang genug zum Ablesen, kurz genug, um Einbrüche zu zeigen.
const WINDOW_S: f32 = 1.;

#[derive(Debug, Default, Clone)]
pub struct Meter {
    frames: VecDeque<(f32, f32)>,
    sum_dt: f32,
    sum_work: f32,
}

impl Meter {
    /// Ein Bild: Abstand zum vorigen (s) und Arbeitszeit (ms).
    pub fn push(&mut self, dt: f32, work_ms: f32) {
        if !(dt > 0. && dt.is_finite()) {
            return;
        }
        self.frames.push_back((dt, work_ms));
        self.sum_dt += dt;
        self.sum_work += work_ms;
        while self.sum_dt > WINDOW_S && self.frames.len() > 1 {
            let (d, w) = self.frames.pop_front().expect("nicht leer");
            self.sum_dt -= d;
            self.sum_work -= w;
        }
    }
    /// Bilder je Sekunde im Fenster.
    pub fn fps(&self) -> f32 {
        if self.sum_dt > 0. {
            self.frames.len() as f32 / self.sum_dt
        } else {
            0.
        }
    }
    /// Mittlere Arbeitszeit je Bild (ms).
    pub fn work_ms(&self) -> f32 {
        if self.frames.is_empty() {
            0.
        } else {
            self.sum_work / self.frames.len() as f32
        }
    }
    /// Längster Bildabstand im Fenster (ms) – zeigt Ruckler, die der Mittelwert verschluckt.
    pub fn worst_ms(&self) -> f32 {
        self.frames.iter().map(|f| f.0).fold(0., f32::max) * 1000.
    }
    /// Anzeigezeile wie im Browser: Bildrate, Arbeitszeit, längstes Bild.
    pub fn line(&self) -> String {
        format!(
            "{:.0} fps · {:.1} ms je Bild · längstes {:.0} ms",
            self.fps(),
            self.work_ms(),
            self.worst_ms()
        )
    }
    /// Farbe wie in der Browserfassung: grün ab 50, gelb ab 30, sonst rot.
    pub fn color(&self) -> [f32; 4] {
        let f = self.fps();
        if f >= 50. {
            [0.56, 1., 0.56, 1.]
        } else if f >= 30. {
            [1., 0.84, 0.25, 1.]
        } else {
            [1., 0.53, 0.53, 1.]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_rate_over_one_second_and_shows_hitches() {
        let mut m = Meter::default();
        for _ in 0..240 {
            m.push(1. / 120., 3.);
        }
        assert!((m.fps() - 120.).abs() < 1.5, "{}", m.fps());
        assert!((m.work_ms() - 3.).abs() < 1e-3);
        // altes Fenster fällt heraus: danach 60 Bilder/s
        for _ in 0..120 {
            m.push(1. / 60., 9.);
        }
        assert!((m.fps() - 60.).abs() < 1., "{}", m.fps());
        assert!((m.work_ms() - 9.).abs() < 0.1);
        // ein Ruckler bleibt im Fenster sichtbar
        m.push(0.1, 9.);
        assert!(m.worst_ms() >= 99., "{}", m.worst_ms());
        assert!(m.line().contains("fps"));
        // ungültige Abstände zählen nicht
        let n = m.fps();
        m.push(0., 1.);
        m.push(f32::NAN, 1.);
        assert_eq!(m.fps(), n);
    }

    #[test]
    fn color_follows_rate() {
        let mut m = Meter::default();
        for _ in 0..60 {
            m.push(1. / 60., 1.);
        }
        assert_eq!(m.color()[1], 1.);
        let mut s = Meter::default();
        for _ in 0..20 {
            s.push(1. / 20., 1.);
        }
        assert_eq!(s.color()[0], 1.);
        assert!(s.color()[1] < 0.6);
    }
}
