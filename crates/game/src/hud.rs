//! Anzeigen im Bild (Port der Spielansicht von `hud.js`): Geld und Uhr, Auftrag mit Zeit, Tacho mit Drehzahl und
//! Gang, Fahrzeugname nach dem Einsteigen, Hinweise, Zielpfeil, Briefing, Ergebnis und Ladeanzeige.
//! Koordinaten in Basiseinheiten (720 Zeilen); `Hud` skaliert auf die Fensterhöhe.
use berlin_engine::camera::Camera;
use berlin_engine::hud::{Align, Hud};
use berlin_sim::carmodels::{spec_line, vehicle_name};
use berlin_sim::daylight::format_clock;
use berlin_sim::mission::{self, Outcome, PlayerView, State};
use berlin_sim::soundscape::{EngineState, engine_for};
use berlin_sim::world::{ENTER_DIST, VEH_INFO_S, World};
use glam::Vec2;
use std::f32::consts::PI;

const YELLOW: [f32; 4] = [1., 0.827, 0.239, 1.];
const WHITE: [f32; 4] = [1., 1., 1., 1.];
const GREY: [f32; 4] = [0.8, 0.8, 0.8, 1.];
const GREEN: [f32; 4] = [0.5, 0.88, 0.48, 1.];
const RED: [f32; 4] = [1., 0.36, 0.36, 1.];
const MARGIN: (f32, f32) = (28., 24.);
/// Kantenlänge der Minikarte (Basiseinheiten) und gezeigte Weltbreite (px, ~400 m), wie `hud.js`.
const MINI: f32 = 176.;
const MINI_SPAN: f32 = 4000.;
const DAYS: [&str; 7] = ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"];

pub fn fmt_time(s: f64) -> String {
    let s = s.max(0.);
    format!(
        "{}:{:02}",
        (s / 60.).floor() as i64,
        (s % 60.).floor() as i64
    )
}
fn alpha(c: [f32; 4], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], c[3] * a]
}
fn panel(h: &mut Hud, x: f32, y: f32, w: f32, hh: f32, a: f32) {
    h.rect(x - 1., y - 1., w + 2., hh + 2., [1., 1., 1., 0.12], 11.);
    h.rect(x, y, w, hh, [0.047, 0.055, 0.078, a], 10.);
}
/// Hinweis unten mittig, auf weichem dunklen Grund.
fn prompt(h: &mut Hud, text: &str, y: f32) {
    let cx = h.width / 2.;
    let w = h.text_width(text, 19.);
    h.rect(
        cx - w / 2. - 22.,
        y - 24.,
        w + 44.,
        34.,
        [0., 0., 0., 0.45],
        17.,
    );
    h.text(text, cx, y, 19., WHITE, Align::Center, true);
}

