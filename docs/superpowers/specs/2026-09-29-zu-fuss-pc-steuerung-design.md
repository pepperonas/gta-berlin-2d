# Zu Fuß am PC: Diablo-Steuerung, Zoom, Tempo, Zielen (0.32.0) – Design + Umsetzungsleitfaden

Stand 29.09.2026. Vom Nutzer Punkt für Punkt entschieden (Antworten unten). **Dieses Dokument ist so geschrieben, dass
eine andere KI/Person ohne Vorwissen weitermachen kann**: Kontext, Ist-Stand mit Dateien/Zeilen, Soll, Aufgabenliste
mit Tests, Projektregeln. Zuerst `CLAUDE.md` (Architektur) und `README.md` lesen.

## Ziel (Nutzerwunsch)

Zu Fuß am Computer soll sich das Spiel wie ein modernes Action-RPG spielen:
1. näher herangezoomt, mit mehr sichtbaren Details;
2. realistische Lauf-/Sprinttempi;
3. neues **Standard-Steuerschema „Diablo“** (Diablo 3) für zu Fuß – die bisherige Steuerung bleibt spielbar/wählbar;
4. die Maus-Zielhilfe ist zu gut („man trifft praktisch immer“) → deutlich schwächer.

## Entscheidungen des Nutzers

| Frage | Entscheidung |
|---|---|
| Tempo | **Joggen + Sprint**: joggen 3,5 m/s (Standard), sprinten 7 m/s mit Ausdauer (erholt sich), langsam gehen 1,5 m/s auf Wunsch (Alt) |
| Angriff | **D3-typisch**: Linksklick auf Person/Auto = angreifen, Linksklick auf Boden = hinlaufen (halten = folgen), Shift+Linksklick = stehen bleiben und zum Zeiger angreifen, Rechtsklick = Tritt |
| Zielhilfe | **Nur wenn der Zeiger auf dem Ziel liegt**, sonst genau zum Zeiger; dazu Streuung, die mit Bewegung wächst |
| Details | **Näher + neue Details**: zu Fuß ~2× Zoom (Mausrad 1,5–2,6×), ab nahem Zoom eine Detailstufe (Berliner Gehweg, Bordsteine, Baumscheiben, feinere Gullys) |

## Projektregeln (verbindlich)

- Plain ES modules, **null Abhängigkeiten**, kein Build. Browser: `npm start` (Port per `PORT=8091 npm start` – **8080
  gehört einem anderen Projekt**; Testserver nie per `pkill`, nur per PID beenden).
- **Simulation DOM-frei und deterministisch** (`web/src/world.js` `createWorld/updateWorld`, fester Schritt 1/60 s).
  Zufall nur über `world.rng`; **Darstellung nie aus `world.rng`** (Hashes nutzen, z. B. `hash01` aus `map.js`).
  Eingaben kommen als **abstraktes Input-Objekt** (Form: `web/src/idle.js`, Tests: `tests/helpers/bot.js idle()`);
  `main.js` ist die einzige Browser-Brücke (Tastatur/Maus/Gamepad → Input-Objekt).
- Tests: `node --test` (`npm test`, ~6 min; einzeln `node --test tests/x.test.js`, nach Name
  `node --test --test-name-pattern="…" tests/x.test.js` – **Muster VOR der Datei**). Echte Karte über
  `tests/helpers/city.js realCity()` (Kreuzberg + Neukölln).
- **TDD**: Test zuerst, rot sehen, dann Code. **Jede neue Schutzprüfung mit Mutationsprobe**: vorher committen, Fehler
  absichtlich einbauen, Test muss scheitern, mit `git checkout -- <datei>` zurück (nie `git stash`).
- **Versionierung**: jede Verhaltensänderung bumpt SemVer in drei Dateien, die übereinstimmen müssen:
  `package.json`, `web/src/version.js`, `xbox/GtaBerlin/Package.appxmanifest` (`Identity Version="X.Y.Z.0"`);
  `CHANGELOG.md` braucht oben `## [X.Y.Z] – YYYY-MM-DD` (`tests/version.test.js` prüft das). Release: annotierter Tag
  `git tag -a vX.Y.Z -m "X.Y.Z"`, `git push --follow-tags`. README „Stand und Prüfumfang“ mit Testzahl nachführen,
  `CLAUDE.md` Architektur nachführen. Aktuelle Version vor dieser Etappe: **0.31.0** (386 Tests grün).
