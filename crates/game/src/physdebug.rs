//! Entwickler-Anzeige der Fahrphysik (F3, nur im Entwickler-Build): Messwerte des gefahrenen Fahrzeugs und
//! Live-Regler für alle Werte aus `feel.json` sowie die wichtigsten Daten des Fahrzeugs. Geänderte Werte gibt F6 als
//! JSON-Abweichung in der Konsole aus (zum Übertragen in die Datendateien).
//!
//! Die Regler schreiben das Spielgefühl über `vehdata::set_game_feel` und das Fahrzeug als `Car::tuned` – die
//! Spieldaten selbst bleiben unverändert, Aussteigen oder ein neues Fahrzeug fängt wieder bei den Daten an.
use berlin_engine::hud::{Align, Hud};
use berlin_sim::vehdata::{Feel, Vehicle, game_feel, set_game_feel};
use berlin_sim::world::World;

/// Ein Regler: Name (wie in den JSON-Dateien), Schritt, Grenzen, Lesen und Schreiben.
pub struct Slider<T> {
    pub key: &'static str,
    pub step: f64,
    pub min: f64,
    pub max: f64,
    pub get: fn(&T) -> f64,
    pub set: fn(&mut T, f64),
}

const fn s<T>(
    key: &'static str,
    step: f64,
    min: f64,
    max: f64,
    get: fn(&T) -> f64,
    set: fn(&mut T, f64),
) -> Slider<T> {
    Slider {
        key,
        step,
        min,
        max,
        get,
        set,
    }
}
fn b(v: bool) -> f64 {
    if v { 1. } else { 0. }
}

/// Alle Werte aus `feel.json` (Schalter als 0/1).
pub const FEEL: &[Slider<Feel>] = &[
    s(
        "realismus",
        0.05,
        0.,
        1.,
        |f| f.realism,
        |f, v| f.realism = v,
    ),
    s(
        "grip_global",
        0.05,
        0.5,
        1.5,
        |f| f.grip_global,
        |f, v| f.grip_global = v,
    ),
    s(
        "bremse_global",
        0.05,
        0.5,
        1.5,
        |f| f.brake_global,
        |f, v| f.brake_global = v,
    ),
    s(
        "lenk_assist",
        0.05,
        0.,
        1.,
        |f| f.steer_assist,
        |f, v| f.steer_assist = v,
    ),
    s(
        "drift_assist_stufe",
        1.,
        0.,
        2.,
        |f| f.drift_assist as f64,
        |f, v| f.drift_assist = v as u8,
    ),
    s(
        "aquaplaning_staerke",
        0.05,
        0.,
        1.5,
        |f| f.aquaplaning,
        |f, v| f.aquaplaning = v,
    ),
    s(
        "eis_grip_minimum",
        0.01,
        0.,
        0.3,
        |f| f.ice_grip_min,
        |f, v| f.ice_grip_min = v,
    ),
    s(
        "gewichtsverlagerung_skala",
        0.05,
        0.,
        2.,
        |f| f.weight_transfer,
        |f, v| f.weight_transfer = v,
    ),
    s(
        "kamera_zoom_nach_tempo",
        1.,
        0.,
        1.,
        |f| b(f.camera_zoom_by_speed),
        |f, v| f.camera_zoom_by_speed = v > 0.5,
    ),
    s(
        "lkw_begrenzer_tunebar",
        1.,
        0.,
        1.,
        |f| b(f.truck_limiter_tunable),
        |f, v| f.truck_limiter_tunable = v > 0.5,
    ),
    s(
        "wandkontakt_gleiten",
        1.,
        0.,
        1.,
        |f| b(f.wall_slide),
        |f, v| f.wall_slide = v > 0.5,
    ),
    s(
        "anfahr_zuschlag",
        0.1,
        0.,
        2.,
        |f| f.launch_boost,
        |f, v| f.launch_boost = v,
    ),
    s(
        "anfahr_bis_kmh",
        5.,
        5.,
        150.,
        |f| f.launch_until * 3.6,
        |f, v| f.launch_until = v / 3.6,
    ),
];

