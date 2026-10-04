#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteropType {
    Int,
    Bool,
    Unit,
}

impl InteropType {
    pub fn rust_name(&self) -> &'static str {
        match self {
            Self::Int => "i64",
            Self::Bool => "bool",
            Self::Unit => "()",
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub const ABI_VERSION: &str = "ardisa-c-abi-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteropContract {
    pub abi_version: &'static str,
    pub function: RustFunction,
    pub unsafe_call_isolated: bool,
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
        if function
            .params
            .iter()
            .any(|(_, ty)| !matches!(ty, InteropType::Int | InteropType::Bool | InteropType::Unit))
            || !matches!(
                function.return_type,
                InteropType::Int | InteropType::Bool | InteropType::Unit
            )
        {
            return Err("Rust interop exposes only ABI-safe scalar types".into());
        }
        Ok(Self { function })
    }

    pub fn contract(&self) -> InteropContract {
        InteropContract {
            abi_version: ABI_VERSION,
            function: self.function.clone(),
            unsafe_call_isolated: true,
        }
    }

    pub fn function(&self) -> &RustFunction {
        &self.function
    }

    pub fn wrapper(&self) -> String {
        let f = &self.function;
        let params = f
            .params
            .iter()
            .map(|(name, ty)| format!("{name}: {}", ty.rust_name()))
            .collect::<Vec<_>>()
            .join(", ");
        let args = f
            .params
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            r#"unsafe extern "C" {{
    fn {symbol}({params}) -> {ret};
}}

fn {name}({params}) -> {ret} {{
    unsafe {{ {symbol}({args}) }}
}}
"#,
            symbol = f.symbol,
            name = f.name,
            params = params,
            ret = f.return_type.rust_name(),
            args = args,
        )
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
