#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

seed="bootstrap/stage0.aexe"
bridge_source="bootstrap/compiler.seed.ardisa"
bridge="bootstrap/stage0.bridge.aexe"
source="bootstrap/compiler.ardisa"
candidate="bootstrap/stage0.rebuilt.aexe"

rm -rf bootstrap/evidence
mkdir -p bootstrap/evidence

test -s "$seed"
test -s "$bridge_source"
grep -q '^ARDISA-EXEC-V1$' "$seed"

sha256sum "$seed" "$bridge_source" "$source" > bootstrap/evidence/bootstrap-inputs.sha256

# Stage 0 is the trusted native executor/compiler for the previous compiler
# revision. Use it to build a native bridge compiler, then use that bridge
# compiler to build the current compiler. Rust only decodes and executes
# ARDISA-EXEC-V1 artifacts; it never parses or lowers either source file.
cargo run -q --release -p ardisa -- bootstrap compile-from-executable "$seed" "$bridge_source" "$bridge"

test -s "$bridge"
grep -q '^ARDISA-EXEC-V1$' "$bridge"

cargo run -q --release -p ardisa -- bootstrap compile-from-executable "$bridge" "$source" "$candidate"

test -s "$candidate"
grep -q '^ARDISA-EXEC-V1$' "$candidate"

sha256sum "$bridge" "$candidate" > bootstrap/evidence/native-transition.sha256
cp "$candidate" "$seed"

./bootstrap/verify_self_hosting.sh
sha256sum "$seed" > bootstrap/evidence/stage0.final.sha256
rm -f "$bridge" "$candidate"
