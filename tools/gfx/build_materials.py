#!/usr/bin/env python3
"""Bodenmaterialien der nativen Fassung aus CC0-Quellen (ambientCG) bauen.

    python3 tools/gfx/build_materials.py            # lädt fehlende Pakete, baut alles neu
    python3 tools/gfx/build_materials.py --offline  # nur aus dem Zwischenspeicher (tools/gfx/.cache)

Je Material entstehen zwei 512²-PNG unter data/gfx/materials/ (genau 2:1 aus den 1K-Texturen verkleinert, damit
die Kachel nahtlos bleibt):
  <name>_detail.png  – Farbbild, je Kanal durch eine periodisch weichgezeichnete Fassung geteilt (Hochpass: großflächige
                       Flecken der Vorlage fallen weg, sie wären bei jeder Kachel an derselben Stelle) und auf 0,5
                       gelegt (sRGB). Der Shader multipliziert es mit der Kartenfarbe aus mesh.rs: die Textur bringt
                       Struktur, OSM die Farbe; großräumige Schwankung kommt aus dem Rauschen im Shader.
  <name>_nr.png      – R, G = Normale (OpenGL-Konvention, G nach oben), B = Rauheit, A = Umgebungsverdeckung.
Dazu data/gfx/materials/manifest.json mit Quelle, Lizenz, Urheber, Größe der Vorlage und SHA-256 des Pakets. Nicht
von Hand ändern – Skript erneut laufen lassen. Welche Fläche welches Material nutzt und wie groß eine Kachel in
Metern ist, steht in data/gfx/material_map.json.
"""
import hashlib
import io
import json
import sys
import urllib.request
import zipfile
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "data" / "gfx" / "materials"
CACHE = Path(__file__).resolve().parent / ".cache"
SIZE = 512

# Name im Spiel, ambientCG-Kennung, Zweck, Radius des Hochpasses in Bildpunkten (512er-Bild): klein bei
# gleichförmigen Belägen (Asphalt: nur Körnung), groß, wo Steine und Platten ihre eigene Schattierung behalten sollen. Erwartete SHA-256 der Pakete stehen im Manifest; weicht ein neuer
# Download ab (Quelle geändert), bricht das Skript ab, statt still andere Bilder einzubauen.
MATERIALS = [
    ("asphalt", "Asphalt025C", "Fahrbahn (Asphalt)", 6),
    ("kopfstein", "PavingStones046", "Kopfsteinpflaster (Granit)", 48),
    ("platten", "Concrete010", "Gehwegplatten, Plätze, Brückenflächen", 48),
    ("gras", "Grass001", "Rasen, Parks, Friedhöfe, Kleingärten, Gründächer", 16),
    ("schotter", "Gravel043", "Gleisbett, Sand, unbefestigte Wege", 24),
]


def fetch(asset: str, offline: bool) -> bytes:
    CACHE.mkdir(exist_ok=True)
    path = CACHE / f"{asset}_1K-JPG.zip"
    if not path.exists():
        if offline:
            sys.exit(f"{path} fehlt (ohne --offline laden)")
        url = f"https://ambientcg.com/get?file={asset}_1K-JPG.zip"
        print(f"lade {url}")
        req = urllib.request.Request(url, headers={"User-Agent": "gta-berlin-build/1.0"})
        with urllib.request.urlopen(req, timeout=120) as r:
            path.write_bytes(r.read())
    return path.read_bytes()


def info(asset: str, offline: bool) -> dict:
    path = CACHE / f"{asset}.json"
    if not path.exists() and not offline:
        url = f"https://ambientcg.com/api/v2/full_json?id={asset}&include=dimensionsData"
        req = urllib.request.Request(url, headers={"User-Agent": "gta-berlin-build/1.0"})
        with urllib.request.urlopen(req, timeout=60) as r:
            path.write_bytes(r.read())
    if not path.exists():
        return {}
    found = json.loads(path.read_text()).get("foundAssets", [])
    return found[0] if found else {}


