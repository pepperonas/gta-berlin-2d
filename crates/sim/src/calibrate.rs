//! Standardtests der Fahrphysik (headless, ohne Darstellung), geteilt von `tools/physics-calibrate` und den
//! Unit-Tests: Beschleunigung, Höchsttempo, Bremsweg, Querbeschleunigung auf der Kreisbahn. Jeder Test liefert
//! sein Ergebnis und den Verlauf im 100-ms-Takt (Zeit, Tempo, Gang, Drehzahl, Strecke).
//!
//! Bedingungen wie im Prompt: trockener Asphalt, `realismus = 1`, `grip_global = 1`; Zuladung nach
//! `Vehicle::calib_load` (LKW und Busse voll, sonst leer). Fahrräder beschleunigen mit Sprint und fahren Vmax mit
//! Dauerleistung.
use crate::vehdata::{Feel, G, Vehicle};
use crate::vphys::{Env, HZ, Input, STEP, State, steer_limit, step};
use std::collections::BTreeMap;

/// Ein Messpunkt des Verlaufs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub t: f64,
    pub kmh: f64,
    pub gear: usize,
    pub rpm: f64,
    pub dist: f64,
}
/// Ergebnis eines Tests: Messwerte (Schlüssel wie die Ziele: `0_100`, `vmax`, `brems_100`, `quer_g`) und Verlauf.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Run {
    pub values: BTreeMap<String, f64>,
    pub trace: Vec<Sample>,
}

fn sample(t: f64, s: &State) -> Sample {
    Sample {
        t,
        kmh: s.vx * 3.6,
        gear: s.gear + 1,
        rpm: s.rpm,
        dist: s.dist,
    }
}
/// Zweirad wie bei Zeitschriften-Messungen gefahren: der Testfahrer hält den Wheelie an der Grenze und bremst
/// an der Blockiergrenze (auch ohne ABS) – sonst endete jede Vollgas- bzw. Vollbremsung im Sturz.
fn tester(v: &Vehicle) -> std::borrow::Cow<'_, Vehicle> {
    if v.two_wheel {
        let mut t = v.clone();
        t.wheelie_control = true;
        t.brake.abs = true;
        std::borrow::Cow::Owned(t)
    } else {
        std::borrow::Cow::Borrowed(v)
    }
}

fn start(v: &Vehicle) -> State {
    State {
        load: v.calib_load,
        ..Default::default()
    }
}

/// Zielgeschwindigkeiten (km/h) der Beschleunigung: aus den Zielen `0_X`, sonst 50/100/200 (Zweiräder 25).
pub fn accel_marks(v: &Vehicle) -> Vec<f64> {
    let mut m: Vec<f64> = v
        .targets
        .keys()
        .filter_map(|k| k.strip_prefix("0_").and_then(|x| x.parse().ok()))
        .collect();
    // Standard: 0-50, 0-100, 0-200 (wenn das Fahrzeug 220 km/h schafft); Fahrrad und Scooter 0-25
    let reach = v.targets.get("vmax").copied().unwrap_or(300.);
    let std: &[f64] = if v.two_wheel && reach < 60. {
        &[25.]
    } else {
        &[50., 100., 200.]
    };
    for &x in std {
        if x < reach * 0.92 && (x < 200. || reach >= 220.) {
            m.push(x);
        }
    }
    m.sort_by(f64::total_cmp);
    m.dedup();
    m
}

/// Beschleunigung aus dem Stand, Volllast (Launch Control und Traktionskontrolle, soweit vorhanden). Gestartet wird
/// wie bei Zeitschriften-Messungen mit Bremse und Gas: der Turbolader ist beim Lösen der Bremse gespannt.
pub fn accel(v: &Vehicle, feel: &Feel) -> Run {
    let v = &*tester(v);
    let marks = accel_marks(v);
    let mut s = start(v);
    s.boost = 1.;
    let inp = Input {
        throttle: 1.,
        sprint: true,
        ..Default::default()
    };
    let env = Env::default();
    let mut run = Run::default();
    let max_t = 180.;
    let mut t = 0f64;
    let mut next = 0;
    while t < max_t && next < marks.len() {
        if (t * 10. + 1e-6).floor() as usize >= run.trace.len() {
            run.trace.push(sample(t, &s));
        }
        step(v, feel, &mut s, &inp, &env, STEP);
        t += STEP;
        while next < marks.len() && s.vx * 3.6 >= marks[next] {
            run.values
                .insert(format!("0_{}", marks[next].round() as i64), t);
            next += 1;
        }
    }
    run
}

