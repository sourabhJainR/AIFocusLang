use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LearningKey {
    pub project: String,
    pub task_kind: String,
    pub diagnostic: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearningStatus {
    Observed,
    VerifiedRepair,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvenanceKind {
    Repair,
    Replay,
    Regression,
    Promotion,
    Rollback,
}

impl ProvenanceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Repair => "repair",
            Self::Replay => "replay",
            Self::Regression => "regression",
            Self::Promotion => "promotion",
            Self::Rollback => "rollback",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "repair" => Ok(Self::Repair),
            "replay" => Ok(Self::Replay),
            "regression" => Ok(Self::Regression),
            "promotion" => Ok(Self::Promotion),
            "rollback" => Ok(Self::Rollback),
            _ => Err("invalid Ardisa learning provenance kind".into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearningProvenance {
    pub id: String,
    pub kind: ProvenanceKind,
    pub diagnostic: String,
    pub parent_id: Option<String>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearningEntry {
    pub key: LearningKey,
    pub message: String,
    pub occurrences: usize,
    pub status: LearningStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PersistentCompilerLearning {
    entries: HashMap<LearningKey, LearningEntry>,
    provenance: Vec<LearningProvenance>,
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
                status: LearningStatus::Observed,
            });
        entry.message = diagnostic.message.clone();
        entry.occurrences += 1;
    }

    pub fn record_verified_repair(
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
        let occurrences = {
            let entry = self.entries.entry(key.clone()).or_insert_with(|| LearningEntry {
                key,
                message: diagnostic.message.clone(),
                occurrences: 0,
                status: LearningStatus::Observed,
            });
            entry.message = diagnostic.message.clone();
            entry.occurrences += 1;
            entry.status = LearningStatus::VerifiedRepair;
            entry.occurrences
        };
        self.record_provenance(
            format!("repair:{}:{}", diagnostic.code, occurrences),
            ProvenanceKind::Repair,
            diagnostic.code,
            None,
            vec!["verified-repair".into()],
        );
    }

    pub fn record_provenance(
        &mut self,
        id: impl Into<String>,
        kind: ProvenanceKind,
        diagnostic: impl Into<String>,
        parent_id: Option<String>,
        evidence: Vec<String>,
    ) {
        let id = id.into();
        if self.provenance.iter().any(|item| item.id == id) {
            return;
        }
        self.provenance.push(LearningProvenance {
            id,
            kind,
            diagnostic: diagnostic.into(),
            parent_id,
            evidence,
        });
    }

    pub fn link_replay(
        &mut self,
        id: impl Into<String>,
        parent_id: impl Into<String>,
        evidence: Vec<String>,
    ) {
        self.record_provenance(
            id,
            ProvenanceKind::Replay,
            "replay",
            Some(parent_id.into()),
            evidence,
        );
    }

    pub fn link_regression(
        &mut self,
        id: impl Into<String>,
        parent_id: impl Into<String>,
        diagnostic: impl Into<String>,
        evidence: Vec<String>,
    ) {
        self.record_provenance(
            id,
            ProvenanceKind::Regression,
            diagnostic,
            Some(parent_id.into()),
            evidence,
        );
    }

    pub fn record_promotion(
        &mut self,
        id: impl Into<String>,
        capability: impl Into<String>,
        evidence: Vec<String>,
    ) {
        self.record_provenance(id, ProvenanceKind::Promotion, capability, None, evidence);
    }

    pub fn record_rollback(
        &mut self,
        id: impl Into<String>,
        capability: impl Into<String>,
        evidence: Vec<String>,
    ) {
        self.record_provenance(id, ProvenanceKind::Rollback, capability, None, evidence);
    }

    pub fn provenance(&self) -> &[LearningProvenance] {
        &self.provenance
    }

    pub fn is_verified(&self, key: &LearningKey) -> bool {
        self.entries
            .get(key)
            .is_some_and(|entry| entry.status == LearningStatus::VerifiedRepair)
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
        let mut out = String::from("ARDISA-LEARNING-V3\n");
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
            out.push('\t');
            out.push_str(match entry.status {
                LearningStatus::Observed => "observed",
                LearningStatus::VerifiedRepair => "verified-repair",
            });
            out.push('\n');
        }
        let mut provenance = self.provenance.clone();
        provenance.sort_by(|a, b| a.id.cmp(&b.id));
        for item in provenance {
            out.push_str("P\t");
            out.push_str(&escape(&item.id));
            out.push('\t');
            out.push_str(item.kind.as_str());
            out.push('\t');
            out.push_str(&escape(&item.diagnostic));
            out.push('\t');
            out.push_str(&escape(item.parent_id.as_deref().unwrap_or("")));
            out.push('\t');
            out.push_str(&escape(&item.evidence.join("\u{001f}")));
            out.push('\n');
        }
        fs::write(path, out).map_err(|error| error.to_string())
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
        let mut lines = text.lines();
        let version = lines.next();
        if !matches!(
            version,
            Some("ARDISA-LEARNING-V1") | Some("ARDISA-LEARNING-V2")
        ) {
            return Err("unsupported Ardisa learning format".into());
        }
        let v3 = version == Some("ARDISA-LEARNING-V3");
        let v2 = v3 || version == Some("ARDISA-LEARNING-V2");
        let mut memory = Self::default();
        for line in lines {
            if v3 && line.starts_with("P\t") {
                let fields = line
                    .split('\t')
                    .skip(1)
                    .map(unescape)
                    .collect::<Result<Vec<_>, _>>()?;
                if fields.len() != 5 {
                    return Err("invalid Ardisa provenance record".into());
                }
                let evidence = if fields[4].is_empty() {
                    Vec::new()
                } else {
                    fields[4].split('\u{1f}').map(str::to_string).collect()
                };
                memory.provenance.push(LearningProvenance {
                    id: fields[0].clone(),
                    kind: ProvenanceKind::parse(&fields[1])?,
                    diagnostic: fields[2].clone(),
                    parent_id: (!fields[3].is_empty()).then(|| fields[3].clone()),
                    evidence,
                });
                continue;
            }
            let fields = line
                .split('\t')
                .map(unescape)
                .collect::<Result<Vec<_>, _>>()?;
            if (v2 && fields.len() != 6) || (!v2 && fields.len() != 5) {
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
            let status = if v2 {
                match fields[5].as_str() {
                    "observed" => LearningStatus::Observed,
                    "verified-repair" => LearningStatus::VerifiedRepair,
                    _ => return Err("invalid Ardisa learning status".into()),
                }
            } else {
                LearningStatus::Observed
            };
            memory.entries.insert(
                key.clone(),
                LearningEntry {
                    key,
                    message: fields[4].clone(),
                    occurrences,
                    status,
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
        assert!(!restored.is_verified(&key));
        let mut verified = memory.clone();
        verified.record_verified_repair("project-a", "compiler-edit", &diagnostic);
        assert!(verified.is_verified(&key));
        verified.link_replay(
            "replay-1",
            "repair:AIF304:2",
            vec!["stage2:fingerprint".into()],
        );
        verified.link_regression("regression-1", "replay-1", "AIF304", vec!["ci:red".into()]);
        verified.record_promotion("promotion-1", "ownership", vec!["holdout:100".into()]);
        verified.record_rollback("rollback-1", "ownership", vec!["regression-1".into()]);
        verified.save(&path).unwrap();
        let restored = PersistentCompilerLearning::load(&path).unwrap();
        assert_eq!(restored.provenance().len(), 5);
        assert!(
            restored
                .provenance()
                .iter()
                .any(|item| item.id == "replay-1")
        );
        let _ = fs::remove_file(path);
    }
}
