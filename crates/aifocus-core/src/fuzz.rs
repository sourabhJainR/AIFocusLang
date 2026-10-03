use crate::{Item, format, lower, ownership, parse, sema};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedCase {
    pub seed: u64,
    pub source: String,
}

pub fn generate(seed: u64) -> GeneratedCase {
    let mut rng = Rng(seed);
    let a = rng.next_i64();
    let b = rng.next_i64();
    let c = rng.next_i64();
    let source = format!(
        "module generated\nfn main(a: Int, b: Int) -> Int\n  let x = a + {}\n  let y = x * {}\n  y + b + {}\n",
        a.rem_euclid(97),
        b.rem_euclid(17),
        c.rem_euclid(53),
    );
    GeneratedCase { seed, source }
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
    fn generated_cases_are_deterministic() {
        assert_eq!(generate(42), generate(42));
        assert_ne!(generate(42), generate(43));
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
