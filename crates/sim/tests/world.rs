//! Integrationstests der Simulation auf den echten Berlin-Kacheln (Kreuzberg/Neukölln um den Missionsort).
use berlin_sim::car::{Driver, Role};
use berlin_sim::city::{City, DiskSource, Ground};
use berlin_sim::mission::State;
use berlin_sim::pedestrians::PedState;
use berlin_sim::roadgraph::{Light, signal_state};
use berlin_sim::save::{FileStorage, read_save, write_save};
use berlin_sim::world::{DT, Input, World};

fn root() -> std::path::PathBuf {
    berlin_map_loader::default_data_root()
}
fn world(seed: u32) -> World {
    let city = City::open(&root(), Box::new(DiskSource::new(root()))).expect("Karte");
    let w = World::new(city, seed, 22, 55);
    assert!(!w.loading, "synchrone Quelle lädt sofort");
    w
}
fn idle() -> Input {
    Input::default()
}
fn run(w: &mut World, steps: usize, input: Input) {
    for _ in 0..steps {
        w.update(&input, DT);
    }
}

#[test]
fn populates_traffic_pedestrians_and_parked_cars() {
    let mut w = world(1989);
    run(&mut w, 30, idle());
    let npc = w
        .cars
        .iter()
        .filter(|c| c.driver == Some(Driver::Npc))
        .count();
    let curb = w.cars.iter().filter(|c| c.role == Role::Curb).count();
    assert!(npc >= 15, "{}", berlin_sim::world::describe(&w));
    assert!(w.peds.len() >= 40, "{}", berlin_sim::world::describe(&w));
    assert!(curb > 20, "Parker am Straßenrand: {curb}");
    assert!(w.lanes.len() > 1000);
    // Spieler steht am Späti, das eigene Auto auf dem Parkplatz
    let p = w.city.places.player_car;
    let own = w.car(w.player_car_id.unwrap()).unwrap();
    assert!((own.x - p.x).abs() < 1. && (own.y - p.y).abs() < 1.);
}

#[test]
fn traffic_soak_stays_on_roads_and_moves() {
    let mut w = world(7);
    run(&mut w, 30, idle());
    // zurückgelegte Strecke je Auto (weit gefahrene Autos werden außerhalb des Kamera-Radius abgebaut)
    let mut travelled: std::collections::HashMap<u32, (f64, f64, f64)> = Default::default();
    let mut crashes = 0;
    for step in 0..3600 {
        w.update(&idle(), DT);
        crashes += w
            .events
            .iter()
            .filter(|e| matches!(e, berlin_sim::events::Event::Crash { .. }))
            .count();
        if step % 30 == 0 {
            for c in w.cars.iter().filter(|c| c.driver == Some(Driver::Npc)) {
                let e = travelled.entry(c.id).or_insert((c.x, c.y, 0.));
                e.2 += (c.x - e.0).hypot(c.y - e.1);
                (e.0, e.1) = (c.x, c.y);
            }
        }
        if step % 60 == 0 {
            let npc: Vec<(u32, f64, f64, f64)> = w
                .cars
                .iter()
                .filter(|c| c.driver == Some(Driver::Npc))
                .map(|c| (c.id, c.x, c.y, c.angle))
                .collect();
            for (id, x, y, a) in npc {
                assert!(x.is_finite() && y.is_finite() && a.is_finite());
                assert!(
                    w.city.in_building(x, y).is_none(),
                    "KI-Auto {id} steht in einem Haus ({x}, {y})"
                );
            }
            for p in &w.peds {
                assert!(p.x.is_finite() && p.y.is_finite());
            }
        }
    }
    let total = travelled.len();
    let moved = travelled.values().filter(|t| t.2 > 300.).count();
    assert!(
        total >= 30,
        "Verkehr wird nachgeführt: {total} Autos insgesamt"
    );
    assert!(
        moved * 10 >= total * 7,
        "nur {moved} von {total} KI-Autos sind mehr als 30 m gefahren"
    );
    assert!(
        crashes < 40,
        "zu viele Unfälle im freien Verkehr: {crashes}"
    );
    // Passanten gehen tatsächlich (nicht alle stehen)
    assert!(w.peds.iter().filter(|p| p.state == PedState::Walk).count() > 10);
}

