# Native Rust-Portierung

## Stand: Phase 5 + HUD und Gamepad

Rust Edition 2024, mindestens Rust 1.95, Cargo-Workspace mit vier Crates (`berlin-sim` siehe
Abschnitt Phase 3 unten):

- `berlin-engine`: winit 0.30, wgpu 29, WGSL, glam-Kamera, Render-Loop,
  indizierte Kachel-Meshes, Depth-Pass und instanzierter Sprite-Atlas.
- `gta-berlin`: Startprogramm, Geokoordinaten, Datenprüfung und GPU-Aufnahmen.
- `berlin-map-loader`: serde/serde_json-Decoder für Format v3, GRS80-Projektion,
  Geometriehilfen, Berliner Gebäude-/Dachpaletten und Hintergrund-Streaming.

`cargo run` startet am Missionsort in Kreuzberg mit den vorhandenen Daten aus
`web/data/berlin`. Der aktuelle Index enthält **2921 Kacheln**; die Zahl 2920 im
Portierungsprompt ist gegenüber dem vorhandenen Datenstand um eine Kachel kleiner.
Straßen, Gehwege, Wege, Kreuzungen, Landflächen, Wasser, Gleise, Gebäude und
Bäume sind sichtbar. Die Kartenansicht ist noch kein spielbarer Rust-Port:
Seit Phase 3 fahren darauf Verkehr und Spieler (siehe unten); Gamepad und Audio folgen.

## Daten und Streaming

Die bestehenden JSON-Dateien werden unverändert verwendet. Ein benannter
Hintergrundthread liest und dekodiert Kacheln, trianguliert Polygone mit earcutr,
baut Straßen-Meshes und erzeugt Gebäude-/Sprite-Geometrie. Die Render-Schleife
führt keine Dateizugriffe oder Triangulierung aus. Kameraaufträge ersetzen ältere
Aufträge; die Übergabe fertiger Zustände ist auf zwei Snapshots begrenzt.

Geladen wird innerhalb mindestens 7000 Kartenpixeln, gehalten innerhalb mindestens
11000 Pixeln. Große Viewports vergrößern den Radius. Die nächste Kachel kommt
zuerst; höchstens 64 Kacheln bleiben resident. Fehlgeschlagene Dateien werden
nach 500/1000/2000/3000 ms erneut versucht und beim ersten Fehler protokolliert.
Der Fenstertitel zeigt den Ladefortschritt und die Kartenattribution.

Globale Objekt-IDs halten Referenzen auf alle residenten Kacheln. Genau eine
Kachel zeichnet ein geteiltes Objekt; beim Entladen wechselt diese Zuständigkeit
zur nächsten Kachel. Nur betroffene Kachel-Batches werden neu zusammengesetzt
und hochgeladen. Bereiche mit ID −1 gehören ihrer Kachel; ihre Dreiecke werden
am exakten Kachelrand abgeschnitten. GPU-Puffer ferner Kacheln werden freigegeben.

## Geometrie und Darstellung

`geom.rs` portiert Punkt-in-Ring, Gerade-/Ungerade-Füllung, Flächen, Boundingbox,
Segmentabstand, Bogenlängen, Projektion auf Linien, Parallelversatz,
Delta-Koordinaten, Ringindex und Rechteck-Clipping nach glam::Vec2.
Die GRS80/Krüger-Projektion nutzt f64 und entspricht `projection.js`, inklusive
JS-Rundung. Der Decoder kontrolliert Version, Schlüssel, Koordinatenlisten und
Knotenreferenzen. Codes und Bitfelder entsprechen `citycodes.js`.

Polygone werden mit konkaven Außenringen, Innenhöfen, getrennten Außenringen
und Inseln in Löchern trianguliert. Die Verschachtelung folgt der Canvas-
Gerade-/Ungerade-Regel, auch bei Gebäuden mit mehreren getrennten Grundrissen.
Straßen/Gleise verwenden zusammenhängende Vertex-/Index-Batches je Kachel;
begrenzte Gehrungen verbinden Straßensegmente. OSM-Ebenen bestimmen die
Zeichenreihenfolge für Brücken und Unterführungen.

Gebäude erhalten extrudierte Fassaden, offene Wandzüge für Tordurchfahrten und
Dachparallaxe. Dach-/Fassadenstil, Farbprioritäten und Paletten stammen aus
`roofs.js` und `buildcolors.js`: OSM-Farbe vor Material vor geschätztem Stil.
Sattel-, Walm-, Zelt-, Mansard-, Pult- und Berliner Dachflächen werden nach
Grundriss und Hauptachse erzeugt und auf die triangulierten Grundrisse begrenzt.
Kuppel-/Runddächer verwenden variierende Normalen; die Schattierung der
Dachflächen und Fassaden erfolgt im WGSL-Fragment-Shader anhand der Sonnenrichtung.

