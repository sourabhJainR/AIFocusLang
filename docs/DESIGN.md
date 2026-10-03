# Ardisa design principles

Ardisa is **AI-first, minimalistic, robust, effective, and traceable**.

## AI-first, not AI-dependent

Ardisa is designed for AI coding from the language model outward: small syntax, explicit structure, stable identities, precise diagnostics, targeted edits, and machine-verifiable evidence.

An AI model or coding agent is a development client. Ardisa programs do not require an AI service or runtime.

## Minimalism without weakened safety

Remove ceremony, not safety. If the compiler can prove a safe transformation, Ardisa should make the programmer express less. If it cannot prove safety, the language keeps an explicit boundary rather than hiding risk.

## Structure beats text

The compiler exposes syntax and semantic structures so tools can make targeted changes instead of repeatedly rewriting whole files.

## Traceability is a compiler feature

Every meaningful construct should be connectable across source spans, stable node identities, semantic facts, generated Rust, diagnostics, tests, and verification evidence.

This makes changes explainable and makes repair loops precise.

## Readability beats cleverness

Generated programs must be understandable without the history of the prompt that produced them.

## Rust compatibility is an escape hatch

Ardisa should call Rust and lower to Rust, preventing premature ecosystem isolation.

## Small core, strong tooling

Keep the language core small. Put machine-facing complexity in compiler tooling.

## AI-facing compiler contract

The long-term compiler API should expose a structured representation containing module identity, stable node IDs, source spans, declarations and references, inferred types, ownership transitions, effects and dependencies, diagnostics, structural edit operations, test requirements, and verification evidence.

AI tools should edit structure whenever possible and preserve traceability through each transformation.
