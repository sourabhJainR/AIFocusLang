use std::time::{Duration, Instant};

use crate::{fuzz, ir, native, ownership, parse, sema};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkResult {
    pub cases: usize,
    pub parsed: usize,
    pub semantically_valid: usize,
    pub ownership_valid: usize,
    pub native_compiled: usize,
    pub native_executed: usize,
    pub failed_cases: usize,
    pub fingerprint: u64,
    pub duration_ms: u128,
    pub total_source_bytes: usize,
    pub native_instruction_count: usize,
    pub lowered_rust_bytes: usize,
}

impl BenchmarkResult {
    pub fn deterministic_report(&self) -> String {
        format!(
            "cases={} parsed={} semantic={} ownership={} native_compiled={} native_executed={} failed={} source_bytes={} native_instructions={} rust_bytes={} fingerprint={:016x}",
            self.cases,
            self.parsed,
            self.semantically_valid,
            self.ownership_valid,
            self.native_compiled,
            self.native_executed,
            self.failed_cases,
            self.total_source_bytes,
            self.native_instruction_count,
            self.lowered_rust_bytes,
            self.fingerprint
        )
    }

impl BenchmarkResult {
    pub fn generation_success_rate(&self) -> f64 {
        if self.cases == 0 {
            0.0
        } else {
            self.native_executed as f64 / self.cases as f64
        }
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
        failed_cases: 0,
        fingerprint: 0xcbf29ce484222325,
        duration_ms: 0,
        total_source_bytes: 0,
        native_instruction_count: 0,
        lowered_rust_bytes: 0,
    };

    for offset in 0..cases {
        let case = fuzz::generate(seed_start.wrapping_add(offset as u64));
        result.fingerprint = fingerprint(result.fingerprint, &case.source);
        result.total_source_bytes += case.source.len();
        let Ok(module) = parse(&case.source) else {
            result.failed_cases += 1;
            continue;
        };
        result.parsed += 1;
        if sema::check(&module).is_err() {
            result.failed_cases += 1;
            continue;
        }
        result.semantically_valid += 1;
        if ownership::infer(&module).is_err() {
            result.failed_cases += 1;
            continue;
        }
        result.ownership_valid += 1;
        let lowered = ir::lower(&module);
        let rust = crate::lower::lower(&module);
        result.lowered_rust_bytes += rust.rust.len();
        let code = match native::compile(&lowered) {
            Ok(code) => code,
            Err(_) => {
                result.failed_cases += 1;
                continue;
            }
        };
        result.native_compiled += 1;
        result.native_instruction_count += code.len();
        if native::run(
            &code,
            &[
                ("a".into(), native::NativeValue::Int(3)),
                ("b".into(), native::NativeValue::Int(4)),
            ],
        )
        .is_ok()
        {
            result.native_executed += 1;
        } else {
            result.failed_cases += 1;
        }
    }
    result.duration_ms = started.elapsed().as_millis();
    result
}

fn fingerprint(mut hash: u64, source: &str) -> u64 {
    for byte in source.bytes() {
        hash = hash
            .wrapping_mul(0x100000001b3)
            .wrapping_add(u64::from(byte));
    }
    hash
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
        assert_eq!(result.failed_cases, 0);
        assert!(result.generation_success_rate() > 0.9);
        assert_eq!(result.fingerprint, run(0, 64).fingerprint);
        assert_eq!(
            result.deterministic_report(),
            run(0, 64).deterministic_report()
        );
        assert!(result.deterministic_report().contains("native_instructions="));
    }
}
