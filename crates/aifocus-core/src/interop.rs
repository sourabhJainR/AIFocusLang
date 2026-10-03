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
pub struct SafeRustBoundary {
    function: RustFunction,
}

impl SafeRustBoundary {
    pub fn new(function: RustFunction) -> Result<Self, String> {
        if function.symbol.is_empty() || function.name.is_empty() {
            return Err("Rust interop symbols and wrapper names must be non-empty".into());
        }
        if function.params.iter().any(|(name, _)| name.is_empty()) {
            return Err("Rust interop parameter names must be non-empty".into());
        }
        Ok(Self { function })
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
    }}

pub fn validate(function: &RustFunction) -> Result<(), String> {
    SafeRustBoundary::new(function.clone()).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ffi_safe_scalar_boundary() {
        let function = RustFunction {
            symbol: "native_add".into(),
            name: "add".into(),
            params: vec![("a".into(), InteropType::Int), ("b".into(), InteropType::Int)],
            return_type: InteropType::Int,
        };
        let boundary = SafeRustBoundary::new(function).unwrap();
        assert!(boundary.wrapper().contains("unsafe extern \"C\""));
        assert!(boundary.wrapper().contains("fn add(a: i64, b: i64) -> i64"));
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
