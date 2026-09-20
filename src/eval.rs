use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufRead, BufReader, Write},
    path::Path,
    time::Instant,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    client::TypeSafeClient,
    error::ErrorKind,
    types::{BatchInput, Context, Question},
};

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
    Ok(load_file(path)?.len())
}

#[cfg(test)]
pub fn validate_reader(reader: impl BufRead) -> Result<usize, String> {
    Ok(load_reader(reader)?.len())
}

pub fn load_file(path: &Path) -> Result<Vec<EvalCase>, String> {
    let file = File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    load_reader(BufReader::new(file))
}

fn load_reader(reader: impl BufRead) -> Result<Vec<EvalCase>, String> {
    let mut ids = BTreeSet::new();
    let mut cases = Vec::new();
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
        cases.push(case);
    }
    if cases.is_empty() {
        return Err("evaluation file must contain at least one case".into());
    }
    Ok(cases)
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum CaseStatus {
    Pass,
    Fail,
    Error,
}

#[derive(Debug, Serialize)]
struct CaseResult<'a> {
    #[serde(rename = "type")]
    record_type: &'static str,
    id: &'a str,
    status: CaseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    passed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    measured: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved_model: Option<String>,
    elapsed_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<CaseError>,
}

#[derive(Debug, Serialize)]
struct CaseError {
    kind: ErrorKind,
}

#[derive(Debug, Serialize)]
struct EvalSummary {
    #[serde(rename = "type")]
    record_type: &'static str,
    total: usize,
    passed: usize,
    failed: usize,
    errors: usize,
    input_tokens: u64,
    output_tokens: u64,
    average_elapsed_ms: f64,
}

pub async fn run_file(
    path: &Path,
    client: &TypeSafeClient,
    writer: impl Write,
) -> Result<(), String> {
    let cases = load_file(path)?;
    run_cases(cases, client, writer).await
}

async fn run_cases(
    cases: Vec<EvalCase>,
    client: &TypeSafeClient,
    mut writer: impl Write,
) -> Result<(), String> {
    let mut passed = 0;
    let mut failed = 0;
    let mut errors = 0;
    let mut input_tokens = 0u64;
    let mut output_tokens = 0u64;
    let mut total_elapsed_ms = 0u64;

    for case in &cases {
        let input = BatchInput {
            state: case.state.clone(),
            questions: BTreeMap::from([("case".into(), case.question.clone())]),
            model: None,
        };
        let started = Instant::now();
        let report = client.evaluate_detailed(input).await;
        let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        total_elapsed_ms = total_elapsed_ms.saturating_add(elapsed_ms);
        let result = match report.result {
            Ok(value) => {
                let measured = measured_value(&case.question, &value).ok_or_else(|| {
                    "validated API response is missing the case answer".to_string()
                })?;
                let is_pass = case.expected.matches(&measured);
                if is_pass {
                    passed += 1;
                } else {
                    failed += 1;
                }
                input_tokens = input_tokens.saturating_add(report.input_tokens.unwrap_or(0));
                output_tokens = output_tokens.saturating_add(report.output_tokens.unwrap_or(0));
                CaseResult {
                    record_type: "case",
                    id: &case.id,
                    status: if is_pass {
                        CaseStatus::Pass
                    } else {
                        CaseStatus::Fail
                    },
                    passed: Some(is_pass),
                    measured: Some(measured),
                    resolved_model: report.resolved_model,
                    elapsed_ms,
                    input_tokens: report.input_tokens,
                    output_tokens: report.output_tokens,
                    error: None,
                }
            }
            Err(error) => {
                errors += 1;
                CaseResult {
                    record_type: "case",
                    id: &case.id,
                    status: CaseStatus::Error,
                    passed: None,
                    measured: None,
                    resolved_model: None,
                    elapsed_ms,
                    input_tokens: None,
                    output_tokens: None,
                    error: Some(CaseError { kind: error.kind }),
                }
            }
        };
        write_json_line(&mut writer, &result)?;
    }

    write_json_line(
        &mut writer,
        &EvalSummary {
            record_type: "summary",
            total: cases.len(),
            passed,
            failed,
            errors,
            input_tokens,
            output_tokens,
            average_elapsed_ms: total_elapsed_ms as f64 / cases.len() as f64,
        },
    )
}

