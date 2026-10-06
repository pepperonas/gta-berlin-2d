//! Lokaler Koop: ein zweiter Spieler in derselben Welt.
//!
//! Alles, was einem Spieler gehört (Figur, Auto, Bahn, Kamera, Hinweise, Fahrhilfen), liegt in einem [`Seat`]. Die
//! Welt führt Spieler 1 wie bisher in ihren eigenen Feldern; Spieler 2 sitzt in `World::p2`. Für seine Schritte
//! tauscht die Welt die beiden Sitze (`swap_seat`), lässt den gewohnten Spielercode laufen und tauscht zurück – so
//! kann Spieler 2 ohne eigenen Code laufen, springen, fahren, Bahn fahren und kämpfen. Ohne Spieler 2 ändert sich
//! nichts am Ablauf (Einzelspieler bleibt bitgleich).

use crate::station::Station;
use crate::world::{Camera, Notice, Player, World};

/// Fokus-Schlüssel der Kacheln um Spieler 2.
pub const P2_FOCUS: &str = "p2";
/// Ab diesem Abstand der beiden Kameras (px) gilt die Stadt um beide als belebt: die Zielbevölkerung wächst.
pub const APART: f64 = 2500.;
/// Faktor auf die Zielbevölkerung, solange die Spieler getrennt sind (zwei Bildausschnitte wollen gefüllt sein).
pub const APART_FACTOR: f64 = 1.6;

/// Alles, was einem Spieler gehört.
pub struct Seat {
    pub player: Player,
    pub player_car_id: Option<u32>,
    pub player_train: Option<crate::ride::PlayerTrain>,
    pub camera: Camera,
    pub foot_zoom: f64,
    pub esp: bool,
    pub esp_full: bool,
    pub abs: bool,
    pub truck_limiter: bool,
    pub underground: f64,
    pub st_near: Vec<Station>,
    pub st_tick: u32,
    pub st_gen: u64,
    pub notice: Option<Notice>,
    pub veh_info: Option<(u32, f64)>,
    pub focus_key: String,
}

impl World {
    /// Sitz von Spieler 2 mit den Feldern von Spieler 1 tauschen (zweimal = wie vorher). Ohne Spieler 2 nichts.
    pub fn swap_seat(&mut self) {
        let Some(mut s) = self.p2.take() else { return };
        std::mem::swap(&mut self.player, &mut s.player);
        std::mem::swap(&mut self.player_car_id, &mut s.player_car_id);
        std::mem::swap(&mut self.player_train, &mut s.player_train);
        std::mem::swap(&mut self.camera, &mut s.camera);
        std::mem::swap(&mut self.foot_zoom, &mut s.foot_zoom);
        std::mem::swap(&mut self.esp, &mut s.esp);
        std::mem::swap(&mut self.esp_full, &mut s.esp_full);
        std::mem::swap(&mut self.abs, &mut s.abs);
        std::mem::swap(&mut self.truck_limiter, &mut s.truck_limiter);
        std::mem::swap(&mut self.underground, &mut s.underground);
        std::mem::swap(&mut self.st_near, &mut s.st_near);
        std::mem::swap(&mut self.st_tick, &mut s.st_tick);
        std::mem::swap(&mut self.st_gen, &mut s.st_gen);
        std::mem::swap(&mut self.notice, &mut s.notice);
        std::mem::swap(&mut self.veh_info, &mut s.veh_info);
        std::mem::swap(&mut self.focus_key, &mut s.focus_key);
        self.p2 = Some(s);
    }

    /// `f` mit Spieler 2 auf dem Platz von Spieler 1 ausführen; `None` ohne Spieler 2.
    pub fn with_p2<R>(&mut self, f: impl FnOnce(&mut World) -> R) -> Option<R> {
        self.p2.as_ref()?;
        self.swap_seat();
        let r = f(self);
        self.swap_seat();
        Some(r)
    }

    /// Welcher Sitz gerade auf dem Platz von Spieler 1 sitzt: 0 = Spieler 1, 1 = Spieler 2 (während `with_p2`).
    pub fn seat_index(&self) -> u8 {
        u8::from(self.focus_key == P2_FOCUS)
    }

    /// Ist Spieler 2 dabei?
    pub fn coop(&self) -> bool {
        self.p2.is_some()
    }

