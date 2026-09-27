// Bildschirm-Zustandsmaschine: Titel → Spiel ⇄ Pause, Steuerungshilfe, Missions-Ergebnis.
// Ohne DOM: bekommt abstrakte Eingaben und einen storage, liefert Ereignisse für Audio.
import { createWorld, updateWorld, restartMission, findTeleportSpot, teleportTo } from './world.js';
import { readSave, writeSave, applySave } from './save.js';
import { resetMission } from './mission.js';

export function createMenu(items) { return { items, index: Math.max(0, items.findIndex((i) => i.enabled !== false)) }; }

// Bewegt die Auswahl (überspringt deaktivierte Einträge) und liefert die gewählte id, 'back' oder null.
export function menuInput(menu, input) {
  const n = menu.items.length;
  const move = (d) => {
    for (let k = 1; k <= n; k++) {
      const i = (menu.index + d * k + n * k) % n;
      if (menu.items[i].enabled !== false) { menu.index = i; return true; }
    }
    return false;
  };
  if (input.menuUp && move(-1)) return 'move';
  if (input.menuDown && move(1)) return 'move';
  if (input.confirm) return menu.items[menu.index]?.id ?? null;
  if (input.back) return 'back';
  return null;
}

// city: dekodierte Karte; im Browser kommt sie asynchron nach (setCity), bis dahin zeigt der Titel „Lade Stadt …“.
export function createGame({ storage, canQuit = false, seed = 1989, city = null } = {}) {
  const g = { screen: 'title', world: null, storage, canQuit, seed, city, toast: null, returnTo: 'title', events: [], quitRequested: false, showBigMap: false, teleport: null };
  g.titleMenu = buildTitleMenu(g);
  return g;
}

export function setCity(g, city) { g.city = city; }

// Teleport per Klick auf den Stadtplan: erst Ziel vormerken, dann bestätigen (Dialog).
export function requestTeleport(g, x, y) {
  if (g.screen !== 'playing' || !g.world || !g.showBigMap || g.teleport) return false;
  const st = g.world.mission.state;
  if (st === 'toPickup' || st === 'toDropoff' || st === 'briefing') { g.toast = { text: 'Während eines Auftrags nicht möglich', t: 2 }; return false; }
  const spot = findTeleportSpot(g.world, x, y);
  if (!spot) { g.toast = { text: 'Dort kann man nicht hin (außerhalb des Gebiets)', t: 2 }; return false; }
  g.teleport = spot;
  return true;
}

export function confirmTeleport(g, yes) {
  if (!g.teleport) return;
  if (yes) { teleportTo(g.world, g.teleport); g.showBigMap = false; g.toast = { text: `Teleportiert: ${g.teleport.name}`, t: 2.5 }; }
  g.teleport = null;
}

function buildTitleMenu(g) {
  const hasSave = !!readSave(g.storage);
  const items = [
    { id: 'continue', label: 'Fortsetzen', enabled: hasSave },
    { id: 'new', label: 'Neues Spiel' },
    { id: 'controls', label: 'Steuerung' },
  ];
  if (g.canQuit) items.push({ id: 'quit', label: 'Beenden' });
  const m = createMenu(items);
  m.index = hasSave ? 0 : 1;
  return m;
}

function pauseMenu() {
  return createMenu([
    { id: 'resume', label: 'Weiterspielen' },
    { id: 'save', label: 'Spiel speichern' },
    { id: 'restart', label: 'Mission neu starten' },
    { id: 'controls', label: 'Steuerung' },
    { id: 'title', label: 'Zum Hauptmenü' },
  ]);
}

function resultMenu(success) {
  return createMenu(success
    ? [{ id: 'continue', label: 'Weiter' }]
    : [{ id: 'retry', label: 'Erneut versuchen' }, { id: 'continue', label: 'Frei weiterspielen' }]);
}

export function startNewGame(g) {
  g.world = createWorld({ city: g.city, seed: g.seed });
  g.screen = 'playing';
}

export function continueGame(g) {
  const s = readSave(g.storage);
  if (!s) return false;
  g.world = createWorld({ city: g.city, seed: g.seed });
  applySave(g.world, s);
  g.screen = 'playing';
  g.toast = { text: 'Spielstand geladen', t: 2 };
  return true;
}

export function saveGame(g) {
  const ok = writeSave(g.storage, g.world);
  g.toast = { text: ok ? 'Spiel gespeichert' : 'Speichern fehlgeschlagen', t: 2 };
  return ok;
}

export function updateGame(g, input, dt) {
  const ev = (g.events = []);
  if (g.toast && (g.toast.t -= dt) <= 0) g.toast = null;
  const pick = (menu) => {
    const r = menuInput(menu, input);
    if (r === 'move') ev.push({ type: 'ui-move' });
    else if (r) ev.push({ type: r === 'back' ? 'ui-back' : 'ui' });
    return r === 'move' ? null : r;
  };

  switch (g.screen) {
    case 'title': {
      if (!g.city) break; // Karte lädt noch
      const r = pick(g.titleMenu);
      if (r === 'continue') continueGame(g);
      else if (r === 'new') startNewGame(g);
      else if (r === 'controls') { g.returnTo = 'title'; g.screen = 'controls'; }
      else if (r === 'quit') g.quitRequested = true;
      break;
    }
    case 'controls':
      if (input.back || input.confirm) { g.screen = g.returnTo; ev.push({ type: 'ui-back' }); }
      break;
    case 'paused': {
      const r = pick(g.pauseMenu);
      if (r === 'resume' || r === 'back') g.screen = 'playing';
      else if (r === 'save') saveGame(g);
      else if (r === 'restart') { restartMission(g.world); g.screen = 'playing'; g.toast = { text: 'Mission neu gestartet', t: 2 }; }
      else if (r === 'controls') { g.returnTo = 'paused'; g.screen = 'controls'; }
      else if (r === 'title') { g.screen = 'title'; g.titleMenu = buildTitleMenu(g); g.world = null; }
      break;
    }
    case 'playing': {
      const w = g.world, m = w.mission;
      // Bestätigungsdialog für den Teleport: Welt steht still, A/Enter = ja, B/Esc = nein.
      if (g.teleport) {
        if (input.confirm) { confirmTeleport(g, true); ev.push({ type: 'ui' }); }
        else if (input.back || input.pause) { confirmTeleport(g, false); ev.push({ type: 'ui-back' }); }
        break;
      }
      if (input.pause) { g.screen = 'paused'; g.pauseMenu = pauseMenu(); ev.push({ type: 'ui' }); break; }
      if (input.mapToggle) g.showBigMap = !g.showBigMap;
      if (m.state === 'success' || m.state === 'failed') {
        if (!g.resultMenu) g.resultMenu = resultMenu(m.state === 'success');
        const r = pick(g.resultMenu);
        if (r === 'continue' || (r === 'back' && m.state === 'success')) {
          const wasSuccess = m.state === 'success';
          resetMission(m); for (const c of w.cars) c.cargo = false; g.resultMenu = null;
          if (wasSuccess) saveGame(g);
        } else if (r === 'retry') { restartMission(w); g.resultMenu = null; }
        break;
      }
      updateWorld(w, input, dt);
      ev.push(...w.events);
      break;
    }
  }
  return ev;
}
