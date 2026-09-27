# Technische Entscheidung: Weg auf die Xbox

Stand der Recherche: **27.09.2026**. Alle Quellen am selben Tag geprüft. Ein Teil der Microsoft-Seiten zu „UWP auf Xbox“ ist
inzwischen ins Archiv verschoben (Texte von 2017–2023). Sie sind trotzdem die aktuellste offizielle Beschreibung.

## Ausgangslage

- Das Repository war leer (kein Rust/Bevy, kein Bestand). Die Entscheidung konnte also frei fallen.
- Entwicklungsrechner: MacBook mit Apple Silicon.
- Ziel: eine normale Xbox Series X|S im **Developer Mode**, private Installation.

## Öffentlicher Weg vs. GDK

| | Retail-Konsole im Dev Mode + **UWP** | **Xbox GDK / GDKX** |
|---|---|---|
| Zugang | Partner-Center-Entwicklerkonto; für Einzelpersonen seit 10.09.2025 **kostenlos** | Konsolen-GDK (GDKX) nur unter NDA, z. B. über ID@Xbox |
| Freigabe nötig | nein | ja |
| Verteilung | nur Sideloading auf eigene Konsolen im Dev Mode | Store über ID@Xbox |
| Hier gewählt | **ja** | nein |

Das öffentliche GDK auf GitHub (`microsoft/GDK`) ist für **PC**-Spiele. Konsolenzugriff gibt es darüber nicht.

## Gewählte Architektur

**HTML5-Spiel (Canvas 2D, Web Audio) in einer C#-UWP-Hülle mit WebView2 (WinUI 2).**

- Der Spielkern ist reines JavaScript ohne Abhängigkeiten. Er läuft identisch im Browser auf dem Mac und in der WebView2
  auf der Xbox. So ist er vollständig auf dem Mac entwickel- und testbar (`npm test`, `npm start`).
- Die Hülle (`xbox/`) lädt die Spieldateien aus dem Paket über
  `CoreWebView2.SetVirtualHostNameToFolderMapping("gta-berlin.local", "Web", …)`, ohne Netzwerk.
