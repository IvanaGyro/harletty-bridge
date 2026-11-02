#!/usr/bin/env bash
# Lay out the Media Foundation decoder's archive in DEST: the DLL, the install
# scripts, the default option files and the licence texts, all at the top
# level. The workflows zip DEST's contents as harletty-mf-<version>-windows-<arch>.zip.
#   scripts/stage-mf-package.sh DLL DEST
set -euo pipefail
dll="${1:?usage: stage-mf-package.sh DLL DEST}"
dest="${2:?usage: stage-mf-package.sh DLL DEST}"
repo="$(cd "$(dirname "$0")/.." && pwd)"

mkdir -p "$dest"
cp "$dll" "$dest/harletty_mf.dll"
cp "$repo"/mf-decoder/install/{install.ps1,uninstall.ps1,common.ps1,mf.yaml,mf-config.yaml} "$dest/"
"$repo/scripts/stage-licences.sh" mf "$dest"
ls -l "$dest"
