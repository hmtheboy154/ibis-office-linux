#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) ibis inc.
# Additional terms under AGPL-3.0 section 7(b): see NOTICE.
# Build the Linux app icons, crates/ibis-office-app/linux/hicolor/, from the
# 1024x1024 master design.
#
#   scripts/gen-linux-icons.sh
#
# The master is the same artwork the Windows icons are made from
# (crates/ibis-office-app/windows/png/master/ibisOffice_Windows_transparent.png):
# the hicolor sizes a desktop asks for, each downscaled from it, plus the
# scalable one (the logo) for the desktops that draw it themselves.
#
# Needs ImageMagick (magick, or convert on ImageMagick 6). The icons are
# checked in, so this is only run when the design changes.
set -euo pipefail
cd "$(dirname "$0")/.."

SRC=crates/ibis-office-app/windows/png/master/ibisOffice_Windows_transparent.png
DEST=crates/ibis-office-app/linux/hicolor
SIZES=(16 22 24 32 48 64 128 256 512)

resize() {
  if command -v magick >/dev/null; then
    magick "$SRC" -filter Lanczos -resize "${1}x${1}" "$2"
  else
    convert "$SRC" -filter Lanczos -resize "${1}x${1}" "$2"
  fi
}

for size in "${SIZES[@]}"; do
  dir="$DEST/${size}x${size}/apps"
  mkdir -p "$dir"
  resize "$size" "$dir/jp.ne.ibis.office.png"
  echo "wrote $dir/jp.ne.ibis.office.png"
done

mkdir -p "$DEST/scalable/apps"
cp crates/ibis-office-app/logo/ibisOffice_logo.svg "$DEST/scalable/apps/jp.ne.ibis.office.svg"
echo "wrote $DEST/scalable/apps/jp.ne.ibis.office.svg"
