//! Waffenrad (Port von `weaponwheel.js`): eine Taste, die kurz getippt etwas anderes tut als gehalten.
//!  – Maus: beide Tasten zusammen halten (zu Fuß) = Rad genau am Zeiger; gewählt ist das Feld in Richtung des Zeigers
//!    ab der Radmitte. Beide Tasten loslassen nimmt die Waffe, kurzes Doppeltippen tut nichts.
//!  – Controller: rechten Stick drücken öffnet das Rad, der Stick wählt, nochmal drücken oder A nimmt, B bricht ab.
//! Solange das Rad offen ist, läuft das Spiel in Zeitlupe.
use berlin_engine::hud::{Align, Hud};
use berlin_sim::combat::{Combat, WEAPONS};
use glam::Vec2;

/// s bis das Rad aufgeht (kürzer = Tippen)
pub const HOLD: f64 = 0.22;
/// Totzone um die Mitte (HUD-Einheiten)
pub const DEAD: f32 = 14.;
/// Spieltempo bei offenem Rad
pub const SLOW: f64 = 0.3;
pub const RADIUS: f32 = 190.;
pub const INNER: f32 = 72.;
/// Controller: Stickausschlag, ab dem gewählt wird
pub const STICK_DEAD: f32 = 0.45;
/// s: Zeitlupe und Einblenden
pub const EASE: f64 = 0.12;

