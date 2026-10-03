use std::collections::HashMap;

use crate::{
    ast::{BinaryOp, Block, Expr, ExprKind, Function, Item, Module, StmtKind, Type, TypeKind},
    source::{Diagnostic, Span},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature {
    pub params: Vec<Type>,
    pub return_type: Option<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SemanticModel {
    pub inferred_types: HashMap<crate::NodeId, Type>,
    pub function_returns: HashMap<String, Type>,
}

pub fn check(module: &Module) -> Result<(), Vec<Diagnostic>> {
    analyze(module).map(|_| ())
}

pub fn analyze(module: &Module) -> Result<SemanticModel, Vec<Diagnostic>> {
    let mut checker = Checker {
        functions: HashMap::new(),
        inferred_types: HashMap::new(),
        function_returns: HashMap::new(),
        errors: Vec::new(),
    };

    for item in &module.items {
        let Item::Function(function) = item;
        let signature = FunctionSignature {
            params: function.params.iter().map(|p| p.ty.clone()).collect(),
            return_type: function.return_type.clone(),
        };
        if checker
            .functions
            .insert(function.name.clone(), signature)
            .is_some()
        {
            checker.error(
                "AIF300",
                format!("duplicate function '{}'", function.name),
                function.span,
            );
        }
    }

    // Infer omitted return types to a fixed point. This makes call sites benefit
    // from information discovered in functions declared later in the module.
    for _ in 0..module.items.len().max(1) {
        let before = checker.function_returns.clone();
        for item in &module.items {
            let Item::Function(function) = item;
            if function.return_type.is_none() {
                let mut locals = function
                    .params
                    .iter()
                    .map(|p| (p.name.clone(), p.ty.clone()))
                    .collect::<HashMap<_, _>>();
                if let Some(ty) = checker.check_block(&function.body, &mut locals) {
                    checker.function_returns.insert(function.name.clone(), ty);
                }
            } else if let Some(ty) = function.return_type.clone() {
                checker.function_returns.insert(function.name.clone(), ty);
            }
        }
        if before == checker.function_returns {
            break;
        }
    }

    for item in &module.items {
        let Item::Function(function) = item;
        checker.check_function(function);
    }

    if checker.errors.is_empty() {
        Ok(SemanticModel {
            inferred_types: checker.inferred_types,
            function_returns: checker.function_returns,
        })
    } else {
        Err(checker.errors)
    }
}

struct Checker {
    functions: HashMap<String, FunctionSignature>,
    inferred_types: HashMap<crate::NodeId, Type>,
    function_returns: HashMap<String, Type>,
    errors: Vec<Diagnostic>,
}

impl Checker {
    fn check_function(&mut self, function: &Function) {
        let mut locals = HashMap::new();
        for param in &function.params {
            if locals
                .insert(param.name.clone(), param.ty.clone())
                .is_some()
            {
                self.error(
                    "AIF301",
                    format!("duplicate parameter '{}'", param.name),
                    param.span,
                );
            }
            self.inferred_types.insert(param.id, param.ty.clone());
        }

        let block_type = self.check_block(&function.body, &mut locals);
        if let Some(expected) = &function.return_type {
            if let Some(actual) = block_type {
                if !same_type(&actual, expected) {
                    self.error(
                        "AIF302",
                        format!(
                            "function '{}' returns {}, expected {}",
                            function.name,
                            actual.display_name(),
                            expected.display_name()
                        ),
                        function.body.span,
                    );
                }
            }
        }
    }

    fn check_block(&mut self, block: &Block, locals: &mut HashMap<String, Type>) -> Option<Type> {
        let mut last = None;
        for stmt in &block.stmts {
            match &stmt.kind {
                StmtKind::Let { name, value } => {
                    if locals.contains_key(name) {
                        self.error(
                            "AIF303",
                            format!("binding '{}' shadows an existing local", name),
                            stmt.span,
                        );
                    }
                    if let Some(ty) = self.check_expr(value, locals) {
                        locals.insert(name.clone(), ty);
                    }
                    last = None;
                }
                StmtKind::Return(value) => {
                    last = value.as_ref().and_then(|e| self.check_expr(e, locals));
                }
                StmtKind::Expr(expr) => {
                    last = self.check_expr(expr, locals);
                }
            }
        }
        last
    }

    fn check_expr(&mut self, expr: &Expr, locals: &HashMap<String, Type>) -> Option<Type> {
        let result = match &expr.kind {
            ExprKind::Int(_) => Some(type_node(TypeKind::Int, expr.span)),
            ExprKind::Bool(_) => Some(type_node(TypeKind::Bool, expr.span)),
            ExprKind::String(_) => Some(type_node(TypeKind::String, expr.span)),
            ExprKind::Name(name) => {
                if let Some(ty) = locals.get(name) {
                    Some(ty.clone())
                } else if let Some(signature) = self.functions.get(name) {
                    signature
                        .return_type
                        .clone()
                        .or_else(|| self.function_returns.get(name).cloned())
                } else {
                    self.error("AIF304", format!("unknown name '{name}'"), expr.span);
                    None
                }
            }
            ExprKind::Group(inner) => self.check_expr(inner, locals),
            ExprKind::Binary { op, left, right } => {
                let left_type = self.check_expr(left, locals)?;
                let right_type = self.check_expr(right, locals)?;
                match op {
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                        if !is_kind(&left_type, &TypeKind::Int)
                            || !is_kind(&right_type, &TypeKind::Int)
                        {
                            self.error(
                                "AIF305",
                                "arithmetic operators require Int operands",
                                expr.span,
                            );
                            None
                        } else {
                            Some(type_node(TypeKind::Int, expr.span))
                        }
                    }
                    BinaryOp::Equal => {
                        if !same_type(&left_type, &right_type) {
                            self.error(
                                "AIF306",
                                "equality operands must have the same type",
                                expr.span,
                            );
                            None
                        } else {
                            Some(type_node(TypeKind::Bool, expr.span))
                        }
                    }
                }
            }
            ExprKind::Call { callee, args } => {
                let ExprKind::Name(name) = &callee.kind else {
                    self.error(
                        "AIF307",
                        "only named functions are callable in this language version",
                        callee.span,
                    );
                    return None;
                };
                let Some(signature) = self.functions.get(name).cloned() else {
                    self.error("AIF308", format!("unknown function '{name}'"), callee.span);
                    return None;
                };
                if args.len() != signature.params.len() {
                    self.error(
                        "AIF309",
                        format!(
                            "function '{}' expects {} argument(s), got {}",
                            name,
                            signature.params.len(),
                            args.len()
                        ),
                        expr.span,
                    );
                }
                for (index, arg) in args.iter().enumerate() {
                    if let Some(actual) = self.check_expr(arg, locals) {
                        if let Some(expected) = signature.params.get(index) {
                            if !same_type(&actual, expected) {
                                self.error(
                                    "AIF310",
                                    format!(
                                        "argument {} to '{}' has type {}, expected {}",
                                        index + 1,
                                        name,
                                        actual.display_name(),
                                        expected.display_name()
                                    ),
                                    arg.span,
                                );
                            }
                        }
                    }
                }
                signature
                    .return_type
                    .or_else(|| self.function_returns.get(name).cloned())
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition_type = self.check_expr(condition, locals)?;
                if !is_kind(&condition_type, &TypeKind::Bool) {
                    self.error("AIF311", "if condition must be Bool", condition.span);
                }
                let mut then_locals = locals.clone();
                let then_type = self.check_block(then_branch, &mut then_locals);
                let else_branch = match else_branch {
                    Some(branch) => branch,
                    None => return Some(type_node(TypeKind::Unit, expr.span)),
                };
                let mut else_locals = locals.clone();
                let else_type = self.check_block(else_branch, &mut else_locals);
                match (then_type, else_type) {
                    (Some(left), Some(right)) if same_type(&left, &right) => Some(left),
                    (Some(_), Some(_)) => {
                        self.error(
                            "AIF312",
                            "if branches must produce the same type",
                            expr.span,
                        );
                        None
                    }
                    _ => Some(type_node(TypeKind::Unit, expr.span)),
                }
            }
        };
        if let Some(ref ty) = result {
            self.inferred_types.insert(expr.id, ty.clone());
        }
        result
    }

    fn error(&mut self, code: &'static str, message: impl Into<String>, span: Span) {
        self.errors
            .push(Diagnostic::error(code, message, Some(span)));
    }
}

fn type_node(kind: TypeKind, span: Span) -> Type {
    Type {
        id: crate::NodeId(0),
        span,
        kind,
    }
}

fn is_kind(ty: &Type, kind: &TypeKind) -> bool {
    &ty.kind == kind
}

fn same_type(left: &Type, right: &Type) -> bool {
    left.kind == right.kind
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn accepts_well_typed_function() {
        let module = parse("module x\nfn add(a: Int, b: Int) -> Int\n  a + b\n").unwrap();
        assert!(check(&module).is_ok());
    }

    #[test]
    fn catches_unknown_name() {
        let module = parse("module x\nfn add(a: Int) -> Int\n  a + missing\n").unwrap();
        let errors = check(&module).unwrap_err();
        assert!(errors.iter().any(|error| error.code == "AIF304"));
    }

    #[test]
    fn catches_argument_type_mismatch() {
        let module = parse(
            "module x\nfn add(a: Int, b: Int) -> Int\n  a + b\nfn main() -> Int\n  add(true, 1)\n",
        )
        .unwrap();
        let errors = check(&module).unwrap_err();
        assert!(errors.iter().any(|error| error.code == "AIF310"));
    }

    #[test]
    fn infers_omitted_return_type_and_propagates_it_to_calls() {
        let module = parse("module x\nfn value()\n  42\nfn main() -> Int\n  value()\n").unwrap();
        let model = analyze(&module).unwrap();
        assert_eq!(model.function_returns["value"].kind, TypeKind::Int);
        assert_eq!(
            model
                .inferred_types
                .values()
                .filter(|t| t.kind == TypeKind::Int)
                .count(),
            2
        );
    }

    #[test]
    fn infers_return_through_later_declaration() {
        let module = parse("module x\nfn main() -> Int\n  value()\nfn value()\n  42\n").unwrap();
        assert!(check(&module).is_ok());
    }
}
