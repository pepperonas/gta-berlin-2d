//! Fahrzeuge (Port von `car.js`): Arcade-Fahrphysik für Verkehr, Parker und Räder, echte Fahrdynamik
//! (`dynamics.rs`) für das vom Spieler gefahrene Auto; Kollision gegen die Stadt und gegeneinander, Schaden.
use crate::carmodels::{Kind, car_model, kind, spec_of};
use crate::city::{CircleKind, City, Ground, Solid, WallKind};
use crate::collision::{Obb, circle_vs_obb, obb_vs_obb, obb_vs_rect, obb_vs_segment};
use crate::dynamics::{self, Assists, Body, Controls, DynState, Surface};
use crate::events::Event;
use crate::levels::LevelState;
use crate::math::sign;
use crate::traction::{Aqua, DRY, Traction};
use std::collections::HashMap;

pub const LENGTH: f64 = 42.;
pub const WIDTH: f64 = 20.;
pub const MAX_SPEED: f64 = 330.;
pub const MAX_REVERSE: f64 = 110.;
pub const ACCEL: f64 = 240.;
pub const BRAKE: f64 = 560.;
pub const HANDBRAKE: f64 = 260.;
pub const DRAG: f64 = 0.32;
pub const GRIP: f64 = 9.;
pub const HANDBRAKE_GRIP: f64 = 1.4;
pub const STEER_RATE: f64 = 2.8;
pub const HEALTH: f64 = 100.;
pub const DAMAGE_THRESHOLD: f64 = 70.;
pub const DAMAGE_FACTOR: f64 = 0.07;
pub const RESTITUTION: f64 = 0.3;
/// Wandkontakt gleiten (`feel.wandkontakt_gleiten`): Verlust entlang der Wand je Steilheit des Aufpralls (0 =
/// streifend, 1 = frontal) und Rückprall senkrecht zur Wand
pub const WALL_SLIDE_LOSS: f64 = 0.8;
pub const WALL_BOUNCE: f64 = 0.15;
/// Poller geben schon bei langsamem Anstoßen nach (px/s ≈ 3 km/h).
pub const KNOCK_SPEED: f64 = 8.;
pub const KNOCK_SLOW: f64 = 0.96;
pub const KNOCK_DAMAGE: f64 = 0.75;
pub const SPEED_TO_KMH: f64 = 0.36;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driver {
    Player,
    Npc,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Traffic,
    Player,
    Parked,
    Curb,
}

#[derive(Debug, Clone)]
pub struct Car {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub vx: f64,
    pub vy: f64,
    pub ang_vel: f64,
    pub kind: &'static str,
    pub model: Option<&'static str>,
    pub hw: f64,
    pub hh: f64,
    pub health: f64,
    pub wrecked: bool,
    pub wreck_t: f64,
    /// brennendes Wrack: Sekunden bis zur Explosion; danach `exploded` (ausgebrannt)
    pub burn: Option<f64>,
    pub exploded: bool,
    pub driver: Option<Driver>,
    pub role: Role,
    pub color: u32,
    pub controls: Controls,
    pub cargo: bool,
    pub horn: bool,
    pub horn_was: bool,
    /// beschossen (von wo): world.rs lässt den KI-Fahrer aussteigen und fliehen
    pub shot_at: Option<(f64, f64)>,
    /// Einsatzfahrzeug: Martinshorn, Blaulicht (am Einsatzort ohne Horn), Einsatz und ob er erledigt ist
    pub siren: bool,
    pub blue: bool,
    pub duty: Option<crate::services::Duty>,
    pub done: bool,
    /// Linienbus (transitlive.rs) und seine Linie
    pub bus: Option<Box<crate::transitlive::BusDuty>>,
    pub line: Option<String>,
    /// Warnblinker (Paketwagen hält in zweiter Reihe) bzw. Müllabfuhr bei der Arbeit
    pub hazard: bool,
    pub work: bool,
    pub skid: f64,
    pub spin: f64,
    pub level: LevelState,
    pub level_init: bool,
    pub dyn_state: Option<DynState>,
    /// Fahrphysik-Zustand (vphys, Spielerauto mit Fahrzeugdaten)
    pub phys: Option<Box<crate::vphys::State>>,
    /// Untergrund je Rad für den nächsten Schritt (von der Welt gesetzt); welches Rad auf der Fahrbahn stand
    /// (Bordsteinwechsel); Winterreifen statt der Sommerreifen
    pub env: Option<Box<crate::vphys::Env>>,
    pub wheel_road: [Option<bool>; 4],
    pub season_tire: Option<&'static str>,
    pub esp: bool,
    pub esp_full: bool,
    /// Entwickler-Anzeige: live verstellter Datensatz für dieses Fahrzeug (statt der Spieldaten)
    pub tuned: Option<Box<crate::vehdata::Vehicle>>,
    /// KI nahe am Spieler: volle Fahrphysik statt des kinematischen Modells (Physik-LOD, `World::AI_FULL_RADIUS`)
    pub lod_full: bool,
    /// zuletzt gefahrener Untergrund je Rad (Anzeige)
    pub env_seen: Option<Box<crate::vphys::Env>>,
    /// Lkw-Begrenzer abgeschaltet (Konsole `begrenzer aus`)
    pub no_limiter: bool,
    pub abs: bool,
    pub traction: Traction,
    pub aqua: f64,
    pub aqua_yaw: f64,
    pub park_key: Option<(i64, i8, usize)>,
    pub ai: Option<Box<crate::traffic::Ai>>,
}

