use std::collections::{BTreeMap, HashMap};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};

use crate::TypeKind;
use crate::ir::{IrFunction, IrModule, IrOp, IrValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeInstr {
    PushInt(i64),
    PushBool(bool),
    PushUnit,
    PushString(String),
    PushList(usize),
    Index,
    Len,
    Append(String),
    MakeOk,
    Chr,
    MakeErr,
    Unwrap,
    Load(String),
    Store(String),
    StoreIndex(String),
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    JumpIfFalse(usize),
    Jump(usize),
    Return,
    Pop,
    Call {
        callee: String,
        argc: usize,
    },
    ScopeStart,
    ScopeEnd,
    Spawn {
        name: String,
        callee: String,
        argc: usize,
    },
    Join {
        name: String,
    },
    Cancel {
        name: String,
    },
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
    ResultOk(Box<NativeValue>),
    ResultErr(Box<NativeValue>),
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeError {
    Unsupported(String),
    Cancelled(String),
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
    if function.params.iter().any(|(_, ty)| !is_native_type(ty)) {
        return Err(NativeError::Unsupported(
            "native backend does not support this parameter type".into(),
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

fn is_native_type(ty: &TypeKind) -> bool {
    match ty {
        TypeKind::Int | TypeKind::Bool | TypeKind::String | TypeKind::Unit => true,
        TypeKind::List(element) => is_native_type(&element.kind),
        TypeKind::Result(ok, err) => is_native_type(&ok.kind) && is_native_type(&err.kind),
        TypeKind::Named(_) | TypeKind::Generic(_, _) => false,
    }
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
        IrOp::Scope { ops } => {
            code.push(NativeInstr::ScopeStart);
            for op in ops {
                emit_op(op, code)?;
            }
            code.push(NativeInstr::ScopeEnd);
        }
        IrOp::Spawn { name, call } => {
            let IrValue::Call { callee, args } = call else {
                return Err(NativeError::Unsupported(
                    "spawn requires a function call".into(),
                ));
            };
            for arg in args {
                emit_value(arg, code)?;
            }
            code.push(NativeInstr::Spawn {
                name: name.clone(),
                callee: callee.clone(),
                argc: args.len(),
            });
        }
        IrOp::Join { name } => code.push(NativeInstr::Join { name: name.clone() }),
        IrOp::Cancel { name } => code.push(NativeInstr::Cancel { name: name.clone() }),
    }
    Ok(())
}

fn emit_branch(ops: &[IrOp], code: &mut Vec<NativeInstr>) -> Result<(), NativeError> {
    if ops.is_empty() {
        code.push(NativeInstr::PushUnit);
        return Ok(());
    }
    for (index, op) in ops.iter().enumerate() {
        let is_last = index + 1 == ops.len();
        if is_last {
            match op {
                IrOp::Expr(value) => emit_value(value, code)?,
                IrOp::Return(_) => emit_op(op, code)?,
                _ => {
                    emit_op(op, code)?;
                    code.push(NativeInstr::PushUnit);
                }
            }
        } else {
            emit_op(op, code)?;
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
                crate::BinaryOp::Mod => NativeInstr::Mod,
                crate::BinaryOp::Equal => NativeInstr::Equal,
                crate::BinaryOp::NotEqual => NativeInstr::NotEqual,
                crate::BinaryOp::Less => NativeInstr::Less,
                crate::BinaryOp::LessEqual => NativeInstr::LessEqual,
                crate::BinaryOp::Greater => NativeInstr::Greater,
                crate::BinaryOp::GreaterEqual => NativeInstr::GreaterEqual,
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
            emit_branch(then_ops, code)?;
            let jump_end = code.len();
            code.push(NativeInstr::Jump(usize::MAX));
            let else_start = code.len();
            emit_branch(else_ops, code)?;
            let end = code.len();
            code[jump_if] = NativeInstr::JumpIfFalse(else_start);
            code[jump_end] = NativeInstr::Jump(end);
        }
        IrValue::Call { callee, args } => {
            for arg in args {
                emit_value(arg, code)?;
            }
            if callee == "chr" && args.len() == 1 {
                code.push(NativeInstr::Chr);
            } else if callee == "ok" && args.len() == 1 {
                code.push(NativeInstr::MakeOk);
            } else if callee == "err" && args.len() == 1 {
                code.push(NativeInstr::MakeErr);
            } else if callee == "unwrap" && args.len() == 1 {
                code.push(NativeInstr::Unwrap);
            } else if callee == "len" && args.len() == 1 {
                code.push(NativeInstr::Len);
            } else if callee == "push" && args.len() == 2 {
                let crate::ir::IrValue::Name(name) = &args[0] else {
                    return Err(NativeError::Unsupported(
                        "push currently requires a named list binding".into(),
                    ));
                };
                emit_value(&args[1], code)?;
                code.push(NativeInstr::Append(name.clone()));
            } else {
                code.push(NativeInstr::Call {
                    callee: callee.clone(),
                    argc: args.len(),
                });
            }
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
            NativeInstr::PushUnit => stack.push(NativeValue::Unit),
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
                let index = usize::try_from(index)
                    .map_err(|_| NativeError::Type("negative index".into()))?;
                match collection {
                    NativeValue::List(values) => {
                        let value = values
                            .get(index)
                            .cloned()
                            .ok_or_else(|| NativeError::Type("list index out of bounds".into()))?;
                        stack.push(value);
                    }
                    NativeValue::String(value) => {
                        let byte = value.as_bytes().get(index).copied().ok_or_else(|| {
                            NativeError::Type("string index out of bounds".into())
                        })?;
                        stack.push(NativeValue::Int(i64::from(byte)));
                    }
                    _ => {
                        return Err(NativeError::Type(
                            "indexing requires a list or String".into(),
                        ));
                    }
                }
            }
            NativeInstr::Len => {
                let value = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("len from empty stack".into()))?;
                let length = match value {
                    NativeValue::String(value) => value.len(),
                    NativeValue::List(values) => values.len(),
                    _ => {
                        return Err(NativeError::Type("len requires String or List".into()));
                    }
                };
                stack.push(NativeValue::Int(length as i64));
            }
            NativeInstr::Append(name) => {
                let value = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("push value missing".into()))?;
                let Some(NativeValue::List(items)) = locals.get_mut(&name) else {
                    return Err(NativeError::Type("push requires a List binding".into()));
                };
                items.push(value);
                stack.push(NativeValue::Unit);
            }
            NativeInstr::Chr => {
                let value = pop_int(&mut stack)?;
                let byte = u8::try_from(value)
                    .map_err(|_| NativeError::Type("chr requires a byte in 0..=255".into()))?;
                stack.push(NativeValue::String(char::from(byte).to_string()));
            }
            NativeInstr::MakeOk => {
                let value = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("ok value missing".into()))?;
                stack.push(NativeValue::ResultOk(Box::new(value)));
            }
            NativeInstr::MakeErr => {
                let value = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("err value missing".into()))?;
                stack.push(NativeValue::ResultErr(Box::new(value)));
            }
            NativeInstr::Unwrap => {
                let value = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("unwrap value missing".into()))?;
                match value {
                    NativeValue::ResultOk(value) => stack.push(*value),
                    NativeValue::ResultErr(_) => {
                        return Err(NativeError::Type("unwrap on Err".into()));
                    }
                    _ => return Err(NativeError::Type("unwrap requires Result".into())),
                }
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

            NativeInstr::Add
            | NativeInstr::Sub
            | NativeInstr::Mul
            | NativeInstr::Div
            | NativeInstr::Mod => {
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
                    NativeInstr::Mod => {
                        if right == 0 {
                            return Err(NativeError::Type("modulo by zero".into()));
                        }
                        left % right
                    }
                    _ => unreachable!(),
                };
                stack.push(NativeValue::Int(value));
            }
            NativeInstr::Equal
            | NativeInstr::NotEqual
            | NativeInstr::Less
            | NativeInstr::LessEqual
            | NativeInstr::Greater
            | NativeInstr::GreaterEqual => {
                let right = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("empty stack".into()))?;
                let left = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("empty stack".into()))?;
                let result = match instr {
                    NativeInstr::Equal => left == right,
                    NativeInstr::NotEqual => left != right,
                    NativeInstr::Less => compare_ints(&left, &right, |a, b| a < b)?,
                    NativeInstr::LessEqual => compare_ints(&left, &right, |a, b| a <= b)?,
                    NativeInstr::Greater => compare_ints(&left, &right, |a, b| a > b)?,
                    NativeInstr::GreaterEqual => compare_ints(&left, &right, |a, b| a >= b)?,
                    _ => unreachable!(),
                };
                stack.push(NativeValue::Bool(result));
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
            NativeInstr::Call { .. }
            | NativeInstr::ScopeStart
            | NativeInstr::ScopeEnd
            | NativeInstr::Spawn { .. }
            | NativeInstr::Join { .. }
            | NativeInstr::Cancel { .. } => {
                return Err(NativeError::Unsupported(
                    "direct run does not support function calls or concurrency; use run_program"
                        .into(),
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

struct NativeTask {
    cancel: Arc<AtomicBool>,
    join: Option<JoinHandle<Result<NativeValue, NativeError>>>,
}

impl NativeTask {
    fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
}

impl Drop for NativeTask {
    fn drop(&mut self) {
        self.cancel();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
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
    run_function(program, function, args, None)
}

fn run_function(
    program: &NativeProgram,
    function: &NativeFunction,
    args: &[NativeValue],
    cancellation: Option<Arc<AtomicBool>>,
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
    let mut scopes: Vec<BTreeMap<String, NativeTask>> = Vec::new();
    for (name, value) in function.params.iter().zip(args.iter()) {
        locals.insert(name.clone(), value.clone());
    }

    while pc < function.code.len() {
        if cancellation
            .as_ref()
            .is_some_and(|token| token.load(Ordering::Acquire))
        {
            return Err(NativeError::Cancelled("task cancelled".into()));
        }
        let instr = function.code[pc].clone();
        pc += 1;
        match instr {
            NativeInstr::ScopeStart => scopes.push(BTreeMap::new()),
            NativeInstr::ScopeEnd => {
                let mut tasks = scopes.pop().ok_or_else(|| {
                    NativeError::InvalidProgram("scope end without scope start".into())
                })?;
                let names = tasks.keys().cloned().collect::<Vec<_>>();
                for name in names {
                    let mut task = tasks.remove(&name).expect("task disappeared");
                    let result = task
                        .join
                        .take()
                        .expect("task already joined")
                        .join()
                        .map_err(|_| NativeError::Unsupported(format!("task '{name}' panicked")))?;
                    if let Err(error) = result {
                        if matches!(error, NativeError::Cancelled(_)) {
                            continue;
                        }
                        for sibling in tasks.values() {
                            sibling.cancel();
                        }
                        return Err(error);
                    }
                }
            }
            NativeInstr::Spawn { name, callee, argc } => {
                let scope = scopes.last_mut().ok_or_else(|| {
                    NativeError::InvalidProgram("spawn must occur inside a scope".into())
                })?;
                if scope.contains_key(&name) {
                    return Err(NativeError::InvalidProgram(format!(
                        "duplicate task '{name}'"
                    )));
                }
                if stack.len() < argc {
                    return Err(NativeError::InvalidProgram(
                        "spawn has fewer stack arguments than declared".into(),
                    ));
                }
                let start = stack.len() - argc;
                let call_args = stack.split_off(start);
                let child_program = program.clone();
                let token = Arc::new(AtomicBool::new(false));
                let child_token = token.clone();
                let join = thread::spawn(move || {
                    let function = child_program.functions.get(&callee).ok_or_else(|| {
                        NativeError::InvalidProgram(format!("unknown function '{callee}'"))
                    })?;
                    run_function(&child_program, function, &call_args, Some(child_token))
                });
                scope.insert(
                    name,
                    NativeTask {
                        cancel: token,
                        join: Some(join),
                    },
                );
            }
            NativeInstr::Join { name } => {
                let scope = scopes.last_mut().ok_or_else(|| {
                    NativeError::InvalidProgram("join must occur inside a scope".into())
                })?;
                let mut task = scope
                    .remove(&name)
                    .ok_or_else(|| NativeError::InvalidProgram(format!("unknown task '{name}'")))?;
                let result = task
                    .join
                    .take()
                    .expect("task already joined")
                    .join()
                    .map_err(|_| NativeError::Unsupported(format!("task '{name}' panicked")))?;
                match result {
                    Ok(_) => stack.push(NativeValue::Unit),
                    Err(NativeError::Cancelled(_)) => stack.push(NativeValue::Unit),
                    Err(error) => {
                        for sibling in scope.values() {
                            sibling.cancel();
                        }
                        return Err(error);
                    }
                }
            }
            NativeInstr::Cancel { name } => {
                let scope = scopes.last_mut().ok_or_else(|| {
                    NativeError::InvalidProgram("cancel must occur inside a scope".into())
                })?;
                let task = scope
                    .get(&name)
                    .ok_or_else(|| NativeError::InvalidProgram(format!("unknown task '{name}'")))?;
                task.cancel();
            }
            NativeInstr::Call { callee, argc } => {
                if stack.len() < argc {
                    return Err(NativeError::InvalidProgram(
                        "call has fewer stack arguments than declared".into(),
                    ));
                }
                let start = stack.len() - argc;
                let call_args = stack.split_off(start);
                let callee_fn = program.functions.get(&callee).ok_or_else(|| {
                    NativeError::InvalidProgram(format!("unknown function '{callee}'"))
                })?;
                let value = run_function(program, callee_fn, &call_args, cancellation.clone())?;
                stack.push(value);
            }
            NativeInstr::PushInt(value) => stack.push(NativeValue::Int(value)),
            NativeInstr::PushBool(value) => stack.push(NativeValue::Bool(value)),
            NativeInstr::PushUnit => stack.push(NativeValue::Unit),
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
                let index = usize::try_from(index)
                    .map_err(|_| NativeError::Type("negative index".into()))?;
                match collection {
                    NativeValue::List(values) => stack.push(
                        values
                            .get(index)
                            .cloned()
                            .ok_or_else(|| NativeError::Type("list index out of bounds".into()))?,
                    ),
                    NativeValue::String(value) => stack.push(NativeValue::Int(i64::from(
                        value.as_bytes().get(index).copied().ok_or_else(|| {
                            NativeError::Type("string index out of bounds".into())
                        })?,
                    ))),
                    _ => {
                        return Err(NativeError::Type(
                            "indexing requires a list or String".into(),
                        ));
                    }
                }
            }
            NativeInstr::Len => {
                let value = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("len from empty stack".into()))?;
                let length = match value {
                    NativeValue::String(value) => value.len(),
                    NativeValue::List(values) => values.len(),
                    _ => return Err(NativeError::Type("len requires String or List".into())),
                };
                stack.push(NativeValue::Int(length as i64));
            }
            NativeInstr::Append(name) => {
                let value = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("push value missing".into()))?;
                let Some(NativeValue::List(items)) = locals.get_mut(&name) else {
                    return Err(NativeError::Type("push requires a List binding".into()));
                };
                items.push(value);
                stack.push(NativeValue::Unit);
            }
            NativeInstr::Chr => {
                let value = pop_int(&mut stack)?;
                let byte = u8::try_from(value)
                    .map_err(|_| NativeError::Type("chr requires a byte in 0..=255".into()))?;
                stack.push(NativeValue::String(char::from(byte).to_string()));
            }
            NativeInstr::MakeOk => {
                let value = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("ok value missing".into()))?;
                stack.push(NativeValue::ResultOk(Box::new(value)));
            }
            NativeInstr::MakeErr => {
                let value = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("err value missing".into()))?;
                stack.push(NativeValue::ResultErr(Box::new(value)));
            }
            NativeInstr::Unwrap => {
                match stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("unwrap value missing".into()))?
                {
                    NativeValue::ResultOk(value) => stack.push(*value),
                    NativeValue::ResultErr(_) => {
                        return Err(NativeError::Type("unwrap on Err".into()));
                    }
                    _ => return Err(NativeError::Type("unwrap requires Result".into())),
                }
            }
            NativeInstr::Load(name) => {
                stack.push(locals.get(&name).cloned().ok_or_else(|| {
                    NativeError::InvalidProgram(format!("unknown local '{name}'"))
                })?);
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
                *items
                    .get_mut(index)
                    .ok_or_else(|| NativeError::Type("list index out of bounds".into()))? = value;
                locals.insert(name, NativeValue::List(items));
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
            NativeInstr::Sub | NativeInstr::Mul | NativeInstr::Div | NativeInstr::Mod => {
                let right = pop_int(&mut stack)?;
                let left = pop_int(&mut stack)?;
                let value = match instr {
                    NativeInstr::Sub => left - right,
                    NativeInstr::Mul => left * right,
                    NativeInstr::Div => {
                        if right == 0 {
                            return Err(NativeError::Type("division by zero".into()));
                        } else {
                            left / right
                        }
                    }
                    NativeInstr::Mod => {
                        if right == 0 {
                            return Err(NativeError::Type("modulo by zero".into()));
                        } else {
                            left % right
                        }
                    }
                    _ => unreachable!(),
                };
                stack.push(NativeValue::Int(value));
            }
            NativeInstr::Equal
            | NativeInstr::NotEqual
            | NativeInstr::Less
            | NativeInstr::LessEqual
            | NativeInstr::Greater
            | NativeInstr::GreaterEqual => {
                let right = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("empty stack".into()))?;
                let left = stack
                    .pop()
                    .ok_or_else(|| NativeError::InvalidProgram("empty stack".into()))?;
                let result = match instr {
                    NativeInstr::Equal => left == right,
                    NativeInstr::NotEqual => left != right,
                    NativeInstr::Less => compare_ints(&left, &right, |a, b| a < b)?,
                    NativeInstr::LessEqual => compare_ints(&left, &right, |a, b| a <= b)?,
                    NativeInstr::Greater => compare_ints(&left, &right, |a, b| a > b)?,
                    NativeInstr::GreaterEqual => compare_ints(&left, &right, |a, b| a >= b)?,
                    _ => unreachable!(),
                };
                stack.push(NativeValue::Bool(result));
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

fn compare_ints(
    left: &NativeValue,
    right: &NativeValue,
    predicate: impl FnOnce(i64, i64) -> bool,
) -> Result<bool, NativeError> {
    let (NativeValue::Int(left), NativeValue::Int(right)) = (left, right) else {
        return Err(NativeError::Type(
            "ordering operators require Int operands".into(),
        ));
    };
    Ok(predicate(*left, *right))
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


/// Stable, dependency-free serialization for bootstrap artifacts.
///
/// The format is deliberately textual and line-oriented so an existing
/// Ardisa VM can consume a compiler artifact without invoking the Rust compiler.
pub const ARTIFACT_MAGIC: &str = "ARDISA-EXEC-V1";

pub fn encode_program(program: &NativeProgram) -> String {
    let mut out = String::from(ARTIFACT_MAGIC);
    out.push('\n');
    for (name, function) in &program.functions {
        out.push_str("FN|");
        out.push_str(&escape_artifact(name));
        out.push('|');
        out.push_str(&function.params.len().to_string());
        out.push('|');
        out.push_str(&function.params.iter().map(|p| escape_artifact(p)).collect::<Vec<_>>().join(","));
        out.push('\n');
        for instr in &function.code {
            out.push_str("I|");
            out.push_str(&encode_instr(instr));
            out.push('\n');
        }
        out.push_str("END\n");
    }
    out
}

pub fn decode_program(input: &str) -> Result<NativeProgram, NativeError> {
    let mut lines = input.lines();
    if lines.next() != Some(ARTIFACT_MAGIC) {
        return Err(NativeError::InvalidProgram("invalid Ardisa executable magic".into()));
    }
    let mut functions = BTreeMap::new();
    let mut current: Option<(String, Vec<String>, Vec<NativeInstr>)> = None;
    for line in lines {
        if let Some(rest) = line.strip_prefix("FN|") {
            if current.is_some() {
                return Err(NativeError::InvalidProgram("nested function in executable".into()));
            }
            let mut parts = rest.split('|');
            let name = unescape_artifact(parts.next().unwrap_or(""))?;
            let _count = parts.next().unwrap_or("0").parse::<usize>()
                .map_err(|_| NativeError::InvalidProgram("invalid parameter count".into()))?;
            let params = if let Some(raw) = parts.next() {
                if raw.is_empty() { Vec::new() } else {
                    raw.split(',').map(unescape_artifact).collect::<Result<Vec<_>, _>>()?
                }
            } else { Vec::new() };
            current = Some((name, params, Vec::new()));
        } else if line == "END" {
            let (name, params, code) = current.take()
                .ok_or_else(|| NativeError::InvalidProgram("function terminator without function".into()))?;
            functions.insert(name, NativeFunction { params, code });
        } else if let Some(rest) = line.strip_prefix("I|") {
            let (_, _, code) = current.as_mut()
                .ok_or_else(|| NativeError::InvalidProgram("instruction outside function".into()))?;
            code.push(decode_instr(rest)?);
        } else if !line.is_empty() {
            return Err(NativeError::InvalidProgram("unknown executable record".into()));
        }
    }
    if current.is_some() {
        return Err(NativeError::InvalidProgram("unterminated executable function".into()));
    }
    Ok(NativeProgram { functions })
}

fn escape_artifact(value: &str) -> String {
    value.replace('\\', "\\\\").replace('|', r"\p").replace(',', r"\c").replace('\n', r"\n")
}

fn unescape_artifact(value: &str) -> Result<String, NativeError> {
    let mut out = String::new();
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            out.push(match ch { 'p' => '|', 'c' => ',', 'n' => '\n', '\\' => '\\', other => other });
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            out.push(ch);
        }
    }
    if escaped { return Err(NativeError::InvalidProgram("trailing artifact escape".into())); }
    Ok(out)
}

fn encode_instr(instr: &NativeInstr) -> String {
    match instr {
        NativeInstr::PushInt(v) => format!("PushInt:{v}"),
        NativeInstr::PushBool(v) => format!("PushBool:{v}"),
        NativeInstr::PushUnit => "PushUnit".into(),
        NativeInstr::PushString(v) => format!("PushString:{}", escape_artifact(v)),
        NativeInstr::PushList(v) => format!("PushList:{v}"),
        NativeInstr::Index => "Index".into(), NativeInstr::Len => "Len".into(),
        NativeInstr::Append(v) => format!("Append:{}", escape_artifact(v)),
        NativeInstr::MakeOk => "MakeOk".into(), NativeInstr::Chr => "Chr".into(),
        NativeInstr::MakeErr => "MakeErr".into(), NativeInstr::Unwrap => "Unwrap".into(),
        NativeInstr::Load(v) => format!("Load:{}", escape_artifact(v)),
        NativeInstr::Store(v) => format!("Store:{}", escape_artifact(v)),
        NativeInstr::StoreIndex(v) => format!("StoreIndex:{}", escape_artifact(v)),
        NativeInstr::Add => "Add".into(), NativeInstr::Sub => "Sub".into(),
        NativeInstr::Mul => "Mul".into(), NativeInstr::Div => "Div".into(),
        NativeInstr::Mod => "Mod".into(), NativeInstr::Equal => "Equal".into(),
        NativeInstr::NotEqual => "NotEqual".into(), NativeInstr::Less => "Less".into(),
        NativeInstr::LessEqual => "LessEqual".into(), NativeInstr::Greater => "Greater".into(),
        NativeInstr::GreaterEqual => "GreaterEqual".into(),
        NativeInstr::JumpIfFalse(v) => format!("JumpIfFalse:{v}"),
        NativeInstr::Jump(v) => format!("Jump:{v}"), NativeInstr::Return => "Return".into(),
        NativeInstr::Pop => "Pop".into(),
        NativeInstr::Call { callee, argc } => format!("Call:{},{}", escape_artifact(callee), argc),
        NativeInstr::ScopeStart => "ScopeStart".into(), NativeInstr::ScopeEnd => "ScopeEnd".into(),
        NativeInstr::Spawn { name, callee, argc } => format!("Spawn:{},{},{}", escape_artifact(name), escape_artifact(callee), argc),
        NativeInstr::Join { name } => format!("Join:{}", escape_artifact(name)),
        NativeInstr::Cancel { name } => format!("Cancel:{}", escape_artifact(name)),
    }
}

fn decode_instr(s: &str) -> Result<NativeInstr, NativeError> {
    let mut p=s.splitn(2, ':'); let op=p.next().unwrap_or(""); let arg=p.next().unwrap_or("");
    let bad=||NativeError::InvalidProgram(format!("invalid instruction '{s}'"));
    let int=|v:&str|v.parse::<usize>().map_err(|_|bad());
    Ok(match op {
        "PushInt"=>NativeInstr::PushInt(arg.parse().map_err(|_|bad())?),
        "PushBool"=>NativeInstr::PushBool(arg=="true"),
        "PushUnit"=>NativeInstr::PushUnit,"Index"=>NativeInstr::Index,"Len"=>NativeInstr::Len,
        "PushString"=>NativeInstr::PushString(unescape_artifact(arg)?),
        "PushList"=>NativeInstr::PushList(int(arg)?),"Append"=>NativeInstr::Append(unescape_artifact(arg)?),
        "MakeOk"=>NativeInstr::MakeOk,"Chr"=>NativeInstr::Chr,"MakeErr"=>NativeInstr::MakeErr,"Unwrap"=>NativeInstr::Unwrap,
        "Load"=>NativeInstr::Load(unescape_artifact(arg)?),"Store"=>NativeInstr::Store(unescape_artifact(arg)?),
        "StoreIndex"=>NativeInstr::StoreIndex(unescape_artifact(arg)?),"Add"=>NativeInstr::Add,"Sub"=>NativeInstr::Sub,
        "Mul"=>NativeInstr::Mul,"Div"=>NativeInstr::Div,"Mod"=>NativeInstr::Mod,"Equal"=>NativeInstr::Equal,
        "NotEqual"=>NativeInstr::NotEqual,"Less"=>NativeInstr::Less,"LessEqual"=>NativeInstr::LessEqual,
        "Greater"=>NativeInstr::Greater,"GreaterEqual"=>NativeInstr::GreaterEqual,
        "JumpIfFalse"=>NativeInstr::JumpIfFalse(int(arg)?),"Jump"=>NativeInstr::Jump(int(arg)?),
        "Return"=>NativeInstr::Return,"Pop"=>NativeInstr::Pop,
        "Call"=>{let mut x=arg.split(','); NativeInstr::Call{callee:unescape_artifact(x.next().ok_or_else(bad)?)?,argc:int(x.next().ok_or_else(bad)?)?}},
        "ScopeStart"=>NativeInstr::ScopeStart,"ScopeEnd"=>NativeInstr::ScopeEnd,
        "Spawn"=>{let mut x=arg.split(','); NativeInstr::Spawn{name:unescape_artifact(x.next().ok_or_else(bad)?)?,callee:unescape_artifact(x.next().ok_or_else(bad)?)?,argc:int(x.next().ok_or_else(bad)?)?}},
        "Join"=>NativeInstr::Join{name:unescape_artifact(arg)?},"Cancel"=>NativeInstr::Cancel{name:unescape_artifact(arg)?},
        _=>return Err(bad()),
    })
}

fn pop_int(stack: &mut Vec<NativeValue>) -> Result<i64, NativeError> {
    match stack.pop() {
        Some(NativeValue::Int(value)) => Ok(value),
        _ => Err(NativeError::Type("expected Int value".into())),
    }
}

#[cfg(test)]
mod artifact_tests {
    use super::*;

    #[test]
    fn executable_artifact_round_trips_deterministically() {
        let module = crate::parse(
            r#"module artifact
fn main(a: String) -> String
  a + "!"
"#,
        ).unwrap();
        crate::sema::check(&module).unwrap();
        crate::ownership::infer(&module).unwrap();
        let program = compile_program(&crate::ir::lower(&module)).unwrap();
        let encoded = encode_program(&program);
        assert!(encoded.starts_with(ARTIFACT_MAGIC));
        let decoded = decode_program(&encoded).unwrap();
        assert_eq!(decoded, program);
        assert_eq!(encode_program(&decoded), encoded);
        assert_eq!(
            run_program(&decoded, "main", &[NativeValue::String("Ardisa".into())]).unwrap(),
            NativeValue::String("Ardisa!".into())
        );
    }

    #[test]
    fn malformed_executable_is_rejected() {
        assert!(matches!(
            decode_program("ARDISA-EXEC-V1\nI|Return\n"),
            Err(NativeError::InvalidProgram(_))
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_type_coverage_includes_aggregate_parameters() {
        assert!(is_native_type(&TypeKind::Int));
        assert!(is_native_type(&TypeKind::List(Box::new(crate::ast::Type {
            id: crate::NodeId(1),
            span: crate::source::Span::new(0, 0),
            kind: TypeKind::Int,
        }))));
        assert!(is_native_type(&TypeKind::Result(
            Box::new(crate::ast::Type {
                id: crate::NodeId(2),
                span: crate::source::Span::new(0, 0),
                kind: TypeKind::Int,
            }),
            Box::new(crate::ast::Type {
                id: crate::NodeId(3),
                span: crate::source::Span::new(0, 0),
                kind: TypeKind::String,
            }),
        )));
        assert!(!is_native_type(&TypeKind::Named("Custom".into())));
    }

    #[test]
    fn compiles_and_runs_aggregate_values_without_rust() {
        let module = crate::parse(
            "module x\nfn main(values: List<Int>) -> Int\n  len(values)\n",
        )
        .unwrap();
        crate::sema::check(&module).unwrap();
        let ir = crate::ir::lower(&module);
        let program = compile_program(&ir).unwrap();
        let result = run_program(
            &program,
            "main",
            &[NativeValue::List(vec![NativeValue::Int(1), NativeValue::Int(2)])],
        )
        .unwrap();
        assert_eq!(result, NativeValue::Int(2));
    }

    #[test]
    fn compiles_and_runs_result_values_without_rust() {
        let module = crate::parse(
            "module x\nfn main(value: Result<Int, String>) -> Int\n  unwrap(value)\n",
        )
        .unwrap();
        crate::sema::check(&module).unwrap();
        let ir = crate::ir::lower(&module);
        let program = compile_program(&ir).unwrap();
        let result = run_program(
            &program,
            "main",
            &[NativeValue::ResultOk(Box::new(NativeValue::Int(9)))],
        )
        .unwrap();
        assert_eq!(result, NativeValue::Int(9));
    }

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
    fn compiles_and_runs_statement_style_if() {
        let module = crate::parse(
            "module x
fn main(a: Int) -> Int
  let value = 0
  if a == 0
    set value = 1
  else
    set value = 2
  value
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
    fn executes_structured_scope_with_joined_child() {
        let module = crate::parse(
            "module x
fn worker(a: Int) -> Int
  a + 1
fn main() -> Int
  scope
    spawn worker_task = worker(4)
    join worker_task
  7
",
        )
        .unwrap();
        crate::sema::check(&module).unwrap();
        crate::concurrency::analyze(&module).unwrap();
        let program = compile_program(&crate::ir::lower(&module)).unwrap();
        assert_eq!(
            run_program(&program, "main", &[]).unwrap(),
            NativeValue::Int(7)
        );
    }

    #[test]
    fn executes_structured_scope_with_cancelled_child() {
        let module = crate::parse(
            "module x
fn worker() -> Int
  while true
    1
  return 0
fn main() -> Int
  scope
    spawn worker_task = worker()
    cancel worker_task
  9
",
        )
        .unwrap();
        crate::sema::check(&module).unwrap();
        crate::concurrency::analyze(&module).unwrap();
        let program = compile_program(&crate::ir::lower(&module)).unwrap();
        assert_eq!(
            run_program(&program, "main", &[]).unwrap(),
            NativeValue::Int(9)
        );
    }

    #[test]
    fn cancellation_is_a_normal_scope_terminal_state() {
        let module = crate::parse(
            "module x
fn worker() -> Int
  while true
    1
  return 0
fn main() -> Int
  scope
    spawn worker_task = worker()
    cancel worker_task
  9
",
        )
        .unwrap();
        crate::sema::check(&module).unwrap();
        crate::concurrency::analyze(&module).unwrap();
        let program = compile_program(&crate::ir::lower(&module)).unwrap();
        assert_eq!(
            run_program(&program, "main", &[]).unwrap(),
            NativeValue::Int(9)
        );
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
