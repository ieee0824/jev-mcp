use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::types::{Answer, Evaluation};

const POLICY_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionDisposition {
    Accept,
    Verify,
    Reevaluate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionReason {
    HighConfidence,
    ModerateConfidence,
    LowConfidence,
    HighCertainty,
    ModerateCertainty,
    LowCertainty,
    OpenChoiceSelected,
    EvaluationFailed,
}

#[derive(Debug, Serialize)]
pub struct PolicyDecision {
    pub disposition: DecisionDisposition,
    pub reason: DecisionReason,
}

#[derive(Debug, Serialize)]
pub struct PolicyAssessment {
    pub version: u8,
    pub answers: BTreeMap<String, PolicyDecision>,
}

#[derive(Debug, Serialize)]
pub struct FailureAssessment {
    pub version: u8,
    pub disposition: DecisionDisposition,
    pub reason: DecisionReason,
}

#[derive(Debug, Clone)]
pub struct PolicyConfig {
    accept_threshold: f64,
    verify_threshold: f64,
    open_choices: BTreeSet<String>,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self::new(
            0.90,
            0.60,
            [
                "unknown",
                "other",
                "unclear",
                "insufficient_evidence",
                "insufficient_information",
                "not_enough_information",
                "none_of_the_above",
            ],
        )
        .expect("default policy is valid")
    }
}

impl PolicyConfig {
    pub fn from_env() -> Result<Self, String> {
        let defaults = Self::default();
        let accept = read_threshold("JEV_POLICY_ACCEPT_THRESHOLD", defaults.accept_threshold)?;
        let verify = read_threshold("JEV_POLICY_VERIFY_THRESHOLD", defaults.verify_threshold)?;
        let choices = match std::env::var("JEV_POLICY_OPEN_CHOICES") {
            Err(std::env::VarError::NotPresent) => defaults.open_choices,
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err("JEV_POLICY_OPEN_CHOICES must be valid UTF-8".into());
            }
            Ok(value) => value
                .split(',')
                .map(normalize_choice)
                .filter(|value| !value.is_empty())
                .collect(),
        };
        Self::new(accept, verify, choices)
    }

    pub fn new(
        accept_threshold: f64,
        verify_threshold: f64,
        open_choices: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, String> {
        if !accept_threshold.is_finite()
            || !verify_threshold.is_finite()
            || !(0.0..=1.0).contains(&accept_threshold)
            || !(0.0..=accept_threshold).contains(&verify_threshold)
        {
            return Err("policy thresholds must satisfy 0 <= verify <= accept <= 1".into());
        }
        let open_choices: BTreeSet<_> = open_choices
            .into_iter()
            .map(|value| normalize_choice(value.as_ref()))
            .filter(|value| !value.is_empty())
            .collect();
        Ok(Self {
            accept_threshold,
            verify_threshold,
            open_choices,
        })
    }

    pub fn assess(&self, evaluation: &Evaluation) -> PolicyAssessment {
        PolicyAssessment {
            version: POLICY_VERSION,
            answers: evaluation
                .answers
                .iter()
                .map(|(id, answer)| (id.clone(), self.assess_answer(answer)))
                .collect(),
        }
    }

    pub fn failure() -> FailureAssessment {
        FailureAssessment {
            version: POLICY_VERSION,
            disposition: DecisionDisposition::Reevaluate,
            reason: DecisionReason::EvaluationFailed,
        }
    }

    fn assess_answer(&self, answer: &Answer) -> PolicyDecision {
        match answer {
            Answer::Noul { noul } => {
                let certainty = (noul - 0.5).abs() * 2.0;
                self.decision_for_signal(
                    certainty,
                    DecisionReason::HighCertainty,
                    DecisionReason::ModerateCertainty,
                    DecisionReason::LowCertainty,
                )
            }
            Answer::Choice {
                choice, confidence, ..
            } => {
                if self.open_choices.contains(&normalize_choice(choice)) {
                    PolicyDecision {
                        disposition: DecisionDisposition::Reevaluate,
                        reason: DecisionReason::OpenChoiceSelected,
                    }
                } else {
                    self.decision_for_signal(
                        *confidence,
                        DecisionReason::HighConfidence,
                        DecisionReason::ModerateConfidence,
                        DecisionReason::LowConfidence,
                    )
                }
            }
            Answer::Score { confidence, .. } => self.decision_for_signal(
                *confidence,
                DecisionReason::HighConfidence,
                DecisionReason::ModerateConfidence,
                DecisionReason::LowConfidence,
            ),
        }
    }

    fn decision_for_signal(
        &self,
        signal: f64,
        high: DecisionReason,
        moderate: DecisionReason,
        low: DecisionReason,
    ) -> PolicyDecision {
        if at_or_above(signal, self.accept_threshold) {
            PolicyDecision {
                disposition: DecisionDisposition::Accept,
                reason: high,
            }
        } else if at_or_above(signal, self.verify_threshold) {
            PolicyDecision {
                disposition: DecisionDisposition::Verify,
                reason: moderate,
            }
        } else {
            PolicyDecision {
                disposition: DecisionDisposition::Reevaluate,
                reason: low,
            }
        }
    }
}