impl Car {
    pub fn new(
        id: u32,
        x: f64,
        y: f64,
        angle: f64,
        color: u32,
        role: Role,
        kind_name: &str,
    ) -> Self {
        let k = kind(kind_name);
        // Pkw: Maße des Modells (das Modell folgt aus Nummer und Rolle), sonst die der Fahrzeugart
        let (hw, hh, color) = if k.name == "car" {
            let m = crate::carmodels::car_model(id, k.name, role == Role::Player, None);
            let (hw, hh) = crate::carmodels::body_dims(m)
                .map_or((k.l / 2., k.w / 2.), |(l, w)| (l * 5., w * 5.));
            // Lack aus Modell und Nummer (realistische Verteilung); das Spielerauto behält seine Farbe
            let color = if role == Role::Player {
                color
            } else {
                let r = ((id as f64 * 0.618_033_988_75 + 0.137).fract() * 977.).fract();
                crate::carmodels::paint_for(m, r)
            };
            (hw, hh, color)
        } else {
            (k.l / 2., k.w / 2., color)
        };
        Self {
            id,
            x,
            y,
            angle,
            vx: 0.,
            vy: 0.,
            ang_vel: 0.,
            kind: k.name,
            model: None,
            hw,
            hh,
            health: HEALTH,
            wrecked: false,
            wreck_t: 0.,
            burn: None,
            exploded: false,
            driver: None,
            role,
            color,
            controls: Controls::default(),
            cargo: false,
            horn: false,
            horn_was: false,
            shot_at: None,
            siren: false,
            blue: false,
            duty: None,
            done: false,
            bus: None,
            line: None,
            hazard: false,
            work: false,
            skid: 0.,
            spin: 0.,
            level: LevelState::default(),
            level_init: false,
            dyn_state: None,
            phys: None,
            env: None,
            wheel_road: [None; 4],
            season_tire: None,
            esp: true,
            esp_full: false,
            tuned: None,
            lod_full: false,
            env_seen: None,
            no_limiter: false,
            abs: true,
            traction: DRY,
            aqua: 0.,
            aqua_yaw: 0.,
            park_key: None,
            ai: None,
        }
    }
    pub fn kind_info(&self) -> &'static Kind {
        kind(self.kind)
    }
    pub fn obb(&self) -> Obb {
        Obb {
            x: self.x,
            y: self.y,
            angle: self.angle,
            hw: self.hw,
            hh: self.hh,
        }
    }
    pub fn forward_speed(&self) -> f64 {
        self.vx * self.angle.cos() + self.vy * self.angle.sin()
    }
    pub fn speed(&self) -> f64 {
        self.vx.hypot(self.vy)
    }
    pub fn lvl(&self) -> i8 {
        self.level.lvl
    }
    /// Zuladung 0…1: Lkw und Busse fahren zufällig beladen (fest je Fahrzeug), alles andere leer.
    pub fn cargo_load(&self) -> f64 {
        match data_vehicle(self) {
            Some(v)
                if matches!(
                    v.class.as_str(),
                    "lkw" | "lkw_sattel" | "bus" | "bus_gelenk"
                ) =>
            {
                crate::math::hash01(self.id as f64 * 3.71 + 0.5)
            }
            _ => 0.,
        }
    }
    /// Masse (kg) aus den Fahrzeugdaten samt Zuladung; ohne Datensatz aus der Größe geschätzt.
    pub fn mass(&self) -> f64 {
        match data_vehicle(self) {
            Some(v) => v.loaded(self.cargo_load()).0,
            None => (self.hw * self.hh * 4. * 0.9).max(80.),
        }
    }
    /// Modell (Fahrverhalten, Name): fest, nach Art oder aus der Nummer.
    /// Festes Modell setzen; ein Pkw übernimmt dessen Maße (Bild und Kollision).
    pub fn set_model(&mut self, m: &'static str) {
        self.model = Some(m);
        if self.kind == "car"
            && let Some((l, w)) = crate::carmodels::body_dims(m)
        {
            (self.hw, self.hh) = (l * 5., w * 5.);
        }
    }
    pub fn model_name(&self) -> &'static str {
        car_model(self.id, self.kind, self.role == Role::Player, self.model)
    }
    /// Hat das Fahrzeug eine Sirene (Polizei, Rettungswagen, Datensatz mit Merkmal `sirene`)?
    pub fn has_siren(&self) -> bool {
        matches!(self.kind, "police" | "ambulance")
            || crate::vehdata::game_vehicle(self.model_name()).is_some_and(|v| {
                v.flags
                    .get("sirene")
                    .is_some_and(|f| f.as_bool() == Some(true))
            })
    }
}

