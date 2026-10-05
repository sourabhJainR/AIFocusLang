#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

seed="bootstrap/stage0.aexe"
source="bootstrap/compiler.ardisa"
candidate="bootstrap/evidence/stage0.rebuilt.aexe"

rm -rf bootstrap/evidence
mkdir -p bootstrap/evidence

test -s "$seed"
grep -q '^ARDISA-EXEC-V1$' "$seed"

# Rust is used only as the minimal ARDISA-EXEC-V1 executor here.
# The trusted compiler artifact is produced by executing the checked-in
# Stage-0 executable against bootstrap/compiler.ardisa.
cargo run -q --release -p ardisa -- bootstrap compile-from-executable "$seed" "$source" "$candidate"

test -s "$candidate"
grep -q '^ARDISA-EXEC-V1$' "$candidate"

cp "$candidate" bootstrap/stage0.aexe

./bootstrap/verify_self_hosting.sh
