# GTA Berlin

Spielbares Top-down-Open-World-Spiel mit schräger Draufsicht in **Kreuzberg und Nord-Neukölln, 1:1 aus OpenStreetMap**:
echte Straßen, Gebäude, Spree, Landwehrkanal, Parks und Bäume, dazu Verkehr, Passanten, fahrbare Autos und eine
vollständige Mission. Läuft im Browser (Entwicklung auf dem Mac)
und in einer UWP-Hülle für die **Xbox Series X|S im Developer Mode** (privat, Sideloading).

Alle Grafiken und Klänge sind selbst erzeugte Platzhalter (Canvas-Zeichnung, Web-Audio-Synthese) und austauschbar, siehe
[`web/assets/README.md`](web/assets/README.md). Keine Namen, Grafiken, Musik, Karten oder Dialoge aus fremden Spielen.

**Kartendaten © OpenStreetMap-Mitwirkende (ODbL)**, Bezirksgrenzen und Baumbestand: Geoportal Berlin (LOR 2021, Straßen- und Anlagenbäume; dl-de/zero-2.0). Die
Attribution steht im Titelbildschirm und auf dem Stadtplan. `web/data/city.json` ist eine aus OSM abgeleitete Datenbank und
steht unter der ODbL; bei einer Veröffentlichung gilt Share-Alike für diese Datei. Geschäfte im Spiel (Späti „Zum Kiez“,
„Lager 7“) sind erfunden.

> Hinweis zum Titel: „GTA“ ist eine Marke von Take-Two/Rockstar. Für ein privates Projekt auf der eigenen Konsole ist das
> unkritisch; vor einer Veröffentlichung sollte das Spiel umbenannt werden.

Technische Entscheidung, Quellen und offene Punkte: [`docs/TECHNIK.md`](docs/TECHNIK.md).

## Inhalt des Prototyps

- **Karte:** Kreuzberg (LOR-Prognoseräume Kreuzberg Nord/Süd/Ost) und Nord-Neukölln (Prognoseraum 0810 „Neukölln“:
  Schillerpromenade, Neuköllner Mitte, Reuterstraße, Rixdorf, Köllnische Heide) im Maßstab 1:1 (10 px = 1 m), ca.
  8 × 6 km. Rund 27 500 Gebäude mit echter Grundfläche und Höhe (Geschosszahl aus OSM), 29 500 Straßenstücke mit Namen,
  Breite, Einbahnregeln und Brücken, Spree und Kanäle mit Kaimauern, Parks, Friedhöfe, Kleingärten, Gleisanlagen,
  U1-Hochbahn (jedes Gleis einzeln und maßstäblich, wie in OSM erfasst; mehrere Linien teilen sich dieselben Gleise),
  42 000 Straßenbäume. **Kein Baumstamm steht auf einer Fahrbahn** (die Krone darf überragen): OSM-Bäume
  auf der geschätzten Fahrbahnbreite rückt der Karten-Build an den Bordstein ihrer Straßenseite, ohne freien Platz
  entfallen sie; der Build bricht bei einem Verstoß ab, das Spiel verwirft solche Bäume beim Laden, Tests prüfen beides.
- **Straßenraum wie in echt:** Fahrbahnbreite Bordstein zu Bordstein (`width:carriageway`/`width`, in Berlin meist aus
  ALKIS), Fahrstreifen je Richtung, Parkstreifen je Seite (parallel, schräg, senkrecht, halb auf dem Gehweg) **mit geparkten
  Autos** (übernehmbar), Radfahrstreifen, Tempo 30/50 je Straße, Kopfsteinpflaster (optisch und fahrdynamisch),
  Kreuzungsflächen mit Eckradius, Markierungen (Mittellinie, Spurtrenner, Radstreifen, Parkstreifen).
- **Verkehrsregeln:** 550 Ampelkreuzungen mit Umlauf (die KI hält bei Rot an der Haltelinie), 1 661 Querungen
  (Zebrastreifen: die KI hält für Fußgänger, Passanten queren bevorzugt dort), 183 Abbiegeverbote, mehrspurige
  Hauptstraßen (rechts abbiegen von der rechten, links von der linken Spur). Der Spieler darf bei Rot fahren.
