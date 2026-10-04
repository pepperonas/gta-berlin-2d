# GTA Berlin

## Native Rust-Portierung – Phase 5

Die native Fassung läuft mit `cargo run` (macOS: Metal, Windows: DX12) und ist
seit Phase 3 spielbar: Kollision (SAT), Fahrphysik mit Einspurmodell/ESP/ABS,
KI-Verkehr mit Ampeln und Vorfahrt, Passanten, die Mission „Kisten für den Kiez“
und Spielstände als JSON-Datei; seit Phase 4 Tag und Nacht mit Schatten,
Laternen und Scheinwerfern (`--uhr 22:30`, Taste T: +1 Stunde); seit Phase 5
synthetisierter Klang (Motoren, Reifen, Verkehr, Stadt; Taste M, `--stumm`),
dazu HUD, Gamepad und Wetter (Regen, Schnee, Nebel, Gewitter, Glätte;
`--wetter heavysnow`, Taste N), Minikarte, Stadtplan (Tab), Titelbildschirm und
Pausenmenü. Waffen und Nahverkehr folgen;
das Browser-Spiel bleibt die vollständige Referenz.

```bash
cargo run --release                   # Titelbildschirm (Fortsetzen / Neues Spiel / Steuerung / Beenden)
cargo run --release -- --new          # sofort neu beginnen (Stand wird beim Speichern überschrieben)
cargo run --release -- --fortsetzen   # sofort mit dem gespeicherten Stand weiterspielen
cargo run --release -- --check-sim 120  # Simulation ohne Fenster
cargo run -- --free                   # freie Kartenansicht (Phase 2)
cargo test --workspace
cargo run -- --check-map
```

Zu Fuß: WASD/Pfeile, Shift sprinten, Alt langsam. F ein-/aussteigen, E Aktion
(halten = Kisten einladen). Im Auto: W/S Gas/Bremse (gehalten: rückwärts), A/D
lenken, Leertaste Handbremse, H Hupe, X ESP, Y/Z ABS. F5 speichert, nach einem
erledigten Auftrag wird automatisch gespeichert. Mausrad zoomt, Esc/P pausiert (Menü: speichern, Mission neu, Hauptmenü).
Mit Controller: linker Stick, RT/LT Gas/Bremse, Y ein-/aussteigen, A Aktion, B/RB Handbremse, X Hupe.
Details und Plattformgrenzen: [Native Architektur und Build](docs/NATIVE-RUST.md).
Die folgenden Browser-/UWP-Anleitungen beziehen sich auf den bisherigen Prototyp.

