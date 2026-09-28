// Befehlszeile (console.js): Zerlegen, Vorschläge, Tasten, Befehle – und im Spiel (game.js): Welt steht still,
// Teleport ohne Rückfrage, Statistik-Seite.
import test from 'node:test';
import assert from 'node:assert/strict';
import { tokenize, suggest, execute, consoleKey, createConsole, openConsole, rankMatches, clockArg, placeIndex, findCommand, COMMANDS } from '../web/src/console.js';
import { createGame, updateGame } from '../web/src/game.js';
import { memoryStorage } from '../web/src/save.js';
import { idle } from './helpers/bot.js';
import { realCity, realOverview } from './helpers/city.js';

const city = realCity();
city.overview = realOverview();
const press = (g, patch = {}) => updateGame(g, { ...idle(), ...patch }, 1 / 60);
function newGame() { const g = createGame({ storage: memoryStorage(), city }); press(g, { confirm: true }); assert.equal(g.screen, 'playing'); return g; }
const ctxOf = (g) => ({ game: g, world: g.world, city });
const type = (con, s, ctx) => { for (const ch of s) consoleKey(con, ch, ctx); };

test('Zerlegen: Anführungszeichen halten Leerzeichen zusammen; Uhrzeit großzügig', () => {
  assert.deepEqual(tokenize('tp "Kottbusser Tor" x').map((t) => t.t), ['tp', 'Kottbusser Tor', 'x']);
  assert.deepEqual(tokenize('  zeit   12  ').map((t) => [t.t, t.start]), [['zeit', 2], ['12', 9]]);
  assert.deepEqual(tokenize('tp "Kott').map((t) => t.t), ['tp', 'Kott'], 'offenes Anführungszeichen');
  assert.equal(clockArg('21:30'), 1290); assert.equal(clockArg('21.30'), 1290); assert.equal(clockArg('2130'), 1290);
  assert.equal(clockArg('9'), 540); assert.equal(clockArg('7 Uhr'), 420); assert.equal(clockArg('9h'), 540);
  for (const bad of ['24', '12:60', 'x', '', '123']) assert.equal(clockArg(bad), null, bad);
});

test('Treffer: Namensanfang vor Wortanfang vor irgendwo, Umlaute egal', () => {
  const items = ['Oranienplatz', 'Am Oranienburger Tor', 'Kottbusser Tor', 'Görlitzer Park'].map((name) => ({ name }));
  assert.deepEqual(rankMatches(items, 'or').map((i) => i.name), ['Oranienplatz', 'Am Oranienburger Tor', 'Kottbusser Tor', 'Görlitzer Park']);
  assert.deepEqual(rankMatches(items, 'gorl').map((i) => i.name), ['Görlitzer Park']);
  assert.deepEqual(rankMatches(items, '').map((i) => i.name), items.map((i) => i.name), 'ohne Eingabe: Reihenfolge der Liste');
});

test('Vorschläge: Befehle, Werte und Orte mit grauer Ergänzung', () => {
  const g = newGame(), ctx = ctxOf(g);
  const all = suggest('', ctx);
  assert.ok(all.items.length >= 8 && all.items.some((i) => i.label === 'wetter'), 'leere Zeile listet Befehle');
  const we = suggest('we', ctx);
  assert.equal(we.items[0].label, 'wetter'); assert.equal(we.ghost, 'tter');
  assert.ok(!we.items.some((i) => i.label === 'weather'), 'Alias nur, wenn nichts Eigenes passt');
  assert.equal(suggest('weath', ctx).items[0].label, 'weather');
  const wx = suggest('wetter sch', ctx);
  assert.deepEqual(wx.items.map((i) => i.label), ['schnee', 'schneesturm']);
  assert.equal(wx.ghost, 'nee');
  assert.equal(suggest('wetter ', ctx).items[0].label, 'auto', 'nach dem Leerzeichen: alle Werte');
  const tp = suggest('tp kottb', ctx);
  assert.equal(tp.items[0].label, 'Kottbusser Tor'); assert.equal(tp.items[0].hint, 'U-Bahnhof');
  assert.equal(tp.ghost, 'usser Tor');
  const tp2 = suggest('tp kottbusser d', ctx);
  assert.equal(tp2.items[0].label, 'Kottbusser Damm', 'Ortsname mit Leerzeichen als ein Argument');
  const kinds = new Set(placeIndex(city).map((p) => p.kind));
  for (const k of ['Bezirk', 'Ortsteil', 'U-Bahnhof', 'S-Bahnhof', 'Straße']) assert.ok(kinds.has(k), `Orte der Art ${k}`);
  assert.equal(placeIndex(city), placeIndex(city), 'einmal je Stadt gebaut');
  assert.deepEqual(suggest('blabla x', ctx).items, [], 'unbekannter Befehl: keine Werte');
});

