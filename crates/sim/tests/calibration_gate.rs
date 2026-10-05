//! Fahrphysik Phase 8: Der Kalibrierlauf ist ein Test. Er misst jedes Fahrzeug mit seinen kalibrierten Werten
//! (`vehicles.calibrated.json`) und schlägt fehl, sobald ein Zielwert außerhalb der Toleranz liegt, der nicht in
//! `data/vehicles/bekannte_abweichungen.json` mit Grund verzeichnet ist. Ein verzeichneter Wert, der wieder passt,
//! lässt den Test ebenfalls scheitern – die Liste soll nichts Erledigtes mitschleppen.
use berlin_sim::calibrate;
use berlin_sim::vehdata::{Feel, VehicleDb};
use std::collections::{BTreeMap, BTreeSet};

const KNOWN: &str = include_str!("../../../data/vehicles/bekannte_abweichungen.json");

#[test]
fn every_vehicle_meets_its_targets_or_has_a_documented_reason() {
    let db = VehicleDb::embedded().unwrap();
    let feel = Feel::simulation();
    let known: BTreeMap<String, String> = serde_json::from_str(KNOWN).unwrap();
    for (k, why) in &known {
        assert!(why.len() > 10, "{k}: Grund fehlt");
    }
    let mut fails = BTreeSet::new();
    let mut checked = 0;
    for v in &db.vehicles {
        if v.targets.is_empty() {
            continue;
        }
        let vv = db.calibrated(&v.id).unwrap();
        let keys: Vec<&str> = v.targets.keys().map(String::as_str).collect();
        let mut m = BTreeMap::new();
        if keys.iter().any(|k| k.starts_with("0_")) {
            m.extend(calibrate::accel(&vv, &feel).values);
        }
        if keys.contains(&"vmax") {
            m.extend(calibrate::vmax(&vv, &feel).values);
        }
        for k in keys.iter().filter(|k| k.starts_with("brems_")) {
            let kmh: f64 = k[6..].parse().unwrap();
            m.extend(calibrate::brake(&vv, &feel, kmh).values);
        }
        if keys.contains(&"quer_g") {
            m.extend(calibrate::lateral(&vv, &feel).values);
        }
        for (k, t) in &v.targets {
            checked += 1;
            let ok = m.get(k).is_some_and(|a| calibrate::within(k, *t, *a));
            if !ok {
                fails.insert(format!("{}/{k}", v.id));
            }
        }
    }
    let known_keys: BTreeSet<String> = known.keys().cloned().collect();
    let new: Vec<_> = fails.difference(&known_keys).collect();
    let healed: Vec<_> = known_keys.difference(&fails).collect();
    eprintln!(
        "Kalibrierung: {} von {checked} Zielwerten in der Toleranz, {} bekannte Abweichungen",
        checked - fails.len(),
        known.len()
    );
    assert!(new.is_empty(), "neu außerhalb der Toleranz: {new:?}");
    assert!(
        healed.is_empty(),
        "passt wieder, aus der Liste streichen: {healed:?}"
    );
}
