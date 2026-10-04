//! Autopilot (Port von `tests/helpers/bot.js`): spielt die ganze Mission über dieselben abstrakten Eingaben wie ein
//! Mensch – zu Fuß zum Auftraggeber, annehmen, zum Auto, mit A* über den echten Straßengraphen zur Lagerhalle,
//! einladen, zum Abgabeort, abliefern. Gefahren wird mit Pure Pursuit auf der rechten Fahrspur, vor Kurven wird
//! vorausschauend gebremst, Schleifen im Weg werden übersprungen, festgefahren setzt er zurück.
use berlin_sim::car::Car;
use berlin_sim::city::{City, DiskSource, Edge, Place};
use berlin_sim::mission::{LOAD_TIME, Outcome, State};
use berlin_sim::world::{DT, Input, World};
use std::collections::{BinaryHeap, HashMap, HashSet};

fn root() -> std::path::PathBuf {
    berlin_map_loader::default_data_root()
}
fn wrap(a: f64) -> f64 {
    berlin_sim::math::wrap_angle(a)
}

#[derive(Debug, Clone, Copy)]
struct P {
    x: f64,
    y: f64,
}

fn usable(e: &Edge) -> bool {
    e.inside && e.cls <= 9 && !(e.cls == 9 && e.w < 40.) && !e.blocked && !e.passage
}

/// Polylinie seitlich versetzen (positiv = rechts in Laufrichtung, y nach unten)
fn offset(pts: &[(f64, f64)], d: f64) -> Vec<(f64, f64)> {
    if d == 0. || pts.len() < 2 {
        return pts.to_vec();
    }
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (a, b) = (pts[i.saturating_sub(1)], pts[(i + 1).min(n - 1)]);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let l = dx.hypot(dy).max(1e-9);
            (pts[i].0 - dy / l * d, pts[i].1 + dx / l * d)
        })
        .collect()
}

#[derive(PartialEq)]
struct Open(f64, i64);
impl Eq for Open {}
impl PartialOrd for Open {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Open {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
    }
}

