//! Motorsound-Anzeige (F4 im Entwickler-Build, Konsole `motorsound`): zeigt, was der eigene Sample-Motor spielt –
//! Profil, Drehzahl, Gas, Gang, Begrenzer, aktive Loops mit Überblendgewicht und Tonhöhe – und erlaubt, Drehzahl,
//! Gas, Gang und Profil unabhängig vom Fahren einzustellen. F7 (oder die Zeile „A/B“) spielt im Wechsel die
//! Referenzaufnahme auf dem Pegel der Loops. Bedienung wie die Physik-Anzeige: Bild auf/ab wählt eine Zeile,
//! Komma/Punkt verstellt sie.
use crate::sound::{EngineOverride, EngineView, Listener};
use berlin_engine::hud::{Align, Hud};
use berlin_sim::enginesound::config;
use std::sync::Arc;

const ROWS: [&str; 6] = ["Quelle", "Profil", "Drehzahl", "Gas", "Gang", "A/B"];

pub struct EngineDebug {
    pub open: bool,
    /// Regler statt Fahrzeug
    pub manual: bool,
    pub row: usize,
    pub rpm: f64,
    pub throttle: f64,
    pub gear: usize,
    pub profile: usize,
    /// A: Referenzaufnahme, B: Samples
    pub ab: bool,
    reference: Option<Arc<[f32]>>,
    pub status: String,
}
impl Default for EngineDebug {
    fn default() -> Self {
        Self {
            open: false,
            manual: false,
            row: 0,
            rpm: 3000.,
            throttle: 0.5,
            gear: 3,
            profile: 1,
            ab: false,
            reference: None,
            status: String::new(),
        }
    }
}

impl EngineDebug {
    pub fn select(&mut self, dir: i32) {
        self.row = (self.row as i32 + dir).rem_euclid(ROWS.len() as i32) as usize;
    }
    pub fn adjust(&mut self, dir: f64) {
        let n = config().all_profiles().len();
        match self.row {
            0 => self.manual = !self.manual,
            1 => self.profile = (self.profile as i64 + dir as i64).rem_euclid(n as i64) as usize,
            2 => self.rpm = (self.rpm + dir * 250.).clamp(600., 10000.),
            3 => self.throttle = (self.throttle + dir * 0.1).clamp(0., 1.),
            4 => self.gear = (self.gear as i64 + dir as i64).clamp(1, 7) as usize,
            _ => self.toggle_ab(),
        }
        // wer an Drehzahl, Gas, Gang oder Profil dreht, will die Regler hören
        if (1..=4).contains(&self.row) {
            self.manual = true;
        }
    }
    pub fn toggle_ab(&mut self) {
        self.ab = !self.ab;
        if self.ab && self.reference.is_none() {
            self.reference = berlin_audio::sampler::load_reference();
            if self.reference.is_none() {
                self.ab = false;
                self.status =
                    "Referenz nicht gefunden (data/audio/engine/v10/reference.wav)".into();
            }
        }
    }
    /// Einstellungen an den Klang weitergeben (jedes Bild vor `Listener::frame`).
    pub fn sync(&self, l: &mut Listener) {
        let on = self.open;
        l.engine_override = (on && self.manual).then_some(EngineOverride {
            rpm: self.rpm,
            throttle: self.throttle,
            gear: self.gear,
            profile: self.profile,
        });
        l.reference = if on && self.ab {
            self.reference.clone()
        } else {
            None
        };
    }

