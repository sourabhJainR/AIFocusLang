//! AI-native runtime values and low-overhead evaluation telemetry.
//!
//! These structures are intentionally dependency-free and deterministic so they
//! can be used by the compiler bootstrap before an external runtime exists.

use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, PartialEq)]
pub struct Embedding {
    values: Vec<f32>,
}

impl Embedding {
    pub fn new(values: Vec<f32>) -> Self {
        Self { values }
    }

    pub fn values(&self) -> &[f32] {
        &self.values
    }

    pub fn dimension(&self) -> usize {
        self.values.len()
    }

    pub fn dot(&self, other: &Self) -> Option<f32> {
        (self.dimension() == other.dimension()).then(|| {
            self.values
                .iter()
                .zip(&other.values)
                .map(|(a, b)| a * b)
                .sum()
        })
    }

    pub fn cosine_similarity(&self, other: &Self) -> Option<f32> {
        let dot = self.dot(other)?;
        let a = self.values.iter().map(|v| v * v).sum::<f32>().sqrt();
        let b = other.values.iter().map(|v| v * v).sum::<f32>().sqrt();
        if a == 0.0 || b == 0.0 {
            None
        } else {
            Some(dot / (a * b))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Probability(f32);

impl Probability {
    pub fn new(value: f32) -> Option<Self> {
        value
            .is_finite()
            .then_some(value)
            .filter(|v| (0.0..=1.0).contains(v))
            .map(Self)
    }

    pub fn value(self) -> f32 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Distribution {
    weights: Vec<f32>,
}

impl Distribution {
    pub fn from_logits(logits: &[f32]) -> Self {
        if logits.is_empty() {
            return Self {
                weights: Vec::new(),
            };
        }
        let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let mut weights: Vec<f32> = logits.iter().map(|x| (x - max).exp()).collect();
        let sum: f32 = weights.iter().sum();
        if sum > 0.0 {
            for x in &mut weights {
                *x /= sum;
            }
        }
        Self { weights }
    }

    pub fn weights(&self) -> &[f32] {
        &self.weights
    }

    pub fn entropy(&self) -> f32 {
        -self
            .weights
            .iter()
            .filter(|p| **p > 0.0)
            .map(|p| p * p.ln())
            .sum::<f32>()
    }

    pub fn confidence(&self) -> f32 {
        self.weights.iter().copied().fold(0.0, f32::max)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantization {
    Fp32,
    Fp16,
    Bf16,
    Int8,
    Int4,
    Int2,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tensor {
    shape: Vec<usize>,
    values: Vec<f32>,
    quantization: Quantization,
}

impl Tensor {
    pub fn new(shape: Vec<usize>, values: Vec<f32>, quantization: Quantization) -> Option<Self> {
        let elements = shape.iter().try_fold(1usize, |a, b| a.checked_mul(*b))?;
        (elements == values.len()).then_some(Self {
            shape,
            values,
            quantization,
        })
    }

    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    pub fn values(&self) -> &[f32] {
        &self.values
    }

    pub fn quantization(&self) -> Quantization {
        self.quantization
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Modality {
    Text,
    Vision,
    Audio,
    Other(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticValue {
    pub embedding: Embedding,
    pub modality: Modality,
    pub metadata: BTreeMap<String, String>,
}

impl SemanticValue {
    pub fn new(embedding: Embedding, modality: Modality) -> Self {
        Self {
            embedding,
            modality,
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceEvent {
    pub sequence: u64,
    pub level: TraceLevel,
    pub operation: &'static str,
    pub span: Option<(usize, usize)>,
    pub duration_ns: u64,
    pub confidence: Option<Probability>,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct TraceBuffer {
    events: VecDeque<TraceEvent>,
    capacity: usize,
    next_sequence: u64,
}

impl TraceBuffer {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity),
            capacity,
            next_sequence: 0,
        }
    }

    pub fn push(
        &mut self,
        level: TraceLevel,
        operation: &'static str,
        duration_ns: u64,
        message: impl Into<String>,
    ) {
        if self.capacity == 0 {
            return;
        }
        let event = TraceEvent {
            sequence: self.next_sequence,
            level,
            operation,
            span: None,
            duration_ns,
            confidence: None,
            message: message.into(),
        };
        self.next_sequence = self.next_sequence.wrapping_add(1);
        if self.events.len() == self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    pub fn events(&self) -> impl DoubleEndedIterator<Item = &TraceEvent> {
        self.events.iter()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeError {
    pub code: &'static str,
    pub message: String,
    pub operation: Option<&'static str>,
    pub span: Option<(usize, usize)>,
}

impl RuntimeError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            operation: None,
            span: None,
        }
    }

    pub fn at(mut self, span: (usize, usize)) -> Self {
        self.span = Some(span);
        self
    }

    pub fn in_operation(mut self, operation: &'static str) -> Self {
        self.operation = Some(operation);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probability_rejects_invalid_values() {
        assert!(Probability::new(0.5).is_some());
        assert!(Probability::new(1.1).is_none());
    }

    #[test]
    fn logits_form_normalized_distribution() {
        let d = Distribution::from_logits(&[1.0, 2.0, 3.0]);
        assert!((d.weights().iter().sum::<f32>() - 1.0).abs() < 0.0001);
        assert!(d.confidence() > 0.6);
    }

    #[test]
    fn embedding_similarity_is_bounded() {
        let a = Embedding::new(vec![1.0, 0.0]);
        assert!((a.cosine_similarity(&a).unwrap() - 1.0).abs() < 0.0001);
    }

    #[test]
    fn trace_buffer_is_bounded() {
        let mut t = TraceBuffer::with_capacity(2);
        t.push(TraceLevel::Info, "parse", 10, "one");
        t.push(TraceLevel::Info, "sema", 20, "two");
        t.push(TraceLevel::Warn, "native", 30, "three");
        assert_eq!(t.events().count(), 2);
        assert_eq!(t.events().next().unwrap().sequence, 1);
    }
}
