#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) ibis inc.
# Additional terms under AGPL-3.0 section 7(b): see NOTICE.
# Build ibisOffice for Linux as an AppImage, a single file that runs on any
# Linux distribution (the desktop integration it carries inside: the menu
# entry with its icon, the documents it opens).
#
#   scripts/bundle-linux.sh                # build into target/dist/AppDir
#   scripts/bundle-linux.sh --appimage     # ... and pack it with appimagetool
#
# Needs appimagetool (https://github.com/AppImage/AppImageKit/releases, or
# AppImage's apt repository) for --appimage; without it the AppDir is still
# written and can be packed later, or run as it is (AppRun).
set -euo pipefail
cd "$(dirname "$0")/.."

APPIMAGE=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --appimage) APPIMAGE=1; shift ;;
    -h|--help) sed -n '3,15p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

# The version shown for Linux, from version.txt (a platform, a tab and its
# version, per line).
VERSION=$(awk -F'\t' '$1 == "Linux" { sub(/\r$/, "", $2); print $2 }' version.txt)
[[ -n $VERSION ]] || { echo "version.txt has no Linux line" >&2; exit 1; }
ARCH=$(uname -m)

# The paths of this machine (the home folder, the repository) are not to go
# into the executable (Rust keeps source paths for panics and the like): they
# are rewritten to short relative ones, in a target directory of its own so
# that the everyday builds (target/release) are not rebuilt by the flags.
REMAP="--remap-path-prefix=$HOME/.cargo/registry/src=crates.io --remap-path-prefix=$HOME/.cargo/git/checkouts=git --remap-path-prefix=$HOME/.rustup=rustup --remap-path-prefix=$(pwd)=."
echo "Building ibisOffice $VERSION for Linux ($ARCH)..."
RUSTFLAGS="$REMAP" CARGO_TARGET_DIR=target/dist cargo build --release --locked -p ibis-office-app -p ibis-office-updater

APPSRC=crates/ibis-office-app
APPDIR=target/dist/AppDir
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/metainfo" \
  "$APPDIR/usr/share/mime/packages" "$APPDIR/usr/share/doc/ibis-office"
cp target/dist/release/ibisOffice "$APPDIR/usr/bin/ibisOffice"
# The automatic update's helper. An AppImage is mounted read-only, so the
# update cannot swap the copy inside it; it is kept here for the updater to
# fall back on, and for a manual unpack (see the README).
[[ -f target/dist/release/ibisOfficeUpdater ]] && cp target/dist/release/ibisOfficeUpdater "$APPDIR/usr/bin/"

# hicolor, as the desktop looks the icons up (one directory per size).
mkdir -p "$APPDIR/usr/share/icons"
cp -r "$APPSRC/linux/hicolor" "$APPDIR/usr/share/icons/hicolor"
cp "$APPSRC/linux/jp.ne.ibis.office.desktop" "$APPDIR/usr/share/applications/"
cp "$APPSRC/linux/jp.ne.ibis.office.metainfo.xml" "$APPDIR/usr/share/metainfo/"
cp "$APPSRC/linux/jp.ne.ibis.office.xml" "$APPDIR/usr/share/mime/packages/"
# Its licence and the third parties' notices (made afresh from the
# dependencies, as scripts/bundle-macos.sh does for the macOS app).
for notice in LICENSE NOTICE THIRD-PARTY-NOTICES.md COMMERCIAL-LICENSE.md CHANGELOG.md; do
  cp "$notice" "$APPDIR/usr/share/doc/ibis-office/"
done
python3 scripts/gen-web-licenses.py linux
cp "$APPSRC/linux/license.md" "$APPDIR/usr/share/doc/ibis-office/"
cp "$APPSRC/fonts/carlito/OFL.txt" "$APPDIR/usr/share/doc/ibis-office/Carlito-OFL.txt"

# AppRun runs the app inside the image (the AppImage spec; no shell needed
# for it, a line of sh is enough).
cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
# The AppRun of ibisOffice: runs the app from inside the mounted image.
HERE=$(dirname "$(readlink -f "$0")")
exec "$HERE/usr/bin/ibisOffice" "$@"
EOF
chmod +x "$APPDIR/AppRun"
# The desktop entry at the root of the AppDir too, and the icon beside it
# under the name its Icon= key says, and as .DirIcon: that is where the
# AppImage spec (and appimagetool) look for them.
cp "$APPSRC/linux/jp.ne.ibis.office.desktop" "$APPDIR/"
cp "$APPSRC/linux/hicolor/256x256/apps/jp.ne.ibis.office.png" "$APPDIR/jp.ne.ibis.office.png"
cp "$APPSRC/linux/hicolor/256x256/apps/jp.ne.ibis.office.png" "$APPDIR/.DirIcon"

if [[ $APPIMAGE == 1 ]]; then
  command -v appimagetool >/dev/null || { echo "appimagetool is not installed" >&2; exit 1; }
  # The file's name: the version and the architecture. The desktop entry's
  # name (jp.ne.ibis.office) is what the desktop looks the AppImage up by.
  OUT=target/dist/ibisOffice-$VERSION-$ARCH.AppImage
  appimagetool "$APPDIR" "$OUT"
  echo "Built $OUT"
else
  echo "Wrote $APPDIR. Run it with: $APPDIR/AppRun"
  echo "Pack it with: appimagetool $APPDIR target/dist/ibisOffice-$VERSION-$ARCH.AppImage"
fi
