// Fahrdynamik des gefahrenen Autos (rein rechnerisch, deterministisch): Einspurmodell mit Reifenkräften an Vorder- und
// Hinterachse. Ziel: zugängliches, arcadiges Fahrgefühl. Der Charakter jedes Autos folgt aus der Physik (unten); eine
// Spielspaß-Schicht (DYN.fun) macht es großzügiger als die Wirklichkeit – mehr Grip, viel kräftigere Bremsen, etwas
// mehr Leistung bei echtem Höchsttempo, engerer Wendekreis, flinke Lenkung, ESP fängt ein ausbrechendes Heck ab
// (statt es nur abzuwürgen), Handbremse zum Driften (Heck verliert Seitenhalt, der Schwung bleibt). „esp aus“ = roh.
// - Schräglaufwinkel je Achse → Seitenkraft über eine Reifenkennlinie (steigt, erreicht bei ≈ 8° ihr Maximum, fällt im
//   Rutschen auf ≈ 80 %): über dem Grenzbereich schiebt das Auto oder bricht aus.
// - Kammscher Kreis je Achse: Antriebs- oder Bremskraft verbraucht Haftung, die dann seitlich fehlt. Frontantrieb schiebt
//   unter Last über die Vorderräder (Untersteuern), Heckantrieb drückt das Heck herum (Übersteuern), Allrad verteilt.
// - Dynamische Achslast: Beschleunigen lädt die Hinterachse (Heckantrieb zieht besser an, Front dreht durch), Bremsen
//   die Vorderachse (Heck wird leicht, Lastwechsel). Je höher der Schwerpunkt, desto stärker; dazu verliert ein hoher
//   Schwerpunkt in schnellen Kurven Haftung (Querlastverschiebung).
// - Motorlage: Gewichtsverteilung und Gierträgheit – Mittelmotor dreht spontan ein, Heckmotor pendelt (schweres Heck).
// - Handbremse blockiert die Hinterräder: Gleitreibung entgegen der Rutschrichtung, das Heck kommt.
// - ABS an der Fußbremse; Durchdrehen und Rutschen sind sichtbar (car.spin, car.skid) und hörbar.
// Einheiten innen SI (m, s, N, kg), außen px (10 px = 1 m). car.dyn hält den Zustand für Anzeige und Tests.
import { specOf } from './carmodels.js';
import { AQUA } from './traction.js';

export const DYN = {
  substeps: 4,
  g: 9.81,
  peak: 1.4,          // Formfaktor der Reifenkennlinie (sin(C·atan(Bα)); Gleiten ≈ sin(C·π/2) ≈ 0,8 des Maximums)
  stiff: [17, 19],    // Schräglaufsteifigkeit je Radlast (1/rad) vorn/hinten
  steerRate: 4,       // rad/s Lenkeinschlag (Lenkrad drehen), zurück zur Mitte schneller
  centerRate: 6,
  steerAssist: 1.3,   // Einschlag höchstens so weit, dass das Auto knapp (×1,3) über die Haftgrenze kann
  locked: 0.78,       // Gleitreibung durchdrehender Räder relativ zur Haftung
  abs: 0.95,          // ABS nutzt 95 % der Haftung zum Bremsen, der Rest bleibt zum Lenken
  esp: [0.9, 0.07, 0.14, 0.85], // ASR/ESP: Antrieb je Achse ≤ 90 % der Haftung, ab 4° Schräglauf zurückgenommen (bis −85 % bei 12°)
  espYaw: 5,
  lift: 0.97,         // Zweirad: Anzug/Bremse höchstens 97 % der Kraft, bei der ein Rad abhebt          // 1/s: ESP bremst die Gierrate zurück, sobald sie über der gelenkten liegt (Heck kommt → abgefangen)
  // Spielspaß: grip (Reifen), brake (Bremsen zusätzlich zum Grip), power (Anzug; Höchsttempo bleibt echt),
  // steer (Wendekreis), handbrake: [Seitenhalt hinten, Bremskraft hinten] relativ zur Haftung
  lowLock: [0.95, 3, 9], // Rangieren: bis 3 m/s darf die Lenkung bis 0,95 rad einschlagen, bis 9 m/s auf den Normalwert
  fun: { grip: 1.4, brake: 1.25, power: 1.2, steer: 1.2, handbrake: [0.4, 0.3] },
  // Zugkraftverlauf: kräftiger Stadt-Antritt, auslaufend bis 90 km/h. Allrad hat bereits
  // sehr viel Starttraktion; Zweiräder bleiben durch Wheelie/Stoppie begrenzt.
  launch: { gain: 0.28, awd: 0.35, maxAccel: 10, from: 20 / 3.6, to: 90 / 3.6 },
  // Oberhalb des Stadtbereichs zählt die mittlere Radleistung statt ständig Spitzenleistung.
  // Die 14 % sind eine Spielabstimmung für Antriebsverluste/Leistungsband, kein Messwert.
  wheelPower: { high: 0.86, from: 50 / 3.6, to: 120 / 3.6 },
  roll: 0.015,        // Rollwiderstand
  engineBrake: 0.9,   // m/s² Schleppmoment am Antrieb ohne Gas
  reverse: 8.3,       // m/s (30 km/h)
  suspension: 0.12,   // s Zeitkonstante für Nicken/Wanken
  kinematic: [1, 4],   // m/s: darunter rollt das Auto rein geometrisch, darüber mit Reifenkräften (weich überblendet)
  espFrom: 4,          // m/s: ESP regelt erst oberhalb (Rangieren, Anfahren mit Einschlag)
  rwdDriftGrip: 0.72,  // zusätzliche Driftfreude bei ausgeschaltetem ESP
  drift: { duration: 1, minSpeed: 8, kick: 0.2, rearGrip: 0.65, yaw: 0.15, follow: 3.8 },
};

