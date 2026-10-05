#!/usr/bin/env bash
set -euo pipefail

rm -rf bootstrap/evidence
mkdir -p bootstrap/evidence target

cargo run -q --release -p ardisa -- bootstrap chain bootstrap/compiler.ardisa bootstrap/evidence
cmp -s bootstrap/evidence/stage2.aexe bootstrap/evidence/stage3.aexe
test -s bootstrap/stage0.aexe
grep -q '^ARDISA-EXEC-V1$' bootstrap/stage0.aexe

echo "BOOTSTRAP-GATE: native stage-0/1/2/3 chain verified."
