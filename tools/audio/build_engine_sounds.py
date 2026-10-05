#!/usr/bin/env python3
"""Baut aus Referenzaufnahmen die Motor-Samples des Spiels (docs/audio.md).

Bänke (je eine Aufnahme, wird nur gelesen):
  v10  data/audio/raw/engine_reference_lambo.m4a → data/audio/engine/v10/  (wegfahrender Supersportwagen, 8,7 s)
  v12  data/audio/raw/engine_reference_svj.m4a   → data/audio/engine/v12/  (Twin-Turbo-V12 am Prüfstand, 169 s)
Ausgang je Bank: Loops (Last/Schub), Einzelklänge, Referenz für A/B, manifest.json

Aufruf  : python tools/audio/build_engine_sounds.py [BANK …]   (ohne Angabe: alle)
Braucht : ffmpeg, Pakete aus tools/audio/requirements.txt

Ablauf je Loop (siehe docs/audio.md):
  1. Grundton über einen Obertonkamm verfolgen (wie analyze_reference.py), geglättet.
  2. „Sweep glätten“: Das Quellfenster wird zeitvariabel nachgetastet, sodass die Tonhöhe auf dem Mittelwert des
     Fensters stillsteht (ds/dn = f_ziel / f(s)). Ohne das würde ein Ausschnitt aus einem Hochdrehen als Loop jaulen.
  3. Pegelschwankungen innerhalb des Fensters ausgleichen (±4 dB höchstens).
  4. Länge = ganze Zahl von Grundtonperioden; Loop-Punkt mit gleichstarkem Crossfade (25 ms) über das, was hinter
     dem Ende kommt – der Übergang Ende → Anfang ist damit lückenlos, ohne Klick.
  5. Lautheit aller Loops auf einen Zielwert (über den mehrfach wiederholten Loop gemessen).
Schub-Loops (Gas weg): v10 nur abgeleitet (die Aufnahme enthält keinen Schub, das Auto fährt weg, siehe
SEGMENTE.md); v12 aus dem echten Ausrollen am Prüfstand, nur unterhalb davon abgeleitet.

Idempotent: Gleicher Eingang ergibt bitgleiche Dateien (kein Zufall, feste Parameter, 16-Bit-PCM-WAV).
"""
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
import pyloudnorm
import soundfile as sf
from scipy import signal

ROOT = Path(__file__).resolve().parents[2]
SR = 48000
C = 343.0  # Schallgeschwindigkeit m/s

LOOP_LUFS = -18.0
XFADE_S = 0.025

# v10 – Drehzahl-Annahme B (analysis/SEGMENTE.md): V10, Grundton = halbe Zündfrequenz → Drehzahl = f0 × 24.
# Last-Loops: Name, Quellfenster (s), Notch-Filter (Hz) gegen den Fremdton am Anfang, Sweep glätten?
# Der Leerlauf wird nicht geglättet: dort ist die Tonhöhe ohnehin fast konstant, die Verfolgung aber unsicher.
V10 = {
    "src": "data/audio/raw/engine_reference_lambo.m4a",
    "rpm_per_hz": 24.0,
    "annahme": "V10, Grundton = halbe Zündfrequenz: Drehzahl = f0 × 24, Doppler des wegfahrenden Autos herausgerechnet",
    "verfolgung": "kamm",
    "doppler": True,
    "loops": [
        ("idle", 0.35, 0.95, [236.9, 453.8], False),
        ("r2600", 1.30, 1.62, [], True),
        ("r3300", 1.80, 2.32, [], True),
        ("r4900", 2.64, 2.88, [], True),
        ("r6000", 2.98, 3.22, [], True),
        ("r7200", 3.30, 3.64, [], True),
    ],
    # (Name, Quellfenster, Startfrequenz der Linie) – v10 hat keine echten Schub-Loops
    "schub": [],
    # Einzelklänge: Name, Art, von, bis (s), Ein-/Ausblendung (s), Zielpegel
    "einzel": [
        ("start", "start", 0.105, 0.95, (0.002, 0.25), ("lufs", -16.0)),
        ("blip", "blip", 1.05, 1.70, (0.01, 0.25), ("lufs", -16.0)),
        ("shift_1", "schalten", 3.70, 4.06, (0.005, 0.08), ("lufs", -20.0)),
        ("shift_2", "schalten", 5.30, 5.52, (0.005, 0.06), ("lufs", -20.0)),
        ("shift_3", "schalten", 7.09, 7.27, (0.005, 0.05), ("lufs", -20.0)),
        ("pop_1", "pop", 7.845, 7.965, (0.002, 0.05), ("peak", -3.0)),
        ("pop_2", "pop", 8.015, 8.135, (0.002, 0.05), ("peak", -3.0)),
        ("pop_3", "pop", 8.215, 8.335, (0.002, 0.05), ("peak", -3.0)),
        ("pop_4", "pop", 3.745, 3.865, (0.002, 0.05), ("peak", -3.0)),
        ("pop_5", "pop", 3.88, 4.0, (0.002, 0.05), ("peak", -3.0)),
    ],
    # Referenz für A/B: ganze Aufnahme
    "referenz": None,
}

