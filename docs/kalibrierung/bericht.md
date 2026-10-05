# Kalibrierbericht Fahrphysik

Erzeugt von `cargo run --release -p physics-calibrate`. Bedingungen: trockener Asphalt, `realismus = 1`, `grip_global = 1`; LKW und Busse voll beladen, sonst leer. Toleranzen: 0-X ±7 %, Vmax ±3 %, Bremsweg ±5 %, Querbeschleunigung ±0,05 g. Stellschrauben nur innerhalb der erlaubten Grenzen (cwA ±15 %, μ ±10 %, Übersetzung, Schaltzeit, Wirkungsgrad ±3 %, Bremskraft); Masse, Leistung und Drehmoment unverändert. Verläufe im 100-ms-Takt: `docs/kalibrierung/csv/<id>.csv`.

**184 von 207 Zielwerten in der Toleranz** (88 Fahrzeuge, Laufzeit 12 s).

| Fahrzeug | Test | Ziel | Ist | Abweichung | | Stellschrauben |
|---|---|---|---|---|---|---|
| **Kiezrad Trekking** (fahrrad_city) | 0–25 km/h | 8.0 s | 3.9 s | -50.7 % | ✗ | cwA -15 %, μ -10 %, Bremskraft -46 %, Wirkungsgrad -3 % |
|  | Bremsweg 25 km/h | 4.8 m | 4.8 m | -0.0 % | ✓ |  |
|  | Vmax | 32 km/h | 27 km/h | -14.1 % | ✗ |  |
| | *Grund:* 0_25: Ziel beschreibt einen Alltagsantritt, der Test fährt Sprint (Kraftgrenze der Kurve muskel); vmax: Leistung reicht auch mit cwA −15 % nicht (Fahrwiderstände bei 32 km/h größer als 800 W × Wirkungsgrad) | | | | | |
| **Avus Carbon** (rennrad) | 0–25 km/h | 5.5 s | 2.6 s | -52.4 % | ✗ | cwA -15 %, μ -10 %, Bremskraft -42 %, Wirkungsgrad -3 % |
|  | Bremsweg 25 km/h | 4.5 m | 4.5 m | +0.0 % | ✓ |  |
|  | Vmax | 55 km/h | 39 km/h | -28.6 % | ✗ |  |
| | *Grund:* 0_25: Ziel beschreibt einen Alltagsantritt, der Test fährt Sprint (Kraftgrenze der Kurve muskel); vmax: Leistung reicht auch mit cwA −15 % nicht (Fahrwiderstände bei 55 km/h größer als 1200 W × Wirkungsgrad) | | | | | |
| **Flitz Max** (escooter) | Vmax | 20 km/h | 19 km/h | -3.0 % | ✓ | – |
| **Flitz Max (offen)** (escooter_entdrosselt) | 0–25 km/h | 5.0 s | 4.7 s | -6.2 % | ✓ | cwA -15 %, μ -10 %, Bremskraft -49 %, Wirkungsgrad -3 % |
|  | Bremsweg 25 km/h | 5.5 m | 5.5 m | -0.0 % | ✓ |  |
|  | Vmax | 35 km/h | 34 km/h | -2.2 % | ✓ |  |
| **Flitz Duo** (escooter_performance) | 0–50 km/h | 3.8 s | 3.7 s | -1.3 % | ✓ | cwA -15 %, μ +10 %, Bremskraft +60 % |
|  | Bremsweg 50 km/h | 13.0 m | 13.8 m | +5.8 % | ✗ |  |
|  | Vmax | 80 km/h | 79 km/h | -1.5 % | ✓ |  |
| | *Grund:* brems_50: Haftung begrenzt: auch mit μ +10 % und ABS kein kürzerer Bremsweg | | | | | |
| **Kiezflitzer 50** (roller_45) | 0–45 km/h | 8.0 s | 7.3 s | -9.2 % | ✗ | μ -10 %, Bremskraft -20 %, Wirkungsgrad -3 % |
|  | Bremsweg 50 km/h | 12.0 m | 12.0 m | +0.0 % | ✓ |  |
|  | Vmax | 45 km/h | 45 km/h | +0.4 % | ✓ |  |
| | *Grund:* 0_45: zu schnell: auch mit längster Übersetzung | | | | | |
| **Tegel Nackt 700** (motorrad_naked) | 0–100 km/h | 3.9 s | 3.9 s | -0.0 % | ✓ | cwA -15 %, μ -6 %, Bremskraft -10 %, Übersetzung ×1.30, Schaltzeit ×1.25 |
|  | Bremsweg 100 km/h | 40.0 m | 40.0 m | -0.0 % | ✓ |  |
|  | Vmax | 214 km/h | 210 km/h | -2.1 % | ✓ |  |
| **Tegel RR 1000** (superbike) | 0–100 km/h | 3.1 s | 2.6 s | -14.8 % | ✗ | μ +10 %, Bremskraft +60 % |
|  | Bremsweg 100 km/h | 36.0 m | 37.4 m | +3.8 % | ✓ |  |
|  | Vmax | 299 km/h | 299 km/h | -0.1 % | ✓ |  |
| | *Grund:* 0_100: zu schnell: die Kippgrenze g·l_h/h = 1.07 g erlaubt mehr; eine längere Übersetzung nähme den Wheelie beim Ampelstart (Akzeptanzszene 13 hat Vorrang) | | | | | |
| **Havelland Fatline** (cruiser) | 0–100 km/h | 4.4 s | 4.7 s | +6.3 % | ✓ | cwA +15 %, μ -8 %, Bremskraft -10 %, Übersetzung ×1.60, Wirkungsgrad +3 % |
|  | Bremsweg 100 km/h | 44.0 m | 44.0 m | -0.0 % | ✓ |  |
|  | Vmax | 180 km/h | 189 km/h | +5.3 % | ✗ |  |
| | *Grund:* vmax: Leistung zu groß für das Ziel, auch mit cwA +15 % | | | | | |
| **Havel Mini 65** (kleinwagen_65ps) | 0–100 km/h | 14.4 s | 15.3 s | +6.2 % | ✓ | cwA +4 %, μ -5 %, Bremskraft +60 %, Übersetzung ×1.60, Schaltzeit ×0.75, Wirkungsgrad +3 % |
|  | Bremsweg 100 km/h | 38.0 m | 38.4 m | +1.0 % | ✓ |  |
|  | Querbeschl. | 0.85 g | 0.85 g | +0.00 g | ✓ |  |
|  | Vmax | 164 km/h | 164 km/h | +0.0 % | ✓ |  |
| **Spree Ronda 1.5** (kompakt_benzin) | 0–100 km/h | 8.6 s | 8.2 s | -5.1 % | ✓ | cwA -2 %, μ -2 %, Übersetzung ×1.60, Schaltzeit ×1.50 |
|  | Bremsweg 100 km/h | 35.0 m | 35.0 m | +0.1 % | ✓ |  |
|  | Querbeschl. | 0.95 g | 0.96 g | +0.01 g | ✓ |  |
|  | Vmax | 224 km/h | 224 km/h | +0.0 % | ✓ |  |
| **Spree Ronda Basis** (kompakt_fuenf_felder) | 0–100 km/h | 10.5 s | 10.5 s | -0.2 % | ✓ | cwA -0 %, Übersetzung ×1.30 |
|  | Vmax | 200 km/h | 200 km/h | -0.0 % | ✓ |  |
| **Spree Ronda Sport+** (hot_hatch) | 0–100 km/h | 6.2 s | 5.8 s | -6.9 % | ✓ | μ -8 %, Übersetzung ×1.60, Schaltzeit ×1.50 |
|  | Bremsweg 100 km/h | 34.0 m | 34.0 m | -0.1 % | ✓ |  |
|  | Querbeschl. | 1.00 g | 0.99 g | -0.01 g | ✓ |  |
|  | Vmax | 250 km/h | 250 km/h | -0.2 % | ✓ |  |
| **Spree Pendler Diesel** (kombi_diesel) | 0–100 km/h | 9.3 s | 8.8 s | -5.3 % | ✓ | cwA -9 %, μ -7 %, Bremskraft +60 %, Übersetzung ×1.60, Schaltzeit ×1.50 |
|  | Bremsweg 100 km/h | 36.0 m | 36.4 m | +1.2 % | ✓ |  |
|  | Querbeschl. | 0.90 g | 0.91 g | +0.01 g | ✓ |  |
|  | Vmax | 223 km/h | 223 km/h | -0.0 % | ✓ |  |
| **Spree Pendler Streife** (polizei_kombi) | 0–100 km/h | 9.3 s | 8.8 s | -5.3 % | ✓ | cwA -9 %, μ -7 %, Bremskraft +60 %, Übersetzung ×1.60, Schaltzeit ×1.50 |
|  | Bremsweg 100 km/h | 36.0 m | 36.4 m | +1.2 % | ✓ |  |
|  | Querbeschl. | 0.90 g | 0.91 g | +0.01 g | ✓ |  |
|  | Vmax | 223 km/h | 223 km/h | -0.0 % | ✓ |  |
| **Teltower T5 Droschke** (taxi_diesel) | 0–100 km/h | 7.6 s | 7.9 s | +3.6 % | ✓ | cwA +15 %, μ -5 %, Bremskraft +60 %, Übersetzung ×1.60, Schaltzeit ×1.25, Wirkungsgrad +3 % |
|  | Bremsweg 100 km/h | 35.0 m | 35.8 m | +2.2 % | ✓ |  |
|  | Querbeschl. | 0.90 g | 0.93 g | +0.03 g | ✓ |  |
|  | Vmax | 238 km/h | 242 km/h | +1.7 % | ✓ |  |
| **Teltower T5 Allrad** (business_limo) | 0–100 km/h | 4.8 s | 5.0 s | +4.0 % | ✓ | μ -10 %, Bremskraft +60 %, Übersetzung ×1.60, Wirkungsgrad +3 % |
|  | Bremsweg 100 km/h | 34.0 m | 34.5 m | +1.4 % | ✓ |  |
|  | Querbeschl. | 0.95 g | 0.97 g | +0.02 g | ✓ |  |
|  | Vmax | 250 km/h | 250 km/h | -0.0 % | ✓ |  |
| **Spree Tiga 150** (suv_kompakt) | 0–100 km/h | 9.1 s | 9.3 s | +1.6 % | ✓ | cwA -1 %, μ -10 %, Bremskraft +60 %, Übersetzung ×1.60, Schaltzeit ×1.50, Wirkungsgrad +3 % |
|  | Bremsweg 100 km/h | 37.0 m | 37.6 m | +1.7 % | ✓ |  |
|  | Querbeschl. | 0.85 g | 0.88 g | +0.03 g | ✓ |  |
|  | Vmax | 207 km/h | 207 km/h | +0.0 % | ✓ |  |
| **Teltower TX5 Diesel** (suv_gross) | 0–100 km/h | 6.1 s | 6.5 s | +5.7 % | ✓ | cwA +15 %, μ -6 %, Bremskraft +60 %, Übersetzung ×1.60, Wirkungsgrad +3 % |
|  | Bremsweg 100 km/h | 36.0 m | 36.3 m | +0.7 % | ✓ |  |
|  | Querbeschl. | 0.90 g | 0.91 g | +0.01 g | ✓ |  |
|  | Vmax | 230 km/h | 232 km/h | +0.8 % | ✓ |  |
| **Grunewald Kommandant 500** (gelaendewagen) | 0–100 km/h | 5.4 s | 5.7 s | +5.7 % | ✓ | μ -10 %, Bremskraft +60 %, Übersetzung ×1.60, Wirkungsgrad +3 % |
|  | Bremsweg 100 km/h | 40.0 m | 44.1 m | +10.2 % | ✗ |  |
|  | Querbeschl. | 0.80 g | 0.69 g | -0.11 g | ✗ |  |
|  | Vmax | 210 km/h | 210 km/h | -0.0 % | ✓ |  |
| | *Grund:* brems_100: Haftung begrenzt: auch mit μ +10 % und ABS kein kürzerer Bremsweg; quer_g: Reifenhaftung reicht auch mit μ +10 % nicht (Lastverlagerung, Schwerpunkt) | | | | | |
| **Spree Kasten 6** (transporter_kasten) | 0–100 km/h | 11.4 s | 11.3 s | -1.0 % | ✓ | cwA -9 %, Bremskraft +14 %, Übersetzung ×1.60, Schaltzeit ×1.50, Wirkungsgrad +3 % |
|  | Bremsweg 100 km/h | 40.0 m | 40.0 m | +0.0 % | ✓ |  |
|  | Querbeschl. | 0.75 g | 0.74 g | -0.01 g | ✓ |  |
|  | Vmax | 182 km/h | 182 km/h | -0.0 % | ✓ |  |
| **Spree Großraum 317** (kastenwagen_35t) | 0–100 km/h | 15.0 s | 14.0 s | -6.7 % | ✓ | cwA -11 %, μ -4 %, Bremskraft -27 %, Übersetzung ×0.40, Schaltzeit ×1.50, Wirkungsgrad -3 % |
|  | Bremsweg 100 km/h | 42.0 m | 42.0 m | -0.0 % | ✓ |  |
|  | Querbeschl. | 0.70 g | 0.72 g | +0.02 g | ✓ |  |
|  | Vmax | 160 km/h | 160 km/h | -0.0 % | ✓ |  |
| **Voltwerk Kiez 3** (e_kompakt) | 0–100 km/h | 7.3 s | 7.7 s | +5.4 % | ✓ | μ +4 %, Bremskraft +60 %, Wirkungsgrad +3 % |
|  | Bremsweg 100 km/h | 35.0 m | 35.7 m | +1.9 % | ✓ |  |
|  | Querbeschl. | 0.90 g | 0.93 g | +0.03 g | ✓ |  |
|  | Vmax | 160 km/h | 159 km/h | -0.3 % | ✓ |  |
| **Voltwerk Blitz Tri** (e_performance) | 0–100 km/h | 2.1 s | 2.6 s | +25.8 % | ✗ | cwA -15 %, μ -8 %, Bremskraft +9 %, Wirkungsgrad +3 % |
|  | Bremsweg 100 km/h | 34.0 m | 34.0 m | +0.0 % | ✓ |  |
|  | Querbeschl. | 1.00 g | 1.00 g | -0.00 g | ✓ |  |
|  | Vmax | 282 km/h | 282 km/h | -0.1 % | ✓ |  |
| | *Grund:* 0_100: physikalisch nicht erreichbar: selbst konstante Spitzenleistung ohne Schaltpausen braucht 2.3 s | | | | | |
| **Wannsee Boxer 6 S** (sportwagen_s) | 0–100 km/h | 3.5 s | 3.5 s | +0.5 % | ✓ | cwA +15 %, μ +4 %, Bremskraft -12 %, Übersetzung ×1.60, Schaltzeit ×1.50 |
|  | Bremsweg 100 km/h | 31.5 m | 31.5 m | -0.0 % | ✓ |  |
|  | Querbeschl. | 1.10 g | 1.10 g | +0.00 g | ✓ |  |
|  | Vmax | 308 km/h | 312 km/h | +1.3 % | ✓ |  |
| **Wannsee Boxer 6 Turbo** (turbo_s) | 0–100 km/h | 2.5 s | 2.6 s | +3.0 % | ✓ | cwA +15 %, μ +10 %, Bremskraft -8 %, Übersetzung ×1.60 |
|  | 0–200 km/h | 8.4 s | 8.4 s | +0.0 % | ✓ |  |
|  | Bremsweg 100 km/h | 30.0 m | 30.0 m | +0.0 % | ✓ |  |
|  | Querbeschl. | 1.20 g | 1.18 g | -0.02 g | ✓ |  |
|  | Vmax | 322 km/h | 351 km/h | +9.1 % | ✗ |  |
| | *Grund:* vmax: Leistung zu groß für das Ziel, auch mit cwA +15 % | | | | | |
| **Oberbaum Tempesta Allrad** (supercar_awd) | 0–100 km/h | 2.9 s | 2.9 s | -0.3 % | ✓ | cwA +15 %, μ +10 %, Bremskraft -11 %, Übersetzung ×1.60 |
|  | Bremsweg 100 km/h | 31.0 m | 31.0 m | -0.0 % | ✓ |  |
|  | Querbeschl. | 1.25 g | 1.19 g | -0.06 g | ✗ |  |
|  | Vmax | 325 km/h | 333 km/h | +2.4 % | ✓ |  |
| | *Grund:* quer_g: Reifenhaftung reicht auch mit μ +10 % nicht (Lastverlagerung, Schwerpunkt) | | | | | |
| **Westend Pfeil 720** (supercar_rwd) | 0–100 km/h | 2.9 s | 2.7 s | -6.3 % | ✓ | cwA +15 %, μ +6 %, Bremskraft -9 %, Übersetzung ×1.60, Schaltzeit ×1.50 |
|  | 0–200 km/h | 7.8 s | 7.5 s | -4.0 % | ✓ |  |
|  | Bremsweg 100 km/h | 30.0 m | 30.0 m | -0.0 % | ✓ |  |
|  | Querbeschl. | 1.25 g | 1.24 g | -0.01 g | ✓ |  |
|  | Vmax | 341 km/h | 365 km/h | +7.1 % | ✗ |  |
| | *Grund:* vmax: Leistung zu groß für das Ziel, auch mit cwA +15 % | | | | | |
| **Mühlendamm Sechzehn** (hypercar) | 0–100 km/h | 2.4 s | 2.3 s | -2.1 % | ✓ | μ +10 %, Bremskraft -12 %, Schaltzeit ×1.50, Wirkungsgrad -3 % |
|  | 0–200 km/h | 6.1 s | 5.8 s | -4.2 % | ✓ |  |
|  | Bremsweg 100 km/h | 31.0 m | 31.0 m | -0.0 % | ✓ |  |
|  | Querbeschl. | 1.30 g | 1.30 g | -0.00 g | ✓ |  |
|  | Vmax | 420 km/h | 420 km/h | -0.1 % | ✓ |  |
| **Voltwerk Blitzschlag** (hypercar_elektro) | 0–100 km/h | 2.0 s | 2.2 s | +8.7 % | ✗ | cwA -15 %, μ +10 %, Bremskraft -13 %, Wirkungsgrad +3 % |
|  | Bremsweg 100 km/h | 30.0 m | 30.0 m | -0.0 % | ✓ |  |
|  | Querbeschl. | 1.30 g | 1.30 g | -0.00 g | ✓ |  |
|  | Vmax | 412 km/h | 411 km/h | -0.2 % | ✓ |  |
| | *Grund:* 0_100: physikalisch nicht erreichbar: selbst konstante Spitzenleistung ohne Schaltpausen braucht 2.3 s | | | | | |
| **Tempelhof Stier 69** (muscle_klassik) | 0–100 km/h | 6.8 s | 6.9 s | +1.6 % | ✓ | cwA +15 %, μ -3 %, Bremskraft -41 % |
|  | Bremsweg 100 km/h | 50.0 m | 50.0 m | +0.0 % | ✓ |  |
|  | Querbeschl. | 0.75 g | 0.75 g | +0.00 g | ✓ |  |
|  | Vmax | 220 km/h | 224 km/h | +2.0 % | ✓ |  |
| **Tempelhof Fohlen V8** (muscle_modern) | 0–100 km/h | 4.6 s | 4.6 s | +0.0 % | ✓ | μ -7 %, Bremskraft -4 %, Übersetzung ×1.20, Schaltzeit ×1.25 |
|  | Bremsweg 100 km/h | 34.0 m | 34.0 m | +0.0 % | ✓ |  |
|  | Querbeschl. | 1.00 g | 1.00 g | -0.00 g | ✓ |  |
|  | Vmax | 250 km/h | 250 km/h | -0.0 % | ✓ |  |
| **Lausitz Seiten 15** (drift_coupe) | 0–100 km/h | 4.6 s | 4.5 s | -2.2 % | ✓ | cwA +15 %, μ +1 %, Bremskraft +60 %, Übersetzung ×1.60, Schaltzeit ×1.50 |
|  | Bremsweg 100 km/h | 34.0 m | 35.3 m | +3.7 % | ✓ |  |
|  | Querbeschl. | 1.00 g | 1.03 g | +0.03 g | ✓ |  |
|  | Vmax | 250 km/h | 284 km/h | +13.5 % | ✗ |  |
| | *Grund:* vmax: Leistung zu groß für das Ziel, auch mit cwA +15 % | | | | | |
| **Adlershof Kugel** (oldtimer_kaefer) | 0–100 km/h | 32.0 s | 31.9 s | -0.2 % | ✓ | cwA +8 %, μ -6 %, Bremskraft -27 %, Übersetzung ×1.60, Schaltzeit ×1.25 |
|  | Bremsweg 100 km/h | 58.0 m | 58.0 m | +0.0 % | ✓ |  |
|  | Querbeschl. | 0.70 g | 0.70 g | -0.00 g | ✓ |  |
|  | Vmax | 115 km/h | 115 km/h | -0.0 % | ✓ |  |
| **Lausitz Kolibri 601** (trabant) | 0–100 km/h | 21.0 s | 35.1 s | +67.0 % | ✗ | cwA +15 %, μ -6 %, Bremskraft -16 %, Übersetzung ×0.75, Schaltzeit ×0.50, Wirkungsgrad +3 % |
|  | Bremsweg 100 km/h | 55.0 m | 55.0 m | +0.0 % | ✓ |  |
|  | Querbeschl. | 0.70 g | 0.70 g | -0.00 g | ✓ |  |
|  | Vmax | 107 km/h | 108 km/h | +1.1 % | ✓ |  |
| | *Grund:* 0_100: physikalisch nicht erreichbar: selbst konstante Spitzenleistung ohne Schaltpausen braucht 23.8 s | | | | | |
| **Teltower Diesel 240** (oldtimer_w123) | 0–100 km/h | 20.0 s | 21.0 s | +5.1 % | ✓ | cwA +14 %, μ +2 %, Bremskraft +60 %, Übersetzung ×1.45, Wirkungsgrad +3 % |
|  | Bremsweg 100 km/h | 45.0 m | 46.1 m | +2.4 % | ✓ |  |
|  | Querbeschl. | 0.75 g | 0.78 g | +0.03 g | ✓ |  |
|  | Vmax | 143 km/h | 143 km/h | -0.0 % | ✓ |  |
| **Oberlausitz L75 Verteiler** (lkw_75t) | 0–80 km/h | 22.0 s | 22.1 s | +0.5 % | ✓ | μ +10 %, Bremskraft -43 %, Übersetzung ×1.60, Schaltzeit ×0.50, Wirkungsgrad +3 % |
|  | Bremsweg 80 km/h | 48.0 m | 48.0 m | -0.0 % | ✓ |  |
|  | Querbeschl. | 0.55 g | 0.56 g | +0.01 g | ✓ |  |
|  | Vmax | 89 km/h | 89 km/h | +0.1 % | ✓ |  |
| **Oberlausitz Fernlast 48** (sattelzug_40t) | 0–80 km/h | 40.0 s | 49.1 s | +22.7 % | ✗ | μ -10 %, Bremskraft -50 %, Übersetzung ×1.50, Schaltzeit ×0.50, Wirkungsgrad +3 % |
|  | Bremsweg 80 km/h | 56.0 m | 55.0 m | -1.8 % | ✓ |  |
|  | Querbeschl. | 0.35 g | 0.35 g | +0.00 g | ✓ |  |
|  | Vmax | 89 km/h | 89 km/h | +0.1 % | ✓ |  |
| | *Grund:* 0_80: zu langsam: Ideal mit konstanter Spitzenleistung 39.4 s, Drehmomentverlauf/Schaltpausen kosten den Rest | | | | | |
| **Kiezwerk Presse 26** (muellwagen) | 0–50 km/h | 20.0 s | 19.1 s | -4.4 % | ✓ | μ -10 %, Bremskraft -50 %, Übersetzung ×1.45, Schaltzeit ×1.50, Wirkungsgrad -3 % |
|  | Bremsweg 50 km/h | 24.0 m | 22.1 m | -8.1 % | ✗ |  |
|  | Querbeschl. | 0.40 g | 0.45 g | +0.05 g | ✓ |  |
|  | Vmax | 85 km/h | 85 km/h | +0.1 % | ✓ |  |
| | *Grund:* brems_50: Bremsweg kürzer als das Ziel, auch mit Bremskraft −50 % | | | | | |
| **Kiezwerk Stadtbus 12** (stadtbus) | 0–50 km/h | 15.0 s | 14.7 s | -2.3 % | ✓ | μ -7 %, Bremskraft -28 %, Übersetzung ×1.60, Schaltzeit ×1.50 |
|  | Bremsweg 50 km/h | 27.0 m | 27.0 m | +0.0 % | ✓ |  |
|  | Querbeschl. | 0.45 g | 0.60 g | +0.15 g | ✗ |  |
|  | Vmax | 80 km/h | 80 km/h | +0.2 % | ✓ |  |
| | *Grund:* quer_g: Nutzfahrzeug: rutscht vor dem Kippen; im Spiel begrenzt die Wankstabilisierung (RSC, mit ESP) auf 75 % der Kippgrenze, gemessen wird ohne ESP | | | | | |
| **Kiezwerk Doppelstock** (doppeldecker) | 0–50 km/h | 18.0 s | 17.2 s | -4.6 % | ✓ | μ +10 %, Bremskraft -31 %, Übersetzung ×1.60, Schaltzeit ×1.50, Wirkungsgrad -3 % |
|  | Bremsweg 50 km/h | 28.0 m | 28.0 m | +0.0 % | ✓ |  |
|  | Querbeschl. | 0.40 g | 0.44 g | +0.04 g | ✓ |  |
|  | Vmax | 80 km/h | 80 km/h | +0.2 % | ✓ |  |
| **Kiezwerk Gelenk 18** (gelenkbus) | 0–50 km/h | 17.0 s | 18.0 s | +6.0 % | ✓ | μ -10 %, Bremskraft -31 %, Übersetzung ×1.60, Wirkungsgrad +3 % |
|  | Bremsweg 50 km/h | 28.0 m | 28.0 m | -0.0 % | ✓ |  |
|  | Querbeschl. | 0.45 g | 0.55 g | +0.10 g | ✗ |  |
|  | Vmax | 80 km/h | 80 km/h | +0.2 % | ✓ |  |
| | *Grund:* quer_g: Nutzfahrzeug: rutscht vor dem Kippen; im Spiel begrenzt die Wankstabilisierung (RSC, mit ESP) auf 75 % der Kippgrenze, gemessen wird ohne ESP | | | | | |
| **Kiezwerk E-Stadtbus 12** (e_bus) | 0–50 km/h | 12.0 s | 12.1 s | +0.5 % | ✓ | μ -6 %, Bremskraft -32 %, Wirkungsgrad +3 % |
|  | Bremsweg 50 km/h | 26.0 m | 26.0 m | -0.0 % | ✓ |  |
|  | Querbeschl. | 0.45 g | 0.58 g | +0.13 g | ✗ |  |
|  | Vmax | 80 km/h | 79 km/h | -0.6 % | ✓ |  |
| | *Grund:* quer_g: Nutzfahrzeug: rutscht vor dem Kippen; im Spiel begrenzt die Wankstabilisierung (RSC, mit ESP) auf 75 % der Kippgrenze, gemessen wird ohne ESP | | | | | |
| **Lausitz Kolibri** (zweitakter) | Vmax | 107 km/h | 107 km/h | -0.5 % | ✓ | – |
| **Havel Piccolo** (kleinwagen) | Vmax | 170 km/h | 170 km/h | -0.2 % | ✓ | – |
| **Spree Ronda** (kompakt) | Vmax | 210 km/h | 210 km/h | -0.2 % | ✓ | – |
| **Teltower T6** (limousine) | Vmax | 240 km/h | 240 km/h | -0.1 % | ✓ | – |
| **Teltower T6 Droschke** (taxi) | Vmax | 210 km/h | 210 km/h | -0.1 % | ✓ | – |
| **Märker Allwetter** (kombi) | Vmax | 230 km/h | 230 km/h | -0.1 % | ✓ | – |
| **Spree Kasten** (transporter) | Vmax | 160 km/h | 160 km/h | -0.3 % | ✓ | – |
| **Voltwerk E-Terra** (elektro) | Vmax | 220 km/h | 219 km/h | -0.3 % | ✓ | – |
| **Grunewald Keiler** (gelaende) | Vmax | 190 km/h | 190 km/h | -0.2 % | ✓ | – |
| **Oberbaum Furia** (sportwagen) | Vmax | 300 km/h | 300 km/h | -0.1 % | ✓ | – |
| **Wannsee Boxer 6** (heckcoupe) | Vmax | 290 km/h | 290 km/h | -0.1 % | ✓ | – |
| **Spree Ronda Sport** (hothatch) | Vmax | 250 km/h | 250 km/h | -0.1 % | ✓ | – |
| **Köpenick Spyder** (roadster) | Vmax | 205 km/h | 205 km/h | -0.2 % | ✓ | – |
| **Tempelhof Stier V8** (musclecar) | Vmax | 250 km/h | 250 km/h | -0.0 % | ✓ | – |
| **Adlershof Kanzler** (oldtimer) | Vmax | 150 km/h | 150 km/h | -0.3 % | ✓ | – |
| **Grunewald Kipper** (pickup) | Vmax | 180 km/h | 180 km/h | -0.2 % | ✓ | – |
| **Havel Kiezbus** (kleinbus) | Vmax | 115 km/h | 115 km/h | -0.2 % | ✓ | – |
| **Märker Schotter** (rallye) | Vmax | 230 km/h | 230 km/h | -0.1 % | ✓ | – |
| **Oberbaum Tempesta** (supersport) | Vmax | 325 km/h | 325 km/h | -0.1 % | ✓ | – |
| **Tempelhof GT 40** (gtcoupe) | Vmax | 318 km/h | 318 km/h | -0.1 % | ✓ | – |
| **Adlershof Flèche** (leichtbau) | Vmax | 250 km/h | 250 km/h | -0.1 % | ✓ | – |
| **Voltwerk Blitz GT** (elektrosport) | Vmax | 260 km/h | 259 km/h | -0.2 % | ✓ | – |
| **Spree Großraum 316** (sprinter) | Vmax | 160 km/h | 160 km/h | -0.3 % | ✓ | – |
| **Havel Kasten Hoch** (hochdach) | Vmax | 180 km/h | 162 km/h | -10.0 % | ✗ | cwA -15 % |
| | *Grund:* vmax: Leistung reicht auch mit cwA −15 % nicht (Fahrwiderstände bei 180 km/h größer als 90 kW × Wirkungsgrad) | | | | | |
| **Märker RS Avant** (powerkombi) | Vmax | 280 km/h | 280 km/h | -0.0 % | ✓ | – |
| **Teltower T4 Tourer** (familienkombi) | Vmax | 225 km/h | 225 km/h | -0.2 % | ✓ | – |
| **Teltower T5** (business) | Vmax | 250 km/h | 250 km/h | -0.0 % | ✓ | – |
| **Teltower T3 RS** (sportlimo) | Vmax | 290 km/h | 290 km/h | -0.0 % | ✓ | – |
| **Grunewald Senator** (luxus) | Vmax | 250 km/h | 250 km/h | +0.0 % | ✓ | – |
| **Wannsee Coupé 240** (coupe) | Vmax | 285 km/h | 285 km/h | -0.1 % | ✓ | – |
| **Lausitz Hachi** (leichtcoupe) | Vmax | 226 km/h | 226 km/h | -0.1 % | ✓ | – |
| **Grunewald Kommandant** (gklasse) | Vmax | 210 km/h | 210 km/h | -0.1 % | ✓ | – |
| **Grunewald Förster 110** (defender) | Vmax | 191 km/h | 191 km/h | -0.1 % | ✓ | – |
| **Lausitz Taiga** (niva) | Vmax | 142 km/h | 141 km/h | -0.4 % | ✓ | – |
| **Spree Tiga** (kompaktsuv) | Vmax | 200 km/h | 198 km/h | -1.0 % | ✓ | – |
| **Oberbaum Cayo** (sportsuv) | Vmax | 245 km/h | 245 km/h | -0.1 % | ✓ | – |
| **Teltower TX7** (grosssuv) | Vmax | 243 km/h | 243 km/h | -0.1 % | ✓ | – |
| **Tegel Blitz 1000** (motorcycle) | Vmax | 240 km/h | 240 km/h | -0.2 % | ✓ | – |
| **Kiezflitzer 125** (scooter) | Vmax | 95 km/h | 95 km/h | -0.1 % | ✓ | – |
| **Teltower T6 Streife** (police) | Vmax | 240 km/h | 240 km/h | -0.1 % | ✓ | – |
| **Spree Kasten RTW** (ambulance) | Vmax | 140 km/h | 140 km/h | -0.1 % | ✓ | – |
| **Spree Kasten Paket** (delivery) | Vmax | 145 km/h | 145 km/h | -0.2 % | ✓ | – |
| **Oberlausitz L75** (truck) | Vmax | 100 km/h | 100 km/h | -0.2 % | ✓ | – |
| **Kiezwerk Presse 26** (garbage) | Vmax | 85 km/h | 85 km/h | +0.2 % | ✓ | – |
| **Kiezwerk Stadtbus 12** (bus) | Vmax | 90 km/h | 90 km/h | +0.1 % | ✓ | – |

