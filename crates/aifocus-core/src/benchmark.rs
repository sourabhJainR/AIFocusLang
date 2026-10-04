use std::time::{Duration, Instant};

use crate::{fuzz, native, parse, sema, ownership, ir};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkResult {
    pub cases: usize,
    pub parsed: usize,
    pub semantically_valid: usize,
    pub ownership_valid: usize,
    pub native_compiled: usize,
    pub native_executed: usize,
    pub duration_ms: u128,
}

impl BenchmarkResult {
    pub fn generation_success_rate(&self) -> f64 {
        if self.cases == 0 { 0.0 } else { self.native_executed as f64 / self.cases as f64 }
    }

    pub fn within_budget(&self, budget: Duration) -> bool {
        self.duration_ms <= budget.as_millis()
    }
}

pub fn run(seed_start: u64, cases: usize) -> BenchmarkResult {
    let started = Instant::now();
    let mut result = BenchmarkResult {
        cases,
        parsed: 0,
        semantically_valid: 0,
        ownership_valid: 0,
        native_compiled: 0,
        native_executed: 0,
        duration_ms: 0,
    };

    for offset in 0..cases {
        let case = fuzz::generate(seed_start.wrapping_add(offset as u64));
        let Ok(module) = parse(&case.source) else { continue };
        result.parsed += 1;
        if sema::check(&module).is_err() { continue }
        result.semantically_valid += 1;
        if ownership::infer(&module).is_err() { continue }
        result.ownership_valid += 1;
        let code = match native::compile(&ir::lower(&module)) {
            Ok(code) => code,
            Err(_) => continue,
        };
        result.native_compiled += 1;
        if native::run(&code, &[("a".into(), native::NativeValue::Int(3)), ("b".into(), native::NativeValue::Int(4))]).is_ok() {
            result.native_executed += 1;
        }
    }
    result.duration_ms = started.elapsed().as_millis();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ai_cases_have_a_measurable_success_rate() {
        let result = run(0, 64);
        assert_eq!(result.cases, 64);
        assert_eq!(result.parsed, 64);
        assert_eq!(result.semantically_valid, 64);
        assert_eq!(result.ownership_valid, 64);
        assert!(result.generation_success_rate() > 0.9);
    }
}