/// Untergrund → Fahrwerte (car.js `SURFACE`).
pub fn surface_of(g: Ground) -> Surface {
    match g {
        Ground::Cobble => Surface {
            drag: 1.25,
            grip: 0.85,
            top: 0.92,
        },
        Ground::Sidewalk => Surface {
            drag: 1.3,
            grip: 0.95,
            top: 0.9,
        },
        Ground::Grass => Surface {
            drag: 3.2,
            grip: 0.55,
            top: 0.5,
        },
        Ground::Water => Surface {
            drag: 4.,
            grip: 0.4,
            top: 0.3,
        },
        Ground::Road | Ground::Plaza | Ground::Building => Surface {
            drag: 1.,
            grip: 1.,
            top: 1.,
        },
    }
}

/// Ein Schritt Fahrphysik. `ground` = Untergrund unter dem Auto (None = Straße).
pub fn step_car(car: &mut Car, dt: f64, ground: Option<Ground>) {
    let ctl = if car.wrecked {
        Controls {
            handbrake: true,
            ..Default::default()
        }
    } else {
        car.controls
    };
    let surf = surface_of(ground.unwrap_or(Ground::Road));
    let tr = car.traction;
    let aq = car.aqua > 0.;
    let k_brake = tr.brake * if aq { Aqua::BRAKE } else { 1. };
    let k_lat = tr.lat * if aq { Aqua::LAT } else { 1. };
    let k_steer = tr.steer * if aq { Aqua::STEER } else { 1. };
    car.spin = 0.;
    let info = car.kind_info();
    // Der Spieler fährt mit echter Fahrdynamik; Verkehr, geparkte und geschobene Autos sowie Räder arcadig.
    // Vierrädrige Fahrzeuge mit Datensatz fahren über den Kern `vphys`, Zweiräder bis Phase 5 über `dynamics`.
    if (car.driver == Some(Driver::Player) || car.lod_full) && !car.wrecked {
        let tuned = car.tuned.take();
        if let Some(v) = tuned.as_deref().or_else(|| vphys_vehicle(car)) {
            step_vphys(car, v, ctl, ground.unwrap_or(Ground::Road), dt);
            car.tuned = tuned;
            return;
        }
    }
    if car.driver == Some(Driver::Player) && info.top.is_none() && !car.wrecked {
        let spec = spec_of(car.model_name());
        let mut body = Body {
            x: car.x,
            y: car.y,
            angle: car.angle,
            vx: car.vx,
            vy: car.vy,
            ang_vel: car.ang_vel,
        };
        let d = car.dyn_state.get_or_insert_with(DynState::default);
        let out = dynamics::step_dynamics(
            &mut body,
            d,
            spec,
            dt,
            surf,
            tr,
            ctl,
            Assists {
                esp: car.esp,
                abs: car.abs,
                aqua: aq,
                aqua_yaw: car.aqua_yaw,
            },
        );
        (car.x, car.y, car.angle, car.vx, car.vy, car.ang_vel) =
            (body.x, body.y, body.angle, body.vx, body.vy, body.ang_vel);
        car.spin = out.spin;
        car.skid = out.skid;
        if car.aqua > 0. {
            car.aqua = (car.aqua - dt).max(0.);
        }
        return;
    }
    let (mut s, mut c) = car.angle.sin_cos();
    let mut vf = car.vx * c + car.vy * s;
    let mut vr = -car.vx * s + car.vy * c;
    let pw = info.power;
    // KI und geschobene Autos: Grenzen aus den Fahrzeugdaten (ein KI-Bus fährt wie ein Bus), ohne Datensatz die
    // alten Spielwerte
    let lim = data_vehicle(car).map(Limits::of);
    let top = lim.map_or(info.top.unwrap_or(MAX_SPEED * (0.55 + 0.45 * pw)), |l| {
        l.top
    }) * surf.top;
    if ctl.throttle > 0. && vf < top {
        let a = match lim {
            Some(l) => l.accel_boosted(vf.max(0.), crate::vehdata::game_feel().launch(vf / 10.)),
            None => {
                let t = if vf > 0. { 1. - (vf / top) * 0.55 } else { 1.4 };
                ACCEL * pw * info.accel * t
            }
        };
        vf += a * ctl.throttle * tr.accel * dt;
        car.spin = if ctl.throttle > 0.8 && tr.accel < 0.7 && vf < 150. {
            1.
        } else {
            0.
        };
    }
    if ctl.brake > 0. {
        if vf > 5. {
            let b = lim.map_or(BRAKE, |l| l.brake);
            vf = (vf - b * ctl.brake * k_brake * dt).max(0.);
        } else if vf > -(if info.top.is_some() { 22. } else { MAX_REVERSE }) {
            vf -= ACCEL * 0.6 * ctl.brake * dt;
        }
    }
    if ctl.handbrake {
        vf -= sign(vf) * vf.abs().min(HANDBRAKE * k_brake * dt);
    }
    vf -= vf * DRAG * surf.drag * dt;
    if ctl.throttle == 0. && ctl.brake == 0. && vf.abs() < 4. {
        vf = 0.;
    }
    let grip = (if ctl.handbrake {
        HANDBRAKE_GRIP
    } else {
        GRIP * surf.grip
    }) * k_lat;
    car.skid = if vr.abs() > 70. {
        (vr.abs() / 200.).min(1.)
    } else {
        0.
    };
    vr *= (-grip * dt).exp();
    let av = vf.abs();
    let speed_factor = (av / 80.).clamp(0., 1.) * (1. - 0.45 * (av / MAX_SPEED).clamp(0., 1.));
    let target = ctl.steer
        * STEER_RATE
        * speed_factor
        * sign(vf)
        * if ctl.handbrake { 1.35 } else { 1. }
        * k_steer
        + if aq { car.aqua_yaw } else { 0. };
    car.ang_vel += (target - car.ang_vel) * (12. * dt).min(1.);
    // Querbeschleunigung höchstens μ·g, gemindert um das Wetter
    if let Some(l) = lim
        && av > 20.
    {
        let cap = l.lat * k_lat.min(1.) / av;
        car.ang_vel = car.ang_vel.clamp(-cap, cap);
    }
    car.angle += car.ang_vel * dt;
    (s, c) = car.angle.sin_cos();
    car.vx = vf * c - vr * s;
    car.vy = vf * s + vr * c;
    car.x += car.vx * dt;
    car.y += car.vy * dt;
    if car.aqua > 0. {
        car.aqua = (car.aqua - dt).max(0.);
    }
}

