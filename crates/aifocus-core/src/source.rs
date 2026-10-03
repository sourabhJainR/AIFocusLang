#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
}

impl Diagnostic {
    pub fn error(code: &'static str, message: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            span,
        }
    }

    pub fn location(&self, source: &str) -> Option<Location> {
        let span = self.span?;
        let prefix = &source[..span.start.min(source.len())];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column_start = prefix.rfind('\n').map_or(0, |index| index + 1);
        let column = source[column_start..span.start.min(source.len())]
            .chars()
            .count()
            + 1;
        Some(Location { line, column })
    }

    pub fn to_json(&self, source: &str) -> String {
        let location = self.location(source);
        let line = location.map_or("null".into(), |value| value.line.to_string());
        let column = location.map_or("null".into(), |value| value.column.to_string());
        format!(
            "{{\"code\":\"{}\",\"severity\":\"{}\",\"message\":\"{}\",\"line\":{},\"column\":{}}}",
            escape_json(self.code),
            self.severity.as_str(),
            escape_json(&self.message),
            line,
            column
        )
    }
}

fn escape_json(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_machine_readable_location() {
        let diagnostic = Diagnostic::error("AIF001", "bad", Some(Span::new(7, 8)));
        assert_eq!(
            diagnostic.location("module x\nfn"),
            Some(Location { line: 1, column: 8 })
        );
        assert!(diagnostic.to_json("module x\nfn").contains("\"code\":\"AIF001\""));
    }
}