- **Zugänge:** 1 718 Tordurchfahrten in Hinterhöfe (Hauswand dort offen), Poller und Modalfilter (auch Diagonalsperren)
  sperren Straßen für Autos, Fußgänger kommen durch; rund 9 000 Zäune, Mauern und Hecken; 5 857 Hauseingänge als Türen.
- **Bäume aus dem Berliner Baumbestand:** 76 000 Straßen- und Anlagenbäume mit Gattung (Farbe/Form), Kronendurchmesser
  und Stammdicke; OSM-Bäume nur noch, wo das Kataster keinen Baum kennt.
- **POIs und Hausnummern:** rund 8 700 Orte aus OSM – U- und S-Bahnhöfe, Bushaltestellen, Einkaufszentren (Neukölln
  Arcaden), Supermärkte (Penny …), Läden, Restaurants, Bars, Cafés, Dienstleister, Kultur, Hotels – als Schilder über den
  Dächern, Bahnhöfe auch auf Minikarte und Stadtplan; im HUD steht der nächste Ort („Bar: …“). 25 000 Hausnummern:
  der Straßenname im HUD trägt die Nummer des nächsten Hauses („Karl-Marx-Straße 3“).
- Die Bezirksgrenze ist eine unsichtbare Wand; ein 250-m-Streifen außerhalb wird
  abgedunkelt dargestellt. Alles mit Kollisionen (Hauswände, Ufer, Gleise, Bäume).
- Spielfigur zu Fuß (gehen, sprinten), Auto: einsteigen, aussteigen, Gas, Bremse, Rückwärtsgang, Lenken, Handbremse/Drift,
  Hupe, Schaden bis zum Wrack, Bremsspuren, Funken, Rauch.
- Autos anderer Verkehrsteilnehmer fahren im Rechtsverkehr auf dem echten Straßennetz (Einbahnstraßen, Kurventempo je
  Abbiegewinkel, Tempo je Straßenklasse), bremsen vor Hindernissen, hupen, lösen Blockaden auf und fahren sich frei. Man
  kann sie auch übernehmen: der Fahrer flieht. Verkehr und Passanten leben im Umkreis um die Kamera (ca. 75–180 m) und
  werden dahinter abgebaut.
- Passanten gehen die Gehwege entlang der echten Straßen, biegen ab, überqueren Straßen (und warten auf fahrende Autos),
  fliehen vor Rasern, Hupen und Unfällen, stehen nach einem Anfahren wieder auf.
- Mission „Kisten für den Kiez“: Auftrag am Späti in der Wrangelstraße (Wrangelkiez) annehmen → zur Lagerhalle in
  Nord-Neukölln fahren → dort anhalten und **A halten** zum Einladen → zurück zur Wrangelstraße → abliefern. Das Zeitlimit
  berechnet der Karten-Build aus der kürzesten Route (derzeit 8,6 km → 760 s). Scheitern bei Zeitablauf oder wenn das
  Auto mit der Ware zum Wrack wird. Lohn mit Zeitbonus und Schadensabzug, Bestzeit. Die Orte stehen in
  [`data/places.json`](data/places.json) und lassen sich austauschen.
- HUD: echter Straßenname (an Kreuzungen „A / B“, sonst Kiez oder Bezirk), Geld, Auftrag und Timer, Minikarte, Richtungspfeil mit Entfernung, Tempo und Fahrzeugzustand,
  Stadtplan auf der Ansicht-Taste. **Teleport:** Klick auf den Stadtplan, dann Bestätigungsdialog; man landet zu Fuß
  auf dem nächsten Gehweg, im Auto auf der nächsten Fahrspur in Fahrtrichtung, Verkehr und Passanten entstehen sofort am
  neuen Ort. Während eines laufenden Auftrags gesperrt (sonst wäre die Mission trivial). Tastensymbole wechseln zwischen Controller und Tastatur.
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
| Teleport (auf dem Stadtplan) | – | Mausklick auf die Karte, dann Ja/Nein (Maus, A/Enter, B/Esc) |
| Pause | Menü-Taste | Esc / P |
| Menüs | Steuerkreuz/Stick, A wählen, B zurück | Pfeile, Enter, Esc |

## Auf dem Mac spielen und testen

Voraussetzung: Node.js ≥ 20. Keine weiteren Abhängigkeiten, kein `npm install` nötig.

```bash
npm start          # Dev-Server auf http://localhost:8080 (anderer Port: PORT=9000 npm start)
npm test           # 73 Tests: Kartenpipeline, Karte, Kollision, Fahrphysik, Verkehr, Passanten, Mission, Speichern, Menüs, Eingabe
```

