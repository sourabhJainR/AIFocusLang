# Ardisa compiler and self-hosting architecture

## Selected approach

Ardisa will use a small, auditable, deterministic compiler pipeline rather than a text-rewriting bootstrap compiler.

1. Lexer produces structured token records with kind, value and source position.
2. Parser uses recursive descent with Pratt precedence and builds an arena-backed AST. Nodes are structural records; textual AST serialization is never an intermediate representation.
3. Semantic analysis builds a deterministic symbol/type environment and records inferred types and diagnostics by node id.
4. Ownership analysis is a separate flow-sensitive pass over the same AST. It tracks moves, shared borrows, mutable borrows and lifetime regions.
5. Effect analysis computes read/write/call/spawn/join/cancel effects and propagates them through the call graph.
6. Typed IR is control-flow aware. Functions contain basic blocks, typed operations and explicit terminators. This prevents control flow from being hidden inside strings or expression blobs.
7. Optimization runs on typed IR and is semantics preserving. Constant folding must never delete an executable terminal expression; control-flow simplification must preserve returns and side effects.
8. Instruction selection lowers typed IR to the complete ARDISA-EXEC-V1 instruction set. Artifact emission is canonical and deterministic.
9. The compiler is itself an Ardisa program. The Rust implementation is bootstrap infrastructure only and must not participate in stage-2/stage-3 compilation.
10. The fixed-point gate compares the complete canonical stage-2 and stage-3 artifacts byte-for-byte.

## Bootstrap stages

- Stage 0: trusted Rust implementation of the language runtime/compiler primitives.
- Stage 1: Stage-0 compiles the Ardisa compiler source to ARDISA-EXEC-V1.
- Stage 2: Stage-1 executable compiles the same compiler source.
- Stage 3: Stage-2 executable compiles the same compiler source again.
- Fixed point: stage 2 and stage 3 artifacts are byte-identical.

## Concepts adopted from established self-hosting compiler practice

- Keep the bootstrap compiler small enough to audit and make the first native compiler progressively self-sufficient.
- Use explicit CFG/basic-block IR so optimization and code generation have stable structural inputs.
- Treat bootstrap as a chain of independently executable stages, not as a metadata claim.
- Make deterministic artifact comparison a release gate rather than relying only on functional tests.
- Maintain differential, fuzz, holdout and lifecycle tests around the compiler so a green self-hosting build cannot mask semantic regressions.

## Ardisa-specific edge

The compiler IR, ownership model, effect model, diagnostics and AI-facing structural editing model share stable node identities and source spans. This allows compiler transformations to remain traceable without making the compiler depend on an external AI framework.

## Readiness rule

`self_hosting_ready` remains false until all of the following are demonstrated by executable evidence:

- native Ardisa lexer/parser/AST;
- native semantic/type/ownership/effect analysis;
- native typed IR and optimization;
- complete native instruction selection and ARDISA-EXEC-V1 emission;
- compiler.ardisa compiled by the native compiler;
- stage-2 and stage-3 compilation succeed;
- stage-2 and stage-3 artifacts are byte-identical;
- CI records the evidence and the independent/differential/holdout corpus remains green.