#!/usr/bin/env python3
"""Baut die Schuss-Samples des Spiels aus der „Free Firearm Sound Library“ (CC0, docs/audio.md).

Quelle : https://opengameart.org/content/the-free-firearm-sound-library – „Prepared SFX Library.7z“ (194 MB),
         CC0 („NO RIGHTS RESERVED … may be used without royalty or credit“), Aufnahmen von Ben Jaszczak,
         Brian Nelson, Kevin Heras und Matthew Nanney (2013/14). Wird bei Bedarf geladen (Prüfsumme) und nur
         im Zwischenspeicher tools/audio/.cache/ entpackt – ins Repo kommen nur die fertigen Samples.
Ausgang: data/audio/weapons/ – je Waffe 2–3 Schussvarianten (Mono, 48 kHz, 16 Bit) und manifest.json

Je Variante zwei Schichten derselben Waffe, am Knall ausgerichtet:
  nah    („near distance, left/right“): der trockene, harte Knall des Schützen
  mittel („mid distance, front“): Körper und Echo vom Schießstand – leiser und 15 ms später dazugemischt, wie
         der Widerhall von Fassaden. Daraus entsteht der Nachhall, den die trockene Nahaufnahme nicht hat.
Maschinenpistole: die Aufnahmen sind Zweierstöße (Abstand ~0,09 s); der Knall nimmt nur den ersten Schuss, das
Spiel schießt den Takt selbst und blendet den Nachhall des vorigen Schusses aus (synth.rs, „Choke“).

Aufruf : python tools/audio/build_weapon_sounds.py    (braucht 7z, ffmpeg nicht nötig; Pakete aus requirements.txt)
Idempotent: gleicher Eingang ergibt bitgleiche Dateien.
"""
import hashlib
import json
import subprocess
import sys
import urllib.request
from pathlib import Path

import librosa
import numpy as np
import soundfile as sf
from scipy import signal

ROOT = Path(__file__).resolve().parents[2]
CACHE = ROOT / "tools/audio/.cache"
OUT = ROOT / "data/audio/weapons"
URL = "https://opengameart.org/sites/default/files/Prepared%20SFX%20Library.7z"
SHA256 = "cc1ab5a99a0a365105c7c5dd783f4b0b1fe90938114d3ceec53856bfe005f7d6"
LIB = "Prepared SFX Library"
SR = 48000
LENGTH_S = 1.5
MID_DELAY_S = 0.015

# Waffe im Spiel → (nah, mittel, Pegel der Mittel-Schicht dB, Knall-Länge s oder None = ganzer Schuss, Varianten)
WEAPONS = {
    "pistol": {
        "vorbild": "Walther PPQ, 9 mm",
        "nah": "Walther PPQ/X_39P.wav",
        "mittel": "Walther PPQ/X_31P.wav",
        "mittel_db": -4.0,
        "knall_s": None,
        "varianten": 3,
    },
    "smg": {
        "vorbild": "Carl Gustav M45 („Swedish K“), 9 mm",
        "nah": "Carl Gustav M45/G_31P.wav",
        "mittel": "Carl Gustav M45/G_20P.wav",
        "mittel_db": -6.0,
        "knall_s": 0.085,
        "varianten": 3,
    },
    "shotgun": {
        "vorbild": "Benelli Nova, Pump-Action, Kaliber 12",
        "nah": "Nova/O_21P.wav",
        "mittel": "Nova/O_17P.wav",
        "mittel_db": -3.0,
        "knall_s": None,
        "varianten": 2,
    },
}


def fetch() -> Path:
    """Bibliothek laden (falls nötig), Prüfsumme prüfen, die drei Waffen entpacken."""
    CACHE.mkdir(parents=True, exist_ok=True)
    arc = CACHE / "ffsl.7z"
    if not arc.exists():
        print("lade", URL)
        urllib.request.urlretrieve(URL, arc)
    digest = hashlib.sha256(arc.read_bytes()).hexdigest()
    if digest != SHA256:
        sys.exit(f"Prüfsumme stimmt nicht: {digest}")
    wanted = sorted({w[k].split("/")[0] for w in WEAPONS.values() for k in ("nah", "mittel")})
    if not all((CACHE / LIB / d).exists() for d in wanted):
        subprocess.run(
            ["7z", "x", "-y", f"-o{CACHE}", str(arc)] + [f"{LIB}/{d}/*" for d in wanted],
            check=True,
            stdout=subprocess.DEVNULL,
        )
    return CACHE / LIB