pub fn draw(
    w: &World,
    engine: Option<&EngineState>,
    warn: Option<&str>,
    camera: &Camera,
    viewport: Vec2,
    h: &mut Hud,
) {
    let (mx, my) = MARGIN;
    if w.loading {
        let (bw, bh) = (360., 56.);
        panel(h, h.width / 2. - bw / 2., 360. - bh / 2., bw, bh, 0.85);
        h.text(
            "Lade Stadtteil …",
            h.width / 2.,
            368.,
            22.,
            WHITE,
            Align::Center,
            false,
        );
        return;
    }
    // oben links: Geld, Wochentag und Uhrzeit
    let mw = h.text(
        &format!("{} €", group(w.money as i64)),
        mx,
        my + 26.,
        22.,
        GREEN,
        Align::Left,
        true,
    );
    let night = w.clock >= 1230. || w.clock < 330.;
    let clock = format!(
        "{} {} {}  {} · {:.0} °C",
        if night { "☾" } else { "☀" },
        DAYS[w.day as usize % 7],
        format_clock(w.clock),
        berlin_sim::weather::label(w.sky.kind),
        w.temp
    );
    h.text(
        &clock,
        mx + mw + 16.,
        my + 25.,
        15.,
        if night {
            [0.76, 0.8, 1., 1.]
        } else {
            [1., 0.89, 0.6, 1.]
        },
        Align::Left,
        true,
    );

    let pv = PlayerView {
        x: w.player.x,
        y: w.player.y,
        in_car: w.player.in_car,
    };
    let (objective, target) = w.mission.objective(&w.city.places, pv, &w.cars);
    let ms = w.mission.state;
    // unten links: Minikarte (beim Briefing ausgeblendet)
    if ms != State::Briefing {
        minimap(w, target, mx, 720. - my - MINI - 10., MINI, h);
    }
    // oben rechts: Auftrag und Zeit
    if !objective.is_empty() {
        let r = h.width - mx;
        let lw = h.text("AUFTRAG", r, my + 11., 11., YELLOW, Align::Right, true);
        h.rect(r - lw - 36., my + 5., 26., 3., YELLOW, 0.);
        h.text(objective, r, my + 36., 18., WHITE, Align::Right, true);
        if matches!(ms, State::ToPickup | State::ToDropoff) {
            let t = w.mission.timer;
            let urgent = t < 15.;
            let blink = urgent && (w.time * 4.).floor() as i64 % 2 == 0;
            let tw = h.text(
                &fmt_time(t),
                r,
                my + 68.,
                26.,
                if blink {
                    RED
                } else if urgent {
                    [1., 0.5, 0.5, 1.]
                } else {
                    WHITE
                },
                Align::Right,
                true,
            );
            let label = if ms == State::ToDropoff {
                "Kisten geladen ✓"
            } else {
                "Zeit bis Ladenschluss"
            };
            h.text(label, r - tw - 12., my + 66., 13., GREY, Align::Right, true);
        }
    }
    // unten rechts: Tacho, darüber Fahrzeugname nach dem Einsteigen
    let car = w.player_car();
    if let Some(c) = car {
        let (cx, cy, rad) = (h.width - mx - 56., 720. - my - 68., 54.);
        // Warnschild (Wetter an der Stelle) links vom Tacho
        if let Some(text) = warn {
            let tw = h.text_width(text, 14.).max(100.) + 44.;
            let (x, y) = (cx - rad * 1.3 - tw - 14., cy - 13.);
            let blink = text == "Aquaplaning!" && (w.time * 6.).floor() as i64 % 2 == 0;
            h.rect(
                x,
                y,
                tw,
                26.,
                if blink {
                    [1., 0.31, 0.24, 0.88]
                } else {
                    [1., 0.75, 0.16, 0.88]
                },
                4.,
            );
            h.text(
                "⚠",
                x + 12.,
                y + 19.,
                14.,
                [0.1, 0.1, 0.1, 1.],
                Align::Left,
                false,
            );
            h.text(
                text,
                x + 32.,
                y + 18.,
                14.,
                [0.1, 0.1, 0.1, 1.],
                Align::Left,
                false,
            );
        }
        h.ellipse(cx, cy, rad * 1.3, rad * 1.3, [0., 0., 0., 0.32]);
        let (a0, span) = (PI * 0.75, PI * 1.5);
        let kmh = (c.speed() * 0.36).round();
        let e = engine_for(c);
        let (red, red_from) = (e.red, if e.electric { 1.01 } else { 0.86 });
        let f = engine
            .map(|s| (s.rpm / red).clamp(0., 1.) as f32)
            .unwrap_or(0.);
        h.arc(cx, cy, rad - 2.5, 5., a0, a0 + span, [1., 1., 1., 0.14]);
        if red_from < 1. {
            h.arc(
                cx,
                cy,
                rad - 2.5,
                5.,
                a0 + span * red_from,
                a0 + span,
                [1., 0.27, 0.22, 0.6],
            );
        }
        let hot = f >= red_from;
        if f > 0.001 {
            h.arc(
                cx,
                cy,
                rad - 2.5,
                5.,
                a0,
                a0 + span * f,
                if hot { [1., 0.31, 0.25, 1.] } else { WHITE },
            );
        }
        // Striche je 1000 U/min (ab 9500 je 2000)
        let step = if red > 9500. { 2000. } else { 1000. };
        let mut r = 0.;
        while r <= red + 1. {
            let q = (r / red) as f32;
            let a = a0 + span * q;
            let hot = q >= red_from;
            let col = if hot {
                [1., 0.47, 0.41, 0.9]
            } else {
                [1., 1., 1., 0.6]
            };
            h.arc(cx, cy, rad - 11., 6., a - 0.012, a + 0.012, col);
            r += step;
        }
        h.text(
            &format!("{kmh}"),
            cx,
            cy + 11.,
            32.,
            WHITE,
            Align::Center,
            true,
        );
        h.text("KM/H", cx, cy + 25., 9., GREY, Align::Center, true);
        if let Some(s) = engine {
            let gear = if s.reverse {
                "R".into()
            } else if s.electric {
                "D".into()
            } else {
                s.gear.to_string()
            };
            h.text(
                &gear,
                cx,
                cy + rad - 1.,
                17.,
                if s.reverse {
                    [0.6, 0.82, 1., 1.]
                } else if hot {
                    [1., 0.42, 0.35, 1.]
                } else {
                    YELLOW
                },
                Align::Center,
                true,
            );
        }
        // Zustand: Bogen innen unten links, nur bei Schaden
        let hp = (c.health / 100.).clamp(0., 1.) as f32;
        if hp < 0.999 {
            h.arc(
                cx,
                cy,
                rad + 7.,
                2.5,
                a0,
                a0 + span * 0.25,
                [0., 0., 0., 0.5],
            );
            let col = if hp > 0.6 {
                [0.37, 0.83, 0.37, 1.]
            } else if hp > 0.3 {
                [0.91, 0.77, 0.25, 1.]
            } else {
                [0.91, 0.28, 0.24, 1.]
            };
            if hp > 0. {
                h.arc(cx, cy, rad + 7., 2.5, a0, a0 + span * 0.25 * hp, col);
            }
        }
        // kleine Anzeigen links vom Tacho
        let mut tags: Vec<(&str, [f32; 4])> = Vec::new();
        if c.wrecked {
            tags.push(("SCHROTT", [1., 0.42, 0.35, 1.]));
        }
        if c.cargo {
            tags.push(("▣ KISTEN", [0.9, 0.7, 0.38, 1.]));
        }
        if let Some(d) = &c.dyn_state {
            let regulating = w.esp && d.esp > 0. && (w.time * 8.).floor() as i64 % 2 == 0;
            let warn = [1., 0.69, 0.13, 1.];
            let ok = [0.56, 0.83, 0.61, 1.];
            tags.push((
                if regulating {
                    "ESP REGELT"
                } else if w.esp {
                    "ESP AN"
                } else {
                    "ESP AUS"
                },
                if regulating || !w.esp { warn } else { ok },
            ));
            tags.push((
                if w.abs { "ABS AN" } else { "ABS AUS" },
                if w.abs { ok } else { warn },
            ));
        }
        let lx = cx - rad - 14.;
        let n = tags.len();
        for (i, (t, col)) in tags.into_iter().enumerate() {
            h.text(
                t,
                lx,
                cy + rad - 4. - (n - 1 - i) as f32 * 17.,
                13.,
                col,
                Align::Right,
                true,
            );
        }
        // Fahrzeugname (blendet ein und aus)
        if let Some((id, t)) = w.veh_info
            && id == c.id
        {
            let t = t as f32;
            let a = (t / 0.35).min((VEH_INFO_S as f32 - t) / 0.9).clamp(0., 1.);
            if a > 0.01 {
                let x = h.width - mx + (1. - (t / 0.35).min(1.)) * 24.;
                let y = cy - rad - 26.;
                let model = c.model_name();
                h.text(
                    &spec_line(model).to_uppercase(),
                    x,
                    y,
                    12.,
                    alpha(GREY, a),
                    Align::Right,
                    true,
                );
                h.text(
                    &vehicle_name(model),
                    x,
                    y - 20.,
                    30.,
                    alpha(WHITE, a),
                    Align::Right,
                    true,
                );
            }
        }
    }
    // Zielpfeil: über dem Ziel, wenn es im Bild ist, sonst am Bildrand mit Entfernung
    if let Some((tx, ty)) = target
        && !matches!(ms, State::Briefing | State::Success | State::Failed)
    {
        let s = camera.world_to_screen(Vec2::new(tx as f32, ty as f32), 0., viewport) / h.scale;
        let src = car.map(|c| (c.x, c.y)).unwrap_or((w.player.x, w.player.y));
        let meters = ((tx - src.0).hypot(ty - src.1) / 10.).round();
        let (l, rr, top, bottom) = (mx + 40., h.width - mx - 40., my + 120., 720. - my - 160.);
        if s.x > l && s.x < rr && s.y > top && s.y < bottom {
            let bob = (w.time * 5.).sin() as f32 * 5.;
            h.triangle(s.x, s.y - 40. + bob, 12., PI / 2., [0., 0., 0., 0.8]);
            h.triangle(s.x, s.y - 41. + bob, 10., PI / 2., YELLOW);
        } else {
            let (cx, cy) = (h.width / 2., 360.);
            let a = (s.y - cy).atan2(s.x - cx);
            let (ux, uy) = (a.cos(), a.sin());
            let kx = if ux > 0. {
                (rr - cx) / ux
            } else if ux < 0. {
                (l - cx) / ux
            } else {
                f32::INFINITY
            };
            let ky = if uy > 0. {
                (bottom - cy) / uy
            } else if uy < 0. {
                (top - cy) / uy
            } else {
                f32::INFINITY
            };
            let k = kx.min(ky);
            let (ax, ay) = (cx + ux * k, cy + uy * k);
            h.triangle(ax, ay, 16., a, [0., 0., 0., 0.8]);
            h.triangle(ax, ay, 13., a, YELLOW);
            h.text(
                &format!("{meters} m"),
                ax - ux * 34.,
                ay - uy * 30. + 6.,
                16.,
                WHITE,
                Align::Center,
                true,
            );
        }
    }
    // Hinweise unten mittig
    let mut hint: Option<String> = w
        .mission
        .prompt
        .map(|p| p.replacen("A ", "E ", 1).replacen("A:", "E:", 1));
    let mut hint_y = 720. - my - 24.;
    if hint.is_none()
        && let Some(n) = &w.notice
    {
        hint = Some(n.text.clone());
        hint_y = 720. - my - 150.;
    }
    if hint.is_none()
        && car.is_none()
        && w.cars
            .iter()
            .any(|c| !c.wrecked && (c.x - w.player.x).hypot(c.y - w.player.y) < ENTER_DIST)
    {
        hint = Some("F: Einsteigen".into());
    }
    if hint.is_none()
        && let Some(c) = car
    {
        if c.wrecked {
            hint = Some("F: Aussteigen – das Auto ist Schrott".into());
        } else if c.speed() < 20. && w.veh_info.is_some() {
            hint = Some("F: Aussteigen".into());
        }
    }
    if let Some(t) = hint
        && !matches!(ms, State::Briefing | State::Success | State::Failed)
    {
        prompt(h, &t, hint_y);
    }
    if w.mission.load > 0. && ms == State::ToPickup {
        let (bw, x, y) = (260., h.width / 2. - 130., 720. - my - 66.);
        h.rect(x - 1., y - 1., bw + 2., 8., [0., 0., 0., 0.6], 0.);
        h.rect(
            x,
            y,
            bw * (w.mission.load / mission::LOAD_TIME).min(1.) as f32,
            6.,
            YELLOW,
            0.,
        );
    }
    match ms {
        State::Briefing => {
            let (bw, bh) = (820., 210.);
            let (x, y) = (h.width / 2. - bw / 2., 720. - my - bh - 60.);
            panel(h, x, y, bw, bh, 0.85);
            for (i, line) in mission::BRIEFING.iter().enumerate() {
                let (size, col) = if i == 0 { (18., YELLOW) } else { (19., WHITE) };
                h.text(
                    line,
                    x + 28.,
                    y + 40. + i as f32 * 32.,
                    size,
                    col,
                    Align::Left,
                    false,
                );
            }
            h.text(
                "E: Auftrag starten",
                x + bw - 28.,
                y + bh - 21.,
                18.,
                YELLOW,
                Align::Right,
                false,
            );
        }
        State::Success | State::Failed => {
            h.rect(0., 0., h.width, 720., [0., 0., 0., 0.55], 0.);
            let cx = h.width / 2.;
            match &w.mission.result {
                Some(Outcome::Success {
                    time,
                    reward,
                    bonus,
                    damage_penalty,
                    new_best,
                    ..
                }) => {
                    h.text(
                        "AUFTRAG ERFÜLLT",
                        cx,
                        190.,
                        56.,
                        [0.44, 0.88, 0.42, 1.],
                        Align::Center,
                        true,
                    );
                    let lines = [
                        format!(
                            "Zeit: {}{}",
                            fmt_time(*time),
                            if *new_best {
                                "   ★ Neue Bestzeit"
                            } else {
                                ""
                            }
                        ),
                        format!(
                            "Lohn {} €  +  Zeitbonus {} €  -  Schaden {} €",
                            mission::REWARD,
                            bonus,
                            damage_penalty
                        ),
                        format!("Ausgezahlt: {reward} €"),
                    ];
                    for (i, l) in lines.iter().enumerate() {
                        let (size, col) = if i == 2 {
                            (26., [0.56, 0.89, 0.53, 1.])
                        } else {
                            (20., [0.93, 0.93, 0.93, 1.])
                        };
                        h.text(l, cx, 250. + i as f32 * 34., size, col, Align::Center, true);
                    }
                    h.text(
                        "Fortschritt wird automatisch gespeichert.",
                        cx,
                        400.,
                        15.,
                        GREY,
                        Align::Center,
                        true,
                    );
                }
                Some(Outcome::Failed { reason }) => {
                    h.text(
                        "AUFTRAG GESCHEITERT",
                        cx,
                        190.,
                        56.,
                        RED,
                        Align::Center,
                        true,
                    );
                    h.text(reason, cx, 250., 21., WHITE, Align::Center, true);
                }
                None => {}
            }
            h.text("E: weiter", cx, 460., 22., YELLOW, Align::Center, true);
        }
        _ => {}
    }
}

