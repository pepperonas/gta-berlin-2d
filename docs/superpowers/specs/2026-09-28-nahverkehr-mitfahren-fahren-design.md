# Nahverkehr: mitfahren und selbst fahren – Entwurf

Stand 2026-09-28, freigegeben im Gespräch. Ziel-Version 0.29.0 (Etappen 0.29.x/0.30.x möglich).

## Ziel

Der Spieler kann in Bus, Straßenbahn, S-Bahn und U-Bahn **als Fahrgast** einsteigen (überall in der Nähe, auch während
der Fahrt) und Straßenbahn, S-Bahn und U-Bahn **selbst führen** – nur Gas und Bremse, gelenkt wird nicht, der Zug folgt
seiner Linie. Unterirdisch gibt es eine **Tunnelansicht**. Busse lassen sich wie bisher als Fahrer übernehmen
(Autoklau-Weg), neu ist das Mitfahren.

## Ausgangslage (Code heute)

- `transit.js`: Linienmuster (`shape`, `stops`, `stopNames`, `off`, `deps`, `dwell`), virtuelle Fahrzeuge
  `{ tau, delay, key, live?, gone?, blockedT? }`, Lage aus `positionAt(p, tau)` + `pointOn`, Wagen aus `trainCars`.
- `transitlive.js`: Busse werden echte KI-Autos (`materializeBus`, `car.duty`), Straßenbahnen bleiben kinematisch
  (`tramBlocked`, `w.railObs`), S-/U-Bahn nur virtuell, gezeichnet nur, wo ein Gleis oberirdisch liegt (`transitVisible`).
- Spieler: `tryEnter`/`tryExit` nur für `w.cars`, `p.inCar`; Kamera folgt `playerCar`.
- Tunnelgleise sind nicht in der Karte (Build verwirft `tunnel=*`); die GTFS-Linienwege laufen aber durch die Tunnel.

## Begriffe

- **Fahrt (ride):** Spieler sitzt in einem Nahverkehrsfahrzeug. Zustand `w.player.ride`.
- **Spielerzug:** eine Bahn, die der Spieler führt. Sie verlässt den Fahrplan und lebt in `w.playerTrain`.
- **Unter Tage:** Stelle einer Linie, an der kein sichtbares Gleis liegt (dieselbe Prüfung wie `transitVisible`,
  nur für Schienenmodi S/U; Straßenbahn ist immer oberirdisch).

## 1. Einsteigen und Aussteigen

### Wer ist wo?

`w.player.ride` (nur gesetzt während einer Fahrt):

```
{ kind: 'passenger' | 'driver', mode: 'bus'|'tram'|'sbahn'|'ubahn',
  ref: { pid, key } | { carId } | { playerTrain: true },   // Fahrplan-Fahrzeug, Bus-Auto oder Spielerzug
  car: 2,                    // Wagen, in dem man sitzt (für die Lage)
  lastStop: { x, y, name },  // für Speichern und Notausstieg
  since: w.time }
```

`p.inCar` bleibt `null` während einer Nahverkehrsfahrt (außer beim Bus als Fahrer = bisheriger Weg). Alle Stellen, die
„zu Fuß“ annehmen, prüfen zusätzlich `!p.ride` (Kampf, Kollision, Passanten-Kontakte, Waffenrad, Mission-Einladen).

### Tasten

| | Tastatur | Controller |
|---|---|---|
| Als Fahrgast einsteigen / aussteigen | E | A |
| Als Fahrer übernehmen / aussteigen | F oder rechte Maus tippen | Y |
| Türen öffnen/schließen (als Fahrer, an einer Haltestelle) | E | A |
| Gas / Bremse (als Fahrer) | W / S | RT / LT |
| Notbremse | Leertaste | B |
| Wenden am Linienende | E | A |

„Aktion“ (E/A) hat Vorrang für Aufträge (Mission-Einladen); nur wenn keine Missionsaktion möglich ist, gilt es als
Einsteigen.

### Einsteigen als Fahrgast

