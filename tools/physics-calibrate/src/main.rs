//! Kalibrier-Werkzeug der Fahrphysik (headless). Simuliert jedes Fahrzeug durch die Standardtests
//! (`berlin_sim::calibrate`) und dreht dabei nur die erlaubten Stellschrauben, in dieser Reihenfolge:
//!
//! 1. `cwA` ±15 % für die Vmax,
//! 2. Reifen-μ ±10 % für Bremsweg und Querbeschleunigung,
//! 3. Bremskraft für den Bremsweg (nur wirksam, wo die Bremse und nicht die Haftung begrenzt),
//! 4. Übersetzungen (alle Gänge außer dem letzten), Schaltzeit und Antriebswirkungsgrad ±3 % für die Beschleunigung.
//!
//! Masse, Leistung und Drehmoment werden nie verändert. Was so nicht erreichbar ist, meldet das Werkzeug mit Grund.
//!
//! Aufruf (aus dem Repo):
//!   cargo run --release -p physics-calibrate                 kalibrieren, Bericht und Dateien schreiben
//!   cargo run --release -p physics-calibrate -- --pruefen    nur prüfen (kalibrierte Werte), Rückgabe 1 bei Abweichung
//!   cargo run --release -p physics-calibrate -- --nur ID     ein Fahrzeug
//! Ausgabe: `data/vehicles/vehicles.calibrated.json`, `docs/kalibrierung/bericht.md`, `docs/kalibrierung/csv/<id>.csv`.
use anyhow::{Context, Result};
use berlin_sim::calibrate::{self, Run};
use berlin_sim::vehdata::{Calibration, Feel, Vehicle, VehicleDb};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Grenzen der Stellschrauben.
const CWA: (f64, f64) = (0.85, 1.15);
const MU: (f64, f64) = (0.90, 1.10);
const BRAKE: (f64, f64) = (0.5, 1.6);
const GEAR: (f64, f64) = (0.4, 1.6);
const SHIFT: (f64, f64) = (0.5, 1.5);
const ETA: (f64, f64) = (0.97, 1.03);

fn tol(key: &str, target: f64) -> f64 {
    match key {
        "vmax" => target * 0.03,
        "quer_g" => 0.05,
        k if k.starts_with("brems_") => target * 0.05,
        _ => target * 0.07,
    }
}

fn watts(w: f64) -> String {
    if w < 2000. {
        format!("{w:.0} W")
    } else {
        format!("{:.0} kW", w / 1e3)
    }
}
fn heavy(v: &Vehicle) -> bool {
    ["bus", "lkw"].iter().any(|p| v.class.starts_with(p))
}
fn muscle(v: &Vehicle) -> bool {
    matches!(v.engine.power, berlin_sim::vehdata::Power::Muscle { .. })
}

/// Untergrenze der Beschleunigungszeit auf `kmh`: konstante Spitzenleistung ohne Schaltpausen und Getriebeverluste
/// über Wirkungsgrad +3 %, kleinster Massenfaktor 1,04, cwA −15 %, Traktion mit μ +10 % auf der statischen
/// Achslast. Liegt sie über dem Ziel, kann keine erlaubte Schraube das Ziel erreichen.
fn ideal_time(v: &Vehicle, kmh: f64) -> f64 {
    use berlin_sim::vehdata::Drive;
    let (m, _) = v.loaded(v.calib_load);
    let p = v.engine.watts.max(v.engine.peak_watts) * (v.efficiency * ETA.1).min(1.);
    let share = match v.drive {
        Drive::Fwd => v.front,
        Drive::Rwd => 1. - v.front,
        Drive::Awd => 1.,
    };
    let grip = v.tire.mu_long * MU.1 * share * m * 9.81;
    let (target, dt) = (kmh / 3.6, 0.005);
    let (mut speed, mut t) = (0., 0.);
    while speed < target && t < 300. {
        let f = (p / speed.max(0.1)).min(grip)
            - 0.5 * 1.2 * v.cw_a * CWA.0 * speed * speed
            - v.tire.rolling * m * 9.81;
        if f <= 0. {
            return f64::INFINITY;
        }
        speed += f / (m * 1.04) * dt;
        t += dt;
    }
    t
}

