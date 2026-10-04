//! Fahrphysik-Kern (datengetrieben, `vehdata::Vehicle`). Fester Zeitschritt 120 Hz (über 60 m/s zwei
//! Unterschritte), unabhängig vom Zeitschritt des Aufrufers.
//!
//! Modell: Einspurmodell mit zwei Achsen, aber getrennten Radlasten links/rechts für die Lastabhängigkeit des
//! Reifens. Längs- und Querlastverlagerung folgen mit der Zeitkonstante des Fahrwerks (`chassis.tau`). Seitenkraft
//! nach vereinfachter Magic Formula, kombiniert über die Reibungsellipse (wer voll bremst, kann nicht voll lenken).
//! Antrieb: Drehmomentkurve mit Leistungsgrenze, Gänge mit automatischem Schalten und Zugkraftunterbrechung,
//! Turbo-Lag, Elektro (konstantes Moment bis zur Eckgeschwindigkeit, dann konstante Leistung, Rekuperation),
//! Muskelkraft (Dauer- bzw. Sprintleistung, Kraftgrenze). Bremse: Druckaufbau, ABS hält knapp unter dem
//! Haftmaximum, ohne ABS blockieren Räder (Gleitreibung entgegen der Gleitrichtung, kein Seitenhalt mehr).
//! Zweiräder: Wheelie- und Stoppie-Grenze. Unter 5 m/s Übergang in ein kinematisches Modell (kein Schlupf-Jitter
//! im Stand); stehende Fahrzeuge rollen nicht weg.
//!
//! Koordinaten: x/y in Metern, Gierwinkel mathematisch (gegen den Uhrzeigersinn); Geschwindigkeiten im
//! Fahrzeugsystem (vx vorwärts, vy nach links).
use crate::vehdata::{Diff, Drive, Esp, Feel, G, Power, RHO, Vehicle};

pub const HZ: f64 = 120.;
pub const STEP: f64 = 1. / HZ;
/// m/s: darüber zwei Unterschritte je 1/120 s
pub const FAST: f64 = 60.;
/// m/s: Übergang kinematisch → dynamisch
pub const KIN: [f64; 2] = [2., 5.];
/// Lastabhängigkeit: mu(Fz) = mu0 · (1 − 0,1 · (Fz/Fz_nenn − 1))
pub const LOAD_SENS: f64 = 0.1;
/// ABS hält diesen Anteil des Haftmaximums, Traktionskontrolle ebenso
pub const ABS_HOLD: f64 = 0.99;
/// … und gibt bei voller Lenkung so viel Längskraft ab, damit Seitenhalt bleibt
pub const ABS_STEER: f64 = 0.35;
/// Hinterachse: ABS/EBD regeln dort vorsichtiger, damit das Heck beim Bremsen Seitenhalt behält
pub const ABS_REAR: f64 = 0.12;
/// ESP: Stellstärke gegen zu großen Schwimmwinkel (1/s je rad Überschreitung)
pub const ESP_BETA_GAIN: f64 = 10.;
pub const TCS_HOLD: f64 = 0.98;
/// durchdrehende Räder: verbleibender Seitenhalt
pub const SPIN_LAT: f64 = 0.4;
/// Rückwärtsgang: Höchsttempo (m/s) und wie lange die Bremse im Stand gehalten werden muss (s)
pub const REV_MAX: f64 = 7.;
pub const REV_HOLD: f64 = 0.3;
/// Bremse: Wärmekapazität je kg Fahrzeugmasse (J/(kg·K)) – ein Stopp aus 100 km/h heizt um rund 120 K;
/// Kühlung durch Stillstand bzw. Fahrtwind (1/s bzw. 1/m)
pub const BRAKE_HEAT_CAP: f64 = 3.2;
pub const BRAKE_COOL: f64 = 0.01;
pub const BRAKE_COOL_V: f64 = 0.004;
/// Bremsfading: Temperatur (K über Umgebung), ab der bzw. bis zu der die Wirkung sinkt; größter Verlust bei
/// `fading` = 2 (Trommel)
pub const FADE_FROM: f64 = 150.;
pub const FADE_FULL: f64 = 450.;
pub const FADE_MAX: f64 = 0.6;
/// ESP: zugelassener Schwimmwinkel (rad) und Gierraten-Abweichung (rad/s) je Modus, Stellstärke (1/s)
pub const ESP_BETA: [f64; 2] = [0.105, 0.23];
pub const ESP_YAW_TOL: [f64; 2] = [0.1, 0.3];
pub const ESP_GAIN: f64 = 6.;
/// Eigenlenkgradient für die Soll-Gierrate (s²/m²)
pub const ESP_K_US: f64 = 0.0015;
/// Lenk-Assist: ab diesem Schwimmwinkel dämpft er die Gierrate zur Soll-Gierrate (1/s bei Stärke 1)
pub const ASSIST_BETA: f64 = 0.05;
pub const ASSIST_RATE: f64 = 2.;
/// Hinterachslenkung: Anteil des Vorderradeinschlags, gegenläufig im Stand, gleichläufig über ~70 km/h
pub const REAR_STEER_LOW: f64 = 0.25;
pub const REAR_STEER_HIGH: f64 = 0.08;
/// Fahrgäste stürzen ab dieser Verzögerung (in g); zurückgesetzt unter `PAX_RESET`
pub const PAX_FALL: f64 = 0.3;
pub const PAX_RESET: f64 = 0.2;
/// Fahrbahnunebenheit: Höhe (m) bei `rough` = 1, Wellenlänge (m), Dämpfungsmaß der Federung
pub const BUMP_H: f64 = 0.012;
pub const BUMP_L: f64 = 0.35;
pub const BUMP_ZETA: f64 = 0.3;

/// Eingabe (Gas, Bremse 0…1; Lenkung −1 rechts … 1 links).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Input {
    pub throttle: f64,
    pub brake: f64,
    pub steer: f64,
    pub handbrake: bool,
    /// Fahrrad: Sprint statt Dauerleistung
    pub sprint: bool,
    /// ESP-Modus des Fahrers (`None` = wie im Fahrzeug); `Some(Off)` schaltet auch die Traktionskontrolle ab
    pub esp: Option<Esp>,
    /// ABS abgeschaltet (Konsole)
    pub no_abs: bool,
}

