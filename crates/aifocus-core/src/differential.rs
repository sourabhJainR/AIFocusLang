use crate::{lower, ownership, parse, sema};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DifferentialCase {
    pub name: &'static str,
    pub source: &'static str,
}

pub const CORPUS: &[DifferentialCase] = &[
    DifferentialCase {
        name: "arithmetic",
        source: "module x\nfn main(a: Int, b: Int) -> Int\n  a + b * 2\n",
    },
    DifferentialCase {
        name: "conditional",
        source: "module x\nfn main(a: Int) -> Int\n  if a == 0\n    return 1\n  else\n    return a\n",
    },
];

pub fn verify_case(case: &DifferentialCase) -> Result<(), String> {
    let module = parse(case.source).map_err(|errors| format!("{}: parse failed", case.name))?;
    sema::check(&module).map_err(|errors| format!("{}: semantic check failed: {errors:?}", case.name))?;
    ownership::infer(&module)
        .map_err(|errors| format!("{}: ownership check failed: {errors:?}", case.name))?;
    let lowered = lower::lower(&module).map_err(|errors| format!("{}: lowering failed: {errors:?}", case.name))?;
    if lowered.rust.trim().is_empty() {
        return Err(format!("{}: lowering produced empty Rust", case.name));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_is_non_empty() {
        assert!(!CORPUS.is_empty());
    }

    #[test]
    fn parse_format_parse_preserves_semantic_identity() {
        for case in CORPUS {
            let first = parse(case.source).unwrap();
            let formatted = crate::format::format_module(&first);
            let second = parse(&formatted).unwrap();
            assert_eq!(first.items.len(), second.items.len(), "{}", case.name);
            for (left, right) in first.items.iter().zip(second.items.iter()) {
                match (left, right) {
                    (crate::Item::Function(a), crate::Item::Function(b)) => {
                        assert_eq!(a.id, b.id, "{}", case.name);
                    }
                }
            }
        }
    }

    #[test]
    fn every_corpus_case_survives_the_compiler_pipeline() {
        for case in CORPUS {
            verify_case(case).unwrap_or_else(|error| panic!("{error}"));
        }
    }
}
