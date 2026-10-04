//! Deterministic module dependency graph for compiler and bootstrap stages.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleNode {
    pub name: String,
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ModuleGraph {
    nodes: BTreeMap<String, ModuleNode>,
    edges: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleGraphError {
    DuplicateModule(String),
    UnknownDependency { module: String, dependency: String },
    Cycle(Vec<String>),
}

impl ModuleGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_module(
        &mut self,
        name: impl Into<String>,
        source_path: Option<String>,
    ) -> Result<(), ModuleGraphError> {
        let name = name.into();
        if self.nodes.contains_key(&name) {
            return Err(ModuleGraphError::DuplicateModule(name));
        }
        self.nodes.insert(
            name.clone(),
            ModuleNode {
                name: name.clone(),
                source_path,
            },
        );
        self.edges.entry(name).or_default();
        Ok(())
    }

    pub fn add_dependency(
        &mut self,
        module: &str,
        dependency: &str,
    ) -> Result<(), ModuleGraphError> {
        if !self.nodes.contains_key(dependency) {
            return Err(ModuleGraphError::UnknownDependency {
                module: module.into(),
                dependency: dependency.into(),
            });
        }
        if !self.nodes.contains_key(module) {
            return Err(ModuleGraphError::UnknownDependency {
                module: module.into(),
                dependency: dependency.into(),
            });
        }
        self.edges
            .entry(module.into())
            .or_default()
            .insert(dependency.into());
        Ok(())
    }

    pub fn dependencies(&self, module: &str) -> impl Iterator<Item = &str> {
        self.edges
            .get(module)
            .into_iter()
            .flat_map(|items| items.iter().map(String::as_str))
    }

    pub fn topological_order(&self) -> Result<Vec<String>, ModuleGraphError> {
        let mut state = BTreeMap::<String, u8>::new();
        let mut order = Vec::with_capacity(self.nodes.len());
        let mut stack = Vec::new();

        for name in self.nodes.keys() {
            if state.get(name).copied().unwrap_or(0) == 0 {
                self.visit(name, &mut state, &mut stack, &mut order)?;
            }
        }

        Ok(order)
    }

    fn visit(
        &self,
        name: &str,
        state: &mut BTreeMap<String, u8>,
        stack: &mut Vec<String>,
        order: &mut Vec<String>,
    ) -> Result<(), ModuleGraphError> {
        state.insert(name.into(), 1);
        stack.push(name.into());

        for dependency in self.dependencies(name) {
            match state.get(dependency).copied().unwrap_or(0) {
                0 => self.visit(dependency, state, stack, order)?,
                1 => {
                    let start = stack
                        .iter()
                        .position(|item| item == dependency)
                        .unwrap_or(0);
                    return Err(ModuleGraphError::Cycle(stack[start..].to_vec()));
                }
                _ => {}
            }
        }

        stack.pop();
        state.insert(name.into(), 2);
        order.push(name.into());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_dependencies_before_dependents() {
        let mut graph = ModuleGraph::new();
        graph.add_module("parser", None).unwrap();
        graph.add_module("lexer", None).unwrap();
        graph.add_dependency("parser", "lexer").unwrap();
        assert_eq!(
            graph.topological_order().unwrap(),
            vec!["lexer".to_string(), "parser".to_string()]
        );
    }

    #[test]
    fn rejects_cycles() {
        let mut graph = ModuleGraph::new();
        graph.add_module("a", None).unwrap();
        graph.add_module("b", None).unwrap();
        graph.add_dependency("a", "b").unwrap();
        graph.add_dependency("b", "a").unwrap();
        assert!(matches!(
            graph.topological_order(),
            Err(ModuleGraphError::Cycle(_))
        ));
    }
}