const clamp = (x, a, b) => Math.max(a, Math.min(b, x));
const sgn = (x) => (x > 0 ? 1 : x < 0 ? -1 : 0);
const smooth = (x) => { const t = clamp(x, 0, 1); return t * t * (3 - 2 * t); };

// Hüllkurve der automatischen Übersetzung: unten drehmomentbegrenzt, oben P/v.
// Sanfte Übergänge vermeiden einen künstlichen Schub bei 50/70 km/h. Die
// Anfahrhilfe erhöht Zugkraft UND übertragbare Längskraft, sonst würde ASR sie abschneiden.
export function driveEnvelope(spec, speed, grip = 1, steer = 0) {
  const v = Math.max(0, speed), L = DYN.launch, W = DYN.wheelPower;
  const launch = 1 - smooth((v - L.from) / (L.to - L.from));
  const surface = smooth((grip - 0.45) / 0.55);
  const straight = 1 - smooth(Math.abs(steer) / 0.5);
  const gain = spec.twoWheel ? 0 : L.gain * (spec.drive === 'awd' ? L.awd : 1);
  // Bereits extrem schnelle Sport-/Allradautos erhalten keinen weiteren Gripbonus.
  // Achslast am Zielwert, inklusive Lasttransfer: verhindert eine Verstärkungsschleife
  // aus mehr Grip → mehr Beschleunigung → mehr Hinterachslast → noch mehr Grip.
  const transfer = L.maxAccel / DYN.g * spec.h / spec.wb;
  const share = spec.drive === 'awd' ? 1 : spec.drive === 'fwd' ? spec.front - transfer : 1 - spec.front + transfer;
  const atLimit = spec.mu * DYN.fun.grip * DYN.esp[0] * DYN.g * Math.max(0.1, share);
  const headroom = Math.max(0, L.maxAccel / atLimit - 1);
  const traction = 1 + Math.min(gain, headroom) * launch * surface * straight;
  const power = spec.kW * 1000 * DYN.fun.power * (1 - (1 - W.high) * smooth((v - W.from) / (W.to - W.from)));
  return { force: Math.min(power / spec.vLow, power / Math.max(0.5, v)) * traction, traction };
}

// Reifenkennlinie: Kraftanteil (−1…1) bei Schräglauf α für Steifigkeit B (je Radlast)
export const tireCurve = (alpha, stiff) => Math.sin(DYN.peak * Math.atan(stiff / DYN.peak * alpha));

