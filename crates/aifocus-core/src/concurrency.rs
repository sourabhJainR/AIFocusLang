use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};

use crate::{Block, ExprKind, Item, Module, StmtKind};

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
    let mut reports = Vec::new();
    let mut errors = Vec::new();
    for item in &module.items {
        let Item::Function(function) = item;
        analyze_block(&function.body, &mut reports, &mut errors);
    }
    if errors.is_empty() {
        Ok(reports)
    } else {
        Err(errors)
    }
}

fn analyze_block(block: &Block, reports: &mut Vec<ScopeReport>, errors: &mut Vec<String>) {
    for stmt in &block.stmts {
        if let StmtKind::Scope { body } = &stmt.kind {
            let mut tasks = HashMap::<String, TaskState>::new();
            let mut report = ScopeReport { tasks: Vec::new() };
            for child in &body.stmts {
                match &child.kind {
                    StmtKind::Spawn { name, call } => {
                        if tasks.insert(name.clone(), TaskState::Running).is_some() {
                            errors.push(format!("AIF501: duplicate task '{name}' in scope"));
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
                    StmtKind::Join { name } => {
                        match tasks.get(name).copied() {
                            Some(TaskState::Running) => { tasks.insert(name.clone(), TaskState::Joined); }
                            Some(_) => errors.push(format!("AIF504: task '{name}' is already terminal")),
                            None => errors.push(format!("AIF502: unknown task '{name}' in scope")),
                        }
                    }
                    StmtKind::Cancel { name } => {
                        match tasks.get(name).copied() {
                            Some(TaskState::Running) => { tasks.insert(name.clone(), TaskState::Cancelled); }
                            Some(_) => errors.push(format!("AIF504: task '{name}' is already terminal")),
                            None => errors.push(format!("AIF502: unknown task '{name}' in scope")),
                        }
                    }
                    StmtKind::Scope { .. } => analyze_block(child_block(child), reports, errors),
                    _ => {}
                }
            }
            for (name, done) in tasks {
                if done == TaskState::Running {
                    errors.push(format!(
                        "AIF503: task '{name}' must be joined or cancelled before scope exit"
                    ));
                }
            }
            reports.push(report);
            analyze_block(body, reports, errors);
        }
    }
}

fn child_block(stmt: &crate::Stmt) -> &Block {
    match &stmt.kind {
        StmtKind::Scope { body } => body,
        _ => unreachable!(),
    }
}

#[derive(Debug, Clone)]
pub #[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskState {
    Running,
    Joined,
    Cancelled,
}

struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

pub struct TaskHandle<T> {
    token: CancellationToken,
    join: Option<JoinHandle<T>>,
}

impl<T: Send + 'static> TaskHandle<T> {
    pub fn cancel(&self) {
        self.token.cancelled.store(true, Ordering::Release);
    }

    pub fn join(mut self) -> thread::Result<T> {
        self.join.take().expect("task already joined").join()
    }

    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }
}

pub fn spawn<T, F>(task: F) -> TaskHandle<T>
where
    T: Send + 'static,
    F: FnOnce(CancellationToken) -> T + Send + 'static,
{
    let token = CancellationToken {
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let child_token = token.clone();
    let join = thread::spawn(move || task(child_token));
    TaskHandle {
        token,
        join: Some(join),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_joined_structured_scope() {
        let module = crate::parse(
            "module x\nfn main()\n  scope\n    spawn worker = work(1)\n    join worker\n",
        )
        .unwrap();
        let reports = analyze(&module).unwrap();
        assert_eq!(reports[0].tasks[0].name, "worker");
    }

    #[test]
    fn rejects_unfinished_task_at_scope_exit() {
        let module =
            crate::parse("module x\nfn main()\n  scope\n    spawn worker = work(1)\n").unwrap();
        let errors = analyze(&module).unwrap_err();
        assert!(errors.iter().any(|e| e.starts_with("AIF503")));
    }

    #[test]
    fn rejects_double_terminal_task_control() {
        let module = crate::parse(
            "module x
fn main()
  scope
    spawn worker = work(1)
    join worker
    cancel worker
",
        ).unwrap();
        let errors = analyze(&module).unwrap_err();
        assert!(errors.iter().any(|e| e.starts_with("AIF504")));
    }

    #[test]
    fn cancellation_token_is_cooperative() {
        let handle = spawn(|token| {
            while !token.is_cancelled() {
                thread::yield_now();
            }
            7
        });
        handle.cancel();
        assert_eq!(handle.join().unwrap(), 7);
    }
}
