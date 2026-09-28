# GTA Berlin

Spielbares Top-down-Open-World-Spiel mit schräger Draufsicht in **ganz Berlin, 1:1 aus OpenStreetMap**:
echte Straßen, Gebäude, Spree, Havel, Seen, Wälder, Parks und rund eine Million Bäume, dazu Verkehr, Passanten, fahrbare Autos und eine
vollständige Mission. Läuft im Browser (Entwicklung auf dem Mac)
und in einer UWP-Hülle für die **Xbox Series X|S im Developer Mode** (privat, Sideloading).

Alle Grafiken und Klänge sind selbst erzeugte Platzhalter (Canvas-Zeichnung, Web-Audio-Synthese) und austauschbar, siehe
[`web/assets/README.md`](web/assets/README.md). Keine Namen, Grafiken, Musik, Karten oder Dialoge aus fremden Spielen.

**Kartendaten © OpenStreetMap-Mitwirkende (ODbL)**, Bezirksgrenzen und Baumbestand: Geoportal Berlin (LOR 2021, Straßen- und Anlagenbäume; dl-de/zero-2.0). Die
Attribution steht im Titelbildschirm und auf dem Stadtplan. `web/data/berlin/` ist eine aus OSM abgeleitete Datenbank und
steht unter der ODbL; bei einer Veröffentlichung gilt Share-Alike für diese Dateien. Geschäfte im Spiel (Späti „Zum Kiez“,
„Lager 7“) sind erfunden.

> Hinweis zum Titel: „GTA“ ist eine Marke von Take-Two/Rockstar. Für ein privates Projekt auf der eigenen Konsole ist das
> unkritisch; vor einer Veröffentlichung sollte das Spiel umbenannt werden.

Technische Entscheidung, Quellen und offene Punkte: [`docs/TECHNIK.md`](docs/TECHNIK.md).

## Version

Aktuell **0.4.0** (Semantic Versioning; solange die Version mit `0.` beginnt, ist es ein Prototyp). Änderungen je
Version stehen in [`CHANGELOG.md`](CHANGELOG.md), die Version steht auch unten rechts im Titelbildschirm.

## Inhalt des Prototyps

- **Karte:** ganz Berlin (alle 12 Bezirke, 97 Ortsteile; Stadtgrenze aus den LOR-Prognoseräumen) im Maßstab 1:1
  (10 px = 1 m), ca. 46 × 38 km. 531 000 Gebäude mit echter Grundfläche und Höhe (Geschosszahl aus OSM), 323 000
  Straßenstücke mit Namen, Breite, Einbahnregeln und Brücken, Spree, Havel, Kanäle und Seen mit Uferwänden, Wälder, Parks,
  Friedhöfe, Kleingärten, Gleisanlagen, Hochbahnen (jedes Gleis einzeln und maßstäblich, wie in OSM erfasst).
  **Kein Baumstamm steht auf einer Fahrbahn** (die Krone darf überragen): OSM-Bäume
  auf der geschätzten Fahrbahnbreite rückt der Karten-Build an den Bordstein ihrer Straßenseite, ohne freien Platz
  entfallen sie; der Build bricht bei einem Verstoß ab, das Spiel verwirft solche Bäume beim Laden, Tests prüfen beides
  für jede Kachel von Berlin.
- **Nachladen:** Die Karte liegt in 2 920 Kacheln zu 640 × 640 m (zusammen 138 MB, größte Kachel 0,21 MB). Das Spiel
  lädt nur die Kacheln um die Kamera (bis 700 m) und gibt ferne wieder frei; im Browser bleiben so ca. 10–20 Kacheln und
  60–110 MB JS-Heap geladen. Fehlt beim Teleport oder nach dem Laden eines Spielstands noch ein Stadtteil, steht die Welt
  kurz still („Lade Stadtteil …“).
- **Straßenraum wie in echt:** Fahrbahnbreite Bordstein zu Bordstein (`width:carriageway`/`width`, in Berlin meist aus
  ALKIS), Fahrstreifen je Richtung, Parkstreifen je Seite (parallel, schräg, senkrecht, halb auf dem Gehweg) **mit geparkten
  Autos** (übernehmbar), Radfahrstreifen, Tempo 30/50 je Straße, Kopfsteinpflaster (optisch und fahrdynamisch),
  Kreuzungsflächen mit Eckradius, Markierungen (Mittellinie, Spurtrenner, Radstreifen, Parkstreifen).
- **Verkehrsregeln:** 4 032 Ampelkreuzungen mit Umlauf (die KI hält bei Rot an der Haltelinie), 12 883 Querungen
  (Zebrastreifen: die KI hält für Fußgänger, Passanten queren bevorzugt dort), 1 336 Abbiegeverbote, mehrspurige
  Hauptstraßen (rechts abbiegen von der rechten, links von der linken Spur). Der Spieler darf bei Rot fahren.
- **Zugänge:** 4 676 Tordurchfahrten in Hinterhöfe (Hauswand dort offen), Poller und Modalfilter (auch Diagonalsperren)
  sperren Straßen für Autos, Fußgänger kommen durch; rund 47 000 Zäune, Mauern und Hecken; 41 000 Hauseingänge als Türen.
- **Bäume aus dem Berliner Baumbestand:** alle 962 000 Straßen- und Anlagenbäume mit Gattung (Farbe/Form), Kronendurchmesser
  und Stammdicke; OSM-Bäume nur noch, wo das Kataster keinen Baum kennt.
- **POIs und Hausnummern:** rund 50 000 Orte aus OSM – U- und S-Bahnhöfe, Bushaltestellen, Einkaufszentren (Neukölln
  Arcaden), Supermärkte (Penny …), Läden, Restaurants, Bars, Cafés, Dienstleister, Kultur, Hotels – als Schilder über den
  Dächern, Bahnhöfe auch auf Minikarte und Stadtplan; im HUD steht der nächste Ort („Bar: …“). 415 000 Hausnummern:
  der Straßenname im HUD trägt die Nummer des nächsten Hauses („Karl-Marx-Straße 3“).