- **Controller:** Die Web-Gamepad-API ist in UWP-WebView2 laut offenem Microsoft-Issue defekt
  ([WebView2Feedback #4366](https://github.com/MicrosoftEdge/WebView2Feedback/issues/4366), „Blocking“). Deshalb liest die
  Hülle den Controller nativ über `Windows.Gaming.Input.Gamepad` und schickt alle 8 ms eine Lesung per
  `PostWebMessageAsJson` an die Seite. `web/src/input.js` (`fromHostReading`) wandelt sie in ein Standard-Gamepad um.
  Funktioniert die Web-API doch, werden beide Quellen zusammengeführt (ODER bzw. stärkerer Ausschlag), ohne Doppelwirkung.
- **Xbox-Eigenheiten in der Hülle:**
  - `RequiresPointerMode = WhenRequested`: kein Maus-Cursor-Modus.
  - `BackRequested` wird abgefangen, sonst würde **B** die App schließen.
  - `VirtualKey.Gamepad*` wird abgefangen, sonst zieht die XY-Fokusnavigation den Fokus aus der WebView2
    ([#4284](https://github.com/MicrosoftEdge/WebView2Feedback/issues/4284)).
  - `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--autoplay-policy=no-user-gesture-required`: Controller-Eingaben aus der Hülle
    zählen nicht als Nutzergeste, Ton wäre sonst stumm. Die WinUI-2-Variante kennt keine `CoreWebView2EnvironmentOptions`,
    daher die Umgebungsvariable (dieselbe Technik nutzt Microsofts Remote-Debugging-Anleitung).
- **Spielstand:** `localStorage`, liegt in der Hülle im WebView2-Profil im App-Datenordner.

### Verworfene Alternativen

- **Rust/Bevy:** Rust hat nur ein Tier-3-Ziel für UWP (`x86_64-uwp-windows-msvc`), Bevy hat keinen UWP-/Xbox-Dev-Mode-Pfad.
  Deutlich mehr Risiko ohne Vorteil.
- **MonoGame:** UWP wurde in MonoGame 3.8.2 abgekündigt ([MonoGame #8406](https://github.com/MonoGame/MonoGame/issues/8406)).
- **JavaScript-UWP (WWAHost/EdgeHTML, `navigator.gamepadInputEmulation`):** Altbestand. Ob es auf aktueller Xbox-Firmware
  noch läuft, ist nicht belegt.
- **Natives C++/DirectX 11:** möglich, aber auf dem Mac weder bau- noch testbar und für einen 2D-Prototyp unverhältnismäßig.

## Ressourcen auf der Konsole

Laut [System resources for UWP apps and games](https://learn.microsoft.com/en-us/windows/uwp/xbox-apps/system-resource-allocation) (Archiv):

| | Speicher (Vordergrund) | CPU | GPU |
|---|---|---|---|
| App | 1 GB | geteilt | ca. 45 % geteilt |
| **Spiel** | **5 GB** | 4 exklusive + 2 geteilte Kerne | voll |

DirectX 11/12 mit Feature Level 11.0, nur x64. Damit die App als Spiel läuft: im Xbox Device Portal unter
*Settings → Preference Settings* **„Treat UWP apps as games by default“** aktivieren
([Device Portal for Xbox](https://learn.microsoft.com/en-us/windows/uwp/xbox-apps/device-portal-xbox), Archiv). Der Prototyp
braucht im Browser deutlich unter 200 MB, liefe also vermutlich auch im App-Modus.

## Bauen und Signieren: Windows ist Pflicht

- Visual Studio 2022 mit Workload **„Universal Windows Platform development“** / „WinUI application development“ inkl.
  UWP-Tools und ein Windows-SDK (Projekt: Ziel 10.0.22621, Minimum 10.0.17763). Auf dem PC Entwicklermodus einschalten
  ([Getting started with UWP on Xbox](https://learn.microsoft.com/en-us/windows/uwp/xbox-apps/getting-started), Archiv).
- Auf macOS gibt es weder MSBuild für UWP-XAML noch SignTool. `makemsix` (microsoft/msix-packaging) kann zwar packen,
  aber nicht signieren und keine UWP-App kompilieren. Eine **Windows-11-ARM-VM auf dem Mac** (Parallels/UTM) ist die
  pragmatische Option. Mit VS 2022 auf ARM64 für x64 zu bauen sollte gehen, ist hier aber **nicht getestet**.

## Installation

- Dev Mode aktivieren: Xbox-Dev-Mode-App aus dem Store, Code unter `partner.microsoft.com/xboxconfig/devices` eingeben
  ([Xbox One Developer Mode activation](https://learn.microsoft.com/en-us/windows/uwp/xbox-apps/devkit-activation), Archiv).
- Weg A: Visual Studio → *Remote Machine* → IP der Xbox, Authentifizierung „Universal (Unencrypted Protocol)“, PIN aus Dev
  Home. Auf der Konsole muss ein Benutzer angemeldet sein (sonst Fehler 0x87e10008).
- Weg B: Device Portal (`https://<xbox-ip>:11443`) → *Add* → `.msix`/`.appx` plus Abhängigkeiten (VCLibs, Microsoft.UI.Xaml,
  .NET Native Runtime/Framework aus dem `Dependencies\x64`-Ordner des Pakets) hochladen.

## Karte: ganz Berlin aus offenen Daten (Stand 27.09.2026)

**Quellen**

| Was | Quelle | Lizenz |
|---|---|---|
| Stadtgrenze, Bezirke | Geoportal Berlin, WFS `lor_2021` (`c_lor_pgr_2021`, alle 58 LOR-Prognoseräume 2021; Bezirk aus dem Feld `bez`) | Datenlizenz Deutschland – Zero 2.0 |
| Ortsteile (97), Straßen, Gebäude, Wasser, Grün, Gleise, Bäume, Kiez-Namen, POIs, Hausnummern, Ampeln, Querungen, Poller, Zäune, Abbiegeverbote | OpenStreetMap, Berlin-Auszug von Geofabrik (`berlin-latest.osm.pbf`, täglich, ~100 MB) | ODbL 1.0 |
| Straßen- und Anlagenbäume (Gattung, Höhe, Kronendurchmesser, Stammumfang), 962 545 Bäume | Geoportal Berlin, WFS `baumbestand` | Datenlizenz Deutschland – Zero 2.0 |

Warum Geofabrik statt Overpass: ganz Berlin ergäbe über Overpass mehrere GB JSON in Dutzenden Abfragen und sprengt die
Nutzungsgrenzen der öffentlichen Server (~1 GB/Tag). Der Auszug enthält dieselben OSM-Daten mit allen Tags; Ortsteile
kommen als `boundary=administrative`, `admin_level=10` mit.

**Pipeline** (`tools/osm/`, nur Node, keine Abhängigkeiten)

1. `fetch.mjs` lädt den PBF-Auszug, die LOR-Prognoseräume und den Baumbestand (seitenweise zu 20 000, nach `gisid`
   sortiert, damit die Seiten stabil sind) nach `data/raw/` (gitignored, ~160 MB, Abruf ~2 min).
2. `pbf.mjs` liest das PBF-Format selbst (Protocol Buffers von Hand, Blöcke mit `node:zlib`): 7,9 Mio. Knoten in ~5 s.
   `store.mjs` hält die Koordinaten aller Knoten in typisierten Feldern (nach ID sortiert, Suche per Halbierung, ~130 MB)
   und nur getaggte Knoten, relevante Wege und Relationen als Objekte.
3. `build.mjs` projiziert WGS84 mit einer transversalen Mercator-Projektion (Krüger-Reihen, GRS80, Mittelmeridian =
   Stadtmitte; Längenfehler am Stadtrand < 10⁻⁵) in Meter und dann in Spiel-Pixel (10 px = 1 m). Norden ist oben.
   - Grenze: die 58 LOR-Polygone werden vereinigt, indem gemeinsame Kanten wegfallen (die Geoportal-Daten sind
     topologisch sauber); Ergebnis ist ein einziger Ring. Bezirke ebenso je Bezirk.
   - Straßen werden an gemeinsam genutzten Knoten in Kanten geteilt (Graph für Verkehr, Passanten, Navigation, Namen);
     Querschnitt siehe unten. Einbahn aus `oneway`, `junction=roundabout`, Autobahn. Tunnel entfallen, Tordurchfahrten
     bleiben.
   - Gebäude inkl. Multipolygone mit Innenhöfen; Höhe aus `height`, sonst `building:levels` × 3,2 m + 1 m, sonst Standard
     nach Gebäudetyp. Douglas-Peucker mit 0,3 m.
   - Wände: Uferlinien und ebenerdige Gleise (±2,5 m) werden in 1-m-Stücke zerlegt; Stücke in Brücken-Korridoren und an
     Bahnübergängen entfallen. Brücken bekommen Geländer.
   - Kreuzungsflächen (Knoten mit ≥ 3 Straßen oder einem Knick, Radius = größte halbe Breite + 2 m) und die Spurkürzung
     je Knoten werden im Build berechnet, damit das Spiel sie auch am Rand des Geladenen richtig kennt.
   - Bäume: Ein Stamm darf keine Fahrbahn bis Klasse „service“ und keine Kreuzungsfläche berühren. Bäume darauf werden
     quer zur Straße an den Bordstein ihrer Seite geschoben (Reihen bleiben Reihen), sonst verworfen; ebenso Bäume in
     Häusern/Wasser. Abschließende Prüfung, bei Verstoß bricht der Build ab (Stand 27.09.: 95 202 verschoben, 2 429
     verworfen). OSM-Bäume nur, wo das Kataster im Umkreis von 4 m keinen Baum kennt.
   - POIs (Knoten, Wege, Relationen mit `shop`, `amenity`, `tourism`, Bahnhöfen, Bushaltestellen) in 13 Kategorien,
     Position = Knoten bzw. Schwerpunkt; Bahnhöfe je Name im Umkreis von 400 m, Bushaltestellen von 80 m nur einmal.
     Hausnummern aus `addr:housenumber` + `addr:street`, Gebäude und Eingangsknoten mit derselben Nummer einmal.
   - Missionsorte aus `data/places.json` rasten auf die nächste Straße ein; das Zeitlimit folgt aus der kürzesten Route
     (Dijkstra mit Binärheap) mit 10 m/s + 60 s.
   - Abdeckung: je Bezirk werden alle Datenschichten gezählt (und wie viel des Hauptnetzes gemessene Breiten, Parkstreifen,
     Tempo und Belag trägt) und in `index.json` unter `meta.coverage` abgelegt.
4. `tiles.mjs` zerlegt das Ergebnis in Kacheln zu 640 × 640 m (`web/data/berlin/tiles/<x>_<y>.json`):
   - Straßenkanten, Gebäude, Wege, Gleise, Wände, Zäune und kleine Flächen liegen in jeder Kachel, die ihr Hüllrechteck
     berührt (Straßen samt halber Breite + 1 m), und tragen eine globale Nummer; lange Linien werden vorher in Stücke
     geteilt. Große Flächen (Seen, Wälder) werden je Kachel abgeschnitten (Sutherland–Hodgman, 1 m Überlappung).
   - Punkte (Bäume, POIs, Hausnummern, Poller, Querungen, Ampeln, Abbiegeverbote) gehören genau einer Kachel.
   - Eine Kachel enthält damit alle Straßen, die einen ihrer Bäume berühren könnten; die Baumregel lässt sich Kachel für
     Kachel prüfen.
   - Dazu `index.json` (Grenze, Bezirke, Ortsteile, Kieze, Missionsorte, Abdeckung, Liste der Kacheln) und
     `overview.json` (Stadtplan: vereinfachte Flächen, Straßen bis Wohnstraße, Bahnen, Bahnhöfe). Alles deterministisch.
5. Im Spiel setzt `web/src/map.js` die Kacheln um die Kamera zusammen (`city.focus`): bis 400 m muss alles geladen sein
   (sonst steht die Welt still), bis 700 m wird vorgeladen, jenseits 1,1 km freigegeben. Mehrfach abgelegte Objekte
   werden nur einmal angelegt (Referenzzähler) und mit ihren Raster-Einträgen entfernt, wenn keine Kachel sie mehr
   hält. Spurgraph (`roadgraph.js`) wächst mit: Spuren entstehen mit ihrer Kante, Nachfolger werden nach jedem Nachladen
   neu bestimmt. Knoten kennen ihre Kanten nach Nummer sortiert, damit das Verhalten nicht von der Ladefolge abhängt.

**Straßenquerschnitt** (`tools/osm/crosssection.mjs`, zur Laufzeit `web/src/street.js`): Bordstein-zu-Bordstein-Breite
aus `width:carriageway` › `width` › Summe aus Fahrstreifen, Parkstreifen (`parking:<seite>` + `:orientation`; parallel
2 m, schräg 4,5 m, senkrecht 5 m, halb auf dem Gehweg die Hälfte) und Radfahrstreifen (`cycleway:<seite>=lane`). Passt
das nicht zusammen, werden Park-/Radstreifen anteilig gekürzt. Spurlage von der Mitte nach außen; bleiben bei
Gegenverkehr weniger als 2,6 m je Richtung, fahren beide Richtungen mittig (enge Berliner Nebenstraßen). Geparkte Autos
stehen auf deterministischen Stellplätzen, frei ab 5 m hinter der Kreuzungsecke (StVO § 12).

**Regeln und Zugänge:** Ampelknoten = Kreuzung mit `highway=traffic_signals` im Umkreis von 35 m; fester Umlauf
50 s (zwei Achsen, je 20 s grün, 3 s gelb, 2 s alles rot, Versatz je Kreuzung). Abbiegeverbote aus
`type=restriction` (von Weg, über Knoten, nach Weg). Poller/Schranken auf einer Straße sperren alle Kanten durch den
Knoten; Sperrlinien (`barrier=*` als Weg), die eine Straße kreuzen, ebenso; bei `traffic_intervention=diagonal_diverter`
auch knapp daneben verlaufende Straßen. Tordurchfahrten (`tunnel=building_passage`) öffnen die Hauswand im Korridor.
Einzelne Poller auf Gehwegen sperren nichts (Berlin hat viele davon gegen Gehwegparken).

**Verkehrsfluss ohne Ampel** (`web/src/traffic.js`): Jede KI-Route besteht aus Spurstücken; vor dem Ende eines Stücks
bittet das Auto um Einfahrt (`mayEnter`). Eine Kreuzung ohne Ampel hält eine Reservierung (`world.jres`: Zufahrt,
Autos, Bewegung je Auto als Sehne Spurende → Zielspur). Hinein darf, wer hinter der Kreuzung Platz hat und entweder aus
der reservierenden Zufahrt kommt (Kolonne, höchstens 6 s) oder mit keiner reservierten Bewegung in Konflikt steht
(Sehnen kreuzen sich nicht, kommen sich nicht näher als 2 m, andere Zielspur). Engstellen (Straßen, auf denen beide
Richtungen mittig fahren) werden über alle zusammenhängenden Abschnitte gleichen Namens als eine Einheit reserviert
(`world.nres`: Richtung, Autos); Sackgassen-Engstellen nur einzeln. Wer nicht einfahren darf, hält 2,4 m vor der Linie
(vor Engstellen 7,4 m), sucht nach 3 s einen anderen Weg, und wer sonst steht, gibt seine Reservierungen nach 2 s frei.
Hindernisse werden entlang der eigenen Route gemessen. Das ersetzt Vorfahrtregeln: es ist keine StVO-Vorfahrt, aber es
verklemmt nicht.

**Tageslicht und Licht** (`web/src/daylight.js` rein rechnerisch, `web/src/lighting.js` zeichnet): `lightAt(minuten)`
liefert Sonnenrichtung und Schattenlänge (Azimut 50° bei Aufgang 5:30 bis 310° bei Untergang 20:30, Mittagshöhe 58°,
Schatten höchstens 2,4 × Höhe, zum Horizont hin ausgeblendet), das Umgebungslicht je Farbkanal aus Stützstellen
(stetig interpoliert), den Anteil beleuchteter Fenster und ob Laternen brennen. Hausschatten: jede Wand überstreicht
beim Verschieben um Sonne × Höhe ein Viereck; alle Vierecke eines Bildes gehen gleich orientiert in einen Pfad und
werden deckend in eine eigene Bildschirm-Ebene gefüllt, die einmal mit 30 % Deckkraft aufgetragen wird – so dunkeln
Überlappungen nicht doppelt ab und Innenhöfe werden nur dort beschattet, wo ihre Wände hineinwerfen. Lichtkarte: eine
zweite Ebene mit dem Umgebungslicht füllen, Lichtquellen additiv (`lighter`) aus vorgerenderten Verlaufs-Sprites
(rund, Kegel) dazu, dann per `multiply` über die Welt; HUD, POI-Schilder und Grenze kommen danach und bleiben hell.
Damit Laternenlicht nicht auf Dächern liegt, folgt in der Lichtkarte ein zweiter Durchgang in derselben Tiefenfolge wie
das Bild: Häuser und Baumkronen werden mit der Umgebungsfarbe übermalt, erleuchtete Fenster (deckungsgleiches
Fenstermuster, nur die hellen Fenster) und Laternenköpfe leuchten selbst. Straßenlaternen (`web/src/lamps.js`): je Kante
so viele, wie der mittlere Abstand ergibt (OSM zerlegt Straßen in kurze Stücke; ein fester Abstand je Stück ließe die
meisten leer), am Bordstein plus 0,7 m. Qualitätsstufe: Median der reinen Zeichenzeit über 120 Bilder; über 14 ms
entfallen Baumschatten und der zweite Hausdurchgang, unter 8 ms kommen sie zurück.

**Boden** (`web/src/textures.js`, `web/src/decals.js`): Texturen sind Kachelmuster (64–96 px, doppelt aufgelöst,
per Muster-Transformation halbiert), als `fillStyle`/`strokeStyle` in Weltkoordinaten – sie kleben an der Welt und
kosten je Form nichts extra. Ein strenges Plattenraster auf dem Gehweg wurde verworfen: es liegt achsparallel über
schräg verlaufenden Straßen und wirkt wie ein gekachelter Platz. Decals entstehen deterministisch je Kante aus dem
Querschnitt (Rinnstein am Bordstein, Flicken und Deckel zwischen den Fahrstreifenrändern) und werden als ein Pfad je
Art zwischengespeichert; in der niedrigen Qualitätsstufe entfallen sie.

**Häuser** (`web/src/roofs.js`, gezeichnet in `render.js drawRoof`): OSM kennt Dachformen in Berlin nur lückenhaft,
deshalb entscheidet das Spiel aus Gebäudeart, Höhe und Seed: Altbau (12–24 m) meist mit „Berliner Dach“, niedrige
Wohnhäuser meist Satteldach, Industrie Wellblech. Aufbauten werden je Haus einmal gewürfelt (Anzahl nach Grundfläche)
und nur angenommen, wenn ein 5 × 5-Punkteraster über der Fläche samt Rand im Grundriss liegt. Alles hängt am
Gebäudeobjekt und verschwindet mit ihm, wenn seine Kachel entladen wird. Niedrige Qualitätsstufe: ohne Aufbauten,
Kiesmuster und Wellblechrillen.

**Bewusste Vereinfachungen:** feste Ampelumläufe statt Signalplänen, keine StVO-Vorfahrt („rechts vor links“) an
ungeregelten Kreuzungen (stattdessen Reservierung, s. o.),
keine Spurwechsel, keine Höhenebenen außer Brücken/Hochbahn (optisch), Straßen außerhalb der Grenze nur als Kulisse.

## Offene Punkte, nur auf echter Hardware prüfbar

1. Ob WebView2 auf der Konsole ohne Zusatzpaket startet: Die Runtime stellt das Xbox-OS bereit, nicht belegt.
2. Ob die Umgebungsvariable für Autoplay auf der Xbox wirkt. Falls nicht: Ton startet erst nach Browser-Geste, das Spiel
   bleibt aber vollständig spielbar.
3. Ob `localStorage` in der WebView2 über App-Neustarts erhalten bleibt (auf Windows ist das so).
4. Ob die Controller-Weiterleitung mit 8-ms-Timer flüssig genug ist (Latenz) und ob die Web-Gamepad-API inzwischen
   eventuell doch funktioniert.
5. Ob ein kostenloses Einzelentwicklerkonto die Dev-Mode-Aktivierung erlaubt (die Doku sagt nur „fully registered“).
6. Bildrate auf der Konsole. Auf dem Mac hält der Browser mit der echten Karte bei 1280×720 und 1920×1080 die volle Bildwiederholrate (Frame-Abstand Median 10,0 ms, p95 10,9 ms, gemessen im Playwright-Chromium); über die Xbox sagt das nichts aus.
7. Nachladen und Speicher auf der Konsole: Auf dem Mac bleiben 10–20 Kacheln geladen (JS-Heap 45–110 MB), eine Kachel
   ist höchstens 0,21 MB groß; das App-Paket wird durch die Karte ~140 MB größer.
