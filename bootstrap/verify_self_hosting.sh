#!/usr/bin/env bash
set -euo pipefail

rm -rf bootstrap/evidence
mkdir -p bootstrap/evidence target

seed="bootstrap/stage0.aexe"
source="bootstrap/compiler.ardisa"
stage1="bootstrap/evidence/stage1.aexe"
stage2="bootstrap/evidence/stage2.aexe"
stage3="bootstrap/evidence/stage3.aexe"

test -s "$seed"
grep -q '^ARDISA-EXEC-V1$' "$seed"

sha256sum "$seed" "$source" > bootstrap/evidence/input-provenance.sha256

cargo run -q --release -p ardisa -- bootstrap chain "$source" bootstrap/evidence

test -s "$stage1"
test -s "$stage2"
test -s "$stage3"

sha256sum "$stage1" "$stage2" "$stage3" > bootstrap/evidence/stage-hashes.sha256
cmp -s "$stage2" "$stage3"

test -s bootstrap/evidence/corpus-native_hello.aexe

echo "BOOTSTRAP-GATE: Stage 0 -> Stage 1 -> Stage 2 -> Stage 3 verified."
echo "BOOTSTRAP-GATE: Stage 2 == Stage 3 byte-for-byte."
