# Ardisa security and resource-safety model

Ardisa is being built so safety constraints are part of the compiler and runtime contract, not prompts or comments. This document separates implemented enforcement from goals that are not yet proven.

## Enforced in the current implementation slice

- Structured scopes can share a ResourceBudget. The default scope budget allows at most 64 concurrently reserved tasks; custom budgets can cap tasks, reserved bytes, and cumulative operations.
- Task/byte reservations use atomic checked arithmetic. Exceeding a limit fails closed with a typed error; leases release reserved capacity on drop.
- Structured-scope task creation reserves capacity before starting a worker. The lease is retained until the worker has terminated and been joined.
- Evidence promotion now requires at least one envelope with a structurally valid HTTPS source URI, a 64-hex-character SHA-256 field, and a verification-receipt reference, in addition to the existing canary/holdout inputs.
- AI Mode constructs remain rejected by semantic analysis until their semantics are implemented; the parser cannot silently grant a capability by accepting a declaration.

## Important boundaries

- Resource reservations are cooperative accounting. They bound only work that opts into the budget API; they do not intercept every Rust allocation or OS resource.
- The current structured task API uses cooperative cancellation. Rust cannot forcibly terminate a thread that ignores its cancellation token. Joining such a worker can block indefinitely; do not run untrusted or potentially non-cooperative work in an in-process thread when a hard timeout is required. A process/OS sandbox with enforceable limits is needed for that guarantee.
- A task budget does not prove deadlock freedom. Deadlock freedom requires a restricted synchronization model or static lock-order/effect analysis plus runtime enforcement; arbitrary foreign calls can block indefinitely.
- Source URI, digest, and receipt shape checks are not cryptographic authentication. A trusted verifier must resolve the source, recompute the digest, validate the receipt/signature, and produce the canary/holdout results. User text, model output, test names, or a self-reported status are not proof.
- The current compiler does not yet provide a complete capability sandbox, general memory-safe native runtime for every possible operation, authenticated evidence pipeline, or formal proof checker. No release or agent should claim those guarantees until the corresponding enforcement and adversarial tests exist.

## Required promotion policy

1. Treat all human- and AI-authored source as untrusted.
2. Compile only through the same parser, type checker, ownership checker, effect checker, and backend validation path.
3. Reject unsupported safety declarations and effects rather than erasing them.
4. Enforce per-task and per-session resource budgets; use process isolation for untrusted or non-cooperative work.
5. Bind claims to immutable source/artifact digests and verifier-generated receipts. Recompute hashes and validate signatures; never accept a model's assertion as evidence.
6. Require adversarial tests for limit exhaustion, cancellation, worker panic, resource cleanup, malformed evidence, stale artifacts, and replay before promotion.
7. Keep separate statuses for statically proved, runtime checked, structurally evidenced, unknown, and failed. Tests or syntactically valid references are not formal proof.

## Next security gates

- Integrate resource budgets into every runtime allocation and execution entry point, with explicit per-session budgets.
- Define a bounded execution API with deadlines and process isolation for non-cooperative workloads.
- Add a static synchronization/effect model and tests for lock-order cycles; do not claim general deadlock freedom before this exists.
- Replace caller-supplied evidence booleans with verifier-issued, authenticated receipts tied to source and artifact digests.
- Complete Vault capability enforcement across parsing, typed IR, native execution, and interop; reject unsupported effects at the backend boundary.
- Run sanitizer/Miri where applicable, concurrency stress tests, fuzzing, differential tests, and resource-exhaustion tests in CI.