//! Mitfahren und Zug führen (Port von `ride.js`, `playertrain.js`, `trainphysics.js` und dem Mitfahr-Teil von
//! `world.js`). Fahrzeuge werden referenziert, nie kopiert: ein Fahrplan-Fahrzeug (`Ref::Veh`), ein Bus als KI-Auto
//! (`Ref::Car`) oder der Zug, den der Spieler führt (`Ref::PlayerTrain`). Verschwindet das Fahrzeug, steigt die Figur
//! an der letzten Haltestelle aus. Mitfahren mit G (Controller: Steuerkreuz unten), auch Aufspringen; Aussteigen
//! jederzeit, schnell = Abspringen mit Sturz; unter Tage und auf der Hochbahn nur am Bahnhof (Ausgang an der Straße).
//! Am Führerstand übernimmt E die Bahn: dann gilt eigene Fahrphysik (nur Tempo entlang der Linie), Türen an Halten,
//! Trinkgeld für sanftes, genaues Halten, Wenden am Linienende.
use crate::events::Event;
use crate::math::hash01;
use crate::transit::{CarPos, Mode, Pattern, Shape, point_on_shape, position_at, train_cars};
use crate::world::{Notice, World};

/// px: Reichweite zum Einsteigen; Führerstand an der Spitze
pub const REACH: f64 = 25.;
pub const CAB: f64 = 30.;
/// px/s: darüber Auf-/Abspringen (25 bzw. 10 km/h), ab 40 km/h verletzt das Abspringen
pub const HOP_ON: f64 = 25. / 0.36;
pub const HOP_OFF: f64 = 10. / 0.36;
pub const HURT_FROM: f64 = 40. / 0.36;
pub const STUN: f64 = 1.2;
pub const HURT: f64 = 10.;

/// Fahrphysik je Bahnart: Höchsttempo (px/s), Anfahren, Bremse, Notbremse (px/s²).
pub fn drive_params(mode: Mode) -> (f64, f64, f64, f64) {
    match mode {
        Mode::Tram => (60. / 0.36, 13., 15., 25.),
        Mode::SBahn => (100. / 0.36, 10., 12., 25.),
        _ => (70. / 0.36, 11., 12., 25.),
    }
}
pub const ROLL: f64 = 0.6;
/// px: Haltebereich um eine Haltestelle, „genau“ gehalten
pub const STOP_ZONE: f64 = 250.;
pub const STOP_EXACT: f64 = 30.;
/// px/s: darunter steht der Zug
pub const STILL_V: f64 = 3.;
pub const TIP_MAX: f64 = 10.;
/// px/s²: sanft gebremst
pub const GENTLE: f64 = 13.;
/// s: Türen schließen selbst
pub const DOORS_AUTO: f64 = 20.;
/// px Abstand zum Zug voraus
pub const SAFE: f64 = 80.;
/// px: Straßenbahn hält so weit vor einem Hindernis
pub const OBSTACLE_MARGIN: f64 = 40.;