- **Vollständigkeit:** Der Karten-Build zählt jede Datenschicht je Bezirk (Straßen, gemessene Breiten, Parkstreifen,
  Tempo, Belag, Gebäude, Höhen, Bäume, POIs, Haltestellen, Bahnhöfe, Hausnummern, Ampeln, Querungen, Abbiegeverbote,
  Poller, Zäune, Durchfahrten, Türen, Kreuzungen) und gibt die Tabelle aus; ein Test verlangt jede Schicht in allen 12
  Bezirken.
- Die Stadtgrenze ist eine unsichtbare Wand; ein 250-m-Streifen außerhalb wird
  abgedunkelt dargestellt. Alles mit Kollisionen (Hauswände, Ufer, Gleise, Bäume).
- Spielfigur zu Fuß (gehen, sprinten), Auto: einsteigen, aussteigen, Gas, Bremse, Rückwärtsgang, Lenken, Handbremse/Drift,
  Hupe, Schaden bis zum Wrack, Bremsspuren, Funken, Rauch.
- Autos anderer Verkehrsteilnehmer fahren im Rechtsverkehr auf dem echten Straßennetz (Einbahnstraßen, Kurventempo je
  Abbiegewinkel, Tempo je Straßenklasse), bremsen vor Hindernissen, hupen, lösen Blockaden auf und fahren sich frei. Man
  kann sie auch übernehmen: der Fahrer flieht. Verkehr und Passanten leben im Umkreis um die Kamera (ca. 75–180 m) und
  werden dahinter abgebaut.
- Passanten gehen die Gehwege entlang der echten Straßen, biegen ab, überqueren Straßen (und warten auf fahrende Autos),
  fliehen vor Rasern, Hupen und Unfällen, stehen nach einem Anfahren wieder auf.
- **Tagesrhythmus und Stadtleben:** Die Uhr kennt Wochentage (neues Spiel: Freitag, 16:00; im HUD „Fr 16:00“). Wie viel
  Verkehr fährt, folgt Werktag (Berufsverkehr morgens und abends) und Wochenende (später, flacher) und dem Ort: gezählte
  Kfz je Werktag auf den Hauptstraßen (Verkehrsmengen 2019) und die Einwohnerdichte (Umweltatlas 2022) samt Läden in der
  Nähe bestimmen, wie belebt eine Gegend ist; Freitag- und Samstagnacht füllt das Nachtleben die Kieze mit Bars.
  Autos entstehen bevorzugt auf stark befahrenen Straßen. Menschen tun etwas an echten Orten: Wartende an Haltestellen,
  Raucher vor Bars, Schlangen vor Clubs in Partynächten, Leute mit Flasche vor Spätis, Gäste an Cafétischen,
  Plaudernde vor dem Imbiss, Schaufenstergucker, Straßenmusik am U-Bahnhof, Sitzende auf den 76 000 Bänken aus OSM,
  Gruppen auf Decken in Parks (nachmittags, am Wochenende mehr); dazu Jogger morgens und abends und Hundehalter.
  Fahrradständer mit wechselnd vielen Rädern und orange Mülleimer stehen an ihren echten Plätzen. Wer erschreckt wird,
  flieht und geht danach normal weiter. Neue Leute erscheinen nur außer Sicht, niemand verschwindet vor den Augen.
- **Fahrzeuge, Tiere, Geräusche:** Neben Pkw fahren LKW, Paketwagen (halten in zweiter Reihe mit Warnblinker), das
  orange Müllauto (werktags morgens, mit Müllwerkern) sowie Streifen- und Rettungswagen mit Blaulicht und
  Martinshorn: Liegt ein Toter auf der Straße, kommt ein Rettungswagen, fährt gezielt hin und nimmt ihn mit; Schüsse
  rufen die Polizei. Radfahrer und E-Roller fahren auf den Radstreifen bzw. am Fahrbahnrand und halten meist bei Rot;
  Leih-Roller stehen am Gehweg. Tauben fliegen auf, Enten schwimmen weg. Man hört Stadt, Verkehr, Vögel, Kneipen,
  Wasser, Hochbahn, Sirenen und zur vollen Stunde die Kirchenglocke.
- **Wetter und Nachtlichter:** Sonne, Wolken, Regen und Morgennebel wechseln im Drei-Stunden-Takt (für dieselbe Welt
  immer gleich; `?wetter=regen` legt es fest). Wolkenschatten ziehen über die Stadt; bei Regen wird der Asphalt nass und
  glänzt, Pfützen spiegeln Himmel und Lichter, Leute tragen Schirme, weniger sind draußen, Autos rutschen mehr. Nebel
  legt sich über alles. Fassaden sind auf der Sonnenseite heller. Nachts leuchten Neonschilder vor Bars, Clubs, Spätis
  und Imbissen.
- Mission „Kisten für den Kiez“: Auftrag am Späti in der Wrangelstraße (Wrangelkiez) annehmen → zur Lagerhalle in
  Neukölln fahren → dort anhalten und **A halten** zum Einladen → zurück zur Wrangelstraße → abliefern. Das Zeitlimit
  berechnet der Karten-Build aus der kürzesten Route (derzeit 8,7 km → 930 s). Scheitern bei Zeitablauf oder wenn das
  Auto mit der Ware zum Wrack wird. Lohn mit Zeitbonus und Schadensabzug, Bestzeit. Die Orte stehen in
  [`data/places.json`](data/places.json) und lassen sich austauschen.
