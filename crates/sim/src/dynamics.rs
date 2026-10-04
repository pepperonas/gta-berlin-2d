//! Fahrdynamik des gefahrenen Autos (Port von `dynamics.js`): Einspurmodell mit Reifenkräften an Vorder- und
//! Hinterachse, Kammscher Kreis, dynamische Achslast, Motorlage, ABS, ASR/ESP, Handbremsen-Drift, Wheelie und
//! Stoppie bei Zweirädern. Eine Spielspaß-Schicht (`FUN_*`) macht es großzügiger als die Wirklichkeit.
//! Einheiten innen SI (m, s, N, kg), außen px (10 px = 1 m).
use crate::carmodels::{Drive, Spec};
use crate::math::{sign, smoothstep01};
use crate::traction::{Aqua, Traction};

pub const SUBSTEPS: usize = 4;
pub const G: f64 = 9.81;
pub const PEAK: f64 = 1.4;
pub const STIFF: [f64; 2] = [17., 19.];
pub const STEER_RATE: f64 = 4.;
pub const CENTER_RATE: f64 = 6.;
pub const STEER_ASSIST: f64 = 1.3;
pub const LOCKED: f64 = 0.78;
pub const ABS: f64 = 0.95;
pub const ESP: [f64; 4] = [0.9, 0.07, 0.14, 0.85];
pub const ESP_YAW: f64 = 5.;
pub const LIFT: f64 = 0.97;
pub const LOW_LOCK: [f64; 3] = [0.95, 3., 9.];
pub const FUN_GRIP: f64 = 1.4;
pub const FUN_BRAKE: f64 = 1.25;
pub const FUN_POWER: f64 = 1.2;
pub const FUN_STEER: f64 = 1.2;
pub const FUN_HANDBRAKE: [f64; 2] = [0.4, 0.3];
pub const LAUNCH_GAIN: f64 = 0.28;
pub const LAUNCH_AWD: f64 = 0.35;
pub const LAUNCH_MAX_ACCEL: f64 = 10.;
pub const LAUNCH_FROM: f64 = 20. / 3.6;
pub const LAUNCH_TO: f64 = 90. / 3.6;
pub const WHEEL_POWER_HIGH: f64 = 0.86;
pub const WHEEL_POWER_FROM: f64 = 50. / 3.6;
pub const WHEEL_POWER_TO: f64 = 120. / 3.6;
pub const ROLL: f64 = 0.015;
pub const ENGINE_BRAKE: f64 = 0.9;
pub const REVERSE: f64 = 8.3;
pub const SUSPENSION: f64 = 0.12;
pub const KINEMATIC: [f64; 2] = [1., 4.];
pub const ESP_FROM: f64 = 4.;
pub const RWD_DRIFT_GRIP: f64 = 0.72;
pub const DRIFT_DURATION: f64 = 1.;
pub const DRIFT_MIN_SPEED: f64 = 8.;
pub const DRIFT_KICK: f64 = 0.2;
pub const DRIFT_REAR_GRIP: f64 = 0.65;
pub const DRIFT_YAW: f64 = 0.15;
pub const DRIFT_FOLLOW: f64 = 3.8;

