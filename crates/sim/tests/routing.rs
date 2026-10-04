//! Navigation auf den echten Berlin-Daten: Graph aus allen Kacheln, Routen zwischen den Auftragsorten.
use berlin_sim::routing::{Mode, RouteGraph};

#[test]
fn routes_across_berlin_between_the_mission_places() {
    let root = berlin_map_loader::default_data_root();
    let g = RouteGraph::build(&root, None).expect("Graph");
    assert!(
        g.edges.len() > 200_000,
        "ganz Berlin: {} Kanten",
        g.edges.len()
    );
    let idx: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("index.json")).unwrap()).unwrap();
    let at = |k: &str| {
        let p = &idx["places"][k];
        (p["x"].as_f64().unwrap(), p["y"].as_f64().unwrap())
    };
    let (giver, pickup, dropoff) = (at("giver"), at("pickup"), at("dropoff"));
    for mode in [Mode::Car, Mode::Foot] {
        for (a, b) in [(giver, pickup), (pickup, dropoff), (dropoff, giver)] {
            let r = g
                .route(a, b, mode)
                .unwrap_or_else(|| panic!("{mode:?}: keine Route"));
            assert_eq!(r.pts[0], a);
            assert_eq!(*r.pts.last().unwrap(), b);
            let straight = (b.0 - a.0).hypot(b.1 - a.1) / 10.;
            assert!(
                r.len_m >= straight * 0.98 && r.len_m < straight * 3.,
                "{mode:?}: {:.0} m für {straight:.0} m Luftlinie",
                r.len_m
            );
            // durchgehend: kein Sprung über ein Stück, das länger als jede Straße wäre
            let worst = r
                .pts
                .windows(2)
                .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
                .fold(0., f64::max);
            assert!(worst < 4000., "{mode:?}: Lücke von {worst:.0} px");
            assert!(r.secs > 0. && r.edges.len() >= 2);
        }
    }
    // weit außerhalb: keine Straße zum Einrasten → keine Route
    assert!(g.route(giver, (-1e6, -1e6), Mode::Car).is_none());
}
