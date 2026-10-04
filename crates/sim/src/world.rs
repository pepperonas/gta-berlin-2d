//! Spielwelt (Port der Kernschleife von `world.js`): verbindet Stadt, Spieler, Autos, Passanten und Mission zu
//! einem deterministischen Simulationsschritt. Eingaben kommen als abstrakter Zustand ([`Input`]), Ausgaben als
//! Ereignisse ([`Event`]). Der feste Schritt ist [`DT`].
//!
//! Noch nicht portiert (spätere Phasen): Waffen/Nahkampf, Tagesrhythmus und Wetterverlauf, Räder/Tiere/Linienverkehr,
//! Aufenthaltsorte, Einsatzfahrzeuge, U-Bahnhöfe, Klick-Steuerung.
use crate::car::{self, Car, Driver, Knocked, Role, collide_car_world, collide_cars, step_car};
use crate::carmodels::{CAR_COLORS, is_open_kind};
use crate::city::{City, Ground, Solid, point_along};
use crate::collision::{
    Grid, Obb, Rect, circle_vs_circle, circle_vs_obb, circle_vs_rect, circle_vs_segment,
    obb_vs_rect, obb_vs_segment,
};
use crate::events::Event;
use crate::levels::{initial_level, step_level, touch};
use crate::math::{Rng, damp, hash01};
use crate::mission::{Mission, MissionInput, PlayerView, State};
use crate::pedestrians::{self, MovingCar, Ped, PedCtx, PedState, Sidewalks, create_ped, scare};
use crate::roadgraph::{LaneGraph, parking_strip};
use crate::save::{SaveData, SavedCar, SavedPoint};
use crate::traction::{GroundWeather, road_condition, traction_of};
use crate::traffic::{
    Agent, Ctx, Reservations, Walker, claim_narrow, drive_ai, drop_claims, narrow_free,
    place_on_lane,
};
use std::collections::HashSet;

pub const DT: f64 = 1. / 60.;
pub const PLAYER_RADIUS: f64 = 7.;
pub const WALK: f64 = 15.;
pub const JOG: f64 = 35.;
pub const SPRINT: f64 = 70.;
pub const ENTER_DIST: f64 = 40.;
pub const STAMINA_DRAIN: f64 = 12.;
pub const STAMINA_RECOVER: f64 = 20.;
pub const STAMINA_PAUSE: f64 = 1.;
pub const STAMINA_AGAIN: f64 = 0.25;
pub const TRAFFIC_CARS: usize = 22;
pub const TRAFFIC_PEDS: usize = 55;
pub const SPAWN_MIN: f64 = 750.;
pub const SPAWN_MAX: f64 = 1800.;
pub const DESPAWN: f64 = 2400.;
pub const PARKED_SHARE: f64 = 0.6;
pub const PARKED_RADIUS: f64 = 1300.;
pub const PARKED_DESPAWN: f64 = 1800.;
pub const CLOCK_START: f64 = 16. * 60.;
pub const START_DAY: u32 = 4;
pub const FOOT_ZOOM: f64 = 2.;
pub const GRID_CELL: f64 = 128.;
pub const GRID_REACH: f64 = 90.;
pub const PED_HP: f64 = 100.;
pub const VEH_INFO_S: f64 = 4.5;

/// Abstrakte Eingabe eines Schritts (wie `idle.js`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Input {
    pub move_x: f64,
    pub move_y: f64,
    pub sprint: bool,
    pub walk_slow: bool,
    pub throttle: f64,
    pub brake: f64,
    pub steer: f64,
    pub handbrake: bool,
    pub horn: bool,
    /// Ein-/Aussteigen (Flanke)
    pub enter_exit: bool,
    /// Aktion (Flanke) und gehalten
    pub action: bool,
    pub action_held: bool,
    pub esp_toggle: bool,
    pub abs_toggle: bool,
    /// Kampf (nur zu Fuß wirksam)
    pub combat: crate::combat::CombatInput,
    /// Klicksteuerung zu Fuß (Diablo-Schema): Zeigerpunkt, gedrückt/gehalten, mit Strg, Doppelklick
    pub click_world: Option<(f64, f64)>,
    pub click_pressed: bool,
    pub click_held: bool,
    pub click_force: bool,
    pub click_double: bool,
}

/// Laufender Klickauftrag der Spielfigur (world.js `p.click`).
#[derive(Debug, Clone, PartialEq)]
pub enum Click {
    /// Weg abarbeiten; `follow` = beim Halten dem Zeiger nachlaufen
    Walk {
        path: Vec<(f64, f64)>,
        i: usize,
        follow: bool,
    },
    /// zur Person laufen und angreifen (`done` = schon ein Angriff)
    Target { ped: u32, done: bool },
    /// zum Auto laufen; `approach` = nur danebenstellen, sonst kurz an der Tür und einsteigen
    Enter {
        car: u32,
        approach: bool,
        path: Vec<(f64, f64)>,
        i: usize,
        to: (f64, f64),
        door: Option<f64>,
    },
    /// mit Strg: am Platz angreifen, wohin gezeigt wird (bzw. auf die Person darunter)
    Force { at: (f64, f64), ped: Option<u32> },
}
/// Klick: an der Tür stehen (s), Doppelklick-Fenster (s), Annäherung über die Einsteigweite hinaus (px).
pub const CLICK_DOOR: f64 = 0.35;
pub const CLICK_NEAR_CAR: f64 = 30.;

#[derive(Debug, Clone)]
pub struct Player {
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub in_car: Option<u32>,
    pub step: f64,
    pub stun: f64,
    pub stamina: f64,
    pub tired: bool,
    pub rest: f64,
    pub swimming: bool,
    pub move_speed: f64,
    pub level: crate::levels::LevelState,
    pub level_init: bool,
    pub combat: crate::combat::Combat,
    pub click: Option<Click>,
    pub click_t: f64,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub text: String,
    pub t: f64,
}

/// Stellplatz: Schlüssel (Kante, Seite, Nummer), Lage, Ausrichtung.
pub type Slot = ((i64, i8, usize), f64, f64, f64);

pub struct World {
    pub city: City,
    pub lanes: LaneGraph,
    pub sidewalks: Sidewalks,
    pub rng: Rng,
    pub cars: Vec<Car>,
    pub peds: Vec<Ped>,
    pub events: Vec<Event>,
    pub time: f64,
    pub clock: f64,
    pub day: u32,
    pub day_count: u32,
    /// Boden: Nässe, Schneedecke, Glätte (0…1)
    pub weather: GroundWeather,
    /// Himmel: aktuelles Wetterbild aus weather.rs
    pub sky: crate::weather::Weather,
    /// Temperatur in °C
    pub temp: f64,
    /// erzwungenes Wetterbild (Befehlszeile/Taste) statt des Tagesverlaufs
    pub force_weather: Option<&'static str>,
    /// Wetter folgt dem Tagesverlauf (sonst bleibt es klar und der Boden trocken)
    pub weather_cycle: bool,
    pub seed: u32,
    pub player: Player,
    pub player_car_id: Option<u32>,
    pub mission: Mission,
    pub money: f64,
    pub completed: f64,
    pub best_time: Option<f64>,
    pub camera: Camera,
    pub res: Reservations,
    pub knocked: Knocked,
    pub esp: bool,
    pub abs: bool,
    pub notice: Option<Notice>,
    /// Name und Technik nach dem Einsteigen (Auto-ID, Sekunden)
    pub veh_info: Option<(u32, f64)>,
    pub car_target: usize,
    pub ped_target: usize,
    pub loading: bool,
    populated: bool,
    pending_save: Option<SaveData>,
    parked_keys: HashSet<(i64, i8, usize)>,
    park_tick: u64,
    next_car: u32,
    next_ped: u32,
    focus_key: String,
}

fn spot_free_static(city: &mut City, knocked: &Knocked, x: f64, y: f64, r: f64, lvl: i8) -> bool {
    for h in city.solids.query(&Rect::around(x, y, r)) {
        let s = *city.solids.get(h);
        if !car::blocks(knocked, &s, lvl) {
            continue;
        }
        let m = match s {
            Solid::Wall { seg, .. } => circle_vs_segment(x, y, r, &seg),
            Solid::Circle {
                x: cx,
                y: cy,
                r: cr,
                ..
            } => circle_vs_circle(x, y, r, cx, cy, cr),
            Solid::Rect(rc) => circle_vs_rect(x, y, r, &rc),
        };
        if m.is_some() {
            return false;
        }
    }
    true
}

