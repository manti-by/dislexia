#!/usr/bin/env bash
#
# Writes the checksums that ship with a release.
#
# One aggregate checksums.txt for a human to eyeball, plus a per-asset .sha256
# alongside every archive for scripted verification.
#
# Reads dist/*.zip, which the release job gets from downloading the build
# artifacts. Must run from the repository root.

set -euo pipefail

cd dist

shopt -s nullglob
archives=(./*.zip)
if [ "${#archives[@]}" -eq 0 ]; then
    echo "no archives in dist/; the build artifacts did not download" >&2
    exit 1
fi

sha256sum "${archives[@]}" > checksums.txt

for f in "${archives[@]}"; do
    sha256sum "$f" > "$f.sha256"
done

cat checksums.txt
