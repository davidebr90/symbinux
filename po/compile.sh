#!/usr/bin/env bash
# Compile every po/<code>.po into src/symbinux/locale/<code>/LC_MESSAGES/symbinux.mo
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(dirname "$here")"
domain="symbinux"

while read -r lang; do
    case "$lang" in
        ''|\#*) continue ;;
    esac
    src="$here/$lang.po"
    dest_dir="$repo/src/symbinux/locale/$lang/LC_MESSAGES"
    mkdir -p "$dest_dir"
    msgfmt "$src" --output-file="$dest_dir/$domain.mo"
    echo "compiled $lang -> $dest_dir/$domain.mo"
done < "$here/LINGUAS"