/// Alle Messwerte eines Fahrzeugs mit gegebener Kalibrierung.
fn measure(v: &Vehicle, cal: &Calibration, feel: &Feel, keys: &[&str]) -> BTreeMap<String, f64> {
    let vv = cal.apply(v);
    let mut out = BTreeMap::new();
    let want = |p: &str| keys.iter().any(|k| k.starts_with(p));
    if keys.iter().any(|k| k.starts_with("0_")) {
        out.extend(calibrate::accel(&vv, feel).values);
    }
    if want("vmax") {
        out.extend(calibrate::vmax(&vv, feel).values);
    }
    for k in keys.iter().filter(|k| k.starts_with("brems_")) {
        let kmh: f64 = k[6..].parse().unwrap_or(100.);
        out.extend(calibrate::brake(&vv, feel, kmh).values);
    }
    if want("quer_g") {
        out.extend(calibrate::lateral(&vv, feel).values);
    }
    out
}

/// Normierte Abweichung (1 = an der Toleranzgrenze) über die gegebenen Ziele; fehlende Messung = groß.
fn score(v: &Vehicle, m: &BTreeMap<String, f64>, keys: &[&str]) -> f64 {
    keys.iter()
        .filter_map(|k| v.targets.get(*k).map(|t| (k, t)))
        .map(|(k, t)| match m.get(*k) {
            Some(a) => (a - t).abs() / tol(k, *t),
            None => 99.,
        })
        .fold(0., f64::max)
}

fn keys_of<'a>(v: &'a Vehicle, prefix: &[&str]) -> Vec<&'a str> {
    v.targets
        .keys()
        .map(String::as_str)
        .filter(|k| prefix.iter().any(|p| k.starts_with(p)))
        .collect()
}

/// Bisektion über eine monoton wirkende Schraube: f(x) = Messwert − Ziel; liefert den besten Wert im Intervall.
fn bisect(lo: f64, hi: f64, mut f: impl FnMut(f64) -> f64) -> f64 {
    let (flo, fhi) = (f(lo), f(hi));
    if flo.signum() == fhi.signum() {
        return if flo.abs() < fhi.abs() { lo } else { hi };
    }
    let (mut a, mut b, mut fa) = (lo, hi, flo);
    for _ in 0..18 {
        let m = (a + b) / 2.;
        let fm = f(m);
        if fm.signum() == fa.signum() {
            a = m;
            fa = fm;
        } else {
            b = m;
        }
    }
    (a + b) / 2.
}

struct Outcome {
    cal: Calibration,
    measured: BTreeMap<String, f64>,
    reasons: Vec<String>,
}

