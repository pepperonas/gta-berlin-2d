//! Große Karte (Stadtplan von ganz Berlin, `hud.js drawBigMap` + `maplabels.js`): Tab bzw. View öffnet sie,
//! WASD/Pfeile/linker Stick verschieben, +/−, Bild↑/↓ bzw. RT/LT zoomen. Die Welt läuft weiter, der Spieler steht.
use berlin_engine::hud::{Align, Hud};
use berlin_engine::{KeyCode, Keys};
use berlin_map_loader::overview::{Overview, PointLabel, Station, StreetLine};
use berlin_sim::mission::PlayerView;
use berlin_sim::world::World;
use glam::Vec2;

/// Zoomgrenzen: 1 = ganz Berlin, 64 ≈ 1 m je Bildpunkt (hud.js).
pub const ZOOM_MAX: f32 = 64.;
/// Pixel je Meter der Kartendaten.
const PX_PER_M: f32 = 10.;

/// Beschriftungsstufen: sichtbar, solange die Karte zwischen lo und hi Metern je HUD-Pixel zeigt (maplabels.js).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Bezirk,
    Ortsteil,
    Kiez,
    Station,
    MainStreet,
    Street,
}
impl Kind {
    fn tier(self) -> (f32, f32) {
        match self {
            Kind::Bezirk => (18., f32::INFINITY),
            Kind::Ortsteil => (3., 40.),
            Kind::Kiez => (0., 11.),
            Kind::Station => (0., 8.),
            Kind::MainStreet => (0., 6.),
            Kind::Street => (0., 2.6),
        }
    }
    pub fn visible(self, mpp: f32) -> bool {
        let (lo, hi) = self.tier();
        mpp > lo && mpp <= hi
    }
    pub fn size(self) -> f32 {
        match self {
            Kind::Bezirk => 17.,
            Kind::Ortsteil => 15.,
            Kind::Kiez => 13.,
            Kind::Station => 12.,
            Kind::MainStreet => 12.,
            Kind::Street => 11.,
        }
    }
    fn color(self) -> [f32; 4] {
        match self {
            Kind::Bezirk => [1., 1., 1., 1.],
            Kind::Ortsteil => [0.9, 0.9, 0.9, 1.],
            Kind::Kiez => [1., 0.878, 0.541, 1.],
            Kind::Station => [0.847, 0.847, 0.847, 1.],
            Kind::MainStreet => [0.957, 0.957, 0.957, 1.],
            Kind::Street => [0.91, 0.91, 0.91, 1.],
        }
    }
}