test('Tasten: tippen, Tab ergänzt, Pfeile wählen, Enter führt aus und bleibt offen, Verlauf, Esc schließt', () => {
  const g = newGame(), ctx = ctxOf(g), con = createConsole();
  openConsole(con, ctx);
  assert.ok(con.open && con.sugg.items.length);
  type(con, 'wet', ctx);
  assert.equal(consoleKey(con, 'Tab', ctx), 'edit');
  assert.equal(con.text, 'wetter ', 'Befehl ergänzt, Leerzeichen für das Argument');
  type(con, 'sch', ctx);
  assert.equal(consoleKey(con, 'ArrowDown', ctx), 'nav'); assert.equal(con.sel, 0);
  consoleKey(con, 'ArrowDown', ctx); assert.equal(con.sel, 1);
  assert.equal(consoleKey(con, 'Enter', ctx, 5), 'run');
  assert.equal(g.world.forceWeather, 'heavysnow', 'gewählter Vorschlag ausgeführt');
  assert.ok(con.open, 'bleibt offen'); assert.equal(con.text, '');
  assert.deepEqual(con.log.map((l) => l.text), ['> wetter schneesturm', 'Wetter: Schneesturm']);
  type(con, 'zeit 7', ctx); consoleKey(con, 'Enter', ctx);
  assert.equal(g.world.clock, 420);
  consoleKey(con, 'ArrowUp', ctx); assert.equal(con.text, 'zeit 7', 'Verlauf zurück');
  consoleKey(con, 'ArrowUp', ctx); assert.equal(con.text, 'wetter schneesturm');
  consoleKey(con, 'ArrowDown', ctx); assert.equal(con.text, 'zeit 7', '↓ blättert im Verlauf vor');
  consoleKey(con, 'ArrowDown', ctx); assert.equal(con.text, '', 'zurück zur leeren Zeile');
  consoleKey(con, 'ArrowDown', ctx); assert.equal(con.sel, 0, 'danach in die Vorschläge');
  consoleKey(con, 'ArrowUp', ctx); consoleKey(con, 'Escape', ctx); openConsole(con, ctx);
  consoleKey(con, 'Enter', ctx);
  assert.equal(con.open, false, 'leere Zeile schließt');
  openConsole(con, ctx); type(con, 'x', ctx);
  assert.equal(consoleKey(con, 'Escape', ctx), 'close'); assert.equal(con.open, false);
  assert.equal(consoleKey(con, 'F5', ctx), null, 'andere Tasten tun nichts');
});

test('Befehle ändern die Welt; Fehler mit Hinweis statt Wirkung', () => {
  const g = newGame(), w = g.world, ctx = ctxOf(g);
  const run = (l) => execute(l, ctx);
  assert.ok(run('zeit abend').ok); assert.equal(w.clock, 19 * 60 + 30);
  assert.ok(run('uhr 06:15').ok, 'Alias'); assert.equal(w.clock, 375);
  assert.ok(run('wetter gewitter').ok); assert.equal(w.forceWeather, 'thunder'); assert.ok(w.wet >= 0.6, 'Gewitter macht nass');
  assert.ok(run('wetter klar').ok); assert.equal(w.forceWeather, 'clear');
  assert.ok(run('wetter auto').ok); assert.equal(w.forceWeather, null);
  assert.ok(run('schnee 0,5').ok); assert.equal(w.snow, 0.5);
  assert.ok(run('nass 1').ok); assert.equal(w.wet, 1);
  assert.ok(run('tempo 0').ok); assert.equal(w.clockRate, 0);
  assert.ok(run('verkehr 2').ok); assert.equal(w.trafficScale, 2);
  assert.ok(run('passanten 0').ok); assert.equal(w.pedScale, 0);
  assert.ok(run('fps').ok); assert.equal(g.debug.fps, true, 'ohne Argument: umschalten');
  assert.ok(run('fps aus').ok); assert.equal(g.debug.fps, false);
  assert.ok(run('ebenen an').ok); assert.equal(g.debug.levels, true);
  assert.ok(run('qualitaet niedrig').ok); assert.equal(g.debug.quality, 'low');
  const clock = w.clock, snow = w.snow;
  for (const [line, re] of [['zeit 25', /HH:MM/], ['schnee 2', /0 bis 1/], ['wetter hagel', /sonnig/], ['tempo -1', /tempo/],
    ['zeit', /Fehlt: zeit/], ['tp', /Fehlt/], ['tp xyzxyzxyz', /Kein Ort/], ['wetr', /Unbekannter Befehl.*wetter/], ['gott vielleicht', /an\|aus/]]) {
    const r = run(line);
    assert.equal(r.ok, false, line); assert.match(r.msg, re, line);
  }
  assert.equal(w.clock, clock); assert.equal(w.snow, snow, 'Fehler ändern nichts');
  assert.ok(run('hilfe').ok);
  for (const c of COMMANDS) assert.equal(findCommand(c.name), c);
  assert.equal(execute('zeit 12', { game: g, world: null, city }).ok, false, 'außerhalb des Spiels nur hilfe');
});