def image(z: zipfile.ZipFile, suffix: str, mode: str) -> Image.Image:
    name = next(n for n in z.namelist() if n.endswith(suffix))
    img = Image.open(io.BytesIO(z.read(name))).convert(mode)
    # 1K → 512: genau halbieren (Kastenfilter), die Kachel bleibt nahtlos
    assert img.size == (1024, 1024), f"{name}: {img.size}"
    return img.reduce(2)


def periodic_blur(img: np.ndarray, sigma: float) -> np.ndarray:
    """Gauß-Weichzeichner mit Umlauf (über die FFT): die Kachel bleibt nahtlos."""
    h, w = img.shape[:2]
    fy = np.fft.fftfreq(h)[:, None]
    fx = np.fft.fftfreq(w)[None, :]
    kernel = np.exp(-2 * (np.pi * sigma) ** 2 * (fx * fx + fy * fy))
    out = np.empty_like(img)
    for c in range(img.shape[2]):
        out[..., c] = np.real(np.fft.ifft2(np.fft.fft2(img[..., c]) * kernel))
    return out


def build(name: str, asset: str, sigma: float, offline: bool, previous: dict) -> dict:
    data = fetch(asset, offline)
    sha = hashlib.sha256(data).hexdigest()
    old = previous.get(name, {}).get("sha256")
    if old and old != sha and "--neu" not in sys.argv:
        sys.exit(f"{asset}: Paket hat sich geändert ({old[:12]} → {sha[:12]}); mit --neu bewusst übernehmen")
    z = zipfile.ZipFile(io.BytesIO(data))
    color = np.asarray(image(z, "_Color.jpg", "RGB"), dtype=np.float64)
    mean = color.reshape(-1, 3).mean(axis=0)
    # Hochpass: Struktur bis etwa `sigma` Bildpunkte bleibt, alles Gröbere wird zur mittleren Farbe
    low = np.maximum(periodic_blur(color, sigma), 1.0)
    detail = np.clip(color / low * 0.5, 0, 1)
    Image.fromarray((detail * 255 + 0.5).astype(np.uint8), "RGB").save(OUT / f"{name}_detail.png", optimize=True)
    normal = np.asarray(image(z, "_NormalGL.jpg", "RGB"), dtype=np.uint8)
    rough = np.asarray(image(z, "_Roughness.jpg", "L"), dtype=np.uint8)
    try:
        ao = np.asarray(image(z, "_AmbientOcclusion.jpg", "L"), dtype=np.uint8)
    except StopIteration:
        ao = np.full(rough.shape, 255, np.uint8)
    nr = np.dstack([normal[..., 0], normal[..., 1], rough, ao])
    Image.fromarray(nr, "RGBA").save(OUT / f"{name}_nr.png", optimize=True)
    meta = info(asset, offline)
    return {
        "quelle": asset,
        "seite": f"https://ambientcg.com/view?id={asset}",
        "urheber": "ambientCG (Lennart Demes)",
        "lizenz": "CC0 1.0",
        "vorlage_cm": [meta.get("dimensionX", 0), meta.get("dimensionY", 0)],
        "mittel_srgb": [round(float(c), 2) for c in mean],
        "hochpass_px": sigma,
        "sha256": sha,
    }


def main() -> None:
    offline = "--offline" in sys.argv
    OUT.mkdir(parents=True, exist_ok=True)
    man_path = OUT / "manifest.json"
    previous = json.loads(man_path.read_text()).get("materialien", {}) if man_path.exists() else {}
    out = {}
    for name, asset, purpose, sigma in MATERIALS:
        out[name] = {"zweck": purpose, **build(name, asset, sigma, offline, previous)}
        print(f"{name}: {asset}")
    manifest = {
        "_hinweis": "Erzeugt von tools/gfx/build_materials.py – nicht von Hand ändern.",
        "groesse": SIZE,
        "materialien": out,
    }
    man_path.write_text(json.dumps(manifest, indent=1, ensure_ascii=False) + "\n")
    print(f"{len(out)} Materialien → {OUT}")


if __name__ == "__main__":
    main()
