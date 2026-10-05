//! Arcade-Drift-Schicht (Fahrphysik Phase 7, Vorbild: Gyro-Assist ferngesteuerter Drift-Autos). Der Spieler gibt
//! Richtung und Gas, die Schicht hält den Driftwinkel und lenkt selbst gegen – Driften fühlt sich nach Können an
//! und ist trotzdem in Sekunden gelernt.
//!
//! Zustände: `Grip → Entry → Drift → Exit → Grip`, dazu `Drift → Spin`.
//!
//! - **Einleitung** (eine Bedingung genügt): Handbremse > 0,12 s bei > 25 km/h mit Lenkeinschlag; Power-Over
//!   (angetriebene Hinterachse – Heck- oder Allrad –, Gas > 80 %, Querbeschleunigung > 0,5 g, Leistungsreserve); Lastwechsel (Lenkung
//!   binnen 0,3 s von einer Seite zur anderen bei > 50 km/h, Gas weg und wieder drauf); Bremsdrift (Anbremsen und
//!   Einlenken). Nass oder lose: alle Schwellen 30 % niedriger; `drift.faehigkeit` skaliert sie zusätzlich.
//! - **Im Drift:** Seitenhaftung hinten auf `drift.grip_hinten`, Zielwinkel `15° + Gas·(max_winkel − 15°)`, die
//!   Lenkung krümmt die Bahn, der Assist (PD auf Schwimmwinkel und dessen Änderung) dreht das Auto zum Zielwinkel
//!   und stellt die Vorderräder in Fahrtrichtung. Tempoverlust nur 0,03–0,08 g je nach Winkel.
//! - **Ausleitung:** Schwimmwinkel < 6° für 0,2 s oder < 15 km/h; die Haftung blendet über 0,25 s zurück.
//!   **Dreher:** Schwimmwinkel > `max_winkel` + 25° (Assist-Stufe 2: erst ab 100°).
//! - **Wertung:** Punkte += Winkel · Tempo · dt, Kette ohne Grip-Phase erhöht den Multiplikator, eine Kollision
//!   verwirft den laufenden Drift.
//!
//! Rein: alle Eingänge über [`Ctx`], keine Zeit, kein Zufall. Winkel in rad, Tempo in m/s.
use crate::vehdata::G;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Grip,
    Entry,
    Drift,
    Exit,
    Spin,
}

/// Was den Drift ausgelöst hat (Anzeige, Tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Handbrake,
    PowerOver,
    Feint,
    Brake,
}

/// Einleitung per Handbremse: so lange gezogen (s), ab diesem Tempo (m/s), mit diesem Lenkeinschlag
pub const HB_HOLD: f64 = 0.12;
pub const HB_SPEED: f64 = 25. / 3.6;
pub const STEER_MIN: f64 = 0.2;
/// Power-Over: Gas, Querbeschleunigung (g)
pub const PO_THROTTLE: f64 = 0.8;
pub const PO_LAT_G: f64 = 0.5;
pub const PO_SPEED: f64 = 25. / 3.6;
/// Lastwechsel: Seitenwechsel der Lenkung binnen (s), ab Tempo (m/s), Gas weg binnen (s) vor dem Wiederauflegen
pub const FEINT_WINDOW: f64 = 0.3;
pub const FEINT_SPEED: f64 = 50. / 3.6;
pub const FEINT_LIFT: f64 = 0.6;
/// Bremsdrift: Bremse, Lenkung, Tempo (m/s)
pub const BRAKE_MIN: f64 = 0.5;
pub const BRAKE_STEER: f64 = 0.5;
pub const BRAKE_SPEED: f64 = 40. / 3.6;
/// nass oder lose: Schwellen so viel niedriger
pub const LOOSE: f64 = 0.7;
/// Einleitung: so viel Schwimmwinkel macht daraus einen Drift (rad), sonst nach so langer Zeit (s) zurück
pub const ENTRY_BETA: f64 = 0.15;
pub const ENTRY_MAX: f64 = 0.7;
/// Zielwinkel-Grundwert (rad)
pub const BASE_ANGLE: f64 = 15. * std::f64::consts::PI / 180.;
/// darunter gilt das Gas als weg (Ziel 0°, Ausleitung)
pub const LIFT: f64 = 0.1;
/// Ausleitung: Winkel (rad) und Haltezeit (s), Mindesttempo (m/s), Rückblende der Haftung (s)
pub const EXIT_BETA: f64 = 6. * std::f64::consts::PI / 180.;
pub const EXIT_HOLD: f64 = 0.2;
pub const EXIT_SPEED: f64 = 15. / 3.6;
pub const BLEND: f64 = 0.25;
/// Dreher: Abstand zum Höchstwinkel (rad), bei Stufe 2 erst ab diesem Winkel
pub const SPIN_MARGIN: f64 = 25. * std::f64::consts::PI / 180.;
pub const SPIN_LEVEL2: f64 = 100. * std::f64::consts::PI / 180.;
/// Assist: PD-Verstärkungen (1/s², 1/s), Anteil der Stufen 1 und 2, auf Schnee/Eis sanfter und mit mehr Winkel
pub const KP: f64 = 30.;
pub const KD: f64 = 9.;
pub const LEVEL: [f64; 3] = [0., 0.55, 1.];
pub const ICE_GAIN: f64 = 0.55;
pub const ICE_ANGLE: f64 = 10. * std::f64::consts::PI / 180.;
/// Tempoverlust im Drift (g) bei kleinem bzw. großem Winkel
pub const LOSS: [f64; 2] = [0.03, 0.08];
/// Frontantrieb: nur ein kurzer Heckschwenk (s)
pub const FWD_SWING: f64 = 0.8;
/// Kette: so lange darf die Grip-Phase zwischen zwei Drifts sein (s); höchster Multiplikator
pub const CHAIN_GAP: f64 = 1.5;
pub const CHAIN_MAX: u32 = 5;

