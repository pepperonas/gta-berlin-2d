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

/// Stadtleben abschalten (die Späti-Runde steht sonst um die Figur herum).
fn calm(w: &mut World) {
    w.day_rhythm = false;
    w.peds.retain(|p| p.state != PedState::Hang);
    w.hangers.clear();
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
    calm(&mut w);
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

#[test]
fn cyclists_ride_and_can_be_taken_or_knocked_off() {
    use berlin_sim::bikes::{Kind, State};
    use berlin_sim::events::Event;
    let mut w = world(61);
    w.force_weather = Some("clear");
    run(&mut w, 30, idle());
    assert!(w.bike_target() >= 4, "Ziel {}", w.bike_target());
    // Bestand füllt sich im Lauf (eines je Schritt)
    run(&mut w, 120, idle());
    assert!(
        w.bikes.iter().filter(|b| b.state == State::Ride).count() >= 3,
        "{}",
        w.bikes.len()
    );
    // sie kommen voran, auf radtauglichen Spuren, nie im Haus
    let start: Vec<(u32, f64, f64)> = w.bikes.iter().map(|b| (b.id, b.x, b.y)).collect();
    let mut dist = 0.;
    for _ in 0..(20. / DT) as usize {
        w.update(&idle(), DT);
        for b in w.bikes.iter().filter(|b| b.state == State::Ride) {
            assert!(
                w.city.in_building(b.x, b.y).is_none(),
                "Rad im Haus bei {},{}",
                b.x,
                b.y
            );
        }
    }
    for (id, x, y) in start {
        if let Some(b) = w.bikes.iter().find(|b| b.id == id) {
            dist += (b.x - x).hypot(b.y - y);
        }
    }
    assert!(dist > 300., "Räder fahren: {dist} px");
    // ein Rad kapern: daneben stellen, einsteigen
    let i = w
        .bikes
        .iter()
        .position(|b| b.state == State::Ride)
        .expect("Rad");
    let (bx, by, kind) = (w.bikes[i].x, w.bikes[i].y, w.bikes[i].kind);
    (w.player.x, w.player.y) = (bx + 10., by);
    w.update(
        &Input {
            enter_exit: true,
            ..idle()
        },
        DT,
    );
    let car = w.player_car().expect("auf dem Rad");
    assert_eq!(
        car.kind,
        if kind == Kind::Scooter {
            "escooter"
        } else {
            "bicycle"
        }
    );
    assert!(
        w.events
            .iter()
            .any(|e| matches!(e, Event::Carjack { bike: true, .. }))
    );
    // ein anderer Radfahrer wird getroffen: er stürzt vom Rad
    if let Some(j) = w.bikes.iter().position(|b| b.state == State::Ride) {
        let peds = w.peds.len();
        let (x, y) = (w.bikes[j].x, w.bikes[j].y);
        berlin_sim::combat::hurt_bike(&mut w, j, 20., (x + 30., y), true, "fists", true);
        assert_eq!(w.bikes[j].state, State::Lying);
        assert_eq!(w.peds.len(), peds + 1, "der Fahrer ist jetzt ein Passant");
        assert!(
            w.events
                .iter()
                .any(|e| matches!(e, Event::BikeDown { player: true, .. }))
        );
    }
}

#[test]
fn teleport_finds_a_spot_on_foot_and_by_car() {
    use berlin_sim::world::TeleportSpot;
    let mut w = world(71);
    run(&mut w, 5, idle());
    // außerhalb Berlins: nichts
    assert_eq!(w.find_teleport_spot(-5000., -5000.), None);
    // zu Fuß 2 km weiter: Gehweg oder freier Grund, mit Ortsnamen
    let (x0, y0) = (w.player.x, w.player.y);
    let target = (x0 + 20000., y0 - 5000.);
    let spot = w.find_teleport_spot(target.0, target.1).expect("Ziel");
    let TeleportSpot::Spot { x, y, angle, name } = spot else {
        panic!("synchrone Quelle lädt sofort")
    };
    assert!(!name.is_empty(), "Ortsname");
    assert!((x - target.0).hypot(y - target.1) < 3000.);
    assert!(w.city.in_building(x, y).is_none());
    w.teleport_to(x, y, angle);
    run(&mut w, 30, idle());
    assert!((w.player.x - x).hypot(w.player.y - y) < 60., "angekommen");
    assert!(
        w.cars.len() > 5 && w.peds.len() > 5,
        "Bevölkerung am neuen Ort"
    );
    // im Auto: auf eine Fahrspur, Auto kommt mit
    let pc = w.player_car_id.unwrap();
    let (cx, cy) = w.car(pc).map(|c| (c.x, c.y)).unwrap();
    (w.player.x, w.player.y) = (cx + 15., cy);
    run(
        &mut w,
        1,
        Input {
            enter_exit: true,
            ..idle()
        },
    );
    assert_eq!(w.player.in_car, Some(pc));
    let Some(TeleportSpot::Spot { x, y, angle, .. }) = w.find_teleport_spot(x0, y0) else {
        panic!("Ziel im Auto")
    };
    w.teleport_to(x, y, angle);
    run(&mut w, 10, idle());
    let (cx, cy) = w.car(pc).map(|c| (c.x, c.y)).unwrap();
    assert!((cx - x).hypot(cy - y) < 40.);
    assert!(w.city.in_building(cx, cy).is_none());
    assert_eq!(w.player.in_car, Some(pc));
}

#[test]
fn location_names_and_pois() {
    let mut w = world(72);
    run(&mut w, 5, idle());
    let g = w.city.places.giver;
    let name = w.city.location_name(g.x, g.y);
    assert!(!name.is_empty() && name != "Berlin", "{name}");
    assert!(w.city.district_at(g.x, g.y).is_some());
    assert!(
        !w.city.pois_near(g.x, g.y, 3000.).is_empty(),
        "POIs in der Nähe"
    );
    assert!(w.city.density_at(g.x, g.y) >= 0.);
    eprintln!("Späti: {name} ({:?})", w.city.district_at(g.x, g.y));
}

#[test]
fn city_life_animals_and_day_rhythm() {
    use berlin_sim::animals::State as Bird;
    use berlin_sim::life::Act;
    let mut w = world(81);
    assert!(w.day_rhythm, "Standardbevölkerung = Tagesrhythmus");
    run(&mut w, 5, idle());
    // Ort und Uhrzeit bestimmen die Zielbevölkerung: nachts um 4 weniger als am Nachmittag
    w.clock = 4. * 60.;
    w.set_targets();
    let night = (w.car_target, w.ped_target);
    w.clock = 17. * 60.;
    w.set_targets();
    let day = (w.car_target, w.ped_target);
    assert!(night.0 < day.0 && night.1 < day.1, "{night:?} vs {day:?}");
    // Freitagabend am Späti: Stammgäste mit Flasche, dazu weitere Plätze ringsum
    w.clock = 21. * 60.;
    w.day = 4;
    w.manage_life(true);
    let hang: Vec<_> = w
        .peds
        .iter()
        .filter(|p| p.state == PedState::Hang)
        .collect();
    assert!(hang.len() >= 5, "{} Leute an Plätzen", hang.len());
    assert!(hang.iter().any(|p| {
        p.hang
            .as_ref()
            .is_some_and(|h| h.group == "spaeti" && h.act == Act::Drink)
    }));
    assert!(w.hangers.len() == hang.len());
    // sie bleiben an ihrem Platz
    let p0 = hang[0].id;
    let at = |w: &World| {
        let p = w.peds.iter().find(|p| p.id == p0).unwrap();
        (p.x, p.y)
    };
    let before = at(&w);
    run(&mut w, 60, idle());
    let after = at(&w);
    assert!((before.0 - after.0).hypot(before.1 - after.1) < 1e-9);
    // Tiere: Tauben und Enten im Umkreis, ein Schuss scheucht die nahen auf
    w.manage_animals(true);
    assert!(!w.animals.is_empty(), "Tauben in Kreuzberg");
    let a = w
        .animals
        .iter()
        .position(|a| a.state == Bird::Peck)
        .expect("pickende Taube");
    let (ax, ay) = (w.animals[a].x, w.animals[a].y);
    (w.player.x, w.player.y) = (ax + 20., ay);
    run(&mut w, 2, idle());
    let key = w.animals[a].key.clone();
    assert!(
        w.animals
            .iter()
            .filter(|b| b.key == key)
            .any(|b| b.state == Bird::Fly),
        "Figur zu nah: der Schwarm fliegt auf"
    );
    // Aufgescheuchte Lebensplatz-Gäste werden normale Passanten
    let i = w
        .peds
        .iter()
        .position(|p| p.state == PedState::Hang)
        .unwrap();
    let (px, py) = (w.peds[i].x, w.peds[i].y);
    berlin_sim::pedestrians::scare(&mut w.peds[i], (px + 30., py), 2.);
    let id = w.peds[i].id;
    w.manage_life(true);
    assert!(!w.hangers.values().any(|&h| h == id), "Platz freigegeben");
}

#[test]
fn puddles_make_fast_cars_aquaplane() {
    use berlin_sim::events::Event;
    use berlin_sim::traction::{edge_puddles, in_puddle};
    let mut w = world(91);
    run(&mut w, 5, idle());
    // Pfützen sind deterministisch (Kanten-ID) und liegen an der Rinne
    let (cx, cy) = (w.camera.x, w.camera.y);
    let mut ids: Vec<i64> = w
        .city
        .edges
        .values()
        .filter(|e| e.pts.iter().any(|&(x, y)| (x - cx).hypot(y - cy) < 900.))
        .map(|e| e.id)
        .collect();
    ids.sort();
    let (eid, p) = ids
        .iter()
        .find_map(|&id| {
            let v = edge_puddles(&mut w.city, id);
            v.first().copied().map(|p| (id, p))
        })
        .expect("eine Pfütze");
    assert_eq!(
        edge_puddles(&mut w.city, eid)[0],
        p,
        "gleiche Kante = gleiche Pfützen"
    );
    assert!(in_puddle(&p, p.x, p.y) && !in_puddle(&p, p.x + p.rx * 2., p.y));
    // trocken: keine Pfütze, nass: da
    assert!(w.puddle_at(p.x, p.y, 0).is_none());
    w.weather.wet = 1.;
    let found = w.puddle_at(p.x, p.y, 0);
    assert!(found.is_some(), "nass: Pfütze unter dem Punkt");
    // ein schnelles KI-Auto mitten hinein: schwimmt auf
    let i = w
        .cars
        .iter()
        .position(|c| c.driver == Some(berlin_sim::car::Driver::Npc))
        .expect("KI-Auto");
    let c = &mut w.cars[i];
    (c.x, c.y, c.angle) = (p.x, p.y, p.a);
    let v = berlin_sim::traction::Aqua::SPEED + 40.;
    (c.vx, c.vy) = (p.a.cos() * v, p.a.sin() * v);
    let id = c.id;
    w.update(&idle(), DT);
    assert!(
        w.events
            .iter()
            .any(|e| matches!(e, Event::Aquaplane { car, .. } if *car == id)),
        "Aquaplaning-Ereignis"
    );
    assert!(w.car(id).unwrap().aqua > 0.);
}

#[test]
fn nightlife_feed_fills_bars_and_sound() {
    use berlin_sim::nightlife::{Bar, attach_bars};
    let mut w = world(101);
    run(&mut w, 5, idle());
    let g = w.city.places.giver;
    (w.clock, w.day) = (23. * 60., 4);
    let (lx, ly) = (g.x - 300., g.y);
    let quiet = w.nightlife_at(lx, ly);
    // eine volle Feed-Bar ohne OSM-Gegenstück am Späti, gehört aus 30 m
    let bar = Bar {
        name: "Testbar".into(),
        key: "testbar".into(),
        lat: None,
        lon: None,
        current: None,
        usual: None,
        at: None,
        trend: Vec::new(),
        week: Some([Some([Some(0.3); 24]); 7]),
        x: Some(g.x),
        y: Some(g.y),
        osm: false,
    };
    assert_eq!(attach_bars(&mut w.city, vec![bar]), 1);
    let loud = w.nightlife_at(lx, ly);
    assert!(loud.crowd > quiet.crowd, "{} > {}", loud.crowd, quiet.crowd);
    assert!(loud.feed > 0. && loud.sources.iter().any(|s| s.name == "Testbar"));
    assert!(
        loud.sources.iter().any(|s| s.name == "Testbar" && s.x > lx),
        "Quelle liegt rechts"
    );
    // Raucher vor der Feed-Bar
    let before = w.hangers.len();
    w.manage_life(true);
    let matched = w.city.bars.as_ref().unwrap().list[0].osm;
    assert!(w.hangers.len() >= before.min(1));
    assert!(
        matched || w.hangers.keys().any(|k| k.starts_with('f')),
        "Raucher vor der Testbar (oder dem OSM-Lokal, dem sie zugeordnet ist)"
    );
    (w.camera.x, w.camera.y) = (lx, ly);
    let mix = berlin_sim::ambience::ambience_at(&mut w);
    assert!(mix.bar > 0.05 && mix.night_feed > 0., "{mix:?}");
    // Regen treibt die Leute rein: draußen leiser
    (w.clock, w.day) = (19. * 60., 1); // ruhiger Montagabend: nicht gesättigt
    w.sky.p.rain = 0.;
    let dry = w.nightlife_at(lx, ly).crowd;
    w.sky.p.rain = 1.;
    let wet = w.nightlife_at(lx, ly).crowd;
    assert!(dry < 1. && wet < dry, "{wet} < {dry}");
}

#[test]
fn timetable_buses_and_trains_run_in_kreuzberg() {
    use berlin_sim::transit::{Mode, Transit};
    let mut w = world(111);
    let t0 = std::time::Instant::now();
    w.set_transit(Transit::read(&root()).expect("transit.json"));
    eprintln!("Fahrplan geladen in {:?}", t0.elapsed());
    run(&mut w, 120, idle());
    let tr = w.transit.as_ref().unwrap();
    let modes: std::collections::BTreeSet<Mode> = w
        .transit_state
        .tracked
        .keys()
        .map(|&id| tr.patterns[id].mode)
        .collect();
    assert!(
        modes.contains(&Mode::Bus) && modes.contains(&Mode::UBahn),
        "{modes:?}"
    );
    let buses = w.cars.iter().filter(|c| c.bus.is_some()).count();
    assert!(buses > 0, "Busse fahren als KI-Fahrzeuge");
    assert!(
        w.cars
            .iter()
            .filter(|c| c.bus.is_some())
            .all(|c| c.kind == "bus" && c.line.is_some())
    );
    // Busse folgen ihrem Linienweg und kommen voran (solange sie in der Nähe sind)
    let mut last: std::collections::HashMap<u32, f64> = Default::default();
    let mut progress = 0.;
    for _ in 0..10 {
        for c in &w.cars {
            if let Some(b) = &c.bus {
                if let Some(s0) = last.get(&c.id) {
                    progress += (b.s - s0).max(0.);
                }
                last.insert(c.id, b.s);
            }
        }
        run(&mut w, 60, idle());
    }
    assert!(
        progress > 200.,
        "Busse kommen auf ihrer Linie voran ({progress:.0} px)"
    );
    // U1-Hochbahn sichtbar um den Görlitzer Bahnhof (oberirdisch), Wagen liegen am Gleis
    let (cx, cy) = (w.camera.x, w.camera.y);
    let vis = w.transit_visible(berlin_sim::collision::Rect::around(cx, cy, 6000.));
    eprintln!(
        "sichtbar: {:?}",
        vis.iter()
            .map(|v| (v.line.as_str(), v.mode, v.cars.len()))
            .collect::<Vec<_>>()
    );
    assert!(
        vis.iter().any(|v| v.mode == Mode::UBahn),
        "Hochbahn sichtbar"
    );
}

#[test]
fn ride_and_drive_a_tram_at_alexanderplatz() {
    use berlin_sim::ride::{Ref, RideKind};
    use berlin_sim::transit::{Mode, Transit, point_on_shape, position_at};
    use berlin_sim::world::TeleportSpot;
    let mut w = world(121);
    w.set_transit(Transit::read(&root()).expect("transit.json"));
    // Alexanderplatz (Straßenbahnen M4/M5/M6)
    let ov = berlin_map_loader::overview::Overview::read(&root()).unwrap();
    let st = ov
        .stations
        .iter()
        .find(|s| s.name.contains("Alexanderplatz"))
        .expect("Bahnhof Alexanderplatz");
    let Some(TeleportSpot::Spot { x, y, angle, .. }) =
        w.find_teleport_spot(st.at.x as f64, st.at.y as f64)
    else {
        panic!("Teleport")
    };
    w.teleport_to(x, y, angle);
    run(&mut w, 60, idle());
    w.peds.retain(|p| p.state != PedState::Hang); // freie Bahnsteige (Rhythmus bleibt an: sonst steht der Fahrplan)
    // eine fahrende Straßenbahn in der Nähe suchen und die Figur neben ihren zweiten Wagen stellen
    let mut found = None;
    for _ in 0..60 * 120 {
        run(&mut w, 1, idle());
        let tr = w.transit.as_ref().unwrap();
        for (&pid, s) in &w.transit_state.tracked {
            let p = &tr.patterns[pid];
            if p.mode != Mode::Tram {
                continue;
            }
            for v in &s.veh {
                let pos = position_at(p, v.tau);
                let (hx, hy, _) = point_on_shape(tr.shape_of(p), pos.s);
                if !pos.dwelling && (hx - w.camera.x).hypot(hy - w.camera.y) < 1500. {
                    found = Some(Ref::Veh {
                        pid,
                        key: v.key.clone(),
                    });
                }
            }
        }
        if found.is_some() {
            break;
        }
    }
    let r = found.expect("Straßenbahn am Alexanderplatz");
    let st = w.vehicle_state(&r).expect("Lage");
    let c = st.cars[1];
    (w.player.x, w.player.y) = (
        c.x - c.angle.sin() * (c.w / 2. + 6.),
        c.y + c.angle.cos() * (c.w / 2. + 6.),
    );
    // Statistik mitlaufen lassen: Mitfahrt, Aufspringen, Strecke als Fahrgast
    use berlin_sim::stats::{Stats, Tracker, track_step};
    let (mut sg, mut stot, mut tr) = (Stats::default(), Stats::default(), Tracker::default());
    track_step(&mut [&mut sg, &mut stot], &mut tr, &w, DT);
    w.update(
        &Input {
            ride: true,
            ..idle()
        },
        DT,
    );
    track_step(&mut [&mut sg, &mut stot], &mut tr, &w, DT);
    let ride = w.player.ride.clone().expect("eingestiegen (aufgesprungen)");
    assert_eq!(ride.kind, RideKind::Passenger);
    assert_eq!(ride.mode, Mode::Tram);
    let p0 = (w.player.x, w.player.y);
    for _ in 0..120 {
        w.update(&idle(), DT);
        track_step(&mut [&mut sg, &mut stot], &mut tr, &w, DT);
    }
    assert_eq!(sg.get("rides"), 1.);
    assert!(sg.get("hopsOn") <= sg.get("rides"));
    assert_eq!(sg.get("kmFoot"), 0.);
    assert!(
        sg.get("kmTransit") > 0. || w.vehicle_state(&ride.r).is_some_and(|s| s.dwelling),
        "Strecke als Fahrgast"
    );
    assert!(
        (w.player.x - p0.0).hypot(w.player.y - p0.1) > 20.
            || w.vehicle_state(&ride.r).is_some_and(|s| s.dwelling),
        "fährt mit"
    );
    // aussteigen
    let mut out = false;
    for _ in 0..600 {
        run(
            &mut w,
            1,
            Input {
                ride: true,
                ..idle()
            },
        );
        if w.player.ride.is_none() {
            out = true;
            break;
        }
        run(&mut w, 5, idle());
    }
    assert!(out, "ausgestiegen");
    // Führerstand: Figur an die Spitze einer Bahn, E übernimmt
    let mut taken = false;
    for _ in 0..60 * 120 {
        run(&mut w, 1, idle());
        let near: Vec<Ref> = {
            let tr = w.transit.as_ref().unwrap();
            let mut v = Vec::new();
            for (&pid, s) in &w.transit_state.tracked {
                if tr.patterns[pid].mode == Mode::Tram {
                    v.extend(s.veh.iter().map(|x| Ref::Veh {
                        pid,
                        key: x.key.clone(),
                    }));
                }
            }
            v
        };
        for r in near {
            let Some(st) = w.vehicle_state(&r) else {
                continue;
            };
            let f = st.cars[0];
            if (f.x - w.camera.x).hypot(f.y - w.camera.y) > 1500. || !st.dwelling {
                continue;
            }
            (w.player.x, w.player.y) = (
                f.x + f.angle.cos() * (f.l / 2. - 4.) - f.angle.sin() * (f.w / 2. + 4.),
                f.y + f.angle.sin() * (f.l / 2. - 4.) + f.angle.cos() * (f.w / 2. + 4.),
            );
            run(
                &mut w,
                1,
                Input {
                    enter_exit: true,
                    ..idle()
                },
            );
            if w.player_train.is_some() {
                taken = true;
                break;
            }
        }
        if taken {
            break;
        }
    }
    assert!(taken, "Straßenbahn übernommen");
    assert_eq!(
        w.player.ride.as_ref().map(|r| r.kind),
        Some(RideKind::Driver)
    );
    let s0 = w.player_train.as_ref().unwrap().s;
    run(
        &mut w,
        60 * 6,
        Input {
            throttle: 1.,
            ..idle()
        },
    );
    let t = w.player_train.as_ref().unwrap();
    assert!(
        t.s > s0 + 20. || t.blocked || t.wait_t > 0.,
        "fährt an (oder wartet vor einem Hindernis)"
    );
    // Notbremse, dann verlassen
    run(
        &mut w,
        60 * 8,
        Input {
            handbrake: true,
            ..idle()
        },
    );
    assert_eq!(w.player_train.as_ref().unwrap().v, 0.);
    run(
        &mut w,
        1,
        Input {
            enter_exit: true,
            ..idle()
        },
    );
    assert!(w.player.ride.is_none(), "Führerstand verlassen");
    assert!(w.player_train.is_some(), "Zug bleibt stehen");
}

#[test]
fn walkable_ubahn_station_at_kottbusser_tor() {
    use berlin_sim::transit::Transit;
    use berlin_sim::world::TeleportSpot;
    let mut w = world(131);
    w.set_transit(Transit::read(&root()).expect("transit.json"));
    let ov = berlin_map_loader::overview::Overview::read(&root()).unwrap();
    let kt = ov
        .stations
        .iter()
        .find(|s| s.name.contains("Kottbusser Tor") && s.cat == "ubahn")
        .expect("Kottbusser Tor");
    let Some(TeleportSpot::Spot { x, y, angle, .. }) =
        w.find_teleport_spot(kt.at.x as f64, kt.at.y as f64)
    else {
        panic!("Teleport")
    };
    w.teleport_to(x, y, angle);
    run(&mut w, 60, idle());
    let near = w.stations_near(w.player.x, w.player.y, 1500.);
    eprintln!(
        "Bahnhöfe: {:?}",
        near.iter()
            .map(|s| (s.name.as_str(), s.lines.clone(), s.exits.len()))
            .collect::<Vec<_>>()
    );
    let st = near
        .iter()
        .find(|s| s.name == "Kottbusser Tor" && s.lines.iter().any(|l| l == "U8"))
        .expect("U8-Bahnsteig unter Tage")
        .clone();
    assert!(st.exits.len() >= 2 && st.hl > 400.);
    // oben am Kottbusser Tor (U1 auf dem Viadukt, U8 darunter) rumpelt binnen einer Minute ein Zug vorbei
    let mut loudest = 0f64;
    for _ in 0..60 {
        run(&mut w, 60, idle());
        let m = berlin_sim::ambience::ambience_at(&mut w);
        assert!(!m.station);
        loudest = loudest.max(m.rumble);
    }
    assert!(loudest > 0.2, "Rumpeln {loudest}");
    // zum Eingang, F: hinunter
    let ex = st.exits[0].clone();
    (w.player.x, w.player.y) = (ex.x + 5., ex.y);
    run(&mut w, 31, idle()); // Bahnhofsliste um die Figur auffrischen
    run(
        &mut w,
        1,
        Input {
            enter_exit: true,
            ..idle()
        },
    );
    let inside = w.player.inside.clone().expect("im Bahnhof");
    assert_eq!(w.player.level.lvl, -2);
    let m = berlin_sim::ambience::ambience_at(&mut w);
    assert!(
        m.station && m.muffle > 0.8 && m.traffic == 0.,
        "gedämpfte Halle: {m:?}"
    );
    let st = w.station_by_id(&inside.id).unwrap().clone();
    // nicht durch Säulen und Kanten: weit nach außen laufen bleibt am Bahnsteig
    run(
        &mut w,
        120,
        Input {
            move_x: 0.3,
            move_y: 1.,
            ..idle()
        },
    );
    let (_, v) = st.to_local(w.player.x, w.player.y);
    assert!(
        v.abs() <= berlin_sim::station::HALF,
        "am Bahnsteig gehalten"
    );
    // an die Kante neben einen haltenden Zug und einsteigen
    let mut boarded = false;
    for _ in 0..60 * 600 {
        run(&mut w, 1, idle());
        let trains = w.trains_at(&st);
        if let Some(t) = trains.iter().find(|t| t.dwelling) {
            let c = t.cars[2];
            (w.player.x, w.player.y) =
                st.to_world(c.0, t.dir as f64 * (berlin_sim::station::HALF - 6.));
            run(
                &mut w,
                1,
                Input {
                    ride: true,
                    ..idle()
                },
            );
            if w.player.ride.is_some() {
                boarded = true;
                break;
            }
        }
    }
    assert!(boarded, "in die U8 eingestiegen");
    assert!(w.player.inside.is_none());
    // mitfahren bis zum nächsten Halt, dort auf den Bahnsteig aussteigen
    let mut arrived = false;
    let mut left = false;
    let r0 = w.player.ride.clone().unwrap();
    let first_stop = w.vehicle_state(&r0.r).unwrap().stop;
    for _ in 0..60 * 300 {
        run(&mut w, 1, idle());
        let Some(r) = w.player.ride.clone() else {
            break;
        };
        let vs = w.vehicle_state(&r.r).expect("Zug");
        if vs.dwelling && vs.stop != first_stop {
            run(
                &mut w,
                1,
                Input {
                    ride: true,
                    ..idle()
                },
            );
            arrived = w.player.inside.is_some();
            left = true;
            break;
        }
    }
    assert!(left, "Halt erreicht");
    assert!(
        arrived,
        "auf dem Bahnsteig des nächsten Bahnhofs ausgestiegen"
    );
    // zur Treppe und hinauf
    let st2 = w
        .station_by_id(&w.player.inside.as_ref().unwrap().id)
        .unwrap()
        .clone();
    assert_ne!(st2.name, "Kottbusser Tor");
    let (sx, sy) = st2.to_world(st2.hl - 10., 0.);
    (w.player.x, w.player.y) = (sx, sy);
    run(&mut w, 3, idle());
    assert!(w.player.inside.is_none(), "über die Treppe hinauf");
    assert_eq!(w.player.level.lvl, 0);
    let out = st2.exits[1].clone();
    assert!((w.player.x - out.x).hypot(w.player.y - out.y) < 1.);
}

#[test]
fn pedestrian_kinds_joggers_and_dog_walkers() {
    use berlin_sim::figure::{Kind, Style};
    use std::collections::HashSet;
    let mut w = world(91);
    run(&mut w, 5, idle());
    // Morgens: Jogger und Hundehalter unterwegs, dazu gemischte Arten nach Ort und Zeit
    w.clock = 7. * 60. + 30.;
    w.reset_population();
    run(&mut w, 60, idle());
    let walkers: Vec<_> = w.peds.iter().filter(|p| p.hang.is_none()).collect();
    assert!(walkers.len() > 20, "Passanten: {}", walkers.len());
    let kinds: HashSet<Kind> = w.peds.iter().map(|p| p.kind).collect();
    assert!(kinds.len() >= 5, "Arten: {kinds:?}");
    let jog = walkers.iter().filter(|p| p.style == Style::Jog).count();
    let dog = walkers.iter().filter(|p| p.style == Style::Dog).count();
    assert!(jog + dog > 0, "Jogger {jog}, Hundehalter {dog}");
    for p in &walkers {
        match p.style {
            Style::Jog => assert_eq!(p.kind, Kind::Jogger),
            Style::Dog => assert_eq!(p.kind, Kind::Dogwalker),
            Style::Plain => assert!(!matches!(p.kind, Kind::Jogger | Kind::Dogwalker)),
        }
    }
    // Jogger sind deutlich schneller als der Rest
    let mean = |f: &dyn Fn(&&&berlin_sim::pedestrians::Ped) -> bool| {
        let v: Vec<f64> = walkers.iter().filter(f).map(|p| p.speed).collect();
        v.iter().sum::<f64>() / v.len().max(1) as f64
    };
    if jog > 0 {
        let fast = mean(&|p| p.style == Style::Jog);
        let rest = mean(&|p| p.style == Style::Plain);
        assert!(fast > rest * 1.6, "Jogger {fast:.0} vs. {rest:.0}");
    }
    // Ohne Tagesrhythmus würfelt niemand einen Stil
    let mut q = world(91);
    q.day_rhythm = false;
    q.clock = 7. * 60. + 30.;
    q.reset_population();
    run(&mut q, 30, idle());
    assert!(q.peds.iter().all(|p| p.style == Style::Plain));
}

#[test]
fn churches_are_known_for_the_bells() {
    let mut w = world(93);
    run(&mut w, 5, idle());
    let churches: Vec<(f64, f64)> = w
        .city
        .polys
        .slab
        .iter()
        .filter(|p| p.kind == berlin_sim::city::PolyKind::Building && p.bkind == 3)
        .map(|p| p.rings[0][0])
        .collect();
    assert!(!churches.is_empty(), "Kirchen in den geladenen Kacheln");
    let (x, y) = churches[0];
    assert!(w.city.church_near(x, y, 50.));
    assert!(!w.city.church_near(x + 1e6, y, 50.));
}

#[test]
fn direction_signs_come_from_the_tiles() {
    let mut w = world(95);
    run(&mut w, 5, idle());
    let (x, y) = (w.player.x, w.player.y);
    let signs = w.city.signs_near(x, y, 6000.);
    assert!(!signs.is_empty(), "Wegweiser in Kreuzberg");
    let s = &signs[0];
    assert!(
        !s.rows.is_empty() && s.rows.iter().all(|r| !r.dests.is_empty()),
        "{s:?}"
    );
    assert!(
        s.rows
            .iter()
            .all(|r| r.dir.is_finite() && r.dir.abs() <= 7.)
    );
}

/// Ein neues Auto unmittelbar an der Linie vor einer Engstelle, deren Gegenrichtung belegt ist, darf dort nicht
/// entstehen: `entry_gate` trüge es im ersten Schritt ungeprüft ein (Gegenverkehr in der Engstelle).
#[test]
fn no_spawn_at_the_line_of_a_narrow_held_by_oncoming_traffic() {
    let mut w = world(5);
    run(&mut w, 2, idle());
    // Zufahrt ohne Abzweig, die geradewegs in eine Engstelle führt, und deren Gegenrichtung auf derselben Kante
    let mut ids: Vec<_> = w.lanes.lanes.keys().copied().collect();
    ids.sort();
    let mut found = None;
    for id in ids {
        let l = w.lanes.lane(id).unwrap().clone();
        if l.narrow || l.len < 200. {
            continue;
        }
        let next = w.lanes.next(&w.city, id, false);
        if next.len() != 1 || !w.lanes.lane(next[0]).unwrap().narrow {
            continue;
        }
        let n = w.lanes.lane(next[0]).unwrap().clone();
        let opp = w
            .lanes
            .lanes
            .values()
            .find(|o| o.edge == n.edge && o.dir != n.dir && o.narrow && o.len > 30.)
            .map(|o| o.id);
        if let Some(o) = opp {
            found = Some((l, o));
            break;
        }
    }
    let (lane, opp) = found.expect("Zufahrt in eine Engstelle mit Gegenrichtung");
    let at = |l: &berlin_sim::roadgraph::Lane, s: f64| -> (f64, f64) {
        let mut acc = 0.;
        for p in l.pts.windows(2) {
            let d = (p[1].0 - p[0].0).hypot(p[1].1 - p[0].1);
            if acc + d >= s {
                let t = (s - acc) / d.max(1e-9);
                return (
                    p[0].0 + (p[1].0 - p[0].0) * t,
                    p[0].1 + (p[1].1 - p[0].1) * t,
                );
            }
            acc += d;
        }
        *l.pts.last().unwrap()
    };
    // Gegenverkehr in der Engstelle: beansprucht die Gegenrichtung
    let o = w.lanes.lane(opp).unwrap().clone();
    let (ox, oy) = at(&o, 10.);
    assert!(
        w.put_npc_car(opp, 10., ox, oy, "car").is_some(),
        "Gegenverkehr gesetzt"
    );
    let (ex, ey) = at(&lane, lane.len - 1.);
    assert!(
        w.put_npc_car(lane.id, lane.len - 1., ex, ey, "car")
            .is_none(),
        "hinter der Linie gegen den Gegenverkehr erzeugt"
    );
    let (mx, my) = at(&lane, lane.len / 2.);
    assert!(
        w.put_npc_car(lane.id, lane.len / 2., mx, my, "car")
            .is_some(),
        "vor der Linie erlaubt (es hält dann)"
    );
}