#[derive(Debug, Clone, PartialEq)]
pub enum Ref {
    Veh { pid: usize, key: String },
    Car(u32),
    PlayerTrain,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RideKind {
    Passenger,
    Driver,
}
/// Letzte Haltestelle (Ausstieg, wenn das Fahrzeug verschwindet).
#[derive(Debug, Clone, PartialEq)]
pub struct LastStop {
    pub x: f64,
    pub y: f64,
    pub name: String,
    pub i: usize,
    pub pid: usize,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Ride {
    pub kind: RideKind,
    pub r: Ref,
    pub mode: Mode,
    /// Wagen, in dem die Figur sitzt
    pub car: usize,
    pub last_stop: LastStop,
    pub since: f64,
    pub line: String,
    pub dest: String,
    pub speed: f64,
    pub underground: bool,
}

/// Lage eines Fahrzeugs.
#[derive(Debug, Clone, PartialEq)]
pub struct VState {
    pub mode: Mode,
    pub pid: usize,
    pub s: f64,
    pub speed: f64,
    pub cars: Vec<CarPos>,
    pub dwelling: bool,
    pub stop: usize,
    pub underground: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doors {
    Closed,
    Open,
}
/// Fahrschalter-Zustand des eigenen Zugs.
#[derive(Debug, Clone, PartialEq)]
pub struct Drive {
    pub mode: Mode,
    pub v: f64,
    pub doors: Doors,
    pub door_t: f64,
    /// Verzögerungen der letzten 8 s (Dauer, Wert) – für das Trinkgeld
    pub decel: Vec<(f64, f64)>,
    pub stopped: bool,
}
impl Drive {
    pub fn new(mode: Mode, v: f64) -> Self {
        Self {
            mode,
            v,
            doors: Doors::Closed,
            door_t: 0.,
            decel: Vec::new(),
            stopped: v <= 0.,
        }
    }
    pub fn max_decel(&self) -> f64 {
        self.decel.iter().map(|d| d.1).fold(0., f64::max)
    }
}
/// Eingabe des Fahrschalters.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DriveInput {
    pub throttle: f64,
    pub brake: f64,
    pub emergency: bool,
    /// freier Weg (px): davor Zwangsbremsung
    pub limit: f64,
    /// Schienenhaftung (Wetter)
    pub adhesion: f64,
}
/// Zug führen (trainphysics.js stepDrive): Anfahren mit abnehmender Zugkraft über 40 % des Höchsttempos, Bremse und
/// Notbremse konstant, Ausrollen langsam; vor einem Hindernis/Endhalt bremst der Zug selbsttätig. Nie rückwärts.
pub fn step_drive(d: &mut Drive, inp: &DriveInput, dt: f64) {
    if dt <= 0. {
        return;
    }
    let (vmax, acc, brake, emergency) = drive_params(d.mode);
    let adh = if inp.adhesion > 0. { inp.adhesion } else { 1. };
    let v0 = d.v;
    if d.doors != Doors::Closed {
        d.v = 0.;
        d.door_t += dt;
        return;
    }
    let a = if inp.emergency {
        -emergency * adh
    } else if inp.brake > 0. {
        -brake * inp.brake * adh
    } else if inp.throttle > 0. {
        let k = if d.v < vmax * 0.4 {
            1.
        } else {
            ((vmax - d.v) / (vmax * 0.6)).max(0.)
        };
        acc * adh * inp.throttle * k
    } else {
        -ROLL
    };
    let mut v = (d.v + a * dt).clamp(0., vmax);
    if inp.limit.is_finite() {
        let allowed = (2. * emergency * adh * (inp.limit - v * dt).max(0.))
            .max(0.)
            .sqrt();
        if v > allowed {
            v = v.min(allowed).max(0.);
        }
        if inp.limit <= 1. {
            v = 0.;
        }
    }
    d.v = v;
    let dec = (emergency * adh).min(((v0 - v) / dt).max(0.));
    d.decel.push((dt, dec));
    let mut sum = 0.;
    let mut cut = 0;
    for (i, e) in d.decel.iter().enumerate().rev() {
        sum += e.0;
        if sum > 8. {
            cut = i;
            break;
        }
    }
    if cut > 0 {
        d.decel.drain(..cut);
    }
    d.stopped = v < STILL_V;
}
/// Haltestelle im Haltebereich um s: (Index, Abstand); bei Gleichstand die spätere.
pub fn stop_info(p: &Pattern, s: f64) -> Option<(usize, f64)> {
    let mut best: Option<(usize, f64)> = None;
    for (i, &st) in p.stops.iter().enumerate() {
        let dist = s - st;
        if dist.abs() <= STOP_ZONE && best.is_none_or(|b| dist.abs() <= b.1.abs()) {
            best = Some((i, dist));
        }
    }
    best
}
/// Trinkgeld: genau gehalten und sanft gebremst.
pub fn tip_for(dist: f64, max_dec: f64) -> f64 {
    let place = if dist.abs() <= STOP_EXACT {
        1.
    } else {
        (1. - (dist.abs() - STOP_EXACT) / (STOP_ZONE - STOP_EXACT)).max(0.)
    };
    let gentle = if max_dec <= GENTLE {
        1.
    } else {
        (1. - (max_dec - GENTLE) / GENTLE).max(0.)
    };
    (TIP_MAX * place * gentle * 100.).round() / 100.
}

/// Der vom Spieler übernommene Zug (verlässt den Fahrplan).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerTrain {
    pub pid: usize,
    pub s: f64,
    pub v: f64,
    pub drive: Drive,
    pub next_stop: usize,
    pub served: Vec<usize>,
    pub at_stop: Option<(usize, f64)>,
    /// s ohne Fahrer (nach 30 s außer Sicht weg)
    pub left_t: Option<f64>,
    pub passengers: u32,
    pub wait_t: f64,
    pub blocked: bool,
}

/// Abstand Punkt → Wagen-Rechteck (0 innen).
fn dist_to_car(x: f64, y: f64, c: &CarPos) -> f64 {
    let (dx, dy) = (x - c.x, y - c.y);
    let (ca, sa) = (c.angle.cos(), c.angle.sin());
    let lx = (dx * ca + dy * sa).abs() - c.l / 2.;
    let ly = (-dx * sa + dy * ca).abs() - c.w / 2.;
    lx.max(0.).hypot(ly.max(0.))
}

/// Fahrzeug in Reichweite: Referenz, Art, Abstand, nächster Wagen, Abstand zur Spitze.
#[derive(Debug, Clone, PartialEq)]
pub struct Near {
    pub r: Ref,
    pub mode: Mode,
    pub dist: f64,
    pub car: usize,
    pub front: f64,
}

fn bus_cars(x: f64, y: f64, angle: f64) -> Vec<CarPos> {
    vec![CarPos {
        x,
        y,
        angle,
        l: crate::transit::BUS_L,
        w: crate::transit::BUS_W,
        first: true,
        last: true,
    }]
}

/// Tempo aus dem Fahrplan: Abstand der Halte durch die Fahrzeit ohne Haltezeit.
pub fn speed_of_pattern(p: &Pattern, tau: f64) -> f64 {
    let pos = position_at(p, tau);
    if pos.dwelling || pos.stop == 0 {
        return 0.;
    }
    let i = pos.stop;
    let span = p.off[i] - p.off[i - 1];
    (p.stops[i] - p.stops[i - 1]) / (span - p.dwell.min(span * 0.4)).max(1.)
}

impl World {
    fn shape_and_pattern(&self, pid: usize) -> Option<(&Pattern, &Shape)> {
        let tr = self.transit.as_ref()?;
        let p = tr.patterns.get(pid)?;
        Some((p, tr.shape_of(p)))
    }
    fn underground_s(&mut self, pid: usize, s: f64) -> bool {
        let Some(tr) = self.transit.as_ref() else {
            return false;
        };
        let p = &tr.patterns[pid];
        self.ug
            .underground_at_s(&mut self.city, p, tr.shape_of(p), s)
    }