fn calibrate_one(v: &Vehicle, feel: &Feel) -> Outcome {
    let mut cal = Calibration::default();
    let mut reasons = Vec::new();
    let all: Vec<&str> = v.targets.keys().map(String::as_str).collect();
    for _pass in 0..2 {
        // 1. Vmax über cwA (nur wenn kein Begrenzer die Vmax bestimmt)
        if let Some(&t) = v.targets.get("vmax") {
            // ein Begrenzer bestimmt die Vmax nur, wenn das Fahrzeug ihn auch erreicht
            let limited = v.limiter.is_some_and(|l| l * 3.6 <= t * 1.01)
                && calibrate::vmax(&cal.apply(v), feel).values["vmax"] >= t - tol("vmax", t);
            if !limited {
                cal.cw_a = bisect(CWA.0, CWA.1, |x| {
                    let c = Calibration { cw_a: x, ..cal };
                    // mehr cwA → weniger Vmax: Vorzeichen so, dass die Funktion steigt
                    t - calibrate::vmax(&c.apply(v), feel).values["vmax"]
                });
            }
        }
        // 2. μ für Bremsweg und Querbeschleunigung
        let bq = keys_of(v, &["brems_", "quer_g"]);
        if !bq.is_empty() {
            let mut best = (f64::INFINITY, 1.);
            let mut x = MU.0;
            while x <= MU.1 + 1e-9 {
                let c = Calibration { mu: x, ..cal };
                let sc = score(v, &measure(v, &c, feel, &bq), &bq);
                if sc < best.0 - 1e-9 {
                    best = (sc, x);
                }
                x += 0.01;
            }
            cal.mu = best.1;
        }
        // 3. Bremskraft
        let bk = keys_of(v, &["brems_"]);
        if let Some(k) = bk.first() {
            let t = v.targets[*k];
            cal.brake = bisect(BRAKE.0, BRAKE.1, |x| {
                let c = Calibration { brake: x, ..cal };
                t - measure(v, &c, feel, &[k])[*k]
            });
            // eine Schraube ohne Wirkung (haftungsbegrenzt) bleibt unverändert
            let base = measure(v, &cal, feel, &[k])[*k];
            let c1 = Calibration { brake: 1., ..cal };
            if (measure(v, &c1, feel, &[k])[*k] - base).abs() < 0.05 {
                cal.brake = 1.;
            }
        }
        // 4. Beschleunigung: Übersetzung, Schaltzeit, Wirkungsgrad
        let ak = keys_of(v, &["0_"]);
        if !ak.is_empty() {
            let mut best = (score(v, &measure(v, &cal, feel, &ak), &ak), cal);
            let gears = v.gearbox.ratios.len() > 1;
            // Wheelie-Control: nie länger übersetzen – sonst erreicht der erste Gang die Kippgrenze nicht mehr und das
            // Motorrad macht beim Ampelstart keinen Wheelie (Akzeptanzszene 13 hat Vorrang vor der 0–100-Zeit)
            let floor = if v.two_wheel && v.wheelie_control {
                1.
            } else {
                0.
            };
            let gear_steps: Vec<f64> = if gears {
                (0..=24)
                    .map(|i| GEAR.0 + (GEAR.1 - GEAR.0) * i as f64 / 24.)
                    .filter(|&g| g >= floor)
                    .collect()
            } else {
                vec![1.]
            };
            let shift_steps: Vec<f64> = if gears && v.gearbox.shift > 0. {
                vec![SHIFT.0, 0.75, 1., 1.25, SHIFT.1]
            } else {
                vec![1.]
            };
            for &g in &gear_steps {
                for &sh in &shift_steps {
                    for &e in &[ETA.0, 1., ETA.1] {
                        let c = Calibration {
                            gear: g,
                            shift: sh,
                            efficiency: e,
                            ..cal
                        };
                        let sc = score(v, &measure(v, &c, feel, &ak), &ak);
                        // bei Gleichstand die kleinste Änderung
                        let change = (g - 1.).abs() + (sh - 1.).abs() * 0.1 + (e - 1.).abs();
                        let bchange = (best.1.gear - 1.).abs()
                            + (best.1.shift - 1.).abs() * 0.1
                            + (best.1.efficiency - 1.).abs();
                        if sc < best.0 - 1e-6 || (sc <= 1. && best.0 <= 1. && change < bchange) {
                            best = (sc, c);
                        }
                    }
                }
            }
            cal = best.1;
        }
    }
    let measured = measure(v, &cal, feel, &all);
    // Gründe für verbleibende Abweichungen
    for k in &all {
        let t = v.targets[*k];
        let Some(&a) = measured.get(*k) else {
            continue;
        };
        if (a - t).abs() <= tol(k, t) {
            continue;
        }
        let edge = |x: f64, (lo, hi): (f64, f64)| (x - lo).abs() < 1e-3 || (x - hi).abs() < 1e-3;
        let why = match *k {
            "vmax" if v.limiter.is_some_and(|l| l * 3.6 < t) => {
                format!("Begrenzer {:.0} km/h liegt unter dem Ziel", v.limiter.unwrap() * 3.6)
            }
            "vmax" if a < t && edge(cal.cw_a, CWA) => format!(
                "Leistung reicht auch mit cwA −15 % nicht (Fahrwiderstände bei {t:.0} km/h größer als {} × Wirkungsgrad)",
                watts(v.engine.peak_watts.max(v.engine.watts))
            ),
            "vmax" if a > t && edge(cal.cw_a, CWA) => "Leistung zu groß für das Ziel, auch mit cwA +15 %".into(),
            "quer_g" if a > t && heavy(v) => {
                "Nutzfahrzeug: rutscht vor dem Kippen; im Spiel begrenzt die Wankstabilisierung (RSC, mit ESP) auf 75 % der Kippgrenze, gemessen wird ohne ESP".into()
            }
            "quer_g" if a > t => {
                "Reifenhaftung zu groß, auch mit μ −10 % (Untersteuern/ESP-Eingriff begrenzen real, Phase 3)".into()
            }
            "quer_g" if a < t => "Reifenhaftung reicht auch mit μ +10 % nicht (Lastverlagerung, Schwerpunkt)".into(),
            "quer_g" => "Reifenhaftung zu groß, auch mit μ −10 %".into(),
            k if k.starts_with("brems_") && a > t => {
                "Haftung begrenzt: auch mit μ +10 % und ABS kein kürzerer Bremsweg".into()
            }
            k if k.starts_with("brems_") && v.two_wheel => {
                "Zweirad bremst an der Überschlaggrenze (Stoppie); die Bremskraft-Schraube wirkt erst darunter".into()
            }
            k if k.starts_with("brems_") => "Bremsweg kürzer als das Ziel, auch mit Bremskraft −50 %".into(),
            k if k.starts_with("0_") && a > t => {
                let kmh: f64 = k[2..].parse().unwrap_or(100.);
                let ideal = ideal_time(v, kmh);
                if ideal > t + tol(k, t) {
                    format!(
                        "physikalisch nicht erreichbar: selbst konstante Spitzenleistung ohne Schaltpausen braucht {ideal:.1} s"
                    )
                } else {
                    format!(
                        "zu langsam: Ideal mit konstanter Spitzenleistung {ideal:.1} s, Drehmomentverlauf/Schaltpausen kosten den Rest"
                    )
                }
            }
            k if k.starts_with("0_") && muscle(v) => {
                "Ziel beschreibt einen Alltagsantritt, der Test fährt Sprint (Kraftgrenze der Kurve muskel)".into()
            }
            k if k.starts_with("0_") && v.two_wheel && v.wheelie_control => {
                let (_, b) = v.axle_distances();
                let (_, h) = v.loaded(v.calib_load);
                format!(
                    "zu schnell: die Kippgrenze g·l_h/h = {:.2} g erlaubt mehr; eine längere Übersetzung nähme den \
                     Wheelie beim Ampelstart (Akzeptanzszene 13 hat Vorrang)",
                    b / h
                )
            }
            k if k.starts_with("0_") => "zu schnell: auch mit längster Übersetzung".into(),
            _ => "außerhalb der Toleranz".into(),
        };
        reasons.push(format!("{k}: {why}"));
    }
    Outcome {
        cal,
        measured,
        reasons,
    }
}