- HUD: echter Straßenname (an Kreuzungen „A / B“, sonst Kiez oder Bezirk), Geld, Auftrag und Timer, Minikarte, Richtungspfeil mit Entfernung, Tempo und Fahrzeugzustand,
  Stadtplan von ganz Berlin auf der Ansicht-Taste (Mausrad zoomt bis auf Straßenebene, Ziehen verschiebt). Die
  Beschriftung folgt dem Maßstab wie bei kiez-finder: ganz Berlin mit den 12 Bezirken, näher die Ortsteile (Tegel,
  Prenzlauer Berg …), dann die Kiez-Namen aus OSM (Flughafenkiez, Reuterkiez … – 568 Kieze) und Bahnhöfe, ab etwa 3 m
  je Bildpunkt die Namen der Hauptstraßen und ab 2,6 m aller Straßen, entlang der Straße und ohne Überlappung.
  **Teleport:** Klick auf den Stadtplan, dann Bestätigungsdialog; man landet zu Fuß
  auf dem nächsten Gehweg, im Auto auf der nächsten Fahrspur in Fahrtrichtung, Verkehr und Passanten entstehen sofort am
  neuen Ort. Während eines laufenden Auftrags gesperrt (sonst wäre die Mission trivial). Tastensymbole wechseln zwischen Controller und Tastatur.
- **Tageszeit und Licht:** Die Spieluhr läuft beschleunigt (1 Echtsekunde = 1 Spielminute, ein Tag = 24 min, neues
  Spiel um 16:00, die Uhrzeit steht im HUD und im Spielstand). Die Sonne steht wie im Berliner Sommer (Aufgang 5:30,
  Untergang 20:30): Häuser und Bäume werfen Schatten in Sonnenrichtung, morgens und abends lang und warm, mittags kurz;
  Autos und Figuren haben einen mitwandernden Schatten. In der Dämmerung färbt sich das Licht golden, dann blau; nachts
  ist die Stadt dunkel: Straßenlaternen (Berliner Gaslaternen in Straßen mit Gasbeleuchtung) werfen Lichtflecken,
  ein Teil der Fenster ist erleuchtet, Ampeln und Schaufenster leuchten, fahrende Autos haben Scheinwerferkegel und
  Rücklichter, der Auftragsort leuchtet; Häuser verdecken, was dahinter am Boden leuchtet. Zum Anschauen:
  `http://localhost:8080/?uhr=21:30` stellt die Uhr jeder neuen Welt.
- **Boden mit Details:** Gehweg, Asphalt, Kopfsteinpflaster, Gras, Wald, Sand, Plätze und Gleisschotter haben
  Texturen; Bordsteine mit Rinnstein, Gullys, Kanaldeckel, Asphaltflicken, Risse und Ölflecken auf den Parkstreifen;
  Baumscheiben unter Straßenbäumen, Baumkronen als Blattballen, dunkleres Wasser am Ufer.
- **Häuser:** Dachformen nach Gebäudeart (Berliner Dach mit Ziegelrand, Flachdach mit Attika, Satteldach, Wellblech)
  mit Schornsteinen, Lichtschächten, Oberlichtern, Lüftungsgeräten, Solarmodulen und Dachterrassen; Fassaden als
  Altbau, Plattenbau, Neubau oder Industriebau; Kontaktschatten am Fassadenfuß.
- **Autos und Menschen:** fünf Automodelle (Kleinwagen, Limousine, Kombi, Transporter, Berliner Taxi) mit Scheiben,
  Spiegeln und lenkenden Vorderrädern, Blinker der KI vor dem Abbiegen, Rückfahrlicht; Passanten mit Armen und Beinen
  im Gang, verschiedener Kleidung und Haarfarbe, manche mit Rucksack oder Tasche.
- **Kämpfen:** zu Fuß Fäuste, Tritte, Baseballschläger, Messer, Pistole, Maschinenpistole und Schrotflinte (alle von
  Anfang an, Munition unbegrenzt mit Nachladen). Zielen in Blickrichtung mit Zielhilfe, mit der Maus auf den Zeiger.
  Kugeln stoppen an Hauswänden, beschädigen Autos bis zum Wrack (der Fahrer flieht); Getroffene fallen um, nach genug
  Treffern bleiben sie liegen, mit Blut. Schüsse vertreiben die Passanten ringsum. Etwa jeder siebte Passant wehrt
  sich mit den Fäusten. Die Spielfigur hat 100 Lebenspunkte (heilen nach einer Pause); bei 0 wacht man im nächsten
  echten Krankenhaus auf, 10 % des Geldes sind weg, ein laufender Auftrag scheitert.
- **Jedes Fensterformat:** Grundformat ist 16:9 (1280 × 720). Es wird so skaliert, dass es ganz ins Fenster passt;
  breitere oder höhere Fenster zeigen mehr Stadt, das HUD hängt an den Rändern, Menüs liegen mittig im 16:9-Rahmen.
- **Maus:** alle Menüs (Titel, Pause, Ergebnis, Steuerung) mit Zeigen und Klicken bedienbar, eigener Mauszeiger im
  Stil des Spiels (gelber Pfeil, heller Pfeil über Anklickbarem, Zielkreuz und Verschiebe-Pfeile auf dem Stadtplan);
  beim Fahren und Laufen blendet er sich aus, wenn die Maus 2 s ruht.
- Startmenü, Pause, Mission neu starten, Speichern (ein Speicherplatz, automatisch nach jedem erfüllten Auftrag), Fortsetzen.
  Alles komplett mit dem Controller bedienbar.

## Steuerung