/// Anhänger eines Gespanns (Sattelauflieger, Nachläufer) in Spielkoordinaten: Mittelpunkt, Winkel, halbe Länge
/// und halbe Breite (px). Der Knickwinkel kommt aus der Fahrphysik, ohne sie hängt der Anhänger gerade.
pub fn trailer_pose(car: &Car) -> Option<(f64, f64, f64, f64, f64)> {
    let v = data_vehicle(car)?;
    let hc = v.hitch.as_ref()?;
    // Gelenk: e hinter der Hinterachse (Königszapfen leicht davor); die Achsen liegen grob mittig im Wagen
    let hitch = (-v.wheelbase / 2. - hc.e) * 10.;
    let (fy, fx) = car.angle.sin_cos();
    let (hx, hy) = (car.x + fx * hitch, car.y + fy * hitch);
    // vphys: Knickwinkel gegen den Uhrzeigersinn, Spielwinkel im Uhrzeigersinn
    let art = car.phys.as_ref().map_or(0., |s| s.art);
    let a2 = car.angle + art;
    let half = hc.trailer_len * 5.;
    // Mittelpunkt des Anhängers: vom Gelenk um (halbe Länge − Überhang vor dem Gelenk) nach hinten
    let back = half - hc.overhang * 10.;
    Some((
        hx - a2.cos() * back,
        hy - a2.sin() * back,
        a2,
        half,
        v.width * 5.,
    ))
}

/// Grenzen des kinematischen KI-Modells aus den Fahrzeugdaten (Spielsystem: px, px/s, px/s²).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    /// Höchsttempo
    pub top: f64,
    /// Antriebskraft je Masse bei Stillstand (Traktion) und Leistung je Masse (W/kg, für P/(m·v))
    pub traction: f64,
    pub power: f64,
    /// Verzögerung bei Vollbremsung, größte Querbeschleunigung (trocken)
    pub brake: f64,
    pub lat: f64,
}
impl Limits {
    pub fn of(v: &crate::vehdata::Vehicle) -> Self {
        use crate::vehdata::{Drive, G};
        let kmh = v
            .targets
            .get("vmax")
            .copied()
            .or(v.limiter.map(|l| l * 3.6))
            .unwrap_or(200.);
        let (m, _) = v.loaded(0.);
        let driven = match v.drive {
            Drive::Fwd => v.front,
            Drive::Rwd => 1. - v.front,
            Drive::Awd => 1.,
        };
        Self {
            top: kmh / 3.6 * 10.,
            traction: v.tire.mu * v.tire.mu_long * driven * G * 10.,
            power: v.engine.watts * v.efficiency / m,
            brake: v.brake.gain.min(v.tire.mu * v.tire.mu_long) * G * 10.,
            lat: v.tire.mu * G * 10.,
        }
    }
    /// Beschleunigung (px/s²) bei Tempo `vf` (px/s): Traktion, darüber Leistung durch Tempo.
    pub fn accel(&self, vf: f64) -> f64 {
        let v = (vf / 10.).max(1.);
        self.traction.min(self.power / v * 10.)
    }
    /// Wie `accel`, mit Anfahr-Zuschlag `k` auf die Leistung (die Traktionsgrenze bleibt).
    pub fn accel_boosted(&self, vf: f64, k: f64) -> f64 {
        let v = (vf / 10.).max(1.);
        self.traction.min(self.power * k / v * 10.)
    }
}

