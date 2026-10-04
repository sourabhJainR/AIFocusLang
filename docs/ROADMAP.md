# Ardisa roadmap

The implementation is evidence-gated. A phase is complete only after its executable acceptance checks pass in CI and the status is reflected here.

## Completed foundations
- Tiny .ardisa language surface, lexer, parser, AST, spans, formatter and deterministic diagnostics.
- Flow-sensitive ownership state propagation across conditional and loop control flow.
- Read/write/call effect analysis with resource-level read/write sets and transitive invalidation.
- Static structured-concurrency scope checking with runtime cancellation, deterministic cleanup order, child-failure propagation and sibling cancellation.
- Narrow safe Rust scalar ABI contract with isolated unsafe wrapper generation.
- Differential tests, deterministic generated cases and benchmark fingerprints.
- Versioned compiler protocol with verification requirements, node queries, atomic multi-edit transactions and deterministic `ardisa-wire-v1` request framing.
- Persistent compiler learning with observed and verified-repair states.
- Reproducible stage2 Ardisa-authored compiler-pipeline replay.
- Direct `ardisa run` execution through the dependency-free native backend for the supported CLI argument types.
- Rust lowering remains available as an interoperability/build path, including valid lowering of mutable bindings.

## Remaining phases
### A. Semantic completeness
- explicit lifetime/borrow-region syntax rather than conservative flow joins;
- broader type inference, patterns, traits/interfaces and richer Result/error semantics;
- richer ownership/type interaction for aggregate values.

### B. Runtime semantics
- compiler-level task failure diagnostics and source mapping;
- runtime trace exposure through the compiler protocol;
- native execution of structured-concurrency operations.

### C. Interoperability
- compile-time ABI fixtures;
- safe aggregate/reference types with explicit ownership and ABI rules;
- explicit unsafe escape-block contract.

### D. Verification and benchmarks
- mutation-based fuzzing;
- independent native/reference result oracle across the supported subset;
- compile-time and generated-code-size metrics;
- reproducible benchmark reports.

### E. Compiler protocol
- richer AST/symbol/type/effect query operations;
- transaction diagnostics with source/evidence mapping;
- response framing and a stable stdio transport.

### F. Learning and engineering evidence
- verified repair provenance;
- replay/regression linkage;
- release-level work/evidence manifests.

### G. Native compiler and bootstrap
- complete IR coverage for the supported language subset;
- backend differential corpus;
- true compiler self-compilation;
- deterministic self-rebuild and independent bootstrap verification.

True self-hosting remains intentionally unclaimed until the Rust host is no longer required to compile the compiler itself.
