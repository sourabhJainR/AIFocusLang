use crate::{ir, native, ownership, parse, sema};
use std::collections::BTreeMap;

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
    pub reproducible: bool,
    pub self_hosting_ready: bool,
    pub blocker: Option<&'static str>,
}

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
            .map(Vec::len)
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
            .map(Vec::len)
            .sum(),
    );

    let main_code = stage1_program
        .functions
        .get("main")
        .expect("bootstrap main must compile");
    let native_result = native::run(
        main_code,
        &[
            ("a".into(), native::NativeValue::Int(3)),
            ("b".into(), native::NativeValue::Int(4)),
        ],
    )
    .ok();

    let reproducible = stage0.source_fingerprint == stage1.source_fingerprint
        && stage0.instruction_count == stage1.instruction_count
        && stage0.functions == stage1.functions;

    BootstrapReport {
        parsed: true,
        semantically_valid: true,
        ownership_valid: true,
        native_compiled: true,
        native_result,
        stage0: Some(stage0),
        stage1: Some(stage1),
        reproducible,
        self_hosting_ready: false,
        blocker: Some(
            "full self-hosting still requires the compiler implementation itself to be expressible in Ardisa, including source processing, collections, control flow, diagnostics, and module/runtime support",
        ),
    }
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
        reproducible: false,
        self_hosting_ready: false,
        blocker: Some(blocker),
    }
}

fn artifact(
    stage: u8,
    source: &str,
    module: &crate::ast::Module,
    instruction_count: usize,
) -> BootstrapArtifact {
    BootstrapArtifact {
        stage,
        source_fingerprint: fingerprint(source),
        instruction_count,
        functions: module.items.len(),
    }
}

fn fingerprint(source: &str) -> u64 {
    source.bytes().fold(0xcbf29ce484222325u64, |hash, byte| {
        hash.wrapping_mul(0x100000001b3).wrapping_add(u64::from(byte))
    })
}

pub fn stage_manifest() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("stage0", "Rust-hosted Ardisa compiler primitives"),
        (
            "stage1",
            "Native Ardisa program compiled by the same deterministic pipeline",
        ),
        (
            "stage2",
            "Reserved for Ardisa compiler compiling itself",
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_pipeline_is_multi_function_and_reproducible() {
        let report = verify();
        assert!(
            report.parsed && report.semantically_valid && report.ownership_valid
        );
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
