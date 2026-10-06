//! Controller-Vibration aus dem Spielgeschehen (rein, ohne Gerät): Ereignisse eines Schritts und der Zustand des
//! eigenen Autos → höchstens eine neue Vibration. Stöße (Unfall, Treffer, Tod) gehen vor; durchdrehende oder
//! blockierende Räder rattern fein und werden laufend nachgelegt, solange sie anhalten.
use berlin_engine::Rumble;
use berlin_sim::events::Event;

/// Abstand, in dem ein Dauerrattern nachgelegt wird (s); die einzelne Vibration läuft etwas länger, damit es nahtlos ist.
const CONT_EVERY: f64 = 0.09;

#[derive(Debug, Default)]
pub struct Rumbler {
    last_cont: f64,
    /// bis wann ein Stoß läuft (Dauerrattern überschreibt ihn nicht)
    busy_until: f64,
}

/// Lage des eigenen Fahrzeugs für das Dauerrattern.
#[derive(Debug, Clone, Copy, Default)]
pub struct Drive {
    /// Schlupf der Antriebsräder 0…1 (durchdrehen)
    pub spin: f64,
    /// blockierende Räder 0…1
    pub lock: f64,
    /// ABS/ESP greifen ein
    pub assist: bool,
}

impl Rumbler {
    /// `me` = Position der Spielfigur, `own_car` = gefahrenes Auto, `t` = Spielzeit.
    pub fn step(
        &mut self,
        events: &[Event],
        me: (f64, f64),
        own_car: Option<u32>,
        drive: Option<Drive>,
        t: f64,
    ) -> Option<Rumble> {
        let mut best: Option<Rumble> = None;
        let mut take = |r: Rumble| {
            if best.is_none_or(|b| r.strong + r.weak > b.strong + b.weak) {
                best = Some(r);
            }
        };
        for e in events {
            match *e {
                Event::Crash { strength, car, .. } if Some(car) == own_car => {
                    let s = strength.clamp(0., 1.) as f32;
                    take(Rumble {
                        strong: 0.35 + 0.65 * s,
                        weak: 0.5 * s,
                        ms: 120 + (220. * s) as u32,
                    });
                }
                Event::PlayerHurt { dmg, .. } => take(Rumble {
                    strong: (0.35 + dmg as f32 / 60.).min(1.),
                    weak: 0.3,
                    ms: 160,
                }),
                // Explosion in der Nähe: Wucht nach Entfernung (12 m voll, 60 m nichts mehr)
                Event::Explosion { x, y, .. } => {
                    let d = (x - me.0).hypot(y - me.1);
                    let k = (1. - (d - 120.) / 480.).clamp(0., 1.) as f32;
                    if k > 0.05 {
                        take(Rumble {
                            strong: k,
                            weak: 0.7 * k,
                            ms: 250 + (350. * k) as u32,
                        });
                    }
                }
                Event::Wasted { .. } => take(Rumble {
                    strong: 1.,
                    weak: 0.6,
                    ms: 450,
                }),
                // eigener Schuss: gemeldet an der Mündung, 10 px vor der Figur
                Event::Shot { x, y, weapon, .. } if (x - me.0).hypot(y - me.1) < 14. => {
                    let heavy = weapon == "shotgun";
                    take(Rumble {
                        strong: if heavy { 0.45 } else { 0.12 },
                        weak: if heavy { 0.4 } else { 0.28 },
                        ms: if heavy { 110 } else { 55 },
                    });
                }
                _ => {}
            }
        }
        if let Some(r) = best {
            self.busy_until = t + r.ms as f64 / 1000.;
            return Some(r);
        }
        // Dauerrattern: Räder drehen durch oder blockieren
        let d = drive?;
        let level = d.spin.max(d.lock).clamp(0., 1.) as f32;
        if level < 0.25 || t < self.busy_until || t - self.last_cont < CONT_EVERY {
            return None;
        }
        self.last_cont = t;
        Some(Rumble {
            strong: 0.05 + if d.lock > d.spin { 0.15 * level } else { 0. },
            weak: 0.12 + 0.22 * level + if d.assist { 0.08 } else { 0. },
            ms: 130,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shocks_beat_continuous_and_only_own_events_count() {
        let mut r = Rumbler::default();
        let crash = |car, s| Event::Crash {
            x: 0.,
            y: 0.,
            strength: s,
            car,
        };
        assert!(
            r.step(&[crash(7, 1.)], (0., 0.), Some(1), None, 0.)
                .is_none(),
            "fremdes Auto"
        );
        let hard = r
            .step(&[crash(1, 1.)], (0., 0.), Some(1), None, 0.)
            .unwrap();
        let soft = r
            .step(&[crash(1, 0.1)], (0., 0.), Some(1), None, 1.)
            .unwrap();
        assert!(hard.strong > soft.strong && hard.ms > soft.ms);
        // während eines Stoßes kein Dauerrattern
        let spin = Some(Drive {
            spin: 1.,
            ..Default::default()
        });
        r.step(&[crash(1, 1.)], (0., 0.), Some(1), None, 2.);
        assert!(r.step(&[], (0., 0.), Some(1), spin, 2.1).is_none());
        assert!(
            r.step(&[], (0., 0.), Some(1), spin, 2.5).is_some(),
            "danach rattert es"
        );
        assert!(
            r.step(&[], (0., 0.), Some(1), spin, 2.52).is_none(),
            "nicht jedes Bild neu"
        );
        // ruhig fahren: nichts
        let calm = Some(Drive {
            spin: 0.1,
            ..Default::default()
        });
        assert!(r.step(&[], (0., 0.), Some(1), calm, 5.).is_none());
        // eigener Schuss zuckt kurz, fremder nicht
        let shot = |x| Event::Shot {
            x,
            y: 0.,
            a: 0.,
            weapon: "pistol",
            traces: vec![],
        };
        assert!(r.step(&[shot(500.)], (0., 0.), None, None, 6.).is_none());
        let s = r.step(&[shot(10.)], (0., 0.), None, None, 7.).unwrap();
        assert!(s.ms <= 60 && s.strong < 0.2);
    }
}