    /// Spieler 2 neben Spieler 1 zu Fuß aufstellen. `false`, wenn er schon da ist oder kein Platz frei war.
    pub fn join_p2(&mut self) -> bool {
        if self.p2.is_some() {
            return false;
        }
        let (x, y, lvl) = (self.player.x, self.player.y, self.player.level.lvl);
        // im Auto: neben der Tür, sonst ein Stück neben der Figur; der erste freie Platz im Kreis
        let r0 = if self.player.in_car.is_some() {
            40.
        } else {
            22.
        };
        let mut spot = None;
        'ring: for ring in 0..6 {
            let r = r0 + ring as f64 * 14.;
            for k in 0..12 {
                let a = std::f64::consts::TAU * k as f64 / 12. + ring as f64 * 0.3;
                let (sx, sy) = (x + a.cos() * r, y + a.sin() * r);
                if self.spot_free_here(sx, sy, crate::world::PLAYER_RADIUS + 2., lvl) {
                    spot = Some((sx, sy));
                    break 'ring;
                }
            }
        }
        let Some((sx, sy)) = spot else { return false };
        let mut player = Player::fresh();
        (player.x, player.y, player.angle) = (sx, sy, self.player.angle);
        player.level = self.player.level;
        player.level_init = self.player.level_init;
        self.p2 = Some(Box::new(Seat {
            player,
            player_car_id: None,
            player_train: None,
            camera: Camera {
                x: sx,
                y: sy,
                zoom: self.camera.zoom,
            },
            foot_zoom: self.foot_zoom,
            esp: self.esp,
            esp_full: self.esp_full,
            abs: self.abs,
            truck_limiter: self.truck_limiter,
            underground: 0.,
            st_near: Vec::new(),
            st_tick: 0,
            st_gen: u64::MAX,
            notice: Some(Notice {
                text: "Spieler 2 ist dabei".into(),
                t: 2.5,
            }),
            veh_info: None,
            focus_key: P2_FOCUS.into(),
        }));
        true
    }

    /// Spieler 2 verlässt das Spiel: sein Fahrzeug bleibt stehen, seine Kacheln werden freigegeben.
    pub fn leave_p2(&mut self) {
        let Some(s) = self.p2.take() else { return };
        if let Some(c) = s
            .player
            .in_car
            .and_then(|id| self.cars.iter_mut().find(|c| c.id == id))
        {
            c.driver = None;
            c.controls = Default::default();
        }
        self.city.release(&s.focus_key);
    }

    /// Kameramitten, um die die Stadt lebt (Spieler 1, dann Spieler 2), und deren Anzahl.
    pub fn foci(&self) -> ([(f64, f64); 2], usize) {
        let a = (self.camera.x, self.camera.y);
        match &self.p2 {
            Some(s) => ([a, (s.camera.x, s.camera.y)], 2),
            None => ([a, a], 1),
        }
    }

    /// Abstand zur nächsten Kamera (Einzelspieler: zur Kamera).
    pub fn focus_dist(&self, x: f64, y: f64) -> f64 {
        let (f, n) = self.foci();
        let mut d = (x - f[0].0).hypot(y - f[0].1);
        if n == 2 {
            d = d.min((x - f[1].0).hypot(y - f[1].1));
        }
        d
    }

    /// Liegt (x, y) im Rechteck ±(hx, hy) um irgendeine Kamera (Rand eingeschlossen)?
    pub fn in_view_any(&self, x: f64, y: f64, hx: f64, hy: f64) -> bool {
        let (f, n) = self.foci();
        f[..n]
            .iter()
            .any(|&(cx, cy)| (x - cx).abs() <= hx && (y - cy).abs() <= hy)
    }

    /// Sind die Spieler so weit auseinander, dass zwei Ausschnitte belebt werden müssen?
    pub fn apart(&self) -> bool {
        let (f, n) = self.foci();
        n == 2 && (f[0].0 - f[1].0).hypot(f[0].1 - f[1].1) > APART
    }

    /// Fährt ein Spieler dieses Auto? 0 = Spieler 1, 1 = Spieler 2.
    pub fn seat_of_car(&self, id: u32) -> Option<usize> {
        if self.player.in_car == Some(id) {
            Some(0)
        } else if self
            .p2
            .as_ref()
            .is_some_and(|s| s.player.in_car == Some(id))
        {
            Some(1)
        } else {
            None
        }
    }

    /// Hinweis an den Spieler, der dieses Auto fährt (sonst an Spieler 1).
    pub fn notify_car(&mut self, car: u32, text: &str, t: f64) {
        let n = Some(Notice {
            text: text.into(),
            t,
        });
        match (self.seat_of_car(car), self.p2.as_mut()) {
            (Some(1), Some(s)) => s.notice = n,
            _ => self.notice = n,
        }
    }

    /// Alle Spieler zu Fuß im Freien (Lage, Ebene), für Ausweichen und Bremsen der KI.
    pub fn players_on_foot(&self) -> ([(f64, f64, i8); 2], usize) {
        let mut out = [(0., 0., 0); 2];
        let mut n = 0;
        let mut add = |p: &Player| {
            if p.in_car.is_none() {
                out[n] = (p.x, p.y, p.level.lvl);
                n += 1;
            }
        };
        add(&self.player);
        if let Some(s) = &self.p2 {
            add(&s.player);
        }
        (out, n)
    }
}

/// Abstand zum nächsten Punkt (bei einem Punkt genau dessen Abstand – Einzelspiel rechnet wie vorher).
pub fn min_dist(foci: &[(f64, f64)], x: f64, y: f64) -> f64 {
    let mut d = f64::INFINITY;
    for &(cx, cy) in foci {
        d = d.min((x - cx).hypot(y - cy));
    }
    d
}