/// Tausenderpunkte wie `toLocaleString('de-DE')`.
pub fn group(v: i64) -> String {
    let s = v.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(ch);
    }
    if v < 0 { format!("-{out}") } else { out }
}

/// Minikarte (`hud.js drawMinimap`): Kartenmeshes als Ausschnitt um Spieler bzw. eigenes Auto, darüber andere Autos,
/// das Ziel (am Rand festgehalten), der Spielerpfeil und ein „N“.
pub fn minimap(w: &World, target: Option<(f64, f64)>, x: f32, y: f32, size: f32, h: &mut Hud) {
    let pc = w.player_car();
    let (px, py, angle) = pc.map_or((w.player.x, w.player.y, w.player.angle), |c| {
        (c.x, c.y, c.angle)
    });
    h.rect(x, y, size, size, [0.227, 0.239, 0.267, 1.], 6.);
    h.map_inset(x, y, size, size, [px as f32, py as f32], MINI_SPAN);
    let k = size / MINI_SPAN;
    let (cx, cy) = (x + size / 2., y + size / 2.);
    let to_mini = |wx: f64, wy: f64| (cx + (wx - px) as f32 * k, cy + (wy - py) as f32 * k);
    let inside =
        |mx: f32, my: f32| mx > x + 2. && mx < x + size - 2. && my > y + 2. && my < y + size - 2.;
    for c in &w.cars {
        if pc.is_some_and(|p| p.id == c.id) {
            continue;
        }
        let (mx, my) = to_mini(c.x, c.y);
        if !inside(mx, my) {
            continue;
        }
        let color = if c.cargo {
            [0.878, 0.69, 0.376, 1.]
        } else if Some(c.id) == w.player_car_id {
            [1., 0.478, 0.478, 1.]
        } else {
            [0.82, 0.82, 0.82, 0.55]
        };
        h.rect(mx - 1.5, my - 1.5, 3., 3., color, 0.);
    }
    if let Some((tx, ty)) = target {
        let (mut mx, mut my) = to_mini(tx, ty);
        let lim = size / 2. - 9.;
        let f = (mx - cx).abs().max((my - cy).abs()) / lim;
        if f > 1. {
            mx = cx + (mx - cx) / f;
            my = cy + (my - cy) / f;
        }
        h.ellipse(mx, my, 7., 7., [0., 0., 0., 1.]);
        h.ellipse(mx, my, 5.5, 5.5, YELLOW);
    }
    // Spielerpfeil mit dunklem Rand
    let a = angle as f32;
    h.triangle(cx, cy, 9., a, [0., 0., 0., 1.]);
    h.triangle(cx, cy, 7., a, WHITE);
    // Rahmen
    let edge = [0., 0., 0., 0.7];
    h.line(x - 1., y - 1., x + size + 1., y - 1., 2.5, edge);
    h.line(
        x - 1.,
        y + size + 1.,
        x + size + 1.,
        y + size + 1.,
        2.5,
        edge,
    );
    h.line(x - 1., y - 1., x - 1., y + size + 1., 2.5, edge);
    h.line(
        x + size + 1.,
        y - 1.,
        x + size + 1.,
        y + size + 1.,
        2.5,
        edge,
    );
    h.text(
        "N",
        cx,
        y + 15.,
        12.,
        [0.9, 0.9, 0.9, 1.],
        Align::Center,
        true,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formatting() {
        assert_eq!(fmt_time(125.9), "2:05");
        assert_eq!(fmt_time(-3.), "0:00");
        assert_eq!(group(1234567), "1.234.567");
        assert_eq!(group(500), "500");
        assert_eq!(group(-1000), "-1.000");
    }
    #[test]
    fn minimap_centres_on_the_player_and_marks_the_target() {
        use berlin_sim::city::{City, DiskSource};
        let root = berlin_map_loader::default_data_root();
        let city = City::open(&root, Box::new(DiskSource::new(root.clone()))).unwrap();
        let w = World::new(city, 3, 8, 0);
        let mut h = Hud::new([1280., 720.]);
        h.text("davor", 10., 10., 12., WHITE, Align::Left, false);
        let before = h.items.len() as u32;
        // Ziel weit weg: wird am Rand festgehalten
        let target = (w.player.x + 50_000., w.player.y);
        minimap(&w, Some(target), 40., 500., MINI, &mut h);
        let m = h.map.expect("Kartenausschnitt");
        assert_eq!(m.center, [w.player.x as f32, w.player.y as f32]);
        assert_eq!(m.span, MINI_SPAN);
        assert_eq!(m.rect, [40., 500., MINI, MINI]);
        assert_eq!(
            m.split,
            before + 1,
            "Hintergrund unter, Markierungen über der Karte"
        );
        let yellow = h
            .items
            .iter()
            .find(|i| i.color == YELLOW)
            .expect("Zielpunkt");
        assert!(yellow.center[0] <= 40. + MINI && yellow.center[0] > 40. + MINI / 2.);
    }
}