# v12 – Twin-Turbo-V12 am Prüfstand (analysis_svj/SEGMENTE.md): Mikrofon steht, kein Doppler. Die hellste Linie ist
# die Zündfrequenz (Drehzahl/60 × 6) → Drehzahl = f0 × 10; ein Zug läuft von ~250 auf ~870 Hz (≈ Begrenzer 8700).
# Last-Loops aus dem Volllast-Zug 21,2–27,4 s, Leerlauf 15,0–16,9 s (danach läuft die Linie weg); Schub aus dem Ausrollen 83,4–89,3 s.
# Linienverfolgung: Startfrequenz je Fenster (dritter Wert in den Loop-Zeilen: Notch-Liste bleibt leer).
V12 = {
    "src": "data/audio/raw/engine_reference_svj.m4a",
    "rpm_per_hz": 10.0,
    "annahme": "V12 (Viertakt), hellste Linie = Zündfrequenz: Drehzahl = f0 × 10; Prüfstand, kein Doppler",
    "verfolgung": "linie",
    "doppler": False,
    "loops": [
        ("idle", 15.0, 16.9, 129.0),
        ("r2700", 21.55, 21.95, 263.0),
        ("r3300", 22.60, 23.00, 318.0),
        ("r4100", 23.60, 23.90, 395.0),
        ("r5600", 24.75, 25.05, 560.0),
        ("r6800", 25.55, 25.85, 660.0),
        ("r8400", 26.85, 27.15, 830.0),
    ],
    # echte Schub-Loops (Gas weg, Ausrollen am Prüfstand): Name, Quellfenster, Startfrequenz
    "schub": [
        ("r4000", 88.05, 88.40, 414.0),
        ("r4600", 87.30, 87.80, 480.0),
        ("r5800", 86.00, 86.50, 595.0),
        ("r7700", 83.70, 84.20, 795.0),
    ],
    # unterhalb des tiefsten echten Schub-Loops: aus diesen Last-Loops abgeleitet
    "schub_abgeleitet": ["idle", "r2700"],
    "einzel": [
        ("blip_1", "blip", 70.85, 71.95, (0.01, 0.25), ("lufs", -16.0)),
        ("blip_2", "blip", 74.22, 75.05, (0.01, 0.25), ("lufs", -16.0)),
        ("pop_1", "pop", 99.645, 99.765, (0.002, 0.05), ("peak", -3.0)),
        ("pop_2", "pop", 99.81, 99.93, (0.002, 0.05), ("peak", -3.0)),
        ("pop_3", "pop", 104.68, 104.80, (0.002, 0.05), ("peak", -3.0)),
        ("pop_4", "pop", 113.525, 113.645, (0.002, 0.05), ("peak", -3.0)),
        ("pop_5", "pop", 131.095, 131.215, (0.002, 0.05), ("peak", -3.0)),
        ("pop_6", "pop", 137.83, 137.95, (0.002, 0.05), ("peak", -3.0)),
        ("pop_7", "pop", 138.51, 138.63, (0.002, 0.05), ("peak", -3.0)),
    ],
    # Referenz für A/B: Zug, Ausrollen und Fehlzündungen statt der ganzen 169 s (sonst 16 MB WAV)
    "referenz": [(20.5, 31.5), (79.5, 89.5), (98.5, 105.0), (137.0, 139.5)],
}
# r4 – Mercedes-Benz 190E 2.3-16V (Reihenvierzylinder) am Prüfstand, Chippy569 „Kickstarter Dyno Session“ (CC0,
# Mikrofon DPA 4062 über dem Scheinwerfer, steht – kein Doppler). Vierzylinder-Viertakt: hellste Linie = Zündfrequenz
# (Drehzahl/60 × 2) → Drehzahl = f0 × 30; bestätigt an den Festdrehzahlen 2500 (81 Hz) und 6500 (210 Hz, darüber
# die Kurbelwellendrehung 105 Hz und ihre Vielfachen). Teile: Lastzug 68→236 Hz (Teil 0), Ausrollen 233→36 Hz
# (Teil 1), Leerlauf 34 Hz (Teil 2), Start (Teil 3). Zeiten als (Teil, s).
R4 = {
    "teile": [
        {"freesound": 162499, "sha256": "32976ff8599b600b3045d5db7f3336f8236134e959d13a27d372fd246db1f0a3",
         "titel": "Acceleration Sweep - 4062"},
        {"freesound": 162504, "sha256": "e7a6972d3ac0b7d38254af36dff5e306aa0458f3ac1e92845cdf469eae76fd37",
         "titel": "17_MedSweep_D6"},
        {"freesound": 162516, "sha256": "f2ace5576d090a92369bb5f82ed579be94783da5c300ce0cad18282e4c054c09",
         "titel": "21_500_4062"},
        {"freesound": 162526, "sha256": "44f14083114a59b7522c5bd63a1b2175bfabf2d506c21d314d218790bf1a021e",
         "titel": "23_StartBig_4062"},
    ],
    "lizenz": "CC0 1.0",
    "urheber": "Chippy569",
    "rpm_per_hz": 30.0,
    "annahme": "Reihenvierzylinder (Viertakt), hellste Linie = Zündfrequenz: Drehzahl = f0 × 30; Prüfstand, kein Doppler",
    "verfolgung": "linie",
    "doppler": False,
    "loops": [
        ("idle", (2, 3.0), (2, 3.8), 34.0, False),
        ("r2000", (0, 1.15), (0, 1.35), 67.0),
        ("r3300", (0, 1.95), (0, 2.1), 114.0),
        ("r4300", (0, 2.40), (0, 2.58), 143.0),
        ("r5300", (0, 2.95), (0, 3.12), 177.0),
        ("r6300", (0, 3.42), (0, 3.60), 210.0),
        ("r7000", (0, 3.92), (0, 4.10), 232.0),
    ],
    "schub": [
        ("r1500", (1, 7.20), (1, 7.50), 51.0),
        ("r2800", (1, 4.95), (1, 5.35), 95.0),
        ("r3800", (1, 3.55), (1, 3.95), 128.0),
        ("r5000", (1, 2.05), (1, 2.45), 167.0),
        ("r6400", (1, 0.45), (1, 0.85), 216.0),
    ],
    "schub_abgeleitet": ["idle"],
    "einzel": [
        ("start", "start", (3, 0.10), (3, 1.60), (0.002, 0.25), ("peak", -1.0)),
    ],
    "referenz": [((0, 0.0), (0, 4.27)), ((1, 0.0), (1, 8.5))],
}
# d4 – Diesel-Vierzylinder, Renault Master F3500 dCi135 (Transporter), Soundholder (Freesound, CC BY 3.0), steht.
# Viertakt-Vierzylinder: hellste Linie = Zündfrequenz → Drehzahl = f0 × 30. Teil 0: Drehzahlrampe 44 → 144 Hz
# (1330 → 4330 1/min, Nenndrehzahl eines Transporterdiesels) und zurück auf 51 Hz – hoch = Last, runter = Schub.
# Teil 1: Anlassen, Leerlauf stetig 25,9 Hz (≈ 780 1/min), Gasstöße.
D4 = {
    "teile": [
        {"freesound": 425846, "sha256": "859156521b08388ae55a14d4424458cdbf7c828a3aaa24af9918a6cf86a100e5",
         "titel": "Renault Master F3500 dCi135 Engine RPM Ramp 2 Mono"},
        {"freesound": 425845, "sha256": "b31137b94000096490e8a7cf010b5c102639e172a0b27aed9194a0dd40b910a4",
         "titel": "Renault Master F3500 dCi135 Engine Start Up Idle Gas Pressing And Shut Down Mono"},
    ],
    "lizenz": "CC BY 3.0",
    "urheber": "Soundholder",
    "rpm_per_hz": 30.0,
    "annahme": "Diesel-Vierzylinder (Viertakt), hellste Linie = Zündfrequenz: Drehzahl = f0 × 30; steht, kein Doppler",
    "verfolgung": "linie",
    "doppler": False,
    "loops": [
        ("idle", (1, 7.0), (1, 9.0), 25.9, False),
        ("r1700", (0, 1.95), (0, 2.25), 54.0),
        ("r2300", (0, 3.30), (0, 3.55), 77.0),
        ("r2900", (0, 4.25), (0, 4.50), 96.0),
        ("r3500", (0, 5.20), (0, 5.45), 116.0),
        ("r4100", (0, 6.15), (0, 6.40), 134.0),
        ("r4300", (0, 8.00), (0, 8.50), 144.0),
    ],
    "schub": [
        ("r1700", (0, 16.0), (0, 16.5), 55.5),
        ("r2400", (0, 13.40), (0, 13.80), 84.0),
        ("r3000", (0, 12.00), (0, 12.35), 102.0),
        ("r3600", (0, 10.8), (0, 11.1), 120.0),
        ("r4200", (0, 9.40), (0, 9.70), 140.0),
    ],
    "schub_abgeleitet": ["idle"],
    "einzel": [
        ("start", "start", (1, 0.70), (1, 2.40), (0.002, 0.25), ("peak", -1.0)),
        ("blip_1", "blip", (1, 2.60), (1, 5.40), (0.01, 0.25), ("lufs", -16.0)),
        ("blip_2", "blip", (1, 11.60), (1, 14.40), (0.01, 0.25), ("lufs", -16.0)),
    ],
    "referenz": [((0, 0.0), (0, 17.45))],
}
# d6 – großer Sechszylinder-Diesel: Mack-Sattelzug an der Ampel, Leerlauf und langsames Anfahren unter Last, kyles
# „truck mack transport semi trailer idle at red light and pull away slow long diesel rev“ (Freesound, CC0).
# Sechszylinder-Viertakt: hellste Linie = Zündfrequenz → Drehzahl = f0 × 20; Leerlauf 32,4 Hz ≈ 650 1/min, Anfahren
# 40 → 97 Hz (800 → 1940 1/min) im ersten Gang. Er rollt langsam an (Doppler ≈ 1 %, vernachlässigt). Echter Schub
# fehlt – abgeleitet wie beim V10. (Eine Bus-Aufnahme im Stand war verworfen: ihre Linien sind oberhalb von
# 1230 1/min mehrdeutig.)
D6 = {
    "teile": [
        {"freesound": 451935, "sha256": "138b892eab3b90d0ce99243a56ab709b8d7f0b78c5c7d7a45852e5ba004f6d3d",
         "titel": "truck mack transport semi trailer idle at red light and pull away slow long diesel rev"},
    ],
    "lizenz": "CC0 1.0",
    "urheber": "kyles",
    "rpm_per_hz": 20.0,
    "annahme": "Sechszylinder-Diesel (Viertakt), hellste Linie = Zündfrequenz: Drehzahl = f0 × 20; rollt langsam an",
    "verfolgung": "linie",
    "doppler": False,
    "loops": [
        ("idle", (0, 1.2), (0, 2.6), 32.4, False),
        ("r1150", (0, 3.62), (0, 3.92), 57.5),
        ("r1350", (0, 4.20), (0, 4.45), 66.5),
        ("r1550", (0, 4.85), (0, 5.08), 76.5),
        ("r1750", (0, 5.2), (0, 5.42), 84.0),
        ("r1900", (0, 5.70), (0, 6.00), 96.5),
    ],
    "schub": [],
    "schub_abgeleitet": ["idle", "r1150", "r1350", "r1550", "r1750", "r1900"],
    "einzel": [],
    "referenz": [((0, 0.0), (0, 10.0))],
}
BANKS = {"v10": V10, "v12": V12, "r4": R4, "d4": D4, "d6": D6}
GAP_S = 0.5