/// Bildausschnitt: Rechteck im HUD (Basiseinheiten), Maßstab `f` (HUD-Einheiten je Welt-px), Ursprung.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub f: f32,
    pub ox: f32,
    pub oy: f32,
}
impl View {
    pub fn to_screen(self, p: Vec2) -> Vec2 {
        Vec2::new(self.ox + p.x * self.f, self.oy + p.y * self.f)
    }
    /// Meter je HUD-Pixel
    pub fn mpp(self) -> f32 {
        1. / (self.f * PX_PER_M)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub kind: Kind,
    pub text: String,
    pub at: Vec2,
    /// Drehung (Straßennamen entlang der Straße), sonst 0
    pub angle: f32,
}

/// Beschriftungen setzen (maplabels.js ohne Straßennamen): Wichtigeres zuerst, Überlappendes entfällt, alles
/// bleibt im Kartenrechteck und außerhalb der `blocked`-Flächen. `measure(text, size)` = Breite in HUD-Einheiten.
pub fn place_labels(
    ov: &Labels,
    view: &View,
    measure: &dyn Fn(&str, f32) -> f32,
    blocked: &[[f32; 4]],
) -> Vec<Placed> {
    let mpp = view.mpp();
    let mut boxes: Vec<[f32; 4]> = blocked.to_vec();
    let mut out = Vec::new();
    // Hüllrechteck des (gedrehten) Textes; gesetzt wird nur, was ganz im Bild und frei ist
    let mut put_at = |kind: Kind, text: &str, at: Vec2, angle: f32, pad: f32| -> bool {
        let (tw, th) = (measure(text, kind.size()), kind.size());
        let (c, s) = (angle.cos().abs(), angle.sin().abs());
        let (hw, hh) = ((tw * c + th * s) / 2. + pad, (tw * s + th * c) / 2. + pad);
        let b = [at.x - hw, at.y - hh, at.x + hw, at.y + hh];
        let inside =
            b[0] >= view.x && b[2] <= view.x + view.w && b[1] >= view.y && b[3] <= view.y + view.h;
        if !inside
            || boxes
                .iter()
                .any(|o| b[0] < o[2] && b[2] > o[0] && b[1] < o[3] && b[3] > o[1])
        {
            return false;
        }
        boxes.push(b);
        out.push(Placed {
            kind,
            text: text.to_owned(),
            at,
            angle,
        });
        true
    };
    let mut put = |kind: Kind, text: &str, at: Vec2| {
        put_at(kind, text, at, 0., 3.);
    };
    let mut big_first = |kind: Kind, list: &[PointLabel]| {
        if !kind.visible(mpp) {
            return;
        }
        let mut items: Vec<&PointLabel> = list.iter().collect();
        items.sort_by(|a, b| b.area.total_cmp(&a.area));
        for l in items {
            put(kind, &l.text, view.to_screen(l.at));
        }
    };
    big_first(Kind::Bezirk, &ov.bezirke);
    big_first(Kind::Ortsteil, &ov.ortsteile);
    if Kind::Station.visible(mpp) {
        for s in &ov.stations {
            let w = measure(&s.name, Kind::Station.size());
            put(
                Kind::Station,
                &s.name,
                view.to_screen(s.at) + Vec2::new(9. + w / 2., 0.),
            );
        }
    }
    if Kind::Kiez.visible(mpp) {
        for k in &ov.kieze {
            put(Kind::Kiez, &k.text, view.to_screen(k.at));
        }
    }
    // Straßennamen entlang der Straße (maplabels.js streetLabels): fast gerade Läufe (Knick < 0,3 rad)
    // zusammenfassen, nur Läufe länger als der Name, die längsten zuerst, gleiche Namen nicht zu dicht, nie kopfüber
    for kind in [Kind::MainStreet, Kind::Street] {
        if !kind.visible(mpp) {
            continue;
        }
        let main = kind == Kind::MainStreet;
        let w0 = Vec2::new((view.x - view.ox) / view.f, (view.y - view.oy) / view.f);
        let w1 = Vec2::new(
            (view.x + view.w - view.ox) / view.f,
            (view.y + view.h - view.oy) / view.f,
        );
        let mut cands: Vec<(f32, &str, Vec2, Vec2)> = Vec::new();
        for st in &ov.lines {
            if (main && st.class > 5) || (!main && st.class <= 5) {
                continue;
            }
            if st.max.x < w0.x || st.min.x > w1.x || st.max.y < w0.y || st.min.y > w1.y {
                continue;
            }
            let tw = measure(&st.name, kind.size()) + 16.;
            let p = &st.pts;
            let mut i0 = 0;
            for i in 1..p.len() {
                let end = i == p.len() - 1;
                let bend = !end && {
                    let a = (p[i + 1] - p[i]).to_angle();
                    let b = (p[i] - p[i0]).to_angle();
                    let mut d = a - b;
                    while d > std::f32::consts::PI {
                        d -= std::f32::consts::TAU;
                    }
                    while d < -std::f32::consts::PI {
                        d += std::f32::consts::TAU;
                    }
                    d.abs() > 0.3
                };
                if !end && !bend {
                    continue;
                }
                let len = p[i].distance(p[i0]) * view.f;
                if len >= tw {
                    cands.push((len, &st.name, view.to_screen(p[i0]), view.to_screen(p[i])));
                }
                i0 = i;
            }
        }
        cands.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(b.1)));
        let mut done: std::collections::HashMap<&str, Vec<Vec2>> = Default::default();
        for (_, name, a, b) in cands {
            let m = (a + b) * 0.5;
            if done
                .get(name)
                .is_some_and(|v| v.iter().any(|q| q.distance(m) < 320.))
            {
                continue;
            }
            let mut ang = (b - a).to_angle();
            if ang > std::f32::consts::FRAC_PI_2 {
                ang -= std::f32::consts::PI;
            } else if ang < -std::f32::consts::FRAC_PI_2 {
                ang += std::f32::consts::PI;
            }
            if put_at(kind, name, m, ang, 2.) {
                done.entry(name).or_default().push(m);
            }
        }
    }
    out
}

