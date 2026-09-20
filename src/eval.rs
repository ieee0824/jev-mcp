use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use serde::Deserialize;

use crate::types::{BatchInput, Context, Question};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalCase {
    pub id: String,
    pub state: Context,
    pub question: Question,
    pub expected: Expected,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Expected {
    Noul { min: f64, max: f64 },
    Choice { allowed: Vec<String> },
    Score { min: f64, max: f64 },
}

pub fn validate_file(path: &Path) -> Result<usize, String> {
    let file = File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    validate_reader(BufReader::new(file))
}

pub fn validate_reader(reader: impl BufRead) -> Result<usize, String> {
    let mut ids = BTreeSet::new();
    let mut count = 0;
    for (index, line) in reader.lines().enumerate() {
        let line_number = index + 1;
        let line = line.map_err(|error| format!("line {line_number}: {error}"))?;
        if line.trim().is_empty() {
            return Err(format!("line {line_number}: blank lines are not allowed"));
        }
        let case: EvalCase = serde_json::from_str(&line)
            .map_err(|error| format!("line {line_number}: invalid case: {error}"))?;
        case.validate()
            .map_err(|error| format!("line {line_number}: {error}"))?;
        if !ids.insert(case.id.clone()) {
            return Err(format!("line {line_number}: duplicate id {:?}", case.id));
        }
        count += 1;
    }
    if count == 0 {
        return Err("evaluation file must contain at least one case".into());
    }
    Ok(count)
}

impl EvalCase {
    fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("id must not be blank".into());
        }
        BatchInput {
            state: self.state.clone(),
            questions: BTreeMap::from([("case".into(), self.question.clone())]),
            model: None,
        }
        .validate()?;
        match (&self.question, &self.expected) {
            (Question::Noul { .. }, Expected::Noul { min, max }) => {
                validate_range(*min, *max, 1.0, "Noul")
            }
            (Question::Choice { criteria, .. }, Expected::Choice { allowed }) => {
                if allowed.is_empty() {
                    return Err("Choice expected.allowed must not be empty".into());
                }
                let mut unique = BTreeSet::new();
                for option in allowed {
                    if !criteria.contains_key(option) {
                        return Err(format!(
                            "Choice expected option {option:?} is not in question criteria"
                        ));
                    }
                    if !unique.insert(option) {
                        return Err(format!(
                            "Choice expected.allowed contains duplicate option {option:?}"
                        ));
                    }
                }
                Ok(())
            }
            (Question::Score { criteria, .. }, Expected::Score { min, max }) => {
                validate_range(*min, *max, (criteria.len() - 1) as f64, "Score")
            }
            _ => Err("question type and expected type must match".into()),
        }
    }
}

fn validate_range(min: f64, max: f64, upper: f64, kind: &str) -> Result<(), String> {
    if !min.is_finite() || !max.is_finite() || min < 0.0 || max < min || max > upper {
        return Err(format!(
            "{kind} expected range must satisfy 0 <= min <= max <= {upper}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn validates_all_expected_kinds() {
        let input = concat!(
            r#"{"id":"n","state":"evidence","question":{"type":"noul","instructions":"Relevant?"},"expected":{"type":"noul","min":0.7,"max":1.0}}"#,
            "\n",
            r#"{"id":"c","state":[],"question":{"type":"choice","instructions":"Owner?","criteria":{"api":null,"unknown":null}},"expected":{"type":"choice","allowed":["api","unknown"]}}"#,
            "\n",
            r#"{"id":"s","state":{},"question":{"type":"score","instructions":"Risk?","criteria":["low","high"]},"expected":{"type":"score","min":0.0,"max":0.5}}"#,
            "\n"
        );
        assert_eq!(validate_reader(Cursor::new(input)).unwrap(), 3);
    }

    #[test]
    fn rejects_empty_duplicate_invalid_and_mismatched_cases() {
        let invalid = [
            ("", "at least one case"),
            ("\n", "blank lines"),
            (
                concat!(
                    r#"{"id":"same","state":"s","question":{"type":"noul","instructions":"q"},"expected":{"type":"noul","min":0.0,"max":1.0}}"#,
                    "\n",
                    r#"{"id":"same","state":"s","question":{"type":"noul","instructions":"q"},"expected":{"type":"noul","min":0.0,"max":1.0}}"#
                ),
                "duplicate id",
            ),
            (
                r#"{"id":"bad","state":"s","question":{"type":"score","instructions":"q","criteria":["only"]},"expected":{"type":"score","min":0,"max":0}}"#,
                "Score requires 2 to 10 levels",
            ),
            (
                r#"{"id":"bad","state":"s","question":{"type":"noul","instructions":"q"},"expected":{"type":"choice","allowed":["x"]}}"#,
                "must match",
            ),
            (
                r#"{"id":"bad","state":"s","question":{"type":"noul","instructions":"q"},"expected":{"type":"noul","min":0.8,"max":0.2}}"#,
                "expected range",
            ),
        ];
        for (input, expected) in invalid {
            let error = validate_reader(Cursor::new(input)).unwrap_err();
            assert!(error.contains(expected), "{error:?}");
        }
    }
}
