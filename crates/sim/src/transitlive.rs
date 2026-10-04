//! Nahverkehr in der Welt (Port von `transitlive.js`): Fahrplan-Fahrzeuge fortschreiben (`transit.rs`),
//! Straßenbahnen halten vor Hindernissen und klingeln, Busse werden nahe der Kamera zu echten KI-Fahrzeugen, die
//! ihre Halte der Reihe nach anfahren (Wartende steigen ein). Straßenbahnwagen sind für Verkehr und Spieler feste,
//! fahrende Hindernisse. S- und U-Bahn werden nur gezeichnet, wo ihr Gleis oberirdisch liegt. Nur in der Welt mit
//! Tagesrhythmus und wenn Fahrplandaten geladen sind (`World::set_transit`).
use crate::car::{Car, Driver, Role, damage};
use crate::collision::{Obb, Rect, circle_vs_obb, obb_vs_obb};
use crate::events::Event;
use crate::life::{VIEW_HALF_X, VIEW_HALF_Y};
use crate::pedestrians::PedState;
use crate::traffic::{Follow, RailObs, drop_claims, project_near};
use crate::transit::{
    BUS_COLOR, BUS_LIVE, CarPos, Mode, Pattern, Transit, Veh, point_on_shape, position_at,
    speed_at, train_cars,
};
use crate::world::World;
use std::sync::Arc;

/// s: so lange wartet eine Straßenbahn auf Gegenverkehr, bevor sie ihn beiseiteschiebt
pub const TRAM_PATIENCE: f64 = 20.;

/// Bus im Linienbetrieb.
#[derive(Debug, Clone, PartialEq)]
pub struct BusDuty {
    pub pid: usize,
    /// nächster Halt (Index)
    pub stop: usize,
    pub veh: String,
    /// Lage auf dem Linienweg
    pub s: f64,
    /// s neben dem Weg
    pub off: f64,
    pub boarding: bool,
    pub best_s: f64,
    pub progress_t: f64,
}

/// Sichtbare Bahn: Linie, Art, Farbe, Wagen, fährt gerade.
#[derive(Debug, Clone, PartialEq)]
pub struct Visible {
    pub line: String,
    pub mode: Mode,
    pub color: String,
    pub cars: Vec<CarPos>,
    /// Ebene je Wagen (Gleis darunter; Straßenbahn 0)
    pub lvl: Vec<i8>,
    pub lit: bool,
}

fn out_of_view(w: &World, x: f64, y: f64, pad: f64) -> bool {
    (x - w.camera.x).abs() > VIEW_HALF_X + pad || (y - w.camera.y).abs() > VIEW_HALF_Y + pad
}

impl World {
    /// Fahrplandaten an die Welt hängen (sonst gibt es keinen Nahverkehr).
    pub fn set_transit(&mut self, t: Transit) {
        self.transit = Some(Box::new(t));
        self.transit_state = crate::transit::State {
            seed: self.seed,
            ..Default::default()
        };
        self.transit_populated = false;
    }

    /// Steht an (x, y) etwas im Weg einer Straßenbahn? `head_on`: Fahrtrichtung der Bahn – frontal entgegenkommende
    /// Fahrzeuge zählen dann nicht (Rückfall nach langem Warten).
    pub fn rail_obstacle_at(&self, x: f64, y: f64, head_on: Option<f64>) -> bool {
        let hit = |ox: f64, oy: f64, r: f64| (ox - x).hypot(oy - y) < r;
        let p = &self.player;
        if p.in_car.is_none() && p.ride.is_none() && !p.combat.dead && hit(p.x, p.y, 22.) {
            return true;
        }
        for c in &self.cars {
            if hit(c.x, c.y, 24. + c.hw * 0.4)
                && !head_on.is_some_and(|h| (c.angle - h).cos() < -0.5)
            {
                return true;
            }
        }
        for b in &self.bikes {
            if b.state == crate::bikes::State::Ride && hit(b.x, b.y, 18.) {
                return true;
            }
        }
        self.peds
            .iter()
            .any(|q| !matches!(q.state, PedState::Dead | PedState::Hang) && hit(q.x, q.y, 16.))
    }

