# Ardisa artifact gap matrix

Status is measured by executable evidence, not by the existence of a module.

| Area | Current evidence | Remaining artifact/gap |
|---|---|---|
| Lexer/parser/AST | deterministic parser, source spans, structural bootstrap replay | broader grammar conformance corpus and documented grammar reference |
| Ownership/borrowing | move/use-after-move and borrow transition checks | flow-sensitive borrow regions across complex control flow and mutable borrow syntax |
| Effects | read/write/call collection and call propagation | field/resource-level dependency invalidation and incremental invalidation evidence |
| Structured concurrency | static scope analysis plus runtime scope cleanup/cancellation | failure propagation and richer deterministic runtime traces |
| Rust interop | explicit scalar ABI contract and isolated unsafe wrapper | compile-time ABI fixtures and safe aggregate/reference types |
| Fuzz/property testing | deterministic generators and round-trip properties | larger syntax corpus, mutation/fuzz harness, native/reference result oracle |
| AI compiler protocol | versioned snapshot with AST/IR/effects/ownership/verification data | structural query/transaction operations and stable machine-readable encoding |
| Persistent learning | deterministic save/load and recurring diagnostics | verified repair outcomes, provenance, replay and regression linkage |
| Native IR/backend | executable native instruction backend | complete supported-language coverage and backend differential corpus |
| Benchmarks | generated-case pipeline benchmark | correctness oracle, compile-time/size metrics and reproducible benchmark reports |
| Bootstrap | stage2 deterministic replay | true self-compilation and self-rebuild without the Rust host |
| Release hardening | CI and standalone documentation | release smoke tests, reproducibility manifest and supported-subset declaration |

The remaining gaps are intentionally not marked complete until their acceptance tests exist and pass.