- UI-Texte, README, Doku: Deutsch. Code/Commits: Englisch.

## Ist-Stand (wo was liegt)

- **Tempo**: `web/src/config.js:21` `PLAYER = { radius: 7, walk: 80, run: 155, enterDist: 40 }` (px/s; **10 px = 1 m**
  → heute 8 m/s gehen, 15,5 m/s rennen). Passanten `config.js:22` `PED = { radius: 6, walk: 36, run: 120 }` (3,6 / 12
  m/s). Bewegung zu Fuß: `world.js` `updatePlayerOnFoot` (um Zeile ~489): `run = input.sprint || mag > 0.92`,
  `speed = (run ? PLAYER.run : PLAYER.walk) * (input.sprint ? 1 : mag)`. Passanten: `pedestrians.js:67` (`speed: PED.walk
  * (0.8 + rng() * 0.45)`), `:217` (Flucht mit `PED.run`), `combat.js:301` (Kämpfer).
- **Kamera**: `world.js` `export const FOOT_ZOOM = 1.3` (~Zeile 809) und `updateCamera` direkt darunter (zu Fuß
  `zoom = FOOT_ZOOM`; im Auto/Zug geschwindigkeitsabhängig). `cam.zoom` wird gedämpft (`damp(..., 2, dt)`).
- **Zielhilfe**: `combat.js:23` `ASSIST = { cone: 0.32, coneMouse: 0.1 }` (halber Kegel in rad); `aimAssist(w, p, ang,
  range, cone)` `combat.js:130` (sucht Ziel im Kegel, zielt exakt auf dessen Mitte, wenn frei sichtbar); Aufruf
  `combat.js:266` (`aim.mouse ? ASSIST.coneMouse : ASSIST.cone`). Waffen-Tabelle `WEAPONS` in `combat.js` (Streuung je
  Waffe vorhanden).
- **Eingabe**: `web/src/input.js` – Tastatur `readKeys` (Zeilen ~45–55: WASD/Pfeile, Shift = `sprint`, Strg = `fire`,
  V = `kick`, R = `reload`, Q = `wpnNext`, E = Aktion `a`, F = `y` Ein-/Aussteigen, G = `rideBtn`, H = Hupe, M = Karte,
  Space = `rb`/Handbremse), Gamepad `readPad`, `InputState.frame()` leitet die abstrakten Aktionen ab; Mausziel
  `aimWorld` setzt `main.js`. Rechte Maustaste: `weaponwheel.js createRightButton` (tippen = `enterExit`, halten =
  Waffenrad), in `main.js` (mousedown/mouseup), Mausrad = Waffe weiter/zurück bzw. im Rad drehen. Linke Maustaste =
  schießen (`pointer.fire`). Tab ist frei (main.js verhindert nur das Browser-Standardverhalten).
- **Steuerungsbildschirm**: `game.js` (Bildschirm „controls“) und `hud.js` (`drawControls`/Tabellen-Layout, Test
  `tests/hud-layout.test.js` „Steuerungsbildschirm: Tabelle passt in den 720px-Rahmen“).
- **Darstellung**: `render.js` (`Renderer.draw`, `quality` 'high'/'low' nach Zeichenzeit), Bodentexturen
  `textures.js` (`sidewalk`, `asphalt`, `cobble`, …, Muster in Weltkoordinaten), Fahrbahn-Kleinteile `decals.js`
  (`edgeDecals`: gully/manhole/patch/crack/oil, gecacht `e._decals`), Straßenquerschnitt `street.js laneOffsets/
  parkingStrip` (`e.cs` ist im Spiel in px), Bäume `city.list('tree')`/`render`-Hash.