- Reichweite: Spieler innerhalb von 2,5 m (25 px) neben irgendeinem Wagen (Abstand zum Wagen-Rechteck) auf gleicher
  Ebene. Nahverkehrsfahrzeuge in Reichweite werden über `transitCarsNear(w, x, y, r)` (rein) gesucht: virtuelle
  Fahrzeuge im Umkreis (mit `trainCars`) und Bus-Autos.
- Auch während der Fahrt (Nutzerwunsch „überall“). Ab 25 km/h Relativtempo gilt es als **Aufspringen**: gelingt immer,
  aber mit Ereignis `hop-on` (Ton, Statistik).
- U-Bahn/S-Bahn unter Tage sind von der Straße aus nicht erreichbar (keine Wagen in Reichweite, weil man nicht unter
  Tage steht).

### Aussteigen

- Steht das Fahrzeug oder fährt < 10 km/h: normal aussteigen, Platz neben der Tür (wie `tryExit`, gleiche
  Freiplatzsuche, erst Bahnsteigseite = rechts in Fahrtrichtung, dann links).
- Schneller: **Abspringen** – der Spieler landet neben dem Wagen mit Schwung in Fahrtrichtung und ist 1,2 s betäubt
  (`stun`), leichter Schaden ab 40 km/h (10 HP, nie tödlich im Gottmodus).
- Unter Tage nur, wenn der Zug an einem Bahnsteig steht; sonst Hinweis „Nur am Bahnsteig“. Man kommt am
  **Straßenausgang** heraus: nächster Bahnhof-POI (`ubahn`/`sbahn`/`bahn`) mit demselben Namen wie der Halt, sonst
  nächster freier Gehwegplatz über dem Bahnsteig (`nearestSpot`).
- Verschwindet das Fahrzeug (Fahrplan-Fahrzeug fährt aus der verfolgten Zone, Linie endet, Neuaufbau nach Teleport):
  Notausstieg an der letzten Haltestelle (`ride.lastStop`), Hinweis „Endstation“.

### Fahrgast-Ansicht

- Spieler unsichtbar, Kamera folgt dem Wagen (wie beim Auto, Zoom nach Tempo).
- HUD-Leiste oben: Linienname in Linienfarbe, Richtung (letzter Halt), nächster Halt + Entfernung, bei Halt „Hält –
  E: aussteigen“.
- Busfahrgast: der Bus bleibt KI-gesteuert (`car.duty`), der Spieler ist an `carId` gebunden.

## 2. Selbst fahren (Straßenbahn, S-Bahn, U-Bahn)

### Übernahme

- F/Y innerhalb von 3 m an der **Spitze des ersten Wagens** (Führerstand). Das virtuelle Fahrzeug wird `gone`, sein
  Zustand wird in einen Spielerzug überführt – ohne Sprung (gleiche Lage, gleiches Tempo).
- Busse: unverändert (Autoklau). Als Busfahrer bleibt die Linienführung aus (`ai.follow` gehört zur KI).

### Spielerzug `w.playerTrain`

```
{ pid, mode, s, v,            // Lage auf dem Linienweg (px) und Tempo (px/s, ≥ 0; rückwärts gibt es nicht)
  doors: 'closed'|'open', doorT,
  stop: { i, arrivedT } | null, // an welcher Haltestelle gerade gehalten wird
  served: Set of stop indices, passengers: n, cars (Anzahl aus TRAIN) }
```

- **Physik (rein, `trainphysics.js`):** `stepTrain(t, input, dt, cfg)` – Beschleunigung aus Gas (Kennlinie: voll bis
  ~40 % der Höchstgeschwindigkeit, dann abnehmend), Bremse bis ≈ 1,2 m/s² (Straßenbahn 1,5), Notbremse 2,5 m/s²,
  Rollwiderstand; keine Bewegung bei offenen Türen. Höchsttempo: Straßenbahn 60, U-Bahn 70, S-Bahn 100 km/h
  (`TRAIN_DRIVE` in `config.js`).
