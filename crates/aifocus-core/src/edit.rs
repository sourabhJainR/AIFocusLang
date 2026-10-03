use crate::{
    ast::{Expr, ExprKind, Item, Module, NodeId, Stmt, StmtKind},
    source::Span,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralEdit {
    Replace { node: NodeId, source: String },
    InsertBefore { node: NodeId, source: String },
    Delete { node: NodeId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditResult {
    pub source: String,
    pub changed: Span,
    pub replaced_node: NodeId,
    pub module: Module,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditError {
    pub message: String,
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for EditError {}

pub fn apply(source: &str, module: &Module, edit: StructuralEdit) -> Result<EditResult, EditError> {
    let (node, span, replacement) = match edit {
        StructuralEdit::Replace { node, source } => {
            let span = find_span(module, node).ok_or_else(|| missing(node))?;
            (node, span, source)
        }
        StructuralEdit::InsertBefore { node, source } => {
            let span = find_span(module, node).ok_or_else(|| missing(node))?;
            (node, Span::new(span.start, span.start), source)
        }
        StructuralEdit::Delete { node } => {
            let span = find_span(module, node).ok_or_else(|| missing(node))?;
            (node, span, String::new())
        }
    };

    if span.start > span.end || span.end > source.len() {
        return Err(EditError {
            message: "node span is outside the source".into(),
        });
    }

    let mut next = String::with_capacity(source.len() + replacement.len());
    next.push_str(&source[..span.start]);
    next.push_str(&replacement);
    next.push_str(&source[span.end..]);

    let parsed = crate::parse(&next).map_err(|errors| EditError {
        message: errors
            .into_iter()
            .map(|e| format!("{}: {}", e.code, e.message))
            .collect::<Vec<_>>()
            .join("; "),
    })?;

    Ok(EditResult {
        source: next,
        changed: Span::new(span.start, span.start + replacement.len()),
        replaced_node: node,
        module: parsed,
    })
}

fn missing(node: NodeId) -> EditError {
    EditError {
        message: format!("node {:?} was not found", node),
    }
}

fn find_span(module: &Module, target: NodeId) -> Option<Span> {
    if module.id == target {
        return Some(module.span);
    }
    for item in &module.items {
        if let Item::Function(function) = item {
            if function.id == target {
                return Some(function.span);
            }
            for parameter in &function.params {
                if parameter.id == target {
                    return Some(parameter.span);
                }
                if parameter.ty.id == target {
                    return Some(parameter.ty.span);
                }
            }
            if let Some(span) = find_block(&function.body, target) {
                return Some(span);
            }
        }
    }
    None
}

fn find_block(block: &crate::ast::Block, target: NodeId) -> Option<Span> {
    if block.id == target {
        return Some(block.span);
    }
    for stmt in &block.stmts {
        if let Some(span) = find_stmt(stmt, target) {
            return Some(span);
        }
    }
    None
}

fn find_stmt(stmt: &Stmt, target: NodeId) -> Option<Span> {
    if stmt.id == target {
        return Some(stmt.span);
    }
    match &stmt.kind {
        StmtKind::Let { value, .. } => find_expr(value, target),
        StmtKind::Return(value) => value.as_ref().and_then(|e| find_expr(e, target)),
        StmtKind::Expr(expr) => find_expr(expr, target),
    }
}

fn find_expr(expr: &Expr, target: NodeId) -> Option<Span> {
    if expr.id == target {
        return Some(expr.span);
    }
    match &expr.kind {
        ExprKind::Binary { left, right, .. } => {
            find_expr(left, target).or_else(|| find_expr(right, target))
        }
        ExprKind::Call { callee, args } => {
            find_expr(callee, target).or_else(|| args.iter().find_map(|e| find_expr(e, target)))
        }
        ExprKind::Group(inner) => find_expr(inner, target),
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => find_expr(condition, target)
            .or_else(|| find_block(then_branch, target))
            .or_else(|| else_branch.as_ref().and_then(|b| find_block(b, target))),
        ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::String(_) | ExprKind::Name(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Item;

    #[test]
    fn replaces_expression_by_stable_node_id() {
        let source = "module x
fn f() -> Int
  1
";
        let module = crate::parse(source).unwrap();
        let Item::Function(function) = &module.items[0];
        let StmtKind::Expr(expr) = &function.body.stmts[0].kind else {
            panic!("expected expression")
        };

        let result = apply(
            source,
            &module,
            StructuralEdit::Replace {
                node: expr.id,
                source: "42".into(),
            },
        ).unwrap();

        assert_eq!(result.source, "module x
fn f() -> Int
  42
");
        let Item::Function(updated) = &result.module.items[0];
        let StmtKind::Expr(updated_expr) = &updated.body.stmts[0].kind else {
            panic!("expected expression")
        };
        assert_eq!(updated_expr.id, expr.id);
    }

    #[test]
    fn whitespace_does_not_change_function_identity() {
        let a = crate::parse("module x
fn f() -> Int
  1
").unwrap();
        let b = crate::parse("module x

fn f() -> Int
    1
").unwrap();
        let Item::Function(fa) = &a.items[0];
        let Item::Function(fb) = &b.items[0];
        assert_eq!(fa.id, fb.id);
    }
}
