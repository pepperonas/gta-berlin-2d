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

## Karte: ganz Berlin aus offenen Daten (Stand 28.09.2026)

**Quellen**

| Was | Quelle | Lizenz |
|---|---|---|
| Stadtgrenze, Bezirke | Geoportal Berlin, WFS `lor_2021` (`c_lor_pgr_2021`, alle 58 LOR-Prognoseräume 2021; Bezirk aus dem Feld `bez`) | Datenlizenz Deutschland – Zero 2.0 |
| Ortsteile (97), Straßen, Gebäude, Wasser, Grün, Gleise, Bäume, Kiez-Namen, POIs, Hausnummern, Ampeln, Querungen, Poller, Zäune, Abbiegeverbote | OpenStreetMap, Berlin-Auszug von Geofabrik (`berlin-latest.osm.pbf`, täglich, ~100 MB) | ODbL 1.0 |
| Straßen- und Anlagenbäume (Gattung, Höhe, Kronendurchmesser, Stammumfang), 962 545 Bäume | Geoportal Berlin, WFS `baumbestand` | Datenlizenz Deutschland – Zero 2.0 |
| Einwohnerdichte 2022 (Einwohner je Hektar je Block, 26 397 Blöcke) | Geoportal Berlin, WFS `ua_einwohnerdichte_2022` (Umweltatlas) | Datenlizenz Deutschland – Zero 2.0 |
| Verkehrsmengen 2019 (Kfz je Werktag, 9 922 Abschnitte des Hauptstraßennetzes) | Geoportal Berlin, WFS `verkehrsmengen_2019` (`dtvw2019kfz`) | Datenlizenz Deutschland – Zero 2.0 |
| Fahrplan (Linien, Linienwege, Halte, Fahrten) | VBB Verkehrsverbund Berlin-Brandenburg GmbH, GTFS (`https://www.vbb.de/vbbgtfs`) | CC BY 3.0 (Namensnennung „VBB Verkehrsverbund Berlin-Brandenburg GmbH“) |

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

Die Sperren gelten für den KI-Verkehr; das Spielerauto fährt Poller und Schranken ab etwa 16 km/h Aufprall-
geschwindigkeit um (`car.js knockOver`, Schwelle `KNOCK` in `config.js`). Umgefahrene merkt sich die Welt unter dem
Ortsschlüssel des Pollers (`world.knocked`, Ort → Fallrichtung) – nicht am Objekt, das beim Entladen der Kachel
verschwindet; alle Kollisionen (Auto, Spielfigur, Passanten, Schüsse, Stellplatzprüfung) übergehen liegende Poller
(`isDown`). Zäune, Mauern, Hecken und Pollerlinien öffnen sich an Tor-Knoten und überall, wo ein Weg oder eine
Straße sie kreuzt (Schnittpunkt der Linien, auch ohne gemeinsamen Knoten – in OSM hat längst nicht jede Querung
einen), jeweils `GATE_M` = 4,4 m breit. Vorher waren Tore 1,8 m breit und Querungen ohne Tor zu; das Tempelhofer
Feld war mit dem Auto nicht erreichbar. `tests/helpers/city.js reachability` misst das per Flutfüllung auf einem
Raster (Kreis mit Auto- bzw. Fußgängerradius gegen alle festen Hindernisse).

**Brücken** (`build.mjs bridgeFills` und Geländer, `render.js` Straßenschritt): Gegenläufige Einbahn-Brückenfahrbahnen
gleichen Namens, deren Bordsteine höchstens 7 m auseinanderliegen, bekommen die Lücke als Fahrbahn (`fill`,
Kachelfeld 12, Vorzeichen wie `offsetLine`: + rechts der Kantenrichtung). Geländer entstehen für alle Brückenwege
(Straßen und Wege), werden aber mit `makeCutter` überall entfernt, wo eine andere Brückenfahrbahn samt Gehwegbreite
(2,5 m), ein anderer Brückenweg, eine gefüllte Lücke oder eine Straße darunter liegt – der eigene Korridor zählt
nicht (Kennung je Weg). So steht ein Geländer nur am äußeren Rand der ganzen Brücke. Zeichenfolge: Straßen am Boden →
Bordstein der Brücken → Wege auf Brücken → Brückenfahrbahnen samt Lücke → Radwege → Markierungen.
Unsichtbare Wände (Ufer, Gleisränder, Geländer) prüft der Build zum Schluss mit `surfaceIndex` (exakter Punkttest der
Fahrfläche je Straßenseite: Fahrbahn + Lücke + Radweg, Kreuzungsscheiben, auch Tordurchfahrten und gesperrte Straßen)
und `cutWhere` Meter für Meter; Korridore allein ließen an Brückenenden, wo Bänder zweier Kanten zusammenstoßen,
Reste stehen. Geländer liegen bei `reachOf(Seite) + 0,6 m`, also außerhalb von Radweg und Lücke.
Radwege neben der Fahrbahn (`cycleway:*=track`, `crosssection.mjs cycleTrack`) stehen im Querschnitt (`track` je Seite,
Kachelfelder 13/14) und werden jenseits des Bordsteins als rote Streifen gezeichnet.