/// Untergrund an einer Achse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ground {
    pub mu_rel: f64,
    /// Reifenfaktor für die Kategorie (trocken/nass/schnee/eis/lose)
    pub tire_factor: f64,
    pub rolling_extra: f64,
    /// Unebenheit 0…1 (Kopfstein ~0,6): treibt die Federung, Radlast schwankt
    pub rough: f64,
}
impl Ground {
    pub const DRY: Ground = Ground {
        mu_rel: 1.,
        tire_factor: 1.,
        rolling_extra: 0.,
        rough: 0.,
    };
}
/// Untergrund je Achse (vorn, hinten).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Env {
    pub axle: [Ground; 2],
}
impl Default for Env {
    fn default() -> Self {
        Self {
            axle: [Ground::DRY; 2],
        }
    }
}

/// Zustand eines Fahrzeugs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub x: f64,
    pub y: f64,
    pub yaw: f64,
    pub vx: f64,
    pub vy: f64,
    pub r: f64,
    /// Zuladung 0…1
    pub load: f64,
    pub gear: usize,
    pub shift_t: f64,
    pub boost: f64,
    /// gefilterte Beschleunigungen für die Lastverlagerung (m/s²)
    pub ax_f: f64,
    pub ay_f: f64,
    pub brake_p: f64,
    /// Radeinschlag vorn (rad)
    pub delta: f64,
    pub rpm: f64,
    /// Schlupf (längs) je Achse, −1…1 (Anzeige), durchdrehend/blockierend
    pub slip: [f64; 2],
    pub spin: [bool; 2],
    pub locked: [bool; 2],
    pub abs: bool,
    pub tcs: bool,
    /// Radlasten (N): vorn links, vorn rechts, hinten links, hinten rechts
    pub fz: [f64; 4],
    /// Schräglaufwinkel je Achse (rad)
    pub alpha: [f64; 2],
    /// zurückgelegter Weg (m)
    pub dist: f64,
    /// Rückwärtsgang eingelegt; Zeit, die die Bremse im Stand gehalten wird
    pub reverse: bool,
    pub rev_t: f64,
    /// Bremsentemperatur (K über Umgebung)
    pub brake_temp: f64,
    /// ESP greift ein (vorige Unterschritte); Untersteuern 0…1 (Anzeige)
    pub esp_active: bool,
    pub understeer: f64,
    /// Radeinschlag hinten (rad, Hinterachslenkung)
    pub delta_r: f64,
    /// Federweg je Achse (m, positiv = eingefedert) und Geschwindigkeit
    pub susp: [f64; 2],
    pub susp_v: [f64; 2],
    /// Vollbremsung mit stehenden Fahrgästen: Zähler (je Bremsung einmal) und laufender Zustand
    pub passenger_falls: u32,
    pub hard_brake: bool,
}
impl State {
    pub fn speed(&self) -> f64 {
        self.vx.hypot(self.vy)
    }
    /// Schwimmwinkel (rad)
    pub fn beta(&self) -> f64 {
        if self.vx.abs() < 0.5 {
            0.
        } else {
            self.vy.atan2(self.vx.abs())
        }
    }
}

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

/// Bezugstempo eines Fahrzeugs (Ziel-Vmax oder aus der Leistung), für die Elektro-Eckgeschwindigkeit.
pub fn vmax_ref(v: &Vehicle) -> f64 {
    let (m, _) = v.loaded(v.calib_load);
    v.targets
        .get("vmax")
        .map(|k| k / 3.6)
        .unwrap_or_else(|| v.power_limited_vmax(m))
}

/// Größter Radeinschlag bei Tempo v: δ_max / (1 + (v/v_s)²), mindestens δ_hs.
pub fn steer_limit(v: &Vehicle, speed: f64) -> f64 {
    let s = &v.steering;
    (s.delta_max / (1. + (speed / s.v_s).powi(2))).max(s.delta_hs.min(s.delta_max))
}

/// Zugkraft am Rad bei Volllast-Anteil `throttle` (N), ohne Traktionsgrenze; aktualisiert Drehzahl und Turbo.
/// Anteil des Moments, der während eines Gangwechsels am Rad ankommt.
pub fn shift_torque(kind: &str) -> f64 {
    match kind {
        "dsg" => 0.9,
        k if k.starts_with("wandler") => 0.6,
        "manuell_quickshifter" | "manuell_sequenziell" => 0.5,
        _ => 0.,
    }
}

fn drive_force(v: &Vehicle, s: &mut State, inp: &Input, speed: f64, dt: f64) -> f64 {
    let th = inp.throttle.clamp(0., 1.);
    let eta = v.efficiency;
    let r = v.wheel_r;
    let f = match &v.engine.power {
        Power::Muscle {
            force_max,
            w_cont,
            w_sprint,
        } => {
            let p = if inp.sprint { *w_sprint } else { *w_cont };
            (p * eta / speed.max(0.5)).min(*force_max) * th
        }
        Power::Electric { corner_frac } => {
            // Höchstdrehzahl des Motors bei der Bezugs-Vmax (einstufiges Getriebe)
            let vm = vmax_ref(v);
            let vc = (vm * corner_frac).max(1.);
            let p = v.engine.watts * eta;
            s.rpm = speed / vc * 4000.;
            (p / vc).min(p / speed.max(0.1)) * th * ((vm - speed) / 0.5).clamp(0., 1.)
        }
        Power::Curve(_) => {
            let e = &v.engine;
            if v.gearbox.cvt {
                // stufenlos: Motor hält die Drehzahl der Spitzenleistung
                s.rpm = e.n_peak_power * th.max(0.3);
                let low = v.gearbox.ratios[0] * 6.;
                (e.nm * low * eta / r).min(e.peak_watts * eta / speed.max(0.5)) * th
            } else {
                // Turbo: Ladedruck folgt dem Gas mit `lag`; ohne Ladedruck rund 55 % Moment. Launch Control spannt
                // den Ladedruck im Stand vor.
                if v.launch && speed < 3. && th > 0.9 {
                    s.boost = 1.;
                } else if e.lag > 0. {
                    s.boost += (th - s.boost) * (dt / e.lag).min(1.);
                } else {
                    s.boost = th;
                }
                let ratio = v.gearbox.ratios[s.gear.min(v.gearbox.ratios.len() - 1)];
                let n_wheel = speed / r * ratio * 60. / (2. * std::f64::consts::PI);
                // anfahren: Kupplung schleift bei der Drehzahl, ab der das volle Moment anliegt
                let launch = e.n_max
                    * match &e.power {
                        Power::Curve(c) => c
                            .iter()
                            .find(|p| p.1 >= 0.999)
                            .map_or(0.45, |p| p.0)
                            .clamp(0.3, 0.55),
                        _ => 0.45,
                    };
                let n = n_wheel.max(e.n_idle + th * (launch - e.n_idle));
                s.rpm = n.min(e.n_max);
                if n_wheel >= e.n_max {
                    0.
                } else {
                    // Schalten: Doppelkupplung und Wandler schalten unter Last, Handschalter trennen die Kupplung
                    let during = if s.shift_t > 0. {
                        shift_torque(&v.gearbox.kind)
                    } else {
                        1.
                    };
                    let boost = if e.lag > 0. {
                        0.55 + 0.45 * s.boost
                    } else {
                        1.
                    };
                    e.torque(n) * boost * th * ratio * eta / r * during
                }
            }
        }
    };
    // Rückwärtsgang: Zugkraft nach hinten, Tempo begrenzt
    if s.reverse {
        return -f * ((REV_MAX - speed) / 0.5).clamp(0., 1.);
    }
    // Begrenzer
    match v.limiter {
        Some(l) => f * ((l - speed) / 0.5).clamp(0., 1.),
        None => f,
    }
}

