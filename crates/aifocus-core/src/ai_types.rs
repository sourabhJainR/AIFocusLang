//! Runtime foundations for uncertainty-aware AI values.
//! These are runtime APIs, not yet first-class Ardisa source-language types.
//! They avoid hidden model calls, implicit repair, and unsubstantiated GPU guarantees.

use std::collections::{BTreeMap, BTreeSet};
use crate::ai::{Probability, RuntimeError, Tensor};

#[derive(Debug, Clone, PartialEq)]
pub struct Alternative<T> {
    pub value: T,
    pub probability: Probability,
}

/// A selected value plus explicit uncertainty metadata. Callers must choose
/// the confidence threshold appropriate to the operation.
#[derive(Debug, Clone, PartialEq)]
pub struct Probabilistic<T> {
    value: T,
    confidence: Probability,
    alternatives: Vec<Alternative<T>>,
}

impl<T> Probabilistic<T> {
    pub fn new(value: T, confidence: Probability, alternatives: Vec<Alternative<T>>) -> Self {
        Self { value, confidence, alternatives }
    }
    pub fn value(&self) -> &T { &self.value }
    pub fn confidence(&self) -> Probability { self.confidence }
    pub fn alternatives(&self) -> &[Alternative<T>] { &self.alternatives }
    pub fn meets_threshold(&self, threshold: Probability) -> bool {
        self.confidence.value() >= threshold.value()
    }
    pub fn into_value(self) -> T { self.value }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphTensorNode {
    pub id: String,
    pub label: String,
    pub tensor_row: usize,
    pub confidence: Option<Probability>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphTensorEdge {
    pub from: String,
    pub relation: String,
    pub to: String,
}

/// Tensor storage plus referentially validated semantic graph metadata.
/// This does not itself provide a vector database, GPU allocator, or model.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphTensor {
    tensor: Tensor,
    nodes: Vec<GraphTensorNode>,
    edges: Vec<GraphTensorEdge>,
}

impl GraphTensor {
    pub fn new(tensor: Tensor, nodes: Vec<GraphTensorNode>, edges: Vec<GraphTensorEdge>) -> Result<Self, RuntimeError> {
        let row_count = tensor.shape().first().copied().ok_or_else(|| RuntimeError::new("AIT001", "graph-tensor requires a leading tensor axis"))?;
        let mut ids = BTreeSet::new();
        for node in &nodes {
            if !ids.insert(node.id.as_str()) {
                return Err(RuntimeError::new("AIT002", format!("duplicate graph node id '{}'", node.id)));
            }
            if node.tensor_row >= row_count {
                return Err(RuntimeError::new("AIT003", format!("tensor row {} for node '{}' is out of bounds", node.tensor_row, node.id)));
            }
        }
        for edge in &edges {
            if !ids.contains(edge.from.as_str()) || !ids.contains(edge.to.as_str()) {
                return Err(RuntimeError::new("AIT004", format!("edge '{} -> {}' references an unknown node", edge.from, edge.to)));
            }
        }
        Ok(Self { tensor, nodes, edges })
    }
    pub fn tensor(&self) -> &Tensor { &self.tensor }
    pub fn nodes(&self) -> &[GraphTensorNode] { &self.nodes }
    pub fn edges(&self) -> &[GraphTensorEdge] { &self.edges }