#[test]
fn signals_alternate_on_a_real_junction() {
    let w = world(1);
    let v = *w
        .city
        .signals
        .iter()
        .min()
        .expect("Ampeln im geladenen Gebiet");
    let (mut green0, mut green1, mut both) = (0, 0, 0);
    for k in 0..500 {
        let t = k as f64 * 0.1;
        let a = signal_state(&w.city, v, 0., t) == Light::Green;
        let b = signal_state(&w.city, v, std::f64::consts::FRAC_PI_2, t) == Light::Green;
        green0 += a as u32;
        green1 += b as u32;
        both += (a && b) as u32;
    }
    assert_eq!(both, 0, "kreuzende Achsen nie gleichzeitig grün");
    assert!(
        (195..=205).contains(&green0) && (195..=205).contains(&green1),
        "{green0} / {green1}"
    );
}

#[test]
fn player_enters_drives_and_cannot_pass_buildings() {
    let mut w = world(3);
    // zum eigenen Auto laufen und einsteigen
    let pc = w.player_car_id.unwrap();
    let (cx, cy) = {
        let c = w.car(pc).unwrap();
        (c.x, c.y)
    };
    w.player.x = cx + 20.;
    w.player.y = cy;
    run(
        &mut w,
        1,
        Input {
            enter_exit: true,
            ..idle()
        },
    );
    assert_eq!(w.player.in_car, Some(pc));
    assert_eq!(w.car(pc).unwrap().driver, Some(Driver::Player));
    // Vollgas mit wechselndem Einschlag: das Auto bewegt sich, landet aber nie in einem Haus
    let mut max_speed: f64 = 0.;
    for k in 0..1800 {
        let steer = if (k / 120) % 2 == 0 { 0.6 } else { -0.4 };
        w.update(
            &Input {
                throttle: 1.,
                steer,
                ..idle()
            },
            DT,
        );
        let (x, y, v) = {
            let c = w.car(pc).unwrap();
            (c.x, c.y, c.speed())
        };
        max_speed = max_speed.max(v);
        assert!(
            w.city.in_building(x, y).is_none(),
            "Spielerauto im Haus bei Schritt {k}"
        );
        assert!(w.city.inside_border(x, y));
    }
    assert!(
        max_speed > 50.,
        "Spielerauto kam nicht in Fahrt: {max_speed}"
    );
    assert!(
        w.car(pc).unwrap().dyn_state.is_some(),
        "echte Fahrdynamik aktiv"
    );
    // aussteigen
    run(
        &mut w,
        120,
        Input {
            brake: 1.,
            ..idle()
        },
    );
    run(
        &mut w,
        1,
        Input {
            enter_exit: true,
            ..idle()
        },
    );
    if w.player.in_car.is_none() {
        assert!(w.city.in_building(w.player.x, w.player.y).is_none());
        assert_eq!(w.car(pc).unwrap().driver, None);
    }
}

#[test]
fn player_car_accelerates_on_a_straight_lane() {
    let mut w = world(8);
    let pc = w.player_car_id.unwrap();
    let (px, py) = (w.player.x, w.player.y);
    // lange gerade Spur in der Nähe, ohne anderen Verkehr
    let lane = w
        .lanes
        .lanes
        .values()
        .filter(|l| {
            (l.pts[1].0 - l.pts[0].0).hypot(l.pts[1].1 - l.pts[0].1) > 500.
                && (l.pts[0].0 - px).hypot(l.pts[0].1 - py) < 3000.
        })
        .min_by_key(|l| l.id)
        .expect("gerade Spur")
        .clone();
    w.cars.retain(|c| c.id == pc);
    w.peds.clear();
    w.car_target = 0;
    w.ped_target = 0;
    let i = w.cars.iter().position(|c| c.id == pc).unwrap();
    let (a, b) = (lane.pts[0], lane.pts[1]);
    (w.cars[i].x, w.cars[i].y, w.cars[i].angle) = (a.0, a.1, (b.1 - a.1).atan2(b.0 - a.0));
    (w.player.x, w.player.y) = (a.0, a.1);
    run(
        &mut w,
        1,
        Input {
            enter_exit: true,
            ..idle()
        },
    );
    assert_eq!(w.player.in_car, Some(pc));
    run(
        &mut w,
        150,
        Input {
            throttle: 1.,
            ..idle()
        },
    );
    let c = w.car(pc).unwrap();
    assert!(
        c.speed() * 0.36 > 50.,
        "nach 2,5 s erst {:.0} km/h",
        c.speed() * 0.36
    );
    // Bremsen bringt es zum Stehen (gehalten setzt es danach zurück, wie in der JS-Fassung)
    let mut stopped = None;
    for k in 0..240 {
        w.update(
            &Input {
                brake: 1.,
                ..idle()
            },
            DT,
        );
        if w.car(pc).unwrap().forward_speed() <= 5. {
            stopped = Some(k);
            break;
        }
    }
    assert!(
        stopped.is_some_and(|k| k < 150),
        "Bremsweg zu lang: {stopped:?}"
    );
    run(
        &mut w,
        120,
        Input {
            brake: 1.,
            ..idle()
        },
    );
    assert!(
        w.car(pc).unwrap().forward_speed() < -5.,
        "Bremse gehalten = rückwärts"
    );
}

