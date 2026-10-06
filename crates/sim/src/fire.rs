//! Brennende Wracks und Fahrzeug-Explosionen.
//!
//! Wird ein Fahrzeug zum Wrack (Unfall, Beschuss, Überschlag), fängt es Feuer und explodiert nach `BURN_S` (plus
//! einer festen Streuung je Fahrzeug, aus der Nummer – kein Welt-Zufall). Die Druckwelle verletzt Menschen und
//! Spieler, beschädigt und schiebt andere Fahrzeuge (Kettenreaktionen) und scheucht die Umgebung auf. Wer im
//! brennenden Fahrzeug sitzt, wird gewarnt; explodiert es mit ihm, wird er hinausgeschleudert und verletzt.
//! Fahrräder und E-Roller brennen nicht.

use crate::car::{Car, Driver};
use crate::events::Event;
use crate::pedestrians::PedState;
use crate::world::{Notice, World};

/// Sekunden vom Wrack bis zur Explosion (dazu bis zu `BURN_SPREAD`)
pub const BURN_S: f64 = 4.;
pub const BURN_SPREAD: f64 = 1.5;
/// Druckwellen-Radius eines Pkw (px; 10 px = 1 m), größere Fahrzeuge mehr
pub const BLAST_R: f64 = 80.;
/// Schaden in der Mitte: Menschen, Spieler, Fahrzeuge (nach außen linear weniger)
pub const BLAST_PED: f64 = 160.;
pub const BLAST_PLAYER: f64 = 70.;
pub const BLAST_CAR: f64 = 90.;
/// Stoß auf Fahrzeuge in der Mitte (px/s)
pub const BLAST_PUSH: f64 = 260.;
/// so weit flüchten Passanten vor dem Knall
pub const SCARE_R: f64 = 400.;

/// Brennt dieses Fahrzeug, wenn es zum Wrack wird?
pub fn can_burn(c: &Car) -> bool {
    !c.kind_info().bike
}

/// Stärke der Explosion nach Größe (Pkw ≈ 1, Lkw und Bus mehr, Motorrad weniger)
pub fn strength(c: &Car) -> f64 {
    (c.hw / 22.).clamp(0.6, 1.6)
}

/// Brennzeit eines Wracks (fest je Fahrzeugnummer).
pub fn burn_time(id: u32) -> f64 {
    BURN_S + crate::math::hash01(id as f64 * 7.31 + 0.5) * BURN_SPREAD
}

impl World {
    /// Ein Schritt: neue Wracks entzünden, Brennzeit herunterzählen, fällige explodieren lassen.
    pub(crate) fn update_fires(&mut self, dt: f64) {
        if !self.explosions {
            return;
        }
        let mut due = Vec::new();
        for c in &mut self.cars {
            if !c.wrecked || c.exploded || !can_burn(c) {
                continue;
            }
            match c.burn.as_mut() {
                None => {
                    c.burn = Some(burn_time(c.id));
                    self.events.push(Event::CarFire {
                        x: c.x,
                        y: c.y,
                        car: c.id,
                    });
                }
                Some(t) => {
                    *t -= dt;
                    if *t <= 0. {
                        due.push(c.id);
                    }
                }
            }
        }
        // Warnung, wer in einem brennenden Wagen sitzt
        self.fire_warning();
        self.with_p2(|w| w.fire_warning());
        for id in due {
            self.explode(id);
        }
    }