/// Segment zur Richtung (dx, dy) bei n Feldern: 0 oben, im Uhrzeigersinn; `None` in der Totzone.
pub fn slot(dx: f32, dy: f32, n: usize, dead: f32) -> Option<usize> {
    if n == 0 || dx.hypot(dy) < dead {
        return None;
    }
    let tau = std::f32::consts::TAU;
    let a = (dx.atan2(-dy) + tau) % tau;
    let step = tau / n as f32;
    Some(((a + step / 2.) / step).floor() as usize % n)
}
/// Mitte eines Segments → Einheitsvektor im Bild (x rechts, y unten).
pub fn slot_dir(i: usize, n: usize) -> Vec2 {
    let a = i as f32 * std::f32::consts::TAU / n as f32;
    Vec2::new(a.sin(), -a.cos())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Outcome {
    /// kurz getippt
    pub tap: bool,
    pub pick: Option<usize>,
    pub opened: bool,
    pub closed: bool,
}

/// Zustand einer Rad-Taste.
#[derive(Debug, Clone, Default)]
pub struct WheelButton {
    pub down: bool,
    pub open: bool,
    pub hover: Option<usize>,
    t0: f64,
    pub opened_at: f64,
    /// Mitte des Rads und Zeigerversatz (HUD-Einheiten)
    pub center: Vec2,
    pub v: Vec2,
    last: Vec2,
    n: usize,
}
impl WheelButton {
    pub fn press(&mut self, t: f64, at: Vec2) {
        *self = Self {
            down: true,
            t0: t,
            last: at,
            center: at,
            ..Self::default()
        };
    }
    /// Jeden Schritt: `can_open` = zu Fuß im Spiel; `current` = gewählte Waffe.
    pub fn tick(&mut self, t: f64, can_open: bool, current: usize, n: usize) -> Outcome {
        if self.open && !can_open {
            self.open = false;
            self.down = false;
            return Outcome {
                closed: true,
                ..Default::default()
            };
        }
        if self.down && !self.open && can_open && t - self.t0 >= HOLD - 1e-9 {
            self.open = true;
            self.hover = Some(current);
            self.n = n;
            self.v = Vec2::ZERO;
            self.opened_at = t;
            self.center = self.last;
            return Outcome {
                opened: true,
                ..Default::default()
            };
        }
        Outcome::default()
    }
    /// Mitte festlegen (am Zeiger, im Bild gehalten).
    pub fn place(&mut self, c: Vec2) {
        if self.open {
            self.center = c;
            self.mov(self.last);
        }
    }
    /// Maus: echter Zeiger (HUD-Einheiten).
    pub fn mov(&mut self, p: Vec2) {
        self.last = p;
        if !self.open {
            return;
        }
        self.v = p - self.center;
        if let Some(i) = slot(self.v.x, self.v.y, self.n, DEAD) {
            self.hover = Some(i);
        }
    }
    /// Controller: Stick −1…1; losgelassen behält die Wahl.
    pub fn aim(&mut self, x: f32, y: f32) {
        if !self.open || x.hypot(y) < STICK_DEAD {
            return;
        }
        self.v = Vec2::new(x, y) * RADIUS;
        if let Some(i) = slot(x, y, self.n, 0.) {
            self.hover = Some(i);
        }
    }
    /// Zifferntaste: wählen und schließen.
    pub fn choose(&mut self, i: usize) -> Outcome {
        if !self.open || i >= self.n {
            return Outcome::default();
        }
        self.open = false;
        self.down = false;
        Outcome {
            pick: Some(i),
            closed: true,
            ..Default::default()
        }
    }
    /// Controller (Stickdruck): sofort öffnen, die aktuelle Wahl vorgewählt.
    pub fn open_now(&mut self, t: f64, at: Vec2, current: usize, n: usize) {
        *self = Self {
            open: true,
            hover: Some(current),
            opened_at: t,
            center: at,
            last: at,
            n,
            ..Self::default()
        };
    }
    /// Controller: schließen und das gezeigte Feld nehmen.
    pub fn close_pick(&mut self) -> Outcome {
        if !self.open {
            return Outcome::default();
        }
        self.open = false;
        self.down = false;
        Outcome {
            pick: self.hover,
            closed: true,
            ..Default::default()
        }
    }
    /// Schließen ohne Wahl.
    pub fn cancel(&mut self) -> Outcome {
        let was = self.open;
        self.open = false;
        self.down = false;
        Outcome {
            closed: was,
            ..Default::default()
        }
    }
    /// Controller-Rad an einer Taste: drücken öffnet bzw. nimmt, A nimmt, B bricht ab, der Stick wählt.
    #[allow(clippy::too_many_arguments)]
    pub fn pad_step(
        &mut self,
        t: f64,
        at: Vec2,
        toggle: bool,
        confirm: bool,
        back: bool,
        stick: (f32, f32),
        can_open: bool,
        current: usize,
        n: usize,
    ) -> Outcome {
        if self.open && !can_open {
            return self.cancel();
        }
        if toggle && can_open && !self.open {
            self.open_now(t, at, current, n);
            return Outcome {
                opened: true,
                ..Default::default()
            };
        }
        if !self.open {
            return Outcome::default();
        }
        self.aim(stick.0, stick.1);
        if back {
            return self.cancel();
        }
        if toggle || confirm {
            return self.close_pick();
        }
        Outcome::default()
    }
    pub fn release(&mut self, t: f64) -> Outcome {
        if !self.down {
            return Outcome::default();
        }
        self.down = false;
        if self.open {
            self.open = false;
            return Outcome {
                pick: self.hover,
                closed: true,
                ..Default::default()
            };
        }
        Outcome {
            tap: t - self.t0 < HOLD,
            ..Default::default()
        }
    }
}

/// Zeitlupe weich ein- und ausblenden.
pub fn ease_time_scale(cur: f64, open: bool, dt: f64) -> f64 {
    let target = if open { SLOW } else { 1. };
    let next = cur + (target - cur) * (dt / EASE).min(1.);
    if (next - target).abs() < 0.005 {
        target
    } else {
        next
    }
}

/// Rad zeichnen (hud.js drawWeaponWheel): Segmente mit Namen und Munition, gezeigtes gelb, Mitte mit Name/Magazin.
pub fn draw(h: &mut Hud, w: &WheelButton, c: &Combat, age: f64, pad: bool) {
    let n = WEAPONS.len();
    let (cx, cy) = (w.center.x, w.center.y);
    let u = (age / (EASE * 1.4)).clamp(0., 1.) as f32;
    let e = 1. - (1. - u).powi(3);
    h.rect(0., 0., h.width, 720., [0.03, 0.04, 0.055, 0.35 * e], 0.);
    let step = std::f32::consts::TAU / n as f32;
    let gap = 0.035;
    let scale = 0.86 + 0.14 * e;
    let (r0, rr) = (INNER * scale, RADIUS * scale);
    for (i, wp) in WEAPONS.iter().enumerate() {
        let on = w.hover == Some(i);
        let sel = i == c.weapon;
        let mid = i as f32 * step - std::f32::consts::FRAC_PI_2;
        let (a0, a1) = (mid - step / 2. + gap, mid + step / 2. - gap);
        let ro = if on { rr + 10. } else { rr };
        let fill = if on {
            [1., 0.827, 0.24, 0.92]
        } else if sel {
            [0.18, 0.2, 0.235, 0.9]
        } else {
            [0.08, 0.09, 0.12, 0.82]
        };
        h.arc(cx, cy, r0, ro - r0, a0, a1, fill);
        let d = slot_dir(i, n);
        let rm = (r0 + ro) / 2.;
        let (tx, ty) = (cx + d.x * rm, cy + d.y * rm);
        let col = if on {
            [0.1, 0.11, 0.13, 1.]
        } else if sel {
            [1., 0.827, 0.24, 1.]
        } else {
            [0.91, 0.91, 0.91, 1.]
        };
        let short = match wp.id {
            "fists" => "Fäuste",
            "bat" => "Schläger",
            "knife" => "Messer",
            "pistol" => "Pistole",
            "smg" => "MP",
            "grenade" => "Granate",
            "molotov" => "Molotow",
            _ => "Flinte",
        };
        h.text(short, tx, ty, 14., col, Align::Center, !on);
        if !wp.melee {
            let mag = c.mag[i];
            h.text(
                &format!("{mag}/{}", wp.mag),
                tx,
                ty + 18.,
                12.,
                if mag == 0 && !on {
                    [1., 0.5, 0.5, 1.]
                } else {
                    col
                },
                Align::Center,
                false,
            );
        }
        if !pad {
            h.text(
                &(i + 1).to_string(),
                cx + d.x * (ro - 14.),
                cy + d.y * (ro - 14.) + 5.,
                11.,
                [col[0], col[1], col[2], 0.5],
                Align::Center,
                false,
            );
        }
    }
    h.ellipse(cx, cy, r0 - 8., r0 - 8., [0.047, 0.055, 0.075, 0.9]);
    // Zeiger (Controller): wohin der Stick zeigt
    let vl = w.v.length();
    if pad && vl > 2. {
        let l = (r0 - 12.).min(vl * (r0 - 12.) / RADIUS * 1.6);
        let u = w.v / vl;
        h.line(
            cx + u.x * 8.,
            cy + u.y * 8.,
            cx + u.x * l,
            cy + u.y * l,
            3.,
            [1., 0.83, 0.24, 0.55],
        );
        h.ellipse(cx + u.x * l, cy + u.y * l, 4.5, 4.5, [1., 0.83, 0.24, 1.]);
    }
    let k = w.hover.unwrap_or(c.weapon);
    let wp = &WEAPONS[k];
    h.text(
        &wp.name.to_uppercase(),
        cx,
        cy - 4.,
        if wp.name.len() > 12 { 11. } else { 14. },
        [1., 0.83, 0.24, 1.],
        Align::Center,
        true,
    );
    let info = if wp.melee {
        "Nahkampf".to_string()
    } else if wp.throw {
        format!("{} übrig", c.mag[k])
    } else {
        format!("{} / {}", c.mag[k], wp.mag)
    };
    h.text(
        &info,
        cx,
        cy + 20.,
        14.,
        [0.87, 0.87, 0.87, 1.],
        Align::Center,
        true,
    );
    let hint = if pad {
        "Stick wählt · LB loslassen nimmt"
    } else {
        "Zeigen wählt · beide Tasten loslassen nimmt"
    };
    h.text(
        hint,
        cx,
        cy + rr + 34.,
        13.,
        [0.85, 0.85, 0.85, 1.],
        Align::Center,
        true,
    );
}

/// Eintrag des Fahrhilfen-Rads: Name, Zustand, verfügbar.
pub struct AssistItem {
    pub name: &'static str,
    pub state: String,
    pub enabled: bool,
}

/// Fahrhilfen-Rad (Controller: rechten Stick drücken im Fahrzeug): ESP, ABS, Sirene. Gleiche Form wie das
/// Waffenrad, ohne Zeitlupe.
pub fn draw_assist(h: &mut Hud, w: &WheelButton, items: &[AssistItem], age: f64) {
    let n = items.len().max(1);
    let (cx, cy) = (w.center.x, w.center.y);
    let u = (age / (EASE * 1.4)).clamp(0., 1.) as f32;
    let e = 1. - (1. - u).powi(3);
    let step = std::f32::consts::TAU / n as f32;
    let gap = 0.05;
    let scale = (0.86 + 0.14 * e) * 0.75;
    let (r0, rr) = (INNER * scale, RADIUS * scale);
    for (i, it) in items.iter().enumerate() {
        let on = w.hover == Some(i) && it.enabled;
        let mid = i as f32 * step - std::f32::consts::FRAC_PI_2;
        let (a0, a1) = (mid - step / 2. + gap, mid + step / 2. - gap);
        let ro = if on { rr + 8. } else { rr };
        let fill = if on {
            [1., 0.827, 0.24, 0.92]
        } else {
            [0.08, 0.09, 0.12, if it.enabled { 0.85 } else { 0.5 }]
        };
        h.arc(cx, cy, r0, ro - r0, a0, a1, fill);
        let d = slot_dir(i, n);
        let rm = (r0 + ro) / 2.;
        let (tx, ty) = (cx + d.x * rm, cy + d.y * rm);
        let col = if on {
            [0.1, 0.11, 0.13, 1.]
        } else if it.enabled {
            [0.93, 0.93, 0.93, 1.]
        } else {
            [0.55, 0.55, 0.55, 1.]
        };
        h.text(it.name, tx, ty - 2., 14., col, Align::Center, !on);
        h.text(&it.state, tx, ty + 15., 11., col, Align::Center, false);
    }
    h.ellipse(cx, cy, r0 - 6., r0 - 6., [0.047, 0.055, 0.075, 0.9]);
    h.text(
        "FAHRHILFEN",
        cx,
        cy + 4.,
        11.,
        [1., 0.83, 0.24, 1.],
        Align::Center,
        true,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stick_click_opens_selects_and_confirms() {
        let mut w = WheelButton::default();
        let at = Vec2::new(640., 360.);
        // Drücken öffnet, die aktuelle Waffe ist vorgewählt
        let o = w.pad_step(0., at, true, false, false, (0., 0.), true, 2, 6);
        assert!(o.opened && w.open && w.hover == Some(2));
        // Stick nach rechts wählt das Feld rechts, nochmal drücken nimmt es
        w.pad_step(0.1, at, false, false, false, (1., 0.), true, 2, 6);
        assert_eq!(w.hover, slot(1., 0., 6, 0.));
        let o = w.pad_step(0.2, at, true, false, false, (0., 0.), true, 2, 6);
        assert_eq!(o.pick, slot(1., 0., 6, 0.));
        assert!(!w.open);
        // A nimmt ebenfalls, B bricht ohne Wahl ab
        w.pad_step(0.3, at, true, false, false, (0., 0.), true, 0, 6);
        assert_eq!(
            w.pad_step(0.4, at, false, true, false, (0., 0.), true, 0, 6)
                .pick,
            Some(0)
        );
        w.pad_step(0.5, at, true, false, false, (0., 0.), true, 0, 6);
        let o = w.pad_step(0.6, at, false, false, true, (0., 0.), true, 0, 6);
        assert!(o.pick.is_none() && o.closed && !w.open);
        // nicht öffnen, wo es nicht darf (im Auto, tot); offenes Rad schließt dann ohne Wahl
        assert!(
            !w.pad_step(0.7, at, true, false, false, (0., 0.), false, 0, 6)
                .opened
        );
        w.pad_step(0.8, at, true, false, false, (0., 0.), true, 0, 6);
        let o = w.pad_step(0.9, at, false, false, false, (0., 0.), false, 0, 6);
        assert!(o.closed && o.pick.is_none());
    }
    #[test]
    fn slots_go_clockwise_from_the_top() {
        assert_eq!(slot(0., -50., 6, DEAD), Some(0));
        assert_eq!(slot(50., -20., 6, DEAD), Some(1));
        assert_eq!(slot(0., 50., 6, DEAD), Some(3));
        assert_eq!(slot(-50., -20., 6, DEAD), Some(5));
        assert_eq!(slot(3., 3., 6, DEAD), None, "Totzone");
        let d = slot_dir(3, 6);
        assert!(d.x.abs() < 1e-6 && (d.y - 1.).abs() < 1e-6);
    }
    #[test]
    fn tap_hold_pick_and_cancel() {
        let mut w = WheelButton::default();
        w.press(0., Vec2::new(300., 300.));
        assert!(w.release(0.1).tap, "kurz = tippen");
        w.press(1., Vec2::new(300., 300.));
        assert!(!w.tick(1.1, true, 2, 6).opened);
        assert!(w.tick(1.25, true, 2, 6).opened);
        assert_eq!(w.hover, Some(2), "Start: aktuelle Waffe");
        w.mov(Vec2::new(300., 240.));
        assert_eq!(w.hover, Some(0));
        w.mov(Vec2::new(302., 302.));
        assert_eq!(w.hover, Some(0), "in der Totzone bleibt die Wahl");
        let o = w.release(1.5);
        assert_eq!((o.pick, o.closed, o.tap), (Some(0), true, false));
        // im Auto (nicht mehr zu Fuß) schließt das Rad ohne Wahl
        w.press(2., Vec2::ZERO);
        w.tick(2.3, true, 0, 6);
        assert!(w.tick(2.4, false, 0, 6).closed);
        assert!(!w.open);
        // Controller: Stick wählt
        w.press(3., Vec2::ZERO);
        w.tick(3.3, true, 4, 6);
        w.aim(0.9, 0.1);
        assert_eq!(w.hover, Some(2));
        assert_eq!(w.choose(5).pick, Some(5));
        assert!((ease_time_scale(1., true, 1.) - SLOW).abs() < 1e-9);
    }
}
