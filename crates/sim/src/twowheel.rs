//! Zweiräder (Fahrphysik Phase 5): eigenes Modell ohne Reifen-Schräglauf. Fahrrad, E-Scooter, Roller und
//! Motorrad fahren kinematisch – die Bahn folgt der Schräglage:
//!
//! - Lenkung fordert einen Anteil der höchstmöglichen Krümmung (`kappa_max`): langsam begrenzt der Lenkeinschlag
//!   (`DELTA_LOW`, Wenden mit 2–3 m Radius), schnell die Schräglage, die der Fahrer nutzt (`LEAN_SKILL` der
//!   trockenen Haftgrenze, höchstens `max_schraeglage`). Daraus die Wunsch-Schräglage φ = atan(v²·κ/g); sie folgt mit
//!   begrenzter Rate (Einlenken braucht bei Tempo Zeit, wie Gegenlenken in echt); gefahren wird die Krümmung der
//!   tatsächlichen Schräglage g·tan(φ)/v². Langsam (unter 3–6 m/s, Füße am Boden) fährt das Rad der Lenkung nach.
//!   Der Fahrer legt sich höchstens so weit, wie die tatsächliche Haftung trägt (`LEAN_SKILL` von atan μ): voller
//!   Einschlag allein wirft auch auf Nässe oder Kopfstein nicht ab, dort werden die Bögen nur weiter. Stürze kommen
//!   von Bremsen/Gas in voller Schräglage (Reibungskreis) und plötzlichem Haftverlust (Pfütze, Schiene).
//! - Haftgrenze: braucht die Schräglage mehr als atan(μ) (neben der Längskraft, Reibungskreis) → Lowsider.
//! - Wheelie, wenn die Beschleunigung g·l_h/h überschreitet (l_h = Schwerpunkt bis Hinterachse), Stoppie beim
//!   Bremsen über g·l_v/h. Wheelie-Control hält beides an der Grenze, aber spielbar (kurze Wheelies beim
//!   Ampelstart sind erwünscht). Ohne Kontrolle überschlägt sich das Rad über `FLIP`.
//! - Bremsen: die Vorderbremse trägt den Großteil, begrenzt durch Überschlag und Haftung; ABS nur, wo vorhanden.
//!   Ein Vorderrad, das länger als `FRONT_LOCK_S` blockiert, stürzt (außer langsam und geradeaus); kurzes
//!   Überbremsen rutscht nur.
//! - Fahrrad: Fahrerleistung (Dauer, Sprint per Taste mit Ausdauer), Kraftgrenze aus der Muskel-Kurve.
//! - Motorrad: Gas in Schräglage auf losem Untergrund lässt das Heck leicht und kontrollierbar ausbrechen.
//! - Berlin: flach gequerte Straßenbahnschiene (`Ground::groove`) und Bordsteine an kleinen Rädern werfen ab; große
//!   Fahrräder ab `CURB_FALL_BIG`. Motorräder stößt der Bordstein nur und bremst sie.
//!
//! Zustand und Koordinaten wie `vphys` (Meter, Gierwinkel gegen den Uhrzeigersinn, Lenkung positiv = links).
use crate::vehdata::{Feel, G, Power, RHO, Vehicle};
use crate::vphys::{
    AQUA_REST, Env, HZ, Input, State, aquaplaning_grip, aquaplaning_speed, drive_force,
    mass_factor, shift, smooth,
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
    /// Aufprall (Auto, Wand, Baum) – mit der Spieler-Fahrhilfe der einzige Sturzgrund
    Crash,
}
impl Fall {
    pub fn label(self) -> &'static str {
        match self {
            Fall::Lowside => "Weggerutscht!",
            Fall::FrontLock => "Vorderrad blockiert!",
            Fall::Flip => "Überschlagen!",
            Fall::Rail => "In die Schiene geraten!",
            Fall::Curb => "Am Bordstein gestürzt!",
            Fall::Crash => "Abgeworfen!",
        }
    }
}

