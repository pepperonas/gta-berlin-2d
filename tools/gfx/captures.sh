#!/usr/bin/env bash
# Testszenen der Grafik-Überarbeitung: reproduzierbare Aufnahmen (2560 × 1440, abseits des Fensters gezeichnet) und
# Bildzeiten (CPU-Arbeit, GPU per Zeitstempel; Median und P95) je Szene, Zoom und Grafikmodus.
#
#   tools/gfx/captures.sh PHASE                 # z. B. 00-basis
#   MODI="hd hd:niedrig pixel" tools/gfx/captures.sh 01-architektur   # Modus[:Qualität]
#   SZENEN="block boulevard" ZOOMS=1.2 tools/gfx/captures.sh 02-boden
#
# Ergebnis: docs/images/native/grafik/PHASE/<szene>-z<zoom>-<modus>.webp (eingecheckt, WebP q85) und
# …/png/ (verlustfrei, nicht eingecheckt) sowie die Messwerte in docs/images/native/grafik/metrics.json
# (Schlüssel Phase → Modus → Szene → Zoom). Modus „basis“ = Stand vor der Überarbeitung (ohne --grafik); mit
# Qualität heißt der Modus z. B. „hd_niedrig“. Die Läufe rücken genau einen Simulationsschritt je Bild vor – gleiche
# Szene, gleiches Bild; Unterschiede zwischen Phasen sind damit reine Darstellung.
set -euo pipefail
export LC_ALL=C
cd "$(dirname "$0")/../.."
phase=${1:?Phase fehlt, z. B. 00-basis}
modi=${MODI:-basis}
zooms=${ZOOMS:-"1.2 2.6"}
frames=${FRAMES:-360}
out="docs/images/native/grafik/$phase"
mkdir -p "$out/png"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

cargo build --release -q
bin=./target/release/gta-berlin

# Name | Argumente (fester Seed, feste Uhr, festes Wetter)
scenes=(
  "block|--uhr 12:30 --wetter clear --geo 52.5010 13.4190"
  "boulevard|--uhr 19:30 --wetter clear --geo 52.5030 13.3270"
  "park|--uhr 13:00 --wetter clear --geo 52.5098 13.3445"
  "spree|--uhr 21:00 --wetter clear --geo 52.5015 13.4455"
  "regennacht|--uhr 23:30 --wetter heavyrain --geo 52.4898 13.4300"
  "schnee|--uhr 10:00 --wetter snow --geo 52.5362 13.4175"
  "bahnhof|--uhr 13:00 --wetter clear --geo 52.4869 13.4247 --bildschirm bahnhof"
  "autos|--uhr 13:00 --wetter clear --geo 52.4730 13.4030 --bildschirm autos"
  "leute|--uhr 13:00 --wetter clear --bildschirm leute"
)
want=${SZENEN:-}
for entry in "${scenes[@]}"; do
  name=${entry%%|*}
  args=${entry#*|}
  if [[ -n "$want" && " $want " != *" $name "* ]]; then continue; fi
  for zoom in $zooms; do
    for spec in $modi; do
      mode=${spec%%:*}
      extra=()
      [[ "$mode" != basis ]] && extra=(--grafik "$mode")
      tagmode=$mode
      if [[ "$spec" == *:* ]]; then
        extra+=(--qualitaet "${spec#*:}")
        tagmode="${mode}_${spec#*:}"
      fi
      tag="$name-z$zoom-$tagmode"
      echo "== $tag"
      # shellcheck disable=SC2086
      "$bin" --new --stumm --seed 7 --bars aus $args --zoom "$zoom" --fenster 2560x1440 \
        --smoke-frames "$frames" --messung "$tmp/$tag.json" --capture "$out/png/$tag.png" \
        "${extra[@]}" 2>&1 | grep -E "Messung|Aufnahme|Fehler|Error" || true
      [[ -f "$out/png/$tag.png" ]] || { echo "Keine Aufnahme: $tag" >&2; exit 1; }
      cwebp -quiet -q 85 "$out/png/$tag.png" -o "$out/$tag.webp"
    done
  done
done

python3 - "$phase" "$tmp" <<'EOF'
import json, os, sys
phase, tmp = sys.argv[1], sys.argv[2]
path = "docs/images/native/grafik/metrics.json"
data = json.load(open(path)) if os.path.exists(path) else {}
for f in sorted(os.listdir(tmp)):
    if not f.endswith(".json"):
        continue
    scene, zoom, mode = f[:-5].rsplit("-", 2)
    data.setdefault(phase, {}).setdefault(mode, {}).setdefault(scene, {})[zoom.lstrip("z")] = json.load(open(os.path.join(tmp, f)))
with open(path, "w") as fh:
    json.dump(data, fh, indent=1, ensure_ascii=False, sort_keys=True)
    fh.write("\n")
print("Messwerte:", path)
EOF
