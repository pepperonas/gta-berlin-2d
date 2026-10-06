//! Straßenname und nächste Kreuzung auf den echten Kacheln.
use berlin_sim::city::{City, DiskSource};
use berlin_sim::streetinfo::{AHEAD_M, street_label_at};
use berlin_sim::world::{DT, Input, World};

fn world() -> World {
    let root = berlin_map_loader::default_data_root();
    let city = City::open(&root, Box::new(DiskSource::new(root.clone()))).expect("Karte");
    let mut w = World::new(city, 71, 0, 0);
    for _ in 0..5 {
        w.update(&Input::default(), DT);
    }
    w
}

#[test]
fn names_the_street_and_the_next_crossing_ahead() {
    let mut w = world();
    // am Startpunkt steht man an einer benannten Straße
    let (x, y) = (w.player.x, w.player.y);
    let s = w.city.scale;
    let near = w
        .city
        .nearest_edge(x, y, 40. * s, |e| !e.name.is_empty() && e.cls <= 8)
        .expect("Straße in der Nähe");
    let e = w.city.edges[&near.edge].clone();
    // auf der Fahrbahnmitte, in beide Richtungen der Straße
    let (px, py) = (near.x, near.y);
    let fwd = street_label_at(&mut w, px, py, (near.ux, near.uy)).expect("vorwärts");
    let back = street_label_at(&mut w, px, py, (-near.ux, -near.uy)).expect("rückwärts");
    assert_eq!(fwd.street, e.name);
    assert_eq!(back.street, e.name);
    let mut found = 0;
    for l in [&fwd, &back] {
        if let Some((name, d)) = &l.cross {
            found += 1;
            assert_ne!(name, &e.name, "Querstraße heißt anders");
            assert!(*d >= 0. && *d <= AHEAD_M + 1., "Entfernung {d}");
        }
    }
    assert!(
        found >= 1,
        "mindestens in einer Richtung eine Kreuzung: {fwd:?} / {back:?}"
    );
    // beim Näherkommen schrumpft die Entfernung
    if let Some((name, d)) = fwd.cross.clone()
        && d > 20.
    {
        let step = 10. * s;
        let closer = street_label_at(
            &mut w,
            px + near.ux * step,
            py + near.uy * step,
            (near.ux, near.uy),
        )
        .unwrap();
        let (n2, d2) = closer.cross.expect("Kreuzung bleibt");
        assert_eq!(n2, name);
        assert!(d2 < d - 5., "{d} → {d2}");
    }
}