| Aktion | Xbox-Controller | Tastatur |
|---|---|---|
| Laufen / Lenken | Linker Stick | WASD / Pfeiltasten |
| Sprinten | A halten oder Stick voll | Umschalt |
| Gas / Bremse, Rückwärts | RT / LT | W / S |
| Handbremse | RB oder B | Leertaste |
| Ein-/Aussteigen | Y | F |
| Aktion (Auftrag, Einladen, Abliefern) | A | E / Enter |
| Hupe | X | H |
| Stadtplan | Ansicht-Taste | M |
| Menüs | Steuerkreuz, A / B | Maus: zeigen wählt aus, Klick bestätigt; Tastenhinweise (A/B) sind anklickbar |
| Stadtplan zoomen / verschieben | – | Mausrad / Ziehen |
| Teleport (auf dem Stadtplan) | – | Mausklick auf die Karte, dann Ja/Nein (Maus, A/Enter, B/Esc) |
| Angreifen / Schießen (zu Fuß) | RT | linke Maustaste oder Strg |
| Zielen (zu Fuß) | rechter Stick | Maus (Figur zielt auf den Zeiger) |
| Treten (zu Fuß) | B | V |
| Waffe wechseln / wählen | LB / RB | Q, Mausrad / 1–6 |
| Nachladen | X | R |
| Pause | Menü-Taste | Esc / P |
| Menüs | Steuerkreuz/Stick, A wählen, B zurück | Pfeile, Enter, Esc |

## Auf dem Mac spielen und testen

Voraussetzung: Node.js ≥ 20. Keine weiteren Abhängigkeiten, kein `npm install` nötig.

```bash
npm start          # Dev-Server auf http://localhost:8080 (anderer Port: PORT=9000 npm start)
npm test           # 185 Tests: Kartenpipeline, Karte, Kollision, Fahrphysik, Verkehr, Passanten, Mission, Speichern, Menüs, Eingabe
```

### Karte neu erzeugen

Die fertige Karte liegt in `web/data/berlin/` (`index.json` mit Grenzen, Ortsteilen und Missionsorten, `overview.json`
für den Stadtplan, `tiles/<x>_<y>.json` je 640 × 640 m; zusammen 138 MB) im Repository; dafür ist kein Netz nötig.
Neu bauen, z. B. für aktuellere OSM-Daten oder andere Missionsorte:

```bash
npm run map:fetch                     # OSM-Auszug Berlin (Geofabrik, PBF ~100 MB), LOR-Grenzen, Baumbestand, Einwohnerdichte und Verkehrsmengen (WFS Geoportal Berlin) → data/raw/ (gitignored, ~2 min)
npm run map:build                     # data/raw/ + data/places.json → web/data/berlin/ (deterministisch, ~40 s, braucht ~6 GB Arbeitsspeicher)
npm run map:preview -- out.svg        # Sichtprüfung als SVG (Standard 4 × 4 km um den Späti; Ausschnitt: out.svg x y breite höhe in px)
```

Der Build liest die PBF-Datei mit einem eigenen Leser (`tools/osm/pbf.mjs`, ohne Abhängigkeiten) und gibt am Ende die
Abdeckung je Bezirk aus.

`map:build` bricht mit einer Meldung ab, wenn ein Missionsort in einem Haus, außerhalb des Gebiets oder ohne
Straßenverbindung liegt. Maßstab ändern: `node tools/osm/build.mjs --scale 5` (dann auch `PX_PER_M` in
`web/src/config.js` anpassen).

Ein Xbox-Controller am Mac (USB/Bluetooth) funktioniert im Browser über die Web-Gamepad-API. `file://` geht nicht, weil
ES-Module einen HTTP-Server brauchen.

Die Tests enthalten einen **Autopiloten**, der die komplette Mission über dieselben abstrakten Eingaben durchspielt wie ein
Spieler (annehmen, einsteigen, zur Lagerhalle fahren, einladen, zurückfahren, abliefern), sowie einen 90-Sekunden-Dauertest
mit Verkehr und Passanten.

## Projektaufbau

```
web/                 das Spiel (statisch, läuft so im Browser und in der Xbox-Hülle)
  index.html
  src/config.js      alle Spielkonstanten
  data/berlin/       die Karte (aus tools/osm/build.mjs): index.json, overview.json, tiles/
  src/map.js         Kacheln nachladen und freigeben, Raster-Hashes, Untergrund, Straßennamen
  src/geom.js        Polylinien/Polygone (geteilt mit dem Karten-Build)
  src/citycodes.js   Klassen-Codes des Kartenformats
  src/collision.js   Kreis / Rechteck / gedrehte Box / Wandsegment (SAT), Raster-Hash
  src/car.js         Fahrphysik, Schaden
  src/roadgraph.js   Fahrspurgraph aus dem Straßennetz
  src/traffic.js     Verkehrs-KI (Spur folgen, Abbiegen, Hindernisse)
  src/pedestrians.js Passanten auf den Gehwegen
  src/mission.js     Mission als Zustandsmaschine
  src/world.js       Simulationsschritt (ohne DOM)
  src/game.js        Bildschirme und Menüs (ohne DOM)
  src/save.js        Spielstand
  src/input.js       Tastatur, Web-Gamepad, Controller-Daten aus der Xbox-Hülle
  src/render.js      Welt-Rendering (schräge Draufsicht)
  src/daylight.js    Tageslicht aus der Spieluhr (Sonnenstand, Umgebungslicht; ohne Canvas)
  src/lighting.js    Schattenwurf und Lichtkarte (eigene Bildschirm-Ebenen)
  src/lamps.js       Standorte der Straßenlaternen (ohne Canvas)
  src/decals.js      Gullys, Kanaldeckel, Flicken, Risse, Ölflecken je Straße (ohne Canvas)
  src/textures.js    Bodentexturen als Muster
  src/roofs.js       Dachform, Fassadenstil und Dachaufbauten je Gebäude (ohne Canvas)
  src/vehicles.js    Automodelle, Sprite-Cache, Räder, Licht, Blinker
  src/combat.js      Waffen, Zielhilfe, Schüsse (Strahltest), Nahkampf, Treffer (ohne DOM)
  src/hud.js         HUD, Menüs, Overlays (Title-Safe-Rand 5 %)
  src/audio.js       synthetisierte Klänge
  src/assets.js      Platzhaltergrafiken + Austausch per manifest.json
  assets/            manifest.json, eigene Sprites/Sounds
xbox/                UWP-Hülle (C#, WinUI 2 WebView2) für Visual Studio
data/places.json     Missionsorte (lat/lon bzw. OSM-Weg der Lagerhalle)
tools/osm/           Kartenpipeline: fetch.mjs, pbf.mjs (PBF-Leser), store.mjs, build.mjs, crosssection.mjs, tiles.mjs, preview.mjs, geo.mjs
tools/serve.mjs      Dev-Server
tools/prepare-xbox.mjs  kopiert web/ in die Hülle, erzeugt Paket-Logos
tests/               node:test
docs/TECHNIK.md      Entscheidung, Quellen, offene Punkte
```