/// Höchsttempo: Volllast bis zur Beharrung (weniger als 0,2 km/h Zuwachs in 5 s) oder bis zum Begrenzer.
pub fn vmax(v: &Vehicle, feel: &Feel) -> Run {
    let mut s = start(v);
    let inp = Input {
        throttle: 1.,
        ..Default::default()
    };
    let env = Env::default();
    let mut run = Run::default();
    let mut t = 0f64;
    let mut best = 0.;
    let mut best_t = 0.;
    while t < 600. {
        if (t * 10. + 1e-6).floor() as usize >= run.trace.len() {
            run.trace.push(sample(t, &s));
        }
        step(v, feel, &mut s, &inp, &env, STEP);
        t += STEP;
        let k = s.vx * 3.6;
        if k > best + 0.2 {
            best = k;
            best_t = t;
        }
        if t - best_t > 5. {
            break;
        }
    }
    run.values.insert("vmax".into(), best.max(s.vx * 3.6));
    run
}

/// Bremsweg aus `kmh` bis zum Stand, Vollbremsung, inklusive Aufbauzeit.
pub fn brake(v: &Vehicle, feel: &Feel, kmh: f64) -> Run {
    let v = &*tester(v);
    let mut s = start(v);
    // passenden Gang einlegen (Motorbremse wie im Fahrbetrieb)
    s.vx = kmh / 3.6;
    let n = v.gearbox.ratios.len();
    s.gear = (0..n)
        .rev()
        .find(|&g| {
            let rpm = s.vx / v.wheel_r * v.gearbox.ratios[g] * 60. / (2. * std::f64::consts::PI);
            rpm > v.engine.n_max * 0.35
        })
        .unwrap_or(0);
    let inp = Input {
        brake: 1.,
        ..Default::default()
    };
    let env = Env::default();
    let mut run = Run::default();
    let mut t = 0f64;
    while s.vx > 0.01 && t < 60. {
        if (t * 10. + 1e-6).floor() as usize >= run.trace.len() {
            run.trace.push(sample(t, &s));
        }
        step(v, feel, &mut s, &inp, &env, STEP);
        t += STEP;
    }
    run.trace.push(sample(t, &s));
    run.values
        .insert(format!("brems_{}", kmh.round() as i64), s.dist);
    run
}

/// Querbeschleunigung: stationäre Kreisfahrt (Radius 40 m), Tempo langsam steigern, bis das Auto die Bahn nicht mehr
/// hält. Ergebnis: größte über 1 s gemittelte Querbeschleunigung in g, solange die Bahn gehalten wird.
pub fn lateral(v: &Vehicle, feel: &Feel) -> Run {
    const R: f64 = 40.;
    let mut s = start(v);
    s.vx = 5.;
    // Kreis um (0, R) gegen den Uhrzeigersinn, Start bei (0, 0) mit Blick +x
    let env = Env::default();
    let mut run = Run::default();
    let mut t = 0f64;
    let mut ay_win: Vec<f64> = Vec::new();
    let mut best = 0f64;
    let mut lost = 0.;
    let mut i_err = 0.;
    while t < 240. {
        let speed = s.vx.max(0.1);
        // Zieltempo steigt um 0,12 m/s je Sekunde
        let v_t = 5. + 0.12 * t;
        let e = v_t - speed;
        i_err = (i_err + e * STEP).clamp(-5., 5.);
        let th = (0.4 * e + 0.15 * i_err).clamp(0., 1.);
        let br = (-0.5 * e).clamp(0., 1.);
        // Pure Pursuit: Zielpunkt auf dem Kreis, Bogenlänge voraus
        let ang = (s.y - R).atan2(s.x); // Winkel des Fahrzeugs um den Mittelpunkt (0, R)
        let ld = (0.6 * speed).max(6.);
        let tgt_a = ang + ld / R;
        let (tx, ty) = (R * tgt_a.cos(), R + R * tgt_a.sin());
        let (dx, dy) = (tx - s.x, ty - s.y);
        let alpha = dy.atan2(dx) - s.yaw;
        let alpha = (alpha + std::f64::consts::PI).rem_euclid(2. * std::f64::consts::PI)
            - std::f64::consts::PI;
        let delta = (2. * v.wheelbase * alpha.sin() / ld).atan();
        let lim = steer_limit(v, speed);
        let inp = Input {
            throttle: th,
            brake: br,
            steer: (delta / lim).clamp(-1., 1.),
            // ESP aus: gemessen wird die Haftgrenze (Skidpad), nicht der Regeleingriff
            esp: Some(crate::vehdata::Esp::Off),
            ..Default::default()
        };
        if (t * 10. + 1e-6).floor() as usize >= run.trace.len() {
            run.trace.push(sample(t, &s));
        }
        step(v, feel, &mut s, &inp, &env, STEP);
        t += STEP;
        let rad_err = ((s.x).hypot(s.y - R) - R).abs();
        let ay = s.vx * s.r;
        ay_win.push(ay);
        if ay_win.len() > HZ as usize {
            ay_win.remove(0);
        }
        // nur stabile Fahrt zählt: Bahn gehalten und kein Heckausbruch (Schwimmwinkel unter 6°)
        if rad_err < 1.5 && t > 5. && s.beta().abs() < 6f64.to_radians() {
            let avg = ay_win.iter().sum::<f64>() / ay_win.len() as f64;
            best = best.max(avg.abs() / G);
            lost = 0.;
        } else if t > 5. {
            lost += STEP;
            if lost > 1.5 {
                break;
            }
        }
    }
    run.values.insert("quer_g".into(), best);
    run
}

