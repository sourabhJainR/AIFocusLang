#!/usr/bin/env bash
set -euo pipefail

mkdir -p bootstrap/evidence target

ROOT="$(pwd)"
PROBE="$ROOT/bootstrap/repro_probe.ardisa"
PAYLOAD="$ROOT/bootstrap/probe_payload.ardisa"
STAGE1="$ROOT/target/bootstrap-probe-stage1.aexe"
STAGE2="$ROOT/target/bootstrap-probe-stage2.aexe"
STAGE3="$ROOT/target/bootstrap-probe-stage3.aexe"

if [[ ! -f "$PROBE" || ! -f "$PAYLOAD" ]]; then
  echo "BOOTSTRAP-GATE: missing bootstrap bootstrap probe files"
  exit 70
fi

cargo run -q -p ardisa -- bootstrap compile "$PROBE" "$STAGE1"
cargo run -q -p ardisa -- bootstrap compile-from-executable "$STAGE1" "$PAYLOAD" "$STAGE2"
cargo run -q -p ardisa -- bootstrap compile-from-executable "$STAGE2" "$PAYLOAD" "$STAGE3"

cmp -s "$STAGE2" "$STAGE3" || {
  echo "BOOTSTRAP-GATE: stage-2/stage-3 mismatch"
  exit 71
}

cargo run -q -p ardisa -- bootstrap verify "$STAGE2"
cargo run -q -p ardisa -- bootstrap verify "$STAGE3"
{
  echo "probe_stage2_stage3_byte_identical=true"
  echo "probe_stage2=$STAGE2"
  echo "probe_stage3=$STAGE3"
  echo "compiler_self_hosting=false"
  echo "compiler_self_hosting_requires_native_pipeline=true"
} > bootstrap/evidence/self-hosting.txt
echo "BOOTSTRAP-GATE: deterministic executable replay verified for the bootstrap probe."
