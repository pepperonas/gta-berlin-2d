// Mission „Kisten für den Kiez“ als reine Zustandsmaschine (ohne DOM, vollständig testbar).
//   available → briefing → toPickup → toDropoff → success
//                                 ↘ failed (Zeit abgelaufen / Ware zerstört)
import { MISSION } from './config.js';
import { speedOf } from './car.js';

export const BRIEFING = [
  'Kalle vom Späti „Zum Kiez“:',
  '„Mein Lieferant hat mich hängen lassen. In der Lagerhalle in Neukölln',
  'stehen meine Kisten. Hol sie ab und bring sie her – aber zackig,',
  'um acht kommen die Stammgäste. Und fahr mir die Ware nicht zu Schrott!“',
];

export function createMission() {
  return { state: 'available', timer: 0, elapsed: 0, load: 0, cargoCarId: null, result: null, prompt: null, briefingPage: 0 };
}

export function resetMission(m) {
  Object.assign(m, createMission());
}

const inZone = (x, y, p, r = MISSION.zoneRadius) => Math.hypot(x - p.x, y - p.y) <= r;

// ctx: { places, player, cars, timeLimit, input: { action, actionHeld } } – liefert Ereignisse für Audio/HUD.
export function updateMission(m, ctx, dt) {
  const events = [];
  const { places, player, input } = ctx;
  const car = player.inCar ? ctx.cars.find((c) => c.id === player.inCar) : null;
  m.prompt = null;

  switch (m.state) {
    case 'available':
      if (!car && !player.ride && inZone(player.x, player.y, places.giver, MISSION.giverRadius)) {
        m.prompt = 'A: Auftrag annehmen';
        if (input.action) { m.state = 'briefing'; m.briefingPage = 0; events.push({ type: 'ui' }); }
      }
      break;
    case 'briefing':
      if (input.action) {
        m.state = 'toPickup';
        m.timer = ctx.timeLimit ?? MISSION.timeLimit;
        m.timeLimit = m.timer;
        m.elapsed = 0;
        events.push({ type: 'mission-start' });
      }
      break;
    case 'toPickup': {
      tick(m, dt, events);
      if (m.state !== 'toPickup') break;
      if (inZone(player.x, player.y, places.pickup)) {
        if (!car) m.prompt = 'Du brauchst ein Auto für die Kisten';
        else if (car.wrecked) m.prompt = 'Dieses Auto ist Schrott';
        else if (speedOf(car) > MISSION.stopSpeed) m.prompt = 'Anhalten zum Einladen';
        else {
          m.prompt = 'A gedrückt halten: Kisten einladen';
          if (input.actionHeld) {
            m.load += dt;
            if (m.load >= MISSION.loadTime) {
              car.cargo = true;
              m.cargoCarId = car.id;
              m.state = 'toDropoff';
              m.load = 0;
              events.push({ type: 'pickup' });
            }
          } else m.load = Math.max(0, m.load - dt * 2);
        }
      } else m.load = 0;
      break;
    }
    case 'toDropoff': {
      const cargoCar = ctx.cars.find((c) => c.id === m.cargoCarId);
      if (!cargoCar || cargoCar.wrecked) { fail(m, 'Die Ware ist hinüber – das Auto ist Schrott.', events); break; }
      tick(m, dt, events);
      if (m.state !== 'toDropoff') break;
      if (car === cargoCar && inZone(car.x, car.y, places.dropoff)) {
        if (speedOf(car) > MISSION.stopSpeed) m.prompt = 'Anhalten zum Abliefern';
        else {
          m.prompt = 'A: Kisten abliefern';
          if (input.action) succeed(m, cargoCar, events);
        }
      }
      break;
    }
  }
  return events;
}

function tick(m, dt, events) {
  m.elapsed += dt;
  const before = Math.ceil(m.timer);
  m.timer = Math.max(0, m.timer - dt);
  if (m.timer <= 10 && Math.ceil(m.timer) !== before && m.timer > 0) events.push({ type: 'tick' });
  if (m.timer <= 0) fail(m, 'Zeit abgelaufen – die Stammgäste sitzen auf dem Trockenen.', events);
}

export function failMission(m, reason, events) { fail(m, reason, events); }

function fail(m, reason, events) {
  m.state = 'failed';
  m.result = { success: false, reason };
  events.push({ type: 'mission-fail' });
}

function succeed(m, car, events) {
  // Zeitbonus auf das 120-s-Referenzlimit normiert, damit lange Routen nicht mehr Bonus bringen.
  const bonus = Math.round(m.timer / (m.timeLimit || MISSION.timeLimit) * 120 * MISSION.timeBonus);
  const damagePenalty = Math.round((100 - car.health) * 2);
  const reward = Math.max(100, MISSION.reward + bonus - damagePenalty);
  car.cargo = false;
  m.state = 'success';
  m.result = { success: true, time: m.elapsed, reward, bonus, damagePenalty, health: car.health };
  events.push({ type: 'mission-success' });
}

// Ziel für Pfeil/Minikarte und Text fürs HUD.
export function missionObjective(m, ctx) {
  const { places, player } = ctx;
  switch (m.state) {
    case 'available': return { text: 'Kalle am Späti wartet auf dich', target: places.giver };
    case 'toPickup':
      return { text: player.inCar ? 'Fahr zur Lagerhalle in Neukölln' : 'Besorg dir ein Auto und fahr zur Lagerhalle', target: places.pickup };
    case 'toDropoff': {
      const c = ctx.cars.find((x) => x.id === m.cargoCarId);
      if (c && player.inCar !== c.id) return { text: 'Zurück zum Wagen mit den Kisten', target: { x: c.x, y: c.y } };
      return { text: 'Bring die Kisten zum Parkplatz am Späti', target: places.dropoff };
    }
    default: return { text: '', target: null };
  }
}
