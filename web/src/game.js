// Bildschirm-Zustandsmaschine: Titel → Spiel ⇄ Pause, Steuerungshilfe, Missions-Ergebnis.
// Ohne DOM: bekommt abstrakte Eingaben und einen storage, liefert Ereignisse für Audio.
import { createWorld, updateWorld, restartMission, findTeleportSpot, teleportTo } from './world.js';
import { readSave, writeSave, applySave } from './save.js';
import { resetMission } from './mission.js';
import { createConsole } from './console.js';
import { createStats, createTracker, trackStep, mergeStats } from './stats.js';

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
  // Maus: Zeigen wählt aus, Klicken bestätigt (deaktivierte Einträge reagieren nicht)
  const usable = (i) => Number.isInteger(i) && menu.items[i] && menu.items[i].enabled !== false;
  if (usable(input.menuPick)) { menu.index = input.menuPick; return menu.items[menu.index].id; }
  if (usable(input.menuHover) && input.menuHover !== menu.index) { menu.index = input.menuHover; return 'move'; }
  if (input.menuUp && move(-1)) return 'move';
  if (input.menuDown && move(1)) return 'move';
  if (input.confirm) return menu.items[menu.index]?.id ?? null;
  if (input.back) return 'back';
  return null;
}

// city: dekodierte Karte; im Browser kommt sie asynchron nach (setCity), bis dahin zeigt der Titel „Lade Stadt …“.
export function createGame({ storage, canQuit = false, seed = 1989, city = null } = {}) {
  const g = { screen: 'title', world: null, storage, canQuit, seed, city, toast: null, returnTo: 'title', events: [], quitRequested: false, showBigMap: false, teleport: null,
    console: createConsole(), debug: { fps: false, levels: false, silhouettes: true, quality: null },
    // Statistik: dieses Spiel und über alle Spiele (main.js lädt/speichert sie in IndexedDB); savedGame = Stand des
    // gespeicherten Spiels (für „Fortsetzen“), statQueue = Ereignisse außerhalb der Simulation (Teleport, Cheats)
    stats: { game: createStats(), total: createStats(), savedGame: null }, tracker: createTracker(), statQueue: [], statsDirty: false };
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
  if (!spot) { g.toast = { text: 'Dort kann man nicht hin (außerhalb von Berlin)', t: 2 }; return false; }
  g.teleport = spot; // { pending } solange der Stadtteil dort noch lädt
  return true;
}

// Wartet ein Teleport-Ziel auf seine Kacheln, erneut nachsehen.
function resolveTeleport(g) {
  const t = g.teleport;
  if (!t?.pending) return;
  const spot = findTeleportSpot(g.world, t.x, t.y);
  if (spot?.pending) return;
  if (!spot) { g.toast = { text: 'Dort kann man nicht hin', t: 2 }; g.teleport = null; g.world.city.release('teleport'); return; }
  g.teleport = t.auto ? { ...spot, auto: true, name: t.name ?? spot.name } : spot; // Konsole: bleibt ohne Rückfrage
}

export function confirmTeleport(g, yes) {
  if (!g.teleport) return;
  if (yes && g.teleport.pending) return; // Ziel lädt noch
  if (yes) { teleportTo(g.world, g.teleport); g.showBigMap = false; g.toast = { text: `Teleportiert: ${g.teleport.name}`, t: 2.5 }; g.statQueue.push({ type: 'teleport' }); }
  else g.world.city.release('teleport');
  g.teleport = null;
}

function buildTitleMenu(g) {
  const hasSave = !!readSave(g.storage);
  const items = [
    { id: 'continue', label: 'Fortsetzen', enabled: hasSave },
    { id: 'new', label: 'Neues Spiel' },
    { id: 'controls', label: 'Steuerung' },
    { id: 'stats', label: 'Statistik' },
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
    { id: 'stats', label: 'Statistik' },
    { id: 'title', label: 'Zum Hauptmenü' },
  ]);
}

function resultMenu(success) {
  return createMenu(success
    ? [{ id: 'continue', label: 'Weiter' }]
    : [{ id: 'retry', label: 'Erneut versuchen' }, { id: 'continue', label: 'Frei weiterspielen' }]);
}

// Antwort der Statistik-Datenbank übernehmen (kommt asynchron, evtl. erst nach dem Spielstart): Gesamtstand und schon
// Gezähltes zusammenzählen; der Stand des gespeicherten Spiels gehört nur zu „Fortsetzen“ – in ein neues Spiel darf er
// nicht hineinlaufen.
export function applyStoredStats(g, total, saved) {
  g.stats.total = mergeStats(total, g.stats.total);
  if (g.world && g.stats.continued) g.stats.game = mergeStats(saved, g.stats.game);
  else if (!g.world) g.stats.savedGame = saved;
}

export function startNewGame(g) {
  g.world = createWorld({ city: g.city, seed: g.seed });
  g.screen = 'playing';
  g.stats.game = createStats(); g.stats.savedGame = null; g.stats.continued = false; g.tracker = createTracker(); g.statsDirty = true; // neue Zählung
}

export function continueGame(g) {
  const s = readSave(g.storage);
  if (!s) return false;
  g.world = createWorld({ city: g.city, seed: g.seed });
  applySave(g.world, s);
  g.screen = 'playing';
  g.stats.game = g.stats.savedGame ?? createStats(); g.stats.continued = true; g.tracker = createTracker(); // Statistik des gespeicherten Spiels
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
      else if (r === 'stats') { g.returnTo = 'title'; g.screen = 'stats'; }
      else if (r === 'quit') g.quitRequested = true;
      break;
    }
    case 'controls': case 'stats':
      if (input.back || input.confirm || (g.screen === 'stats' && input.pause)) { g.screen = g.returnTo; ev.push({ type: 'ui-back' }); }
      break;
    case 'paused': {
      const r = pick(g.pauseMenu);
      if (r === 'resume' || r === 'back') g.screen = 'playing';
      else if (r === 'save') saveGame(g);
      else if (r === 'restart') { restartMission(g.world); g.screen = 'playing'; g.toast = { text: 'Mission neu gestartet', t: 2 }; }
      else if (r === 'controls') { g.returnTo = 'paused'; g.screen = 'controls'; }
      else if (r === 'stats') { g.returnTo = 'paused'; g.screen = 'stats'; }
      else if (r === 'title') { g.screen = 'title'; g.titleMenu = buildTitleMenu(g); g.world = null; }
      break;
    }
    case 'playing': {
      const w = g.world, m = w.mission;
      if (g.console.open) break; // Befehlszeile offen: die Welt steht still (Tasten gehen nur an die Konsole)
      // Bestätigungsdialog für den Teleport: Welt steht still, A/Enter = ja, B/Esc = nein. Aus der Konsole (auto)
      // ohne Rückfrage, sobald die Kacheln am Ziel da sind.
      if (g.teleport) {
        resolveTeleport(g);
        if (!g.teleport) break;
        if (g.teleport.auto && !g.teleport.pending) { confirmTeleport(g, true); break; }
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
      // Statistik: Ereignisse dieses Schritts plus die der Konsole/des Stadtplans (Teleport, Cheats)
      trackStep([g.stats.game, g.stats.total], g.tracker, w, g.statQueue.length ? [...w.events, ...g.statQueue.splice(0)] : w.events, dt);
      g.statsDirty = true;
      break;
    }
  }
  return ev;
}