- **Lage:** `pointOn(p.shape, s)` für die Spitze, Wagen dahinter über `trainCars(p, s)`.
- **Hindernisse vorn (Abstand entlang des Linienwegs):** Fahrplan-Fahrzeuge desselben Musters und aller Muster, deren
  Weg an der Stelle auf ≤ 3 m mit dem eigenen übereinstimmt (gleiche Strecke), Straßenbahn zusätzlich alles, was
  `tramBlocked` heute erkennt. Der Zug hält selbsttätig 8 m davor (Zwangsbremsung, Hinweis „Zug voraus“), fährt nie
  hinein. Autos, die in eine fahrende Straßenbahn geraten, stoßen wie heute über `w.railObs` ab und nehmen Schaden.
- **Haltestellen:** Steht der Zug (v < 0,3 m/s) mit der Spitze innerhalb von ±25 m um eine Haltestelle, gilt er als
  „an der Haltestelle“. E/A öffnet die Türen; nach dem Öffnen steigen Fahrgäste aus/ein (Zahl deterministisch aus
  Uhrzeit, Linie, Halt), Türen schließen mit E/A oder nach 20 s selbst. Trinkgeld: bis 10 €, voll bei Halt innerhalb
  ±3 m und sanfter Bremsung (größte Verzögerung der letzten 8 s ≤ 1,3 m/s²), anteilig bis ±25 m. Ohne Halten
  vorbeifahren ist erlaubt (keine Strafe).
- **Linienende:** Am letzten Halt stoppt der Zug zwangsweise. E/A „Wenden“: der Zug wechselt auf ein Muster derselben
  Linie in Gegenrichtung, dessen erster Halt ≤ 60 m entfernt ist (Suche über `name`/Halte), `s` = Lage dort; gibt es
  keins, bleibt nur Aussteigen.
- **Aussteigen als Fahrer:** der Zug bleibt stehen und wird nach 30 s außer Sicht entfernt (kein Rückweg in den
  Fahrplan nötig – neue Abfahrten füllen die Linie).
- `w.railObs` bekommt die Wagen des Spielerzugs (Straßenbahn wie heute; S/U nur, wo oberirdisch, damit Autos auf
  Bahnübergängen anstoßen).

### Andere Züge

Fahrplan-Fahrzeuge desselben Musters hinter dem Spielerzug bleiben stehen, wenn ihr Abstand unter 60 m fällt
(`blocked`-Mechanik wie die Straßenbahn heute), damit niemand durch den Spielerzug fährt. Mehr Leitsystem gibt es nicht.

## 3. Tunnelansicht

- **Unter Tage erkennen (rein, `tunnel.js`):** `undergroundAt(city, x, y, mode)` – S/U: kein sichtbares Gleis
  (`rail`-Feature) innerhalb 50 px, gleiche Regel wie `transitVisible`; Straßenbahn/Bus nie. Je Muster werden die
  unterirdischen Abschnitte als Bogenlängen-Intervalle berechnet und am Muster zwischengespeichert, nur für die nahe
  Umgebung (Kacheln müssen geladen sein; Rückfall: nicht unter Tage).
- **Überblendung:** `w.underground` (0…1) folgt weich (0,6 s), solange der Spieler unter Tage fährt oder steht.
- **Zeichnen (`render.js`, neuer Durchgang nach der Welt, vor dem HUD):**
  - Stadt abdunkeln: Fläche in Tunnelfarbe mit Deckkraft 0,72 × `w.underground`.
  - Röhre: für jedes Muster, dessen unterirdischer Abschnitt im Bild liegt, ein Betonband (Breite = Wagenbreite + 2 m)
    entlang des Linienwegs, Gleise darin, Lichtpunkte alle 25 m; die Linie des Spielers kräftiger, andere gedämpft.
  - Bahnsteige: an Halten im Tunnel ein helles Rechteck beidseits (40 m × 4 m, Richtung = Linienweg), Name in
    Linienfarbe.
  - Züge im Tunnel (der eigene und Fahrplan-Züge in der Röhre) werden in diesem Durchgang gezeichnet – bisher
    verschwanden sie.
