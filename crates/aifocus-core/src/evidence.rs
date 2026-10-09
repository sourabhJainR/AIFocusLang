#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceEnvelope {
    pub episode_id: String,
    pub capability: String,
    pub task_kind: String,
    pub outcome: String,
    pub verification: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityDecision {
    Promote,
    Hold,
    Rollback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityEvaluation {
    pub decision: CapabilityDecision,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EvidenceGraph {
    pub envelopes: Vec<EvidenceEnvelope>,
    pub edges: Vec<(String, String)>,
}

impl EvidenceGraph {
    pub fn add(&mut self, envelope: EvidenceEnvelope) {
        self.envelopes.push(envelope);
    }

    pub fn link(&mut self, from_episode: impl Into<String>, to_episode: impl Into<String>) {
        self.edges.push((from_episode.into(), to_episode.into()));
    }

    pub fn can_promote(
        &self,
        capability: &str,
        canary_passed: bool,
        holdout_pass_rate: u8,
    ) -> CapabilityEvaluation {
        let envelopes = self.envelopes.iter()
            .filter(|e| e.capability == capability)
            .collect::<Vec<_>>();
        if envelopes.is_empty() {
            return CapabilityEvaluation {
                decision: CapabilityDecision::Hold,
                reason: "no evidence envelope exists for capability".into(),
            };
        }
        if !envelopes.iter().any(|e| e.has_structural_source_backing()) {
            return CapabilityEvaluation {
                decision: CapabilityDecision::Hold,
                reason: "evidence lacks a source URI, SHA-256 digest, and verification receipt reference".into(),
            };
        }
        evaluate(canary_passed, holdout_pass_rate, false)
    }
}

pub fn evaluate(
    canary_passed: bool,
    holdout_pass_rate: u8,
    rollback_requested: bool,
) -> CapabilityEvaluation {
    if rollback_requested {
        return CapabilityEvaluation {
            decision: CapabilityDecision::Rollback,
            reason: "rollback requested by verification evidence".into(),
        };
    }
    if canary_passed && holdout_pass_rate >= 95 {
        CapabilityEvaluation {
            decision: CapabilityDecision::Promote,
            reason: "canary passed and holdout pass rate is at least 95%".into(),
        }
    } else {
        CapabilityEvaluation {
            decision: CapabilityDecision::Hold,
            reason: "promotion gate not satisfied".into(),
        }
    }
}

impl EvidenceEnvelope {
    /// Structural evidence gate only. A trusted verifier must validate the referenced
    /// source, digest, and receipt before a promotion decision is treated as authenticated.
    pub fn has_structural_source_backing(&self) -> bool {
        let has_source = self.evidence.iter().any(|item| {
            item.strip_prefix("source_uri=")
                .is_some_and(|uri| uri.starts_with("https://") && uri.len() > "https://".len())
        });
        let has_digest = self.evidence.iter().any(|item| {
            item.strip_prefix("source_sha256=").is_some_and(|digest| {
                digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit())
            })
        });
        let has_receipt = self.evidence.iter().any(|item| {
            item.strip_prefix("verification_receipt=")
                .is_some_and(|receipt| !receipt.trim().is_empty())
        });
        has_source && has_digest && has_receipt
    }

    pub fn new(
        episode_id: impl Into<String>,
        capability: impl Into<String>,
        task_kind: impl Into<String>,
        outcome: impl Into<String>,
        verification: impl Into<String>,
        evidence: Vec<String>,
    ) -> Self {
        Self {
            episode_id: episode_id.into(),
            capability: capability.into(),
            task_kind: task_kind.into(),
            outcome: outcome.into(),
            verification: verification.into(),
            evidence,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotes_only_after_canary_and_holdout_gate() {
        let result = evaluate(true, 95, false);
        assert_eq!(result.decision, CapabilityDecision::Promote);
    }

    #[test]
    fn holds_when_holdout_gate_is_not_met() {
        let result = evaluate(true, 94, false);
        assert_eq!(result.decision, CapabilityDecision::Hold);
    }

    #[test]
    fn evidence_graph_requires_evidence_before_promotion() {
        let graph = EvidenceGraph::default();
        assert_eq!(
            graph.can_promote("ownership-analysis", true, 100).decision,
            CapabilityDecision::Hold
        );
        let mut graph = graph;
        graph.add(EvidenceEnvelope::new(
            "episode-1",
            "ownership-analysis",
            "compiler-change",
            "passed",
            "deep",
            vec![
                "ci:green".into(),
                "source_uri=https://example.invalid/build-report.json".into(),
                format!("source_sha256={}", "a".repeat(64)),
                "verification_receipt=ci-run-1".into(),
            ],
        ));
        assert_eq!(
            graph.can_promote("ownership-analysis", true, 100).decision,
            CapabilityDecision::Promote
        );
    }

    #[test]
    fn evidence_without_source_digest_and_receipt_cannot_promote() {
        let mut graph = EvidenceGraph::default();
        graph.add(EvidenceEnvelope::new(
            "episode-2", "source-backed-capability", "compiler-change", "passed", "deep",
            vec!["ci:green".into(), "test:100".into()],
        ));
        assert_eq!(graph.can_promote("source-backed-capability", true, 100).decision, CapabilityDecision::Hold);
    }

    #[test]
    fn malformed_digest_is_not_structural_source_backing() {
        let envelope = EvidenceEnvelope::new(
            "episode-3", "x", "compiler-change", "passed", "deep",
            vec![
                "source_uri=https://example.invalid/source".into(),
                "source_sha256=not-a-digest".into(),
                "verification_receipt=ci-run-1".into(),
            ],
        );
        assert!(!envelope.has_structural_source_backing());
    }

    #[test]
    fn rollback_overrides_promotion() {
        let result = evaluate(true, 100, true);
        assert_eq!(result.decision, CapabilityDecision::Rollback);
    }

    #[test]
    fn envelope_carries_traceable_evidence() {
        let envelope = EvidenceEnvelope::new(
            "episode-1",
            "ownership-analysis",
            "compiler-change",
            "passed",
            "deep",
            vec![
                "ci:green".into(),
                "test:256".into(),
                "source_uri=https://example.invalid/build-report.json".into(),
                format!("source_sha256={}", "a".repeat(64)),
                "verification_receipt=ci-run-37953217310".into(),
            ],
        );
        assert_eq!(envelope.evidence.len(), 5);
        assert_eq!(envelope.episode_id, "episode-1");
    }
}
