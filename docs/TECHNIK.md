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

## Karte: Kreuzberg und Nord-Neukölln aus offenen Daten (Stand 27.09.2026)

**Quellen**

| Was | Quelle | Lizenz |
|---|---|---|
| Gebietsgrenzen | Geoportal Berlin, WFS `lor_2021` (`c_lor_pgr_2021`, LOR-Prognoseräume 2021): 0210/0220/0230 = Kreuzberg, 0810 „Neukölln“ = Nord-Neukölln | Datenlizenz Deutschland – Zero 2.0 |
| Straßen, Gebäude, Wasser, Grün, Gleise, Bäume, Kiez-Namen | OpenStreetMap über die Overpass-API (Hüllrechteck + 250 m) | ODbL 1.0 |

„Nord-Neukölln“ ist kein Ortsteil, sondern die übliche Bezeichnung für den LOR-Prognoseraum 0810 nördlich der Ringbahn
(plus Köllnische Heide). Die Grenzen Kreuzbergs entsprechen den drei Kreuzberger Prognoseräumen.

**Pipeline** (`tools/osm/`, nur Node, keine Abhängigkeiten)

1. `fetch.mjs` lädt beides nach `data/raw/` (gitignored, ca. 95 MB, Abruf ~15 s).
2. `build.mjs` projiziert WGS84 mit einer transversalen Mercator-Projektion (Krüger-Reihen, GRS80, Mittelmeridian =
   Gebietsmitte) in Meter und dann in Spiel-Pixel (10 px = 1 m). Norden ist oben, der Kartenausschnitt ist nicht gedreht.
   - Grenze: die vier LOR-Polygone werden vereinigt, indem gemeinsame Kanten wegfallen (die Geoportal-Daten sind
     topologisch sauber).
   - Straßen werden an gemeinsam genutzten Knoten in Kanten geteilt (Graph für Verkehr, Passanten, Navigation, Namen);
     Breite aus `width`, sonst `lanes` × 3,3 m (+ Parkstreifen), sonst Standard je Klasse; Einbahn aus `oneway`,
     `junction=roundabout`, Autobahn. Tunnel und Durchfahrten entfallen.
   - Gebäude inkl. Multipolygone mit Innenhöfen; Höhe aus `height`, sonst `building:levels` × 3,2 m + 1 m, sonst Standard
     nach Gebäudetyp. Douglas-Peucker mit 0,3 m.
   - Wände: Uferlinien und ebenerdige Gleise (±2,5 m) werden in 1-m-Stücke zerlegt; Stücke in Brücken-Korridoren und an
     Bahnübergängen entfallen. Brücken bekommen Geländer.
   - Bäume: Ein Stamm (0,5 m) darf keine Fahrbahn bis Klasse „service“ berühren. Bäume darauf werden quer zur Straße
     an den Bordstein ihrer Seite geschoben (Reihen bleiben Reihen), sonst verworfen; ebenso Bäume in Häusern/Wasser.
     Abschließende Prüfung, bei Verstoß bricht der Build ab. Ursache sind meist geschätzte Fahrbahnbreiten mit
     Parkstreifen, auf denen in Wirklichkeit die Baumscheiben liegen (Stand 27.09.: 7 251 verschoben, 490 verworfen).
   - POIs (Knoten, Wege, Relationen mit `shop`, `amenity`, `tourism`, Bahnhöfen, Bushaltestellen) in 13 Kategorien,
     Position = Knoten bzw. Schwerpunkt; Bahnhöfe je Name im Umkreis von 400 m, Bushaltestellen von 80 m nur einmal.
     Hausnummern aus `addr:housenumber` + `addr:street`, Gebäude und Eingangsknoten mit derselben Nummer einmal.
   - Missionsorte aus `data/places.json` rasten auf die nächste Straße ein; das Zeitlimit folgt aus der kürzesten Route
     (Dijkstra) mit 12 m/s + 40 s.
   - Ausgabe `web/data/city.json`: ganzzahlige, delta-kodierte Koordinaten, deterministisch (zweimal bauen = gleiche Datei).
3. Im Spiel dekodiert `web/src/map.js` die Datei (~0,5 s) und legt Raster-Hashes für Darstellung, Straßensegmente,
   Flächen und Kollision an. Kollision mit Gebäuden läuft über Wandsegmente, dafür ist keine Triangulierung nötig.

**Bewusste Vereinfachungen:** eine Spur je Richtung für den KI-Verkehr (versetzt um ein Viertel der Fahrbahnbreite), keine
Ampeln, keine Höhenebenen außer Brücken/Hochbahn (optisch), Straßen außerhalb der Grenze nur als Kulisse.

## Offene Punkte, nur auf echter Hardware prüfbar

1. Ob WebView2 auf der Konsole ohne Zusatzpaket startet: Die Runtime stellt das Xbox-OS bereit, nicht belegt.
2. Ob die Umgebungsvariable für Autoplay auf der Xbox wirkt. Falls nicht: Ton startet erst nach Browser-Geste, das Spiel
   bleibt aber vollständig spielbar.
3. Ob `localStorage` in der WebView2 über App-Neustarts erhalten bleibt (auf Windows ist das so).
4. Ob die Controller-Weiterleitung mit 8-ms-Timer flüssig genug ist (Latenz) und ob die Web-Gamepad-API inzwischen
   eventuell doch funktioniert.
5. Ob ein kostenloses Einzelentwicklerkonto die Dev-Mode-Aktivierung erlaubt (die Doku sagt nur „fully registered“).
6. Bildrate auf der Konsole. Auf dem Mac hält der Browser mit der echten Karte bei 1280×720 und 1920×1080 die volle Bildwiederholrate (Frame-Abstand Median 10,0 ms, p95 10,9 ms, gemessen im Playwright-Chromium); über die Xbox sagt das nichts aus.
7. Ladezeit (6,6 MB JSON, ~0,5 s Dekodieren auf dem Mac) und Speicher (JS-Heap ~110 MB im Browser) auf der Konsole.
