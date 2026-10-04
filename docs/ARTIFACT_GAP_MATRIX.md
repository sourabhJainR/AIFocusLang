# Ardisa artifact gap matrix

Status is measured by executable evidence, not by the existence of a module.

| Area | Current evidence | Remaining artifact/gap |
|---|---|---|
| Lexer/parser/AST | deterministic parser, source spans, formatter, structural bootstrap replay | broader grammar conformance corpus and documented grammar reference |
| Ownership/borrowing | flow-sensitive move/use-after-move state joins plus explicit shared/mutable borrow transitions | explicit lifetime/borrow-region syntax and richer aggregate ownership semantics |
| Effects | read/write/call collection, resource sets, transitive propagation and invalidation query | field-level resources and measured incremental rebuild evidence |
| Structured concurrency | static scope analysis plus runtime cleanup, deterministic task ordering, child failure propagation and sibling cancellation | compiler-integrated failure diagnostics, protocol traces and native execution |
| Rust interop | explicit scalar ABI contract and isolated unsafe wrapper | compile-time ABI fixtures and safe aggregate/reference types |
| Fuzz/property testing | deterministic arithmetic/conditional/list/string generators, round-trip properties, benchmark fingerprint | mutation-based fuzzing and stronger native/reference result oracle |
| AI compiler protocol | versioned snapshot, verification requirements, node query, atomic transactions and deterministic wire request codec | richer query surface, response framing and stdio transport |
| Persistent learning | deterministic V2 save/load, V1 compatibility, observed vs verified-repair state | provenance, replay and regression linkage |
| Native IR/backend | executable native instruction backend and direct CLI execution | complete supported-language coverage and backend differential corpus |
| Benchmarks | generated-case pipeline benchmark with failure count and deterministic fingerprint | correctness oracle, compile-time/size metrics and published reproducible reports |
| Bootstrap | stage2 deterministic replay | true self-compilation and self-rebuild without the Rust host |
| Release hardening | CI and standalone documentation | release smoke tests, reproducibility manifest and supported-subset declaration |

The remaining gaps are intentionally not marked complete until their acceptance tests exist and pass.