/// Datensatz eines Fahrzeugs (auch für Wracks und KI; ohne Winterreifen).
pub fn data_vehicle(car: &Car) -> Option<&'static crate::vehdata::Vehicle> {
    let id = match car.kind {
        "bicycle" => "fahrrad_city",
        "escooter" => "escooter",
        _ => car.model_name(),
    };
    crate::vehdata::game_vehicle(id)
}

/// Datensatz für den Fahrphysik-Kern (vierrädrig, mit Daten; Winterreifen, wenn die Welt sie gewählt hat).
pub fn vphys_vehicle(car: &Car) -> Option<&'static crate::vehdata::Vehicle> {
    if car.wrecked {
        return None;
    }
    // Fahrrad und E-Scooter (Fahrzeugarten ohne Pkw-Modell) über ihre Datensätze
    let id = match car.kind {
        "bicycle" => "fahrrad_city",
        "escooter" => "escooter",
        _ => car.model_name(),
    };
    crate::vehdata::game_vehicle_tire(id, car.season_tire)
}

/// Spielerauto über den Fahrphysik-Kern. Das Spiel rechnet in Pixeln (10 px = 1 m) mit y nach unten und
/// Winkeln im Uhrzeigersinn; `vphys` in Metern, y nach links, Gierwinkel gegen den Uhrzeigersinn. Der Kern
/// rechnet jeden Schritt im Fahrzeugsystem ab dem aktuellen Ort; Lage und Geschwindigkeiten kommen jedes Mal aus
/// dem Auto (Stöße und Teleports wirken so ohne Umweg).
fn step_vphys(car: &mut Car, v: &crate::vehdata::Vehicle, ctl: Controls, ground: Ground, dt: f64) {
    use crate::vehdata::Esp;
    use crate::vphys::{Env, Ground as Road, Input, step};
    const PX: f64 = 10.;
    // die Drift-Schicht gehört dem Spieler; KI fährt mit reiner Fahrphysik
    let feel_ai;
    let feel = if car.driver == Some(Driver::Player) {
        crate::vehdata::game_feel()
    } else {
        feel_ai = crate::vehdata::Feel {
            drift_layer: false,
            moto_assist: false,
            ..crate::vehdata::game_feel().clone()
        };
        &feel_ai
    };
    let surf = surface_of(ground);
    let tr = car.traction;
    let aq = car.aqua > 0.;
    let (sa, ca) = car.angle.sin_cos();
    let vf = car.vx * ca + car.vy * sa;
    let vr = -car.vx * sa + car.vy * ca;
    let env_set = car.env.take();
    let load = car.cargo_load();
    let s = car.phys.get_or_insert_with(Default::default);
    s.load = load;
    (s.x, s.y, s.yaw) = (0., 0., 0.);
    s.vx = vf / PX;
    s.vy = -vr / PX;
    s.r = -car.ang_vel;
    // Untergrund (Phase 4 rechnet je Rad): Haftung aus Untergrund und Witterung, Kopfstein rüttelt
    // Untergrund je Rad aus der Welt (`World::wheel_env`); ohne ihn (Tests ohne Welt) einheitlich aus dem
    // Untergrund unter der Mitte und der Witterung
    let env = env_set.map(|e| *e).unwrap_or_else(|| {
        let base = Road {
            mu_rel: surf.grip * tr.lat,
            rough: if ground == Ground::Cobble { 0.6 } else { 0. },
            rolling_extra: (surf.drag - 1.) * 0.02,
            ..Road::DRY
        };
        let mut front = base;
        if aq {
            front.mu_rel *= Aqua::LAT;
        }
        Env {
            wheel: [front, front, base, base],
            ..Env::default()
        }
    });
    let esp = match (car.esp, car.esp_full) {
        (false, _) => Esp::Off,
        (true, true) => Esp::Full,
        (true, false) => Esp::Sport,
    };
    let inp = Input {
        throttle: ctl.throttle,
        brake: ctl.brake,
        steer: -ctl.steer,
        handbrake: ctl.handbrake,
        sprint: ctl.sprint,
        esp: Some(esp),
        no_abs: !car.abs,
        no_limiter: car.no_limiter
            && matches!(v.class.as_str(), "lkw" | "lkw_sattel")
            && feel.truck_limiter_tunable,
    };
    step(v, feel, s, &inp, &env, dt);
    car.env_seen = Some(Box::new(env));
    // zurück ins Spielsystem
    let (dx, dy) = (s.x * PX, -s.y * PX);
    car.x += dx * ca - dy * sa;
    car.y += dx * sa + dy * ca;
    car.angle -= s.yaw;
    car.ang_vel = -s.r + if aq { car.aqua_yaw * dt * 2. } else { 0. };
    let (sa, ca) = car.angle.sin_cos();
    let (vf, vr) = (s.vx * PX, -s.vy * PX);
    car.vx = vf * ca - vr * sa;
    car.vy = vf * sa + vr * ca;
    // Anzeige, Klang, Vibration (Felder wie bei `dynamics`)
    let spin = |i: usize| if s.spin[i] { 1. } else { 0. };
    let d = car.dyn_state.get_or_insert_with(DynState::default);
    d.delta = -s.delta_eff;
    d.ax = s.ax_f;
    d.ay = -s.ay_f;
    d.alpha_f = -s.alpha[0];
    d.alpha_r = -s.alpha[1];
    d.spin_f = spin(0);
    d.spin_r = spin(1);
    d.lock_r = if s.locked[1] { 1. } else { 0. };
    d.esp = if s.esp_active || s.tcs { 1. } else { 0. };
    d.understeer = s.understeer;
    // Zweirad: Schräglage (Spielsystem: positiv = nach rechts), Wheelie, Stoppie
    d.lean = -s.lean;
    d.wheelie = s.pitch.max(0.);
    d.stoppie = (-s.pitch).max(0.);
    let lat = vr.abs();
    car.spin = d.spin_f.max(d.spin_r);
    car.skid = if s.locked.iter().any(|&l| l) || s.abs {
        (vf.abs() / 200.).min(1.)
    } else if lat > 40. {
        (lat / 200.).min(1.)
    } else {
        0.
    };
    // Drift: Spuren proportional zum Winkel, Qualm ab etwa 20°
    let angle = if s.drift.active() { s.beta().abs() } else { 0. };
    if let Some(d) = car.dyn_state.as_mut() {
        d.drift_angle = angle;
    }
    if angle > 20f64.to_radians() {
        car.skid = car
            .skid
            .max((0.25 + (angle.to_degrees() - 20.) / 40. * 0.5).min(1.));
    }
    if car.aqua > 0. {
        car.aqua = (car.aqua - dt).max(0.);
    }
}

