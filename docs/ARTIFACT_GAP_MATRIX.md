# Ardisa artifact gap matrix

Status is measured by executable evidence, not by the existence of a module.

| Area | Current evidence | Remaining artifact/gap |
|---|---|---|
| Lexer/parser/AST | deterministic parser, source spans, formatter, structural bootstrap replay | broader grammar conformance corpus and documented grammar reference |
| Ownership/borrowing | move/use-after-move and borrow transition checks | flow-sensitive borrow regions across complex control flow and mutable borrow syntax |
| Effects | read/write/call collection and call propagation | field/resource-level dependency invalidation and incremental invalidation evidence |
| Structured concurrency | static scope analysis plus runtime scope cleanup/cancellation | failure propagation and richer deterministic runtime traces |
| Rust interop | explicit scalar ABI contract and isolated unsafe wrapper | compile-time ABI fixtures and safe aggregate/reference types |
| Fuzz/property testing | deterministic arithmetic/conditional/list/string generators, round-trip properties, benchmark fingerprint | mutation-based fuzzing and stronger native/reference result oracle |
| AI compiler protocol | versioned snapshot, verification requirements, stable node query, atomic multi-edit transactions | stable machine-readable wire encoding and broader query surface |
| Persistent learning | deterministic V2 save/load, V1 compatibility, observed vs verified-repair state | provenance, replay and regression linkage |
| Native IR/backend | executable native instruction backend | complete supported-language coverage and backend differential corpus |
| Benchmarks | generated-case pipeline benchmark with failure count and deterministic fingerprint | correctness oracle, compile-time/size metrics and published reproducible reports |
| Bootstrap | stage2 deterministic replay | true self-compilation and self-rebuild without the Rust host |
| Release hardening | CI and standalone documentation | release smoke tests, reproducibility manifest and supported-subset declaration |

The remaining gaps are intentionally not marked complete until their acceptance tests exist and pass.
