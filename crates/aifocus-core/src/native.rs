use std::collections::HashMap;

use crate::ir::{IrFunction, IrModule, IrOp, IrValue};
use crate::TypeKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeInstr {
    PushInt(i64),
    PushBool(bool),
    Load(String),
    Store(String),
    Add,
    Sub,
    Mul,
    Div,
    Equal,
    JumpIfFalse(usize),
    Jump(usize),
    Return,
    Pop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeValue {
    Int(i64),
    Bool(bool),
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeError {
    Unsupported(String),
    InvalidProgram(String),
    Type(String),
}

pub fn compile(module: &IrModule) -> Result<Vec<NativeInstr>, NativeError> {
    let function = module
        .functions
        .first()
        .ok_or_else(|| NativeError::InvalidProgram("module has no functions".into()))?;
    compile_function(function)
}

pub fn compile_function(function: &IrFunction) -> Result<Vec<NativeInstr>, NativeError> {
    if function
        .params
        .iter()
        .any(|(_, ty)| !matches!(ty, TypeKind::Int | TypeKind::Bool))
    {
        return Err(NativeError::Unsupported(
            "native backend currently supports Int and Bool parameters only".into(),
        ));
    }
    let mut code = Vec::new();
    for op in &function.ops {
        emit_op(op, &mut code)?;
    }
    if !matches!(code.last(), Some(NativeInstr::Return)) {
        code.push(NativeInstr::Return);
    }
    Ok(code)
}

fn emit_op(op: &IrOp, code: &mut Vec<NativeInstr>) -> Result<(), NativeError> {
    match op {
        IrOp::Let { name, value } => {
            emit_value(value, code)?;
            code.push(NativeInstr::Store(name.clone()));
        }
        IrOp::Return(value) => {
            if let Some(value) = value {
                emit_value(value, code)?;
            } else {
                code.push(NativeInstr::PushInt(0));
            }
            code.push(NativeInstr::Return);
        }
        IrOp::Expr(value) => {
            emit_value(value, code)?;
            code.push(NativeInstr::Pop);
        }
        IrOp::Scope { .. }
        | IrOp::Spawn { .. }
        | IrOp::Join { .. }
        | IrOp::Cancel { .. } => {
            return Err(NativeError::Unsupported(
                "native backend does not yet execute concurrency operations".into(),
            ))
        }
    }
    Ok(())
}

fn emit_value(value: &IrValue, code: &mut Vec<NativeInstr>) -> Result<(), NativeError> {
    match value {
        IrValue::Int(value) => code.push(NativeInstr::PushInt(*value)),
        IrValue::Bool(value) => code.push(NativeInstr::PushBool(*value)),
        IrValue::Name(name) => code.push(NativeInstr::Load(name.clone())),
        IrValue::Binary { op, left, right } => {
            emit_value(left, code)?;
            emit_value(right, code)?;
            code.push(match op {
                crate::BinaryOp::Add => NativeInstr::Add,
                crate::BinaryOp::Sub => NativeInstr::Sub,
                crate::BinaryOp::Mul => NativeInstr::Mul,
                crate::BinaryOp::Div => NativeInstr::Div,
                crate::BinaryOp::Equal => NativeInstr::Equal,
            });
        }
        IrValue::If {
            condition,
            then_ops,
            else_ops,
        } => {
            emit_value(condition, code)?;
            let jump_if = code.len();
            code.push(NativeInstr::JumpIfFalse(usize::MAX));
            for op in then_ops {
                emit_op(op, code)?;
            }
            let jump_end = code.len();
            code.push(NativeInstr::Jump(usize::MAX));
            let else_start = code.len();
            for op in else_ops {
                emit_op(op, code)?;
            }
            let end = code.len();
            code[jump_if] = NativeInstr::JumpIfFalse(else_start);
            code[jump_end] = NativeInstr::Jump(end);
        }
        IrValue::String(_) | IrValue::Call { .. } => {
            return Err(NativeError::Unsupported(
                "native backend currently supports literals, names, arithmetic, equality, and if".into(),
            ))
        }
    }
    Ok(())
}

pub fn run(code: &[NativeInstr], args: &[(String, NativeValue)]) -> Result<NativeValue, NativeError> {
    let mut pc = 0usize;
    let mut stack = Vec::new();
    let mut locals = HashMap::new();
    for (name, value) in args {
        locals.insert(name.clone(), value.clone());
    }

    while pc < code.len() {
        let instr = code[pc].clone();
        pc += 1;
        match instr {
            NativeInstr::PushInt(value) => stack.push(NativeValue::Int(value)),
            NativeInstr::PushBool(value) => stack.push(NativeValue::Bool(value)),
            NativeInstr::Load(name) => stack.push(
                locals
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| NativeError::InvalidProgram(format!("unknown local '{name}'")))?,
            ),
            NativeInstr::Store(name) => {
                let value = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("store from empty stack".into()))?;
                locals.insert(name, value);
            }
            NativeInstr::Add | NativeInstr::Sub | NativeInstr::Mul | NativeInstr::Div => {
                let right = pop_int(&mut stack)?;
                let left = pop_int(&mut stack)?;
                let value = match code[pc - 1] {
                    NativeInstr::Add => left + right,
                    NativeInstr::Sub => left - right,
                    NativeInstr::Mul => left * right,
                    NativeInstr::Div => {
                        if right == 0 {
                            return Err(NativeError::Type("division by zero".into()));
                        }
                        left / right
                    }
                    _ => unreachable!(),
                };
                stack.push(NativeValue::Int(value));
            }
            NativeInstr::Equal => {
                let right = stack.pop().ok_or_else(|| NativeError::InvalidProgram("empty stack".into()))?;
                let left = stack.pop().ok_or_else(|| NativeError::InvalidProgram("empty stack".into()))?;
                stack.push(NativeValue::Bool(left == right));
            }
            NativeInstr::JumpIfFalse(target) => {
                let value = stack.pop().ok_or_else(|| NativeError::InvalidProgram("empty condition stack".into()))?;
                if value != NativeValue::Bool(true) {
                    pc = target;
                }
            }
            NativeInstr::Jump(target) => pc = target,
            NativeInstr::Return => return Ok(stack.pop().unwrap_or(NativeValue::Unit)),
            NativeInstr::Pop => {
                stack.pop().ok_or_else(|| NativeError::InvalidProgram("pop from empty stack".into()))?;
            }
        }
    }
    Err(NativeError::InvalidProgram("program terminated without return".into()))
}