pub fn damage(car: &mut Car, impact: f64, events: &mut Vec<Event>) {
    if impact > DAMAGE_THRESHOLD {
        car.health = (car.health - (impact - DAMAGE_THRESHOLD) * DAMAGE_FACTOR).max(0.);
        events.push(Event::Crash {
            x: car.x,
            y: car.y,
            strength: (impact / 300.).clamp(0., 1.),
            car: car.id,
        });
        if car.health <= 0. && !car.wrecked {
            car.wrecked = true;
            events.push(Event::Wreck {
                x: car.x,
                y: car.y,
                car: car.id,
                player: false,
            });
        }
    }
}

fn apply_impact(car: &mut Car, nx: f64, ny: f64, events: &mut Vec<Event>) -> f64 {
    let vn = car.vx * nx + car.vy * ny;
    if vn >= 0. {
        return 0.;
    }
    if crate::vehdata::game_feel().wall_slide {
        // Wandkontakt gleiten: Tempo entlang der Wand bleibt, gemindert nach dem Aufprallwinkel (streifend kaum,
        // frontal fast ganz); senkrecht prallt das Auto schwach ab
        let speed = car.vx.hypot(car.vy).max(1e-6);
        let steep = -vn / speed;
        let (tx, ty) = (car.vx - vn * nx, car.vy - vn * ny);
        let keep = 1. - WALL_SLIDE_LOSS * steep;
        car.vx = tx * keep - WALL_BOUNCE * vn * nx;
        car.vy = ty * keep - WALL_BOUNCE * vn * ny;
    } else {
        car.vx -= (1. + RESTITUTION) * vn * nx;
        car.vy -= (1. + RESTITUTION) * vn * ny;
        car.vx *= 0.85;
        car.vy *= 0.85;
    }
    car.ang_vel *= 0.5;
    damage(car, -vn, events);
    -vn
}

/// Umgefahrene Poller (Schlüssel → Fallrichtung), je Welt gemerkt.
pub type Knocked = HashMap<(i64, i64), f64>;

/// Sperrt ein festes Hindernis ein Objekt der Ebene `lvl`? Stadtgrenze immer; Häuser, Bäume, Kisten und Poller am
/// Boden sperren Boden und Unterführung, nicht die Brücke; Wände (Ufer, Gleis, Geländer, Zaun) nur ihre Ebene.
pub fn blocks(knocked: &Knocked, s: &Solid, lvl: i8) -> bool {
    match s {
        Solid::Circle {
            kind: CircleKind::Barrier { key },
            ..
        } if knocked.contains_key(key) => false,
        Solid::Wall {
            kind: WallKind::Border,
            ..
        } => true,
        Solid::Wall {
            kind: WallKind::Wall,
            lvl: l,
            ..
        } => *l == lvl,
        _ => lvl <= 0,
    }
}