    /// Lage eines Fahrzeugs; `None`, wenn es verschwunden ist.
    pub fn vehicle_state(&mut self, r: &Ref) -> Option<VState> {
        let tr = self.transit.as_ref()?;
        match r {
            Ref::Car(id) => {
                let c = self.cars.iter().find(|c| c.id == *id && !c.wrecked)?;
                let b = c.bus.as_ref()?;
                Some(VState {
                    mode: Mode::Bus,
                    pid: b.pid,
                    s: b.s,
                    speed: c.speed(),
                    cars: bus_cars(c.x, c.y, c.angle),
                    dwelling: b.boarding,
                    stop: b.stop,
                    underground: false,
                })
            }
            Ref::PlayerTrain => {
                let t = self.player_train.as_ref()?;
                let p = &tr.patterns[t.pid];
                let (pid, s, v, stop, mode) = (t.pid, t.s, t.v, t.next_stop, p.mode);
                let cars = train_cars(tr.shape_of(p), mode, s);
                let ug = self.underground_s(pid, s);
                Some(VState {
                    mode,
                    pid,
                    s,
                    speed: v,
                    cars,
                    dwelling: v < STILL_V,
                    stop,
                    underground: ug,
                })
            }
            Ref::Veh { pid, key } => {
                let v = self
                    .transit_state
                    .tracked
                    .get(pid)?
                    .veh
                    .iter()
                    .find(|v| v.key == *key && !v.gone)?;
                if v.live.is_some() {
                    return None;
                }
                let p = &tr.patterns[*pid];
                if p.mode == Mode::Bus {
                    return None;
                }
                let pos = position_at(p, v.tau);
                if pos.done {
                    return None;
                }
                let speed = if v.blocked_t > 0. {
                    0.
                } else {
                    speed_of_pattern(p, v.tau)
                };
                let (mode, pid) = (p.mode, *pid);
                let cars = train_cars(tr.shape_of(p), mode, pos.s);
                let ug = self.underground_s(pid, pos.s);
                Some(VState {
                    mode,
                    pid,
                    s: pos.s,
                    speed,
                    cars,
                    dwelling: pos.dwelling,
                    stop: pos.stop,
                    underground: ug,
                })
            }
        }
    }

    /// Ebene eines Wagens: S-/U-Bahn = Ebene des Gleises darunter, Straßenbahn und Bus = Ebene der Figur.
    fn car_level(&mut self, st: &VState, c: &CarPos) -> i8 {
        if !st.mode.rail() {
            return self.player.level.lvl;
        }
        crate::tunnel::rail_level_at(&mut self.city, c.x, c.y, crate::tunnel::PROBE).unwrap_or(0)
    }
    /// Hochbahn: Wagen über dem Boden – aussteigen nur am Bahnhof.
    fn elevated(&mut self, st: &VState, c: &CarPos) -> bool {
        st.mode.rail() && !st.underground && self.car_level(st, c) >= 1
    }

    /// Fahrzeuge in Reichweite (je Fahrzeug der nächste Wagen), nächstes zuerst.
    pub fn transit_near(&mut self, x: f64, y: f64, r: f64) -> Vec<Near> {
        let Some(tr) = self.transit.as_ref() else {
            return Vec::new();
        };
        let mut refs = Vec::new();
        for (&pid, s) in &self.transit_state.tracked {
            let p = &tr.patterns[pid];
            if p.mode == Mode::Bus {
                continue;
            }
            for v in &s.veh {
                if v.gone || v.live.is_some() {
                    continue;
                }
                let (hx, hy, _) = point_on_shape(tr.shape_of(p), position_at(p, v.tau).s);
                if (hx - x).abs() < 2200. && (hy - y).abs() < 2200. {
                    refs.push(Ref::Veh {
                        pid,
                        key: v.key.clone(),
                    });
                }
            }
        }
        for c in &self.cars {
            if c.bus.is_some()
                && c.driver == Some(crate::car::Driver::Npc)
                && (c.x - x).abs() < 200.
                && (c.y - y).abs() < 200.
            {
                refs.push(Ref::Car(c.id));
            }
        }
        if self.player_train.is_some() {
            refs.push(Ref::PlayerTrain);
        }
        let mut out: Vec<Near> = Vec::new();
        for rf in refs {
            let Some(st) = self.vehicle_state(&rf) else {
                continue;
            };
            if st.underground {
                continue;
            }
            let mut best: Option<Near> = None;
            for (i, c) in st.cars.iter().enumerate() {
                let d = dist_to_car(x, y, c);
                if d > r {
                    continue;
                }
                // gleiche Ebene; auf eine andere (Hochbahn von der Straße) nur, solange der Zug hält (Treppen)
                if !st.dwelling && self.car_level(&st, c) != self.player.level.lvl {
                    continue;
                }
                let f = &st.cars[0];
                let (fx, fy) = (
                    f.x + f.angle.cos() * f.l / 2.,
                    f.y + f.angle.sin() * f.l / 2.,
                );
                if best.as_ref().is_none_or(|b| d < b.dist) {
                    best = Some(Near {
                        r: rf.clone(),
                        mode: st.mode,
                        dist: d,
                        car: i,
                        front: (x - fx).hypot(y - fy),
                    });
                }
            }
            out.extend(best);
        }
        out.sort_by(|a, b| a.dist.total_cmp(&b.dist));
        out
    }

