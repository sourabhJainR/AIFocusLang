# Ardisa roadmap

The implementation is evidence-gated. A phase is complete only after its executable acceptance checks pass in CI and the status is reflected here.

## Completed foundations
- Tiny .ardisa language surface, lexer, parser, AST, spans, formatter and deterministic diagnostics.
- Ownership analysis with move/use-after-move and borrow transition checks.
- Read/write/call effect analysis and call dependency propagation.
- Static structured-concurrency scope checking and runtime cancellation/join cleanup.
- Narrow safe Rust scalar ABI contract with isolated unsafe wrapper generation.
- Differential tests, deterministic generated cases and benchmark fingerprints.
- Versioned compiler protocol with verification requirements, node queries and atomic multi-edit transactions.
- Persistent compiler learning with observed and verified-repair states.
- Reproducible stage2 Ardisa-authored compiler-pipeline replay.

## Remaining phases
### A. Semantic completeness
- flow-sensitive borrow regions across branches, loops and nested scopes;
- explicit mutable borrow semantics;
- broader type inference, patterns, traits/interfaces and Result/error semantics;
- field/resource-level effect dependencies and incremental invalidation.

### B. Runtime semantics
- task failure propagation and sibling cancellation;
- deterministic runtime task traces;
- stronger structured-concurrency integration with compiler diagnostics.

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
- stable machine-readable wire encoding;
- richer AST/symbol/type/effect query operations;
- transaction diagnostics with source/evidence mapping.

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