## Auf die Xbox bringen: Schritt für Schritt

Kurzfassung: **Das Paket muss auf einem Windows-Rechner gebaut und signiert werden.** Auf macOS gibt es dafür weder
MSBuild für UWP noch SignTool. Eine Windows-11-ARM-VM auf dem Mac (Parallels oder UTM) reicht voraussichtlich; ungetestet.

### 1. Xbox in den Developer Mode versetzen

1. Unter <https://storedeveloper.microsoft.com> ein Entwicklerkonto anlegen. Für Einzelpersonen seit September 2025
   kostenlos (Ausweis und Selfie zur Verifikation).
2. Auf der Xbox die App **„Xbox Dev Mode“** aus dem Store installieren und starten. Sie zeigt einen Aktivierungscode.
3. Im Partner Center unter <https://partner.microsoft.com/xboxconfig/devices> den Code eingeben.
4. In der Dev-Mode-App **„Switch and restart“**. Die Konsole startet in **Dev Home**. (Zurück in den Normalbetrieb geht
   es über Dev Home; im Dev Mode laufen keine Retail-Spiele.)
5. In Dev Home unter *Remote Access Settings* **Xbox Device Portal** aktivieren und Benutzername/Passwort setzen.
   Die Adresse steht dort, üblicherweise `https://<xbox-ip>:11443`.
6. Im Device Portal: *Settings → Preference Settings → „Treat UWP apps as games by default“* einschalten
   (Spiel-Ressourcen: 5 GB statt 1 GB Speicher).

### 2. Windows-Rechner vorbereiten (einmalig)

1. Visual Studio 2022 (Community reicht) mit dem Workload **„WinUI application development“** und darin den
   **„Universal Windows Platform tools“** (in älteren Installern: Workload „Universal Windows Platform development“)
   sowie einem Windows-11-SDK (10.0.22621 oder neuer).
2. Windows: *Einstellungen → System → Für Entwickler → Entwicklermodus* einschalten.
3. Node.js ≥ 20 installieren.

### 3. Paket erstellen und signieren

```powershell
git clone <dieses-repo> gta-berlin
cd gta-berlin
npm test                  # optional
node tools/prepare-xbox.mjs
start xbox\GtaBerlin.sln
```

In Visual Studio:

1. Oben **Release** und **x64** wählen (die Xbox kann nur x64).
2. `Package.appxmanifest` öffnen → Reiter *Packaging* → **Choose Certificate… → Create…**. Herausgeber `CN=GtaBerlinDev`
   (muss zum `Publisher` im Manifest passen). Es entsteht ein selbstsigniertes Testzertifikat (`.pfx`, von `.gitignore`
   ausgeschlossen).
3. Projekt rechts anklicken → **Publish → Create App Packages… → Sideloading**, kein automatisches Update, Architektur
   nur **x64**, Konfiguration **Release**. Ergebnis unter `xbox\GtaBerlin\AppPackages\GtaBerlin_0.1.x.0_Test\`:
   `GtaBerlin_…_x64.msix` (oder `.appx`), die `.cer`-Datei und der Ordner `Dependencies\x64\`.

Nach jeder Änderung am Spiel: erneut `node tools/prepare-xbox.mjs`, dann neu paketieren. Der Build bricht mit einer klaren
Meldung ab, wenn `Web\index.html` fehlt.

### 4a. Installieren über das Xbox Device Portal

1. Im Browser `https://<xbox-ip>:11443` öffnen (Zertifikatswarnung bestätigen), mit den Dev-Home-Zugangsdaten anmelden.
2. *Home* → **Add**.
3. Die `.msix`/`.appx` wählen. Häkchen für Abhängigkeiten setzen und **alle Dateien aus `Dependencies\x64\`** hinzufügen
   (u. a. `Microsoft.VCLibs…`, `Microsoft.UI.Xaml.2.8…`, `Microsoft.NET.Native.Framework…`, `Microsoft.NET.Native.Runtime…`),
   bei Nachfrage die `.cer`.
4. Installieren. **GTA Berlin** erscheint in Dev Home unter den installierten Apps.

### 4b. Alternative: direkt aus Visual Studio

1. In Dev Home **„Show Visual Studio pin“** aufrufen. Auf der Konsole muss ein Benutzer angemeldet sein (sonst Fehler
   `0x87e10008`).
2. In Visual Studio Zielgerät **Remote Machine**, Adresse der Xbox, Authentifizierung **Universal (Unencrypted Protocol)**.
3. **Debug → Start without Debugging** (bzw. F5). VS kümmert sich um Signatur und Abhängigkeiten, beim ersten Mal wird
   der PIN abgefragt.

### 5. Spiel starten

In Dev Home das Spiel auswählen und mit **A** starten. Im Hauptmenü mit dem Steuerkreuz wählen, **A** bestätigt.
„Beenden“ im Hauptmenü schließt die App (gibt es nur in der Xbox-Hülle).

Fehlersuche im Debug-Build: Die Hülle schaltet Remote-Debugging frei. Im Device Portal WebView2-Debugging öffnen bzw. am PC
in Edge `edge://inspect` mit der Konsole verbinden
([Anleitung](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/remote-debugging-xbox)).

