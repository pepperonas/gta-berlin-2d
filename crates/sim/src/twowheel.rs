//! Zweiräder (Fahrphysik Phase 5): eigenes Modell ohne Reifen-Schräglauf. Fahrrad, E-Scooter, Roller und
//! Motorrad fahren kinematisch – die Bahn folgt der Schräglage:
//!
//! - Lenkung gibt eine Wunschkrümmung κ vor, daraus die Wunsch-Schräglage φ = atan(v²·κ/g), begrenzt durch
//!   `max_schraeglage` (Bodenfreiheit). Die Schräglage folgt mit begrenzter Rate (Einlenken braucht bei Tempo Zeit,
//!   wie Gegenlenken in echt); gefahren wird die Krümmung der tatsächlichen Schräglage g·tan(φ)/v². Langsam (unter
//!   4 m/s, Füße am Boden) fährt das Rad direkt der Lenkung nach.
//! - Haftgrenze: braucht die Schräglage mehr als atan(μ) (neben der Längskraft, Reibungskreis) → Lowsider.
//! - Wheelie, wenn die Beschleunigung g·l_h/h überschreitet (l_h = Schwerpunkt bis Hinterachse), Stoppie beim
//!   Bremsen über g·l_v/h. Wheelie-Control hält beides an der Grenze, aber spielbar (kurze Wheelies beim
//!   Ampelstart sind erwünscht). Ohne Kontrolle überschlägt sich das Rad über `FLIP`.
//! - Bremsen: die Vorderbremse trägt den Großteil, begrenzt durch Überschlag und Haftung; ABS nur, wo vorhanden.
//!   Ein blockiertes Vorderrad stürzt (außer langsam und geradeaus).
//! - Fahrrad: Fahrerleistung (Dauer, Sprint per Taste mit Ausdauer), Kraftgrenze aus der Muskel-Kurve.
//! - Motorrad: Gas in Schräglage auf losem Untergrund lässt das Heck leicht und kontrollierbar ausbrechen.
//! - Berlin: flach gequerte Straßenbahnschiene (`Ground::groove`) und Bordsteine an kleinen Rädern werfen ab.
//!
//! Zustand und Koordinaten wie `vphys` (Meter, Gierwinkel gegen den Uhrzeigersinn, Lenkung positiv = links).
use crate::vehdata::{Feel, G, Power, RHO, Vehicle};
use crate::vphys::{
    AQUA_REST, Env, HZ, Input, State, aquaplaning_grip, aquaplaning_speed, drive_force,
    mass_factor, shift, smooth, steer_limit,
};

/// Warum ein Zweirad gestürzt ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fall {
    /// Haftgrenze in Schräglage überschritten
    Lowside,
    /// blockiertes Vorderrad
    FrontLock,
    /// Wheelie oder Stoppie überschlagen
    Flip,
    /// in die Straßenbahnrille gerutscht
    Rail,
    /// kleines Rad am Bordstein
    Curb,
}
impl Fall {
    pub fn label(self) -> &'static str {
        match self {
            Fall::Lowside => "Weggerutscht!",
            Fall::FrontLock => "Vorderrad blockiert!",
            Fall::Flip => "Überschlagen!",
            Fall::Rail => "In die Schiene geraten!",
            Fall::Curb => "Am Bordstein gestürzt!",
        }
    }
}