    /// Befehlszeile `sprengen`: das nächste heile Fahrzeug (nicht das eigene) um (x, y) wird zum brennenden Wrack
    /// und explodiert nach `fuse` Sekunden. `None`, wenn keins im Umkreis `r` steht.
    pub fn ignite_nearest(&mut self, x: f64, y: f64, r: f64, fuse: f64) -> Option<u32> {
        // nie das eigene Auto (gefahren oder geparkt) und nicht das von Spieler 2
        let own = [
            self.player.in_car,
            self.player_car_id,
            self.p2.as_ref().and_then(|s| s.player.in_car),
        ];
        let i = self
            .cars
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.wrecked && !own.contains(&Some(c.id)) && can_burn(c))
            .map(|(i, c)| (i, (c.x - x).hypot(c.y - y)))
            .filter(|(_, d)| *d < r)
            .min_by(|a, b| a.1.total_cmp(&b.1))?
            .0;
        let c = &mut self.cars[i];
        c.health = 0.;
        c.wrecked = true;
        c.burn = Some(fuse);
        let (cx, cy, id) = (c.x, c.y, c.id);
        self.events.push(Event::Wreck {
            x: cx,
            y: cy,
            car: id,
            player: true,
        });
        self.events.push(Event::CarFire {
            x: cx,
            y: cy,
            car: id,
        });
        Some(id)
    }

    fn fire_warning(&mut self) {
        let burning = self
            .player
            .in_car
            .and_then(|id| self.car(id))
            .is_some_and(|c| c.burn.is_some() && !c.exploded);
        if burning && self.notice.as_ref().is_none_or(|n| n.text != FIRE_TEXT) {
            self.notice = Some(Notice {
                text: FIRE_TEXT.into(),
                t: 1.,
            });
        }
    }

    /// Explosion des Fahrzeugs `id`: Druckwelle anwenden, Ereignis melden.
    pub fn explode(&mut self, id: u32) {
        let Some(i) = self.cars.iter().position(|c| c.id == id) else {
            return;
        };
        let k = strength(&self.cars[i]);
        let c = &mut self.cars[i];
        c.burn = None;
        c.exploded = true;
        c.driver = c.driver.filter(|d| *d == Driver::Player);
        c.ai = None;
        // das Wrack hüpft
        c.ang_vel += (crate::math::hash01(id as f64 * 3.3) - 0.5) * 2.;
        let (x, y, lvl) = (c.x, c.y, c.lvl());
        let r = BLAST_R * k;
        self.events.push(Event::Explosion {
            x,
            y,
            car: id,
            strength: (k / 1.6).clamp(0., 1.),
        });
        // Menschen
        for q in 0..self.peds.len() {
            let p = &self.peds[q];
            if p.state == PedState::Dead || p.level.lvl != lvl {
                continue;
            }
            let d = (p.x - x).hypot(p.y - y);
            if d < r {
                let dmg = BLAST_PED * (1. - d / r);
                crate::combat::hurt_ped(self, q, dmg, (x, y), false, "explosion", false);
            }
        }
        for p in &mut self.peds {
            if p.state != PedState::Dead && (p.x - x).hypot(p.y - y) < SCARE_R {
                crate::pedestrians::scare(p, (x, y), 4.);
            }
        }
        // andere Fahrzeuge: Schaden und Stoß (wer dabei zum Wrack wird, brennt und explodiert später selbst)
        for j in 0..self.cars.len() {
            let o = &mut self.cars[j];
            if o.id == id || o.lvl() != lvl {
                continue;
            }
            let (dx, dy) = (o.x - x, o.y - y);
            let d = dx.hypot(dy);
            let reach = r * 1.3 + o.hw;
            if d >= reach {
                continue;
            }
            let f = 1. - d / reach;
            let (ux, uy) = if d > 1. { (dx / d, dy / d) } else { (1., 0.) };
            o.vx += ux * BLAST_PUSH * f;
            o.vy += uy * BLAST_PUSH * f;
            o.ang_vel += (crate::math::hash01((id + o.id) as f64) - 0.5) * 3. * f;
            if !o.wrecked {
                o.health = (o.health - BLAST_CAR * f).max(0.);
                if o.health <= 0. {
                    o.wrecked = true;
                    let (ox, oy, oid) = (o.x, o.y, o.id);
                    self.events.push(Event::Wreck {
                        x: ox,
                        y: oy,
                        car: oid,
                        player: false,
                    });
                }
            }
        }
        // Spieler (beide Sitze)
        self.blast_player(x, y, id, r);
        self.with_p2(|w| w.blast_player(x, y, id, r));
    }

    fn blast_player(&mut self, x: f64, y: f64, car: u32, r: f64) {
        if self.player.in_car == Some(car) {
            // im explodierenden Wagen: hinausgeschleudert (neben die Tür), schwer verletzt
            self.player.in_car = None;
            if let Some(c) = self.cars.iter_mut().find(|c| c.id == car) {
                c.driver = None;
                let a = c.angle + std::f64::consts::FRAC_PI_2;
                self.player.x = c.x + a.cos() * (c.hh + 14.);
                self.player.y = c.y + a.sin() * (c.hh + 14.);
            }
            self.player.stun = 1.2;
            crate::combat::hurt_player(self, BLAST_PLAYER * 1.2, (x, y));
            return;
        }
        if self.player.in_car.is_some() || self.player.ride.is_some() {
            return; // im eigenen Fahrzeug: das bekommt den Schaden (oben)
        }
        let d = (self.player.x - x).hypot(self.player.y - y);
        if d < r {
            self.player.stun = self.player.stun.max(0.6);
            crate::combat::hurt_player(self, BLAST_PLAYER * (1. - d / r), (x, y));
        }
    }
}

const FIRE_TEXT: &str = "Das Auto brennt – raus hier!";