fn csv(v: &Vehicle, feel: &Feel) -> String {
    let mut out = String::from("test,t_s,kmh,gang,drehzahl,strecke_m\n");
    for (name, Run { trace, .. }) in calibrate::all(v, feel) {
        for s in trace {
            let _ = writeln!(
                out,
                "{name},{:.1},{:.2},{},{:.0},{:.2}",
                s.t, s.kmh, s.gear, s.rpm, s.dist
            );
        }
    }
    out
}

fn fmt_val(k: &str, x: f64) -> String {
    match k {
        "quer_g" => format!("{x:.2} g"),
        "vmax" => format!("{x:.0} km/h"),
        k if k.starts_with("brems_") => format!("{x:.1} m"),
        _ => format!("{x:.1} s"),
    }
}
fn label(k: &str) -> String {
    match k {
        "vmax" => "Vmax".into(),
        "quer_g" => "Querbeschl.".into(),
        k if k.starts_with("brems_") => format!("Bremsweg {} km/h", &k[6..]),
        k => format!("0–{} km/h", &k[2..]),
    }
}
fn screws(c: &Calibration) -> String {
    let mut s = Vec::new();
    let pct = |x: f64| format!("{:+.0} %", (x - 1.) * 100.);
    if (c.cw_a - 1.).abs() > 1e-3 {
        s.push(format!("cwA {}", pct(c.cw_a)));
    }
    if (c.mu - 1.).abs() > 1e-3 {
        s.push(format!("μ {}", pct(c.mu)));
    }
    if (c.brake - 1.).abs() > 1e-3 {
        s.push(format!("Bremskraft {}", pct(c.brake)));
    }
    if (c.gear - 1.).abs() > 1e-3 {
        s.push(format!("Übersetzung ×{:.2}", c.gear));
    }
    if (c.shift - 1.).abs() > 1e-3 {
        s.push(format!("Schaltzeit ×{:.2}", c.shift));
    }
    if (c.efficiency - 1.).abs() > 1e-3 {
        s.push(format!("Wirkungsgrad {}", pct(c.efficiency)));
    }
    if s.is_empty() {
        "–".into()
    } else {
        s.join(", ")
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let check = args.iter().any(|a| a == "--pruefen");
    let only = args
        .iter()
        .position(|a| a == "--nur")
        .and_then(|i| args.get(i + 1).cloned());
    let root = repo_root();
    let dir = root.join("data/vehicles");
    let db = VehicleDb::from_dir(&dir)?;
    let feel = Feel::simulation();
    let vehicles: Vec<&Vehicle> = db
        .vehicles
        .iter()
        .filter(|v| only.as_ref().is_none_or(|o| &v.id == o))
        .collect();
    anyhow::ensure!(!vehicles.is_empty(), "kein Fahrzeug ausgewählt");
    let t0 = std::time::Instant::now();
    // parallel über Fahrzeuge
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunks: Vec<Vec<&Vehicle>> = vehicles
        .chunks(vehicles.len().div_ceil(threads))
        .map(|c| c.to_vec())
        .collect();
    let mut results: Vec<(String, Outcome)> = std::thread::scope(|sc| {
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                let feel = &feel;
                let db = &db;
                sc.spawn(move || {
                    chunk
                        .into_iter()
                        .map(|v| {
                            let out = if check {
                                let cal = db.calibration.get(&v.id).copied().unwrap_or_default();
                                let keys: Vec<&str> =
                                    v.targets.keys().map(String::as_str).collect();
                                let measured = measure(v, &cal, feel, &keys);
                                Outcome {
                                    cal,
                                    measured,
                                    reasons: Vec::new(),
                                }
                            } else {
                                calibrate_one(v, feel)
                            };
                            (v.id.clone(), out)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("Kalibrier-Thread"))
            .collect()
    });
    let order: Vec<&str> = vehicles.iter().map(|v| v.id.as_str()).collect();
    results.sort_by_key(|(id, _)| order.iter().position(|o| o == id));
    // Bericht
    let mut md = String::new();
    let mut ok_all = 0;
    let mut out_of = 0;
    let mut rows = String::new();
    let mut cal_json = serde_json::Map::new();
    if only.is_some() {
        // bestehende Kalibrierungen der übrigen Fahrzeuge behalten
        for (k, c) in &db.calibration {
            cal_json.insert(k.clone(), c.to_json());
        }
    }
    for (id, o) in &results {
        let v = db.get(id).context("Fahrzeug")?;
        let mut first = true;
        let mut all_ok = true;
        for (k, t) in &v.targets {
            let a = o.measured.get(k);
            let ok = a.is_some_and(|a| (a - t).abs() <= tol(k, *t));
            if a.is_some() {
                if ok {
                    ok_all += 1;
                } else {
                    out_of += 1;
                    all_ok = false;
                }
            }
            let dev = a.map_or("–".into(), |a| match k.as_str() {
                "quer_g" => format!("{:+.2} g", a - t),
                _ => format!("{:+.1} %", (a - t) / t * 100.),
            });
            let _ = writeln!(
                rows,
                "| {} | {} | {} | {} | {} | {} | {} |",
                if first {
                    format!("**{}** ({})", v.name, v.id)
                } else {
                    String::new()
                },
                label(k),
                fmt_val(k, *t),
                a.map_or("–".into(), |a| fmt_val(k, *a)),
                dev,
                if a.is_none() {
                    "–"
                } else if ok {
                    "✓"
                } else {
                    "✗"
                },
                if first { screws(&o.cal) } else { String::new() },
            );
            first = false;
        }
        if !o.reasons.is_empty() {
            let _ = writeln!(rows, "| | *Grund:* {} | | | | | |", o.reasons.join("; "));
        }
        let mut j = o.cal.to_json();
        if let serde_json::Value::Object(m) = &mut j {
            m.insert("_ok".into(), serde_json::Value::Bool(all_ok));
            if !o.reasons.is_empty() {
                m.insert(
                    "_grund".into(),
                    serde_json::Value::String(o.reasons.join("; ")),
                );
            }
        }
        cal_json.insert(id.clone(), j);
    }
    let _ = writeln!(
        md,
        "# Kalibrierbericht Fahrphysik\n\nErzeugt von `cargo run --release -p physics-calibrate`{}. Bedingungen: trockener Asphalt, \
         `realismus = 1`, `grip_global = 1`; LKW und Busse voll beladen, sonst leer. Toleranzen: 0-X ±7 %, Vmax ±3 %, \
         Bremsweg ±5 %, Querbeschleunigung ±0,05 g. Stellschrauben nur innerhalb der erlaubten Grenzen \
         (cwA ±15 %, μ ±10 %, Übersetzung, Schaltzeit, Wirkungsgrad ±3 %, Bremskraft); Masse, Leistung und \
         Drehmoment unverändert. Verläufe im 100-ms-Takt: `docs/kalibrierung/csv/<id>.csv`.\n\n\
         **{} von {} Zielwerten in der Toleranz** ({} Fahrzeuge, Laufzeit {:.0} s).\n\n\
         | Fahrzeug | Test | Ziel | Ist | Abweichung | | Stellschrauben |\n|---|---|---|---|---|---|---|\n{rows}",
        if check { " -- --pruefen" } else { "" },
        ok_all,
        ok_all + out_of,
        results.len(),
        t0.elapsed().as_secs_f64()
    );
    let out = root.join("docs/kalibrierung");
    std::fs::create_dir_all(out.join("csv"))?;
    if !check {
        std::fs::write(out.join("bericht.md"), &md)?;
        std::fs::write(
            dir.join("vehicles.calibrated.json"),
            serde_json::to_string_pretty(&serde_json::Value::Object(cal_json))? + "\n",
        )?;
        let db2 = VehicleDb::from_dir(&dir)?;
        for v in &vehicles {
            let cv = db2.calibrated(&v.id).context("kalibriert")?;
            std::fs::write(
                out.join("csv").join(format!("{}.csv", v.id)),
                csv(&cv, &feel),
            )?;
        }
    }
    println!(
        "{} von {} Zielwerten in der Toleranz, {} Fahrzeuge, {:.0} s",
        ok_all,
        ok_all + out_of,
        results.len(),
        t0.elapsed().as_secs_f64()
    );
    if check && out_of > 0 {
        std::process::exit(1);
    }
    Ok(())
}
