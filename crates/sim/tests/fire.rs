//! Brennende Wracks und Explosionen auf den echten Kacheln.
use berlin_sim::car::{Car, Role};
use berlin_sim::city::{City, DiskSource};
use berlin_sim::events::Event;
use berlin_sim::fire::{BURN_S, BURN_SPREAD};
use berlin_sim::pedestrians::PedState;
use berlin_sim::world::{DT, Input, World};

fn world(seed: u32) -> World {
    let root = berlin_map_loader::default_data_root();
    let city = City::open(&root, Box::new(DiskSource::new(root.clone()))).expect("Karte");
    let mut w = World::new(city, seed, 0, 0);
    for _ in 0..5 {
        w.update(&Input::default(), DT);
    }
    w
}
/// Steps until (and including) the first step whose events match; returns that step's index.
fn run_until(w: &mut World, max: usize, input: Input, f: impl Fn(&Event) -> bool) -> Option<usize> {
    for k in 0..max {
        w.update(&input, DT);
        if w.events.iter().any(&f) {
            return Some(k);
        }
    }
    None
}

#[test]
fn wreck_burns_then_explodes_and_hurts_the_surroundings() {
    let mut w = world(41);
    let pc = w.player_car_id.unwrap();
    let (x, y) = w.car(pc).map(|c| (c.x, c.y)).unwrap();
    // Spieler weg vom Geschehen
    w.player.x = x + 600.;
    // ein zweites Auto direkt daneben, ein Passant nah dran
    let id2 = 9_999;
    w.cars
        .push(Car::new(id2, x + 55., y, 0., 0x336699, Role::Parked, "car"));
    let i = w.cars.iter().position(|c| c.id == pc).unwrap();
    w.cars[i].health = 0.;
    w.cars[i].wrecked = true;
    let fire = run_until(
        &mut w,
        5,
        Input::default(),
        |e| matches!(e, Event::CarFire { car, .. } if *car == pc),
    );
    assert!(fire.is_some(), "Wrack fängt Feuer");
    assert!(w.car(pc).unwrap().burn.is_some());
    let max = ((BURN_S + BURN_SPREAD) / DT) as usize + 10;
    let boom = run_until(
        &mut w,
        max,
        Input::default(),
        |e| matches!(e, Event::Explosion { car, .. } if *car == Some(pc)),
    );
    let k = boom.expect("explodiert");
    assert!(
        k as f64 * DT >= BURN_S - 0.1,
        "erst nach der Brennzeit ({k} Schritte)"
    );
    assert!(w.car(pc).unwrap().exploded);
    let other = w.car(id2).unwrap();
    assert!(other.health < 100., "Nachbar beschädigt");
    assert!(
        other.vx.hypot(other.vy) > 10. || other.wrecked,
        "Nachbar weggestoßen"
    );
}

#[test]
fn explosions_chain_from_car_to_car() {
    let mut w = world(42);
    let pc = w.player_car_id.unwrap();
    let (x, y) = w.car(pc).map(|c| (c.x, c.y)).unwrap();
    w.player.x = x + 800.;
    let id2 = 9_998;
    let mut c2 = Car::new(id2, x + 50., y, 0., 0x993333, Role::Parked, "car");
    c2.health = 20.;
    w.cars.push(c2);
    let i = w.cars.iter().position(|c| c.id == pc).unwrap();
    w.cars[i].health = 0.;
    w.cars[i].wrecked = true;
    let max = (2. * (BURN_S + BURN_SPREAD) / DT) as usize + 60;
    let second = run_until(
        &mut w,
        max,
        Input::default(),
        |e| matches!(e, Event::Explosion { car, .. } if *car == Some(id2)),
    );
    assert!(
        second.is_some(),
        "Kettenreaktion: das angeschlagene Nachbarauto fliegt hinterher"
    );
}

#[test]
fn player_inside_is_warned_and_thrown_out() {
    let mut w = world(43);
    let pc = w.player_car_id.unwrap();
    let (x, y) = w.car(pc).map(|c| (c.x, c.y)).unwrap();
    (w.player.x, w.player.y) = (x + 15., y);
    w.update(
        &Input {
            enter_exit: true,
            ..Default::default()
        },
        DT,
    );
    assert_eq!(w.player.in_car, Some(pc));
    let hp0 = w.player.combat.hp;
    let i = w.cars.iter().position(|c| c.id == pc).unwrap();
    w.cars[i].health = 0.;
    w.cars[i].wrecked = true;
    for _ in 0..10 {
        w.update(&Input::default(), DT);
    }
    assert!(
        w.notice.as_ref().is_some_and(|n| n.text.contains("brennt")),
        "Warnung"
    );
    let max = ((BURN_S + BURN_SPREAD) / DT) as usize + 10;
    run_until(&mut w, max, Input::default(), |e| {
        matches!(e, Event::Explosion { .. })
    })
    .expect("explodiert");
    assert_eq!(w.player.in_car, None, "hinausgeschleudert");
    assert!(w.player.combat.hp < hp0, "verletzt");
    assert!(w.city.in_building(w.player.x, w.player.y).is_none());
}

