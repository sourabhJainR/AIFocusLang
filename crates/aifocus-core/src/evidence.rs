use std::collections::BTreeSet;

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

/// Trust boundary for validating evidence source authenticity and receipt provenance.
/// Implementations must verify fetched source bytes against the SHA-256 digest and validate
/// receipt authenticity using configured trusted identities. Returning Ok is an authorization
/// decision; the default evidence graph deliberately does not implement this trait.
pub trait EvidenceSourceVerifier {
    fn verify_source_backing(&self, envelope: &EvidenceEnvelope) -> Result<(), String>;
}

impl EvidenceGraph {
    pub fn add(&mut self, envelope: EvidenceEnvelope) {
        self.envelopes.push(envelope);
    }

    pub fn link(&mut self, from_episode: impl Into<String>, to_episode: impl Into<String>) {
        self.edges.push((from_episode.into(), to_episode.into()));
    }

    /// Check episode identity and edge references before treating the graph as evidence.
    /// Public fields remain available for serialization, so validation is repeated at use time.
    pub fn validate_integrity(&self) -> Result<(), String> {
        let mut episode_ids = BTreeSet::new();
        for envelope in &self.envelopes {
            if envelope.episode_id.trim().is_empty() {
                return Err("evidence graph contains an empty episode ID".into());
            }
            if !episode_ids.insert(envelope.episode_id.as_str()) {
                return Err(format!("duplicate episode ID '{}'", envelope.episode_id));
            }
        }
        for (from, to) in &self.edges {
            if from == to {
                return Err(format!("self-referential evidence edge for episode '{from}'"));
            }
            if !episode_ids.contains(from.as_str()) {
                return Err(format!("evidence edge references missing source episode '{from}'"));
            }
            if !episode_ids.contains(to.as_str()) {
                return Err(format!("evidence edge references missing target episode '{to}'"));
            }
        }
        Ok(())
    }

    /// Fail-closed promotion check. String metadata is not proof of source authenticity.
    /// Call `can_promote_with_verifier` with a trusted verifier to authorize promotion.
    pub fn can_promote(
        &self,
        capability: &str,
        _canary_passed: bool,
        _holdout_pass_rate: u8,
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
        if !envelopes.iter().all(|e| e.has_structural_source_backing()) {
            return CapabilityEvaluation {
                decision: CapabilityDecision::Hold,
                reason: "evidence lacks a source URI, SHA-256 digest, or verification receipt reference".into(),
            };
        }
        CapabilityEvaluation {
            decision: CapabilityDecision::Hold,
            reason: "source metadata is only structural; a trusted source and receipt verifier is required".into(),
        }
    }

    /// Evaluate promotion only after a trusted verifier authenticates every matching envelope.
    /// The verifier is the trust boundary: implementations must validate the actual source
    /// content/digest and the receipt against configured trusted identities, not just string shape.
    pub fn can_promote_with_verifier<V: EvidenceSourceVerifier>(
        &self,
        capability: &str,
        canary_passed: bool,
        holdout_pass_rate: u8,
        verifier: &V,
    ) -> CapabilityEvaluation {
        if let Err(reason) = self.validate_integrity() {
            return CapabilityEvaluation {
                decision: CapabilityDecision::Hold,
                reason: format!("evidence graph integrity failure: {reason}"),
            };
        }
        let envelopes = self.envelopes.iter()
            .filter(|e| e.capability == capability)
            .collect::<Vec<_>>();
        if envelopes.is_empty() {
            return CapabilityEvaluation {
                decision: CapabilityDecision::Hold,
                reason: "no evidence envelope exists for capability".into(),
            };
        }
        for envelope in &envelopes {
            if !envelope.has_structural_source_backing() {
                return CapabilityEvaluation {
                    decision: CapabilityDecision::Hold,
                    reason: "evidence lacks structurally valid source backing".into(),
                };
            }
            if let Err(reason) = verifier.verify_source_backing(envelope) {
                return CapabilityEvaluation {
                    decision: CapabilityDecision::Hold,
                    reason: format!("trusted evidence verification failed: {reason}"),
                };
            }
        }
        evaluate_verified_gates(canary_passed, holdout_pass_rate)
    }}