## Stand und Prüfumfang

**Funktioniert und ist geprüft (auf dem Mac):**

- Alle 185 automatischen Tests grün, darunter:
  - die Kartenpipeline an einer kleinen künstlichen Stadt (Projektion, Grenzvereinigung, Straßengraph, Einbahnstraße,
    Kaimauer an der Brücke offen, Geländer, Gebäudehöhe, Missionsorte, Bäume an den Bordstein, POI-Kategorien,
    Bahnhof-/Adress-Dubletten, deterministischer Build);
  - Bäume: auf der ausgelieferten Karte kein Stamm auf einer Fahrbahn, in einem Haus oder im Wasser; das Spiel verwirft
    solche Bäume auch aus einer fremden Karte (gegengeprüft: ohne die Schutzfunktion schlagen die Tests fehl);
  - ganz Berlin: jede Kachel einzeln geladen, kein Baumstamm auf einer Fahrbahn (alle 1 030 333 Bäume), danach alles
    wieder entladen ohne Rückstände; jede Datenschicht in allen 12 Bezirken gefüllt; bekannte Orte in allen Bezirken
    (Alexanderplatz, Rathaus Spandau, Köpenick, Marzahn, Tegel, Zoo, Rathaus Steglitz …) im richtigen Ortsteil und Bezirk
    an der richtigen Straße; Kacheln fügen sich ohne doppelte Straßen oder Gebäude; der PBF-Leser an einer selbst
    erzeugten Datei;
  - Fensterformate von 640 × 360 bis 3440 × 1440 (auch hochkant): Minikarte, Auftrag und Tacho ganz sichtbar und
    ohne Überlappung, Menüeinträge und Klickflächen mittig im 16:9-Rahmen;
  - Stadtplan-Beschriftung: je Maßstab die richtigen Ebenen (Bezirke → Ortsteile → Kieze/Bahnhöfe → Straßennamen),
    Tegel, Prenzlauer Berg, Flughafenkiez, Karl-Marx-Straße, Flughafenstraße an ihren Orten, nichts überlappt oder liegt
    unter der Hinweisleiste, kein Text kopfüber (gegengeprüft: ohne Kollisionsprüfung schlägt der Test fehl);
  - Nachladen wie im Browser (Kacheln kommen asynchron): Welt steht, bis der Stadtteil da ist, Teleport nach Spandau lädt
    erst das Ziel, das alte Viertel wird danach freigegeben;
  - POIs: jeder U-/S-Bahnhof genau einmal, Neukölln Arcaden in Neukölln, Penny, Mindestzahlen je Kategorie;
    Hausnummern der Oranienstraße liegen an der Oranienstraße und erscheinen im Straßennamen;
  - die echte Karte: Oranienstraße/Sonnenallee/Kottbusser Damm/Karl-Marx-Straße sind durchgängig
    vorhanden, Brücken sind befahrbar, Kreuzungen nennen beide Straßen;
  - der vollständige Missionsablauf per Autopilot über die echte Route Wrangelstraße → Neukölln → zurück
    (ohne Verkehr, innerhalb von 930 s), Scheitern durch Zeitablauf und Totalschaden;
  - Vollgas gegen Hauswand, Kaimauer und Gebietsgrenze (jeder Schritt geprüft);
  - Tageslicht: Mittag hell mit kurzem Schatten, Mitternacht dunkel mit Laternen, morgens und abends lange Schatten in
    entgegengesetzte Richtungen, über 24 h stetig; die Spieluhr läuft 1 min je Sekunde, springt über Mitternacht und
    steht im Spielstand (alte Stände ohne Uhr bleiben gültig); Hausschatten gleich orientiert (sonst löschen sich
    Überlappungen aus), Schatten werfende Häuser außerhalb des Bildes werden gefunden; bei Tag Schatten ohne Lichtkarte,
    nachts genau eine Lichtkarte je Bild (für fünf dieser Prüfungen gegengeprüft, dass sie fehlschlagen, wenn man die
    Funktion abschaltet);
  - Straßenlaternen im Kerngebiet: keine auf einer Fahrbahn, in einem Haus oder im Wasser, Dichte je km plausibel,
    Ausleger über der Fahrbahn, Gaslaternen warm, Grundstückszufahrten keine Kreuzungen, deterministisch; Nachtfenster
    je Haus fest und abends zahlreicher als nachts; Laternen in der Lichtkarte, Häuser verdecken Bodenlicht nur in hoher
    Qualitätsstufe; Qualitätsstufe mit Hysterese (sechs Mutationsproben, alle erkannt);
  - Boden: über 20 000 Gullys, Kanaldeckel, Flicken, Risse und Ölflecken im Kerngebiet liegen alle auf der eigenen
    Fahrbahn und außerhalb der Kreuzungsfläche, auf Pflaster keine Flicken, Ölflecken nur mit Parkstreifen,
    deterministisch; jede Bodentextur genau einmal je Zeichenfläche gemalt, ohne Canvas einfarbiger Rückfall (fünf
    Mutationsproben, alle erkannt);
  - Dächer: über 50 000 Dachaufbauten im Kerngebiet liegen ganz im Grundriss, überlappen nicht und folgen der
    Hauptachse; Dachform und Fassade passen zur Gebäudeart (Kirchen Satteldach, Industrie Wellblech, Satteldach nur bis
    9 m, Plattenbau nur über 24 m), einmal je Haus bestimmt und deterministisch (fünf Mutationsproben, alle erkannt);
  - Autos und Menschen: Modell je Auto fest, alle fünf Modelle kommen vor, Taxis elfenbein, Spielerauto Limousine;
    die KI blinkt vor dem Rechts- bzw. Linksabbiegen richtig, geradeaus und weit vor der Kreuzung nicht; Sprite-Cache
    bleibt begrenzt, ohne Canvas flacher Rückfall; Aussehen der Passanten je Person fest und vielfältig (sechs
    Mutationsproben, alle erkannt);
  - Kampf: Faust trifft vorn, nicht hinten, Tritt trifft; drei Pistolentreffer töten, Tote liegen und verschwinden erst
    später außer Sicht; Kugeln stoppen an Hauswänden; Schrotflinte fächert 8 Kugeln, MP feuert Dauerfeuer und lädt
    nach, Pistole nur Einzelfeuer; Zielhilfe wählt das nächste Ziel im Kegel; beschossene Autos werden zum Wrack, der
    Fahrer flieht; Schüsse erschrecken Passanten; im Auto kein Schießen; Verkehr fährt über Tote; die neuen Tasten sind
    verdrahtet (W feuert nicht); Strahltests; Waffenfeld passt in jedes Fensterformat (acht Mutationsproben, alle
    erkannt); Gegenwehr (wer sich wehrt, schlägt zurück, andere fliehen, Kämpfer in der Nähe mischen mit), Heilen nach
    der Pause, K. o. mit Neustart am nächsten Krankenhaus samt Geldabzug und gescheitertem Auftrag, Anfahren verletzt,
    im Auto und am Boden keine weiteren Treffer (neun Mutationsproben, alle erkannt);
  - Tagesrhythmus und Stadtleben: Berufsverkehr, ruhige Nacht, Wochenende später, alle Kurven über eine Woche stetig;
    Nachtleben Freitag/Samstag voll, die Nacht zählt bis 6 Uhr zum Vortag; am Späti nachts weniger Autos als im
    Berufsverkehr und Freitagnacht mehr Menschen als Dienstagnacht, Zielwerte in Grenzen; Verkehrsmengen landen im
    Build auf der passenden Kante und nicht auf der Querstraße, Dichteraster spart Innenhöfe aus; Tätigkeiten je Ort
    und Uhrzeit (keine Clubschlange am Dienstag, kein Café nachts); Plätze deterministisch, nie im Haus, Gruppen nicht
    auf der Fahrbahn, Liegende nur auf Wiesen, Sitzende nur auf Bänken; neue Leute nur außer Sicht, niemand verschwindet
    im Bild, Erschreckte geben ihren Platz auf; feste Bevölkerung (Tests, Demo) ohne Rhythmus; Wochentag wechselt um
    Mitternacht und steht im Spielstand (14 Mutationsproben, alle erkannt);
  - Fahrzeuge, Einsätze, Räder, Tiere, Klang: Müllauto nur werktags morgens in Wohnstraßen, Paketwagen nicht sonntags,
    LKW langsamer als Pkw; hinter LKW und Müllauto kein Auffahrunfall und Abstand zwischen den Stoßstangen; Paketwagen
    hält mit Warnblinker, der Verkehr wartet, danach geht es weiter; Zielfahrt erreicht über das Entfernungsfeld den
    Einsatzort; ein Toter ruft einen Rettungswagen, der außer Sicht entsteht, mit Martinshorn anfährt, am Einsatzort
    hält, den Toten mitnimmt und mit Sondersignal abfährt; Schüsse rufen genau einen Streifenwagen; Radfahrer nur auf
    der rechten Spur, auf dem Radstreifen, wo es einen gibt, nie auf Hauptstraßen ohne Radstreifen, in 90 s Verkehr immer
    auf der Fahrbahn und nie festgefahren, stürzen, wenn ein Auto sie trifft, und halten meist bei Rot; Leih-Roller nur
    auf dem Gehweg; Tauben nie im Haus, Enten nur im Wasser, Tauben fliegen auf und kommen nur außer Sicht wieder;
    Vogelgesang nach Tageszeit, Kneipengemurmel, Martinshorn tatü-tata, Glocke zur vollen Stunde nur bei einer Kirche;
    Umgebungsschichten stumm, bis eine Mischung kommt; neue Fahrzeuge, Blaulicht, Räder und Tiere zeichnen ohne
    ungültige Koordinaten (25 Mutationsproben, alle erkannt);
  - Wetter: deterministisch, minütlich stetig über zehn Tage samt Mitternacht, Nebel nur morgens, alle Wetterbilder
    kommen vor, Regenanteil plausibel; Boden schnell nass und in 10 min trocken; Wolken nehmen Schatten, lösen tags aber
    keine Lichtkarte aus, Regen und Nebel schon; ohne Tagesrhythmus immer klar (auch gleich beim Erzeugen); Tagnummer
    läuft über Mitternacht und steht im Spielstand; bei Regen weniger Menschen; nasses Auto rutscht weiter; Schirme erst
    ab echtem Regen und je Person fest; über 15 000 Pfützen, alle auf der Fahrbahn und nie unter Häusern; Wolken ziehen
    mit dem Wind; Leuchtreklame-Texte und Flackern; Fassadenlicht nach Sonnenstand; Regen im Klang; Zeichnen bei Regen
    in der Nacht und bei Nebel ohne ungültige Koordinaten, mit Leuchtreklame und Widerschein (19 Mutationsproben, alle
    erkannt);
  - Gebäude-Regeln im Build: Brückenpfeiler, Kreuzgänge, Dächer und schwebende Teile werden keine Häuser, Bauteile
    nur ohne umgebenden Umriss (das höchste einer Gruppe); an der echten Oberbaumbrücke stehen die Türme (≥ 30 m),
    kein Haus auf der Fahrbahn, ein Brückendeck unter dem POI, das über dem Wasser nicht als Wasser gilt; Krankenhäuser
    in allen 12 Bezirken (vier Mutationsproben, alle erkannt);
  - Verkehrsfluss: je 3 min an den engsten Stellen (Rixdorf, Wrangelkiez, Weserstraße) steht kein Auto über 90 s und es
    gibt kaum Zusammenstöße; auf Engstellen nie Gegenverkehr gleichzeitig, jedes Auto darauf ist eingetragen; vor einer
    belegten Kreuzung oder Engstelle hält die KI vor der Linie; Bewegungen, die sich nicht kreuzen, dürfen gleichzeitig
    in eine Kreuzung; Schlange an der roten Ampel ohne Auffahren; angefahrene Passanten stehen wieder auf (für sieben
    dieser Regeln gegengeprüft, dass der Test fehlschlägt, wenn man sie abschaltet);
  - ein 90-s-Dauertest mit 24 Autos und 55 Passanten (0,0 % der Stichproben neben der Fahrbahn, kein Auto in einer Hauswand,
    kein Passant in einem Gebäude) und ein Test, dass die Bevölkerung der Kamera folgt;
  - Teleport: nur bei offenem Stadtplan, außerhalb des Gebiets abgelehnt, Abbruch ändert nichts, Welt steht während des
    Dialogs, Ziel zu Fuß auf dem Gehweg bzw. im Auto auf der Fahrbahn, während eines Auftrags gesperrt;
  - Gleise maßstäblich (Spurweite 1435 mm, zwei Schienen je Gleis) und in Ebenen gezeichnet (Bett → Schwellen →
    Schienen über alle Gleise), damit parallele Gleise und Weichen sich nicht übermalen;
  - Straßenquerschnitt aus Tag-Kombinationen (Breite, Park-/Radstreifen, Spuren, Tempo, Belag, enge Straßen);
    Tordurchfahrt öffnet die Hauswand, Poller sperren eine Straße, Ampel wird der Kreuzung zugeordnet, Abbiegeverbot
    filtert die Folgespur, Baumkataster ersetzt den OSM-Baum; KI hält bei Rot und fährt bei Grün; geparkte Autos
    stehen auf dem Parkstreifen; Stichproben auf der echten Karte (Ampeln an Kottbusser Tor und Hermannplatz,
    Durchfahrten, Zebrastreifen, Kataster). Für acht dieser Schutzprüfungen wurde gegengeprüft, dass sie fehlschlagen,
    wenn man die jeweilige Funktion absichtlich abschaltet;
  - gestrichelte Straßenmarkierungen haben in jedem Bild den Strichversatz 0 (aufzeichnender Canvas-Ersatz; vorher
    übernahmen sie den animierten Versatz des Missionskreises und „flossen“);
  - Speichern/Laden inkl. kaputter, alter (Rasterstadt) und ungültiger Positionen, Menüführung nur mit Controller-Aktionen.
