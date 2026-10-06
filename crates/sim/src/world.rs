//! Spielwelt (Port der Kernschleife von `world.js`): verbindet Stadt, Spieler, Autos, Passanten und Mission zu
//! einem deterministischen Simulationsschritt. Eingaben kommen als abstrakter Zustand ([`Input`]), Ausgaben als
//! Ereignisse ([`Event`]). Der feste Schritt ist [`DT`].
use crate::car::{self, Car, Driver, Knocked, Role, collide_car_world, collide_cars, step_car};
use crate::carmodels::{CAR_COLORS, is_open_kind};
use crate::city::{City, Ground, Solid, WallKind, WallSub, point_along};
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
use std::collections::{HashMap, HashSet};

pub const DT: f64 = 1. / 60.;
pub const PLAYER_RADIUS: f64 = 7.;
pub const WALK: f64 = 15.;
pub const JOG: f64 = 35.;
pub const SPRINT: f64 = 70.;
/// Sprung: Absprunggeschwindigkeit (px/s) und Schwerkraft (px/s², 10 px = 1 m) → ~0,65 s in der Luft, ~0,5 m hoch
pub const JUMP_V: f64 = 32.;
pub const GRAVITY: f64 = 98.;
/// ab dieser Höhe (px) halten niedrige Hindernisse (Zäune, Gleisseiten, Poller, Kisten) die Figur nicht mehr auf
pub const JUMP_CLEAR: f64 = 1.5;
/// Schwung beim Sprung (px/s, mindestens; sprintend schneller). Im Gehtempo (15 px/s) käme die Figur in der Zeit über
/// der Freigabehöhe (~0,55 s) nur 8 px weit, braucht aber 2 × Radius (14 px) über die Zaunlinie; und wer per Klick an
/// einen Zaun gelaufen ist, steht ~14 px davor (der Laufweg endet an der letzten freien Rasterzelle) – 45 px/s tragen
/// ~25 px weit.
pub const JUMP_CARRY: f64 = 45.;
pub const ENTER_DIST: f64 = 40.;
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
    /// Klicksteuerung zu Fuß (Diablo-Schema): Zeigerpunkt, gedrückt/gehalten, mit Strg. Eingestiegen wird nie per
    /// Klick (nur per Taste); ein Klick auf ein Auto oder Rad läuft bloß hin.
    pub click_world: Option<(f64, f64)>,
    pub click_pressed: bool,
    pub click_held: bool,
    pub click_force: bool,
    /// Darf ein Klick angreifen (Person anklicken = zuschlagen, Strg = am Platz)? Am PC aus: dort schießt nur die
    /// rechte Maustaste, ein Linksklick auf eine Person läuft bloß hin.
    pub click_attack: bool,
    /// Mitfahren: einsteigen bzw. aussteigen (Flanke, G / Steuerkreuz unten)
    pub ride: bool,
    /// Springen (zu Fuß, gedrückt in diesem Schritt)
    pub jump: bool,
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
    /// zum Auto laufen und danebenstellen (eingestiegen wird nur per Taste)
    Approach {
        car: u32,
        path: Vec<(f64, f64)>,
        i: usize,
        to: (f64, f64),
    },
    /// mit Strg: am Platz angreifen, wohin gezeigt wird (bzw. auf die Person darunter)
    Force { at: (f64, f64), ped: Option<u32> },
    /// fahrenden Radler angreifen (nur mit Angriffsrecht), bis der Fahrer runter ist
    Bike { bike: u32 },
}
/// so lange ohne Vorankommen, dann plant der Klick-Laufweg neu (s)
pub const CLICK_STALL: f64 = 0.6;

