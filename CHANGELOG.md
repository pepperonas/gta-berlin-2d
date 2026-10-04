# Changelog

Alle nennenswerten Änderungen an GTA Berlin. Format nach [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionen nach [Semantic Versioning](https://semver.org/lang/de/). Solange die Version mit `0.` beginnt, ist das Spiel
ein Prototyp: Spielstände, Kartenformat und Steuerung können sich zwischen Versionen ändern.

## [Unreleased] – Entwicklungsstand 2026-10-04

### Neu

- Native Rust-Portierung: Farbabstimmung (warm in der Dämmerung, kühl in der Nacht) und Vignette.
- Native Rust-Portierung: erleuchtete Fenster am Abend, in der Nacht und bei trübem Wetter (Wohnungen, Lichtfarben,
  Fernsehflackern, Vorhänge), im eigenen Durchgang nach dem Licht.
- Native Rust-Portierung: U-/S-Bahnhöfe und Bahnhofseingänge auf der Minikarte; Bar-Auslastung live aus dem Netz
  (`--bars live|URL`, Befehl `bars live`, alle zwei Minuten neu).
- Native Rust-Portierung: zwölf Menschen-Typen nach Bezirk, Uhrzeit und Wochentag (Tempo und Aussehen), Jogger
  und Hundehalter mit Hund an der Leine, Kinderwagen, Stock, Aktentasche und Kopfbedeckungen (`--bildschirm leute`).
- Native Rust-Portierung, Nahverkehr C: begehbare U-Bahnhöfe aus dem Fahrplan (Eingänge an der Straße,
  Bahnsteig mit Treppen, Säulen, Abfahrtstafeln und Wartenden, Einsteigen am Bahnsteig, Aussteigen unter Tage am
  nächsten Bahnsteig) und die Tunnelansicht bei Fahrten unter Tage (`--bildschirm bahnhof|tunnelfahrt`).
- Native Rust-Portierung, Phasen 1–2: Cargo-Workspace mit winit/wgpu, Metal-/DX12-
  Konfiguration, steuerbarer Kamera und 60-/120-FPS-Zielmodus. Asynchrones Streaming
  der bestehenden Berlin-Kacheln, indizierte GPU-Meshes für Straßen/Flächen/Gebäude,
  Dach-/Fassadenschattierung und instanzierter Baum-/Decal-Atlas. `cargo run` startet
  die native Kartenansicht; `--check-map` prüft alle Kacheln, `--capture` exportiert
  ein GPU-Renderziel. Die Spiellogik bleibt vorerst im JS-Prototyp; native Xbox-
  Ausführung ist unbestätigt. Stand und Grenzen: `docs/NATIVE-RUST.md`.
- Native Rust-Portierung, Phase 3: neues Crate `berlin-sim` mit SAT-Kollision, Raster-Hash und
  Nachbarschaftsraster, Stadtmodell mit Kachel-Streaming, Ebenen/Portalen, Arcade- und Einspur-Fahrphysik
  (Lastverschiebung, ESP/ASR, ABS, Drift, Wheelie/Stoppie, Haftung je Untergrund und Nässe/Schnee/Glätte),
  Spurgraph mit Ampeln, KI-Verkehr mit Kreuzungs-/Engstellen-Reservierungen, Passanten, Parkern, der Mission
  „Kisten für den Kiez“ und Spielständen als JSON-Datei (Format der Browserfassung). `cargo run` ist damit
  spielbar; Autos, Passanten und Missionsziel werden instanziert gezeichnet. `--check-sim SEKUNDEN` prüft die
  Simulation ohne Fenster. 58 Rust-Tests, davon 10 Integrationstests auf den echten Kacheln.
- Native Rust-Portierung, Phase 4: Tag-/Nachtlauf aus der Spieluhr (`daylight.rs`), Laternenstandorte wie
  `lamps.js`, Hausschatten per Wand-Extrusion im Vertex-Shader und Baumkronenschatten in einer Schattenmaske,
  Lichtkarte mit Laternen, Scheinwerferkegeln, Rück-/Bremslichtern und Ampeln; Dächer bekommen nur das
  Umgebungslicht. `--uhr HH:MM` und Taste T (+1 Stunde).
- Native Rust-Portierung, Phase 5: synthetisierter Klang im neuen Crate `berlin-audio` (DSP-Bausteine nach
  Web-Audio-Vorbild, Synthesizer, Ausgabe über `cpal`): Motoren mit Wellentabellen aus dem Zylinderspektrum,
  Schalten, Turbo, Reifen, Quietschen, Fahrtwind, vier Fremdfahrzeuge mit Panorama und Doppler, Stadt, Vögel,
  Wasser, Dämpfung im Auto, Ereignisklänge und Schritte. Mischregeln (`enginevoice`, `soundscape`, `ambience`)
  rein und getestet in `berlin-sim`. Taste M, `--stumm`, `--audio-wav` für eine gemessene Offline-Fahrt.
- Native Rust-Fassung: HUD im Bild (Bitmapschrift mit Umlauten; Geld, Uhr, Auftrag mit Zeit, Tacho mit Drehzahl
  und Gang, Fahrzeugname, Zielpfeil, Hinweise, Briefing, Ergebnis) und Gamepad über `gilrs` mit der Belegung der
  Browserfassung. `--im-auto` startet im eigenen Auto.

- Native Rust-Fassung: Wetter (`sim/weather.rs` nach `weather.js`): Dreistundenblöcke, Wind und Böen, Blitze und
  Donner, Temperatur, Nässe/Schnee/Glätte auf der Straße mit Haftung und Sturmböen. Im Bild gedämpftes Tageslicht,
  Regen, Schnee, Nebelschleier und Blitze; im HUD Wetter, Temperatur und Warnschild; Donner und Regen im Klang.
  `--wetter ART` und Taste N.
- Native Rust-Fassung: Minikarte unten links (echte Kartenmeshes mit zweiter Kamera im HUD-Rechteck, Häuser als
  dunkle Grundrisse; Autos, Ziel am Rand, Spielerpfeil, Nordmarke).
- Native Rust-Fassung: große Karte (Tab/View): Stadtplan aus `overview.json` mit eigener Linien-/Flächen-Pipeline,
  Zoom 1–64, Beschriftung von Bezirken, Ortsteilen, Kiezen und Bahnhöfen ohne Überlappung; `--stadtplan ZOOM`.
  Die View-Taste schaltet jetzt die Karte statt des Tons (Ton: M).
- Native Rust-Fassung: Titelbildschirm (lebende Stadt, Skyline; Fortsetzen/Neues Spiel/Steuerung/Beenden),
  Pausenmenü (Esc/P/Menü-Taste; Speichern, Mission neu starten, Hauptmenü) und Steuerungstafel. Ohne Option
  startet das Spiel im Titel; `--new`, `--fortsetzen`, `--bildschirm pause|steuerung`.
- Native Rust-Fassung: Ergebnismenü nach Aufträgen (Weiter / Erneut versuchen / Frei weiterspielen, Welt steht)
  und Statistik (`sim/stats.rs` nach `stats.js`; dieses Spiel und insgesamt, `stats.json` neben dem Spielstand,
  aus Titel und Pause erreichbar).
- Native Rust-Fassung: Kampf (`sim/combat.rs` nach `combat.js`): sechs Waffen und Tritt, Schüsse als Strahlen,
  Nahkampfbogen, Zielhilfe am Stick, Kämpfer unter den Passanten, beschossene Fahrer fliehen, K. o. mit
  Krankenhaus und Gebühr; Mündungsfeuer, Leuchtspuren, Blut, Lebensleiste, Waffenanzeige, Kampfklänge und
  Kampfstatistik. `--kampf-demo` für Aufnahmen.
- Native Rust-Fassung: Maus. Zu Fuß zielt die Figur auf den Zeiger (rastet auf Personen und Autos ein), links
  angreifen, rechts treten, Fadenkreuz; Menüs per Zeigen und Klicken; Stadtplan mit Rad-Zoom um den Zeiger und
  Ziehen.
- Native Rust-Fassung: Diablo-Schema zu Fuß (Standard, ←/→ in der Steuerungstafel, `settings.json`): Klick läuft
  mit Wegsuche (`sim/footpath.rs`) hin, greift Personen an, steigt in Autos ein; Strg + Klick greift am Platz an.
  Smoke-Tests und Aufnahmen ignorieren Eingaben.
- Native Rust-Fassung: Polizei und Rettungsdienst (`sim/services.rs` nach `services.js`): Tote rufen einen
  Rettungswagen, Schüsse einen Streifenwagen; Zielfahrt über den Spurgraph, Martinshorn und Blaulicht (nachts als
  Lichtquelle), gelegentliche Vorbeifahrten.
- Native Rust-Fassung: Fahrzeugarten im Verkehr (`sim/fleet.rs` nach `fleet.js`): Lkw, Paketwagen, Müllauto,
  Motorrad und Roller nach Uhrzeit, Wochentag und Straße; Arbeitshalte mit Warnblinker bzw. Müllwerkern;
  Kastenwagen- und Zweirad-Darstellung; `--fahrzeugschau` für Aufnahmen.
- Native Rust-Fassung: Wackelnde Streaming-Tests behoben (etwa jeder fünfte Lauf schlug fehl). Parallele Tests
  bekamen unter macOS denselben Testordner, weil die Uhr nur mikrosekundengenau ist; jetzt mit Zähler im Namen.
- Native Rust-Fassung: Fahrräder und E-Roller (`sim/bikes.rs` nach `bikes.js`): auf Radstreifen bzw. am
  Fahrbahnrand, Ampeln und Hindernisse, Stürze nach Zusammenstoß oder Treffer, Räder nehmen und kapern,
  Radfahrer als Ziele und per Klick; Statistik dazu.
- Native Rust-Fassung: Waffenrad (`game/wheel.rs` nach `weaponwheel.js`): rechte Maustaste bzw. LB halten,
  Zeiger oder Stick wählt, Zeitlupe; tippen tritt bzw. nimmt die vorige Waffe.
- Native Rust-Fassung: Teleport per Klick auf den Stadtplan mit Rückfrage und Ortsnamen; lädt das Ziel nach,
  landet zu Fuß auf dem Gehweg und im Auto auf einer Fahrspur. `city.rs` liest dazu POIs, Hausnummern,
  Stadtmöbel, Einwohnerdichte sowie Bezirke und Ortsteile.
- Native Rust-Fassung: Stadtleben. Tagesrhythmus (`rhythm.rs`): Zielbevölkerung nach Uhrzeit, Wochentag,
  Verkehrszählung, Einwohnerdichte und Lokalen. Lebensorte (`life.rs`): Wartende, Raucher, Clubschlange,
  Späti-Runde, Cafégäste, Schaufenstergucker, Straßenmusik, Bänke und Decken im Park (neuer Passantenzustand
  `Hang`). Tauben und Enten (`animals.rs`), die auffliegen bzw. wegschwimmen, und abgestellte E-Roller.
- Native Rust-Fassung: Wetter am Boden. Pfützen lösen Aquaplaning aus (mit Spritzwasser-Klang); nasser Asphalt,
  Pfützen mit Regenringen, Aufschlagringe, Schneedecke, Schnee und Matschspuren auf den Straßen, Reifenspuren im
  Schnee, Bodennebel und Wolkenschatten.
- Native Rust-Fassung: Befehlszeile (Enter) wie im Browser – Uhrzeit, Wochentag, Wetter samt Wettertafel, Dichte,
  Teleport zu Straßen/Bahnhöfen/Ortsteilen/Kiezen/Bezirken, Geld, Gesundheit, Munition, Gottmodus, Fahrzeuge;
  Vorschläge mit Tippfehler-Toleranz, Verlauf, Statistikzeile für Konsolenbefehle.
- Native Rust-Fassung: Nachtleben. Typische Auslastung je Lokalart, Bar-Feed (gostumblr-Format, `web/data/bars.json`,
  `--bars`, Befehl `bars`), Raucher und Schlangen vor vollen Bars, Stimmengewirr, Lachen, Gläser und Club-Bass im Klang.
- Native Rust-Fassung: Nahverkehr nach VBB-Fahrplan – Busse als KI auf ihrer Linie mit Halten, Straßenbahnen mit
  Gleisen, Klingel und Vorrang, S- und U-Bahn auf oberirdischen Gleisen; `--befehl` für Aufnahmen.
- Native Rust-Fassung: Mitfahren (G) in Bus und Bahn mit Auf- und Abspringen, Bahn am Führerstand übernehmen (F) und
  selbst fahren – Zwangsbremsung, Türen, Trinkgeld für sanftes, genaues Halten, Wenden am Linienende; Fahrgast- und
  Fahrerleiste im HUD.
- Filmische Stadtgrafik mit einer gemeinsamen Material-/Lichtpalette, feineren Boden- und Fassadentexturen,
  detaillierteren Baumkronen, animierten Wasserreflexen, nassen Lichtspiegelungen und dezentem Nacht-Bloom.
- Begrenzte Bewegungseffekte: Reifenrauch, Staub, Gischt, Schneestaub, Sprung-/Landewolken und Schwimmwellen.
- Grafikleistung: halbe Rasterauflösung für Licht- und Schattenebenen bei niedriger Qualität; Hausschatten
  außerhalb des Bildausschnitts werden vor dem Pfadaufbau verworfen.
- Separate Entwicklungsseite `graphics-review.html`: sieben feste Szenen, Qualitäts- und Zoomvergleich,
  PNG-Export, Renderprofil und reproduzierbare Renderzeitmessung ohne Zugriff auf Spielstände.
- Spieler kann zu Fuß mit Leertaste springen, Zäune und niedrige Ufer überwinden sowie auf Karten-Wasserflächen
  schwimmen. Bewegung im Wasser ist verlangsamt; Schwimmer kommen über Uferkanten zurück an Land.
- Enter-Befehl `tag` mit Alias `wochentag`/`day`: Wochentag anzeigen oder setzen, deutsche Namen,
  Zweibuchstaben-Kürzel, 1–7 (Montag–Sonntag), `Sonnabend`, Autovervollständigung und direkte Eingabe wie `Freitag`.
- Fahrzeug-Klangprofile in `enginevoice.js`: Zylinder-/Zündimpulse, Boxer und V8-Bänke, Diesel, Zweitakter,
  Elektroantriebe; Ansaugung, Turbo, Lastwechsel und Innenraumdämmung.
- Wiederholbare Messung aller 45 motorisierten Modelle über `tools/benchmark-vehicles.mjs` und Regressionstests
  für Beschleunigung, Kraftverlauf, Wetter, Lenken beim Start, Zeitschritte und Höchsttempo.
- Lebensbalken über verletzten, noch lebenden NPCs.
- **M** schaltet den gesamten Ton an/aus; Stadtplan auf **Tab**.

### Geändert

- Enter in der Befehlszeile übernimmt zunächst die sichtbare Autovervollständigung; ein weiteres Enter führt sie aus.
  Die Fahrzeugschadenswerte wurden halbiert, sodass Zusammenstöße und umgefahrene Poller doppelt so viel aushalten.
- Der Befehl `leben` repariert jetzt auch das eigene Fahrzeug vollständig und setzt ein Wrack wieder fahrbereit.
- Dächer werden in einem begrenzten Rastercache gehalten; außerhalb des Bildes liegende Hauskörper werden
  früher verworfen, Masken verdeckter Figuren bündeln Wandflächen. Niedrige Qualität reduziert zusätzlich
  Regen, Schneefall, Partikel und fremde Silhouetten; Nacht-Bloom und zusätzliche Spiegelungen entfallen.
- Bahnhöfe erhalten strukturierte Böden, Leuchtbänder und Säulenschatten; Tunnel detailliertere Gleise und Lichtinseln.
- Schwimmpose mit sichtbaren Arm-/Beinschlägen; beim Springen bleiben Markierung und Schatten am Boden.
- Stadt-Antritt insbesondere für Front-/Hecktriebler kräftiger; Bonus läuft bis 90 km/h aus. Oberhalb 50 km/h
  beginnt die Anpassung auf mittlere Radleistung. Bestehende Starttraktion extremer Sport-/Allradfahrzeuge wird
  nicht zusätzlich erhöht. Höchstgeschwindigkeiten bleiben erhalten; Wetter, ESP und Drift werden berücksichtigt.
- Fahrzeugzeichnung über `vehicleart.js`: mehr Karosseriedetails, differenzierte Silhouetten und Oberflächen,
  vierfache Sprite-Auflösung, überarbeitete Sonderfahrzeuge und Wracks.
- Motorklang dunkler: breitere Impulse, tieferer Grundton und Bassanhebung, weniger Verzerrung,
  dezentere Ansaug-/Turbohöhen; dieselben Modellprofile im Verkehr.
- Mausbelegung: links Bewegung, rechts Angriff, im Steuerungsmenü mit ↓ vertauschbar und lokal gespeichert.
  Fahrzeuge und unterirdische Bahnhofseingänge werden per F betreten; Waffenrad über beide Maustasten gemeinsam.
- NPCs bleiben bei nicht tödlichem Schaden stehen bzw. fliehen oder kämpfen. Fahrzeugtreffer verursachen
  geschwindigkeitsabhängigen Schaden mit 1,1 s Schutz vor wiederholten Kontakt-Treffern.
- Poller und Hütchen geben ab etwa 3 km/h Aufpralltempo nach; geringerer Tempo- und Gesundheitsverlust.
- Kompakter Fahrzeugstatus mit sichtbaren ESP-/ABS-Anzeigen.

### Behoben

- Treffer in der Luft frieren die Sprungbewegung nicht mehr ein. Die Schwimmpose beginnt erst beim Wasserkontakt;
  Sprung-/Schwimmzustand wird beim Fahrzeugeinstieg und Wiedererscheinen zurückgesetzt.
- Tunnellichter werden nach dem Tunnelkörper gezeichnet und dadurch nicht mehr übermalt.
- Zaunkollisionssegmente werden beim Laden zusätzlich sichtbar gezeichnet; abgeleitete Abschnitte sollen
  nicht mehr nur als unsichtbare Barriere existieren. Gilt für alle geladenen Kartenkacheln.
- Gangpendeln in der Klang-/Drehzahllogik reduziert; Reifenquietschen mit Fahrdynamik folgt tatsächlichem
  Schlupf statt allein dem Bremspedal.
- Soundtest legt das Pkw-Modell explizit fest statt zufällig einen Diesel als Benziner zu erwarten.

### Dokumentation und Prüfstatus

- [Grafikbericht](docs/GRAFIK-UPDATE-2026-10-01.md) mit Vorher-/Nachher-Bildern, Speicherbudgets,
  gezielten Prüfungen und gemessenen Renderzeiten. Das 60-FPS-Ziel ist noch nicht in allen Szenen erreicht.
- README-Steuerung, Wochentag, Wettertafel, Fahrzeugdarstellung, Klang und Tests aktualisiert.
- [Updatebericht](docs/SPIEL-UPDATE-2026-10-01.md) und
  [Beschleunigungsbericht mit Herstellerquellen und 45 Messreihen](docs/FAHRZEUG-BESCHLEUNIGUNG.md).
- 33 gezielte Tests bestanden; vollständiger vorheriger Lauf 444/461 bestanden. Die 17 Fehler lassen sich
  auch mit der alten Fahrphysik reproduzieren. Keine Behauptung einer vollständig grünen Suite.
- Paketversion bleibt 0.46.0; dieser Abschnitt beschreibt noch keinen eigenständigen Versionsrelease.

## [0.46.0] – 2026-09-30

### Geändert
- **Schönere, realistischere Menschen:** Spieler und Passanten neu gezeichnet – Schultern statt Ellipse (Anzug eckig,
  Mantel länger), Arme mit Ellbogen und Händen (kurze Ärmel zeigen die Unterarme), Schuhe mit Sohle, Kopf mit Ohren,
  Nasenspitze, Haaransatz und Strähnen, Glatze mit Haarkranz, Iro mit rasierten Seiten, Locken, Zopf, Dutt, lange Haare
  auf den Schultern. Kleidung mit Kragen, Nähten, Revers und Krawatte, Zweireiher-Knöpfen, Kapuze auf dem Rücken,
  Reflexstreifen, Rucksack mit Gurten; Mützen mit Strickrippen und Bommel, Caps mit Nähten und Schirm, Hut mit Band,
  Strohhut, Bauhelm mit Grat, Kopftuch mit Falten, Kopfhörer. Volumen-Schattierung und Sonnenlicht auf der Schulter.
  Liegende zeigen ihr Gesicht (Augen geschlossen), die Arme liegen neben dem Körper.
- Der Spieler ist an einem Ring am Boden zu erkennen statt an einem Umriss auf dem Körper.
- Rumpf und Kopf werden je Aussehen einmal als Bild gerechnet (geteilter Speicher, älteste zuerst verworfen); bei
  niedriger Qualitätsstufe ohne Konturen an Armen und Beinen.

### Behoben
- Rucksack wurde doppelt als Fläche gezeichnet (`roundRect` und `rect` zugleich).

## [0.45.0] – 2026-09-30

### Neu
- **19 neue Pkw nach echten Vorbildern** (Namen erfunden, keine Marken), jeder mit eigener Technik, eigenem Umriss und
  Motorklang: Supersportwagen (V10-Mittelmotor, Allrad, 640 PS), GT-Coupé mit Front-Mittelmotor, leichter
  Mittelmotor-Zweisitzer, Elektro-Sportlimousine (760 PS), Sportcoupé, leichtes Drift-Coupé, Business-, Sport- und
  Luxuslimousine, Familien- und Power-Kombi (600 PS), Großraumtransporter, Hochdachkombi, kantiger V8-Geländewagen,
  Expeditions-Geländewagen, kleiner Geländewagen ohne Fahrhilfen, Kompakt-, Sport- und großes SUV. Jetzt 37 Pkw-Modelle.
- **Fahrzeugnamen:** Jedes Modell hat einen Namen („Oberbaum Furia“, „Spree Ronda“, „Tegel Blitz 1000“ …). Beim
  Einsteigen blendet das HUD Name und Technik ein – ohne Kasten, Schrift mit Kontur, blendet nach ≈ 4 s aus.
- **Drehzahlmesser:** runder Tacho mit Drehzahlbogen (Striche je 1000 U/min, roter Bereich), km/h in der Mitte, Gang
  (R rückwärts, D beim Elektroauto), Schaden als dünner Bogen.

### Geändert
- **HUD neu gestaltet:** kleiner und ohne dunkle Kästen – Ort, Geld, Uhr, Auftrag, Waffe und Hinweise als Schrift mit
  schwarzer Kontur (schmale Schrift: Bahnschrift auf Xbox/Windows, DIN auf dem Mac), Minikarte kleiner mit Lebensleiste
  darunter, Waffenanzeige mit Symbol und Magazin, Hinweise auf weichem Schleier.
- Leistung in **PS** statt kW, Antrieb als **FWD/RWD/AWD**.

### Behoben
- **Reifenspuren im Schnee:** lagen eine Wagenlänge auseinander vor und hinter dem Auto (halbe Länge und halbe Breite
  vertauscht). Jetzt vier Spuren genau unter den Rädern – in Kurven und beim Driften laufen Vorder- und Hinterräder
  auseinander –, Zweiräder ziehen eine Spur, auf Brücken keine Spur am Boden; die Rille hat einen dunkleren Kern.

## [0.44.0] – 2026-09-30

### Geändert
- **Schneller:** Die Simulation braucht pro Schritt nur noch die Hälfte der Zeit (Stadtverkehr 2,6 → 1,4 ms) –
  bitgleiche Ergebnisse: Nachbarschaftsraster für Passant↔Auto, Auto↔Auto und die Hindernissuche der KI, Zebrastreifen
  je Zelle gemerkt, Takt der Fahrpläne je Spielminute gemerkt, Positionen auf Linienwegen per binärer Suche, grobe
  Vorabprüfung bei Kreis↔Auto-Kollisionen.
- **Weniger Zeichenbefehle:** nachts halb so viele (erleuchtete Fenster als fertige Pfade je Spielminute), Fassaden mit
  einem Verlauf statt drei Flächen und einem statt zwei Rahmenwechseln, Minikarte aus einem selten neu gezeichneten
  Vorrat (730 → 170 Befehle für das HUD).

### Neu
- **Dynamische Auflösung:** Fällt die Bildrate unter ≈ 48 Bilder/s (z. B. wenn die Grafikkarte der Engpass ist), rendert
  das Spiel intern mit 85 % bzw. 70 % der Auflösung und schaltet wieder hoch, wenn Luft ist. Befehl
  `aufloesung auto|100|85|70`; die FPS-Anzeige (`fps an`) zeigt die Stufe.

## [0.43.0] – 2026-09-30

### Neu
- **Motorrad und Motorroller:** im Verkehr (tagsüber, am Wochenende mehr) und mit `auto motorrad` / `auto roller`.
  Echte Zweirad-Physik: zu viel Gas hebt das Vorderrad (Wheelie-Grenze begrenzt den Anzug), zu hartes Bremsen das
  Hinterrad (Stoppie-Grenze) – deshalb bremst ein Motorrad länger als ein Auto. In Kurven legt sich der Fahrer hinein.
  Ein Aufprall ab ≈ 27 km/h wirft ihn ab: benommen, verletzt, die Maschine liegt; aufsteigen richtet sie wieder auf.
  Offen wie ein Rad (Stadt ungedämpft hörbar, keine Kisten, nie Missionsauto), Kamera näher, eigener Motorklang
  (kreischender Vierzylinder, Einzylinder-Roller).
- **Sieben neue Pkw:** Hot Hatch (starker Fronttriebler), Roadster (offen, leicht, 50:50), Muscle-Car (V8, schwere Nase,
  keine Fahrhilfen, schwächere Bremsen – quertreibt am Gas), Oldtimer (Trommelbremsen, Diagonalreifen, kein ESP),
  Pick-up (leere Ladefläche → wenig Traktion hinten), Kleinbus mit Heckmotor, Rallye-Kompakt (Allrad) – jeweils mit
  eigener Zeichnung und eigenem Motorklang.

## [0.42.0] – 2026-09-30

### Neu
- **U-Bahnhöfe leichter betreten:** Zusätzlich zu den Treppen über den Bahnsteigenden gibt es einen Eingang genau am
  U-/S-Symbol der Station (dort, wo man ihn sucht). In der Nähe eines Eingangs zeigt ein Hinweis „E: Hinunter zur
  U-Bahn …“ – E führt hinunter, hineinlaufen geht weiter auch. Die Minikarte zeigt die Eingänge.

### Behoben
- Oberirdische S-Bahnhöfe (z. B. Ostkreuz, Plänterwald) galten als unterirdisch und hatten Treppen in den Boden; jetzt
  muss der ganze Bahnsteig ohne sichtbares Gleis sein.
- Welche Bahnsteige ein Bahnhof hatte, hing davon ab, von wo man zuerst kam (z. B. fehlte die U7 an der Yorckstraße,
  wenn man von der S-Bahn kam).
- „Mission neu starten“ im U-Bahnhof setzte einen wieder auf den Bahnsteig; ein Teleport auf einen Eingang ließ einen
  sofort hinunterfallen.
- Straßenbahnen hielten für die Figur unten im Bahnhof, Passanten warteten auf sie, Schüsse unten erschreckten Passanten
  oben und riefen die Polizei.
- Abfahrtstafel: Ankunftszeiten passen jetzt genau zu den einfahrenden Zügen.
- Fahrdynamik: Gas beim Rückwärtsrollen bremst jetzt ab, Wiese/Wasser/Pflaster begrenzen das Tempo wieder, nach dem
  Aussteigen bleibt das Auto nicht schief stehen, Aquaplaning giert wie beim übrigen Verkehr.
- Der Spielstand merkt sich das Automodell (und damit das Fahrverhalten) des eigenen Autos.
- Grafik: Ölbänder enden vor Kreuzungen, Kontaktschatten liegen nicht mehr auf Brücken, Schmutz nicht auf dem Wasser;
  bei niedriger Zeichenqualität entfallen die teuren Zusatzebenen. Tacho: Modellname überlappt nicht mehr.

## [0.41.0] – 2026-09-30

### Neu
- **Sechs neue Pkw-Modelle:** Kompaktwagen, Elektro-SUV, Geländewagen, Sportwagen (Mittelmotor), Heckmotor-Coupé und
  Zweitakter – mit eigener Zeichnung (Lüftungsschlitze, Panoramadach, Reserverad, zweifarbiges Dach …) und eigenem
  Motorklang (Zweitakter knattert, Elektro summt, Achtzylinder dreht hoch). `auto <modell>` stellt eines bereit.
- **Echte Fahrdynamik für das eigene Auto:** Antrieb (Front/Heck/Allrad), Motorlage, Gewichtsverteilung,
  Schwerpunkthöhe, Leistung und Reifen je Modell; Lastverschiebung, Kammscher Kreis, Reifenkennlinie. Heckantrieb
  übersteuert unter Last, Frontantrieb untersteuert, Allrad zieht heraus, Mittelmotor ist agil, hoher Schwerpunkt wankt.
- **Spielspaß vor Simulation:** mehr Grip, kräftige Bremsen, etwas mehr Anzug, enger Wendekreis, ASR/ESP fängt
  Ausbrecher ab (Leuchte im Tacho, `esp aus` für rohes Fahren), Handbremse für kontrollierte Drifts.
- Karosserie nickt und wankt sichtbar mit dem Schwerpunkt; Tacho zeigt Modell und Antrieb; Einsteigen nennt die Technik.

## [0.40.0] – 2026-09-30

### Geändert
- **Realistischere Grafik:** Großräumige Gebrauchsspuren über Boden, Grün und Straßen (Schmutzflecken und ausgeblichene
  Stellen in zwei Maßstäben, ohne sichtbare Kachel; Schnee deckt sie zu), dunkles Ölband in der Mitte jedes
  Fahrstreifens wie auf Luftbildern, weicher Kontaktschatten am Fuß jedes Hauses, Straßenschmutz am Fassadensockel,
  Moos und Ruß auf Dächern, wandernde Lichtreflexe auf dem Wasser, Plätze mit Granitplatten im Läuferverband statt
  Schachbrett, leichte Vignette.

## [0.39.0] – 2026-09-30

### Neu
- **U-Bahnhöfe betreten:** Unterirdische U- und S-Bahnhöfe haben Treppenabgänge mit U-/S-Schild am Gehweg. Hinunter
  wechselt die Ansicht vom Stadtplan in den Bahnhof: Bahnsteig, Treppen, Säulen, geflieste Wände, Namensschilder,
  Fahrgastanzeigen, wartende Fahrgäste und Züge nach dem echten Fahrplan. G an der Bahnsteigkante steigt in einen
  haltenden Zug, G am nächsten unterirdischen Halt steigt auf dessen Bahnsteig aus; die Treppen an beiden Enden führen
  zu ihrem Ausgang. Unten gedämpfter Klang, keine Ziele von der Straße; Spielstände legen die Figur an den Ausgang.

## [0.38.0] – 2026-09-30

### Neu
- **Radfahrer abschießen und umhauen:** Schüsse, Schläge und Tritte holen Radfahrer und E-Roller-Fahrer vom Rad; sie
  stürzen, nehmen den Treffer (können daran sterben), das Rad bleibt liegen. Zielhilfe und Maus-Klick erfassen sie.
- **Räder kapern:** F/Y direkt neben einem Rad (oder Doppelklick) zieht den Fahrer herunter – er flieht – und man fährt
  selbst: Fahrrad ≈ 26 km/h, E-Roller ≈ 20 km/h, ohne Motorgeräusch und ohne gedämpfte Umgebung, Kamera nah. Liegende
  Räder aufheben, absteigen lässt das Rad stehen. Kisten passen nicht aufs Rad.
- Statistik: „Räder gekapert“ und „Radfahrer vom Rad geholt“; Befehl `auto fahrrad` / `auto e-roller` (deutsche Namen
  für alle Fahrzeugarten).

## [0.37.0] – 2026-09-29

### Geändert
- **Ruhigere Maussteuerung (Diablo):** Kein pulsierender Ring mehr am Laufziel, nur ein kurzer Ring beim Klick. Zeiger
  und Umriss wechseln nur noch, wenn ein Klick mehr als laufen täte (Person angreifen, Auto daneben einsteigen), und
  erst, wenn der Zeiger kurz darauf ruht – kein Flackern mehr beim Überstreichen geparkter Autos.
- **Einsteigen mit der Maus:** Ein Klick auf ein entferntes Auto läuft nur hin; eingestiegen wird mit einem weiteren
  Klick, per Doppelklick oder F. An der Tür hält die Figur kurz an (Tür geht auf), statt sofort im Auto zu sitzen.
- **Befehlszeile wie eine Befehlspalette:** Uhrzeit, Wetter und Orte ohne Befehlswort (`22:30`, `regen`,
  `alexanderplatz`), Tippfehler werden verziehen, Enter nimmt bei unvollständiger Eingabe den besten Vorschlag, nach
  Erfolg schließt sie (Umschalt+Enter lässt sie offen), Hilfezeile zum getippten Befehl, zuletzt benutzte Befehle
  oben, Vorschläge per Maus wählbar, Esc leert zuerst, Strg/Alt+Rücktaste löscht ein Wort.

### Behoben
- Der Hinweis „Einsteigen“ lag über der offenen Befehlszeile.

## [0.36.0] – 2026-09-29

### Geändert
- **Realistischere Wetterdarstellung:**
  - Regen: Tropfen mit hellem Kopf und blassem Schweif, Aufschlagringe am Boden, Regenringe in den Pfützen,
    Regenwände aus Rauschen, die mit dem Wind ziehen (statt gerader Bänder).
  - Nässe: Asphalt deutlich dunkler mit mattem Himmelsglanz, auch Gehwege und Höfe dunkler; Pfützen mit nassem Rand
    und Himmelsspiegelung, die zur Mitte heller wird.
  - Wolkenschatten: ausgefranste Wolkenformen verschiedener Größe statt runder Flecken, mehr Wolken = mehr Schatten.
  - Nebel: Bodennebel auf Straßen und Höfen mit ziehenden Schwaden, die Dächer ragen heraus; darüber leichterer Dunst.
  - Schnee: nahe Flocken weich und unscharf; im Schneesturm fegen Schneeschleier quer durchs Bild.

### Behoben
- Bei Regen lagen dunkle Kreise auf den Kreuzungen (die Nässe wurde dort doppelt aufgetragen).

## [0.35.0] – 2026-09-29

### Geändert
- **Diablo-Steuerung überarbeitet:** Der Mauszeiger zeigt vorher, was ein Klick tut – Pfeil (hinlaufen), rotes
  Fadenkreuz über Personen (angreifen), Auto-Symbol über Autos (hinlaufen und einsteigen). Das Ziel unter dem Zeiger
  wird umkreist bzw. umrandet, das Laufziel am Boden markiert.
- **Weniger ungewollte Schüsse:** Ein Klick auf ein Auto schießt nicht mehr, sondern steigt ein (angreifen nur mit
  Strg + Klick). Was ein Klick bedeutet, entscheidet der Moment des Drückens: Wer auf den Boden klickt und hält, läuft
  dem Zeiger nach und greift nichts an, was er dabei überstreicht; ein Angriff bleibt bei der angeklickten Person und
  springt danach nicht auf die nächste über. Das Fadenkreuz vor der Figur erscheint nur noch beim Strg-Angriff.
- **Waffenrad mit der Maus:** öffnet direkt am Mauszeiger; gewählt ist die Waffe, auf die der Zeiger zeigt (ein kleiner
  Ruck reicht, weit draußen zählt die Richtung), statt einer aufsummierten Mausbewegung. Linksklick ins Rad nimmt die
  Waffe sofort. Am Bildrand rückt das Rad so weit herein, dass es ganz sichtbar ist.

## [0.34.1] – 2026-09-29

### Geändert
- **Nachtleben aus gostumblr:** Das Spiel versteht jetzt genau die Antwort von gostumblr
  (`https://app.gostumblr.com/api/v1/bars/busyness`: Live-Auslastung, „üblich“, 24-h-Verlauf je Bar) und den
  Wochenschnitt aller Bars; daraus wird je Bar ein Stundenprofil für die ganze Woche in Berliner Zeit. `npm start` holt
  gostumblr jetzt von selbst (alle 2 min, `BARS_URL=aus` schaltet ab), `npm run bars:fetch` ohne Angabe ebenso.
- Der Bar-Schnappschuss `web/data/bars.json` liegt nicht mehr im Git (wird mit `bars:fetch` erzeugt).

## [0.34.0] – 2026-09-29

### Neu
- **Nachtleben hörbar:** Vor Bars, Kneipen, Biergärten und Clubs hört man abends Stimmengewirr, Lachen, Gläserklirren
  und gedämpften Bass – je nach Uhrzeit und Wochentag (Freitag-/Samstagnacht am meisten, Clubs erst ab 23 Uhr), aus
  der Richtung des Lokals; bei Regen und Schnee gehen die Leute rein und es wird draußen leiser.
- **Bar-Auslastung vom eigenen Server (gostumblr):** Das Spiel liest einen Auslastungs-Feed (Wochenprofil je Stunde
  und/oder aktuelle Auslastung). Vor den dort genannten Bars ist das Nachtleben am deutlichsten, mit mehr Rauchern auf
  dem Gehweg und Schlangen vor Clubs; auch Bars, die OpenStreetMap nicht kennt, sind an ihrer Koordinate zu hören.
  Quelle: `?bars=URL`, Befehl `bars URL`, `BARS_URL=… npm start` (Dev-Server reicht den Feed durch) oder der
  Schnappschuss `npm run bars:fetch`. Ohne Feed klingt das Nachtleben nach den OSM-Lokalen.
- **Realistischer Fahrzeugklang:** Motor mit Drehzahl und Gängen je Fahrzeugart (Benziner, Diesel mit Nageln, Lkw und
  Bus tief), Zündfrequenz, Auspuff im Takt, Schaltpausen; Reifen rollen, rumpeln auf Kopfsteinpflaster, zischen auf
  nasser Straße, knirschen im Schnee, quietschen beim Rutschen (auf Schnee und Nässe rauscht es nur); Fahrtwind.
- **Fremde Autos einzeln hörbar:** die vier nächsten mit Richtung und Dopplereffekt beim Vorbeifahren.
- **Wetter klingt echter:** einzelne Regentropfen, Regen trommelt aufs Autodach, Wind pfeift in Böen, Schnee dämpft
  die Höhen; im Auto klingt alles draußen dumpf.
- **Schritte:** zu Fuß hört man jeden Schritt – knirschend im Schnee, platschend bei Nässe, dumpf im Gras.

### Geändert
- Umgefahrene Poller scheppern metallisch, Hupe zweistimmig, Unfälle mit Blechknautschen und Glassplittern.
- Ein Kompressor vor dem Ausgang verhindert Übersteuern, wenn viel zugleich klingt.

## [0.33.0] – 2026-09-29

### Neu
- **Alle Poller lassen sich umfahren:** Pollerreihen, die in OpenStreetMap als Linie eingetragen sind, waren bisher eine
  feste Wand; jetzt stehen dort einzelne Poller (höchstens 1,5 m auseinander), die man mit Schwung umfährt. Fußgänger
  kommen weiter durch, der KI-Verkehr meidet gesperrte Straßen wie bisher.
- **Reifenspuren im Schnee:** Alle Autos ziehen zwei Spuren durch den Schnee; sie verblassen nach einigen Minuten, bei
  Schneefall schneller.

### Behoben
- Schornsteine auf Satteldächern konnten an Einbuchtungen des Grundrisses aus dem Haus ragen.

## [0.32.1] – 2026-09-29

### Geändert
- Diablo-Steuerung: Umschalt sprintet (statt Leertaste), rechte Maustaste halten öffnet das Waffenrad (statt Tab),
  rechte Maustaste tippen tritt; am Platz angreifen jetzt mit Strg (statt Umschalt + Klick).

## [0.32.0] – 2026-09-29

### Neu
- **Neue Standard-Steuerung zu Fuß am PC (nach Diablo 3):** Klick auf den Boden läuft (halten = folgen, Wegfindung),
  Klick auf Person/Auto greift an, Umschalt + Klick greift auf der Stelle an, rechte Maustaste tritt, Tab = Waffenrad,
  Mausrad zoomt, Leertaste sprintet, Alt geht. Die bisherige Steuerung bleibt als „Klassisch“ wählbar (Menü Steuerung).
- **Näherer Zoom zu Fuß** (2,0, per Mausrad 1,5–2,6) mit Detailstufe: Berliner Gehwege, Granit-Bordsteine, Baumscheiben.
- **Realistische Geschwindigkeiten:** Joggen 3,5 m/s, Gehen 1,5 m/s, Sprint 7 m/s mit Ausdauer; Passanten 1,3 m/s.

### Geändert
- Maus-Zielhilfe rastet nur noch ein, wenn der Zeiger auf dem Ziel liegt; Streuung wächst mit der Bewegung.

### Behoben
- Fliehende Passanten liefen in Häuser.
- Straßenbahnen warteten ewig auf ein Auto, das nicht zurücksetzen konnte; nach 20 s fahren sie vorsichtig vorbei.
- Die Erkennung einer entgegenkommenden Bahn für Autos prüft jetzt denselben Bereich wie die Bahn selbst.

## [0.31.0] – 2026-09-29

### Neu
- **Straßenbahnen haben Vorrang:** Wo der Linienweg einer Bahn in der Gegenspur liegt, setzen entgegenkommende Autos
  und Busse zurück, statt ewig mit der Bahn aufeinander zu warten (betraf vor allem Linie 21, M10, M13).
- Unter Hochbahn-Viadukten bleibt die Straße bei Regen und Frost trocken.

### Behoben
- Wenden am Endhalt öffnete zugleich die Türen am neuen ersten Halt.
- Erneutes Türöffnen am selben Halt zählte als weiterer bedienter Halt.
- Übernahm man einen zweiten Zug, verschwand der erste im Bild; jetzt fährt er als Fahrplanzug weiter.
- Der Sprung an die letzte Haltestelle (K. o., Notausstieg) zählte als Fußweg.
- Pfützen am Rand des geladenen Gebiets wurden mit unvollständigen Häusern zwischengespeichert.
- Aquaplaning auch mit der Fahrzeugmitte in einer Pfütze (nicht nur mit den Vorderrädern).
- Der Zielpfeil lag unten rechts im Warnschild; das Warnschild blieb im Layout stehen, wenn es verschwand.
- Die Temperatur zeigte bis zum ersten Schritt 0 °C.
- Weniger Rechenaufwand bei trockenem Wetter ohne Sturm.

## [0.30.0] – 2026-09-29

### Neu
- **Wetter wirkt aufs Fahren:** Nässe, Schnee und Glätte verlängern den Bremsweg (etwa +30 %, doppelt, dreifach),
  lassen die Räder durchdrehen und das Heck rutschen, schwächen die Lenkung.
- **Temperatur** (neben der Uhr) und **Glätte:** Nässe bei Frost friert über, Brücken zuerst; unter Brücken und in
  Durchfahrten bleibt es trocken.
- **Aquaplaning** an den sichtbaren Pfützen über etwa 70 km/h, mit Spritzwasser und Platschen.
- **Sturmböen** versetzen Autos, auf Brücken stärker, leichte Autos mehr.
- **KI-Verkehr** fährt bei schlechtem Wetter langsamer, bremst sanfter, hält in Fahrt mehr Abstand.
- **Eigener Zug:** schlechtere Haftung auf nassen oder vereisten Schienen (im Tunnel trocken).
- Warnschild über dem Tacho, Konsole `glaette` und `temp`, Statistik „Aquaplaning“.

## [0.29.0] – 2026-09-29

### Neu
- **Mitfahren im Nahverkehr:** neben einem Bus, einer Straßenbahn, S- oder U-Bahn **G** (Controller: Steuerkreuz
  unten) steigt ein – auch in Fahrt (Aufspringen ab 25 km/h); **G** noch einmal steigt aus, schnell ist das Abspringen
  mit Sturz (ab 40 km/h mit Schaden). Unter Tage nur am Bahnsteig, Ausgang an der Straße. Leiste oben mittig mit Linie,
  Ziel und nächstem Halt.
- **Straßenbahn, S- und U-Bahn selbst fahren:** vorn am Führerstand **Y/F** übernimmt den Zug. Gas/Bremse/Notbremse,
  Türen an der Haltestelle mit **E/A**, Trinkgeld bis 10 € für einen sanften, genauen Halt, Wenden am Endhalt. Vor
  Zügen auf derselben Strecke und vor Hindernissen auf dem Straßenbahngleis bremst der Zug selbst, Fahrplanzüge
  dahinter warten.
- **Tunnelansicht:** unter Tage wird die Stadt abgedunkelt; Röhren, Bahnsteige mit Bahnhofsnamen und die Züge darin
  werden gezeichnet.
- Statistik-Abschnitt **Nahverkehr** (Mitfahrten, Strecke als Fahrgast und Zugführer, geführte Bahnen, bediente Halte,
  Trinkgeld, Auf-/Abspringen); die Statistikseite hat dafür drei Spalten. Töne für Ein-/Aussteigen, Türgong,
  Trinkgeld und „Zug voraus“.

### Behoben
- Parkende Autos standen auf Straßenbahngleisen (die Gleise kennt nur der Fahrplan, nicht die Karte) und hielten
  Straßenbahnen dauerhaft auf.
- Ein Test zum Nachladen fehlgeschlagener Kacheln schlug im vollen Testlauf gelegentlich fehl (er hing an der echten
  Uhr); die Wiederholpause der Karte hat jetzt eine einsetzbare Uhr.

### Regeln
- Einsteigen nur auf gleicher Ebene: in einen Zug auf dem Viadukt (U1) von der Straße nur, während er im Bahnhof hält;
  von der Hochbahn aussteigen nur am Bahnhof, Ausgang an der Straße.

### Bekannt
- Wo der Linienweg einer Straßenbahn in der Gegenspur liegt (z. B. M10 an der Herzbergstraße), können sie und ein
  entgegenkommender Bus aufeinander warten.

## [0.28.0] – 2026-09-28

### Neu
- **Waffenrad am Controller:** LB halten öffnet das Rad, der rechte Stick wählt, Loslassen nimmt die Waffe, B bricht
  ab; LB kurz tippen wechselt wie bisher zur vorigen Waffe.
- Im offenen Rad: Mausrad dreht die Wahl weiter, 1–6 wählt sofort und schließt, Esc bricht ab (ohne Pausemenü).
- Das Rad zeigt, wohin Maus oder Stick zeigen (Zeiger in der Mitte), die Munition jeder Waffe, die Zifferntasten und
  einen Bedienhinweis; es blendet beim Öffnen ein, die Zeitlupe setzt weich ein und aus.

### Behoben
- Rechts halten beim Schießen (linke Taste gedrückt) öffnete kein Rad, das Loslassen ging verloren.
- Loslassen außerhalb des Fensters oder ein Fensterwechsel ließen das Rad hängen.
- Wer weit hinausgezogen hatte, musste den ganzen Weg zurück, bevor die Wahl wechselte (Zeiger jetzt auf den Radius
  begrenzt).
- Nach der Wahl riss die Figur herum zu der Stelle, an der der Mauszeiger beim Auswählen stand; das Ziel bleibt jetzt,
  bis die Maus wieder bewegt wird.

## [0.27.0] – 2026-09-28

### Neu
- **Verschiedene Menschen:** zwölf Typen (Alltag, Büro, Tourist, Senior, Jugendliche, Kiez, Handwerk, Punk, Kopftuch,
  Kinderwagen, Jogger, Gassigeher) mit eigener Kleidung, Frisur, Kopfbedeckung, Statur und Zubehör. Welche unterwegs
  sind, folgt aus Bezirk, Uhrzeit, Wochentag und Tätigkeit; der Typ bestimmt das Gehtempo (Senioren und Kinderwagen
  langsam, Büroleute zügig).
- **Gangbild** für Spieler und Passanten: Schritte und gegengleicher Armschwung im Takt des Tempos, Schulterdrehung,
  Wippen, Rennen mit Vorlage und langen Schritten, weiches Anlaufen/Anhalten, Atmen und Gewichtsverlagerung im Stand,
  geglättete Drehung. Der Spieler läuft in Laufrichtung und dreht den Oberkörper zum Ziel, entgegen der Zielrichtung
  geht er rückwärts.
- Neu gezeichnete Figur: Beine mit Schuhen (der gehobene Fuß größer), Ärmel und Hände, Rumpf je Oberteil (Jacke,
  Anzug mit Krawatte, Mantel, Hoodie, Warnweste, Sport), Kopf mit Frisur und Kopfbedeckung, Licht von der Sonnenseite,
  feine Kontur. Spieler in oranger Jacke mit weißem Rand.
- Prüfseite `web/lab/figures.html`: alle Typen vergrößert in allen Posen.

## [0.26.0] – 2026-09-28

### Neu
- **Befehlszeile** (Enter im Spiel): Uhrzeit, Spieluhr-Tempo, Wetter, Schnee, Nässe, Verkehrs- und Passantendichte,
  Teleport zu jedem Ort Berlins, Cheats (Geld, Gesundheit, Munition, Gottmodus, Fahrzeug, Reparatur) und Anzeigen zum
  Prüfen (Bildrate, Ebenen, Silhouetten, Zeichenqualität). Vorschläge beim Tippen mit grauer Ergänzung (Tab/→),
  ↑↓ wählt Vorschläge bzw. blättert im Verlauf; die Welt steht still, solange die Zeile offen ist.
- **Statistik** je Spiel und über alle Spiele, gespeichert in IndexedDB: Strecke (zu Fuß/im Auto), Höchstgeschwindigkeit,
  Spielzeit, Brücken, Teleports, überfahrene Menschen und Radfahrer, Unfälle, Poller, geklaute Autos, Tote (erschossen
  / Nahkampf), Schüsse, Kugeln, Treffer und Quote je Waffe, zerstörte Autos, Aufträge, Geld, Krankenhauskosten,
  Cheats. Menüpunkt „Statistik“ im Titel und in der Pause, Befehl `stats`.

### Geändert
- „Aktion“ liegt nur noch auf E (Enter öffnet die Befehlszeile); in Menüs bestätigt Enter weiter.
- **Baumschatten** neu: der Stamm wirft einen Streifen vom Fuß bis in die Krone (der Schatten hängt am Baum), die Krone
  wirft ihren eigenen lappigen Umriss statt einer Scheibe, mit weichem Rand und Lichtflecken je nach Baumart (Birke
  licht, Kastanie und Nadelbaum dicht), und wird bei tiefer Sonne entlang der Sonne gestreckt.

## [0.25.2] – 2026-09-28

### Behoben
- Gebäude aus Bauteilen (`building:part`) stimmen jetzt, z. B. am Neuen Kranzler Eck / Kurfürstendamm 231:
  - Obergeschosse über einer Arkade (`min_height`/`building:min_level`) wurden verworfen; übrig blieb ein 3 m hoher
    Sockel als flache Platte. Sie heben jetzt das Haus darunter auf ihre Höhe (446 Häuser); nur ohne etwas darunter
    (Überbauung einer Straße) entfallen sie weiter.
  - In verschachtelten Teilen blieb nur das höchste mit seinem eigenen Umriss – der Sockel darum fehlte. Jetzt bleibt
    der Sockel (außer im Wasser/auf dem Brückendeck: Pfeiler der Oberbaumbrücke), höhere Teile stehen darauf.
  - Teile im Umriss eines Gebäudes, die deutlich höher sind (Turm auf dem Block), wurden ignoriert; sie stehen jetzt
    als eigenes Gebäude darauf (3 260 in Berlin). Ein Haus in einem anderen wird nach diesem gezeichnet.
  - Die Art eines Bauteils kommt aus `building:part` (retail, commercial …): Geschäftshäuser bekommen keine
    Altbau-Dächer und -Fassaden mehr.

## [0.25.1] – 2026-09-28

### Behoben
- A 100: zwei Viadukte verschiedener Ebene nebeneinander wurden als „Gegenfahrbahnen mit Lücke“ gepaart – die Lücke
  zur Gegenfahrbahn wird nur noch zwischen Fahrbahnen derselben Ebene gefüllt. Damit gibt es auf keiner Brücke Berlins
  mehr eine Sperre (die letzte am Rand des A-100-Viadukts ist weg).
- Am Knoten zwischen zwei Brückenstücken (Biegung) verlor ein Auto am Innenrand kurz seine Fahrbahn und konnte auf die
  Rampe darunter wechseln (Heerstraße): Stücke der eigenen Ebene, die nicht am Portal enden, zählen jetzt auch dort.
- Brückenprüfung: falsche Ebene zählt ab 3 m Strecke (statt Anteil – auf 10-m-Stücken war ein Wechsel 1 m hinter dem
  Knoten schon ein Befund); in der Spur stimmt die Ebene überall. Übrig sind 4 Stellen am äußersten Rand, an denen die
  Gegenfahrbahn ihre Brücke erst später beginnt (dort ist man tatsächlich auf deren Bodenstück).

## [0.25.0] – 2026-09-28

### Hinzugefügt
- Rechte Maustaste: kurz tippen steigt ins Auto ein bzw. aus (wie F / Y); gedrückt halten öffnet zu Fuß das
  Waffenrad. Das Kontextmenü des Browsers ist abgeschaltet.
- Waffenrad: ein runder Kranz mit einem Segment je Waffe und eigenem Symbol (Faust, Schläger, Messer, Pistole,
  Maschinenpistole, Schrotflinte). Die Richtung der Maus ab der Stelle des Drucks wählt, das gezeigte Segment leuchtet
  gelb und wird größer, in der Mitte stehen Name und Munition; Loslassen nimmt die Waffe. In der Mitte (Totzone) bleibt
  die letzte Wahl stehen. Solange das Rad offen ist, läuft das Spiel in Zeitlupe und es wird nicht geschossen; steigt
  man ein oder wird umgehauen, schließt es.
- Das Waffenfeld unten rechts zeigt das Symbol der gewählten Waffe.

## [0.24.1] – 2026-09-28

### Behoben
- Brücken berlinweit befahrbar gemacht – mit einem neuen Prüfwerkzeug (`tools/check-bridges.mjs`), das jede der 895
  Brückenfahrbahnen in jeder Richtung über die ganze Breite abfährt, die Ebene wie im Spiel führt und jede Sperre
  meldet. Vorher: 57 Sperren, jetzt keine (bis auf eine Überlappung zweier A-100-Viadukte in OSM am äußersten Rand).
  Die Ursachen:
  - Elsenbrücke: ein Zaun unter der Brücke sperrte die Brücke darüber (Zäune sperren nur noch Straßen ihrer Ebene;
    53 Straßen waren so fälschlich gesperrt).
  - Am Rand der Brücke (Lücke zur Gegenfahrbahn, Radweg) lag man außerhalb des Portals, blieb unten und prallte an die
    Kaimauer darunter: Portale umfassen die ganze Breite, im Portal gilt die nächste Fläche mit etwas Spielraum.
  - Am Widerlager lagen Kaimauern, Gleisränder und Zäune quer über der Brückenbreite, geöffnet war nur die Breite der
    Straße dahinter (Lessingbrücke u. a.): an allen 1 605 Brückenenden jetzt über die ganze Brückenbreite offen.
  - Eine Straße, die unter dem Brückenkopf hindurchführt (A 100 unter dem Kaiserdamm) oder schräg abzweigt, hielt
    Autos unten; im Portal zählt die eigene Ebene nur noch auf der Straße, auf der man entlangfährt.
  - Das Portal einer tieferen Brücke zog Autos auf der höheren herunter (Gottlieb-Dunkel-Brücke über der A 100).
  - Mehrere Portale übereinander (Richtungsfahrbahnen) wurden nicht alle berücksichtigt; unplausible OSM-Breiten
    (30 m auf 7 m Länge an der Südostallee) zählen am Brückenkopf höchstens 10 m.
- Dachaufbauten werden exakt gegen den Grundriss geprüft (Kanten gegen das Rechteck) statt an 5 × 5 Stichpunkten –
  eine schmale Einbuchtung oder ein kleiner Hof zwischen den Punkten ließ eine Klimaanlage über die Dachkante ragen.

## [0.24.0] – 2026-09-28

### Hinzugefügt
- Physik nach Ebenen: Wer auf der Brücke fährt oder läuft, stößt nur mit dem zusammen, was auf der Brücke ist; wer
  darunter durchfährt, fährt unter Geländer und Brückenverkehr hindurch. Wände tragen ihre Ebene (Ufer am Boden,
  Gleisränder auf der Ebene des Gleises, Geländer auf der Brücke); Häuser und Bäume stehen am Boden. Autos, Menschen,
  Räder und die Spielfigur berühren sich nur auf derselben Ebene (am Rampenfuß beide), die KI bremst auf der Brücke
  nicht mehr für Verkehr darunter, Fußgänger unten warten nicht auf Autos oben, Schüsse gehen nicht durchs Deck.
- Untergrund nach Ebene: auf der Brücke zählt nur die Brücke (kein Wasser, kein Haus darunter), am Boden ist Wasser
  unter der Brücke Wasser.

### Geändert
- Kaimauern und Gleisränder laufen unter Brücken durch, Brückengeländer über den Straßen darunter – vorher mussten sie
  dort weggeschnitten werden, weil alles in einer Ebene lag (man konnte von der Brücke ins Wasser oder aufs Gleis
  darunter geraten).

## [0.23.0] – 2026-09-28

### Hinzugefügt
- Fenster gehen einzeln an: jedes Fenster hat seinen eigenen Zeitpunkt, abends gehen die Lichter nach und nach in
  zufälliger Folge an, Räume einer Wohnung kurz nacheinander, nachts macht hier und da jemand kurz Licht. Warmes,
  neutrales und kaltes Licht, gedimmt hinter dem Vorhang, flackernde Fernseher, Fensterkreuze; Büros, Schulen und
  Hallen abends hell und nachts dunkel; bei trübem Wetter auch tagsüber Licht. Im Nebel dringt Fensterlicht nur gedämpft
  durch. Vorher schaltete ein ganzes Haus in vier Stufen und das Muster wiederholte sich alle vier Fenster.
- Starkregen: dichter, in drei Tiefen, mit Gischtschleier und durchziehenden Regenwänden.
- Sturm: starker, böiger Wind; Bäume neigen sich und schwanken, Laub und Papier fegen durchs Bild, Regen treibt schräg,
  der Wind heult; weniger Menschen und Räder unterwegs.
- Gewitter (nachmittags an unbeständigen Tagen): Blitzstrahl mit Verästelungen und Einschlagslicht, Himmelsblitz mit
  Nachblitzen, der die Szene – nachts auch – kurz hell macht; Donner kommt nach Entfernung verzögert, nah als Knall
  mit Grollen, fern als dumpfes Rollen.
- Dichter Nebel mit ziehenden Schwaden und kaum Sicht.
- Schneefall und Schneesturm (an Wintertagen): Flocken in drei Tiefen, die im Wind treiben und taumeln, im Sturm als
  Striche, weißer Dunst. Schnee bleibt liegen, je nach Dauer und Stärke fleckig bis geschlossen: auf Gehwegen, Grün,
  Brückendecks, Dächern (Sonnenseite hell, Schattenseite bläulich), Baumkronen und geparkten Autos. Straßen bekommen
  Matsch mit festgefahrenen Reifenspuren je Fahrstreifen und Schneewälle am Bordstein, Hauptstraßen sind freier.
  Autos rutschen auf Schnee (bis 45 % weniger Seitenhalt), Schnee dämpft den Stadtlärm und hellt die Nacht auf,
  Tauwetter macht die Straßen nass. Gischt bzw. Schneestaub hinter schnellen Autos.
- `?wetter=starkregen|sturm|gewitter|dichternebel|schnee|schneesturm` und `?schneedecke=0…1` für die Sichtprüfung.

### Geändert
- Wetterlagen je Tag (gewöhnlich, unbeständig, Winter); Schneedecke und Nässe stehen im Spielstand.
- Unter geschlossener Wolkendecke keine einzelnen Wolkenschatten mehr.

## [0.22.0] – 2026-09-28

### Hinzugefügt
- Ebenen aus OpenStreetMap: OSM kennt keine Höhen für Wege, aber die Höhenordnung (`bridge`, `layer`, `tunnel`).
  Jede Straße, jeder Weg, jedes Gleis und jedes Brückendeck hat jetzt eine Ebene: Brücke ≥ 1, Boden 0, offene
  Unterführung oder Einschnitt negativ (`layer=-1` ohne Tunnel); echte Tunnel bleiben draußen, `bridge=no` zählt nicht
  als Brücke. Offene Unterführungen, die der Build bisher wegwarf, sind jetzt in der Karte (770 Straßenstücke), z. B.
  die Tamara-Danz-Straße unter der Rampe der Warschauer Brücke: sie hängt an der Helen-Ernst-Straße, nicht an der Brücke.
- Portale: Knoten, an denen Wege verschiedener Ebenen zusammentreffen (Rampenfuß, Treppe; 6 463 in Berlin). Nur dort
  wechselt ein Auto, eine Person, ein Rad oder die Spielfigur die Ebene – nach der Fläche, auf der es wirklich ist;
  wer unter oder über einer Brücke kreuzt, bleibt auf seiner Ebene.
- Zeichnen nach Ebenen: erst Unterführungen, dann Boden, dann Brücken. Was unter einer höheren Fläche liegt (Auto in
  der Unterführung, Passant unter der Brücke, Straßenbahn unter der Straßenbrücke), wird vor der Brücke gezeichnet
  und bekommt dort, wo sie es verdeckt, eine Silhouette. Straßenbahngleise auf Brücken liegen oben, Zäune, Poller und
  Stadtmöbel am Boden unter der Brücke.

### Geändert
- Die Brüstung einer Brücke endet am Rampenfuß gerade statt mit einem runden Bogen über der Straße darunter.

### Bekannt
- Kollisionen, Verkehr und Oberfläche wissen noch nichts von Ebenen (folgt in einer späteren Version); bis dahin bleiben die
  bisherigen Zuschnitte der Wände unter Brücken.

## [0.21.1] – 2026-09-28

### Behoben
- Unsichtbare Sperren auf Fahrflächen, berlinweit gesucht und entfernt: Brückengeländer standen mitten auf den neuen
  Radwegen (Oberbaumbrücke) und in Brückenlücken, Uferlinien und Gleisränder lagen unter Brücken auf der Fahrbahn,
  in Kreuzungsflächen, auf Radwegen und in Tordurchfahrten. Eine Suche über ganz Berlin fand vorher mehrere tausend
  Meter solcher Wände, jetzt keinen. Unsichtbare Wände (Ufer, Gleisränder, Geländer) werden im Build zum Schluss Meter
  für Meter gegen die befahrbare Fläche geprüft (Fahrbahn, Lücke zur Gegenfahrbahn, Radweg neben der Fahrbahn,
  Kreuzungsscheibe, auch Tordurchfahrten und gesperrte Straßen) und dort entfernt; Geländer stehen außerhalb von Radweg
  und Lücke. Sichtbare Zäune bleiben, wo sie wirklich stehen (Mittelzäune, Diagonalsperren, Zäune am Radweg).

## [0.21.0] – 2026-09-28

### Hinzugefügt
- Radwege neben der Fahrbahn (`cycleway=track`, z. B. auf der Oberbaumbrücke 3 m außen): rote Pflasterstreifen
  jenseits des Bordsteins, auch auf Brücken.

### Behoben
- Oberbaumbrücke: zwischen den beiden Richtungsfahrbahnen lag ein 4,5 m breiter Streifen Brückendeck, der wie ein
  Radweg in der Mitte aussah. Liegen die Richtungsfahrbahnen einer Brücke bis 7 m auseinander, ist die Lücke jetzt
  Fahrbahn; der Radweg liegt außen.
- Warschauer Brücke: Brückengeländer standen als unsichtbare Wände mitten auf der Gegenfahrbahn (jede Brückenfahrbahn
  hatte beidseitig ein Geländer, die Richtungsfahrbahnen liegen dort teils übereinander) und Geländer von Brückenwegen
  sperrten die Straße darunter. Geländer stehen jetzt nur noch am äußeren Rand der ganzen Brücke; Wege auf Brücken
  haben ebenfalls ein Geländer, damit niemand ins Wasser läuft.
- Gehwege auf Brücken wurden über die Fahrbahn gezeichnet (heller Streifen quer über der Warschauer Brücke). Jetzt:
  Straßen unten, darüber die Brückenwege, darüber die Brückenfahrbahnen.

## [0.20.1] – 2026-09-28

### Behoben
- Wegweiser nannten dasselbe Ziel in mehreren Zeilen (z. B. zweimal „Neukölln“). Jetzt steht jedes Ziel höchstens
  einmal auf einem Schild: die wichtigste Zeile bekommt es (echte OSM-Beschilderung, dann geradeaus, dann die
  flachere Abbiegung); eine leer gewordene Zeile nennt ihren Straßennamen, ist auch der vergeben, entfällt sie.

### Hinzugefügt
- Verdeckte Wegweiser (unter Baumkronen, hinter Häusern) bekommen eine Silhouette mit Beschriftung: im verdeckten Teil
  scheint die Tafel halbtransparent durch.

## [0.20.0] – 2026-09-28

### Hinzugefügt
- Wegweiser an großen Kreuzungen und Kreiseln (785 Kreuzungen, 1 689 Schilder in ganz Berlin): je Zufahrt ein
  gelbes Schild rechts etwa 35 m vor der Kreuzung, je Ausfahrt eine Zeile mit Pfeil (zeigt in die Kartenrichtung der
  Ausfahrt), Zielen und Bundesstraßennummer. Führt eine Ausfahrt im Ortsteil weiter, nennt eine weiße Zeile den
  Straßennamen. Nachts sind die Tafeln angestrahlt.
- Ziele: zuerst die echte Beschilderung aus OSM (Relationen `destination_sign` von–über–nach, `destination`-Tags in
  Fahrtrichtung – 965 Zeilen); sonst verfolgt der Karten-Build die Straße bis 4 km und nennt die ersten Ortsteile,
  in die sie führt, dazu „Zentrum“, wenn sie deutlich auf die Mitte zuführt. Am Kottbusser Tor etwa: Kottbusser Damm
  → Neukölln, Skalitzer Straße westwärts → Zentrum · Tiergarten.
- Richtungsfahrbahnen und alle Knoten eines Kreisels gelten als eine Kreuzung; Einbahnstraßen werden beachtet,
  Zeilen, die fast zurückführen, entfallen.

## [0.19.1] – 2026-09-28

### Geändert
- Silhouetten der anderen Fahrzeuge und Personen schwächer (Fläche 6 % statt 14 %, Kontur 32 % statt 60 %, dünner);
  die Spielfigur bleibt deutlich orange. Werte zentral in `render.js SILHOUETTE`.

## [0.19.0] – 2026-09-28

### Geändert
- Silhouetten für alle: Autos, Busse, Straßenbahnwagen, Radfahrer und Passanten bekommen einen Umriss, wenn eine
  Baumkrone, ein Haus (Fassade, Dach, Tordurchfahrt) oder die Hochbahn über ihnen liegt – die Spielfigur bzw. ihr Auto
  orange und pulsierend, alle anderen hell und zurückhaltend.
- Der Umriss erscheint genau im verdeckten Teil (Umriss ∩ Verdecker, auf einer kleinen Hilfsfläche verrechnet): ragt
  nur die Motorhaube unter einen Baum, ist nur die Motorhaube umrissen. Vorher zählte allein der Mittelpunkt.

## [0.18.0] – 2026-09-28

### Geändert
- Nord-Neukölln: Mietshäuser (12–26 m) ohne Dachform in OSM bekommen meist (85 %) ein durchgehendes Steildach über
  Vorderhaus und Flügeln statt des Berliner Dachs mit flacher Mitte; wo OSM die Dachform kennt, gilt die.
- Dachtiefe je Traufkante: ein Strahl von der Kante nach innen misst die Hausdicke dahinter, der First liegt in ihrer
  Mitte. Vorderhaus und Seitenflügel bekommen so je ihren eigenen First (vorher eine mittlere Tiefe fürs ganze Haus,
  die im Vorderhaus einen flachen Streifen ließ). An Ecken gilt der schmalere Flügel.

### Behoben
- An sehr spitzen Grundriss-Ecken konnte ein Dach-Eckpunkt außerhalb der Traufe landen (verdrehte Fläche).

## [0.17.1] – 2026-09-28

### Behoben
- Teleport auf offenen Grund (Parks, Plätze, Höfe, das Tempelhofer Feld) landete immer an der nächsten Straße – vom
  Tempelhofer Feld aus über einen Kilometer entfernt am Columbiadamm oder an der Ringbahnstraße. Jetzt landet man
  genau am gewählten Punkt bzw. an der nächsten freien Stelle bis 30 m, zu Fuß wie im Auto. Klicks auf Straßen und
  Häuser führen weiter zum nächsten Gehweg bzw. zur nächsten Fahrspur.

## [0.17.0] – 2026-09-28

### Hinzugefügt
- Poller und Schranken lassen sich mit dem Auto umfahren (ab etwa 16 km/h): sie fallen um, bremsen den Wagen etwas
  und bleiben liegen, auch wenn der Stadtteil neu geladen wird. Zu Fuß sind stehende Poller weiter ein Hindernis
  (zwischen ihnen passt man durch), über liegende läuft man drüber. Der KI-Verkehr meidet gesperrte Straßen weiter.
- Silhouette: Wird die Spielfigur oder ihr Auto von einer Baumkrone, einem Haus (Dach, Fassade, Tordurchfahrt) oder
  der Hochbahn bzw. einer Bahnbrücke verdeckt, zeichnet das Spiel ihren Umriss obendrauf (auch nachts).

### Behoben
- Eingezäunte Flächen waren mit dem Auto unerreichbar, darunter das ganze Tempelhofer Feld: Tore öffneten den Zaun
  nur 1,8 m breit, und Wege, die einen Zaun ohne Tor-Knoten kreuzen, bekamen gar keine Öffnung. Jetzt öffnen Tore
  und jede Querung eines Wegs oder einer Straße Zäune, Mauern, Hecken und Pollerlinien 4,4 m breit (56 000
  Öffnungen); gezeichnet wird der Zaun ebenso mit Lücke. Das Auto erreicht damit 93–98 % der Fläche, die man zu Fuß
  erreicht (nachgemessen per Flutfüllung in fünf Bezirken; der Rest sind Hofdurchgänge unter Autobreite).

## [0.16.0] – 2026-09-28

### Hinzugefügt
- Dächer mit echter Form: Sattel-, Walm-, Zelt-, Mansard-, Pult-, Tonnen- und Kuppeldach als einzelne Dachflächen,
  je nach Sonnenstand hell oder dunkel schattiert, mit Ziegelreihen, First- und Gratlinien, Gauben und Schornsteinen
  auf dem First. Das Berliner Dach ist ein geneigter Ziegel- oder Schieferstreifen zu Straße und Hof um eine flache
  Bitumenmitte (vorher ein farbiger Rand). Verwinkelte Grundrisse (Blockrand mit Hof, L-Formen) bekommen den Streifen
  entlang jeder Außen- und Hofkante mit Gehrung an den Ecken.
- Aussehen aus OpenStreetMap: Dachform (`roof:shape`, gut 100 000 Häuser), Dach- und Fassadenfarbe
  (`roof:colour`, `building:colour`), Dach- und Fassadenmaterial, Gebäudetyp (Villa, Reihenhaus, Mietshaus, Geschäft,
  öffentliches Gebäude, Garage) und der Bezirk kommen je Gebäude in die Kacheln (+3 MB).
- Mehr Abwechslung: realistische Dachfarben (Biberschwanz- und Pfannenziegel, Schiefer/Anthrazit, Betondachstein,
  Bitumen und Kies, Kupfer auf Kirchen und Kuppeln, Gründächer), Fassaden nach Material (Backstein, Beton, Glas, Holz,
  Naturstein), Stuckfarben im Altbau, helle Putzvillen, farbig sanierte Plattenbauten in Marzahn-Hellersdorf und
  Lichtenberg (dort ab fünf Geschossen Platte mit Flachdach), Villen und Einfamilienhäuser mit Walm- oder Satteldach.

### Geändert
- Flachdächer grau statt beige; Dachaufbauten auf dem Berliner Dach nur noch auf der flachen Mitte.

### Behoben
- Fehlende Versionsüberschrift von 0.14.0 im Changelog.

## [0.15.0] – 2026-09-28

### Hinzugefügt
- Öffentlicher Verkehr nach dem echten VBB-Fahrplan (GTFS, CC BY 3.0 „VBB Verkehrsverbund Berlin-Brandenburg GmbH“):
  Busse, Straßenbahnen, S- und U-Bahnen auf ihren wirklichen Linienwegen mit ihren Halten, Fahr- und Haltezeiten.
  Der Fahrplan bestimmt den Takt zur Spieluhr (werktags, samstags, sonntags, auch Nachtlinien nach Mitternacht);
  die Fahrzeuge selbst fahren in Echtzeit.
- Busse (12 m, gelb, Liniennummer auf dem Dach) werden nahe der Kamera zu echten Verkehrsteilnehmern: sie folgen
  ihrem Linienweg über den Spurgraph, halten an ihren Halten, Wartende steigen ein. Verlieren sie den Weg, geben
  sie die Linie ab und fahren außer Sicht davon.
- Straßenbahnen fahren auf Gleisen in der Fahrbahn (aus den Linienwegen gezeichnet), halten vor Hindernissen und
  klingeln; ihre Wagen sind für Autos und Spielfigur feste Hindernisse, der Verkehr wartet hinter ihnen.
- S- und U-Bahnen fahren auf ihren Gleisen, sichtbar nur, wo das Gleis oberirdisch liegt (Hochbahn, Bahndamm); im
  Tunnel rumpeln sie hörbar unter der Straße. Nachts leuchten die Wagen, Scheinwerfer vorn.
- Busspuren: Gegenbusspuren in Einbahnstraßen (`oneway:bus=no`) und reine Busstraßen sind nur für Busse; der
  übrige Verkehr nutzt sie nie.
- `npm run map:fetch -- --gtfs` lädt den Fahrplan, `npm run map:transit` baut daraus `web/data/berlin/transit.json`
  (3,3 MB, ohne die Karte neu zu bauen).

## [0.14.0] – 2026-09-28

### Hinzugefügt
- Wetter: sonnig, wolkig, bedeckt, Regen und (nur morgens) Nebel, je drei Stunden, mit 45 Minuten Übergang; fest aus
  Welt-Samen und Tagnummer, also für dieselbe Welt immer gleich. Das HUD zeigt das Wetter neben der Uhr.
  `?wetter=regen|nebel|sonnig|wolkig|bedeckt` legt es fest.
- Wolken nehmen der Sonne die Schatten, ziehen als große weiche Schatten mit dem Wind über Straßen und Dächer und
  machen die Szene grauer.
- Regen: fallende, windschiefe Tropfen mit Spritzern, Regenschleier, bei Regen gehen weniger Menschen raus und kaum
  jemand fährt Rad; viele tragen Schirme (je Person fest). Der Boden wird nass (schnell) und trocknet langsam: nasser
  Asphalt glänzt dunkel, an der Rinne stehen Pfützen, die tags den Himmel spiegeln; nachts spiegeln sich Laternen,
  Scheinwerfer, Ampeln und Schaufenster darin. Auf nasser Fahrbahn haben Autos weniger Seitenhalt. Regen rauscht,
  Vögel schweigen. Bei Regen und Nebel gehen Scheinwerfer auch tagsüber an.
- Nebel: Dunst über allem, um die Bildmitte lichter, Lichter bekommen einen weiten Hof.
- Fassaden: die der Sonne zugewandte Seite ist heller und wärmer, die abgewandte dunkler – mit dem Sonnenstand.
- Leuchtreklame bei Nacht vor Kneipen und Bars (oft mit ihrem echten Namen), Clubs, Spätis, Döner- und Pizzaläden,
  Imbissen und Hotels, in Neonfarben mit farbigem Schein auf dem Gehweg; manche Röhren flackern.

## [0.13.0] – 2026-09-28

### Hinzugefügt
- Mehr Fahrzeugarten mit eigenen Maßen, Motorleistung und Aussehen: 7,5-t-LKW (tagsüber auf Hauptstraßen),
  Paketwagen (werktags, nicht sonntags), das orange Müllauto (werktags morgens in Wohnstraßen), Streifenwagen und
  Rettungswagen. Der Verkehr hält Abstand zwischen den Stoßstangen, auch hinter einem 8,6 m langen Müllauto.
- Arbeit auf der Straße: Paketwagen halten alle 150–500 m in zweiter Reihe mit Warnblinker, das Müllauto alle
  35–80 m mit Rundumleuchte und zwei Müllwerkern samt Tonne; der Verkehr dahinter wartet.
- Einsätze: Liegt ein Toter auf der Straße, kommt nach kurzer Zeit ein Rettungswagen mit Martinshorn (440/585 Hz im
  Wechsel) und Blaulicht, fährt über den Spurgraph gezielt zum Einsatzort (über Rot langsam), hält dort, nimmt den
  Toten mit und fährt mit Sondersignal ab. Schüsse rufen einen Streifenwagen (höchstens einer je 45 s). Ab und zu
  fährt ein Einsatz einfach vorbei. Einsatzfahrzeuge entstehen und verschwinden außer Sicht.
- Radfahrer und E-Roller: auf dem Radstreifen, wo die Straße einen hat, sonst am rechten Fahrbahnrand von
  Nebenstraßen; Einbahnstraßen und Abbiegeverbote gelten auch für sie, die meisten halten bei Rot. Autos bleiben
  dahinter. Wer angefahren wird, stürzt, das Rad bleibt liegen. Abgestellte Leih-Roller stehen (und liegen) am Gehweg.
- Tiere: Taubenschwärme auf Plätzen, vor Imbissen und an Bahnhöfen, die auffliegen, wenn man hingeht, ein Auto
  vorbeirast, gehupt oder geschossen wird; Enten in Ufernähe auf Spree, Kanälen und Seen, die wegschwimmen.
- Umgebungsklang je nach Ort und Zeit: Stadtrauschen, Verkehr (LKW lauter), Vögel in Grün und Bäumen (Morgenchor,
  nachts still), Gemurmel vor Kneipen, Wasser am Ufer, das Rumpeln der Hochbahn, das Martinshorn in der Nähe und die
  Kirchenglocke zur vollen Stunde, wenn eine Kirche in Hörweite ist.
- Nachts leuchtet Blaulicht in die Straße, Warnblinker blinken orange.

## [0.12.0] – 2026-09-28

### Hinzugefügt
- Wochentage: Die Uhr zählt Mo–So (neues Spiel Freitag, 16:00), das HUD zeigt „Fr 22:34“, der Spielstand merkt sich
  den Tag (alte Stände bleiben gültig).
- Tagesrhythmus: Verkehr und Menschen folgen Werktag (Berufsverkehr) und Wochenende; Freitag- und Samstagnacht füllt
  das Nachtleben die Kieze mit Bars. Wie belebt ein Ort ist, bestimmen echte Daten: Verkehrsmengen 2019 (Kfz je
  Werktag, 31 305 Kanten gemessen, der Rest nach Straßenklasse geschätzt) und die Einwohnerdichte 2022 (64-m-Raster),
  beide aus dem Geoportal Berlin (dl-de/zero-2.0). Autos entstehen bevorzugt auf stark befahrenen Straßen.
- Stadtleben an echten Orten: Wartende an Haltestellen, Raucher vor Bars, Clubschlangen in Partynächten, Leute mit
  Flasche vor Spätis (und Kalles Stammgäste am Auftrags-Späti), Cafégäste an Tischen, Plaudernde vor dem Imbiss,
  Schaufenstergucker, Straßenmusik am U-Bahnhof, Sitzende auf Bänken, Gruppen auf Decken in Parks; Jogger und
  Hundehalter unter den Spaziergängern; Zigaretten und Handys glimmen nachts.
- Stadtmöbel aus OSM: 76 380 Bänke, Picknicktische, Fahrradständer (mit je Stunde wechselnd vielen Rädern) und orange
  Mülleimer.

### Geändert
- Die Zahl der Autos und Passanten ist nicht mehr fest, sondern wird um die Kamera je nach Ort und Zeit eingestellt;
  Überzählige verschwinden außer Sicht. Tests und Titel-Demo mit fester Bevölkerung bleiben unverändert.
- `?uhr=` baut die Bevölkerung für die gewählte Uhrzeit neu auf.

## [0.11.0] – 2026-09-27

### Hinzugefügt
- Gegenwehr: Etwa jeder siebte Passant wehrt sich (fest je Person). Wer geschlagen oder angeschossen wird und
  überlebt, steht auf und kommt mit den Fäusten zurück; Kämpfer in der Nähe einer Schlägerei mischen mit. Sie geben
  nach 20 s auf, wenn man ins Auto steigt oder über 45 m wegläuft. Alle anderen fliehen.
- Lebenspunkte der Spielfigur (100, Leiste im Waffenfeld): Faustschläge (9) und Anfahren (nach Tempo) verletzen,
  roter Bildrand bei Treffern; nach 8 s ohne Treffer heilt es (4 LP/s). Im Auto ist man geschützt.
- K. o. bei 0 LP: Die Figur geht zu Boden, nach 3 s wacht man am nächsten der 60 Berliner Krankenhäuser auf (der
  Stadtteil wird bei Bedarf erst geladen), 10 % des Geldes sind weg, ein laufender Auftrag scheitert.

## [0.10.1] – 2026-09-27

### Behoben
- Oberbaumbrücke (und alle vergleichbar erfassten Bauwerke in Berlin): Über Fahrbahn und U-Bahn stand ein 16 m hoher
  Klotz – der Brückenpfeiler (`building=bridge`, ohne Höhe → Standardhöhe) wurde als Haus gebaut, die Turmspitze
  (`min_height=15`) als 18-m-Klotz vom Boden aus, die eigentlichen Türme (34 m, nur als `building:part` erfasst)
  fehlten ganz, und zwischen Fahrbahnen und Viadukt sah man Wasser. Der Karten-Build
  - baut aus Brückenbauwerken (`building=bridge`, `man_made=bridge`), Dächern und schwebenden Teilen (ab 3 m über Grund
    bzw. ab dem 1. Geschoss) keine Häuser mehr – unter Überbauungen kann man jetzt hindurchfahren;
  - übernimmt Bauteile (`building:part`) ohne umgebenden Gebäudeumriss als Gebäude; bei ineinanderliegenden Teilen
    das höchste mit eigenem Umriss (der Turm auf dem Pfeiler, nicht ein turmhoher Pfeiler) – 2 209 Gebäude in Berlin;
  - zeichnet Brückendecks (`man_made=bridge`) als Mauerwerk über dem Wasser; dort gilt der Untergrund nicht als
    Wasser.

### Hinzugefügt
- Liste der 60 Berliner Krankenhäuser (alle 12 Bezirke) im Kartenindex – Grundlage für den Neustart nach K. o.

## [0.10.0] – 2026-09-27

### Hinzugefügt
- Kämpfen zu Fuß: Fäuste, Baseballschläger, Messer, Pistole, Maschinenpistole und Schrotflinte, alle von Anfang an,
  dazu Treten mit jeder Waffe. Munition unbegrenzt, Magazine werden nachgeladen (automatisch, wenn leer).
  - Pistole und Schrotflinte feuern je Druck, die MP im Dauerfeuer; die Schrotflinte fächert 8 Kugeln.
  - Schüsse sind sofortige Strahlen: Sie stoppen an Hauswänden, der Stadtgrenze, Bäumen und Kisten (über Zäune,
    Gleise und Kaikanten fliegen sie hinweg) und treffen das erste Ziel. Nahkampf trifft in einem Bogen vor der Figur.
  - Zielen in Blickrichtung mit Zielhilfe (rastet auf das nächste Ziel in einem Kegel mit freier Sicht ein); mit der
    Maus zielt die Figur auf den Zeiger (engere Zielhilfe), mit dem Controller zielt der rechte Stick.
- Treffer: Passanten fallen um, nach genug Treffern bleiben sie liegen (Blutlache) und verschwinden nach frühestens
  60 s außer Sicht. Blutspritzer in Schussrichtung, Blut am Boden verblasst langsam. Schüsse erschrecken Passanten im
  Umkreis von 42 m. Kugeln beschädigen Autos bis zum Wrack; der Fahrer eines beschossenen Autos steigt aus und flieht.
  Der Verkehr fährt über Tote hinweg, statt ewig davor zu warten.
- Darstellung: Waffe in der Hand, Schlag-, Stich-, Schwung- und Trittbewegung, Rückstoß, Mündungsfeuer (nachts als
  Lichtquelle), Leuchtspuren, Einschlagfunken, Fadenkreuz. HUD unten rechts zu Fuß: Waffe, Waffenleiste, Magazin,
  Nachladebalken. Zu Fuß zoomt die Kamera etwas heran (1,3-fach).
- Klänge je Waffe (synthetisiert), Schlag, Schwung, Einschlag, Nachladen, Waffenwechsel.

Steuerung zu Fuß (im Auto unverändert): RT / Strg / linke Maustaste angreifen · rechter Stick / Maus zielen ·
B / V treten · LB, RB / Q, Mausrad Waffe wechseln · 1–6 Waffe wählen · X / R nachladen.

## [0.9.0] – 2026-09-27

### Hinzugefügt
- Fünf Automodelle in derselben Kollisionsbox: Kleinwagen, Limousine, Kombi, Transporter und das Berliner Taxi
  (hellelfenbein mit Dachschild). Karosserie mit Wölbung, Scheiben mit Spiegelung, Dach, Außenspiegel; je Modell ×
  Farbe einmal doppelt aufgelöst gezeichnet (Cache begrenzt). Das Modell folgt der Autonummer, die Simulation und ihr
  Zufall bleiben unberührt; das Spielerauto ist eine Limousine.
- Sichtbare Bewegung: Räder an den Ecken, die Vorderräder lenken mit; Rückfahrlicht beim Rückwärtsfahren; die
  KI blinkt vor dem Abbiegen (ab 11 m vor der Kreuzung bis hinein, links/rechts aus dem Abbiegewinkel); Wracks mit
  Ruß und Beulen.
- Passanten mit Armen und Beinen im Gang (Schrittlänge aus der zurückgelegten Strecke), Hosen, Schuhe, Haarfarben und
  manchmal Rucksack oder Tasche – je Person fest.
- Gemessen: 1920 × 1080 reine Zeichenzeit 1,7–2,8 ms je Bild, JS-Heap 69 MB; bei 4-fach gedrosselter CPU 6,4 ms
  (Qualität bleibt hoch), bei 20-fach 38,7 ms → automatisch „niedrig“, danach wieder „hoch“.

## [0.8.0] – 2026-09-27

### Hinzugefügt
- Dächer nach Gebäudeart, Höhe und Seed (ohne neue Kartendaten): Altbauten mit „Berliner Dach“ (Ziegelrand, flache
  Mitte aus Kies/Bitumen), Flachdächer mit Attika, Satteldächer mit First entlang der Hauptachse und verschatteter
  Dachhälfte (Einfamilien- und Reihenhäuser bis 9 m, Schuppen, Kirchen in Schiefer oder Kupfergrün), Wellblech auf
  Industrie und Lagern.
- Dachaufbauten: Schornsteine, Lichtschächte, Oberlichter, Lüftungsgeräte, Solarmodule und Dachterrassen – nur
  innerhalb des Grundrisses, ohne Überlappung, an der Hauptachse ausgerichtet (im Kerngebiet rund 200 000).
- Fassaden je Stil: Altbau (hohe Fenster, Gesims je Geschoss), Plattenbau (Raster mit Balkonbändern, nur über 24 m),
  Neubau (Fensterband mit Pfosten), Industrie (Oberlichtband); Fenster in drei Glastönen.
- Kontaktschatten am Fuß jeder sichtbaren Fassade.

### Behoben (beim Testen gefunden)
- Dachaufbauten konnten bei eingebuchteten Grundrissen über die Hauskante ragen, weil nur die vier Ecken geprüft
  wurden; jetzt ein 5 × 5-Raster über die ganze Fläche.

## [0.7.0] – 2026-09-27

### Hinzugefügt
- Bodentexturen als Muster in Weltkoordinaten (einmal gemalt, deterministisch): Gehweg aus hellem Stein mit feiner
  Körnung und unregelmäßigen Platten, Asphalt mit Körnung, Großpflaster in versetzten Reihen, Gras/Kleingärten/Friedhof/
  Sportplatz mit Farbflecken, Wald mit Unterholz, Sand, Plätze mit kleinen Platten, Gleisschotter.
- Bordstein mit dunklem Rinnstein zur Fahrbahn (auch an den Kreuzungsflächen); Parkstreifen als hellere Tönung, durch
  die der Asphalt scheint.
- Kleinteile auf der Fahrbahn, deterministisch je Straßenstück: Gullys am Bordstein (etwa alle 25 m je Seite),
  Kanaldeckel, Asphaltflicken und Risse (Nebenstraßen flickiger, auf Pflaster keine), Ölflecken auf Parkstreifen.
  Alles liegt auf der eigenen Fahrbahn und nicht in der Kreuzungsfläche (Test über das ganze Kerngebiet).
- Baumkronen als lappige Blattballen mit Lichtkante und dunklem Rand (je Gattung drei Varianten, einmal gerendert);
  Baumscheiben unter Straßenbäumen im Gehweg.
- Wasser wird zum Ufer hin dunkler (Schatten der Kaimauer).

### Behoben (beim Testen gefunden)
- Kanaldeckel lagen auf Richtungsfahrbahnen (Einbahn-Hälften breiter Straßen) neben der Fahrbahn: die „Fahrbahnmitte“
  aus dem Querschnitt liegt dort am Bordstein.

## [0.6.0] – 2026-09-27

### Hinzugefügt
- Straßenlaternen: je Straße deterministisch am Bordstein auf dem Gehweg (Hauptstraßen im Mittel alle 25 m, sonst
  30 m, ab 10 m Fahrbahnbreite beidseitig versetzt), nie auf einer Fahrbahn, in einem Haus oder im Wasser, frei von
  Kreuzungsecken. Grundstückszufahrten und Fußwege zählen dabei nicht als Kreuzung. Mast mit Ausleger zur Fahrbahn;
  in Straßen mit Gasbeleuchtung (212 Abschnitte im Kerngebiet) Berliner Gaslaternen mit wärmerem Licht. Sie brennen von
  20:15 bis 5:45 und werfen einen Lichtfleck auf Gehweg und Fahrbahn.
- Erleuchtete Fenster: nachts leuchtet je Haus ein fester Teil der Fenster (abends mehr, gegen 3 Uhr wenige, tagsüber
  keine), ohne Flackern.
- Häuser und Baumkronen verdecken in der Lichtkarte, was dahinter am Boden leuchtet – kein Laternenschein auf Dächern.
- Ampeln werfen farbigen Schein, Läden, Lokale und Bahnhöfe warmes Licht aus dem Schaufenster auf den Gehweg.
- Automatische Qualitätsstufe: Braucht das Zeichnen im Median über 14 ms, verzichtet das Spiel auf Baumschatten und
  den zweiten Hausdurchgang der Lichtkarte (zurück unter 8 ms). Gemessen auf dem Mac: 1–3 ms reine Zeichenzeit je Bild
  bei 1920 × 1080, tags wie nachts.

## [0.5.0] – 2026-09-27

### Hinzugefügt
- Tageszeit: Die Spieluhr läuft beschleunigt (1 s = 1 Spielminute, ein Tag = 24 min), ein neues Spiel beginnt um 16:00.
  Die Uhrzeit steht im HUD neben dem Geld (☀/☾) und wird mit dem Spielstand gespeichert; ältere Spielstände ohne Uhr
  bleiben gültig. Der Titelbildschirm zeigt Abendstimmung. `?uhr=21:30` in der Adresse stellt die Uhr zum Anschauen.
- Schattenwurf: Häuser und Bäume werfen Schatten in Sonnenrichtung (Sonnenstand wie im Berliner Sommer; morgens nach
  Westen, mittags kurz nach Norden, abends lang nach Osten, zum Horizont hin verblassend). Die Schatten liegen in einer
  eigenen Ebene, damit sich Überlappungen nicht doppelt abdunkeln; auch Häuser knapp außerhalb des Bildes werfen hinein.
  Autos und Figuren haben einen mitwandernden Schatten statt eines festen Versatzes.
- Licht: goldene Stunde, Sonnenuntergang, blaue Stunde und bläuliche Nacht über eine Lichtkarte, die per „multiply“
  über die Welt gelegt wird (HUD und Schilder bleiben hell). Fahrende Autos haben nachts Scheinwerferkegel, Rücklichter
  und hellere Bremslichter; der Auftragsort und die Spielfigur leuchten, damit man sie findet. Kein messbarer
  Mehraufwand: 1920 × 1080 weiterhin Median 10 ms je Bild bei Tag, Abend und Nacht.

## [0.4.1] – 2026-09-27

### Behoben
- Verkehrschaos (Autos verkeilt, quer, Stoßstange an Stoßstange). Es waren viele Ursachen, jede für sich gemessen
  (Simulation an 5 Orten in Kreuzberg und Neukölln: Zusammenstöße 233 → 1, Rückwärts-Rangieren 271 → 10 in je 3 min;
  Autos, die länger als 60 s am Stück standen, 76 → 2 und keines mehr über 2 min in je 5 min):
  - Autos in der Schlange rammten den Vordermann („Hindernis ignorieren“ nach Wartezeit) – entfernt; Mindestabstand
    jetzt eine Autolänge plus 1,5 m (vorher kürzer als ein Auto).
  - Poller standen auf der Fahrbahn (allein im Kerngebiet 4 234), Zäune und Gleismauern lagen innerhalb der geschätzten
    Straßenbreite. Der Build schiebt Poller an den Bordstein und schneidet Zäune/Gleiswände aus befahrbaren Fahrbahnen.
  - Einmündungen ohne Ampel werden je Auto reserviert; einfahren darf, wer frei ist und hinter der Kreuzung Platz hat,
    Kolonnen dürfen 6 s nachrücken, dann ist die andere Zufahrt dran. Bewegungen, die sich nicht kreuzen (Gegenverkehr
    geradeaus, zweimal rechts), dürfen gleichzeitig hinein – vorher verklemmten sich zwei Kolonnen zwischen zwei nah
    beieinanderliegenden Kreuzungen.
  - Reservierungen der gerade verlassenen Kreuzung hielten bis 8,5 m dahinter (auch dort, wo gleich die nächste kommt);
    jetzt nur noch, bis das Heck heraus ist.
  - Wer warten muss, hält 2,4 m vor der Linie (vorher 1 m – dort schaltete die Route schon auf „in der Kreuzung“ und das
    Auto fuhr ohne Reservierung hinein), vor einer Engstelle 5 m weiter zurück, damit der Gegenverkehr ausschwenken kann.
  - Engstellen (zu schmal für Begegnungsverkehr) zählen über alle ihre Abschnitte als eine; ein Auto blieb eingetragen,
    auch wenn es nur einen Abschnitt verließ (vorher trug die erste Freigabe es ganz aus und Gegenverkehr fuhr hinein).
    In eine Sackgassen-Engstelle darf nur einer zur Zeit (wer drin ist, wendet und kommt zurück).
  - Hindernisse werden entlang der Route gemessen, die das Auto gleich fährt, nicht entlang seiner Längsachse: mitten im
    Abbiegen galt sonst ein korrekt auf der Gegenspur wartendes Auto als „im Weg“.
  - Abgestellte Autos ohne Fahrer (Spieler-, Missionsauto) zählen wie Parker: die KI fährt vorbei, wenn Platz ist,
    statt ewig davor zu warten.
  - Kommt man nicht weiter, sucht man sich nach 3 s einen anderen Weg; zwei Autos Kühler an Kühler: eines setzt zurück.
  - Angefahrene Passanten standen nie wieder auf und blockierten die Straße; Passanten, die zurück auf den Gehweg wollen,
    geben nach 4 s ohne Fortschritt auf; wartende Autos hupen, Passanten weichen dem Hupen aus.
  - Außerhalb des Sichtbereichs festgefahrene Autos werden abgebaut und neu erzeugt.

## [0.4.0] – 2026-09-27

### Geändert
- Skalierung für jedes Fensterformat: 16:9 (1280 × 720) ist das Grundformat und wird so skaliert, dass es ganz ins
  Fenster passt (Maßstab = min(Breite/1280, Höhe/720)). Vorher richtete sich alles nur nach der Höhe; in schmalen oder
  hohen Fenstern rückten Minikarte, Auftrag und Tacho aus dem Bild oder übereinander. Das Spiel-HUD hängt an den
  Fensterrändern, Titel, Pause, Ergebnis und Steuerung liegen mittig in einem 16:9-Rahmen, die Welt zeigt mindestens den
  16:9-Ausschnitt (breitere oder höhere Fenster zeigen mehr Stadt), POI-Schilder skalieren wie das HUD.

## [0.3.1] – 2026-09-27

### Behoben
- „Lade Stadtteil …“ blieb stehen, wenn der Spielserver nicht mehr lief (Kacheln kamen nie an, die Anzeige sagte nicht
  warum). Jetzt erscheint „Keine Verbindung zum Spielserver – läuft gta2d?“ mit Countdown, neue Versuche nach 0,5 / 1 /
  2 / 3 s, und das Spiel läuft sofort weiter, sobald der Server wieder da ist. Beim Laden zeigt die Anzeige den
  Fortschritt (Kacheln x / y).
- Ruckler beim Nachladen während der Fahrt: Eingetroffene Kacheln werden nicht mehr alle auf einmal eingebaut, sondern
  in Zeitscheiben (4 ms je Bild beim Spielen, 30 ms solange die Welt wartet), die nächsten zuerst. Bei 4-fach gedrosselter
  CPU längste Unterbrechung 123 statt 380 ms.

## [0.3.0] – 2026-09-27

### Hinzugefügt
- Stadtplan beschriftet je nach Zoom: die 12 Bezirke, näher die Ortsteile (Tegel, Prenzlauer Berg …), dann die
  Kiez-Namen aus OSM (Flughafenkiez, Reuterkiez … – 568, auch als Fläche eingetragene) in Gelb und die Bahnhöfe, ganz nah
  die Straßennamen entlang der Straße (Hauptstraßen zuerst). Nichts überlappt, die Hinweisleiste bleibt frei.
- Karten-Build verkettet Straßenstücke gleichen Namens zu Straßenzügen (Stadtplan 2,0 statt 6,6 MB) und legt
  Beschriftungspunkte im Inneren der Ortsteile und Bezirke ab.
- Zoom bis ca. 1 m je Bildpunkt.

### Behoben
- Schlägt das Laden einer Kachel fehl, fragt das Spiel sie erst nach 1, 3 und dann 10 s erneut an und meldet den
  Fehler einmal (vorher in jedem Bild).

## [0.2.0] – 2026-09-27

### Hinzugefügt
- Menüs mit der Maus: Zeigen wählt einen Eintrag, Klicken bestätigt; die Tastenhinweise unten (A/B) sind anklickbar.
  Mausklicks laufen als abstrakte Eingabe durch dieselbe Menülogik wie Controller und Tastatur.
- Eigener Mauszeiger im Stil des Spiels: gelber Pfeil, heller Pfeil mit Klick-Strahlen über Anklickbarem, Zielkreuz auf
  dem Stadtplan, Verschiebe-Pfeile beim Ziehen. Beim Spielen verschwindet er, wenn die Maus 2 s ruht.

## [0.1.1] – 2026-09-27

### Behoben
- Dauerhaftes Rauschen während des Spiels: Die „Stadt-Atmosphäre“ war ein endlos geschleiftes weißes Rauschen
  (1-s-Schleife, Tiefpass 500 Hz), das ab dem ersten Tastendruck lief. Sie ist entfernt; Rausch-Stöße gibt es nur noch
  kurz bei Crash, Tür und Zusammenstoß.

## [0.1.0] – 2026-09-27

### Stadt
- **Ganz Berlin** statt Kreuzberg und Nord-Neukölln: alle 12 Bezirke und 97 Ortsteile im Maßstab 1:1, ca. 46 × 38 km –
  529 000 Gebäude, 323 000 Straßenstücke, 962 545 Bäume aus dem Baumbestand (dazu 70 000 aus OSM), 50 000 POIs,
  415 000 Hausnummern, 4 032 Ampelkreuzungen, 1 336 Abbiegeverbote, 4 676 Tordurchfahrten.
- Jede Datenschicht ist in jedem Bezirk gefüllt; der Karten-Build gibt die Abdeckung je Bezirk aus, ein Test prüft sie.
- Ortsnamen im HUD nach Ortsteil (z. B. „Neukölln“ statt „Nord-Neukölln“).

### Spiel
- Die Karte wird in Kacheln zu 640 × 640 m um die Kamera nachgeladen und wieder freigegeben (JS-Heap im Browser
  45–110 MB statt ~280 MB für den bisherigen Ausschnitt). Fehlt ein Stadtteil noch, steht die Welt kurz still.
- Stadtplan von ganz Berlin mit Zoom (Mausrad) und Verschieben (Ziehen); Teleport überallhin, das Ziel lädt vorher.
- Spielstände dürfen überall in Berlin liegen.

### Werkzeuge
- OSM-Daten aus dem Geofabrik-Auszug mit eigenem PBF-Leser statt Overpass; Build ~40 s.
- Kartenformat 3: `web/data/berlin/` (index.json, overview.json, tiles/) ersetzt `web/data/city.json`.

## [0.0.1] – 2026-09-27

Erste versionierte Fassung. Enthält alles bis zu diesem Stand:

### Spiel
- Top-down-Open-World in Kreuzberg und Nord-Neukölln im Maßstab 1:1 (10 px = 1 m) aus OpenStreetMap und den
  LOR-Grenzen des Geoportals Berlin.
- Zu Fuß und im Auto, Mission „Kisten für den Kiez“ mit Zeitlimit aus der echten Route, Speichern in `localStorage`.
- Stadtkarte mit Teleport per Mausklick und Bestätigungsdialog.
- POIs (Geschäfte, Bars, Restaurants, U-/S-Bahnhöfe) und Hausnummern in Ortsangabe und Beschriftung.

### Stadt
- Straßenraum aus OSM-Tags: Fahrbahnbreite Bordstein zu Bordstein, Fahrstreifen je Richtung, Park- und
  Radfahrstreifen, Tempo 30/50, Kopfsteinpflaster; geparkte Autos auf den Parkstreifen.
- Verkehrsregeln: Ampeln an 550 Kreuzungen, Zebrastreifen, Abbiegeverbote, Spurwahl beim Abbiegen.
- Tordurchfahrten in Hinterhöfe, Poller und Modalfilter, Zäune/Mauern/Hecken, Hauseingänge.
- Bäume aus dem Berliner Baumbestand (Gattung, Krone, Stamm); kein Stamm steht auf der Fahrbahn.
- Gleise maßstabsgerecht und in Ebenen (Tunnel, ebenerdig, Hochbahn).

### Plattform
- Browser (Mac) und C#-UWP/WebView2-Hülle für die Xbox im Developer Mode (noch nie kompiliert, unverifiziert).