/// Wegpunkte von `from` nach `to`: Einmündung auf die nächste Straße, Straßenzug auf der rechten Spur, Ziel.
fn route(city: &mut City, from: P, to: P) -> Vec<P> {
    let a = city
        .nearest_edge(from.x, from.y, 800., usable)
        .expect("Start an einer Straße");
    let b = city
        .nearest_edge(to.x, to.y, 800., usable)
        .expect("Ziel an einer Straße");
    let ea = city.edges[&a.edge].clone();
    let eb = city.edges[&b.edge].clone();
    let goals: HashSet<i64> = [eb.a, eb.b].into();
    let nd = |c: &City, k: i64| {
        let n = &c.nodes[&k];
        (n.x, n.y)
    };
    let mut g: HashMap<i64, f64> = HashMap::new();
    let mut prev: HashMap<i64, Option<(i64, i64)>> = HashMap::new();
    let mut open = BinaryHeap::new();
    for s in [ea.a, ea.b] {
        let (x, y) = nd(city, s);
        let d = (x - from.x).hypot(y - from.y);
        g.insert(s, d);
        prev.insert(s, None);
        open.push(Open(d, s));
    }
    let mut goal = None;
    while let Some(Open(_, u)) = open.pop() {
        if goals.contains(&u) {
            goal = Some(u);
            break;
        }
        let edges = city.nodes[&u].edges.clone();
        for k in edges {
            let Some(e) = city.edges.get(&k).filter(|e| usable(e)) else {
                continue;
            };
            let v = if e.a == u { e.b } else { e.a };
            if !city.nodes.contains_key(&v) {
                continue;
            }
            let dv = g[&u] + e.len;
            if dv < g.get(&v).copied().unwrap_or(f64::INFINITY) {
                g.insert(v, dv);
                prev.insert(v, Some((u, k)));
                let (x, y) = nd(city, v);
                open.push(Open(dv + (x - to.x).hypot(y - to.y), v));
            }
        }
    }
    let goal = goal.expect("keine Route");
    let mut legs = Vec::new();
    let mut cur = goal;
    while let Some(Some((u, e))) = prev.get(&cur) {
        legs.push((*u, *e));
        cur = *u;
    }
    legs.reverse();
    let mut pts: Vec<P> = Vec::new();
    for (u, k) in legs {
        let e = &city.edges[&k];
        let fwd = e.a == u;
        let mut p = e.pts.clone();
        if !fwd {
            p.reverse();
        }
        // rechte Spur der eigenen Richtung laut Querschnitt (wie die KI)
        let lo = berlin_sim::roadgraph::lane_offsets(&e.cs, city.scale);
        let lanes: Vec<f64> = if fwd {
            lo.fwd
        } else {
            lo.bwd.iter().map(|o| -o).collect()
        };
        // etwas zur Mitte hin: halb auf dem Gehweg geparkte Autos ragen in die rechte Spur
        let off = lanes.last().copied().map_or(0., |o| o - o.signum() * 5.);
        for (x, y) in offset(&p, off) {
            pts.push(P { x, y });
        }
    }
    // Doppelpunkte weg, lange Stücke alle 25 px unterteilen
    let mut out: Vec<P> = Vec::new();
    for p in pts {
        if let Some(q) = out.last().copied() {
            let d = (p.x - q.x).hypot(p.y - q.y);
            if d <= 3. {
                continue;
            }
            let n = (d / 25.) as usize;
            for k in 1..n {
                let t = k as f64 / n as f64;
                out.push(P {
                    x: q.x + (p.x - q.x) * t,
                    y: q.y + (p.y - q.y) * t,
                });
            }
        }
        out.push(p);
    }
    // Haarnadeln entfernen (versetzte Spuren stoßen an Kreuzungen aneinander)
    loop {
        let mut changed = false;
        for i in 1..out.len().saturating_sub(1) {
            let (a, b, c) = (out[i - 1], out[i], out[i + 1]);
            let t1 = (b.y - a.y).atan2(b.x - a.x);
            let t2 = (c.y - b.y).atan2(c.x - b.x);
            if wrap(t2 - t1).abs() > 2. {
                out.remove(i);
                changed = true;
                break;
            }
        }
        if !changed {
            break;
        }
    }
    out.insert(0, P { x: a.x, y: a.y });
    out.push(P { x: b.x, y: b.y });
    out.push(to);
    out
}

#[derive(Default)]
struct Opts {
    stop: bool,
    next: Option<P>,
    reverse_at: Option<f64>,
    cruise: f64,
    cap: Option<f64>,
}

/// Zum Ziel lenken (Pure Pursuit: Krümmung 2·sin(Winkel)/Abstand, Lenkwinkel über den Radstand) und Tempo halten.
fn drive_to(car: &Car, t: P, input: &mut Input, o: &Opts) -> f64 {
    let (dx, dy) = (t.x - car.x, t.y - car.y);
    let d = dx.hypot(dy);
    let diff = wrap(dy.atan2(dx) - car.angle);
    let sp = car.speed();
    let spec = berlin_sim::carmodels::spec_of(car.model_name());
    let wb = spec.wb * 10.;
    let kappa = 2. * diff.sin() / d.max(20.);
    input.steer = if diff.abs() > std::f64::consts::FRAC_PI_2 {
        diff.signum()
    } else {
        ((kappa * wb).atan() / (spec.steer_max * berlin_sim::dynamics::FUN_STEER)).clamp(-1., 1.)
    };
    let mut want = if o.stop {
        ((d - 6.) * 1.2).clamp(0., 200.)
    } else if diff.abs() > 0.7 {
        70.
    } else {
        o.cruise
    };
    if let Some(n) = o.next.filter(|_| !o.stop) {
        let turn = wrap((n.y - t.y).atan2(n.x - t.x) - dy.atan2(dx)).abs();
        if turn > 0.35 {
            want = want.min(60. + d * 0.9 - turn * 20.);
        }
    }
    if let Some(c) = o.cap {
        want = want.min(c);
    }
    if sp < want {
        input.throttle = 1.;
        input.brake = 0.;
    } else {
        input.throttle = 0.;
        input.brake = if sp - want > 30. || o.cap.is_some_and(|c| sp > c + 6.) {
            1.
        } else {
            0.3
        };
    }
    let rev = o.reverse_at.unwrap_or(if d < 60. { 1.8 } else { 2.4 });
    if diff.abs() > rev {
        input.throttle = 0.;
        input.brake = 1.;
        input.steer = -input.steer;
    }
    d
}