/// Antriebsart, soweit sie die Schicht betrifft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drive {
    Front,
    Rear,
    All,
}

/// Eingänge eines Schritts.
#[derive(Debug, Clone, Copy)]
pub struct Ctx {
    pub speed: f64,
    /// Schwimmwinkel (rad, vphys: negativ = Heck nach rechts draußen) und Gierrate (rad/s)
    pub beta: f64,
    pub r: f64,
    /// Querbeschleunigung (m/s²)
    pub ay: f64,
    pub throttle: f64,
    pub brake: f64,
    pub steer: f64,
    pub handbrake: bool,
    pub drive: Drive,
    /// Antriebskraft über der Haftung der Hinterachse (Power-Over möglich)
    pub reserve: bool,
    /// nasse oder lose Fahrbahn; Schnee/Eis
    pub loose: bool,
    pub ice: bool,
    /// `drift.faehigkeit` 0…1, `drift.max_winkel` (rad), `drift.grip_hinten`
    pub ability: f64,
    pub max_angle: f64,
    pub rear_grip: f64,
    /// Assist-Stufe 0…2 (`feel.drift_assist_stufe`)
    pub level: u8,
}

/// Wertung.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Score {
    /// Punkte des laufenden Drifts (ohne Multiplikator), der laufende Multiplikator
    pub current: f64,
    pub mult: u32,
    /// alle verbuchten Punkte, bester Einzeldrift
    pub total: f64,
    pub best: f64,
    /// zuletzt verbucht (für die Anzeige), verworfen durch Kollision
    pub last: f64,
    pub crashed: bool,
}

/// Zustand der Schicht (lebt in `vphys::State`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Drift {
    pub phase: Phase,
    /// Zeit in der Phase (s)
    pub t: f64,
    /// Seitenhaftung hinten (1 = voll)
    pub grip: f64,
    /// Kurvenrichtung des Drifts (+1 links, −1 rechts)
    pub side: f64,
    pub trigger: Option<Trigger>,
    /// Zielwinkel (rad, Betrag)
    pub target: f64,
    hb_t: f64,
    low_t: f64,
    /// Zeit seit dem letzten Gaswegnehmen bzw. Seitenwechsel der Lenkung (None = noch keins)
    lift_t: Option<f64>,
    last_steer: f64,
    flip_t: Option<f64>,
    prev_beta: f64,
    beta_rate: f64,
    gap: f64,
    pub score: Score,
}

/// Ausgaben für den Physikkern.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Out {
    /// Seitenhaftung hinten (Faktor)
    pub rear_grip: f64,
    /// Giermoment des Assists je Gierträgheit (rad/s²)
    pub yaw_acc: f64,
    /// Anteil, zu dem die Vorderräder in Fahrtrichtung stehen (Gegenlenken), 0…1
    pub countersteer: f64,
    /// Tempoverlust, auf den die Schicht begrenzt (m/s², positiv), 0 = keine Begrenzung
    pub max_loss: f64,
    /// ESP und Antriebsschlupfregelung halten sich heraus
    pub aids_off: bool,
}