#[test]
fn on_foot_walks_and_is_blocked_by_walls() {
    let mut w = world(4);
    let (x0, y0) = (w.player.x, w.player.y);
    for dir in [(1., 0.), (0., 1.), (-1., 0.), (0., -1.)] {
        for _ in 0..600 {
            w.update(
                &Input {
                    move_x: dir.0,
                    move_y: dir.1,
                    ..idle()
                },
                DT,
            );
            assert!(
                w.city.in_building(w.player.x, w.player.y).is_none(),
                "Spieler im Haus"
            );
        }
    }
    assert!((w.player.x - x0).hypot(w.player.y - y0) > 1. || w.player.step > 100.);
    // Sprint leert die Ausdauer
    run(
        &mut w,
        300,
        Input {
            move_x: 1.,
            sprint: true,
            ..idle()
        },
    );
    assert!(w.player.stamina < 1.);
}

#[test]
fn mission_runs_through_world_inputs() {
    let mut w = world(5);
    let pl = w.city.places.clone();
    w.player.x = pl.giver.x;
    w.player.y = pl.giver.y;
    run(&mut w, 1, idle());
    assert_eq!(w.mission.prompt, Some("A: Auftrag annehmen"));
    run(
        &mut w,
        1,
        Input {
            action: true,
            ..idle()
        },
    );
    assert_eq!(w.mission.state, State::Briefing);
    run(
        &mut w,
        1,
        Input {
            action: true,
            ..idle()
        },
    );
    assert_eq!(w.mission.state, State::ToPickup);
    let limit = pl.time_limit.unwrap();
    assert!((w.mission.timer - limit).abs() < 0.1);
    // ins Auto, zur Lagerhalle versetzen (Teleport wie die Konsole), anhalten, einladen
    let pc = w.player_car_id.unwrap();
    let i = w.cars.iter().position(|c| c.id == pc).unwrap();
    (w.player.x, w.player.y) = (w.cars[i].x + 15., w.cars[i].y);
    run(
        &mut w,
        1,
        Input {
            enter_exit: true,
            ..idle()
        },
    );
    assert_eq!(w.player.in_car, Some(pc));
    let i = w.cars.iter().position(|c| c.id == pc).unwrap();
    (w.cars[i].x, w.cars[i].y, w.cars[i].vx, w.cars[i].vy) = (pl.pickup.x, pl.pickup.y, 0., 0.);
    w.camera.x = pl.pickup.x;
    w.camera.y = pl.pickup.y;
    w.reset_population();
    run(
        &mut w,
        150,
        Input {
            action_held: true,
            ..idle()
        },
    );
    assert_eq!(
        w.mission.state,
        State::ToDropoff,
        "Einladen: {:?}",
        w.mission.prompt
    );
    assert!(w.car(pc).unwrap().cargo);
    let i = w.cars.iter().position(|c| c.id == pc).unwrap();
    (w.cars[i].x, w.cars[i].y, w.cars[i].vx, w.cars[i].vy) = (pl.dropoff.x, pl.dropoff.y, 0., 0.);
    w.camera.x = pl.dropoff.x;
    w.camera.y = pl.dropoff.y;
    w.reset_population();
    run(&mut w, 2, idle());
    run(
        &mut w,
        1,
        Input {
            action: true,
            ..idle()
        },
    );
    assert_eq!(w.mission.state, State::Success, "{:?}", w.mission.prompt);
    assert!(w.money >= 100. && w.completed == 1. && w.best_time.is_some());
}

#[test]
fn save_roundtrip_through_file() {
    let mut w = world(6);
    w.money = 4321.;
    w.completed = 2.;
    let pc = w.player_car_id.unwrap();
    let i = w.cars.iter().position(|c| c.id == pc).unwrap();
    w.cars[i].health = 64.;
    w.cars[i].model = Some("rallye");
    let data = w.make_save(1.0);
    let dir = std::env::temp_dir().join(format!("gta-berlin-world-save-{}", std::process::id()));
    let mut st = FileStorage::new(dir.join("save.json"));
    write_save(&mut st, &data).unwrap();
    let back = read_save(&st).expect("Stand lesbar");
    std::fs::remove_dir_all(&dir).unwrap();
    let mut w2 = world(6);
    w2.apply_save(back);
    let c2 = w2.car(w2.player_car_id.unwrap()).unwrap();
    assert_eq!((w2.money, w2.completed), (4321., 2.));
    assert_eq!((c2.health, c2.model), (64., Some("rallye")));
    assert!((c2.x - w.cars[i].x).abs() <= 0.5 && (c2.y - w.cars[i].y).abs() <= 0.5);
    // der Spieler steht neben dem Auto, nicht in einem Haus
    assert!((w2.player.x - c2.x).hypot(w2.player.y - c2.y) <= 25.);
    assert!(w2.city.in_building(w2.player.x, w2.player.y).is_none());
}