impl World {
    /// Neue Welt am Missionsort. `cars`/`peds`: Zielbevölkerung um die Kamera.
    pub fn new(city: City, seed: u32, cars: usize, peds: usize) -> Self {
        let mut w = Self {
            city,
            lanes: LaneGraph::default(),
            sidewalks: Sidewalks::default(),
            rng: Rng::new(seed.wrapping_add(7)),
            cars: Vec::new(),
            peds: Vec::new(),
            events: Vec::new(),
            time: 0.,
            clock: CLOCK_START,
            day: START_DAY,
            day_count: 0,
            weather: GroundWeather::default(),
            sky: crate::weather::weather_at(seed, 0, CLOCK_START, Some("clear")),
            temp: crate::weather::temperature_at(seed, 0, CLOCK_START, None),
            force_weather: None,
            weather_cycle: true,
            seed,
            player: Player {
                x: 0.,
                y: 0.,
                angle: 0.,
                in_car: None,
                step: 0.,
                stun: 0.,
                stamina: 1.,
                tired: false,
                rest: 0.,
                swimming: false,
                move_speed: 0.,
                level: Default::default(),
                level_init: false,
                combat: Default::default(),
                click: None,
                click_t: 0.,
            },
            player_car_id: None,
            mission: Mission::default(),
            money: 0.,
            completed: 0.,
            best_time: None,
            camera: Camera {
                x: 0.,
                y: 0.,
                zoom: 1.,
            },
            res: Reservations::default(),
            knocked: Knocked::new(),
            esp: true,
            abs: true,
            notice: None,
            veh_info: None,
            car_target: cars,
            ped_target: peds,
            loading: true,
            populated: false,
            pending_save: None,
            parked_keys: HashSet::new(),
            park_tick: 0,
            next_car: 1,
            next_ped: 1,
            focus_key: "world".into(),
        };
        w.spawn_player_and_car();
        if let Some(pc) = w.city.places.parked.first().copied() {
            let id = w.new_car_id();
            w.cars.push(Car::new(
                id,
                pc.x,
                pc.y,
                pc.angle,
                0x16a085,
                Role::Parked,
                "car",
            ));
        }
        w.camera.x = w.player.x;
        w.camera.y = w.player.y;
        w.stream();
        w
    }
    fn new_car_id(&mut self) -> u32 {
        let id = self.next_car;
        self.next_car += 1;
        id
    }
    pub fn car(&self, id: u32) -> Option<&Car> {
        self.cars.iter().find(|c| c.id == id)
    }
    pub fn player_car(&self) -> Option<&Car> {
        self.player.in_car.and_then(|id| self.car(id))
    }
    fn car_index(&self, id: u32) -> Option<usize> {
        self.cars.iter().position(|c| c.id == id)
    }

    fn spawn_player_and_car(&mut self) {
        let pl = self.city.places.clone();
        self.player.x = pl.player_spawn.x;
        self.player.y = pl.player_spawn.y;
        self.player.in_car = None;
        self.player.angle = -std::f64::consts::FRAC_PI_2;
        self.player.swimming = false;
        let idx = match self.player_car_id.and_then(|id| self.car_index(id)) {
            Some(i) => i,
            None => {
                let id = self.new_car_id();
                self.cars
                    .push(Car::new(id, 0., 0., 0., 0xc0392b, Role::Player, "car"));
                self.player_car_id = Some(id);
                self.cars.len() - 1
            }
        };
        let c = &mut self.cars[idx];
        (c.x, c.y, c.angle) = (pl.player_car.x, pl.player_car.y, pl.player_car.angle);
        (
            c.vx, c.vy, c.ang_vel, c.health, c.wrecked, c.wreck_t, c.cargo,
        ) = (0., 0., 0., car::HEALTH, false, 0., false);
        c.driver = None;
        c.ai = None;
        c.controls = Default::default();
    }

    /// Kacheln um die Kamera nachladen; `false` = der Stadtteil ist noch nicht da (die Welt steht still).
    pub fn stream(&mut self) -> bool {
        let (x, y) = (self.camera.x, self.camera.y);
        self.loading = !self.city.focus(&self.focus_key, x, y);
        self.lanes.sync(&mut self.city);
        if !self.loading && !self.populated {
            self.populate();
        }
        !self.loading
    }
    fn populate(&mut self) {
        self.populated = true;
        for _ in 0..self.car_target {
            self.spawn_traffic(120., SPAWN_MAX);
        }
        for _ in 0..self.ped_target {
            self.spawn_ped(60., SPAWN_MAX);
        }
    }
    /// Verkehr, Passanten und Parker verwerfen (nach einem Ortswechsel).
    pub fn reset_population(&mut self) {
        let keep: Vec<bool> = self
            .cars
            .iter()
            .map(|c| {
                Some(c.id) == self.player_car_id
                    || Some(c.id) == self.player.in_car
                    || c.cargo
                    || c.role == Role::Parked
            })
            .collect();
        let mut i = 0;
        self.cars.retain(|c| {
            let k = keep[i];
            i += 1;
            if !k && let Some(pk) = c.park_key {
                self.parked_keys.remove(&pk);
            }
            if !k {
                drop_claims(&mut self.res, c.id);
            }
            k
        });
        self.peds.clear();
        self.populated = false;
    }

    fn spawn_traffic(&mut self, min_r: f64, max_r: f64) -> Option<u32> {
        let (cx, cy) = (self.camera.x, self.camera.y);
        for _ in 0..8 {
            let (lane, s, x, y) = self.lanes.spawn_spot(&mut self.rng, cx, cy, min_r, max_r)?;
            if !self.cars.iter().all(|o| (o.x - x).hypot(o.y - y) > 70.) {
                continue;
            }
            let agents: Vec<Agent> = self.cars.iter().map(Agent::of).collect();
            let mut ev = Vec::new();
            let free = {
                let mut ctx = Ctx {
                    city: &mut self.city,
                    lanes: &mut self.lanes,
                    agents: &agents,
                    walkers: &[],
                    agent_grid: None,
                    walker_grid: None,
                    player_on_foot: None,
                    rng: &mut self.rng,
                    time: self.time,
                    res: &mut self.res,
                    events: &mut ev,
                };
                narrow_free(&mut ctx, lane)
            };
            if !free {
                continue;
            }
            let dtv = self
                .lanes
                .lane(lane)
                .and_then(|l| self.city.edges.get(&l.edge))
                .map(|e| e.dtv)
                .unwrap_or(8000.);
            if self.rng.float() > (dtv / 15000.).clamp(0.12, 1.) {
                continue;
            }
            let color = CAR_COLORS[self.rng.index(CAR_COLORS.len())];
            let id = self.new_car_id();
            let mut car = Car::new(id, x, y, 0., color, Role::Traffic, "car");
            place_on_lane(
                &mut car,
                &mut self.lanes,
                &self.city,
                lane,
                s,
                &mut self.rng,
            );
            car.driver = Some(Driver::Npc);
            let seg = car.ai.as_ref().map(|a| a.segs[0].uid).unwrap_or(0);
            let mut agents2 = agents;
            agents2.push(Agent::of(&car));
            let mut ctx = Ctx {
                city: &mut self.city,
                lanes: &mut self.lanes,
                agents: &agents2,
                walkers: &[],
                agent_grid: None,
                walker_grid: None,
                player_on_foot: None,
                rng: &mut self.rng,
                time: self.time,
                res: &mut self.res,
                events: &mut ev,
            };
            claim_narrow(&mut ctx, id, lane, seg);
            self.cars.push(car);
            return Some(id);
        }
        None
    }

    fn spawn_ped(&mut self, min_r: f64, max_r: f64) -> Option<u32> {
        let (cx, cy) = (self.camera.x, self.camera.y);
        let sp = pedestrians::spawn_spot(
            &mut self.city,
            &mut self.sidewalks,
            &mut self.rng,
            cx,
            cy,
            min_r,
            max_r,
        )?;
        let id = self.next_ped;
        self.next_ped += 1;
        let mut p = create_ped(id, &mut self.city, &mut self.sidewalks, sp, &mut self.rng);
        p.dead_t = 0.;
        self.peds.push(p);
        Some(id)
    }