fn measured_value(question: &Question, response: &Value) -> Option<Value> {
    let answer = response.get("answers")?.get("case")?;
    match question {
        Question::Noul { .. } => answer.get("noul").cloned(),
        Question::Choice { .. } => answer.get("choice").cloned(),
        Question::Score { .. } => answer.get("score").cloned(),
    }
}

impl Expected {
    fn matches(&self, measured: &Value) -> bool {
        match self {
            Expected::Noul { min, max } | Expected::Score { min, max } => measured
                .as_f64()
                .is_some_and(|value| (*min..=*max).contains(&value)),
            Expected::Choice { allowed } => measured
                .as_str()
                .is_some_and(|value| allowed.iter().any(|item| item == value)),
        }
    }
}

fn write_json_line(writer: &mut impl Write, value: &impl Serialize) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, value).map_err(|error| error.to_string())?;
    writer.write_all(b"\n").map_err(|error| error.to_string())
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
    use std::{
        io::Cursor,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

    use serde_json::json;

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

    #[tokio::test]
    async fn runner_continues_after_mismatch_and_http_error() {
        let input = [("pass", 0.7, 1.0), ("fail", 0.8, 1.0), ("error", 0.0, 1.0)]
            .into_iter()
            .map(|(id, min, max)| {
                json!({
                    "id": id,
                    "state": format!("private-state-{id}"),
                    "question": {"type":"noul", "instructions": format!("private-question-{id}")},
                    "expected": {"type":"noul", "min":min, "max":max}
                })
                .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        let cases = load_reader(Cursor::new(input)).unwrap();
        let mock = MockServer::start().await;
        let count = Arc::new(AtomicUsize::new(0));
        let calls = count.clone();
        Mock::given(method("POST"))
            .respond_with(
                move |_: &wiremock::Request| match calls.fetch_add(1, Ordering::SeqCst) {
                    0 => ResponseTemplate::new(200).set_body_json(json!({
                        "model":"jev-test", "answers":{"case":{"type":"noul","noul":0.9}},
                        "usage":{"input_tokens":10,"output_tokens":2}
                    })),
                    1 => ResponseTemplate::new(200).set_body_json(json!({
                        "model":"jev-test", "answers":{"case":{"type":"noul","noul":0.2}},
                        "usage":{"input_tokens":11,"output_tokens":3}
                    })),
                    _ => ResponseTemplate::new(500),
                },
            )
            .expect(3)
            .mount(&mock)
            .await;
        let client = TypeSafeClient::new(
            mock.uri(),
            Some("private-api-key".into()),
            "jev-latest".into(),
        )
        .unwrap();
        let mut output = Vec::new();
        run_cases(cases, &client, &mut output).await.unwrap();
        let text = String::from_utf8(output).unwrap();
        let records: Vec<Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();

        assert_eq!(records.len(), 4);
        assert_eq!(records[0]["status"], "pass");
        assert_eq!(records[0]["passed"], true);
        assert_eq!(records[0]["measured"], 0.9);
        assert_eq!(records[0]["resolved_model"], "jev-test");
        assert_eq!(records[1]["status"], "fail");
        assert_eq!(records[1]["passed"], false);
        assert_eq!(records[2]["status"], "error");
        assert_eq!(records[2]["error"]["kind"], "http");
        assert_eq!(records[3]["type"], "summary");
        assert_eq!(records[3]["total"], 3);
        assert_eq!(records[3]["passed"], 1);
        assert_eq!(records[3]["failed"], 1);
        assert_eq!(records[3]["errors"], 1);
        assert_eq!(records[3]["input_tokens"], 21);
        assert_eq!(records[3]["output_tokens"], 5);
        assert!(records[3]["average_elapsed_ms"].is_number());
        for private in ["private-state", "private-question", "private-api-key"] {
            assert!(!text.contains(private));
        }
    }
}