- Im Browser (Chromium via Playwright): Titel mit Ladeanzeige, Spiel, HUD mit echtem Straßennamen, Minikarte,
  Stadtplan; Stichproben an Kottbusser Tor (U1-Hochbahn), Admiralbrücke, Hermannplatz, Kottbusser Damm. Volle
  Bildwiederholrate bei 1280×720 und 1920×1080 auch bei Höchsttempo (Frame-Abstand Median 10,0 ms, p95 10,9 ms),
  JS-Heap ca. 110 MB. Mit Straßenraum, 86 000 Bäumen und geparkten Autos (Stand 27.09.): Oranienplatz bei 1920×1080
  Median 8,3 ms je Bild, p95 9,3 ms, JS-Heap ca. 280 MB. Ganz Berlin mit Nachladen (Stand 27.09.): JS-Heap 45–110 MB;
  Teleport nach Spandau; Schnellfahrt über 12 km mit 1,2 km/s (36-fache Höchstgeschwindigkeit) ohne Ladepause, Bildzeit
  Median 8,3 ms, p99 10,3 ms, höchstens 17,9 ms; 0 Konsolenfehler.
- Die Controller-Brücke der Xbox-Hülle ist auf der **Web-Seite** getestet: Mit einer Attrappe von `chrome.webview`, die
  Lesungen im Format der C#-Hülle schickt, lief der Weg Menü → Spiel → Auftrag annehmen → Pause → B → Speichern →
  Hauptmenü → Beenden (schickt `quit` an die Hülle).