fn pop_int(stack: &mut Vec<NativeValue>) -> Result<i64, NativeError> {
    match stack.pop() {
        Some(NativeValue::Int(value)) => Ok(value),
        _ => Err(NativeError::Type("expected Int value".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_and_runs_arithmetic_without_rust() {
        let module = crate::parse("module x\nfn main(a: Int, b: Int) -> Int\n  a + b * 2\n").unwrap();
        crate::sema::check(&module).unwrap();
        let ir = crate::ir::lower(&module);
        let code = compile(&ir).unwrap();
        let result = run(
            &code,
            &[(String::from("a"), NativeValue::Int(3)), (String::from("b"), NativeValue::Int(4))],
        )
        .unwrap();
        assert_eq!(result, NativeValue::Int(11));
    }

    #[test]
    fn compiles_and_runs_conditionals_without_rust() {
        let module = crate::parse(
            "module x\nfn main(a: Int) -> Int\n  if a == 0\n    return 1\n  else\n    return 2\n",
        )
        .unwrap();
        let ir = crate::ir::lower(&module);
        let code = compile(&ir).unwrap();
        let result = run(&code, &[(String::from("a"), NativeValue::Int(0))]).unwrap();
        assert_eq!(result, NativeValue::Int(1));
    }

    #[test]
    fn rejects_unsupported_calls() {
        let module = crate::parse(
            "module x\nfn main() -> Int\n  helper()\n",
        )
        .unwrap();
        let ir = crate::ir::lower(&module);
        assert!(matches!(compile(&ir), Err(NativeError::Unsupported(_))));
    }
}
