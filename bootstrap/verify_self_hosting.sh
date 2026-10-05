#!/usr/bin/env bash
set -euo pipefail

mkdir -p bootstrap/evidence target

ROOT="$(pwd)"
COMPILER="$ROOT/bootstrap/compiler.ardisa"
STAGE1="$ROOT/target/bootstrap-stage1.aexe"
STAGE2="$ROOT/target/bootstrap-stage2.aexe"
STAGE3="$ROOT/target/bootstrap-stage3.aexe"

if [[ ! -f "$COMPILER" ]]; then
  echo "SELF-HOSTING-GATE: missing bootstrap/compiler.ardisa"
  exit 70
fi

cargo run -q -p ardisa -- bootstrap compile "$COMPILER" "$STAGE1"
cargo run -q -p aifocus-cli -- bootstrap compile-from-executable "$STAGE1" "$COMPILER" "$STAGE2"
cargo run -q -p aifocus-cli -- bootstrap compile-from-executable "$STAGE2" "$COMPILER" "$STAGE3"

cmp -s "$STAGE2" "$STAGE3" || {
  echo "SELF-HOSTING-GATE: stage-2/stage-3 mismatch"
  exit 71
}

cargo run -q -p aifocus-cli -- bootstrap verify "$STAGE2"
cargo run -q -p aifocus-cli -- bootstrap verify "$STAGE3"
{
  echo "stage2_stage3_byte_identical=true"
  echo "stage2=$STAGE2"
  echo "stage3=$STAGE3"
} > bootstrap/evidence/self-hosting.txt
echo "SELF-HOSTING-GATE: deterministic stage-2/stage-3 rebuild verified"