/// Lenkeinschlag (rad), den der Fahrer langsam nutzt (Wenden)
pub const DELTA_LOW: f64 = 0.6;
/// Anteil der Haftgrenze, den der Fahrer an Schräglage höchstens nutzt
pub const LEAN_SKILL: f64 = 0.92;
/// Lenkrate (rad/s) im Stand; sie fällt mit dem Tempo (m/s)
pub const BAR_RATE: f64 = 3.;
pub const BAR_RATE_V: f64 = 8.;
/// Schräglagenrate (rad/s) langsam bzw. ihre Abnahme mit dem Tempo (m/s)
pub const LEAN_RATE: f64 = 1.6;
pub const LEAN_RATE_V: f64 = 25.;
/// Unter diesem Tempo fährt das Rad der Lenkung direkt nach (Füße am Boden), darüber über die Schräglage
pub const KIN: [f64; 2] = [3., 6.];
/// Haftgrenze: so viel Reserve, bevor es wegrutscht
pub const LOWSIDE_MARGIN: f64 = 1.03;
/// Nickwinkel (rad): Wheelie-Control hält hier; ohne Kontrolle überschlägt es sich ab `FLIP`
pub const WHEELIE_CONTROL: f64 = 0.17;
/// Wheelie-Control hält das Vorderrad knapp in der Luft: über diesem Nickwinkel (rad) nimmt sie den Antrieb auf
/// diesen Anteil der Kippgrenze zurück
pub const WC_HOLD: f64 = 0.08;
pub const WC_TRIM: f64 = 0.8;
/// … und solange das Vorderrad schon in der Luft ist, auf so viel (langsames, kontrolliertes Anheben)
pub const WC_LIFT: f64 = 1.08;
/// nach einem Wheelie hält sie das Vorderrad so lange (s) mit diesem Anteil am Boden
pub const WC_COOL: f64 = 0.8;
pub const WC_DOWN: f64 = 0.97;
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
/// Fahrrad: frontal gegen den Bordstein ab diesem Tempo (m/s, ~23 km/h) Sturz, darunter rollt es hinauf
pub const CURB_FALL_BIG: f64 = 6.5;
/// großes Rad am Bordstein: Tempoverlust je m/s und m Kantenhöhe (höchstens `CURB_LOSS_MAX` des Tempos)
pub const CURB_LOSS: f64 = 0.3;
pub const CURB_LOSS_MAX: f64 = 0.25;
/// Zweirad ohne ABS: ein länger als so lange (s) blockiertes Vorderrad stürzt
pub const FRONT_LOCK_S: f64 = 0.4;
/// Spieler-Fahrhilfe (`Feel::moto_assist`): kleinste Drehrate (rad/s), die der Lenker bei Tempo noch erreicht –
/// 0,7 rad/s ≈ 60 m Radius bei 150 km/h (rein physikalisch ~180 m)
pub const ASSIST_YAW_MIN: f64 = 0.7;
/// … Bordsteinstoß kostet höchstens so viel Tempo (statt `CURB_LOSS_MAX`)
pub const ASSIST_CURB_LOSS: f64 = 0.05;
/// … Schieben im Stand (Bremse halten): Tempo rückwärts (m/s) und Anlauf (m/s²)
pub const PUSH_SPEED: f64 = 1.3;
pub const PUSH_ACCEL: f64 = 1.5;
/// … Nickwinkel (rad), auf den ein Wheelie bzw. Stoppie begrenzt wird
pub const ASSIST_PITCH: f64 = 0.35;
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

/// Höchste Krümmung (1/m), die der Fahrer bei `speed` (m/s) und Haftung `mu` fährt: langsam der Lenkeinschlag,
/// schnell die genutzte Schräglage. Der Lenkbefehl (−1…1) ist ein Anteil davon – auch für die KI.
pub fn kappa_max(v: &Vehicle, speed: f64, mu: f64) -> f64 {
    let lean_cap = v.max_lean.min(lean_limit(mu, 0.) * LEAN_SKILL);
    let k_low = DELTA_LOW.tan() / v.wheelbase;
    let k_lean = G * lean_cap.tan() / (speed * speed).max(1e-6);
    k_low.min(k_lean)
}