#[derive(Debug, Clone)]
pub struct Player {
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub in_car: Option<u32>,
    pub step: f64,
    pub stun: f64,
    pub swimming: bool,
    pub move_speed: f64,
    pub level: crate::levels::LevelState,
    pub level_init: bool,
    pub combat: crate::combat::Combat,
    pub click: Option<Click>,
    pub click_t: f64,
    /// fährt mit bzw. führt eine Bahn (ride.rs)
    pub ride: Option<crate::ride::Ride>,
    /// im U-Bahnhof (station.rs); nach dem Hinaufgehen nicht gleich wieder hinunter
    pub inside: Option<crate::station::Inside>,
    pub entry_guard: Option<(f64, f64)>,
    /// Klick-Laufweg: Ort und Zeit ohne Vorankommen (steht vor einem Auto, das selbst wartet)
    pub click_stall: (f64, f64, f64),
    /// Sprung: Höhe über dem Boden (px) und Steiggeschwindigkeit (px/s)
    pub z: f64,
    pub vz: f64,
    /// Sprung: Schwung beim Absprung (px/s); in der Luft lenkt man nicht, die Figur fliegt so weiter
    pub jump_v: (f64, f64),
    /// Klicksteuerung: wohin zuletzt geklickt wurde (ein Sprung zielt dorthin, nach der Landung geht es weiter –
    /// der Laufweg endet vor einem Zaun, das Ziel liegt dahinter)
    pub click_goal: Option<(f64, f64)>,
    /// gerade gelandet (der Klick-Laufweg wird danach neu geplant)
    pub landed: bool,
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
    /// Radfahrer und E-Roller (bikes.rs)
    pub bikes: Vec<crate::bikes::Bike>,
    pub bike_paths: crate::bikes::Paths,
    next_bike: u32,
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
    /// erzwungene Temperatur (°C, Befehlszeile `temp`)
    pub force_temp: Option<f64>,
    /// Tempo der Spieluhr (1 = normal, 0 = steht; Befehlszeile `tempo`)
    pub clock_rate: f64,
    /// Kamerazoom zu Fuß (`world.js w.footZoom`)
    pub foot_zoom: f64,
    /// unverwundbar (Befehlszeile `gott`)
    pub god: bool,
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
    /// Blaulichteinsätze (services.rs); aus = ruhige Testwelt
    pub services: bool,
    /// Fahrzeugarten nach Tageszeit (fleet.rs); aus = nur Pkw
    pub rhythm: bool,
    pub emergency: crate::services::Emergency,
    pub knocked: Knocked,
    pub esp: bool,
    /// ESP voll statt sportlich (Fahrphysik `vphys`; aus = `esp` false)
    pub esp_full: bool,
    /// Tempobegrenzer schwerer Lkw (abschaltbar, wenn `feel.lkw_begrenzer_tunebar`)
    pub truck_limiter: bool,
    /// Physik-LOD-Umkreis der KI (px; 0 = alle kinematisch)
    pub ai_full_radius: f64,
    pub abs: bool,
    pub notice: Option<Notice>,
    /// Name und Technik nach dem Einsteigen (Auto-ID, Sekunden)
    pub veh_info: Option<(u32, f64)>,
    pub car_target: usize,
    pub ped_target: usize,
    /// Tagesrhythmus: Zielbevölkerung nach Uhrzeit, Wochentag und Ort, Stadtleben und Tiere (nur bei der
    /// Standardbevölkerung; Tests mit festen Zahlen bleiben ruhig)
    pub day_rhythm: bool,
    /// Dichte-Faktoren für Verkehr und Passanten (Befehlszeile)
    pub traffic_scale: f64,
    pub ped_scale: f64,
    /// Tauben und Enten (animals.rs) und ihre Schwarmplätze
    pub animals: Vec<crate::animals::Animal>,
    pub flocks: std::collections::BTreeMap<String, (crate::animals::Kind, f64, f64)>,
    /// Stadtleben: Platzschlüssel → Passanten-ID
    pub hangers: std::collections::BTreeMap<String, u32>,
    life_cache: crate::life::LifeCache,
    /// abgestellte E-Roller je Kante um die Kamera (nur Darstellung)
    pub scooters: std::collections::BTreeMap<i64, Vec<crate::bikes::ParkedScooter>>,
    /// Fahrplan (transit.json), verfolgte Muster, Busse schon aufgestellt, Tunnel-Zwischenspeicher
    pub transit: Option<Box<crate::transit::Transit>>,
    pub transit_state: crate::transit::State,
    pub transit_populated: bool,
    pub ug: crate::tunnel::Cache,
    /// vom Spieler übernommener Zug; Tunnelansicht 0 (oben) … 1 (unter Tage)
    pub player_train: Option<crate::ride::PlayerTrain>,
    pub underground: f64,
    /// Bahnhöfe (aus dem Fahrplan), die um die Figur und ihr Zähler
    pub stations: crate::station::Cache,
    pub st_near: Vec<crate::station::Station>,
    pub st_tick: u32,
    pub st_gen: u64,
    /// Straßenbahnwagen nahe der Kamera als Hindernisse (transitlive.rs)
    pub rail_obs: Vec<crate::traffic::RailObs>,
    /// Pfützen je Kante (Aquaplaning und Darstellung), mit den Rollern um die Kamera abgebaut
    pub puddles: HashMap<i64, Vec<crate::traction::Puddle>>,
    scoot_t: f64,
    rhythm_t: f64,
    life_t: f64,
    anim_t: f64,
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

/// Niedrige Hindernisse, über die man springen kann: Zäune, Gleisseiten, Poller, Kisten. Hauswände, Mauern und
/// Hecken, Bäume, Kaikanten (Absturz ins Wasser) und Brückengeländer (Absturz von der Brücke – die Ebenen kennen
/// keinen Fall) bleiben Hindernisse.
pub fn jumpable(s: &Solid) -> bool {
    match s {
        Solid::Wall { kind, sub, .. } => {
            *kind == WallKind::Wall && matches!(sub, WallSub::Fence | WallSub::Rail)
        }
        Solid::Circle { kind, .. } => matches!(kind, crate::city::CircleKind::Barrier { .. }),
        Solid::Rect(_) => true,
    }
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
            force_temp: None,
            clock_rate: 1.,
            foot_zoom: FOOT_ZOOM,
            god: false,
            weather_cycle: true,
            seed,
            player: Player {
                x: 0.,
                y: 0.,
                angle: 0.,
                in_car: None,
                step: 0.,
                stun: 0.,
                swimming: false,
                move_speed: 0.,
                level: Default::default(),
                level_init: false,
                combat: Default::default(),
                click: None,
                click_t: 0.,
                ride: None,
                inside: None,
                entry_guard: None,
                click_stall: (0., 0., 0.),
                z: 0.,
                vz: 0.,
                jump_v: (0., 0.),
                click_goal: None,
                landed: false,
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
            services: true,
            rhythm: true,
            emergency: crate::services::Emergency::new(0.5),
            knocked: Knocked::new(),
            esp: true,
            esp_full: crate::vehdata::game_feel().esp_default == "voll",
            truck_limiter: true,
            ai_full_radius: AI_FULL_RADIUS,
            abs: true,
            notice: None,
            veh_info: None,
            car_target: cars,
            ped_target: peds,
            day_rhythm: cars == TRAFFIC_CARS && peds == TRAFFIC_PEDS,
            traffic_scale: 1.,
            ped_scale: 1.,
            animals: Vec::new(),
            flocks: Default::default(),
            hangers: Default::default(),
            life_cache: Default::default(),
            scooters: Default::default(),
            puddles: HashMap::new(),
            rail_obs: Vec::new(),
            transit: None,
            transit_state: Default::default(),
            transit_populated: false,
            ug: Default::default(),
            player_train: None,
            underground: 0.,
            stations: Default::default(),
            st_near: Vec::new(),
            st_tick: 0,
            st_gen: u64::MAX,
            scoot_t: -99.,
            rhythm_t: -99.,
            life_t: -99.,
            anim_t: -99.,
            loading: true,
            populated: false,
            bikes: Vec::new(),
            bike_paths: Default::default(),
            next_bike: 1,
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
        if self.day_rhythm {
            self.set_targets();
            self.rhythm_t = self.time;
        }
        self.manage_life(true);
        self.manage_animals(true);
        for _ in 0..self.car_target {
            self.spawn_traffic(120., SPAWN_MAX);
        }
        for _ in 0..self.ped_target {
            self.spawn_ped(60., SPAWN_MAX);
        }
        for _ in 0..self.bike_target() {
            self.spawn_bike(120., SPAWN_MAX);
        }
    }
    /// Zielbevölkerung aus Tagesrhythmus und Ort; bei Regen und Nebel gehen weniger Menschen raus.
    pub fn set_targets(&mut self) {
        let (cx, cy) = (self.camera.x, self.camera.y);
        let (c, p) = crate::rhythm::population_targets(
            &mut self.city,
            cx,
            cy,
            self.clock,
            self.day,
            (TRAFFIC_CARS, TRAFFIC_PEDS),
        );
        self.car_target = (c as f64 * self.traffic_scale).round() as usize;
        let min = if self.ped_scale == 0. { 0. } else { 4. };
        self.ped_target = (p as f64 * crate::weather::people_factor(&self.sky.p) * self.ped_scale)
            .round()
            .max(min) as usize;
    }

    /// Stadtleben: Passanten mit Tätigkeit an ihren Plätzen halten (life.rs). Neue entstehen nur außer Sicht, außer
    /// direkt nach dem Aufbau eines Ortes (Spielbeginn, Teleport); wer nicht mehr gebraucht wird, geht außer Sicht.
    pub fn manage_life(&mut self, all: bool) {
        use crate::life;
        if !self.day_rhythm || (!all && self.time - self.life_t < life::EVERY) {
            return;
        }
        self.life_t = self.time;
        let (cx, cy) = (self.camera.x, self.camera.y);
        let in_view = |x: f64, y: f64| {
            (x - cx).abs() < life::VIEW_HALF_X && (y - cy).abs() < life::VIEW_HALF_Y
        };
        let want = life::life_spots(
            &mut self.city,
            &mut self.life_cache,
            cx,
            cy,
            self.clock,
            self.day,
            life::RADIUS,
        );
        let keys: HashSet<&str> = want.iter().map(|s| s.key.as_str()).collect();
        let mut drop_ids = Vec::new();
        self.hangers.retain(|key, id| {
            let Some(p) = self.peds.iter().find(|p| p.id == *id) else {
                return false;
            };
            if p.state != PedState::Hang {
                return false; // aufgescheucht: geht als normaler Passant weiter
            }
            let far = (p.x - cx).hypot(p.y - cy) > life::DESPAWN;
            if far || (!keys.contains(key.as_str()) && !in_view(p.x, p.y)) {
                drop_ids.push(*id);
                return false;
            }
            true
        });
        self.peds.retain(|p| !drop_ids.contains(&p.id));
        for s in want {
            if self.hangers.contains_key(&s.key) || self.hangers.len() >= life::MAX_HANGERS {
                continue;
            }
            if !all && in_view(s.x, s.y) {
                continue;
            }
            let Some(sp) = crate::pedestrians::nearest_spot(
                &mut self.city,
                &mut self.sidewalks,
                s.x,
                s.y,
                200.,
            ) else {
                continue;
            };
            let id = self.next_ped;
            self.next_ped += 1;
            let mut p = create_ped(id, &mut self.city, &mut self.sidewalks, sp, &mut self.rng);
            (p.x, p.y, p.facing, p.state) = (s.x, s.y, s.face, PedState::Hang);
            p.dead_t = 0.;
            self.assign_kind(&mut p, Some(s.act));
            self.hangers.insert(s.key.clone(), id);
            p.hang = Some(s);
            self.peds.push(p);
        }
    }

    /// Tiere um die Kamera halten (alle 0,5 s): fehlende Schwärme außer Sicht aufstellen, ferne abbauen.
    pub fn manage_animals(&mut self, all: bool) {
        use crate::animals::{self as an, Kind, State};
        use crate::life::{DESPAWN, VIEW_HALF_X, VIEW_HALF_Y};
        if !self.day_rhythm || (!all && self.time - self.anim_t < an::EVERY) {
            return;
        }
        self.anim_t = self.time;
        let (cx, cy) = (self.camera.x, self.camera.y);
        let in_view = |x: f64, y: f64| {
            (x - cx).abs() < VIEW_HALF_X + 60. && (y - cy).abs() < VIEW_HALF_Y + 60.
        };
        let spots = an::animal_spots(&mut self.city, &mut self.life_cache, cx, cy, an::RADIUS);
        let want: HashSet<&str> = spots.iter().map(|s| s.key.as_str()).collect();
        self.animals.retain(|a| {
            let far = (a.x - cx).hypot(a.y - cy) > DESPAWN;
            let flown = a.state == State::Fly && !in_view(a.x, a.y);
            !(far || flown || (!want.contains(a.key.as_str()) && !in_view(a.x, a.y)))
        });
        self.flocks
            .retain(|k, f| want.contains(k.as_str()) || in_view(f.1, f.2));
        let mut ducks = self.flocks.values().filter(|f| f.0 == Kind::Duck).count();
        let mut pigeons = self.flocks.len() - ducks;
        for s in &spots {
            if self.flocks.contains_key(&s.key) {
                continue;
            }
            let full = if s.kind == Kind::Duck {
                ducks >= an::MAX_DUCKS
            } else {
                pigeons >= an::MAX_FLOCKS
            };
            if full || (!all && in_view(s.x, s.y)) {
                continue;
            }
            self.flocks.insert(s.key.clone(), (s.kind, s.x, s.y));
            if s.kind == Kind::Duck {
                ducks += 1;
            } else {
                pigeons += 1;
            }
            for i in 0..s.n {
                let b = an::make_bird(&mut self.rng, s, i);
                self.animals.push(b);
            }
        }
        // aufgescheuchte Schwärme kommen erst wieder, wenn ihr Platz außer Sicht ist
        let animals = &self.animals;
        self.flocks
            .retain(|k, f| animals.iter().any(|a| &a.key == k) || in_view(f.1, f.2));
    }

    /// Abgestellte E-Roller der Kanten um die Kamera (alle 0,5 s, rein aus den Kanten-IDs).
    pub fn manage_scooters(&mut self) {
        if self.time - self.scoot_t < 0.5 {
            return;
        }
        self.scoot_t = self.time;
        let (cx, cy) = (self.camera.x, self.camera.y);
        let near: HashSet<i64> = self
            .city
            .edge_segs
            .query(&Rect::around(cx, cy, 1300.))
            .into_iter()
            .filter_map(|h| self.city.edge_segs.get(h).edge)
            .filter(|id| self.city.edges.get(id).is_some_and(|e| e.lvl < 1))
            .collect();
        self.scooters.retain(|id, _| near.contains(id));
        if self.puddles.len() > 4000 {
            self.puddles.retain(|id, _| near.contains(id));
        }
        let wet = self.weather.wet > crate::traction::PUDDLE_WET * 0.5;
        for id in near {
            if wet {
                self.puddles_of(id);
            }
            if !self.scooters.contains_key(&id) {
                let v = crate::bikes::parked_scooters(&mut self.city, &mut self.sidewalks, id);
                self.scooters.insert(id, v);
            }
        }
    }

    /// Platz frei für einen Kreis (feste Hindernisse dieser Ebene, Autos)?
    pub fn spot_free_here(&mut self, x: f64, y: f64, r: f64, lvl: i8) -> bool {
        spot_free_static(&mut self.city, &self.knocked, x, y, r, lvl)
            && self
                .cars
                .iter()
                .all(|c| circle_vs_obb(x, y, r, &c.obb()).is_none())
    }

    /// Nachtleben an (x, y): Lokale vor ihrem Gehweg, mit Feed; Regen und Schnee nach drinnen.
    pub fn nightlife_at(&mut self, x: f64, y: f64) -> crate::nightlife::Heard {
        let (clock, day, rain, snow) = (self.clock, self.day, self.sky.p.rain, self.sky.p.snow);
        let cache = &mut self.life_cache;
        let mut front = |c: &mut City, qx: f64, qy: f64| cache.front(c, qx, qy).map(|f| (f.x, f.y));
        crate::nightlife::nightlife_at(
            &mut self.city,
            &mut front,
            x,
            y,
            clock,
            day,
            None,
            rain,
            snow,
        )
    }

    /// Pfützen einer Kante (gecacht, solange die Kachel um die Kamera vollständig ist).
    pub fn puddles_of(&mut self, eid: i64) -> &[crate::traction::Puddle] {
        if !self.puddles.contains_key(&eid) {
            let v = crate::traction::edge_puddles(&mut self.city, eid);
            self.puddles.insert(eid, v);
        }
        &self.puddles[&eid]
    }
    /// Pfütze unter (x, y) auf Ebene `lvl` (nur bei genug Nässe, nicht überdacht).
    pub fn puddle_at(&mut self, x: f64, y: f64, lvl: i8) -> Option<crate::traction::Puddle> {
        if self.weather.wet <= crate::traction::PUDDLE_WET
            || crate::traction::covered(&mut self.city, x, y, lvl)
        {
            return None;
        }
        for e in crate::traction::roads_under(&mut self.city, x, y, lvl) {
            let hit = self
                .puddles_of(e)
                .iter()
                .find(|p| crate::traction::in_puddle(p, x, y))
                .copied();
            if hit.is_some() {
                return hit;
            }
        }
        None
    }

    fn update_animals(&mut self, dt: f64) {
        use crate::animals::{SCARE_CAR, SCARE_PERSON};
        if self.animals.is_empty() {
            return;
        }
        let mut threats = Vec::new();
        if self.player.in_car.is_none() && !self.player.combat.dead {
            threats.push((self.player.x, self.player.y, SCARE_PERSON));
        }
        for c in &self.cars {
            if c.vx.abs() + c.vy.abs() > 60. {
                threats.push((c.x, c.y, SCARE_CAR + c.hw));
            }
        }
        for p in &self.peds {
            // Fliehende und Jogger scheuchen Tauben auf
            if p.state == PedState::Flee
                || (p.state == PedState::Walk && p.style == crate::figure::Style::Jog)
            {
                threats.push((p.x, p.y, 35.));
            }
        }
        let shots: Vec<(f64, f64)> = self
            .events
            .iter()
            .filter_map(|e| match *e {
                Event::Shot { x, y, .. } | Event::Horn { x, y, .. } => Some((x, y)),
                _ => None,
            })
            .collect();
        crate::animals::update_animals(
            &mut self.animals,
            &mut self.city,
            &mut self.rng,
            &threats,
            &shots,
            dt,
        );
    }

    /// Radfahrer: ein Anteil der Fußgänger-Zielzahl, bei Regen, Schnee und Sturm weniger (nur mit Tagesrhythmus).
    pub fn bike_target(&self) -> usize {
        if !self.rhythm {
            return 0;
        }
        (self.ped_target as f64 * crate::bikes::SHARE * crate::bikes::weather_factor(&self.sky.p))
            .round() as usize
    }
    fn spawn_bike(&mut self, min_r: f64, max_r: f64) -> Option<u32> {
        use crate::bikes;
        let (cx, cy) = (self.camera.x, self.camera.y);
        let (lane, s) = bikes::spawn_spot(
            &self.city,
            &mut self.lanes,
            &mut self.bike_paths,
            &mut self.rng,
            cx,
            cy,
            min_r,
            max_r,
        )?;
        let kind = if self.rng.float() < bikes::SCOOTER_SHARE {
            bikes::Kind::Scooter
        } else {
            bikes::Kind::Bike
        };
        let b = bikes::create(
            self.next_bike,
            &self.city,
            &self.lanes,
            &mut self.bike_paths,
            lane,
            s,
            &mut self.rng,
            kind,
        )?;
        if self
            .cars
            .iter()
            .any(|c| (c.x - b.x).abs() < c.hw + 12. && (c.y - b.y).abs() < c.hw + 12.)
            || self
                .bikes
                .iter()
                .any(|o| (o.x - b.x).hypot(o.y - b.y) < 30.)
        {
            return None;
        }
        self.next_bike += 1;
        let id = b.id;
        self.bikes.push(b);
        Some(id)
    }
    /// Fahrer runter (Auto, Schuss, Schlag): das Rad bleibt liegen, der Fahrer wird ein Passant, der (bei `fall`)
    /// erschrocken wegläuft. Liefert den Index des neuen Passanten.
    pub fn dismount(&mut self, i: usize, from: (f64, f64), fall: bool) -> Option<usize> {
        let b = &mut self.bikes[i];
        b.state = crate::bikes::State::Lying;
        b.t = 0.;
        b.speed = 0.;
        b.cross = None;
        let (x, y, shirt, lvl) = (b.x, b.y, b.shirt(), b.level.lvl);
        let sp = pedestrians::nearest_spot(&mut self.city, &mut self.sidewalks, x, y, 600.)?;
        let id = self.next_ped;
        self.next_ped += 1;
        let mut p = create_ped(id, &mut self.city, &mut self.sidewalks, sp, &mut self.rng);
        (p.x, p.y, p.shirt) = (x, y, shirt);
        p.level.lvl = lvl;
        p.level_init = true;
        if fall {
            scare(&mut p, from, 3.);
        }
        self.peds.push(p);
        Some(self.peds.len() - 1)
    }
    /// Rad nehmen: aus dem Radfahrer wird ein Fahrzeug der Art Fahrrad/E-Roller (Autophysik), ein Fahrer wird
    /// heruntergezogen und flieht (bei Tempo stürzt er). Liefert die Kennung des neuen Fahrzeugs.
    fn take_bike(&mut self, i: usize) -> u32 {
        let b = self.bikes[i].clone();
        let kind = if b.kind == crate::bikes::Kind::Scooter {
            "escooter"
        } else {
            "bicycle"
        };
        let id = self.new_car_id();
        let mut car = Car::new(id, b.x, b.y, b.angle, 0x1e272e, Role::Parked, kind);
        let v = if b.state == crate::bikes::State::Ride {
            b.speed * 0.3
        } else {
            0.
        };
        (car.vx, car.vy) = (b.angle.cos() * v, b.angle.sin() * v);
        car.level = b.level;
        car.level_init = true;
        if b.state == crate::bikes::State::Ride {
            let (px, py) = (self.player.x, self.player.y);
            if let Some(k) = self.dismount(i, (px, py), b.speed > 40.) {
                let side = (-b.angle.sin() * 10., b.angle.cos() * 10.);
                let q = &mut self.peds[k];
                (q.x, q.y) = (b.x + side.0, b.y + side.1);
                scare(q, (px, py), 3.5);
            }
            self.events.push(Event::Carjack {
                x: b.x,
                y: b.y,
                bike: true,
            });
        }
        self.bikes.remove(i);
        self.cars.push(car);
        id
    }
    /// Räder fahren lassen; wer von einem schnellen Auto erwischt wird, stürzt (Fahrer flieht, Rad bleibt liegen).
    fn update_bikes(&mut self, dt: f64) {
        use crate::bikes::{self, Obstacle, State};
        let mut obstacles: Vec<Obstacle> =
            Vec::with_capacity(self.cars.len() + self.peds.len() + self.bikes.len() + 1);
        for c in &self.cars {
            obstacles.push(Obstacle {
                x: c.x,
                y: c.y,
                lat: c.hh + 5.,
                len: c.hw,
                car: Some((c.angle, c.vx.abs() + c.vy.abs() < 20.)),
                bike: None,
            });
        }
        for p in self.peds.iter().filter(|p| p.state != PedState::Dead) {
            obstacles.push(Obstacle {
                x: p.x,
                y: p.y,
                lat: 9.,
                len: 0.,
                car: None,
                bike: None,
            });
        }
        for o in self.bikes.iter().filter(|o| o.state == State::Ride) {
            obstacles.push(Obstacle {
                x: o.x,
                y: o.y,
                lat: 8.,
                len: 0.,
                car: None,
                bike: Some(o.id),
            });
        }
        if self.player.in_car.is_none() {
            obstacles.push(Obstacle {
                x: self.player.x,
                y: self.player.y,
                lat: 10.,
                len: 0.,
                car: None,
                bike: None,
            });
        }
        let mut hits = Vec::new();
        for i in 0..self.bikes.len() {
            let b = &mut self.bikes[i];
            bikes::update(
                b,
                &self.city,
                &mut self.lanes,
                &mut self.bike_paths,
                &mut self.rng,
                self.time,
                &obstacles,
                dt,
            );
            if b.state != State::Ride {
                continue;
            }
            for c in &self.cars {
                if (c.x - b.x).abs() > c.hw + 8. || (c.y - b.y).abs() > c.hw + 8. || c.speed() < 60.
                {
                    continue;
                }
                if circle_vs_obb(b.x, b.y, bikes::RADIUS, &c.obb()).is_some() {
                    hits.push((i, c.id, c.x, c.y, c.speed()));
                    break;
                }
            }
        }
        let player_car = self.player.in_car;
        for (i, car, cx, cy, speed) in hits {
            let (x, y) = (self.bikes[i].x, self.bikes[i].y);
            self.dismount(i, (cx, cy), true);
            self.events.push(Event::Hit {
                x,
                y,
                car,
                player: Some(car) == player_car,
                speed,
                bike: true,
            });
        }
        // Fernes, Verschwundenes und lange Liegendes außer Sicht abbauen; Fehlendes im Ring erzeugen
        let (cx, cy) = (self.camera.x, self.camera.y);
        self.bikes.retain(|b| {
            let d = (b.x - cx).hypot(b.y - cy);
            b.state != State::Gone
                && d < DESPAWN
                && !(b.state == State::Lying && b.t > 30. && d > 900.)
        });
        let riding = self.bikes.iter().filter(|b| b.state == State::Ride).count();
        let target = self.bike_target();
        if riding < target {
            self.spawn_bike(SPAWN_MIN, SPAWN_MAX);
        } else if riding > target + 2
            && let Some(k) = self
                .bikes
                .iter()
                .position(|b| (b.x - cx).hypot(b.y - cy) > SPAWN_MIN)
        {
            self.bikes.remove(k);
        }
        if self.time % 5. < dt {
            self.bike_paths.prune(&self.lanes);
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
        self.bikes.clear();
        self.animals.clear();
        self.flocks.clear();
        self.scooters.clear();
        self.puddles.clear();
        self.transit_state = crate::transit::State {
            seed: self.seed,
            ..Default::default()
        };
        self.transit_populated = false;
        self.rail_obs.clear();
        self.player_train = None;
        self.player.inside = None;
        self.player.entry_guard = None;
        self.hangers.clear();
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
                    rails: &[],
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
            // Fahrzeugart nach Uhrzeit, Wochentag und Straße (höchstens ein Müllauto in der Nähe)
            let cls = self
                .lanes
                .lane(lane)
                .and_then(|l| self.city.edges.get(&l.edge))
                .map_or(5, |e| e.cls);
            let mut kind = if self.rhythm {
                crate::fleet::pick_kind(self.clock, self.day, cls, self.rng.float())
            } else {
                "car"
            };
            if kind == "garbage" && self.cars.iter().any(|o| o.kind == "garbage") {
                kind = "car";
            }
            let pal = crate::carmodels::kind(kind).colors;
            if kind != "car" && !self.cars.iter().all(|o| (o.x - x).hypot(o.y - y) > 110.) {
                continue;
            }
            let color = pal[self.rng.index(pal.len())];
            return self.put_npc_car_colored(lane, s, x, y, kind, color);
        }
        None
    }

    /// KI-Auto der Art `kind` auf eine Spur setzen (Engstelle wird beansprucht); Farbe aus der Art.
    pub fn put_npc_car(
        &mut self,
        lane: crate::roadgraph::LaneId,
        s: f64,
        x: f64,
        y: f64,
        kind: &'static str,
    ) -> Option<u32> {
        let free = {
            let agents: Vec<Agent> = self.cars.iter().map(Agent::of).collect();
            let mut ev = Vec::new();
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
                rails: &[],
            };
            narrow_free(&mut ctx, lane)
        };
        if !free {
            return None;
        }
        let color = crate::carmodels::kind(kind)
            .colors
            .first()
            .copied()
            .unwrap_or(0xcccccc);
        self.put_npc_car_colored(lane, s, x, y, kind, color)
    }
    fn put_npc_car_colored(
        &mut self,
        lane: crate::roadgraph::LaneId,
        s: f64,
        x: f64,
        y: f64,
        kind: &'static str,
        color: u32,
    ) -> Option<u32> {
        let agents: Vec<Agent> = self.cars.iter().map(Agent::of).collect();
        let mut ev = Vec::new();
        {
            let id = self.new_car_id();
            let mut car = Car::new(id, x, y, 0., color, Role::Traffic, kind);
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
                rails: &[],
            };
            // direkt an der Linie geboren: nur, wenn die Einfahrt frei ist (sonst Gegenverkehr in der Engstelle)
            if !crate::traffic::spawn_allowed(&mut ctx, &car) {
                return None;
            }
            claim_narrow(&mut ctx, id, lane, seg);
            self.cars.push(car);
            Some(id)
        }
    }

    /// Aufnahmen (`--fahrzeugschau`): je ein Fahrzeug jeder Art auf der nächsten Fahrspur hintereinander, stehend,
    /// Paketwagen mit Warnblinker, Müllauto bei der Arbeit, Einsatzfahrzeuge mit Blaulicht.
    /// Fahrzeug neben der Figur abstellen (Befehlszeile `auto`): Art aus `carmodels::KINDS`, optional ein Pkw-Modell.
    /// Vorführung (`--drift-demo`): das Spielerauto mittig auf die nächste Spur stellen, in Spurrichtung mit
    /// `speed` px/s anschieben und andere Autos im Umkreis entfernen. Liefert, ob es geklappt hat.
    pub fn demo_launch(&mut self, speed: f64) -> bool {
        let Some(i) = self.player.in_car.and_then(|id| self.car_index(id)) else {
            return false;
        };
        let (x, y, a) = (self.cars[i].x, self.cars[i].y, self.cars[i].angle);
        let Some(hit) = self.lanes.nearest_lane(x, y, Some(a), 300., false) else {
            return false;
        };
        let Some(l) = self.lanes.lane(hit.lane) else {
            return false;
        };
        let (p, q) = (l.pts[hit.i], l.pts[(hit.i + 1).min(l.pts.len() - 1)]);
        let ang = (q.1 - p.1).atan2(q.0 - p.0);
        let id = self.cars[i].id;
        let c = &mut self.cars[i];
        (c.x, c.y, c.angle) = (hit.x, hit.y, ang);
        (c.vx, c.vy) = (ang.cos() * speed, ang.sin() * speed);
        if let Some(s) = c.phys.as_mut() {
            s.yaw = -ang;
        }
        self.cars
            .retain(|c| c.id == id || (c.x - hit.x).hypot(c.y - hit.y) > 700.);
        true
    }

    /// Fahrzeug aus den Fahrzeugdaten (z. B. `sattelzug_40t`) neben der Spielfigur abstellen: Art nach Klasse,
    /// Maße aus den Daten (beim Gespann nur das Zugfahrzeug; der Anhänger hängt am Gelenk).
    pub fn spawn_data_vehicle(&mut self, id: &str) -> Option<u32> {
        let v = crate::vehdata::shared().get(id)?;
        let kind = data_kind(v);
        let len = v.hitch.as_ref().map_or(v.length, |h| h.front_len);
        let a = self.player.angle;
        let (x, y) = (self.player.x + a.cos() * 80., self.player.y + a.sin() * 80.);
        let cid = self.new_car_id();
        let (hw, hh) = (len * 5., v.width * 5.);
        let (sx, sy, angle) = self.open_spot(x, y, Some((cid, hw, hh)))?;
        let color = CAR_COLORS[(cid as usize) % CAR_COLORS.len()];
        let mut c = Car::new(cid, sx, sy, angle, color, Role::Parked, kind);
        (c.hw, c.hh) = (hw, hh);
        // Pkw: Maße für Bild und Kollision wie im Verkehr (body_dims), sonst aus den Daten
        c.set_model(v.id.as_str());
        c.level.lvl = self.player.level.lvl;
        self.cars.push(c);
        Some(cid)
    }

    pub fn spawn_vehicle(&mut self, kind: &str, model: Option<&'static str>) -> Option<u32> {
        let k = crate::carmodels::kind(kind);
        let a = self.player.angle;
        let (x, y) = (self.player.x + a.cos() * 60., self.player.y + a.sin() * 60.);
        let id = self.new_car_id();
        let (sx, sy, angle) = self.open_spot(x, y, Some((id, k.l / 2., k.w / 2.)))?;
        let color = CAR_COLORS[(id as usize) % CAR_COLORS.len()];
        let mut c = Car::new(id, sx, sy, angle, color, Role::Parked, k.name);
        if let Some(m) = model {
            c.set_model(m);
        }
        c.level.lvl = self.player.level.lvl;
        self.cars.push(c);
        Some(id)
    }

    /// Aufnahmen: je eine Person jeder Art steht in einer Reihe neben der Spielfigur, die Kamera geht nah heran.
    pub fn people_show(&mut self) {
        let (px, py) = (self.player.x, self.player.y);
        let Some(sp) = pedestrians::nearest_spot(&mut self.city, &mut self.sidewalks, px, py, 400.)
        else {
            return;
        };
        self.peds.clear();
        self.hangers.clear();
        self.foot_zoom = 3.2;
        let (ox, oy) = self.sidewalks.point(&mut self.city, sp.edge, sp.side, sp.s);
        let (bx, by) = self
            .sidewalks
            .point(&mut self.city, sp.edge, sp.side, sp.s + 2.);
        let l = (bx - ox).hypot(by - oy).max(1e-6);
        let (ux, uy) = ((bx - ox) / l, (by - oy) / l);
        for (i, kind) in crate::figure::ALL.into_iter().enumerate() {
            let id = self.next_ped;
            self.next_ped += 1;
            let mut p = create_ped(id, &mut self.city, &mut self.sidewalks, sp, &mut self.rng);
            // geradlinig in Gehweg-Richtung, Blick quer dazu
            let k = i as f64 - 5.5;
            (p.x, p.y) = (ox + ux * k * 24., oy + uy * k * 24.);
            p.facing = uy.atan2(ux) - std::f64::consts::FRAC_PI_2;
            p.state = PedState::Idle;
            p.t = 1e9;
            p.kind = kind;
            p.style = match kind {
                crate::figure::Kind::Jogger => crate::figure::Style::Jog,
                crate::figure::Kind::Dogwalker => crate::figure::Style::Dog,
                _ => crate::figure::Style::Plain,
            };
            self.peds.push(p);
        }
        // Spielfigur ans Ende der Reihe, damit sie niemanden verdeckt
        (self.player.x, self.player.y) = (ox - ux * 170., oy - uy * 170.);
    }

    pub fn vehicle_show(&mut self) {
        let (px, py) = (self.player.x, self.player.y);
        let Some(hit) = self.lanes.nearest_lane(px, py, None, 400., false) else {
            return;
        };
        let kinds = [
            // Gespanne aus den Fahrzeugdaten, leicht geknickt
            ("truck", Some("sattelzug_40t")),
            ("bus", Some("gelenkbus")),
            ("car", None),
            ("truck", None),
            ("delivery", None),
            ("garbage", None),
            ("police", None),
            ("ambulance", None),
            ("motorcycle", None),
            ("scooter", None),
        ];
        let (mut lane, mut s) = (hit.lane, 40.);
        for (kind, model) in kinds {
            let k = crate::carmodels::kind(kind);
            let data = model.and_then(|m| crate::vehdata::shared().get(m));
            // ganze Länge des Gespanns
            let len = data.map_or(k.l, |v| v.length * 10.);
            s += len / 2.;
            // über das Spurende hinaus auf der geradesten Folgespur weiter
            while let Some(len) = self.lanes.lane(lane).map(|l| l.len).filter(|&len| s > len) {
                let next = self.lanes.next(&self.city, lane, false);
                let Some(&n) = next.first() else { return };
                s -= len;
                lane = n;
            }
            let Some((x, y)) = self.lanes.lane(lane).map(|l| {
                let p = crate::city::point_along(&l.pts, s);
                (p.x, p.y)
            }) else {
                return;
            };
            if let Some(id) = self.put_npc_car(lane, s, x, y, kind)
                && let Some(c) = self.cars.iter_mut().find(|c| c.id == id)
            {
                if let Some(ai) = c.ai.as_mut() {
                    ai.hold = 1e9;
                }
                c.hazard = kind == "delivery";
                c.work = kind == "garbage";
                c.blue = matches!(kind, "police" | "ambulance");
                if let Some(v) = data {
                    c.model = Some(v.id.as_str());
                    let front = v.hitch.as_ref().map_or(v.length, |h| h.front_len);
                    (c.hw, c.hh) = (front * 5., v.width * 5.);
                    // Zugfahrzeug nach vorn, damit der Anhänger auf dem Platz des Gespanns steht
                    let shift = (v.length - front) * 5.;
                    let (sa, ca) = c.angle.sin_cos();
                    (c.x, c.y) = (c.x + ca * shift, c.y + sa * shift);
                    c.phys = Some(Box::new(crate::vphys::State {
                        art: 0.25,
                        ..Default::default()
                    }));
                }
            }
            s += len / 2. + 14.;
        }
    }

    /// Auto entfernen (Reservierungen freigeben).
    pub fn remove_car(&mut self, id: u32) {
        drop_claims(&mut self.res, id);
        self.cars.retain(|c| c.id != id);
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
        if self.day_rhythm {
            // Jogger und Hundehalter je nach Tageszeit
            p.style = crate::figure::walker_style(self.clock, &mut self.rng);
            if p.style == crate::figure::Style::Jog {
                p.shirt = crate::figure::JOG_SHIRTS[id as usize % 5];
            }
        }
        self.assign_kind(&mut p, None);
        self.peds.push(p);
        Some(id)
    }

    /// Menschen-Typ (`figure.rs`): Jogger/Hundehalter aus ihrem Stil, sonst nach Ort, Uhrzeit, Wochentag (und
    /// Tätigkeit) aus der Nummer, ohne den Welt-Zufall zu verbrauchen. Der Typ bestimmt das Gehtempo mit.
    fn assign_kind(&self, p: &mut Ped, act: Option<crate::life::Act>) {
        use crate::figure::{Ctx, Kind, Style, pick_kind};
        p.kind = match p.style {
            Style::Jog => Kind::Jogger,
            Style::Dog => Kind::Dogwalker,
            Style::Plain => pick_kind(
                p.id,
                &Ctx {
                    minutes: self.clock,
                    day: self.day,
                    bezirk: self.city.bezirk_at(p.x, p.y),
                    act,
                },
            ),
        };
        p.speed *= p.kind.speed();
    }

    fn manage_population(&mut self) {
        let (cx, cy) = (self.camera.x, self.camera.y);
        if self.day_rhythm && self.time - self.rhythm_t >= 2. {
            // Tageszeit und Ort bestimmen, wie viel los ist
            self.rhythm_t = self.time;
            self.set_targets();
        }
        let keep = |w: &World, c: &Car| {
            Some(c.id) == w.player_car_id
                || Some(c.id) == w.player.in_car
                || c.cargo
                || matches!(c.role, Role::Parked | Role::Curb)
                || c.driver == Some(Driver::Player)
                || (c.duty.is_some() && !c.done)
                || c.bus.is_some()
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
                c.bus.is_none()
                    && (c.driver == Some(Driver::Npc)
                        || (c.driver.is_none() && c.role == Role::Traffic))
            })
            .count();
        if npc < self.car_target {
            self.spawn_traffic(SPAWN_MIN, SPAWN_MAX);
        }
        // weniger los als eben (Tageszeit, anderer Ort): Überzählige außer Sicht verschwinden lassen, eins je Schritt
        if self.day_rhythm && npc > self.car_target + 2 {
            let i = self.cars.iter().position(|c| {
                c.driver == Some(Driver::Npc)
                    && !keep(self, c)
                    && (c.x - cx).hypot(c.y - cy) > SPAWN_MIN
            });
            if let Some(i) = i {
                drop_claims(&mut self.res, self.cars[i].id);
                self.cars.remove(i);
            }
        }
        let walkers = self
            .peds
            .iter()
            .filter(|q| q.state == PedState::Walk && q.hang.is_none())
            .count();
        if self.day_rhythm && walkers > self.ped_target + 4 {
            let i = self.peds.iter().position(|q| {
                q.state == PedState::Walk
                    && q.hang.is_none()
                    && (q.x - cx).hypot(q.y - cy) > SPAWN_MIN
            });
            if let Some(i) = i {
                self.peds.remove(i);
            }
        }
        if self
            .peds
            .iter()
            .filter(|p| p.state != PedState::Dead && p.hang.is_none())
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
        let over = p.z >= JUMP_CLEAR;
        for h in self.city.solids.query(&Rect::around(p.x, p.y, r + 2.)) {
            let s = *self.city.solids.get(h);
            if !car::blocks(&self.knocked, &s, lvl) || (over && jumpable(&s)) {
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
        // ein Rad (fahrend oder liegend) in Greifweite, näher als jedes Auto? Dann das nehmen
        let (px, py) = (self.player.x, self.player.y);
        let car_d = self
            .cars
            .iter()
            .filter(|c| !c.wrecked)
            .map(|c| (c.x - px).hypot(c.y - py))
            .fold(f64::INFINITY, f64::min);
        let bike = self
            .bikes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.state != crate::bikes::State::Gone)
            .map(|(i, b)| (i, (b.x - px).hypot(b.y - py)))
            .filter(|(_, d)| *d < crate::bikes::GRAB && *d < car_d)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((i, _)) = bike {
            let id = self.take_bike(i);
            return self.try_enter_car(Some(id));
        }
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
            self.events.push(Event::Carjack { x, y, bike: false });
            self.fleeing_driver(i, px, py, 3.5);
        }
        let id = self.cars[i].id;
        drop_claims(&mut self.res, id);
        let c = &mut self.cars[i];
        c.driver = Some(Driver::Player);
        c.ai = None;
        c.controls = Default::default();
        c.dyn_state = None;
        c.phys = None;
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
        let landed = std::mem::take(&mut self.player.landed);
        if input.move_x.hypot(input.move_y) > 0.05 {
            self.player.click = None;
            self.player.click_goal = None;
            return out;
        }
        let lvl = self.player.level.lvl;
        let (px, py) = (self.player.x, self.player.y);
        let walk = |w: &mut World, to: (f64, f64)| {
            // im Bahnhof: gerade Linie, keep_inside hält am Bahnsteig
            if let Some(st) = w
                .player
                .inside
                .as_ref()
                .and_then(|i| w.stations.by_id.get(&i.id))
            {
                let (mut x, mut y) = to;
                st.keep_inside(&mut x, &mut y, PLAYER_RADIUS);
                return Some(vec![(px, py), (x, y)]);
            }
            crate::footpath::find_foot_path(w, (px, py), to, lvl).filter(|p| p.len() > 1)
        };
        if let Some(at) = input.click_world {
            if input.click_pressed {
                self.player.click_t = 0.15;
                self.player.click_goal = Some(at);
                self.player.click = self.click_intent(
                    at,
                    input.click_force && input.click_attack,
                    input.click_attack,
                    walk,
                );
                // nur ein Laufklick hat ein Ziel, über das gesprungen werden kann
                if !matches!(self.player.click, Some(Click::Walk { .. })) {
                    self.player.click_goal = None;
                }
            } else if input.click_held {
                match &mut self.player.click {
                    Some(Click::Walk { follow: true, .. }) => {
                        self.player.click_t -= dt;
                        if self.player.click_t <= 0. {
                            // gehalten: dem Zeiger nachlaufen (Weg alle 0,15 s neu)
                            self.player.click_t = 0.15;
                            self.player.click_goal = Some(at);
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
        // nach der Landung: weiter zum Klickziel, Weg von hier neu (das Ziel lag hinter dem Zaun)
        if landed
            && let Some(g) = self.player.click_goal
            && matches!(self.player.click, None | Some(Click::Walk { .. }))
        {
            self.player.click = if (g.0 - px).hypot(g.1 - py) > 6. {
                walk(self, g).map(|path| Click::Walk {
                    path,
                    i: 1,
                    follow: false,
                })
            } else {
                None
            };
        }
        // Sprung bei Klicksteuerung: Richtung Klickziel, nicht entlang des Umwegs
        if input.jump
            && let Some(g) = self.player.click_goal
            && matches!(self.player.click, None | Some(Click::Walk { .. }))
            && (g.0 - px).hypot(g.1 - py) > 6.
        {
            let d = (g.0 - px).hypot(g.1 - py);
            out.move_x = (g.0 - px) / d;
            out.move_y = (g.1 - py) / d;
            return out;
        }
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
            Click::Approach {
                car,
                mut path,
                mut i,
                mut to,
            } => {
                let Some((cx, cy)) = self
                    .car(car)
                    .filter(|c| Some(c.id) != self.player.in_car)
                    .map(|c| (c.x, c.y))
                else {
                    self.player.click = None;
                    return out;
                };
                let d = (cx - px).hypot(cy - py);
                if d < ENTER_DIST - 2. {
                    // daneben angekommen: stehen bleiben, zum Auto schauen – einsteigen nur per Taste
                    stop(&mut out);
                    self.player.angle = (cy - py).atan2(cx - px);
                    self.player.click = None;
                    return out;
                }
                // kein Vorankommen (ein wartendes Auto steht im Weg und wartet seinerseits
                // auf den Spieler): neu planen, stehende Autos sind dann Hindernis
                let st = &mut self.player.click_stall;
                if (px - st.0).hypot(py - st.1) > 4. {
                    *st = (px, py, 0.);
                } else {
                    st.2 += dt;
                }
                let stalled = st.2 > CLICK_STALL;
                if stalled {
                    st.2 = 0.;
                }
                // Weg zum Auto (um Häuser herum), neu, wenn es weggefahren ist
                if stalled || path.is_empty() || (cx - to.0).hypot(cy - to.1) > 30. {
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
                self.player.click = Some(Click::Approach { car, path, i, to });
            }
            Click::Bike { bike } => {
                // nur fahrende Radler, bis der Fahrer vom Rad ist
                let Some(b) = self
                    .bikes
                    .iter()
                    .find(|b| b.id == bike && b.state == crate::bikes::State::Ride)
                    .map(|b| (b.x, b.y))
                else {
                    self.player.click = None;
                    return out;
                };
                let d = (b.0 - px).hypot(b.1 - py);
                let wp = self.player.combat.weapon();
                let reach = if wp.melee {
                    wp.range + 6.
                } else {
                    wp.range * 0.85
                };
                if d > reach {
                    toward(&mut out, (b.0, b.1));
                } else {
                    out.combat.aim_world = Some((b.0, b.1));
                    out.combat.fire = true;
                    out.combat.fire_pressed = self.player.combat.cool <= 0.;
                }
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

    /// Was ein Klick bedeutet: Person bzw. fahrender Radler → angreifen (nur mit `attack`), Auto → danebenstellen,
    /// sonst (Boden, Wrack, liegendes Rad) hinlaufen; mit Strg am Platz angreifen. Eingestiegen wird nie per Klick.
    /// Ohne `attack` greift ein Klick nie an: Personen und fahrende Radler werden nur angelaufen.
    fn click_intent(
        &mut self,
        at: (f64, f64),
        force: bool,
        attack: bool,
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
        if let Some(id) = ped.filter(|_| attack) {
            return Some(Click::Target {
                ped: id,
                done: false,
            });
        }
        let riding = self
            .bikes
            .iter()
            .filter(|b| b.state == crate::bikes::State::Ride && b.level.lvl == lvl)
            .find(|b| (b.x - at.0).hypot(b.y - at.1) < crate::bikes::RADIUS + 5.)
            .map(|b| b.id);
        if let Some(id) = riding.filter(|_| attack) {
            return Some(Click::Bike { bike: id });
        }
        let car = self
            .cars
            .iter()
            .filter(|c| c.lvl() == lvl && Some(c.id) != self.player.in_car && !c.wrecked)
            .find(|c| {
                let (s, co) = c.angle.sin_cos();
                let (dx, dy) = (at.0 - c.x, at.1 - c.y);
                (dx * co + dy * s).abs() < c.hw + 2. && (-dx * s + dy * co).abs() < c.hh + 2.
            })
            .map(|c| c.id);
        if let Some(id) = car {
            return Some(Click::Approach {
                car: id,
                path: Vec::new(),
                i: 1,
                to: (f64::NAN, f64::NAN),
            });
        }
        let path = walk(self, at)?;
        Some(Click::Walk {
            path,
            i: 1,
            follow: true,
        })
    }

    /// Teleport-Ziel zu einem Kartenpunkt (world.js findTeleportSpot): abseits der Fahrbahn genau dorthin bzw. an die
    /// nächste freie Stelle bis 30 m, sonst im Auto auf die nächste Fahrspur (in Fahrtrichtung), zu Fuß auf den
    /// nächsten Gehweg. `None` außerhalb Berlins oder ohne passende Stelle.
    pub fn find_teleport_spot(&mut self, x: f64, y: f64) -> Option<TeleportSpot> {
        if !self.city.inside_border(x, y) {
            return None;
        }
        if !self.city.focus("teleport", x, y) {
            return Some(TeleportSpot::Pending);
        }
        self.lanes.sync(&mut self.city);
        let car = self.player_car().map(|c| (c.id, c.hw, c.hh));
        let ground = self.city.surface_at(x, y, None);
        let mut spot = None;
        if matches!(ground, Ground::Grass | Ground::Plaza | Ground::Sidewalk) {
            spot = self.open_spot(x, y, car);
        }
        if spot.is_none() {
            spot = if car.is_some() {
                self.lanes
                    .nearest_lane(x, y, None, 3000., false)
                    .and_then(|h| {
                        let p = &self.lanes.lane(h.lane)?.pts;
                        let (a, b) = (p[h.i], p[(h.i + 1).min(p.len() - 1)]);
                        Some((h.x, h.y, (b.1 - a.1).atan2(b.0 - a.0)))
                    })
            } else {
                pedestrians::nearest_spot(&mut self.city, &mut self.sidewalks, x, y, 3000.).map(
                    |sp| {
                        let (px, py) = self.sidewalks.point(&mut self.city, sp.edge, sp.side, sp.s);
                        (px, py, 0.)
                    },
                )
            };
        }
        let (sx, sy, angle) = spot?;
        if !self.city.inside_border(sx, sy) || self.city.in_building(sx, sy).is_some() {
            return None;
        }
        let name = self.city.location_name(sx, sy);
        Some(TeleportSpot::Spot {
            x: sx,
            y: sy,
            angle,
            name,
        })
    }
    /// Freie Stelle auf offenem Grund um (x, y), zuerst der Punkt selbst (world.js openSpot).
    fn open_spot(
        &mut self,
        x: f64,
        y: f64,
        car: Option<(u32, f64, f64)>,
    ) -> Option<(f64, f64, f64)> {
        let s = self.city.scale;
        let angles: &[f64] = if car.is_some() {
            &[
                0.,
                std::f64::consts::FRAC_PI_2,
                std::f64::consts::FRAC_PI_4,
                -std::f64::consts::FRAC_PI_4,
            ]
        } else {
            &[0.]
        };
        let mut r = 0.;
        while r <= 30. * s {
            let n = if r == 0. {
                1
            } else {
                ((std::f64::consts::TAU * r / (2. * s)).round() as usize).max(8)
            };
            for k in 0..n {
                let a0 = k as f64 / n as f64 * std::f64::consts::TAU;
                let (px, py) = (x + a0.cos() * r, y + a0.sin() * r);
                for &a in angles {
                    if self.spot_free(px, py, a, car) {
                        return Some((px, py, a));
                    }
                }
            }
            r += 2. * s;
        }
        None
    }
    fn spot_free(&mut self, px: f64, py: f64, angle: f64, car: Option<(u32, f64, f64)>) -> bool {
        if !self.city.inside_border(px, py) {
            return false;
        }
        let t = self.city.surface_at(px, py, None);
        if matches!(t, Ground::Building | Ground::Water) || self.city.in_building(px, py).is_some()
        {
            return false;
        }
        let Some((id, hw, hh)) = car else {
            return crate::footpath::foot_free(self, px, py, 0);
        };
        let probe = Obb {
            x: px,
            y: py,
            angle,
            hw: hw + 4.,
            hh: hh + 4.,
        };
        let (c, sn) = (angle.cos(), angle.sin());
        for h in self.city.solids.query(&Rect::around(px, py, hw + hh + 8.)) {
            let sol = *self.city.solids.get(h);
            if !crate::car::blocks(&self.knocked, &sol, 0) {
                continue;
            }
            let hit = match sol {
                Solid::Wall { seg, .. } => crate::collision::obb_vs_segment(&probe, &seg).is_some(),
                Solid::Circle { x, y, r, .. } => circle_vs_obb(x, y, r, &probe).is_some(),
                Solid::Rect(rc) => crate::collision::obb_vs_rect(&probe, &rc).is_some(),
            };
            if hit {
                return false;
            }
        }
        // die ganze Karosserie auf festem Grund (nicht halb im Wasser)
        for (u, v) in [(1., 1.), (-1., 1.), (1., -1.), (-1., -1.)] {
            let (cx, cy) = (
                px + c * u * probe.hw - sn * v * probe.hh,
                py + sn * u * probe.hw + c * v * probe.hh,
            );
            if self.city.surface_at(cx, cy, None) == Ground::Water {
                return false;
            }
        }
        self.cars
            .iter()
            .all(|o| o.id == id || (o.x - px).hypot(o.y - py) > 40.)
    }
    /// Teleport ausführen: eigenes Auto bzw. Figur dorthin, Verkehr und Passanten am neuen Ort aufbauen.
    pub fn teleport_to(&mut self, x: f64, y: f64, angle: f64) {
        if let Some(i) = self.player.in_car.and_then(|id| self.car_index(id)) {
            let c = &mut self.cars[i];
            (c.x, c.y, c.angle, c.vx, c.vy, c.ang_vel) = (x, y, angle, 0., 0., 0.);
            c.level_init = false;
            c.dyn_state = None;
            c.phys = None;
        }
        self.player.x = x;
        self.player.y = y;
        self.player.level_init = false;
        self.player.click = None;
        self.camera.x = x;
        self.camera.y = y;
        self.reset_population();
        self.stream();
        self.city.release("teleport");
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
        // wer Auto fährt, schiebt keinen Kinderwagen
        p.kind = if crate::math::hash01(f64::from(id) * 7.3 + 1.) < 0.3 {
            crate::figure::Kind::Business
        } else {
            crate::figure::Kind::Everyday
        };
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
        car.phys = None;
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
        // Sprinten ohne Ausdauergrenze (Wunsch 06.10.2026: „unendlich lang rennen“)
        let sprinting = want_sprint;
        // Sprung: nur vom Boden, nicht schwimmend, nicht benommen, nicht im Bahnhof
        let airborne = p.z > 0. || p.vz > 0.;
        if input.jump && !airborne && !p.swimming && !stunned && p.inside.is_none() {
            p.vz = JUMP_V;
            // mit Anlauf: Richtung bleibt, mindestens Jogg-Tempo (Hechtsprung); aus dem Stand: senkrecht
            p.jump_v = if mag > 0.05 {
                let l = mx.hypot(my);
                let v = if sprinting { SPRINT } else { JUMP_CARRY };
                // (gesprungen wird auch aus dem Stand am Zaun: die Klicksteuerung gibt dann die Richtung zum Ziel)
                (mx / l * v, my / l * v)
            } else {
                (0., 0.)
            };
            self.events.push(Event::Jump { x: p.x, y: p.y });
        }
        let p = &mut self.player;
        if p.z > 0. || p.vz > 0. {
            p.vz -= GRAVITY * dt;
            p.z += p.vz * dt;
            if p.z <= 0. {
                (p.z, p.vz) = (0., 0.);
                p.landed = true;
                self.events.push(Event::Land { x: p.x, y: p.y });
            }
        }
        let p = &mut self.player;
        if p.z > 0. || p.vz > 0. {
            // in der Luft: Schwung vom Absprung, keine Lenkung
            let (vx, vy) = p.jump_v;
            p.x += vx * dt;
            p.y += vy * dt;
            p.move_speed = vx.hypot(vy);
            if p.move_speed > 0. {
                p.angle = vy.atan2(vx);
            }
        } else if mag > 0.05 {
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
        // im U-Bahnhof: nur Bahnsteig und Säulen
        if let Some(st) = self
            .player
            .inside
            .as_ref()
            .and_then(|i| self.stations.by_id.get(&i.id))
        {
            let p = &mut self.player;
            st.keep_inside(&mut p.x, &mut p.y, PLAYER_RADIUS);
            p.swimming = false;
            return;
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
        for b in &mut self.bikes {
            step_level(&mut self.city, b.x, b.y, Some(b.angle), &mut b.level);
        }
        if let Some(c) = self
            .player
            .in_car
            .and_then(|id| self.cars.iter().find(|c| c.id == id))
        {
            self.player.level = c.level;
            self.player.level_init = true;
        } else if self.player.ride.is_some() {
            // im Wagen: update_ride setzt die Ebene (Gleis, Tunnel)
        } else if let Some(st) = self
            .player
            .inside
            .as_ref()
            .and_then(|i| self.stations.by_id.get(&i.id))
        {
            self.player.level.lvl = st.lvl; // auf dem Bahnsteig (Tunnel −2, Hochbahn wie das Gleis)
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
        // Aufschwimmen in einer Pfütze: Mitte, dann beide Vorderräder; nicht erneut, solange es noch schwimmt.
        // (Das Spielerauto mit Fahrphysik schwimmt über die Wasserhöhe je Rad auf, `wheel_env`.)
        let c = &self.cars[i];
        if c.phys.is_some() && c.driver == Some(crate::car::Driver::Player) {
            let (ax, ay) = self.gust_accel(&self.cars[i]);
            let c = &mut self.cars[i];
            c.vx += ax * dt;
            c.vy += ay * dt;
            return;
        }
        let (ca, sa) = (c.angle.cos(), c.angle.sin());
        let vf = c.vx * ca + c.vy * sa;
        if !cond.covered
            && w.wet > crate::traction::PUDDLE_WET
            && vf > crate::traction::Aqua::SPEED
            && c.aqua <= 0.
        {
            let (fx, fy, lvl) = (c.hw * 0.7, c.hh * 0.8, c.lvl());
            let probes = [
                (c.x, c.y),
                (c.x + ca * fx + sa * fy, c.y + sa * fx - ca * fy),
                (c.x + ca * fx - sa * fy, c.y + sa * fx + ca * fy),
            ];
            let hit = probes
                .into_iter()
                .find_map(|(x, y)| self.puddle_at(x, y, lvl));
            if let Some(p) = hit {
                let id = self.cars[i].id;
                let player = self.player.in_car == Some(id);
                let c = &mut self.cars[i];
                c.aqua = crate::traction::Aqua::TIME;
                c.aqua_yaw = crate::traction::aqua_yaw(&p);
                let (x, y) = (c.x, c.y);
                self.events.push(Event::Aquaplane {
                    x,
                    y,
                    car: id,
                    player,
                });
            }
        }
        // Sturmböe schiebt fahrende Autos quer (traction.js gustPush)
        let (ax, ay) = self.gust_accel(&self.cars[i]);
        let c = &mut self.cars[i];
        c.vx += ax * dt;
        c.vy += ay * dt;
    }

    /// Zweirad gestürzt: der Fahrer fliegt in Fahrtrichtung ab, landet benommen und verletzt sich je nach Tempo;
    /// das Rad rutscht liegend aus (wieder aufsteigen richtet es auf).
    pub fn throw_rider(&mut self, i: usize, why: crate::twowheel::Fall) {
        let c = &mut self.cars[i];
        let (x, y, vx, vy, lvl) = (c.x, c.y, c.vx, c.vy, c.lvl());
        let speed = vx.hypot(vy);
        c.driver = None;
        c.controls = Default::default();
        let level = c.level;
        // Landepunkt: ein Stück voraus, wenn dort Platz ist, sonst neben dem Rad
        let fly = (speed * 0.12).min(60.);
        let (dx, dy) = if speed > 1. {
            (vx / speed, vy / speed)
        } else {
            (1., 0.)
        };
        let cands = [
            (x + dx * fly, y + dy * fly),
            (x + dx * fly * 0.5, y + dy * fly * 0.5),
            (x - dy * 14., y + dx * 14.),
            (x + dy * 14., y - dx * 14.),
            (x, y),
        ];
        let spot = cands
            .into_iter()
            .find(|&(px, py)| {
                spot_free_static(&mut self.city, &self.knocked, px, py, PLAYER_RADIUS, lvl)
            })
            .unwrap_or((x, y));
        self.player.in_car = None;
        (self.player.x, self.player.y) = spot;
        self.player.level = level;
        self.player.stun = 1.2 + (speed / 300.).min(1.);
        self.events.push(Event::Bump {
            x: spot.0,
            y: spot.1,
        });
        self.notice = Some(Notice {
            text: why.label().into(),
            t: 2.,
        });
        // Verletzung nach Aufprallgeschwindigkeit (km/h ≈ px/s · 0,36)
        let dmg = (speed * 0.36 - 8.).max(0.) * 0.6;
        crate::combat::hurt_player(self, dmg, (x, y));
    }

    /// Untergrund je Rad eines Autos mit Fahrphysik (vorn links, vorn rechts, hinten links, hinten rechts): Belag
    /// aus der Karte, Straßenbahnschienen, Pfützen, Witterung; dazu Bordsteinwechsel und im Winter die Reifen.
    pub fn wheel_env(&mut self, i: usize, v: &crate::vehdata::Vehicle) -> crate::vphys::Env {
        use crate::city::Ground as G;
        use crate::surface::{Material, Spot, Weather, ground, resolve};
        use berlin_map_loader::citycodes::surface as sf;
        const CURB_M: f64 = 0.12;
        let w = self.weather;
        let c = &self.cars[i];
        let (x, y, lvl, id) = (c.x, c.y, c.lvl(), c.id);
        let (sa, ca) = c.angle.sin_cos();
        let (along, side) = (c.hw * 0.68, c.hh * 0.82);
        // links im Spielsystem (y nach unten) = (sin, −cos)
        let (lx, ly) = (sa, -ca);
        let wheels = [
            (along, side),
            (along, -side),
            (-along, side),
            (-along, -side),
        ];
        let any_weather = w.wet > 0. || w.snow > 0. || w.ice > 0. || w.glaze > 0.;
        let cond = if any_weather {
            crate::traction::road_condition(&mut self.city, &w, x, y, lvl)
        } else {
            Default::default()
        };
        // Winter: Alltagsautos fahren Winter- oder Ganzjahresreifen (fest je Auto)
        let winter = self.temp < 7. || w.snow > 0.01;
        let season = if winter && crate::vehdata::swaps_in_winter(&v.tire.id) {
            Some(if crate::math::hash01(id as f64 * 7.13) < 0.7 {
                "winter"
            } else {
                "ganzjahr"
            })
        } else {
            None
        };
        self.cars[i].season_tire = season;
        let v = crate::vehdata::game_vehicle_tire(self.cars[i].model_name(), season).unwrap_or(v);
        let wx = Weather {
            wet: cond.wet,
            snow: cond.snow,
            ice: cond.ice,
            glaze: if cond.covered { 0. } else { w.glaze },
            rain: self.sky.p.rain,
            temp: self.temp,
            covered: cond.covered,
        };
        let db = crate::vehdata::shared();
        let mut env = crate::vphys::Env::default();
        for (k, (a, b)) in wheels.into_iter().enumerate() {
            let (px, py) = (x + ca * a + lx * b, y + sa * a + ly * b);
            let (g, code, main) = self.city.pavement_at(px, py, Some(lvl));
            let material = match g {
                G::Cobble => Material::Cobble,
                G::Road if code == sf::PLATES => Material::Plates,
                G::Road if code == sf::UNPAVED => Material::Unpaved,
                G::Road => Material::Asphalt,
                G::Sidewalk => Material::Plates,
                G::Plaza | G::Building => Material::Concrete,
                G::Grass => Material::Grass,
                G::Water => Material::Water,
            };
            let road = g.is_road();
            let rail = road
                && self
                    .transit
                    .as_mut()
                    .is_some_and(|t| t.tram_track_near(px, py, 9.));
            let puddle = road
                && w.wet > crate::traction::PUDDLE_WET
                && self.puddle_at(px, py, lvl).is_some();
            // Zweirad: flach gequerte Straßenbahnschiene (unter 25°) zieht das Vorderrad in die Rille – je flacher
            // und nasser, desto wahrscheinlicher
            let groove = if v.two_wheel && k == 0 && rail {
                let heading = sa.atan2(ca);
                let track = self
                    .transit
                    .as_mut()
                    .and_then(|t| t.tram_track_angle(px, py, 9.));
                track.map_or(0., |ta| groove_risk(heading, ta, cond.wet))
            } else {
                0.
            };
            let mix = resolve(
                &Spot {
                    material,
                    main,
                    rail,
                    puddle,
                },
                &wx,
            );
            let mut gr = ground(db, &v.tire, &mix);
            gr.groove = groove;
            env.surface[k] = mix.main();
            // Bordstein: Wechsel zwischen Fahrbahn und Gehweg unter dem Rad
            let on = matches!(g, G::Road | G::Cobble);
            let off = matches!(g, G::Sidewalk);
            let was = self.cars[i].wheel_road[k];
            if let Some(prev) = was
                && ((prev && off) || (!prev && on))
            {
                gr.curb = CURB_M;
            }
            if on || off {
                self.cars[i].wheel_road[k] = Some(on);
            }
            env.wheel[k] = gr;
        }
        env
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
        if c.aqua > 0.
            || c.phys
                .as_ref()
                .is_some_and(|s| s.aqua[0].max(s.aqua[1]) > 0.3)
        {
            return Some("Aquaplaning!");
        }
        let cond = road_condition(&mut self.city, &self.weather, c.x, c.y, c.lvl());
        if self.weather.glaze > 0.05 && !cond.covered {
            return Some("Glatteis!");
        }
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
        self.temp = self.force_temp.unwrap_or_else(|| {
            wx::temperature_at(self.seed, self.day_count, self.clock, self.force_weather)
        });
        g.ice = wx::step_ice(g.ice, g.wet, self.temp, dt);
        // Eisregen: klare Ankündigung, sobald er Glatteis legt
        let glaze_was = g.glaze;
        g.glaze = wx::step_glaze(glaze_was, self.sky.p.rain, self.temp, dt);
        if glaze_was == 0. && g.glaze > 0. {
            self.notice = Some(Notice {
                text: "Eisregen – Glatteis!".into(),
                t: 4.,
            });
        }
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
        self.clock += dt * self.clock_rate;
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
        let driver = self
            .player
            .ride
            .as_ref()
            .is_some_and(|r| r.kind == crate::ride::RideKind::Driver);
        let dead = self.player.combat.dead;
        if input.enter_exit
            && !dead
            && self.player.in_car.is_none()
            && self.player.inside.is_none()
            && self.player.ride.is_none()
        {
            self.update_station_presence(input); // F am Eingang: hinunter zum Bahnsteig
        }
        if input.ride && !dead && self.player.in_car.is_none() && !driver {
            if self.player.ride.is_some() {
                self.alight_transit();
            } else if self.player.inside.is_some() {
                self.board_at_platform();
            } else {
                self.board_transit();
            }
        } else if input.enter_exit && !dead && self.player.inside.is_none() {
            if driver {
                self.leave_train();
            } else if self.player.ride.is_none() {
                if self.player.in_car.is_some() {
                    self.try_exit();
                } else {
                    // am Führerstand einer Bahn: übernehmen, sonst wie immer ein Auto
                    let (x, y) = (self.player.x, self.player.y);
                    let cab = self
                        .transit_near(x, y, crate::ride::CAB + 10.)
                        .into_iter()
                        .find(|h| {
                            h.car == 0
                                && h.front <= crate::ride::CAB
                                && h.mode != crate::transit::Mode::Bus
                        });
                    if !cab.is_some_and(|c| self.take_train(&c)) {
                        self.try_enter();
                    }
                }
            }
        }
        self.refresh_stations();
        // Wenden verbraucht den Tastendruck (sonst öffnete er gleich die Türen am neuen ersten Halt)
        let turned = input.action && driver && self.at_terminus() && self.turn_around();
        let train_input = Input {
            action: input.action && !turned,
            ..*input
        };
        let pc = self.player.in_car.and_then(|id| self.car_index(id));
        if let Some(i) = pc {
            // ESP: Sport → aus → voll → Sport
            if input.esp_toggle && !self.cars[i].wrecked {
                (self.esp, self.esp_full) = match (self.esp, self.esp_full) {
                    (true, false) => (false, false),
                    (false, _) => (true, true),
                    (true, true) => (true, false),
                };
                self.notice = Some(Notice {
                    text: format!("ESP {}", esp_label(self.esp, self.esp_full)),
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
            let (esp, esp_full, abs, lim) = (self.esp, self.esp_full, self.abs, self.truck_limiter);
            let c = &mut self.cars[i];
            if c.wrecked {
                c.controls = Default::default();
                c.horn = false;
            } else {
                c.controls.throttle = input.throttle;
                c.controls.brake = input.brake;
                c.controls.steer = input.steer;
                c.controls.handbrake = input.handbrake;
                c.controls.sprint = input.sprint;
                c.horn = input.horn;
            }
            c.esp = esp;
            c.esp_full = esp_full;
            c.no_limiter = !lim;
            c.abs = abs;
            if c.horn && !c.horn_was {
                self.events.push(Event::Horn {
                    x: c.x,
                    y: c.y,
                    npc: false,
                });
            }
            c.horn_was = c.horn;
        } else if !self.player.combat.dead && self.player.ride.is_none() {
            let input = &self.click_control(input, dt);
            self.update_player_on_foot(input, dt);
            let mut no_enter = *input;
            no_enter.enter_exit = false; // hinunter nur einmal je Tastendruck (oben schon geprüft)
            self.update_station_presence(&no_enter);
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
                rails: &self.rail_obs,
            };
            update_service(&mut self.cars[i], ctx.lanes, ctx.city, ctx.rng, dt);
            drive_ai(&mut self.cars[i], &mut ctx, dt);
        }
        // Tempo des Spielerautos vor den Zusammenstößen (Drift-Wertung)
        let mut pre_hit: Option<(u32, f64, f64)> = None;
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
            let falls = c.phys.as_ref().map_or(0, |s| s.passenger_falls);
            let aqua_was = c.phys.as_ref().map_or(0., |s| s.aqua[0].max(s.aqua[1]));
            let curbs = c.phys.as_ref().map_or(0, |s| s.curb_hits);
            // Spielerauto mit Fahrphysik: Untergrund je Rad
            if c.driver == Some(crate::car::Driver::Player)
                && let Some(v) = crate::car::vphys_vehicle(c)
            {
                let env = self.wheel_env(i, v);
                self.cars[i].env = Some(Box::new(env));
            }
            // Physik-LOD: KI im Umkreis des Spielers fährt mit voller Fahrphysik (vierrädrig, mit Datensatz)
            let (px, py) = (self.player.x, self.player.y);
            let c = &mut self.cars[i];
            c.lod_full = c.ai.is_some()
                && !c.wrecked
                && (c.x - px).hypot(c.y - py) < self.ai_full_radius
                && crate::car::vphys_vehicle(c).is_some_and(|v| !v.two_wheel);
            let fallen_was = c.phys.as_ref().is_some_and(|s| s.fallen.is_some());
            let rolled_was = c.phys.as_ref().is_some_and(|s| s.rolled);
            let jack_was = c.phys.as_ref().is_some_and(|s| s.jackknifed);
            step_car(c, dt, Some(ground));
            // umgekippt: das Fahrzeug ist hin
            if !rolled_was && c.phys.as_ref().is_some_and(|s| s.rolled) && !c.wrecked {
                c.health = 0.;
                c.wrecked = true;
                let (x, y, id) = (c.x, c.y, c.id);
                let player = c.driver == Some(crate::car::Driver::Player);
                self.events.push(Event::Crash {
                    x,
                    y,
                    strength: 1.,
                    car: id,
                });
                self.events.push(Event::Wreck {
                    x,
                    y,
                    car: id,
                    player,
                });
                if player {
                    self.notice = Some(Notice {
                        text: "Umgekippt!".into(),
                        t: 2.,
                    });
                }
            }
            // Sattelzug eingeknickt: Hinweis einmal je Vorfall
            let c = &self.cars[i];
            if !jack_was
                && c.driver == Some(crate::car::Driver::Player)
                && c.phys.as_ref().is_some_and(|s| s.jackknifed)
            {
                self.notice = Some(Notice {
                    text: "Eingeknickt!".into(),
                    t: 2.,
                });
                let (x, y, id) = (c.x, c.y, c.id);
                self.events.push(Event::Crash {
                    x,
                    y,
                    strength: 0.6,
                    car: id,
                });
            }
            let c = &mut self.cars[i];
            if !fallen_was
                && c.driver == Some(crate::car::Driver::Player)
                && let Some(why) = c.phys.as_ref().and_then(|s| s.fallen)
            {
                self.throw_rider(i, why);
            }
            let c = &self.cars[i];
            if let Some(s) = c.phys.as_ref() {
                let aq = s.aqua[0].max(s.aqua[1]);
                if aq > 0.5 && aqua_was <= 0.5 {
                    self.events.push(Event::Aquaplane {
                        x: c.x,
                        y: c.y,
                        car: c.id,
                        player: true,
                    });
                }
                if s.curb_hits > curbs {
                    self.events.push(Event::Curb {
                        x: c.x,
                        y: c.y,
                        car: c.id,
                    });
                }
            }
            if c.phys.as_ref().is_some_and(|s| s.passenger_falls > falls) {
                self.events.push(Event::PassengersFell {
                    x: c.x,
                    y: c.y,
                    car: c.id,
                });
                self.notice = Some(Notice {
                    text: "Fahrgäste gestürzt!".into(),
                    t: 1.6,
                });
            }
            let c = &mut self.cars[i];
            if c.driver == Some(crate::car::Driver::Player) {
                pre_hit = Some((c.id, c.vx, c.vy));
            }
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
            // ein Zusammenstoß verwirft den laufenden Drift (Wertung) – auch ein streifender ohne Schaden, sobald er
            // spürbar Tempo kostet
            let id = self.cars[i].id;
            let jolt = pre_hit.filter(|p| p.0 == id).is_some_and(|(_, vx, vy)| {
                (self.cars[i].vx - vx).hypot(self.cars[i].vy - vy) > DRIFT_JOLT
            });
            if (jolt
                || self
                    .events
                    .iter()
                    .any(|e| matches!(e, Event::Crash { car, .. } if *car == id)))
                && let Some(s) = self.cars[i].phys.as_mut()
            {
                s.drift.crash();
            }
            let c = &self.cars[i];
            (self.player.x, self.player.y, self.player.angle) = (c.x, c.y, c.angle);
        }
        self.update_transit(dt);
        self.update_player_train(&train_input, dt);
        if self.player.ride.is_some() {
            self.update_ride();
        }
        // Tunnelansicht weich ein- und ausblenden
        let ug = if self.player.ride.as_ref().is_some_and(|r| r.underground) {
            1.
        } else {
            0.
        };
        self.underground += (ug - self.underground) * (dt / 0.6).min(1.);
        if (self.underground - ug).abs() < 0.01 {
            self.underground = ug;
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
        if self.player.in_car.is_none()
            && self.player.ride.is_none()
            && self.player.inside.is_none()
        {
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
        self.update_bikes(dt);
        crate::services::manage_emergency(self, dt);
        self.manage_population();
        self.manage_parked();
        self.manage_life(false);
        self.manage_animals(false);
        self.update_animals(dt);
        self.manage_scooters();
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
                                bike: false,
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
        let (mut tx, mut ty, mut zoom) = (self.player.x, self.player.y, self.foot_zoom);
        if self.player.inside.is_some() {
            zoom = 1.1; // im U-Bahnhof: mehr vom Bahnsteig im Bild
        } else if let Some(r) = &self.player.ride {
            let ahead = if r.kind == crate::ride::RideKind::Driver {
                r.speed * 0.6
            } else {
                0.
            };
            tx = self.player.x + self.player.angle.cos() * ahead;
            ty = self.player.y + self.player.angle.sin() * ahead;
            zoom = 1. - (r.speed / 330.).clamp(0., 1.) * 0.28;
        } else if let Some(c) = self.player_car() {
            tx = c.x + c.vx * 0.45;
            ty = c.y + c.vy * 0.45;
            let k = c.kind_info();
            zoom = if k.bike {
                self.foot_zoom * 0.8
            } else if k.moto {
                1.45 - (c.speed() / 500.).clamp(0., 1.) * 0.45
            } else {
                camera_zoom_for(c.speed(), crate::vehdata::game_feel().camera_zoom_by_speed)
            };
        }
        // im Drift zieht die Kamera leicht nach
        let follow = if self
            .player_car()
            .and_then(|c| c.phys.as_ref())
            .is_some_and(|s| s.drift.active())
        {
            3.2
        } else {
            5.
        };
        self.camera.x = damp(self.camera.x, tx, follow, dt);
        self.camera.y = damp(self.camera.y, ty, follow, dt);
        self.camera.zoom = damp(self.camera.zoom, zoom, 2., dt);
    }

    // --- Spielstand -------------------------------------------------------------------------

    pub fn make_save(&self, now_ms: f64) -> SaveData {
        let car = self
            .player_car_id
            .and_then(|id| self.car(id))
            .filter(|c| !c.wrecked);
        let pos = match &self.player.ride {
            // im Wagen: an der letzten Haltestelle (world.js rideExit; bei S-/U-Bahn deren Straßenpunkt)
            Some(r) => (r.last_stop.x, r.last_stop.y),
            // im U-Bahnhof: am näheren Ausgang oben
            None if self.player.inside.is_some() => self
                .player
                .inside
                .as_ref()
                .and_then(|i| self.stations.by_id.get(&i.id))
                .map(|st| {
                    let (u, _) = st.to_local(self.player.x, self.player.y);
                    let ex = &st.exits[if u < 0. { 0 } else { 1 }];
                    (ex.x, ex.y)
                })
                .unwrap_or((self.player.x, self.player.y)),
            None => self
                .player_car()
                .map(|c| (c.x, c.y))
                .unwrap_or((self.player.x, self.player.y)),
        };
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

/// Arbeitshalte von Paketwagen und Müllauto (services.js updateService): nach einer Fahrstrecke an einer passenden
/// Stelle anhalten, Warnblinker bzw. Müllwerker an.
fn update_service(
    c: &mut Car,
    lanes: &mut crate::roadgraph::LaneGraph,
    city: &crate::city::City,
    rng: &mut crate::math::Rng,
    dt: f64,
) {
    let kind = c.kind;
    if kind != "delivery" && kind != "garbage" {
        return;
    }
    let v = c.speed();
    let id = c.id;
    let Some(ai) = c.ai.as_mut() else { return };
    if ai.hold <= 0. {
        c.hazard = false;
        c.work = false;
    }
    ai.odo += v * dt;
    // wer neu in der Szene auftaucht, ist mitten auf seiner Tour: der Rest bis zum ersten Halt ist ein Bruchteil
    // des üblichen Abstands (Restlebensdauer eines Erneuerungsprozesses), sonst hielte kaum ein Lieferwagen, bevor
    // er außer Sicht ist
    let next = *ai.next_stop.get_or_insert_with(|| {
        crate::fleet::next_stop_after(kind, rng.float()) * crate::math::hash01(id as f64 * 1.37)
    });
    if ai.hold > 0. || ai.odo < next || v > 160. {
        return;
    }
    let Some(lane) = ai
        .segs
        .get(ai.current_seg())
        .and_then(|s| lanes.lane(s.lane))
    else {
        return;
    };
    let p = &lane.pts;
    let (Some(&(ex, ey)), Some(&(sx, sy))) = (p.last(), p.first()) else {
        return;
    };
    let cls = city.edges.get(&lane.edge).map_or(5, |e| e.cls);
    let (to_end, from_start) = ((ex - c.x).hypot(ey - c.y), (sx - c.x).hypot(sy - c.y));
    if !crate::fleet::may_stop_on(kind, cls, to_end, from_start) {
        return;
    }
    ai.hold = crate::fleet::stop_duration(kind, rng.float());
    ai.odo = 0.;
    ai.next_stop = Some(crate::fleet::next_stop_after(kind, rng.float()));
    if kind == "delivery" {
        c.hazard = true;
    } else {
        c.work = true;
    }
}

/// Ziel eines Teleports (Stadtplan, Konsole).
#[derive(Debug, Clone, PartialEq)]
pub enum TeleportSpot {
    /// Stadtteil dort lädt noch (später erneut fragen)
    Pending,
    Spot {
        x: f64,
        y: f64,
        angle: f64,
        name: String,
    },
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

/// Anzeige des ESP-Modus.
pub fn esp_label(esp: bool, full: bool) -> &'static str {
    match (esp, full) {
        (false, _) => "AUS",
        (true, true) => "VOLL",
        (true, false) => "SPORT",
    }
}

/// Sturzwahrscheinlichkeit (0…1), wenn ein Zweirad mit Kurs `heading` ein Gleis mit Richtung `track` quert: unter
/// 25° steigt sie mit flacherem Winkel und mit der Nässe; flach und nass ist der Sturz sicher.
pub fn groove_risk(heading: f64, track: f64, wet: f64) -> f64 {
    let d = (heading - track).rem_euclid(std::f64::consts::PI);
    let cross = d.min(std::f64::consts::PI - d);
    let lim = 25f64.to_radians();
    if cross < lim {
        ((1. - cross / lim) * (0.35 + 0.65 * wet.clamp(0., 1.)) * 2.).min(1.)
    } else {
        0.
    }
}

/// Physik-LOD: KI-Fahrzeuge in diesem Umkreis (px, 150 m) fahren mit voller Fahrphysik, weiter weg kinematisch
/// mit denselben Grenzen aus den Daten.
pub const AI_FULL_RADIUS: f64 = 1500.;

/// Kamera-Zoom im Auto nach Tempo (px/s): bis 330 px/s (~120 km/h) von 1 auf 0,72, darüber (wenn
/// `kamera_zoom_nach_tempo`) weiter bis 0,45 bei 1000 px/s (~360 km/h), damit man bei Hypercar-Tempo noch sieht,
/// wohin es geht.
pub fn camera_zoom_for(speed: f64, by_speed: bool) -> f64 {
    let base = 1. - (speed / 330.).clamp(0., 1.) * 0.28;
    if !by_speed || speed <= 330. {
        return base;
    }
    0.72 - ((speed - 330.) / 670.).clamp(0., 1.) * 0.27
}

/// Geschwindigkeitsänderung durch einen Zusammenstoß (px/s), ab der ein laufender Drift verworfen wird
pub const DRIFT_JOLT: f64 = 50.;

/// Fahrzeugart im Spiel für einen Datensatz (Größe, Stimme, Spurwahl der Art).
pub fn data_kind(v: &crate::vehdata::Vehicle) -> &'static str {
    match v.class.as_str() {
        _ if v.two_wheel && v.id.starts_with("fahrrad") || v.id == "rennrad" => "bicycle",
        _ if v.two_wheel && v.id.starts_with("escooter") => "escooter",
        _ if v.two_wheel && v.id.starts_with("roller") => "scooter",
        _ if v.two_wheel => "motorcycle",
        "lkw" | "lkw_sattel" => "truck",
        "bus" | "bus_gelenk" => "bus",
        _ => "car",
    }
}
