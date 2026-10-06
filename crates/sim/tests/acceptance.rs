//! Fahrphysik Phase 8: die 14 Akzeptanzszenen des Auftrags als Tests. Jede Szene fährt mit den kalibrierten
//! Spieldaten und dem Spielgefühl des Spiels (`Feel::game`, Realismus 0,7) und schreibt ihre Messwerte aus
//! (`cargo test --release --test acceptance -- --nocapture`); die Ergebnisse stehen im Phasenbericht in
//! `docs/NATIVE-RUST.md`.
use berlin_sim::drift::Phase;
use berlin_sim::surface::{Material, Spot, Weather, ground, resolve};
use berlin_sim::twowheel::Fall;
use berlin_sim::vehdata::{Esp, Feel, Vehicle, shared};
use berlin_sim::vphys::{Env, Ground, HZ, Input, STEP, State, steer_limit, step};

fn car(id: &str) -> Vehicle {
    shared().calibrated(id).unwrap_or_else(|| panic!("{id}"))
}
/// Spielgefühl ohne Motorrad-Fahrhilfe: die Abnahmeszenen beschreiben die Fahrphysik (Sturz am Bordstein,
/// Wheelie-Control …); die Hilfe hat eigene Tests in `twowheel.rs`.
fn feel() -> Feel {
    Feel {
        moto_assist: false,
        ..Feel::game()
    }
}
fn kmh(s: &State) -> f64 {
    s.vx.hypot(s.vy) * 3.6
}
fn at(kmh: f64) -> State {
    State {
        vx: kmh / 3.6,
        gear: 2,
        ..Default::default()
    }
}
/// Untergrund aus den echten Daten (Belag, Wetter) für ein Fahrzeug.
fn surface(v: &Vehicle, m: Material, w: Weather) -> Env {
    let mix = resolve(
        &Spot {
            material: m,
            ..Default::default()
        },
        &w,
    );
    Env::uniform(ground(shared(), &v.tire, &mix))
}
fn wet() -> Weather {
    Weather {
        wet: 1.,
        rain: 1.,
        temp: 12.,
        ..Default::default()
    }
}
/// Lenkeingabe für einen Kreis mit Radius `r` (kinematisch).
fn steer_for(v: &Vehicle, speed: f64, r: f64) -> f64 {
    ((v.wheelbase / r).atan() / steer_limit(v, speed)).clamp(-1., 1.)
}
/// Gas, das ein Tempo hält (P-Regler).
fn hold(s: &State, kmh_want: f64) -> f64 {
    ((kmh_want - kmh(s)) * 0.08 + 0.25).clamp(0., 1.)
}
fn run(
    v: &Vehicle,
    f: &Feel,
    s: &mut State,
    env: &Env,
    secs: f64,
    mut inp: impl FnMut(&State) -> Input,
) {
    for _ in 0..(secs * HZ) as usize {
        let i = inp(s);
        step(v, f, s, &i, env, STEP);
    }
}

/// Szene 1+2: Kreuzung, 90° auf einem Kurvenradius von 18 m.
const R_CORNER: f64 = 18.;

/// Fährt die 90°-Kurve mit gehaltenem Tempo; liefert (Kurvenradius, der tatsächlich gefahren wurde, größtes
/// Untersteuern).
fn corner(id: &str, speed: f64, env: Env) -> (f64, f64) {
    let v = car(id);
    let f = feel();
    let mut s = at(speed);
    let mut under: f64 = 0.;
    // bis 90° Kurswinkel (Fahrtrichtung, nicht Fahrzeugachse) oder 6 s
    let mut t = 0.;
    while (s.yaw + s.beta()).abs() < std::f64::consts::FRAC_PI_2 && t < 6. {
        let sp = s.vx.max(1.);
        let i = Input {
            throttle: hold(&s, speed),
            steer: steer_for(&v, sp, R_CORNER),
            esp: Some(Esp::Sport),
            ..Default::default()
        };
        step(&v, &f, &mut s, &i, &env, STEP);
        // stationär (nach dem Einlenken)
        if t > 0.6 {
            under = under.max(s.understeer);
        }
        t += STEP;
    }
    // gefahrener Radius: Abstand des Endpunkts von der Kurve um (0, R)
    let r = s.x.hypot(s.y - R_CORNER).max(1.);
    let radius = if (s.yaw + s.beta()).abs() >= std::f64::consts::FRAC_PI_2 * 0.98 {
        // Viertelkreis geschafft: Radius aus dem Endpunkt
        (s.x.abs() + s.y.abs()) / 2.
    } else {
        r * 3.
    };
    (radius, under)
}

