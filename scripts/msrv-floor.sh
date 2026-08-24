#!/bin/sh
# MSRV floor gate. Copies the crate to a scratch directory, strips the
# [dev-dependencies] section and the lockfile, then builds the library with
# the declared rust-version. Consumers never compile dev-dependencies, so
# the floor verifies exactly what a consumer receives.
#
#   scripts/msrv-floor.sh <cargo> <version> [--doc]
#
# <cargo> must resolve toolchains by +<version>. Pass --doc to also run the
# doctests at the floor, which works because the copy has no dev-dependencies.
set -eu
CARGO="$1"
VERSION="$2"
DOC="${3:-}"
SRC="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
cp -R "$SRC" "$SCRATCH/crate"
rm -rf "$SCRATCH/crate/target" "$SCRATCH/crate/fuzz" "$SCRATCH/crate/Cargo.lock"
awk 'BEGIN{skip=0} /^\[dev-dependencies\]$/{skip=1;next} /^\[/{skip=0} !skip' \
    "$SRC/Cargo.toml" > "$SCRATCH/crate/Cargo.toml"
"$CARGO" "+$VERSION" build --lib --manifest-path "$SCRATCH/crate/Cargo.toml" --target-dir "$SCRATCH/target"
if [ "$DOC" = "--doc" ]; then
    "$CARGO" "+$VERSION" test --doc --manifest-path "$SCRATCH/crate/Cargo.toml" --target-dir "$SCRATCH/target"
fi