#[test]
fn simulation_is_deterministic() {
    let hash = |seed| {
        let mut w = world(seed);
        run(&mut w, 600, idle());
        let mut h = 0f64;
        for c in &w.cars {
            h += c.x * 1.3 + c.y * 0.7 + c.id as f64;
        }
        for p in &w.peds {
            h += p.x * 0.11 + p.y * 0.13;
        }
        (h, w.cars.len(), w.peds.len())
    };
    assert_eq!(hash(11), hash(11));
    assert_ne!(hash(11).0, hash(12).0);
}

#[test]
fn surfaces_on_real_map() {
    let mut w = world(1);
    let p = w.city.places.player_car;
    assert!(w.city.surface_at(p.x, p.y, Some(0)) != Ground::Building);
    // Ein Punkt auf einer echten Fahrbahn ist Straße
    let e = w
        .city
        .edges
        .values()
        .find(|e| e.cls <= 7 && e.w > 60. && e.lvl == 0 && !e.passage)
        .unwrap()
        .clone();
    let mid = berlin_sim::city::point_along(&e.pts, e.len / 2.);
    assert!(w.city.surface_at(mid.x, mid.y, Some(0)).is_road());
}

#[test]
fn lamps_stand_on_sidewalks() {
    let mut w = world(2);
    let mut cache = berlin_sim::lamps::LampCache::default();
    let (x, y) = (w.player.x, w.player.y);
    let lamps = cache.near(&mut w.city, x, y, 1500.);
    // Referenz: lamps.js edgeLamps auf derselben Karte im selben Ausschnitt ergibt 41
    assert_eq!(
        lamps.len(),
        41,
        "Laternen um den Späti wie in der JS-Fassung"
    );
    for l in &lamps {
        assert!(
            w.city.in_building(l.x, l.y).is_none(),
            "Laterne im Haus bei ({}, {})",
            l.x,
            l.y
        );
        assert!(
            !w.city.tree_on_road(l.x, l.y, 3.),
            "Laterne auf der Fahrbahn bei ({}, {})",
            l.x,
            l.y
        );
        assert!((l.nx.hypot(l.ny) - 1.).abs() < 1e-9);
    }
    // deterministisch: dieselben Standorte beim zweiten Abfragen (aus dem Zwischenspeicher wie neu berechnet)
    let again = berlin_sim::lamps::LampCache::default().near(&mut w.city, x, y, 1500.);
    assert_eq!(lamps.len(), again.len());
}

#[test]
fn weather_covers_the_road_and_warns_the_driver() {
    let mut w = world(7);
    w.force_weather = Some("heavysnow");
    run(&mut w, 600, idle());
    assert_eq!(w.sky.kind, "heavysnow");
    assert!(
        w.weather.snow > 0.01,
        "Schneedecke wächst: {}",
        w.weather.snow
    );
    assert!(w.temp < 2., "Schneewetter ist kalt: {}", w.temp);
    // zu Fuß keine Warnung, im eigenen Auto auf verschneiter Straße schon
    assert_eq!(w.road_warning(), None);
    w.weather.snow = 0.8;
    let pc = w.player_car_id.expect("eigenes Auto");
    let (x, y) = w.car(pc).map(|c| (c.x, c.y)).unwrap();
    (w.player.x, w.player.y) = (x + 15., y);
    run(
        &mut w,
        1,
        Input {
            enter_exit: true,
            ..idle()
        },
    );
    assert!(w.player.in_car.is_some());
    let warn = w.road_warning();
    assert!(matches!(warn, Some("Schnee" | "Glätte")), "{warn:?}");
    // Schnee macht die Reifen stumpf
    let tr = w.car(pc).unwrap().traction;
    assert!(tr.lat < 1. && tr.brake < 1., "{tr:?}");
    // klares Wetter: Straßen trocknen, kein Schild
    w.force_weather = Some("clear");
    w.weather = Default::default();
    run(&mut w, 60, idle());
    assert_eq!(w.road_warning(), None);
}

