# Ardisa roadmap

Ardisa is built to make AI-first development minimalistic, robust, effective, and traceable.

## Phase 0: bootstrap

- Tiny readable syntax.
- Stable AST and source spans.
- Deterministic diagnostics.
- Dependency-light compiler.
- CI and regression fixtures.

## Phase 1: parser and AST foundation

- Lexer with byte-accurate spans.
- Recursive-descent parser.
- Error recovery with multiple diagnostics.
- Canonical formatter.
- Snapshot and conformance tests.
- Stable semantic identities that survive whitespace and local structural edits.

## Phase 2: semantic safety core

- Rust-compatible ownership and borrowing.
- Type inference for common application code.
- Pattern matching.
- Traits/interfaces.
- Async and structured concurrency.
- Explicit unsafe boundary.

## Phase 3: AI compiler contract

Expose stable AST IDs, spans, symbols, inferred types, effects, ownership transitions, diagnostics, structural edits, and verification requirements.

AI tools should edit structure rather than blindly rewriting text whenever possible.

## Phase 4: verified Rust lowering

- Lower the safe Ardisa subset to readable Rust.
- Preserve source maps.
- Support direct Rust escape blocks.
- Verify generated Rust with rustc.
- Differential-test reference programs.
- Preserve source-to-Rust-to-test evidence.

## Phase 5: reduce Rust friction

Candidate changes include inferred lifetimes where provably safe, simpler ownership transfer syntax, safer shared-state patterns, structured errors, explicit effects, and simpler async composition.

No feature should weaken memory safety merely to make AI generation easier.

## Phase 6: native compiler

Only after semantics and lowering are stable: incremental compilation, parallel compilation, deterministic builds, and Rust crate interoperability.

## Phase 7: AI engineering loop

```text
source -> AST -> plan -> structural edit -> compile -> test
-> diagnose -> repair -> verify -> evidence -> traceable outcome
```

## Completion criteria

Every phase follows:

```text
implement -> PR -> verify -> fix -> review -> merge -> re-evaluate -> next phase
```

A phase is complete only when implementation, tests, CI, review, and merge are clean.
