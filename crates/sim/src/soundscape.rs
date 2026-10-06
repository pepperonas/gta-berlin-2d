//! Fahrzeug- und Schrittklang (Port von `soundscape.js`, rein rechnerisch): Drehzahl mit Gängen je Motor, Reifen
//! auf Belag/Nässe/Schnee, Quietschen, Fahrtwind, die nächsten fremden Autos als Stimmen (Entfernung, Richtung,
//! Doppler) und Schritte. Der Synthesizer (`berlin-audio`) macht daraus Klang.
use crate::car::Car;
use crate::city::Ground;
use crate::enginevoice::{Voice, voice_for};
use crate::world::World;
use std::collections::HashMap;

/// Motor: Leerlauf/Abregeldrehzahl, Zylinder, Gänge als px/s je 1000 U/min.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Engine {
    pub idle: f64,
    pub red: f64,
    pub cyl: u32,
    pub two_stroke: bool,
    pub gears: &'static [f64],
    pub diesel: bool,
    pub electric: bool,
}
pub const ENGINES: &[(&str, Engine)] = &[
    (
        "car",
        Engine {
            idle: 800.,
            red: 6400.,
            cyl: 4,
            two_stroke: false,
            gears: &[14., 24., 35., 46., 57., 68.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "police",
        Engine {
            idle: 750.,
            red: 6200.,
            cyl: 6,
            two_stroke: false,
            gears: &[16., 27., 39., 51., 63., 76.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "ambulance",
        Engine {
            idle: 700.,
            red: 4200.,
            cyl: 4,
            two_stroke: false,
            gears: &[18., 30., 44., 58., 72., 84.],
            diesel: true,
            electric: false,
        },
    ),
    (
        "delivery",
        Engine {
            idle: 750.,
            red: 4300.,
            cyl: 4,
            two_stroke: false,
            gears: &[17., 29., 43., 57., 70., 82.],
            diesel: true,
            electric: false,
        },
    ),
    (
        "truck",
        Engine {
            idle: 600.,
            red: 2600.,
            cyl: 6,
            two_stroke: false,
            gears: &[18., 30., 45., 62., 80., 100., 120.],
            diesel: true,
            electric: false,
        },
    ),
    (
        "garbage",
        Engine {
            idle: 600.,
            red: 2400.,
            cyl: 6,
            two_stroke: false,
            gears: &[16., 27., 40., 55., 72., 90.],
            diesel: true,
            electric: false,
        },
    ),
    (
        "bus",
        Engine {
            idle: 600.,
            red: 2500.,
            cyl: 6,
            two_stroke: false,
            gears: &[22., 38., 56., 78., 100., 122.],
            diesel: true,
            electric: false,
        },
    ),
    (
        "motorcycle",
        Engine {
            idle: 1200.,
            red: 13000.,
            cyl: 4,
            two_stroke: false,
            gears: &[14., 20., 27., 34., 42., 52.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "scooter",
        Engine {
            idle: 1600.,
            red: 9000.,
            cyl: 1,
            two_stroke: false,
            gears: &[30.],
            diesel: false,
            electric: false,
        },
    ),
];
pub const MODEL_ENGINES: &[(&str, Engine)] = &[
    (
        "zweitakter",
        Engine {
            idle: 950.,
            red: 4500.,
            cyl: 2,
            two_stroke: true,
            gears: &[17., 32., 48., 66.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "kleinwagen",
        Engine {
            idle: 850.,
            red: 6200.,
            cyl: 3,
            two_stroke: false,
            gears: &[14., 25., 38., 55., 77.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "kompakt",
        Engine {
            idle: 800.,
            red: 6500.,
            cyl: 4,
            two_stroke: false,
            gears: &[14., 24., 35., 47., 63., 90.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "limousine",
        Engine {
            idle: 750.,
            red: 6500.,
            cyl: 6,
            two_stroke: false,
            gears: &[15., 25., 37., 50., 64., 82., 105.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "taxi",
        Engine {
            idle: 750.,
            red: 4500.,
            cyl: 4,
            two_stroke: false,
            gears: &[18., 30., 45., 60., 78., 100., 130.],
            diesel: true,
            electric: false,
        },
    ),
    (
        "kombi",
        Engine {
            idle: 800.,
            red: 6500.,
            cyl: 5,
            two_stroke: false,
            gears: &[15., 25., 37., 50., 66., 99.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "transporter",
        Engine {
            idle: 750.,
            red: 4300.,
            cyl: 4,
            two_stroke: false,
            gears: &[17., 29., 43., 57., 75., 104.],
            diesel: true,
            electric: false,
        },
    ),
    (
        "gelaende",
        Engine {
            idle: 700.,
            red: 4600.,
            cyl: 6,
            two_stroke: false,
            gears: &[16., 27., 40., 54., 70., 90., 115.],
            diesel: true,
            electric: false,
        },
    ),
    (
        "elektro",
        Engine {
            idle: 0.,
            red: 16000.,
            cyl: 0,
            two_stroke: false,
            gears: &[38.],
            diesel: false,
            electric: true,
        },
    ),
    (
        "sportwagen",
        Engine {
            idle: 950.,
            red: 8500.,
            cyl: 8,
            two_stroke: false,
            gears: &[14., 23., 33., 45., 58., 74., 98.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "heckcoupe",
        Engine {
            idle: 900.,
            red: 7800.,
            cyl: 6,
            two_stroke: false,
            gears: &[15., 25., 36., 49., 63., 80., 104.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "hothatch",
        Engine {
            idle: 850.,
            red: 6800.,
            cyl: 4,
            two_stroke: false,
            gears: &[14., 24., 35., 48., 62., 82.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "roadster",
        Engine {
            idle: 850.,
            red: 7200.,
            cyl: 4,
            two_stroke: false,
            gears: &[13., 22., 32., 43., 56., 80.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "musclecar",
        Engine {
            idle: 650.,
            red: 6000.,
            cyl: 8,
            two_stroke: false,
            gears: &[18., 30., 44., 60., 78., 118.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "oldtimer",
        Engine {
            idle: 700.,
            red: 5200.,
            cyl: 6,
            two_stroke: false,
            gears: &[16., 30., 48., 80.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "pickup",
        Engine {
            idle: 700.,
            red: 4500.,
            cyl: 4,
            two_stroke: false,
            gears: &[17., 29., 43., 57., 75., 111.],
            diesel: true,
            electric: false,
        },
    ),
    (
        "kleinbus",
        Engine {
            idle: 850.,
            red: 4500.,
            cyl: 4,
            two_stroke: false,
            gears: &[14., 26., 42., 71.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "rallye",
        Engine {
            idle: 900.,
            red: 7000.,
            cyl: 3,
            two_stroke: false,
            gears: &[13., 22., 32., 43., 56., 92.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "supersport",
        Engine {
            idle: 1000.,
            red: 8500.,
            cyl: 10,
            two_stroke: false,
            gears: &[18., 24., 33., 45., 62., 85., 115.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "gtcoupe",
        Engine {
            idle: 800.,
            red: 7000.,
            cyl: 8,
            two_stroke: false,
            gears: &[21., 29., 39., 54., 73., 100., 137.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "leichtbau",
        Engine {
            idle: 900.,
            red: 6800.,
            cyl: 4,
            two_stroke: false,
            gears: &[17., 23., 32., 44., 59., 81., 111.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "elektrosport",
        Engine {
            idle: 0.,
            red: 16000.,
            cyl: 0,
            two_stroke: false,
            gears: &[45.],
            diesel: false,
            electric: true,
        },
    ),
    (
        "sprinter",
        Engine {
            idle: 700.,
            red: 4200.,
            cyl: 4,
            two_stroke: false,
            gears: &[18., 24., 33., 45., 62., 84., 115.],
            diesel: true,
            electric: false,
        },
    ),
    (
        "hochdach",
        Engine {
            idle: 750.,
            red: 4500.,
            cyl: 4,
            two_stroke: false,
            gears: &[22., 31., 43., 61., 86., 121.],
            diesel: true,
            electric: false,
        },
    ),
    (
        "powerkombi",
        Engine {
            idle: 800.,
            red: 6800.,
            cyl: 8,
            two_stroke: false,
            gears: &[19., 25., 33., 43., 56., 73., 95., 124.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "familienkombi",
        Engine {
            idle: 750.,
            red: 4600.,
            cyl: 4,
            two_stroke: false,
            gears: &[23., 31., 42., 58., 79., 108., 148.],
            diesel: true,
            electric: false,
        },
    ),
    (
        "business",
        Engine {
            idle: 700.,
            red: 6800.,
            cyl: 6,
            two_stroke: false,
            gears: &[17., 22., 29., 38., 50., 65., 85., 111.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "sportlimo",
        Engine {
            idle: 850.,
            red: 7200.,
            cyl: 6,
            two_stroke: false,
            gears: &[19., 26., 35., 48., 65., 89., 122.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "luxus",
        Engine {
            idle: 650.,
            red: 6200.,
            cyl: 8,
            two_stroke: false,
            gears: &[19., 24., 32., 42., 55., 71., 93., 122.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "coupe",
        Engine {
            idle: 850.,
            red: 7200.,
            cyl: 6,
            two_stroke: false,
            gears: &[18., 25., 34., 47., 64., 87., 120.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "leichtcoupe",
        Engine {
            idle: 850.,
            red: 7500.,
            cyl: 4,
            two_stroke: false,
            gears: &[17., 23., 33., 46., 65., 91.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "gklasse",
        Engine {
            idle: 650.,
            red: 6200.,
            cyl: 8,
            two_stroke: false,
            gears: &[16., 21., 27., 35., 46., 60., 78., 102.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "defender",
        Engine {
            idle: 700.,
            red: 4800.,
            cyl: 6,
            two_stroke: false,
            gears: &[18., 24., 32., 41., 54., 70., 92., 120.],
            diesel: true,
            electric: false,
        },
    ),
    (
        "niva",
        Engine {
            idle: 800.,
            red: 5400.,
            cyl: 4,
            two_stroke: false,
            gears: &[17., 24., 36., 54., 79.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "kompaktsuv",
        Engine {
            idle: 800.,
            red: 6000.,
            cyl: 4,
            two_stroke: false,
            gears: &[15., 21., 29., 39., 54., 74., 101.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "sportsuv",
        Engine {
            idle: 750.,
            red: 6700.,
            cyl: 6,
            two_stroke: false,
            gears: &[17., 22., 29., 38., 50., 65., 85., 110.],
            diesel: false,
            electric: false,
        },
    ),
    (
        "grosssuv",
        Engine {
            idle: 700.,
            red: 6500.,
            cyl: 6,
            two_stroke: false,
            gears: &[17., 23., 30., 39., 51., 66., 86., 113.],
            diesel: false,
            electric: false,
        },
    ),
];
/// Schallgeschwindigkeit in px/s (343 m/s).
pub const SOUND_SPEED: f64 = 3430.;

/// Motor eines Fahrzeugs: dasselbe Modell klingt im Verkehr und selbst gefahren gleich.
pub fn engine_for(car: &Car) -> &'static Engine {
    let model = car.model_name();
    MODEL_ENGINES
        .iter()
        .find(|(k, _)| *k == model)
        .or_else(|| ENGINES.iter().find(|(k, _)| *k == car.kind))
        .map(|(_, e)| e)
        .unwrap_or(&ENGINES[0].1)
}

/// Fortlaufender Motorzustand eines Fahrzeugs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EngineState {
    pub rpm: f64,
    pub gear: usize,
    pub shift_t: f64,
    pub shift_id: u32,
    pub load: f64,
    /// Zündfrequenz (Hz)
    pub fire: f64,
    /// Arbeitszyklen je Sekunde (Grundton der Wellentabelle)
    pub cycle: f64,
    pub norm: f64,
    pub cyl: u32,
    pub diesel: bool,
    pub electric: bool,
    pub reverse: bool,
    pub speed: f64,
    pub coast: bool,
    pub boost: f64,
    pub regen: f64,
    pub voice: Voice,
}
impl Default for EngineState {
    fn default() -> Self {
        Self {
            rpm: -1.,
            gear: 1,
            shift_t: 0.,
            shift_id: 0,
            load: 0.,
            fire: 0.,
            cycle: 0.,
            norm: 0.,
            cyl: 4,
            diesel: false,
            electric: false,
            reverse: false,
            speed: 0.,
            coast: false,
            boost: 0.,
            regen: 0.,
            voice: voice_for("kompakt", false, false),
        }
    }
}

fn clamp01(v: f64) -> f64 {
    v.clamp(0., 1.)
}

/// Motorzustand fortschreiben: Hochschalten kurz vor der Abregeldrehzahl (bei wenig Gas früher), zurück unter ~40 %;
/// beim Schalten sackt die Drehzahl ab und die Last setzt kurz aus.
pub fn step_engine(st: &mut EngineState, car: &Car, dt: f64) {
    let e = engine_for(car);
    let v = car.speed();
    let thr = if car.wrecked {
        0.
    } else {
        clamp01(car.controls.throttle)
    };
    if st.rpm < 0. {
        st.rpm = e.idle;
    }
    st.shift_t = (st.shift_t - dt).max(0.);
    let rev = car.forward_speed() < -5.;
    let up = e.red * (0.62 + 0.3 * thr);
    let down = e.red * 0.38;
    let rpm_in = |g: usize| v / e.gears[g - 1] * 1000.;
    let previous = st.gear;
    st.gear = st.gear.clamp(1, e.gears.len());
    if !rev && st.shift_t == 0. && !e.electric {
        if st.gear < e.gears.len() && rpm_in(st.gear) > up && rpm_in(st.gear + 1) > down {
            st.gear += 1;
            st.shift_t = 0.18;
        } else if st.gear > 1 && rpm_in(st.gear) < down && rpm_in(st.gear - 1) < up * 0.92 {
            st.gear -= 1;
            st.shift_t = 0.12;
        }
    } else if rev || e.electric {
        st.gear = 1;
    }
    if st.gear != previous {
        st.shift_id += 1;
    }
    let wheel = rpm_in(st.gear) * if rev { 1.3 } else { 1. };
    let slip = if car.spin > 0. { 0.35 } else { 0. } + if st.gear == 1 { thr * 0.3 } else { 0. };
    let mut target = if e.electric {
        wheel
    } else {
        (e.idle + thr * e.red * 0.12).max(wheel + slip * e.red * 0.5)
    };
    if car.wrecked {
        target = 0.;
    }
    target = target.min(e.red);
    let k = if st.shift_t > 0. { 1.8 } else { 1. };
    st.rpm += (target - st.rpm) * (dt * if target > st.rpm { 7. } else { 5. } * k).min(1.);
    st.load = if st.shift_t > 0. { 0. } else { thr };
    st.fire = st.rpm / 60. * (e.cyl as f64 / if e.two_stroke { 1. } else { 2. });
    st.norm = clamp01((st.rpm - e.idle) / (e.red - e.idle));
    st.diesel = e.diesel;
    st.electric = e.electric;
    st.cyl = e.cyl;
    st.cycle = st.rpm / if e.two_stroke { 60. } else { 120. };
    st.voice = voice_for(car.model_name(), e.electric, e.diesel);
    st.reverse = rev;
    st.speed = v;
    st.coast = thr < 0.07 && v > 40.;
    let boost_target = st.voice.turbo * thr * clamp01((st.norm - 0.12) * 2.);
    st.boost +=
        (boost_target - st.boost) * (dt * if boost_target > st.boost { 2.8 } else { 8. }).min(1.);
    st.regen = if e.electric && v > 10. {
        clamp01(car.controls.brake + (1. - thr) * 0.22)
    } else {
        0.
    };
}

/// Reifen und Fahrtwind (0…1 je Schicht).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Tires {
    pub roll: f64,
    pub cobble: f64,
    pub offroad: f64,
    pub wet: f64,
    pub snow: f64,
    pub skid: f64,
    pub slide: f64,
    pub wind: f64,
    pub splash: f64,
    /// Driftwinkel 0…1 (0 = kein Drift, 1 = 60° und mehr): Tonhöhe des Quietschens
    pub angle: f64,
}
pub fn tire_state(ground: Ground, wet: f64, snow: f64, car: &Car) -> Tires {
    let v = car.speed();
    let vn = clamp01(v / 330.);
    let (s, c) = car.angle.sin_cos();
    let lat = (-car.vx * s + car.vy * c).abs();
    let snow = clamp01(snow * 1.4);
    let wet = clamp01(wet - snow * 0.5);
    let dynamic = car.dyn_state.is_some();
    let brake = if !dynamic && car.controls.brake > 0.6 && v > 90. {
        0.6
    } else {
        0.
    };
    let hand = if !dynamic && car.controls.handbrake && v > 60. {
        0.8
    } else {
        0.
    };
    let grip = 1. - snow.max(wet * 0.8);
    let skid = clamp01(
        ((lat - 40.) / 120.)
            .max(brake)
            .max(hand)
            .max(if car.spin > 0. { 0.7 } else { 0. })
            .max(if dynamic { car.skid } else { 0. }),
    ) * if v > 15. || car.spin > 0. { 1. } else { 0. };
    Tires {
        roll: vn,
        cobble: if ground == Ground::Cobble { vn } else { 0. },
        offroad: if ground == Ground::Grass { vn } else { 0. },
        wet: wet * clamp01(v / 120.),
        snow: snow * clamp01(v / 60.),
        skid: skid * grip,
        slide: skid * (1. - grip),
        wind: vn * vn,
        splash: if car.aqua > 0. { 1. } else { 0. },
        angle: car
            .dyn_state
            .as_ref()
            .map_or(0., |d| clamp01(d.drift_angle.to_degrees() / 60.)),
    }
}

/// Stimme eines fremden Fahrzeugs (lauteste zuerst).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CarVoice {
    pub id: u32,
    pub d: f64,
    pub gain: f64,
    pub pan: f64,
    /// Dopplerfaktor (> 1 kommt näher)
    pub rate: f64,
    pub engine: EngineState,
    pub tire: f64,
}

/// Merkt sich den Motorzustand jedes hörbaren Fahrzeugs zwischen den Abfragen.
#[derive(Default)]
pub struct Voices {
    state: HashMap<u32, (EngineState, f64)>,
}
impl Voices {
    /// Die nächsten `n` fremden Fahrzeuge im Umkreis `r` um den Hörer (x, y, vx, vy).
    pub fn near(
        &mut self,
        world: &World,
        listener: (f64, f64, f64, f64),
        n: usize,
        r: f64,
        exclude: Option<u32>,
    ) -> Vec<CarVoice> {
        let mut out = Vec::new();
        for c in &world.cars {
            if Some(c.id) == exclude || c.wrecked || c.driver.is_none() || c.kind_info().bike {
                continue;
            }
            if let Some(v) = self.voice(world, c, listener, r) {
                out.push(v);
            }
        }
        // Fahrzeuge außer Hörweite vergessen
        if self.state.len() > 256 {
            let live: Vec<u32> = world.cars.iter().map(|c| c.id).collect();
            self.state.retain(|id, _| live.contains(id));
        }
        out.sort_by(|a, b| b.gain.total_cmp(&a.gain));
        out.truncate(n);
        out
    }
    /// Stimme eines Fahrzeugs für den Hörer (x, y, vx, vy) im Umkreis `r`; `None` außer Hörweite.
    pub fn voice(
        &mut self,
        world: &World,
        c: &Car,
        listener: (f64, f64, f64, f64),
        r: f64,
    ) -> Option<CarVoice> {
        let (lx, ly, lvx, lvy) = listener;
        {
            let (dx, dy) = (c.x - lx, c.y - ly);
            let d = dx.hypot(dy);
            if d >= r {
                return None;
            }
            let e = engine_for(c);
            let v = c.speed();
            let (st, at) = self
                .state
                .entry(c.id)
                .or_insert((EngineState::default(), world.time));
            let dt = (world.time - *at).clamp(0., 0.2);
            step_engine(st, c, if dt > 0. { dt } else { 0.05 });
            *at = world.time;
            let dd = if d > 0. { d } else { 1. };
            let (ux, uy) = (dx / dd, dy / dd);
            let vr = -((c.vx - lvx) * ux + (c.vy - lvy) * uy);
            let big = if e.diesel && matches!(c.kind, "truck" | "bus" | "garbage") {
                1.6
            } else {
                1.
            };
            let gain = (1. - d / r).powi(2)
                * (0.35 + 0.65 * clamp01(v / 200. + c.controls.throttle * 0.4))
                * big;
            Some(CarVoice {
                id: c.id,
                d,
                gain,
                pan: (dx / 300.).clamp(-1., 1.),
                rate: SOUND_SPEED / (SOUND_SPEED - vr.clamp(-1500., 1500.)),
                engine: *st,
                tire: clamp01(v / 330.),
            })
        }
    }
}

/// Schrittlänge nach Tempo (Gehen 0,75 m, Joggen 1,1 m, Sprint 1,6 m).
pub fn stride_of(speed: f64) -> f64 {
    if speed < 20. {
        7.5
    } else if speed < 45. {
        11.
    } else {
        16.
    }
}
/// Anzahl der Schritte zwischen zwei Ständen des Wegzählers.
pub fn steps_between(prev: f64, now: f64, speed: f64) -> u32 {
    if speed <= 0. || now <= prev {
        return 0;
    }
    let l = stride_of(speed);
    ((now / l).floor() - (prev / l).floor()).max(0.) as u32
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Footstep {
    Hard,
    Grass,
    Wet,
    Snow,
}
pub fn footstep_kind(ground: Ground, wet: f64, snow: f64) -> Footstep {
    if snow > 0.25 {
        Footstep::Snow
    } else if ground == Ground::Grass {
        Footstep::Grass
    } else if wet > 0.35 {
        Footstep::Wet
    } else {
        Footstep::Hard
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::car::Role;

    fn car(vx: f64, throttle: f64) -> Car {
        let mut c = Car::new(1, 0., 0., 0., 0, Role::Player, "car");
        c.vx = vx;
        c.controls.throttle = throttle;
        c
    }
    #[test]
    fn engine_idles_revs_and_shifts_up() {
        let mut st = EngineState::default();
        let c = car(0., 0.);
        for _ in 0..60 {
            step_engine(&mut st, &c, 1. / 60.);
        }
        let e = engine_for(&c);
        assert_eq!(e.cyl, 6, "Spielerauto ist eine Limousine (Sechszylinder)");
        assert!((st.rpm - e.idle).abs() < 1. && st.gear == 1);
        assert!(
            (st.fire - st.rpm / 60. * 3.).abs() < 1e-9 && (st.cycle - st.rpm / 120.).abs() < 1e-9
        );
        let mut gears = vec![];
        for v in (0..300).step_by(10) {
            let c = car(v as f64, 1.);
            for _ in 0..30 {
                step_engine(&mut st, &c, 1. / 60.);
            }
            gears.push(st.gear);
        }
        assert!(
            gears.windows(2).all(|w| w[1] >= w[0]),
            "beim Beschleunigen nur hochschalten: {gears:?}"
        );
        assert!(*gears.last().unwrap() >= 4 && st.rpm <= e.red);
        let mut c = car(-50., 0.);
        c.controls.brake = 1.;
        step_engine(&mut st, &c, 0.1);
        assert!(st.reverse && st.gear == 1);
    }
    #[test]
    fn tires_and_steps() {
        let mut c = car(300., 0.);
        c.vy = 0.;
        let t = tire_state(Ground::Cobble, 0., 0., &c);
        assert!(t.cobble > 0.8 && t.skid == 0. && t.wind > 0.8);
        c.vy = 160.;
        let t = tire_state(Ground::Road, 0., 0., &c);
        assert!(t.skid == 1. && t.slide == 0.);
        let t = tire_state(Ground::Road, 0., 1., &c);
        assert!(
            t.skid == 0. && t.slide == 1.,
            "auf Schnee rauscht das Rutschen nur"
        );
        assert_eq!(steps_between(0., 30., 15.), 4);
        assert_eq!(steps_between(30., 30., 15.), 0);
        assert_eq!(footstep_kind(Ground::Grass, 0., 0.), Footstep::Grass);
        assert_eq!(footstep_kind(Ground::Road, 0.5, 0.), Footstep::Wet);
        assert_eq!(footstep_kind(Ground::Grass, 0., 0.5), Footstep::Snow);
    }
}
