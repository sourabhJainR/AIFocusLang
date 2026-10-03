//! Core language model and parser for AIFocusLang.

pub mod source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module { pub name: String, pub items: Vec<Item> }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item { Function(Function) }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub params: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter { pub name: String, pub ty: Type }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type { Int, Bool, String, Unit, Named(String), Result(Box<Type>, Box<Type>) }

impl Type {
    pub fn display_name(&self) -> String {
        match self {
            Self::Int => "Int".into(),
            Self::Bool => "Bool".into(),
            Self::String => "String".into(),
            Self::Unit => "()".into(),
            Self::Named(name) => name.clone(),
            Self::Result(ok, err) => format!("Result<{}, {}>", ok.display_name(), err.display_name()),
        }
    }
}

pub fn parse(source: &str) -> Result<Module, Vec<source::Diagnostic>> {
    let mut lines = source.lines().enumerate().peekable();
    let mut module_name = None;
    let mut items = Vec::new();
    let mut errors = Vec::new();

    while let Some(&(line_no, raw)) = lines.peek() {
        let line = raw.trim();
        if line.is_empty() { lines.next(); continue; }

        if let Some(name) = line.strip_prefix("module ") {
            lines.next();
            if module_name.is_some() {
                errors.push(source::Diagnostic::error("AIF002", "duplicate module declaration", Some(source::Span::new(line_no, line_no + 1))));
            } else if valid_name(name.trim()) {
                module_name = Some(name.trim().to_owned());
            } else {
                errors.push(source::Diagnostic::error("AIF003", "invalid module name", Some(source::Span::new(line_no, line_no + 1))));
            }
            continue;
        }

        if line.starts_with("fn ") {
            lines.next();
            match parse_function(line, &mut lines) {
                Ok(function) => items.push(Item::Function(function)),
                Err(message) => errors.push(source::Diagnostic::error("AIF004", message, Some(source::Span::new(line_no, line_no + 1)))),
            }
            continue;
        }

        lines.next();
        errors.push(source::Diagnostic::error("AIF005", format!("unexpected declaration: {line}"), Some(source::Span::new(line_no, line_no + 1))));
    }

    if module_name.is_none() {
        errors.push(source::Diagnostic::error("AIF006", "missing module declaration", None));
    }

    if errors.is_empty() {
        Ok(Module { name: module_name.expect("checked above"), items })
    } else {
        Err(errors)
    }
}

fn parse_function<'a, I>(signature: &str, lines: &mut std::iter::Peekable<I>) -> Result<Function, String>
where I: Iterator<Item = (usize, &'a str)>
{
    let rest = signature.strip_prefix("fn ").ok_or("invalid function declaration")?;
    let open = rest.find('(').ok_or("function parameters are required")?;
    let close = rest.rfind(')').ok_or("missing ')' in function declaration")?;
    let name = rest[..open].trim();
    if !valid_name(name) { return Err("invalid function name".into()); }

    let params_text = &rest[open + 1..close];
    let params = if params_text.trim().is_empty() {
        Vec::new()
    } else {
        let mut params = Vec::new();
        for part in params_text.split(',') {
            let (name, ty) = part.trim().split_once(':').ok_or("parameters require name: Type")?;
            if !valid_name(name.trim()) { return Err("invalid parameter name".into()); }
            params.push(Parameter { name: name.trim().into(), ty: parse_type(ty.trim())? });
        }
        params
    };

    let tail = rest[close + 1..].trim();
    let return_type = if tail.is_empty() {
        None
    } else if let Some(ty) = tail.strip_prefix("->") {
        Some(parse_type(ty.trim())?)
    } else {
        return Err("expected -> ReturnType".into());
    };

    let mut body = String::new();
    while let Some(&(_, raw)) = lines.peek() {
        if raw.trim().is_empty() {
            lines.next();
            if !body.is_empty() { body.push('\n'); }
            continue;
        }
        if raw.chars().next().is_none_or(|c| !c.is_whitespace()) { break; }
        lines.next();
        if !body.is_empty() { body.push('\n'); }
        body.push_str(raw.trim());
    }

    Ok(Function { name: name.into(), params, return_type, body })
}

fn parse_type(value: &str) -> Result<Type, String> {
    match value {
        "Int" => Ok(Type::Int),
        "Bool" => Ok(Type::Bool),
        "String" => Ok(Type::String),
        "()" => Ok(Type::Unit),
        _ if value.starts_with("Result<") && value.ends_with('>') => {
            let inner = &value[7..value.len() - 1];
            let (ok, err) = inner.split_once(',').ok_or("Result requires two types")?;
            Ok(Type::Result(Box::new(parse_type(ok.trim())?), Box::new(parse_type(err.trim())?)))
        }
        _ if valid_name(value) => Ok(Type::Named(value.into())),
        _ => Err(format!("unknown type {value}")),
    }
}

fn valid_name(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_small_program() {
        let module = parse("module hello\n\nfn add(a: Int, b: Int) -> Int\n  a + b\n").unwrap();
        assert_eq!(module.name, "hello");
        assert_eq!(module.items.len(), 1);
        let Item::Function(function) = &module.items[0];
        assert_eq!(function.name, "add");
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.return_type, Some(Type::Int));
        assert_eq!(function.body, "a + b");
    }

    #[test]
    fn parses_multiple_functions() {
        let module = parse("module hello\nfn one() -> Int\n  1\nfn two() -> Int\n  2\n").unwrap();
        assert_eq!(module.items.len(), 2);
        assert_eq!(module.items[1], Item::Function(Function { name: "two".into(), params: vec![], return_type: Some(Type::Int), body: "2".into() }));
    }

    #[test]
    fn reports_missing_module() {
        let errors = parse("fn main() -> Int\n  1\n").unwrap_err();
        assert!(errors.iter().any(|e| e.code == "AIF006"));
    }
}