/// Massenfaktor der Drehträgheit: 1,04 + 0,0018·i², höchstens 1,6 (die Lehrbuch-Faustformel nennt 0,0025; für
/// heutige leichte Antriebsstränge ergibt das im ersten Gang zu viel, Pkw liegen dort bei 1,3–1,4). Elektro
/// einstufig rund 1,08, Fahrrad 1,03.
pub fn mass_factor(v: &Vehicle, s: &State) -> f64 {
    match &v.engine.power {
        Power::Muscle { .. } => 1.03,
        Power::Electric { .. } => 1.08,
        Power::Curve(_) if v.gearbox.cvt => 1.1,
        // Zweirad: leichter Antriebsstrang im Verhältnis zur Masse
        Power::Curve(_) if v.two_wheel => {
            let i = v.gearbox.ratios[s.gear.min(v.gearbox.ratios.len() - 1)];
            (1.04 + 0.0004 * i * i).min(1.2)
        }
        // beim Bremsen und Schalten ist der Motor ausgekuppelt: nur die Räder drehen mit
        Power::Curve(_) if s.brake_p > 0.05 || s.shift_t > 0. => 1.04,
        Power::Curve(_) => {
            let i = v.gearbox.ratios[s.gear.min(v.gearbox.ratios.len() - 1)];
            (1.04 + 0.0018 * i * i).min(1.6)
        }
    }
}

/// Automatik: hochschalten nahe n_max bei Volllast, früher bei Teillast; runterschalten bei niedriger Drehzahl.
fn shift(v: &Vehicle, s: &mut State, inp: &Input, speed: f64, dt: f64) {
    let n_gears = v.gearbox.ratios.len();
    if n_gears <= 1 || !matches!(v.engine.power, Power::Curve(_)) || v.gearbox.cvt || s.reverse {
        return;
    }
    if s.shift_t > 0. {
        s.shift_t -= dt;
        return;
    }
    let e = &v.engine;
    let to_n =
        |g: usize| speed / v.wheel_r * v.gearbox.ratios[g] * 60. / (2. * std::f64::consts::PI);
    let th = inp.throttle.clamp(0., 1.);
    let up = e.n_max * (0.6 + 0.37 * th);
    let down = e.n_max * (0.28 + 0.2 * th);
    if s.gear + 1 < n_gears && to_n(s.gear) > up {
        // nur hoch, wenn der nächste Gang mehr Zugkraft am Rad liefert oder die Drehzahlgrenze erreicht ist
        s.gear += 1;
        s.shift_t = v.gearbox.shift;
    } else if s.gear > 0 && to_n(s.gear) < down && to_n(s.gear - 1) < e.n_max * 0.9 {
        s.gear -= 1;
        s.shift_t = v.gearbox.shift * 0.5;
    }
}

/// Ein Schritt über `dt` (beliebig; intern in 1/120 s bzw. 1/240 s).
pub fn step(v: &Vehicle, feel: &Feel, s: &mut State, inp: &Input, env: &Env, dt: f64) {
    let mut left = dt;
    while left > 1e-9 {
        let h = if s.speed() > FAST { STEP / 2. } else { STEP }.min(left);
        let eff = select_direction(v, s, inp, h);
        substep(v, feel, s, &eff, env, h);
        left -= h;
    }
}

/// Rückwärtsgang wie im Spiel üblich: Bremse im Stand halten legt ihn ein, dann treibt die Bremstaste rückwärts
/// und das Gaspedal bremst; Gas im Stand legt wieder den Vorwärtsgang ein. Fahrräder schieben nicht rückwärts.
pub fn select_direction(v: &Vehicle, s: &mut State, inp: &Input, dt: f64) -> Input {
    if matches!(v.engine.power, Power::Muscle { .. }) {
        return *inp;
    }
    if s.reverse {
        if inp.throttle > 0.05 && s.vx > -0.4 {
            s.reverse = false;
            s.rev_t = 0.;
        }
    } else if s.vx.abs() < 0.4 && inp.brake > 0.3 && inp.throttle < 0.05 {
        s.rev_t += dt;
        if s.rev_t >= REV_HOLD {
            s.reverse = true;
            s.gear = 0;
        }
    } else {
        s.rev_t = 0.;
    }
    if s.reverse {
        Input {
            throttle: inp.brake,
            brake: inp.throttle,
            ..*inp
        }
    } else {
        *inp
    }
}

/// Wirkung der Bremse bei Temperatur `temp` (1 = kalt).
pub fn brake_fade(v: &Vehicle, temp: f64) -> f64 {
    1. - FADE_MAX * (v.brake.fading / 2.).clamp(0., 1.) * smooth(FADE_FROM, FADE_FULL, temp)
}

/// ESP-Modus, der gilt: die Fahrereinstellung – außer das Fahrzeug hat gar kein ESP (Oldtimer, Drift-Aufbau);
/// der Datensatz nennt die Grundeinstellung.
pub fn esp_mode(v: &Vehicle, inp: &Input) -> Esp {
    match inp.esp {
        Some(e) if v.esp != Esp::Off => e,
        _ => v.esp,
    }
}

/// Soll-Gierrate aus Lenkung und Tempo (Einspurmodell mit leichtem Untersteuern), begrenzt durch die Haftung.
pub fn yaw_reference(v: &Vehicle, vx: f64, delta: f64, delta_r: f64, mu: f64) -> f64 {
    let l = v.wheelbase;
    let r = vx * (delta.tan() - delta_r.tan()) / (l * (1. + ESP_K_US * vx * vx));
    let cap = mu * G / vx.abs().max(1.);
    r.clamp(-cap, cap)
}