#[test]
fn statistics_track_distance_time_and_entering() {
    use berlin_sim::stats::{Stats, Tracker, track_step};
    let mut w = world(11);
    let (mut game, mut total, mut tr) = (Stats::default(), Stats::default(), Tracker::default());
    let step =
        |w: &mut World, input: Input, game: &mut Stats, total: &mut Stats, tr: &mut Tracker| {
            w.update(&input, DT);
            track_step(&mut [game, total], tr, w, DT);
        };
    for _ in 0..120 {
        step(
            &mut w,
            Input {
                move_x: 1.,
                ..idle()
            },
            &mut game,
            &mut total,
            &mut tr,
        );
    }
    assert!((game.get("timePlayed") - 2.).abs() < 1e-6);
    let foot = game.get("kmFoot");
    assert!(foot > 0.001 && foot < 0.03, "2 s joggen: {foot} km");
    assert_eq!(game.get("kmTotal"), foot);
    // ein Sprung (Neustart, Laden) zählt nicht als Strecke
    w.player.x += 5000.;
    step(&mut w, idle(), &mut game, &mut total, &mut tr);
    assert!(game.get("kmTotal") - foot < 0.001);
    // Einsteigen zählt einmal, Zeit im Auto läuft
    let pc = w.player_car_id.unwrap();
    let (x, y) = w.car(pc).map(|c| (c.x, c.y)).unwrap();
    (w.player.x, w.player.y) = (x + 15., y);
    step(
        &mut w,
        Input {
            move_x: 0.,
            ..idle()
        },
        &mut game,
        &mut total,
        &mut tr,
    );
    step(
        &mut w,
        Input {
            enter_exit: true,
            ..idle()
        },
        &mut game,
        &mut total,
        &mut tr,
    );
    for _ in 0..30 {
        step(&mut w, idle(), &mut game, &mut total, &mut tr);
    }
    assert_eq!(game.get("carsEntered"), 1.);
    assert!(game.get("timeCar") > 0.4);
    assert_eq!(game, total, "beide Stände bekommen dasselbe");
}

/// Passanten vor die Spielfigur stellen und ruhig halten (für Treffertests).
fn ped_in_front(w: &mut World, dist: f64) -> usize {
    let (x, y, lvl) = (w.player.x, w.player.y, w.player.level.lvl);
    let i = w
        .peds
        .iter()
        .position(|p| p.state != PedState::Dead && !berlin_sim::combat::is_fighter(p.id))
        .expect("Passant");
    let p = &mut w.peds[i];
    (p.x, p.y) = (x + dist, y);
    p.level.lvl = lvl;
    p.state = PedState::Idle;
    p.t = 99.;
    i
}

#[test]
fn shooting_and_melee_hurt_pedestrians() {
    use berlin_sim::combat::{CombatInput, Target, cast_ray};
    use berlin_sim::events::Event;
    let mut w = world(21);
    run(&mut w, 10, idle());
    // eine freie Schusslinie nach Osten suchen (die Figur steht an einer Straße)
    let lvl = w.player.level.lvl;
    let (mut ang, mut found) = (0., false);
    for k in 0..16 {
        let a = k as f64 * std::f64::consts::TAU / 16.;
        let (x, y) = (w.player.x, w.player.y);
        if cast_ray(&mut w, x, y, a, 80., lvl).hit.is_none() {
            ang = a;
            found = true;
            break;
        }
    }
    assert!(found, "keine freie Richtung");
    let i = ped_in_front(&mut w, 0.);
    let (x, y) = (w.player.x, w.player.y);
    (w.peds[i].x, w.peds[i].y) = (x + ang.cos() * 50., y + ang.sin() * 50.);
    let id = w.peds[i].id;
    assert_eq!(
        cast_ray(&mut w, x, y, ang, 200., lvl).hit,
        Some(Target::Ped(i))
    );
    // Pistole wählen und zielen
    let aim = CombatInput {
        aim_x: ang.cos(),
        aim_y: ang.sin(),
        ..Default::default()
    };
    run(
        &mut w,
        1,
        Input {
            combat: CombatInput {
                weapon_slot: 4,
                ..aim
            },
            ..idle()
        },
    );
    assert_eq!(w.player.combat.weapon().id, "pistol");
    run(&mut w, 15, idle());
    let mut shots = 0;
    for _ in 0..40 {
        w.update(
            &Input {
                combat: CombatInput {
                    fire: true,
                    fire_pressed: true,
                    ..aim
                },
                ..idle()
            },
            DT,
        );
        shots += w
            .events
            .iter()
            .filter(|e| matches!(e, Event::Shot { .. }))
            .count();
        if w.events
            .iter()
            .any(|e| matches!(e, Event::Kill { player: true, .. }))
        {
            break;
        }
        w.update(
            &Input {
                combat: aim,
                ..idle()
            },
            DT,
        );
    }
    let p = w.peds.iter().find(|p| p.id == id).expect("Passant noch da");
    assert_eq!(p.state, PedState::Dead, "drei Pistolentreffer à 34 töten");
    assert!((3..=6).contains(&shots), "{shots} Schüsse");
    assert_eq!(w.player.combat.mag[3], 12 - shots as u32);
    // Nahkampf: ein Faustschlag aus nächster Nähe trifft und lässt bluten
    let j = ped_in_front(&mut w, 0.);
    let (x, y) = (w.player.x, w.player.y);
    (w.peds[j].x, w.peds[j].y) = (x + ang.cos() * 14., y + ang.sin() * 14.);
    run(
        &mut w,
        1,
        Input {
            combat: CombatInput {
                weapon_slot: 1,
                ..aim
            },
            ..idle()
        },
    );
    run(&mut w, 20, idle());
    let hp = w.peds[j].hp;
    w.update(
        &Input {
            combat: CombatInput { fire: true, ..aim },
            ..idle()
        },
        DT,
    );
    assert!(w.events.iter().any(|e| matches!(
        e,
        Event::Swing {
            hit: true,
            npc: false,
            ..
        }
    )));
    assert!(w.events.iter().any(|e| matches!(e, Event::Blood { .. })));
    assert!(w.peds[j].hp <= hp - 20. + 1e-9, "Faust: 20 Schaden");
}

