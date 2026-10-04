use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::{Item, format, lower, ownership, parse, sema};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedCase {
    pub seed: u64,
    pub kind: &'static str,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationCase {
    pub seed: u64,
    pub mutation: &'static str,
    pub source: String,
    pub expected_valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationVerification {
    pub seed: u64,
    pub mutation: &'static str,
    pub accepted: bool,
    pub crashed: bool,
}

pub const MALFORMED_CORPUS: &[&str] = &[
    "",
    "module",
    "module x\nfn main( -> Int\n  1\n",
    "module x\nfn main() -> Int\n    1\n  broken\n",
    "module x\nfn main() -> Int\n  [1,\n",
];

pub fn generate(seed: u64) -> GeneratedCase {
    let mut rng = Rng(seed);
    let a = rng.next_i64().rem_euclid(97);
    let b = rng.next_i64().rem_euclid(53);
    let kind = match seed % 4 {
        0 => "arithmetic",
        1 => "conditional",
        2 => "list",
        _ => "string",
    };
    let source = match kind {
        "arithmetic" => format!(
            "module generated\nfn main(a: Int, b: Int) -> Int\n  let x = a + {a}\n  let y = x * {b}\n  y + b\n"
        ),
        "conditional" => "module generated
fn main(a: Int, b: Int) -> Int
  if a == 0
    return b
  else
    return a + b
"
        .into(),
        "list" => "module generated
fn main(a: Int, b: Int) -> Int
  let xs = [a, b]
  len(xs) + a
"
        .into(),
        _ => r#"module generated
fn main(a: Int, b: Int) -> Int
  let text = "ardisa"
  len(text) + a
"#
        .into(),
    };
    GeneratedCase {
        seed,
        kind,
        source,
        expected_valid: true,
    }
}

pub fn mutate(case: &GeneratedCase, seed: u64) -> MutationCase {
    let (mutation, source, expected_valid) = match seed % 8 {
        0 if case.source.contains(" + ") => {
            ("add-to-sub", case.source.replacen(" + ", " - ", 1), true)
        }
        1 if case.source.contains(" == ") => {
            ("eq-to-ne", case.source.replacen(" == ", " != ", 1), true)
        }
        2 if case.source.contains(" * ") => {
            ("mul-to-mod", case.source.replacen(" * ", " % ", 1), true)
        }
        3 => (
            "remove-module",
            case.source.replacen("module generated\n", "", 1),
            false,
        ),
        4 => (
            "corrupt-indent",
            format!("{}  broken\n", case.source),
            false,
        ),
        5 => (
            "truncate",
            case.source[..case.source.len() / 2].to_string(),
            false,
        ),
        6 => ("invalid-token", format!("{}\n@\n", case.source), false),
        _ => ("whitespace", format!("{}\n", case.source), true),
    };
    MutationCase {
        seed,
        mutation,
        source,
        expected_valid,
    }
}

pub fn verify_mutation(case: &MutationCase) -> Result<MutationVerification, String> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        verify(&GeneratedCase {
            seed: case.seed,
            kind: "mutation",
            source: case.source.clone(),
            expected_valid: case.expected_valid,
        })
    }));
    match result {
        Ok(Ok(())) if case.expected_valid => Ok(MutationVerification {
            seed: case.seed,
            mutation: case.mutation,
            accepted: true,
            crashed: false,
        }),
        Ok(Ok(())) => Err(format!("invalid mutation {} was accepted", case.mutation)),
        Ok(Err(_)) if !case.expected_valid => Ok(MutationVerification {
            seed: case.seed,
            mutation: case.mutation,
            accepted: false,
            crashed: false,
        }),
        Ok(Err(error)) => Err(format!(
            "valid mutation {} unexpectedly failed: {error}",
            case.mutation
        )),
        Err(_) => Err(format!(
            "compiler panicked while verifying mutation {}",
            case.mutation
        )),
    }
}

pub fn verify_malformed(source: &str) -> Result<(), String> {
    if catch_unwind(AssertUnwindSafe(|| parse(source))).is_err() {
        return Err("compiler panicked on malformed input".into());
    }
    Ok(())
}

pub fn verify(case: &GeneratedCase) -> Result<(), String> {
    let first = parse(&case.source).map_err(|e| format!("parse {:?}: {e:?}", case.seed))?;
    sema::check(&first).map_err(|e| format!("sema {:?}: {e:?}", case.seed))?;
    ownership::infer(&first).map_err(|e| format!("ownership {:?}: {e:?}", case.seed))?;
    let formatted = format::format_module(&first);
    let second = parse(&formatted).map_err(|e| format!("round-trip {:?}: {e:?}", case.seed))?;
    if first.items.len() != second.items.len() {
        return Err(format!("item count changed for seed {}", case.seed));
    }
    for (left, right) in first.items.iter().zip(second.items.iter()) {
        match (left, right) {
            (Item::Function(a), Item::Function(b)) if a.id == b.id => {}
            _ => return Err(format!("function identity changed for seed {}", case.seed)),
        }
    }
    if lower::lower(&second).rust.trim().is_empty() {
        return Err(format!("empty lowering for seed {}", case.seed));
    }
    Ok(())
}

struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn next_i64(&mut self) -> i64 {
        self.next_u64() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_mutations_cover_accept_and_reject_paths() {
        let mut accepted = 0;
        let mut rejected = 0;
        for seed in 0..1024 {
            let generated = generate(seed);
            let mutation = mutate(&generated, seed.wrapping_add(17));
            let verification = verify_mutation(&mutation).unwrap_or_else(|error| panic!("{error}"));
            if verification.accepted {
                accepted += 1;
            } else {
                rejected += 1;
            }
            assert!(!verification.crashed);
        }
        assert!(accepted > 0);
        assert!(rejected > 0);
    }

    #[test]
    fn malformed_corpus_never_panics() {
        for source in MALFORMED_CORPUS {
            verify_malformed(source).unwrap_or_else(|error| panic!("{error}"));
        }
    }

    #[test]
    fn generated_cases_are_deterministic() {
        assert_eq!(generate(42), generate(42));
        assert_ne!(generate(42), generate(43));
        assert_eq!(generate(0).kind, "arithmetic");
        assert_eq!(generate(1).kind, "conditional");
        assert_eq!(generate(2).kind, "list");
        assert_eq!(generate(3).kind, "string");
    }

    #[test]
    fn generated_cases_survive_compiler_properties() {
        for seed in 0..256 {
            verify(&generate(seed)).unwrap_or_else(|error| panic!("{error}"));
        }
    }

    #[test]
    fn generated_inputs_stay_bounded() {
        for seed in 0..256 {
            let case = generate(seed);
            assert!(case.source.len() < 512);
        }
    }
}
