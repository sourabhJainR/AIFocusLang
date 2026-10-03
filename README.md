# Ardisa

Ardisa is an AI-first systems programming language designed to make software development **minimalistic, robust, effective, and traceable**.

Ardisa keeps useful Rust foundations—native performance, predictable memory use, strong types, explicit control flow, and memory safety—while reducing ceremony that makes code harder for people and AI coding systems to generate, understand, change, and verify.

## Design goals

- **AI-first:** structure, intent, diagnostics, and verification are first-class compiler concepts.
- **Minimalistic:** a small language surface with little ceremony.
- **Robust:** safety is preserved; convenience must not silently weaken correctness.
- **Effective:** common application and systems tasks should require less code and fewer concepts.
- **Traceable:** source, AST nodes, diagnostics, generated Rust, tests, and verification evidence remain connected.
- **Readable:** generated and handwritten code should remain understandable without prompt history.
- **Rust-compatible:** Ardisa can lower to Rust and use Rust as an interoperability escape hatch.
- **No AI runtime dependency:** AI systems are development clients, not required for program execution.

## AI-first development model

Ardisa is built around a structured development loop:

```text
intent
  -> structured source
  -> AST + stable identity
  -> semantic analysis
  -> ownership / effects
  -> readable Rust
  -> compile + test
  -> diagnostics + evidence
  -> targeted structural edit
  -> verify
```

The goal is to make every change understandable, reviewable, reproducible, and attributable to a source construct.

## Example

```ardisa
module math

fn add(a: Int, b: Int) -> Int
  a + b

fn divide(a: Int, b: Int) -> Result<Int, MathError>
  if b == 0
    Err(MathError::DivisionByZero)
  else
    Ok(a / b)
```

The syntax is intentionally small and indentation-aware so both people and coding agents can work with it directly.

## Architecture

1. Small readable source language.
2. Stable AST, source spans, diagnostics, and semantic identity.
3. Rust lowering for the verified subset.
4. Rust interoperability and explicit escape hatches.
5. AI-facing compiler APIs for structural editing and verification.
6. Native compiler work only after language semantics are stable.

## Repository layout

- `crates/aifocus-core` - core compiler model and analysis
- `crates/aifocus-cli` - compiler frontend CLI
- `examples` - Ardisa language examples
- `docs` - language and implementation design

The directory names remain `aifocus-*` for now to preserve the bootstrap history; the public language and package identity are Ardisa.

## Status

The compiler bootstrap currently includes an indentation-aware lexer, structured AST, parser, formatter, diagnostics, semantic/type checking, ownership inference, readable Rust lowering, and rustc verification.

The next implementation focus is a stable semantic identity and structural-edit contract for AI tooling, followed by richer type inference, borrowing, effects, concurrency, interoperability, differential testing, and native compilation.

See `docs/ROADMAP.md` and `docs/DESIGN.md`.