#[test]
fn knockout_brings_the_player_to_a_hospital() {
    use berlin_sim::combat::{PLAYER_HP, RESPAWN_DELAY, hurt_player};
    use berlin_sim::events::Event;
    let mut w = world(22);
    run(&mut w, 10, idle());
    w.money = 1000.;
    let (x0, y0) = (w.player.x, w.player.y);
    hurt_player(&mut w, 60., (x0 + 10., y0));
    assert_eq!(w.player.combat.hp, PLAYER_HP - 60.);
    hurt_player(&mut w, 60., (x0 + 10., y0));
    assert!(w.player.combat.dead);
    assert!(w.events.iter().any(|e| matches!(e, Event::Wasted { .. })));
    // liegt still, auch mit Eingaben
    run(
        &mut w,
        (RESPAWN_DELAY / DT) as usize - 5,
        Input {
            move_x: 1.,
            ..idle()
        },
    );
    assert_eq!((w.player.x, w.player.y), (x0, y0));
    let mut respawned = false;
    for _ in 0..600 {
        w.update(&idle(), DT);
        if w.events
            .iter()
            .any(|e| matches!(e, Event::Respawn { fee, .. } if *fee == 100.))
        {
            respawned = true;
            break;
        }
    }
    assert!(respawned, "nach dem K. o. im Krankenhaus aufgewacht");
    assert!(!w.player.combat.dead && w.player.combat.hp == PLAYER_HP);
    assert_eq!(w.money, 900., "10 % Krankenhausgebühr");
    let h = w.city.nearest_hospital(x0, y0).map(|h| (h.x, h.y)).unwrap();
    assert!(
        (w.player.x - h.0).hypot(w.player.y - h.1) < 1500.,
        "auf dem Gehweg beim Krankenhaus (es liegt oft mitten im Gelände)"
    );
    assert!(w.city.in_building(w.player.x, w.player.y).is_none());
}

#[test]
fn a_shot_at_car_makes_the_driver_flee() {
    use berlin_sim::car::Driver;
    let mut w = world(23);
    run(&mut w, 30, idle());
    let i = w
        .cars
        .iter()
        .position(|c| c.driver == Some(Driver::Npc))
        .expect("KI-Auto");
    let peds = w.peds.len();
    let (x, y) = (w.cars[i].x, w.cars[i].y);
    berlin_sim::combat::hurt_car(&mut w, i, 20., (x + 100., y));
    let id = w.cars[i].id;
    run(&mut w, 1, idle());
    let c = w.cars.iter().find(|c| c.id == id).unwrap();
    assert!(c.driver.is_none() && !c.wrecked, "Fahrer ist ausgestiegen");
    assert!(w.peds.len() > peds, "und rennt als Passant weg");
}