**Ebenen** (`tools/osm/levels.mjs levelOf`, Spiel `web/src/levels.js`, Zeichnen `render.js drawFrame`): OSM gibt
Wegen keine absolute Höhe, wohl aber die Reihenfolge übereinander (`bridge`, `layer`, `tunnel`) und die Verbindungen
über gemeinsame Knoten. Daraus bekommt jede Straße, jeder Weg, jedes Gleis und jedes Brückendeck eine Ebene
(Laufzeitfeld `lvl`, −2 … +3; Brücke = max(1, layer), `layer<0` ohne Tunnel = offene Unterführung, `layer>0` ohne
Brücke = Boden, Tunnel verworfen). Kachelformat: Kanten-Flags Bits 4–6, Wege-/Kreuzungs-`fl` Bits 2–4 (Kreuzung
zusätzlich 5–7 für die niedrigste Ebene), Gleise/Flächen optionales Schlussfeld – drei Bit mit Vorzeichen
(`citycodes.js packLvl/unpackLvl`). **Portale** (`build.mjs portalsOf`, Kachelfeld `portals`) sind Knoten, an denen
Straßen/Wege verschiedener Ebenen zusammentreffen; nur dort wechselt ein Objekt die Ebene (`stepLevel`): es gilt die
Fläche, auf der es eindeutig ist (Lotfußpunkt im Stück, innerhalb der Breite), auf der eigenen Ebene bleibt es. Ein
Sicherheitsnetz setzt Objekte ohne Fläche ihrer Ebene in der Nähe auf den Boden zurück; neue Objekte bekommen die
Ebene der Straße in Blickrichtung (`initialLevel`). `world.js updateLevels` führt das je Schritt für Autos, Personen,
Räder und die Spielfigur (im Auto: die Ebene des Autos). Gezeichnet wird Ebene für Ebene (Decks, Wege, Straßen,
Markierungen, Nässe; am Boden Zäune, Poller, Möbel); nach jeder Ebene folgen die Objekte, die unter der nächsten
höheren Fläche liegen (`occlusion.js levelSurfaces/surfacesOver`, im Portal zählt nichts als Decke), danach die
Brücke über ihnen, die Silhouette zeigt den verdeckten Teil. Straßenbahnen haben keine eigene Ebene: sie liegen oben,
wo ihr Linienweg auf einer Brücke und nicht zugleich auf einer Bodenstraße liegt (`trackLevel`, bei Überlappung gilt
das Stück davor). Kollision, Verkehr und Oberfläche sind noch nicht ebenenbewusst (folgt).