/// Daten des gefahrenen Fahrzeugs (Schlüssel wie in `vehicles.json`).
pub const VEHICLE: &[Slider<Vehicle>] = &[
    s(
        "masse",
        25.,
        50.,
        50000.,
        |v| v.mass_empty,
        |v, x| v.mass_empty = x,
    ),
    s("cg_h", 0.02, 0.2, 3., |v| v.cg_empty, |v, x| v.cg_empty = x),
    s("vorn", 0.01, 0.2, 0.8, |v| v.front, |v, x| v.front = x),
    s(
        "motor.kw",
        5.,
        0.1,
        1500.,
        |v| v.engine.watts / 1000.,
        |v, x| v.engine.watts = x * 1000.,
    ),
    s(
        "motor.nm",
        10.,
        1.,
        3000.,
        |v| v.engine.nm,
        |v, x| v.engine.nm = x,
    ),
    s(
        "reifen.mu",
        0.02,
        0.2,
        2.,
        |v| v.tire.mu,
        |v, x| v.tire.mu = x,
    ),
    s("aero.cwA", 0.02, 0.1, 12., |v| v.cw_a, |v, x| v.cw_a = x),
    s(
        "bremse.kraft",
        0.02,
        0.2,
        2.,
        |v| v.brake.gain,
        |v, x| v.brake.gain = x,
    ),
    s(
        "bremse.vorn",
        0.02,
        0.3,
        0.9,
        |v| v.brake.front,
        |v, x| v.brake.front = x,
    ),
    s(
        "fahrwerk.tau",
        0.01,
        0.02,
        0.6,
        |v| v.chassis.tau,
        |v, x| v.chassis.tau = x,
    ),
    s(
        "fahrwerk.wank",
        0.25,
        0.,
        12.,
        |v| v.chassis.roll,
        |v, x| v.chassis.roll = x,
    ),
    s(
        "drift.faehigkeit",
        0.05,
        0.,
        1.,
        |v| v.drift.ability,
        |v, x| v.drift.ability = x,
    ),
    s(
        "drift.max_winkel",
        1.,
        0.,
        90.,
        |v| v.drift.max_angle.to_degrees(),
        |v, x| v.drift.max_angle = x.to_radians(),
    ),
    s(
        "drift.grip_hinten",
        0.02,
        0.3,
        1.,
        |v| v.drift.rear_grip,
        |v, x| v.drift.rear_grip = x,
    ),
];

/// Zustand der Anzeige.
pub struct PhysDebug {
    pub open: bool,
    /// gewählter Regler (zuerst die aus `feel.json`, dann die Fahrzeugdaten)
    pub sel: usize,
    /// Ausgangswerte für die Abweichung
    feel0: Feel,
}

impl Default for PhysDebug {
    fn default() -> Self {
        Self {
            open: false,
            sel: 0,
            feel0: game_feel().clone(),
        }
    }
}

fn fmt(x: f64, step: f64) -> String {
    if step >= 1. {
        format!("{x:.0}")
    } else if step >= 0.05 {
        format!("{x:.2}")
    } else {
        format!("{x:.3}")
    }
}

/// Ausgangsdatensatz des gefahrenen Fahrzeugs (Spieldaten, kalibriert).
fn base_vehicle(w: &World) -> Option<&'static Vehicle> {
    w.player_car().and_then(berlin_sim::car::vphys_vehicle)
}

impl PhysDebug {
    pub fn count(w: &World) -> usize {
        FEEL.len()
            + if base_vehicle(w).is_some() {
                VEHICLE.len()
            } else {
                0
            }
    }

    pub fn select(&mut self, w: &World, d: i32) {
        let n = Self::count(w) as i32;
        self.sel = (self.sel as i32 + d).rem_euclid(n.max(1)) as usize;
    }