#[test]
fn mouse_aim_snaps_onto_the_target_under_the_cursor() {
    use berlin_sim::combat::{CombatInput, pick_target};
    let mut w = world(24);
    run(&mut w, 10, idle());
    let i = ped_in_front(&mut w, 40.);
    let (tx, ty) = (w.peds[i].x, w.peds[i].y);
    // knapp neben die Person gezeigt: rastet auf ihre Mitte ein
    assert_eq!(pick_target(&w, tx + 3., ty - 2.), Some((tx, ty)));
    assert_eq!(pick_target(&w, tx + 30., ty + 30.), None);
    let aim = CombatInput {
        aim_world: Some((tx + 3., ty - 2.)),
        ..Default::default()
    };
    run(
        &mut w,
        1,
        Input {
            combat: aim,
            ..idle()
        },
    );
    let want = (ty - w.player.y).atan2(tx - w.player.x);
    assert!(
        (w.player.combat.aim - want).abs() < 1e-9,
        "zielt genau auf die Mitte"
    );
    assert!(
        (w.player.angle - want).abs() < 1e-9,
        "die Figur schaut zum Zeiger"
    );
}

#[test]
fn foot_path_goes_around_buildings() {
    use berlin_sim::footpath::{find_foot_path, foot_free};
    let mut w = world(31);
    run(&mut w, 5, idle());
    let lvl = w.player.level.lvl;
    let from = (w.player.x, w.player.y);
    // Ziele jenseits eines Hauses (Gerade blockiert); Hinterhöfe ohne Zugang sind unerreichbar – dann endet der Weg
    // am nächsten erreichbaren Punkt, das prüfen alle Kandidaten, das Herumlaufen der erste erreichbare
    let mut around = None;
    for r in [250., 400., 600.] {
        for k in 0..24 {
            let a = k as f64 * std::f64::consts::TAU / 24.;
            let to = (from.0 + a.cos() * r, from.1 + a.sin() * r);
            if !foot_free(&mut w, to.0, to.1, lvl) {
                continue;
            }
            let blocked = (1..40).any(|s| {
                let t = s as f64 / 40.;
                w.city
                    .in_building(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)
                    .is_some()
            });
            if !blocked {
                continue;
            }
            let path = find_foot_path(&mut w, from, to, lvl).expect("Weg");
            assert_eq!(path[0], from);
            // kein Wegpunkt und keine Strecke durch ein Haus
            for seg in path.windows(2) {
                for s in 0..=20 {
                    let t = s as f64 / 20.;
                    let (x, y) = (
                        seg[0].0 + (seg[1].0 - seg[0].0) * t,
                        seg[0].1 + (seg[1].1 - seg[0].1) * t,
                    );
                    assert!(
                        w.city.in_building(x, y).is_none(),
                        "Weg durchs Haus bei {x},{y}"
                    );
                }
            }
            let end = *path.last().unwrap();
            if around.is_none() && end == to {
                around = Some(path.len());
            }
        }
    }
    let n = around.expect("mindestens ein Ziel hinter einem Haus ist erreichbar");
    assert!(n >= 3, "um das Haus herum braucht es Ecken");
}

#[test]
fn click_walks_attacks_and_enters() {
    use berlin_sim::world::Click;
    let mut w = world(32);
    run(&mut w, 5, idle());
    let lvl = w.player.level.lvl;
    // Klick auf freien Boden in der Nähe: die Figur läuft hin
    let from = (w.player.x, w.player.y);
    let to = (0..16)
        .map(|k| {
            let a = k as f64 * std::f64::consts::TAU / 16.;
            (from.0 + a.cos() * 120., from.1 + a.sin() * 120.)
        })
        .find(|&(x, y)| berlin_sim::footpath::foot_free(&mut w, x, y, lvl))
        .expect("freier Punkt");
    w.update(
        &Input {
            click_world: Some(to),
            click_pressed: true,
            click_held: true,
            ..idle()
        },
        DT,
    );
    assert!(matches!(w.player.click, Some(Click::Walk { .. })));
    run(&mut w, 600, idle());
    assert!(
        (w.player.x - to.0).hypot(w.player.y - to.1) < 8.,
        "angekommen"
    );
    assert!(w.player.click.is_none());
    // Klick auf eine Person: hinlaufen und zuschlagen (Fäuste)
    let i = ped_in_front(&mut w, 60.);
    let (px, py, id) = (w.peds[i].x, w.peds[i].y, w.peds[i].id);
    let hp = w.peds[i].hp;
    w.update(
        &Input {
            click_world: Some((px, py)),
            click_pressed: true,
            ..idle()
        },
        DT,
    );
    assert!(matches!(w.player.click, Some(Click::Target { ped, .. }) if ped == id));
    run(&mut w, 180, idle());
    let p = w.peds.iter().find(|p| p.id == id).unwrap();
    assert!(p.hp < hp, "getroffen");
    // Klick auf das eigene Auto in der Nähe (Doppelklick): hinlaufen, kurz an der Tür, einsteigen
    let pc = w.player_car_id.unwrap();
    let (cx, cy) = w.car(pc).map(|c| (c.x, c.y)).unwrap();
    w.update(
        &Input {
            click_world: Some((cx, cy)),
            click_pressed: true,
            click_double: true,
            ..idle()
        },
        DT,
    );
    assert!(
        matches!(
            w.player.click,
            Some(Click::Enter {
                approach: false,
                ..
            })
        ),
        "{:?}",
        w.player.click
    );
    run(&mut w, 1200, idle());
    assert_eq!(w.player.in_car, Some(pc), "eingestiegen");
}