- Zu Fuß ändert sich nichts (keine Bahnhofs-Innenräume).

## 4. Speichern, Statistik, Sonstiges

- **Speichern während einer Fahrt:** gespeichert wird `ride.lastStop` als Spielerposition (zu Fuß). Beim Laden steht
  man dort; die Fahrt ist beendet.
- **Statistik:** neue Zähler `rides` (Mitfahrten), `kmTransit`, `kmTrainDriven`, `stopsServed`, `tipsEarned`,
  `hopsOn`/`hopsOff`.
- **Ereignisse** für Ton/Effekte: `board`, `alight`, `hop-on`, `hop-off`, `doors-open`, `doors-close`, `train-take`,
  `tip`, `train-blocked`, `turnaround`.
- **Determinismus:** Fahrgastzahlen, Trinkgeld und Tunnelabschnitte ohne `w.rng`; der Spielerzug ist Simulationszustand
  (fester Zeitschritt), Nachladen von Kacheln ändert ihn nicht.
- **Konsole:** `tp` beendet eine Fahrt (Notausstieg am Ziel).

## Nicht enthalten (bewusst)

Signale/Blockabstände, Weichen stellen, Fahrplanpünktlichkeit als Wertung, Bahnhofs-Innenräume, Busse auf ihrer
Linienführung als Spielerfahrer, Fahrgäste als sichtbare Figuren im Wagen.

## Einheiten und neue Module

| Modul | Aufgabe | Abhängigkeiten |
|---|---|---|
| `web/src/ride.js` (rein) | Fahrzeuge in Reichweite, Einsteigen/Aussteigen/Abspringen, Lage des Fahrgasts, Notausstieg | transit.js, map.js |
| `web/src/trainphysics.js` (rein) | Zugfahrt: Tempo, Bremsen, Haltestellen, Türen, Trinkgeld, Wenden | config.js |
| `web/src/playertrain.js` | Spielerzug in der Welt: Übernahme, Hindernisse, railObs, andere Züge blockieren | transit.js, transitlive.js, trainphysics.js |
| `web/src/tunnel.js` (rein) | unterirdische Abschnitte je Muster, `undergroundAt` | map.js |
| `render.js` / `hud.js` | Tunnel-Durchgang, Züge unter Tage, Fahrgast-/Fahrer-Leiste, Tacho, Türen | – |

## Tests (node:test, Mutationsproben je neuer Schutzprüfung)

- Einsteigen: Reichweite je Wagen, gleiche Ebene, während der Fahrt (Aufspringen), Bus als Fahrgast lässt die KI fahren.
- Aussteigen: stehend neben der Tür, schnell = Abspringen mit Betäubung, unter Tage nur am Bahnsteig und Ausgang am
  Bahnhof, Notausstieg, wenn das Fahrzeug verschwindet.
- Zugphysik: Höchsttempo je Art, Bremsweg, Notbremse, stehen bei offenen Türen, nie rückwärts, bildratenunabhängig.
- Haltestellen: Halten erkannt, Türen, Trinkgeld voll/anteilig/keins, Durchfahren ohne Strafe, Wenden am Linienende.
- Hindernisse: Spielerzug hält vor einem Fahrplan-Zug auf derselben Strecke, fährt nie hinein; Fahrplan-Zug hinter dem
  Spielerzug wartet; Straßenbahn hält vor Autos/Passanten.
- Tunnel: echte Karte – U8 am Kottbusser Tor liegt unter Tage, U1 an der Skalitzer Straße oberirdisch (Hochbahn);
  Überblendung stetig; Züge unter Tage werden gezeichnet, wenn der Spieler unter Tage ist.
- Speichern während der Fahrt → Laden an der letzten Haltestelle zu Fuß. Statistikzähler.
- Bestehende Tests (Verkehr, Autopilot, Nahverkehr) unverändert grün.
- Browser: Straßenbahn M10 mitfahren und fahren, U8 als Fahrgast durch den Tunnel, U1 auf der Hochbahn fahren,
  Aufspringen/Abspringen, Bildrate.
