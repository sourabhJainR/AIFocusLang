//! Core language model for AIFocusLang.

pub mod source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub name: String,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Function(Function),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub params: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Bool,
    String,
    Unit,
    Named(String),
    Result(Box<Type>, Box<Type>),
}

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
