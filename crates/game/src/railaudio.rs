//! S-/U-Bahn-Klang je Bild: aus der Welt die Mischung für den Synthesizer (`railsound::RailMix`) und die
//! Einzelklänge der Bahn (Schienenstöße, Druckluft beim Halten, Warnton vor der Abfahrt).
//!
//! - Im Zug (mitfahren oder selbst fahren): Fahrschichten nach Tempo und Beschleunigung, Schienenstöße über einen
//!   Kilometerzähler, im Tunnel Nachhall.
//! - Im U-Bahnhof: der lauteste Zug am Bahnsteig aus seiner Richtung, Halle mit Nachhall.
//! - Beim Halten: Druckluft, kurz vor der Abfahrt drei Warntöne (die Haltezeit ist bekannt: `RAIL_DWELL_S`).
use berlin_audio::synth::Sfx;
use berlin_sim::railsound::{
    HALL_STATION, HALL_TUNNEL, Heard, Motion, RailMix, joint_level, joints_between,
    platform_layers, ride_layers,
};
use berlin_sim::ride::{Ref, RideKind};
use berlin_sim::transit::RAIL_DWELL_S;
use berlin_sim::world::World;
use std::collections::HashMap;

/// s vor der Abfahrt: Warnton
pub const BEEP_BEFORE: f64 = 2.4;
/// s: Glättung der gemessenen Beschleunigung
const ACCEL_TC: f64 = 0.25;

/// Halt eines Zuges: seit wann, Warnton schon gegeben.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Dwell {
    since: f64,
    beeped: bool,
}

/// Druckluft beim Anhalten, Warnton `BEEP_BEFORE` vor der Abfahrt (einmal je Halt, nur mit `beep`).
fn dwell_cue(
    state: &mut Option<Dwell>,
    dwelling: bool,
    t: f64,
    k: f32,
    beep: bool,
    sfx: &mut Vec<Sfx>,
) {
    if !dwelling {
        *state = None;
        return;
    }
    match state {
        None => {
            *state = Some(Dwell {
                since: t,
                beeped: false,
            });
            sfx.push(Sfx::AirHiss(k));
        }
        Some(d) if beep && !d.beeped && t - d.since >= RAIL_DWELL_S - BEEP_BEFORE => {
            d.beeped = true;
            sfx.push(Sfx::DepartBeep(k));
        }
        _ => {}
    }
}

#[derive(Debug, Default)]
pub struct RailAudio {
    /// zurückgelegter Weg des eigenen Zugs (px) für die Schienenstöße
    odo: f64,
    /// Zug und Tempo im vorigen Bild (Beschleunigung messen)
    prev: Option<(String, f64)>,
    accel: f64,
    ride_dwell: Option<Dwell>,
    platform: HashMap<String, Dwell>,
}