impl Drift {
    /// Vollständig in einem Drift (für Feedback: Rauch, Klang, Kamera).
    pub fn active(&self) -> bool {
        matches!(self.phase, Phase::Entry | Phase::Drift)
    }

    /// Zeit seit dem Ende des letzten verbuchten Drifts (s; unendlich nach Kollision oder Dreher).
    pub fn since_end(&self) -> f64 {
        if self.active() { 0. } else { self.gap }
    }

    /// Schicht aus (Zweirad, ESP voll, Rückwärts): alles zurücksetzen, die Wertung bleibt.
    pub fn off(&mut self) {
        *self = Drift {
            score: self.score,
            grip: 1.,
            ..Default::default()
        };
    }

    /// Kollision: der laufende Drift zählt nicht, die Kette reißt.
    pub fn crash(&mut self) {
        if self.phase != Phase::Grip || self.score.current > 0. {
            self.score.current = 0.;
            self.score.mult = 0;
            self.score.crashed = true;
            self.gap = f64::INFINITY;
        }
    }

    fn go(&mut self, p: Phase) {
        self.phase = p;
        self.t = 0.;
    }

    fn bank(&mut self) {
        let mult = self.score.mult.max(1) as f64;
        let pts = self.score.current * mult;
        if pts > 0. {
            self.score.total += pts;
            self.score.best = self.score.best.max(pts);
            self.score.last = pts;
            self.score.crashed = false;
        }
        self.score.current = 0.;
    }

    /// Einleitung erkennen (nur in der Grip-Phase).
    fn trigger(&mut self, c: &Ctx, dt: f64) -> Option<Trigger> {
        // Schwellen: fähige Autos leiten leichter ein, nass/lose 30 % leichter
        let k = (1.6 - 0.8 * c.ability) * if c.loose { LOOSE } else { 1. };
        self.hb_t = if c.handbrake { self.hb_t + dt } else { 0. };
        if c.throttle < 0.2 {
            self.lift_t = Some(0.);
        } else if let Some(t) = self.lift_t.as_mut() {
            *t += dt;
        }
        // Lenkung wechselt die Seite
        let side = if c.steer.abs() > 0.3 {
            c.steer.signum()
        } else {
            0.
        };
        if side != 0. {
            if self.last_steer != 0. && side != self.last_steer {
                self.flip_t = Some(0.);
            }
            self.last_steer = side;
        }
        if let Some(t) = self.flip_t.as_mut() {
            *t += dt;
        }
        let steering = c.steer.abs() > STEER_MIN * k;
        if self.hb_t > HB_HOLD && c.speed > HB_SPEED * k && steering {
            return Some(Trigger::Handbrake);
        }
        if c.drive == Drive::Front {
            return None;
        }
        if c.throttle > PO_THROTTLE.min(1.) * k.min(1.)
            && c.ay.abs() > PO_LAT_G * G * k
            && c.speed > PO_SPEED * k
            && c.reserve
        {
            return Some(Trigger::PowerOver);
        }
        if self.flip_t.is_some_and(|t| t < FEINT_WINDOW)
            && c.speed > FEINT_SPEED * k
            && self.lift_t.is_some_and(|t| t > 0. && t < FEINT_LIFT)
            && c.throttle > 0.5
        {
            return Some(Trigger::Feint);
        }
        if c.brake > BRAKE_MIN * k && c.steer.abs() > BRAKE_STEER * k && c.speed > BRAKE_SPEED * k {
            return Some(Trigger::Brake);
        }
        None
    }