    /// Gewählten Regler um `d` Schritte verstellen.
    pub fn adjust(&mut self, w: &mut World, d: f64) {
        if self.sel < FEEL.len() {
            let sl = &FEEL[self.sel];
            let mut f = game_feel().clone();
            let x = ((sl.get)(&f) + d * sl.step).clamp(sl.min, sl.max);
            (sl.set)(&mut f, x);
            set_game_feel(f);
            return;
        }
        let Some(base) = base_vehicle(w) else { return };
        let Some(id) = w.player.in_car else { return };
        let Some(c) = w.cars.iter_mut().find(|c| c.id == id) else {
            return;
        };
        let sl = &VEHICLE[self.sel - FEEL.len()];
        let v = c.tuned.get_or_insert_with(|| Box::new(base.clone()));
        let x = ((sl.get)(v) + d * sl.step).clamp(sl.min, sl.max);
        (sl.set)(v, x);
        // Gierträgheit folgt der Masse wie beim Laden der Daten
        let (a, bb) = v.axle_distances();
        v.iz = v.mass_empty * a * bb * 1.05;
    }

    /// Geänderte Werte als JSON-Abweichung (`{"feel": {...}, "fahrzeug": {"id": ..., ...}}`).
    pub fn export(&self, w: &World) -> String {
        diff_json(
            &self.feel0,
            game_feel(),
            base_vehicle(w),
            w.player_car().and_then(|c| c.tuned.as_deref()),
        )
    }