- **Kollision/Laufraster**: Hindernisse in `world.solids` (SpatialHash; Segmente, Kreise, Rechtecke, Ebenen `lvl`;
  `car.js blocks(world, s, lvl)` entscheidet), Häuser `map.js inBuilding`. Test-Helfer `tests/helpers/city.js
  reachability(city, box, start, r, {cell})` flutet ein Raster – gutes Vorbild für das Laufraster.

## Soll

### A. Steuerschema (Einstellung)

- `game.settings.controls = 'diablo' | 'classic'`, Standard **'diablo'**, gespeichert in `localStorage`
  (Schlüssel `gta-controls`; ohne Speicher Standard). Umschalten im Menü „Steuerung“ (neue Zeile/Taste im
  Steuerungsbildschirm; Maus und A/Enter). Gilt **nur zu Fuß mit Maus/Tastatur**; Auto, Zug, Gamepad unverändert.
- **Klassisch** = heutiges Verhalten (WASD laufen, Maus zielt, Links schießt, Rechts tippen/halten = Ein-/Aussteigen/
  Waffenrad, Mausrad = Waffe), aber mit den neuen Tempi, Zoom und der neuen Zielhilfe.
- **Diablo** (zu Fuß):
  | Eingabe | Wirkung |
  |---|---|
  | Linksklick auf Boden | hinlaufen (Weg um Hindernisse); **halten** = dem Zeiger folgen (Ziel alle ~0,15 s neu) |
  | Linksklick auf Person/Auto | angreifen: Schusswaffe → bis in Reichweite laufen, dann dorthin schießen (halten = weiter); Nahkampf → hinlaufen und schlagen |
  | Shift + Linksklick | stehen bleiben, zum Zeiger angreifen (auch ohne Ziel) |
  | Rechtsklick | Tritt zum Zeiger (bisher V) |
  | Mausrad | Zoom (1,5–2,6×) |
  | 1–6, Q | Waffe wählen/weiter; **Tab halten** = Waffenrad (Maus wählt, Loslassen nimmt) |
  | Leertaste halten | sprinten |
  | Alt halten | langsam gehen |
  | F / E | Ein-/Aussteigen / Aktion wie bisher |
  | WASD | funktioniert weiterhin und **bricht einen Klick-Weg ab** |
- Umsetzungshinweis: `main.js` übersetzt Mausereignisse in **neue abstrakte Felder** am Input-Objekt (in `idle.js`
  ergänzen, alle `false`/`null` im Leerlauf), z. B. `moveTo: {x,y}|null` (Weltpunkt, Klick/halten),
  `attackTarget: {kind:'ped'|'car', id}|null`, `forceAttack: bool` (Shift+Links), `kick` (Rechts), `walkSlow` (Alt),
  `sprint` (Leertaste im Diablo-Schema). Die **Simulation** (`world.js`) entscheidet daraus Laufen/Angreifen –
  so bleibt alles in Node testbar. Welches Ziel unter dem Zeiger liegt, bestimmt eine reine Funktion (z. B.
  `pickTarget(world, x, y)` → nächstgelegener lebender Passant im Radius Körper+4 px oder Auto per OBB-Test).

### B. Wegfindung (Klick zum Laufen)

- Reine Funktion (neue Datei `web/src/footpath.js`): `findFootPath(world, from, to, lvl) → [{x,y}…] | null`.
  A* auf einem **Raster 8 px (0,8 m)** im Rechteck um Start und Ziel (+ 20 m Rand, höchstens 250 m Kantenlänge);
  eine Zelle ist frei, wenn ein Kreis mit `PLAYER.radius` dort nicht mit `world.solids` (über `blocks`, Ebene `lvl`),
  Häusern (`inBuilding`) oder stehenden Autos kollidiert. 8-Nachbarschaft, Ergebnis vereinfachen (Sichtlinien-
  Glättung). Unerreichbares Ziel → Weg zur **nächstgelegenen erreichbaren** Zelle. Deterministisch.
- Die Figur folgt dem Weg (Wegpunkte abarbeiten, Tempo wie unten), bricht bei WASD/neuem Klick ab; am Ziel kurze
  Klick-Markierung (Darstellung, `render.js`). Kosten messen (Ziel: < 5 ms je Klick im Test).