    /// Merge metadata only; tensor layout/arithmetic remains explicit because
    /// the two tensor shapes may differ. Conflicts require a chosen policy.
    pub fn merge_metadata(&self, other: &Self, policy: GraphConflictPolicy) -> Result<(Vec<GraphTensorNode>, Vec<GraphTensorEdge>), RuntimeError> {
        let mut nodes: BTreeMap<String, GraphTensorNode> = self.nodes.iter().cloned().map(|n| (n.id.clone(), n)).collect();
        for incoming in &other.nodes {
            match nodes.get(&incoming.id) {
                None => { nodes.insert(incoming.id.clone(), incoming.clone()); }
                Some(existing) if existing == incoming => {}
                Some(_) => match policy {
                    GraphConflictPolicy::Reject => return Err(RuntimeError::new("AIT005", format!("conflicting graph node '{}'", incoming.id))),
                    GraphConflictPolicy::PreferLeft => {}
                    GraphConflictPolicy::PreferHigherConfidence => {
                        let old_conf = nodes.get(&incoming.id).and_then(|n| n.confidence).map(Probability::value).unwrap_or(0.0);
                        let new_conf = incoming.confidence.map(Probability::value).unwrap_or(0.0);
                        if new_conf > old_conf { nodes.insert(incoming.id.clone(), incoming.clone()); }
                    }
                }
            }
        }
        let mut edges: BTreeSet<GraphTensorEdge> = self.edges.iter().cloned().collect();
        edges.extend(other.edges.iter().cloned());
        for edge in &edges {
            if !nodes.contains_key(&edge.from) || !nodes.contains_key(&edge.to) {
                return Err(RuntimeError::new("AIT004", "merged edge references an unknown node"));
            }
        }
        Ok((nodes.into_values().collect(), edges.into_iter().collect()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphConflictPolicy { Reject, PreferLeft, PreferHigherConfidence }

/// A value admitted only after caller-supplied structural validation.
/// Repair is opt-in, bounded, and every candidate is validated before return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guaranteed<T> { value: T }

impl<T> Guaranteed<T> {
    pub fn try_new(value: T, validate: impl FnOnce(&T) -> Result<(), RuntimeError>) -> Result<Self, RuntimeError> {
        validate(&value)?;
        Ok(Self { value })
    }
    pub fn as_ref(&self) -> &T { &self.value }
    pub fn into_inner(self) -> T { self.value }

    /// Try the initial value, then at most max_attempts repaired candidates.
    /// The repair callback is supplied by the application; no AI is invoked here.
    pub fn repair_with(mut candidate: T, max_attempts: usize, mut validate: impl FnMut(&T) -> Result<(), RuntimeError>, mut repair: impl FnMut(&T, usize) -> Result<T, RuntimeError>) -> Result<Self, RuntimeError> {
        if validate(&candidate).is_ok() { return Ok(Self { value: candidate }); }
        for attempt in 1..=max_attempts {
            candidate = repair(&candidate, attempt)?;
            if validate(&candidate).is_ok() { return Ok(Self { value: candidate }); }
        }
        Err(RuntimeError::new("AIT006", format!("payload did not satisfy its schema after {} repair attempt(s)", max_attempts)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{Quantization, Tensor};
    fn p(v: f32) -> Probability { Probability::new(v).expect("valid probability") }

    #[test]
    fn probabilistic_requires_explicit_threshold_check() {
        let value = Probabilistic::new(true, p(0.7), vec![]);
        assert!(!value.meets_threshold(p(0.85)));
        assert!(value.meets_threshold(p(0.6)));
    }

    #[test]
    fn graph_tensor_checks_rows_and_edge_references() {
        let tensor = Tensor::new(vec![2, 2], vec![0.1, 0.2, 0.3, 0.4], Quantization::Fp32).unwrap();
        let nodes = vec![GraphTensorNode { id: "intent".into(), label: "User Intent".into(), tensor_row: 0, confidence: Some(p(0.9)) }];
        let bad_edge = GraphTensorEdge { from: "intent".into(), relation: "relates_to".into(), to: "missing".into() };
        assert_eq!(GraphTensor::new(tensor.clone(), nodes.clone(), vec![bad_edge]).unwrap_err().code, "AIT004");
        let bad_node = GraphTensorNode { id: "overflow".into(), label: "Overflow".into(), tensor_row: 2, confidence: None };
        assert_eq!(GraphTensor::new(tensor, vec![bad_node], vec![]).unwrap_err().code, "AIT003");
    }

    #[test]
    fn graph_merge_requires_explicit_conflict_policy() {
        let tensor = Tensor::new(vec![1, 1], vec![1.0], Quantization::Fp32).unwrap();
        let left = GraphTensor::new(tensor.clone(), vec![GraphTensorNode { id: "x".into(), label: "left".into(), tensor_row: 0, confidence: Some(p(0.6)) }], vec![]).unwrap();
        let right = GraphTensor::new(tensor, vec![GraphTensorNode { id: "x".into(), label: "right".into(), tensor_row: 0, confidence: Some(p(0.9)) }], vec![]).unwrap();
        assert!(left.merge_metadata(&right, GraphConflictPolicy::Reject).is_err());
        let (nodes, _) = left.merge_metadata(&right, GraphConflictPolicy::PreferHigherConfidence).unwrap();
        assert_eq!(nodes[0].label, "right");
    }

    #[test]
    fn guaranteed_repairs_are_bounded_and_revalidated() {
        let valid = |v: &i32| if *v >= 0 { Ok(()) } else { Err(RuntimeError::new("SCHEMA", "must be nonnegative")) };
        let value = Guaranteed::repair_with(-1, 2, valid, |_, _| Ok(7)).unwrap();
        assert_eq!(*value.as_ref(), 7);
        let failed = Guaranteed::repair_with(-1, 1, valid, |_, _| Ok(-2));
        assert_eq!(failed.unwrap_err().code, "AIT006");
    }
}