    pub fn draw(&self, h: &mut Hud, w: &World) {
        if !self.open {
            return;
        }
        let (x0, mut y) = (16f32, 150f32);
        let wd = 400f32;
        let rows = 25 + Self::count(w);
        h.rect(
            x0 - 8.,
            y - 18.,
            wd,
            rows as f32 * 15. + 30.,
            [0., 0., 0., 0.62],
            6.,
        );
        let white = [1., 1., 1., 1.];
        let grey = [0.7, 0.72, 0.76, 1.];
        line(
            h,
            &mut y,
            x0,
            "FAHRPHYSIK  Bild auf/ab, Komma/Punkt, F6 ausgeben",
            grey,
        );
        let car = w.player_car();
        let s = car.and_then(|c| c.phys.as_deref());
        if let (Some(c), Some(s)) = (car, s) {
            line(
                h,
                &mut y,
                x0,
                &format!(
                    "{:.0} km/h  Gang {}  {:.0} U/min  β {:.1}°",
                    c.speed() * 0.36,
                    s.gear + 1,
                    s.rpm,
                    s.beta().to_degrees()
                ),
                white,
            );
            line(
                h,
                &mut y,
                x0,
                &format!(
                    "Schräglauf vorn {:.1}° hinten {:.1}°  Schlupf {:.2}/{:.2}",
                    s.alpha[0].to_degrees(),
                    s.alpha[1].to_degrees(),
                    s.slip[0],
                    s.slip[1]
                ),
                white,
            );
            // Eingriffe als Lämpchen: leuchtet gelb, solange das System eingreift
            for (k, (name, on)) in [("ABS", s.abs), ("ESP", s.esp_active), ("TCS", s.tcs)]
                .into_iter()
                .enumerate()
            {
                let x = x0 + k as f32 * 46.;
                h.rect(
                    x - 2.,
                    y - 10.,
                    40.,
                    13.,
                    if on {
                        [1., 0.78, 0.2, 0.95]
                    } else {
                        [1., 1., 1., 0.12]
                    },
                    3.,
                );
                h.text(
                    name,
                    x + 18.,
                    y,
                    11.,
                    if on { [0.1, 0.1, 0.1, 1.] } else { grey },
                    Align::Center,
                    false,
                );
            }
            h.text(
                &format!("Aquaplaning {:.0} %", s.aqua[0].max(s.aqua[1]) * 100.),
                x0 + 146.,
                y,
                12.,
                white,
                Align::Left,
                false,
            );
            y += 15.;
            line(
                h,
                &mut y,
                x0,
                &format!(
                    "Drift {:?}  Ziel {:.0}°  Haftung hinten {:.2}  Punkte {:.0}",
                    s.drift.phase,
                    s.drift.target.to_degrees(),
                    s.drift.grip,
                    s.drift.score.current
                ),
                white,
            );
            // Radlasten als Balken, μ_eff mit Untergrund je Rad
            let total: f64 = s.fz.iter().sum::<f64>().max(1.);
            let env = c.env_seen.as_deref();
            let feel = game_feel();
            let mu_tire = car
                .and_then(|c| c.tuned.as_deref().or_else(|| base_vehicle(w)))
                .map_or(1., |v| v.tire.mu);
            for (k, name) in ["VL", "VR", "HL", "HR"].iter().enumerate() {
                let share = (s.fz[k] / total) as f32;
                h.rect(x0 + 26., y - 9., 80., 9., [1., 1., 1., 0.15], 0.);
                h.rect(
                    x0 + 26.,
                    y - 9.,
                    80. * (share * 2.).min(1.),
                    9.,
                    [0.4, 0.8, 1., 0.9],
                    0.,
                );
                let (mu, surf) = env.map_or((0., "?"), |e| {
                    (mu_tire * e.wheel[k].grip() * feel.grip(), e.surface[k])
                });
                h.text(name, x0, y, 12., grey, Align::Left, false);
                h.text(
                    &format!("{:.0} N  μ {mu:.2}  {surf}", s.fz[k]),
                    x0 + 114.,
                    y,
                    12.,
                    white,
                    Align::Left,
                    false,
                );
                y += 15.;
            }
        } else {
            line(h, &mut y, x0, "(kein Fahrzeug mit Fahrphysik)", grey);
        }
        y += 6.;
        let feel = game_feel();
        for (i, sl) in FEEL.iter().enumerate() {
            let sel = i == self.sel;
            let changed = ((sl.get)(feel) - (sl.get)(&self.feel0)).abs() > 1e-9;
            line(
                h,
                &mut y,
                x0,
                &format!(
                    "{}{}\t{}",
                    if sel { "> " } else { "  " },
                    sl.key,
                    fmt((sl.get)(feel), sl.step)
                ),
                if sel {
                    [1., 0.85, 0.3, 1.]
                } else if changed {
                    [0.6, 0.95, 0.6, 1.]
                } else {
                    white
                },
            );
        }
        if let Some(base) = base_vehicle(w) {
            let tuned = w.player_car().and_then(|c| c.tuned.as_deref());
            line(h, &mut y, x0, &format!("Fahrzeug {}", base.id), grey);
            for (i, sl) in VEHICLE.iter().enumerate() {
                let sel = FEEL.len() + i == self.sel;
                let v = tuned.unwrap_or(base);
                let changed = ((sl.get)(v) - (sl.get)(base)).abs() > 1e-9;
                line(
                    h,
                    &mut y,
                    x0,
                    &format!(
                        "{}{}\t{}",
                        if sel { "> " } else { "  " },
                        sl.key,
                        fmt((sl.get)(v), sl.step)
                    ),
                    if sel {
                        [1., 0.85, 0.3, 1.]
                    } else if changed {
                        [0.6, 0.95, 0.6, 1.]
                    } else {
                        white
                    },
                );
            }
        }
    }
}