### Karte neu erzeugen

Die fertige Karte liegt als `web/data/city.json` (9,7 MB, gzip ca. 3,2 MB) im Repository; dafür ist kein Netz nötig.
Neu bauen, z. B. für aktuellere OSM-Daten oder andere Missionsorte:

```bash
npm run map:fetch                     # LOR-Grenzen + Baumbestand (WFS Geoportal Berlin) + OSM (Overpass, 2 Abfragen, Ausweich-Server) → data/raw/ (~100 MB, gitignored)
npm run map:build                     # data/raw/ + data/places.json → web/data/city.json (deterministisch, ~3 s)
npm run map:preview -- out.svg        # Sichtprüfung als SVG (optional Ausschnitt: out.svg x y breite höhe in px)
```

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
  data/city.json     die Karte (aus tools/osm/build.mjs)
  src/map.js         Karte dekodieren, Raster-Hashes, Untergrund, Straßennamen
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
  src/hud.js         HUD, Menüs, Overlays (Title-Safe-Rand 5 %)
  src/audio.js       synthetisierte Klänge
  src/assets.js      Platzhaltergrafiken + Austausch per manifest.json
  assets/            manifest.json, eigene Sprites/Sounds
xbox/                UWP-Hülle (C#, WinUI 2 WebView2) für Visual Studio
data/places.json     Missionsorte (lat/lon bzw. OSM-Weg der Lagerhalle)
tools/osm/           Kartenpipeline: fetch.mjs, build.mjs, preview.mjs, geo.mjs
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

- Alle 73 automatischen Tests grün, darunter:
  - die Kartenpipeline an einer kleinen künstlichen Stadt (Projektion, Grenzvereinigung, Straßengraph, Einbahnstraße,
    Kaimauer an der Brücke offen, Geländer, Gebäudehöhe, Missionsorte, Bäume an den Bordstein, POI-Kategorien,
    Bahnhof-/Adress-Dubletten, deterministischer Build);
  - Bäume: auf der ausgelieferten Karte kein Stamm auf einer Fahrbahn, in einem Haus oder im Wasser; das Spiel verwirft
    solche Bäume auch aus einer fremden Karte (gegengeprüft: ohne die Schutzfunktion schlagen die Tests fehl);
  - POIs: jeder U-/S-Bahnhof genau einmal, Neukölln Arcaden in Nord-Neukölln, Penny, Mindestzahlen je Kategorie;
    Hausnummern der Oranienstraße liegen an der Oranienstraße und erscheinen im Straßennamen;
  - die echte Karte: bekannte Orte (Kottbusser Tor, Hermannplatz, Rathaus Neukölln, Mehringdamm) liegen im richtigen
    Bezirk an der richtigen Straße, Oranienstraße/Sonnenallee/Kottbusser Damm/Karl-Marx-Straße sind durchgängig
    vorhanden, Brücken sind befahrbar, Kreuzungen nennen beide Straßen;
  - der vollständige Missionsablauf per Autopilot über die echte Route Wrangelstraße → Nord-Neukölln → zurück
    (ca. 427 s von 760 s, ohne Verkehr), Scheitern durch Zeitablauf und Totalschaden;
  - Vollgas gegen Hauswand, Kaimauer und Gebietsgrenze (jeder Schritt geprüft);
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
  Median 8,3 ms je Bild, p95 9,3 ms, JS-Heap ca. 280 MB.
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
  Darstellung auf dem Fernseher (Title-Safe-Rand). Ladezeit und Speicher der 5,8-MB-Karte auf der Konsole.

**Bekannte Grenzen der Karte:** Straßenbreiten sind aus OSM geschätzt (`width`/`lanes`, sonst Standard je Straßenklasse),
die Ampeln laufen mit einem festen Zwei-Phasen-Umlauf statt echter Signalpläne, Diagonalsperren sind für die KI ganz
gesperrt (erlaubte Abbiegungen dort meidet sie), die Spree-Brücken zu Friedrichshain
(Oberbaumbrücke u. a.) liegen auf der Bezirksgrenze und sind darum nicht befahrbar, und Gebäude ohne Geschossangabe
bekommen eine Standardhöhe (16 m).

Details und Quellen: [`docs/TECHNIK.md`](docs/TECHNIK.md).