    fn last_stop_of(&self, st: &VState) -> LastStop {
        let Some((p, sh)) = self.shape_and_pattern(st.pid) else {
            return LastStop {
                x: self.player.x,
                y: self.player.y,
                name: String::new(),
                i: 0,
                pid: st.pid,
            };
        };
        let i = (if st.dwelling {
            st.stop
        } else {
            st.stop.saturating_sub(1)
        })
        .min(p.stops.len().saturating_sub(1));
        let (x, y, _) = point_on_shape(sh, p.stops[i]);
        LastStop {
            x,
            y,
            name: p.stop_names.get(i).cloned().unwrap_or_default(),
            i,
            pid: st.pid,
        }
    }
    fn line_and_dest(&self, pid: usize) -> (String, String) {
        self.shape_and_pattern(pid)
            .map_or_else(Default::default, |(p, _)| {
                (
                    p.name.clone(),
                    p.stop_names.last().cloned().unwrap_or_default(),
                )
            })
    }
    fn notice(&mut self, t: &str) {
        self.notice = Some(Notice {
            text: t.into(),
            t: 1.5,
        });
    }

    /// Mitfahren (G): Fahrzeug in Reichweite eines Wagens, auch in Fahrt (Aufspringen).
    pub fn board_transit(&mut self) -> bool {
        if self.player.stun > 0. {
            return false; // gestürzt: erst aufstehen
        }
        let (x, y) = (self.player.x, self.player.y);
        let Some(hit) = self
            .transit_near(x, y, REACH)
            .into_iter()
            .find(|h| h.r != Ref::PlayerTrain)
        else {
            return false;
        };
        let Some(st) = self.vehicle_state(&hit.r) else {
            return false;
        };
        let hop = st.speed > HOP_ON;
        let (line, dest) = self.line_and_dest(st.pid);
        self.player.ride = Some(Ride {
            kind: RideKind::Passenger,
            r: hit.r,
            mode: st.mode,
            car: hit.car,
            last_stop: self.last_stop_of(&st),
            since: self.time,
            line: line.clone(),
            dest,
            speed: st.speed,
            underground: false,
        });
        self.player.click = None;
        self.events.push(Event::Board { line, hop, x, y });
        true
    }

    /// Straßenausgang eines Bahnhofs: Bahnhofs-POI mit passendem Namen in der Nähe, sonst nächster Gehweg.
    pub fn station_exit(&mut self, pid: usize, i: usize) -> (f64, f64) {
        let Some((p, sh)) = self.shape_and_pattern(pid) else {
            return (self.player.x, self.player.y);
        };
        let (ax, ay, _) = point_on_shape(sh, p.stops[i.min(p.stops.len() - 1)]);
        let raw = p.stop_names.get(i).cloned().unwrap_or_default();
        let name = raw
            .trim_start_matches("S ")
            .trim_start_matches("U ")
            .split(" (")
            .next()
            .unwrap_or("")
            .to_string();
        let pois = self.city.pois_near(ax, ay, 500.);
        let station = |q: &&crate::city::Poi| matches!(q.cat, "ubahn" | "sbahn" | "bahn");
        let near = |q: &&crate::city::Poi| (q.x - ax).hypot(q.y - ay);
        let pick = pois
            .iter()
            .filter(station)
            .filter(|q| name.is_empty() || q.name.contains(&name))
            .min_by(|a, b| near(a).total_cmp(&near(b)))
            .or_else(|| {
                pois.iter()
                    .filter(station)
                    .min_by(|a, b| near(a).total_cmp(&near(b)))
            });
        let (bx, by) = pick.map_or((ax, ay), |q| (q.x, q.y));
        match crate::pedestrians::nearest_spot(&mut self.city, &mut self.sidewalks, bx, by, 300.) {
            Some(sp) => self.sidewalks.point(&mut self.city, sp.edge, sp.side, sp.s),
            None => (bx, by),
        }
    }

    /// Freier Platz neben Wagen i: rechts in Fahrtrichtung zuerst, dann links, dann hinter dem letzten Wagen.
    fn alight_spot(&mut self, st: &VState, i: usize) -> Option<(f64, f64)> {
        let c = st.cars[i.min(st.cars.len() - 1)];
        let (nx, ny) = (-c.angle.sin(), c.angle.cos());
        let d = c.w / 2. + 12.;
        let last = st.cars[st.cars.len() - 1];
        let cands = [
            (c.x + nx * d, c.y + ny * d),
            (c.x - nx * d, c.y - ny * d),
            (
                last.x - last.angle.cos() * (last.l / 2. + 14.),
                last.y - last.angle.sin() * (last.l / 2. + 14.),
            ),
        ];
        let lvl = self.player.level.lvl;
        cands
            .into_iter()
            .find(|&(x, y)| self.spot_free_here(x, y, 8., lvl))
    }

