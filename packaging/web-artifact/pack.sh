#!/usr/bin/env bash
# Package the trunk web build (dist/web) for a claude.ai Artifact or any host with a ~15 MB
# per-file limit: the wasm engine is gzipped and split into part0.wasm, part1.wasm, which
# knupdf.html downloads, joins and unzips (DecompressionStream) before starting the app.
#
#   (cd apps/pdfcraft-web && trunk build --release)
#   packaging/web-artifact/pack.sh [OUT_DIR]     # default OUT_DIR: dist/web-artifact
#
# Publish OUT_DIR/knupdf.html as the page, with pdfcraft.js, part*.wasm and knupdf.svg beside it.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="${1:-$ROOT/dist/web-artifact}"
WEB="$ROOT/dist/web"
rm -rf "$OUT" && mkdir -p "$OUT"
cp "$WEB"/pdfcraft-web-*.js "$OUT/pdfcraft.js"
gzip -9 -c "$WEB"/pdfcraft-web-*_bg.wasm > "$OUT/app.wasm.gz"
TOTAL=$(stat -c%s "$OUT/app.wasm.gz")
(cd "$OUT" && split -b 7500000 -d -a 1 app.wasm.gz part && rm app.wasm.gz && for f in part?; do mv "$f" "$f.wasm"; done)
PARTS=$(cd "$OUT" && printf "'./%s'\n" part*.wasm | paste -sd, -)
sed -e "s/const TOTAL = [0-9]*;/const TOTAL = $TOTAL;/" \
    -e "s#const parts = \[[^]]*\];#const parts = [$PARTS];#" \
    "$ROOT/packaging/web-artifact/knupdf.html" > "$OUT/knupdf.html"
cp "$ROOT/assets/app-icon-knupdf/knupdf.svg" "$OUT/"
ls -la "$OUT"