/// Schräglagenrate (rad/s) langsam bzw. ihre Abnahme mit dem Tempo (m/s)
pub const LEAN_RATE: f64 = 1.6;
pub const LEAN_RATE_V: f64 = 25.;
/// Unter diesem Tempo fährt das Rad der Lenkung direkt nach (Füße am Boden), darüber über die Schräglage
pub const KIN: [f64; 2] = [3., 6.];
/// Haftgrenze: so viel Reserve, bevor es wegrutscht
pub const LOWSIDE_MARGIN: f64 = 1.03;
/// Nickwinkel (rad): Wheelie-Control hält hier; ohne Kontrolle überschlägt es sich ab `FLIP`
pub const WHEELIE_CONTROL: f64 = 0.17;
pub const FLIP: f64 = 0.8;
/// Nickrate je g Überschuss (rad/s) und Zurückfallen (rad/s)
pub const PITCH_RATE: f64 = 2.5;
pub const PITCH_BACK: f64 = 1.2;
/// Sprint: so lange (s) reicht die Puste, so lange dauert die Erholung; erschöpft erst ab diesem Rest wieder
pub const SPRINT_S: f64 = 8.;
pub const RECOVER_S: f64 = 15.;
pub const RESTART: f64 = 0.4;
/// Kopfstein schüttelt Zweiräder: Haftverlust je Unebenheit
pub const ROUGH_GRIP: f64 = 0.15;
/// Heckausbruch beim Motorrad auf losem Untergrund (rad/s bei Vollgas, voller Schräglage, μ_rel 0)
pub const SLIDE_YAW: f64 = 0.6;
/// Kleines Rad (Radius in m): am Bordstein ab diesem Tempo (m/s) Sturz, darunter harter Halt
pub const SMALL_WHEEL: f64 = 0.2;
pub const CURB_FALL_V: f64 = 3.;
/// gestürztes Zweirad rutscht mit diesem Anteil der Haftung
pub const SLIDE_MU: f64 = 0.45;

/// Ein Schritt über `dt` (intern 120 Hz).
pub fn step(v: &Vehicle, feel: &Feel, s: &mut State, inp: &Input, env: &Env, dt: f64) {
    let mut left = dt;
    let h = 1. / HZ;
    while left > 1e-9 {
        let d = h.min(left);
        substep(v, feel, s, inp, env, d);
        left -= d;
    }
}

fn hash01(x: f64) -> f64 {
    let z = (x * 12.9898 + 78.233).sin() * 43758.5453;
    z - z.floor()
}

/// Größte Schräglage, die die Haftung trägt (rad), bei Längsbeschleunigung `ax` (m/s²).
pub fn lean_limit(mu: f64, ax: f64) -> f64 {
    let lat = (mu * mu - (ax / G).powi(2)).max(0.).sqrt();
    lat.atan()
}