#[test]
fn nearby_people_die_or_flee() {
    let root = berlin_map_loader::default_data_root();
    let city = City::open(&root, Box::new(DiskSource::new(root.clone()))).expect("Karte");
    let mut w = World::new(city, 44, 0, 40);
    for _ in 0..30 {
        w.update(&Input::default(), DT);
    }
    let pc = w.player_car_id.unwrap();
    let (x, y) = w.car(pc).map(|c| (c.x, c.y)).unwrap();
    w.player.x = x + 800.;
    let mut p = w.peds.first().cloned().expect("Passanten in der Welt");
    p.id = 77_777;
    (p.x, p.y) = (x + 20., y);
    p.level = w.car(pc).unwrap().level;
    p.state = PedState::Idle;
    p.t = 60.;
    w.peds.push(p);
    let i = w.cars.iter().position(|c| c.id == pc).unwrap();
    w.cars[i].health = 0.;
    w.cars[i].wrecked = true;
    let max = ((BURN_S + BURN_SPREAD) / DT) as usize + 20;
    run_until(&mut w, max, Input::default(), |e| {
        matches!(e, Event::Explosion { .. })
    })
    .expect("explodiert");
    let q = w
        .peds
        .iter()
        .find(|q| q.id == 77_777)
        .expect("Passant noch da");
    assert!(
        q.state == PedState::Dead || q.hp < 100.,
        "Passant neben dem Wrack getroffen: {:?} {}",
        q.state,
        q.hp
    );
}

#[test]
fn switch_off_keeps_wrecks_quiet() {
    let mut w = world(45);
    w.explosions = false;
    let pc = w.player_car_id.unwrap();
    let i = w.cars.iter().position(|c| c.id == pc).unwrap();
    w.cars[i].health = 0.;
    w.cars[i].wrecked = true;
    let any = run_until(&mut w, 600, Input::default(), |e| {
        matches!(e, Event::CarFire { .. } | Event::Explosion { .. })
    });
    assert!(any.is_none());
}

#[test]
fn console_ignite_never_picks_the_own_car() {
    let mut w = world(46);
    let pc = w.player_car_id.unwrap();
    let (x, y) = w.car(pc).map(|c| (c.x, c.y)).unwrap();
    // nur das eigene Auto in der Nähe: nichts
    w.cars.retain(|c| c.id == pc);
    assert_eq!(w.ignite_nearest(x, y, 600., 0.5), None);
    // ein fremdes daneben: das brennt
    w.cars.push(Car::new(
        9_997,
        x + 60.,
        y,
        0.,
        0x777777,
        Role::Parked,
        "car",
    ));
    assert_eq!(w.ignite_nearest(x, y, 600., 0.5), Some(9_997));
    let c = w.car(9_997).unwrap();
    assert!(c.wrecked && c.burn.is_some());
    assert!(!w.car(pc).unwrap().wrecked);
}

#[test]
fn a_fire_is_heard_nearby_and_fades_with_distance() {
    use berlin_sim::ambience::{FIRE_HEAR, fire_level};
    let mut w = world(47);
    let pc = w.player_car_id.unwrap();
    let (x, y) = w.car(pc).map(|c| (c.x, c.y)).unwrap();
    assert_eq!(fire_level(&w, x, y), 0., "ohne Brand still");
    let i = w.cars.iter().position(|c| c.id == pc).unwrap();
    w.cars[i].health = 0.;
    w.cars[i].wrecked = true;
    w.update(&Input::default(), DT);
    assert!(w.car(pc).unwrap().burn.is_some());
    let near = fire_level(&w, x, y);
    let mid = fire_level(&w, x + FIRE_HEAR * 0.5, y);
    assert!(near > 0.9, "direkt daneben laut: {near}");
    assert!(mid > 0. && mid < near * 0.5, "halbe Hörweite leiser: {mid}");
    assert_eq!(fire_level(&w, x + FIRE_HEAR + 10., y), 0.);
}
