# Ardisa implementation plan

This plan is the executable roadmap for completing Ardisa from the current bootstrap checkpoint.

## Completion gates

Every phase follows the same loop: implement -> test locally/CI -> review the diff -> fix failures -> merge -> re-evaluate the next gate.

A phase is complete only when:
- the requested behavior exists in executable code;
- negative cases are tested;
- generated or lowered output is verified where applicable;
- the public contract and status are documented;
- CI is green on the merged commit.

## Phase 1: repository contract and independence

- remove historical external-project naming from public Ardisa modules;
- document standalone build/runtime expectations;
- establish the implementation plan and evidence rules.

## Phase 2: language semantic completeness

- make ownership/borrowing flow-sensitive across branches, loops, scopes and calls;
- add explicit borrow regions and mutable-borrow transitions;
- strengthen type inference, patterns, Result/error semantics and function signatures;
- make effects and dependency invalidation cover all AST constructs.

## Phase 3: structured concurrency runtime

- define spawn/join/cancel/scope semantics;
- enforce task lifetime and cancellation propagation;
- verify failure propagation, sibling cancellation and scope exit behavior;
- expose deterministic task traces for diagnostics.

## Phase 4: safe Rust interoperability

- define the safe FFI/interop type contract;
- validate ABI-compatible primitive and aggregate types;
- isolate unsafe escape blocks and require explicit boundaries;
- verify generated Rust and interop contracts with compile-time fixtures.

## Phase 5: differential/property/fuzz verification

- expand the reference corpus;
- add deterministic generated programs;
- fuzz lexer/parser/formatter/semantic/lowering boundaries;
- require parse/format/parse and Ardisa/native result invariants.

## Phase 6: AI-native compiler protocol

- stabilize machine-readable schema for AST, spans, symbols, types, effects, ownership, diagnostics, edits and verification requirements;
- add structural query and transaction operations;
- guarantee deterministic IDs and source mappings;
- test malformed requests and stale-node edits.

## Phase 7: persistent compiler learning

- persist verified diagnostic patterns and repair outcomes;
- distinguish observation, attempted repair, verified repair and recurring regression;
- make learning deterministic and replayable;
- keep learning advisory and outside compiler authority.

## Phase 8: native IR/backend and benchmarks

- make the IR complete for the supported language subset;
- verify native execution against the reference semantics;
- add language/compiler benchmarks covering correctness, compile time and generated-code size;
- publish reproducible benchmark fixtures and methodology.

## Phase 9: bootstrap and self-hosting

- replace structural replay with compiler-source compilation through the language compiler;
- perform stage0 -> stage1 -> stage2 artifact comparison;
- prove deterministic self-rebuild;
- only then mark self-hosting ready.

## Phase 10: release hardening

- clean obsolete artifacts and naming;
- document supported language subset and known limitations;
- add release smoke tests and reproducibility checks;
- publish evidence-backed status without overstating compiler maturity.