/// Evaluate policy thresholds without source authentication.
/// This function never authorizes promotion: use EvidenceGraph::can_promote_with_verifier.
pub fn evaluate(
    _canary_passed: bool,
    _holdout_pass_rate: u8,
    rollback_requested: bool,
) -> CapabilityEvaluation {
    if rollback_requested {
        CapabilityEvaluation {
            decision: CapabilityDecision::Rollback,
            reason: "rollback requested by verification evidence".into(),
        }
    } else {
        CapabilityEvaluation {
            decision: CapabilityDecision::Hold,
            reason: "policy thresholds alone are insufficient; trusted source verification is required".into(),
        }
    }
}

fn evaluate_verified_gates(
    canary_passed: bool,
    holdout_pass_rate: u8,
) -> CapabilityEvaluation {
    if canary_passed && holdout_pass_rate >= 95 {
        CapabilityEvaluation {
            decision: CapabilityDecision::Promote,
            reason: "trusted source verification passed, canary passed, and holdout pass rate is at least 95%".into(),
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
    fn policy_thresholds_alone_never_authorize_promotion() {
        let result = evaluate(true, 100, false);
        assert_eq!(result.decision, CapabilityDecision::Hold);
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
            CapabilityDecision::Hold
        );
        struct TestVerifier;
        impl EvidenceSourceVerifier for TestVerifier {
            fn verify_source_backing(&self, envelope: &EvidenceEnvelope) -> Result<(), String> {
                if envelope.episode_id == "episode-1" {
                    Ok(())
                } else {
                    Err("unrecognized test receipt".into())
                }
            }
        }
        assert_eq!(
            graph.can_promote_with_verifier("ownership-analysis", true, 100, &TestVerifier).decision,
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

    #[test]
    fn structurally_plausible_but_unverified_receipt_cannot_promote() {
        struct RejectAll;
        impl EvidenceSourceVerifier for RejectAll {
            fn verify_source_backing(&self, _envelope: &EvidenceEnvelope) -> Result<(), String> {
                Err("receipt signature is not trusted".into())
            }
        }
        let mut graph = EvidenceGraph::default();
        graph.add(EvidenceEnvelope::new(
            "episode-untrusted", "capability", "test", "passed", "claimed-verified",
            vec![
                "source_uri=https://example.invalid/source".into(),
                format!("source_sha256={}", "a".repeat(64)),
                "verification_receipt=made-up-run-id".into(),
            ],
        ));
        assert_eq!(graph.can_promote("capability", true, 100).decision, CapabilityDecision::Hold);
        let result = graph.can_promote_with_verifier("capability", true, 100, &RejectAll);
        assert_eq!(result.decision, CapabilityDecision::Hold);
        assert!(result.reason.contains("trusted evidence verification failed"));
    }

    #[test]
    fn promotion_rejects_dangling_duplicate_and_self_referential_edges() {
        struct AcceptAll;
        impl EvidenceSourceVerifier for AcceptAll {
            fn verify_source_backing(&self, _envelope: &EvidenceEnvelope) -> Result<(), String> {
                Ok(())
            }
        }
        let backed = |episode: &str| EvidenceEnvelope::new(
            episode, "capability", "test", "passed", "verified",
            vec![
                "source_uri=https://example.com/source".into(),
                format!("source_sha256={}", "b".repeat(64)),
                "verification_receipt=receipt-1".into(),
            ],
        );

        let mut dangling = EvidenceGraph::default();
        dangling.add(backed("one"));
        dangling.link("one", "missing");
        assert_eq!(dangling.can_promote_with_verifier("capability", true, 100, &AcceptAll).decision, CapabilityDecision::Hold);
        assert!(dangling.validate_integrity().unwrap_err().contains("missing target"));

        let mut duplicate = EvidenceGraph::default();
        duplicate.add(backed("same"));
        duplicate.add(backed("same"));
        assert!(duplicate.validate_integrity().unwrap_err().contains("duplicate episode ID"));
        assert_eq!(duplicate.can_promote_with_verifier("capability", true, 100, &AcceptAll).decision, CapabilityDecision::Hold);

        let mut self_edge = EvidenceGraph::default();
        self_edge.add(backed("one"));
        self_edge.link("one", "one");
        assert!(self_edge.validate_integrity().unwrap_err().contains("self-referential"));
    }

}
