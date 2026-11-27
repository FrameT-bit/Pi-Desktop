#!/usr/bin/env bash
# Rebuild the twelve theme/tone/scale variants from vendored masks and SVGs.
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
python3 "$root/tools/gen-icons.py" "$@"
