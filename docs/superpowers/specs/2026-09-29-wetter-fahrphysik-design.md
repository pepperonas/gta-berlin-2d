# Wetterabhängige Fahrphysik (0.30.0) – Design

Stand 29.09.2026. Vom Nutzer am 28.09. vorgemerkt, in diesem Gespräch Frage für Frage entschieden.

## Ziel

Das Wetter soll man am Steuer spüren, nicht nur sehen: Nässe, Schnee und Glätte verlängern Bremswege, lassen Räder
durchdrehen und das Heck rutschen, Pfützen lassen bei Tempo aufschwimmen, Sturmböen versetzen Fahrzeuge. Der
KI-Verkehr fährt vorsichtiger, der eigene Zug (0.29.0) bremst auf nassen oder vereisten Schienen schlechter.
Stufe **„spürbar, aber fair“**: deutlich merkbar, aber Aufträge mit Zeitlimit bleiben bei jedem Wetter schaffbar.

## Entscheidungen des Nutzers

| Frage | Entscheidung |
|---|---|
| Stärke | spürbar, aber fair (nass Bremsweg ≈ +30 %, Schnee ≈ ×2, Glätte ≈ ×3; Aquaplaning abfangbar) |
| Glätte | kleine Temperaturkurve; Glätte = Nässe bei ≤ 0 °C |
| Aquaplaning | nur an den sichtbaren Pfützen (`edgePuddles`) ab etwa 70 km/h |
| Böen | alle Fahrzeuge, leichte stärker, auf Brücken ×1,6 |
| Bahnen | nur der eigene Zug (Nässe, Frost), Fahrplanzüge unberührt |

## Aufbau

Eine reine Stelle für alle Haftungsregeln, keine Weltabhängigkeit in `car.js`:

- **`weather.js temperatureAt(seed, dayCount, clock)`** (rein): Tageskurve mit Tiefstwert um 5 Uhr und Höchstwert um
  15 Uhr, stetig über Mitternacht (Tiefstwert des Folgetags). Spanne je Tagestyp: Winter −6…+3 °C, wechselhaft
  +4…+14 °C, normal +8…+22 °C, je Tag ±2 °C Verschiebung aus `seed`/`dayCount` (Hash, kein `world.rng`). Mit
  erzwungenem Schneewetter (`w.forceWeather` schnee/schneesturm) höchstens +1 °C, damit Glätte und Schnee plausibel
  zusammenpassen.
- **`w.ice`** (0…1, Spielzustand wie `w.wet`/`w.snow`, gespeichert): wächst, solange `w.wet > 0.1` und Temperatur
  ≤ 0 °C (voll in etwa 10 Spielminuten), taut über 0 °C (in etwa 20 Spielminuten). Nur mit `w.rhythm`, sonst 0.
  Alte Spielstände ohne `ice` laden mit 0. Konsole `glaette 0.8` setzt ihn wie `nass`/`schnee`, `temp` meldet die
  Temperatur.
- **`web/src/traction.js`** (neu, rein):
  - `roadCondition(world, x, y, lvl)` → `{ wet, snow, ice, puddle, covered, bridge }` für eine Stelle. Überdacht
    (Durchfahrt `e.passage`, Boden unter einer Brücke = eine Fläche höherer Ebene darüber) ⇒ `wet = snow = ice = 0`.
    Auf Brücken (`lvl ≥ 1`) Glätte ×1,5 (auf 1 begrenzt). `puddle` = der Punkt liegt in einer `edgePuddles`-Ellipse
    einer nahen Straße und es ist nass (`wet > 0.3`); liefert dann die Pfütze (für den Hash).
  - `tractionOf(cond)` → Faktoren `{ brake, accel, lat, steer }` (1 = trocken), Tabelle unten.
  - `gustPush(world, car, lvl)` → Querbeschleunigung `{ ax, ay }` in px/s² (Windrichtung aus `w.weather.wind`,
    Stärke `storm × max(0, gustAt − 0.9)`, Brücke ×1,6, geteilt durch die Fahrzeuggröße `hw·hh / (21·10)`); 0 für
    stehende Autos (< 5 px/s) und geparkte (`role === 'curb'`, `driver === null`).