#[test]
fn scene_01_compact_takes_the_junction_at_40_and_understeers_at_70() {
    let v = car("kompakt_benzin");
    let dry = surface(&v, Material::Asphalt, Weather::default());
    let (r40, u40) = corner("kompakt_benzin", 40., dry);
    let (r70, u70) = corner("kompakt_benzin", 70., dry);
    eprintln!(
        "Szene 1: 40 km/h Radius {r40:.1} m, Untersteuern {u40:.2}; 70 km/h Radius {r70:.1} m, Untersteuern {u70:.2}"
    );
    assert!(r40 < R_CORNER * 1.25 && u40 < 0.3, "40 km/h problemlos");
    // sichtbar: Radius wächst deutlich, ein Drittel der gewünschten Gierrate fehlt
    assert!(u70 > 0.3 && r70 > R_CORNER * 1.5, "70 km/h untersteuert");
}

/// Kurve nass, am Ausgang Vollgas; liefert den größten Schwimmwinkel (°) und den am Ende.
fn wet_exit(f: &Feel, esp: Esp, countersteer: bool) -> (f64, f64) {
    let v = car("sportwagen_s");
    let env = surface(&v, Material::Asphalt, wet());
    let mut s = at(35.);
    let mut peak: f64 = 0.;
    run(&v, f, &mut s, &env, 1.2, |s| Input {
        throttle: hold(s, 35.),
        steer: steer_for(&v, s.vx.max(1.), R_CORNER),
        esp: Some(esp),
        ..Default::default()
    });
    for _ in 0..(2.5 * HZ) as usize {
        let beta = s.beta();
        let steer =
            steer_for(&v, s.vx.max(1.), R_CORNER * 2.) + if countersteer { beta * 2.5 } else { 0. };
        // wer gegenlenkt, nimmt auch kurz Gas zurück
        let throttle = if countersteer && beta.abs() > 0.15 {
            0.3
        } else {
            1.
        };
        let i = Input {
            throttle,
            steer: steer.clamp(-1., 1.),
            esp: Some(esp),
            ..Default::default()
        };
        step(&v, f, &mut s, &i, &env, STEP);
        peak = peak.max(beta.abs());
    }
    (peak.to_degrees(), s.beta().abs().to_degrees())
}

#[test]
fn scene_02_sports_car_wet_exit_full_throttle() {
    // reine Fahrphysik (ohne Drift-Schicht): ESP sport fängt das Heck, ohne ESP nur mit Gegenlenken
    let mut sim = feel();
    sim.drift_layer = false;
    let (sport, sport_end) = wet_exit(&sim, Esp::Sport, false);
    let (off, _) = wet_exit(&sim, Esp::Off, false);
    let (off_cs, off_cs_end) = wet_exit(&sim, Esp::Off, true);
    // im Spiel mit Drift-Schicht: der Gyro-Assist übernimmt das Gegenlenken
    let (game_off, game_end) = wet_exit(&feel(), Esp::Off, false);
    eprintln!(
        "Szene 2: ESP sport {sport:.0}° (Ende {sport_end:.0}°); ohne ESP {off:.0}°, mit Gegenlenken {off_cs:.0}° (Ende \
         {off_cs_end:.0}°); Spiel ohne ESP (Drift-Assist) {game_off:.0}° (Ende {game_end:.0}°)"
    );
    assert!(sport > 2., "Heck kommt auch mit ESP: {sport}");
    // ESP sport lässt bis etwa 13° Schwimmwinkel zu (sportlich) und hält das Auto dort
    assert!(
        sport < 20. && sport_end <= sport + 0.5,
        "ESP sport fängt es"
    );
    assert!(off > 35., "ohne ESP ohne Gegenlenken dreht es sich: {off}");
    assert!(
        off_cs < off * 0.6 && off_cs_end < 15.,
        "Gegenlenken fängt es"
    );
    assert!(game_off < 80., "im Spiel hält der Assist das Auto");
}

