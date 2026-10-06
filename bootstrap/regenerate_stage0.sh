#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

seed="bootstrap/stage0.aexe"
transition="bootstrap/stage0.transition.aexe"
source="bootstrap/compiler.ardisa"
candidate="bootstrap/stage0.rebuilt.aexe"

rm -rf bootstrap/evidence
mkdir -p bootstrap/evidence

test -s "$seed"
test -s "$transition"
test -s "$source"
grep -q '^ARDISA-EXEC-V1$' "$transition"

sha256sum "$seed" "$transition" "$source" > bootstrap/evidence/bootstrap-inputs.sha256

# The transition artifact is a previously verified native ARDISA compiler.
# Rust only decodes and executes ARDISA-EXEC-V1; it never parses or lowers
# bootstrap/compiler.ardisa.
cargo run -q --release -p ardisa -- bootstrap compile-from-executable "$transition" "$source" "$candidate"

test -s "$candidate"
grep -q '^ARDISA-EXEC-V1$' "$candidate"

sha256sum "$transition" "$candidate" > bootstrap/evidence/native-transition.sha256
cp "$candidate" "$seed"

./bootstrap/verify_self_hosting.sh
sha256sum "$seed" > bootstrap/evidence/stage0.final.sha256
rm -f "$candidate"
