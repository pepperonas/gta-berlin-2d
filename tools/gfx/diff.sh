#!/usr/bin/env bash
# Bildvergleich zweier Aufnahmen gleicher Größe (ImageMagick): Anteil abweichender Bildpunkte (Toleranz 2 %) und
# mittlere absolute Abweichung; optional ein Differenzbild (abweichende Punkte rot über dem abgedunkelten Original).
#
#   tools/gfx/diff.sh vorher.png nachher.png [differenz.png]
#   Ausgabe: <Anteil %> <mittlere Abweichung 0…1>
set -euo pipefail
export LC_ALL=C
a=${1:?Bild A fehlt}
b=${2:?Bild B fehlt}
out=${3:-}
total=$(magick identify -format '%[fx:w*h]' "$a")
# compare schreibt die Kennzahl nach stderr und endet bei Unterschieden mit 1
ae=$(magick compare -metric AE -fuzz 2% "$a" "$b" null: 2>&1 >/dev/null | sed -E 's/.*\(([0-9.e+-]+)\).*/\1/' || true)
mae=$(magick compare -metric MAE "$a" "$b" null: 2>&1 >/dev/null | sed -E 's/.*\(([0-9.e+-]+)\).*/\1/' || true)
pct=$(awk -v n="$ae" -v t="$total" 'BEGIN { printf "%.3f", 100 * n / t }')
echo "$pct $mae"
if [[ -n "$out" ]]; then
  magick compare -fuzz 2% -highlight-color red -lowlight-color '#00000000' "$a" "$b" /tmp/.gfxdiff-$$.png 2>/dev/null || true
  magick "$a" -modulate 60 /tmp/.gfxdiff-$$.png -composite "$out"
  rm -f /tmp/.gfxdiff-$$.png
fi