    fn manage_population(&mut self) {
        let (cx, cy) = (self.camera.x, self.camera.y);
        let keep = |w: &World, c: &Car| {
            Some(c.id) == w.player_car_id
                || Some(c.id) == w.player.in_car
                || c.cargo
                || matches!(c.role, Role::Parked | Role::Curb)
                || c.driver == Some(Driver::Player)
        };
        let stuck = |c: &Car| {
            c.driver == Some(Driver::Npc)
                && c.ai
                    .as_ref()
                    .is_some_and(|a| a.still_t > 30. || a.head_on >= 4)
                && (c.x - cx).hypot(c.y - cy) > 1100.
        };
        let drop: Vec<u32> = self
            .cars
            .iter()
            .filter(|c| !(keep(self, c) || ((c.x - cx).hypot(c.y - cy) < DESPAWN && !stuck(c))))
            .map(|c| c.id)
            .collect();
        for id in &drop {
            drop_claims(&mut self.res, *id);
        }
        self.cars.retain(|c| !drop.contains(&c.id));
        self.peds.retain(|p| (p.x - cx).hypot(p.y - cy) < DESPAWN);
        let npc = self
            .cars
            .iter()
            .filter(|c| {
                c.driver == Some(Driver::Npc) || (c.driver.is_none() && c.role == Role::Traffic)
            })
            .count();
        if npc < self.car_target {
            self.spawn_traffic(SPAWN_MIN, SPAWN_MAX);
        }
        if self
            .peds
            .iter()
            .filter(|p| p.state != PedState::Dead)
            .count()
            < self.ped_target
        {
            self.spawn_ped(SPAWN_MIN * 0.8, SPAWN_MAX);
        }
    }

    /// Stellplätze einer Kante (deterministisch): Mitte des Parkstreifens, Ausrichtung je Aufstellung.
    pub fn parking_slots(&self, eid: i64) -> Vec<Slot> {
        let city = &self.city;
        let Some(e) = city.edges.get(&eid) else {
            return Vec::new();
        };
        let s = city.scale;
        let mut slots = Vec::new();
        if !(e.inside && e.cls <= 8 && !e.bridge) {
            return slots;
        }
        let corner_gap = |n: i64| {
            let Some(nd) = city.nodes.get(&n) else {
                return 5. * s;
            };
            let r = nd
                .edges
                .iter()
                .filter(|&&k| k != eid)
                .filter_map(|k| city.edges.get(k))
                .map(|o| o.w / 2.)
                .fold(0., f64::max);
            if nd.edges.len() > 2 {
                r + 5. * s
            } else {
                2. * s
            }
        };
        let (m0, m1) = (corner_gap(e.a), corner_gap(e.b));
        let pl = &city.places;
        let mut avoid = vec![
            (pl.dropoff.x, pl.dropoff.y),
            (pl.player_car.x, pl.player_car.y),
            (pl.pickup.x, pl.pickup.y),
        ];
        avoid.extend(pl.parked.iter().map(|p| (p.x, p.y)));
        for side in [-1i8, 1] {
            let (offset, depth, orient, kind) = parking_strip(&e.cs, side);
            use berlin_map_loader::citycodes::park;
            if (kind != park::LANE && kind != park::HALF) || depth < 0.9 * s {
                continue;
            }
            let step = [5.6, 3.0, 2.6][orient.min(2) as usize] * s;
            let mut i = 0;
            let mut st = m0 + step / 2.;
            while st < e.len - m1 - step / 2. {
                if hash01((e.id * 977 + (side as i64 + 1) * 31 + i as i64 * 7919) as f64)
                    < PARKED_SHARE
                {
                    let p = point_along(&e.pts, st);
                    let (x, y) = (p.x - p.uy * offset, p.y + p.ux * offset);
                    if !avoid.iter().any(|q| (q.0 - x).hypot(q.1 - y) < 12. * s) {
                        let mut angle =
                            p.uy.atan2(p.ux) + if side < 0 { std::f64::consts::PI } else { 0. };
                        if orient == 2 {
                            angle += side as f64 * std::f64::consts::FRAC_PI_2;
                        } else if orient == 1 {
                            angle += side as f64 * std::f64::consts::FRAC_PI_4;
                        }
                        slots.push(((eid, side, i), x, y, angle));
                    }
                }
                i += 1;
                st += step;
            }
        }
        slots
    }
    fn slot_free(&mut self, x: f64, y: f64, angle: f64) -> bool {
        let probe = Obb {
            x,
            y,
            angle,
            hw: car::LENGTH / 2.,
            hh: car::WIDTH / 2.,
        };
        for h in self.city.solids.query(&probe.bounds()) {
            let s = *self.city.solids.get(h);
            if !car::blocks(&self.knocked, &s, 0) {
                continue;
            }
            let m = match s {
                Solid::Wall { seg, .. } => obb_vs_segment(&probe, &seg),
                Solid::Circle { x, y, r, .. } => circle_vs_obb(x, y, r, &probe),
                Solid::Rect(r) => obb_vs_rect(&probe, &r),
            };
            if m.is_some_and(|m| m.depth > 1.) {
                return false;
            }
        }
        self.cars.iter().all(|c| (c.x - x).hypot(c.y - y) > 30.)
    }
    /// Parkende Autos im Umkreis der Kamera erzeugen, ferne wieder abbauen.
    fn manage_parked(&mut self) {
        let (cx, cy) = (self.camera.x, self.camera.y);
        let pid = self.player_car_id;
        let mut freed = Vec::new();
        self.cars.retain(|c| {
            if c.role != Role::Curb
                || c.driver == Some(Driver::Player)
                || Some(c.id) == pid
                || c.cargo
            {
                return true;
            }
            if (c.x - cx).hypot(c.y - cy) < PARKED_DESPAWN {
                return true;
            }
            freed.extend(c.park_key);
            false
        });
        for k in freed {
            self.parked_keys.remove(&k);
        }
        self.park_tick += 1;
        if self.park_tick % 20 != 1 {
            return;
        }
        let r = PARKED_RADIUS;
        let mut seen = Vec::new();
        for h in self.city.edge_segs.query(&Rect::around(cx, cy, r)) {
            if let Some(e) = self.city.edge_segs.get(h).edge
                && !seen.contains(&e)
            {
                seen.push(e);
            }
        }
        for e in seen {
            for (key, x, y, angle) in self.parking_slots(e) {
                if self.parked_keys.contains(&key)
                    || (x - cx).hypot(y - cy) > r
                    || !self.slot_free(x, y, angle)
                {
                    continue;
                }
                let color = CAR_COLORS
                    [(hash01(x * 31. + y) * CAR_COLORS.len() as f64) as usize % CAR_COLORS.len()];
                let id = self.new_car_id();
                let mut c = Car::new(id, x, y, angle, color, Role::Curb, "car");
                c.park_key = Some(key);
                c.controls.handbrake = true;
                self.parked_keys.insert(key);
                self.cars.push(c);
            }
        }
    }

    fn push_circle_out(&mut self, r: f64) {
        let lvl = self.player.level.lvl;
        let p = &mut self.player;
        for h in self.city.solids.query(&Rect::around(p.x, p.y, r + 2.)) {
            let s = *self.city.solids.get(h);
            if !car::blocks(&self.knocked, &s, lvl) {
                continue;
            }
            let m = match s {
                Solid::Wall { seg, .. } => circle_vs_segment(p.x, p.y, r, &seg),
                Solid::Circle { x, y, r: cr, .. } => circle_vs_circle(p.x, p.y, r, x, y, cr),
                Solid::Rect(rc) => circle_vs_rect(p.x, p.y, r, &rc),
            };
            if let Some(m) = m {
                p.x += m.nx * m.depth;
                p.y += m.ny * m.depth;
            }
        }
    }