def compose(name: str, teile: list) -> tuple[np.ndarray, list[float]]:
    """Freesound-Teile (tools/audio/.cache/sfx, sonst laden; Prüfsumme) zu einer Zeitachse, mit Lücken dazwischen."""
    cache = ROOT / "tools/audio/.cache/sfx"
    cache.mkdir(parents=True, exist_ok=True)
    parts, offsets, t = [], [], 0.0
    for teil in teile:
        sid = teil["freesound"]
        hits = list(cache.glob(f"fs_{sid}.*"))
        if not hits:
            sys.path.insert(0, str(Path(__file__).parent))
            import freesound as fs

            meta = fs.info(sid)
            path = cache / f"fs_{sid}.{meta['type']}"
            path.write_bytes(fs.download(sid))
        else:
            path = hits[0]
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest != teil["sha256"]:
            sys.exit(f"{name}: Prüfsumme von Freesound {sid} stimmt nicht ({digest})")
        y = decode(path)
        offsets.append(t)
        parts += [y, np.zeros(int(GAP_S * SR))]
        t += len(y) / SR + GAP_S
    return np.concatenate(parts), offsets


def decode(src: Path) -> np.ndarray:
    with tempfile.TemporaryDirectory() as d:
        wav = Path(d) / "ref.wav"
        subprocess.run(
            ["ffmpeg", "-v", "error", "-y", "-i", str(src), "-ac", "1", "-ar", str(SR), "-c:a", "pcm_f32le", str(wav)],
            check=True,
        )
        y, sr = sf.read(wav, dtype="float64")
    assert sr == SR
    return y


