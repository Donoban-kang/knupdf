#!/usr/bin/env bash
# Build the KnuPdf Windows x64 installer and portable zip on Linux (Ubuntu 24.04 tested).
#
#   packaging/windows-knupdf/build-linux.sh [OUT_DIR]      # default OUT_DIR: dist/windows
#
# Needs: rustup target x86_64-pc-windows-gnu, mingw-w64 (gcc, windres, strip), NSIS (makensis),
# curl and network access for the OCR models.
#   sudo apt-get install -y gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64 nsis
#   rustup target add x86_64-pc-windows-gnu
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="${1:-$ROOT/dist/windows}"
TARGET=x86_64-pc-windows-gnu
VERSION="$(sed -n '/^\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"/\1/p}' "$ROOT/Cargo.toml" | head -1)"
STAGE="$ROOT/target/knupdf-windows-stage"
PORTABLE_NAME="KnuPdf-$VERSION-windows-x64-portable"

echo "==> KnuPdf $VERSION for Windows x64"
cd "$ROOT"
# Static C runtime: the exe needs only DLLs that ship with Windows 10/11.
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C target-feature=+crt-static" \
PDFCRAFT_REQUIRE_WINRES=1 \
  cargo build --release -p pdfcraft --target "$TARGET"

rm -rf "$STAGE" && mkdir -p "$STAGE/models" "$OUT"
cp "target/$TARGET/release/pdfcraft.exe" "$STAGE/KnuPdf.exe"
x86_64-w64-mingw32-strip "$STAGE/KnuPdf.exe"
cp assets/app-icon-knupdf/knupdf.ico "$STAGE/KnuPdf.ico"
cp assets/app-icon-knupdf/LICENSE.txt "$STAGE/LICENSE-icon.txt"
cp LICENSE-MIT LICENSE-APACHE NOTICE assets/fonts/OFL-KnupdfHangul.txt \
   packaging/windows-knupdf/LICENSE-KnuPdf.txt "$STAGE/"

# OCR models (ATTRIBUTION.toml [[fetched]] kind = "model"), checked against their SHA-256.
fetch_model() {
  local file="$1" sha="$2"
  curl -sSfL -o "$STAGE/models/$file" "https://ocrs-models.s3-accelerate.amazonaws.com/$file"
  echo "$sha  $STAGE/models/$file" | sha256sum -c -
}
fetch_model text-detection.rten f15cfb56bd02c4bf478a20343986504a1f01e1665c2b3a0ad66340f054b1b5ca
fetch_model text-recognition.rten e484866d4cce403175bd8d00b128feb08ab42e208de30e42cd9889d8f1735a6e
cat > "$STAGE/models/ATTRIBUTION.txt" <<'EOF'
KnuPdf OCR models

text-detection.rten   Ocrs text detection model
text-recognition.rten Ocrs text recognition model

Author: Robert Knight (ocrs-models; trained on HierText)
Source: https://github.com/robertknight/ocrs-models  (model card: https://huggingface.co/robertknight/ocrs)
Licence: Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0)
         https://creativecommons.org/licenses/by-sa/4.0/legalcode
The files are unmodified.
EOF

echo "==> installer"
makensis -V2 -DVERSION="$VERSION" -DSTAGE="$STAGE" \
  -DOUT="$OUT/KnuPdf-$VERSION-windows-x64-setup.exe" packaging/windows-knupdf/knupdf.nsi

echo "==> portable zip"
PORT="$ROOT/target/$PORTABLE_NAME"
rm -rf "$PORT" && mkdir -p "$PORT" && cp -r "$STAGE"/. "$PORT"/ && rm "$PORT/KnuPdf.ico"
printf 'KnuPdf portable mode.\r\n이 파일이 KnuPdf.exe 옆에 있으면 설정을 같은 폴더의 PdfCraftData에 저장합니다(휴대용 모드).\r\n' > "$PORT/portable.txt"
rm -f "$OUT/$PORTABLE_NAME.zip"
(cd "$ROOT/target" && python3 -c "import shutil,sys; shutil.make_archive(sys.argv[1],'zip','.',sys.argv[2])" "$OUT/$PORTABLE_NAME" "$PORTABLE_NAME")

cd "$OUT" && sha256sum "KnuPdf-$VERSION-windows-x64-setup.exe" "$PORTABLE_NAME.zip"