#[test]
fn scene_03_turbo_s_and_small_car_after_three_seconds() {
    let f = feel();
    let at3 = |id: &str| {
        let v = car(id);
        let mut s = State {
            boost: 1.,
            ..Default::default()
        };
        run(&v, &f, &mut s, &Env::default(), 3., |_| Input {
            throttle: 1.,
            esp: Some(Esp::Sport),
            ..Default::default()
        });
        kmh(&s)
    };
    let (turbo, small) = (at3("turbo_s"), at3("kleinwagen_65ps"));
    eprintln!("Szene 3: nach 3 s Turbo S {turbo:.0} km/h, Kleinwagen {small:.0} km/h");
    assert!(turbo > 100. && small < 45. && turbo > 2.5 * small);
}

/// Vollbremsung aus 80 km/h mit leichtem Lenken; liefert (Bremsweg, Kursänderung in °, Räder blockiert).
fn brake_from_80(id: &str, env: &Env) -> (f64, f64, bool) {
    let v = car(id);
    let f = feel();
    let mut s = at(80.);
    let mut locked = false;
    let mut t = 0.;
    while s.vx > 0.05 && t < 10. {
        let i = Input {
            brake: 1.,
            steer: 0.5,
            esp: Some(Esp::Sport),
            ..Default::default()
        };
        step(&v, &f, &mut s, &i, env, STEP);
        locked |= s.locked[0] && s.locked[1];
        t += STEP;
    }
    (s.dist, (s.yaw + s.beta()).to_degrees().abs(), locked)
}

#[test]
fn scene_04_oldtimer_full_brake_on_wet_cobbles() {
    let old = car("oldtimer_w123");
    let env = surface(&old, Material::Cobble, wet());
    let (d_old, turn_old, locked) = brake_from_80("oldtimer_w123", &env);
    let modern = car("kompakt_benzin");
    let (d_new, turn_new, _) =
        brake_from_80("kompakt_benzin", &surface(&modern, Material::Cobble, wet()));
    eprintln!(
        "Szene 4: Oldtimer {d_old:.1} m, Kurs {turn_old:.0}°, blockiert {locked}; Kompaktwagen (ABS) {d_new:.1} m, \
         Kurs {turn_new:.0}°"
    );
    assert!(locked, "Räder blockieren");
    assert!(
        turn_old < turn_new * 0.5,
        "blockiert lässt es sich kaum lenken"
    );
    assert!(d_old > d_new * 1.25, "rutscht deutlich weiter");
}

/// Gierantwort auf einen kleinen Lenkimpuls bei 110 km/h, trocken bzw. in der Pfütze (beide Spurrinnen voll Wasser).
fn steer_response(water: f64) -> (f64, f64, f64) {
    let v = car("kompakt_benzin");
    let f = feel();
    let mut s = at(110.);
    s.gear = 4;
    let mut env = Env::default();
    for w in &mut env.wheel {
        w.water_mm = water;
    }
    let mut aqua: f64 = 0.;
    run(&v, &f, &mut s, &env, 0.5, |s| {
        aqua = aqua.max(s.aqua[0]);
        Input {
            throttle: hold(s, 110.),
            steer: 0.04,
            esp: Some(Esp::Sport),
            ..Default::default()
        }
    });
    let r = s.r;
    // danach wieder trocken: fängt sich
    run(&v, &f, &mut s, &Env::default(), 2., |s| Input {
        throttle: hold(s, 110.),
        esp: Some(Esp::Sport),
        ..Default::default()
    });
    (r, aqua, s.beta().abs().to_degrees())
}

#[test]
fn scene_05_car_through_a_deep_rut_puddle_at_110() {
    let (dry, _, _) = steer_response(0.);
    let (wet, aqua, after) = steer_response(12.);
    eprintln!(
        "Szene 5: Gierantwort trocken {dry:.3} rad/s, in der Pfütze {wet:.3} rad/s (Aquaplaning {aqua:.2}), Schwimmwinkel \
         danach {after:.1}°"
    );
    assert!(
        aqua > 0.5 && wet < dry * 0.6,
        "sichtbarer Verlust der Lenkung"
    );
    assert!(wet > dry * 0.05, "kein Totalausfall");
    assert!(after < 3., "fängt sich");
}