fn substep(v: &Vehicle, feel: &Feel, s: &mut State, inp: &Input, env: &Env, dt: f64) {
    let (m, h) = v.loaded(s.load);
    let l = v.wheelbase;
    // Schwerpunkt bis Vorder- bzw. Hinterachse
    let (a, b) = v.axle_distances();
    let speed = s.vx.max(0.);
    if s.fallen.is_some() {
        // liegt und rutscht aus
        let dv = SLIDE_MU * v.tire.mu * G * dt;
        let sp = s.vx.hypot(s.vy);
        if sp > dv {
            s.vx -= s.vx / sp * dv;
            s.vy -= s.vy / sp * dv;
        } else {
            (s.vx, s.vy) = (0., 0.);
        }
        s.r *= (-3. * dt).exp();
        s.lean = s.lean.signum() * std::f64::consts::FRAC_PI_2;
        integrate(s, dt);
        return;
    }
    // Haftung je Rad (vorn, hinten): Untergrund, Kopfstein-Schütteln, Aquaplaning
    let v_ap = aquaplaning_speed(
        v.tire_kpa,
        if s.tread > 0. { s.tread } else { 1. },
        v.tire_width_mm,
    );
    let mu_at = |k: usize| {
        let g = env.wheel[k];
        let aq = 1. - (1. - aquaplaning_grip(speed, v_ap, g.water_mm)) * feel.aqua();
        (v.tire.mu * g.grip() * feel.grip() * (1. - ROUGH_GRIP * g.rough) * aq)
            .max(feel.ice_grip_min)
    };
    let (mu_f, mu_r) = (mu_at(0), mu_at(2));
    for (i, k) in [(0, 0), (1, 2)] {
        let g = env.wheel[k];
        let aq = aquaplaning_grip(speed, v_ap, g.water_mm);
        s.aqua[i] = ((1. - aq) / (1. - AQUA_REST)).clamp(0., 1.);
    }
    let mu = mu_f.min(mu_r);
    // Berlin: Straßenbahnrille flach gequert (einmal je Schiene), kleines Rad am Bordstein
    let groove = env.wheel[0].groove;
    if groove > 0. && speed > 1. {
        if !s.groove_seen {
            s.groove_seen = true;
            if hash01(s.dist.floor() + v.wheelbase * 100.) < groove {
                fall(s, Fall::Rail);
                return;
            }
        }
    } else {
        s.groove_seen = false;
    }
    if env.wheel[0].curb > 0. && speed > 0.5 {
        if v.wheel_r < SMALL_WHEEL {
            if speed > CURB_FALL_V {
                fall(s, Fall::Curb);
                return;
            }
            s.vx = 0.;
        } else {
            s.vx -= (0.3 * env.wheel[0].curb * speed).min(speed * 0.2);
            s.pitch += 0.05;
        }
        s.curb_hits += 1;
    }
    // Antrieb: Fahrrad sprintet mit Ausdauer
    let muscle = matches!(v.engine.power, Power::Muscle { .. });
    let mut di = *inp;
    if muscle {
        let want = inp.sprint && inp.throttle > 0.05;
        if s.tired && s.exertion < RESTART {
            s.tired = false;
        }
        di.sprint = want && !s.tired;
        if di.sprint {
            s.exertion = (s.exertion + dt / SPRINT_S).min(1.);
            if s.exertion >= 1. {
                s.tired = true;
            }
        } else {
            s.exertion = (s.exertion - dt / RECOVER_S).max(0.);
        }
    } else {
        di.sprint = false;
    }
    s.reverse = false;
    shift(v, s, &di, speed, dt);
    let f_drive = drive_force(v, s, &di, speed, dt).max(0.);
    let df = mass_factor(v, s);
    let f_drag = 0.5 * RHO * v.cw_a * speed * speed;
    let cr = v.tire.rolling + (env.wheel[0].rolling_extra + env.wheel[2].rolling_extra) / 2.;
    let f_roll = if speed > 0.05 { cr * m * G } else { 0. };
    // Hinterrad: Antrieb bis zur Haftung (Last unter Beschleunigung)
    let rear_load = m * G * a / l;
    let drive_cap = mu_r * v.tire.mu_long * (rear_load + m * h / l * (f_drive / m).min(G));
    let mut f_drive = f_drive.min(drive_cap.max(0.));
    // Wheelie: über g·l_h/h hebt das Vorderrad; Wheelie-Control hält knapp an der Grenze
    let a_wheelie = G * b / h;
    let a_drive = (f_drive - f_drag - f_roll) / (m * df);
    if a_drive > a_wheelie {
        let excess = (a_drive - a_wheelie) / G;
        s.pitch += excess * PITCH_RATE * dt;
        if v.wheelie_control && s.pitch > WHEELIE_CONTROL {
            s.pitch = WHEELIE_CONTROL;
            f_drive = (a_wheelie * m * df + f_drag + f_roll).min(f_drive);
        }
    } else if s.pitch > 0. {
        s.pitch = (s.pitch - PITCH_BACK * dt).max(0.);
    }
    // Bremsen: Druckaufbau, Vorderbremse bis Überschlag (g·l_v/h) und Haftung, den Rest hinten
    let bt = v.brake.build.max(0.02);
    s.brake_p += (inp.brake.clamp(0., 1.) - s.brake_p).clamp(-dt / 0.05, dt / bt);
    let f_brake = s.brake_p * m * G * v.brake.gain * feel.brake();
    let front_load = m * G * b / l;
    let decel_est = (f_brake / m).min(G * 1.5);
    let front_grip = mu_f * v.tire.mu_long * (front_load + m * h / l * decel_est);
    let abs = v.brake.abs && !inp.no_abs;
    let want_front = f_brake * v.brake.front.max(0.6);
    let a_stoppie = G * a / h;
    let stoppie_cap = m * a_stoppie;
    let mut f_front = want_front;
    if abs {
        f_front = f_front.min(front_grip * 0.97).min(stoppie_cap * 1.02);
    } else if f_front > front_grip && speed > 2. {
        // ohne ABS: blockiertes Vorderrad – langsam und geradeaus geht es noch gut
        if speed > 6. || s.lean.abs() > 0.1 {
            fall(s, Fall::FrontLock);
            return;
        }
        f_front = front_grip * v.tire.slide_ratio;
    }
    let rear_brake_load = (m * G * a / l - m * h / l * decel_est).max(m * G * 0.05);
    let f_rear = (f_brake - want_front)
        .max(0.)
        .min(mu_r * v.tire.mu_long * rear_brake_load);
    let f_brake_total = if speed > 0.05 { f_front + f_rear } else { 0. };
    // Stoppie: über g·l_v/h hebt das Heck
    let decel = f_brake_total / m;
    if decel > a_stoppie {
        s.pitch -= (decel - a_stoppie) / G * PITCH_RATE * dt;
        if abs && s.pitch < -WHEELIE_CONTROL {
            s.pitch = -WHEELIE_CONTROL;
        }
    } else if s.pitch < 0. {
        s.pitch = (s.pitch + PITCH_BACK * dt).min(0.);
    }
    if s.pitch.abs() > FLIP {
        fall(s, Fall::Flip);
        return;
    }
    // Motorbremse beim Gaswegnehmen
    let coast = if inp.throttle < 0.05 && speed > 1. && !muscle {
        m * G * 0.02
    } else {
        0.
    };
    let ax = (f_drive - f_drag - f_roll - f_brake_total - coast) / (m * df.max(1.));
    s.vx = (s.vx + ax * dt).max(0.);
    // gebremst fast im Stand: steht
    if s.brake_p > 0.05 && s.vx < 0.1 && f_drive <= 0. {
        s.vx = 0.;
    }
    let speed = s.vx;
    // Lenkung → Wunschkrümmung → Wunsch-Schräglage, Rate begrenzt
    let lim = steer_limit(v, speed);
    let delta_cmd = inp.steer.clamp(-1., 1.) * lim;
    let rate = v.steering.delta_max / 0.25 * v.steering.rate_factor / (1. + speed / 30.);
    s.delta += (delta_cmd - s.delta).clamp(-rate * dt, rate * dt);
    let kappa_cmd = s.delta.tan() / l;
    let lean_want = (speed * speed * kappa_cmd / G)
        .atan()
        .clamp(-v.max_lean, v.max_lean);
    let lean_rate = LEAN_RATE / (1. + speed / LEAN_RATE_V);
    s.lean += (lean_want - s.lean).clamp(-lean_rate * dt, lean_rate * dt);
    // Haftgrenze in Schräglage (Reibungskreis mit der Längskraft)
    if speed > KIN[0] && s.lean.abs() > lean_limit(mu, ax) * LOWSIDE_MARGIN {
        fall(s, Fall::Lowside);
        return;
    }
    let kappa_lean = G * s.lean.tan() / (speed * speed).max(1.);
    let w = smooth(KIN[0], KIN[1], speed);
    let kappa = kappa_cmd * (1. - w) + kappa_lean * w;
    s.r = speed * kappa;
    s.vy = 0.;
    // Motorrad auf losem Untergrund: Gas in Schräglage lässt das Heck leicht kommen
    if !muscle && inp.throttle > 0.2 && speed > 5. {
        let loose =
            (1. - env.wheel[2].grip()).clamp(0., 1.) * smooth(0.3, 0.6, 1. - env.wheel[2].grip());
        if loose > 0. {
            let yaw =
                SLIDE_YAW * inp.throttle * (s.lean.sin() / v.max_lean.sin()).clamp(-1., 1.) * loose;
            s.r += yaw;
            s.vy = -yaw * speed * 0.15;
        }
    }
    s.ax_f += (ax - s.ax_f) * (dt / 0.1).min(1.);
    s.ay_f = s.vx * s.r;
    s.fz = [front_load, 0., rear_load, 0.];
    integrate(s, dt);
}

