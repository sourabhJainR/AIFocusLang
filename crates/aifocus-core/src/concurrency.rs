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
                    StmtKind::Join { name } => match tasks.get(name).copied() {
                        Some(TaskState::Running) => {
                            tasks.insert(name.clone(), TaskState::Joined);
                        }
                        Some(_) => {
                            errors.push(format!("AIF504: task '{name}' is already terminal"))
                        }
                        None => errors.push(format!("AIF502: unknown task '{name}' in scope")),
                    },
                    StmtKind::Cancel { name } => match tasks.get(name).copied() {
                        Some(TaskState::Running) => {
                            tasks.insert(name.clone(), TaskState::Cancelled);
                        }
                        Some(_) => {
                            errors.push(format!("AIF504: task '{name}' is already terminal"))
                        }
                        None => errors.push(format!("AIF502: unknown task '{name}' in scope")),
                    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskState {
    Running,
    Joined,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct CancellationToken {
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


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeEvent {
    Spawned(String),
    Joined(String),
    Cancelled(String),
}

pub struct StructuredScope {
    tasks: HashMap<String, (TaskHandle<()>, TaskState)>,
    events: Vec<ScopeEvent>,
}

impl StructuredScope {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            events: Vec::new(),
        }
    }

    pub fn spawn<F>(&mut self, name: impl Into<String>, task: F) -> Result<(), String>
    where
        F: FnOnce(CancellationToken) + Send + 'static,
    {
        let name = name.into();
        if self.tasks.contains_key(&name) {
            return Err(format!("AIF501: duplicate task '{name}'"));
        }
        self.tasks
            .insert(name.clone(), (spawn(task), TaskState::Running));
        self.events.push(ScopeEvent::Spawned(name));
        Ok(())
    }

    pub fn join(&mut self, name: &str) -> Result<(), String> {
        let Some((handle, state)) = self.tasks.remove(name) else {
            return Err(format!("AIF502: unknown task '{name}'"));
        };
        if state != TaskState::Running {
            return Err(format!("AIF504: task '{name}' is already terminal"));
        }
        match handle.join() {
            Ok(()) => {
                self.events.push(ScopeEvent::Joined(name.into()));
                Ok(())
            }
            Err(_) => Err(format!("AIF505: task '{name}' panicked")),
        }
    }

    pub fn cancel(&mut self, name: &str) -> Result<(), String> {
        let Some((handle, state)) = self.tasks.get_mut(name) else {
            return Err(format!("AIF502: unknown task '{name}'"));
        };
        if *state != TaskState::Running {
            return Err(format!("AIF504: task '{name}' is already terminal"));
        }
        handle.cancel();
        *state = TaskState::Cancelled;
        self.events.push(ScopeEvent::Cancelled(name.into()));
        Ok(())
    }

    pub fn finish(mut self) -> Result<Vec<ScopeEvent>, String> {
        let names = self.tasks.keys().cloned().collect::<Vec<_>>();
        for name in names {
            let (handle, state) = self.tasks.remove(&name).expect("task disappeared");
            if handle.join().is_err() {
                return Err(format!("AIF505: task '{name}' panicked"));
            }
            if state == TaskState::Running {
                self.events.push(ScopeEvent::Joined(name));
            }
        }
        Ok(self.events)
    }

    pub fn events(&self) -> &[ScopeEvent] {
        &self.events
    }
}

impl Default for StructuredScope {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for StructuredScope {
    fn drop(&mut self) {
        for (handle, state) in self.tasks.values_mut() {
            if *state == TaskState::Running {
                handle.cancel();
                *state = TaskState::Cancelled;
            }
        }
        for (_, (handle, _)) in self.tasks.drain() {
            let _ = handle.join();
        }
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
        )
        .unwrap();
        let errors = analyze(&module).unwrap_err();
        assert!(errors.iter().any(|e| e.starts_with("AIF504")));
    }


    #[test]
    fn structured_scope_cleans_up_unfinished_tasks_on_drop() {
        let mut scope = StructuredScope::new();
        scope
            .spawn("worker", |token| {
                while !token.is_cancelled() {
                    thread::yield_now();
                }
            })
            .unwrap();
        assert_eq!(scope.events(), &[ScopeEvent::Spawned("worker".into())]);
    }

    #[test]
    fn structured_scope_requires_explicit_terminal_state() {
        let mut scope = StructuredScope::new();
        scope
            .spawn("worker", |_token| {})
            .unwrap();
        assert!(scope.join("worker").is_ok());
        assert_eq!(
            scope.events(),
            &[
                ScopeEvent::Spawned("worker".into()),
                ScopeEvent::Joined("worker".into())
            ]
        );
    }

    #[test]
    fn structured_scope_cancellation_is_followed_by_scope_cleanup() {
        let mut scope = StructuredScope::new();
        scope
            .spawn("worker", |token| {
                while !token.is_cancelled() {
                    thread::yield_now();
                }
            })
            .unwrap();
        scope.cancel("worker").unwrap();
        let events = scope.finish().unwrap();
        assert_eq!(
            events,
            vec![
                ScopeEvent::Spawned("worker".into()),
                ScopeEvent::Cancelled("worker".into())
            ]
        );
    }

    #[test]
    fn structured_scope_rejects_duplicate_tasks() {
        let mut scope = StructuredScope::new();
        scope.spawn("worker", |_token| {}).unwrap();
        assert!(scope.spawn("worker", |_token| {}).is_err());
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
