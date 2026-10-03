# Design principles

## Readability beats cleverness

Generated programs must be understandable without the history of the prompt that produced them.

## Structure beats text

The compiler exposes stable syntax and semantic structures so tools can make targeted edits.

## Proof before convenience

Convenient syntax can remove ceremony, but it must lower to explicit ownership, effects, and control flow.

## AI is a development client

AIFocusLang has no required model or AI runtime. Models and coding agents are compiler clients.

## Rust compatibility is an escape hatch

AIFocusLang should call Rust and lower to Rust, preventing premature ecosystem isolation.

## Small core, strong tooling

Keep the language core small. Put machine-facing complexity in compiler tooling.

## AI-facing compiler contract

The long-term compiler API should expose JSON or an equivalent structured format containing module identity, stable node IDs, source spans, declarations, references, inferred types, effects, ownership transitions, diagnostics, suggested edits, and test/evidence requirements.