/// Größte Schräglage, die die Haftung trägt (rad), bei Längsbeschleunigung `ax` (m/s²).
pub fn lean_limit(mu: f64, ax: f64) -> f64 {
    let lat = (mu * mu - (ax / G).powi(2)).max(0.).sqrt();
    lat.atan()
}

fn substep(v: &Vehicle, feel: &Feel, s: &mut State, inp: &Input, env: &Env, dt: f64) {
    let assist = feel.moto_assist;
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
                if assist {
                    // die Rille reißt am Lenker, wirft aber nicht ab
                    s.lean += 0.12 * if s.lean >= 0. { 1. } else { -1. };
                } else {
                    fall(s, Fall::Rail);
                    return;
                }
            }
        }
    } else {
        s.groove_seen = false;
    }
    if env.wheel[0].curb > 0. && speed > 0.5 {
        if assist {
            // Spieler: spürbarer Stoß, kein Sturz und kaum Tempoverlust (einmal je Kante)
            if !s.curb_seen {
                let k = (env.wheel[0].curb / 0.12).clamp(0.3, 1.5);
                s.vx -= (CURB_LOSS * k * speed).min(speed * ASSIST_CURB_LOSS);
                s.pitch = (s.pitch + 0.05 * k).min(0.2);
            }
        } else if v.wheel_r < SMALL_WHEEL {
            if speed > CURB_FALL_V {
                fall(s, Fall::Curb);
                return;
            }
            s.vx = 0.;
        } else if matches!(v.engine.power, Power::Muscle { .. })
            && speed > CURB_FALL_BIG
            && env.wheel[0].curb > 0.08
        {
            fall(s, Fall::Curb);
            return;
        } else if !s.curb_seen {
            // großes Rad: der Stoß kostet Tempo und hebt kurz die Front, gestürzt wird nicht
            let k = (env.wheel[0].curb / 0.12).clamp(0.3, 1.5);
            s.vx -= (CURB_LOSS * k * speed).min(speed * CURB_LOSS_MAX);
            s.pitch = (s.pitch + 0.05 * k).min(0.2);
        }
        s.curb_seen = true;
        s.curb_hits += 1;
    } else {
        s.curb_seen = false;
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
    // Wheelie-Control: hebt das Vorderrad über `WC_HOLD`, nimmt sie Leistung bis unter die Kippgrenze zurück – das
    // Rad kommt sanft herunter, kurze Wheelies statt Fahrt an der Grenze
    if v.wheelie_control {
        if s.pitch > WC_HOLD {
            s.wc_t = WC_COOL;
        }
        let k = if s.wc_t > 0. {
            // nach einem Anheben: herunterholen und kurz unten halten
            if s.pitch > 0. { WC_TRIM } else { WC_DOWN }
        } else if s.pitch > 0. {
            WC_LIFT
        } else {
            f64::INFINITY
        };
        f_drive = f_drive.min(a_wheelie * k * m * df + f_drag + f_roll);
        s.wc_t = (s.wc_t - dt).max(0.);
    }
    let a_drive = (f_drive - f_drag - f_roll) / (m * df);
    if a_drive > a_wheelie {
        let excess = (a_drive - a_wheelie) / G;
        s.pitch += excess * PITCH_RATE * dt;
        if v.wheelie_control {
            s.pitch = s.pitch.min(WHEELIE_CONTROL);
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
        // ohne ABS: blockiertes Vorderrad rutscht; hält es zu lange an, stürzt es – langsam und geradeaus geht es
        // noch gut
        s.lock_t += dt;
        if !assist && s.lock_t > FRONT_LOCK_S && (speed > 6. || s.lean.abs() > 0.1) {
            fall(s, Fall::FrontLock);
            return;
        }
        f_front = front_grip * v.tire.slide_ratio;
    } else {
        s.lock_t = 0.;
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
    if assist {
        // Spieler: Wheelie und Stoppie bleiben steil, aber das Rad überschlägt sich nicht
        s.pitch = s.pitch.clamp(-ASSIST_PITCH, ASSIST_PITCH);
    } else if s.pitch.abs() > FLIP {
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
    // Spieler-Hilfe: Bremse halten im Stand schiebt das Rad mit Schrittgeschwindigkeit zurück (wie in GTA)
    let pushing = assist && inp.brake > 0.3 && inp.throttle < 0.05 && s.vx <= 0.05;
    if pushing {
        s.vx = (s.vx.min(0.) - PUSH_ACCEL * dt).max(-PUSH_SPEED);
        s.reverse = true;
    } else if s.vx < 0. {
        // losgelassen oder Gas: das Schieben läuft aus
        s.vx = (s.vx + (2. * PUSH_ACCEL).max(ax) * dt).min(0.);
    } else {
        s.vx = (s.vx + ax * dt).max(0.);
        // gebremst fast im Stand: steht
        if s.brake_p > 0.05 && s.vx < 0.1 && f_drive <= 0. {
            s.vx = 0.;
        }
    }
    let speed = s.vx;
    // Lenkung → Wunschkrümmung (Anteil der höchstmöglichen) → Lenkwinkel (Rate begrenzt) → Wunsch-Schräglage
    // bemessen am trockenen Nenngrip: Nässe, Kopfstein oder Schiene muss der Fahrer selbst berücksichtigen.
    // Spieler-Hilfe: bei Tempo mindestens `ASSIST_YAW_MIN` Drehrate (direkte Lenkung statt ~180 m Radius)
    let k_phys = kappa_max(v, speed, v.tire.mu * feel.grip());
    let k_max = if assist {
        k_phys.max(ASSIST_YAW_MIN / speed.abs().max(1.))
    } else {
        k_phys
    };
    let kappa_target = inp.steer.clamp(-1., 1.) * k_max;
    let delta_cmd = (l * kappa_target).atan();
    let rate = BAR_RATE / (1. + speed / BAR_RATE_V);
    s.delta += (delta_cmd - s.delta).clamp(-rate * dt, rate * dt);
    let kappa_cmd = s.delta.tan() / l;
    // der Fahrer legt sich nur so weit, wie die Haftung unter ihm trägt (Nässe, Kopfstein: weitere Bögen)
    let lean_cap = if assist {
        // Spieler: die Schräglage ist Anzeige (die Bahn folgt dem Lenker), nur durch das Rad begrenzt
        v.max_lean
    } else {
        v.max_lean.min(lean_limit(mu, 0.) * LEAN_SKILL)
    };
    let lean_want = (speed * speed * kappa_cmd / G)
        .atan()
        .clamp(-lean_cap, lean_cap);
    let lean_rate = LEAN_RATE / (1. + speed / LEAN_RATE_V) * if assist { 2.5 } else { 1. };
    s.lean += (lean_want - s.lean).clamp(-lean_rate * dt, lean_rate * dt);
    let kappa = if assist {
        // Spieler: kein Wegrutschen; auf Nässe, Pflaster oder Eis wird der Bogen nur etwas weiter
        let wet = (mu / (v.tire.mu * feel.grip()).max(1e-6)).clamp(0.55, 1.);
        kappa_cmd * if speed.abs() > KIN[1] { wet } else { 1. }
    } else {
        // Haftgrenze in Schräglage (Reibungskreis mit der Längskraft)
        if speed > KIN[0] && s.lean.abs() > lean_limit(mu, ax) * LOWSIDE_MARGIN {
            fall(s, Fall::Lowside);
            return;
        }
        let kappa_lean = G * s.lean.tan() / (speed * speed).max(1.);
        let w = smooth(KIN[0], KIN[1], speed);
        kappa_cmd * (1. - w) + kappa_lean * w
    };
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

/// Zweirad stürzt: liegt auf der Seite, rutscht aus (auch für Aufpralle aus `world`).
pub fn fall(s: &mut State, why: Fall) {
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

    /// Mit festem Tempo `kmh` und vollem Einschlag `secs` fahren: (Radius m, Schräglage rad, Sturz).
    fn full_lock(v: &Vehicle, env: &Env, kmh: f64, secs: f64) -> (f64, f64, Option<Fall>) {
        let feel = Feel::simulation();
        let mut s = State {
            vx: kmh / 3.6,
            gear: 2,
            ..Default::default()
        };
        let inp = Input {
            steer: 1.,
            ..Default::default()
        };
        for _ in 0..(secs * HZ) as usize {
            s.vx = kmh / 3.6;
            vstep(v, &feel, &mut s, &inp, env, 1. / HZ);
            if s.fallen.is_some() {
                break;
            }
        }
        (s.vx / s.r.abs().max(1e-9), s.lean, s.fallen)
    }

    #[test]
    fn motorcycles_turn_like_motorcycles() {
        let db = db();
        let dry = Env::default();
        for id in ["roller_45", "motorrad_naked", "superbike", "cruiser"] {
            let v = db.get(id).unwrap();
            // Wenden im Schritttempo: Lenker eingeschlagen, Radius wie in echt 2–3 m
            let (r, _, f) = full_lock(v, &dry, 5., 2.);
            assert!(
                f.is_none() && (1.5..=3.).contains(&r),
                "{id}: Wenderadius {r:.1} m"
            );
            // trocken und mit Tempo: voller Einschlag nutzt die Haftung, wirft aber nicht ab; der Radius folgt
            // der Schräglage (R = v²/(g·tan φ))
            for kmh in [30., 60., 100.] {
                let (r, lean, f) = full_lock(v, &dry, kmh, 3.);
                assert!(f.is_none(), "{id} bei {kmh} km/h: {f:?}");
                let ideal = (kmh / 3.6f64).powi(2) / (G * lean.abs().tan());
                assert!(
                    (r / ideal - 1.).abs() < 0.08,
                    "{id} {kmh}: R {r:.1} vs {ideal:.1}"
                );
                assert!(lean.abs() <= v.max_lean + 1e-6);
            }
        }
        // die Schräglage bestimmt die Kurve: das Superbike (49°) fährt bei 60 km/h deutlich enger als der
        // Cruiser, der mit den Trittbrettern bei 30° aufsetzt
        let (r_sb, _, _) = full_lock(db.get("superbike").unwrap(), &dry, 60., 3.);
        let (r_cr, _, _) = full_lock(db.get("cruiser").unwrap(), &dry, 60., 3.);
        assert!(
            r_cr > r_sb * 1.6,
            "Superbike {r_sb:.0} m, Cruiser {r_cr:.0} m"
        );
        assert!(
            (20. ..=35.).contains(&r_sb),
            "Superbike bei 60 km/h: {r_sb:.0} m"
        );
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
    fn full_lock_on_wet_cobbles_widens_the_arc_hard_braking_in_lean_lowsides() {
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
        assert!(
            s.fallen.is_none(),
            "voller Einschlag allein: {:?}",
            s.fallen
        );
        assert!(s.lean.abs() <= lean_limit(0.5 * v.tire.mu, 0.) + 1e-6);
        // in voller Schräglage voll bremsen: der Reibungskreis reicht nicht
        run(
            v,
            &mut s,
            Input {
                brake: 1.,
                steer: 1.,
                ..Default::default()
            },
            &wet,
            1.5,
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
        // nasser Belag + Vollbremsung ohne ABS: Vorderrad blockiert – kurz überbremsen rutscht nur
        let wet = Env::uniform(Ground {
            mu_rel: 0.5,
            ..Ground::DRY
        });
        let mut tap = s.clone();
        run(
            v,
            &mut tap,
            Input {
                brake: 1.,
                ..Default::default()
            },
            &wet,
            FRONT_LOCK_S * 0.75,
        );
        assert!(tap.fallen.is_none() && tap.lock_t > 0.);
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
        // Motorräder stürzen am Bordstein nie, auch schnell und an hoher Kante – der Stoß kostet nur Tempo
        let mut high = Env::default();
        high.wheel[0].curb = 0.18;
        for id in ["motorrad_naked", "dirtbike", "superbike", "cruiser"] {
            let m = db.get(id).unwrap();
            let mut s = State {
                vx: 20.,
                gear: 3,
                ..Default::default()
            };
            run(m, &mut s, Input::default(), &high, 0.05);
            assert!(s.fallen.is_none(), "{id}");
            assert!(s.vx < 20. && s.vx > 10., "{id}: {}", s.vx);
        }
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

    fn game_run(v: &Vehicle, s: &mut State, inp: Input, env: &Env, secs: f64) {
        let feel = Feel {
            moto_assist: true,
            ..Feel::simulation()
        };
        for _ in 0..(secs * HZ) as usize {
            vstep(v, &feel, s, &inp, env, 1. / HZ);
        }
    }

    #[test]
    fn rider_assist_keeps_the_bike_up_on_wet_ground_and_steers_at_speed() {
        let db = db();
        let v = db.get("motorrad_naked").unwrap();
        let wet = Env::uniform(Ground {
            mu_rel: 0.5,
            ..Ground::DRY
        });
        // voll eingelenkt voll bremsen im Regen: ohne Hilfe Lowside, mit Hilfe nur ein weiterer Bogen
        let mut s = State {
            vx: 18.,
            gear: 3,
            ..Default::default()
        };
        game_run(
            v,
            &mut s,
            Input {
                brake: 1.,
                steer: 1.,
                ..Default::default()
            },
            &wet,
            2.,
        );
        assert!(s.fallen.is_none(), "{:?}", s.fallen);
        // bei 150 km/h trocken: Radius höchstens rund 60 m (Mindest-Drehrate)
        let feel = Feel {
            moto_assist: true,
            ..Feel::simulation()
        };
        let mut s = State {
            vx: 150. / 3.6,
            gear: 5,
            ..Default::default()
        };
        let inp = Input {
            steer: 1.,
            ..Default::default()
        };
        for _ in 0..(2. * HZ) as usize {
            s.vx = 150. / 3.6;
            vstep(v, &feel, &mut s, &inp, &Env::default(), 1. / HZ);
        }
        let r = s.vx / s.r.abs().max(1e-9);
        assert!(s.fallen.is_none() && r <= 62., "Radius {r:.0} m");
        // ohne Hilfe deutlich weiter
        let (r_phys, _, _) = full_lock(v, &Env::default(), 150., 2.);
        assert!(r_phys > r * 1.3, "{r_phys:.0} vs {r:.0}");
    }

    #[test]
    fn rider_assist_pushes_backwards_and_shrugs_off_curbs() {
        let db = db();
        let v = db.get("motorrad_naked").unwrap();
        // im Stand Bremse halten: das Motorrad rollt rückwärts (Schieben), höchstens Schritttempo
        let mut s = State::default();
        game_run(
            v,
            &mut s,
            Input {
                brake: 1.,
                ..Default::default()
            },
            &Env::default(),
            2.,
        );
        assert!(
            (-PUSH_SPEED - 1e-9..=-PUSH_SPEED * 0.9).contains(&s.vx),
            "{}",
            s.vx
        );
        // loslassen: kommt wieder zum Stehen
        game_run(v, &mut s, Input::default(), &Env::default(), 1.5);
        assert!(s.vx.abs() < 1e-6, "{}", s.vx);
        // Bordstein bei 50 km/h: höchstens 5 % Tempo weg, kein Sturz, kein Überschlag
        let mut curb = Env::default();
        curb.wheel[0].curb = 0.18;
        let v0 = 50. / 3.6;
        let mut s = State {
            vx: v0,
            gear: 3,
            ..Default::default()
        };
        game_run(v, &mut s, Input::default(), &curb, 0.05);
        assert!(s.fallen.is_none(), "{:?}", s.fallen);
        assert!(
            s.vx >= v0 * (1. - ASSIST_CURB_LOSS) - 0.2,
            "{} von {v0}",
            s.vx
        );
        assert!(s.pitch.abs() <= ASSIST_PITCH + 1e-9);
    }
}