/// Auto gegen die statische Welt (Gebäude, Wände, Kisten, Bäume, Poller, Stadtgrenze).
pub fn collide_car_world(
    car: &mut Car,
    city: &mut City,
    knocked: &mut Knocked,
    events: &mut Vec<Event>,
) {
    for _ in 0..3 {
        let mut hit = false;
        for h in city.solids.query(&car.obb().bounds()) {
            let s = *city.solids.get(h);
            if !blocks(knocked, &s, car.lvl()) {
                continue;
            }
            let o = car.obb();
            let m = match s {
                Solid::Wall { seg, .. } => obb_vs_segment(&o, &seg),
                Solid::Circle { x, y, r, .. } => circle_vs_obb(x, y, r, &o).map(|c| c.inverted()),
                Solid::Rect(r) => obb_vs_rect(&o, &r),
            };
            let Some(m) = m else { continue };
            if let Solid::Circle {
                x,
                y,
                kind: CircleKind::Barrier { key },
                ..
            } = s
                && -(car.vx * m.nx + car.vy * m.ny) > KNOCK_SPEED
            {
                knocked.insert(key, car.vy.atan2(car.vx));
                car.vx *= KNOCK_SLOW;
                car.vy *= KNOCK_SLOW;
                car.health = (car.health - KNOCK_DAMAGE).max(0.);
                events.push(Event::Knock { x, y, car: car.id });
                continue;
            }
            car.x += m.nx * m.depth;
            car.y += m.ny * m.depth;
            apply_impact(car, m.nx, m.ny, events);
            hit = true;
        }
        if !hit {
            break;
        }
    }
}