**Wegweiser** (`tools/osm/signs.mjs`, rein; Spiel `web/src/signs.js`, gezeichnet in `render.js drawSign`): Kreuzung =
Knoten, an dem sich mindestens zwei verschieden benannte Straßen bis Klasse 4 (secondary) treffen, oder ein Knoten
an einem Kreisel (`junction=roundabout/circular`). Knoten näher als 45 m (Richtungsfahrbahnen) und alle Knoten eines
Kreisels werden zusammengefasst (höchstens 180 m Ausdehnung). Rand-Kanten bis Klasse 5 mit genau einem Ende in der
Kreuzung sind Zu- bzw. Ausfahrten, je nach Einbahnrichtung. Je Zufahrt entsteht ein Schild; Zeilen, die um mehr als
149° zurückführen, entfallen. Ziele in dieser Reihenfolge: Relation `destination_sign` (von-Weg, Kreuzungsknoten,
nach-Weg), `destination`-Tags der ausfahrenden Straße in Fahrtrichtung (`:forward`/`:backward`, das nackte Tag nur
in Weg-Richtung oder auf Einbahnstraßen), sonst eine Verfolgung: gleiche Straße weiter, sonst die geradeste Straße
bis Klasse 5, bis 4 km, alle 100 m wird der Ortsteil nachgeschlagen; die ersten zwei neuen Ortsteile ab 250 m
kommen aufs Schild, „Zentrum“ davor, wenn der Endpunkt mehr als 1 km näher an der Mitte liegt (nur außerhalb von
2,5 km um die Mitte). Bleibt die Straße im Ortsteil, steht ihr Name auf einer weißen Zeile. Jedes Ziel nur einmal je Schild (`dedupeRows`:
OSM-Zeilen, dann nach kleinster Abbiegung; leere Zeilen nennen ihre Straße oder entfallen). Verdeckte Tafeln laufen
wie Fahrzeuge durch `occludersOf` (15 Stichpunkte über die Tafel) und scheinen im verdeckten Teil mit Schrift durch. Standort: 35 m vor der
Kreuzung (höchstens 70 % der Zufahrt), rechts neben der Fahrbahn, mit `roadClearance` (gemeinsam mit den Pollern)
vom Fahrbahnrand weggerückt; liegt der Platz in einem Haus, bleibt das Schild ungezeichnet (`vis` 0). Die Tafel
wird je Schild einmal in dreifacher Auflösung gemalt und danach nur gestempelt; sie reicht vom Pfosten weg von der
Straße. Kachelformat: `signs: [x, y, Fahrtrichtung ×1000, Name, sichtbar, [[Richtung ×1000, Abbiegen ×1000, Ziele,
Nummer]]]`.

**Silhouetten** (`web/src/occlusion.js occludersOf`, rein; gezeichnet in `render.js drawCovered`): Nach den
tiefensortierten Objekten und dem Viadukt sucht der Renderer für jedes sichtbare Fahrzeug (Auto, Bus, Straßenbahn-
wagen, Rad) und jede Person die Verdecker, die in der Zeichenfolge nach ihr kommen und sie berühren – geprüft an neun
Stichpunkten der Grundfläche (Mitte, Ecken, Kantenmitten): Baumkrone (Kreis über dem Stamm), Haus (Grundriss um
Bruchteile des Dachversatzes verschoben – deckt Dach, Fassade und Tordurchfahrt) oder Viadukt/Bahnbrücke (halbe
Deckbreite um das Gleis). Für jedes verdeckte Objekt entsteht auf zwei wiederverwendeten kleinen Hilfsflächen eine
Maske aus allen Verdeckern (jede Wand als Viereck zwischen Fuß und Dach, dazu das Dach mit Höfen, Kronenkreise,
Deckstreifen) und der Umriss (abgerundetes Rechteck mit Frontscheibe bzw. Kreis), der per `destination-in` auf die
Maske beschnitten und nach der Lichtkarte ins Bild gesetzt wird. So ist genau der verdeckte Teil umrissen. Kosten an
dichten Stellen 7–21 Umrisse je Bild bei 2,4–3,2 ms Zeichenzeit.

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

**Gebäude im Build** (`tools/osm/build.mjs buildingTreatment/mergeParts`): Aus Brückenbauwerken (`building=bridge`,
`man_made=bridge`), Dächern (`building=roof`) und schwebenden Teilen (`min_height` ≥ 3 m oder `building:min_level`
≥ 1) werden keine Häuser – sie berühren den Boden nicht, eine Mauer wäre falsch (Beispiel Oberbaumbrücke: Pfeiler im
Wasser, Kreuzgang unter der U-Bahn, Turmspitze). Bauteile (`building:part`) gelten nur, wenn kein Gebäudeumriss sie
enthält (sonst ist der Umriss das Gebäude); ineinanderliegende Teile bilden eine Gruppe, von der das höchste Teil mit
seinem Umriss übernommen wird. `man_made=bridge`-Flächen werden Brückendecks (`AREA_KIND.bridge`), über dem Wasser
gezeichnet. Der Index trägt alle Krankenhäuser (`hospitals`).

