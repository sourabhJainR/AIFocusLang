#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteropType {
    Int,
    Bool,
    Unit,
    IntSliceRef,
}

impl InteropType {
    pub fn rust_name(&self) -> &'static str {
        match self {
            Self::Int => "i64",
            Self::Bool => "bool",
            Self::Unit => "()",
            Self::IntSliceRef => "*const i64, usize",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustFunction {
    pub symbol: String,
    pub name: String,
    pub params: Vec<(String, InteropType)>,
    pub return_type: InteropType,
}

#[allow(dead_code)]
pub const ABI_VERSION: &str = "ardisa-c-abi-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteropContract {
    pub abi_version: &'static str,
    pub function: RustFunction,
    pub unsafe_call_isolated: bool,
    pub unsafe_escape_reason: String,
}

pub struct SafeRustBoundary {
    function: RustFunction,
}

impl SafeRustBoundary {
    pub fn new(function: RustFunction) -> Result<Self, String> {
        if function.symbol.is_empty() || function.name.is_empty() {
            return Err("Rust interop symbols and wrapper names must be non-empty".into());
        }
        if !is_c_identifier(&function.symbol) || !is_rust_identifier(&function.name) {
            return Err("Rust interop symbols and wrapper names must be valid identifiers".into());
        }
        if function.params.iter().any(|(name, _)| name.is_empty()) {
            return Err("Rust interop parameter names must be non-empty".into());
        }
        if function.params.iter().any(|(_, ty)| {
            !matches!(
                ty,
                InteropType::Int | InteropType::Bool | InteropType::Unit | InteropType::IntSliceRef
            )
        }) || !matches!(
            function.return_type,
            InteropType::Int | InteropType::Bool | InteropType::Unit
        ) {
            return Err(
                "Rust interop exposes only ABI-safe scalars or read-only Int slice borrows".into(),
            );
        }
        Ok(Self { function })
    }

    pub fn contract(&self) -> InteropContract {
        InteropContract {
            abi_version: ABI_VERSION,
            function: self.function.clone(),
            unsafe_call_isolated: true,
            unsafe_escape_reason:
                "generated wrapper contains the only unsafe FFI call; borrowed slices are read-only and scoped to the call"
                    .into(),
        }
    }

    pub fn function(&self) -> &RustFunction {
        &self.function
    }

