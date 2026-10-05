use crate::ir::{IrModule, IrOp, IrValue};

pub fn optimize(mut module: IrModule) -> IrModule {
    for function in &mut module.functions {
        function.ops = std::mem::take(&mut function.ops).into_iter().filter_map(fold_op).collect();
    }
    module
}

fn fold_op(op: IrOp) -> Option<IrOp> {
    match op {
        IrOp::Expr(value) => Some(IrOp::Expr(fold_value(value))),
        IrOp::Let { name, value } => Some(IrOp::Let { name, value: fold_value(value) }),
        IrOp::Set { name, value } => Some(IrOp::Set { name, value: fold_value(value) }),
        IrOp::SetIndex { collection, index, value } => Some(IrOp::SetIndex { collection: fold_value(collection), index: fold_value(index), value: fold_value(value) }),
        IrOp::Return(value) => Some(IrOp::Return(value.map(fold_value))),
        IrOp::Scope { ops } => Some(IrOp::Scope { ops: ops.into_iter().filter_map(fold_op).collect() }),
        IrOp::Spawn { name, call } => Some(IrOp::Spawn { name, call: fold_value(call) }),
        IrOp::Join { name } => Some(IrOp::Join { name }),
        IrOp::Cancel { name } => Some(IrOp::Cancel { name }),
        IrOp::While { condition, ops } => Some(IrOp::While { condition: fold_value(condition), ops: ops.into_iter().filter_map(fold_op).collect() }),
    }
}

fn fold_value(value: IrValue) -> IrValue {
    match value {
        IrValue::Binary { op, left, right } => {
            let left = fold_value(*left);
            let right = fold_value(*right);
            match (&op, &left, &right) {
                (crate::BinaryOp::Add, IrValue::Int(a), IrValue::Int(b)) => IrValue::Int(a + b),
                (crate::BinaryOp::Sub, IrValue::Int(a), IrValue::Int(b)) => IrValue::Int(a - b),
                (crate::BinaryOp::Mul, IrValue::Int(a), IrValue::Int(b)) => IrValue::Int(a * b),
                (crate::BinaryOp::Div, IrValue::Int(a), IrValue::Int(b)) if *b != 0 => IrValue::Int(a / b),
                (crate::BinaryOp::Mod, IrValue::Int(a), IrValue::Int(b)) if *b != 0 => IrValue::Int(a % b),
                (crate::BinaryOp::Equal, _, _) => IrValue::Bool(left == right),
                (crate::BinaryOp::NotEqual, _, _) => IrValue::Bool(left != right),
                (crate::BinaryOp::Less, IrValue::Int(a), IrValue::Int(b)) => IrValue::Bool(a < b),
                (crate::BinaryOp::LessEqual, IrValue::Int(a), IrValue::Int(b)) => IrValue::Bool(a <= b),
                (crate::BinaryOp::Greater, IrValue::Int(a), IrValue::Int(b)) => IrValue::Bool(a > b),
                (crate::BinaryOp::GreaterEqual, IrValue::Int(a), IrValue::Int(b)) => IrValue::Bool(a >= b),
                _ => IrValue::Binary { op, left: Box::new(left), right: Box::new(right) },
            }
        }
        IrValue::If { condition, then_ops, else_ops } => {
            let condition = fold_value(*condition);
            match condition {
                IrValue::Bool(true) => branch_value(then_ops).unwrap_or(IrValue::Bool(false)),
                IrValue::Bool(false) => branch_value(else_ops).unwrap_or(IrValue::Bool(false)),
                condition => IrValue::If { condition: Box::new(condition), then_ops, else_ops },
            }
        }
        IrValue::List(values) => IrValue::List(values.into_iter().map(fold_value).collect()),
        IrValue::Index(collection, index) => IrValue::Index(Box::new(fold_value(*collection)), Box::new(fold_value(*index))),
        IrValue::Call { callee, args } => IrValue::Call { callee, args: args.into_iter().map(fold_value).collect() },
        other => other,
    }
}

fn branch_value(ops: Vec<IrOp>) -> Option<IrValue> {
    ops.into_iter().rev().find_map(|op| match op {
        IrOp::Expr(value) => Some(fold_value(value)),
        IrOp::Return(Some(value)) => Some(fold_value(value)),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folds_constant_arithmetic_without_dropping_terminal_expression() {
        let module = crate::parse("module x\nfn main() -> Int\n  2 + 3 * 4\n").unwrap();
        let optimized = optimize(crate::ir::lower(&module));
        assert!(matches!(optimized.functions[0].ops[0], IrOp::Expr(IrValue::Int(14))));
    }
    #[test]
    fn preserves_non_constant_expression() {
        let module = crate::parse("module x\nfn main(a: Int) -> Int\n  a + 1\n").unwrap();
        let optimized = optimize(crate::ir::lower(&module));
        assert!(matches!(optimized.functions[0].ops[0], IrOp::Expr(IrValue::Binary { .. })));
    }
}