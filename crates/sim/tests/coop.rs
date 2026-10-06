//! Lokaler Koop auf den echten Kacheln: Spieler 2 läuft, fährt und spielt den Auftrag mit, die Stadt lebt um beide.
use berlin_sim::car::Driver;
use berlin_sim::city::{City, DiskSource};
use berlin_sim::mission::State;
use berlin_sim::world::{DT, Input, World};

fn world(seed: u32) -> World {
    let root = berlin_map_loader::default_data_root();
    let city = City::open(&root, Box::new(DiskSource::new(root.clone()))).expect("Karte");
    World::new(city, seed, 22, 55)
}
fn idle() -> Input {
    Input::default()
}
fn run(w: &mut World, steps: usize, a: Input, b: Input) {
    for _ in 0..steps {
        w.update_coop(&a, &b, DT);
    }
}
fn p2(w: &World) -> &berlin_sim::world::Player {
    &w.p2.as_ref().expect("Spieler 2").player
}

#[test]
fn p2_joins_beside_p1_and_walks_on_its_own() {
    let mut w = world(11);
    run(&mut w, 5, idle(), idle());
    assert!(w.join_p2());
    assert!(!w.join_p2(), "nur ein zweiter Spieler");
    let (x1, y1) = (w.player.x, w.player.y);
    let (x2, y2) = (p2(&w).x, p2(&w).y);
    let d = (x2 - x1).hypot(y2 - y1);
    assert!(d > 10. && d < 120., "neben Spieler 1: {d}");
    assert!(w.city.in_building(x2, y2).is_none());
    run(
        &mut w,
        60,
        idle(),
        Input {
            move_x: 1.,
            ..idle()
        },
    );
    assert!(p2(&w).x > x2 + 10., "Spieler 2 läuft");
    assert!(
        (w.player.x - x1).abs() < 1e-9 && (w.player.y - y1).abs() < 1e-9,
        "Spieler 1 bleibt stehen"
    );
}

#[test]
fn p2_drives_own_car_and_cannot_take_p1s() {
    let mut w = world(12);
    run(&mut w, 5, idle(), idle());
    let pc = w.player_car_id.unwrap();
    let (cx, cy) = {
        let c = w.car(pc).unwrap();
        (c.x, c.y)
    };
    // Spieler 1 steigt in sein Auto
    (w.player.x, w.player.y) = (cx + 20., cy);
    run(
        &mut w,
        1,
        Input {
            enter_exit: true,
            ..idle()
        },
        idle(),
    );
    assert_eq!(w.player.in_car, Some(pc));
    // Spieler 2 direkt daneben: das Auto ist besetzt, er nimmt es nicht
    assert!(w.join_p2());
    {
        let s = w.p2.as_mut().unwrap();
        (s.player.x, s.player.y) = (cx - 20., cy);
    }
    run(
        &mut w,
        1,
        idle(),
        Input {
            enter_exit: true,
            ..idle()
        },
    );
    assert_ne!(p2(&w).in_car, Some(pc), "Auto von Spieler 1 bleibt seins");
    assert_eq!(w.player.in_car, Some(pc));
    assert_eq!(w.car(pc).unwrap().driver, Some(Driver::Player));
    // Spieler 1 steigt aus und geht weg; jetzt darf Spieler 2 einsteigen und fahren
    run(
        &mut w,
        1,
        Input {
            enter_exit: true,
            ..idle()
        },
        idle(),
    );
    assert_eq!(w.player.in_car, None);
    w.player.x += 300.;
    {
        let c = w.car(pc).unwrap();
        let (x, y) = (c.x, c.y);
        let s = w.p2.as_mut().unwrap();
        (s.player.x, s.player.y) = (x - 20., y);
    }
    run(
        &mut w,
        1,
        idle(),
        Input {
            enter_exit: true,
            ..idle()
        },
    );
    assert_eq!(p2(&w).in_car, Some(pc));
    let start = {
        let c = w.car(pc).unwrap();
        (c.x, c.y)
    };
    run(
        &mut w,
        180,
        idle(),
        Input {
            throttle: 1.,
            ..idle()
        },
    );
    let c = w.car(pc).unwrap();
    assert!(
        (c.x - start.0).hypot(c.y - start.1) > 30.,
        "Spieler 2 fährt"
    );
    let q = p2(&w);
    assert!(
        (q.x - c.x).abs() < 1e-6 && (q.y - c.y).abs() < 1e-6,
        "Figur sitzt im Auto"
    );
    assert_eq!(w.player.in_car, None, "Spieler 1 bleibt draußen");
}