/// Höhe der Fahrbahn (m) an Wegpunkt `x` (geglättetes Rauschen, deterministisch).
fn road_height(x: f64, rough: f64) -> f64 {
    if rough <= 0. {
        return 0.;
    }
    let u = x / BUMP_L;
    let i = u.floor();
    let f = u - i;
    let h = |n: f64| {
        let z = (n * 127.1 + 311.7).sin() * 43758.5453;
        (z - z.floor()) * 2. - 1.
    };
    let t = f * f * (3. - 2. * f);
    (h(i) + (h(i + 1.) - h(i)) * t) * BUMP_H * rough
}

fn substep(v: &Vehicle, feel: &Feel, s: &mut State, inp: &Input, env: &Env, dt: f64) {
    let (m, h) = v.loaded(s.load);
    let (a, b) = v.axle_distances();
    let l = v.wheelbase;
    let iz = v.iz * m / v.mass_empty;
    let speed = s.vx.abs();
    let dir = if s.vx < -0.05 { -1. } else { 1. };
    // Lenkung: Ziel aus der Eingabe, tempoabhängig begrenzt; Rate: volle Auslenkung in 0,25 s (langsamer bei Tempo)
    let lim = steer_limit(v, speed);
    let target = inp.steer.clamp(-1., 1.) * lim;
    let rate = v.steering.delta_max / 0.25 * v.steering.rate_factor / (1. + speed / 30.);
    let back = target.abs() < s.delta.abs() && target.signum() == s.delta.signum() || target == 0.;
    let step_max = rate * if back { 2. } else { 1. } * dt;
    s.delta += (target - s.delta).clamp(-step_max, step_max);
    let delta = s.delta;
    // Hinterachslenkung: langsam gegenläufig (wendiger), schnell gleichläufig (stabiler)
    s.delta_r = if v.steering.rear {
        let k = REAR_STEER_LOW - (REAR_STEER_LOW + REAR_STEER_HIGH) * smooth(10., 19., speed);
        -k * delta
    } else {
        0.
    };
    let delta_r = s.delta_r;
    let esp = esp_mode(v, inp);
    let tcs = v.tcs && inp.esp != Some(Esp::Off);
    let abs = v.brake.abs && !inp.no_abs;
    // Bremsdruck mit Aufbauzeit
    let bt = v.brake.build.max(0.02);
    s.brake_p += (inp.brake.clamp(0., 1.) - s.brake_p).clamp(-dt / 0.05, dt / bt);
    // Radlasten: statisch, Abtrieb, Lastverlagerung (gefiltert)
    let tr = feel.transfer();
    let down = 0.5 * RHO * v.cl_a * s.vx * s.vx;
    let fz_f0 = m * G * v.front + down * v.front;
    let fz_r0 = m * G * (1. - v.front) + down * (1. - v.front);
    let dlong = m * s.ax_f * h / l * tr;
    let mut fz_axle = [(fz_f0 - dlong).max(0.), (fz_r0 + dlong).max(0.)];
    // Federung je Achse über Fahrbahnunebenheiten: Feder-Dämpfer mit der Eigenfrequenz des Fahrwerks; die
    // Federkraftschwankung ist die Radlastschwankung (hartes Fahrwerk auf Kopfstein = weniger Grip)
    let w0 = 2. * std::f64::consts::PI * v.chassis.hz.max(0.3);
    for (i, fz) in fz_axle.iter_mut().enumerate() {
        let rough = env.axle[i].rough;
        if rough <= 0. && s.susp[i] == 0. && s.susp_v[i] == 0. {
            continue;
        }
        let x = s.dist - if i == 1 { l } else { 0. };
        let y = road_height(x, rough);
        let y1 = road_height(x + speed * dt, rough);
        let yv = (y1 - y) / dt;
        let acc = w0 * w0 * (y - s.susp[i]) + 2. * BUMP_ZETA * w0 * (yv - s.susp_v[i]);
        s.susp_v[i] += acc * dt;
        s.susp[i] += s.susp_v[i] * dt;
        let m_ax = m * if i == 0 { v.front } else { 1. - v.front };
        *fz = (*fz + m_ax * acc).max(0.);
    }
    let dlat = if v.track > 0. {
        m * s.ay_f * h / v.track * tr
    } else {
        0.
    };
    let share = [v.front, 1. - v.front];
    let nom = [m * G * v.front / 2., m * G * (1. - v.front) / 2.];
    let mut cap_lat = [0.; 2];
    for i in 0..2 {
        let g = env.axle[i];
        let mu0 = (v.tire.mu * g.mu_rel * g.tire_factor * feel.grip()).max(feel.ice_grip_min);
        let d = dlat * share[i] / 2.;
        let wheels = [
            (fz_axle[i] / 2. - d).clamp(0., fz_axle[i]),
            (fz_axle[i] / 2. + d).clamp(0., fz_axle[i]),
        ];
        s.fz[i * 2] = wheels[0];
        s.fz[i * 2 + 1] = wheels[1];
        cap_lat[i] = wheels
            .iter()
            .map(|&fz| {
                let mu = mu0 * (1. - LOAD_SENS * (fz / nom[i].max(1.) - 1.)).max(0.5);
                mu * fz
            })
            .sum();
    }
    let cap_long = [cap_lat[0] * v.tire.mu_long, cap_lat[1] * v.tire.mu_long];
    // Antrieb, Schalten
    shift(v, s, inp, speed, dt);
    let mut f_drive = drive_force(v, s, inp, speed, dt);
    // Motorbremse bzw. Rekuperation beim Gaswegnehmen (auf der Antriebsachse)
    let coast = inp.throttle < 0.05 && speed > 1.;
    let mut f_coast = 0.;
    if coast {
        f_coast = m
            * G
            * match &v.engine.power {
                Power::Electric { .. } => v.recuperation_g * smooth(1., 5., speed),
                Power::Muscle { .. } => 0.,
                Power::Curve(_) => {
                    let n = v.gearbox.ratios.len();
                    let low = if n > 1 {
                        1. - s.gear as f64 / (n - 1) as f64
                    } else {
                        0.
                    };
                    let retarder = if v.brake.retarder.is_some() { 0.03 } else { 0. };
                    0.03 + 0.05 * low + retarder
                }
            };
    }
    // Zweiräder: Wheelie-/Stoppie-Grenze (der Fahrer dosiert; Wheelie-Control hält genau an der Grenze)
    if v.two_wheel {
        let wheelie = m * G * v.front * l / h;
        f_drive = f_drive.min(wheelie + 0.5 * RHO * v.cw_a * s.vx * s.vx);
    }
    // Drehträgheit von Rädern, Antriebsstrang und Motor (`mass_factor`): sie zehrt am Motormoment, nicht an der
    // Haftgrenze. Am Reifen kommt das Moment an, das nach dem Beschleunigen der Drehmassen übrig bleibt; ist die
    // Haftung die Grenze, beschleunigt das Auto mit der vollen Reifenkraft.
    let still = speed < 0.05;
    let cr = v.tire.rolling + (env.axle[0].rolling_extra + env.axle[1].rolling_extra) / 2.;
    let f_roll = if still { 0. } else { cr * m * G * dir };
    let f_drag = 0.5 * RHO * v.cw_a * s.vx * s.vx.abs();
    if f_drive != 0. {
        let df = mass_factor(v, s);
        let a_free = (f_drive - f_drag - f_roll) / (m * df);
        f_drive -= (df - 1.) * m * a_free;
    }
    let split = match v.drive {
        Drive::Fwd => [1., 0.],
        Drive::Rwd => [0., 1.],
        Drive::Awd => [v.awd_front, 1. - v.awd_front],
    };
    // ESP nimmt Gas weg, solange es eingreift (voll fast ganz, Sport nur teilweise)
    if s.esp_active && f_drive * dir > 0. {
        f_drive *= if esp == Esp::Full { 0.1 } else { 0.35 };
    }
    let mut f_brake = s.brake_p * m * G * v.brake.gain * feel.brake() * brake_fade(v, s.brake_temp);
    if v.two_wheel {
        f_brake = f_brake.min(m * G * (1. - v.front) * l / h);
    }
    let bsplit = [v.brake.front, 1. - v.brake.front];
    // Geschwindigkeit am Reifen (Fahrzeugsystem), Schräglauf
    let vy_ax = [s.vy + a * s.r, s.vy - b * s.r];
    let vxs = speed.max(0.5);
    let alpha = [
        vy_ax[0].atan2(vxs) - delta * dir,
        vy_ax[1].atan2(vxs) - delta_r * dir,
    ];
    s.alpha = alpha;
    let (bb, cc) = v.tire.shape();
    let mf = |x: f64| (cc * (bb * x).atan()).sin();
    let mut drive = [f_drive * split[0], f_drive * split[1]];
    if v.drive == Drive::Awd {
        // Allrad: Moment, das eine Achse nicht übertragen kann, wandert zur anderen (Kupplung bzw. Doppelmotor)
        for i in 0..2 {
            let over = drive[i] - cap_long[i] * TCS_HOLD;
            if over > 0. {
                drive[i] -= over;
                drive[1 - i] += over;
            }
        }
    }
    let mut brake_ax = [f_brake * bsplit[0], f_brake * bsplit[1]];
    if v.two_wheel {
        // Zweirad: die Vorderbremse trägt, was ihre Haftung hergibt, den Rest die Hinterbremse
        let front = f_brake.min(cap_long[0] * ABS_HOLD);
        brake_ax = [front, f_brake - front];
    }
    let mut fx = [0.; 2];
    let mut fy = [0.; 2];
    s.abs = false;
    s.tcs = false;
    for i in 0..2 {
        let drive_i = drive[i];
        let brake_i = brake_ax[i] + f_coast * split[i];
        let locked_hb = inp.handbrake && i == 1 && !v.two_wheel;
        // Bremsen wirkt entgegen der Bewegung; im Stand hält es (Haftreibung)
        let demand = drive_i - if still { 0. } else { brake_i * dir };
        let c = cap_long[i].max(1.);
        let mut lat_k = 1.;
        s.spin[i] = false;
        s.locked[i] = false;
        if locked_hb || (!abs && brake_i > c && !still && drive_i < brake_i) {
            // blockiert: Gleitreibung entgegen der Gleitrichtung des Reifens (im Reifensystem; vorn um den
            // Lenkwinkel gedreht), kein Seitenhalt – ein blockiertes Vorderrad lenkt nicht
            s.locked[i] = true;
            let d = if i == 0 { delta } else { 0. };
            let (sd, cd) = d.sin_cos();
            let (vl, vt) = (s.vx * cd + vy_ax[i] * sd, -s.vx * sd + vy_ax[i] * cd);
            let vc = vl.hypot(vt).max(0.1);
            let f = c * v.tire.slide_ratio;
            fx[i] = -f * vl / vc;
            fy[i] = -f * vt / vc;
            s.slip[i] = -1.;
            continue;
        }
        if demand.abs() <= c {
            fx[i] = demand;
            s.slip[i] = demand / c * v.tire.peak_slip_ratio;
        } else if demand * dir > 0. || still {
            // Antrieb über der Haftung
            if tcs {
                s.tcs = true;
                fx[i] = demand.signum() * c * TCS_HOLD;
                s.slip[i] = v.tire.peak_slip_ratio * demand.signum();
            } else {
                s.spin[i] = true;
                fx[i] = demand.signum() * c * v.tire.slide_ratio;
                s.slip[i] = demand.signum() * 0.5;
                lat_k = SPIN_LAT;
            }
        } else {
            // Bremsen über der Haftung: ABS hält knapp darunter; bei Lenkeinschlag gibt es Längskraft zugunsten
            // der Lenkbarkeit ab
            s.abs = true;
            let rear = if i == 1 {
                ABS_REAR * (0.5 + inp.steer.abs())
            } else {
                0.
            };
            fx[i] = demand.signum() * c * (ABS_HOLD - ABS_STEER * inp.steer.abs() - rear);
            s.slip[i] = -v.tire.peak_slip_ratio;
        }
        let used = (fx[i] / c).clamp(-1., 1.);
        let lat = cap_lat[i] * (1. - used * used).max(0.).sqrt() * lat_k;
        fy[i] = -lat * mf(alpha[i]);
    }
    // Sperrdifferential: weniger Leistungsverlust durch durchdrehende Räder (Phase 3 verfeinert)
    if v.diff != Diff::Open {
        for (f, spin) in fx.iter_mut().zip(s.spin) {
            if spin {
                *f *= 1.04;
            }
        }
    }
    let (sd, cd) = delta.sin_cos();
    let (sr, cr_) = delta_r.sin_cos();
    let mut fxb = fx[0] * cd - fy[0] * sd + fx[1] * cr_ - fy[1] * sr - f_drag - f_roll;
    let fyb = fx[0] * sd + fy[0] * cd + fx[1] * sr + fy[1] * cr_;
    let mut mz = a * (fx[0] * sd + fy[0] * cd) - b * (fx[1] * sr + fy[1] * cr_);
    // Soll-Gierrate (ESP, Lenk-Assist, Untersteuer-Anzeige)
    // (Querhaftung, die neben der aktuellen Längsbeschleunigung übrig bleibt – das misst ein echtes ESP über den
    // Querbeschleunigungssensor)
    let mu_now = v.tire.mu * env.axle[0].mu_rel * env.axle[0].tire_factor * feel.grip();
    let long_used = (s.ax_f / (mu_now * v.tire.mu_long * G).max(0.1)).clamp(-1., 1.);
    let mu_lat = mu_now * (1. - long_used * long_used).max(0.).sqrt().max(0.2);
    let r_ref = yaw_reference(v, s.vx, delta, delta_r, mu_lat);
    s.understeer = if speed > 5. && r_ref.abs() > 0.05 && s.r * r_ref > 0. {
        ((r_ref.abs() - s.r.abs()) / r_ref.abs()).clamp(0., 1.)
    } else {
        0.
    };
    // ESP: weicht Gierrate oder Schwimmwinkel zu weit ab, bremst es einzelne Räder (Giermoment gegen die
    // Abweichung, Verzögerung durch die einseitige Bremse) und nimmt Gas weg
    s.esp_active = false;
    // (ESP braucht das ABS-Steuergerät: ohne ABS kein ESP)
    if esp != Esp::Off && abs && speed > 5. && v.track > 0. {
        let k = if esp == Esp::Full { 0 } else { 1 };
        let err = s.r - r_ref;
        let beta = s.beta();
        if err.abs() > ESP_YAW_TOL[k] || beta.abs() > ESP_BETA[k] {
            let excess = err.signum() * (err.abs() - ESP_YAW_TOL[k] * 0.5).max(0.);
            // zu großer Schwimmwinkel: Gierrate abbauen (Heck kommt), unabhängig von der Soll-Gierrate
            let slide = (beta.abs() - ESP_BETA[k] * 0.7).max(0.) * s.r.signum();
            let want = -(ESP_GAIN * excess + ESP_BETA_GAIN * slide) * iz;
            let half = v.track / 2.;
            let cap = (cap_long[0] + cap_long[1]) * 0.5 * half;
            let m_esp = want.clamp(-cap, cap);
            if m_esp.abs() > 1. {
                mz += m_esp;
                // einseitiges Bremsen verzögert zusätzlich – regelt das ABS schon an der Haftgrenze, entsteht das
                // Moment stattdessen durch Lösen einer Seite (weniger Verzögerung)
                let f_side = m_esp.abs() / half;
                if s.abs {
                    let braking = (fx[0] + fx[1]).abs();
                    fxb += f_side.min(braking * 0.5) * dir;
                } else {
                    fxb -= f_side * dir;
                }
                s.esp_active = true;
            }
        }
    }
    // Lenk-Assist (Spielgefühl): wer nicht gegenlenkt, dem dämpft er das Ausbrechen leicht
    if feel.steer_assist > 0. && speed > 5. && s.beta().abs() > ASSIST_BETA && inp.steer * s.r >= 0.
    {
        mz -= (s.r - r_ref) * iz * feel.steer_assist * ASSIST_RATE;
    }
    // Integration (halbimplizit); die Drehträgheit steckt schon in der Antriebskraft
    let ax = fxb / m;
    // Bremse heizt mit der umgesetzten Bremsleistung, Stillstand und Fahrtwind kühlen
    if !still {
        let used = (brake_ax[0] + brake_ax[1]).min(cap_long[0] + cap_long[1]);
        s.brake_temp += used * speed / (m * BRAKE_HEAT_CAP) * dt;
    }
    s.brake_temp -= s.brake_temp * (BRAKE_COOL + BRAKE_COOL_V * speed) * dt;
    // Linienbus: Vollbremsung mit stehenden Fahrgästen (ein Ereignis je Bremsung)
    if v.flags.contains_key("fahrgaeste") {
        if s.brake_p > 0.3 && -ax * dir > PAX_FALL * G {
            if !s.hard_brake {
                s.hard_brake = true;
                s.passenger_falls += 1;
            }
        } else if -ax * dir < PAX_RESET * G {
            s.hard_brake = false;
        }
    }
    let ay = fyb / m;
    let mut vx = s.vx + (ax + s.r * s.vy) * dt;
    let mut vy = s.vy + (ay - s.r * s.vx) * dt;
    let mut r = s.r + mz / iz * dt;
    // im Stand nicht durch Bremse oder Rollwiderstand rückwärts
    if s.vx != 0. && vx.signum() != s.vx.signum() && f_drive * dir <= 0. {
        vx = 0.;
    }
    if still && f_drive.abs() < 1e-6 {
        vx = 0.;
    }
    // unter 2…5 m/s kinematisch: Gierrate aus Lenkung, kein Querschlupf
    let w = smooth(KIN[0], KIN[1], vx.abs());
    if w < 1. {
        // ein blockiertes Vorderrad lenkt auch hier nicht
        let d_f = if s.locked[0] { 0. } else { delta };
        let r_kin = vx * (d_f.tan() - delta_r.tan()) / l;
        r = r_kin + (r - r_kin) * w;
        vy *= w;
    }
    // gefilterte Lastverlagerung
    let tau = v.chassis.tau.max(0.02);
    s.ax_f += (ax - s.ax_f) * (dt / tau).min(1.);
    s.ay_f += (ay - s.ay_f) * (dt / tau).min(1.);
    s.vx = vx;
    s.vy = vy;
    s.r = r;
    let (sy, cy) = s.yaw.sin_cos();
    s.x += (vx * cy - vy * sy) * dt;
    s.y += (vx * sy + vy * cy) * dt;
    s.yaw += r * dt;
    s.dist += vx.hypot(vy) * dt;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vehdata::VehicleDb;

    fn db() -> VehicleDb {
        VehicleDb::embedded().unwrap()
    }
    fn run(v: &Vehicle, s: &mut State, inp: Input, secs: f64) {
        let feel = Feel::simulation();
        for _ in 0..(secs * HZ) as usize {
            step(v, &feel, s, &inp, &Env::default(), STEP);
        }
    }

    #[test]
    fn standing_car_does_not_roll_or_jitter() {
        let db = db();
        let v = db.get("kompakt_benzin").unwrap();
        let mut s = State::default();
        run(v, &mut s, Input::default(), 5.);
        assert_eq!((s.x, s.y, s.vx, s.vy, s.r), (0., 0., 0., 0., 0.));
        // Bremse im Stand: kurz getippt kein Rückwärtsrollen …
        let brake = Input {
            brake: 1.,
            ..Default::default()
        };
        run(v, &mut s, brake, REV_HOLD * 0.8);
        assert_eq!(s.vx, 0.);
        assert!(!s.reverse);
        // … gehalten legt sie den Rückwärtsgang ein, das Tempo ist begrenzt
        run(v, &mut s, brake, 6.);
        assert!(s.reverse);
        assert!(s.vx < -REV_MAX * 0.9 && s.vx >= -REV_MAX - 0.1, "{}", s.vx);
        // Gas bremst rückwärts, im Stand gilt wieder vorwärts
        let gas = Input {
            throttle: 1.,
            ..Default::default()
        };
        run(v, &mut s, gas, 3.);
        assert!(!s.reverse && s.vx > 0., "{}", s.vx);
    }

    #[test]
    fn deterministic_for_equal_input() {
        let db = db();
        let v = db.get("sportwagen_s").unwrap();
        let go = |s: &mut State| {
            run(
                v,
                s,
                Input {
                    throttle: 1.,
                    ..Default::default()
                },
                4.,
            );
            run(
                v,
                s,
                Input {
                    throttle: 0.6,
                    steer: 0.4,
                    ..Default::default()
                },
                3.,
            );
            run(
                v,
                s,
                Input {
                    brake: 1.,
                    steer: -0.2,
                    ..Default::default()
                },
                2.,
            );
        };
        let (mut a, mut b) = (State::default(), State::default());
        go(&mut a);
        go(&mut b);
        assert_eq!(a, b);
        // gleicher Ablauf mit 60-Hz-Aufrufen statt 120 Hz: identisch (fester innerer Zeitschritt)
        let mut c = State::default();
        let feel = Feel::simulation();
        let env = Env::default();
        for (inp, secs) in [
            (
                Input {
                    throttle: 1.,
                    ..Default::default()
                },
                4.,
            ),
            (
                Input {
                    throttle: 0.6,
                    steer: 0.4,
                    ..Default::default()
                },
                3.,
            ),
            (
                Input {
                    brake: 1.,
                    steer: -0.2,
                    ..Default::default()
                },
                2.,
            ),
        ] {
            for _ in 0..(secs * 60.) as usize {
                step(v, &feel, &mut c, &inp, &env, 1. / 60.);
            }
        }
        assert!(
            (c.x - a.x).abs() < 1e-6 && (c.y - a.y).abs() < 1e-6,
            "{} {}",
            c.x,
            a.x
        );
    }

    #[test]
    fn weight_transfer_loads_the_rear_when_accelerating_and_the_front_when_braking() {
        let db = db();
        let v = db.get("kompakt_benzin").unwrap();
        let mut s = State::default();
        run(
            v,
            &mut s,
            Input {
                throttle: 1.,
                ..Default::default()
            },
            2.,
        );
        let (f, r) = (s.fz[0] + s.fz[1], s.fz[2] + s.fz[3]);
        let m = v.mass_empty;
        assert!(r > m * G * (1. - v.front) * 1.05, "hinten belastet: {r}");
        run(
            v,
            &mut s,
            Input {
                throttle: 1.,
                ..Default::default()
            },
            6.,
        );
        run(
            v,
            &mut s,
            Input {
                brake: 1.,
                ..Default::default()
            },
            0.6,
        );
        let f2 = s.fz[0] + s.fz[1];
        assert!(f2 > m * G * v.front * 1.2 && f2 > f, "vorn belastet: {f2}");
        // Summe der Radlasten = Gewicht (ohne Abtrieb)
        let sum: f64 = s.fz.iter().sum();
        assert!((sum - m * G).abs() < m * G * 0.02, "{sum}");
    }

    #[test]
    fn locked_wheels_without_abs_stop_longer_and_cannot_steer() {
        let db = db();
        let base = db.get("kompakt_benzin").unwrap().clone();
        let mut no_abs = base.clone();
        no_abs.brake.abs = false;
        let stop = |v: &Vehicle, steer: f64| {
            let mut s = State {
                vx: 100. / 3.6,
                gear: 4,
                ..Default::default()
            };
            // bis zum Stillstand (danach legte die gehaltene Bremse den Rückwärtsgang ein)
            let inp = Input {
                brake: 1.,
                steer,
                ..Default::default()
            };
            let feel = Feel::simulation();
            // Kurswinkel (Bewegungsrichtung), sobald das Auto unter 5 m/s fällt: blockiert dreht sich der Aufbau
            // womöglich, die Bahn aber nicht
            let mut course = 0.;
            for _ in 0..(8. * HZ) as usize {
                step(v, &feel, &mut s, &inp, &Env::default(), STEP);
                if s.speed() < 6. && course == 0. {
                    course = (s.yaw + s.vy.atan2(s.vx)).abs();
                }
                if s.vx <= 0. {
                    break;
                }
            }
            (s.dist, course)
        };
        // geradeaus: ohne ABS 10 bis 20 % länger (Prompt)
        let (d_abs, _) = stop(&base, 0.);
        let (d_lock, _) = stop(&no_abs, 0.);
        assert!(
            d_lock > d_abs * 1.1 && d_lock < d_abs * 1.2,
            "ABS {d_abs}, blockiert {d_lock}"
        );
        // mit Lenkeinschlag: mit ABS dreht das Auto in die Kurve, blockiert schiebt es geradeaus (gemessen an der
        // Richtungsänderung bis zum Stillstand; der Rest blockiert entsteht in der Aufbauzeit)
        let (_, yaw_abs) = stop(&base, 0.5);
        let (_, yaw_lock) = stop(&no_abs, 0.5);
        assert!(
            yaw_abs > 0.15 && yaw_abs > yaw_lock * 3.,
            "mit ABS lenkbar: {yaw_abs} gegen {yaw_lock}"
        );
    }

    #[test]
    fn electric_pulls_hard_from_standstill_and_holds_power_above_the_corner() {
        let db = db();
        let v = db.get("e_performance").unwrap();
        let mut s = State::default();
        let mut inp = Input {
            throttle: 1.,
            ..Default::default()
        };
        let p = &mut s;
        drive_force(v, p, &inp, 0., STEP);
        let low = drive_force(v, p, &inp, 5., STEP);
        let high = drive_force(v, p, &inp, 60., STEP);
        assert!((low * 5. - high * 60.).abs() > 0. && low > high);
        assert!(
            (high * 60. - v.engine.watts * v.efficiency).abs() < 1.,
            "konstante Leistung"
        );
        inp.throttle = 0.;
        assert_eq!(drive_force(v, p, &inp, 30., STEP), 0.);
    }

    /// Vollbremsung aus `kmh`, Bremsweg; Bremstemperatur bleibt im Zustand.
    fn brake_stop(v: &Vehicle, s: &mut State, kmh: f64) -> f64 {
        let feel = Feel::simulation();
        (s.vx, s.vy, s.r, s.reverse, s.rev_t) = (kmh / 3.6, 0., 0., false, 0.);
        s.gear = v.gearbox.ratios.len() - 1;
        let d0 = s.dist;
        let inp = Input {
            brake: 1.,
            ..Default::default()
        };
        for _ in 0..(20. * HZ) as usize {
            step(v, &feel, s, &inp, &Env::default(), STEP);
            if s.vx <= 0. {
                break;
            }
        }
        s.dist - d0
    }

    #[test]
    fn drum_brakes_fade_after_repeated_stops_ceramics_do_not() {
        let db = db();
        for (id, fades) in [("oldtimer_kaefer", true), ("turbo_s", false)] {
            let v = db.get(id).unwrap();
            let mut s = State::default();
            let first = brake_stop(v, &mut s, 100.);
            let mut last = first;
            for _ in 0..3 {
                // kurze Pause zwischen den Bremsungen: die Bremse kühlt kaum
                run(v, &mut s, Input::default(), 2.);
                last = brake_stop(v, &mut s, 100.);
            }
            assert!(s.brake_temp > FADE_FROM, "{id}: {}", s.brake_temp);
            if fades {
                assert!(last > first * 1.15, "{id}: {first} → {last}");
            } else {
                assert!(last < first * 1.02, "{id}: {first} → {last}");
            }
            // Fahrtwind kühlt
            let hot = s.brake_temp;
            s.vx = 25.;
            run(
                v,
                &mut s,
                Input {
                    throttle: 0.3,
                    ..Default::default()
                },
                20.,
            );
            assert!(s.brake_temp < hot * 0.5, "{id}: {hot} → {}", s.brake_temp);
        }
    }

    #[test]
    fn esp_modes_allow_more_slip_in_sport_and_none_when_off() {
        // Heckantrieb, nasse Straße, Vollgas aus der Kurve: der größte Schwimmwinkel wächst von voll über Sport
        // nach aus
        let db = db();
        let v = db.get("sportwagen_s").unwrap();
        let wet = Env {
            axle: [Ground {
                mu_rel: 0.7,
                ..Ground::DRY
            }; 2],
        };
        let peak = |esp: Esp| {
            let feel = Feel::simulation();
            let mut s = State {
                vx: 15.,
                gear: 1,
                ..Default::default()
            };
            let mut beta: f64 = 0.;
            for k in 0..(4. * HZ) as usize {
                let inp = Input {
                    throttle: 1.,
                    steer: if k < 60 { 0.8 } else { 0.4 },
                    esp: Some(esp),
                    ..Default::default()
                };
                step(v, &feel, &mut s, &inp, &wet, STEP);
                beta = beta.max(s.beta().abs());
            }
            beta
        };
        let (full, sport, off) = (peak(Esp::Full), peak(Esp::Sport), peak(Esp::Off));
        assert!(
            full < sport && sport < off,
            "voll {full} sport {sport} aus {off}"
        );
        assert!(full < 0.2, "voll {full}");
    }

    #[test]
    fn rear_axle_steering_turns_tighter_slowly_and_calmer_fast() {
        let db = db();
        let base = db.get("turbo_s").unwrap().clone();
        assert!(base.steering.rear);
        let mut plain = base.clone();
        plain.steering.rear = false;
        // Wenden bei Schritttempo: kleinerer Kreis (größere Gierrate bei gleicher Lenkung)
        let yaw = |v: &Vehicle, speed: f64, steer: f64| {
            let mut s = State {
                vx: speed,
                ..Default::default()
            };
            run(
                v,
                &mut s,
                Input {
                    throttle: 0.15,
                    steer,
                    esp: Some(Esp::Off),
                    ..Default::default()
                },
                1.5,
            );
            s.r.abs()
        };
        assert!(yaw(&base, 3., 1.) > yaw(&plain, 3., 1.) * 1.1);
        // schnell: gleichläufig, die Gierantwort ist gedämpfter
        assert!(yaw(&base, 35., 0.3) < yaw(&plain, 35., 0.3));
    }

    #[test]
    fn cobbles_shake_the_suspension_and_cost_grip() {
        let db = db();
        let v = db.get("kompakt_benzin").unwrap();
        let cob = Env {
            axle: [Ground {
                rough: 0.6,
                ..Ground::DRY
            }; 2],
        };
        let feel = Feel::simulation();
        let mut s = State {
            vx: 14.,
            gear: 2,
            ..Default::default()
        };
        let (mut lo, mut hi) = (f64::MAX, 0f64);
        for _ in 0..(3. * HZ) as usize {
            step(
                v,
                &feel,
                &mut s,
                &Input {
                    throttle: 0.3,
                    ..Default::default()
                },
                &cob,
                STEP,
            );
            let f = s.fz[0] + s.fz[1];
            (lo, hi) = (lo.min(f), hi.max(f));
        }
        let m = v.mass_empty;
        // Radlast vorn schwankt spürbar (glatt: gar nicht)
        assert!(hi - lo > m * G * v.front * 0.1, "{lo} … {hi}");
        assert!(s.susp[0] != 0.);
        // glatte Straße: Federung ruht
        let mut s2 = State {
            vx: 14.,
            gear: 2,
            ..Default::default()
        };
        run(v, &mut s2, Input::default(), 1.);
        assert_eq!(s2.susp, [0.; 2]);
    }

    #[test]
    fn full_stop_in_a_bus_throws_standing_passengers_once() {
        let db = db();
        let v = db.get("stadtbus").unwrap();
        let mut s = State {
            load: 1.,
            ..Default::default()
        };
        // sanftes Bremsen: niemand stürzt
        s.vx = 50. / 3.6;
        run(
            v,
            &mut s,
            Input {
                brake: 0.3,
                ..Default::default()
            },
            1.,
        );
        assert_eq!(s.passenger_falls, 0);
        // Vollbremsung: genau ein Ereignis
        brake_stop(v, &mut s, 50.);
        assert_eq!(s.passenger_falls, 1);
        // ein Pkw ohne Fahrgäste nie
        let car = db.get("kompakt_benzin").unwrap();
        let mut c = State::default();
        brake_stop(car, &mut c, 100.);
        assert_eq!(c.passenger_falls, 0);
    }
}
