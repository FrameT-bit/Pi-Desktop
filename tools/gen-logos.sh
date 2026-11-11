#!/usr/bin/env bash
# Generate pixelated provider logos for the header and model list.
#
# Source: resources/logos/src/<slug>.{svg,png,ico} (Simple Icons MIT +
# official favicons/marks, see src/README.md).
# Output: resources/logos/<slug>-dark.png  (ink #ebe7e4, dark chrome)
#         resources/logos/<slug>-light.png (ink #252f3d, light chrome)
#
# A fixed 2px block keeps the artwork on a pixel grid: grid 12 = 24px, the
# exact slot used by the model list. Providers whose glyph loses its shape at
# that size use a larger grid; the chosen grid is documented in the README.
#
# Usage: tools/gen-logos.sh
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
src="$root/resources/logos/src"
out="$root/resources/logos"
tmp="${TMPDIR:-/tmp}/pi-desktop-logos"
mkdir -p "$tmp"

providers=(
  anthropic deepseek googlegemini groq meta mistralai nvidia ollama openai
  openrouter xai cerebras fireworks together baseten moonshotai zai minimax
  qwen opencode vercel huggingface githubcopilot antling xiaomi radius
)

# Grid per provider, always with a 2px block. Grid 12 renders at 24px; 14 at
# 28px (same as Groq); 10 at 20px (fine details).
grid_for() {
  case "$1" in
    groq) echo 14 ;;          # diagonal bolt; below this the step disappears
    openai) echo 12 ;;        # spiral knot closes into a square at 10
    meta) echo 10 ;;          # infinity mark becomes two diamonds at 8
    googlegemini) echo 10 ;;  # four-point star
    nvidia) echo 10 ;;        # eye + tail
    deepseek) echo 10 ;;      # whale from the official favicon
    cerebras) echo 14 ;;      # concentric C needs the extra rows
    githubcopilot) echo 14 ;; # face details disappear at 12
    huggingface) echo 14 ;;   # eyes and mouth close at 12
    qwen) echo 14 ;;          # knot facets need the extra rows
    minimax) echo 14 ;;       # waveform gaps close at 12
    moonshotai) echo 14 ;;    # moon stripes
    openai) echo 14 ;;        # interlocking knot
    openrouter) echo 14 ;;    # arrow gaps
    xiaomi) echo 14 ;;        # "mi" holes survive at 14
    vercel) echo 10 ;;        # plain triangle, no detail to preserve
    *) echo 12 ;;
  esac
}

sampling_for() {
  case "$1" in
    huggingface|qwen|minimax|openai|openrouter|xiaomi)
      echo nearest ;;
    *) echo average ;;
  esac
}

source_for() {
  case "$1" in
    huggingface|xiaomi|minimax|openai)
      echo "$src/$1-mask.png"; return 0 ;;
    qwen) echo "$src/qwen.png"; return 0 ;;
  esac
  for ext in svg png ico; do
    if [ -f "$src/$1.$ext" ]; then
      echo "$src/$1.$ext"
      return 0
    fi
  done
  return 1
}

# Flat favicons need their background removed before tinting, or the tint
# fills the whole square. Prepared masks live next to the sources.
prepared_input() {
  case "$1" in
    groq)
      convert "$src/groq.ico[2]" -alpha off -colorspace gray -threshold 85% "$tmp/groq-mask.png"
      convert -size 48x48 xc:white "$tmp/groq-mask.png" -alpha off \
        -compose CopyOpacity -composite "$tmp/groq.png"
      echo "$tmp/groq.png" ;;
    deepseek)
      convert "$src/deepseek.ico" -fuzz 12% -transparent white "$tmp/deepseek.png"
      echo "$tmp/deepseek.png" ;;
    *)
      source_for "$1" ;;
  esac
}

for provider in "${providers[@]}"; do
  if ! input="$(prepared_input "$provider")"; then
    echo "warning: missing resources/logos/src/$provider.{svg,png,ico}" >&2
    continue
  fi
  grid="$(grid_for "$provider")"
  python3 "$root/tools/pixelify.py" "$input" \
    -o "$out/$provider-dark.png" --block 2 --grid "$grid" --tint '#ebe7e4'
  python3 "$root/tools/pixelify.py" "$input" \
    -o "$out/$provider-light.png" --block 2 --grid "$grid" --tint '#252f3d'
done

echo "logos written to $out"