#[test]
fn p2_can_accept_the_mission() {
    let mut w = world(13);
    run(&mut w, 5, idle(), idle());
    assert!(w.join_p2());
    let pl = w.city.places.clone();
    // Spieler 2 am Späti, Spieler 1 ein Stück weiter
    w.player.x = pl.giver.x + 400.;
    w.player.y = pl.giver.y;
    {
        let s = w.p2.as_mut().unwrap();
        (s.player.x, s.player.y) = (pl.giver.x, pl.giver.y);
    }
    run(&mut w, 1, idle(), idle());
    assert_eq!(w.mission.prompt, Some("A: Auftrag annehmen"));
    run(
        &mut w,
        1,
        idle(),
        Input {
            action: true,
            ..idle()
        },
    );
    assert_eq!(w.mission.state, State::Briefing);
    run(
        &mut w,
        1,
        idle(),
        Input {
            action: true,
            ..idle()
        },
    );
    assert_eq!(w.mission.state, State::ToPickup, "Spieler 2 bestätigt");
}

#[test]
fn city_lives_around_both_players_and_shrinks_back() {
    let mut w = world(14);
    run(&mut w, 30, idle(), idle());
    assert!(w.join_p2());
    // Spieler 2 rund 1 km weiter östlich auf die Straße setzen (die Kacheln dort lädt der zweite Fokus)
    let (x, y) = (w.player.x + 10000., w.player.y);
    let spot = (0..40)
        .flat_map(|i| (0..40).map(move |j| (x + i as f64 * 40. - 800., y + j as f64 * 40. - 800.)))
        .find(|&(sx, sy)| w.city.in_building(sx, sy).is_none() && w.city.inside_border(sx, sy))
        .expect("freier Platz");
    {
        let s = w.p2.as_mut().unwrap();
        (s.player.x, s.player.y) = spot;
        (s.camera.x, s.camera.y) = spot;
        s.player.level_init = false;
    }
    run(&mut w, 600, idle(), idle());
    assert!(!w.loading, "beide Ausschnitte geladen");
    assert!(w.apart());
    let near2 = |w: &World| {
        let q = p2(w);
        let (x, y) = (q.x, q.y);
        (
            w.cars
                .iter()
                .filter(|c| c.driver == Some(Driver::Npc) && (c.x - x).hypot(c.y - y) < 2500.)
                .count(),
            w.peds
                .iter()
                .filter(|p| (p.x - x).hypot(p.y - y) < 2500.)
                .count(),
        )
    };
    let (cars2, peds2) = near2(&w);
    assert!(cars2 >= 4, "Verkehr um Spieler 2: {cars2}");
    assert!(peds2 >= 8, "Passanten um Spieler 2: {peds2}");
    let p1 = (w.player.x, w.player.y);
    let peds1 = w
        .peds
        .iter()
        .filter(|p| (p.x - p1.0).hypot(p.y - p1.1) < 2500.)
        .count();
    assert!(peds1 >= 8, "Passanten um Spieler 1 bleiben: {peds1}");
    // Spieler 2 geht: Fokus frei, alles um ihn verschwindet nach und nach
    w.leave_p2();
    assert!(!w.coop());
    run(&mut w, 600, idle(), idle());
    let far = w
        .peds
        .iter()
        .filter(|p| (p.x - spot.0).hypot(p.y - spot.1) < 1500.)
        .count();
    assert_eq!(far, 0, "Passanten am alten Ort von Spieler 2 abgebaut");
}

#[test]
fn coop_is_deterministic() {
    let play = || {
        let mut w = world(15);
        run(&mut w, 5, idle(), idle());
        w.join_p2();
        for i in 0..600 {
            let a = Input {
                move_x: ((i as f64) * 0.02).sin(),
                ..idle()
            };
            let b = Input {
                move_y: ((i as f64) * 0.03).cos(),
                sprint: i % 3 == 0,
                ..idle()
            };
            w.update_coop(&a, &b, DT);
        }
        let q = p2(&w);
        (w.player.x, w.player.y, q.x, q.y, w.cars.len(), w.peds.len())
    };
    assert_eq!(play(), play());
}
