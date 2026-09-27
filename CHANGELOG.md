# Changelog

Alle nennenswerten Änderungen an GTA Berlin. Format nach [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionen nach [Semantic Versioning](https://semver.org/lang/de/). Solange die Version mit `0.` beginnt, ist das Spiel
ein Prototyp: Spielstände, Kartenformat und Steuerung können sich zwischen Versionen ändern.

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