#[test]
fn scene_06_escooter_wet_tram_rails() {
    let v = car("escooter_entdrosselt");
    let f = feel();
    let cross = |deg: f64| {
        let risk = berlin_sim::world::groove_risk(0., deg.to_radians(), 1.);
        let mut s = at(18.);
        let mut env = Env::default();
        env.wheel[0].groove = risk;
        run(&v, &f, &mut s, &env, 0.2, |_| Input {
            throttle: 0.3,
            ..Default::default()
        });
        (risk, s.fallen)
    };
    let (flat, fell) = cross(10.);
    let (steep, ok) = cross(60.);
    eprintln!("Szene 6: 10° nass Risiko {flat:.2} → {fell:?}; 60° Risiko {steep:.2} → {ok:?}");
    assert_eq!(fell, Some(Fall::Rail));
    assert_eq!(ok, None);
}

/// Sattelzug beladen in den Kreisverkehr (Radius 15 m); liefert (umgekippt, größter Knickwinkel °).
fn roundabout(speed: f64) -> (bool, f64) {
    let v = car("sattelzug_40t");
    let f = feel();
    let mut s = at(speed);
    s.load = 1.;
    s.gear = 6;
    let mut art: f64 = 0.;
    run(&v, &f, &mut s, &Env::default(), 6., |s| {
        art = art.max(s.art.abs());
        Input {
            throttle: hold(s, speed),
            steer: steer_for(&v, s.vx.max(1.), 15.),
            esp: Some(Esp::Off),
            ..Default::default()
        }
    });
    (s.rolled, art.to_degrees())
}

#[test]
fn scene_07_loaded_semi_in_the_roundabout() {
    let (tip50, _) = roundabout(50.);
    let (tip25, art25) = roundabout(25.);
    eprintln!("Szene 7: 50 km/h gekippt {tip50}; 25 km/h gekippt {tip25}, Knickwinkel {art25:.0}°");
    assert!(tip50, "kippt bei 50");
    assert!(
        !tip25 && art25 > 15.,
        "25 geht, Auflieger schneidet die Kurve"
    );
}

#[test]
fn scene_08_double_decker_full_brake() {
    let f = feel();
    let stop = |id: &str| {
        let v = car(id);
        let mut s = at(50.);
        let mut t = 0.;
        while s.vx > 0.05 && t < 10. {
            let i = Input {
                brake: 1.,
                ..Default::default()
            };
            step(&v, &f, &mut s, &i, &Env::default(), STEP);
            t += STEP;
        }
        (s.dist, s.passenger_falls)
    };
    let (bus, falls) = stop("doppeldecker");
    let (pkw, _) = stop("kompakt_benzin");
    eprintln!("Szene 8: Doppeldecker {bus:.1} m ({falls} Fahrgast-Ereignis), Pkw {pkw:.1} m");
    assert!(bus > pkw * 1.5 && falls >= 1);
}

/// Handbremse kurz, Gas halten, leicht lenken, dann Gas weg; liefert (Driftlänge m, Streuung des Winkels im
/// gehaltenen Drift °, Zeit bis zur Ausleitung nach Gas weg s, längste Driftphase s).
fn drift(id: &str) -> (f64, f64, f64, f64) {
    let v = car(id);
    let f = feel();
    let mut s = at(55.);
    run(&v, &f, &mut s, &Env::default(), 0.3, |_| Input {
        handbrake: true,
        steer: 0.7,
        throttle: 0.3,
        esp: Some(Esp::Sport),
        ..Default::default()
    });
    let (mut len, mut longest, mut cur, mut angles) = (0., 0f64, 0., Vec::new());
    let start = s.dist;
    let mut entered = None;
    for _ in 0..(3. * HZ) as usize {
        let i = Input {
            throttle: 0.6,
            steer: 0.2,
            esp: Some(Esp::Sport),
            ..Default::default()
        };
        step(&v, &f, &mut s, &i, &Env::default(), STEP);
        if s.drift.phase == Phase::Drift {
            entered.get_or_insert(s.dist);
            cur += STEP;
            longest = longest.max(cur);
            len = s.dist - entered.unwrap();
            if cur > 0.8 {
                angles.push(s.beta().abs().to_degrees());
            }
        } else {
            cur = 0.;
        }
    }
    let _ = start;
    let mean = angles.iter().sum::<f64>() / angles.len().max(1) as f64;
    let spread = angles.iter().map(|a| (a - mean).abs()).fold(0., f64::max);
    // Gas weg: Ausleitung
    let mut t_exit = 0.;
    while matches!(s.drift.phase, Phase::Drift | Phase::Entry) && t_exit < 4. {
        let i = Input {
            steer: 0.,
            esp: Some(Esp::Sport),
            ..Default::default()
        };
        step(&v, &f, &mut s, &i, &Env::default(), STEP);
        t_exit += STEP;
    }
    (len, spread, t_exit, longest)
}