    /// Aussteigen (G): unter Tage und auf der Hochbahn nur am Bahnhof, sonst neben dem Wagen (schnell: abspringen).
    pub fn alight_transit(&mut self) -> bool {
        let Some(r) = self.player.ride.clone() else {
            return false;
        };
        let Some(st) = self.vehicle_state(&r.r) else {
            self.end_ride(false);
            return true;
        };
        let car = st.cars[r.car.min(st.cars.len() - 1)];
        if st.underground || self.elevated(&st, &car) {
            if !st.dwelling {
                self.notice(if st.underground {
                    "Nur am Bahnsteig"
                } else {
                    "Aussteigen nur am Bahnhof"
                });
                return false;
            }
            let (x, y) = self.station_exit(st.pid, st.stop);
            self.player.ride = None;
            (self.player.x, self.player.y) = (x, y);
            self.player.level.lvl = 0;
            self.events.push(Event::Alight { hop: false, x, y });
            return true;
        }
        let Some((x, y)) = self.alight_spot(&st, r.car) else {
            self.notice("Kein Platz zum Aussteigen");
            return false;
        };
        let hop = st.speed > HOP_OFF;
        self.player.ride = None;
        (self.player.x, self.player.y) = (x, y);
        if hop {
            let (fx, fy) = (x + car.angle.cos() * 20., y + car.angle.sin() * 20.);
            let lvl = self.player.level.lvl;
            if self.spot_free_here(fx, fy, 8., lvl) {
                (self.player.x, self.player.y) = (fx, fy); // Schwung in Fahrtrichtung
            }
            self.player.stun = STUN;
            if st.speed > HURT_FROM {
                crate::combat::hurt_player(self, HURT, (car.x, car.y));
            }
        }
        let (px, py) = (self.player.x, self.player.y);
        self.events.push(Event::Alight { hop, x: px, y: py });
        true
    }

    /// Fahrt beenden ohne Fahrzeug (verschwunden, K. o.; beim Teleport ohne Versetzen): an der letzten Haltestelle,
    /// bei S-/U-Bahn an deren Straßenausgang.
    pub fn end_ride(&mut self, teleport: bool) {
        let Some(r) = self.player.ride.take() else {
            return;
        };
        if !teleport {
            let (x, y) = self.ride_exit(&r);
            (self.player.x, self.player.y) = (x, y);
            self.player.level.lvl = 0;
        }
        let (x, y) = (self.player.x, self.player.y);
        self.events.push(Event::RideEnd { x, y });
    }
    /// Wo ein Fahrgast ohne Fahrzeug landet (auch für den Spielstand).
    pub fn ride_exit(&mut self, r: &Ride) -> (f64, f64) {
        let rail = self
            .shape_and_pattern(r.last_stop.pid)
            .is_some_and(|(p, _)| p.mode.rail());
        if rail {
            self.station_exit(r.last_stop.pid, r.last_stop.i)
        } else {
            (r.last_stop.x, r.last_stop.y)
        }
    }

    /// Jeden Schritt: die Figur sitzt im Wagen.
    pub fn update_ride(&mut self) {
        let Some(r) = self.player.ride.clone() else {
            return;
        };
        if self.player.combat.dead {
            self.end_ride(false);
            return;
        }
        let Some(st) = self.vehicle_state(&r.r) else {
            self.end_ride(false);
            return;
        };
        let c = st.cars[r.car.min(st.cars.len() - 1)];
        (self.player.x, self.player.y, self.player.angle) = (c.x, c.y, c.angle);
        if st.underground {
            self.player.level.lvl = -2;
        } else if st.mode.rail() {
            self.player.level.lvl = self.car_level(&st, &c);
        } else if r.underground {
            self.player.level.lvl = 0;
        }
        let last = (st.dwelling && r.kind == RideKind::Passenger).then(|| self.last_stop_of(&st));
        if let Some(ride) = self.player.ride.as_mut() {
            if let Some(l) = last {
                ride.last_stop = l;
            }
            ride.speed = st.speed;
            ride.underground = st.underground;
        }
    }

    // --- Zug führen -------------------------------------------------------------------------------------------