// Achsgeometrie aus den Daten: a = Schwerpunkt → Vorderachse, b = → Hinterachse, Gierträgheit
export function geometry(spec) {
  const a = spec.wb * (1 - spec.front), b = spec.wb * spec.front;
  return { a, b, Iz: spec.mass * a * b * spec.yaw * spec.yaw };
}

// Ein Schritt dt. surf = { grip, drag } (Untergrund), tr = Wetterfaktoren (traction.js), ctl = Bedienung.
export function stepDynamics(car, dt, surf, tr, ctl) {
  const spec = specOf(car), geo = geometry(spec), m = spec.mass, g = DYN.g;
  const absOn = car.abs !== false, abs = absOn ? DYN.abs : 1;
  const d = car.dyn ??= { esp: 0, delta: 0, ax: 0, ay: 0, alphaF: 0, alphaR: 0, spinF: 0, spinR: 0, lockR: 0, understeer: 0 };
  const aq = (car.aqua ?? 0) > 0;
  const F = DYN.fun, muBase = spec.mu * (surf.grip ?? 1) * F.grip;
  const P = spec.kW * 1000 * F.power, vmax = spec.vmax / 3.6 * (surf.top ?? 1); // Wiese, Wasser, Pflaster: langsamer
  const roll = DYN.roll * m * g * (surf.drag ?? 1);
  // Widerstand und Radleistung müssen denselben Maßstab verwenden, sonst sinkt die Spitze.
  const topForce = driveEnvelope(spec, vmax).force;
  const cd = Math.max(0.05, (topForce * 0.92 - DYN.roll * m * g) / (vmax * vmax));
  const steerMax0 = Math.min(0.8, spec.steerMax * F.steer), [lockLow, v0, v1] = DYN.lowLock;
  // Körperfeste Geschwindigkeiten (m/s): u längs, v quer (+ = rechts), w Gierrate (+ = im Uhrzeigersinn)
  let c = Math.cos(car.angle), s = Math.sin(car.angle);
  let u = (car.vx * c + car.vy * s) / 10, v = (-car.vx * s + car.vy * c) / 10, w = car.angVel;
  const h = dt / DYN.substeps;
  const handbrakePressed = ctl.handbrake && !d.handbrakeWas;
  d.handbrakeWas = !!ctl.handbrake;
  if (handbrakePressed && !spec.twoWheel && Math.hypot(u, v) >= DYN.drift.minSpeed) {
    const dir = Math.sign(ctl.steer || d.delta || w);
    if (dir) {
      d.driftT = DYN.drift.duration;
      d.driftDir = dir;
      w += dir * DYN.drift.kick;
    }
  }
  let spinAny = 0, skid = 0;
  for (let k = 0; k < DYN.substeps; k++) {
    d.driftT = Math.max(0, (d.driftT ?? 0) - h);
    const driftBlend = clamp(d.driftT / DYN.drift.duration, 0, 1);
    const speed = Math.hypot(u, v), [k0, k1] = DYN.kinematic, qs = clamp((speed - k0) / (k1 - k0), 0, 1);
    // Lenkung: Einschlag folgt der Eingabe mit endlicher Geschwindigkeit; bei Tempo begrenzt auf knapp über die Haftgrenze
    const steerMax = steerMax0 + (lockLow - steerMax0) * (1 - clamp((Math.abs(u) - v0) / (v1 - v0), 0, 1)); // Spielspaß: enger Wendekreis
    const lim = Math.min(steerMax, Math.max(0.05, Math.atan(DYN.steerAssist * spec.wb * muBase * tr.lat * g / Math.max(1, u * u))));
    const target = clamp(ctl.steer, -1, 1) * lim * (tr.steer ?? 1);
    const rate = Math.abs(target) < Math.abs(d.delta) ? DYN.centerRate : DYN.steerRate;
    d.delta += clamp(target - d.delta, -rate * h, rate * h);
    const delta = d.delta;
    // Achslasten mit Längslastverschiebung (Beschleunigung d.ax aus dem letzten Teilschritt, gefedert)
    const dF = m * d.ax * spec.h / spec.wb;
    const Fzf = clamp(m * g * spec.front - dF, 0.1 * m * g, 0.9 * m * g), Fzr = m * g - Fzf;
    // Querlastverschiebung: hoher Schwerpunkt + schmale Spur → weniger Seitenhaftung in schnellen Kurven
    // (Zweiräder legen sich in die Kurve – dort gibt es keine Querlastverschiebung)
    const mu = muBase * (1 - (spec.twoWheel ? 0 : 0.22 * clamp(spec.h / spec.track * Math.abs(d.ay) / g, 0, 1)));
    const driveCurve = driveEnvelope(spec, Math.abs(u), aq ? 0 : (surf.grip ?? 1) * (tr.accel ?? 1), ctl.steer);
    const launchTraction = ctl.throttle > 0 && u >= -0.5 && !ctl.handbrake && driftBlend <= 0 ? driveCurve.traction : 1;
    const muX = mu * (tr.accel ?? 1) * launchTraction, muB = mu * F.brake * (tr.brake ?? 1) * (aq ? AQUA.brake : 1), muY = mu * (tr.lat ?? 1) * (aq ? AQUA.lat : 1);
    // --- Längskräfte je Achse ---
    let Fxf = 0, Fxr = 0, spinF = 0, spinR = 0, lockF = 0, lockR = 0, esp = 0, wheelie = 0, stoppie = 0;
    const fwd = u > -0.5;
    if (ctl.throttle > 0 && u < -0.5) { // Gas, während das Auto rückwärts rollt: erst abbremsen (wie die Fußbremse)
      const Fb = ctl.throttle * muB * m * g;
      Fxf += Math.min(Fb * spec.bias, muB * Fzf * abs); Fxr += Math.min(Fb * (1 - spec.bias), muB * Fzr * abs);
    }
    if (ctl.throttle > 0 && fwd) {
      const Fd = ctl.throttle * driveCurve.force / driveCurve.traction * launchTraction * (u < vmax ? 1 : 0);
      const share = spec.drive === 'fwd' ? 1 : spec.drive === 'rwd' ? 0 : spec.awdFront;
      let want = [Fd * share, Fd * (1 - share)];
      if (spec.drive === 'awd') { // Mittendifferenzial mit Sperre: was eine Achse nicht übertragen kann, bekommt die andere
        const cap = [muX * Fzf, muX * Fzr];
        for (const [i, j] of [[0, 1], [1, 0]]) if (want[i] > cap[i]) { want[j] += want[i] - cap[i]; want[i] = cap[i]; }
      }
      if (car.esp !== false && driftBlend <= 0) { // ASR im normalen Fahrbetrieb; den bewusst eingeleiteten Drift freigeben
        const [k, a0, span, cut] = DYN.esp, esc = speed > DYN.espFrom ? 1 : 0;
        const lim = (al, Fz) => muX * Fz * k * (1 - esc * cut * clamp((Math.abs(al) - a0) / span, 0, 1));
        const lf = lim(d.alphaF, Fzf), lr = lim(d.alphaR, Fzr);
        if (want[0] > lf || want[1] > lr) esp = 1;
        want = [Math.min(want[0], lf), Math.min(want[1], lr)];
      }
      const drive = (F, Fz) => (F > muX * Fz ? [muX * Fz * DYN.locked, 1] : [F, 0]); // mehr als die Haftung → Räder drehen durch
      [Fxf, spinF] = drive(want[0], Fzf); [Fxr, spinR] = drive(want[1], Fzr);
      if (spec.twoWheel) { // Wheelie: mehr Anzug als g·Anteil vorn·Radstand/Schwerpunkthöhe hebt das Vorderrad – die Elektronik
        const lift = m * g * spec.front * spec.wb / spec.h;   // (bzw. der Fahrer) hält es knapp darunter
        const F = Fxf + Fxr;
        wheelie = clamp((F / lift - 0.85) / 0.15, 0, 1);
        if (F > lift * DYN.lift) { const k = lift * DYN.lift / F; Fxf *= k; Fxr *= k; }
      }
    } else if (ctl.throttle <= 0 && Math.abs(u) > 0.3 && !ctl.brake) { // Schleppmoment am Antrieb
      const Fe = -sgn(u) * DYN.engineBrake * m * Math.min(1, Math.abs(u) / 4);
      if (spec.drive === 'fwd') Fxf += Fe; else if (spec.drive === 'rwd') Fxr += Fe; else { Fxf += Fe * spec.awdFront; Fxr += Fe * (1 - spec.awdFront); }
    }
    if (ctl.brake > 0) {
      if (u > 0.5) { // Fußbremse mit ABS: je Achse höchstens bis an die Haftgrenze
        const Fb = ctl.brake * muB * m * g * (spec.brakeK ?? 1); // alte Bremsanlagen (Trommeln) schaffen weniger
        let bf = Math.min(Fb * spec.bias, muB * Fzf * abs), br = Math.min(Fb * (1 - spec.bias), muB * Fzr * abs);
        if (!absOn) {
          if (bf > muY * Fzf) { lockF = 1; bf = muY * Fzf * DYN.locked; }
          if (br > muY * Fzr) { lockR = 1; br = muY * Fzr * DYN.locked; }
        }
        if (spec.twoWheel) { // Stoppie: zu hart gebremst hebt das Hinterrad – Bremskraft darunter halten
          const lift = m * g * (1 - spec.front) * spec.wb / spec.h;
          stoppie = clamp(((bf + br) / lift - 0.85) / 0.15, 0, 1);
          if (bf + br > lift * DYN.lift) { const k = lift * DYN.lift / (bf + br); bf *= k; br *= k; }
        }
        Fxf -= bf; Fxr -= br;
      } else if (u > -DYN.reverse) { // rückwärts
        const Fr = ctl.brake * Math.min(P / spec.vLow, m * 3.5);
        if (spec.drive === 'fwd') Fxf -= Math.min(Fr, muX * Fzf); else Fxr -= Math.min(Fr, muX * Fzr);
      }
    }
    // --- Querkräfte: Schräglaufwinkel an den Achsen (bei Schritttempo gegen Division durch 0 gesichert) ---
    // Schräglauf aus der Radgeschwindigkeit im eigenen Radsystem (quer zur Laufrichtung des gelenkten Rades): im Stand
    // null (kein Schräglauf, auch bei vollem Einschlag), rückwärts richtig herum
    const vfa = v + w * geo.a, cd0 = Math.cos(delta), sd0 = Math.sin(delta);
    const alphaF = Math.atan2(vfa * cd0 - u * sd0, Math.max(Math.abs(u * cd0 + vfa * sd0), 1.5));
    const alphaR = Math.atan2(v - w * geo.b, Math.max(Math.abs(u), 1.5));
    const lat = (alpha, Fz, Fx, stiffK, slipping) => {
      // Die Anfahrhilfe erweitert nur die Längsachse der Reibellipse. Ohne diese
      // Normierung würde zusätzliche Starttraktion den gesamten Seitenhalt auffressen.
      const longitudinal = Fx > 0 ? Fx / launchTraction : Fx;
      const cap = Math.sqrt(Math.max(0, (muY * Fz) ** 2 - longitudinal * longitudinal)) * (slipping ? 0.8 : 1);
      return -cap * tireCurve(alpha, stiffK);
    };
    // bei Schritttempo übernimmt die Geometrie (unten): Reifenquerkräfte dort ausgeblendet, sonst bremsen sie das Anfahren
    const poweredRwdDrift = car.esp === false && spec.drive === 'rwd' && ctl.throttle > 0.65 && Math.abs(ctl.steer) > 0.15 && u > 4 && !ctl.handbrake;
    let Fyf = lat(alphaF, Fzf, Fxf, DYN.stiff[0], spinF || lockF) * qs;
    const driftRearGrip = 1 - (1 - DYN.drift.rearGrip) * driftBlend;
    let Fyr = lat(alphaR, Fzr, Fxr, DYN.stiff[1], spinR || lockR) * (poweredRwdDrift ? DYN.rwdDriftGrip : driftRearGrip) * qs;
    if (ctl.handbrake && speed > 0.3) { // Handbremse: Heck verliert Seitenhalt (Gleiten), bremst nur mäßig – driften
      const [side, drag] = F.handbrake, vy = v - w * geo.b;
      Fyr = -muY * side * Fzr * clamp(vy / 1.5, -1, 1) * qs;
      Fxr = -sgn(u) * muB * drag * Fzr; lockR = 1;
    }
    // --- Bewegungsgleichungen (Körperfest) ---
    const cs = Math.cos(delta), sn = Math.sin(delta);
    const FfX = Fxf * cs - Fyf * sn, FfY = Fxf * sn + Fyf * cs;
    const drag = cd * u * Math.abs(u) + (Math.abs(u) > 0.05 ? roll * sgn(u) : 0);
    const ax = (FfX + Fxr - drag) / m, ay = (FfY + Fyr) / m;
    let dw = (geo.a * FfY - geo.b * Fyr) / geo.Iz;
    const du = ax + v * w, dv = ay - u * w;
    if (driftBlend > 0 && !spec.twoWheel) {
      const speedMix = clamp((Math.abs(u) - DYN.drift.minSpeed) / 14, 0, 1);
      const counter = clamp((d.driftDir ?? 0) * ctl.steer, 0, 1);
      const driftYaw = u * Math.tan(delta) / spec.wb + (d.driftDir ?? 0) * DYN.drift.yaw * speedMix * driftBlend * (0.25 + 0.75 * counter);
      dw += (driftYaw - w) * DYN.drift.follow;
    }
    // ESP-Gierregelung: dreht das Auto schneller, als die Lenkung verlangt (Übersteuern), bremst es die Drehung ab –
    // das Heck kommt, wird aber gefangen. Nicht bei Handbremse (gewollter Drift) und nicht ohne ESP.
    if (car.esp !== false && driftBlend <= 0 && !ctl.handbrake && speed > DYN.espFrom && u > 0) {
      const wRef = u * Math.tan(delta) / spec.wb, over = Math.abs(w) - Math.abs(wRef);
      if (over > 0.05 && Math.abs(d.alphaR) > 0.06) { dw -= sgn(w) * DYN.espYaw * over; esp = 1; }
    }
    u += du * h; v += dv * h; w += dw * h;
    if (aq) w += ((car.aquaYaw ?? 0) - w) * Math.min(1, 6 * h); // Aquaplaning: das schwimmende Auto giert (wie im Verkehr)
    // Schritttempo: rein geometrisch rollen (Reifenmodell ist dort singulär); dazwischen weich überblenden
    const sp = Math.hypot(u, v), q = clamp((sp - k0) / (k1 - k0), 0, 1);
    if (q < 1) {
      const wk = u * Math.tan(delta) / spec.wb; // Hinterachse rollt ohne Querrutschen: im Schwerpunkt v = ω·b
      w = q * w + (1 - q) * wk; v = q * v + (1 - q) * wk * geo.b;
      if (!(ctl.throttle > 0) && !(ctl.brake > 0) && sp < 0.25) { u = 0; v = 0; w = 0; } // steht
    }
    const tau = h / DYN.suspension;
    d.ax += (ax - d.ax) * Math.min(1, tau); d.ay += (ay - d.ay) * Math.min(1, tau);
    d.alphaF = alphaF * qs; d.alphaR = alphaR * qs; d.spinF = spinF; d.spinR = spinR; d.lockR = lockR; d.esp = esp;
    d.wheelie = wheelie; d.stoppie = stoppie; d.lean = spec.twoWheel ? Math.atan2(d.ay, g) : 0; // Schräglage (Darstellung)
    spinAny = Math.max(spinAny, spinF, spinR);
    if (sp > 3) skid = Math.max(skid, clamp((Math.max(Math.abs(alphaF), Math.abs(alphaR)) - 0.12) / 0.2, 0, 1), driftBlend * 0.75, Math.max(lockF, lockR) * 0.8, (spinF || spinR) ? 0.6 : 0);
    car.angle += w * h;
    c = Math.cos(car.angle); s = Math.sin(car.angle);
    car.x += (u * c - v * s) * 10 * h; car.y += (u * s + v * c) * 10 * h;
  }
  // Untersteuern (+) / Übersteuern (−): Schräglauf vorn minus hinten (Fahrtrichtung beachtet)
  d.understeer = (Math.abs(d.alphaF) - Math.abs(d.alphaR));
  car.vx = (u * c - v * s) * 10; car.vy = (u * s + v * c) * 10; car.angVel = w;
  car.spin = spinAny;
  car.skid = skid;
}