- **`world.js`** schreibt je Schritt vor `stepCar` an jedes Auto `car.traction = tractionOf(roadCondition(...))`
  (ersetzt `car.wet`/`car.snow`), setzt Aquaplaning (`car.aqua`, Restzeit) und addiert den Böenschub zur
  Geschwindigkeit. Probe: Fahrzeugmitte plus die beiden Vorderräder für die Pfütze.
- **`car.js stepCar`** wendet `car.traction` an (fehlt es, wie trocken – bestehende Tests bleiben gültig).

### Haftungsfaktoren

| | Bremsen | Anfahren | Seitenhalt | Lenkung |
|---|---|---|---|---|
| nass (`wet` = 1) | 0,77 | 0,85 | 0,82 | 0,95 |
| Schnee (`snow` = 1) | 0,5 | 0,55 | 0,55 | 0,8 |
| Glätte (`ice` = 1) | 0,33 | 0,4 | 0,35 | 0,65 |

Je Zustand wird linear zwischen 1 und dem Tabellenwert nach seiner Stärke gemischt, die drei werden multipliziert und
das Ergebnis nie unter den Glättewert der Spalte gedrückt (der schlechteste Einzelzustand ist die Untergrenze, kein
nahezu haltloses Auto aus Schnee plus Glätte). Die Handbremse verliert Seitenhalt im selben Verhältnis.

Anfahren: Die Beschleunigung wird mit `accel` skaliert; unter 0,7 und bei Vollgas gilt das als Durchdrehen
(`car.spin` für Partikel: Spritzer bei Nässe, Schnee bei Schnee/Glätte statt Staub; kein Einfluss auf die Simulation
darüber hinaus).

### Aquaplaning

Rad in einer Pfütze (s. o.) und Vorwärtstempo > 70 km/h (194 px/s): `car.aqua = 0.35` s. Solange `car.aqua > 0`:
Seitenhalt ×0,15, Lenkung ×0,2, Bremse ×0,3, zusätzlich ein Gierimpuls von höchstens 0,6 rad/s, Richtung aus einem
Hash der Pfütze (nie `world.rng`). Ereignis `aquaplane { x, y, carId, player }` einmal je Auftreten (Spritzwasser,
Platschen, HUD, Statistik).

### Böen

Maßstab (Test): Pkw bei 50 km/h geradeaus, volle Sturmböe (storm 1, Böenspitze), ohne Gegenlenken: seitlicher Versatz
0,5–1 m über die Böe; auf einer Brücke etwa 1,5 m. Transporter/Bus deutlich weniger (Größe). Die KI lenkt über ihre
Spurverfolgung von selbst zurück.

## KI-Verkehr

`traffic.js driveAi` liest `car.traction` (fehlt: trocken):

- Zieltempo × (0,6 + 0,4 × `brake`).
- Bremskurven (Ampel/Haltelinie, Einfahrtstor, Zebrastreifen, Vordermann) rechnen mit `90 × brake` statt 90 px/s².
- Abstand zum Vordermann (`GAP_PX`-Bereich) × 1/`brake`.
- Kurvendeckel (heute 55 px/s bei großem Lenkwinkel) × `lat`.
- Auf Aquaplaning reagiert die KI nicht eigens (sie fährt durch das langsamere Zieltempo fast nie schnell genug).

Ziel: die bestehenden Dauer- und Engstellentests bleiben grün und laufen zusätzlich bei Schnee und Glätte ohne
Unfälle der KI untereinander.

## Eigener Zug

`trainphysics.js stepDrive(d, input, dt)` bekommt `input.adhesion` (0…1, fehlt: 1). Oberirdisch: nass 0,75, Frost
(Glätte) 0,6 (gemischt wie oben, Untergrenze 0,6); unter Tage (`undergroundAtS`) immer 1. Skaliert Betriebsbremse,
Notbremse und Zugkraft; die Zwangsbremsung rechnet mit der verringerten Notbremsung, der Zug hält weiter vor jedem
Hindernis (bestehender Bremskurven-Test, zusätzlich mit Haftung 0,6). `playertrain.js` bestimmt die Haftung aus
`roadCondition` an der Zugspitze (Tunnel ⇒ 1). Trinkgeldregel unverändert (wird bei Nässe schwerer, das ist gewollt).
Fahrplanzüge bleiben unberührt.

