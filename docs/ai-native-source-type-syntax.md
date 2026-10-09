# AI-native source type syntax

This phase adds generic type syntax to Ardisa's native AST and parser. The syntax is structural and round-trips through the canonical formatter:

```ardisa
module ai_types
fn accept(value: Probabilistic<Int>) -> Guaranteed<String>
  "ok"

fn graph(value: GraphTensor) -> GraphTensor
  value
```

Nested generic forms are also parsed, for example `List<Probabilistic<Int>>`. Generic type arguments are represented as `TypeKind::Generic(name, args)`; they are not flattened into a string. Existing `List<T>` and `Result<T, E>` retain their established AST variants.

## Scope boundary

This step makes the type names representable in source, formatter, and semantic signatures. It does **not** claim runtime construction, inference, native ABI serialization, or first-class execution of `Probabilistic<T>`, `GraphTensor`, or `Guaranteed<T>`. Those need explicit typed-IR/backend representation and runtime ABI work before programs can instantiate or manipulate these values. Unknown generic names remain parseable as user-defined types, consistent with the current named-type policy.

## Validation

Tests cover parse → format → parse stability, nested generic syntax, and semantic signature checking. Workspace, native-phase, differential/holdout, and bootstrap gates remain mandatory before merge.
