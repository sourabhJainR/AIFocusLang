use std::collections::{BTreeMap, HashMap};

use crate::TypeKind;
use crate::ir::{IrFunction, IrModule, IrOp, IrValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeInstr {
    PushInt(i64),
    PushBool(bool),
    PushString(String),
    PushList(usize),
    Index,
    Load(String),
    Store(String),
    StoreIndex(String),
    Add,
    Sub,
    Mul,
    Div,
    Equal,
    JumpIfFalse(usize),
    Jump(usize),
    Return,
    Pop,
    Call { callee: String, argc: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Deterministic collection of compiled Ardisa functions.
pub struct NativeProgram {
    pub functions: BTreeMap<String, NativeFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFunction {
    pub params: Vec<String>,
    pub code: Vec<NativeInstr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Native values are intentionally dependency-free so the native compiler can bootstrap incrementally.
pub enum NativeValue {
    Int(i64),
    Bool(bool),
    String(String),
    List(Vec<NativeValue>),
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeError {
    Unsupported(String),
    InvalidProgram(String),
    Type(String),
}

/// Compile the first function for the legacy single-function API.
pub fn compile(module: &IrModule) -> Result<Vec<NativeInstr>, NativeError> {
    let function = module
        .functions
        .first()
        .ok_or_else(|| NativeError::InvalidProgram("module has no functions".into()))?;
    compile_function(function)
}

pub fn compile_program(module: &IrModule) -> Result<NativeProgram, NativeError> {
    let mut functions = BTreeMap::new();
    for function in &module.functions {
        functions.insert(
            function.name.clone(),
            NativeFunction {
                params: function
                    .params
                    .iter()
                    .map(|(name, _)| name.clone())
                    .collect(),
                code: compile_function(function)?,
            },
        );
    }
    Ok(NativeProgram { functions })
}

pub fn compile_function(function: &IrFunction) -> Result<Vec<NativeInstr>, NativeError> {
    if function
        .params
        .iter()
        .any(|(_, ty)| !matches!(ty, TypeKind::Int | TypeKind::Bool | TypeKind::String))
    {
        return Err(NativeError::Unsupported(
            "native backend currently supports Int and Bool parameters only".into(),
        ));
    }
    let mut code = Vec::new();
    for (index, op) in function.ops.iter().enumerate() {
        let is_last_expression = index + 1 == function.ops.len()
            && matches!(op, IrOp::Expr(_))
            && function.return_type.is_some();
        if is_last_expression {
            if let IrOp::Expr(value) = op {
                emit_value(value, &mut code)?;
                code.push(NativeInstr::Return);
            }
        } else {
            emit_op(op, &mut code)?;
        }
    }
    if !matches!(code.last(), Some(NativeInstr::Return)) {
        code.push(NativeInstr::Return);
    }
    Ok(code)
}

fn emit_op(op: &IrOp, code: &mut Vec<NativeInstr>) -> Result<(), NativeError> {
    match op {
        IrOp::Let { name, value } | IrOp::Set { name, value } => {
            emit_value(value, code)?;
            code.push(NativeInstr::Store(name.clone()));
        }
        IrOp::SetIndex {
            collection,
            index,
            value,
        } => {
            let IrValue::Name(name) = collection else {
                return Err(NativeError::Unsupported(
                    "indexed assignment currently requires a named list binding".into(),
                ));
            };
            code.push(NativeInstr::Load(name.clone()));
            emit_value(index, code)?;
            emit_value(value, code)?;
            code.push(NativeInstr::StoreIndex(name.clone()));
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
        IrOp::While { condition, ops } => {
            let loop_start = code.len();
            emit_value(condition, code)?;
            let jump_if = code.len();
            code.push(NativeInstr::JumpIfFalse(usize::MAX));
            for op in ops {
                emit_op(op, code)?;
            }
            code.push(NativeInstr::Jump(loop_start));
            let end = code.len();
            code[jump_if] = NativeInstr::JumpIfFalse(end);
        }
        IrOp::Scope { .. } | IrOp::Spawn { .. } | IrOp::Join { .. } | IrOp::Cancel { .. } => {
            return Err(NativeError::Unsupported(
                "native backend does not yet execute concurrency operations".into(),
            ));
        }
    }
    Ok(())
}

fn emit_value(value: &IrValue, code: &mut Vec<NativeInstr>) -> Result<(), NativeError> {
    match value {
        IrValue::Int(value) => code.push(NativeInstr::PushInt(*value)),
        IrValue::String(value) => code.push(NativeInstr::PushString(value.clone())),
        IrValue::List(values) => {
            for value in values {
                emit_value(value, code)?;
            }
            code.push(NativeInstr::PushList(values.len()));
        }
        IrValue::Index { collection, index } => {
            emit_value(collection, code)?;
            emit_value(index, code)?;
            code.push(NativeInstr::Index);
        }
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
        IrValue::Call { callee, args } => {
            for arg in args {
                emit_value(arg, code)?;
            }
            code.push(NativeInstr::Call {
                callee: callee.clone(),
                argc: args.len(),
            });
        }
    }
    Ok(())
}

pub fn run(
    code: &[NativeInstr],
    args: &[(String, NativeValue)],
) -> Result<NativeValue, NativeError> {
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
            NativeInstr::PushString(value) => stack.push(NativeValue::String(value)),
            NativeInstr::PushList(len) => {
                if stack.len() < len {
                    return Err(NativeError::InvalidProgram(
                        "list has insufficient stack values".into(),
                    ));
                }
                let start = stack.len() - len;
                let values = stack.drain(start..).collect();
                stack.push(NativeValue::List(values));
            }
            NativeInstr::Index => {
                let index = pop_int(&mut stack)?;
                let collection = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("index from empty stack".into()))?;
                let NativeValue::List(values) = collection else {
                    return Err(NativeError::Type("indexing requires a list".into()));
                };
                let index = usize::try_from(index)
                    .map_err(|_| NativeError::Type("negative list index".into()))?;
                let value = values
                    .get(index)
                    .cloned()
                    .ok_or_else(|| NativeError::Type("list index out of bounds".into()))?;
                stack.push(value);
            }
            NativeInstr::Load(name) => {
                stack.push(locals.get(&name).cloned().ok_or_else(|| {
                    NativeError::InvalidProgram(format!("unknown local '{name}'"))
                })?)
            }
            NativeInstr::Store(name) => {
                let value = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("store from empty stack".into()))?;
                locals.insert(name, value);
            }
            NativeInstr::StoreIndex(name) => {
                let value = stack.pop().ok_or_else(|| {
                    NativeError::InvalidProgram("indexed store value missing".into())
                })?;
                let index = pop_int(&mut stack)?;
                let collection = stack.pop().ok_or_else(|| {
                    NativeError::InvalidProgram("indexed store collection missing".into())
                })?;
                let NativeValue::List(mut items) = collection else {
                    return Err(NativeError::Type(
                        "indexed assignment requires a list".into(),
                    ));
                };
                let index = usize::try_from(index)
                    .map_err(|_| NativeError::Type("negative list index".into()))?;
                let slot = items
                    .get_mut(index)
                    .ok_or_else(|| NativeError::Type("list index out of bounds".into()))?;
                *slot = value;
                locals.insert(name, NativeValue::List(items));
            }
            NativeInstr::StoreIndex(name) => {
                let value = stack.pop().ok_or_else(|| {
                    NativeError::InvalidProgram("indexed store value missing".into())
                })?;
                let index = pop_int(&mut stack)?;
                let collection = stack.pop().ok_or_else(|| {
                    NativeError::InvalidProgram("indexed store collection missing".into())
                })?;
                let NativeValue::List(mut items) = collection else {
                    return Err(NativeError::Type(
                    "indexed assignment requires a list".into(),
                ));
                };
                let index = usize::try_from(index)
                    .map_err(|_| NativeError::Type("negative list index".into()))?;
                let slot = items
                    .get_mut(index)
                    .ok_or_else(|| NativeError::Type("list index out of bounds".into()))?;
                *slot = value;
                locals.insert(name, NativeValue::List(items));
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
                let right = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("empty stack".into()))?;
                let left = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("empty stack".into()))?;
                stack.push(NativeValue::Bool(left == right));
            }
            NativeInstr::JumpIfFalse(target) => {
                let value = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("empty condition stack".into()))?;
                if value != NativeValue::Bool(true) {
                    pc = target;
                }
            }
            NativeInstr::Jump(target) => pc = target,
            NativeInstr::Call { .. } => {
                return Err(NativeError::Unsupported(
                    "direct run does not support function calls; use run_program".into(),
                ));
            }
            NativeInstr::Return => return Ok(stack.pop().unwrap_or(NativeValue::Unit)),
            NativeInstr::Pop => {
                stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("pop from empty stack".into()))?;
            }
        }
    }
    Err(NativeError::InvalidProgram(
        "program terminated without return".into(),
    ))
}