#[test]
fn scene_09_drift_coupe_handbrake_drift_at_assist_level_2() {
    assert_eq!(feel().drift_assist, 2);
    let (len, spread, exit, held) = drift("drift_coupe");
    eprintln!(
        "Szene 9: Drift-Coupé {len:.0} m im Drift ({held:.1} s), Winkel schwankt ±{spread:.1}°, Ausleitung nach Gas weg \
         {exit:.1} s"
    );
    assert!(len > 25., "über eine ganze Kreuzung");
    assert!(spread < 8., "Winkel stabil");
    assert!(exit < 1.5, "Gas weg leitet aus");
}

#[test]
fn scene_10_front_wheel_drive_only_swings_briefly() {
    let (_, _, _, held) = drift("kompakt_benzin");
    eprintln!("Szene 10: Kompaktwagen (Frontantrieb) längste Driftphase {held:.2} s");
    assert!(held < 1.0, "kein gehaltener Drift");
}

/// Auf Neuschnee: (0–30 km/h mit Feingefühl = 30 % Gas, Kreis 40 m bei 20 km/h gehalten, Bremsweg aus 50 km/h,
/// 0–50 km/h mit Vollgas). Zeiten in s, Wege in m; 99 = nicht geschafft.
fn on_snow(v: &Vehicle) -> (f64, bool, f64, f64) {
    let f = feel();
    let snow = Weather {
        snow: 1.,
        temp: -3.,
        ..Default::default()
    };
    let env = surface(v, Material::Asphalt, snow);
    let until = |throttle: f64, kmh_to: f64| {
        let mut s = State::default();
        let mut t = 0.;
        while kmh(&s) < kmh_to && t < 99. {
            let i = Input {
                throttle,
                esp: Some(Esp::Sport),
                ..Default::default()
            };
            step(v, &f, &mut s, &i, &env, STEP);
            t += STEP;
        }
        t
    };
    let gentle = until(0.3, 30.);
    let full = until(1., 50.);
    // Kreis 40 m mit 20 km/h: gefahrener Radius aus Tempo und Gierrate
    let mut s = at(20.);
    run(v, &f, &mut s, &env, 8., |s| Input {
        throttle: hold(s, 20.),
        steer: steer_for(v, s.vx.max(1.), 40.),
        esp: Some(Esp::Sport),
        ..Default::default()
    });
    let held = (s.vx / s.r.abs().max(1e-3)) < 50. && s.beta().abs() < 0.15;
    let mut s = at(50.);
    let mut t = 0.;
    while s.vx > 0.05 && t < 99. {
        let i = Input {
            brake: 1.,
            esp: Some(Esp::Sport),
            ..Default::default()
        };
        step(v, &f, &mut s, &i, &env, STEP);
        t += STEP;
    }
    (gentle, held, s.dist, full)
}

#[test]
fn scene_11_semi_slicks_on_fresh_snow_versus_winter_tyres() {
    let slick = car("supercar_rwd");
    assert_eq!(slick.tire.id, "semi_slick");
    // Winterreifen-Mod (Tuning gibt es nicht; das Spiel tauscht im Winter nur normale Sommerreifen)
    let mut winter = slick.clone();
    winter.tire = shared().tires["winter"].clone();
    let (g_s, held_s, b_s, a_s) = on_snow(&slick);
    let (g_w, held_w, b_w, a_w) = on_snow(&winter);
    eprintln!(
        "Szene 11: Neuschnee Semi-Slicks: 0–30 mit Gefühl {g_s:.1} s, Kreis gehalten {held_s}, Bremsweg 50 km/h \
         {b_s:.0} m, 0–50 Vollgas {a_s:.1} s; Winterreifen: {g_w:.1} s, {held_w}, {b_w:.0} m, {a_w:.1} s"
    );
    // kaum fahrbar, aber mit Feingefühl möglich: wenig Gas kommt schneller voran als Vollgas
    assert!(g_s < 60. && g_s < a_s && held_s, "mit Feingefühl möglich");
    assert!(
        held_w && b_w < b_s * 0.6 && a_w < a_s * 0.7,
        "Winterreifen fahrbar"
    );
}