/// Abweichung der Regler von den Ausgangswerten als JSON; nur geänderte Werte, gerundet auf den Reglerschritt.
pub fn diff_json(
    feel0: &Feel,
    now: &Feel,
    base: Option<&Vehicle>,
    tuned: Option<&Vehicle>,
) -> String {
    let mut out = serde_json::Map::new();
    let mut feel = serde_json::Map::new();
    for sl in FEEL {
        let (a, b) = ((sl.get)(feel0), (sl.get)(now));
        if (a - b).abs() > 1e-9 {
            feel.insert(sl.key.into(), json_num(b, sl.step));
        }
    }
    if !feel.is_empty() {
        out.insert("feel".into(), feel.into());
    }
    if let (Some(base), Some(tuned)) = (base, tuned) {
        let mut veh = serde_json::Map::new();
        for sl in VEHICLE {
            let (a, b) = ((sl.get)(base), (sl.get)(tuned));
            if (a - b).abs() > 1e-9 {
                veh.insert(sl.key.into(), json_num(b, sl.step));
            }
        }
        if !veh.is_empty() {
            veh.insert("id".into(), base.id.clone().into());
            out.insert("fahrzeug".into(), veh.into());
        }
    }
    serde_json::Value::Object(out).to_string()
}

/// Eine Zeile; ein Tabulator setzt den Rest an eine feste Spalte (die Schrift ist proportional).
fn line(h: &mut Hud, y: &mut f32, x0: f32, t: &str, c: [f32; 4]) {
    match t.split_once('\t') {
        Some((a, b)) => {
            h.text(a, x0, *y, 12., c, Align::Left, false);
            h.text(b, x0 + 296., *y, 12., c, Align::Left, false);
        }
        None => {
            h.text(t, x0, *y, 12., c, Align::Left, false);
        }
    }
    *y += 15.;
}

fn json_num(x: f64, step: f64) -> serde_json::Value {
    let digits = if step >= 1. {
        0
    } else if step >= 0.05 {
        2
    } else {
        3
    };
    let r = (x * 10f64.powi(digits)).round() / 10f64.powi(digits);
    serde_json::json!(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_feel_key_exists_in_feel_json() {
        let j: serde_json::Value =
            serde_json::from_str(include_str!("../../../data/vehicles/feel.json")).unwrap();
        for sl in FEEL {
            assert!(j.get(sl.key).is_some(), "{} fehlt in feel.json", sl.key);
        }
        assert_eq!(
            FEEL.len(),
            j.as_object().unwrap().len() - 1,
            "esp_spieler_default ist kein Regler"
        );
    }

    #[test]
    fn export_lists_only_changed_values() {
        let f0 = Feel::game();
        let mut f1 = f0.clone();
        f1.realism = 0.4;
        f1.wall_slide = false;
        let base = berlin_sim::vehdata::shared().get("kompakt_benzin").unwrap();
        let mut t = base.clone();
        t.cw_a += 0.1;
        let j: serde_json::Value =
            serde_json::from_str(&diff_json(&f0, &f1, Some(base), Some(&t))).unwrap();
        assert_eq!(j["feel"]["realismus"], 0.4);
        assert_eq!(j["feel"]["wandkontakt_gleiten"], 0.);
        assert_eq!(j["feel"].as_object().unwrap().len(), 2);
        assert_eq!(j["fahrzeug"]["id"], "kompakt_benzin");
        assert_eq!(j["fahrzeug"].as_object().unwrap().len(), 2, "{j}");
        assert_eq!(diff_json(&f0, &f0, Some(base), Some(base)), "{}");
    }

    #[test]
    fn sliders_round_trip_their_values() {
        let mut f = Feel::game();
        for sl in FEEL {
            let x = ((sl.get)(&f) + sl.step).clamp(sl.min, sl.max);
            (sl.set)(&mut f, x);
            assert!(((sl.get)(&f) - x).abs() < 1e-9, "{}", sl.key);
        }
        let mut v = berlin_sim::vehdata::shared()
            .get("kompakt_benzin")
            .unwrap()
            .clone();
        for sl in VEHICLE {
            let x = ((sl.get)(&v) + sl.step).clamp(sl.min, sl.max);
            (sl.set)(&mut v, x);
            assert!(((sl.get)(&v) - x).abs() < 1e-6, "{}", sl.key);
        }
    }
}