def load(path: Path) -> np.ndarray:
    y, sr = sf.read(path, dtype="float64", always_2d=True)
    m = y.mean(axis=1)
    return librosa.resample(m, orig_sr=sr, target_sr=SR, res_type="soxr_hq")


def shots(m: np.ndarray) -> list[int]:
    """Erster Schuss jeder Gruppe (Zweierstöße der MP zählen als einer): Sample-Index des Spitzenwerts."""
    on = librosa.onset.onset_detect(y=m.astype(np.float32), sr=SR, units="samples", delta=0.3)
    starts = []
    for o in on:
        if not starts or o - starts[-1] > 0.5 * SR:
            starts.append(int(o))
    # vom Einsatz auf den Spitzenwert (der Knall) innerhalb von 30 ms
    return [s + int(np.argmax(np.abs(m[s : s + int(0.03 * SR)]))) for s in starts]


def build() -> dict:
    lib = fetch()
    OUT.mkdir(parents=True, exist_ok=True)
    for old in OUT.glob("*.wav"):
        old.unlink()
    manifest = {
        "_hinweis": "Erzeugt von tools/audio/build_weapon_sounds.py – nicht von Hand ändern.",
        "quelle": "The Free Firearm Sound Library (Prepared SFX Library), https://opengameart.org/content/the-free-firearm-sound-library",
        "lizenz": "CC0 1.0 – „Our team holds CC0 NO RIGHTS RESERVED for this library. It may be used without royalty or credit.“",
        "urheber": "Ben Jaszczak, Brian Nelson, Kevin Heras, Matthew Nanney",
        "quelle_sha256": SHA256,
        "samplerate": SR,
        "waffen": {},
    }
    n = int(LENGTH_S * SR)
    pre = int(0.004 * SR)
    for weapon, cfg in WEAPONS.items():
        near = load(lib / cfg["nah"])
        mid = load(lib / cfg["mittel"])
        near_p, mid_p = shots(near), shots(mid)
        files = []
        for i in range(min(cfg["varianten"], len(near_p))):
            a = near_p[i] - pre
            x = np.zeros(n)
            seg = near[a : a + n]
            if cfg["knall_s"]:
                # nur der erste Schuss des Zweierstoßes, mit kurzer Ausblendung vor dem zweiten
                k = int(cfg["knall_s"] * SR)
                seg = seg[:k].copy()
                fade = int(0.012 * SR)
                seg[-fade:] *= np.linspace(1, 0, fade) ** 2
            x[: len(seg)] += seg
            # Mittel-Schicht: Körper und Echo, am Knall ausgerichtet, leicht verzögert
            b = mid_p[i % len(mid_p)] - pre
            d = int(MID_DELAY_S * SR)
            body = mid[b : b + n - d]
            x[d : d + len(body)] += body * 10 ** (cfg["mittel_db"] / 20)
            # Gleichanteil und tiefes Rumpeln weg, Ende ausblenden
            bb, aa = signal.butter(2, 35 / (SR / 2), "high")
            x = signal.lfilter(bb, aa, x)
            tail = int(0.35 * SR)
            x[-tail:] *= np.linspace(1, 0, tail) ** 2
            x *= 10 ** (-1.0 / 20) / np.abs(x).max()
            name = f"{weapon}_{i + 1}.wav"
            sf.write(OUT / name, x.astype(np.float32), SR, subtype="PCM_16")
            files.append(name)
        manifest["waffen"][weapon] = {
            "vorbild": cfg["vorbild"],
            "dateien": files,
            "nah": cfg["nah"],
            "mittel": cfg["mittel"],
            "mittel_db": cfg["mittel_db"],
            "knall_s": cfg["knall_s"],
        }
        print(weapon, files)
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
    return manifest


if __name__ == "__main__":
    build()