/// Beschriftungsdaten (ohne die Zeichnung, die die Engine hält).
#[derive(Debug, Clone, Default)]
pub struct Labels {
    pub width: f32,
    pub height: f32,
    pub bezirke: Vec<PointLabel>,
    pub ortsteile: Vec<PointLabel>,
    pub kieze: Vec<PointLabel>,
    pub stations: Vec<Station>,
    pub streets: Vec<PointLabel>,
    pub lines: Vec<StreetLine>,
}

/// Was ein Klick auf den Stadtplan will (Kartenpixel).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MapClick {
    Waypoint(Vec2),
    Teleport(Vec2),
}

#[derive(Default)]
pub struct BigMap {
    pub open: bool,
    pub z: f32,
    pub center: Vec2,
    pub labels: Labels,
    /// Zeichnung für die Engine, bis sie abgeholt ist
    pub mesh: Option<berlin_map_loader::overview::OverlayMesh>,
    cache: Option<(View, Vec<Placed>)>,
    /// Ziehen mit der Maus: letzter Zeigerpunkt (HUD) und zurückgelegter Weg seit dem Drücken
    drag: Option<Vec2>,
    moved: f32,
}

impl BigMap {
    pub fn new(ov: Overview) -> Self {
        Self {
            open: false,
            z: 1.,
            center: Vec2::new(ov.width / 2., ov.height / 2.),
            labels: Labels {
                width: ov.width,
                height: ov.height,
                bezirke: ov.bezirke,
                ortsteile: ov.ortsteile,
                kieze: ov.kieze,
                stations: ov.stations,
                streets: ov.streets,
                lines: ov.street_lines,
            },
            mesh: Some(ov.mesh),
            cache: None,
            drag: None,
            moved: 0.,
        }
    }
    /// `map` = Stadtplan-Taste der Belegung gedrückt; offen schließt auch B.
    pub fn toggle(&mut self, keys: &Keys, map: bool) -> bool {
        let t = map || (self.open && keys.pad_pressed.b);
        if t && self.labels.width > 0. {
            self.open = !self.open;
            if self.open {
                // wie hud.js: jedes Öffnen beginnt bei ganz Berlin
                self.z = 1.;
                self.center = Vec2::new(self.labels.width / 2., self.labels.height / 2.);
            }
        }
        t
    }
    /// Kartenrechteck im HUD und Maßstab; schiebt die Mitte nicht über den Rand von Berlin hinaus.
    pub fn view(&mut self, hud_width: f32) -> View {
        let (mx, my) = (hud_width * 0.05, 720. * 0.05);
        let (x, y, w, h) = (mx, my + 30., hud_width - 2. * mx, 720. - 2. * my - 60.);
        let (cw, ch) = (self.labels.width.max(1.), self.labels.height.max(1.));
        let f0 = (w / cw).min(h / ch);
        let f = f0 * self.z;
        let (hw, hh) = (w / 2. / f, h / 2. / f);
        self.center.x = if hw * 2. >= cw {
            cw / 2.
        } else {
            self.center.x.clamp(hw, cw - hw)
        };
        self.center.y = if hh * 2. >= ch {
            ch / 2.
        } else {
            self.center.y.clamp(hh, ch - hh)
        };
        View {
            x,
            y,
            w,
            h,
            f,
            ox: x + w / 2. - self.center.x * f,
            oy: y + h / 2. - self.center.y * f,
        }
    }
    /// Verschieben, Zoomen; liefert, was ein Klick (ohne Ziehen) bzw. eine Controller-Taste will: Linksklick/A setzt
    /// den Wegpunkt (bzw. entfernt ihn), Rechtsklick/X teleportiert (mit Rückfrage). Am Controller zielt die
    /// Kartenmitte (Fadenkreuz).
    pub fn control(&mut self, keys: &Keys, dt: f32, v: View) -> Option<MapClick> {
        let f = v.f;
        // Maus: Rad zoomt um den Zeiger, Ziehen verschiebt (hud.js zoomBigMap/panBigMap)
        let m = keys.mouse;
        if m.wheel != 0.
            && let Some(p) = m.hud
        {
            // der Weltpunkt unter dem Zeiger bleibt, wo er ist
            let world = (p - Vec2::new(v.ox, v.oy)) / f;
            let f0 = f / self.z;
            self.z = (self.z * (m.wheel * 0.18).exp()).clamp(1., ZOOM_MAX);
            let f1 = f0 * self.z;
            self.center = world - (p - Vec2::new(v.x + v.w / 2., v.y + v.h / 2.)) / f1;
        }
        if m.left_pressed {
            self.moved = 0.;
        }
        if m.left
            && !m.left_pressed
            && let (Some(p), Some(last)) = (m.hud, self.drag)
        {
            self.center -= (p - last) / f;
            self.moved += p.distance(last);
        }
        let mut click = None;
        let on_map = |p: Vec2| p.x >= v.x && p.x <= v.x + v.w && p.y >= v.y && p.y <= v.y + v.h;
        let world = |p: Vec2| (p - Vec2::new(v.ox, v.oy)) / f;
        if m.left_released
            && self.moved <= 6.
            && let Some(p) = m.hud
            && on_map(p)
        {
            click = Some(MapClick::Waypoint(world(p)));
        }
        if m.right_pressed
            && let Some(p) = m.hud
            && on_map(p)
        {
            click = Some(MapClick::Teleport(world(p)));
        }
        if keys.pad_pressed.a {
            click = Some(MapClick::Waypoint(self.center));
        }
        if keys.pad_pressed.x {
            click = Some(MapClick::Teleport(self.center));
        }
        self.drag = if m.left { m.hud } else { None };
        let held = |k: KeyCode| keys.held.contains(&k);
        let p = &keys.pad;
        let mut d = Vec2::new(
            (held(KeyCode::KeyD) || held(KeyCode::ArrowRight)) as i32 as f32
                - (held(KeyCode::KeyA) || held(KeyCode::ArrowLeft)) as i32 as f32,
            (held(KeyCode::KeyS) || held(KeyCode::ArrowDown)) as i32 as f32
                - (held(KeyCode::KeyW) || held(KeyCode::ArrowUp)) as i32 as f32,
        );
        if p.lx.hypot(p.ly) > 0.22 {
            d = Vec2::new(p.lx, p.ly);
        }
        // 600 HUD-Einheiten je Sekunde, unabhängig vom Zoom
        self.center += d * 600. * dt / f.max(1e-6);
        let zin = held(KeyCode::Equal) || held(KeyCode::NumpadAdd) || held(KeyCode::PageUp);
        let zout = held(KeyCode::Minus) || held(KeyCode::NumpadSubtract) || held(KeyCode::PageDown);
        let rate = (zin as i32 as f32 - zout as i32 as f32) + p.rt - p.lt;
        self.z = (self.z * (rate * 1.6 * dt).exp()).clamp(1., ZOOM_MAX);
        click
    }
    /// `pad`: am Controller bedient (Fadenkreuz in der Mitte zeigt, wohin A/X wirken).
    pub fn draw(&mut self, w: &World, nav: &crate::nav::NavView, pad: bool, h: &mut Hud) {
        let v = self.view(h.width);
        h.rect(0., 0., h.width, 720., [0.047, 0.051, 0.063, 0.94], 0.);
        h.rect(v.x, v.y, v.w, v.h, [0.227, 0.239, 0.267, 1.], 10.);
        h.overview_inset(
            [v.x, v.y, v.w, v.h],
            self.center.into(),
            v.w / v.f,
            self.z >= 4.,
        );
        // Bahnhöfe
        let r = if self.z >= 3. { 6. } else { 3.5 };
        for s in &self.labels.stations {
            let p = v.to_screen(s.at);
            if p.x < v.x + r || p.x > v.x + v.w - r || p.y < v.y + r || p.y > v.y + v.h - r {
                continue;
            }
            station_icon(h, &s.cat, p, r);
        }
        // Beschriftung nur neu setzen, wenn sich die Ansicht geändert hat
        // ausführlich, wenn es passt, sonst knapp (bei 1280 px rastet die Schrift grob)
        let (long, short) = if pad {
            (
                "Stick: verschieben · LT/RT: zoomen · A: Wegpunkt setzen/entfernen · X: teleportieren · B: zurück",
                "Stick: schieben · LT/RT: Zoom · A: Wegpunkt · X: Teleport · B: zurück",
            )
        } else {
            (
                "Ziehen/WASD: verschieben · Rad/+/−: zoomen · Klick: Wegpunkt setzen/entfernen · Rechtsklick: teleportieren",
                "Ziehen/WASD: schieben · Rad: Zoom · Klick: Wegpunkt · Rechtsklick: Teleport",
            )
        };
        let hint_text = if h.text_width(long, 13.) + 24. <= v.w - 20. {
            long
        } else {
            short
        };
        let hint_w = h.text_width(hint_text, 13.) + 24.;
        let hint = [
            v.x + 10.,
            v.y + v.h - 40.,
            v.x + 10. + hint_w,
            v.y + v.h - 10.,
        ];
        let placed = match &self.cache {
            Some((cv, p)) if *cv == v => p.clone(),
            _ => {
                let measure = |t: &str, size: f32| h.text_width(t, size);
                let p = place_labels(&self.labels, &v, &measure, &[hint]);
                self.cache = Some((v, p.clone()));
                p
            }
        };
        for l in &placed {
            if l.angle != 0. {
                h.text_rotated(
                    &l.text,
                    l.at.x,
                    l.at.y,
                    l.kind.size(),
                    l.angle,
                    l.kind.color(),
                    true,
                );
                continue;
            }
            h.text(
                &l.text,
                l.at.x,
                l.at.y + l.kind.size() * 0.35,
                l.kind.size(),
                l.kind.color(),
                Align::Center,
                true,
            );
        }
        // Späti, Ziel, Spieler
        let dot = |h: &mut Hud, at: (f64, f64), r: f32, c: [f32; 4]| {
            let p = v.to_screen(Vec2::new(at.0 as f32, at.1 as f32));
            if p.x < v.x || p.x > v.x + v.w || p.y < v.y || p.y > v.y + v.h {
                return;
            }
            h.ellipse(p.x, p.y, r + 2., r + 2., [0., 0., 0., 1.]);
            h.ellipse(p.x, p.y, r, r, c);
        };
        // Route zum Wegpunkt unter den Markierungen
        if nav.route.len() >= 2 {
            let to = |x: f64, y: f64| {
                let p = v.to_screen(Vec2::new(x as f32, y as f32));
                (p.x, p.y)
            };
            crate::nav::draw_route(h, &nav.route, to, [v.x, v.y, v.w, v.h], 4.);
        }
        if let Some((wx, wy)) = nav.waypoint {
            let p = v.to_screen(Vec2::new(wx as f32, wy as f32));
            if p.x >= v.x && p.x <= v.x + v.w && p.y >= v.y && p.y <= v.y + v.h {
                crate::nav::draw_pin(h, p.x, p.y, 7.);
            }
        }
        if pad {
            // Fadenkreuz: dort wirken A (Wegpunkt) und X (Teleport)
            let c = Vec2::new(v.x + v.w / 2., v.y + v.h / 2.);
            for (dx, dy) in [(1., 0.), (-1., 0.), (0., 1.), (0., -1.)] {
                let (a, b) = (c + Vec2::new(dx, dy) * 5., c + Vec2::new(dx, dy) * 14.);
                h.line(a.x, a.y, b.x, b.y, 4., [0., 0., 0., 0.8]);
                h.line(a.x, a.y, b.x, b.y, 2., [1.; 4]);
            }
        }
        let g = &w.city.places.giver;
        dot(h, (g.x, g.y), 5., [0.878, 0.231, 0.231, 1.]);
        let pv = PlayerView {
            x: w.player.x,
            y: w.player.y,
            in_car: w.player.in_car,
        };
        if let Some(t) = w.mission.objective(&w.city.places, pv, &w.cars).1 {
            dot(h, t, 8., [1., 0.827, 0.239, 1.]);
        }
        let p = w
            .player_car()
            .map_or((w.player.x, w.player.y), |c| (c.x, c.y));
        dot(h, p, 6., [1.; 4]);
        // Kopf, Legende, Hinweis
        h.text(
            "Stadtplan · Berlin",
            v.x,
            v.y - 10.,
            20.,
            [1.; 4],
            Align::Left,
            true,
        );
        // Legende unter der Karte links, Quellenangabe rechts; reicht die Zeile nicht für beide, rutscht die
        // Quellenangabe eine Zeile tiefer (verkleinern hieße bei 1280 px: auf halbe Größe springen)
        let legend = "rot = Späti · gelb = Ziel · violett = Wegpunkt · weiß = du · U/S = Bahnhof";
        let credit = "Kartendaten © OpenStreetMap-Mitwirkende (ODbL)";
        let below = v.y + v.h + 20.;
        let both = h.text_width(legend, 12.) + h.text_width(credit, 12.) + 24. <= v.w;
        h.text(
            legend,
            v.x,
            below,
            12.,
            [0.8, 0.8, 0.8, 1.],
            Align::Left,
            true,
        );
        h.text(
            credit,
            v.x + v.w,
            if both { below } else { below + 16. },
            12.,
            [0.67, 0.67, 0.67, 1.],
            Align::Right,
            true,
        );
        h.rect(
            hint[0],
            hint[1],
            hint[2] - hint[0],
            hint[3] - hint[1],
            [0.06, 0.07, 0.09, 0.75],
            6.,
        );
        h.text(
            hint_text,
            hint[0] + 12.,
            hint[1] + 20.,
            13.,
            [0.93, 0.93, 0.93, 1.],
            Align::Left,
            false,
        );
    }
}

