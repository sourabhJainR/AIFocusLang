use crate::ir::{IrModule, IrOp, IrValue};

pub fn optimize(mut module: IrModule) -> IrModule {
    for function in &mut module.functions {
        let last = function.ops.len().saturating_sub(1);
        function.ops = function.ops.drain(..).enumerate().filter_map(|(i, op)| fold_op(op, i == last)).collect();
    }
    module
}

fn fold_op(op: IrOp, preserve_value: bool) -> Option<IrOp> {
    match op {
        IrOp::Expr(v) => {
            if preserve_value { Some(IrOp::Expr(fold_value(v)?)) } else { None }
        },
        IrOp::Let{name,value} => Some(IrOp::Let{name,value:fold_value(value)?}),
        IrOp::Set{name,value} => Some(IrOp::Set{name,value:fold_value(value)?}),
        IrOp::SetIndex{collection,index,value} => Some(IrOp::SetIndex{collection:fold_value(collection)?,index:fold_value(index)?,value:fold_value(value)?}),
        IrOp::Return(v) => Some(IrOp::Return(v.map(fold_value).transpose()?)),
        IrOp::Scope{ops} => Some(IrOp::Scope{ops:ops.into_iter().filter_map(|op| fold_op(op, false)).collect()}),
        IrOp::Spawn{name,call} => Some(IrOp::Spawn{name,call:fold_value(call)?}),
        IrOp::Join{name} => Some(IrOp::Join{name}),
        IrOp::Cancel{name} => Some(IrOp::Cancel{name}),
        IrOp::While{condition,ops} => Some(IrOp::While{condition:fold_value(condition)?,ops:ops.into_iter().filter_map(fold_op).collect()}),
    }
}

fn fold_value(value: IrValue) -> Option<IrValue> {
    match value {
        IrValue::Binary{op,left,right} => {
            let l=fold_value(*left)?; let r=fold_value(*right)?;
            match (&op,&l,&r) {
                (crate::BinaryOp::Add,IrValue::Int(a),IrValue::Int(b))=>Some(IrValue::Int(a+b)),
                (crate::BinaryOp::Sub,IrValue::Int(a),IrValue::Int(b))=>Some(IrValue::Int(a-b)),
                (crate::BinaryOp::Mul,IrValue::Int(a),IrValue::Int(b))=>Some(IrValue::Int(a*b)),
                (crate::BinaryOp::Div,IrValue::Int(a),IrValue::Int(b)) if *b!=0=>Some(IrValue::Int(a/b)),
                (crate::BinaryOp::Mod,IrValue::Int(a),IrValue::Int(b)) if *b!=0=>Some(IrValue::Int(a%b)),
                (crate::BinaryOp::Equal,_,_)=>Some(IrValue::Bool(l==r)),
                (crate::BinaryOp::NotEqual,_,_)=>Some(IrValue::Bool(l!=r)),
                (crate::BinaryOp::Less,IrValue::Int(a),IrValue::Int(b))=>Some(IrValue::Bool(a<b)),
                (crate::BinaryOp::LessEqual,IrValue::Int(a),IrValue::Int(b))=>Some(IrValue::Bool(a<=b)),
                (crate::BinaryOp::Greater,IrValue::Int(a),IrValue::Int(b))=>Some(IrValue::Bool(a>b)),
                (crate::BinaryOp::GreaterEqual,IrValue::Int(a),IrValue::Int(b))=>Some(IrValue::Bool(a>=b)),
                _=>Some(IrValue::Binary{op,left:Box::new(l),right:Box::new(r)}),
            }
        }
        IrValue::If{condition,then_ops,else_ops} => {
            let c=fold_value(*condition)?;
            match c { IrValue::Bool(true)=>last_value(then_ops), IrValue::Bool(false)=>last_value(else_ops), _=>Some(IrValue::If{condition:Box::new(c),then_ops,else_ops}) }
        }
        IrValue::List(xs)=>Some(IrValue::List(xs.into_iter().filter_map(fold_value).collect())),
        IrValue::Index(c,i)=>Some(IrValue::Index(Box::new(fold_value(*c)?),Box::new(fold_value(*i)?))),
        IrValue::Call{callee,args}=>Some(IrValue::Call{callee,args:args.into_iter().filter_map(fold_value).collect()}),
        other=>Some(other),
    }
}

fn last_value(ops: Vec<IrOp>) -> Option<IrValue> {
    ops.into_iter().rev().find_map(|op|match op {IrOp::Expr(v)=>Some(v),IrOp::Return(Some(v))=>Some(v),_=>None})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folds_constant_arithmetic() {
        let m=crate::parse("module x\nfn main() -> Int\n  2 + 3 * 4\n").unwrap();
        let ir=crate::ir::lower(&m);
        let optimized=optimize(ir);
        assert!(matches!(optimized.functions[0].ops[0],IrOp::Expr(IrValue::Int(14))));
    }
}
