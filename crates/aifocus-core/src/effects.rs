use std::collections::{HashMap, HashSet};

use crate::ast::{Block, Expr, ExprKind, Function, Item, Module, StmtKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectKind {
    Read,
    Write,
    Call,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FunctionEffects {
    pub effects: HashSet<EffectKind>,
    pub calls: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EffectModel {
    pub functions: HashMap<String, FunctionEffects>,
}

pub fn analyze(module: &Module) -> EffectModel {
    let mut functions = HashMap::new();
    for item in &module.items {
        let Item::Function(function) = item;
        let mut effects = FunctionEffects::default();
        collect_block(&function.body, &mut effects);
        functions.insert(function.name.clone(), effects);
    }

    // Propagate call effects to a fixed point so callers inherit the effects of
    // functions declared anywhere in the module.
    for _ in 0..module.items.len().max(1) {
        let before = functions.clone();
        let snapshot = functions.clone();
        for effects in functions.values_mut() {
            for callee in effects.calls.clone() {
                if let Some(callee_effects) = snapshot.get(&callee) {
                    effects.effects.extend(callee_effects.effects.iter().copied());
                }
            }
        }
        if functions == before {
            break;
        }
    }

    EffectModel { functions }
}

fn collect_block(block: &Block, effects: &mut FunctionEffects) {
    for stmt in &block.stmts {
        match &stmt.kind {
            StmtKind::Let { value, .. } => {
                effects.effects.insert(EffectKind::Write);
                collect_expr(value, effects);
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    collect_expr(value, effects);
                }
            }
            StmtKind::Expr(expr) => collect_expr(expr, effects),
        }
    }
}

fn collect_expr(expr: &Expr, effects: &mut FunctionEffects) {
    match &expr.kind {
        ExprKind::Name(_) => {
            effects.effects.insert(EffectKind::Read);
        }
        ExprKind::Binary { left, right, .. } => {
            collect_expr(left, effects);
            collect_expr(right, effects);
        }
        ExprKind::Call { callee, args } => {
            effects.effects.insert(EffectKind::Call);
            if let ExprKind::Name(name) = &callee.kind {
                effects.calls.insert(name.clone());
            } else {
                collect_expr(callee, effects);
            }
            for arg in args {
                collect_expr(arg, effects);
            }
        }
        ExprKind::Group(inner) => collect_expr(inner, effects),
        ExprKind::If { condition, then_branch, else_branch } => {
            collect_expr(condition, effects);
            collect_block(then_branch, effects);
            if let Some(else_branch) = else_branch {
                collect_block(else_branch, effects);
            }
        }
        ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::String(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn propagates_callee_effects_to_callers() {
        let module = parse("module x\nfn leaf(a: Int) -> Int\n  a\nfn main() -> Int\n  leaf(1)\n").unwrap();
        let model = analyze(&module);
        let main = &model.functions["main"];
        assert!(main.effects.contains(&EffectKind::Call));
        assert!(main.effects.contains(&EffectKind::Read));
        assert!(main.calls.contains("leaf"));
    }

    #[test]
    fn records_local_binding_writes() {
        let module = parse("module x\nfn main() -> Int\n  let x = 1\n  x\n").unwrap();
        let model = analyze(&module);
        let main = &model.functions["main"];
        assert!(main.effects.contains(&EffectKind::Write));
        assert!(main.effects.contains(&EffectKind::Read));
    }
}
