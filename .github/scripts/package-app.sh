#!/usr/bin/env bash
#
# Bundles the built binary into Dislexia.app and stages it into a release
# archive.
#
# The bundle is the same layout scripts/bundle.sh produces — Info.plist with
# no LSUIElement (Dock visibility is toggled at runtime), the icns generated
# from media/dislexia.svg — ad-hoc signed for local use, because a
# developer-ID signature needs secrets this repo does not carry. The archive
# holds Dislexia.app plus README.md so a downloaded release unpacks to
# something self-explanatory.
#
# Environment:
#   VERSION  the version being released, without a leading v
#   TARGET   the rust target triple that was built
#   ASSET    the asset suffix, for example macos-aarch64
#
# Writes dist/dislexia-<version>-<asset>.zip. Must run from the repository
# root, on macOS, after `cargo build --release --target $TARGET`.

set -euo pipefail

staging=$(mktemp -d)
trap 'rm -rf "$staging"' EXIT

# The icns comes from the single icon source; regenerate rather than trust a
# stale checked-in asset.
scripts/make-icons.sh

BINARY="target/$TARGET/release/dislexia" BUNDLE_DIR="$staging" scripts/bundle.sh
if [ -f README.md ]; then
    cp README.md "$staging/"
fi

mkdir -p dist
ditto -c -k "$staging" "dist/dislexia-$VERSION-$ASSET.zip"

echo "Packaged dist/dislexia-$VERSION-$ASSET.zip"
