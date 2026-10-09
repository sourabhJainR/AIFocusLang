# Ardisa AI Mode — language design and implementation contract

**Status: design specification; not yet accepted source syntax.** This document defines the next language phase. It must not be described as implemented until the parser, AST, semantic checks, typed IR, native backend, and negative tests all enforce the contract.

AI Mode is an opt-in compilation profile for code that should be easy to generate, inspect, evaluate, reproduce, and constrain. It adds five orthogonal constructs: `Trace`, `Cell`, `Vault`, `Proof`, and `Phase`. Existing Ardisa programs remain valid without AI Mode.

## Goals and non-goals

AI Mode should make security boundaries explicit and machine-checkable. It should fail closed when a requested guarantee cannot be established, preserve evidence through compilation, and make uncertain or unverified behavior visible.

It cannot promise absolute immunity from attacks. Memory safety does not prove business correctness; a hash does not prove authorship; a contract may be unprovable; a sandbox depends on its runtime and OS; and a state machine cannot secure a compromised capability provider. The compiler must report exactly which checks were proved, dynamically checked, delegated to a trusted runtime, or left unverified.

## Illustrative syntax

The examples below are a proposed grammar, not code accepted by the current compiler.

```ardisa
module payments

ai mode secure {
  trace required
  vault {
    capabilities: [clock]
    network: none
    filesystem: none
  }
  proof {
    require: [type_safety, bounds, effect_policy]
    on_unknown: reject
  }
}

cell Percentage = Float where self >= 0.0 && self <= 100.0

proof fn apply_discount(price: Money, discount: Percentage) -> Money {
  requires: price >= 0
  ensures: result >= 0
  decreases: none
  price * (1.0 - discount / 100.0)
}

phase Order {
  Pending -> Paid
  Paid -> Fulfilled
  Pending -> Cancelled
}
```

`Trace`, `Cell`, `Vault`, `Proof`, and `Phase` are reserved design names. The initial implementation should prefer declarations and blocks over magic runtime behavior.

## Construct contracts

### 1. `Trace` — tamper-evident build and decision provenance

A trace record binds a source/build artifact to a canonical source hash, compiler version, dependency lock digest, build configuration, verification results, and optional AI-assistance metadata. AI metadata should use a model/provider identifier and prompt/session digest when explicitly enabled; raw prompts, secrets, private source, or embeddings must not be embedded by default.

- The build pipeline signs attestations with a configured signing identity; the compiler itself must not invent a trusted signature.
- Any source or dependency change invalidates the matching digest and requires a new build attestation.
- Runtime event traces are separate from build provenance and must support redaction and bounded storage.
- Trace output identifies *who/what attested to a build*, not whether the code is correct.

### 2. `Cell` — values that preserve declared invariants

A cell declaration defines a predicate over a value. Construction and mutation must pass through validation; invalid values cannot enter a safe cell through ordinary language operations.

- Constant expressions may be checked at compile time.
- Dynamic values receive generated validation checks unless the compiler can prove the predicate.
- Arithmetic overflow, NaN behavior, floating-point precision, and integer conversion rules must be explicit.
- Foreign-function and unsafe boundaries must validate values on entry and exit.
- A failed check returns a typed error or traps according to the declared policy; it must never silently coerce an invalid value.

### 3. `Vault` — effect and capability boundary

A vault declares the effects and capabilities available to a function or region. File, network, process, clock, random, and secret access require explicit capabilities.

- The effect checker rejects undeclared effects and ambient authority.
- Capability handles are explicit values, scoped, non-forgeable within the safe language, and auditable.
- A runtime sandbox may provide additional isolation, but compiler-level `Vault` alone does not create a hardware enclave or guarantee OS isolation.
- Unsafe code and foreign calls must be explicitly marked and separately reviewed.
- Capability grants are least-privilege and must not be derived from untrusted model output.

### 4. `Proof` — behavioral contracts with honest evidence levels

A proof declaration can specify preconditions, postconditions, invariants, and resource bounds. The compiler should support a staged verification model:

1. Type/effect and syntactic contract validation.
2. Runtime assertion generation for checks that can be evaluated dynamically.
3. SMT/abstract-interpretation proof for supported decidable fragments.
4. Explicit `unknown` or `not_proved` results when proof is unavailable.

A function must not be labelled formally verified merely because its assertions compile or tests pass. Complexity budgets need a defined cost model; wall-clock limits alone are environment-dependent. Timing-sensitive guarantees require dedicated constant-time analysis and target-specific evidence.

### 5. `Phase` — sealed state transitions and resource consumption

A phase declaration defines the legal state graph of a workflow. Transition functions consume the old state token and return the new state token, so safe code cannot reuse the old state after transition.

- All reachable states and transitions are enumerated; illegal transitions are rejected statically where possible and at runtime at untrusted boundaries.
- Authorization remains a separate check: reaching a state is not proof that the caller is authorized.
- Persistence, retries, concurrency, cancellation, and distributed transactions require explicit transition semantics.
- Serialized state must be validated and versioned before it is trusted.

## AI Mode evidence report

Every AI Mode build should emit a machine-readable report containing:

- source/build and dependency digests;
- compiler/toolchain version and target;
- checks run, checks passed, checks failed, and checks skipped;
- proof status per contract (`proved`, `runtime_checked`, `unknown`, or `failed`);
- required and granted capabilities;
- unsafe/FFI boundaries;
- state-transition coverage where applicable;
- reproducibility and differential/holdout test results;
- signature identity and verification status, when signing is configured.

The report must distinguish test evidence from proof evidence and never convert a missing result into success.

## Implementation sequence and merge gates

1. **Grammar/AST:** parse the mode declaration and each construct into dedicated structured AST nodes; formatter round-trip and malformed-input tests.
2. **Semantic model:** validate Cell predicates, effect/capability declarations, Proof clauses, and Phase transition graphs; reject unknown clauses and unsupported guarantees.
3. **Typed IR:** preserve contracts and security metadata structurally, with stable source spans and explicit evidence states.
4. **Runtime/native boundary:** enforce dynamic Cell checks, capability checks, phase-token consumption, and trace-report serialization; unsupported ABI cases fail closed.
5. **Verification:** negative tests for invalid invariants, undeclared effects, false postconditions, illegal/replayed transitions, tampered attestations, and malformed serialized state.
6. **Independent CI:** run workspace tests, native-phase tests, mutation/fuzz tests, differential/holdout corpus, bootstrap reproducibility, and artifact attestation checks before promotion.

No stage may silently lower a security declaration to a comment or discard it in generated code. Until the relevant stage is implemented and tested, the compiler must reject the construct or emit a clear unsupported-feature diagnostic.
