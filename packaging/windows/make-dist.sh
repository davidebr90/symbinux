#!/usr/bin/env bash
# Assemble a self-contained, double-clickable Windows folder for the GUI:
#
#   dist/windows/Symbinux/
#     bin/symbinux-gui.exe + the MSYS2 GTK4 DLL closure
#     lib/gdk-pixbuf-2.0/          (image loaders + cache)
#     share/glib-2.0/schemas/      (compiled GSettings schemas)
#     share/icons/                 (Adwaita symbolic icons + hicolor index)
#     assets/logo/                 (theme-aware wordmark logos)
#     po/                          (runtime translations)
#
# Run from an MSYS2 MINGW64 shell at the repository root, after
# packaging/windows/build-gui.bat. The GUI itself defaults GSK_RENDERER to
# cairo on Windows, so no launcher script or environment setup is needed.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
MINGW="${MINGW_PREFIX:-/mingw64}"
EXE="$ROOT/target/x86_64-pc-windows-gnu/release/symbinux-gui.exe"
DIST="$ROOT/dist/windows/Symbinux"

if [[ ! -f "$EXE" ]]; then
    echo "error: $EXE not found — run packaging/windows/build-gui.bat first" >&2
    exit 1
fi

rm -rf "$DIST"
mkdir -p "$DIST/bin"

cp "$EXE" "$DIST/bin/"

# The transitive mingw64 DLL closure. ldd resolves through PATH, which the
# MINGW64 shell already points at /mingw64/bin.
ldd "$EXE" | awk '$3 ~ /\/mingw64\// { print $3 }' | sort -u | while read -r dll; do
    cp "$dll" "$DIST/bin/"
done

# gdk-pixbuf image loaders (the logo PNGs need them) and their closure.
mkdir -p "$DIST/lib"
cp -r "$MINGW/lib/gdk-pixbuf-2.0" "$DIST/lib/"
for loader in "$DIST"/lib/gdk-pixbuf-2.0/*/loaders/*.dll; do
    ldd "$loader" | awk '$3 ~ /\/mingw64\// { print $3 }'
done | sort -u | while read -r dll; do
    cp -n "$dll" "$DIST/bin/" 2>/dev/null || true
done

# Compiled GSettings schemas (GTK aborts without them).
mkdir -p "$DIST/share/glib-2.0/schemas"
cp "$MINGW/share/glib-2.0/schemas/gschemas.compiled" "$DIST/share/glib-2.0/schemas/"

# Icon themes: GTK's symbolic button icons come from Adwaita.
mkdir -p "$DIST/share/icons"
cp -r "$MINGW/share/icons/Adwaita" "$DIST/share/icons/"
cp -r "$MINGW/share/icons/hicolor" "$DIST/share/icons/"

# Application data resolved relative to the executable.
mkdir -p "$DIST/assets/logo"
cp "$ROOT"/assets/logo/symbinux_logo_transparent_*.png "$DIST/assets/logo/"
mkdir -p "$DIST/po"
cp "$ROOT"/po/*.po "$DIST/po/"

echo "dist ready: $DIST"
du -sh "$DIST"