**Häuser** (`web/src/roofs.js`, Farben `web/src/buildcolors.js`, gezeichnet in `render.js drawRoof`): Der
Karten-Build übernimmt je Gebäude, was OSM weiß – Dachform (`roof:shape`, in Berlin an etwa jedem fünften Haus),
Dach- und Fassadenfarbe, Material, Gebäudetyp – plus den Bezirk, als Bitfeld und zwei Farbzahlen am Ende des
Gebäude-Eintrags (`tools/osm/looks.mjs`, Codes in `citycodes.js`; leere Felder entfallen, zusammen +3 MB). Fehlt
die Dachform, schätzt das Spiel aus Gebäudeart, Höhe, Typ, Bezirk und Seed: Altbau (12–26 m) meist mit „Berliner
Dach“, Villen und Einfamilienhäuser Walm- oder Satteldach, Reihenhäuser Satteldach, Plattenbauten in
Marzahn-Hellersdorf und Lichtenberg flach, Industrie Wellblech.

Die Dachgeometrie (`roofGeometry`) wird einmal je Haus gerechnet. Kompakte Grundrisse (mindestens 82 % ihres
ausgerichteten Hüllrechtecks) bekommen das Dach über dem Rechteck: Satteldach als zwei Hälften mit First auf der
Mittellinie, Walm-, Zelt- und Mansarddach als Streifen entlang der vier Kanten. Verwinkelte Grundrisse (Blockrand
mit Hof, L-Formen) bekommen einen Streifen entlang jeder Außen- und Hofkante; die inneren Ecken liegen auf der
Winkelhalbierenden (Gehrung, gedeckelt bei spitzen Winkeln), so treffen sich die Flächen auf dem Grat. Die Tiefe
gilt je Traufkante: ein Strahl von drei Punkten der Kante nach innen misst die Hausdicke dahinter (Median), beim
Steildach liegt der First in ihrer Mitte (höchstens 8 m hinter der Traufe), beim Berliner Dach 38 % davon bis 4,5 m,
bei der Mansarde 22 % bis 2,8 m; an einer Ecke gilt die kleinere Tiefe der beiden Kanten. So bekommen Vorderhaus
und Seitenflügel je ihren First. In Nord-Neukölln (Bezirk Neukölln, Altbauhöhe) ist das geschätzte Dach meist ein
Steildach statt des Berliner Dachs. Ist eine Kante kürzer als
die doppelte Tiefe, kehrt sich die Innenkante um; dann wird die Fläche zum Walmdreieck. Jede Fläche kennt ihre
Fallrichtung; der Renderer bündelt die Flächen eines Hauses nach Richtung (16 Stufen, je ein `Path2D`) und färbt jedes
Bündel nach Sonnenstand (`facadeLight`, Helligkeit in 21 Stufen je Haus zwischengespeichert). Ziegelreihen (höchstens
320 Linien je Dach) und Grate sind je ein weiterer Pfad; der Grundriss dient als Clip. Die Zeichenzeit blieb beim
Nachmessen an neun Orten zwischen 0,7 und 2,9 ms je Bild. Aufbauten werden je Haus einmal gewürfelt (Anzahl nach Grundfläche)
und nur angenommen, wenn ein 5 × 5-Punkteraster über der Fläche samt Rand im Grundriss liegt. Alles hängt am
Gebäudeobjekt und verschwindet mit ihm, wenn seine Kachel entladen wird. Niedrige Qualitätsstufe: ohne Aufbauten,
Kiesmuster und Wellblechrillen.

**Autos und Figuren** (`web/src/vehicles.js`, `web/src/assets.js`): Das Automodell ist reine Darstellung und wird
aus der Autonummer abgeleitet – nicht aus dem Zufallsgenerator der Welt, sonst würde sich der Verkehr ändern.
Karosserien sind je Modell × Farbe zwischengespeicherte Sprites (höchstens 160, dann wird geleert); Räder, Licht und
Blinker kommen jedes Bild dazu. Der Blinker (`ai.blink`) ist das einzige neue Feld in der Simulation, rein
abgeleitet aus Route und Abbiegewinkel, ohne Zufall. Ein Sprite in `assets/manifest.json` ersetzt weiterhin alles.

**Tagesrhythmus und Stadtleben** (`web/src/rhythm.js`, `web/src/life.js`, rein rechnerisch):
- Daten aus dem Build: Jede Kante trägt `dtv` (Kfz je Werktag). `assignTraffic` tastet die Zähllinien alle 20 m ab und
  gibt den Wert der nächsten Hauptstraßen-Kante (≤ 15 m, Richtung ähnlich – sonst erbt die Querstraße die Zählung);
  1 443 km Zählstrecke treffen 31 305 Kanten, alle anderen bekommen einen Schätzwert nach Straßenklasse
  (`DTV_ESTIMATE`, Vorzeichen im Kachelformat = gemessen/geschätzt). Die Einwohnerdichte wird auf ein 64-m-Raster
  gelegt (`densityGrid`, Wert am Zellmittelpunkt, Innenhöfe ausgespart) und je Kachel als 10 × 10 Werte ausgeliefert,
  nur wo jemand wohnt. Bänke, Picknicktische, Fahrradständer und Mülleimer aus OSM kommen als `furn` (76 380).
