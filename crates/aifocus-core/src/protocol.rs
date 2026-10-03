use crate::{
    EffectModel, IrModule, Module, NodeId, OwnershipModel, edit,
    edit::StructuralEdit, effects, ir, ownership, sema, source::Diagnostic,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerSnapshot {
    pub source: String,
    pub module: Module,
    pub ir: IrModule,
    pub effects: EffectModel,
    pub ownership: OwnershipModel,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilerRequest {
    Inspect,
    ApplyEdit(StructuralEdit),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerResponse {
    pub snapshot: CompilerSnapshot,
    pub changed_node: Option<NodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidSource(Vec<Diagnostic>),
    Semantic(Vec<Diagnostic>),
    Ownership(Vec<Diagnostic>),
    Edit(String),
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSource(errors) => write!(f, "source parse failed: {errors:?}"),
            Self::Semantic(errors) => write!(f, "semantic validation failed: {errors:?}"),
            Self::Ownership(errors) => write!(f, "ownership validation failed: {errors:?}"),
            Self::Edit(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ProtocolError {}

pub fn execute(source: &str, request: CompilerRequest) -> Result<CompilerResponse, ProtocolError> {
    let module = crate::parse(source).map_err(ProtocolError::InvalidSource)?;
    match request {
        CompilerRequest::Inspect => snapshot(source, module).map(|snapshot| CompilerResponse {
            snapshot,
            changed_node: None,
        }),
        CompilerRequest::ApplyEdit(edit_request) => {
            let result = edit::apply(source, &module, edit_request)
                .map_err(|error| ProtocolError::Edit(error.to_string()))?;
            let changed_node = Some(result.replaced_node);
            snapshot(&result.source, result.module).map(|snapshot| CompilerResponse {
                snapshot,
                changed_node,
            })
        }
    }
}

fn snapshot(source: &str, module: Module) -> Result<CompilerSnapshot, ProtocolError> {
    sema::check(&module).map_err(ProtocolError::Semantic)?;
    let ownership = ownership::analyze(&module).map_err(ProtocolError::Ownership)?;
    let effects = effects::analyze(&module);
    let ir = ir::lower(&module);
    let diagnostics = Vec::new();
    Ok(CompilerSnapshot {
        source: source.into(),
        module,
        ir,
        effects,
        ownership,
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Item, StmtKind};

    #[test]
    fn inspect_exposes_ir_effects_and_ownership() {
        let source = "module x\nfn main(a: Int) -> Int\n  a + 1\n";
        let response = execute(source, CompilerRequest::Inspect).unwrap();
        assert_eq!(response.snapshot.module.name, "x");
        assert_eq!(response.snapshot.ir.functions.len(), 1);
        assert!(response.snapshot.effects.functions.contains_key("main"));
        assert!(!response.snapshot.ownership.accesses.is_empty());
        assert!(response.snapshot.diagnostics.is_empty());
    }

    #[test]
    fn apply_edit_returns_new_validated_snapshot() {
        let source = "module x\nfn main() -> Int\n  1\n";
        let module = crate::parse(source).unwrap();
        let Item::Function(function) = &module.items[0];
        let StmtKind::Expr(expr) = &function.body.stmts[0].kind else {
            panic!("expected expression");
        };
        let response = execute(
            source,
            CompilerRequest::ApplyEdit(StructuralEdit::Replace {
                node: expr.id,
                source: "2".into(),
            }),
        )
        .unwrap();
        assert_eq!(response.changed_node, Some(expr.id));
        assert!(response.snapshot.source.contains("2"));
    }

    #[test]
    fn invalid_edit_is_rejected_before_snapshot() {
        let source = "module x\nfn main() -> Int\n  1\n";
        let result = execute(
            source,
            CompilerRequest::ApplyEdit(StructuralEdit::Replace {
                node: NodeId(99),
                source: "2".into(),
            }),
        );
        assert!(matches!(result, Err(ProtocolError::Edit(_))));
    }

    #[test]
    fn diagnostic_memory_can_be_attached_by_clients_without_protocol_state() {
        let memory = crate::DiagnosticMemory::default();
        assert!(memory.history("AIF304").is_none());
    }
}use crate::{
    edit::{self, StructuralEdit},
    effects, ir, ownership, sema,
    EffectModel, IrModule, Module, NodeId, OwnershipModel,
    source::Diagnostic,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerSnapshot {
    pub source: String,
    pub module: Module,
    pub ir: IrModule,
    pub effects: EffectModel,
    pub ownership: OwnershipModel,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilerRequest {
    Inspect,
    ApplyEdit(StructuralEdit),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerResponse {
    pub snapshot: CompilerSnapshot,
    pub changed_node: Option<NodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidSource(Vec<Diagnostic>),
    Semantic(Vec<Diagnostic>),
    Ownership(Vec<Diagnostic>),
    Edit(String),
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSource(errors) => write!(f, "source parse failed: {errors:?}"),
            Self::Semantic(errors) => write!(f, "semantic validation failed: {errors:?}"),
            Self::Ownership(errors) => write!(f, "ownership validation failed: {errors:?}"),
            Self::Edit(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ProtocolError {}

pub fn execute(source: &str, request: CompilerRequest) -> Result<CompilerResponse, ProtocolError> {
    let module = crate::parse(source).map_err(ProtocolError::InvalidSource)?;
    match request {
        CompilerRequest::Inspect => snapshot(source, module).map(|snapshot| CompilerResponse {
            snapshot,
            changed_node: None,
        }),
        CompilerRequest::ApplyEdit(edit_request) => {
            let result = edit::apply(source, &module, edit_request)
                .map_err(|error| ProtocolError::Edit(error.to_string()))?;
            let changed_node = Some(result.replaced_node);
            snapshot(&result.source, result.module).map(|snapshot| CompilerResponse {
                snapshot,
                changed_node,
            })
        }
    }
}

fn snapshot(source: &str, module: Module) -> Result<CompilerSnapshot, ProtocolError> {
    sema::check(&module).map_err(ProtocolError::Semantic)?;
    let ownership = ownership::analyze(&module).map_err(ProtocolError::Ownership)?;
    let effects = effects::analyze(&module);
    let ir = ir::lower(&module);
    let diagnostics = Vec::new();
    Ok(CompilerSnapshot {
        source: source.into(),
        module,
        ir,
        effects,
        ownership,
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Item, StmtKind};

    #[test]
    fn inspect_exposes_ir_effects_and_ownership() {
        let source = "module x\nfn main(a: Int) -> Int\n  a + 1\n";
        let response = execute(source, CompilerRequest::Inspect).unwrap();
        assert_eq!(response.snapshot.module.name, "x");
        assert_eq!(response.snapshot.ir.functions.len(), 1);
        assert!(response.snapshot.effects.functions.contains_key("main"));
        assert!(!response.snapshot.ownership.accesses.is_empty());
        assert!(response.snapshot.diagnostics.is_empty());
    }

    #[test]
    fn apply_edit_returns_new_validated_snapshot() {
        let source = "module x\nfn main() -> Int\n  1\n";
        let module = crate::parse(source).unwrap();
        let Item::Function(function) = &module.items[0];
        let StmtKind::Expr(expr) = &function.body.stmts[0].kind else {
            panic!("expected expression");
        };
        let response = execute(
            source,
            CompilerRequest::ApplyEdit(StructuralEdit::Replace {
                node: expr.id,
                source: "2".into(),
            }),
        )
        .unwrap();
        assert_eq!(response.changed_node, Some(expr.id));
        assert!(response.snapshot.source.contains("2"));
    }

    #[test]
    fn invalid_edit_is_rejected_before_snapshot() {
        let source = "module x\nfn main() -> Int\n  1\n";
        let result = execute(
            source,
            CompilerRequest::ApplyEdit(StructuralEdit::Replace {
                node: NodeId(99),
                source: "2".into(),
            }),
        );
        assert!(matches!(result, Err(ProtocolError::Edit(_))));
    }

    #[test]
    fn diagnostic_memory_can_be_attached_by_clients_without_protocol_state() {
        let memory = crate::DiagnosticMemory::default();
        assert!(memory.history("AIF304").is_none());
    }
}