/// Bedienung (−1…1 bzw. 0…1).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Controls {
    pub throttle: f64,
    pub brake: f64,
    pub steer: f64,
    pub handbrake: bool,
    /// Fahrrad: Sprint (Ausdauer)
    pub sprint: bool,
}
/// Untergrund (car.js `SURFACE`): Rollwiderstand, Haftung, Höchsttempo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
    pub drag: f64,
    pub grip: f64,
    pub top: f64,
}
/// Antriebskraft für Pedalstellung `t` (0…1): bis 90 % linear bis `grip` (höchstens die Motorkraft), darüber weich
/// bis zur vollen Motorkraft; `t` = 1 ergibt genau `engine`.
pub fn pedal_force(t: f64, engine: f64, grip: f64) -> f64 {
    if t >= 1. {
        return engine;
    }
    let usable = engine.min(grip.max(0.));
    let over = ((t - 0.9) / 0.1).clamp(0., 1.);
    let s = over * over * (3. - 2. * over);
    usable * (t / 0.9).min(1.) + (engine - usable).max(0.) * s
}
/// Zustand für Anzeige und Tests.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DynState {
    pub esp: f64,
    pub delta: f64,
    pub ax: f64,
    pub ay: f64,
    pub alpha_f: f64,
    pub alpha_r: f64,
    pub spin_f: f64,
    pub spin_r: f64,
    pub lock_r: f64,
    pub understeer: f64,
    pub wheelie: f64,
    pub stoppie: f64,
    pub lean: f64,
    pub drift_t: f64,
    pub drift_dir: f64,
    pub handbrake_was: bool,
}
/// Bewegungszustand in px und rad.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Body {
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub vx: f64,
    pub vy: f64,
    pub ang_vel: f64,
}
/// Ergebnis eines Schritts (Anzeige/Klang).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Outcome {
    pub spin: f64,
    pub skid: f64,
}
/// Hilfen und Wetter für einen Schritt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Assists {
    pub esp: bool,
    pub abs: bool,
    pub aqua: bool,
    pub aqua_yaw: f64,
}
impl Default for Assists {
    fn default() -> Self {
        Self {
            esp: true,
            abs: true,
            aqua: false,
            aqua_yaw: 0.,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Envelope {
    pub force: f64,
    pub traction: f64,
}
/// Zugkraft-Hüllkurve: unten drehmomentbegrenzt, oben P/v; Anfahrhilfe erhöht Zugkraft und übertragbare Längskraft.
pub fn drive_envelope(spec: &Spec, speed: f64, grip: f64, steer: f64) -> Envelope {
    let v = speed.max(0.);
    let launch = 1. - smoothstep01((v - LAUNCH_FROM) / (LAUNCH_TO - LAUNCH_FROM));
    let surface = smoothstep01((grip - 0.45) / 0.55);
    let straight = 1. - smoothstep01(steer.abs() / 0.5);
    let gain = if spec.two_wheel {
        0.
    } else {
        LAUNCH_GAIN
            * if spec.drive == Drive::Awd {
                LAUNCH_AWD
            } else {
                1.
            }
    };
    let transfer = LAUNCH_MAX_ACCEL / G * spec.h / spec.wb;
    let share = match spec.drive {
        Drive::Awd => 1.,
        Drive::Fwd => spec.front - transfer,
        Drive::Rwd => 1. - spec.front + transfer,
    };
    let at_limit = spec.mu * FUN_GRIP * ESP[0] * G * share.max(0.1);
    let headroom = (LAUNCH_MAX_ACCEL / at_limit - 1.).max(0.);
    let traction = 1. + gain.min(headroom) * launch * surface * straight;
    let power = spec.kw
        * 1000.
        * FUN_POWER
        * (1.
            - (1. - WHEEL_POWER_HIGH)
                * smoothstep01((v - WHEEL_POWER_FROM) / (WHEEL_POWER_TO - WHEEL_POWER_FROM)));
    Envelope {
        force: (power / spec.v_low).min(power / v.max(0.5)) * traction,
        traction,
    }
}

/// Reifenkennlinie: Kraftanteil (−1…1) bei Schräglauf α für Steifigkeit B.
pub fn tire_curve(alpha: f64, stiff: f64) -> f64 {
    (PEAK * (stiff / PEAK * alpha).atan()).sin()
}
/// Achsgeometrie: a = Schwerpunkt → Vorderachse, b = → Hinterachse, Gierträgheit.
pub fn geometry(spec: &Spec) -> (f64, f64, f64) {
    let a = spec.wb * (1. - spec.front);
    let b = spec.wb * spec.front;
    (a, b, spec.mass * a * b * spec.yaw * spec.yaw)
}

/// Ein Schritt dt mit Untergrund, Wetterfaktoren und Bedienung.
#[allow(clippy::too_many_arguments)]
pub fn step_dynamics(
    body: &mut Body,
    d: &mut DynState,
    spec: &Spec,
    dt: f64,
    surf: Surface,
    tr: Traction,
    ctl: Controls,
    assist: Assists,
) -> Outcome {
    let (ga, gb, iz) = geometry(spec);
    let m = spec.mass;
    let abs = if assist.abs { ABS } else { 1. };
    let aq = assist.aqua;
    let mu_base = spec.mu * surf.grip * FUN_GRIP;
    let p = spec.kw * 1000. * FUN_POWER;
    let vmax = spec.vmax / 3.6 * surf.top;
    let roll = ROLL * m * G * surf.drag;
    let top_force = drive_envelope(spec, vmax, 1., 0.).force;
    let cd = ((top_force * 0.92 - ROLL * m * G) / (vmax * vmax)).max(0.05);
    let steer_max0 = (spec.steer_max * FUN_STEER).min(0.8);
    let [lock_low, v0, v1] = LOW_LOCK;
    let (mut s, mut c) = body.angle.sin_cos();
    let mut u = (body.vx * c + body.vy * s) / 10.;
    let mut v = (-body.vx * s + body.vy * c) / 10.;
    let mut w = body.ang_vel;
    let h = dt / SUBSTEPS as f64;
    let handbrake_pressed = ctl.handbrake && !d.handbrake_was;
    d.handbrake_was = ctl.handbrake;
    if handbrake_pressed && !spec.two_wheel && u.hypot(v) >= DRIFT_MIN_SPEED {
        let dir = sign(if ctl.steer != 0. {
            ctl.steer
        } else if d.delta != 0. {
            d.delta
        } else {
            w
        });
        if dir != 0. {
            d.drift_t = DRIFT_DURATION;
            d.drift_dir = dir;
            w += dir * DRIFT_KICK;
        }
    }
    let (mut spin_any, mut skid) = (0f64, 0f64);
    for _ in 0..SUBSTEPS {
        d.drift_t = (d.drift_t - h).max(0.);
        let drift_blend = (d.drift_t / DRIFT_DURATION).clamp(0., 1.);
        let speed = u.hypot(v);
        let [k0, k1] = KINEMATIC;
        let qs = ((speed - k0) / (k1 - k0)).clamp(0., 1.);
        let steer_max = steer_max0
            + (lock_low - steer_max0) * (1. - ((u.abs() - v0) / (v1 - v0)).clamp(0., 1.));
        let lim = steer_max.min(
            (STEER_ASSIST * spec.wb * mu_base * tr.lat * G / (u * u).max(1.))
                .atan()
                .max(0.05),
        );
        let target = ctl.steer.clamp(-1., 1.) * lim * tr.steer;
        let rate = if target.abs() < d.delta.abs() {
            CENTER_RATE
        } else {
            STEER_RATE
        };
        d.delta += (target - d.delta).clamp(-rate * h, rate * h);
        let delta = d.delta;
        let df = m * d.ax * spec.h / spec.wb;
        let fzf = (m * G * spec.front - df).clamp(0.1 * m * G, 0.9 * m * G);
        let fzr = m * G - fzf;
        let mu = mu_base
            * (1.
                - if spec.two_wheel {
                    0.
                } else {
                    0.22 * (spec.h / spec.track * d.ay.abs() / G).clamp(0., 1.)
                });
        let drive_curve = drive_envelope(
            spec,
            u.abs(),
            if aq { 0. } else { surf.grip * tr.accel },
            ctl.steer,
        );
        let launch_traction =
            if ctl.throttle > 0. && u >= -0.5 && !ctl.handbrake && drift_blend <= 0. {
                drive_curve.traction
            } else {
                1.
            };
        let mu_x = mu * tr.accel * launch_traction;
        let mu_b = mu * FUN_BRAKE * tr.brake * if aq { Aqua::BRAKE } else { 1. };
        let mu_y = mu * tr.lat * if aq { Aqua::LAT } else { 1. };
        let (mut fxf, mut fxr) = (0f64, 0f64);
        let (mut spin_f, mut spin_r, mut lock_f, mut lock_r, mut esp, mut wheelie, mut stoppie) =
            (0f64, 0f64, 0f64, 0f64, 0f64, 0f64, 0f64);
        let fwd = u > -0.5;
        if ctl.throttle > 0. && u < -0.5 {
            let fb = ctl.throttle * mu_b * m * G;
            fxf += (fb * spec.bias).min(mu_b * fzf * abs);
            fxr += (fb * (1. - spec.bias)).min(mu_b * fzr * abs);
        }
        if ctl.throttle > 0. && fwd {
            let engine = drive_curve.force / drive_curve.traction
                * launch_traction
                * if u < vmax { 1. } else { 0. };
            // Pedal auf die Haftung abgebildet: die Antriebskraft überstieg schon bei halbem Gas die Reifenhaftung,
            // der Rest des Pedalwegs (Controller-Trigger) war wirkungslos. Bis 90 % Pedal wächst sie gleichmäßig bis
            // zum Haftungslimit der angetriebenen Achse(n), die letzten 10 % geben die übrige Motorkraft frei –
            // Vollgas (1, auch jede Taste) bleibt exakt wie vorher, samt Durchdrehen ohne ESP.
            let grip = match spec.drive {
                Drive::Fwd => mu_x * fzf,
                Drive::Rwd => mu_x * fzr,
                Drive::Awd => mu_x * (fzf + fzr),
            } * if assist.esp { ESP[0] } else { 1. };
            let fd = pedal_force(ctl.throttle, engine, grip);
            let share = match spec.drive {
                Drive::Fwd => 1.,
                Drive::Rwd => 0.,
                Drive::Awd => spec.awd_front,
            };
            let mut want = [fd * share, fd * (1. - share)];
            if spec.drive == Drive::Awd {
                let cap = [mu_x * fzf, mu_x * fzr];
                for (i, j) in [(0, 1), (1, 0)] {
                    if want[i] > cap[i] {
                        want[j] += want[i] - cap[i];
                        want[i] = cap[i];
                    }
                }
            }
            if assist.esp && drift_blend <= 0. {
                let [k, a0, span, cut] = ESP;
                let esc = if speed > ESP_FROM { 1. } else { 0. };
                let lim = |al: f64, fz: f64| {
                    mu_x * fz * k * (1. - esc * cut * ((al.abs() - a0) / span).clamp(0., 1.))
                };
                let (lf, lr) = (lim(d.alpha_f, fzf), lim(d.alpha_r, fzr));
                if want[0] > lf || want[1] > lr {
                    esp = 1.;
                }
                want = [want[0].min(lf), want[1].min(lr)];
            }
            let drive = |f: f64, fz: f64| {
                if f > mu_x * fz {
                    (mu_x * fz * LOCKED, 1.)
                } else {
                    (f, 0.)
                }
            };
            (fxf, spin_f) = drive(want[0], fzf);
            (fxr, spin_r) = drive(want[1], fzr);
            if spec.two_wheel {
                let lift = m * G * spec.front * spec.wb / spec.h;
                let f = fxf + fxr;
                wheelie = ((f / lift - 0.85) / 0.15).clamp(0., 1.);
                if f > lift * LIFT {
                    let k = lift * LIFT / f;
                    fxf *= k;
                    fxr *= k;
                }
            }
        } else if ctl.throttle <= 0. && u.abs() > 0.3 && ctl.brake <= 0. {
            let fe = -sign(u) * ENGINE_BRAKE * m * (u.abs() / 4.).min(1.);
            match spec.drive {
                Drive::Fwd => fxf += fe,
                Drive::Rwd => fxr += fe,
                Drive::Awd => {
                    fxf += fe * spec.awd_front;
                    fxr += fe * (1. - spec.awd_front);
                }
            }
        }
        if ctl.brake > 0. {
            if u > 0.5 {
                let fb = ctl.brake * mu_b * m * G * spec.brake_k;
                let mut bf = (fb * spec.bias).min(mu_b * fzf * abs);
                let mut br = (fb * (1. - spec.bias)).min(mu_b * fzr * abs);
                if !assist.abs {
                    if bf > mu_y * fzf {
                        lock_f = 1.;
                        bf = mu_y * fzf * LOCKED;
                    }
                    if br > mu_y * fzr {
                        lock_r = 1.;
                        br = mu_y * fzr * LOCKED;
                    }
                }
                if spec.two_wheel {
                    let lift = m * G * (1. - spec.front) * spec.wb / spec.h;
                    stoppie = (((bf + br) / lift - 0.85) / 0.15).clamp(0., 1.);
                    if bf + br > lift * LIFT {
                        let k = lift * LIFT / (bf + br);
                        bf *= k;
                        br *= k;
                    }
                }
                fxf -= bf;
                fxr -= br;
            } else if u > -REVERSE {
                let fr = ctl.brake * (p / spec.v_low).min(m * 3.5);
                if spec.drive == Drive::Fwd {
                    fxf -= fr.min(mu_x * fzf);
                } else {
                    fxr -= fr.min(mu_x * fzr);
                }
            }
        }
        let vfa = v + w * ga;
        let (sd0, cd0) = delta.sin_cos();
        let alpha_f = (vfa * cd0 - u * sd0).atan2((u * cd0 + vfa * sd0).abs().max(1.5));
        let alpha_r = (v - w * gb).atan2(u.abs().max(1.5));
        let lat = |alpha: f64, fz: f64, fx: f64, stiff: f64, slipping: bool| {
            let longitudinal = if fx > 0. { fx / launch_traction } else { fx };
            let cap = ((mu_y * fz).powi(2) - longitudinal * longitudinal)
                .max(0.)
                .sqrt()
                * if slipping { 0.8 } else { 1. };
            -cap * tire_curve(alpha, stiff)
        };
        let powered_rwd_drift = !assist.esp
            && spec.drive == Drive::Rwd
            && ctl.throttle > 0.65
            && ctl.steer.abs() > 0.15
            && u > 4.
            && !ctl.handbrake;
        let fyf = lat(alpha_f, fzf, fxf, STIFF[0], spin_f > 0. || lock_f > 0.) * qs;
        let drift_rear_grip = 1. - (1. - DRIFT_REAR_GRIP) * drift_blend;
        let mut fyr = lat(alpha_r, fzr, fxr, STIFF[1], spin_r > 0. || lock_r > 0.)
            * if powered_rwd_drift {
                RWD_DRIFT_GRIP
            } else {
                drift_rear_grip
            }
            * qs;
        if ctl.handbrake && speed > 0.3 {
            let [side, drag] = FUN_HANDBRAKE;
            let vy = v - w * gb;
            fyr = -mu_y * side * fzr * (vy / 1.5).clamp(-1., 1.) * qs;
            fxr = -sign(u) * mu_b * drag * fzr;
            lock_r = 1.;
        }
        let (sn, cs) = delta.sin_cos();
        let ff_x = fxf * cs - fyf * sn;
        let ff_y = fxf * sn + fyf * cs;
        let drag = cd * u * u.abs() + if u.abs() > 0.05 { roll * sign(u) } else { 0. };
        let ax = (ff_x + fxr - drag) / m;
        let ay = (ff_y + fyr) / m;
        let mut dw = (ga * ff_y - gb * fyr) / iz;
        let du = ax + v * w;
        let dv = ay - u * w;
        if drift_blend > 0. && !spec.two_wheel {
            let speed_mix = ((u.abs() - DRIFT_MIN_SPEED) / 14.).clamp(0., 1.);
            let counter = (d.drift_dir * ctl.steer).clamp(0., 1.);
            let drift_yaw = u * delta.tan() / spec.wb
                + d.drift_dir * DRIFT_YAW * speed_mix * drift_blend * (0.25 + 0.75 * counter);
            dw += (drift_yaw - w) * DRIFT_FOLLOW;
        }
        if assist.esp && drift_blend <= 0. && !ctl.handbrake && speed > ESP_FROM && u > 0. {
            let w_ref = u * delta.tan() / spec.wb;
            let over = w.abs() - w_ref.abs();
            if over > 0.05 && d.alpha_r.abs() > 0.06 {
                dw -= sign(w) * ESP_YAW * over;
                esp = 1.;
            }
        }
        u += du * h;
        v += dv * h;
        w += dw * h;
        if aq {
            w += (assist.aqua_yaw - w) * (6. * h).min(1.);
        }
        let sp = u.hypot(v);
        let q = ((sp - k0) / (k1 - k0)).clamp(0., 1.);
        if q < 1. {
            let wk = u * delta.tan() / spec.wb;
            w = q * w + (1. - q) * wk;
            v = q * v + (1. - q) * wk * gb;
            if ctl.throttle <= 0. && ctl.brake <= 0. && sp < 0.25 {
                u = 0.;
                v = 0.;
                w = 0.;
            }
        }
        let tau = h / SUSPENSION;
        d.ax += (ax - d.ax) * tau.min(1.);
        d.ay += (ay - d.ay) * tau.min(1.);
        d.alpha_f = alpha_f * qs;
        d.alpha_r = alpha_r * qs;
        d.spin_f = spin_f;
        d.spin_r = spin_r;
        d.lock_r = lock_r;
        d.esp = esp;
        d.wheelie = wheelie;
        d.stoppie = stoppie;
        d.lean = if spec.two_wheel { d.ay.atan2(G) } else { 0. };
        spin_any = spin_any.max(spin_f).max(spin_r);
        if sp > 3. {
            skid = skid
                .max(((alpha_f.abs().max(alpha_r.abs()) - 0.12) / 0.2).clamp(0., 1.))
                .max(drift_blend * 0.75)
                .max(lock_f.max(lock_r) * 0.8)
                .max(if spin_f > 0. || spin_r > 0. { 0.6 } else { 0. });
        }
        body.angle += w * h;
        (s, c) = body.angle.sin_cos();
        body.x += (u * c - v * s) * 10. * h;
        body.y += (u * s + v * c) * 10. * h;
    }
    d.understeer = d.alpha_f.abs() - d.alpha_r.abs();
    body.vx = (u * c - v * s) * 10.;
    body.vy = (u * s + v * c) * 10.;
    body.ang_vel = w;
    Outcome {
        spin: spin_any,
        skid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carmodels::spec;
    use crate::traction::{DRY, ICE};
    const ROAD: Surface = Surface {
        drag: 1.,
        grip: 1.,
        top: 1.,
    };

    #[test]
    fn pedal_uses_the_whole_travel_up_to_grip_and_full_stays_full() {
        let (engine, grip) = (12_000., 6_000.);
        assert_eq!(pedal_force(1., engine, grip), engine, "Vollgas unverändert");
        assert_eq!(pedal_force(0., engine, grip), 0.);
        assert!(
            (pedal_force(0.45, engine, grip) - grip / 2.).abs() < 1e-6,
            "halber Weg = halbe Haftung"
        );
        assert!(
            (pedal_force(0.9, engine, grip) - grip).abs() < 1e-6,
            "bei 90 % an der Haftungsgrenze"
        );
        let mut last = 0.;
        for i in 0..=100 {
            let f = pedal_force(i as f64 / 100., engine, grip);
            assert!(f >= last - 1e-9, "monoton");
            last = f;
        }
        // schwacher Motor (unter der Haftung): linear bis zur vollen Motorkraft
        assert!((pedal_force(0.45, 3_000., grip) - 1_500.).abs() < 1e-6);
        assert_eq!(pedal_force(0.95, 3_000., grip), 3_000.);
    }

    fn run(
        model: &str,
        secs: f64,
        ctl: Controls,
        tr: Traction,
        start_kmh: f64,
    ) -> (Body, DynState) {
        let sp = spec(model).unwrap();
        let mut b = Body {
            vx: start_kmh / 0.36,
            ..Default::default()
        };
        let mut d = DynState::default();
        for _ in 0..(secs * 60.) as usize {
            step_dynamics(
                &mut b,
                &mut d,
                sp,
                1. / 60.,
                ROAD,
                tr,
                ctl,
                Assists::default(),
            );
        }
        (b, d)
    }
    fn kmh(b: &Body) -> f64 {
        b.vx.hypot(b.vy) * 0.36
    }

    /// Halbes Pedal beschleunigt etwa halb so stark wie Vollgas (vorher fast gleich stark – der Controller-Trigger
    /// wirkte wie ein Schalter); Vollgas bleibt unverändert.
    #[test]
    fn half_pedal_accelerates_about_half_as_hard() {
        let at = |t: f64| {
            let (b, _) = run(
                "limousine",
                1.,
                Controls {
                    throttle: t,
                    ..Default::default()
                },
                DRY,
                0.,
            );
            kmh(&b)
        };
        let (half, full) = (at(0.45), at(1.));
        assert!(full > 30., "Vollgas nach 1 s: {full}");
        let r = half / full;
        assert!(
            r > 0.35 && r < 0.65,
            "halb {half:.1} gegen voll {full:.1} km/h"
        );
        assert!(
            at(0.2) < at(0.45) && at(0.45) < at(0.8),
            "über den ganzen Weg steigend"
        );
    }

    #[test]
    fn launch_and_top_speed_are_plausible() {
        let full = Controls {
            throttle: 1.,
            ..Default::default()
        };
        let (b, _) = run("kompakt", 8., full, DRY, 0.);
        assert!(
            kmh(&b) > 90. && kmh(&b) < 140.,
            "Kompakt nach 8 s: {}",
            kmh(&b)
        );
        let (b, _) = run("sportwagen", 4., full, DRY, 0.);
        assert!(kmh(&b) > 95., "Sportwagen nach 4 s: {}", kmh(&b));
        // Höchsttempo bleibt echt (≈ vmax), auch mit Spielspaß-Leistung
        let (b, _) = run("zweitakter", 90., full, DRY, 0.);
        assert!(
            (95. ..115.).contains(&kmh(&b)),
            "Zweitakter Spitze {}",
            kmh(&b)
        );
    }

    #[test]
    fn brakes_with_abs_and_ice_is_longer() {
        let brake = Controls {
            brake: 1.,
            ..Default::default()
        };
        let stop = |tr| {
            let sp = spec("limousine").unwrap();
            let mut b = Body {
                vx: 100. / 0.36,
                ..Default::default()
            };
            let mut d = DynState::default();
            let mut n = 0;
            while b.vx > 1. && n < 6000 {
                step_dynamics(
                    &mut b,
                    &mut d,
                    sp,
                    1. / 60.,
                    ROAD,
                    tr,
                    brake,
                    Assists::default(),
                );
                n += 1;
            }
            b.x / 10.
        };
        let dry = stop(DRY);
        let ice = stop(ICE);
        assert!(dry > 15. && dry < 40., "Bremsweg trocken {dry} m");
        assert!(ice > dry * 2., "Glätte {ice} m gegen {dry} m");
    }

    #[test]
    fn steering_turns_the_right_way_and_stands_still() {
        let ctl = Controls {
            throttle: 0.4,
            steer: 1.,
            ..Default::default()
        };
        let (b, _) = run("kompakt", 1.5, ctl, DRY, 30.);
        assert!(
            b.angle > 0.3,
            "rechts lenken dreht im Uhrzeigersinn: {}",
            b.angle
        );
        let (b, _) = run("kompakt", 2., Controls::default(), DRY, 0.);
        assert_eq!((b.x, b.y, b.angle), (0., 0., 0.));
    }

    #[test]
    fn handbrake_starts_a_drift_and_esp_catches_oversteer() {
        let sp = spec("limousine").unwrap();
        let mut b = Body {
            vx: 80. / 0.36,
            ..Default::default()
        };
        let mut d = DynState::default();
        let ctl = Controls {
            steer: 1.,
            handbrake: true,
            ..Default::default()
        };
        step_dynamics(
            &mut b,
            &mut d,
            sp,
            1. / 60.,
            ROAD,
            DRY,
            ctl,
            Assists::default(),
        );
        assert!(d.drift_t > 0.9 && d.drift_dir == 1.);
        let mut skid = 0f64;
        for _ in 0..40 {
            skid = skid.max(
                step_dynamics(
                    &mut b,
                    &mut d,
                    sp,
                    1. / 60.,
                    ROAD,
                    DRY,
                    ctl,
                    Assists::default(),
                )
                .skid,
            );
        }
        assert!(skid > 0.5, "Driften quietscht: {skid}");
    }

    #[test]
    fn wheelie_limits_motorcycle_launch() {
        let (_, d) = run(
            "motorcycle",
            0.6,
            Controls {
                throttle: 1.,
                ..Default::default()
            },
            DRY,
            0.,
        );
        assert!(d.wheelie > 0. && d.lean.abs() < 0.01);
    }
}
