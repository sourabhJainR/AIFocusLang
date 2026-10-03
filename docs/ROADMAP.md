# Ardisa roadmap

The canonical source extension is `.ardisa`; all examples, fixtures, tooling, and editor integrations should use `*.ardisa`.

## Phase 0: bootstrap

- Tiny readable syntax.
- Stable AST and source spans.
- Deterministic diagnostics.
- Dependency-light compiler.
- CI and regression fixtures.

## Phase 1: real parser

- Lexer with byte-accurate spans.
- Recursive-descent parser.
- Error recovery with multiple diagnostics.
- Canonical formatter.
- Snapshot and conformance tests.

## Phase 2: Rust semantic core

- Rust-compatible ownership and borrowing.
- Type inference for common application code.
- Pattern matching.
- Traits/interfaces.
- Async and structured concurrency.
- Explicit unsafe boundary.

## Phase 3: AI compiler contract

Expose stable AST IDs, spans, symbols, inferred types, effects, ownership transitions, diagnostics, structural edits, and verification requirements.

AI tools should edit structure rather than blindly rewriting text whenever possible.

## Phase 4: Rust lowering

- Lower the safe Ardisa subset to readable Rust.
- Preserve source maps.
- Support direct Rust escape blocks.
- Verify generated Rust with rustc.
- Differential-test reference programs.

## Phase 5: reduce Rust friction

Candidate changes include inferred lifetimes where provably safe, simpler ownership transfer syntax, safer shared-state patterns, structured errors, explicit effects, and simpler async composition.

No feature should weaken memory safety merely to make AI generation easier.

## Phase 6: native compiler

Only after semantics and lowering are stable: incremental compilation, parallel compilation, deterministic builds, and Rust crate interoperability.

## Phase 7: engineering loop

source -> AST -> plan -> edit -> compile -> test -> diagnose -> repair -> verify -> evidence

The language remains fully usable without an AI system.