def doppler(t: float) -> float:
    """Faktor, um den das wegfahrende Auto tiefer klingt (c/(c+v)), aus einer geschätzten Geschwindigkeit:
    Start bei 1,05 s, 9,5 m/s² bis 3,7 s, danach 5 m/s². Nur für die Drehzahl-Beschriftung, nicht für den Klang."""
    if t <= 1.05:
        v = 0.0
    elif t <= 3.7:
        v = 9.5 * (t - 1.05)
    else:
        v = 9.5 * 2.65 + 5.0 * (t - 3.7)
    return C / (C + v)


def comb_track(y: np.ndarray, hop: int = 240, n_fft: int = 16384) -> tuple[np.ndarray, np.ndarray]:
    """Grundton je Rahmen über einen Obertonkamm (50–450 Hz, acht Vielfache), Zeitachse in Sekunden."""
    win = np.hanning(n_fft)
    pad = np.pad(y, (n_fft // 2, n_fft // 2))
    freqs = np.fft.rfftfreq(n_fft, 1 / SR)
    cand = np.arange(50.0, 450.0, 0.25)
    idx = [np.clip(np.searchsorted(freqs, cand * k), 0, len(freqs) - 1) for k in range(1, 9)]
    frames = range(0, len(y), hop)
    f0 = np.empty(len(frames))
    for j, s in enumerate(frames):
        col = np.log1p(np.abs(np.fft.rfft(pad[s : s + n_fft] * win)) * 100)
        sc = sum(col[ix] / k**0.3 for k, ix in enumerate(idx, 1))
        f0[j] = cand[int(np.argmax(sc))]
    t = np.arange(len(f0)) * hop / SR
    return t, f0


def line_track(y: np.ndarray, t0: float, t1: float, f_start: float, tol: float = 0.07) -> tuple[np.ndarray, np.ndarray]:
    """Eine Linie im Spektrogramm verfolgen: je Rahmen das stärkste Maximum innerhalb ±7 % der vorigen Frequenz
    (parabolisch interpoliert), beginnend bei f_start. Für V12 robuster als der Kamm, der dort auf die halbe
    Frequenz rutschen kann."""
    n_fft, hop = 8192, 480
    win = np.hanning(n_fft)
    a = max(0, int(t0 * SR) - n_fft // 2)
    seg = y[a : int(t1 * SR) + n_fft // 2]
    freqs = np.fft.rfftfreq(n_fft, 1 / SR)
    f = f_start
    out, ts = [], []
    for s0 in range(0, len(seg) - n_fft, hop):
        mag = np.abs(np.fft.rfft(seg[s0 : s0 + n_fft] * win))
        w = max(f * tol, 2.5 * SR / n_fft)  # mindestens zwei Bins breit
        idx = np.where((freqs > f - w) & (freqs < f + w))[0]
        k = min(max(int(idx[np.argmax(mag[idx])]), 1), len(mag) - 2)
        l0, l1, l2 = np.log(mag[k - 1 : k + 2] + 1e-12)
        den = l0 - 2 * l1 + l2
        p = 0.5 * (l0 - l2) / den if den != 0 else 0.0
        f = (k + p) * SR / n_fft
        out.append(f)
        ts.append((a + s0 + n_fft // 2) / SR)
    return np.array(ts), np.array(out)


def smooth_track(t: np.ndarray, f0: np.ndarray, t0: float, t1: float, sweep: bool) -> np.ndarray:
    """Grundton im Fenster als Polynom 2. Grades (Sweeps sind glatt): Median gegen Oktavsprünge, Punkte mehr als
    8 % neben der ersten Anpassung verworfen, neu angepasst. Ohne Sweep: konstanter Median."""
    m = (t >= t0) & (t <= t1)
    tt, ff = t[m], signal.medfilt(f0[m], 15)
    if not sweep:
        return np.array([0.0, 0.0, float(np.median(ff))])
    coef = np.polyfit(tt, ff, 2)
    ok = np.abs(ff / np.polyval(coef, tt) - 1) < 0.08
    return np.polyfit(tt[ok], ff[ok], 2)


def flatten(y: np.ndarray, coef: np.ndarray, t0: float, n_out: int, f_target: float) -> np.ndarray:
    """Zeitvariables Nachtasten: Ausgabe hat konstanten Grundton f_target."""
    s = np.empty(n_out)
    pos = t0 * SR
    for n in range(n_out):
        s[n] = pos
        f = np.polyval(coef, pos / SR)
        pos += f_target / f
    return np.interp(s, np.arange(len(y)), y)


def level_flatten(x: np.ndarray, period: float) -> np.ndarray:
    """Langsame Pegelschwankungen ausgleichen (Hüllkurve über vier Perioden, höchstens ±4 dB)."""
    n = max(16, int(period * 4))
    env = np.sqrt(np.convolve(x**2, np.ones(n) / n, mode="same")) + 1e-9
    env = np.convolve(env, np.ones(n) / n, mode="same")
    g = np.clip(env.mean() / env, 10 ** (-4 / 20), 10 ** (4 / 20))
    return x * g


def loudness(x: np.ndarray, tile: bool) -> float:
    meter = pyloudnorm.Meter(SR)
    z = np.tile(x, int(np.ceil(4 * SR / len(x)))) if tile else np.pad(x, (0, max(0, SR // 2 - len(x))))
    return float(meter.integrated_loudness(z))


def seam_ratio(x: np.ndarray) -> float:
    """Sprung am Loop-Punkt im Verhältnis zum größten Schritt in seiner Umgebung (±64 Samples). ≤ 1 heißt: der
    Übergang Ende → Anfang ist nicht steiler als das Signal dort ohnehin – kein Klick."""
    ring = np.concatenate([x[-64:], x[:64]])
    local = np.abs(np.diff(ring))
    local = np.delete(local, 63)  # der Nahtschritt selbst
    return float(abs(x[0] - x[-1]) / (local.max() + 1e-12))


def residual_cents(x: np.ndarray, f_target: float, linie: bool = False) -> float:
    """Tonhöhendrift im fertigen Loop (Kamm bzw. Linienverfolgung über den zweimal wiederholten Loop)."""
    if linie:
        z = np.tile(x, 3)
        t, f = line_track(z, 0.0, len(z) / SR, f_target, tol=0.05)
        f = f[(t > len(x) / SR * 0.5) & (t < len(x) / SR * 2.5)]
        if len(f) < 3:
            return 0.0
        return float(1200 * np.log2(np.percentile(f, 90) / np.percentile(f, 10)))
    t, f = comb_track(np.tile(x, 2), hop=480, n_fft=8192)
    f = f[(t > 0.05) & (t < len(x) * 2 / SR - 0.05)]
    if len(f) < 3:
        return 0.0
    return float(1200 * np.log2(np.percentile(f, 90) / np.percentile(f, 10)))


def circular_filter(x: np.ndarray, b: np.ndarray, a: np.ndarray) -> np.ndarray:
    """Filter über den dreifach wiederholten Loop, Mitte behalten – so bleibt der Loop-Punkt nahtlos."""
    z = signal.lfilter(b, a, np.tile(x, 3))
    return z[len(x) : 2 * len(x)]


def write(path: Path, x: np.ndarray) -> None:
    peak = np.abs(x).max()
    if peak > 0.999:
        raise SystemExit(f"{path.name}: Übersteuerung ({peak:.3f})")
    sf.write(path, x.astype(np.float32), SR, subtype="PCM_16")


def make_loop(src: np.ndarray, coef: np.ndarray, t0: float, t1: float, f_target: float) -> np.ndarray:
    """Fenster glätten, Pegel ausgleichen, ganze Perioden, Crossfade am Loop-Punkt, Lautheit."""
    period = SR / f_target
    # verfügbare Länge in Ausgabesamples (die Tonhöhe ändert die Dauer leicht)
    span = (t1 - t0) * SR * f_target / float(np.mean(np.polyval(coef, np.linspace(t0, t1, 64))))
    xf = int(XFADE_S * SR)
    periods = int((span - xf) // period)
    n_loop = int(round(periods * period))
    flat = flatten(src, coef, t0, n_loop + xf, f_target)
    flat = level_flatten(flat, period)
    # Loop-Punkt: der Kopf wird mit dem Stück hinter dem Ende überblendet (gleiche Leistung)
    u = np.linspace(0, 1, xf, endpoint=False)
    head = flat[:xf] * np.sin(u * np.pi / 2) + flat[n_loop : n_loop + xf] * np.cos(u * np.pi / 2)
    loop = np.concatenate([head, flat[xf:n_loop]])
    # Gleichanteil weg, Lautheit angleichen
    loop -= loop.mean()
    loop *= 10 ** ((LOOP_LUFS - loudness(loop, True)) / 20)
    return loop


def derive_off(loop: np.ndarray) -> np.ndarray:
    """Schub aus einem Last-Loop ableiten – Tiefpass, Hochpass gegen Wummern, leichte Sättigung (−7 dB im Manifest)."""
    b, a = signal.butter(2, 1500 / (SR / 2), "low")
    off = circular_filter(loop, b, a)
    b, a = signal.butter(1, 60 / (SR / 2), "high")
    off = circular_filter(off, b, a)
    off = np.tanh(off * 2.2) / 2.2
    off -= off.mean()
    off *= 10 ** ((LOOP_LUFS - loudness(off, True)) / 20)
    return off


def resolve(cfg: dict, offsets: list[float]) -> dict:
    """Zeiten der Form (Teil, s) auf die zusammengesetzte Zeitachse umrechnen."""
    def tt(v):
        return offsets[v[0]] + v[1] if isinstance(v, tuple) else v

    out = dict(cfg)
    out["loops"] = [(r[0], tt(r[1]), tt(r[2]), *r[3:]) for r in cfg["loops"]]
    out["schub"] = [(r[0], tt(r[1]), tt(r[2]), *r[3:]) for r in cfg["schub"]]
    out["einzel"] = [(r[0], r[1], tt(r[2]), tt(r[3]), *r[4:]) for r in cfg["einzel"]]
    if cfg["referenz"] is not None:
        out["referenz"] = [(tt(a), tt(b)) for a, b in cfg["referenz"]]
    return out


def build(name: str, cfg: dict) -> list:
    out = ROOT / "data/audio/engine" / name
    if "teile" in cfg:
        y, offsets = compose(name, cfg["teile"])
        cfg = resolve(cfg, offsets)
        quelle = [
            {**t, "url": f"https://freesound.org/s/{t['freesound']}/", "beginn_s": round(o, 3)}
            for t, o in zip(cfg["teile"], offsets)
        ]
        sha = hashlib.sha256(b"".join(t["sha256"].encode() for t in cfg["teile"])).hexdigest()
    else:
        src_path = ROOT / cfg["src"]
        if not src_path.exists():
            sys.exit(f"Referenz fehlt: {src_path}")
        y = decode(src_path)
        quelle = cfg["src"]
        sha = hashlib.sha256(src_path.read_bytes()).hexdigest()
    out.mkdir(parents=True, exist_ok=True)
    for old in out.glob("*.wav"):
        old.unlink()
    linie = cfg["verfolgung"] == "linie"
    rph = cfg["rpm_per_hz"]
    dop = doppler if cfg["doppler"] else (lambda t: 1.0)
    if not linie:
        t_tr, f_tr = comb_track(y)
    manifest = {
        "bank": name,
        "_hinweis": "Erzeugt von tools/audio/build_engine_sounds.py – nicht von Hand ändern.",
        "quelle": quelle,
        "quelle_sha256": sha,
        **({"lizenz": cfg["lizenz"], "urheber": cfg["urheber"]} if "lizenz" in cfg else {}),
        "samplerate": SR,
        "drehzahl_annahme": cfg["annahme"],
        "loops": [],
        "einzel": [],
        "referenz": "reference.wav",
    }
    report = []
    on_loops = {}

    def entry(file, rpm, last, loop, f_target, t0, t1, gain_db, drift):
        return {
            "datei": file,
            "rpm": round(rpm),
            "last": last,
            "samples": len(loop),
            "loop_start": 0,
            "loop_ende": len(loop),
            "grundton_hz": round(f_target, 2),
            "quelle_s": [t0, t1],
            "gain_db": gain_db,
            "drift_cent": None if np.isnan(drift) else round(drift, 1),
        }

    for row in cfg["loops"]:
        if linie:
            # optional fünfter Wert: glätten (Leerlauf nicht – er schwankt von Natur aus, die Verfolgung ist dort
            # unsicher, wie beim V10)
            lname, t0, t1, f_seed, *rest = row
            sweep = rest[0] if rest else True
            t_l, f_l = line_track(y, t0, t1, f_seed)
            coef = smooth_track(t_l, f_l, t0, t1, sweep)
            notches = []
        else:
            lname, t0, t1, notches, sweep = row
            coef = smooth_track(t_tr, f_tr, t0, t1, sweep)
        tm = (t0 + t1) / 2
        f_target = float(np.polyval(coef, tm))
        src = y
        if notches:
            src = y.copy()
            for fn in notches:
                b, a = signal.iirnotch(fn, 12.0, SR)
                src = signal.filtfilt(b, a, src)
        loop = make_loop(src, coef, t0, t1, f_target)
        rpm = f_target * rph / dop(tm)
        drift = residual_cents(loop, f_target, linie) if sweep else float("nan")
        write(out / f"on_{lname}.wav", loop)
        e = entry(f"on_{lname}.wav", rpm, "on", loop, f_target, t0, t1, 0.0, drift)
        manifest["loops"].append(e)
        report.append((e["datei"], e["rpm"], len(loop) / SR, drift, seam_ratio(loop)))
        on_loops[lname] = (loop, e)
        if not linie or lname in cfg.get("schub_abgeleitet", []):
            off = derive_off(loop)
            write(out / f"off_{lname}.wav", off)
            manifest["loops"].append({**e, "datei": f"off_{lname}.wav", "last": "off", "gain_db": -7.0})
            report.append((f"off_{lname}.wav", e["rpm"], len(off) / SR, drift, seam_ratio(off)))

    # echte Schub-Loops (nur v12): Ausrollen, die Drehzahl fällt – geglättet wie die Züge
    for lname, t0, t1, f_seed in cfg["schub"]:
        t_l, f_l = line_track(y, t0, t1, f_seed)
        coef = smooth_track(t_l, f_l, t0, t1, True)
        tm = (t0 + t1) / 2
        f_target = float(np.polyval(coef, tm))
        loop = make_loop(y, coef, t0, t1, f_target)
        drift = residual_cents(loop, f_target, True)
        write(out / f"off_{lname}.wav", loop)
        # echter Schub ist leiser als Last; der Pegelabstand bleibt erhalten (gemessen am Original)
        e = entry(f"off_{lname}.wav", f_target * rph, "off", loop, f_target, t0, t1, -6.0, drift)
        manifest["loops"].append(e)
        report.append((e["datei"], e["rpm"], len(loop) / SR, drift, seam_ratio(loop)))

    for sname, kind, t0, t1, (fin, fout), (mode, target) in cfg["einzel"]:
        x = y[int(t0 * SR) : int(t1 * SR)].copy()
        x -= x.mean()
        n_in, n_out = max(1, int(fin * SR)), max(1, int(fout * SR))
        x[:n_in] *= np.linspace(0, 1, n_in)
        x[-n_out:] *= np.linspace(1, 0, n_out) ** 2
        if mode == "lufs":
            x *= 10 ** ((target - loudness(x, False)) / 20)
        else:
            x *= 10 ** (target / 20) / np.abs(x).max()
        write(out / f"{sname}.wav", x)
        if linie:
            # stärkste Linie zwischen ~75 und ~925 Hz
            t_l, f_l = line_track(y, t0, t1, 500.0, tol=0.85)
            rpm_shot = round(float(np.median(f_l)) * rph)
        else:
            rpm_shot = round(float(np.median(f_tr[(t_tr >= t0) & (t_tr <= t1)])) * rph)
        manifest["einzel"].append(
            {
                "datei": f"{sname}.wav",
                "art": kind,
                "samples": len(x),
                "quelle_s": [t0, t1],
                "gain_db": 0.0,
                "rpm": rpm_shot,
            }
        )

    # Referenz für den A/B-Vergleich (Mono, unverändert im Pegel; lange Aufnahmen nur als Auszug mit Blenden)
    if cfg["referenz"] is None:
        ref = y
    else:
        parts = []
        fade = int(0.05 * SR)
        for t0, t1 in cfg["referenz"]:
            p = y[int(t0 * SR) : int(t1 * SR)].copy()
            p[:fade] *= np.linspace(0, 1, fade)
            p[-fade:] *= np.linspace(1, 0, fade)
            parts.append(p)
        ref = np.concatenate(parts)
        manifest["referenz_auszug_s"] = cfg["referenz"]
        ref *= min(1.0, 0.98 / np.abs(ref).max())
    write(out / "reference.wav", ref)
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
    return report


def main() -> None:
    wanted = sys.argv[1:] or list(BANKS)
    bad = []
    for name in wanted:
        if name not in BANKS:
            sys.exit(f"Unbekannte Bank {name} (bekannt: {', '.join(BANKS)})")
        report = build(name, BANKS[name])
        print(f"== {name}")
        print(f"{'Datei':<16} {'rpm':>6} {'Länge s':>8} {'Drift ct':>9} {'Naht':>6}")
        for f, r, l, d, s in report:
            print(f"{f:<16} {r:>6} {l:>8.3f} {d:>9.1f} {s:>6.2f}")
        bad += [(name, *r) for r in report if (r[3] == r[3] and r[3] > 35) or r[4] > 1.0]
    if bad:
        sys.exit(f"Qualitätsgrenze verletzt: {bad}")


if __name__ == "__main__":
    main()
