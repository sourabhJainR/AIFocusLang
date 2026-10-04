use crate::{ir, native, ownership, parse, sema};
use std::collections::BTreeMap;

/// Minimal native bootstrap fixture used for staged verification.
pub const BOOTSTRAP_SOURCE: &str = "module bootstrap
fn double(a: Int) -> Int
  a * 2
fn main(a: Int, b: Int) -> Int
  a * 2 + b
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapArtifact {
    pub stage: u8,
    pub source_fingerprint: u64,
    pub instruction_count: usize,
    pub functions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapReport {
    pub parsed: bool,
    pub semantically_valid: bool,
    pub ownership_valid: bool,
    pub native_compiled: bool,
    pub native_result: Option<native::NativeValue>,
    pub stage0: Option<BootstrapArtifact>,
    pub stage1: Option<BootstrapArtifact>,
    pub stage2: Option<BootstrapArtifact>,
    pub reproducible: bool,
    pub self_hosting_ready: bool,
    pub blocker: Option<&'static str>,
}

// Bootstrap verification follows the native function representation.
pub fn verify() -> BootstrapReport {
    let Ok(module) = parse(BOOTSTRAP_SOURCE) else {
        return failed("bootstrap source does not parse");
    };
    if sema::check(&module).is_err() {
        return failed("bootstrap source fails semantic validation");
    }
    if ownership::infer(&module).is_err() {
        return failed("bootstrap source fails ownership validation");
    }

    let lowered = ir::lower(&module);
    let Ok(stage0_program) = native::compile_program(&lowered) else {
        return failed("native backend cannot compile the bootstrap subset");
    };
    let stage0 = artifact(
        0,
        BOOTSTRAP_SOURCE,
        &lowered,
        stage0_program
            .functions
            .values()
            .map(|function| function.code.len())
            .sum(),
    );

    let Ok(stage1_program) = native::compile_program(&lowered) else {
        return failed("stage1 compilation failed");
    };
    let stage1 = artifact(
        1,
        BOOTSTRAP_SOURCE,
        &lowered,
        stage1_program
            .functions
            .values()
            .map(|function| function.code.len())
            .sum(),
    );

    let main_code = stage1_program
        .functions
        .get("main")
        .expect("bootstrap main must compile");
    let native_result = native::run(
        main_code.code.as_slice(),
        &[
            ("a".into(), native::NativeValue::Int(3)),
            ("b".into(), native::NativeValue::Int(4)),
        ],
    )
    .ok();

    let reproducible = stage0.source_fingerprint == stage1.source_fingerprint
        && stage0.instruction_count == stage1.instruction_count
        && stage0.functions == stage1.functions;

    let stage2 = self_hosted_pipeline_artifact().ok();

    BootstrapReport {
        parsed: true,
        semantically_valid: true,
        ownership_valid: true,
        native_compiled: true,
        native_result,
        stage0: Some(stage0),
        stage1: Some(stage1),
        stage2: stage2.clone(),
        reproducible: reproducible && stage2.is_some(),
        self_hosting_ready: false,
        blocker: Some(
            "stage2 now executes the Ardisa-authored source pipeline natively; true self-hosting still requires that pipeline to compile and recompile the compiler itself without the Rust host",
        ),
    }
}



const SELF_HOSTED_SOURCES: &[(&str, &str)] = &[
    ("lexer", include_str!("../../../bootstrap/lexer.ardisa")),
    ("parser", include_str!("../../../bootstrap/parser.ardisa")),
    ("ast", include_str!("../../../bootstrap/ast.ardisa")),
    (
        "semantic",
        include_str!("../../../bootstrap/semantic.ardisa"),
    ),
    ("ir", include_str!("../../../bootstrap/ir.ardisa")),
];

fn self_hosted_pipeline_artifact() -> Result<BootstrapArtifact, &'static str> {
    let mut total_instructions = 0usize;
    let mut fingerprints = 0u64;
    let mut programs = BTreeMap::new();

    for (name, source) in SELF_HOSTED_SOURCES {
        let module = parse(source).map_err(|_| "self-hosted source does not parse")?;
        sema::check(&module).map_err(|_| "self-hosted source fails semantic validation")?;
        ownership::infer(&module).map_err(|_| "self-hosted source fails ownership validation")?;
        let lowered = ir::lower(&module);
        let program = native::compile_program(&lowered)
            .map_err(|_| "self-hosted source cannot compile natively")?;
        total_instructions += program
            .functions
            .values()
            .map(|function| function.code.len())
            .sum::<usize>();
        fingerprints ^= fingerprint(&format!("{name}:{source}"));
        programs.insert((*name).to_string(), program);
    }

    let source = "module demo
fn main(a: Int) -> Int
  a + 1
";
    let lexer = programs.get("lexer").ok_or("missing lexer program")?;
    let parser = programs.get("parser").ok_or("missing parser program")?;
    let ast = programs.get("ast").ok_or("missing ast program")?;
    let semantic = programs.get("semantic").ok_or("missing semantic program")?;
    let ir_program = programs.get("ir").ok_or("missing ir program")?;

    let tokens = native::run_program(
        lexer,
        "lex",
        &[native::NativeValue::String(source.into())],
    )
    .map_err(|_| "self-hosted lexer execution failed")?;
    let tokens = match tokens {
        native::NativeValue::String(value) => value,
        _ => return Err("self-hosted lexer returned non-string tokens"),
    };
    let parsed = native::run_program(
        parser,
        "parse",
        &[native::NativeValue::String(tokens)],
    )
    .map_err(|_| "self-hosted parser execution failed")?;
    let parsed = match parsed {
        native::NativeValue::String(value) => value,
        _ => return Err("self-hosted parser returned non-string AST"),
    };
    let ast_value = native::run_program(
        ast,
        "build",
        &[native::NativeValue::String(parsed)],
    )
    .map_err(|_| "self-hosted AST construction failed")?;
    let ast_value = match ast_value {
        native::NativeValue::String(value) => value,
        _ => return Err("self-hosted AST returned non-string representation"),
    };
    let semantic_value = native::run_program(
        semantic,
        "check",
        &[native::NativeValue::String(ast_value.clone())],
    )
    .map_err(|_| "self-hosted semantic analysis failed")?;
    let semantic_value = match semantic_value {
        native::NativeValue::String(value) => value,
        _ => return Err("self-hosted semantic analysis returned non-string result"),
    };
    if semantic_value != "Ok" {
        return Err("self-hosted semantic analysis rejected its own AST");
    }
    let lowered_value = native::run_program(
        ir_program,
        "lower",
        &[native::NativeValue::String(ast_value)],
    )
    .map_err(|_| "self-hosted IR lowering failed")?;
    let lowered_value = match lowered_value {
        native::NativeValue::String(value) => value,
        _ => return Err("self-hosted IR lowering returned non-string IR"),
    };

    Ok(BootstrapArtifact {
        stage: 2,
        source_fingerprint: fingerprint(&format!("{fingerprints}:{tokens}:{lowered_value}")),
        instruction_count: total_instructions,
        functions: programs
            .values()
            .map(|program| program.functions.len())
            .sum(),
    })
}