/// Tempo, mit dem man die Kurven voraus (bis 70 m) mit 4,5 m/s² noch schafft: √(v_Kurve² + 2·a·Abstand).
fn corner_cap(car: &Car, pts: &[P], i0: usize) -> f64 {
    const BRAKE_PX: f64 = 45.;
    let first = pts[(i0 + 1).min(pts.len() - 1)];
    let mut dist = (first.x - car.x).hypot(first.y - car.y);
    let mut cap = f64::INFINITY;
    let mut i = i0 + 1;
    while i + 1 < pts.len() && dist < 700. {
        let (a, b, c) = (pts[i - 1], pts[i], pts[i + 1]);
        let turn = wrap((c.y - b.y).atan2(c.x - b.x) - (b.y - a.y).atan2(b.x - a.x)).abs();
        if turn > 0.3 {
            let vc = if turn > 1.2 {
                45.
            } else if turn > 0.7 {
                75.
            } else {
                120.
            };
            cap = cap.min((vc * vc + 2. * BRAKE_PX * dist).sqrt());
        }
        dist += (c.x - b.x).hypot(c.y - b.y);
        i += 1;
    }
    cap
}

#[derive(Default)]
struct Follow {
    i: usize,
    back: f64,
    back_steer: f64,
    stuck: f64,
    last_i: usize,
    same_t: f64,
}