- `populationTargets` bestimmt alle 2 s die Zielzahl an Autos und Passanten um die Kamera: Tageskurve (Werktag bzw.
  Wochenende; alle Kurven treffen sich um Mitternacht im selben Wert, damit der Wechsel keinen Sprung macht) ×
  örtlicher Faktor (Wurzel des längengewichteten DTV im Umkreis von 300 m bzw. Dichte + Läden im Umkreis) + Nachtleben
  (Freitag/Samstag voll, Nacht zählt bis 6 Uhr zum Vortag, gewichtet mit Bars und Spätis in der Nähe). Überzählige
  Autos und Passanten werden außer Sicht abgebaut; neue Autos entstehen mit Wahrscheinlichkeit nach DTV der Spur.
- `lifeSpots` liefert aus POIs, Bänken und großen Wiesen die Plätze mit Tätigkeit (deterministisch aus Ort, Stunde und
  Wochentag – keine Zufallszahlen der Welt). Gruppen stehen vor dem Laden auf dem Gehweg zur nächsten Straße hin,
  Schlangen entlang des Gehwegs, Liegende auf Decken nur innerhalb der Wiese. `manageLife` (alle 0,5 s) besetzt
  fehlende Plätze nur außer Sicht mit Passanten im Zustand `hang` und baut nicht mehr gewünschte nur außer Sicht ab
  (höchstens 70); wer erschreckt wird, verlässt den Zustand und fällt aus der Verwaltung. Nur die Standardbevölkerung
  hat Rhythmus – Tests und Titel-Demo mit festen Zahlen bleiben unverändert.

**Fahrzeugarten, Einsätze, Räder, Tiere, Klang:**
- `fleet.js` (rein rechnerisch): Maße und Motorleistung je Art (Pkw 4,2 × 2,0 m, LKW 7,6 × 2,4 m, Paketwagen,
  Müllauto 8,6 × 2,5 m, Streifen- und Rettungswagen) und wann welche Art unterwegs ist (`pickKind` nach Uhrzeit,
  Wochentag, Straßenklasse). Kollision und Zeichnung nutzen die Maße je Auto (`hw`/`hh`); die KI rechnet Abstände
  zwischen den Stoßstangen (die Konstanten gelten weiter für zwei Pkw), die Nahprüfung der Autos untereinander wächst
  mit der Länge.
- `services.js`: Arbeitshalte (`ai.hold` in `traffic.js`: Zielgeschwindigkeit 0, der Verkehr dahinter wartet) nach
  gefahrener Strecke, nie näher als 25 m vor bzw. 12 m hinter einer Kreuzung. Einsätze: jeder Tote erzeugt einen
  Rettungseinsatz (nahe beieinander liegende werden zusammengefasst), Schüsse einen Polizeieinsatz (45 s Sperre),
  höchstens zwei Wagen je Art. Einsatzfahrzeuge entstehen außer Sicht 100–200 m vom Einsatzort und fahren über ein
  Entfernungsfeld: `goalField` rechnet rückwärts vom Zielspurstück (Dijkstra über die Vorgänger der Spuren in einem
  Rechteck um Start und Ziel), `extendRoute` nimmt dann an jeder Kreuzung den Nachfolger mit der kleinsten
  Restentfernung. Die bereits geplanten ~40 m Route bleiben, damit Kreuzungsreservierungen gültig bleiben. Mit
  Sondersignal (`ai.urgent`) fährt ein Wagen über Rot (mit 7 m/s in die Kreuzung).
- `bikes.js`: Räder fahren auf dem Spurgraph (Einbahn und Abbiegeverbote gelten), nur auf der rechten Spur,
  seitlich versetzt auf die Radstreifenmitte bzw. 0,8 m vom rechten Fahrbahnrand; Hauptstraßen ohne Radstreifen sind
  tabu. Zwischen zwei Spuren queren sie die Kreuzung gerade. Sie bremsen für alles vor sich, außer für stehenden
  Querverkehr (der wartet auf sie – sonst warten beide ewig), und lösen sich nach 6 s Stillstand ohne Ampel für 2 s von
  allem Hindernis. Autos behandeln Räder wie Fußgänger vor sich. Abgestellte Roller: je Kante deterministisch, an der
  Hausseite des Gehwegs, nie auf Fahrbahn oder im Haus.