/// Alle Tests, die zu den Zielen eines Fahrzeugs passen (Zweiräder: Querbeschleunigung folgt in Phase 5).
pub fn all(v: &Vehicle, feel: &Feel) -> Vec<(String, Run)> {
    let mut out = vec![
        ("beschleunigung".into(), accel(v, feel)),
        ("vmax".into(), vmax(v, feel)),
    ];
    let mut brakes: Vec<f64> = v
        .targets
        .keys()
        .filter_map(|k| k.strip_prefix("brems_").and_then(|x| x.parse().ok()))
        .collect();
    if brakes.is_empty() {
        brakes.push(if v.two_wheel { 25. } else { 100. });
    }
    for b in brakes {
        out.push((format!("bremsen_{b}"), brake(v, feel, b)));
    }
    if !v.two_wheel {
        out.push(("kreisfahrt".into(), lateral(v, feel)));
    }
    out
}

/// Toleranz je Zielwert: 0-X ±7 %, Vmax ±3 %, Bremsweg ±5 %, Querbeschleunigung ±0,05 g.
pub fn within(key: &str, target: f64, actual: f64) -> bool {
    match key {
        "vmax" => (actual - target).abs() <= target * 0.03,
        "quer_g" => (actual - target).abs() <= 0.05,
        k if k.starts_with("brems_") => (actual - target).abs() <= target * 0.05,
        _ => (actual - target).abs() <= target * 0.07,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vehdata::VehicleDb;

    #[test]
    fn standard_tests_produce_values_and_traces() {
        let db = VehicleDb::embedded().unwrap();
        let feel = Feel::simulation();
        let v = db.get("kompakt_benzin").unwrap();
        let a = accel(v, &feel);
        let t100 = a.values["0_100"];
        assert!(t100 > 5. && t100 < 15., "0-100 {t100}");
        assert!(
            a.trace.len() > 40
                && a.trace
                    .windows(2)
                    .all(|w| (w[1].t - w[0].t - 0.1).abs() < 0.02)
        );
        let vm = vmax(v, &feel).values["vmax"];
        assert!(vm > 180. && vm < 260., "Vmax {vm}");
        let b = brake(v, &feel, 100.).values["brems_100"];
        assert!(b > 28. && b < 45., "Bremsweg {b}");
        let q = lateral(v, &feel).values["quer_g"];
        assert!(q > 0.7 && q < 1.2, "quer {q}");
    }

    #[test]
    fn ratios_between_vehicles_match_reality() {
        // Akzeptanztest 3: nach 3 s liegen Welten zwischen Turbo S und Kleinwagen. Der Prompt nennt ~115 und ~25 km/h;
        // ein Kleinwagen, der seine 0–100-Zeit trifft, steht nach 3 s bei gut 30 km/h (Konflikt im Bericht gemeldet)
        let db = VehicleDb::embedded().unwrap();
        let feel = Feel::simulation();
        let at3 = |id: &str| {
            let r = accel(&db.calibrated(id).unwrap(), &feel);
            r.trace
                .iter()
                .find(|s| s.t >= 3.)
                .map(|s| s.kmh)
                .unwrap_or(0.)
        };
        let (turbo, small) = (at3("turbo_s"), at3("kleinwagen_65ps"));
        assert!(
            turbo > 100. && small < 45. && turbo > 2.5 * small,
            "Turbo S {turbo}, Kleinwagen {small}"
        );
    }
}
