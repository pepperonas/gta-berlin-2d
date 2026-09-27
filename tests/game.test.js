import test from 'node:test';
import assert from 'node:assert/strict';
import { createGame, updateGame, createMenu, menuInput } from '../web/src/game.js';
import { memoryStorage } from '../web/src/save.js';
import { idle } from './helpers/bot.js';

const press = (g, patch) => updateGame(g, { ...idle(), ...patch }, 1 / 60);

test('Menü überspringt deaktivierte Einträge und läuft rund', () => {
  const m = createMenu([{ id: 'a', enabled: false }, { id: 'b' }, { id: 'c' }]);
  assert.equal(m.index, 1);
  menuInput(m, { ...idle(), menuDown: true }); assert.equal(m.index, 2);
  menuInput(m, { ...idle(), menuDown: true }); assert.equal(m.index, 1);
  menuInput(m, { ...idle(), menuUp: true }); assert.equal(m.index, 2);
  assert.equal(menuInput(m, { ...idle(), confirm: true }), 'c');
});

test('Titel → Neues Spiel → Pause → Speichern → Hauptmenü → Fortsetzen (nur Controller-Aktionen)', () => {
  const st = memoryStorage();
  const g = createGame({ storage: st });
  assert.equal(g.titleMenu.items[0].enabled, false, 'Fortsetzen ohne Spielstand deaktiviert');
  assert.equal(g.titleMenu.items[g.titleMenu.index].id, 'new');
  press(g, { confirm: true });
  assert.equal(g.screen, 'playing');
  g.world.money = 777;
  press(g, { pause: true });
  assert.equal(g.screen, 'paused');
  press(g, { menuDown: true }); press(g, { confirm: true });
  assert.equal(g.toast.text, 'Spiel gespeichert');
  for (let i = 0; i < 3; i++) press(g, { menuDown: true });
  press(g, { confirm: true });
  assert.equal(g.screen, 'title');
  assert.equal(g.titleMenu.items[0].enabled, true);
  assert.equal(g.titleMenu.index, 0);
  press(g, { confirm: true });
  assert.equal(g.screen, 'playing');
  assert.equal(g.world.money, 777);
});

test('B im Pausemenü setzt fort; Steuerungsseite kehrt zum Aufrufer zurück', () => {
  const g = createGame({ storage: memoryStorage() });
  press(g, { confirm: true });
  press(g, { pause: true });
  press(g, { back: true });
  assert.equal(g.screen, 'playing');
  press(g, { pause: true });
  for (let i = 0; i < 3; i++) press(g, { menuDown: true });
  press(g, { confirm: true });
  assert.equal(g.screen, 'controls');
  press(g, { back: true });
  assert.equal(g.screen, 'paused');
});

test('Beenden nur, wenn die Hülle es anbietet', () => {
  assert.ok(!createGame({ storage: memoryStorage() }).titleMenu.items.some((i) => i.id === 'quit'));
  const g = createGame({ storage: memoryStorage(), canQuit: true });
  for (let i = 0; i < 2; i++) press(g, { menuDown: true });
  press(g, { confirm: true });
  assert.ok(g.quitRequested);
});

test('Gescheiterte Mission: „Erneut versuchen“ setzt zurück zum Späti', () => {
  const g = createGame({ storage: memoryStorage() });
  press(g, { confirm: true });
  const m = g.world.mission;
  m.state = 'failed'; m.result = { success: false, reason: 'x' };
  g.world.player.x = 100; g.world.player.y = 100;
  press(g, {});
  press(g, { confirm: true });
  assert.equal(m.state, 'available');
  assert.ok(Math.hypot(g.world.player.x - g.world.city.places.playerSpawn.x, g.world.player.y - g.world.city.places.playerSpawn.y) < 1);
});
