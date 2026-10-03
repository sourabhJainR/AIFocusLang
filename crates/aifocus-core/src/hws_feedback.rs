#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationDepth {
    Basic,
    Standard,
    Deep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpisodeOutcome {
    Passed,
    Failed,
    Repaired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityFeedback {
    pub capability: String,
    pub task_kind: String,
    pub outcome: EpisodeOutcome,
    pub verification: VerificationDepth,
    pub iterations: u32,
}

impl CapabilityFeedback {
    pub fn is_positive(&self) -> bool {
        matches!(
            self.outcome,
            EpisodeOutcome::Passed | EpisodeOutcome::Repaired
        )
    }

    pub fn should_deepen_verification(&self) -> bool {
        matches!(self.outcome, EpisodeOutcome::Failed) || self.iterations > 3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_episode_requests_deeper_verification() {
        let feedback = CapabilityFeedback {
            capability: "ownership-analysis".into(),
            task_kind: "compiler-change".into(),
            outcome: EpisodeOutcome::Failed,
            verification: VerificationDepth::Standard,
            iterations: 1,
        };
        assert!(!feedback.is_positive());
        assert!(feedback.should_deepen_verification());
    }

    #[test]
    fn repaired_episode_is_positive_but_high_iteration_is_a_signal() {
        let feedback = CapabilityFeedback {
            capability: "type-inference".into(),
            task_kind: "compiler-change".into(),
            outcome: EpisodeOutcome::Repaired,
            verification: VerificationDepth::Deep,
            iterations: 4,
        };
        assert!(feedback.is_positive());
        assert!(feedback.should_deepen_verification());
    }
}