/// U-/S-Bahn-Symbol: blaues Quadrat mit U bzw. grüner Kreis mit S (Größe r = halbe Kante).
pub fn station_icon(h: &mut Hud, cat: &str, p: Vec2, r: f32) {
    let (bg, letter) = match cat {
        "ubahn" => ([0.067, 0.365, 0.659, 1.], "U"),
        "sbahn" => ([0.0, 0.533, 0.278, 1.], "S"),
        _ => ([0.45, 0.45, 0.45, 1.], ""),
    };
    if letter == "S" {
        h.ellipse(p.x, p.y, r, r, bg);
    } else {
        h.rect(p.x - r, p.y - r, 2. * r, 2. * r, bg, r * 0.3);
    }
    if r >= 5. && !letter.is_empty() {
        h.text(
            letter,
            p.x,
            p.y + r * 0.6,
            2. * r - 2.,
            [1.; 4],
            Align::Center,
            false,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn labels() -> Labels {
        let l = |x, y, t: &str, a| PointLabel {
            at: Vec2::new(x, y),
            text: t.into(),
            area: a,
        };
        Labels {
            width: 400_000.,
            height: 300_000.,
            bezirke: vec![
                l(100_000., 100_000., "Klein", 10.),
                l(101_000., 100_000., "Groß", 99.),
            ],
            ortsteile: vec![l(200_000., 150_000., "Ort", 1.)],
            kieze: vec![l(200_100., 150_100., "Kiez", 0.)],
            stations: vec![],
            streets: vec![],
            lines: vec![],
        }
    }
    #[test]
    fn street_names_follow_the_street() {
        let mut lab = labels();
        let (a, b) = (Vec2::new(200_000., 150_000.), Vec2::new(200_400., 150_400.));
        lab.lines = vec![StreetLine {
            class: 6,
            name: "Teststraße".into(),
            pts: vec![a, b],
            min: a,
            max: b,
        }];
        // 0,2 m je HUD-Pixel: Nebenstraßen sichtbar; Straßenmitte in der Bildmitte
        let v = View {
            x: 0.,
            y: 0.,
            w: 1200.,
            h: 600.,
            f: 0.5,
            ox: 600. - 200_200. * 0.5,
            oy: 300. - 150_200. * 0.5,
        };
        let measure = |t: &str, s: f32| t.chars().count() as f32 * s * 0.6;
        let placed = place_labels(&lab, &v, &measure, &[]);
        let st = placed
            .iter()
            .find(|p| p.kind == Kind::Street)
            .expect("Straßenname");
        assert!(
            (st.angle - std::f32::consts::FRAC_PI_4).abs() < 1e-3,
            "{}",
            st.angle
        );
        assert!((st.at - Vec2::new(600., 300.)).length() < 1.);
        // weit herausgezoomt keine Nebenstraßen
        let far = View { f: 0.01, ..v };
        assert!(
            place_labels(&lab, &far, &measure, &[])
                .iter()
                .all(|p| p.kind != Kind::Street)
        );
    }
    #[test]
    fn left_click_sets_a_waypoint_right_click_teleports_dragging_does_neither() {
        use berlin_engine::Mouse;
        use std::collections::HashSet;
        let mut m = BigMap {
            labels: labels(),
            z: 1.,
            ..Default::default()
        };
        let v = m.view(1280.);
        let empty = HashSet::new();
        let step = |m: &mut BigMap, mouse: Mouse| {
            let keys = Keys {
                held: &empty,
                pressed: &empty,
                pad: Default::default(),
                pad_pressed: Default::default(),
                mouse,
                typed: "",
            };
            m.control(&keys, 1. / 60., v)
        };
        let p = Vec2::new(v.x + v.w / 2., v.y + v.h / 2.);
        let at = (p - Vec2::new(v.ox, v.oy)) / v.f;
        let press = Mouse {
            hud: Some(p),
            left: true,
            left_pressed: true,
            ..Default::default()
        };
        let release = Mouse {
            hud: Some(p),
            left_released: true,
            ..Default::default()
        };
        // links: Drücken allein tut nichts, Loslassen ohne Ziehen setzt den Wegpunkt
        assert_eq!(step(&mut m, press), None);
        assert_eq!(step(&mut m, release), Some(MapClick::Waypoint(at)));
        // rechts: teleportiert sofort an die Stelle unter dem Zeiger
        let right = Mouse {
            hud: Some(p),
            right: true,
            right_pressed: true,
            ..Default::default()
        };
        assert_eq!(step(&mut m, right), Some(MapClick::Teleport(at)));
        // gezogen (Karte verschoben): kein Wegpunkt beim Loslassen
        step(&mut m, press);
        let q = p + Vec2::new(40., 0.);
        step(
            &mut m,
            Mouse {
                hud: Some(q),
                left: true,
                ..Default::default()
            },
        );
        assert_eq!(
            step(
                &mut m,
                Mouse {
                    hud: Some(q),
                    left_released: true,
                    ..Default::default()
                }
            ),
            None
        );
        // außerhalb der Karte: nichts
        let out = Vec2::new(2., 2.);
        assert_eq!(
            step(
                &mut m,
                Mouse {
                    hud: Some(out),
                    right_pressed: true,
                    ..Default::default()
                }
            ),
            None
        );
    }
    #[test]
    fn view_clamps_to_berlin_and_zoom_bounds() {
        let mut m = BigMap {
            labels: labels(),
            z: 1.,
            ..Default::default()
        };
        let v = m.view(1280.);
        // ganz Berlin passt ins Rechteck: Mitte = Stadtmitte
        assert_eq!(m.center, Vec2::new(200_000., 150_000.));
        assert!((v.to_screen(Vec2::ZERO).x - v.x).abs() < v.w * 0.2 + 1.);
        m.z = 8.;
        m.center = Vec2::new(-1e9, 1e9);
        let v = m.view(1280.);
        let tl = (Vec2::new(v.x, v.y) - Vec2::new(v.ox, v.oy)) / v.f;
        assert!(
            tl.x.abs() < 1. && (tl.y + v.h / v.f - 300_000.).abs() < 1.,
            "an der Ecke festgehalten"
        );
    }
    #[test]
    fn labels_follow_tiers_and_never_overlap() {
        let mut m = BigMap {
            labels: labels(),
            z: 1.,
            ..Default::default()
        };
        let measure = |t: &str, s: f32| t.chars().count() as f32 * s * 0.6;
        // ganz Berlin: nur Bezirke; der größere gewinnt die überlappende Stelle
        let v = m.view(1280.);
        let p = place_labels(&m.labels, &v, &measure, &[]);
        assert_eq!(
            p.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
            ["Groß"]
        );
        // nah heran: Ortsteil und Kiez, keine Bezirke; ein Sperrrechteck über dem Ortsteil verdrängt ihn
        m.z = 40.;
        m.center = Vec2::new(200_000., 150_000.);
        let v = m.view(1280.);
        assert!(v.mpp() < 11. && v.mpp() > 3. || v.mpp() <= 3.);
        let p = place_labels(&m.labels, &v, &measure, &[]);
        assert!(p.iter().any(|l| l.kind == Kind::Kiez));
        assert!(p.iter().all(|l| l.kind != Kind::Bezirk));
        let at = v.to_screen(Vec2::new(200_000., 150_000.));
        let blocked = [[at.x - 5., at.y - 5., at.x + 5., at.y + 5.]];
        let p = place_labels(&m.labels, &v, &measure, &blocked);
        assert!(p.iter().all(|l| l.text != "Ort"));
    }
}