- Austausch einer Grafik über `manifest.json` (Test-Sprite wurde statt des Platzhalters gezeichnet).

**Geschrieben, aber hier nicht gebaut (braucht Windows + Visual Studio):**

- Die UWP-Hülle `xbox/` (C#, XAML, `.csproj`, Manifest). Sie wurde **nicht kompiliert**; kleinere Anpassungen beim ersten
  Build (z. B. NuGet-Versionen, SDK-Version) sind möglich.
- Es wurde **kein Xbox-Paket erstellt und nichts signiert**.

**Erst auf einer echten Xbox prüfbar:**

- Ob die Hülle startet und die WebView2 das Spiel lädt.
- Controller über `Windows.Gaming.Input` inkl. B-Taste (darf die App nicht schließen) und Fokus.
- Ton ohne Nutzergeste (Autoplay-Argument), Spielstand über Neustarts, Bildrate und Latenz auf der Konsole,
  Darstellung auf dem Fernseher (Title-Safe-Rand). Ladezeit der Kacheln und Speicher auf der Konsole.

**Bekannte Grenzen der Karte:** Straßenbreiten sind aus OSM geschätzt (`width`/`lanes`, sonst Standard je Straßenklasse),
die Ampeln laufen mit einem festen Zwei-Phasen-Umlauf statt echter Signalpläne, Diagonalsperren sind für die KI ganz
gesperrt (erlaubte Abbiegungen dort meidet sie), Gebäude ohne Geschossangabe bekommen eine Standardhöhe (16 m, Einfamilienhäuser
8 m), und nur rund ein Fünftel des Hauptnetzes hat eine gemessene Breite (der Rest ist aus Spuren, Park- und
Radstreifen berechnet). Die Mission bleibt in Kreuzberg/Neukölln; der Rest der Stadt ist frei befahrbar.

Details und Quellen: [`docs/TECHNIK.md`](docs/TECHNIK.md).
