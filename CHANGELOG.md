# Changelog

Alle nennenswerten Änderungen an GTA Berlin. Format nach [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionen nach [Semantic Versioning](https://semver.org/lang/de/). Solange die Version mit `0.` beginnt, ist das Spiel
ein Prototyp: Spielstände, Kartenformat und Steuerung können sich zwischen Versionen ändern.

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