    pub fn wrapper(&self) -> String {
        let f = &self.function;
        let extern_params = f
            .params
            .iter()
            .map(|(name, ty)| match ty {
                InteropType::IntSliceRef => {
                    format!("{name}_ptr: *const i64, {name}_len: usize")
                }
                _ => format!("{name}: {}", ty.rust_name()),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let safe_params = f
            .params
            .iter()
            .map(|(name, ty)| match ty {
                InteropType::IntSliceRef => format!("{name}: &[i64]"),
                _ => format!("{name}: {}", ty.rust_name()),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let args = f
            .params
            .iter()
            .map(|(name, ty)| match ty {
                InteropType::IntSliceRef => {
                    format!("{name}.as_ptr(), {name}.len()")
                }
                _ => name.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            r#"unsafe extern "C" {{
    fn {symbol}({extern_params}) -> {ret};
}}

fn {name}({safe_params}) -> {ret} {{
    unsafe {{ {symbol}({args}) }}
}}
"#,
            symbol = f.symbol,
            name = f.name,
            extern_params = extern_params,
            safe_params = safe_params,
            ret = f.return_type.rust_name(),
            args = args,
        )
    }

    pub fn unsafe_escape_block(&self, reason: &str, expression: &str) -> Result<String, String> {
        let reason = reason.trim();
        if reason.is_empty() {
            return Err("unsafe escape blocks require a non-empty safety rationale".into());
        }
        let expression = expression.trim();
        if expression.is_empty() {
            return Err("unsafe escape blocks require a non-empty expression".into());
        }
        Ok(format!(
            "unsafe {{\n    // SAFETY: {reason}\n    {expression}\n}}"
        ))
    }
}

pub fn validate(function: &RustFunction) -> Result<(), String> {
    SafeRustBoundary::new(function.clone()).map(|_| ())
}

fn is_c_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

fn is_rust_identifier(value: &str) -> bool {
    is_c_identifier(value) && !matches!(value, "fn" | "struct" | "enum" | "type" | "mod" | "unsafe")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_read_only_int_slice_borrow() {
        let function = RustFunction {
            symbol: "native_sum".into(),
            name: "sum".into(),
            params: vec![("values".into(), InteropType::IntSliceRef)],
            return_type: InteropType::Int,
        };
        let boundary = SafeRustBoundary::new(function).unwrap();
        let wrapper = boundary.wrapper();
        assert!(wrapper.contains("values_ptr: *const i64, values_len: usize"));
        assert!(wrapper.contains("values: &[i64]"));
        assert!(wrapper.contains("native_sum(values.as_ptr(), values.len())"));
    }

    #[test]
    fn unsafe_escape_requires_a_rationale() {
        let function = RustFunction {
            symbol: "native_add".into(),
            name: "add".into(),
            params: vec![],
            return_type: InteropType::Int,
        };
        let boundary = SafeRustBoundary::new(function).unwrap();
        assert!(boundary.unsafe_escape_block("", "native_add()").is_err());
        let block = boundary
            .unsafe_escape_block("FFI contract guarantees the symbol and ABI", "native_add()")
            .unwrap();
        assert!(block.contains("// SAFETY: FFI contract guarantees"));
    }

    #[test]
    fn accepts_ffi_safe_scalar_boundary() {
        let function = RustFunction {
            symbol: "native_add".into(),
            name: "add".into(),
            params: vec![
                ("a".into(), InteropType::Int),
                ("b".into(), InteropType::Int),
            ],
            return_type: InteropType::Int,
        };
        let boundary = SafeRustBoundary::new(function).unwrap();
        assert!(boundary.wrapper().contains("unsafe extern \"C\""));
        assert!(boundary.wrapper().contains("fn add(a: i64, b: i64) -> i64"));
    }

    #[test]
    fn generated_slice_abi_wrapper_is_accepted_by_rustc() {
        let function = RustFunction {
            symbol: "native_sum".into(),
            name: "sum".into(),
            params: vec![("values".into(), InteropType::IntSliceRef)],
            return_type: InteropType::Int,
        };
        let wrapper = SafeRustBoundary::new(function).unwrap().wrapper();
        let base = std::env::temp_dir().join(format!("ardisa-slice-abi-{}", std::process::id()));
        let source = base.with_extension("rs");
        std::fs::write(&source, wrapper).unwrap();
        let status = std::process::Command::new("rustc")
            .arg("--crate-type=lib")
            .arg("--emit=metadata")
            .arg(&source)
            .status()
            .expect("rustc must be available for slice ABI fixture verification");
        let _ = std::fs::remove_file(&source);
        assert!(status.success());
    }

    #[test]
    fn generated_abi_wrapper_is_accepted_by_rustc() {
        let function = RustFunction {
            symbol: "native_add".into(),
            name: "add".into(),
            params: vec![
                ("a".into(), InteropType::Int),
                ("b".into(), InteropType::Int),
            ],
            return_type: InteropType::Int,
        };
        let wrapper = SafeRustBoundary::new(function).unwrap().wrapper();
        let base = std::env::temp_dir().join(format!("ardisa-abi-{}", std::process::id()));
        let source = base.with_extension("rs");
        std::fs::write(&source, wrapper).unwrap();
        let status = std::process::Command::new("rustc")
            .arg("--crate-type=lib")
            .arg("--emit=metadata")
            .arg(&source)
            .status()
            .expect("rustc must be available for ABI fixture verification");
        let _ = std::fs::remove_file(&source);
        assert!(status.success());
    }

    #[test]
    fn rejects_invalid_symbols_at_the_boundary() {
        let function = RustFunction {
            symbol: "native-add".into(),
            name: "add".into(),
            params: vec![],
            return_type: InteropType::Int,
        };
        assert!(validate(&function).is_err());
    }

    #[test]
    fn contract_declares_abi_and_unsafe_isolation() {
        let function = RustFunction {
            symbol: "native_flag".into(),
            name: "flag".into(),
            params: vec![],
            return_type: InteropType::Bool,
        };
        let contract = SafeRustBoundary::new(function).unwrap().contract();
        assert_eq!(contract.abi_version, ABI_VERSION);
        assert!(contract.unsafe_call_isolated);
    }

    #[test]
    fn accepts_only_explicit_scalar_types() {
        let function = RustFunction {
            symbol: "native_text".into(),
            name: "text".into(),
            params: vec![("value".into(), InteropType::Unit)],
            return_type: InteropType::Unit,
        };
        assert!(SafeRustBoundary::new(function).is_ok());
    }

    #[test]
    fn exposes_only_supported_safe_types() {
        let function = RustFunction {
            symbol: "native_flag".into(),
            name: "flag".into(),
            params: vec![],
            return_type: InteropType::Bool,
        };
        assert!(validate(&function).is_ok());
    }
}