- `animals.js`: Taubenplätze vor 30 % der Imbisse, Cafés und Bahnhöfe (nach Ort-Hash) und auf Plätzen ab 900 m²;
  Entenplätze an den Ecken großer Wasserflächen, 6–13 m ins Wasser versetzt. Verwaltung wie beim Stadtleben (nur außer
  Sicht aufstellen, Aufgeflogene verschwinden aus dem Bild). Tauben fliehen vor Spielfigur (5,5 m), schnellen Autos,
  Joggern und Hunden, Schüssen und Hupen (45 m) in die Luft und landen woanders; Enten schwimmen weg und bleiben im
  Wasser.
- `ambience.js` (rein rechnerisch) mischt je Ort und Uhrzeit Stadtrauschen, Verkehr, Vögel, Kneipengemurmel, Wasser,
  Hochbahn und das nächste Martinshorn; `bellStrikes` meldet Glockenschläge beim Überschreiten der vollen Stunde, wenn
  eine Kirche (Gebäudeart) im Umkreis von 180 m steht. `audio.js` setzt die Mischung viermal je Sekunde in Rauschen
  mit Filtern, Vogelrufe als kurze Tonfolgen, ein Folgetonhorn-Oszillator und Glocken mit unharmonischen Teiltönen um.
  Hochbahnzüge sind bis zu echten Fahrplänen ein fester Takt.

**Wetter** (`web/src/weather.js` rein rechnerisch, `web/src/wetfx.js` zeichnet): je Tag acht Blöcke zu 3 h, Wetterbild
aus einem Hash von Welt-Samen, Tagnummer (`w.dayCount`, gespeichert) und Block; 45 min Überblendung zum nächsten Block
(über Mitternacht in den nächsten Tag). `weatherLight` passt das Tageslicht an: Sonnenschatten × (1 − 0,85 × Bewölkung),
Umgebungslicht gedämpft, und nur Regen und Nebel heben `dark` (die Lichtkarte mit Scheinwerfern kommt dann auch am Tag –
bloße Wolken nicht, das spart die Lichtkarte). Die Nässe `w.wet` ist Simulationszustand (0,025/s rauf je Regenstärke,
1/600 s runter) und senkt den Seitenhalt der Autos um bis zu 18 %. Darstellung: Wolkenschatten als vorgerenderte weiche
Flecken auf einem 260-m-Weltraster, die mit dem Wind wandern; Regen als bis zu 420 Striche und Spritzer aus Hashes und
Spielzeit (keine Partikelliste); nasse Straßen werden dunkel übermalt, Pfützen je Kante deterministisch an der Rinne
(nie auf Durchfahrten oder unter Überbauungen); in der Lichtkarte bekommt jedes Licht bei Nässe einen schwächeren, zur
Kamera versetzten Widerschein und bei Nebel einen größeren Hof. Fassaden: `facadeLight` = −(Wandnormale · Schattenrichtung)
× Sonnenstärke, als warme bzw. dunkle Lasur über der Grundfarbe. Leuchtreklame: aus den POIs im Bild (höchstens 40), am
Gehweg vor dem Laden, nach der Lichtkarte gezeichnet (leuchtet selbst) plus farbiges Licht in der Lichtkarte. Wie der
Tagesrhythmus gilt das Wetter nur in der Welt mit Standardbevölkerung; Tests und Titel-Demo bleiben klar.

