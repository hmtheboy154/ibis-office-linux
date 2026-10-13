#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) ibis inc.
# Additional terms under AGPL-3.0 section 7(b): see NOTICE.
# Build ibisOffice for Linux and install it, so the desktop knows it: a menu
# entry with its icon, the documents it opens, and an `ibisOffice` command.
#
#   scripts/install-linux.sh                      # into /usr/local (sudo)
#   scripts/install-linux.sh --prefix ~/.local   # into your own home, no sudo
#   scripts/install-linux.sh --bin ~/ibisOffice   # install a build made elsewhere
#
# What it installs, under the prefix (default /usr/local):
#
#   bin/ibisOffice                     the app (UI icons and fonts are in it)
#   bin/ibisOfficeUpdater              the automatic update's helper
#   share/applications/jp.ne.ibis.office.desktop
#   share/icons/hicolor/*/apps/jp.ne.ibis.office.{png,svg}
#   share/mime/packages/jp.ne.ibis.office.xml     the document types
#   share/metainfo/jp.ne.ibis.office.metainfo.xml
#   share/doc/ibis-office/             its licence and the third parties' notices
#
# The desktop entry names the app id the windows carry (shell/platform.rs),
# which is how the taskbar shows the right icon and the windows are grouped.
set -euo pipefail
cd "$(dirname "$0")/.."

PREFIX=${PREFIX:-/usr/local}
BIN=
while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix) PREFIX=$2; shift 2 ;;
    --prefix=*) PREFIX=${1#*=}; shift ;;
    --bin) BIN=$2; shift 2 ;;
    --bin=*) BIN=${1#*=}; shift ;;
    -h|--help) sed -n '3,24p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

# The version shown for Linux, from version.txt (a platform, a tab and its
# version, per line).
VERSION=$(awk -F'\t' '$1 == "Linux" { sub(/\r$/, "", $2); print $2 }' version.txt)
[[ -n $VERSION ]] || { echo "version.txt has no Linux line" >&2; exit 1; }

if [[ -z $BIN ]]; then
  # The paths of this machine (the home folder, the repository) are not to go
  # into the executables (Rust keeps source paths for panics and the like):
  # they are rewritten to short relative ones, in a target directory of its own
  # so that the everyday builds (target/release) are not rebuilt by the flags.
  echo "Building ibisOffice $VERSION for Linux (this takes a while)..."
  REMAP="--remap-path-prefix=$HOME/.cargo/registry/src=crates.io --remap-path-prefix=$HOME/.cargo/git/checkouts=git --remap-path-prefix=$HOME/.rustup=rustup --remap-path-prefix=$(pwd)=."
  RUSTFLAGS="$REMAP" CARGO_TARGET_DIR=target/dist cargo build --release --locked -p ibis-office-app -p ibis-office-updater
  BIN=target/dist/release/ibisOffice
fi
[[ -x $BIN ]] || { echo "no executable at $BIN" >&2; exit 1; }

# sudo only where the prefix is not this user's to write in (walking up to the
# first directory that exists, since the ones being made are not there yet).
writable() {
  local dir=$1
  while [[ ! -e $dir ]]; do dir=$(dirname "$dir"); done
  [[ -w $dir ]]
}
run() {
  if writable "$PREFIX"; then "$@"; else sudo "$@"; fi
}

APP=crates/ibis-office-app
DOC=$PREFIX/share/doc/ibis-office

echo "Installing ibisOffice $VERSION into $PREFIX"
run mkdir -p "$PREFIX/bin" "$PREFIX/share/applications" "$PREFIX/share/metainfo" \
  "$PREFIX/share/mime/packages" "$DOC" "$PREFIX/share/icons/hicolor"
run install -m 755 "$BIN" "$PREFIX/bin/ibisOffice"
# The automatic update starts this one to swap the app for a new version.
if [[ -f target/dist/release/ibisOfficeUpdater ]]; then
  run install -m 755 target/dist/release/ibisOfficeUpdater "$PREFIX/bin/ibisOfficeUpdater"
fi

run install -m 644 "$APP/linux/jp.ne.ibis.office.desktop" "$PREFIX/share/applications/jp.ne.ibis.office.desktop"
run install -m 644 "$APP/linux/jp.ne.ibis.office.metainfo.xml" "$PREFIX/share/metainfo/jp.ne.ibis.office.metainfo.xml"
run install -m 644 "$APP/linux/jp.ne.ibis.office.xml" "$PREFIX/share/mime/packages/jp.ne.ibis.office.xml"
# hicolor, as the desktop looks the icons up (one directory per size).
for icon in "$APP"/linux/hicolor/*/apps/*; do
  size=$(basename "$(dirname "$(dirname "$icon")")")
  run mkdir -p "$PREFIX/share/icons/hicolor/$size/apps"
  run install -m 644 "$icon" "$PREFIX/share/icons/hicolor/$size/apps/$(basename "$icon")"
done
for notice in LICENSE NOTICE THIRD-PARTY-NOTICES.md COMMERCIAL-LICENSE.md CHANGELOG.md; do
  run install -m 644 "$notice" "$DOC/$notice"
done
# The third-party components this build links, and the notices their licences
# ask for (made afresh from the dependencies).
python3 scripts/gen-web-licenses.py linux
run install -m 644 "$APP/linux/license.md" "$DOC/license.md"
run install -m 644 "$APP/fonts/carlito/OFL.txt" "$DOC/Carlito-OFL.txt"

# Tell the desktop about what was installed, when its tools are here (they are
# not in every container; the desktop entry works without them).
if command -v update-desktop-database >/dev/null; then
  run update-desktop-database "$PREFIX/share/applications" || true
fi
if command -v update-mime-database >/dev/null; then
  run update-mime-database "$PREFIX/share/mime" || true
fi
if command -v gtk-update-icon-cache >/dev/null; then
  run gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" || true
fi

cat <<EOF

Installed ibisOffice $VERSION into $PREFIX.

Run it with: ibisOffice <file.docx|file.xlsx|file.pptx>
Remove it with: rm -f $PREFIX/bin/ibisOffice $PREFIX/bin/ibisOfficeUpdater \\
  $PREFIX/share/applications/jp.ne.ibis.office.desktop \\
  $PREFIX/share/metainfo/jp.ne.ibis.office.metainfo.xml \\
  $PREFIX/share/mime/packages/jp.ne.ibis.office.xml \\
  $PREFIX/share/icons/hicolor/*/apps/jp.ne.ibis.office.* && rm -rf $DOC
EOF
