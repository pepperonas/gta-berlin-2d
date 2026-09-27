# Changelog

Alle nennenswerten Änderungen an GTA Berlin. Format nach [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionen nach [Semantic Versioning](https://semver.org/lang/de/). Solange die Version mit `0.` beginnt, ist das Spiel
ein Prototyp: Spielstände, Kartenformat und Steuerung können sich zwischen Versionen ändern.

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
