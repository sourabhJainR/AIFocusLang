# Safe Rust interoperability

Ardisa exposes Rust interoperability through a deliberately narrow boundary.

## Contract

- ABI contract version: `ardisa-c-abi-v1`.
- Only scalar `Int`, `Bool`, and `Unit` types are accepted by the safe boundary.
- Symbols and wrapper names must be valid identifiers.
- The generated wrapper contains one isolated unsafe extern call.
- The safe boundary does not make arbitrary Rust types safe by string generation.

## Expansion rule

Any new interop type must add an explicit ABI mapping, positive and negative fixtures, and generated-wrapper verification before it becomes part of the safe contract.

## Current limitation

Strings, lists, records, references, closures, and trait objects are not part of the safe ABI contract yet. They require explicit ownership and ABI semantics rather than implicit conversion.