    fn try_enter(&mut self) -> bool {
        self.try_enter_car(None)
    }
    /// Einsteigen ins nächste heile Auto in Reichweite, mit `only` nur in dieses (Klick auf ein Auto).
    fn try_enter_car(&mut self, only: Option<u32>) -> bool {
        let (px, py) = (self.player.x, self.player.y);
        let mut best = None;
        let mut bd = if only.is_some() {
            ENTER_DIST + 10.
        } else {
            ENTER_DIST
        };
        for (i, c) in self.cars.iter().enumerate() {
            if c.wrecked || only.is_some_and(|id| id != c.id) {
                continue;
            }
            let d = (c.x - px).hypot(c.y - py);
            if d < bd {
                bd = d;
                best = Some(i);
            }
        }
        let Some(i) = best else { return false };
        if self.cars[i].driver == Some(Driver::Npc) {
            // Fahrer steigt aus und flieht
            let (x, y) = (self.cars[i].x, self.cars[i].y);
            self.events.push(Event::Carjack { x, y });
            self.fleeing_driver(i, px, py, 3.5);
        }
        let id = self.cars[i].id;
        drop_claims(&mut self.res, id);
        let c = &mut self.cars[i];
        c.driver = Some(Driver::Player);
        c.ai = None;
        c.controls = Default::default();
        c.dyn_state = None;
        let (x, y, kind, role) = (c.x, c.y, c.kind, c.role);
        if !c.kind_info().bike {
            self.veh_info = Some((id, 0.));
        }
        self.player.in_car = Some(id);
        self.player.swimming = false;
        let own_ok = self
            .player_car_id
            .and_then(|pid| self.car(pid))
            .is_some_and(|c| !c.wrecked);
        if role != Role::Player && !is_open_kind(kind) && !own_ok {
            self.player_car_id = Some(id);
        }
        self.events.push(Event::Door { x, y });
        true
    }
    /// Klicksteuerung zu Fuß (world.js clickControl): macht aus dem Klick die normalen Eingaben (Laufrichtung,
    /// Zielen, Angriff) und steigt am Ziel ins Auto. WASD bricht den Auftrag ab.
    fn click_control(&mut self, input: &Input, dt: f64) -> Input {
        let mut out = *input;
        if input.move_x.hypot(input.move_y) > 0.05 {
            self.player.click = None;
            return out;
        }
        let lvl = self.player.level.lvl;
        let (px, py) = (self.player.x, self.player.y);
        let walk = |w: &mut World, to: (f64, f64)| {
            crate::footpath::find_foot_path(w, (px, py), to, lvl).filter(|p| p.len() > 1)
        };
        if let Some(at) = input.click_world {
            if input.click_pressed {
                self.player.click_t = 0.15;
                self.player.click =
                    self.click_intent(at, input.click_force, input.click_double, walk);
            } else if input.click_held {
                match &mut self.player.click {
                    Some(Click::Walk { follow: true, .. }) => {
                        self.player.click_t -= dt;
                        if self.player.click_t <= 0. {
                            // gehalten: dem Zeiger nachlaufen (Weg alle 0,15 s neu)
                            self.player.click_t = 0.15;
                            if let Some(path) = walk(self, at) {
                                self.player.click = Some(Click::Walk {
                                    path,
                                    i: 1,
                                    follow: true,
                                });
                            }
                        }
                    }
                    Some(Click::Force { at: a, ped: None }) => *a = at,
                    _ => {}
                }
            }
        }
        let stop = |o: &mut Input| {
            o.move_x = 0.;
            o.move_y = 0.;
            o.combat.fire = false;
            o.combat.fire_pressed = false;
        };
        let Some(click) = self.player.click.clone() else {
            return out;
        };
        let ped_pos = |w: &World, id: u32| {
            w.peds
                .iter()
                .find(|p| p.id == id && p.state != PedState::Dead)
                .map(|p| (p.x, p.y))
        };
        let toward = |o: &mut Input, (x, y): (f64, f64)| {
            let d = (x - px).hypot(y - py).max(1e-9);
            o.move_x = (x - px) / d;
            o.move_y = (y - py) / d;
        };
        match click {
            Click::Force { at, ped } => {
                if !input.click_held && !input.click_pressed {
                    self.player.click = None;
                    return out;
                }
                let aim = ped.and_then(|id| ped_pos(self, id)).unwrap_or(at);
                out.combat.aim_world = Some(aim);
                out.combat.fire = true;
                out.combat.fire_pressed = input.click_pressed || self.player.combat.cool <= 0.;
            }
            Click::Target { ped, done } => {
                let Some(o) = ped_pos(self, ped) else {
                    self.player.click = None;
                    return out;
                };
                let wp = self.player.combat.weapon();
                let d = (o.0 - px).hypot(o.1 - py);
                let reach = if wp.melee {
                    wp.range + 6.
                } else {
                    wp.range * 0.85
                };
                if d > reach {
                    toward(&mut out, o);
                    return out;
                }
                // in Reichweite: ein Klick = ein Angriff; gehalten: weiter, bis die Person liegt
                let ready = self.player.combat.cool <= 0.;
                if !input.click_held && done && ready {
                    self.player.click = None;
                    return out;
                }
                if ready {
                    self.player.click = Some(Click::Target { ped, done: true });
                }
                out.combat.aim_world = Some(o);
                out.combat.fire = true;
                out.combat.fire_pressed = ready;
            }
            Click::Enter {
                car,
                approach,
                mut path,
                mut i,
                mut to,
                door,
            } => {
                let Some((cx, cy)) = self
                    .car(car)
                    .filter(|c| !c.wrecked && Some(c.id) != self.player.in_car)
                    .map(|c| (c.x, c.y))
                else {
                    self.player.click = None;
                    return out;
                };
                let d = (cx - px).hypot(cy - py);
                if d < ENTER_DIST - 2. {
                    stop(&mut out);
                    self.player.angle = (cy - py).atan2(cx - px);
                    if approach {
                        self.player.click = None;
                        return out;
                    }
                    // an der Tür: kurz stehen bleiben (Tür auf), dann einsteigen
                    let left = match door {
                        None => {
                            self.events.push(Event::Door { x: cx, y: cy });
                            CLICK_DOOR
                        }
                        Some(t) => t - dt,
                    };
                    if left <= 0. {
                        self.player.click = None;
                        self.try_enter_car(Some(car));
                    } else if let Some(Click::Enter { door, .. }) = self.player.click.as_mut() {
                        *door = Some(left);
                    }
                    return out;
                }
                // Weg zum Auto (um Häuser herum), neu, wenn es weggefahren ist
                if path.is_empty() || (cx - to.0).hypot(cy - to.1) > 30. {
                    path = walk(self, (cx, cy)).unwrap_or_default();
                    i = 1;
                    to = (cx, cy);
                }
                while path.get(i).is_some_and(|q| (q.0 - px).hypot(q.1 - py) < 5.) {
                    i += 1;
                }
                let goal = path.get(i).copied().unwrap_or((cx, cy));
                toward(&mut out, goal);
                out.combat.fire = false;
                out.combat.fire_pressed = false;
                self.player.click = Some(Click::Enter {
                    car,
                    approach,
                    path,
                    i,
                    to,
                    door,
                });
            }
            Click::Walk {
                path,
                mut i,
                follow,
            } => {
                while path.get(i).is_some_and(|q| (q.0 - px).hypot(q.1 - py) < 5.) {
                    i += 1;
                }
                let Some(&q) = path.get(i) else {
                    self.player.click = None;
                    return out;
                };
                toward(&mut out, q);
                self.player.click = Some(Click::Walk { path, i, follow });
            }
        }
        out
    }

