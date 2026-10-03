# Ardisa

Ardisa is an AI-first systems programming language designed to make software development **minimalistic, robust, effective, and traceable**.

Ardisa keeps Rust's strengths: native performance, predictable memory use, strong types, explicit control flow, and memory safety, while reducing ceremony that makes code harder for people and AI coding systems to generate, understand, change, and verify.

## Design goals

- **AI-first:** structure, intent, diagnostics, and verification are first-class compiler concepts.
- **Minimalistic:** a small language surface with little ceremony.
- **Robust:** safety is preserved; convenience must not silently weaken correctness.
- **Effective:** common application and systems tasks should require less code and fewer concepts.
- **Traceable:** source, AST nodes, diagnostics, generated Rust, tests, and verification evidence remain connected.
- **Readable:** generated and handwritten code should remain understandable without prompt history.

- A smaller, easier-to-generate language surface.
- Intent-first declarations and explicit effects.
- Less lifetime ceremony in application code where the compiler can prove safety.
- Structured errors and deterministic diagnostics.
- Stable syntax trees and source spans for AI-assisted editing.
- Readable lowering to ordinary Rust.
- No mandatory AI service or runtime dependency.

## Architecture

1. Small readable source language.
2. Stable AST, spans, diagnostics and semantic model.
3. Rust lowering for the safe subset.
4. Rust interoperability and escape hatches.
5. Native compiler work only after semantics are stable.
6. AI-facing compiler APIs for structural editing and verification.

## Example

```aif
module math

fn add(a: Int, b: Int) -> Int
  a + b

fn divide(a: Int, b: Int) -> Result<Int, MathError>
  if b == 0
    Err(MathError::DivisionByZero)
  else
    Ok(a / b)
```

The syntax is intentionally easy for both people and coding agents to parse.

## Repository layout

- `crates/aifocus-core` - source model and diagnostics
- `crates/aifocus-cli` - compiler frontend CLI
- `examples` - language examples
- `docs` - language and implementation design

## Status

Bootstrap stage. The first milestone establishes a small language contract and compiler architecture before adding advanced Rust features.

See `docs/ROADMAP.md` and `docs/DESIGN.md`.

## AI-first development model

Ardisa is built around a structured development loop:

```text
intent -> structured source -> AST + stable identity -> semantic analysis
-> ownership / effects -> readable Rust -> compile + test
-> diagnostics + evidence -> targeted structural edit -> verify
```

The goal is to make every change understandable, reviewable, reproducible, and attributable to a source construct.