fn integrate(s: &mut State, dt: f64) {
    let (sy, cy) = s.yaw.sin_cos();
    s.x += (s.vx * cy - s.vy * sy) * dt;
    s.y += (s.vx * sy + s.vy * cy) * dt;
    s.yaw += s.r * dt;
    s.dist += s.vx.hypot(s.vy) * dt;
}

fn fall(s: &mut State, why: Fall) {
    s.fallen = Some(why);
    s.lean = if s.lean >= 0. {
        std::f64::consts::FRAC_PI_2
    } else {
        -std::f64::consts::FRAC_PI_2
    };
    s.pitch = 0.;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vehdata::{Esp, VehicleDb};
    use crate::vphys::{Env, Ground, Input, State, step as vstep};

    fn db() -> VehicleDb {
        VehicleDb::embedded().unwrap()
    }
    fn run(v: &Vehicle, s: &mut State, inp: Input, env: &Env, secs: f64) {
        let feel = Feel::simulation();
        for _ in 0..(secs * HZ) as usize {
            vstep(v, &feel, s, &inp, env, 1. / HZ);
            if s.fallen.is_some() {
                break;
            }
        }
    }

    #[test]
    fn lean_follows_steering_with_a_rate_and_its_limit() {
        let db = db();
        let v = db.get("motorrad_naked").unwrap();
        let mut s = State {
            vx: 20.,
            gear: 3,
            ..Default::default()
        };
        let inp = Input {
            throttle: 0.3,
            steer: 0.6,
            esp: Some(Esp::Off),
            ..Default::default()
        };
        run(v, &mut s, inp, &Env::default(), 0.1);
        let early = s.lean;
        run(v, &mut s, inp, &Env::default(), 2.);
        assert!(s.fallen.is_none(), "{:?}", s.fallen);
        assert!(early > 0. && early < s.lean, "{early} {}", s.lean);
        // Kurvenfahrt entspricht der Schräglage: a_quer = g·tan(φ)
        let ay = s.vx * s.r;
        assert!(
            (ay - G * s.lean.tan()).abs() < 0.5,
            "{ay} {}",
            G * s.lean.tan()
        );
        // nie über die Bodenfreiheit
        assert!(s.lean <= v.max_lean + 1e-9);
    }

    #[test]
    fn too_much_lean_on_wet_cobbles_lowsides() {
        let db = db();
        let v = db.get("motorrad_naked").unwrap();
        let wet = Env::uniform(Ground {
            mu_rel: 0.5,
            ..Ground::DRY
        });
        let mut s = State {
            vx: 18.,
            gear: 3,
            ..Default::default()
        };
        run(
            v,
            &mut s,
            Input {
                throttle: 0.3,
                steer: 1.,
                ..Default::default()
            },
            &wet,
            3.,
        );
        assert_eq!(s.fallen, Some(Fall::Lowside));
        // trocken und sanft: kein Sturz
        let mut s = State {
            vx: 18.,
            gear: 3,
            ..Default::default()
        };
        run(
            v,
            &mut s,
            Input {
                throttle: 0.3,
                steer: 0.3,
                ..Default::default()
            },
            &Env::default(),
            3.,
        );
        assert!(s.fallen.is_none());
    }

    #[test]
    fn superbike_wheelies_under_control_others_without_it_flip() {
        let db = db();
        let sb = db.get("superbike").unwrap();
        assert!(sb.wheelie_control);
        let mut s = State::default();
        let gas = Input {
            throttle: 1.,
            ..Default::default()
        };
        let mut peak: f64 = 0.;
        let feel = Feel::simulation();
        for _ in 0..(3. * HZ) as usize {
            vstep(sb, &feel, &mut s, &gas, &Env::default(), 1. / HZ);
            peak = peak.max(s.pitch);
        }
        assert!(s.fallen.is_none());
        assert!(peak > 0.05 && peak <= WHEELIE_CONTROL + 1e-9, "{peak}");
        // ohne Kontrolle: Vollgas aus dem Stand überschlägt sich
        let mut wild = sb.clone();
        wild.wheelie_control = false;
        let mut s = State::default();
        run(&wild, &mut s, gas, &Env::default(), 4.);
        assert_eq!(s.fallen, Some(Fall::Flip));
    }

    #[test]
    fn front_brake_does_most_without_abs_a_locked_front_crashes() {
        let db = db();
        let v = db.get("roller_45").unwrap();
        assert!(!v.brake.abs);
        let mut s = State {
            vx: 12.,
            ..Default::default()
        };
        // nasser Belag + Vollbremsung ohne ABS: Vorderrad blockiert
        let wet = Env::uniform(Ground {
            mu_rel: 0.5,
            ..Ground::DRY
        });
        run(
            v,
            &mut s,
            Input {
                brake: 1.,
                ..Default::default()
            },
            &wet,
            3.,
        );
        assert_eq!(s.fallen, Some(Fall::FrontLock));
        // mit ABS (Naked Bike) bleibt es stehen, ohne zu stürzen
        let abs = db.get("motorrad_naked").unwrap();
        let mut s = State {
            vx: 20.,
            gear: 3,
            ..Default::default()
        };
        run(
            abs,
            &mut s,
            Input {
                brake: 1.,
                ..Default::default()
            },
            &wet,
            6.,
        );
        assert!(s.fallen.is_none() && s.vx == 0.);
    }

    #[test]
    fn bicycle_sprint_runs_out_of_breath_and_recovers() {
        let db = db();
        let v = db.get("fahrrad_city").unwrap();
        let mut s = State::default();
        let sprint = Input {
            throttle: 1.,
            sprint: true,
            ..Default::default()
        };
        run(v, &mut s, sprint, &Env::default(), SPRINT_S + 1.);
        assert!(s.tired && s.exertion > 0.9);
        let tired_speed = {
            let mut t = s.clone();
            run(v, &mut t, sprint, &Env::default(), 4.);
            t.vx
        };
        // ausgeruht sprintet es schneller als erschöpft
        let mut fresh = State {
            vx: s.vx,
            ..Default::default()
        };
        run(v, &mut fresh, sprint, &Env::default(), 4.);
        assert!(fresh.vx > tired_speed, "{} {tired_speed}", fresh.vx);
        // Erholung
        run(v, &mut s, Input::default(), &Env::default(), RECOVER_S);
        assert!(!s.tired || s.exertion < RESTART);
    }

    #[test]
    fn shallow_rail_crossings_and_curbs_throw_small_wheels() {
        let db = db();
        let scooter = db.get("escooter_entdrosselt").unwrap();
        let mut curb = Env::default();
        curb.wheel[0].curb = 0.12;
        let mut s = State {
            vx: 6.,
            ..Default::default()
        };
        run(scooter, &mut s, Input::default(), &curb, 0.05);
        assert_eq!(s.fallen, Some(Fall::Curb));
        // Fahrrad (großes Rad) rollt drüber
        let bike = db.get("fahrrad_city").unwrap();
        let mut s = State {
            vx: 6.,
            ..Default::default()
        };
        run(bike, &mut s, Input::default(), &curb, 0.05);
        assert!(s.fallen.is_none());
        // sichere Rille (Risiko 1): Sturz; Risiko 0: nichts
        let mut rail = Env::default();
        rail.wheel[0].groove = 1.;
        let mut s = State {
            vx: 6.,
            ..Default::default()
        };
        run(bike, &mut s, Input::default(), &rail, 0.05);
        assert_eq!(s.fallen, Some(Fall::Rail));
    }
}
