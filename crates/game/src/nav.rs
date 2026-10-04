//! Wegpunkt und Route (Stadtplan-Klick): der Straßengraph von ganz Berlin wird beim Start im Hintergrund gebaut
//! (`berlin_sim::routing`), danach folgt die Route dem Spieler – mit Fortschritt, Restweg, Neuberechnung beim
//! Abweichen oder beim Wechsel Auto/zu Fuß, und dem Ende am Ziel.
use berlin_sim::routing::{Mode, Route, RouteGraph};
use std::path::PathBuf;
use std::sync::{Arc, mpsc};

/// Ab hier gilt die Route als verlassen (Kartenpixel, 25 m)
const OFF_ROUTE: f64 = 250.;
/// Ziel erreicht (30 m)
const ARRIVE: f64 = 300.;
/// frühestens so oft neu rechnen (s)
const RECALC_S: f64 = 0.8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavEvent {
    Arrived,
    NoRoute,
}

/// Was Minikarte, HUD und Stadtplan von der Navigation brauchen.
#[derive(Debug, Clone, Default)]
pub struct NavView {
    /// Route ab der Spielerposition (leer, solange keine da ist)
    pub route: Vec<(f64, f64)>,
    pub waypoint: Option<(f64, f64)>,
    pub remaining_m: Option<f64>,
}

/// Farbe der Route und des Wegpunkts (violett: unterscheidet sich vom gelben Auftragsziel und den Straßen)
pub const ROUTE_COLOR: [f32; 4] = [0.74, 0.46, 1., 1.];

