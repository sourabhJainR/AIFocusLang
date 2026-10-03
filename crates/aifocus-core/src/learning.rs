use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LearningKey {
    pub project: String,
    pub task_kind: String,
    pub diagnostic: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearningEntry {
    pub key: LearningKey,
    pub message: String,
    pub occurrences: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PersistentCompilerLearning {
    entries: HashMap<LearningKey, LearningEntry>,
}

impl PersistentCompilerLearning {
    pub fn record(
        &mut self,
        project: impl Into<String>,
        task_kind: impl Into<String>,
        diagnostic: &crate::source::Diagnostic,
    ) {
        let key = LearningKey {
            project: project.into(),
            task_kind: task_kind.into(),
            diagnostic: diagnostic.code.into(),
        };
        let entry = self
            .entries
            .entry(key.clone())
            .or_insert_with(|| LearningEntry {
                key,
                message: diagnostic.message.clone(),
                occurrences: 0,
            });
        entry.message = diagnostic.message.clone();
        entry.occurrences += 1;
    }

    pub fn recurring(&self, key: &LearningKey, threshold: usize) -> bool {
        self.entries
            .get(key)
            .is_some_and(|entry| entry.occurrences >= threshold)
    }

    pub fn entries(&self) -> impl Iterator<Item = &LearningEntry> {
        self.entries.values()
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let mut out = String::from("ARDISA-LEARNING-V1\n");
        let mut entries = self.entries.values().collect::<Vec<_>>();
        entries.sort_by(|a, b| {
            (&a.key.project, &a.key.task_kind, &a.key.diagnostic).cmp(&(
                &b.key.project,
                &b.key.task_kind,
                &b.key.diagnostic,
            ))
        });
        for entry in entries {
            out.push_str(&escape(&entry.key.project));
            out.push('\t');
            out.push_str(&escape(&entry.key.task_kind));
            out.push('\t');
            out.push_str(&escape(&entry.key.diagnostic));
            out.push('\t');
            out.push_str(&entry.occurrences.to_string());
            out.push('\t');
            out.push_str(&escape(&entry.message));
            out.push('\n');
        }
        fs::write(path, out).map_err(|error| error.to_string())
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
        let mut lines = text.lines();
        if lines.next() != Some("ARDISA-LEARNING-V1") {
            return Err("unsupported Ardisa learning format".into());
        }
        let mut memory = Self::default();
        for line in lines {
            let fields = line
                .split('\t')
                .map(unescape)
                .collect::<Result<Vec<_>, _>>()?;
            if fields.len() != 5 {
                return Err("invalid Ardisa learning record".into());
            }
            let occurrences = fields[3]
                .parse::<usize>()
                .map_err(|error| error.to_string())?;
            let key = LearningKey {
                project: fields[0].clone(),
                task_kind: fields[1].clone(),
                diagnostic: fields[2].clone(),
            };
            memory.entries.insert(
                key.clone(),
                LearningEntry {
                    key,
                    message: fields[4].clone(),
                    occurrences,
                },
            );
        }
        Ok(memory)
    }
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn unescape(value: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            match ch {
                't' => out.push('\t'),
                'n' => out.push('\n'),
                '\\' => out.push('\\'),
                _ => return Err("invalid escape in Ardisa learning record".into()),
            }
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            out.push(ch);
        }
    }
    if escaped {
        return Err("trailing escape in Ardisa learning record".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Span;

    #[test]
    fn persists_contextual_learning() {
        let diagnostic =
            crate::source::Diagnostic::error("AIF304", "unknown name 'x'", Some(Span::new(1, 2)));
        let mut memory = PersistentCompilerLearning::default();
        memory.record("project-a", "compiler-edit", &diagnostic);
        let path = std::env::temp_dir().join(format!("ardisa-learning-{}.txt", std::process::id()));
        memory.save(&path).unwrap();
        let restored = PersistentCompilerLearning::load(&path).unwrap();
        let key = LearningKey {
            project: "project-a".into(),
            task_kind: "compiler-edit".into(),
            diagnostic: "AIF304".into(),
        };
        assert!(restored.recurring(&key, 1));
        let _ = fs::remove_file(path);
    }
}
