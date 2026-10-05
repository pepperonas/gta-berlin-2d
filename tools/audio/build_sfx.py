#!/usr/bin/env python3
"""Baut die Klang-Samples des Spiels (außer Motoren und Schüssen) aus frei lizenzierten Aufnahmen (docs/audio.md).

Rezepte: tools/audio/sfx_recipes.json – je Quelle Archiv-URL, SHA-256, Lizenz und Urheber; je Klang die Varianten.
Archive werden nach tools/audio/.cache/sfx/ geladen (Prüfsumme) – ins Repo kommen nur die fertigen Samples.
Ausgang: data/audio/sfx/<klang>_<n>.wav (Mono, 48 kHz, 16 Bit) und manifest.json (Quelle, Lizenz, Urheber je Klang).

Je Variante: Schichten laden (Mono, 48 kHz), am Einsatz ausrichten (erster Wert über 10 % der Spitze, 3 ms davor),
um `at` versetzt und mit `db` gewichtet mischen, Hochpass, auf `laenge_s` kürzen, `fade_s` ausblenden, Spitze auf
`spitze_db` (Standard −1 dBFS). Die Lautstärke im Spiel regelt synth.rs (Pegel je Klang, durch Tests gegen den
früheren Synthese-Klang abgesichert).

Aufruf : python tools/audio/build_sfx.py [KLANG …]     (ohne Angabe: alle; Pakete aus requirements.txt)
Idempotent: gleicher Eingang ergibt bitgleiche Dateien.
"""
import fnmatch
import hashlib
import io
import json
import sys
import urllib.request
import zipfile
from pathlib import Path

import librosa
import numpy as np
import soundfile as sf
from scipy import signal

ROOT = Path(__file__).resolve().parents[2]
RECIPES = ROOT / "tools/audio/sfx_recipes.json"
CACHE = ROOT / "tools/audio/.cache/sfx"
OUT = ROOT / "data/audio/sfx"
SR = 48000


def archive(src_id: str, src: dict) -> zipfile.ZipFile:
    CACHE.mkdir(parents=True, exist_ok=True)
    path = CACHE / f"{src_id}.zip"
    if not path.exists():
        print("lade", src["url"])
        req = urllib.request.Request(src["url"], headers={"User-Agent": "gta-berlin-build"})
        path.write_bytes(urllib.request.urlopen(req).read())
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if digest != src["sha256"]:
        sys.exit(f"{src_id}: Prüfsumme stimmt nicht ({digest})")
    return zipfile.ZipFile(path)


def find(z: zipfile.ZipFile, pattern: str) -> list[str]:
    names = [n for n in z.namelist() if not n.startswith("__MACOSX") and fnmatch.fnmatch(n, pattern)]
    if not names:
        sys.exit(f"keine Datei passt auf {pattern}")
    return sorted(names)


def load(z: zipfile.ZipFile, name: str) -> np.ndarray:
    y, sr = sf.read(io.BytesIO(z.read(name)), dtype="float64", always_2d=True)
    m = y.mean(axis=1)
    if sr != SR:
        m = librosa.resample(m, orig_sr=sr, target_sr=SR, res_type="soxr_hq")
    # am Einsatz ausrichten
    pk = np.abs(m).max()
    on = int(np.argmax(np.abs(m) > pk * 0.1))
    return m[max(0, on - int(0.003 * SR)) :]


def variants(rec: dict, z: zipfile.ZipFile) -> list[list[dict]]:
    if "varianten" in rec:
        return rec["varianten"]
    pats = rec["dateien"] if isinstance(rec["dateien"], list) else [rec["dateien"]]
    return [[{"datei": n, "at": 0}] for p in pats for n in find(z, p)]


def render(rec: dict, layers: list[dict], z: zipfile.ZipFile) -> np.ndarray:
    n = int(rec["laenge_s"] * SR)
    x = np.zeros(n)
    for lay in layers:
        name = find(z, lay["datei"])[0]
        y = load(z, name) * 10 ** (lay.get("db", 0) / 20)
        a = int(lay.get("at", 0) * SR)
        if a < n:
            seg = y[: n - a]
            x[a : a + len(seg)] += seg
    if rec.get("hochpass_hz"):
        b, a = signal.butter(2, rec["hochpass_hz"] / (SR / 2), "high")
        x = signal.lfilter(b, a, x)
    if rec.get("tiefpass_hz"):
        b, a = signal.butter(2, rec["tiefpass_hz"] / (SR / 2), "low")
        x = signal.lfilter(b, a, x)
    fade = int(rec.get("fade_s", 0.05) * SR)
    x[-fade:] *= np.linspace(1, 0, fade) ** 2
    x *= 10 ** (rec.get("spitze_db", -1.0) / 20) / max(np.abs(x).max(), 1e-9)
    return x


def build(only: list[str]) -> None:
    cfg = json.loads(RECIPES.read_text())
    OUT.mkdir(parents=True, exist_ok=True)
    man_path = OUT / "manifest.json"
    manifest = json.loads(man_path.read_text()) if man_path.exists() else {}
    manifest["_hinweis"] = "Erzeugt von tools/audio/build_sfx.py aus tools/audio/sfx_recipes.json – nicht von Hand ändern."
    manifest["samplerate"] = SR
    klaenge = manifest.setdefault("klaenge", {})
    # Klänge, die es im Rezept nicht mehr gibt, verschwinden samt Dateien
    for k in list(klaenge):
        if k not in cfg["klaenge"]:
            for f in klaenge.pop(k)["dateien"]:
                (OUT / f).unlink(missing_ok=True)
    for name, rec in cfg["klaenge"].items():
        if only and name not in only:
            continue
        src = cfg["quellen"][rec["quelle"]]
        z = archive(rec["quelle"], src)
        for old in OUT.glob(f"{name}_*.wav"):
            old.unlink()
        files = []
        for i, layers in enumerate(variants(rec, z)):
            x = render(rec, layers, z)
            f = f"{name}_{i + 1}.wav"
            sf.write(OUT / f, x.astype(np.float32), SR, subtype="PCM_16")
            files.append(f)
        klaenge[name] = {
            "zweck": rec.get("zweck", ""),
            "dateien": files,
            "quelle": rec["quelle"],
        }
        print(name, len(files))
    used = {k["quelle"] for k in klaenge.values()}
    manifest["quellen"] = {
        k: {f: v[f] for f in ("titel", "seite", "lizenz", "urheber")}
        for k, v in cfg["quellen"].items()
        if k in used
    }
    manifest["klaenge"] = dict(sorted(klaenge.items()))
    man_path.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    build(sys.argv[1:])