### C. Tempo und Ausdauer

- `PLAYER`: `walk: 15` (1,5 m/s), `jog: 35` (3,5 m/s, **Standard**), `sprint: 70` (7 m/s), `radius`/`enterDist`
  unverändert. Gamepad/Klassisch: Stick halb (≤ 0,6) = gehen, darüber joggen; Sprint (A bzw. Shift, Diablo:
  Leertaste) = sprinten; Alt = gehen.
- **Ausdauer** `player.stamina` 0…1 (Spielzustand, nicht gespeichert nötig): sinkt beim Sprinten in ~12 s von 1 auf 0,
  erholt sich in ~20 s (nach 1 s Pause); bei 0 nur noch joggen, bis wieder > 0,25. HUD: schmale Leiste im Waffenfeld,
  nur sichtbar beim Sprinten oder solange < 1.
- **Passanten realistisch**: `PED.walk: 13` (1,3 m/s, mit der vorhandenen ±-Streuung), `PED.run: 45` (4,5 m/s). Danach
  die Verkehrs-/Passantentests (`tests/traffic.test.js`, `tests/people.test.js`, `tests/life.test.js`,
  `tests/combat.test.js`) laufen lassen; wo ein Test absolute Tempi/Zeiten annahm, begründet anpassen (im Commit/
  CHANGELOG erwähnen).
- Gangbild (`gait.js`) nutzt das Tempo bereits (Kadenz, Rennen ab höherem Tempo) – prüfen, dass Joggen als Laufen und
  Gehen als Gehen aussieht (Schwellen ggf. auf die neuen Werte legen).

### D. Zielen

- **Maus** (beide Schemata): kein Kegel-Einrasten mehr. Liegt der Zeiger auf einer Person/einem Auto (`pickTarget`),
  zielt der Schuss auf deren Mitte, sonst **genau auf den Zeiger**. Danach wirkt die Waffenstreuung, multipliziert mit
  einem Bewegungsfaktor: Stand ×0,7, gehen ×1,0, joggen ×1,6, sprinten ×2,5. `ASSIST.coneMouse` entfällt.
- **Gamepad**: bisherige Zielhilfe (`ASSIST.cone`) bleibt, Bewegungsfaktor für Streuung gilt auch dort.
- Ziel des Nutzers: auf Distanz und in Bewegung geht spürbar viel daneben (Test: Trefferquote auf ein Ziel in 30 m
  Entfernung im Sprint < 50 %, im Stand auf 8 m > 85 %, Zeiger knapp neben dem Ziel → kein Einrasten).

### E. Zoom und Detailstufe

- Zu Fuß Standard-Zoom **2,0** (statt 1,3), Mausrad stufenlos **1,5–2,6** (nur zu Fuß; gespeichert `localStorage`
  `gta-foot-zoom`). `FOOT_ZOOM` bleibt der Standardwert; `updateCamera` nimmt `w.footZoom ?? FOOT_ZOOM`. Im Auto/Zug
  unverändert. HUD-Elemente (Zielpfeil, Maus→Welt-Umrechnung in `main.js`) nutzen bereits `cam.zoom` – prüfen.
- **Detailstufe** ab `cam.zoom ≥ 1.7` und `renderer.quality === 'high'` (nur Darstellung, deterministisch aus
  Hashes, gecacht wie `decals.js`):
  1. **Berliner Gehweg**: Granitplatten-Band in der Gehwegmitte (große helle Platten mit Fugen), zu beiden Seiten
     Mosaikpflaster (kleine graue Steine) – als zusätzliche Muster in `textures.js` (z. B. `granite`, `mosaic`) und
     entlang der Gehwegstreifen gezeichnet (Gehweg liegt zwischen Bordstein und Hauskante; Lage aus `e.cs`/`e.w`).
  2. **Bordsteinkanten**: helle Granitkante (~0,3 m) entlang der Fahrbahnränder.
  3. **Baumscheiben**: dunkle Erde-/Gitterfläche (~1,4 m) um Straßenbäume.
  4. **Gullys/Kanaldeckel** (aus `decals.js`) bei nahem Zoom feiner (Stäbe/Muster).
  Test: Detailstufe erscheint nur ab Zoom 1,7 und Qualität 'high' (Renderer-Statistik, z. B. `stats.detail`),
  Zeichenzeit bleibt im Budget (`RENDER.budgetMs`), keine ungültigen Koordinaten.