**Unwetter und Schnee** (`weather.js`, `wetfx.js`): Jedes Wetterbild ist ein Satz Werte `{ cloud, rain (bis 1,6),
fog (bis 1,7), snow, storm, thunder }`, zwischen Blöcken stetig überblendet. Welche Bilder ein Tag hat, hängt an seiner
Wetterlage (`dayType`: gewöhnlich, unbeständig mit Sturm und nachmittags Gewitter, Winter mit Schnee). Böen (`gustAt`)
sind eine Summe inkommensurabler Sinuswellen der Spielzeit – kein Zufall, also in jedem Bild gleich. Blitze
(`strikeInSlot`) fallen in 2,4-s-Fenster, je Fenster aus einem Hash; `flashAt` beschreibt Vor-, Haupt- und Nachblitze,
`thunderBetween` liefert Donner, dessen Ankunft (Entfernung / 343 m/s) in einem halboffenen Zeitintervall liegt, damit
kein Donner doppelt oder verschluckt wird. Die Schneedecke `w.snow` ist Simulationszustand wie die Nässe (wächst mit der
Schneefallstärke, taut in ~25 min, bei Regen schneller, Tauwetter macht nass, steht im Spielstand) und nimmt den Autos
bis zu 45 % Seitenhalt. Darstellung: Schneedecke als kachelbare 256-px-Textur je Höhenstufe (fraktales Wertrauschen;
`snowCoverAlpha` schiebt eine Schwelle durch die gemessene Rauschverteilung: dünn = Flecken, tief = geschlossen), vor
Wasser und Straßen gezeichnet; auf Straßen deckender Matsch in einer eigenen Bildschirmebene (sonst summieren sich die
Überlappungen), dann Reifenspuren je Fahrstreifen und Schneewälle am Bordstein; Dächer je Fallrichtung nach Sonnenstand,
Kronenpolster aus festen Hashes, Hauben auf geparkten Autos. Flocken, Regentropfen, Laub und Nebelschwaden sind wie der
Regen Funktionen von Ort-Hash und Spielzeit (keine Partikellisten). Blitze: Zickzack mit Ästen aus dem Hash, nach der
Lichtkarte gezeichnet, dazu kalt-weißes Umgebungslicht in der Lichtkarte selbst (die Szene wird hell, nicht nur
überdeckt). Klang: Windschicht (Bandpass-Rauschen) nach Böen, Donner als Knall plus gefiltertes Grollen in Wellen,
Schneedecke dämpft Stadt und Verkehr.

**Fenster** (`web/src/windows.js`, gezeichnet in `render.js drawLitWindows`): Die Fassade ist ein Raster aus
14 × 16-px-Zellen (deckungsgleich mit dem Fenstermuster). Je Etage bilden 2–4 Fenster eine Wohnung; ein Fenster brennt,
wenn `0,78 × Hash(Wohnung) + 0,22 × Hash(Raum)` unter dem Anteil aus dem Tagesgang liegt – steigt der Anteil, gehen die
Fenster einzeln und in zufälliger Folge an, Räume einer Wohnung kurz nacheinander. Nachts wird je 9 Minuten ausgewürfelt,
wer kurz Licht macht. Arbeitsstätten haben einen eigenen Tagesgang, trübes Wetter hebt den Anteil tagsüber
(`weatherLight` → `gloom`, `windowsLit`). Je Haus wird die Liste einmal pro Spielminute und Fassade berechnet; gezeichnet
wird ein Pfad je Lichtfarbe, in der Lichtkarte heller und im Nebel gedämpft.

**Öffentlicher Verkehr** (`tools/osm/zip.mjs`, `tools/osm/transit.mjs` → `transit.json`; `web/src/transit.js`,
`web/src/transitlive.js`, `web/src/railart.js`):
- Build: ZIP ohne Abhängigkeiten (Zentralverzeichnis selbst gelesen, Einträge per `node:zlib` im Datenstrom – die
  400-MB-`stop_times.txt` liegt nie ganz im Speicher). Stichtage: je Tagesart der Kandidat unter den ersten vier
  Dienstagen/Samstagen/Sonntagen mit den meisten Fahrten (der erste Dienstag im Feed hatte Bauarbeiten, ein Samstag war
  Feiertag). Fahrten werden zu Mustern (Linie + Weg + Haltfolge) zusammengefasst; je Muster bleiben der auf Berlin
  gekürzte, vereinfachte Linienweg, die Halte als Bogenlänge (monoton, bei der Vereinfachung anteilig mitgezogen), die
  Fahrzeiten (Median) und die Abfahrtsminuten je Tagesart (GTFS-Zeiten über 24:00 bleiben als > 1440 erhalten). Die
  Projektion wird aus `index.json meta.origin` exakt wie im Karten-Build rekonstruiert. Nur Bus, Straßenbahn, S- und
  U-Bahn; Regionalbahn, Fähre und Rufbus bleiben draußen.
