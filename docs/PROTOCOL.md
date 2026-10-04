# Compiler protocol contract

The compiler protocol is a repository-local machine-facing contract. It must remain usable without an AI service.

Version: `ardisa-compiler-protocol-v1`.

A compiler snapshot exposes source, stable AST/module data, trace timings, IR, effects, ownership, diagnostics, and explicit verification requirements. The verification list is descriptive evidence of the checks performed by the snapshot pipeline; it does not grant acceptance authority.

## Required verification chain

1. parse
2. semantic validation
3. ownership validation
4. effect analysis
5. IR lowering

Structural edits are re-parsed and re-validated before a response is returned. Invalid node IDs remain edit errors rather than being silently ignored.

## Stability rules

- Protocol version changes are explicit.
- Verification requirement names are stable identifiers.
- Node IDs and source spans remain part of the compiler data model.
- Learning remains advisory and does not alter protocol authority.