#[test]
fn scene_12_bicycle_and_suv_against_a_curb() {
    let f = feel();
    let mut curb = Env::default();
    for w in &mut curb.wheel {
        w.curb = 0.12;
    }
    let bike = car("fahrrad_city");
    let mut s = at(25.);
    run(&bike, &f, &mut s, &curb, 0.02, |_| Input::default());
    // Geländewagen: der Bordstein liegt nur im Moment der Überfahrt unter den Rädern
    let suv = car("gelaendewagen");
    let mut q = at(25.);
    run(&suv, &f, &mut q, &curb, 0.02, |_| Input::default());
    run(&suv, &f, &mut q, &Env::default(), 0.5, |s| Input {
        throttle: hold(s, 25.),
        ..Default::default()
    });
    eprintln!(
        "Szene 12: Fahrrad mit 25 km/h → {:?}; Geländewagen danach {:.0} km/h",
        s.fallen,
        kmh(&q)
    );
    assert_eq!(s.fallen, Some(Fall::Curb));
    assert!(!q.rolled && kmh(&q) > 20., "fährt drüber");
}

#[test]
fn scene_13_superbike_wheelie_at_the_lights() {
    let v = car("superbike");
    let f = feel();
    let mut s = State::default();
    let mut peak: f64 = 0.;
    let mut lifted = 0.;
    run(&v, &f, &mut s, &Env::default(), 3., |s| {
        peak = peak.max(s.pitch);
        if s.pitch > 0.02 {
            lifted += STEP;
        }
        Input {
            throttle: 1.,
            ..Default::default()
        }
    });
    eprintln!(
        "Szene 13: Wheelie bis {:.1}°, {lifted:.1} s in der Luft, gestürzt {:?}",
        peak.to_degrees(),
        s.fallen
    );
    assert!(s.fallen.is_none());
    assert!(peak > 0.05 && peak <= berlin_sim::twowheel::WHEELIE_CONTROL + 1e-9);
    assert!(lifted > 0.1 && lifted < 2.5, "kurz");
}

#[test]
fn scene_14_hypercar_at_350_steers_calmly() {
    let v = car("hypercar");
    let f = feel();
    let mut s = at(350.);
    s.gear = 6;
    let mut rs = Vec::new();
    run(&v, &f, &mut s, &Env::default(), 3., |s| {
        rs.push(s.r);
        Input {
            throttle: 0.8,
            steer: 0.1,
            esp: Some(Esp::Sport),
            ..Default::default()
        }
    });
    // ruhig: die Gierrate wechselt das Vorzeichen nicht und schwingt nicht
    let flips = rs.windows(2).filter(|w| w[0] * w[1] < 0.).count();
    let late = &rs[rs.len() / 2..];
    let (lo, hi) = late
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), &r| (a.min(r), b.max(r)));
    let zoom = berlin_sim::world::camera_zoom_for(350. / 3.6 * 10., true);
    eprintln!(
        "Szene 14: 350 km/h, Gierrate {:.3}…{:.3} rad/s, Vorzeichenwechsel {flips}, Kamera-Zoom {zoom:.2}",
        lo, hi
    );
    assert!(flips <= 1 && hi - lo < 0.02, "kein Zittern");
    assert!(s.beta().abs() < 0.05);
    assert!(zoom < 0.6, "Kamera zoomt heraus");
}

#[test]
fn ground_from_data_is_used() {
    // Gegenprobe der Hilfsfunktion: nasser Kopfstein greift schlechter als trockener Asphalt
    let v = car("kompakt_benzin");
    let w = surface(&v, Material::Cobble, wet()).wheel[0];
    let d = surface(&v, Material::Asphalt, Weather::default()).wheel[0];
    assert!(w.grip() < d.grip() * 0.8 && d.grip() > 0.95);
    assert!(Ground::DRY.grip() > 0.95);
}