## Anzeige, Ton, Konsole, Statistik

- HUD: Temperatur neben Wochentag/Uhr (`☀ Fr 12:05 · 3 °C`). Im Auto oder als Zugführer ein Warnschild über dem
  Tacho bzw. der Fahrerleiste, wenn die Stelle es verdient – Vorrang: „Aquaplaning!“ (blinkt, solange `car.aqua`),
  „Glätte“ (`ice > 0.2`), „Schnee“ (`snow > 0.2`), „Sturm“ (Böenschub über der Schwelle), „Nässe“ (`wet > 0.3`).
  Überlappt nichts (Test in `hud-layout.test.js`).
- Ton: `aquaplane` → neues Platschen (Rauschstoß, tiefpass); Bild: Spritzwasser-Partikel am Auto.
- Konsole: `glaette 0–1`, `temp`.
- Statistik: Zähler `aquaplanes` („Aquaplaning“) im Abschnitt Unterwegs, nur für das Spielerauto.

## Grenzen (bewusst nicht)

Fußgänger und Radfahrer bleiben unberührt; keine Jahreszeiten, kein Laub, keine Streufahrzeuge, keine
Fahrplanverspätungen; Glätte ist flächig (nicht je Straße), außer überdachten Stellen und Brücken.

## Tests (node:test, Mutationsprobe je neuer Schutzprüfung)

- `temperatureAt`: Spannen je Tagestyp, Tiefst-/Höchstzeit, stetig über Mitternacht und Tageswechsel, deterministisch,
  Schneezwang ≤ +1 °C.
- `w.ice`: wächst nur bei Nässe und ≤ 0 °C, taut darüber, Spielstand speichert/lädt, alter Spielstand ⇒ 0, Konsole.
- `roadCondition`: überdacht trocken, Brücke glatter, Pfütze nur bei Nässe und nur in der Ellipse.
- `tractionOf`: Tabellenwerte, Mischung, Untergrenze.
- Autophysik: Bremsweg aus 50 km/h trocken/nass/Schnee/Glätte im Verhältnis ≈ 1 : 1,3 : 2 : 3; Anfahren begrenzt;
  Seitenrutschen in der Kurve stärker; Aquaplaning nur über 70 km/h in einer Pfütze, Dauer 0,35 s, Ereignis einmal,
  deterministisch; Böen-Versatz im Maßstab oben, Brücke stärker, schwere Fahrzeuge weniger, stehende gar nicht.
- KI: Zieltempo und Bremsweg passen sich an; Dauertest bei Schnee/Glätte ohne KI-Unfälle.
- Zug: Haftung verlängert den Bremsweg, Zwangsbremsung bleibt sicher (nie über der verringerten Bremskurve), Tunnel
  unberührt, Fahrplanzüge unberührt.
- HUD: Temperatur, Warnschild je Zustand, Vorrang, keine Überlappung in allen Fenstergrößen.
- Bestehende Tests ohne Wetter unverändert grün (fehlende `traction` = trocken).

## Review Focus

1. Fahrphysik bleibt bildratenunabhängig und deterministisch (kein `world.rng` für Aquaplaning/Böen).
2. Keine Regression bei trockenem Wetter (Faktoren exakt 1, bestehende Fahr- und Verkehrstests).
3. KI-Verkehr bei Glätte: keine Auffahrunfälle, keine Staus aus zu großen Abständen an Engstellen/Reservierungen.
4. Zwangsbremsung des Zugs bei verringerter Haftung hält weiter vor Hindernissen.
5. Spielstand mit `ice`; Konsole und erzwungenes Wetter passen zur Temperatur.

## Version

0.30.0 (Minor: neues Verhalten). CHANGELOG, README (Wetter-Abschnitt, Steuerung unverändert, Stand und Prüfumfang),
CLAUDE.md (Architektur: traction.js, temperatureAt, w.ice), Versionen in drei Dateien, annotierter Tag.