impl RailAudio {
    pub fn step(&mut self, w: &mut World, dt: f64, sfx: &mut Vec<Sfx>) -> RailMix {
        let mut mix = RailMix::default();
        let t = w.time;
        // --- im Zug
        let ride = w.player.ride.clone();
        let state = ride.as_ref().and_then(|r| w.vehicle_state(&r.r));
        match (ride, state) {
            (Some(r), Some(vs)) if vs.mode.rail() => {
                let key = format!("{:?}", r.r);
                let v = vs.speed;
                let raw = match &self.prev {
                    Some((k, pv)) if *k == key && dt > 0. => (v - pv) / dt,
                    _ => 0.,
                };
                self.accel += (raw - self.accel) * (dt / ACCEL_TC).min(1.);
                self.prev = Some((key, v));
                mix.ride = ride_layers(&Motion {
                    mode: vs.mode,
                    v,
                    a: self.accel,
                    tunnel: vs.underground,
                });
                let odo = self.odo + v * dt;
                let level = (joint_level(v) * if vs.underground { 1. } else { 0.8 }) as f32;
                for _ in 0..joints_between(self.odo, odo) {
                    sfx.push(Sfx::RailJoint(level));
                }
                self.odo = odo;
                if vs.underground {
                    mix.hall = HALL_TUNNEL;
                }
                // den eigenen Zug fertigt man selbst ab (Türen per Taste) – Warnton nur als Fahrgast
                let passenger = r.kind == RideKind::Passenger && !matches!(r.r, Ref::PlayerTrain);
                dwell_cue(&mut self.ride_dwell, vs.dwelling, t, 0.8, passenger, sfx);
            }
            _ => {
                self.prev = None;
                self.accel = 0.;
                self.ride_dwell = None;
            }
        }
        // --- im U-Bahnhof
        let inside = w.player.inside.as_ref().map(|i| i.id.clone());
        if let Some(st) = inside.and_then(|id| w.station_by_id(&id).cloned()) {
            mix.hall = HALL_STATION;
            let (px, py) = (w.player.x, w.player.y);
            let (pu, pv) = st.to_local(px, py);
            let trains = w.trains_at(&st);
            let mut heard = Vec::with_capacity(trains.len());
            let mut seen = Vec::with_capacity(trains.len());
            for tr in &trains {
                // nächster Wagen: entlang der Strecke bis zur Wagenkante, quer bis zur Gleismitte
                let Some(&(cu, cv, cl, _)) = tr
                    .cars
                    .iter()
                    .min_by(|a, b| (a.0 - pu).abs().total_cmp(&(b.0 - pu).abs()))
                else {
                    continue;
                };
                let along = ((cu - pu).abs() - cl / 2.).max(0.);
                let (wx, _) = st.to_world(cu, cv);
                heard.push(Heard {
                    mode: tr.mode,
                    dist: along.hypot(cv - pv),
                    pan: ((wx - px) / 500.).clamp(-1., 1.),
                    v: tr.v,
                    a: tr.a,
                    dwelling: tr.dwelling,
                });
                let key = format!("{:?}", tr.r);
                let mut d = self.platform.remove(&key);
                dwell_cue(&mut d, tr.dwelling, t, 0.7, true, sfx);
                if let Some(d) = d {
                    seen.push((key, d));
                }
            }
            self.platform = seen.into_iter().collect();
            (mix.pass, mix.pass_pan) = platform_layers(&heard);
        } else {
            self.platform.clear();
        }
        mix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stop_hisses_once_and_beeps_before_leaving() {
        let mut d = None;
        let mut sfx = Vec::new();
        dwell_cue(&mut d, false, 0., 1., true, &mut sfx);
        assert!(sfx.is_empty());
        dwell_cue(&mut d, true, 10., 1., true, &mut sfx);
        assert!(matches!(sfx[..], [Sfx::AirHiss(_)]));
        dwell_cue(&mut d, true, 11., 1., true, &mut sfx);
        assert_eq!(sfx.len(), 1, "einmal je Halt");
        dwell_cue(
            &mut d,
            true,
            10. + RAIL_DWELL_S - BEEP_BEFORE,
            1.,
            true,
            &mut sfx,
        );
        assert!(matches!(sfx[1], Sfx::DepartBeep(_)));
        dwell_cue(&mut d, true, 17.9, 1., true, &mut sfx);
        assert_eq!(sfx.len(), 2, "ein Warnton");
        dwell_cue(&mut d, false, 18., 1., true, &mut sfx);
        assert!(d.is_none());
        dwell_cue(&mut d, true, 40., 1., true, &mut sfx);
        assert_eq!(sfx.len(), 3, "nächster Halt zischt wieder");
        // ohne Warnton (Zug selbst gefahren): nur Druckluft
        let (mut e, mut quiet) = (None, Vec::new());
        dwell_cue(&mut e, true, 0., 1., false, &mut quiet);
        dwell_cue(&mut e, true, RAIL_DWELL_S, 1., false, &mut quiet);
        assert!(matches!(quiet[..], [Sfx::AirHiss(_)]));
    }
}