/// Strich von a nach b auf das Rechteck [x, y, w, h] zugeschnitten (Liang–Barsky); `None`, wenn er außerhalb liegt.
pub fn clip(a: (f32, f32), b: (f32, f32), r: [f32; 4]) -> Option<((f32, f32), (f32, f32))> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0f32, 1f32);
    for (p, q) in [
        (-dx, a.0 - r[0]),
        (dx, r[0] + r[2] - a.0),
        (-dy, a.1 - r[1]),
        (dy, r[1] + r[3] - a.1),
    ] {
        if p == 0. {
            if q < 0. {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0. {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
        if t0 > t1 {
            return None;
        }
    }
    Some((
        (a.0 + dx * t0, a.1 + dy * t0),
        (a.0 + dx * t1, a.1 + dy * t1),
    ))
}

/// Route als Linie mit dunklem Rand, auf das Rechteck zugeschnitten; `to` rechnet Kartenpixel in HUD-Einheiten.
pub fn draw_route(
    h: &mut berlin_engine::hud::Hud,
    route: &[(f64, f64)],
    to: impl Fn(f64, f64) -> (f32, f32),
    rect: [f32; 4],
    width: f32,
) {
    // Punkte, die auf dem Bildschirm zusammenfallen, überspringen
    let mut pts: Vec<(f32, f32)> = Vec::with_capacity(route.len());
    for &(x, y) in route {
        let p = to(x, y);
        if pts
            .last()
            .is_none_or(|q: &(f32, f32)| (q.0 - p.0).hypot(q.1 - p.1) > 1.5)
        {
            pts.push(p);
        }
    }
    if let (Some(&last), Some(&(x, y))) = (pts.last(), route.last()) {
        let p = to(x, y);
        if last != p {
            pts.push(p);
        }
    }
    let segs: Vec<_> = pts
        .windows(2)
        .filter_map(|w| clip(w[0], w[1], rect))
        .collect();
    for (a, b) in &segs {
        h.line(a.0, a.1, b.0, b.1, width + 2.5, [0.08, 0.04, 0.14, 0.9]);
    }
    for (a, b) in &segs {
        h.line(a.0, a.1, b.0, b.1, width, ROUTE_COLOR);
    }
}

/// Wegpunkt-Markierung: Nadelkopf mit dunklem Rand und hellem Kern.
pub fn draw_pin(h: &mut berlin_engine::hud::Hud, x: f32, y: f32, r: f32) {
    h.ellipse(x, y, r + 2., r + 2., [0.08, 0.04, 0.14, 1.]);
    h.ellipse(x, y, r, r, ROUTE_COLOR);
    h.ellipse(x, y, r * 0.4, r * 0.4, [1.; 4]);
}

#[derive(Default)]
pub struct Nav {
    graph: Option<Arc<RouteGraph>>,
    rx: Option<mpsc::Receiver<RouteGraph>>,
    pub waypoint: Option<(f64, f64)>,
    pub route: Option<Route>,
    mode: Option<Mode>,
    last_calc: f64,
    /// Index des Routenstücks, auf dem der Spieler gerade ist
    progress: usize,
    /// für diesen Wegpunkt gab es keine Route (einmal gemeldet)
    failed: bool,
}

impl Nav {
    /// Graph im Hintergrund bauen (dauert auf zehn Kernen etwa 0,3 s).
    pub fn start(root: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || match RouteGraph::build(&root, None) {
            Ok(g) => {
                let _ = tx.send(g);
            }
            Err(e) => eprintln!("Navigation nicht verfügbar: {e}"),
        });
        Self {
            rx: Some(rx),
            ..Default::default()
        }
    }
    /// Mit fertigem Graphen (Tests).
    #[cfg(test)]
    pub fn with_graph(g: Arc<RouteGraph>) -> Self {
        Self {
            graph: Some(g),
            ..Default::default()
        }
    }
    pub fn ready(&self) -> bool {
        self.graph.is_some()
    }
    /// Wegpunkt setzen; ein Klick nahe am bestehenden (`near` Kartenpixel) entfernt ihn. Liefert, ob jetzt einer steht.
    pub fn toggle(&mut self, at: (f64, f64), near: f64) -> bool {
        if self
            .waypoint
            .is_some_and(|w| (w.0 - at.0).hypot(w.1 - at.1) < near)
        {
            self.clear();
            return false;
        }
        self.waypoint = Some(at);
        self.route = None;
        self.progress = 0;
        self.failed = false;
        self.last_calc = f64::NEG_INFINITY;
        true
    }
    pub fn clear(&mut self) {
        self.waypoint = None;
        self.route = None;
        self.progress = 0;
        self.failed = false;
    }
    /// Ein Schritt: Graph abholen, Ankunft prüfen, bei Bedarf (neu) rechnen, Fortschritt nachführen.
    pub fn step(&mut self, pos: (f64, f64), mode: Mode, t: f64) -> Option<NavEvent> {
        if let Some(rx) = &self.rx
            && let Ok(g) = rx.try_recv()
        {
            self.graph = Some(Arc::new(g));
            self.rx = None;
        }
        let w = self.waypoint?;
        if (w.0 - pos.0).hypot(w.1 - pos.1) < ARRIVE {
            self.clear();
            return Some(NavEvent::Arrived);
        }
        let g = self.graph.clone()?;
        if let Some(r) = &self.route {
            self.progress = advance(&r.pts, self.progress, pos);
        }
        let off = self
            .route
            .as_ref()
            .is_none_or(|r| dist_to(&r.pts[self.progress.min(r.pts.len() - 1)..], pos) > OFF_ROUTE);
        let changed = self.mode != Some(mode);
        if (off || changed) && t - self.last_calc >= RECALC_S && !(self.failed && !changed) {
            self.last_calc = t;
            self.mode = Some(mode);
            match g.route(pos, w, mode) {
                Some(r) => {
                    self.route = Some(r);
                    self.progress = 0;
                    self.failed = false;
                }
                None => {
                    self.route = None;
                    if !self.failed {
                        self.failed = true;
                        return Some(NavEvent::NoRoute);
                    }
                }
            }
        }
        None
    }
    /// Der noch zu fahrende Teil der Route (ab dem Stück, auf dem der Spieler ist), mit seiner Position vorn.
    pub fn ahead(&self, pos: (f64, f64)) -> Vec<(f64, f64)> {
        let Some(r) = &self.route else {
            return Vec::new();
        };
        let i = self.progress.min(r.pts.len().saturating_sub(1));
        let mut v = vec![pos];
        v.extend_from_slice(&r.pts[i + 1..]);
        v
    }
    pub fn view(&self, pos: (f64, f64)) -> NavView {
        NavView {
            route: self.ahead(pos),
            waypoint: self.waypoint,
            remaining_m: self.remaining_m(pos),
        }
    }
    /// Restweg in Metern (Kartenpixel / 10).
    pub fn remaining_m(&self, pos: (f64, f64)) -> Option<f64> {
        self.route.as_ref()?;
        let a = self.ahead(pos);
        Some(
            a.windows(2)
                .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
                .sum::<f64>()
                / 10.,
        )
    }
}

fn seg_dist(a: (f64, f64), b: (f64, f64), p: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0. {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0., 1.)
    } else {
        0.
    };
    (a.0 + dx * t - p.0).hypot(a.1 + dy * t - p.1)
}
/// Kleinster Abstand von `p` zu den ersten Stücken der Punktfolge (die nahen genügen).
fn dist_to(pts: &[(f64, f64)], p: (f64, f64)) -> f64 {
    if pts.len() < 2 {
        return pts
            .first()
            .map_or(f64::INFINITY, |a| (a.0 - p.0).hypot(a.1 - p.1));
    }
    pts.windows(2)
        .take(60)
        .map(|w| seg_dist(w[0], w[1], p))
        .fold(f64::INFINITY, f64::min)
}
/// Fortschritt: das nächste Stück ab `from` (nur vorwärts, in einem Fenster – Kreuzungen, an denen die Route sich
/// selbst nahekommt, lassen den Zeiger sonst springen).
fn advance(pts: &[(f64, f64)], from: usize, p: (f64, f64)) -> usize {
    let end = (from + 40).min(pts.len().saturating_sub(1));
    let mut best = (from, f64::INFINITY);
    for i in from..end {
        let d = seg_dist(pts[i], pts[i + 1], p);
        if d < best.1 - 1e-6 {
            best = (i, d);
        }
    }
    best.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> Arc<RouteGraph> {
        let root = berlin_map_loader::default_data_root();
        // Kreuzberg/Neukölln um Auftraggeber (Kachel 37/30) und Abholort (38/35)
        Arc::new(
            RouteGraph::build(
                &root,
                Some(&|x: u32, y: u32| (35..=40).contains(&x) && (28..=37).contains(&y)),
            )
            .unwrap(),
        )
    }

    #[test]
    fn follows_progress_reroutes_when_leaving_and_ends_at_the_target() {
        let g = graph();
        assert!(!g.is_empty());
        // Auftraggeber → Abholort (rund 3 km)
        let (start, goal) = ((237964., 196126.), (244475., 224520.));
        let mut n = Nav::with_graph(g);
        assert!(n.toggle(goal, 100.));
        assert_eq!(n.step(start, Mode::Foot, 0.), None);
        let r = n.route.clone().expect("Route");
        let total = n.remaining_m(start).unwrap();
        // ein Stück weiter auf der Route: Fortschritt wächst, Restweg sinkt, keine Neuberechnung
        let mid = r.pts[r.pts.len() / 2];
        for &p in &r.pts[..=r.pts.len() / 2] {
            n.step(p, Mode::Foot, 0.1);
        }
        assert!(n.progress > 0);
        assert!(n.remaining_m(mid).unwrap() < total);
        assert_eq!(
            n.route.as_ref().unwrap(),
            &r,
            "auf der Route: keine Neuberechnung"
        );
        // weit weg: neu berechnet (nach der Mindestpause)
        let away = (mid.0 + 2000., mid.1 + 2000.);
        n.step(away, Mode::Foot, 0.2);
        assert_eq!(n.route.as_ref().unwrap(), &r, "Mindestpause");
        n.step(away, Mode::Foot, 1.5);
        assert_ne!(n.route.as_ref().unwrap(), &r, "abgewichen: neue Route");
        // am Ziel: Wegpunkt weg
        assert_eq!(n.step(goal, Mode::Foot, 3.), Some(NavEvent::Arrived));
        assert!(n.waypoint.is_none() && n.route.is_none());
        // erneuter Klick nahe am Wegpunkt entfernt ihn
        assert!(n.toggle(goal, 100.));
        assert!(!n.toggle((goal.0 + 50., goal.1), 100.));
        assert!(n.waypoint.is_none());
        // unerreichbar: einmal gemeldet
        n.toggle((-1e6, -1e6), 100.);
        assert_eq!(n.step(start, Mode::Foot, 5.), Some(NavEvent::NoRoute));
        assert_eq!(n.step(start, Mode::Foot, 7.), None);
    }

    #[test]
    fn clipping_keeps_inside_and_cuts_at_the_edge() {
        let r = [0., 0., 100., 100.];
        assert_eq!(
            clip((10., 10.), (20., 20.), r),
            Some(((10., 10.), (20., 20.)))
        );
        assert_eq!(
            clip((-50., 50.), (50., 50.), r),
            Some(((0., 50.), (50., 50.)))
        );
        assert_eq!(clip((-50., -50.), (-10., 200.), r), None);
        let ((x0, _), (x1, _)) = clip((-100., 50.), (300., 50.), r).unwrap();
        assert!(x0 == 0. && x1 == 100.);
    }

    #[test]
    fn progress_only_moves_forward() {
        let pts = vec![(0., 0.), (100., 0.), (100., 100.), (0., 100.), (0., 10.)];
        // am Ende nahe am Anfang: springt nicht zurück
        assert_eq!(advance(&pts, 3, (0., 5.)), 3);
        assert_eq!(advance(&pts, 0, (100., 50.)), 1);
    }
}