pub fn run_program(
    program: &NativeProgram,
    entry: &str,
    args: &[NativeValue],
) -> Result<NativeValue, NativeError> {
    let function = program
        .functions
        .get(entry)
        .ok_or_else(|| NativeError::InvalidProgram(format!("unknown function '{entry}'")))?;
    run_function(program, function, args)
}

fn run_function(
    program: &NativeProgram,
    function: &NativeFunction,
    args: &[NativeValue],
) -> Result<NativeValue, NativeError> {
    if args.len() != function.params.len() {
        return Err(NativeError::InvalidProgram(format!(
            "function expects {} argument(s), got {}",
            function.params.len(),
            args.len()
        )));
    }
    let mut pc = 0usize;
    let mut stack = Vec::new();
    let mut locals = HashMap::new();
    for (name, value) in function.params.iter().zip(args.iter()) {
        locals.insert(name.clone(), value.clone());
    }

    while pc < function.code.len() {
        let instr = function.code[pc].clone();
        pc += 1;
        match instr {
            NativeInstr::Call { callee, argc } => {
                if stack.len() < argc {
                    return Err(NativeError::InvalidProgram(
                        "call has fewer stack arguments than declared".into(),
                    ));
                }
                let start = stack.len() - argc;
                let call_args = stack.split_off(start);
                let callee = program.functions.get(&callee).ok_or_else(|| {
                    NativeError::InvalidProgram(format!("unknown function '{callee}'"))
                })?;
                let value = run_function(program, callee, &call_args)?;
                stack.push(value);
            }
            NativeInstr::PushInt(value) => stack.push(NativeValue::Int(value)),
            NativeInstr::PushBool(value) => stack.push(NativeValue::Bool(value)),
            NativeInstr::PushString(value) => stack.push(NativeValue::String(value)),
            NativeInstr::PushList(len) => {
                if stack.len() < len {
                    return Err(NativeError::InvalidProgram(
                        "list has insufficient stack values".into(),
                    ));
                }
                let start = stack.len() - len;
                let values = stack.drain(start..).collect();
                stack.push(NativeValue::List(values));
            }
            NativeInstr::Index => {
                let index = pop_int(&mut stack)?;
                let collection = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("index from empty stack".into()))?;
                let NativeValue::List(values) = collection else {
                    return Err(NativeError::Type("indexing requires a list".into()));
                };
                let index = usize::try_from(index)
                    .map_err(|_| NativeError::Type("negative list index".into()))?;
                let value = values
                    .get(index)
                    .cloned()
                    .ok_or_else(|| NativeError::Type("list index out of bounds".into()))?;
                stack.push(value);
            }
            NativeInstr::Load(name) => {
                stack.push(locals.get(&name).cloned().ok_or_else(|| {
                    NativeError::InvalidProgram(format!("unknown local '{name}'"))
                })?)
            }
            NativeInstr::Store(name) => {
                let value = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("store from empty stack".into()))?;
                locals.insert(name, value);
            }
            NativeInstr::Add => {
                let right = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("empty stack".into()))?;
                let left = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("empty stack".into()))?;
                stack.push(add_values(left, right)?);
            }
            NativeInstr::Sub | NativeInstr::Mul | NativeInstr::Div => {
                let right = pop_int(&mut stack)?;
                let left = pop_int(&mut stack)?;
                let value = match instr {
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
                let right = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("empty stack".into()))?;
                let left = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("empty stack".into()))?;
                stack.push(NativeValue::Bool(left == right));
            }
            NativeInstr::JumpIfFalse(target) => {
                let value = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("empty condition stack".into()))?;
                if value != NativeValue::Bool(true) {
                    pc = target;
                }
            }
            NativeInstr::Jump(target) => pc = target,
            NativeInstr::Return => return Ok(stack.pop().unwrap_or(NativeValue::Unit)),
            NativeInstr::Pop => {
                stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("pop from empty stack".into()))?;
            }
        }
    }
    Err(NativeError::InvalidProgram(
        "program terminated without return".into(),
    ))
}