    /// Straßenbahn vor einem Hindernis auf dem Gleis? (Spitze plus 2–8 m voraus)
    fn tram_blocked(&self, tr: &Transit, p: &Pattern, v: &Veh, strict: bool) -> bool {
        let pos = position_at(p, v.tau);
        if pos.dwelling {
            return false;
        }
        let sh = tr.shape_of(p);
        let (hx, hy, ha) = point_on_shape(sh, pos.s);
        if (hx - self.camera.x).abs() > 2500. || (hy - self.camera.y).abs() > 2500. {
            return false;
        }
        let patient = (!strict && v.blocked_t > TRAM_PATIENCE).then_some(ha);
        [20., 45., 75.].iter().any(|d| {
            let (qx, qy, _) = point_on_shape(sh, pos.s + d);
            self.rail_obstacle_at(qx, qy, patient)
        })
    }

    /// Bus als KI-Fahrzeug auf die Spur setzen, die dicht am Linienweg liegt und in seine Richtung zeigt.
    fn materialize_bus(&mut self, tr: &Transit, pid: usize, v: &mut Veh) -> Option<u32> {
        let p = &tr.patterns[pid];
        let sh = tr.shape_of(p);
        let pos = position_at(p, v.tau);
        let (px, py, pa) = point_on_shape(sh, pos.s);
        let hit = self.lanes.nearest_lane(px, py, Some(pa), 250., true)?;
        if (hit.x - px).hypot(hit.y - py) > 60. {
            return None;
        }
        let lane = self.lanes.lane(hit.lane)?;
        let li = hit.i.min(lane.pts.len().saturating_sub(2));
        let (a, b) = (lane.pts[li], lane.pts[(li + 1).min(lane.pts.len() - 1)]);
        let (ldx, ldy) = (b.0 - a.0, b.1 - a.1);
        let ll = ldx.hypot(ldy).max(1e-9);
        if (ldx * pa.cos() + ldy * pa.sin()) / ll < 0.8 {
            return None;
        }
        if self
            .cars
            .iter()
            .any(|c| (c.x - hit.x).hypot(c.y - hit.y) < 130.)
        {
            return None;
        }
        let mut s = 0.;
        for w in lane.pts[..=hit.i.min(lane.pts.len() - 1)].windows(2) {
            s += (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
        }
        let start = lane.pts[hit.i.min(lane.pts.len() - 1)];
        s += (hit.x - start.0).hypot(hit.y - start.1);
        let id = self.put_bus(hit.lane, s, hit.x, hit.y)?;
        let car = self.cars.iter_mut().find(|c| c.id == id)?;
        if let Some(ai) = car.ai.as_mut() {
            ai.follow = Some(Follow {
                pts: Arc::new(sh.pts.clone()),
                cum: Arc::new(sh.cum.clone()),
                s: pos.s,
            });
        }
        car.line = Some(p.name.clone());
        car.bus = Some(Box::new(BusDuty {
            pid,
            stop: pos.stop.max(1),
            veh: v.key.clone(),
            s: pos.s,
            off: 0.,
            boarding: false,
            best_s: -1.,
            progress_t: self.time,
        }));
        v.live = Some(id);
        Some(id)
    }

    /// Nahverkehr fortschreiben (jeden Schritt, nach dem Verkehr).
    pub fn update_transit(&mut self, dt: f64) {
        self.rail_obs.clear();
        let Some(mut tr) = self.transit.take() else {
            return;
        };
        if !self.day_rhythm {
            self.transit = Some(tr);
            return;
        }
        let mut st = std::mem::take(&mut self.transit_state);
        let (cam, clock, day, time) = (
            (self.camera.x, self.camera.y),
            self.clock,
            self.day,
            self.time,
        );
        crate::transit::scan(&mut st, &mut tr, cam, clock, day, time);
        {
            let this = &*self;
            let t: &Transit = &tr;
            // Straßenbahnen halten vor Hindernissen (Wartezeit zählt mit)
            // Fahrplan-Züge desselben Musters hinter dem Spielerzug warten vor seinem Heck
            let pt = this.player_train.as_ref().map(|t| (t.pid, t.s));
            let mut blocked = |p: &Pattern, v: &mut Veh, dt: f64| {
                if let Some((pid, ps)) = pt
                    && pid == p.id
                {
                    let vs = position_at(p, v.tau).s;
                    if vs < ps && ps - p.mode.train_len() - vs < 600. {
                        return true;
                    }
                }
                if p.mode == Mode::Tram && this.tram_blocked(t, p, v, false) {
                    v.blocked_t += dt;
                    true
                } else {
                    false
                }
            };
            crate::transit::advance(&mut st, t, clock, day, dt, &mut blocked);
        }
        // Straßenbahnen: einmal klingeln, wenn sie warten; erst zurücksetzen, wenn wirklich frei
        let mut bells = Vec::new();
        for (&id, s) in st.tracked.iter_mut() {
            let p = &tr.patterns[id];
            if p.mode != Mode::Tram {
                continue;
            }
            for v in s.veh.iter_mut() {
                if v.blocked_t > 2.5 && !v.rang {
                    v.rang = true;
                    let (x, y, _) = point_on_shape(tr.shape_of(p), position_at(p, v.tau).s);
                    bells.push(Event::TramBell { x, y });
                }
                if v.blocked_t > 0. && !self.tram_blocked(&tr, p, v, true) {
                    v.blocked_t = 0.;
                    v.rang = false;
                }
            }
        }
        self.events.extend(bells);
        // Busse in der Nähe auf die Straße holen
        let populated = self.transit_populated;
        let ids: Vec<usize> = st.tracked.keys().copied().collect();
        for id in ids {
            if tr.patterns[id].mode != Mode::Bus {
                continue;
            }
            let n = st.tracked[&id].veh.len();
            for k in 0..n {
                let mut v = st.tracked[&id].veh[k].clone();
                if v.live.is_some() || v.gone {
                    continue;
                }
                let p = &tr.patterns[id];
                let pos = position_at(p, v.tau);
                let (qx, qy, _) = point_on_shape(tr.shape_of(p), pos.s);
                let d = (qx - cam.0).hypot(qy - cam.1);
                let last = *p.stops.last().unwrap_or(&0.);
                if d < BUS_LIVE
                    && (out_of_view(self, qx, qy, 80.) || !populated)
                    && pos.s < last - 200.
                    && self.materialize_bus(&tr, id, &mut v).is_some()
                    && let Some(t) = st.tracked.get_mut(&id)
                {
                    t.veh[k] = v;
                }
            }
        }
        self.transit_populated = true;
        for i in 0..self.cars.len() {
            if self.cars[i].bus.is_some() && !self.cars[i].done {
                self.update_bus(i, &tr, dt);
            }
        }
        // fertige oder ferne Busse abbauen (ein gekaperter Bus bleibt dem Spieler)
        let mut gone: Vec<(usize, String)> = Vec::new();
        let mut drop_ids = Vec::new();
        for c in &self.cars {
            let Some(b) = &c.bus else { continue };
            let far = (c.x - cam.0).hypot(c.y - cam.1) > 2400.;
            let done = (c.done || c.wrecked || c.driver != Some(Driver::Npc))
                && out_of_view(self, c.x, c.y, 200.);
            if far || done {
                gone.push((b.pid, b.veh.clone()));
                if c.driver == Some(Driver::Npc) {
                    drop_ids.push(c.id);
                }
            }
        }
        for (pid, key) in gone {
            if let Some(v) = st
                .tracked
                .get_mut(&pid)
                .and_then(|t| t.veh.iter_mut().find(|v| v.key == key))
            {
                v.gone = true;
            }
        }
        for id in &drop_ids {
            drop_claims(&mut self.res, *id);
        }
        self.cars.retain(|c| !drop_ids.contains(&c.id));
        for c in self.cars.iter_mut() {
            if c.bus.is_some() && c.driver != Some(Driver::Npc) {
                c.bus = None; // gekapert: kein Linienbus mehr
            }
        }
        // Straßenbahnwagen als Hindernisse (nahe der Kamera)
        for (&id, s) in &st.tracked {
            let p = &tr.patterns[id];
            if p.mode != Mode::Tram {
                continue;
            }
            for v in &s.veh {
                let pos = position_at(p, v.tau);
                let (hx, hy, _) = point_on_shape(tr.shape_of(p), pos.s);
                if (hx - cam.0).abs() > 2200. || (hy - cam.1).abs() > 2200. {
                    continue;
                }
                let moving = !pos.dwelling && v.blocked_t <= 0.;
                let spd = if moving { speed_at(p, &pos) } else { 0. };
                for c in train_cars(tr.shape_of(p), p.mode, pos.s) {
                    self.rail_obs.push(RailObs {
                        x: c.x,
                        y: c.y,
                        angle: c.angle,
                        hw: c.l / 2.,
                        hh: c.w / 2.,
                        vx: c.angle.cos() * spd,
                        vy: c.angle.sin() * spd,
                    });
                }
            }
        }
        // Spielerzug: Straßenbahn immer, S-/U-Bahn nur oberirdisch als festes, fahrendes Hindernis
        if let Some((pid, ps, pv)) = self.player_train.as_ref().map(|t| (t.pid, t.s, t.v)) {
            let p = &tr.patterns[pid];
            let sh = tr.shape_of(p);
            if p.mode == Mode::Tram || !self.ug.underground_at_s(&mut self.city, p, sh, ps) {
                for c in train_cars(sh, p.mode, ps) {
                    self.rail_obs.push(RailObs {
                        x: c.x,
                        y: c.y,
                        angle: c.angle,
                        hw: c.l / 2.,
                        hh: c.w / 2.,
                        vx: c.angle.cos() * pv,
                        vy: c.angle.sin() * pv,
                    });
                }
            }
        }
        self.transit_state = st;
        self.transit = Some(tr);
        self.collide_rail();
    }

    /// Bus: folgt dem Linienweg; erreicht er einen Halt, hält er dort (Wartende steigen ein). Nach dem letzten Halt
    /// oder wenn er den Weg verloren hat, ist er fertig und fährt außer Sicht davon.
    fn update_bus(&mut self, i: usize, tr: &Transit, dt: f64) {
        let time = self.time;
        let c = &mut self.cars[i];
        if c.driver != Some(Driver::Npc) || c.ai.is_none() {
            return;
        }
        let Some(d) = c.bus.as_mut() else { return };
        let p = &tr.patterns[d.pid];
        let sh = tr.shape_of(p);
        let (qs, qd) = project_near(&sh.pts, &sh.cum, c.x, c.y, d.s - 100., 800.);
        if qd < 150. {
            d.s = d.s.max(qs);
        }
        d.off = if qd > 150. { d.off + dt } else { 0. };
        let finish = |c: &mut Car| {
            c.done = true;
            if let Some(ai) = c.ai.as_mut() {
                ai.follow = None;
            }
        };
        if d.off > 8. {
            finish(c);
            return;
        }
        if d.boarding {
            if c.ai.as_ref().is_some_and(|a| a.hold > 0.) {
                return;
            }
            let d = c.bus.as_mut().expect("Bus");
            d.boarding = false;
            d.stop += 1;
            if d.stop >= p.stops.len() {
                finish(c);
            }
            return;
        }
        // kein Vorankommen (verwinkelte Kreuzung, Stau): nach 25 s die Linie aufgeben
        if d.s > d.best_s + 20. {
            d.best_s = d.s;
            d.progress_t = time;
        } else if time - d.progress_t > 25. {
            finish(c);
            return;
        }
        let d = c.bus.as_mut().expect("Bus");
        while d.stop < p.stops.len() && d.s > p.stops[d.stop] + 60. {
            d.stop += 1; // verpasste Halte
        }
        if d.stop >= p.stops.len() {
            finish(c);
            return;
        }
        if d.s >= p.stops[d.stop] - 45. && qd < 150. {
            d.boarding = true;
            let stop = d.stop;
            if let Some(ai) = c.ai.as_mut() {
                ai.hold = p.dwell;
            }
            let (sx, sy, _) = point_on_shape(sh, p.stops[stop]);
            let before = self.peds.len();
            self.peds.retain(|x| {
                !(x.state == PedState::Hang
                    && x.hang.as_ref().is_some_and(|h| {
                        matches!(h.act, crate::life::Act::Wait | crate::life::Act::Queue)
                    })
                    && (x.x - sx).hypot(x.y - sy) < 140.)
            });
            let n = before - self.peds.len();
            if n > 0 {
                self.events.push(Event::BusBoard { x: sx, y: sy, n });
            }
            self.events.push(Event::BusStop {
                x: sx,
                y: sy,
                line: p.name.clone(),
                stop: p.stop_names.get(stop).cloned().unwrap_or_default(),
            });
        }
    }

    /// Straßenbahnwagen sind unbeweglich: Autos und Spielfigur werden herausgeschoben, schnelle Autos nehmen Schaden.
    fn collide_rail(&mut self) {
        let obs = std::mem::take(&mut self.rail_obs);
        for o in &obs {
            let ob = Obb {
                x: o.x,
                y: o.y,
                angle: o.angle,
                hw: o.hw,
                hh: o.hh,
            };
            for c in self.cars.iter_mut() {
                if (c.x - o.x).abs() > o.hw + c.hw || (c.y - o.y).abs() > o.hw + c.hw {
                    continue;
                }
                let Some(m) = obb_vs_obb(&c.obb(), &ob) else {
                    continue;
                };
                c.x += m.nx * m.depth;
                c.y += m.ny * m.depth;
                let vn = (c.vx - o.vx) * m.nx + (c.vy - o.vy) * m.ny;
                if vn < 0. {
                    c.vx -= 1.3 * vn * m.nx;
                    c.vy -= 1.3 * vn * m.ny;
                    if -vn > 60. {
                        damage(c, -vn, &mut self.events);
                    }
                }
            }
            let pl = &mut self.player;
            if pl.in_car.is_none()
                && pl.ride.is_none()
                && let Some(m) = circle_vs_obb(pl.x, pl.y, 7., &ob)
            {
                pl.x += m.nx * m.depth;
                pl.y += m.ny * m.depth;
            }
        }
        self.rail_obs = obs;
    }

    /// Sichtbare Bahnen im Rechteck (Straßenbahnen immer, S-/U-Bahn nur, wo ihr Gleis oberirdisch liegt).
    pub fn transit_visible(&mut self, v: Rect) -> Vec<Visible> {
        let Some(tr) = self.transit.take() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let pad = 400.;
        for (&id, s) in &self.transit_state.tracked {
            let p = &tr.patterns[id];
            if p.mode == Mode::Bus || p.mode == Mode::Other {
                continue;
            }
            let sh = tr.shape_of(p);
            let len = if p.mode == Mode::Tram { 320. } else { 1100. };
            for veh in &s.veh {
                let pos = position_at(p, veh.tau);
                let (hx, hy, _) = point_on_shape(sh, pos.s);
                if hx < v.x - pad - len
                    || hx > v.x + v.w + pad + len
                    || hy < v.y - pad - len
                    || hy > v.y + v.h + pad + len
                {
                    continue;
                }
                let mut cars = Vec::new();
                let mut lvl = Vec::new();
                for c in train_cars(sh, p.mode, pos.s) {
                    if p.mode == Mode::Tram {
                        cars.push(c);
                        lvl.push(0);
                    } else if let Some(l) =
                        crate::tunnel::rail_level_at(&mut self.city, c.x, c.y, crate::tunnel::PROBE)
                    {
                        cars.push(c);
                        lvl.push(l);
                    }
                }
                if !cars.is_empty() {
                    out.push(Visible {
                        line: p.name.clone(),
                        mode: p.mode,
                        color: p.color.clone(),
                        cars,
                        lvl,
                        lit: !pos.dwelling,
                    });
                }
            }
        }
        // vom Spieler geführter Zug: Straßenbahn immer, S-/U-Bahn wo das Gleis oben liegt
        if let Some((pid, ps, pv)) = self.player_train.as_ref().map(|t| (t.pid, t.s, t.v)) {
            let p = &tr.patterns[pid];
            let mut cars = Vec::new();
            let mut lvl = Vec::new();
            for c in train_cars(tr.shape_of(p), p.mode, ps) {
                if p.mode == Mode::Tram {
                    cars.push(c);
                    lvl.push(0);
                } else if let Some(l) =
                    crate::tunnel::rail_level_at(&mut self.city, c.x, c.y, crate::tunnel::PROBE)
                {
                    cars.push(c);
                    lvl.push(l);
                }
            }
            if !cars.is_empty() {
                out.push(Visible {
                    line: p.name.clone(),
                    mode: p.mode,
                    color: p.color.clone(),
                    cars,
                    lvl,
                    lit: pv > 0.,
                });
            }
        }
        self.transit = Some(tr);
        out
    }

    /// Straßenbahngleise im Rechteck (Abschnitte der Linienwege).
    pub fn tram_track_segments(&mut self, v: Rect) -> Vec<(crate::city::Pt, crate::city::Pt)> {
        self.transit
            .as_mut()
            .map(|t| t.tram_segments(&v))
            .unwrap_or_default()
    }

    /// Bus auf einer Spur abstellen (Linienbus, gelb).
    fn put_bus(&mut self, lane: crate::roadgraph::LaneId, s: f64, x: f64, y: f64) -> Option<u32> {
        let id = self.put_npc_car(lane, s, x, y, "bus")?;
        if let Some(c) = self.cars.iter_mut().find(|c| c.id == id) {
            c.color = BUS_COLOR;
            c.role = Role::Traffic;
        }
        Some(id)
    }
}