fn at_or_above(value: f64, threshold: f64) -> bool {
    value >= threshold || (value - threshold).abs() <= 1e-12
}

fn read_threshold(name: &str, default: f64) -> Result<f64, String> {
    match std::env::var(name) {
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{name} must be valid UTF-8")),
        Ok(value) => value
            .parse::<f64>()
            .map_err(|_| format!("{name} must be a number between 0 and 1")),
    }
}

fn normalize_choice(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn assess(answer: serde_json::Value) -> PolicyDecision {
        let evaluation: Evaluation = serde_json::from_value(json!({
            "model": "jev-test",
            "answers": {"result": answer},
            "usage": {"input_tokens": 1, "output_tokens": 1}
        }))
        .unwrap();
        PolicyConfig::default()
            .assess(&evaluation)
            .answers
            .remove("result")
            .unwrap()
    }

    #[test]
    fn confidence_boundaries_are_deterministic() {
        for (confidence, disposition) in [
            (0.90, DecisionDisposition::Accept),
            (0.60, DecisionDisposition::Verify),
            (0.59, DecisionDisposition::Reevaluate),
        ] {
            let decision = assess(json!({
                "type":"choice", "choice":"worker",
                "probabilities":{"worker":1.0}, "confidence":confidence
            }));
            assert_eq!(decision.disposition, disposition);
        }
    }

    #[test]
    fn score_uses_its_confidence_without_changing_the_score_distribution() {
        let decision = assess(json!({
            "type":"score", "score":1.4,
            "legend":{"0":"low", "1":"medium", "2":"high"},
            "probabilities":{"0":0.1, "1":0.4, "2":0.5},
            "confidence":0.59
        }));
        assert_eq!(decision.disposition, DecisionDisposition::Reevaluate);
        assert_eq!(decision.reason, DecisionReason::LowConfidence);
    }

    #[test]
    fn noul_uses_distance_from_uncertainty_not_yes_probability_as_confidence() {
        assert_eq!(
            assess(json!({"type":"noul", "noul":0.95})).disposition,
            DecisionDisposition::Accept
        );
        assert_eq!(
            assess(json!({"type":"noul", "noul":0.05})).disposition,
            DecisionDisposition::Accept
        );
        assert_eq!(
            assess(json!({"type":"noul", "noul":0.80})).disposition,
            DecisionDisposition::Verify
        );
        assert_eq!(
            assess(json!({"type":"noul", "noul":0.50})).disposition,
            DecisionDisposition::Reevaluate
        );
    }

    #[test]
    fn open_choices_reevaluate_even_with_high_confidence() {
        for choice in ["unknown", "Other", "insufficient-evidence"] {
            let decision = assess(json!({
                "type":"choice", "choice":choice,
                "probabilities":{choice:1.0}, "confidence":1.0
            }));
            assert_eq!(decision.disposition, DecisionDisposition::Reevaluate);
            assert_eq!(decision.reason, DecisionReason::OpenChoiceSelected);
        }
    }

    #[test]
    fn validates_configurable_thresholds_and_open_choices() {
        assert!(PolicyConfig::new(0.8, 0.4, ["unclear"]).is_ok());
        assert!(PolicyConfig::new(0.5, 0.6, ["unknown"]).is_err());
        assert!(PolicyConfig::new(0.9, 0.6, [""]).is_ok());
    }
}