fn add_values(left: NativeValue, right: NativeValue) -> Result<NativeValue, NativeError> {
    match (left, right) {
        (NativeValue::Int(left), NativeValue::Int(right)) => Ok(NativeValue::Int(left + right)),
        (NativeValue::String(left), NativeValue::String(right)) => {
            Ok(NativeValue::String(format!("{left}{right}")))
        }
        _ => Err(NativeError::Type(
            "String + String or Int + Int required".into(),
        )),
    }
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
        let module = crate::parse(
            "module x
fn main(a: Int, b: Int) -> Int
  a + b * 2
",
        )
        .unwrap();
        crate::sema::check(&module).unwrap();
        let ir = crate::ir::lower(&module);
        let code = compile(&ir).unwrap();
        let result = run(
            &code,
            &[
                (String::from("a"), NativeValue::Int(3)),
                (String::from("b"), NativeValue::Int(4)),
            ],
        )
        .unwrap();
        assert_eq!(result, NativeValue::Int(11));
    }

    #[test]
    fn compiles_and_runs_conditionals_without_rust() {
        let module = crate::parse(
            "module x
fn main(a: Int) -> Int
  if a == 0
    return 1
  else
    return 2
",
        )
        .unwrap();
        let ir = crate::ir::lower(&module);
        let code = compile(&ir).unwrap();
        let result = run(&code, &[(String::from("a"), NativeValue::Int(0))]).unwrap();
        assert_eq!(result, NativeValue::Int(1));
    }

    #[test]
    fn compiles_and_runs_mutable_assignments() {
        let module = crate::parse(
            "module x
fn main(a: Int) -> Int
  let value = a
  set value = value + 2
  value
",
        )
        .unwrap();
        crate::sema::check(&module).unwrap();
        let ir = crate::ir::lower(&module);
        let program = compile_program(&ir).unwrap();
        let result = run_program(&program, "main", &[NativeValue::Int(5)]).unwrap();
        assert_eq!(result, NativeValue::Int(7));
    }

    #[test]
    fn compiles_and_runs_while_loops() {
        let module = crate::parse(
            "module x
fn main(a: Int) -> Int
  while a == 0
    return 7
  return 9
",
        )
        .unwrap();
        crate::sema::check(&module).unwrap();
        let ir = crate::ir::lower(&module);
        let program = compile_program(&ir).unwrap();
        let result = run_program(&program, "main", &[NativeValue::Int(0)]).unwrap();
        assert_eq!(result, NativeValue::Int(7));
    }

    #[test]
    fn compiles_and_runs_string_concatenation() {
        let module = crate::parse(
            r#"module x
fn main() -> String
  "hello " + "ardisa"
"#,
        )
        .unwrap();
        crate::sema::check(&module).unwrap();
        let ir = crate::ir::lower(&module);
        let program = compile_program(&ir).unwrap();
        let result = run_program(&program, "main", &[]).unwrap();
        assert_eq!(result, NativeValue::String("hello ardisa".into()));
    }

    #[test]
    fn compiles_and_runs_function_calls() {
        let module = crate::parse(
            "module x
fn double(a: Int) -> Int
  a * 2
fn main(a: Int) -> Int
  double(a) + 1
",
        )
        .unwrap();
        crate::sema::check(&module).unwrap();
        let ir = crate::ir::lower(&module);
        let program = compile_program(&ir).unwrap();
        let result = run_program(&program, "main", &[NativeValue::Int(3)]).unwrap();
        assert_eq!(result, NativeValue::Int(7));
    }

    #[test]
    fn compiles_and_runs_recursive_calls() {
        let module = crate::parse(
            "module x
fn fact(n: Int) -> Int
  if n == 0
    return 1
  else
    return n * fact(n - 1)
",
        )
        .unwrap();
        crate::sema::check(&module).unwrap();
        let ir = crate::ir::lower(&module);
        let program = compile_program(&ir).unwrap();
        let result = run_program(&program, "fact", &[NativeValue::Int(5)]).unwrap();
        assert_eq!(result, NativeValue::Int(120));
    }
}