fn failed(blocker: &'static str) -> BootstrapReport {
    BootstrapReport {
        parsed: false,
        semantically_valid: false,
        ownership_valid: false,
        native_compiled: false,
        native_result: None,
        stage0: None,
        stage1: None,
        stage2: None,
        reproducible: false,
        self_hosting_ready: false,
        blocker: Some(blocker),
    }
}

fn artifact(
    stage: u8,
    source: &str,
    module: &ir::IrModule,
    instruction_count: usize,
) -> BootstrapArtifact {
    BootstrapArtifact {
        stage,
        source_fingerprint: fingerprint(source),
        instruction_count,
        functions: module.functions.len(),
    }
}

fn fingerprint(source: &str) -> u64 {
    source.bytes().fold(0xcbf29ce484222325u64, |hash, byte| {
        hash.wrapping_mul(0x100000001b3)
            .wrapping_add(u64::from(byte))
    })
}

pub fn stage_manifest() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("stage0", "Rust-hosted Ardisa compiler primitives"),
        (
            "stage1",
            "Native Ardisa program compiled by the same deterministic pipeline",
        ),
        ("stage2", "Reserved for Ardisa compiler compiling itself"),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_pipeline_is_multi_function_and_reproducible() {
        let report = verify();
        assert!(report.parsed && report.semantically_valid && report.ownership_valid);
        assert!(report.native_compiled);
        assert_eq!(
            report.native_result,
            Some(crate::native::NativeValue::Int(10))
        );
        assert!(report.reproducible);
        assert_eq!(report.stage0.as_ref().unwrap().functions, 2);
        assert_eq!(
            report.stage0.as_ref().unwrap().instruction_count,
            report.stage1.as_ref().unwrap().instruction_count
        );
        assert!(!report.self_hosting_ready);
        assert!(report.stage2.is_some());
    }

    #[test]
    fn manifest_explicitly_models_three_bootstrap_stages() {
        let manifest = stage_manifest();
        assert_eq!(manifest.len(), 3);
        assert_eq!(
            manifest["stage2"],
            "Reserved for Ardisa compiler compiling itself"
        );
    }
}