#[test]
fn a_dead_body_calls_an_ambulance_that_takes_it_away() {
    use berlin_sim::services::Phase;
    let mut w = world(41);
    run(&mut w, 10, idle());
    let i = ped_in_front(&mut w, 30.);
    let id = w.peds[i].id;
    w.peds[i].state = PedState::Dead;
    let mut seen_siren = false;
    let mut on_scene = false;
    for _ in 0..(200. / DT) as usize {
        w.update(&idle(), DT);
        for c in w.cars.iter().filter(|c| c.kind == "ambulance") {
            seen_siren |= c.siren;
            on_scene |= c.duty.is_some_and(|d| d.phase == Phase::Scene) && c.blue;
        }
        if !w.peds.iter().any(|p| p.id == id) {
            break;
        }
    }
    assert!(seen_siren, "Rettungswagen mit Martinshorn alarmiert");
    assert!(on_scene, "hält am Einsatzort mit Blaulicht");
    assert!(!w.peds.iter().any(|p| p.id == id), "nimmt den Toten mit");
}

#[test]
fn a_gunshot_calls_the_police() {
    use berlin_sim::combat::CombatInput;
    let mut w = world(42);
    run(&mut w, 10, idle());
    run(
        &mut w,
        1,
        Input {
            combat: CombatInput {
                weapon_slot: 4,
                ..Default::default()
            },
            ..idle()
        },
    );
    run(&mut w, 20, idle());
    run(
        &mut w,
        1,
        Input {
            combat: CombatInput {
                fire: true,
                fire_pressed: true,
                ..Default::default()
            },
            ..idle()
        },
    );
    assert_eq!(
        w.emergency.incidents.len(),
        1,
        "Schuss meldet einen Einsatz"
    );
    // zweiter Schuss gleich danach: kein zweiter Streifenwagen (45 s Pause)
    run(&mut w, 20, idle());
    run(
        &mut w,
        1,
        Input {
            combat: CombatInput {
                fire: true,
                fire_pressed: true,
                ..Default::default()
            },
            ..idle()
        },
    );
    assert_eq!(w.emergency.incidents.len(), 1);
    run(&mut w, (12. / DT) as usize, idle());
    let police: Vec<_> = w
        .cars
        .iter()
        .filter(|c| c.kind == "police" && c.duty.is_some())
        .collect();
    assert_eq!(police.len(), 1, "ein Streifenwagen unterwegs");
    assert!(
        police[0].siren
            && police[0]
                .ai
                .as_ref()
                .is_some_and(|a| a.urgent && a.field.is_some())
    );
    // entsteht außer Sicht
    let p = police[0];
    assert!((p.x - w.camera.x).abs() > 1000. || (p.y - w.camera.y).abs() > 600.);
}

#[test]
fn traffic_mixes_vehicle_kinds_and_delivery_vans_stop() {
    use std::collections::BTreeSet;
    let mut w = world(51);
    w.clock = 10. * 60.; // Freitag, 10 Uhr: Lieferverkehr
    let mut kinds = BTreeSet::new();
    let mut hazard = false;
    for _ in 0..(240. / DT) as usize {
        w.update(&idle(), DT);
        for c in w.cars.iter().filter(|c| c.driver == Some(Driver::Npc)) {
            kinds.insert(c.kind);
            hazard |= c.hazard && c.ai.as_ref().is_some_and(|a| a.hold > 0.);
        }
        // für den Test die Bevölkerung erneuern lassen: Kamera wandert langsam die Straße entlang
    }
    eprintln!("Arten {kinds:?}, Warnblinker {hazard}");
    assert!(kinds.contains("car"));
    assert!(kinds.len() >= 3, "nur {kinds:?}");
    if kinds.contains("delivery") {
        assert!(hazard, "ein Paketwagen hält mit Warnblinker");
    }
    // ohne Tagesrhythmus nur Pkw
    let mut q = world(51);
    q.rhythm = false;
    q.services = false;
    q.reset_population();
    run(&mut q, 120, idle());
    assert!(
        q.cars
            .iter()
            .filter(|c| c.driver == Some(Driver::Npc))
            .all(|c| c.kind == "car")
    );
}
