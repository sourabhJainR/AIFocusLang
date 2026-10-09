//! AI Mode policy primitives.
//!
//! These types model security and verification policy independently of source
//! syntax. They do not create OS/hardware isolation and do not claim a program
//! is secure merely because a policy can be represented. Source parsing and
//! compiler enforcement are introduced in later phases.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownProofPolicy {
    /// Reject compilation when a required proof remains unknown.
    Reject,
    /// Permit the build only when the caller records the unknown result.
    Record,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    Clock,
    Random,
    FilesystemRead,
    FilesystemWrite,
    Network,
    Process,
    Secrets,
    UnsafeInterop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofRequirement {
    TypeSafety,
    Bounds,
    EffectPolicy,
    ContractPostconditions,
    ConstantTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofStatus {
    Proved,
    RuntimeChecked,
    Unknown,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TracePolicy {
    pub required: bool,
    /// Include a provider/model identifier and prompt/session digest only.
    /// Raw prompts, secrets, source, and embeddings are never implied.
    pub include_ai_metadata: bool,
    pub require_signature: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultPolicy {
    pub allowed_capabilities: BTreeSet<Capability>,
}

impl VaultPolicy {
    pub fn deny_all() -> Self {
        Self {
            allowed_capabilities: BTreeSet::new(),
        }
    }

    pub fn permits(&self, capability: Capability) -> bool {
        self.allowed_capabilities.contains(&capability)
    }

    /// Validate observed effects against explicit grants. This is a policy
    /// primitive; the compiler/runtime must call it at a real enforcement
    /// boundary for it to provide protection.
    pub fn validate_effects(
        &self,
        observed: impl IntoIterator<Item = Capability>,
    ) -> Result<(), PolicyViolation> {
        let denied = observed
            .into_iter()
            .filter(|capability| !self.permits(*capability))
            .collect::<BTreeSet<_>>();
        if denied.is_empty() {
            Ok(())
        } else {
            Err(PolicyViolation::UndeclaredCapabilities(denied))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofPolicy {
    pub required: BTreeSet<ProofRequirement>,
    pub on_unknown: UnknownProofPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiModePolicy {
    pub trace: TracePolicy,
    pub vault: VaultPolicy,
    pub proof: ProofPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyViolation {
    UndeclaredCapabilities(BTreeSet<Capability>),
    MissingProofResult(ProofRequirement),
    FailedProof(ProofRequirement),
    UnknownProof(ProofRequirement),
    MissingTrace,
    MissingSignature,
}

impl AiModePolicy {
    pub fn validate_proofs(
        &self,
        results: &BTreeMap<ProofRequirement, ProofStatus>,
    ) -> Result<(), Vec<PolicyViolation>> {
        let mut violations = Vec::new();
        for requirement in &self.proof.required {
            match results.get(requirement) {
                None => violations.push(PolicyViolation::MissingProofResult(*requirement)),
                Some(ProofStatus::Failed) => {
                    violations.push(PolicyViolation::FailedProof(*requirement))
                }
                Some(ProofStatus::Unknown)
                    if self.proof.on_unknown == UnknownProofPolicy::Reject =>
                {
                    violations.push(PolicyViolation::UnknownProof(*requirement))
                }
                // Runtime checks are evidence of runtime enforcement, not a
                // formal proof. Keep the status distinct in the report.
                Some(ProofStatus::Proved | ProofStatus::RuntimeChecked | ProofStatus::Unknown) => {}
            }
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }

    /// Validate build-attestation prerequisites. Cryptographic signing itself
    /// must be performed by a trusted signing service, not this policy model.
    pub fn validate_trace(
        &self,
        trace_present: bool,
        signature_verified: bool,
    ) -> Result<(), PolicyViolation> {
        if self.trace.required && !trace_present {
            return Err(PolicyViolation::MissingTrace);
        }
        if self.trace.required && self.trace.require_signature && !signature_verified {
            return Err(PolicyViolation::MissingSignature);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> AiModePolicy {
        AiModePolicy {
            trace: TracePolicy {
                required: true,
                include_ai_metadata: false,
                require_signature: true,
            },
            vault: VaultPolicy::deny_all(),
            proof: ProofPolicy {
                required: [ProofRequirement::TypeSafety, ProofRequirement::EffectPolicy]
                    .into_iter()
                    .collect(),
                on_unknown: UnknownProofPolicy::Reject,
            },
        }
    }

    #[test]
    fn vault_denies_undeclared_effects_and_accepts_explicit_grants() {
        let mut vault = VaultPolicy::deny_all();
        assert!(vault.validate_effects([Capability::Network]).is_err());
        vault.allowed_capabilities.insert(Capability::Clock);
        assert!(vault.validate_effects([Capability::Clock]).is_ok());
        assert!(vault.validate_effects([Capability::Clock, Capability::Network]).is_err());
    }

    #[test]
    fn required_proofs_fail_closed_when_missing_failed_or_unknown() {
        let policy = policy();
        assert!(policy.validate_proofs(&BTreeMap::new()).is_err());

        let results = [
            (ProofRequirement::TypeSafety, ProofStatus::Proved),
            (ProofRequirement::EffectPolicy, ProofStatus::Failed),
        ]
        .into_iter()
        .collect();
        assert!(policy.validate_proofs(&results).is_err());

        let results = [
            (ProofRequirement::TypeSafety, ProofStatus::Proved),
            (ProofRequirement::EffectPolicy, ProofStatus::Unknown),
        ]
        .into_iter()
        .collect();
        assert!(policy.validate_proofs(&results).is_err());
    }

    #[test]
    fn runtime_checked_is_not_mislabelled_as_formally_proved() {
        let policy = policy();
        let results = [
            (ProofRequirement::TypeSafety, ProofStatus::RuntimeChecked),
            (ProofRequirement::EffectPolicy, ProofStatus::Proved),
        ]
        .into_iter()
        .collect();
        assert!(policy.validate_proofs(&results).is_ok());
        assert_eq!(results[&ProofRequirement::TypeSafety], ProofStatus::RuntimeChecked);
    }

    #[test]
    fn trace_policy_requires_presence_and_configured_signature() {
        let policy = policy();
        assert_eq!(
            policy.validate_trace(false, false),
            Err(PolicyViolation::MissingTrace)
        );
        assert_eq!(
            policy.validate_trace(true, false),
            Err(PolicyViolation::MissingSignature)
        );
        assert!(policy.validate_trace(true, true).is_ok());
    }

    #[test]
    fn optional_trace_does_not_require_an_attestation() {
        let mut policy = policy();
        policy.trace.required = false;
        assert!(policy.validate_trace(false, false).is_ok());
    }
}