- Zeitmaßstab: die Spieluhr läuft 60× schneller als die Fahrzeuge fahren. Der Fahrplan liefert deshalb nur den Takt
  zur Uhrzeit (Abfahrten je Stunde, ±30 min, Nachtfahrten vom Vortag mitgezählt); die Fahrzeuge fahren in Echtzeit mit
  Fahr- und Haltezeiten aus dem Fahrplan. Jedes Muster im Umkreis von 1,2 km führt virtuelle Fahrzeuge, die nur aus
  ihrer Fahrzeit τ bestehen (so laufen sie auch über nicht geladene Kacheln), beim ersten Verfolgen gleichmäßig im
  Takt verteilt (Phase aus dem Muster-Hash), danach fährt je 3600/Takt Sekunden eines ab.
- Busse: nahe der Kamera (≤ 220 m, außer Sicht) werden sie zu KI-Fahrzeugen – nur dort, wo eine Spur höchstens 6 m vom
  Linienweg liegt und in seine Richtung zeigt. Sie folgen dem Weg (`ai.follow`: an jeder Kreuzung die Nachfolgespur
  mit dem kleinsten mittleren Abstand zum Weg, notfalls die dem Wegpunkt 60 m voraus nächste) und halten, sobald ihre
  Lage auf dem Weg den nächsten Halt erreicht. Ohne Vorankommen (25 s) oder abseits des Wegs (> 15 m, 8 s) geben sie die
  Linie ab. Busspuren: `oneway:bus=no`/`oneway:psv=no`/Gegenbusspur ergeben im Build ein Kennzeichen am Querschnitt,
  der Spurgraph legt daraus eine Gegenspur nur für Busse an; reine Busstraßen (`busway`) ebenso. `lane.next` (allgemeiner
  Verkehr) enthält keine Busspuren, `lane.nextBus` schon.
- Straßenbahnen bleiben kinematisch auf dem Weg; vor Spieler, Autos, Rädern oder Fußgängern auf dem Gleis bleibt die
  Fahrzeit stehen (Verspätung), nach 2,5 s klingelt sie. Ihre Wagen gehen als feste Hindernisse in die Kollision
  (Autos und Spielfigur werden herausgeschoben) und in die Hinderniserkennung der KI.
- S-/U-Bahn werden nur gezeichnet, wo ein Gleis der Karte (oberirdisch, Tunnel sind im Build ausgenommen) höchstens
  5 m entfernt liegt; ihr Rumpeln (auch im Tunnel) speist den Umgebungsklang.

**Kampf** (`web/src/combat.js`, in `updateWorld` nach der Bewegung der Spielfigur): Waffen sind eine Tabelle (Schaden,
Reichweite, Pause zwischen Angriffen, Streuung, Magazin, Nachladezeit, Kugeln je Schuss). Schüsse sind sofortige
Strahlen ab der Körpermitte gegen die vorhandenen Kollisionsdaten (Hauswand-Segmente, Stadtgrenze, Baumkreise,
Kisten) und gegen Passanten (Kreis) und Autos (gedrehtes Rechteck); niedrige Wände (Zaun, Gleis, Kai, Geländer)
lassen Kugeln durch. Die Streuung zieht aus dem Welt-Zufall, alles bleibt deterministisch und in Node testbar. Die
Zielhilfe sucht im Kegel um die Zielrichtung das Ziel mit kleinstem „Winkel × 300 + Abstand“ und verlangt freie Sicht.
Maus-Zielen rechnet `main.js` in einen Weltpunkt um (`aimWorld`), wie Menüklicks als abstrakte Eingabe.
Gegenwehr ist ein Passanten-Zustand `fight` (hinlaufen, alle 0,9 s zuschlagen, Aufgeben nach 20 s oder 45 m); ob
jemand sich wehrt, folgt aus seiner Nummer, nicht aus dem Welt-Zufall. Das K. o. nutzt denselben Ladeweg wie der
Teleport (`findTeleportSpot`/`teleportTo`): Liegt das Ziel auf offenem Grund (Wiese, Platz, Gehweg/Hof), sucht
`openSpot` den Punkt selbst und dann Ringe bis 30 m nach einer freien Stelle (kein Haus, kein Wasser, kein
Hindernis; im Auto die ganze Karosserie in vier Ausrichtungen) – vorher sprang man immer zur nächsten Straße, vom
Tempelhofer Feld also über einen Kilometer weit. Auf Fahrbahn oder Haus bleibt es beim nächsten Gehweg bzw. der
nächsten Fahrspur. liegt das nächste Krankenhaus in einem ungeladenen Stadtteil, wartet die
Welt, bis er da ist.

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