    /// Am Führerstand (Wagen 0, Spitze ≤ 30 px) die Bahn übernehmen.
    pub fn take_train(&mut self, hit: &Near) -> bool {
        if matches!(hit.r, Ref::Car(_)) || hit.car != 0 || hit.front > CAB {
            return false;
        }
        if hit.r == Ref::PlayerTrain {
            return self.retake_train();
        }
        let Ref::Veh { pid, key } = &hit.r else {
            return false;
        };
        let Some(st) = self.vehicle_state(&hit.r) else {
            return false;
        };
        if st.mode == Mode::Bus {
            return false;
        }
        let Some(v) = self
            .transit_state
            .tracked
            .get_mut(pid)
            .and_then(|t| t.veh.iter_mut().find(|v| v.key == *key))
        else {
            return false;
        };
        v.gone = true; // aus dem Fahrplan
        let tau = v.tau;
        self.release_train();
        let Some((p, _)) = self.shape_and_pattern(*pid) else {
            return false;
        };
        let pos = position_at(p, tau);
        let passengers = 20 + (hash01((p.id * 31) as f64 + self.clock.floor()) * 60.) as u32;
        let i = pos.stop.saturating_sub(1);
        self.player_train = Some(PlayerTrain {
            pid: *pid,
            s: st.s,
            v: st.speed,
            drive: Drive::new(st.mode, st.speed),
            next_stop: pos.stop,
            served: Vec::new(),
            at_stop: None,
            left_t: None,
            passengers,
            wait_t: 0.,
            blocked: false,
        });
        self.start_driving(*pid, i);
        true
    }
    fn start_driving(&mut self, pid: usize, i: usize) {
        let Some((p, sh)) = self.shape_and_pattern(pid) else {
            return;
        };
        let (x, y, _) = point_on_shape(sh, p.stops[i]);
        let last = LastStop {
            x,
            y,
            name: p.stop_names.get(i).cloned().unwrap_or_default(),
            i,
            pid,
        };
        let (line, dest, mode) = (
            p.name.clone(),
            p.stop_names.last().cloned().unwrap_or_default(),
            p.mode,
        );
        self.player.ride = Some(Ride {
            kind: RideKind::Driver,
            r: Ref::PlayerTrain,
            mode,
            car: 0,
            last_stop: last,
            since: self.time,
            line: line.clone(),
            dest,
            speed: 0.,
            underground: false,
        });
        self.player.click = None;
        if let Some(t) = self.player_train.as_mut() {
            t.left_t = None;
        }
        let (px, py) = (self.player.x, self.player.y);
        self.events.push(Event::TrainTake { line, x: px, y: py });
    }
    fn retake_train(&mut self) -> bool {
        let Some(t) = self.player_train.as_ref() else {
            return false;
        };
        if self.player.ride.is_some() {
            return false;
        }
        let i = t.at_stop.map_or(t.next_stop.saturating_sub(1), |a| a.0);
        let pid = t.pid;
        self.start_driving(pid, i);
        true
    }
    /// Den stehengelassenen eigenen Zug an den Fahrplan zurückgeben (Fahrzeit per Bisektion aus der Bogenlänge).
    pub fn release_train(&mut self) {
        if self
            .player
            .ride
            .as_ref()
            .is_some_and(|r| r.kind == RideKind::Driver)
        {
            return;
        }
        let Some(t) = self.player_train.take() else {
            return;
        };
        let time = self.time;
        let Some((p, _)) = self.shape_and_pattern(t.pid) else {
            return;
        };
        let (mut lo, mut hi) = (0., p.duration);
        for _ in 0..40 {
            let mid = (lo + hi) / 2.;
            if position_at(p, mid).s < t.s {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        if let Some(st) = self.transit_state.tracked.get_mut(&t.pid) {
            st.veh.push(crate::transit::Veh {
                tau: hi,
                delay: 0.,
                key: format!("pt{time:.3}"),
                live: None,
                gone: false,
                blocked_t: 0.,
                rang: false,
            });
        }
    }

    /// Freier Weg bis zum Heck des nächsten Zugs voraus auf derselben Strecke, minus Sicherheitsabstand.
    fn train_ahead(&self, t: &PlayerTrain) -> f64 {
        let Some(tr) = self.transit.as_ref() else {
            return f64::INFINITY;
        };
        let p = &tr.patterns[t.pid];
        let sh = tr.shape_of(p);
        let look = 2500.;
        let (px, py, _) = point_on_shape(sh, t.s + 200.);
        let mut free = f64::INFINITY;
        for (&pid, s) in &self.transit_state.tracked {
            let q = &tr.patterns[pid];
            if q.mode != p.mode {
                continue;
            }
            let qsh = tr.shape_of(q);
            for v in &s.veh {
                if v.gone {
                    continue;
                }
                let qs = position_at(q, v.tau).s;
                let (hx, hy, _) = point_on_shape(qsh, qs);
                if (hx - px).abs() > look || (hy - py).abs() > look {
                    continue;
                }
                let (tx, ty, _) = point_on_shape(qsh, qs - q.mode.train_len());
                let mut d = 0.;
                while d <= look {
                    let (mx, my, _) = point_on_shape(sh, t.s + d);
                    if (mx - tx).hypot(my - ty) <= 30. {
                        free = free.min((d - SAFE).max(0.));
                        break;
                    }
                    d += 20.;
                }
            }
        }
        free
    }
    /// Freier Weg der eigenen Straßenbahn bis zum ersten Hindernis auf dem Gleis.
    fn tram_free(&self, sh: &Shape, s: f64, patient: bool) -> f64 {
        let (_, _, a) = point_on_shape(sh, s);
        let head = patient.then_some(a);
        for d in [20., 45., 75., 110., 150., 200.] {
            let (qx, qy, _) = point_on_shape(sh, s + d);
            if self.rail_obstacle_at(qx, qy, head) {
                return (d - OBSTACLE_MARGIN).max(0.);
            }
        }
        f64::INFINITY
    }
    /// Schienenhaftung an der Zugspitze: oberirdisch aus dem Wetter, im Tunnel trocken.
    fn train_adhesion(&mut self, pid: usize, s: f64) -> f64 {
        if self.underground_s(pid, s) {
            return 1.;
        }
        let Some((p, sh)) = self.shape_and_pattern(pid) else {
            return 1.;
        };
        let rail = p.mode.rail();
        let (x, y, _) = point_on_shape(sh, s);
        let lvl = if rail {
            crate::tunnel::rail_level_at(&mut self.city, x, y, crate::tunnel::PROBE).unwrap_or(0)
        } else {
            0
        };
        let w = self.weather;
        crate::traction::adhesion_of(&crate::traction::road_condition(
            &mut self.city,
            &w,
            x,
            y,
            lvl,
        ))
    }

    /// Am Endhalt, Zug steht, Türen zu und der Halt ist bedient: dann heißt Aktion „Wenden“.
    pub fn at_terminus(&self) -> bool {
        let Some(t) = &self.player_train else {
            return false;
        };
        let Some((p, _)) = self.shape_and_pattern(t.pid) else {
            return false;
        };
        let last = p.stops.len() - 1;
        t.drive.v == 0.
            && t.drive.doors == Doors::Closed
            && (t.s - p.stops[last]).abs() <= STOP_ZONE
            && t.served.contains(&last)
    }
    /// Am Endhalt in die Gegenrichtung: Muster derselben Linie, dessen erster Halt ≤ 60 m vom eigenen Endhalt liegt.
    pub fn turn_around(&mut self) -> bool {
        let Some(t) = self.player_train.as_ref() else {
            return false;
        };
        if t.drive.v > 0. {
            return false;
        }
        let Some(tr) = self.transit.as_ref() else {
            return false;
        };
        let p = &tr.patterns[t.pid];
        let n = p.stops.len();
        if t.s < p.stops[n - 1] - STOP_ZONE {
            return false;
        }
        let (ex, ey, _) = point_on_shape(tr.shape_of(p), p.stops[n - 1]);
        let mut best: Option<(usize, f64)> = None;
        for q in &tr.patterns {
            if q.name != p.name || q.id == p.id || q.mode != p.mode {
                continue;
            }
            let (ax, ay, _) = point_on_shape(tr.shape_of(q), q.stops[0]);
            let d = (ax - ex).hypot(ay - ey);
            if d < 600. && best.is_none_or(|b| d < b.1) {
                best = Some((q.id, d));
            }
        }
        let Some((bid, _)) = best else {
            self.notice("Hier kann nicht gewendet werden");
            return false;
        };
        let q = &tr.patterns[bid];
        let (s0, line, dest) = (
            q.stops[0],
            q.name.clone(),
            q.stop_names.last().cloned().unwrap_or_default(),
        );
        if let Some(t) = self.player_train.as_mut() {
            t.pid = bid;
            t.s = s0;
            t.next_stop = 1;
            t.served = vec![0];
            t.at_stop = Some((0, 0.));
            t.drive.v = 0.;
            t.v = 0.;
        }
        if let Some(r) = self.player.ride.as_mut() {
            r.line = line.clone();
            r.dest = dest;
        }
        self.events.push(Event::TurnAround { line });
        true
    }

    /// Zug ausführen: Fahrschalter, Zwangsbremsung vor Zügen/Hindernissen/Endhalt, Türen und Trinkgeld.
    pub fn update_player_train(&mut self, input: &crate::world::Input, dt: f64) {
        let Some(mut t) = self.player_train.take() else {
            return;
        };
        let driving = self
            .player
            .ride
            .as_ref()
            .is_some_and(|r| r.kind == RideKind::Driver);
        let Some((p, sh)) = self
            .shape_and_pattern(t.pid)
            .map(|(p, s)| (p.clone(), s.clone()))
        else {
            return;
        };
        let end_free = (p.stops[p.stops.len() - 1] - t.s).max(0.);
        let ahead_free = self.train_ahead(&t);
        let strict = if p.mode == Mode::Tram {
            self.tram_free(&sh, t.s, false)
        } else {
            f64::INFINITY
        };
        t.wait_t = if strict < 5. {
            t.wait_t
                + if t.drive.v < 1. || t.wait_t > crate::transitlive::TRAM_PATIENCE {
                    dt
                } else {
                    0.
                }
        } else {
            0.
        };
        let tf = if t.wait_t > crate::transitlive::TRAM_PATIENCE {
            self.tram_free(&sh, t.s, true)
        } else {
            strict
        };
        let limit = end_free.min(ahead_free).min(tf);
        let adhesion = self.train_adhesion(t.pid, t.s);
        let inp = if driving {
            DriveInput {
                throttle: input.throttle,
                brake: input.brake,
                emergency: input.handbrake,
                limit,
                adhesion,
            }
        } else {
            DriveInput {
                brake: 1.,
                limit,
                adhesion,
                ..Default::default()
            }
        };
        let was = t.blocked;
        step_drive(&mut t.drive, &inp, dt);
        t.blocked = ahead_free < 400. && t.drive.v < 5.;
        if t.blocked && !was {
            self.events.push(Event::TrainBlocked);
        }
        t.v = t.drive.v;
        t.s += t.v * dt;
        while t.next_stop < p.stops.len() - 1 && t.s > p.stops[t.next_stop] + STOP_ZONE {
            t.next_stop += 1;
        }
        t.at_stop = if t.drive.stopped {
            stop_info(&p, t.s)
        } else {
            None
        };
        // Türen (Aktion): nur im Stand an einer Haltestelle; am bedienten Endhalt heißt Aktion Wenden
        let terminus = {
            let last = p.stops.len() - 1;
            t.drive.v == 0.
                && t.drive.doors == Doors::Closed
                && (t.s - p.stops[last]).abs() <= STOP_ZONE
                && t.served.contains(&last)
        };
        if driving && input.action && !terminus {
            if t.drive.doors == Doors::Closed
                && let Some((i, dist)) = t.at_stop
            {
                t.drive.doors = Doors::Open;
                t.drive.door_t = 0.;
                let first = !t.served.contains(&i);
                let tip = if first {
                    tip_for(dist, t.drive.max_decel())
                } else {
                    0.
                };
                if first {
                    t.served.push(i);
                }
                let k = (self.clock / 10.).floor();
                let out = (hash01((t.pid * 97 + i) as f64 + k) * 12.) as u32;
                let inn = (hash01((t.pid * 53 + i * 7) as f64 + k) * 14.) as u32;
                t.passengers = t.passengers.saturating_sub(out) + inn;
                self.events.push(Event::DoorsOpen { out, inn, first });
                if tip > 0. {
                    let amount = tip.round();
                    self.money += amount;
                    self.events.push(Event::Tip { amount });
                }
                let (x, y, _) = point_on_shape(&sh, p.stops[i]);
                if let Some(r) = self.player.ride.as_mut() {
                    r.last_stop = LastStop {
                        x,
                        y,
                        name: p.stop_names.get(i).cloned().unwrap_or_default(),
                        i,
                        pid: p.id,
                    };
                }
            } else if t.drive.doors == Doors::Open {
                t.drive.doors = Doors::Closed;
                self.events.push(Event::DoorsClose);
            }
        }
        if t.drive.doors == Doors::Open && t.drive.door_t > DOORS_AUTO {
            t.drive.doors = Doors::Closed;
            self.events.push(Event::DoorsClose);
        }
        // ohne Fahrer: nach 30 s außer Sicht entfernen
        if !driving {
            let left = t.left_t.unwrap_or(0.) + dt;
            t.left_t = Some(left);
            let (hx, hy, _) = point_on_shape(&sh, t.s);
            if left > 30.
                && ((hx - self.camera.x).abs() > 1400. || (hy - self.camera.y).abs() > 900.)
            {
                return; // Zug ist weg
            }
        }
        self.player_train = Some(t);
    }

    /// Führerstand verlassen (E): Tunnel/Hochbahn nur am Bahnsteig, sonst neben dem Zug.
    pub fn leave_train(&mut self) -> bool {
        let Some(st) = self.vehicle_state(&Ref::PlayerTrain) else {
            return false;
        };
        let Some(t) = self.player_train.clone() else {
            return false;
        };
        let first = st.cars[0];
        if st.underground || self.elevated(&st, &first) {
            let Some((i, _)) = t.at_stop.filter(|_| t.drive.v <= 0.) else {
                self.notice(if st.underground {
                    "Nur am Bahnsteig"
                } else {
                    "Aussteigen nur am Bahnhof"
                });
                return false;
            };
            let (x, y) = self.station_exit(t.pid, i);
            self.player.ride = None;
            (self.player.x, self.player.y) = (x, y);
            self.player.level.lvl = 0;
        } else {
            let Some((x, y)) = self.alight_spot(&st, 0) else {
                self.notice("Kein Platz zum Aussteigen");
                return false;
            };
            self.player.ride = None;
            (self.player.x, self.player.y) = (x, y);
            if t.v > HOP_OFF {
                self.player.stun = STUN;
            }
        }
        if let Some(pt) = self.player_train.as_mut() {
            pt.left_t = Some(0.);
        }
        let (x, y) = (self.player.x, self.player.y);
        self.events.push(Event::Alight {
            hop: t.v > HOP_OFF,
            x,
            y,
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drive_accelerates_brakes_and_respects_limits() {
        let mut d = Drive::new(Mode::UBahn, 0.);
        let full = DriveInput {
            throttle: 1.,
            limit: f64::INFINITY,
            adhesion: 1.,
            ..Default::default()
        };
        for _ in 0..60 {
            step_drive(&mut d, &full, 1. / 60.);
        }
        assert!((d.v - 11.).abs() < 0.01, "Anfahren 11 px/s² ({})", d.v);
        for _ in 0..60 * 60 {
            step_drive(&mut d, &full, 1. / 60.);
        }
        assert!(
            (d.v - 70. / 0.36).abs() < 2. && d.v <= 70. / 0.36,
            "Höchsttempo (asymptotisch)"
        );
        // Zwangsbremsung: steht vor dem Hindernis
        let mut s = 0.;
        let lim = 800.;
        for _ in 0..60 * 30 {
            let inp = DriveInput {
                limit: lim - s,
                ..full
            };
            step_drive(&mut d, &inp, 1. / 60.);
            s += d.v / 60.;
        }
        assert!(d.v == 0. && s <= lim + 1., "steht davor: {s}");
        // Türen offen: steht
        d.doors = Doors::Open;
        step_drive(&mut d, &full, 1.);
        assert_eq!(d.v, 0.);
        // Nässe: weniger Zugkraft
        let mut wet = Drive::new(Mode::Tram, 0.);
        step_drive(
            &mut wet,
            &DriveInput {
                adhesion: 0.5,
                ..full
            },
            1.,
        );
        assert!((wet.v - 6.5).abs() < 1e-9);
    }
    #[test]
    fn tips_reward_exact_gentle_stops() {
        assert_eq!(tip_for(10., 5.), TIP_MAX);
        assert_eq!(tip_for(250., 5.), 0.);
        assert!(tip_for(140., 5.) > 0. && tip_for(140., 5.) < TIP_MAX);
        assert_eq!(tip_for(0., 26.), 0., "zu hart gebremst");
    }
}