    pub fn draw(&self, h: &mut Hud, view: &EngineView) {
        if !self.open {
            return;
        }
        let cfg = config();
        let bank = cfg.bank("v10");
        let wd = 430f32;
        let x0 = h.width - wd - 8.;
        let mut y = 150f32;
        let live: Vec<_> = view.out.layers.iter().filter(|l| l.gain > 0.002).collect();
        let rows = 14 + live.len();
        h.rect(
            x0 - 8.,
            y - 18.,
            wd,
            rows as f32 * 15. + 34.,
            [0., 0., 0., 0.62],
            6.,
        );
        let white = [1., 1., 1., 1.];
        let grey = [0.7, 0.72, 0.76, 1.];
        let gold = [1., 0.8, 0.3, 1.];
        let text = |h: &mut Hud, y: &mut f32, s: &str, c: [f32; 4]| {
            h.text(s, x0, *y, 12., c, Align::Left, false);
            *y += 15.;
        };
        text(
            h,
            &mut y,
            "MOTORSOUND  Bild auf/ab, Komma/Punkt, F7 A/B",
            grey,
        );
        let profiles = cfg.all_profiles();
        let pname = profiles
            .get(self.profile)
            .map_or("?", |p| p.name.as_str())
            .to_owned();
        let values = [
            if self.manual { "Regler" } else { "Fahrzeug" }.to_owned(),
            pname,
            format!("{:.0} U/min", self.rpm),
            format!("{:.0} %", self.throttle * 100.),
            format!("{}", self.gear),
            if self.ab {
                "A – Referenzaufnahme".to_owned()
            } else {
                "B – Samples".to_owned()
            },
        ];
        for (i, (name, v)) in ROWS.iter().zip(values).enumerate() {
            let sel = i == self.row;
            let c = if sel { gold } else { white };
            text(
                h,
                &mut y,
                &format!("{} {name:<9} {v}", if sel { ">" } else { " " }),
                c,
            );
        }
        y += 4.;
        if view.profile.is_empty() {
            text(h, &mut y, "kein Sample-Motor: Fahrzeug ohne Profil", grey);
        } else {
            let o = &view.out;
            text(
                h,
                &mut y,
                &format!(
                    "{}  {:.0} U/min  Gas {:.0} %  Gang {}{}",
                    view.profile,
                    o.rpm,
                    o.throttle * 100.,
                    o.gear,
                    if o.limiter { "  BEGRENZER" } else { "" }
                ),
                white,
            );
            text(
                h,
                &mut y,
                &format!(
                    "Tiefpass {:.0} Hz  Bass {:+.1} dB  Höhen {:+.1} dB  Sättigung {:.2}",
                    o.tone.lowpass, o.tone.low_db, o.tone.high_db, o.tone.drive
                ),
                grey,
            );
            text(
                h,
                &mut y,
                "Loop                Gewicht        Tonhöhe",
                grey,
            );
            for l in live {
                let info = &bank.loops[l.idx];
                let bar = 90f32;
                h.rect(x0 + 140., y - 9., bar, 9., [1., 1., 1., 0.12], 2.);
                h.rect(
                    x0 + 140.,
                    y - 9.,
                    bar * l.gain.min(1.),
                    9.,
                    if info.on {
                        [0.95, 0.55, 0.2, 0.95]
                    } else {
                        [0.4, 0.65, 1., 0.95]
                    },
                    2.,
                );
                text(
                    h,
                    &mut y,
                    &format!(
                        "{:<16} {:>5}            ×{:.2}",
                        info.file.trim_end_matches(".wav"),
                        format!("{:.2}", l.gain),
                        l.pitch
                    ),
                    white,
                );
            }
            text(
                h,
                &mut y,
                &format!("Motorstimmen im Bild: {}", view.voices),
                grey,
            );
        }
        if !self.status.is_empty() {
            text(h, &mut y, &self.status, [1., 0.5, 0.4, 1.]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_cycle_and_turning_a_value_switches_to_manual() {
        let mut d = EngineDebug {
            open: true,
            ..Default::default()
        };
        d.select(-1);
        assert_eq!(d.row, ROWS.len() - 1);
        d.select(1);
        d.select(2);
        assert_eq!(d.row, 2);
        d.adjust(1.);
        assert!(d.manual && d.rpm == 3250.);
        d.row = 3;
        for _ in 0..20 {
            d.adjust(1.);
        }
        assert_eq!(d.throttle, 1.);
        d.row = 1;
        let n = config().all_profiles().len();
        for _ in 0..n {
            d.adjust(1.);
        }
        assert_eq!(d.profile, 1, "Profilauswahl läuft im Kreis");
        let mut l = Listener::default();
        d.sync(&mut l);
        assert!(
            l.engine_override
                .is_some_and(|o| o.rpm == 3250. && o.throttle == 1.)
        );
        // A/B lädt die Referenz von der Platte
        d.row = 5;
        d.adjust(1.);
        assert!(d.ab);
        d.sync(&mut l);
        assert!(l.reference.as_ref().is_some_and(|r| r.len() > 48000));
        // geschlossen: nichts überschreibt das Fahren
        d.open = false;
        d.sync(&mut l);
        assert!(l.engine_override.is_none() && l.reference.is_none());
    }

    #[test]
    fn draws_loops_of_a_running_engine() {
        use berlin_sim::enginesound::{EngineSound, SoundInput};
        let p = config().preset("supercar").unwrap();
        let mut e = EngineSound::new(1);
        let inp = SoundInput {
            rpm: Some(4000.),
            gear: Some(3),
            throttle: 0.6,
            ..Default::default()
        };
        let out = e.step(p, config().bank("v10"), &inp, 1., false);
        let view = EngineView {
            profile: "supercar".into(),
            out,
            voices: 3,
        };
        let d = EngineDebug {
            open: true,
            ..Default::default()
        };
        let mut h = Hud::new([1280., 720.]);
        d.draw(&mut h, &view);
        let empty = Hud::new([1280., 720.]).items.len();
        assert!(h.items.len() > empty + 200);
    }
}
