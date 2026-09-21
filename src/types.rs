use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::profile::ExecutionProfile;

/// Text or structured context. Numbers, booleans and null are not valid at the top level.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Context {
    Text(String),
    Object(BTreeMap<String, Value>),
    Array(Vec<Value>),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoulCriteria {
    #[serde(rename = "true", skip_serializing_if = "Option::is_none")]
    pub yes: Option<Context>,
    #[serde(rename = "false", skip_serializing_if = "Option::is_none")]
    pub no: Option<Context>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Question {
    Noul {
        instructions: Context,
        #[serde(skip_serializing_if = "Option::is_none")]
        criteria: Option<NoulCriteria>,
    },
    Choice {
        instructions: Context,
        #[schemars(length(min = 1, max = 255))]
        criteria: BTreeMap<String, Option<Context>>,
    },
    Score {
        instructions: Context,
        #[schemars(length(min = 2, max = 10))]
        criteria: Vec<Context>,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoulInput {
    pub state: Context,
    pub instructions: Context,
    pub criteria: Option<NoulCriteria>,
    /// Defaults to TYPESAFE_MODEL, or jev-latest.
    pub model: Option<String>,
    /// Overrides the server's JEV_PROFILE for this call.
    pub profile: Option<ExecutionProfile>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChoiceInput {
    pub state: Context,
    pub instructions: Context,
    /// Map of option names to descriptions; null means no description. Maximum 255 options.
    #[schemars(length(min = 1, max = 255))]
    pub criteria: BTreeMap<String, Option<Context>>,
    pub model: Option<String>,
    /// Overrides the server's JEV_PROFILE for this call.
    pub profile: Option<ExecutionProfile>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScoreInput {
    pub state: Context,
    pub instructions: Context,
    /// Ordered descriptions of 2 to 10 levels. The score ranges from 0 to length minus 1.
    #[schemars(length(min = 2, max = 10))]
    pub criteria: Vec<Context>,
    pub model: Option<String>,
    /// Overrides the server's JEV_PROFILE for this call.
    pub profile: Option<ExecutionProfile>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BatchInput {
    pub state: Context,
    /// Independent questions evaluated against the same state; answers use these IDs.
    #[schemars(length(min = 1))]
    pub questions: BTreeMap<String, Question>,
    pub model: Option<String>,
    /// Overrides the server's JEV_PROFILE for this call.
    pub profile: Option<ExecutionProfile>,
}

impl BatchInput {
    pub fn single(
        state: Context,
        question: Question,
        model: Option<String>,
        profile: Option<ExecutionProfile>,
    ) -> Self {
        Self {
            state,
            questions: BTreeMap::from([("result".into(), question)]),
            model,
            profile,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.model.as_ref().is_some_and(|s| s.trim().is_empty()) {
            return Err("model must not be blank".into());
        }
        if self.questions.is_empty() {
            return Err("questions must contain at least one question".into());
        }
        for (id, question) in &self.questions {
            if id.trim().is_empty() {
                return Err("question IDs must not be blank".into());
            }
            match question {
                Question::Noul { .. } => {}
                Question::Choice { criteria, .. } => {
                    if !(1..=255).contains(&criteria.len()) {
                        return Err("Choice requires 1 to 255 options".into());
                    }
                    if criteria.keys().any(|key| key.trim().is_empty()) {
                        return Err("Choice option names must not be blank".into());
                    }
                }
                Question::Score { criteria, .. } => {
                    if !(2..=10).contains(&criteria.len()) {
                        return Err("Score requires 2 to 10 levels".into());
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct Evaluation {
    pub model: String,
    pub answers: BTreeMap<String, Answer>,
    pub usage: Usage,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    Noul {
        noul: f64,
    },
    Choice {
        choice: String,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
    Score {
        score: f64,
        legend: BTreeMap<String, Value>,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
}

impl Evaluation {
    pub fn matches(&self, questions: &BTreeMap<String, Question>) -> bool {
        // Reading usage also ensures that the API supplied nonnegative integer counts.
        let _usage = (self.usage.input_tokens, self.usage.output_tokens);
        !self.model.trim().is_empty()
            && self.answers.len() == questions.len()
            && questions.iter().all(|(id, q)| {
                let Some(answer) = self.answers.get(id) else {
                    return false;
                };
                match (q, answer) {
                    (Question::Noul { .. }, Answer::Noul { noul }) => unit(*noul),
                    (
                        Question::Choice { criteria, .. },
                        Answer::Choice {
                            choice,
                            probabilities,
                            confidence,
                        },
                    ) => {
                        criteria.contains_key(choice)
                            && criteria.keys().eq(probabilities.keys())
                            && distribution(probabilities)
                            && unit(*confidence)
                    }
                    (
                        Question::Score { criteria, .. },
                        Answer::Score {
                            score,
                            legend,
                            probabilities,
                            confidence,
                        },
                    ) => {
                        let keys: BTreeMap<_, _> =
                            (0..criteria.len()).map(|i| (i.to_string(), ())).collect();
                        keys.keys().eq(probabilities.keys())
                            && keys.keys().eq(legend.keys())
                            && distribution(probabilities)
                            && unit(*confidence)
                            && score.is_finite()
                            && (0.0..=(criteria.len() - 1) as f64).contains(score)
                    }
                    _ => false,
                }
            })
    }
}

fn unit(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn distribution(values: &BTreeMap<String, f64>) -> bool {
    values.values().all(|v| unit(*v)) && (values.values().sum::<f64>() - 1.0).abs() < 0.001
}
