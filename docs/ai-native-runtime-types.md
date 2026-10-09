# AI-native runtime types

Status: runtime foundation; these are not yet first-class source-language types.

Ardisa's current AI runtime already provides validated probability values, token/logit distributions, embeddings, tensors, quantization metadata, and semantic values. This phase adds three composable runtime wrappers in `ardisa_core::ai_types` without pretending they are parsed or compiled language syntax yet.

## Probabilistic<T>

Stores a selected typed value, a confidence in [0, 1], and typed alternatives with their probabilities. Applications must choose an explicit confidence threshold before taking consequential action.

```rust
let result = Probabilistic::new(
    true,
    Probability::new(0.91).unwrap(),
    vec![],
);
if result.meets_threshold(Probability::new(0.85).unwrap()) {
    // The caller has explicitly accepted this confidence threshold.
}
```

The wrapper does not infer calibration quality from a raw score and does not silently promote low-confidence values.

## GraphTensor

Combines the existing shape-checked Tensor with semantic nodes and directed relationships. Construction rejects duplicate node IDs, out-of-range tensor rows, and edges referencing missing nodes. Metadata merging requires an explicit conflict policy: reject, prefer-left, or prefer-higher-confidence.

Tensor arithmetic and physical layout are intentionally not implicit. A graph merge does not claim GPU acceleration or replace a vector database by itself.

## Guaranteed<T>

Wraps a value only after a caller-provided validator accepts it. An optional repair callback may try a bounded number of candidates, each of which is validated again. Exhaustion returns a structured error.

```rust
let profile = Guaranteed::repair_with(
    candidate,
    2,
    validate_profile,
    repair_profile,
)?;
```

Repair is application-supplied; the runtime does not secretly call a model, execute untrusted generated code, or promise that every malformed payload can be healed. Use deterministic validators and enforce resource/time limits in the caller's repair adapter.

## Safety contract

- Confidence is explicit and bounded; threshold decisions belong to the caller.
- Graph references are validated and conflict resolution is deterministic and explicit.
- Schema validation happens before a value enters Guaranteed<T>; every repaired candidate is checked again.
- No model network calls, hidden retries, GPU behavior, or compiler syntax support is implied by these runtime APIs.

## Next compiler integration gates

First-class syntax such as `Probabilistic<T>`, `GraphTensor`, and `Guaranteed<T>` requires coordinated parser/AST, semantic typing, formatting, typed IR, native execution/ABI, and self-hosting support. Promote the names into the language only after those paths and differential/holdout tests pass.