## Aufgaben (Reihenfolge, je mit Test zuerst)

1. **Tempo + Ausdauer + Passanten** (C). Tests: Tempi je Gangart (Klassisch/Gamepad-Input über `updateWorld`),
   Ausdauer sinkt/erholt sich, bei 0 kein Sprint; Passanten-Tempo; bestehende Suiten grün.
2. **Zielen** (D). Tests: Zeiger neben Ziel → Schuss geht zum Zeiger (kein Einrasten), Zeiger auf Ziel → Mitte,
   Streuung wächst mit Bewegung, Trefferquoten wie oben (mit festem `seed`, viele Schüsse über `shoot`).
3. **Zoom** (E, erster Teil): `w.footZoom`, Mausrad (main.js), Grenzen 1,5–2,6, Standard 2,0; Test über
   `updateCamera` (zu Fuß Ziel-Zoom = footZoom, im Auto unverändert).
4. **Steuerschema-Einstellung + Input-Felder** (A): `game.settings.controls`, Speicher, Steuerungsbildschirm (Layout-
   Test „passt in 720px“ muss grün bleiben); neue Input-Felder in `idle.js`; `main.js`-Übersetzung je Schema;
   Tab-Waffenrad.
5. **Wegfindung + Klick-Laufen** (B): `footpath.js` mit Tests (um ein Haus herum, durch eine Zaunlücke, unerreichbar →
   nächster Punkt, Laufzeit); `world.js` folgt `moveTo`/Weg; WASD bricht ab; Klick-Markierung.
6. **Klick-Angriff** (A): `pickTarget`, Angriff auf Ziel (in Reichweite laufen, dann schießen/schlagen), Shift-Angriff
   am Platz, Rechtsklick-Tritt. Tests über `updateWorld` mit Input-Feldern.
7. **Detailstufe** (E, zweiter Teil): Texturen/Zeichnen, Tests wie oben; im Browser ansehen (Screenshot).
8. **Doku + Release 0.32.0**: README (Steuerungstabelle mit beiden Schemata, Abschnitt „Zu Fuß“, Prüfumfang +
   Testzahl), CLAUDE.md (Input-Felder, footpath.js, Detailstufe, Zielen), CHANGELOG, drei Versionsdateien, Tag, Push.
   Browser-Schlussprüfung (Port 8091): Klick-Laufen um ein Haus, Angriff per Klick, Zoom per Mausrad, Detailstufe.

## Randfälle, an die man denken muss

- Klick auf einen Punkt in einem Haus/im Wasser → nächster erreichbarer Punkt; Klick außerhalb der Stadtgrenze → dito.
- Klick während Waffenrad offen / Konsole offen / Menü → ignorieren (game.js friert die Welt bei offener Konsole ein).
- Rechtsklick-Tippen war bisher Ein-/Aussteigen: im Diablo-Schema ist Rechts = Tritt; Einsteigen nur über F/E.
  **Im Auto** gilt weiter die alte Rechtsklick-Logik (Aussteigen per Tippen).
- Linksklick auf ein Auto mit Fahrer = angreifen (schießen); auf das eigene leere Auto: ebenfalls angreifen (D3:
  Klick = Aktion), Einsteigen bleibt F – bewusst, damit ein Klick nie überraschend einsteigt.
- Ebenen (`lvl`): Wegfindung nur auf der eigenen Ebene; Brücken/Unterführungen über Portale sind ein späterer Schritt
  (bei Klick auf andere Ebene: bis zum nächstgelegenen Punkt der eigenen Ebene).
- Zeiger über HUD-Elementen (Minikarte etc.) → kein Weltklick.
- Speicherstände: Ausdauer/Weg werden nicht gespeichert.
