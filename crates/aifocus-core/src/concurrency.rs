use std::collections::{BTreeMap, HashMap};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};

use crate::{Block, ExprKind, Item, Module, StmtKind, source::Diagnostic};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskTerminal {
    Joined,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSpec {
    pub name: String,
    pub callee: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeReport {
    pub tasks: Vec<TaskSpec>,
}

pub fn analyze(module: &Module) -> Result<Vec<ScopeReport>, Vec<String>> {
    analyze_with_diagnostics(module).map_err(|errors| errors.into_iter().map(|error| error.message).collect())
}

pub fn analyze_with_diagnostics(module: &Module) -> Result<Vec<ScopeReport>, Vec<Diagnostic>> {
    let mut reports = Vec::new();
    let mut errors = Vec::new();
    for item in &module.items {
        let Item::Function(function) = item;
        analyze_block_with_diagnostics(&function.body, &mut reports, &mut errors);
    }
    if errors.is_empty() {
        Ok(reports)
    } else {
        Err(errors)
    }
}
fn analyze_block(block: &Block, reports: &mut Vec<ScopeReport>, errors: &mut Vec<String>) {
    let mut diagnostics = Vec::new();
    analyze_block_with_diagnostics(block, reports, &mut diagnostics);
    errors.extend(diagnostics.into_iter().map(|error| error.message));
}

fn analyze_block_with_diagnostics(
    block: &Block,
    reports: &mut Vec<ScopeReport>,
    errors: &mut Vec<Diagnostic>,
) {
    for stmt in &block.stmts {
        if let StmtKind::Scope { body } = &stmt.kind {
            let mut tasks = HashMap::<String, (TaskState, crate::source::Span)>::new();
            let mut report = ScopeReport { tasks: Vec::new() };
            for child in &body.stmts {
                match &child.kind {
                    StmtKind::Spawn { name, call } => {
                        if tasks.insert(name.clone(), (TaskState::Running, child.span)).is_some() {
                            errors.push(Diagnostic::error(
                                "AIF501",
                                format!("duplicate task '{name}' in scope"),
                                Some(child.span),
                            ));
                            continue;
                        }
                        let callee = match &call.kind {
                            ExprKind::Call { callee, .. } => match &callee.kind {
                                ExprKind::Name(name) => name.clone(),
                                _ => "<dynamic>".into(),
                            },
                            _ => "<invalid>".into(),
                        };
                        report.tasks.push(TaskSpec {
                            name: name.clone(),
                            callee,
                        });
                    }
                    StmtKind::Join { name } | StmtKind::Cancel { name } => {
                        match tasks.get(name).copied() {
                            Some((TaskState::Running, _)) => {
                                let next = if matches!(child.kind, StmtKind::Join { .. }) {
                                    TaskState::Joined
                                } else {
                                    TaskState::Cancelled
                                };
                                tasks.insert(name.clone(), (next, child.span));
                            }
                            Some(_) => errors.push(Diagnostic::error(
                                "AIF504",
                                format!("task '{name}' is already terminal"),
                                Some(child.span),
                            )),
                            None => errors.push(Diagnostic::error(
                                "AIF502",
                                format!("unknown task '{name}' in scope"),
                                Some(child.span),
                            )),
                        }
                    }
                    StmtKind::Scope { .. } => {
                        analyze_block_with_diagnostics(child_block(child), reports, errors)
                    }
                    _ => {}
                }
            }
            for (name, (state, span)) in tasks {
                if state == TaskState::Running {
                    errors.push(Diagnostic::error(
                        "AIF503",
                        format!("task '{name}' must be joined or cancelled before scope exit"),
                        Some(span),
                    ));
                }
            }
            reports.push(report);
            analyze_block_with_diagnostics(body, reports, errors);
        }
    }
}