test('Schummeln: Gesundheit, Munition, Gott, Geld, Fahrzeug – jeder erfolgreiche Cheat wird gezählt', () => {
  const g = newGame(), w = g.world, ctx = ctxOf(g), p = w.player;
  press(g); // Zählung läuft schon (sonst gäbe es kein „vorher“ fürs Geld)
  p.hp = 5; p.mag = p.mag.map(() => 0);
  assert.ok(execute('leben', ctx).ok); assert.ok(p.hp > 50);
  assert.ok(execute('munition', ctx).ok); assert.ok(p.mag.some((n) => n > 0));
  assert.ok(execute('gott an', ctx).ok); assert.equal(w.god, true);
  const cars = w.cars.length;
  assert.ok(execute('auto polizei', ctx).ok || execute('auto police', ctx).ok);
  assert.equal(w.cars.length, cars + 1); assert.equal(w.cars.at(-1).kind, 'police');
  assert.equal(execute('auto rakete', ctx).ok, false);
  execute('geld +500', ctx);
  assert.equal(w.money, 500);
  execute('zeit 12', ctx); // kein Cheat
  const cheats = g.statQueue.filter((e) => e.type === 'cheat').map((e) => e.cmd);
  assert.deepEqual(cheats, ['leben', 'munition', 'gott', 'auto', 'geld'], 'nur erfolgreiche Cheats');
  press(g);
  assert.equal(g.stats.game.cheats, 5); assert.equal(g.stats.total.cheats, 5);
  assert.equal(g.stats.game.moneyEarned, 0, 'Schummelgeld zählt nicht als verdient');
});

test('Im Spiel: offene Befehlszeile hält die Welt an; tp teleportiert ohne Rückfrage; stats öffnet die Statistik', () => {
  const g = newGame(), w = g.world;
  for (let i = 0; i < 10; i++) press(g, { throttle: 1, moveY: -1 });
  openConsole(g.console, ctxOf(g));
  const clock = w.clock, x = w.player.x, y = w.player.y, t = g.stats.game.timePlayed;
  for (let i = 0; i < 30; i++) press(g, { moveY: -1, throttle: 1, pause: i === 5 });
  assert.equal(w.clock, clock); assert.equal(w.player.x, x); assert.equal(w.player.y, y);
  assert.equal(g.screen, 'playing', 'Pause-Taste geht an die Konsole, nicht ans Menü');
  assert.equal(g.stats.game.timePlayed, t, 'Spielzeit steht mit');
  type(g.console, 'tp kottbusser tor', ctxOf(g));
  consoleKey(g.console, 'Enter', ctxOf(g));
  consoleKey(g.console, 'Escape', ctxOf(g));
  assert.ok(g.teleport?.auto);
  for (let i = 0; i < 5 && g.teleport; i++) press(g);
  assert.equal(g.teleport, null, 'ohne Bestätigungsdialog');
  const tor = placeIndex(city).find((p) => p.name === 'Kottbusser Tor' && p.kind === 'U-Bahnhof');
  assert.ok(Math.hypot(w.player.x - tor.x, w.player.y - tor.y) < 400, 'am Kottbusser Tor');
  press(g);
  assert.equal(g.stats.game.teleports, 1);
  assert.equal(g.stats.game.kmTotal < 0.1, true, 'der Sprung zählt nicht als Strecke');
  // Ziel lädt noch (im Browser kommen die Kacheln später): auch dann ohne Rückfrage, sobald sie da sind
  const tel = g.stats.game.teleports;
  g.teleport = { pending: true, x: tor.x + 3000, y: tor.y, auto: true, name: 'Test' };
  for (let i = 0; i < 5 && g.teleport; i++) press(g);
  assert.equal(g.teleport, null, 'aufgelöst und ausgeführt, ohne auf Bestätigung zu warten');
  assert.ok(Math.hypot(w.player.x - tor.x - 3000, w.player.y - tor.y) < 400);
  press(g); assert.equal(g.stats.game.teleports, tel + 1);
  openConsole(g.console, ctxOf(g)); type(g.console, 'stats', ctxOf(g)); consoleKey(g.console, 'Enter', ctxOf(g));
  assert.equal(g.screen, 'stats'); assert.equal(g.console.open, false);
  press(g, { back: true });
  assert.equal(g.screen, 'playing');
});
