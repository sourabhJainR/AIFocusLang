# Ardisa design principles

Ardisa source files use the canonical `.ardisa` extension. Tooling should recognize this extension consistently so source discovery, structural editing, diagnostics, and verification remain traceable.

## Readability beats cleverness

Generated programs must be understandable without the history of the prompt that produced them.

## Structure beats text

The compiler exposes stable syntax and semantic structures so tools can make targeted edits.

## Proof before convenience

Convenient syntax can remove ceremony, but it must lower to explicit ownership, effects, and control flow.

## AI is a development client

Ardisa has no required model or AI runtime. Models and coding agents are compiler clients.

## Rust compatibility is an escape hatch

Ardisa should call Rust and lower to Rust, preventing premature ecosystem isolation.

## Small core, strong tooling

Keep the language core small. Put machine-facing complexity in compiler tooling.

## AI-facing compiler contract

The long-term compiler API should expose JSON or an equivalent structured format containing module identity, stable node IDs, source spans, declarations, references, inferred types, effects, ownership transitions, diagnostics, suggested edits, and test/evidence requirements.


## Repository independence

Ardisa is a standalone language and compiler repository. Its compiler, tests, learning records, verification logic, and engineering feedback types must not require another project at build time, test time, or runtime. External projects may study Ardisa or consume its documented outputs, but Ardisa must remain executable without them.

## Evidence boundary

A feature is complete only when its behavior is exercised by tests that can fail for the relevant defect. Documentation and manifests describe status; they do not substitute for executable verification.
