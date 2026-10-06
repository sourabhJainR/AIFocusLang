#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

seed="bootstrap/stage0.aexe"
source="bootstrap/compiler.ardisa"
candidate="bootstrap/stage0.rebuilt.aexe"

rm -rf bootstrap/evidence
mkdir -p bootstrap/evidence

test -s "$seed"
test -s "$source"

sha256sum "$seed" "$source" > bootstrap/evidence/bootstrap-inputs.sha256

# Stage 0 is the trusted native compiler. Rust only decodes and executes
# ARDISA-EXEC-V1; it never parses, lowers, or constructs the compiler artifact.
cargo run -q --release -p ardisa -- bootstrap compile-from-executable "$seed" "$source" "$candidate"

test -s "$candidate"
grep -q '^ARDISA-EXEC-V1$' "$candidate"

sha256sum "$seed" "$candidate" > bootstrap/evidence/native-stage0.sha256
cp "$candidate" "$seed"

./bootstrap/verify_self_hosting.sh
sha256sum "$seed" > bootstrap/evidence/stage0.final.sha256
rm -f "$candidate"
