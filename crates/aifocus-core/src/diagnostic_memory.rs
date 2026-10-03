use std::collections::HashMap;

use crate::source::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticHistoryEntry {
    pub code: &'static str,
    pub message: String,
    pub occurrences: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiagnosticMemory {
    entries: HashMap<&'static str, DiagnosticHistoryEntry>,
}

impl DiagnosticMemory {
    pub fn record(&mut self, diagnostic: &Diagnostic) {
        let entry = self
            .entries
            .entry(diagnostic.code)
            .or_insert_with(|| DiagnosticHistoryEntry {
                code: diagnostic.code,
                message: diagnostic.message.clone(),
                occurrences: 0,
            });
        entry.message = diagnostic.message.clone();
        entry.occurrences += 1;
    }

    pub fn observe_all(&mut self, diagnostics: &[Diagnostic]) {
        for diagnostic in diagnostics {
            self.record(diagnostic);
        }
    }

    pub fn history(&self, code: &str) -> Option<&DiagnosticHistoryEntry> {
        self.entries.get(code)
    }

    pub fn likely_recurring(&self, code: &str, threshold: usize) -> bool {
        self.history(code)
            .is_some_and(|entry| entry.occurrences >= threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Span;

    #[test]
    fn remembers_recurring_diagnostics() {
        let diagnostic = Diagnostic::error("AIF304", "unknown name 'x'", Some(Span::new(1, 2)));
        let mut memory = DiagnosticMemory::default();
        memory.record(&diagnostic);
        memory.record(&diagnostic);
        assert!(memory.likely_recurring("AIF304", 2));
        assert_eq!(
            memory.history("AIF304").unwrap().message,
            "unknown name 'x'"
        );
    }
}