    /// Was ein Klick bedeutet (combat.js clickIntent): Person → angreifen, heiles Auto daneben oder Doppelklick →
    /// einsteigen, weiter weg → nur hinlaufen, sonst (Boden, Wrack) hinlaufen; mit Strg am Platz angreifen.
    fn click_intent(
        &mut self,
        at: (f64, f64),
        force: bool,
        double: bool,
        walk: impl Fn(&mut World, (f64, f64)) -> Option<Vec<(f64, f64)>>,
    ) -> Option<Click> {
        let lvl = self.player.level.lvl;
        let ped = self
            .peds
            .iter()
            .filter(|p| p.state != PedState::Dead && p.level.lvl == lvl)
            .map(|p| ((p.x - at.0).hypot(p.y - at.1), p.id))
            .filter(|(d, _)| *d < pedestrians::RADIUS + 4.)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id);
        if force {
            return Some(Click::Force { at, ped });
        }
        if let Some(id) = ped {
            return Some(Click::Target {
                ped: id,
                done: false,
            });
        }
        let car = self
            .cars
            .iter()
            .filter(|c| c.lvl() == lvl && Some(c.id) != self.player.in_car)
            .find(|c| {
                let (s, co) = c.angle.sin_cos();
                let (dx, dy) = (at.0 - c.x, at.1 - c.y);
                (dx * co + dy * s).abs() < c.hw + 2. && (-dx * s + dy * co).abs() < c.hh + 2.
            })
            .map(|c| {
                (
                    c.id,
                    c.wrecked,
                    (c.x - self.player.x).hypot(c.y - self.player.y),
                )
            });
        if let Some((id, false, d)) = car {
            return Some(Click::Enter {
                car: id,
                approach: !(d <= ENTER_DIST + CLICK_NEAR_CAR || double),
                path: Vec::new(),
                i: 1,
                to: (f64::NAN, f64::NAN),
                door: None,
            });
        }
        let path = walk(self, at)?;
        Some(Click::Walk {
            path,
            i: 1,
            follow: true,
        })
    }

    /// K. o.: nach kurzer Pause ins nächste Krankenhaus (Gebühr), ein laufender Auftrag platzt (world.js updateKnockout).
    fn update_knockout(&mut self, dt: f64) {
        use crate::combat::{HOSPITAL_FEE, RESPAWN_DELAY};
        let c = &mut self.player.combat;
        c.dead_t += dt;
        if c.dead_t < RESPAWN_DELAY {
            return;
        }
        let (px, py) = (self.player.x, self.player.y);
        let (hx, hy, name) = self
            .city
            .nearest_hospital(px, py)
            .map(|h| (h.x, h.y, h.name.clone()))
            .unwrap_or((
                self.city.places.player_spawn.x,
                self.city.places.player_spawn.y,
                String::new(),
            ));
        if !self.player.combat.moved {
            // erst hinbringen; die Kacheln dort lädt der nächste Schritt
            self.player.combat.moved = true;
            teleport_on_foot(self, hx, hy);
            return;
        }
        // auf den nächsten Gehweg stellen, falls das Krankenhaus mitten im Haus liegt
        if (self.city.in_building(px, py).is_some()
            || self.city.surface_at(px, py, None) == Ground::Water)
            && let Some(sp) =
                pedestrians::nearest_spot(&mut self.city, &mut self.sidewalks, px, py, 800.)
        {
            let (x, y) = self.sidewalks.point(&mut self.city, sp.edge, sp.side, sp.s);
            (self.player.x, self.player.y) = (x, y);
            (self.camera.x, self.camera.y) = (x, y);
        }
        let fee = (self.money * HOSPITAL_FEE).floor();
        self.money -= fee;
        self.player.combat = crate::combat::Combat {
            weapon: self.player.combat.weapon,
            ..Default::default()
        };
        self.player.stun = 0.;
        self.player.level_init = false;
        let fee_txt = if fee > 0. {
            format!(" (-{} €)", fee as i64)
        } else {
            String::new()
        };
        let place = if name.is_empty() {
            String::new()
        } else {
            format!(": {name}")
        };
        self.notice = Some(Notice {
            text: format!("Im Krankenhaus aufgewacht{place}{fee_txt}"),
            t: 5.,
        });
        self.events.push(Event::Respawn {
            x: self.player.x,
            y: self.player.y,
            fee,
        });
        if matches!(self.mission.state, State::ToPickup | State::ToDropoff) {
            self.mission.fail(
                "K. o. – im Krankenhaus aufgewacht, der Auftrag ist geplatzt.",
                &mut self.events,
            );
        }
    }
    fn fleeing_driver(&mut self, car_idx: usize, fx: f64, fy: f64, secs: f64) {
        let (x, y, a, hh, lvl) = {
            let c = &self.cars[car_idx];
            (c.x, c.y, c.angle, c.hh, c.lvl())
        };
        let side = (fx - x) * -a.sin() + (fy - y) * a.cos();
        let s = if side > 0. { -1. } else { 1. };
        let (dx, dy) = (x - a.sin() * (hh + 8.) * s, y + a.cos() * (hh + 8.) * s);
        let Some(spot) =
            pedestrians::nearest_spot(&mut self.city, &mut self.sidewalks, dx, dy, 600.)
        else {
            return;
        };
        let id = self.next_ped;
        self.next_ped += 1;
        let mut p = create_ped(id, &mut self.city, &mut self.sidewalks, spot, &mut self.rng);
        (p.x, p.y) = (dx, dy);
        p.level.lvl = lvl;
        p.level_init = true;
        scare(&mut p, (fx, fy), secs);
        self.peds.push(p);
    }
    fn try_exit(&mut self) -> bool {
        let Some(i) = self.player.in_car.and_then(|id| self.car_index(id)) else {
            return false;
        };
        let c = self.cars[i].clone();
        let (fx, fy) = (c.angle.cos(), c.angle.sin());
        let (rx, ry) = (-fy, fx);
        let side = |s: f64| {
            let d = c.hh + PLAYER_RADIUS + 3.;
            (c.x + rx * d * s, c.y + ry * d * s)
        };
        let cands = [
            side(-1.),
            side(1.),
            (c.x - fx * (c.hw + 10.), c.y - fy * (c.hw + 10.)),
            (c.x + fx * (c.hw + 10.), c.y + fy * (c.hw + 10.)),
        ];
        let lvl = c.lvl();
        let spot = cands.into_iter().find(|&(x, y)| {
            spot_free_static(&mut self.city, &self.knocked, x, y, PLAYER_RADIUS, lvl)
                && self
                    .cars
                    .iter()
                    .all(|o| o.id == c.id || circle_vs_obb(x, y, PLAYER_RADIUS, &o.obb()).is_none())
        });
        let Some((x, y)) = spot else {
            self.notice = Some(Notice {
                text: "Kein Platz zum Aussteigen".into(),
                t: 1.5,
            });
            return false;
        };
        let car = &mut self.cars[i];
        car.driver = None;
        car.dyn_state = None;
        car.controls = Default::default();
        car.controls.handbrake = car.speed() < 60.;
        self.player.in_car = None;
        (self.player.x, self.player.y) = (x, y);
        self.player.level = car.level;
        self.events.push(Event::Door { x: car.x, y: car.y });
        true
    }

    fn update_player_on_foot(&mut self, input: &Input, dt: f64) {
        let p = &mut self.player;
        let stunned = p.stun > 0.;
        if stunned {
            p.stun = (p.stun - dt).max(0.);
        }
        let start = self.city.surface_at(p.x, p.y, Some(p.level.lvl));
        p.swimming = start == Ground::Water;
        let (mx, my) = if stunned {
            (0., 0.)
        } else {
            (input.move_x, input.move_y)
        };
        let mag = mx.hypot(my).min(1.);
        let want_sprint = !p.swimming && input.sprint && mag > 0.05 && !input.walk_slow;
        if p.tired && p.stamina >= STAMINA_AGAIN {
            p.tired = false;
        }
        let sprinting = want_sprint && !p.tired && p.stamina > 0.;
        if sprinting {
            p.stamina = (p.stamina - dt / STAMINA_DRAIN).max(0.);
            p.rest = 0.;
            if p.stamina == 0. {
                p.tired = true;
            }
        } else {
            p.rest += dt;
            if p.rest > STAMINA_PAUSE {
                p.stamina = (p.stamina + dt / STAMINA_RECOVER).min(1.);
            }
        }
        if mag > 0.05 {
            let speed = if p.swimming {
                18.
            } else if sprinting {
                SPRINT
            } else if input.walk_slow || mag <= 0.6 {
                WALK
            } else {
                JOG
            };
            let l = mx.hypot(my);
            let (nx, ny) = (mx / l, my / l);
            p.x += nx * speed * dt;
            p.y += ny * speed * dt;
            p.move_speed = speed;
            p.angle = ny.atan2(nx);
            p.step += speed * dt;
        } else {
            p.move_speed = 0.;
        }
        self.push_circle_out(PLAYER_RADIUS);
        let p = &mut self.player;
        p.x = p.x.clamp(8., self.city.width - 8.);
        p.y = p.y.clamp(8., self.city.height - 8.);
        let end = self.city.surface_at(p.x, p.y, Some(p.level.lvl));
        p.swimming = end == Ground::Water;
    }

    fn update_levels(&mut self) {
        for c in &mut self.cars {
            if c.level_init && c.role == Role::Curb && c.vx.abs() + c.vy.abs() < 2. {
                continue;
            }
            if !c.level_init {
                c.level.lvl = initial_level(&mut self.city, c.x, c.y, Some(c.angle), 30.);
                c.level_init = true;
            } else {
                step_level(&mut self.city, c.x, c.y, Some(c.angle), &mut c.level);
            }
        }
        for p in &mut self.peds {
            if !p.level_init {
                p.level.lvl = initial_level(&mut self.city, p.x, p.y, None, 30.);
                p.level_init = true;
            } else {
                step_level(&mut self.city, p.x, p.y, None, &mut p.level);
            }
        }
        if let Some(c) = self
            .player
            .in_car
            .and_then(|id| self.cars.iter().find(|c| c.id == id))
        {
            self.player.level = c.level;
            self.player.level_init = true;
        } else if !self.player.level_init {
            self.player.level.lvl =
                initial_level(&mut self.city, self.player.x, self.player.y, None, 30.);
            self.player.level_init = true;
        } else {
            let (x, y) = (self.player.x, self.player.y);
            step_level(&mut self.city, x, y, None, &mut self.player.level);
        }
    }

    fn apply_weather(&mut self, i: usize, dt: f64) {
        let c = &self.cars[i];
        if c.role == Role::Curb && c.driver.is_none() {
            return;
        }
        let w = self.weather;
        let storm = self.sky.p.storm;
        if w.wet == 0. && w.snow == 0. && w.ice == 0. && storm <= 0. {
            self.cars[i].traction = crate::traction::DRY;
            return;
        }
        let cond = road_condition(&mut self.city, &w, c.x, c.y, c.lvl());
        self.cars[i].traction = traction_of(&cond);
        // Sturmböe schiebt fahrende Autos quer (traction.js gustPush)
        let (ax, ay) = self.gust_accel(&self.cars[i]);
        let c = &mut self.cars[i];
        c.vx += ax * dt;
        c.vy += ay * dt;
    }

    /// Beschleunigung durch eine Sturmböe (px/s²): Brücken stärker, kleine Autos mehr, das Fahrdynamikmodell
    /// des Spielers bekommt nur einen Teil (es reagiert über die Reifen selbst).
    pub fn gust_accel(&self, c: &Car) -> (f64, f64) {
        let storm = self.sky.p.storm;
        let (wx, wy) = self.sky.wind;
        let wl = wx.hypot(wy);
        if storm <= 0. || wl <= 0. || c.speed() < 5. {
            return (0., 0.);
        }
        let g = crate::weather::gust_at(storm, self.time) - crate::traction::GUST_THRESHOLD;
        if g <= 0. {
            return (0., 0.);
        }
        let a = crate::traction::GUST_PUSH
            * storm
            * g
            * if c.lvl() >= 1 {
                crate::traction::GUST_BRIDGE
            } else {
                1.
            }
            / ((c.hw * c.hh) / 210.)
            * if c.driver == Some(Driver::Player) && c.kind_info().top.is_none() {
                crate::traction::GUST_DYNAMIC
            } else {
                1.
            };
        (wx / wl * a, wy / wl * a)
    }

    /// Warnschild für den Fahrer (traction.js roadWarning): nur im eigenen Fahrzeug.
    pub fn road_warning(&mut self) -> Option<&'static str> {
        let c = self.player_car()?.clone();
        if c.aqua > 0. {
            return Some("Aquaplaning!");
        }
        let cond = road_condition(&mut self.city, &self.weather, c.x, c.y, c.lvl());
        if cond.ice > 0.2 {
            return Some("Glätte");
        }
        if cond.snow > 0.2 {
            return Some("Schnee");
        }
        let (ax, ay) = self.gust_accel(&c);
        if ax.hypot(ay) > crate::traction::GUST_WARN {
            return Some("Sturm");
        }
        (cond.wet > 0.3).then_some("Nässe")
    }

    /// Wetterbild, Temperatur und Boden (Nässe, Schneedecke, Glätte) einen Schritt weiter.
    pub fn step_weather(&mut self, dt: f64) {
        use crate::weather as wx;
        let force = if self.weather_cycle {
            self.force_weather
        } else {
            Some("clear")
        };
        self.sky = wx::weather_at(self.seed, self.day_count, self.clock, force);
        let g = &mut self.weather;
        g.wet = wx::step_wet(g.wet, self.sky.p.rain.min(1.), dt);
        let snow_was = g.snow;
        g.snow = wx::step_snow(snow_was, &self.sky.p, dt);
        if g.snow < snow_was {
            g.wet = g.wet.max((g.snow * 1.5).min(1.)); // Tauwetter: Matsch und nasse Straßen
        }
        self.temp = wx::temperature_at(self.seed, self.day_count, self.clock, self.force_weather);
        g.ice = wx::step_ice(g.ice, g.wet, self.temp, dt);
    }

    /// Ein Simulationsschritt.
    pub fn update(&mut self, input: &Input, dt: f64) {
        self.events.clear();
        self.city.tick(dt);
        if let Some(s) = self.pending_save.clone()
            && !self.resolve_save(&s)
        {
            self.loading = true;
            return;
        }
        if !self.stream() {
            return;
        }
        self.time += dt;
        self.clock += dt;
        if self.clock >= 1440. {
            self.clock -= 1440.;
            self.day = (self.day + 1) % 7;
            self.day_count += 1;
        }
        self.step_weather(dt);
        if let Some(n) = self.notice.as_mut() {
            n.t -= dt;
            if n.t <= 0. {
                self.notice = None;
            }
        }
        if let Some((_, t)) = self.veh_info.as_mut() {
            *t += dt;
            if *t > VEH_INFO_S {
                self.veh_info = None;
            }
        }
        if matches!(
            self.mission.state,
            State::Briefing | State::Success | State::Failed
        ) {
            self.update_mission(input, dt);
            return;
        }
        if input.enter_exit {
            if self.player.in_car.is_some() {
                self.try_exit();
            } else {
                self.try_enter();
            }
        }
        let pc = self.player.in_car.and_then(|id| self.car_index(id));
        if let Some(i) = pc {
            if input.esp_toggle && !self.cars[i].wrecked {
                self.esp = !self.esp;
                self.notice = Some(Notice {
                    text: format!("ESP {}", if self.esp { "AN" } else { "AUS" }),
                    t: 1.6,
                });
            }
            if input.abs_toggle && !self.cars[i].wrecked {
                self.abs = !self.abs;
                self.notice = Some(Notice {
                    text: format!("ABS {}", if self.abs { "AN" } else { "AUS" }),
                    t: 1.6,
                });
            }
            let (esp, abs) = (self.esp, self.abs);
            let c = &mut self.cars[i];
            if c.wrecked {
                c.controls = Default::default();
                c.horn = false;
            } else {
                c.controls.throttle = input.throttle;
                c.controls.brake = input.brake;
                c.controls.steer = input.steer;
                c.controls.handbrake = input.handbrake;
                c.horn = input.horn;
            }
            c.esp = esp;
            c.abs = abs;
            if c.horn && !c.horn_was {
                self.events.push(Event::Horn {
                    x: c.x,
                    y: c.y,
                    npc: false,
                });
            }
            c.horn_was = c.horn;
        } else if !self.player.combat.dead {
            let input = &self.click_control(input, dt);
            self.update_player_on_foot(input, dt);
            crate::combat::update_player_combat(self, &input.combat, dt);
        }
        if self.player.in_car.is_some() || self.player.combat.dead {
            self.player.click = None;
            crate::combat::update_player_combat(self, &input.combat, dt);
        }
        if self.player.combat.dead {
            self.update_knockout(dt);
        }

        // KI: Momentaufnahme + Nachbarschaftsraster (während der Schleife bewegt sich nichts)
        let agents: Vec<Agent> = self.cars.iter().map(Agent::of).collect();
        let walkers: Vec<Walker> = self
            .peds
            .iter()
            .map(|p| Walker {
                x: p.x,
                y: p.y,
                lvl: p.level.lvl,
                alive: p.state != PedState::Dead,
            })
            .collect();
        let mut g_cars = Grid::default();
        g_cars.build(agents.iter().map(|a| (a.x, a.y)), GRID_CELL);
        let mut g_peds = Grid::default();
        g_peds.build(walkers.iter().map(|p| (p.x, p.y)), GRID_CELL);
        let on_foot = self.player.in_car.is_none().then_some((
            self.player.x,
            self.player.y,
            self.player.level.lvl,
        ));
        for i in 0..self.cars.len() {
            if self.cars[i].driver != Some(Driver::Npc) {
                continue;
            }
            let mut ctx = Ctx {
                city: &mut self.city,
                lanes: &mut self.lanes,
                agents: &agents,
                walkers: &walkers,
                agent_grid: Some(&g_cars),
                walker_grid: Some(&g_peds),
                player_on_foot: on_foot,
                rng: &mut self.rng,
                time: self.time,
                res: &mut self.res,
                events: &mut self.events,
            };
            drive_ai(&mut self.cars[i], &mut ctx, dt);
        }
        for i in 0..self.cars.len() {
            let c = &mut self.cars[i];
            if c.driver.is_none() && !c.wrecked && Some(i) != pc {
                c.controls = crate::dynamics::Controls {
                    handbrake: true,
                    ..Default::default()
                };
            }
            if c.role == Role::Curb
                && c.driver.is_none()
                && !c.wrecked
                && c.vx.abs() + c.vy.abs() < 2.
                && c.ang_vel.abs() < 0.01
            {
                (c.vx, c.vy, c.ang_vel) = (0., 0., 0.);
                continue;
            }
            self.apply_weather(i, dt);
            let c = &self.cars[i];
            let ground = self.city.surface_at(c.x, c.y, Some(c.lvl()));
            let c = &mut self.cars[i];
            step_car(c, dt, Some(ground));
            collide_car_world(c, &mut self.city, &mut self.knocked, &mut self.events);
        }
        // Auto gegen Auto: nur Nachbarn, Paare in aufsteigender Folge
        let mut pairs = Grid::default();
        pairs.build(self.cars.iter().map(|c| (c.x, c.y)), GRID_CELL);
        let mut nb = Vec::new();
        for i in 0..self.cars.len() {
            let (ax, ay) = (self.cars[i].x, self.cars[i].y);
            pairs.near(ax, ay, 170., &mut nb);
            for &j in &nb {
                if j <= i {
                    continue;
                }
                let (lo, hi) = self.cars.split_at_mut(j);
                let (a, b) = (&mut lo[i], &mut hi[0]);
                let r = a.hw + b.hw + 4.;
                if (a.x - b.x).abs() < r
                    && (a.y - b.y).abs() < r
                    && touch(&mut self.city, (a.x, a.y, a.lvl()), (b.x, b.y, b.lvl()))
                {
                    collide_cars(a, b, &mut self.events);
                }
            }
        }
        if let Some(i) = self.player.in_car.and_then(|id| self.car_index(id)) {
            let c = &self.cars[i];
            (self.player.x, self.player.y, self.player.angle) = (c.x, c.y, c.angle);
        }
        self.update_levels();

        // Beschossene Autos: KI-Fahrer steigt aus und rennt weg. Wracks: ebenso, Wrack verschwindet später außer Sicht
        for i in 0..self.cars.len() {
            if let Some((fx, fy)) = self.cars[i].shot_at.take()
                && self.cars[i].driver == Some(Driver::Npc)
                && !self.cars[i].wrecked
            {
                self.cars[i].driver = None;
                self.cars[i].ai = None;
                let id = self.cars[i].id;
                drop_claims(&mut self.res, id);
                self.fleeing_driver(i, fx, fy, 5.);
            }
            if !self.cars[i].wrecked {
                continue;
            }
            self.cars[i].wreck_t += dt;
            if self.cars[i].driver == Some(Driver::Npc) {
                self.cars[i].driver = None;
                self.cars[i].ai = None;
                let id = self.cars[i].id;
                drop_claims(&mut self.res, id);
                let (x, y) = (self.cars[i].x, self.cars[i].y);
                self.fleeing_driver(i, x, y, 3.);
            }
        }
        let (cx, cy, pid, inc) = (
            self.camera.x,
            self.camera.y,
            self.player_car_id,
            self.player.in_car,
        );
        self.cars.retain(|c| {
            !(c.wrecked
                && c.wreck_t > 20.
                && Some(c.id) != pid
                && Some(c.id) != inc
                && !c.cargo
                && (c.x - cx).hypot(c.y - cy) > 900.)
        });

        // Spieler zu Fuß gegen Autos
        if self.player.in_car.is_none() {
            for i in 0..self.cars.len() {
                let o = self.cars[i].obb();
                let p = &self.player;
                let Some(m) = circle_vs_obb(p.x, p.y, PLAYER_RADIUS, &o) else {
                    continue;
                };
                if !touch(
                    &mut self.city,
                    (p.x, p.y, p.level.lvl),
                    (o.x, o.y, self.cars[i].lvl()),
                ) {
                    continue;
                }
                let p = &mut self.player;
                p.x += m.nx * m.depth;
                p.y += m.ny * m.depth;
                if self.cars[i].speed() > 120. && p.stun <= 0. {
                    p.stun = 0.8;
                    self.events.push(Event::Bump { x: p.x, y: p.y });
                }
            }
        }

        self.update_peds(dt);
        self.manage_population();
        self.manage_parked();
        self.update_mission(input, dt);
        self.update_camera(dt);
    }

    fn update_peds(&mut self, dt: f64) {
        // Bedrohungen: rasendes Spielerauto, Hupe, Unfall
        struct Threat {
            x: f64,
            y: f64,
            vx: f64,
            vy: f64,
            r: f64,
            always: bool,
            melee: bool,
        }
        let mut threats = Vec::new();
        if let Some(c) = self.player_car()
            && !c.wrecked
            && c.speed() > 130.
        {
            threats.push(Threat {
                x: c.x,
                y: c.y,
                vx: c.vx,
                vy: c.vy,
                r: 90.,
                always: false,
                melee: false,
            });
        }
        for e in &self.events {
            match *e {
                Event::Horn { x, y, npc } => threats.push(Threat {
                    x,
                    y,
                    vx: 0.,
                    vy: 0.,
                    r: if npc { 80. } else { 170. },
                    always: true,
                    melee: false,
                }),
                Event::Crash { x, y, strength, .. } if strength > 0.25 => threats.push(Threat {
                    x,
                    y,
                    vx: 0.,
                    vy: 0.,
                    r: 130.,
                    always: true,
                    melee: false,
                }),
                // Schüsse hört man weit; Blut und Schläge des Spielers nur in der Nähe (Kämpfer mischen mit)
                Event::Shot { x, y, .. } => threats.push(Threat {
                    x,
                    y,
                    vx: 0.,
                    vy: 0.,
                    r: crate::combat::GUNSHOT_SCARE,
                    always: true,
                    melee: false,
                }),
                Event::Blood { x, y, .. } => threats.push(Threat {
                    x,
                    y,
                    vx: 0.,
                    vy: 0.,
                    r: 220.,
                    always: true,
                    melee: false,
                }),
                Event::Swing {
                    x, y, npc: false, ..
                } => threats.push(Threat {
                    x,
                    y,
                    vx: 0.,
                    vy: 0.,
                    r: 90.,
                    always: true,
                    melee: true,
                }),
                _ => {}
            }
        }
        let mut grid = Grid::default();
        grid.build(self.cars.iter().map(|c| (c.x, c.y)), GRID_CELL);
        let moving: Vec<MovingCar> = self
            .cars
            .iter()
            .map(|c| MovingCar {
                x: c.x,
                y: c.y,
                speed: c.speed(),
                lvl: c.lvl(),
            })
            .collect();
        let on_foot = (self.player.in_car.is_none() && !self.player.combat.dead)
            .then_some((self.player.x, self.player.y));
        let alive = !self.player.combat.dead;
        let mut punches = Vec::new();
        let player_car = self.player.in_car;
        let mut near = Vec::new();
        let mut hits: Vec<(f64, f64)> = Vec::new();
        for k in 0..self.peds.len() {
            let ped = &mut self.peds[k];
            if ped.state != PedState::Dead {
                if !matches!(ped.state, PedState::Down | PedState::Flee | PedState::Fight) {
                    for t in &threats {
                        if (ped.x - t.x).hypot(ped.y - t.y) > t.r {
                            continue;
                        }
                        if t.melee && alive && crate::combat::is_fighter(ped.id) {
                            crate::combat::start_fight(ped);
                            break;
                        }
                        if t.always || (ped.x - t.x) * t.vx + (ped.y - t.y) * t.vy > 0. {
                            scare(ped, (t.x, t.y), 2.5);
                            break;
                        }
                    }
                }
                grid.near(ped.x, ped.y, GRID_REACH, &mut near);
                for &ci in &near {
                    let c = &self.cars[ci];
                    let Some(m) = circle_vs_obb(ped.x, ped.y, pedestrians::RADIUS, &c.obb()) else {
                        continue;
                    };
                    if !touch(
                        &mut self.city,
                        (ped.x, ped.y, ped.level.lvl),
                        (c.x, c.y, c.lvl()),
                    ) {
                        continue;
                    }
                    let speed = c.speed();
                    if speed > 55. {
                        if ped.car_hit_cd <= 0. {
                            ped.hp -= speed * 0.32;
                            ped.car_hit_cd = 1.1;
                            if ped.hp <= 0. {
                                ped.state = PedState::Dead;
                                ped.dead_t = 0.;
                            } else {
                                ped.state = PedState::Flee;
                                ped.t = 2.2;
                                ped.threat = (c.x, c.y);
                            }
                            self.events.push(Event::Hit {
                                x: ped.x,
                                y: ped.y,
                                car: c.id,
                                player: Some(c.id) == player_car,
                                speed,
                            });
                            hits.push((ped.x, ped.y));
                        }
                        ped.x += m.nx * (m.depth + 1.);
                        ped.y += m.ny * (m.depth + 1.);
                    } else {
                        ped.x += m.nx * m.depth;
                        ped.y += m.ny * m.depth;
                    }
                }
            }
            let mut cx = PedCtx {
                city: &mut self.city,
                sw: &mut self.sidewalks,
                rng: &mut self.rng,
                knocked: &self.knocked,
                cars: &moving,
                player_on_foot: on_foot,
                punches: &mut punches,
            };
            pedestrians::update_ped(&mut self.peds[k], &mut cx, dt);
        }
        for (x, y) in punches {
            self.events.push(Event::Swing {
                x,
                y,
                weapon: "fists",
                hit: true,
                npc: true,
            });
            crate::combat::hurt_player(self, crate::combat::FIGHT_DMG, (x, y));
        }
        for (hx, hy) in hits {
            for o in &mut self.peds {
                if (o.x - hx).hypot(o.y - hy) < 110. && (o.x, o.y) != (hx, hy) {
                    scare(o, (hx, hy), 2.5);
                }
            }
        }
        let (cx, cy) = (self.camera.x, self.camera.y);
        self.peds.retain(|q| {
            q.state != PedState::Dead
                || ((q.dead_t < 60. || (q.x - cx).hypot(q.y - cy) < 900.) && q.dead_t < 300.)
        });
    }

    fn mission_view(&self) -> PlayerView {
        PlayerView {
            x: self.player.x,
            y: self.player.y,
            in_car: self.player.in_car,
        }
    }
    fn update_mission(&mut self, input: &Input, dt: f64) {
        let pv = self.mission_view();
        let places = self.city.places.clone();
        let tl = places.time_limit;
        let ev = self.mission.update(
            &places,
            pv,
            &mut self.cars,
            tl,
            MissionInput {
                action: input.action,
                action_held: input.action_held,
            },
            dt,
        );
        let success = ev.contains(&Event::MissionSuccess);
        self.events.extend(ev);
        if success
            && let Some(crate::mission::Outcome::Success {
                time,
                reward,
                new_best,
                ..
            }) = self.mission.result.as_mut()
        {
            self.money += *reward;
            self.completed += 1.;
            if self.best_time.is_none_or(|b| *time < b) {
                self.best_time = Some(*time);
                *new_best = true;
            }
        }
    }
    /// Mission neu starten: Spieler zum Späti, eigenes Auto repariert zurück auf den Parkplatz.
    pub fn restart_mission(&mut self) {
        for c in &mut self.cars {
            c.cargo = false;
        }
        self.mission.reset();
        let sp = self.city.places.player_spawn;
        let far = (self.camera.x - sp.x).hypot(self.camera.y - sp.y) > DESPAWN;
        self.spawn_player_and_car();
        self.player.level_init = false;
        self.camera.x = self.player.x;
        self.camera.y = self.player.y;
        if far {
            self.reset_population();
            self.stream();
        }
    }

    pub fn update_camera(&mut self, dt: f64) {
        let (mut tx, mut ty, mut zoom) = (self.player.x, self.player.y, FOOT_ZOOM);
        if let Some(c) = self.player_car() {
            tx = c.x + c.vx * 0.45;
            ty = c.y + c.vy * 0.45;
            let k = c.kind_info();
            zoom = if k.bike {
                FOOT_ZOOM * 0.8
            } else if k.moto {
                1.45 - (c.speed() / 500.).clamp(0., 1.) * 0.45
            } else {
                1. - (c.speed() / 330.).clamp(0., 1.) * 0.28
            };
        }
        self.camera.x = damp(self.camera.x, tx, 5., dt);
        self.camera.y = damp(self.camera.y, ty, 5., dt);
        self.camera.zoom = damp(self.camera.zoom, zoom, 2., dt);
    }

    // --- Spielstand -------------------------------------------------------------------------

    pub fn make_save(&self, now_ms: f64) -> SaveData {
        let car = self
            .player_car_id
            .and_then(|id| self.car(id))
            .filter(|c| !c.wrecked);
        let pos = self
            .player_car()
            .map(|c| (c.x, c.y))
            .unwrap_or((self.player.x, self.player.y));
        let r2 = |v: f64| (v * 100.).round() / 100.;
        SaveData {
            version: crate::save::SAVE_VERSION,
            saved_at: now_ms,
            money: self.money,
            completed: self.completed,
            best_time: self.best_time,
            clock: Some(self.clock.round()),
            day: Some(self.day),
            day_count: Some(self.day_count),
            wet: Some(r2(self.weather.wet)),
            snow: Some(r2(self.weather.snow)),
            ice: Some(r2(self.weather.ice)),
            player: SavedPoint {
                x: pos.0.round(),
                y: pos.1.round(),
            },
            car: car.map(|c| SavedCar {
                x: c.x.round(),
                y: c.y.round(),
                angle: c.angle,
                health: c.health,
                model: (c.kind == "car").then(|| c.model_name().to_owned()),
            }),
        }
    }
    /// Spielstand übernehmen. Liegt der Ort in einem noch nicht geladenen Stadtteil, gilt er vorläufig und wird
    /// geprüft, sobald die Kacheln dort da sind.
    pub fn apply_save(&mut self, s: SaveData) {
        self.money = s.money;
        self.completed = s.completed;
        self.best_time = s.best_time;
        if let Some(c) = s.clock {
            self.clock = c;
        }
        if let Some(d) = s.day {
            self.day = d;
        }
        if let Some(d) = s.day_count {
            self.day_count = d;
        }
        if let Some(v) = s.wet {
            self.weather.wet = v;
        }
        if let Some(v) = s.snow {
            self.weather.snow = v;
        }
        self.weather.ice = s.ice.unwrap_or(0.);
        let p = s
            .car
            .as_ref()
            .map(|c| (c.x, c.y))
            .unwrap_or((s.player.x, s.player.y));
        if self.city.inside_border(p.0, p.1) {
            self.camera.x = p.0;
            self.camera.y = p.1;
        }
        self.pending_save = Some(s.clone());
        self.resolve_save(&s);
    }
    fn resolve_save(&mut self, s: &SaveData) -> bool {
        let (cx, cy) = (self.camera.x, self.camera.y);
        if !self.city.focus(&self.focus_key, cx, cy) {
            return false;
        }
        self.lanes.sync(&mut self.city);
        self.pending_save = None;
        let valid = |w: &mut World, x: f64, y: f64| {
            w.city.inside_border(x, y) && w.city.in_building(x, y).is_none()
        };
        let car_idx = self.player_car_id.and_then(|id| self.car_index(id));
        match (&s.car, car_idx) {
            (Some(sc), Some(i)) if valid(self, sc.x, sc.y) => {
                let c = &mut self.cars[i];
                (c.x, c.y, c.angle, c.health, c.vx, c.vy) =
                    (sc.x, sc.y, sc.angle, sc.health, 0., 0.);
                if let Some(m) = sc.model.as_deref() {
                    c.model = crate::carmodels::CAR_MODELS
                        .iter()
                        .find(|(k, _)| *k == m)
                        .map(|(k, _)| *k);
                }
                for side in [1., -1., 0.] {
                    self.player.x = sc.x - sc.angle.sin() * 24. * side;
                    self.player.y = sc.y + sc.angle.cos() * 24. * side;
                    let (px, py) = (self.player.x, self.player.y);
                    if self.city.in_building(px, py).is_none() {
                        break;
                    }
                }
            }
            _ => {
                if valid(self, s.player.x, s.player.y) {
                    self.player.x = s.player.x;
                    self.player.y = s.player.y;
                }
            }
        }
        self.player.level_init = false;
        self.camera.x = self.player.x;
        self.camera.y = self.player.y;
        self.reset_population();
        true
    }
}

/// Teleport-Hilfe für Werkzeuge und Tests: Spieler (zu Fuß) an einen Punkt setzen.
pub fn teleport_on_foot(w: &mut World, x: f64, y: f64) {
    w.player.in_car = None;
    w.player.x = x;
    w.player.y = y;
    w.player.level_init = false;
    w.camera.x = x;
    w.camera.y = y;
    w.reset_population();
}

/// Kurzer Name für Logs.
pub fn describe(w: &World) -> String {
    let npc = w
        .cars
        .iter()
        .filter(|c| c.driver == Some(Driver::Npc))
        .count();
    format!(
        "{} Autos ({npc} KI), {} Passanten, {} Spuren, Mission {:?}",
        w.cars.len(),
        w.peds.len(),
        w.lanes.len(),
        w.mission.state
    )
}