/// Folgt der Wegpunktliste (Zielpunkt voraus auf der Linie) und hält am letzten Punkt; true = steht am Ziel.
fn follow_route(car: &Car, pts: &[P], input: &mut Input, st: &mut Follow) -> bool {
    // auf das Segment weiterschalten, auf dem das Auto gerade ist
    while st.i + 2 < pts.len() {
        let (a, b) = (pts[st.i], pts[st.i + 1]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let l2 = (dx * dx + dy * dy).max(1e-9);
        if ((car.x - a.x) * dx + (car.y - a.y) * dy) / l2 < 1. {
            break;
        }
        st.i += 1;
    }
    // Schleifen und Zacken überspringen: ist ein späterer Punkt schon nah, dorthin
    let top = (pts.len() - 2).min(st.i + 8);
    for j in (st.i + 1..=top).rev() {
        if (pts[j].x - car.x).hypot(pts[j].y - car.y) < 22. {
            st.i = j;
            break;
        }
    }
    let (a, b) = (pts[st.i], pts[(st.i + 1).min(pts.len() - 1)]);
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l = dx.hypot(dy).max(1e-9);
    let t = (((car.x - a.x) * dx + (car.y - a.y) * dy) / (l * l)).max(0.);
    let mut rest = 36. + car.speed() * 0.2;
    let (mut px, mut py) = (a.x + dx * t.min(1.), a.y + dy * t.min(1.));
    let mut k = st.i + 1;
    let mut aim = P { x: px, y: py };
    while k < pts.len() {
        let seg = (pts[k].x - px).hypot(pts[k].y - py);
        if seg >= rest {
            aim = P {
                x: px + (pts[k].x - px) * rest / seg,
                y: py + (pts[k].y - py) * rest / seg,
            };
            break;
        }
        rest -= seg;
        (px, py) = (pts[k].x, pts[k].y);
        aim = pts[k];
        k += 1;
    }
    let goal = pts[pts.len() - 1];
    let d_goal = (goal.x - car.x).hypot(goal.y - car.y);
    let last = k >= pts.len() || d_goal < 60.;
    let next = pts[(k + 2).min(pts.len() - 1)];
    drive_to(
        car,
        if last { goal } else { aim },
        input,
        &Opts {
            stop: last,
            next: (!last).then_some(next),
            reverse_at: (!last).then_some(2.6),
            cruise: 200.,
            cap: (!last).then(|| corner_cap(car, pts, st.i)),
        },
    );
    // festgefahren: kurz mit Gegenlenkung zurück
    let sp = car.speed();
    if st.back > 0. {
        st.back -= DT;
        input.throttle = 0.;
        input.brake = 1.;
        input.handbrake = false;
        input.steer = -st.back_steer;
    } else if input.throttle > 0. && sp < 8. {
        st.stuck += DT;
        if st.stuck > 1.2 {
            st.back = 1.;
            st.back_steer = if input.steer != 0. { input.steer } else { 1. };
            st.stuck = 0.;
        }
    } else if sp > 20. {
        st.stuck = 0.;
    }
    // Kreisen ohne Fortschritt: nach 4 s auf demselben Wegstück zurücksetzen
    if st.i != st.last_i {
        st.last_i = st.i;
        st.same_t = 0.;
    } else if !last && st.back <= 0. {
        st.same_t += DT;
        if st.same_t > 4. {
            st.back = 1.5;
            st.back_steer = -(if input.steer != 0. { input.steer } else { 1. });
            st.same_t = 0.;
        }
    }
    last && d_goal < 20. && sp < 10.
}

fn step(w: &mut World, i: Input) {
    w.update(&i, DT);
}
fn walk_to(w: &mut World, t: P, secs: f64) -> bool {
    for _ in 0..(secs * 60.) as usize {
        let (dx, dy) = (t.x - w.player.x, t.y - w.player.y);
        let d = dx.hypot(dy);
        if d < 6. {
            return true;
        }
        step(
            w,
            Input {
                move_x: dx / d,
                move_y: dy / d,
                ..Input::default()
            },
        );
    }
    false
}
fn drive_route(w: &mut World, target: P, secs: f64) -> bool {
    let id = w.player.in_car.expect("im Auto");
    let car = w.car(id).unwrap().clone();
    let pts = route(&mut w.city, P { x: car.x, y: car.y }, target);
    let mut st = Follow::default();
    for _ in 0..(secs * 60.) as usize {
        let mut inp = Input::default();
        let car = w.car(id).unwrap().clone();
        if follow_route(&car, &pts, &mut inp, &mut st) {
            return true;
        }
        step(w, inp);
        if std::env::var("BOT_DBG").is_ok() {
            let others: Vec<u32> = w
                .events
                .iter()
                .filter_map(|e| match e {
                    berlin_sim::events::Event::Crash { car, .. } if *car != id => Some(*car),
                    _ => None,
                })
                .collect();
            for other in others {
                let Some(o) = w.car(other).cloned() else {
                    continue;
                };
                let ne = w.city.nearest_edge(o.x, o.y, 300., |_| true);
                eprintln!(
                    "gegen {:?} Art {} Tempo {:.0}, Abstand zur Straßenmitte {:.0} (Breite {:.0}), Bot-Wegpunkt {}",
                    o.role,
                    o.kind,
                    o.speed(),
                    ne.as_ref().map_or(-1., |n| n.d),
                    ne.as_ref().map_or(-1., |n| w.city.edges[&n.edge].w),
                    st.i
                );
            }
        }
    }
    false
}
fn p(pl: &Place) -> P {
    P { x: pl.x, y: pl.y }
}

#[test]
fn autopilot_plays_the_whole_mission() {
    let city = City::open(&root(), Box::new(DiskSource::new(root()))).expect("Karte");
    // ohne Verkehr wie im Browser-Test: der Bot misst die Fahrt, nicht das Ausweichen
    let mut w = World::new(city, 7, 0, 0);
    let pl = w.city.places.clone();
    // das ganze Missionsgebiet laden und halten (A* braucht den Graphen zwischen den Orten)
    let xs = [pl.giver.x, pl.pickup.x, pl.dropoff.x, pl.player_car.x];
    let ys = [pl.giver.y, pl.pickup.y, pl.dropoff.y, pl.player_car.y];
    let (x0, x1) = (
        xs.iter().cloned().fold(f64::MAX, f64::min),
        xs.iter().cloned().fold(f64::MIN, f64::max),
    );
    let (y0, y1) = (
        ys.iter().cloned().fold(f64::MAX, f64::min),
        ys.iter().cloned().fold(f64::MIN, f64::max),
    );
    w.city
        .load_area(x0 - 3000., y0 - 3000., x1 + 3000., y1 + 3000., true);
    // annehmen
    assert!(
        walk_to(&mut w, p(&pl.giver), 30.),
        "Auftraggeber nicht erreicht"
    );
    step(&mut w, Input::default());
    assert_eq!(w.mission.prompt, Some("A: Auftrag annehmen"));
    step(
        &mut w,
        Input {
            action: true,
            ..Input::default()
        },
    );
    step(&mut w, Input::default());
    step(
        &mut w,
        Input {
            action: true,
            ..Input::default()
        },
    );
    assert_eq!(w.mission.state, State::ToPickup);
    // zum eigenen Auto und einsteigen
    let pc = w.player_car_id.expect("eigenes Auto");
    let c = w.car(pc).unwrap().clone();
    assert!(
        walk_to(
            &mut w,
            P {
                x: c.x - 30. * c.angle.cos(),
                y: c.y - 30. * c.angle.sin()
            },
            40.
        ),
        "Auto nicht erreicht"
    );
    step(
        &mut w,
        Input {
            enter_exit: true,
            ..Input::default()
        },
    );
    assert_eq!(w.player.in_car, Some(pc));
    // zur Lagerhalle, anhalten, einladen
    assert!(
        drive_route(&mut w, p(&pl.pickup), 600.),
        "Lagerhalle nicht erreicht ({:?}, {:.0} s übrig)",
        w.mission.state,
        w.mission.timer
    );
    for _ in 0..((LOAD_TIME + 0.2) * 60.) as usize {
        step(
            &mut w,
            Input {
                action_held: true,
                ..Input::default()
            },
        );
    }
    assert_eq!(w.mission.state, State::ToDropoff, "{:?}", w.mission.prompt);
    assert!(w.car(pc).unwrap().cargo);
    // zum Abgabeort und abliefern
    let ok = drive_route(&mut w, p(&pl.dropoff), 600.);
    let car = w.car(pc).unwrap();
    assert!(
        ok,
        "Abgabeort nicht erreicht ({:?}, {:?}, {:.0} s übrig, Auto {:.0}/{:.0} Gesundheit {:.0})",
        w.mission.state, w.mission.result, w.mission.timer, car.x, car.y, car.health
    );
    step(&mut w, Input::default());
    step(
        &mut w,
        Input {
            action: true,
            ..Input::default()
        },
    );
    assert_eq!(w.mission.state, State::Success, "{:?}", w.mission.prompt);
    let Some(Outcome::Success { time, reward, .. }) = w.mission.result.clone() else {
        panic!("kein Ergebnis")
    };
    assert!(reward >= 100.);
    let limit = pl.time_limit.unwrap();
    assert!(time < limit, "Zeit {time:.1} s von {limit} s");
    eprintln!("Bot-Missionszeit {time:.1} s von {limit} s, Belohnung {reward} €");
}