    /// Ein Schritt.
    pub fn update(&mut self, c: &Ctx, dt: f64) -> Out {
        if self.grip <= 0. {
            self.grip = 1.;
        }
        let beta = c.beta.abs();
        self.beta_rate = (c.beta - self.prev_beta) / dt.max(1e-6);
        self.prev_beta = c.beta;
        self.t += dt;
        if c.ability <= 0. || c.max_angle <= 0. {
            *self = Drift {
                score: self.score,
                grip: 1.,
                ..Default::default()
            };
            return Out {
                rear_grip: 1.,
                ..Default::default()
            };
        }
        let max = c.max_angle + if c.ice { ICE_ANGLE } else { 0. };
        match self.phase {
            Phase::Grip => {
                self.gap += dt;
                if self.gap > CHAIN_GAP {
                    self.score.mult = 0;
                }
                if let Some(t) = self.trigger(c, dt) {
                    self.trigger = Some(t);
                    // Kurvenrichtung: wohin gelenkt bzw. gegiert wird
                    self.side = if c.steer.abs() > 0.1 {
                        c.steer.signum()
                    } else {
                        c.r.signum()
                    };
                    self.go(Phase::Entry);
                }
            }
            Phase::Entry => {
                if beta > ENTRY_BETA {
                    self.score.mult = if self.gap <= CHAIN_GAP {
                        (self.score.mult + 1).min(CHAIN_MAX)
                    } else {
                        1
                    };
                    self.score.crashed = false;
                    self.go(Phase::Drift);
                } else if self.t > ENTRY_MAX || c.speed < EXIT_SPEED {
                    self.go(Phase::Exit);
                }
            }
            Phase::Drift => {
                let spin = if c.level >= 2 {
                    SPIN_LEVEL2
                } else {
                    max + SPIN_MARGIN
                };
                if beta > spin {
                    self.score.current = 0.;
                    self.score.mult = 0;
                    self.go(Phase::Spin);
                } else {
                    self.low_t = if beta < EXIT_BETA {
                        self.low_t + dt
                    } else {
                        0.
                    };
                    let fwd_done = c.drive == Drive::Front && self.t > FWD_SWING;
                    if self.low_t > EXIT_HOLD || c.speed < EXIT_SPEED || fwd_done {
                        self.bank();
                        self.gap = 0.;
                        self.go(Phase::Exit);
                    } else {
                        self.score.current += beta.to_degrees() * c.speed * dt;
                    }
                }
            }
            Phase::Exit => {
                if self.grip >= 1. {
                    self.go(Phase::Grip);
                }
            }
            Phase::Spin => {
                if c.speed < EXIT_SPEED || beta < ENTRY_BETA {
                    self.gap = f64::INFINITY;
                    self.go(Phase::Exit);
                }
            }
        }
        // Haftung hinten: im Drift reduziert, sonst über `BLEND` zurück
        let low = c.rear_grip.clamp(0.3, 1.);
        let want = if self.active() { low } else { 1. };
        let rate = (1. - low) / BLEND * dt;
        self.grip += (want - self.grip).clamp(-rate * 3., rate);
        let mut out = Out {
            rear_grip: self.grip,
            ..Default::default()
        };
        if !self.active() {
            return out;
        }
        out.aids_off = true;
        // Zielwinkel aus dem Gas; die Assist-Stärke folgt Stufe, Fähigkeit und Untergrund
        let gain = LEVEL[(c.level as usize).min(2)]
            * (0.5 + 0.5 * c.ability)
            * if c.ice { ICE_GAIN } else { 1. };
        // Gas weg (< 10 %): Ziel 0°, der Assist richtet das Auto gerade – das leitet aus
        self.target = if c.throttle < LIFT {
            0.
        } else {
            (BASE_ANGLE + c.throttle.clamp(0., 1.) * (max - BASE_ANGLE).max(0.)).min(max)
        };
        if gain > 0. && self.phase == Phase::Drift {
            // Heck draußen auf der Kurvenaußenseite: in einer Linkskurve (side +1) ist β negativ
            let tgt = -self.side * self.target;
            // β zu klein (Richtung Grip) → Auto weiter eindrehen; Dämpfung auf die Winkeländerung
            let err = c.beta - tgt;
            // dβ/dt = Bahndrehung − r: eindrehen (r ↑) senkt β; die Dämpfung bremst die Winkeländerung
            out.yaw_acc = gain * (KP * err + KD * self.beta_rate);
            out.countersteer = gain;
            let k = ((beta - BASE_ANGLE) / (max - BASE_ANGLE).max(0.1)).clamp(0., 1.);
            out.max_loss = (LOSS[0] + (LOSS[1] - LOSS[0]) * k) * G;
        } else if self.phase == Phase::Entry {
            out.countersteer = gain * 0.5;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Ctx {
        Ctx {
            speed: 15.,
            beta: 0.,
            r: 0.,
            ay: 0.,
            throttle: 0.,
            brake: 0.,
            steer: 0.,
            handbrake: false,
            drive: Drive::Rear,
            reserve: false,
            loose: false,
            ice: false,
            ability: 0.8,
            max_angle: 55f64.to_radians(),
            rear_grip: 0.6,
            level: 2,
        }
    }
    fn run(d: &mut Drift, c: Ctx, secs: f64) -> Out {
        let mut o = Out::default();
        for _ in 0..(secs * 120.) as usize {
            o = d.update(&c, 1. / 120.);
        }
        o
    }

    #[test]
    fn a_short_handbrake_tug_with_steering_starts_a_drift() {
        let mut d = Drift::default();
        let tug = Ctx {
            handbrake: true,
            steer: 0.6,
            ..ctx()
        };
        run(&mut d, tug, 0.08);
        assert_eq!(d.phase, Phase::Grip, "zu kurz gezogen");
        run(&mut d, tug, 0.1);
        assert_eq!(d.phase, Phase::Entry);
        assert_eq!(d.trigger, Some(Trigger::Handbrake));
        // Schwimmwinkel wächst: Drift, Seitenhaftung hinten sinkt auf den Datenwert
        let o = run(
            &mut d,
            Ctx {
                beta: -0.3,
                throttle: 0.5,
                ..ctx()
            },
            0.6,
        );
        assert_eq!(d.phase, Phase::Drift);
        assert!((o.rear_grip - 0.6).abs() < 1e-9, "{}", o.rear_grip);
        assert!(o.aids_off && o.countersteer > 0.8, "{}", o.countersteer);
        // langsam: keine Einleitung
        let mut d = Drift::default();
        run(&mut d, Ctx { speed: 5., ..tug }, 0.5);
        assert_eq!(d.phase, Phase::Grip);
    }

    #[test]
    fn throttle_sets_the_target_angle() {
        let mut d = Drift {
            phase: Phase::Drift,
            side: 1.,
            grip: 0.6,
            ..Default::default()
        };
        d.update(
            &Ctx {
                beta: -0.4,
                throttle: 0.,
                ..ctx()
            },
            0.01,
        );
        assert_eq!(d.target, 0., "Gas weg: geraderichten");
        d.update(
            &Ctx {
                beta: -0.4,
                throttle: 0.1,
                ..ctx()
            },
            0.01,
        );
        assert!((d.target - (BASE_ANGLE + 0.1 * (55f64.to_radians() - BASE_ANGLE))).abs() < 1e-9);
        d.update(
            &Ctx {
                beta: -0.4,
                throttle: 1.,
                ..ctx()
            },
            0.01,
        );
        assert!((d.target - 55f64.to_radians()).abs() < 1e-9);
    }

    #[test]
    fn assist_turns_in_when_the_angle_is_too_small_and_out_when_too_big() {
        // Linkskurve, Ziel bei Halbgas ~35°: bei 10° eindrehen (positives Giermoment), bei 60° heraus
        let small = |beta: f64| {
            let mut d = Drift {
                phase: Phase::Drift,
                side: 1.,
                grip: 0.6,
                prev_beta: beta,
                ..Default::default()
            };
            d.update(
                &Ctx {
                    beta,
                    throttle: 0.5,
                    ..ctx()
                },
                0.01,
            )
            .yaw_acc
        };
        assert!(small(-10f64.to_radians()) > 0.);
        assert!(small(-60f64.to_radians()) < 0.);
    }

    #[test]
    fn exit_spin_and_scoring() {
        let mut d = Drift::default();
        let start = Ctx {
            handbrake: true,
            steer: 0.6,
            ..ctx()
        };
        run(&mut d, start, 0.2);
        run(
            &mut d,
            Ctx {
                beta: -0.5,
                speed: 15.,
                throttle: 0.6,
                ..ctx()
            },
            2.,
        );
        assert!(d.score.current > 100., "{}", d.score.current);
        // Winkel weg: nach 0,2 s Ausleitung, Punkte verbucht, Haftung blendet in 0,25 s zurück
        run(&mut d, ctx(), 0.25);
        assert_eq!(d.phase, Phase::Exit);
        assert!(d.score.total > 100. && d.score.current == 0.);
        let o = run(&mut d, ctx(), 0.3);
        assert_eq!(d.phase, Phase::Grip);
        assert!((o.rear_grip - 1.).abs() < 1e-9);
        // gleich wieder: Kette, Multiplikator 2
        run(&mut d, start, 0.2);
        run(
            &mut d,
            Ctx {
                beta: -0.5,
                throttle: 0.6,
                ..ctx()
            },
            0.1,
        );
        assert_eq!(d.score.mult, 2);
        // Kollision verwirft den laufenden Drift
        d.crash();
        assert_eq!(d.score.current, 0.);
        assert!(d.score.crashed);
        // Dreher ab 100° (Stufe 2)
        let mut d = Drift {
            phase: Phase::Drift,
            side: 1.,
            ..Default::default()
        };
        run(
            &mut d,
            Ctx {
                beta: -95f64.to_radians(),
                ..ctx()
            },
            0.05,
        );
        assert_eq!(d.phase, Phase::Drift);
        run(
            &mut d,
            Ctx {
                beta: -105f64.to_radians(),
                ..ctx()
            },
            0.05,
        );
        assert_eq!(d.phase, Phase::Spin);
        // Stufe 1: schon ab max_winkel + 25°
        let mut d = Drift {
            phase: Phase::Drift,
            side: 1.,
            ..Default::default()
        };
        run(
            &mut d,
            Ctx {
                beta: -85f64.to_radians(),
                level: 1,
                ..ctx()
            },
            0.05,
        );
        assert_eq!(d.phase, Phase::Spin);
    }

    #[test]
    fn other_triggers_and_vehicle_limits() {
        // Power-Over nur mit Heckantrieb und Reserve
        let po = Ctx {
            throttle: 1.,
            ay: 0.6 * G,
            reserve: true,
            steer: 0.4,
            ..ctx()
        };
        let mut d = Drift::default();
        run(&mut d, po, 0.02);
        assert_eq!(d.trigger, Some(Trigger::PowerOver));
        // Allrad treibt die Hinterachse mit an: ebenfalls; ohne Reserve nicht
        let mut d = Drift::default();
        run(
            &mut d,
            Ctx {
                drive: Drive::All,
                ..po
            },
            0.02,
        );
        assert_eq!(d.trigger, Some(Trigger::PowerOver));
        let mut d = Drift::default();
        run(
            &mut d,
            Ctx {
                reserve: false,
                ..po
            },
            0.1,
        );
        assert_eq!(d.phase, Phase::Grip);
        // Bremsdrift
        let mut d = Drift::default();
        run(
            &mut d,
            Ctx {
                brake: 0.7,
                steer: 0.7,
                ..ctx()
            },
            0.02,
        );
        assert_eq!(d.trigger, Some(Trigger::Brake));
        // Lastwechsel: links, Gas weg, rechts, Gas drauf
        let mut d = Drift::default();
        let fast = Ctx {
            speed: 20.,
            ..ctx()
        };
        run(
            &mut d,
            Ctx {
                steer: 0.8,
                throttle: 0.6,
                ..fast
            },
            0.5,
        );
        run(
            &mut d,
            Ctx {
                steer: 0.8,
                throttle: 0.,
                ..fast
            },
            0.2,
        );
        run(
            &mut d,
            Ctx {
                steer: -0.8,
                throttle: 0.,
                ..fast
            },
            0.05,
        );
        run(
            &mut d,
            Ctx {
                steer: -0.8,
                throttle: 0.8,
                ..fast
            },
            0.05,
        );
        assert_eq!(d.trigger, Some(Trigger::Feint));
        // Frontantrieb: nur Handbremse, kurzer Schwenk
        let mut d = Drift::default();
        run(
            &mut d,
            Ctx {
                drive: Drive::Front,
                ..po
            },
            0.1,
        );
        assert_eq!(d.phase, Phase::Grip);
        // Lkw (Fähigkeit 0): nie
        let mut d = Drift::default();
        run(
            &mut d,
            Ctx {
                ability: 0.,
                handbrake: true,
                steer: 0.8,
                ..ctx()
            },
            0.5,
        );
        assert_eq!(d.phase, Phase::Grip);
        // nass/lose: niedrigere Schwellen (Handbremse schon bei 20 km/h)
        let slow_hb = Ctx {
            speed: 20. / 3.6,
            handbrake: true,
            steer: 0.6,
            ..ctx()
        };
        let mut d = Drift::default();
        run(&mut d, slow_hb, 0.2);
        let dry = d.phase;
        let mut d = Drift::default();
        run(
            &mut d,
            Ctx {
                loose: true,
                ability: 1.,
                ..slow_hb
            },
            0.2,
        );
        assert!(
            dry == Phase::Grip && d.phase == Phase::Entry,
            "{dry:?} {:?}",
            d.phase
        );
    }
}