/// Auto gegen Auto: Trennung und Stoß nach den Massen aus den Fahrzeugdaten (ein Sattelzug schiebt einen
/// Kleinwagen weg), Schaden nach der eigenen Geschwindigkeitsänderung.
pub fn collide_cars(a: &mut Car, b: &mut Car, events: &mut Vec<Event>) {
    let Some(m) = obb_vs_obb(&a.obb(), &b.obb()) else {
        return;
    };
    let (ma, mb) = (a.mass(), b.mass());
    // Anteil, den jedes Auto ausweicht bzw. an Geschwindigkeit ändert (gleiche Massen: je die Hälfte)
    let (ka, kb) = (mb / (ma + mb), ma / (ma + mb));
    a.x += m.nx * m.depth * ka;
    a.y += m.ny * m.depth * ka;
    b.x -= m.nx * m.depth * kb;
    b.y -= m.ny * m.depth * kb;
    let vn = (a.vx - b.vx) * m.nx + (a.vy - b.vy) * m.ny;
    if vn >= 0. {
        return;
    }
    let j = -(1. + RESTITUTION) * vn;
    a.vx += j * ka * m.nx;
    a.vy += j * ka * m.ny;
    b.vx -= j * kb * m.nx;
    b.vy -= j * kb * m.ny;
    damage(a, -vn * 0.8 * 2. * ka, events);
    damage(b, -vn * 0.8 * 2. * kb, events);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::Segment;

    fn car() -> Car {
        Car::new(1, 0., 0., 0., 0xff0000, Role::Traffic, "car")
    }

    #[test]
    fn ai_limits_come_from_the_vehicle_data() {
        let db = crate::vehdata::shared();
        let bus = Limits::of(db.get("stadtbus").unwrap());
        let sport = Limits::of(db.get("sportwagen_s").unwrap());
        // ein Bus beschleunigt und bremst wie ein Bus
        assert!(
            bus.accel(100.) < sport.accel(100.) * 0.4,
            "{bus:?} {sport:?}"
        );
        assert!(bus.brake < sport.brake * 0.6);
        assert!(bus.top < sport.top * 0.35);
        // 80 km/h Begrenzer/Ziel → 222 px/s
        assert!((bus.top - 80. / 3.6 * 10.).abs() < 1.);
        // bei Tempo begrenzt die Leistung, im Stand die Traktion
        assert!(sport.accel(0.) <= sport.traction + 1e-9 && sport.accel(800.) < sport.accel(100.));
    }

    #[test]
    fn grazing_a_wall_slides_along_it_head_on_stops() {
        assert!(crate::vehdata::game_feel().wall_slide);
        let hit = |deg: f64| {
            // Wand mit Normale (0, -1), Auto fährt im Winkel `deg` auf sie zu
            let mut c = car();
            let a = deg.to_radians();
            (c.vx, c.vy) = (a.cos() * 200., a.sin() * 200.);
            let mut ev = Vec::new();
            apply_impact(&mut c, 0., -1., &mut ev);
            (c.vx, c.vy)
        };
        let (gx, gy) = hit(15.);
        assert!(
            gx > 200. * 15f64.to_radians().cos() * 0.75,
            "streifend gleitet weiter: {gx}"
        );
        assert!(gy <= 0., "nicht mehr in die Wand: {gy}");
        let (hx, hy) = hit(90.);
        assert!(hx.hypot(hy) < 40., "frontal fast steht: {}", hx.hypot(hy));
    }

    #[test]
    fn heavy_trucks_push_small_cars_away() {
        let mut truck = Car::new(1, 0., 0., 0., 0, Role::Traffic, "garbage");
        let mut small = Car::new(2, 0., 0., 0., 0, Role::Traffic, "car");
        small.model = Some("kleinwagen");
        let (lw, sw) = (truck.hw, small.hw);
        small.x = lw + sw - 4.;
        truck.vx = 100.;
        assert!(
            truck.mass() > small.mass() * 8.,
            "{} {}",
            truck.mass(),
            small.mass()
        );
        let mut ev = Vec::new();
        collide_cars(&mut truck, &mut small, &mut ev);
        // der Müllwagen verliert kaum Tempo, der Kleinwagen fliegt davon
        assert!(truck.vx > 85., "{}", truck.vx);
        assert!(small.vx > 100., "{}", small.vx);
        // gleiche Autos: wie bisher je die Hälfte
        let (mut a, mut b) = (car(), car());
        b.x = a.hw * 2. - 4.;
        b.id = 1;
        a.vx = 100.;
        collide_cars(&mut a, &mut b, &mut ev);
        assert!((a.vx + b.vx - 100.).abs() < 1e-6);
    }

    #[test]
    fn arcade_accelerates_brakes_and_steers() {
        let mut c = car();
        c.controls.throttle = 1.;
        for _ in 0..120 {
            step_car(&mut c, 1. / 60., None);
        }
        // Beschleunigung aus den Daten: Traktion bzw. Leistung (KI fährt wie das Fahrzeug, kein Arcade-Wert)
        let v = c.forward_speed();
        let lim = Limits::of(data_vehicle(&c).unwrap());
        assert!(v > 50. && v < lim.accel(0.) * 2.05, "{v}");
        c.controls = Controls {
            brake: 1.,
            ..Default::default()
        };
        for _ in 0..60 {
            step_car(&mut c, 1. / 60., None);
        }
        assert!(c.forward_speed() <= 5.);
        // Gras bremst und begrenzt das Höchsttempo
        let mut g = car();
        g.controls.throttle = 1.;
        for _ in 0..600 {
            step_car(&mut g, 1. / 60., Some(Ground::Grass));
        }
        assert!(g.forward_speed() < MAX_SPEED * 0.5);
        let mut s = car();
        s.vx = 150.;
        s.controls = Controls {
            throttle: 0.3,
            steer: 1.,
            ..Default::default()
        };
        for _ in 0..30 {
            step_car(&mut s, 1. / 60., None);
        }
        assert!(s.angle > 0.2);
    }

    #[test]
    fn crash_damages_and_wrecks() {
        let mut c = car();
        let mut ev = Vec::new();
        damage(&mut c, 60., &mut ev);
        assert!(ev.is_empty() && c.health == HEALTH);
        damage(&mut c, 300., &mut ev);
        assert!((c.health - (100. - 230. * 0.07)).abs() < 1e-9);
        for _ in 0..10 {
            damage(&mut c, 400., &mut ev);
        }
        assert!(
            c.wrecked
                && ev
                    .iter()
                    .filter(|e| matches!(e, Event::Wreck { .. }))
                    .count()
                    == 1
        );
    }

    #[test]
    fn car_against_car_is_symmetric() {
        // gleiches Modell = gleiche Masse
        let mut a = car();
        let mut b = Car::new(2, 40., 0., 0., 0, Role::Traffic, "car");
        a.set_model("kompakt");
        b.set_model("kompakt");
        // Kompakter: 4,28 m lang → 2 px Überlappung bei 40 px Abstand, je 1 px auseinander
        assert!((a.hw - 21.4).abs() < 1e-9 && a.hw == b.hw && a.hh == b.hh);
        b.x = 2. * a.hw - 2.;
        a.vx = 200.;
        let mut ev = Vec::new();
        collide_cars(&mut a, &mut b, &mut ev);
        assert!((a.x + 1.).abs() < 1e-9 && (b.x - (2. * a.hw - 1.)).abs() < 1e-9);
        assert!((a.vx + b.vx - 200.).abs() < 1e-9, "Impuls bleibt erhalten");
        assert!(b.vx > 0. && a.vx < 200.);
    }

    #[test]
    fn walls_block_only_their_level() {
        let k = Knocked::new();
        let wall = Solid::Wall {
            seg: Segment {
                ax: 0.,
                ay: 0.,
                bx: 1.,
                by: 0.,
            },
            kind: WallKind::Wall,
            sub: crate::city::WallSub::Quay,
            lvl: 0,
        };
        let border = Solid::Wall {
            seg: Segment {
                ax: 0.,
                ay: 0.,
                bx: 1.,
                by: 0.,
            },
            kind: WallKind::Border,
            sub: crate::city::WallSub::Other,
            lvl: 0,
        };
        assert!(blocks(&k, &wall, 0) && !blocks(&k, &wall, 1) && blocks(&k, &border, 1));
        let post = Solid::Circle {
            x: 0.,
            y: 0.,
            r: 1.5,
            kind: CircleKind::Barrier { key: (0, 0) },
        };
        let mut k2 = Knocked::new();
        k2.insert((0, 0), 0.);
        assert!(blocks(&k, &post, 0) && !blocks(&k2, &post, 0) && !blocks(&k, &post, 1));
    }
}
