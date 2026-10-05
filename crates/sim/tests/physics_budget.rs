//! Fahrphysik Phase 8: Leistungsbudget und Determinismus.
//!
//! Budget laut Auftrag: Physik für den Spieler plus 50 voll simulierte Fahrzeuge unter 2 ms je Bild. Gemessen
//! wird nur im Release-Build (`cargo test --release`); im Debug-Build läuft der Test zur Kontrolle der Logik mit.
use berlin_sim::vehdata::{Esp, Feel, VehicleDb};
use berlin_sim::vphys::{Env, Input, State, step};

fn fleet(db: &VehicleDb) -> Vec<(&berlin_sim::vehdata::Vehicle, State, Input)> {
    let ids: Vec<&str> = db
        .vehicles
        .iter()
        .filter(|v| !v.two_wheel)
        .map(|v| v.id.as_str())
        .collect();
    (0..51)
        .map(|i| {
            let v = db.get(ids[i % ids.len()]).unwrap();
            let s = State {
                vx: 8. + (i % 7) as f64 * 3.,
                gear: 2,
                ..Default::default()
            };
            let inp = Input {
                throttle: 0.4 + (i % 3) as f64 * 0.2,
                steer: ((i % 5) as f64 - 2.) * 0.15,
                esp: Some(Esp::Sport),
                ..Default::default()
            };
            (v, s, inp)
        })
        .collect()
}

#[test]
fn player_plus_fifty_cars_fit_the_frame_budget() {
    let db = VehicleDb::embedded().unwrap();
    let feel = Feel::game();
    let mut cars = fleet(&db);
    let env = Env::default();
    let frames = 600;
    let t = std::time::Instant::now();
    for _ in 0..frames {
        for (v, s, inp) in cars.iter_mut() {
            step(v, &feel, s, inp, &env, 1. / 60.);
        }
    }
    let ms = t.elapsed().as_secs_f64() * 1000. / frames as f64;
    eprintln!("Fahrphysik: 51 Fahrzeuge, {ms:.3} ms je Bild (60 Hz, intern 120 Hz)");
    assert!(
        cars.iter()
            .all(|(_, s, _)| s.x.is_finite() && s.y.is_finite())
    );
    if !cfg!(debug_assertions) {
        assert!(ms < 2., "{ms:.3} ms je Bild");
    }
}

#[test]
fn same_inputs_give_bit_identical_end_positions() {
    let db = VehicleDb::embedded().unwrap();
    let feel = Feel::game();
    let run = || {
        let mut cars = fleet(&db);
        // wechselnde Eingaben inkl. Handbremse (Drift-Schicht) und Bremsen
        for k in 0..1200 {
            for (j, (v, s, inp)) in cars.iter_mut().enumerate() {
                let mut i = *inp;
                i.handbrake = (k + j * 13) % 400 < 20;
                i.brake = if (k + j * 7) % 300 < 40 { 0.8 } else { 0. };
                step(v, &feel, s, &i, &Env::default(), 1. / 60.);
            }
        }
        cars.iter()
            .map(|(_, s, _)| (s.x.to_bits(), s.y.to_bits(), s.yaw.to_bits()))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
}
