use crate::ast::{BinaryOp, Expr, ExprKind, Module, StmtKind, TypeKind};
use crate::source::Span;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedIrModule {
    pub name: String,
    pub functions: Vec<TypedIrFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedIrFunction {
    pub name: String,
    pub params: Vec<(String, TypeKind)>,
    pub return_type: TypeKind,
    pub blocks: Vec<TypedBasicBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedBasicBlock {
    pub id: u32,
    pub ops: Vec<TypedIrOp>,
    pub terminator: TypedTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedIrOp {
    Bind { name: String, value: TypedValue },
    Assign { name: String, value: TypedValue },
    AssignIndex { collection: TypedValue, index: TypedValue, value: TypedValue },
    Expr(TypedValue),
    Scope(Vec<TypedIrOp>),
    Spawn { name: String, call: TypedValue },
    Join(String),
    Cancel(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedTerminator {
    Fallthrough,
    Return(Option<TypedValue>),
    Branch { condition: TypedValue, then_block: u32, else_block: u32 },
    Loop { condition: TypedValue, body_block: u32, exit_block: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedValue {
    pub ty: TypeKind,
    pub span: Span,
    pub kind: TypedValueKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedValueKind {
    Int(i64),
    Bool(bool),
    String(String),
    Name(String),
    List(Vec<TypedValue>),
    Index(Box<TypedValue>, Box<TypedValue>),
    Binary { op: BinaryOp, left: Box<TypedValue>, right: Box<TypedValue> },
    Call { callee: String, args: Vec<TypedValue> },
}

pub fn lower(module: &Module) -> Result<TypedIrModule, Vec<String>> {
    let analysis = crate::sema::analyze(module).map_err(|errors| {
        errors.into_iter().map(|e| format!("{}: {}", e.code, e.message)).collect::<Vec<_>>()
    })?;
    let mut functions = Vec::new();
    let mut errors = Vec::new();

    for item in &module.items {
        let crate::ast::Item::Function(function) = item;
        let return_type = function.return_type.as_ref()
            .map(|t| t.kind.clone())
            .or_else(|| analysis.function_returns.get(&function.name).map(|t| t.kind.clone()))
            .unwrap_or(TypeKind::Unit);
        let mut locals = BTreeMap::new();
        for p in &function.params { locals.insert(p.name.clone(), p.ty.kind.clone()); }
        let mut ops = Vec::new();
        for stmt in &function.body.stmts {
            lower_stmt(stmt, &analysis.inferred_types, &mut locals, &mut ops, &mut errors);
        }
        let terminator = match ops.last() {
            Some(TypedIrOp::Expr(value)) if return_type != TypeKind::Unit => TypedTerminator::Return(Some(value.clone())),
            _ => TypedTerminator::Return(None),
        };
        functions.push(TypedIrFunction {
            name: function.name.clone(),
            params: function.params.iter().map(|p| (p.name.clone(), p.ty.kind.clone())).collect(),
            return_type,
            blocks: vec![TypedBasicBlock { id: 0, ops, terminator }],
        });
    }
    if errors.is_empty() { Ok(TypedIrModule { name: module.name.clone(), functions }) } else { Err(errors) }
}

fn lower_stmt(
    stmt: &crate::ast::Stmt,
    types: &HashMap<crate::NodeId, crate::ast::Type>,
    locals: &mut BTreeMap<String, TypeKind>,
    ops: &mut Vec<TypedIrOp>,
    errors: &mut Vec<String>,
) {
    match &stmt.kind {
        StmtKind::Let { name, value } => {
            if let Some(v) = lower_expr(value, types, locals, errors) {
                locals.insert(name.clone(), v.ty.clone());
                ops.push(TypedIrOp::Bind { name: name.clone(), value: v });
            }
        }
        StmtKind::Set { name, value } => {
            if let Some(v) = lower_expr(value, types, locals, errors) {
                if let Some(expected) = locals.get(name) {
                    if expected != &v.ty { errors.push(format!("assignment type mismatch for {name}")); }
                } else { errors.push(format!("assignment to unknown binding {name}")); }
                ops.push(TypedIrOp::Assign { name: name.clone(), value: v });
            }
        }
        StmtKind::SetIndex { collection, index, value } => {
            if let (Some(c), Some(i), Some(v)) = (
                lower_expr(collection, types, locals, errors),
                lower_expr(index, types, locals, errors),
                lower_expr(value, types, locals, errors),
            ) { ops.push(TypedIrOp::AssignIndex { collection:c,index:i,value:v }); }
        }
        StmtKind::Return(value) => {
            let v = value.as_ref().and_then(|e| lower_expr(e, types, locals, errors));
            ops.push(TypedIrOp::Expr(v.unwrap_or(TypedValue { ty:TypeKind::Unit, span:stmt.span, kind:TypedValueKind::Bool(false) })));
        }
        StmtKind::Expr(e) => if let Some(v)=lower_expr(e,types,locals,errors){ ops.push(TypedIrOp::Expr(v)); },
        StmtKind::Scope { body } => {
            let mut nested=Vec::new();
            let mut scoped=locals.clone();
            for s in &body.stmts { lower_stmt(s,types,&mut scoped,&mut nested,errors); }
            ops.push(TypedIrOp::Scope(nested));
        }
        StmtKind::Spawn { name, call } => if let Some(v)=lower_expr(call,types,locals,errors){ops.push(TypedIrOp::Spawn{name:name.clone(),call:v});},
        StmtKind::Join { name } => ops.push(TypedIrOp::Join(name.clone())),
        StmtKind::Cancel { name } => ops.push(TypedIrOp::Cancel(name.clone())),
        StmtKind::While { condition, body } => {
            if let Some(c)=lower_expr(condition,types,locals,errors) {
                let mut nested=Vec::new(); let mut scoped=locals.clone();
                for s in &body.stmts { lower_stmt(s,types,&mut scoped,&mut nested,errors); }
                ops.push(TypedIrOp::Scope(vec![TypedIrOp::Expr(c), TypedIrOp::Scope(nested)]));
            }
        }
    }
}

fn lower_expr(
    expr:&Expr,
    types:&HashMap<crate::NodeId, crate::ast::Type>,
    locals:&BTreeMap<String,TypeKind>,
    errors:&mut Vec<String>
)->Option<TypedValue>{
    let ty=types.get(&expr.id).map(|t|t.kind.clone()).or_else(||match &expr.kind {
        ExprKind::Int(_)=>Some(TypeKind::Int), ExprKind::Bool(_)=>Some(TypeKind::Bool),
        ExprKind::String(_)=>Some(TypeKind::String), ExprKind::Name(n)=>locals.get(n).cloned(),
        _=>None
    })?;
    let kind=match &expr.kind {
        ExprKind::Int(v)=>TypedValueKind::Int(*v), ExprKind::Bool(v)=>TypedValueKind::Bool(*v),
        ExprKind::String(v)=>TypedValueKind::String(v.clone()), ExprKind::Name(v)=>TypedValueKind::Name(v.clone()),
        ExprKind::Group(e)=>return lower_expr(e,types,locals,errors),
        ExprKind::List(xs)=>TypedValueKind::List(xs.iter().filter_map(|e|lower_expr(e,types,locals,errors)).collect()),
        ExprKind::Index{collection,index}=>TypedValueKind::Index(Box::new(lower_expr(collection,types,locals,errors)?),Box::new(lower_expr(index,types,locals,errors)?)),
        ExprKind::Binary{op,left,right}=>TypedValueKind::Binary{op:*op,left:Box::new(lower_expr(left,types,locals,errors)?),right:Box::new(lower_expr(right,types,locals,errors)?)},
        ExprKind::Call{callee,args}=>{
            let ExprKind::Name(name)=&callee.kind else { errors.push("dynamic call in typed IR".into()); return None; };
            TypedValueKind::Call{callee:name.clone(),args:args.iter().filter_map(|e|lower_expr(e,types,locals,errors)).collect()}
        }
        ExprKind::If{condition,..}=>return lower_expr(condition,types,locals,errors),
    };
    Some(TypedValue{ty,span:expr.span,kind})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lowers_typed_function() {
        let m=crate::parse("module x\nfn main(a: Int) -> Int\n  a + 1\n").unwrap();
        let ir=lower(&m).unwrap();
        assert_eq!(ir.functions[0].return_type,TypeKind::Int);
        assert!(matches!(ir.functions[0].blocks[0].ops[0],TypedIrOp::Expr(_)));
    }
}