Boden-/Dachtexturen und Fassadenfenster entstehen prozedural im Shader, mit
Ableitungen für die Detailreduktion. Ein einmal erzeugter Atlas enthält zwei
Baumkronen und Straßendetails (Gully, Kanaldeckel, Flicken, Riss, Öl, Markierung).
Ein Instanzbuffer je Kachel zeichnet sie ohne einzelne Canvas-Aufrufe.

Die GPU-Fassung übernimmt die Kartenstruktur und grundlegenden Material-/Stilregeln.
Die Darstellung ist noch vereinfacht: Gauben, Schornsteine und andere Dachaufbauten,
individuelle Fenster-/Fassadenstile, Zebrastreifen, Schilder, Stadtmöbel und die
vollständigen Gehweg-/Parkstreifendetails des JS-Renderers sind noch nicht umgesetzt.
Kuppeln erhalten noch keine eigene gewölbte Höhengeometrie. Schatten und dynamische
Beleuchtung sind Phase 4; aktuell ist der Sonnenstand statisch wählbar.

## Mac-Entwicklung

Rust über rustup und Xcode Command Line Tools (`xcode-select --install`) genügen.
Kein Node und keine Windows-SDK-Abhängigkeit für den nativen Start.

```bash
cargo run
cargo run --release
cargo run --release -- --fps 120
cargo run -- --smoke-frames 10
cargo run -- --check-map
cargo run -- --geo 52.5163 13.3777 --zoom 0.72
cargo run -- --geo 52.5014 13.4455 --sun-hour 8 --capture /tmp/oberbaum.png
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Der GPU-Name samt Backend wird beim Start protokolliert. macOS verwendet explizit
Metal, Windows explizit DX12; Linux verwendet Vulkan als Entwicklungsfallback.
Ein Grafikadapter und eine Desktop-Sitzung sind für den Fensterstart erforderlich.
`--smoke-frames N` zählt erst bei vollständig geladenem sichtbaren Kartenausschnitt.
Nach N präsentierten Kartenframes beendet es; Ladefehler und ein Timeout nach
30 Sekunden melden einen Fehler. `--capture PNG` liest ein eigenes GPU-Renderziel
zurück und beendet nach zehn Kartenframes (oder der gewählten Smoke-Framezahl).
`--check-map` prüft alle Kacheldateien und Polygon-Triangulierungen ohne Fenster.
`--data PFAD` wählt einen anderen Datenordner; `--position X Y` setzt Kartenpixel,
`--geo LAT LON` Geokoordinaten. Beide Positionsoptionen schließen sich aus.
`--zoom` erlaubt 0,72–2,6, `--sun-hour` 5,5–20,5 für die Sonnenrichtung.

WASD/Pfeile bewegen die Kamera, Shift erhöht das Bewegungstempo.
Mausrad zoomt zwischen 0,72 und 2,6.
1 = schneller Fahrzeugzoom (0,72), 2 = Standard (1), 3 = Fußzoom (2).
Der Fußzoom der JS-Vorlage beträgt 2 mit Bereich 1,5–2,6; der größere native
Gesamtbereich erlaubt zusätzlich die Fahrzeugansicht. Bei Fokusverlust werden
gedrückte Tasten gelöscht. Esc oder Fensterschließen beendet das Programm.

## Kamera und Frame-Pacing

Der Kartengrundriss verwendet dieselbe affine Projektion wie `render.js`:
Bildschirmmitte + (Weltposition − Kameraposition) × Maßstab × Zoom.
Y zeigt nach Süden. Gebäudehöhen verschieben die Dächer nach oben und erzeugen
positionsabhängige Parallaxe entsprechend `render.js` (heightScale 0,5).
Die Gebäudeparallaxe verwendet den Gebäudemittelpunkt für alle Dachpunkte,
wie die Vorlage. Unit-Tests prüfen
Rückprojektion, Zentrum, Dachversatz und Zoomgrenzen.

FIFO-V-Sync verhindert Tearing; `WaitUntil` begrenzt die Anforderung neuer Frames
auf 60 oder 120 pro Sekunde, mit maximal einem angeforderten Frame Latenz.
Die tatsächliche Rate bleibt vom Monitor und der GPU abhängig. Resize und
HiDPI nutzen physische Pixel. Nullgroße Fenster pausieren Rendering; verlorene
Surfaces werden neu erstellt, veraltete neu konfiguriert, Timeouts übersprungen.
Die Kamera integriert reale Zeit mit einer 100-ms-Obergrenze; die feste
Simulationsschrittweite wird mit Phase 3 ergänzt.

## Windows und Xbox

Ein Windows-Desktop-Build nutzt DX12. Cross-Compilation vom Mac kann vorbereitet
werden mit:

```bash
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin --locked
cargo xwin build --release --target x86_64-pc-windows-msvc -p gta-berlin
```

Dieser Workflow ist noch nicht auf Windows oder Xbox validiert.
Ein Windows-Desktop-EXE ist noch kein installierbares Xbox-Dev-Mode-Paket.
Der bestehende UWP/WebView2-Host kann dieses winit-Programm nicht unmittelbar
als natives UWP-Spiel starten. App-Modell, zulässige APIs, Paketierung und
Grafikzugriff müssen separat geklärt und auf echter Hardware geprüft werden.
60–120 FPS auf Xbox sind ein Ziel, kein Messergebnis.

## Referenzen

- [winit ApplicationHandler](https://docs.rs/winit/0.30.13/winit/application/trait.ApplicationHandler.html)
- [wgpu Surface](https://docs.rs/wgpu/29.0.4/wgpu/struct.Surface.html)
- [earcutr](https://docs.rs/earcutr/0.5.0/earcutr/fn.earcut.html)
- [cargo-xwin](https://github.com/rust-cross/cargo-xwin)

Die bestehenden Karten bleiben unverändert. Browser- und UWP-Dokumentation
beschreibt bis zum Abschluss der Migration weiterhin den spielbaren JS-Prototyp.

## Validierung am 03.10.2026

Auf diesem Mac (Apple M1 Pro, Metal) erfolgreich ausgeführt:
`cargo fmt --all -- --check`, Clippy mit `-D warnings`, `cargo test --workspace`
(zwei Kameratests), sowie jeweils zehn präsentierte Frames im 60- und
120-FPS-Zielmodus. Dies bestätigt Start und GPU-Pipeline, keine gemessene
120-FPS-Dauerleistung. Windows-Cross-Build und Xbox-Hardwaretest stehen aus.

## Validierung Phase 2 am 04.10.2026

Alle 2921 Kacheln mit 2423429 Einträgen (inklusive mehrfach abgelegter Objekte)
wurden gelesen und dekodiert, alle darin enthaltenen Gebäude-/Flächen-/Wasser-
Polygone erfolgreich trianguliert. Die Rust-Tests prüfen unter anderem Innenhöfe,
konkave Grundrisse, getrennte Ringe/verschachtelte Inseln, Projektionsparität,
Referenzzählung und Besitzerwechsel, asynchrones Entladen/Neuladen nach Teleports
sowie Fehler und Wiederholversuche. Drei reale Berlin-Kacheln werden vollständig
in Meshes umgesetzt und auf gültige Indizes sowie endliche Attribute geprüft.

Metal-Läufe auf dem Apple M1 Pro wurden am Kreuzberger Startpunkt und am
Brandenburger Tor (120-FPS-Zielmodus) geprüft; die GPU-Aufnahmen wurden visuell
kontrolliert. Auch die Oberbaumbrücke wurde mit Spree, Gleisen und Baumkronen
bei Morgenlicht geprüft: [GPU-Aufnahme](images/native/phase2-oberbaum.png).
Formatprüfung, Clippy mit `-D warnings` und alle 13 Rust-Tests sind erfolgreich.
Diese Tests belegen Rendering und Streaming, keine gemessene
60-/120-FPS-Dauerleistung. Windows/DX12 und Xbox sind weiter ungeprüft.

## Phase 3: Physik, Kollision und Spiellogik (04.10.2026)

Neues Crate `crates/sim` (`berlin-sim`): die deterministische Simulation ohne Fenster und GPU, portiert aus
den JS-Modulen. Lagen werden in `f64` gerechnet (Kartenpixel bis ~300 000, Schritte von Bruchteilen eines
Pixels). Zufall ist bitgleich `mulberry32`, `hash01` folgt der JS-Umwandlung über ToUint32.

| Rust-Modul | Vorlage | Inhalt |
|---|---|---|
| `collision.rs` | `collision.js`, `grid.js` | Kreis/Rechteck/OBB/Segment, SAT mit kürzestem Ausweg (Wände ohne Dicke), `SpatialHash` mit Handles und Entfernen, `Grid` mit aufsteigenden Indizes |
| `city.rs` | Simulationsteile von `map.js` | eigener Decoder der v3-Kacheln (Knoten mit Kreuzungsradius, Kanten mit Querschnitt, Wände, Gebäude, Flächen, Poller, Bäume, Querungen, Ampeln, Abbiegeverbote, Portale), referenzgezählte geteilte Objekte, Fokus-Streaming mit Wiederholversuchen, `surface_at`, `in_building`, `on_road`, `nearest_edge`, Stadtgrenze |
| `levels.rs` | `levels.js` | Ebenen, Portale, `touch` |
| `car.rs`, `dynamics.rs`, `carmodels.rs`, `traction.rs` | gleichnamige JS-Module | Arcade-Physik für Verkehr, Einspurmodell für das gefahrene Auto (Lastverschiebung, Kammscher Kreis, ESP/ASR, ABS, Handbremsen-Drift, Wheelie/Stoppie), 37 Pkw-Modelle + Sonderfahrzeuge, Haftung je Untergrund und Nässe/Schnee/Glätte, Schaden, umfahrbare Poller |
| `roadgraph.rs` | `roadgraph.js`, `street.js`, `signals.js` | Fahrspuren aus dem Querschnitt, Bézier-Verbinder, Abbiegeregeln, Ampelumlauf |
| `traffic.rs` | `traffic.js` | Pure Pursuit, Kurven-/Ampel-/Zebra-/Abstandsregeln, Blockadelösung, Kreuzungs- und Engstellen-Reservierungen |
| `pedestrians.rs` | `pedestrians.js` | Gehwege, Abbiegen, Queren am Zebrastreifen, Warten vor Autos, Flucht und Rückkehr |
| `mission.rs`, `save.rs` | `mission.js`, `save.js` | Zustandsmaschine „Kisten für den Kiez“; Spielstand als JSON-Datei (gleiches Format wie die Browserfassung, atomar geschrieben) |
| `world.rs` | Kern von `world.js` | fester Schritt 1/60 s, Ein-/Aussteigen, Kollisionen, Bevölkerung um die Kamera, Parker, Mission, Kamera, Spielstand |

Rust-spezifische Entscheidungen: Die KI liest eine Momentaufnahme aller Fahrzeuge (`Agent`), statt während der
Schleife auf andere Autos zuzugreifen – das entspricht der JS-Fassung, in der sich während der KI-Schleife nichts
bewegt. Reservierungen und die Ansprüche je Auto liegen gemeinsam in `Reservations`. Kacheln lädt im Spiel ein
eigener Thread (`ThreadedSource`); die Simulation baut sie nur ein und steht still, bis der Ausschnitt da ist.

Spielstand: macOS `~/Library/Application Support/GTA Berlin/save.json`, Windows `%APPDATA%\GTA Berlin\save.json`,
sonst `$XDG_DATA_HOME/gta-berlin/save.json`; `--save DATEI` wählt eine andere Datei, `--new` beginnt ohne Laden.

Darstellung: Die Engine zeichnet bewegte Objekte über eine instanzierte Pipeline (`Body`: Rechteck mit
abgerundeten Ecken, Ellipse oder Ring, Kantenglättung per SDF). Autos haben Schatten, Dach, Frontscheibe,
Bremslichter und Kisten; Passanten Körper und Kopf; das Missionsziel einen pulsierenden Ring.
[GPU-Aufnahme](images/native/phase3-play.png). Hinweis: In WGSL darf kein Bezeichner `half` heißen – Naga
übernimmt ihn nach Metal, wo `half` ein Typname ist.

**Noch nicht portiert:** Wetter- und Tagesverlauf (Nässe/Schnee/Glätte sind Zustand und wirken, werden aber
noch nicht fortgeschrieben), Pfützen/Aquaplaning-Auslösung und Sturmböen, Waffen und Nahkampf, Räder, Tiere,
Nahverkehr und U-Bahnhöfe, Aufenthaltsorte, Einsatzfahrzeuge, Tagesrhythmus der Bevölkerung, Klick-Steuerung,
Gamepad, HUD-Texte (Hinweise stehen im Fenstertitel), Audio und Licht. Der JS-Bot, der die Mission über
A*-Routen selbst fährt, ist noch nicht portiert; der Missionstest versetzt das Auto.

### Validierung Phase 3 am 04.10.2026

Formatprüfung, Clippy mit `-D warnings` und alle 58 Rust-Tests erfolgreich. `berlin-sim` hat 35 Unit-Tests
(SAT-Fälle inklusive gedrehter Boxen und Wände ohne Dicke, Hash mit Entfernen und Stempelüberlauf, Raster-
Reihenfolge, JS-Referenzwerte für `mulberry32`/`hash01`, Fahrdynamik: Beschleunigung, Spitze, Bremsweg trocken
gegen Glätte, Drift, Wheelie; Mission, Spielstand) und 10 Integrationstests auf den echten Kacheln um den
Missionsort: Bevölkerung, 60-s-Dauerlauf (keine KI im Haus, ≥ 70 % der Autos fahren mehr als 30 m), Ampeln
nie gleichzeitig grün, Spielerauto nie im Haus, Beschleunigen/Bremsen auf gerader Spur, Wände zu Fuß, Mission
über Welteingaben, Spielstand über Datei, Determinismus.

`cargo run --release -- --check-sim 120` auf dem Apple M1 Pro: 120 s Spielzeit in 1,4 s (0,19 ms je Schritt mit
22 KI-Autos, 55 Passanten und rund 30 Parkern), alle KI-Autos in Fahrt, keine Unfälle. Ein Metal-Lauf mit
90 Frames und GPU-Aufnahme wurde visuell kontrolliert. Das belegt Simulation und Zeichnen auf dem Mac, keine
Dauer-Bildrate; Windows/DX12 und Xbox sind weiter ungeprüft.

## Phase 4: Tageslicht, Schatten und Licht (04.10.2026)

Die Uhr der Simulation (1 Echtsekunde = 1 Spielminute) treibt jetzt das Licht. `berlin-sim` enthält die reinen
Rechenteile, die Engine die GPU-Pässe.

| Rust | Vorlage | Inhalt |
|---|---|---|
| `sim/daylight.rs` | `daylight.js` | Sonnenhöhe/-azimut, Schattenrichtung, -länge (≤ 2,4 × Höhe) und -stärke, Umgebungslicht je Kanal, Dunkelheit, Laternen an/aus, erleuchtete Fenster; `format_clock`/`parse_clock` |
| `sim/lamps.rs` | `lamps.js` | Laternenstandorte je Kante (Abstand nach Straßenklasse, beidseitig ab 10 m, Ecken frei, nie auf Fahrbahn/im Haus/im Wasser), Gas-/Haupt-/Nebenstraßenlicht, `LampCache` |
| `map_loader/mesh.rs` | `lighting.js addBuildingShadow` | je Gebäudewand ein Schatten-Viereck (`ShadowVertex`: Fuß, gestauchte Höhe max(18, h × 0,5), Extrusionsflag) |
| `engine/lightpass.rs`, `lighting.wgsl` | `lighting.js` | Schattenmaske, Lichtkarte, Auftragen |

Ablauf je Bild:
1. **Schattenmaske** (R8, volle Auflösung): Der Vertex-Shader versetzt die Wandvierecke um
   `min(900, Höhe × Länge)` entlang der Sonne; zusammen ergeben sie den Schatten des Prismas. Baumkronen werden aus
   dem Sprite-Atlas als Kronenbild in der Höhe der Kronenmitte versetzt und entlang der Sonne gestreckt
   (`treeShadowGeom`). Max-Mischung: Überlappungen dunkeln nicht doppelt. Auch Häuser bis 900 px außerhalb des
   Bildes werfen hinein.
2. **Lichtkarte** (RGBA16F, halbe Auflösung, nur bei Dunkelheit): mit dem Umgebungslicht gefüllt, Lichtquellen
   additiv – Laternen, Scheinwerferkegel, Stand-, Rück- und Bremslichter, Ampeln (Farbe aus dem Umlauf), Schein um
   den Spieler und das Missionsziel. Verlauf und Kegelform wie die Canvas-Sprites der JS-Fassung.
3. **Auftragen** über die vorhandene Tiefe: ein Vollbild-Dreieck in Tiefe 0,5. Boden, Straßen, Autos und Personen
   liegen dahinter (0,55–0,94) und bekommen Schatten (bläulich, 34 % × Sonnenstärke) bzw. die Lichtkarte
   (Multiplikation); Dächer, Fassaden und Kronen liegen davor (≤ 0,45) und bekommen nur das Umgebungslicht – so
   leuchten Laternen nicht auf Dächer, wie `lightOccluders` in der JS-Fassung. Schatten werden vor Bäumen und
   bewegten Objekten aufgetragen, das Licht danach.

Steuerung: `--uhr HH:MM` setzt die Startzeit (auch mit `--free`), `--sun-hour` nimmt jetzt 0–24, Taste T stellt
die Uhr eine Stunde vor. Die Engine erhält das Licht über `Game::lighting`/`Game::lights` (`Lighting`,
`LightSource`); ohne Spiel gilt das Licht aus den Optionen.

**Noch offen:** erleuchtete Fenster, Laternenköpfe als eigene Lichtpunkte, Ladenlicht aus POIs, Blaulicht
(keine Einsatzfahrzeuge), Bloom, Farbabstimmung (`drawGrade`), Vignette, Wetterlicht (Wolken, Nässe, Nebel)
und Silhouetten verdeckter Figuren.

### Validierung Phase 4 am 04.10.2026

Formatprüfung, Clippy mit `-D warnings` und alle 62 Rust-Tests erfolgreich. Neu: Tageslicht (Mittag, Nacht,
Abend, Uhrzeit-Text), Abbildung Tageslicht → Engine-Licht (Sonnen- und Schattenrichtung entgegengesetzt), und
ein Integrationstest der Laternen auf echten Kacheln: genau 41 Laternen um den Späti – dieselbe Zahl liefert
`lamps.js` auf derselben Karte im selben Ausschnitt –, keine im Haus oder auf der Fahrbahn.

Metal-Aufnahmen auf dem Apple M1 Pro wurden visuell geprüft: [19:00 mit langen Schatten nach Osten](images/native/phase4-evening.png),
[22:30 mit Scheinwerfern und Laternen](images/native/phase4-night.png), dazu 9:00 an der Oberbaumbrücke
(Schatten nach Westen) und 17:30 mit Baumkronenschatten. Pixelprobe Mitternacht gegen Mittag: Wiese 105 → 28,
ein rotes Auto unter Laternenlicht 227 → 124. Beobachtet: Der Smoke-Test lief beim ersten Start nach einem
Neubau zweimal in den 30-s-Timeout (danach jeweils erfolgreich); vermutlich wurde das neue Fenster verdeckt
gemeldet und präsentierte keine Bilder – nicht weiter untersucht.

## Phase 5: Synthetisierter Klang (04.10.2026)

Wie in der Browserfassung gibt es keine Aufnahmen: alles wird erzeugt. Die Mischregeln sind rein und getestet in
`berlin-sim`, die Klangerzeugung liegt im neuen Crate `crates/audio` (`berlin-audio`), die Ausgabe läuft über
`cpal` (macOS CoreAudio, Windows WASAPI).

| Rust | Vorlage | Inhalt |
|---|---|---|
| `sim/enginevoice.rs` | `enginevoice.js` | 17 Motorcharaktere (Dreizylinder bis V10, Diesel, Boxer, Zweitakter, Motorrad, Elektro), Zuordnung je Modell, Fourierspektrum eines Arbeitszyklus mit Bankversatz (Crossplane-V8) |
| `sim/soundscape.rs` | `soundscape.js` | Motoren je Art/Modell (Leerlauf, Abregeldrehzahl, Zylinder, Gänge), Drehzahl mit Schalten, Last, Turbo, Rekuperation; Reifen (Abrollen, Pflaster, Nässe, Schnee, Quietschen/Rutschen, Fahrtwind); die vier nächsten Fremdautos mit Pegel, Panorama und Dopplerfaktor; Schritte je Untergrund |
| `sim/ambience.rs` | `ambience.js` | Stadtrauschen, Verkehr, Vögel (Grün, Tageszeit), Wasser, Dämpfung im Auto |
| `audio/dsp.rs` | Web-Audio-Knoten | `setTargetAtTime`-Glättung, Oszillatoren mit PolyBLEP, Wellentabellen (`PeriodicWave`, auf Spitze 1 normiert), Rauschen, Biquad nach Web-Audio-Spezifikation (Tief-/Hochpass mit Q in dB, Bandpass, Peaking, Low-Shelf), Gleichleistungs-Panorama, Sättigung, Kompressor, Hüllkurven |
| `audio/synth.rs` | `audio.js` | Motor (Wellentabelle aus dem Zylinderspektrum, Kurbelwellenton, Ansaugung, Auspuff und Diesel-Nageln im Zündtakt moduliert, Turbo und Abblasen, Auspuffknallen, Rückwärtssummer, Elektro-Summen), Reifen/Wind/Quietschen, Regen aufs Dach, vier Fremdfahrzeug-Stimmen, Umgebungsschichten, Vogelstimmen, Einzelklänge (Unfall, Hupe, Poller, Tür, Mission, Schritte); Bus „draußen“ mit Karosserie-Tiefpass, Hauptpegel 0,55, Kompressor |
| `audio/output.rs` | – | Echtzeit über `cpal` (f32/i16/u16), Offline-WAV |
| `game/sound.rs` | `main.js` | je Simulationsschritt ein Klang-Frame: Ereignisse mit Hörweite 1400 px, eigenes Fahrzeug, Schritte, Stimmen, Umgebung |

Bedienung: M schaltet den Ton um, `--stumm` startet ohne Ausgabe. Ohne Ausgabegerät läuft das Spiel stumm weiter.
`cargo run --release -- --audio-wav fahrt.wav [--audio-seconds 14]` simuliert ohne Fenster eine Messfahrt
(einsteigen, Vollgas, bremsen, Teillast auf einer geraden Spur nahe dem Späti), protokolliert Tempo, Drehzahl,
Gang und Zündton je Sekunde und schreibt den Klang als 16-Bit-WAV (48 kHz).

**Noch offen:** Nachtleben (Stimmengewirr, Lachen, Clubbass), Hochbahn-Rumpeln, Martinshörner, Regen- und
Sturmschichten mit Tropfen und Pfeifen (warten auf das Wettersystem), Donner, Kirchenglocken, Austausch gegen
Dateien über das Asset-Manifest. Die Kompressorkennlinie ist eine eigene Näherung, nicht die exakte
`DynamicsCompressorNode`.

### Validierung Phase 5 am 04.10.2026

Formatprüfung, Clippy mit `-D warnings` und alle 77 Rust-Tests erfolgreich. Neu geprüft: Glättung entspricht
`setTargetAtTime` (1 − e⁻¹ nach einer Zeitkonstante), alle Wellenformen treffen ihre Frequenz, Wellentabellen
geben die Fourieranteile wieder, Filter lassen durch bzw. sperren wie spezifiziert (Bandpass 0 dB in der Mitte,
Peaking +6 dB), Panorama mit konstanter Leistung, Kompressor; Synthesizer: still ohne Eingabe, immer endlich
und begrenzt, Einzelklänge laufen aus, Motor klingt auf seiner Zündfrequenz und folgt der Drehzahl,
Fremdfahrzeug nach links gepannt und per Doppler höher, Karosserie dämpft oberhalb 4 kHz; Gangwahl beim
Beschleunigen nur aufwärts; Motorspektren (Vierzylinder nur auf jeder vierten Zyklusharmonischen, V8 mit
Zwischenanteilen); Klang-Frames auf der echten Karte (Schritte beim Joggen, Tür, Motor des Sechszylinders,
Dämpfung im Auto).

Messfahrt: 0 → 118 km/h in 6 s durch fünf Gänge; der stärkste Ton im Spektrum der WAV-Datei liegt jeweils beim
protokollierten Zündton (z. B. 149/143 Hz, 222/223 Hz, 199/203 Hz); Spitze 0,11, kein Übersteuern. 14 s Klang
rendern in unter einer Sekunde. Echtzeitausgabe auf dem Mac über das Standardgerät (44,1 kHz) gestartet. Gehört
habe ich den Klang nicht – die Prüfung ist rein messtechnisch.

## HUD und Gamepad (04.10.2026)

**HUD** (`engine/hud.rs`, `hud.wgsl`, Anordnung in `game/hud.rs` nach `hud.js`): eine instanzierte Pipeline im
Bildschirmraum für abgerundete Rechtecke, Ellipsen, Kreisbögen, Dreiecke und Schriftzeichen. Die Schrift ist der
gemeinfreie 8×8-Bitmapfont aus `font8x8` (MIT-Crate; Basic Latin und Latin‑1 mit Umlauten und ß), proportional
gesetzt, in ganzzahligen Pixelgrößen für scharfe Kanten, mit dunkler Kontur statt Kästen; €, →, ✓, ★, ☀, ☾ und
⚠ sind selbst gezeichnet, typografische Striche und Anführungszeichen werden auf ASCII abgebildet. Das Spiel legt
die Elemente in Basiseinheiten an (720 Zeilen), skaliert wird auf die Fensterhöhe. Angezeigt werden: Geld,
Wochentag und Uhrzeit (Sonne/Mond), Auftrag mit Restzeit (blinkt unter 15 s) und Zusatztext, Tacho mit
Drehzahlbogen, rotem Bereich, Strichen je 1000 U/min, km/h, Gang (R/D) und Schadensbogen, ESP/ABS-Zustand und
Kisten-/Schrott-Hinweis, Fahrzeugname und Technik nach dem Einsteigen (ein- und ausblendend), Zielpfeil am
Bildrand mit Entfernung bzw. wippende Marke über dem Ziel, Hinweise unten mittig (Ein-/Aussteigen,
Missionshinweise mit E statt A, Meldungen), Ladebalken beim Einladen, Briefing, Ergebnis mit Lohnabrechnung und
Ladeanzeige. Aufnahmen: [Tag](images/native/hud-day.png), [Nacht im Auto](images/native/hud-night.png).

**Gamepad** (`engine/pad.rs` über `gilrs`): erster verbundener Controller, Tastenflanken bleiben bis zum nächsten
Simulationsschritt erhalten. Belegung wie `input.js`: linker Stick Gehen/Lenken (radiale Totzone 0,22), RT Gas,
LT Bremse/rückwärts, RB oder B Handbremse, X Hupe, Y Ein-/Aussteigen, A Aktion (halten: einladen; zu Fuß
sprinten), View Ton an/aus. Tastatur und Controller lassen sich mischen; ausgelenkte Sticks haben Vorrang.

`--im-auto` setzt den Spieler beim Start ins eigene Auto (für Aufnahmen und Tests).

**Noch offen:** Minikarte und große Karte, Pausenmenü und Titelbildschirm, Tastensymbole je Eingabegerät, Waffen-
und Lebensanzeige (keine Kämpfe portiert). Ein echter Controller wurde nicht angeschlossen; die
Belegung ist per Unit-Test geprüft, die gilrs-Anbindung nur durch einen Start ohne Controller.

### Validierung am 04.10.2026

Formatprüfung, Clippy mit `-D warnings` und alle 82 Rust-Tests erfolgreich. Neu: Schrift deckt deutschen Text ab,
Atlas hat die Glyphen, proportionale Breiten und Ausrichtung, Skalierung mit der Fensterhöhe; Formatierung von
Zeit und Geldbeträgen; Tastenflanken des Controllers; Mischung von Tastatur und Controller samt Totzone.
GPU-Aufnahmen bei Tag zu Fuß und nachts im Auto visuell kontrolliert.

## Wetter (04.10.2026)

**Simulation** (`sim/weather.rs`, Port von `weather.js`): elf Wetterarten in Dreistundenblöcken mit 45 Minuten
Überblendung, Tagestypen (normal, wechselhaft, Winter) aus Seed und Spieltag, Wolkenzug, Böen, Blitzeinschläge und
Donner als reine Funktionen von Seed und Spielzeit, Temperatur je Tag und Uhrzeit. Die Welt schreibt Nässe,
Schneedecke und Glätte fort (`World::step_weather`), die Reifenhaftung kommt aus `traction.rs`. Sturmböen
schieben fahrende Autos quer (`World::gust_accel`, auf Brücken stärker); `World::road_warning` liefert das
Warnschild wie `roadWarning` (Aquaplaning, Glätte, Schnee, Sturm, Nässe).

**Bild** (`game/weatherfx.rs`, vereinfacht gegenüber `wetfx.js`): Das Tageslicht wird mit `weather_light`
gedämpft, sodass Wolken die Schatten nehmen und Regen und Nebel den Tag grau machen. Regenstriche fallen schräg
nach dem Wind, Schneeflocken pendeln. Beide entstehen aus Hashes und Spielzeit, ohne Partikellisten. Nebel legt
einen Schleier über das Bild, Blitze hellen Szene und Bild kurz auf. Das HUD zeigt Wetter und Temperatur neben
der Uhr und das Warnschild links vom Tacho.

**Klang:** Regen auf dem Dach im Auto, Regenrauschen, Tropfen und Wind mit Böenpfeifen in der Umgebung. Donner
grollt mit Hüllkurve und Filtergleiten, nahe Einschläge krachen.

**Bedienung:** `--wetter ART` legt ein Wetter fest. Erlaubt sind clear, cloudy, overcast, rain, heavyrain, storm,
thunder, fog, densefog, snow und heavysnow. Taste N schaltet vom Tagesverlauf durch alle Arten und zurück.

Aufnahmen: [Starkregen](images/native/weather-heavyrain.png), [Schneesturm](images/native/weather-heavysnow.png).

**Noch offen:** Diese Effekte aus `wetfx.js` fehlen noch:
- nasse Straßen mit Pfützen und Spiegelungen
- Schneedecke und Matsch am Boden sowie Reifenspuren
- Wolkenschatten und Bodennebel als Rauschmuster
- Regenvorhänge
- Leuchtreklame

### Validierung am 04.10.2026

Formatprüfung, Clippy mit `-D warnings` und alle 86 Rust-Tests sind erfolgreich. Neu geprüft:
- Wetterfunktionen gegen Referenzwerte der JS-Fassung
- verschneite Straße mit Warnschild und stumpfen Reifen im eigenen Auto, danach Trocknen bei klarem Wetter
- 90 s Gewitter mit Donner und Regen im Klangmix

GPU-Aufnahmen bei Starkregen, Schneesturm und dichtem Nebel wurden visuell kontrolliert.