[![Node.js ≥20](https://img.shields.io/badge/Node.js-%E2%89%A520-339933?logo=node.js&logoColor=white)](https://nodejs.org/) [![Plattformen](https://img.shields.io/badge/Plattform-Browser%20%7C%20Xbox%20Dev%20Mode-107C10)](#auf-dem-mac-spielen-und-testen)

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

Dokumentation:

- [Architektur, Plattformentscheidung und Quellen](docs/TECHNIK.md)
- [Bedienung und technische Änderungen vom 01.10.2026](docs/SPIEL-UPDATE-2026-10-01.md)
- [Beschleunigung: Recherche, Messverfahren und alle 45 Fahrzeugwerte](docs/FAHRZEUG-BESCHLEUNIGUNG.md)
- [Grafiküberarbeitung: Bildvergleiche, Effekte und Renderzeiten](docs/GRAFIK-UPDATE-2026-10-01.md)
- [Versionsverlauf](CHANGELOG.md)

## Version

Aktuelle Paketversion **0.46.0**, ergänzt um den Entwicklungsstand vom **02.10.2026** (Semantic Versioning; solange die Version mit `0.` beginnt, ist es ein Prototyp). Änderungen je
Version und noch nicht versionierte Änderungen stehen in [`CHANGELOG.md`](CHANGELOG.md), die Version steht auch unten rechts im Titelbildschirm.

## Inhalt des Prototyps

- **Filmische Stadtgrafik:** wärmeres Tageslicht, kühlere Schatten, feinere Asphalt-/Steinoberflächen,
  detailliertere Fassaden und Baumkronen, bewegte Wasserreflexe, Lichtspiegelungen bei Regen und dezente
  Leuchthöfe nachts. Drift, Nässe, Schnee, Sprünge und Schwimmen erzeugen passende Bewegungseffekte.
  Effektdichte und Details folgen der automatischen Qualitätsstufe. 60 FPS bleiben ein Ziel;
  in dichten Straßenszenen wird das Budget noch überschritten (siehe Grafikbericht).
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
  sperren Straßen für den KI-Verkehr, Fußgänger kommen durch, und mit Schwung fährt man sie um (sie bleiben liegen) –
  auch ganze Pollerreihen, die in OSM als Linie eingetragen sind (einzelne Poller höchstens 1,5 m auseinander);
  rund 47 000 Zäune, Mauern und Hecken, an Toren und überall, wo ein Weg sie kreuzt, 4,4 m breit offen – eingezäunte
  Flächen wie das Tempelhofer Feld erreicht man auch mit dem Auto; 41 000 Hauseingänge als Türen.
- **Brücken:** Geländer nur am äußeren Rand der Brücke (nie auf einer Fahrbahn, nicht über der Straße darunter),
  Richtungsfahrbahnen mit schmaler Lücke bilden eine Fahrbahn, Gehwege liegen unter der Brückenfahrbahn; Radwege
  neben der Fahrbahn (`cycleway=track`) als rote Streifen.
- **Wegweiser:** An großen Kreuzungen und Kreiseln stehen gelbe Wegweiser: je Ausfahrt ein Pfeil in Kartenrichtung
  mit den Ortsteilen, in die sie führt (aus der OSM-Beschilderung oder aus dem Straßenverlauf), „Zentrum“ und
  B-Nummer; weiße Zeilen nennen Straßen im selben Ortsteil; jedes Ziel steht nur einmal auf einem Schild, verdeckte
  Schilder scheinen mit Beschriftung durch.
- **Silhouetten:** Liegt eine Baumkrone, ein Haus, eine Tordurchfahrt oder die Hochbahn über einem Fahrzeug oder einer
  Person, erscheint ihr Umriss genau im verdeckten Teil – für die Spielfigur und ihr Auto orange, für alle anderen dezent hell.
  Bei niedriger Qualität entfallen die zusätzlichen Umrisse anderer Personen und Fahrzeuge.
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
  fliehen vor Rasern, Hupen und Unfällen. Verletzte bleiben auf den Beinen; erst tödlicher Schaden lässt sie umfallen.
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
  immer gleich; `?wetter=regen` legt es fest). Ausgefranste Wolkenschatten ziehen mit dem Wind über die Stadt; bei
  Regen werden Asphalt und Gehwege nass und dunkel, Pfützen mit nassem Rand spiegeln Himmel und Lichter, Tropfen
  ziehen mit Schweif, am Boden und in den Pfützen breiten sich Aufschlagringe aus, Regenwände ziehen mit dem Wind
  durchs Bild; Leute tragen Schirme, weniger sind draußen, Autos rutschen mehr. Nebel liegt als Bodennebel auf Straßen
  und Höfen, aus dem die Dächer herausragen, darüber zieht Dunst. Fassaden sind auf der Sonnenseite heller. Nachts leuchten Neonschilder vor Bars, Clubs, Spätis
  und Imbissen.
- **Unwetter und Schnee:** Starkregen mit Gischtschleier und Regenwänden, Sturm (Bäume biegen sich in den Böen, Laub
  und Papier fegen übers Bild, der Regen treibt schräg, der Wind heult), Gewitter (Blitzstrahl mit Verästelungen,
  Himmelsblitz, der die Nacht für einen Moment zum Tag macht, Donner mit Schallverzug), dichter Nebel mit ziehenden
  Schwaden, Schneefall und Schneesturm. Schnee bleibt liegen – erst fleckig, dann geschlossen – auf Gehwegen, Grün,
  Dächern (Sonnen- und Schattenseite), Baumkronen und geparkten Autos; auf den Straßen Matsch mit festgefahrenen
  Reifenspuren und Schneewällen am Bordstein, Hauptstraßen freier als Nebenstraßen. Autos rutschen auf Schnee, Schnee
  dämpft den Stadtlärm, Tauwetter macht die Straßen nass. Wintertage, unbeständige Tage (Sturm, nachmittags Gewitter)
  und gewöhnliche Tage wechseln; `?wetter=starkregen|sturm|gewitter|dichternebel|schnee|schneesturm` und
  `?schneedecke=0…1` legen es fest.
- **Fenster gehen einzeln an:** Jedes Fenster hat seinen eigenen Zeitpunkt – abends gehen die Lichter in zufälliger
  Folge nach und nach an, Räume einer Wohnung kurz nacheinander, nachts macht hier und da jemand Licht im Bad.
  Glühlampenwarm, neutral, kaltweiß, gedimmt hinter dem Vorhang oder bläulich flackernder Fernseher; Büros und Schulen
  haben abends noch Licht und sind nachts dunkel; bei trübem Wetter brennt auch tagsüber Licht.
- **Busse und Bahnen nach dem VBB-Fahrplan:** Busse, Straßenbahnen, S- und U-Bahnen fahren ihre echten Linien im
  Takt des Fahrplans zur Spieluhr. Busse halten an ihren Halten (Wartende steigen ein) und nutzen Busspuren, Straßenbahnen
  fahren auf Gleisen in der Straße und klingeln, wenn man im Weg steht; S- und U-Bahnen sieht man auf Hochbahn und
  Bahndamm, im Tunnel hört man sie. Fahrplandaten: VBB Verkehrsverbund Berlin-Brandenburg GmbH (CC BY 3.0).
- Mission „Kisten für den Kiez“: Auftrag am Späti in der Wrangelstraße (Wrangelkiez) annehmen → zur Lagerhalle in
  Neukölln fahren → dort anhalten und **A halten** zum Einladen → zurück zur Wrangelstraße → abliefern. Das Zeitlimit
  berechnet der Karten-Build aus der kürzesten Route (derzeit 8,7 km → 930 s). Scheitern bei Zeitablauf oder wenn das
  Auto mit der Ware zum Wrack wird. Lohn mit Zeitbonus und Schadensabzug, Bestzeit. Die Orte stehen in
  [`data/places.json`](data/places.json) und lassen sich austauschen.
- HUD (schlank, ohne Kästen, Schrift mit Kontur): echter Straßenname (an Kreuzungen „A / B“, sonst Kiez oder Bezirk), Geld, Auftrag und Timer, Minikarte mit Lebensleiste, Richtungspfeil mit Entfernung, runder Tacho mit Drehzahl und Gang, Fahrzeugname beim Einsteigen,
  Stadtplan von ganz Berlin auf der Ansicht-Taste (Mausrad zoomt bis auf Straßenebene, Ziehen verschiebt). Die
  Beschriftung folgt dem Maßstab wie bei kiez-finder: ganz Berlin mit den 12 Bezirken, näher die Ortsteile (Tegel,
  Prenzlauer Berg …), dann die Kiez-Namen aus OSM (Flughafenkiez, Reuterkiez … – 568 Kieze) und Bahnhöfe, ab etwa 3 m
  je Bildpunkt die Namen der Hauptstraßen und ab 2,6 m aller Straßen, entlang der Straße und ohne Überlappung.
  **Teleport:** Klick auf den Stadtplan, dann Bestätigungsdialog; auf offenem Grund (Park, Feld, Platz, Hof) landet
  man genau dort bzw. an der nächsten freien Stelle bis 30 m – zu Fuß wie im Auto, etwa mitten auf dem Tempelhofer
  Feld; auf einer Straße oder einem Haus zu Fuß auf dem nächsten Gehweg, im Auto auf der nächsten Fahrspur in Fahrtrichtung, Verkehr und Passanten entstehen sofort am
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
- **Häuser:** Dachform aus OpenStreetMap (`roof:shape`), sonst nach Gebäudeart, Höhe und Typ geschätzt: Berliner Dach
  (geneigter Ziegel- oder Schieferstreifen zu Straße und Hof um eine flache Mitte), Sattel-, Walm-, Zelt-, Mansard-,
  Pult-, Tonnen- und Kuppeldach als einzelne, nach Sonnenstand schattierte Dachflächen mit Ziegelreihen, Graten,
  Gauben und Schornsteinen; Flachdach mit Attika, Kies, Schornsteinen, Lichtschächten, Oberlichtern, Lüftungsgeräten,
  Solarmodulen und Dachterrassen; Wellblech auf Hallen. Farben aus OSM (`roof:colour`, `building:colour`, Material),
  sonst aus Paletten je Stil und Bezirk (Ziegel, Schiefer, Kupfer, Gründach; Stuck im Altbau, Putzvillen, farbige
  Platte in Marzahn-Hellersdorf und Lichtenberg); Fassaden als Altbau, Plattenbau, Neubau oder Industriebau;
  Kontaktschatten am Fassadenfuß.
- **Autos und Menschen:** 37 Pkw-Modelle sowie Nutzfahrzeuge und Zweiräder mit detaillierten Karosserien, Scheiben,
  Spiegeln und lenkenden Vorderrädern, Blinker der KI vor dem Abbiegen, Rückfahrlicht; Passanten mit Armen und Beinen
  im Gang, verschiedener Kleidung und Haarfarbe, manche mit Rucksack oder Tasche.
- **Kämpfen:** zu Fuß Fäuste, Tritte, Baseballschläger, Messer, Pistole, Maschinenpistole und Schrotflinte (alle von
  Anfang an, Munition unbegrenzt mit Nachladen). Zielen in Blickrichtung mit Zielhilfe, mit der Maus auf den Zeiger.
  Kugeln stoppen an Hauswänden, beschädigen Autos bis zum Wrack (der Fahrer flieht). Verletzte NPCs bleiben auf den
  Beinen, fliehen oder wehren sich; ein Lebensbalken zeigt ihren Zustand. Erst tödlicher Schaden lässt sie umfallen. Schüsse vertreiben die Passanten ringsum. Etwa jeder siebte Passant wehrt
  sich mit den Fäusten. Die Spielfigur hat 100 Lebenspunkte (heilen nach einer Pause); bei 0 wacht man im nächsten
  echten Krankenhaus auf, 10 % des Geldes sind weg, ein laufender Auftrag scheitert.
- **Radfahrer:** Radfahrer und E-Roller-Fahrer können getroffen und vom Fahrzeug geholt werden; die Zielhilfe erfasst
  sie. Nicht tödlich verletzte Fahrer fliehen, statt aufgrund des Treffers liegenzubleiben. **Kapern** mit F/Y direkt
  neben dem Rad (fahrende sind schnell – im Sprint einholen). Man fährt selbst mit bis zu etwa 26 km/h auf dem
  Fahrrad bzw. 20 km/h auf dem E-Roller; die Kamera bleibt nah. Absteigen ebenfalls mit F/Y, das Rad bleibt stehen.
  Kisten passen nicht aufs Rad. `auto fahrrad` / `auto e-roller` stellt eines bereit.
- **Jedes Fensterformat:** Grundformat ist 16:9 (1280 × 720). Es wird so skaliert, dass es ganz ins Fenster passt;
  breitere oder höhere Fenster zeigen mehr Stadt, das HUD hängt an den Rändern, Menüs liegen mittig im 16:9-Rahmen.
- **Maus:** alle Menüs (Titel, Pause, Ergebnis, Steuerung) mit Zeigen und Klicken bedienbar, eigener Mauszeiger im
  Stil des Spiels (gelber Pfeil, heller Pfeil über Anklickbarem, Zielkreuz und Verschiebe-Pfeile auf dem Stadtplan);
  beim Fahren und Laufen blendet er sich aus, wenn die Maus 2 s ruht.
- Startmenü, Pause, Mission neu starten, Speichern (ein Speicherplatz, automatisch nach jedem erfüllten Auftrag), Fortsetzen.
  Alles komplett mit dem Controller bedienbar.

## Steuerung

Im Menü **Steuerung** wählst du mit **←/→** das Schema (Diablo/Klassisch).
Mit **↓** vertauschst du die Maustasten für Bewegung und Angriff. Beide Einstellungen werden lokal gespeichert.

- **Standard:** linke Maustaste zum Laufen, rechte Maustaste zum Angreifen mit der gewählten Waffe.
- **Vertauscht:** rechte Maustaste zum Laufen, linke Maustaste zum Angreifen.
- **Einsteigen:** am Fahrzeug **F** drücken. Ein einfacher oder doppelter Mausklick steigt nicht ein.
- **Bahnhof betreten:** nahe einem unterirdischen Bahnhofseingang **F** drücken. Hineinlaufen allein genügt nicht.
- **Waffenrad:** linke und rechte Maustaste gleichzeitig gedrückt halten; eine Taste allein öffnet es nicht.
  Mit dem Zeiger die Waffe wählen, Loslassen übernimmt. **Esc** bricht ab. Am Controller bleibt **LB halten**.
- **WASD/Pfeiltasten**, **Umschalt** zum Sprinten, **Alt** zum langsamen Gehen und **V** zum Treten bleiben verfügbar. **Leertaste** lässt die Figur zu Fuß springen.
- **M** schaltet den gesamten Spielton stumm bzw. wieder ein; der große Stadtplan liegt jetzt auf **Tab**.

Die Mausumstellung ist im Code umgesetzt; der aktuelle automatisierte Prüflauf enthält noch alte Erwartungen an
Klick-Einstieg und Mauskampf. Siehe [Prüfstatus](docs/SPIEL-UPDATE-2026-10-01.md#prüfstatus-und-grenzen).

Zu Fuß: Joggen 3,5 m/s, Gehen 1,5 m/s (Alt oder Stick bis etwa halb ausgelenkt), Sprint 7 m/s mit Ausdauer (reicht etwa 8 s, erholt sich
nach 1 s Pause). Tastaturbewegung ist digital; der linke Controller-Stick hat eine Totzone von 22 % und wählt danach
zwischen Gehen und Joggen. Die Auslenkung dosiert die Gehgeschwindigkeit nicht stufenlos. Im Auto lenkt der linke Stick
analog; RT (Gas) und LT (Bremse) reagieren ebenfalls auf den jeweiligen Druckweg. W/S geben dagegen volles Gas bzw.
volle Bremse, solange die Taste gedrückt ist. Die Kamera zoomt zu Fuß näher heran (2,0); ab Zoom 1,7 zeigt die Detailstufe Berliner Gehwege
(Granitplatten mit Mosaikstreifen), Granit-Bordsteine und Baumscheiben. Zielhilfe mit der Maus rastet nur ein, wenn der
Zeiger auf dem Ziel liegt; die Streuung wächst mit der Bewegung (Stehen am genauesten, Sprinten am ungenauesten). Mit
dem Controller bleibt die Zielhilfe wie gehabt.

| Aktion | Xbox-Controller | Tastatur |
|---|---|---|
| Laufen / Lenken | Linker Stick | WASD / Pfeiltasten |
| Sprinten | A halten oder Stick voll | Umschalt |
| Springen (zu Fuß) | – | Leertaste (am Boden) |
| Gas / Bremse, Rückwärts | RT / LT | W / S |
| Handbremse | RB oder B | Leertaste |
| ESP im Auto umschalten | – | X |
| ABS im Auto umschalten | – | Y |
| Ein-/Aussteigen | Y | F |
| Aktion (Auftrag, Einladen, Abliefern) | A | E |
| Mitfahren / Aussteigen (Bus, Straßenbahn, S-, U-Bahn) | Steuerkreuz unten | G |
| Bahn führen (am Führerstand vorn) / aussteigen | Y | F |
| Als Zugführer: Fahrt / Bremse / Notbremse | RT / LT / B | W / S / Leertaste |
| Als Zugführer: Türen auf/zu, am Endhalt wenden | A | E |
| Befehlszeile (Uhrzeit, Wetter, Teleport, Cheats …) | – | Enter; Tab/→ ergänzt, ↑↓ wählt bzw. blättert im Verlauf, Enter führt aus, Esc schließt |
| Hupe | X | H |
| Stadtplan | Ansicht-Taste | Tab |
| Gesamten Ton an/aus | – | M |
| Menüs | Steuerkreuz, A / B | Maus: zeigen wählt aus, Klick bestätigt; Tastenhinweise (A/B) sind anklickbar |
| Stadtplan zoomen / verschieben | – | Mausrad / Ziehen |
| Teleport (auf dem Stadtplan) | – | Mausklick auf die Karte, dann Ja/Nein (Maus, A/Enter, B/Esc) |
| Angreifen / Schießen (zu Fuß) | RT | rechte Maustaste (vertauscht: linke), oder Strg |
| Zielen (zu Fuß) | rechter Stick | Maus (Figur zielt auf den Zeiger) |
| Treten (zu Fuß) | B | V |
| Waffe wechseln / wählen | LB tippen (zurück) / RB (vor) | Q, Mausrad / 1–6 |
| Waffenrad (zu Fuß) | LB halten, rechter Stick zeigt, LB loslassen wählt, B bricht ab | linke und rechte Maustaste gleichzeitig halten, Maus in Richtung der Waffe, loslassen wählt; Mausrad dreht, 1–6 wählt sofort, Esc bricht ab (Zeitlupe, solange offen; auch beim Schießen) |
| Nachladen | X | R |
| Pause | Menü-Taste | Esc / P |
| Statistik | Menü „Statistik“ (Titel und Pause) | dito, oder Befehl `stats` |
| Menüs | Steuerkreuz/Stick, A wählen, B zurück | Pfeile, Enter, Esc |

**Springen und Schwimmen:** Zu Fuß springst du mit **Leertaste**; lenke währenddessen weiter mit WASD/Pfeiltasten.
Ein Sprung kann Zäune und niedrige Uferkanten überwinden. Hauswände und andere Hindernisse bleiben undurchlässig.
Läufst oder springst du auf einer Wasserfläche ins Wasser, schwimmt die Figur automatisch. Im Wasser bewegst du
dich weiter mit WASD/Pfeiltasten, aber langsamer als an Land; Sprinten ist dort deaktiviert. Schwimme einfach in
Richtung des Ufers, um wieder an Land zu gelangen. Springen ist nur an Land möglich. Schwimmen funktioniert auch
mit dem linken Gamepad-Stick; für Gamepads gibt es derzeit keine Sprungtaste.

## Auf dem Mac spielen und testen

Voraussetzung: Node.js ≥ 20. Keine weiteren Abhängigkeiten, kein `npm install` nötig.

```bash
npm start          # Dev-Server auf http://localhost:8080 (anderer Port: PORT=9000 npm start)
npm test           # Gesamtsuite; aktuellen Prüfstatus unten beachten: Kartenpipeline, Karte, Kollision, Fahrphysik, Verkehr, Passanten, Mission, Speichern, Menüs, Eingabe
```

### Karte neu erzeugen

Die fertige Karte liegt in `web/data/berlin/` (`index.json` mit Grenzen, Ortsteilen und Missionsorten, `overview.json`
für den Stadtplan, `tiles/<x>_<y>.json` je 640 × 640 m; zusammen 138 MB) im Repository; dafür ist kein Netz nötig.
Neu bauen, z. B. für aktuellere OSM-Daten oder andere Missionsorte:

```bash
npm run map:fetch                     # OSM-Auszug Berlin (Geofabrik, PBF ~100 MB), LOR-Grenzen, Baumbestand, Einwohnerdichte und Verkehrsmengen (WFS Geoportal Berlin), VBB-Fahrplan (GTFS ~80 MB) → data/raw/ (gitignored, ~3 min)
npm run map:transit                   # data/raw/gtfs.zip → web/data/berlin/transit.json (Busse und Bahnen, ~20 s, ohne die Karte neu zu bauen)
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
  src/roofs.js       Dachform, Dachflächen, Gauben, Fassadenstil und Dachaufbauten je Gebäude (ohne Canvas)
  src/buildcolors.js Fassaden- und Dachfarben aus OSM, Material, Stil und Bezirk (ohne Canvas)
  src/vehicles.js    Automodelle, Sprite-Cache, Räder, Licht, Blinker
  src/combat.js      Waffen, Zielhilfe, Schüsse (Strahltest), Nahkampf, Treffer (ohne DOM)
  src/hud.js         HUD, Menüs, Overlays (Title-Safe-Rand 5 %)
  src/audio.js       synthetisierte Klänge (Motor, Reifen, fremde Autos, Umgebung, Nachtleben, Schritte)
  src/ambience.js    Umgebungsmischung an der Kamera (ohne Web Audio)
  src/soundscape.js  Drehzahl/Gänge, Reifenzustand, Stimmen fremder Autos mit Doppler, Schritte (ohne Web Audio)
  src/nightlife.js   Bar-Auslastung: Feed-Parser, Verlauf nach Uhrzeit/Wochentag, was man vor Bars hört
  src/projection.js  Kartenprojektion (geteilt mit dem Karten-Build)
  src/assets.js      Platzhaltergrafiken + Austausch per manifest.json
  assets/            manifest.json, eigene Sprites/Sounds
xbox/                UWP-Hülle (C#, WinUI 2 WebView2) für Visual Studio
data/places.json     Missionsorte (lat/lon bzw. OSM-Weg der Lagerhalle)
tools/osm/           Kartenpipeline: fetch.mjs, pbf.mjs (PBF-Leser), store.mjs, build.mjs, crosssection.mjs, tiles.mjs, preview.mjs, geo.mjs
tools/serve.mjs      Dev-Server (mit BARS_URL: reicht den Bar-Feed durch)
tools/fetch-bars.mjs Bar-Feed-Schnappschuss nach web/data/bars.json
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

## Menschen

Passanten sind nicht mehr alle gleich: Alltag, Büroleute (Anzug, Aktentasche), Touristen (Cap oder Sonnenhut, Kamera,
Rucksack), Senioren (Mantel, Hut, Stock, gebeugt und langsam), Jugendliche (Hoodie, Kopfhörer), Kiez (Beanie, Bart,
Jutebeutel), Handwerk (Warnweste, Helm), Punks (Iro, Nieten), Frauen mit Kopftuch, Eltern mit Kinderwagen, Jogger und
Gassigeher. Wer unterwegs ist, hängt von Bezirk, Uhrzeit, Wochentag und Tätigkeit ab; der Typ bestimmt auch das
Gehtempo. Alle – auch der Spieler – laufen mit echtem Gangbild: Schritte und Armschwung im Takt des Tempos, Rennen mit
Vorlage und langen Schritten, weiches Anlaufen und Anhalten, Atmen im Stand. Der Spieler läuft in Laufrichtung und
dreht den Oberkörper zum Ziel (entgegen der Zielrichtung geht er rückwärts). Prüfseite: `http://localhost:8080/lab/figures.html`.

Die Figuren sind von oben wie echte Menschen aufgebaut: Schultern als gerundetes Rechteck (Anzug eckig, Mantel länger),
Arme aus Ober- und Unterarm mit Ellbogen (kurze Ärmel lassen die Unterarme frei), Hände, Schuhe mit Sohle, Kopf mit
Ohren, Nasenspitze, Haaransatz und Strähnen; Kleidung mit Nähten, Kragen, Revers, Kapuze, Reflexstreifen und
Rucksackgurten; Mützen mit Strickrippen und Bommel, Caps mit Schirm und Nähten, Hüte mit Band, Bauhelme mit Grat, Kopftücher
mit Falten. Rumpf und Kopf haben Volumen (Mitte heller, Rand dunkler) und werden je Aussehen einmal als Bild gerechnet;
die Sonne hellt die zugewandte Schulter auf. Liegende zeigen das Gesicht. Der Spieler trägt einen Ring am Boden.
Fertige Figuren aus freien Paketen (z. B. Kenneys CC0-„Topdown Shooter“) passen nicht: nur wenige Charaktere in festen
Posen ohne Gehzyklus, Sitzen, Liegen und Kleidungsvarianten. Eigene Bilder lassen sich weiter über
`web/assets/manifest.json` (`player`, `pedestrian`) einsetzen.

## Fahrzeuge und Fahrgefühl

37 Pkw-Modelle fahren durch Berlin, jedes mit eigenem (erfundenem) Namen und eigener Technik – die neueren nach echten
Vorbildern: Supersportwagen mit V10-Mittelmotor und Allrad („Oberbaum Tempesta“, 640 PS), GT-Coupé mit Front-Mittelmotor
(„Tempelhof GT 40“), leichter Mittelmotor-Zweisitzer („Adlershof Flèche“), Elektro-Sportlimousine („Voltwerk Blitz GT“,
760 PS), Sportwagen mit Mittelmotor („Oberbaum Furia“) und Heckmotor-Coupé („Wannsee Boxer 6“); Sportcoupé („Wannsee Coupé
240“) und leichtes Drift-Coupé („Lausitz Hachi“); Limousinen vom Taxi über die Businesslimousine („Teltower T5“) und
Sportlimousine („Teltower T3 RS“) bis zur Luxuslimousine („Grunewald Senator“); Kombis („Märker Allwetter“, „Teltower T4
Tourer“, Power-Kombi „Märker RS Avant“ mit 600 PS); Transporter, Großraumtransporter („Spree Großraum 316“),
Hochdachkombi und Kleinbus mit Heckmotor; Geländewagen („Grunewald Keiler“, kantiger „Grunewald Kommandant“, „Grunewald
Förster 110“, kleiner „Lausitz Taiga“) und SUVs („Spree Tiga“, „Oberbaum Cayo“, „Teltower TX7“,
„Voltwerk E-Terra“); dazu Kleinwagen, Kompakt, Hot Hatch, Zweitakter, Roadster, Muscle-Car, Oldtimer, Pick-up und
Rallye-Kompakt. Dazu **Motorrad und Motorroller**: zu viel Gas hebt das Vorderrad, zu hartes Bremsen das Hinterrad
(deshalb bremsen sie länger als Autos), in Kurven legt sich der Fahrer hinein, ein Aufprall ab ≈ 27 km/h wirft ihn ab.
Beim Einsteigen blendet das HUD Name und Technik ein (ohne Kasten, Schrift mit Kontur), z. B. „Oberbaum Furia –
Sportwagen · Mittelmotor · RWD · 435 PS“ (Antrieb FWD/RWD/AWD, Leistung in PS). Der Tacho unten rechts zeigt km/h,
die Drehzahl als Bogen mit rotem Bereich, den Gang (R rückwärts, D beim Elektroauto), Antrieb sowie ESP- und ABS-Status.
`auto sportwagen` (oder ein anderes Modell, `auto motorrad`, `auto roller`) stellt eines neben
dich.

Das selbst gefahrene Auto nutzt ein zugängliches, arcadiges Fahrmodell: Mit Heckantrieb lässt sich das Heck beim Gasgeben
in der Kurve bei ausgeschaltetem ESP leichter zum Driften bringen; Frontantrieb schiebt eher über die Vorderräder, Allrad zieht stabil heraus.
Mittelmotoren lenken spontan ein, hohe Fahrzeuge wanken sichtbar. **Alle Fahrzeuge haben ESP und ABS; beide sind standardmäßig aktiv.** **X** schaltet ASR/ESP während der Fahrt um; mit ESP
an wird das Auto leichter abgefangen, aus schaltet es freier und driftet stärker (die ESP-Leuchte zeigt den Eingriff bzw.
den abgeschalteten Zustand). **Y** schaltet ABS um; ohne ABS können die Räder bei starkem Bremsen blockieren und das Auto
lenkt schlechter. Ein kurzer Druck auf die **Handbremse** (Leertaste/B) leitet einen Drift ein; mit Gegenlenken steuerst
und beendest du ihn. ESP kann auch über `esp an` oder `esp aus` in der Befehlszeile gesteuert werden. Der übrige Verkehr fährt weiter mit der einfachen Physik.

**Kräftiger Antritt:** Die neue Abstimmung verstärkt den Start insbesondere bei Front- und Hecktrieblern und lässt
den Bonus zwischen 20 und 90 km/h weich auslaufen. Bereits sehr schnell startende Sport-/Allradautos erhalten keinen
zusätzlichen Gripbonus. Schnee, Eis, Lenkeinschlag und Drifts begrenzen die Hilfe. Beispiel Musclecar: 0–50 km/h
**2,19 → 1,62 s**, 0–70 km/h **3,08 → 2,38 s** (Simulation, trocken, ESP an). Oberhalb des Stadtbereichs nimmt der
Schub stärker ab; die Höchstgeschwindigkeiten bleiben modellabhängig erhalten. Herstellerquellen, bewusste
Arcade-Abweichungen und sämtliche Messwerte: [Beschleunigungsbericht](docs/FAHRZEUG-BESCHLEUNIGUNG.md).

**Detailliertere Fahrzeuge:** modellabhängige Karosseriekonturen, lackierte Flächen mit Lichtverläufen, Fenster,
Dachsäulen, Tür- und Haubenfugen, Spiegel, Griffe, Grill, Stoßfänger, Leuchten und Varianten wie Streifen,
Lufteinlässe oder Dachreling. Der Sprite-Cache rechnet die Bilder in vierfacher Auflösung; die Kollisionsmaße bleiben gleich.

**Dunklerer Motorklang:** eigene Syntheseprofile für Drei- bis Zehnzylinder, Boxer, Diesel, Zweitakter, Motorräder und
Elektroantriebe. Zündimpulse, Ansaugung, Lastwechsel, Turbogeräusch, Rückwärtsgang und Innenraumdämmung prägen den
Klang. Mehr tiefe Anteile, weniger scharfe Obertöne und zurückgenommene Pfeifgeräusche. Auch die Verkehrsautos
verwenden das Profil ihres Modells samt Dopplereffekt. Es sind synthetische Klänge, keine Originalaufnahmen.
Elektroautos haben keinen Verbrennertakt und bleiben im Stand ohne Motorgeräusch.

## Nahverkehr

Busse, Straßenbahnen, S- und U-Bahnen fahren nach dem echten VBB-Fahrplan. **Mitfahren:** neben einem Wagen **G**
(Steuerkreuz unten) – auch in Fahrt (ab 25 km/h aufspringen); **G** noch einmal steigt aus, schnell gefahren ist das
Abspringen mit Sturz (ab 40 km/h mit Schaden). Unter Tage und auf der Hochbahn geht das nur am Bahnhof, man kommt am
Straßenausgang heraus; in einen Zug auf dem Viadukt steigt man von der Straße nur, während er im Bahnhof hält. Oben mittig zeigt eine Leiste Linie, Ziel und nächsten Halt.

**Selbst fahren:** vorn am Führerstand einer Straßenbahn, S- oder U-Bahn **Y/F** – der Zug verlässt den Fahrplan und
gehört dem Spieler (Höchsttempo 60/70/100 km/h); einen stehengelassenen eigenen Zug übernimmt man am Führerstand wieder. Gas und Bremse wie im Auto, Notbremse Leertaste/B. An einer
Haltestelle im Stand öffnet **E/A** die Türen (sie schließen nach 20 s selbst); ein sanfter, genauer Halt bringt bis zu
10 € Trinkgeld. Vor einem Zug auf derselben Strecke und vor Hindernissen auf dem Straßenbahngleis bremst der Zug von
selbst; Fahrplanzüge dahinter warten. Am Endhalt wendet **E/A** auf die Gegenrichtung, wo es eine gibt. Unter Tage wird
die Stadt abgedunkelt, die Röhren mit Bahnsteigen, Bahnhofsnamen und den Zügen darin erscheinen (Tunnelansicht).

Straßenbahnen haben Vorrang: Wo ihr Linienweg in der Gegenspur liegt (vor allem Linie 21, Teile von M10 und M13),
setzen entgegenkommende Autos und Busse zurück, bis die Bahn vorbei ist. Grenzen: keine Signale und Weichen.

**U-Bahnhöfe betreten:** An jedem unterirdischen U- und S-Bahnhof führen Treppenabgänge (blaues U- bzw. grünes
S-Schild) am Gehweg nach unten: je einer über den Enden des Bahnsteigs und einer am U-/S-Symbol der Station. In der Nähe **F** drücken (Hinweis „Hinunter zur U-Bahn …“); die Minikarte zeigt die Eingänge. Nach dem Betreten sieht man statt der Stadt den
Bahnhof: Mittelbahnsteig mit Treppen, Säulen, gefliesten Wänden in der Farbe des Bahnhofs, Namensschildern,
Fahrgastanzeigen (Linie, Ziel, Minuten) und wartenden Fahrgästen. Die Züge kommen nach dem echten Fahrplan, halten und
fahren weiter. An der Bahnsteigkante neben einem haltenden Zug steigt man mit **G** ein; **G** am nächsten
unterirdischen Halt steigt dort auf den Bahnsteig aus. Die Treppen an beiden Enden führen zum jeweiligen Straßenausgang.
Unten ist man von der Straße getrennt (keine Ziele, kein Verkehr, gedämpfter Klang); ein Spielstand legt die Figur an
den Ausgang oben. Die Bahnhöfe werden aus dem Fahrplan erzeugt und liegen unter der echten Strecke; ihre Innenräume
sind schematisch, durch die Tunnel laufen kann man nicht.

## Wetter und Fahren

Das Wetter wirkt aufs Fahren. Auf nasser Straße wird der Bremsweg etwa ein Drittel länger, auf Schnee etwa doppelt,
auf Glätte etwa dreimal so lang; Anfahren, Seitenhalt und Lenkung lassen entsprechend nach, auf Glätte drehen die Räder
durch. **Glätte** entsteht, wo es nass ist und die Temperatur (neben der Uhr) unter 0 °C fällt – an Wintertagen oft
früh morgens; Brücken frieren zuerst, unter Brücken, Hochbahn-Viadukten und in Durchfahrten bleibt die Straße trocken. Wer schneller als
etwa 70 km/h durch eine der sichtbaren **Pfützen** fährt (gut die Hälfte liegt in der Fahrspur, der Rest im Parkstreifen), schwimmt kurz auf (**Aquaplaning**, mit
Spritzwasser). **Sturmböen** versetzen Autos quer, auf Brücken stärker, leichte Autos mehr als Transporter. Der
**KI-Verkehr** fährt bei schlechtem Wetter langsamer, bremst sanfter und hält mehr Abstand. Als **Zugführer** bremst
man auf nassen oder vereisten Schienen schlechter (im Tunnel ist es trocken), die Zwangsbremsung hält trotzdem vor
jedem Hindernis. Über dem Tacho warnt ein Schild: Aquaplaning, Glätte, Schnee, Sturm, Nässe. Im **Schnee** hinterlassen alle Autos Reifenspuren – vier Räder genau unter der Karosserie (in Kurven und beim Driften laufen Vorder- und Hinterräder auseinander), Zweiräder eine Spur; sie verblassen nach einigen Minuten, bei Schneefall schneller.

## Klang und Nachtleben

Alle Klänge sind selbst synthetisiert (Web Audio, keine Samples). Das eigene Auto hat Drehzahl und Gänge (Pkw,
Transporter und Rettungswagen mit Diesel-Nageln, Lkw, Müllauto und Bus tief und langsam), Reifen rumpeln auf
Kopfsteinpflaster, zischen auf nasser Straße, knirschen im Schnee und quietschen beim Rutschen; im Auto klingt die
Stadt draußen dumpf, Regen trommelt aufs Dach. Die vier nächsten fremden Autos sind einzeln zu hören, mit Richtung und
Dopplereffekt. Zu Fuß hört man die eigenen Schritte (Schnee knirscht, Nässe platscht).

**Nachtleben:** Vor Bars, Kneipen, Biergärten und Clubs (aus OpenStreetMap) gibt es abends Stimmengewirr, Lachen,
Gläserklirren und gedämpften Bass, am meisten Freitag- und Samstagnacht, Clubs erst ab 23 Uhr. Bei Regen wird es
draußen leiser.

**Bar-Auslastung (gostumblr):** Die Bars aus [gostumblr](https://github.com/pepperonas/gostumblr) (eigener Server,
`https://app.gostumblr.com/api/v1/bars/busyness`, ohne Anmeldung) sind am lautesten, und davor stehen mehr Leute –
auch Bars, die OpenStreetMap nicht kennt, klingen an ihrer Koordinate. Aus der Antwort zählen Name, `latitude`/
`longitude`, `occupancy_percent` (live), `usual_percent` (Googles „üblich“), `last_scraped` und der 24-h-Verlauf
`trend`; dazu holt das Spiel den Wochenschnitt aller Bars (`…/busyness/weekly?dow=0…6`, 0 = Sonntag). Daraus entsteht
je Bar ein Stundenprofil für die ganze Woche: Form des Wochenschnitts × Beliebtheit der Bar (üblich zur Messzeit
gegenüber dem Schnitt), überschrieben mit den echten Messungen der letzten 24 h – alles in Berliner Zeit, abgespielt
zur Spieluhr. Bars werden per Name (in der Nähe) den OSM-Lokalen zugeordnet.

```bash
npm start                # Dev-Server holt gostumblr live (alle 2 min) und liefert es unter data/bars.json aus
BARS_URL=aus npm start   # ohne gostumblr: Nachtleben nur nach den OSM-Lokalen
npm run bars:fetch       # Schnappschuss nach web/data/bars.json (für die Xbox vor prepare-xbox; nicht im Git)
```

`BARS_URL=https://…` nimmt eine andere Quelle (auch eine Liste, `{ "bars": [...] }`, GeoJSON oder Google-„Stoßzeiten“),
`BARS_TOKEN` schickt einen Bearer-Token mit. Direkt im Browser (`?bars=https://…`, Befehl `bars https://…`, `bars neu`,
`bars aus`) geht es nur, wenn gostumblr die Herkunft per CORS erlaubt (`CORS_ORIGINS`, ab Werk nur gostumblr selbst
und localhost:5173) – über den Dev-Server ist das nicht nötig. Das Spiel lädt den Feed alle 5 min neu.

## Befehlszeile und Statistik

**Enter** öffnet im Spiel eine Befehlszeile (die Welt steht still, solange sie offen ist). Beim Tippen erscheinen
Vorschläge – Befehle, erlaubte Werte, bei `tp` alle Orte Berlins (Bezirke, Ortsteile, Bahnhöfe, Kieze, Straßen) –, der
erste steht grau hinter dem Getippten und kommt mit **Tab**, **→** oder per Mausklick in die Zeile. `hilfe` listet alles.

Sie arbeitet wie eine Befehlspalette:
- **Ohne Befehlswort:** `22:30`, `nacht`, `Freitag`, `regen`, `schneesturm` oder ein Ort wie `alexanderplatz` genügen.
- **Tippfehler** werden verziehen (`wetter schneestrum`, `tp kotbusser tor`). Bei einem vertippten Befehl kommt ein
  Hinweis „meintest du …“.
- **Enter** übernimmt zuerst einen abweichenden Vorschlag sichtbar; ein weiteres **Enter** führt die vervollständigte
  Zeile aus. Mit **↑/↓** wählst du zuerst einen Vorschlag und übernimmst ihn mit **Enter**.
- `leben` stellt die Spieler-Gesundheit wieder her und repariert das eigene Fahrzeug vollständig; `reparieren` repariert
  das Fahrzeug separat.
- Nach Erfolg **schließt** sich die Zeile, die Meldung bleibt kurz stehen; **Umschalt+Enter** lässt sie offen. Bei
  Fehlern bleibt sie mit Hinweis offen.
- Über der Eingabe steht, wie der gerade getippte Befehl aufgebaut ist. Beim Öffnen stehen die **zuletzt benutzten**
  Befehle oben (anklicken führt sie aus).
- **Esc** leert zuerst die Zeile, beim zweiten Mal schließt es; **Strg/Alt+Rücktaste** löscht ein Wort; **↑/↓** blättern
  im Verlauf bzw. in den Vorschlägen.

| Befehl | Wirkung |
|---|---|
| `zeit 21:30` · `zeit 7` · `zeit abend` | Uhrzeit (auch `21.30`, `2130`, `9h`; Wörter: morgen, mittag, nachmittag, abend, daemmerung, nacht, mitternacht) |
| `tag Freitag` · `wochentag Sonntag` · `tag Mo` | Wochentag wählen, mit Vorschlägen für alle sieben Tage. Auch `tag 1` bis `tag 7` (Montag bis Sonntag); `tag` zeigt den aktuellen Tag. Uhrzeit bleibt erhalten. |
| `tempo 10` · `tempo 0` | Tempo der Spieluhr (0 = Uhr steht) |
| `wetter schneesturm` · `wetter auto` | Wetter festlegen bzw. wieder natürlich |
| `schnee 0.5` · `nass 1` · `glaette 0.8` | Schneedecke / Nässe / Glätte der Straßen 0–1 (Glätte taut über 0 °C) |
| `temp` · `temp -5` · `temp auto` | Temperatur zeigen, erzwingen, wieder natürlich |
| `verkehr 2` · `passanten 0` | Dichte von Autos / Fußgängern |
| `tp kottbusser tor` | Teleport ohne Rückfrage (lädt den Stadtteil nach) |
| `geld +1000` · `leben` · `munition` · `gott an` · `auto musclecar` (`spawn sportwagen`) · `reparieren` | Schummeln (wird in der Statistik gezählt); `leben` heilt und repariert das eigene Auto, `reparieren` nur das Auto |
| `fps` · `ebenen` · `silhouetten` · `qualitaet niedrig` | Anzeigen zum Prüfen: Bildrate, Brückenebenen, Umrisse, Zeichenqualität |
| `bars` · `bars https://…` · `bars neu` · `bars aus` | Bar-Auslastungs-Feed zeigen, setzen, neu laden, abschalten |
| `stats` | Statistik öffnen |

**`wetter` + Enter** öffnet eine übersichtliche Wettertafel. Mit **↑/↓** wählst du Wettertyp, Temperatur, Schneedecke,
Straßennässe oder Glätte; **←/→** ändert den Wert direkt. **Enter** übernimmt und schließt die Tafel, **Esc** geht zurück.
Natürliches Wetter ist über `auto` erreichbar, die Temperatur über `temp auto`. Die Tafel verändert Werte sofort; Esc ist kein Rückgängig.

### Wochentag wählen

**Enter → `tag Freitag` → Enter** setzt den Wochentag. Die Argumentvorschläge zeigen alle sieben Tage.
`tag` allein zeigt den aktuellen Tag. Alternativen: `wochentag Sonntag`, `day Montag`, `tag Mo`, `tag Mo.`,
`tag 1` bis `tag 7` (Montag bis Sonntag); `Sonnabend` wird als Samstag verstanden. Auch `Freitag` allein funktioniert.
Groß-/Kleinschreibung spielt keine Rolle. Eine Zahl allein bleibt eine Uhrzeit: `7` bedeutet 07:00, `tag 7` Sonntag.
Die Uhrzeit, der Wetterverlauf und die Wetter-Tagnummer werden nicht vor-/zurückgespult. Verkehr und Nachtleben
lesen den neuen Wochentag beim Weiterlaufen; die Bevölkerung passt sich über ihre normale Verwaltung an.
Der gewählte Tag wird mit dem vorhandenen Spielstand gespeichert. Nach Mitternacht folgt regulär der nächste Tag.

Die **Statistik** zählt je Spiel und über alle Spiele: Strecke (zu Fuß/im Auto), Höchstgeschwindigkeit, Spielzeit,
Brücken, Teleports, Aquaplaning, überfahrene Menschen und Radfahrer, Unfälle, umgefahrene Poller, geklaute Autos, gekaperte Räder, vom Rad geholte Radfahrer, Tote (erschossen
/ Nahkampf), Schüsse, Kugeln, Treffer und Trefferquote **je Waffe**, zerstörte Autos, Aufträge, verdientes Geld,
Krankenhauskosten und Cheats, dazu im Nahverkehr: Mitfahrten, Strecke als Fahrgast und als Zugführer, geführte Bahnen,
bediente Halte, Trinkgeld, Auf- und Abspringen. Gespeichert wird lokal in **IndexedDB** (Datenbank `gta-berlin`, alle 5 s und beim
Verlassen der Seite); ohne IndexedDB zählt sie nur für die Sitzung.

## Stand und Prüfumfang

**Aktueller Prüfstand vom 01.10.2026:** 33 gezielte Tests für Beschleunigung, Fahrdynamik, Fahrzeugbasis und
Motorklang-Zustand bestanden. Im zuvor ausgeführten vollständigen Lauf: 461 Tests, 444 bestanden, 17 fehlgeschlagen.
Alle 17 Fehler sind auch mit der alten Fahrphysik reproduzierbar. Die Suite ist daher **nicht insgesamt grün**.
Der danach ergänzte Wochentag-Befehl wurde nicht erneut automatisiert getestet. Einzelheiten und Grenzen stehen
im [Updatebericht](docs/SPIEL-UPDATE-2026-10-01.md#prüfstatus-und-grenzen).

### Historische Prüfungen früherer Versionen

Die folgende Liste dokumentiert frühere Prüfungen und damalige Bedienregeln. Sie ist kein Nachweis, dass alle
Einträge im aktuellen Entwicklungsstand unverändert gelten; insbesondere Maussteuerung und NPC-Stürze wurden geändert.

- Frühere automatisierte und manuelle Prüfungen, darunter:
  - 0.46.0: Figuren – jeder Typ in allen Haltungen ohne ungültige Werte, Typen verschieden, Rumpf und Kopf als Bild
    gerechnet und beim Gehen wiederverwendet, Bildspeicher begrenzt (älteste zuerst), gleiche Köpfe/Rümpfe geteilt,
    niedrige Detailstufe zeichnet weniger Striche. Im Browser: Prüfseite aller Typen und Haltungen, Spiel bei normalem
    Zoom; Zeichenzeit für 250 Personen ≈ 18 ms statt 15 ms vorher (Software-Renderer ohne Grafikkarte).
  - 0.45.0: alle 37 Pkw erreichen ihr Höchsttempo (± 6 %) und bremsen aus 100 km/h in 18–36 m; alle Modelle kommen im
    Verkehr vor; Schneespuren: Räder eines echten Autos liegen unter der Karosserie an den Flanken (vier Spuren, Spurweite
    = Wagenbreite), Zweirad eine Spur, Drift trennt Vorder- und Hinterradspuren, auf der Brücke keine Spur am Boden;
    HUD in 11 Fenstergrößen (Tacho, Minikarte mit Lebensleiste, Auftrag, Waffe) im Bild und ohne Überlappung.
    Im Browser: Fahrzeug-Einblendung, Drehzahlbogen bei Fahrt, Spuren im Schnee unter den Rädern, Umrisse aller Modelle.
  - 0.38.0: Radfahrer als Ziel – Strahl trifft den fahrenden Radfahrer (nicht das liegende Rad), Schuss holt ihn vom Rad
    (stürzt, Treffer zählt, Statistik), Tritt ebenso; Kapern mit F neben dem Rad (Fahrer runter, Rad aus dem
    Radverkehr, kein Auftragsauto), Vollgas bis höchstens ≈ 26 km/h, absteigen/aufsteigen, E-Roller bleibt E-Roller,
    liegendes Rad ohne Herunterziehen aufheben; Maus: Klick greift an, Doppelklick kapert, liegendes Rad weit weg =
    hinlaufen, nah = aufsteigen. Im Browser: Rad per `auto fahrrad`, Anfahren bis 25 km/h, Darstellung mit Fahrer.
  - 0.37.0: Klick auf ein entferntes Auto läuft nur hin (steht danach am Auto, nicht drin, kein Schuss), Klick
    daneben steigt nach dem Halt an der Tür ein (≥ 0,35 s), Doppelklick aus der Ferne steigt ein. Befehlszeile:
    Uhrzeit/Wetter/Ort ohne Befehlswort, „schnee“ allein = Wetter, mit Wert = Schneedecke, vertippter Befehl gibt
    „meintest du“ statt Teleport, Tippfehler in Werten und Orten gefunden, Enter nimmt bei unvollständiger Eingabe den
    besten Vorschlag, schließt nach Erfolg (Umschalt: offen), bleibt bei Fehler offen, Esc leert zuerst, Wort löschen,
    zuletzt benutzte Befehle oben, Hilfezeile. Im Browser mit echter Maus: Einsteigen per Klick am Auto mit Halt an der
    Tür, `regen` ohne Befehlswort.
  - 0.36.0: Wetterdarstellung – Aufschlagringe (mehr bei Starkregen, junge klein, alte groß und blass), Nässe auf der
    Fahrbahn als eine Ebene (keine dunklen Kreise an Kreuzungen), alle Pfützen gezählt, Bodennebel nur bei Nebel, nasser
    Gehweg nur bei Nässe; alle elf Wetterbilder zeichnen ohne ungültige Koordinaten. Im Browser (Chromium) jede
    Wetterlage bei Tag und Regen bei Nacht angesehen, Wolkenschatten decken bei „wolkig“ rund ein Drittel der Fläche.
  - 0.35.0: Diablo-Klick auf ein geparktes Auto läuft hin und steigt ein, ohne einen Schuss (auch gehalten), Wrack =
    nur hinlaufen, Strg-Klick schießt aufs Auto; gehaltener Bodenklick, dessen Zeiger über eine Person und ein Auto
    wandert, greift nie an und läuft dem Zeiger nach; gehaltener Angriff bleibt bei der angeklickten Person und geht
    nicht auf die daneben über. Waffenrad: öffnet am Zeiger, wählt nach der Richtung des echten Zeigers ab der
    Radmitte (17 Punkte reichen, weit draußen zählt die Richtung), hereingerückte Mitte am Bildrand, Klick ins Rad
    nimmt die Waffe; Zeiger je Absicht (Pfeil/rotes Fadenkreuz/Auto). Im Browser mit echter Maus geprüft: Rad per
    kleinem Ruck und per Klick, Rahmen ums Auto unter dem Zeiger, Klick aufs Auto steigt ein.
  - 0.34.1: gostumblr-Antwort im echten Format (`/api/v1/bars/busyness` + Wochenschnitt, Sonntag zuerst) ergibt je
    Bar ein Stundenprofil in Berliner Zeit (Sommer-/Winterzeit), Beliebtheit skaliert, echte 24-h-Messungen haben
    Vorrang; ohne Wochenschnitt nur gemessene Stunden, sonst typischer Verlauf. Gegen einen nachgebauten gostumblr-Server
    geprüft: `bars:fetch` schreibt den Schnappschuss, der Dev-Server reicht den Feed durch, das Spiel lädt ihn im Browser
    ohne Fehler. **Nicht** geprüft: der echte Server (aus der Testumgebung nicht erreichbar).
  - 0.34.0: Bar-Feed-Parser (Google-Stoßzeiten, Prozent, GeoJSON, Wochentag als Schlüssel, Unbrauchbares fällt weg);
    Nachtleben-Verlauf (Fr/Sa-Nacht voll, Dienstagmittag still, Clubs erst ab 23 Uhr, Feierabendbier, veralteter
    Live-Wert zählt nicht); Projektion im Spiel = Projektion des Builds; Feed-Bar per Name einem echten OSM-Lokal
    zugeordnet und dort am lautesten, Feed-Bar ohne OSM-Eintrag an ihrer Koordinate hörbar und außer Hörweite still,
    Regen macht es leiser, volle Feed-Bar hat mehr Leute davor; Motor schaltet beim Beschleunigen nur hoch, Drehzahl
    zwischen Leerlauf und Abregelung, Diesel tiefer, Gas im Stand; Reifen quietschen trocken, auf Schnee nur Rauschen,
    Fahrtwind ∝ v²; Doppler höher beim Näherkommen, tiefer beim Wegfahren, keiner beim Mitfahren, Richtung, eigenes
    Auto/Wrack/zu weit stumm; Stimmen bleiben ihrem Auto treu; im Auto dumpf; Schritte nach Tempo und Untergrund.
    Im Browser (Chromium) geprüft: Spiel startet mit Feed ohne Fehler, Fahren im Schneesturm ohne Fehler. **Nicht**
    geprüft: wie es klingt (kein Lautsprecher in der Testumgebung) und der echte gostumblr-Feed (Server nicht
    erreichbar, Format daher tolerant geraten).
  - 0.33.0: Pollerreihen (OSM-Linien) werden zu einzelnen umfahrbaren Pollern statt einer Wand (Lücken < 1,8 m);
    Reifenspuren im Schnee (zwei parallele Spuren in Wagenbreite, kein Strich bei Stillstand oder Teleport, verblassen,
    bei Schneefall schneller, nur ohne Schneedecke unsichtbar, im Bild gezeichnet); Schornsteine auf Satteldächern
    exakt im Grundriss.
  - 0.32.0: Laufgeschwindigkeiten und Ausdauer, Passanten-Tempo, Zoom zu Fuß, Wegfindung zu Fuß um Häuser, Klick-Laufen
    (WASD bricht ab), Klick-Angriff und Umschalt-Angriff, Schema-Umschaltung im Steuerungsmenü, Maus-Zielhilfe nur mit
    Zeiger auf dem Ziel, größere Streuung im Laufen, Detailstufe nur bei nahem Zoom; eigene Straßenbahn fährt nach
    20 s Geduld an einem nicht weichenden Auto vorbei; fliehende Passanten betreten keine Häuser mehr.
  - 0.31.0: Straßenbahn-Vorrang (an der M10-Stelle mit Gegenverkehr kommen die eigene Bahn und eine Fahrplanbahn in
    120 s zum nächsten Halt; ohne Zurücksetzen, mit zu enger Seitenprüfung oder ohne Mitzurücksetzen des Hintermanns
    scheitern die Tests); Wenden öffnet keine Türen, erneutes Türöffnen ist kein neuer Halt, ein stehengelassener Zug
    fährt als Fahrplanzug weiter, der Sprung ans Fahrtende ist kein Fußweg; Pfützen in der Fahrspur und Aquaplaning aus
    der Spurmitte, kein Pfützen-Cache am Rand des geladenen Gebiets, trocken ohne Abfrage, Hochbahn überdacht, Pfütze
    unter der Fahrzeugmitte, Warnschild verschwindet mit der Warnung, Zielpfeil weicht dem Schild aus, Temperatur ab
    dem Start, Mindestabstand in der Glätte-Schlange;
  - Wetter-Fahrphysik: Temperaturkurve (Spanne je Tagestyp, 5/15 Uhr, stetig über Mitternacht, erzwungener Schnee
    ≤ +1 °C), Glätte nur bei Nässe und Frost, Spielstand mit Glätte, alter Stand ohne; Haftungsfaktoren (trocken exakt
    1, Mischung mit Untergrenze), überdacht trocken, Brücke glatter, Pfütze nur in der gezeichneten Ellipse und bei
    Nässe; Bremsweg aus 50 km/h ≈ 1 : 1,3 : 2 : 3, Anfahren/Lenken schwächer, Aquaplaning nur über 70 km/h in einer
    echten Pfütze mit genau einem Ereignis, gleich bei 30 und 60 fps; Böen 0,5–1 m Versatz, Brücke stärker, schwere
    Autos weniger, geparkte nie; KI auf freier Strecke langsamer, vor Rot sanfter bremsend und vor der Linie, Schlange
    ohne Auffahren, je 3 min Glätte und Schnee an einer engen Stelle ohne Zusammenstoß und ohne Dauerstau; überdacht nur unter echten Überführungen (nicht an 60 geprüften Brückenanfängen, nicht auf der eigenen Brücke der Straßenbahn); Aquaplaning versetzt die Fahrtrichtung; eigener Zug bremst auf nassen Schienen länger
    (auch in der Welt), Zwangsbremsung unter der verringerten Kurve, Tunnel trocken; Warnschild mit Vorrang und ohne
    Überlappung, Temperatur neben der Uhr, Platschen, Statistik. Jede Schutzprüfung mit Mutationsprobe (über 25, alle
    erkannt; mehrere zunächst blinde führten zu schärferen Tests);
  - Nahverkehr: eigene Taste fürs Mitfahren (G/Steuerkreuz unten, nicht S/Pfeil); einsteigen in Reichweite eines
    Wagens auch in Fahrt, aussteigen neben dem Wagen (nie in Wand, Baum, Poller oder Auto), Abspringen mit Sturz und ab
    40 km/h Schaden, unter Tage nur am Bahnsteig mit Ausgang an der Straße; verschwindet das Fahrzeug, Teleport, K. o.
    oder Speichern während der Fahrt → zu Fuß an der letzten Haltestelle bzw. deren Straßenausgang, nie im Gleisbett;
    Tunnel aus Linienweg ohne sichtbares Gleis (eine querende Hochbahn zählt nicht, nicht geladene Kachel = oben);
    Zugphysik je Modus (Höchsttempo, Bremsweg, Zwangsbremsung folgt der Bremskurve, Trinkgeld nach Genauigkeit und
    Sanftheit); Zug übernehmen nur am Führerstand, fahren, Türen nur im Stand an der Haltestelle, Trinkgeld beim
    sauberen Halt, Türen schließen selbst, Abstand zum Zug voraus, Fahrplanzug dahinter wartet, Wenden am Endhalt über
    die Taste (auch wo der Endhalt im Fahrplan doppelt steht), eigenen Zug wieder übernehmen, Auftrag neu starten holt
    aus dem Zug, Hochbahn nur im Bahnhof ein-/aussteigen,
    eigene Straßenbahn hält vor einem Auto auf dem Gleis ohne es zu schieben; kein Stellplatz auf einem
    Straßenbahngleis (auch wenn der Fahrplan erst nach den Stellplätzen geladen wird); Tunnelansicht mit Röhre, einem
    Bahnsteig je Richtung entlang der Kurve, Namen einmal und zuletzt, eigener Zug am Bahnsteig; Leiste für Fahrgast
    und Fahrer in jedem Fensterformat ohne Überlappung mit dem Auftrag; Statistik und Töne. Jede Schutzprüfung mit
    Mutationsprobe (über 30, alle erkannt); im Browser geprüft: U8 unter Kottbusser Tor (Tunnelansicht), M10 mit F am
    Führerstand übernommen und angefahren;
  - Fahrdynamik: Spielgefühl (0–100 in der Reihenfolge der Datenblätter, echtes Höchsttempo, Bremsweg 100–0 unter 36 m),
    Anfahren auf Schnee Allrad vor Heck vor Front, ASR verhindert Durchdrehen; Vollgas in der Kurve ohne ESP: Heck
    übersteuert, Front untersteuert, Allrad dazwischen; ESP fängt den Heckantrieb ab; Mittelmotor lenkt schneller ein,
    Heckmotor zieht auf Schnee besser an; hoher Schwerpunkt: weniger Kurvengrip, stärkeres Nicken, Kurve drückt die
    Karosserie nach außen; Handbremse dreht das Auto und lässt den Schwung, enger Wendekreis; Verkehr ohne
    Fahrdynamik, deterministisch; sechs Mutationsproben (Lastverschiebung, Antriebsachse, Gierträgheit, Kammscher
    Kreis, Handbremse, Querlast) werden alle erkannt; Missions-Autopilot fährt mit der Fahrdynamik (Pure Pursuit,
    vorausschauendes Bremsen);
  - Gebrauchsspuren: Schmutz nur in Senken, Ausbleichen nur auf Kuppen (dazwischen unverändert), Ölband in jeder
    Spurmitte innerhalb der Fahrbahn, nicht auf Pflaster; Zeichnen mit Schmutz, Ölband, Kontaktschatten, Vignette,
    Schnee deckt den Schmutz zu;
  - Leistung: dynamische Auflösung (langsam → herunter bis 70 %, schnell → hinauf, Hysterese, Pausen zählen nicht);
    die Beschleunigungen der Simulation (Raster, Zwischenspeicher, binäre Suche) ändern nichts am Ergebnis –
    geprüft mit einem Positions-Fingerabdruck über 40 s Stadtverkehr (alt = neu) und der unveränderten Missionszeit des
    Autopiloten; im Browser: 70 % Auflösung steigern die Bildrate im Software-Rendering von 8 auf 14 Bilder/s;
 nie über der Wheelie-, Bremse nie über der Stoppie-Grenze (Rad hebt sichtbar), bremst
    länger als eine Limousine, Roller-Spitze 95 km/h, Schräglage in der Kurve; Oldtimer bremst deutlich länger,
    Muscle-Car ohne ESP bricht unter Gas aus, Hot Hatch mit ESP nicht, Pick-up zieht auf Schnee schlechter an als der
    Allrad; Aufprall wirft den Fahrer ab (benommen, verletzt, Maschine liegt, aufsteigen richtet auf), Zweirad kein
    Missionsauto und ungedämpft, Zweiräder im Verkehr tags häufiger als nachts;
  - U-Bahnhöfe, Fehlerfälle: Eingang höchstens 20 m vom U-/S-Symbol (Bahnart passend, nicht im Haus), E in Reichweite
    führt hinunter, weiter weg nicht; Ostkreuz/Plänterwald nicht begehbar; gleiche Bahnsteige egal von wo man kommt
    (Yorckstraße S/U7); Neustart und Teleport holen heraus bzw. lassen nicht hineinfallen; Straßenbahn hält nicht für die
    Figur unten; Spielstand behält das Automodell; Gas beim Rückwärtsrollen bremst, Wiese begrenzt das Tempo, nach dem
    Aussteigen kein Fahrzustand; Ölband endet vor Kreuzungen, niedrige Qualität spart Ebenen;
  - U-Bahnhöfe: aus dem Fahrplan erzeugt (Kottbusser Tor/U8 unter der Erde, Hochbahnhof Görlitzer Bahnhof nicht),
    Bahnsteig zuglang, beide Richtungen, Eingänge am Gehweg nicht im Haus; hinein über den Eingang, unten nur auf dem
    Bahnsteig, hinaus über die andere Treppe an deren Ausgang, nicht gleich wieder hinein; Einsteigen nur an der Kante
    neben einem haltenden Zug, Aussteigen am nächsten Bahnhof auf dessen Bahnsteig; keine Ziele von oben, gedämpfter
    Klang, Spielstand am Ausgang; im Browser geprüft: Eingang an der Reichenberger Straße, Bahnhofsansicht mit
    Fahrplanzug, Anzeigen und Fahrgästen;
  - Waffenrad-Bedienung: Zeiger auf den Radius begrenzt (zurückziehen wechselt sofort), Bewegung vor dem Öffnen zählt
    nicht, Mausrad dreht eine Raste je Feld, Zifferntaste wählt und schließt, Abbrechen ohne Wahl, Controller-Stick mit
    Totzone (losgelassen bleibt die Wahl), LB tippen/halten, verpasstes Loslassen wird nachgeholt, Zeitlupe blendet
    weich und kommt genau an, HUD mit Zeiger, Munition je Waffe, Tasten und Hinweis je Gerät (11 Mutationsproben, alle
    erkannt); im Browser mit echten Mausereignissen: Rad beim Schießen, Esc ohne Pausemenü, Ziffer, Mausrad, Tippen
    ohne Wechsel, Ziel bleibt nach der Wahl stehen;
  - Menschen: Typen nach Ort, Uhrzeit, Wochentag und Tätigkeit (Büroleute werktags in Mitte, Kiez und Punks in
    Kreuzberg statt Zehlendorf, nachts keine Senioren und Kinderwagen, Gitarre spielt nie ein Kinderwagen), jeder Typ
    vollständig und deterministisch, Tempo je Typ ohne Verbrauch des Welt-Zufalls; Gang: Stand ohne Schritt, Rennen
    weiter ausgreifend, Takt gedeckelt, kein Knick beim Anlaufen/Anhalten (zweite Differenz unter der Krümmung der
    Schrittkurve), Bewegung unabhängig von der Bildrate, Drehung geglättet und begrenzt, Beine in Laufrichtung,
    rückwärts beim Zielen; jeder Typ zeichnet in allen Posen ohne ungültige Werte und sieht anders aus. 16
    Mutationsproben, alle schlagen an;
  - Befehlszeile: Zerlegen (Anführungszeichen), großzügige Uhrzeit, Trefferordnung (Namensanfang vor Wortanfang vor
    irgendwo, Umlaute egal), Vorschläge mit grauer Ergänzung (Befehle, Werte, Orte mit Leerzeichen), Tasten (Tab, ↑↓
    in Vorschlägen und Verlauf, Enter bleibt offen, leere Zeile/Esc schließt), jeder Befehl wirkt, falsche Eingaben
    ändern nichts, nur erfolgreiche Cheats zählen, Schummelgeld ist kein verdientes Geld; im Spiel steht die Welt bei
    offener Zeile still, `tp` teleportiert ohne Rückfrage auch zu noch ladenden Stadtteilen;
  - Statistik: Strecke zu Fuß/im Auto, Sprünge zählen nicht, Kugeln je Waffe (Schrot jede Kugel), Treffer und Tote nur
    vom Spieler, Unfälle/Poller/Wrack nur am eigenen Auto, Brücken je Auffahrt, verdientes Geld nur als Zuwachs,
    Einlesen fremder/kaputter Stände, Zusammenzählen, IndexedDB-Weg und Rückfall; eine späte Datenbank-Antwort mischt
    nie ein altes Spiel in ein neues; die Seite passt in den 720er-Rahmen;
  - Baumschatten: Stamm vom Fuß bis in die Krone, Krone quer so breit wie der Baum und bei tiefer Sonne entlang der
    Sonne gestreckt, Richtung wie die Hausschatten. Für 17 Prüfungen aus diesen drei Bereichen wurde gegengeprüft, dass
    sie fehlschlagen, wenn man den Fehler absichtlich wieder einbaut;
  - Gebäude aus Bauteilen: Sockel mit Hochhaus (beide), Turm auf dem Block (eigenes Gebäude), Obergeschosse über der
    Arkade heben das Haus darunter, Überbauung ohne etwas darunter entfällt, Pfeiler im Wasser wird kein Haus, Art aus
    `building:part`;
  - Waffenrad: Mausrichtung → Segment (oben 0, im Uhrzeigersinn, Grenzen genau zwischen den Mitten, Totzone),
    rechte Taste tippen = ein-/aussteigen, halten = Rad auf mit der aktuellen Waffe, Totzone behält die Wahl,
    Loslassen wählt, im Auto kein Rad und kein versehentliches Aussteigen nach langem Halten, Rad schließt beim
    Einsteigen; sechs verschiedene Symbole mit gültigen Koordinaten; HUD zeichnet je Waffe ein Segment, das gezeigte
    gelb, Name und Munition in der Mitte (6 Mutationsproben, alle erkannt); im Browser geprüft (Messer gewählt,
    Tippen steigt ein und aus, Kontextmenü unterdrückt);
  - Brücken: jede der 895 Brückenfahrbahnen Berlins in jeder Richtung über die ganze Breite abgefahren
    (`node tools/check-bridges.mjs`, blockweise, ~30 s) – keine Sperre, in der Spur überall die richtige Ebene,
    höchstens 4 Randstellen mit falscher Ebene (je ≤ 6 m, gestaffelte Brückenanfänge der Gegenfahrbahn); sechs Brücken mit je einer gefundenen Ursache (Elsenbrücke, Kaiserdamm,
    Gottlieb-Dunkel-Brücke, Lessingbrücke, Südostallee, Kiefholzstraße) einzeln ohne Befund; die Prüfung ist
    gegengeprüft (mit ebenenblinden Wänden meldet sie die Elsenbrücke), 13 Mutationsproben; Lücke zur Gegenfahrbahn nur
    zwischen Fahrbahnen derselben Ebene;
  - Physik nach Ebenen: Stadtgrenze sperrt jede Ebene, Häuser/Bäume/Poller den Boden, Wände nur ihre Ebene; an der
    Warschauer Brücke quert ein Brückengeländer die Unterführung – unten fährt ein Auto hindurch, mit Ebene 1 prallt es
    ab; Kaimauer läuft unter der Oberbaumbrücke weiter; auf der Brückenachse oben Fahrbahn, unten Wasser, neben der
    Fahrbahn oben Gehweg; Kontakte nur auf derselben Ebene oder im gemeinsamen Portal; ein KI-Auto auf der Brücke bremst
    nicht für ein Auto darunter; ein Schuss trifft nicht durchs Deck (10 Mutationsproben, alle erkannt; zwei zunächst
    blinde führten zu schärferen Untergrund-Prüfungen);
  - Fenster: je Fenster ein eigener Zeitpunkt (mehr als 60 % verschiedene Schwellen, höchstens 8 von 72 Fenstern je
    2-%-Schritt, die ersten Lichter über die ganze Fassade verteilt), Wohnungsnachbarn näher beieinander als andere
    Etagen, mittags dunkel, abends mehr als um 3 Uhr, nachts einzelne Wechsel, Fenster im Raster der Fassade, vier und
    mehr Lichtfarben, Fernseher flackern, Büros abends hell und nachts dunkel; im Bild kommen zwischen 19 und 21 Uhr
    fast jede zweite Minute Fenster dazu, nie mehr als 10 % auf einmal;
  - Unwetter: alle elf Wetterbilder kommen vor, Schnee nur an Wintertagen, Gewitter nur nachmittags, Nebel nur morgens,
    alle Werte minütlich stetig; Böen ohne Zufall, Sturm treibt Regen schräg und Laub durchs Bild; Blitze deterministisch
    mit Nachblitzen, Donner nach Entfernung/Schallgeschwindigkeit, genau einmal (auch auf der Grenze zweier Abfragen);
    Schneedecke wächst mit der Schneefallstärke, taut ohne, bei Regen schneller; dünn fleckig, tief geschlossen, Textur
    kachelt; Hauptstraßen freier; Starkregen und dichter Nebel dunkler, Schnee hellt die Nacht auf; Autos rutschen auf
    Schnee, Spielstand merkt sich Schnee und Nässe, Tauwetter macht nass; Wind hörbar, Schnee dämpft; Zeichnen bei allen
    elf Wetterbildern ohne ungültige Koordinaten, Blitzstrahl im Bild (20 Mutationsproben, alle erkannt; eine zunächst
    blinde führte zu einem Grenzfall-Test);
  - Ebenen: `levelOf` (Brücke, Unterführung, Tunnel, `bridge=no`, Klemmen) und die Bits im Kachelformat; an der
    Warschauer Brücke liegt die Brücke oben, die Tamara-Danz-Straße als Unterführung darunter, beide ohne gemeinsamen
    Knoten, verbunden nur über Portale; wer die Rampe hinauffährt, ist oben, wer unten durchläuft, bleibt unten; im
    Portal bleibt man auf der eigenen Fahrbahn, auch wenn die Rampe näher liegt; ein Auto unter der Brücke wird vor ihr
    gezeichnet und bekommt eine Silhouette, eins auf der Brücke nicht (8 Mutationsproben, alle erkannt; eine zunächst
    blinde führte zu einem eigenen Portal-Test);
  - keine unsichtbare Wand (Ufer, Gleisrand, Geländer) auf befahrbarer Fläche – Kerngebiet und sieben Brücken,
    an denen es klemmte; die Oberbaumbrücke ist über die ganze Breite (beide Richtungen, Lücke, Radwege) mit dem Auto
    befahrbar; Punkttest der Fahrfläche und Zuschnitt (vier Mutationsproben, alle erkannt);
  - Brücken: Lücke zwischen gegenläufigen Brückenfahrbahnen bis 7 m wird Fahrbahn, zur richtigen Seite, nur bei
    gleichem Namen; `cycleway=track` im Querschnitt; an Oberbaum- und Warschauer Brücke kein Geländerstück auf einer
    Fahrbahn, Lücke gefüllt, Radweg außen; Brückenwege unter der Brückenfahrbahn gezeichnet, Radwege auf der Brücke
    gezeichnet (10 Mutationsproben, alle erkannt; zwei zunächst blinde führten zu schärferen Prüfungen);
  - Wegweiser: an einer künstlichen Kreuzung je Zufahrt ein Schild rechts 35 m davor, Zeilen links → geradeaus →
    rechts, kein Wenden (auch nicht in eine andere Straße), Einbahnstraßen, OSM-Beschilderung vor der Verfolgung
    (Tag nur in Weg-Richtung, `destination:backward` dagegen, Relation nur für ihre Zufahrt), „Zentrum“ nur Richtung
    Mitte, Straßenname im selben Ortsteil, Kreisel als eine Kreuzung, Schild im Haus nicht aufgestellt; echte Karte:
    Kotti weist nach Neukölln (Süden) und Richtung Zentrum, kein Schild auf einer Fahrbahn oder in einem Haus; im
    Bild gezeichnet, Tafel einmal gemalt und von der Fahrbahn weg; kein Ziel doppelt auf einem Schild (Karte und
    Einzelfälle), verdeckte Schilder mit Silhouette (19 Mutationsproben, alle erkannt; zwei zunächst
    blinde führten zu schärferen Prüfungen);
  - Erreichbarkeit (Flutfüllung auf einem 0,5–0,8-m-Raster mit Auto- bzw. Fußgängerbreite gegen Hauswände, Zäune,
    Ufer, Bäume und Poller): das Tempelhofer Feld ist vom Columbiadamm aus mit dem Auto und zu Fuß erreichbar (Mitte,
    Nord-, Süd- und Ostrand); in Kreuzberg und Marzahn erreicht das Auto über 90 % der Fußgängerfläche; im Build
    bekommt ein Zaun, den ein Fußweg ohne Tor-Knoten kreuzt, eine autobreite Lücke (auch im Bild), eine Hecke ohne
    Querung bleibt ganz; Poller fallen mit Schwung um (Ereignis, Auto bremst leicht und fährt weiter), langsam nicht,
    bleiben nach dem Nachladen liegen, gelten je Welt, halten zu Fuß auf und liegend nicht mehr; Verdeckung durch
    Krone, Haus (Durchfahrt, hinter dem Haus, nicht davor und nicht an der Vorderkante) und Viadukt, die Silhouette
    wird nur gezeichnet, wenn etwas verdeckt, auch im Auto in einer Durchfahrt; teilweise Verdeckung zählt (nur die
    Motorhaube unter der Krone, Heck unter dem Dach), wer vor dem Baum steht, liegt obenauf; im Stadtverkehr bekommen
    verdeckte Autos und Passanten je einen Umriss, freie nicht (18 Mutationsproben, alle erkannt; drei zunächst blinde
    führten zu schärferen Prüfungen);
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
  - Dächer: über 60 000 Dachaufbauten im Kerngebiet liegen ganz im Grundriss (auch Schornsteine auf dem First),
    überlappen nicht und folgen der Hauptachse; OSM-Dachformen (über 3000 Häuser im Kerngebiet) haben Vorrang,
    geschätzte Formen passen zu Art und Höhe, alle Formen kommen vor; über 97 % der Dachflächen liegen im Grundriss,
    keine verdrehte Fläche, Fallrichtung zur Traufe, Gauben im Grundriss, Ziegelreihen begrenzt; Rechteck mit
    Sattel- bzw. Walmdach exakt nachgerechnet; OSM-Farben und -Materialien gehen vor, Steildächer immer in Ziegel oder
    Schiefer, Flachdächer grau, fünfgeschossige Häuser in Marzahn-Hellersdorf sind Platte, dieselben in Mitte nicht;
    Nord-Neukölln: Mietshäuser ohne OSM-Form zu 75–95 % mit Steildach, Kreuzberg nicht; Vorderhaus mit Seitenflügel
    bekommt je Flügel den First in der Mitte (6 m bzw. 3,5 m hinter der Traufe);
    Aussehen aus den OSM-Tags kommt über den Karten-Build im Spiel an, ohne leere Felder in den Kacheln; beim Zeichnen
    ist die Dachfläche zur Sonne heller, wechselt mit dem Sonnenstand, Ziegelreihen nur in hoher Qualität (zwölf
    Mutationsproben, alle erkannt; eine anfangs blinde Probe führte zu einem schärferen Bezirkstest);
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
  - ÖPNV: ZIP- und CSV-Leser; Fahrplan-Build an einem künstlichen GTFS (Linie auf Berlin gekürzt, Halte auf dem Weg,
    Fahrzeiten, Abfahrten nach Mitternacht, Regionalbahn draußen); der „normalste“ Stichtag gewinnt gegen Bauarbeiten und
    Feiertage; Takt je Uhrzeit und Wochentag, Lage mit Halten nie rückwärts, Fahrzeuge gleichmäßig im Takt, neue fahren
    ab, fertige verschwinden; echte Daten: U1, U8, S7, M29, M10 werktags morgens mindestens sechsmal je Stunde, alle Halte
    auf dem Weg im Kartengebiet; Busse fahren an der Sonnenallee ihre Linie und bedienen Halte, kein Pkw benutzt eine
    Busspur; Straßenbahn hält vor einem Hindernis, klingelt und schiebt Autos aus ihren Wagen; S-/U-Bahn nur auf
    oberirdischem Gleis sichtbar; ein Bus steht am Halt, fährt ein gutes Stück seine Linie entlang und wird dicht an ihr
    aufgesetzt (19 von 20 Mutationsproben erkannt; die 6-m-Grenze beim Aufsetzen wirkt bei den echten Daten nie, weil die
    richtungsgebundene Spursuche schon nur nahe Spuren findet – sie bleibt als Sicherheitsrand);
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
    Dialogs, Ziel zu Fuß auf dem Gehweg bzw. im Auto auf der Fahrbahn, während eines Auftrags gesperrt; auf offenem
    Grund genau dorthin (Tempelhofer Feld zu Fuß und im Auto, nie in ein Haus, freier Punkt bleibt, wo er ist;
    vier Mutationsproben, alle erkannt);
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
