use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use reqwest::{Client, header, redirect::Policy};
use serde_json::{Value, json};

use crate::{
    error::ToolError,
    profile::{ExecutionProfile, ProfileSet, ProfileSettings},
    types::{BatchInput, Evaluation},
};

const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct TypeSafeClient {
    http: Client,
    endpoint: String,
    authorization: Option<header::HeaderValue>,
    default_model: String,
    default_profile: ExecutionProfile,
    profiles: ProfileSet,
}

pub struct EvaluationSuccess {
    pub value: Value,
    pub evaluation: Evaluation,
}

pub struct EvaluationReport {
    pub result: Result<EvaluationSuccess, ToolError>,
    pub attempts: usize,
    pub timeout_count: usize,
    pub profile: ExecutionProfile,
    pub requested_model: String,
    pub resolved_model: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

impl TypeSafeClient {
    pub fn from_env() -> Result<Self, String> {
        let key = match std::env::var("TYPESAFE_API_KEY") {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(_) => return Err("TYPESAFE_API_KEY must be valid UTF-8".into()),
        };
        let model = match std::env::var("TYPESAFE_MODEL") {
            Ok(value) => value,
            Err(std::env::VarError::NotPresent) => "jev-latest".into(),
            Err(_) => return Err("TYPESAFE_MODEL must be valid UTF-8".into()),
        };
        let profile = ExecutionProfile::from_env()?;
        Self::new_with_profile(ENDPOINT.into(), key, model, profile)
    }

    #[cfg(test)]
    pub(crate) fn new(
        endpoint: String,
        key: Option<String>,
        model: String,
    ) -> Result<Self, String> {
        Self::new_with_profile(endpoint, key, model, ExecutionProfile::Reliable)
    }

    pub(crate) fn new_with_profile(
        endpoint: String,
        key: Option<String>,
        model: String,
        default_profile: ExecutionProfile,
    ) -> Result<Self, String> {
        if model.trim().is_empty() {
            return Err("TYPESAFE_MODEL must not be blank".into());
        }
        let api_key = key
            .filter(|s| !s.trim().is_empty())
            .map(|key| {
                let mut value =
                    header::HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| {
                        "TYPESAFE_API_KEY contains invalid header characters".to_string()
                    })?;
                value.set_sensitive(true);
                Ok::<_, String>(value)
            })
            .transpose()?;
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .redirect(Policy::none())
            .build()
            .map_err(|_| "Could not initialize the HTTP client".to_string())?;
        Ok(Self {
            http,
            endpoint,
            authorization: api_key,
            default_model: model,
            default_profile,
            profiles: ProfileSet::default(),
        })
    }

    #[cfg(test)]
    pub async fn evaluate(&self, input: BatchInput) -> Result<Value, ToolError> {
        self.evaluate_detailed(input)
            .await
            .result
            .map(|success| success.value)
    }

    pub async fn evaluate_detailed(&self, input: BatchInput) -> EvaluationReport {
        let requested_model = input
            .model
            .clone()
            .unwrap_or_else(|| self.default_model.clone());
        let profile = input.profile.unwrap_or(self.default_profile);
        let settings = self.profiles.get(profile);
        let attempts = AtomicUsize::new(0);
        let timeout_count = AtomicUsize::new(0);
        let result = if let Err(error) = input.validate() {
            Err(ToolError::validation(error))
        } else if let Some(key) = self.authorization.as_ref() {
            match tokio::time::timeout(
                settings.total_budget,
                self.request(&input, key, settings, &attempts, &timeout_count),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => {
                    timeout_count.fetch_add(1, Ordering::Relaxed);
                    Err(ToolError::timeout(
                        "TypeSafe request exceeded the profile's total budget",
                    ))
                }
            }
        } else {
            Err(ToolError::authentication(
                "TYPESAFE_API_KEY is not set. Set it in the server environment and restart the server.",
                None,
            ))
        };
        let (resolved_model, input_tokens, output_tokens) = match &result {
            Ok(success) => (
                Some(success.evaluation.model.clone()),
                Some(success.evaluation.usage.input_tokens),
                Some(success.evaluation.usage.output_tokens),
            ),
            Err(_) => (None, None, None),
        };
        EvaluationReport {
            result,
            attempts: attempts.load(Ordering::Relaxed),
            timeout_count: timeout_count.load(Ordering::Relaxed),
            profile,
            requested_model,
            resolved_model,
            input_tokens,
            output_tokens,
        }
    }

    async fn request(
        &self,
        input: &BatchInput,
        key: &header::HeaderValue,
        settings: ProfileSettings,
        attempts: &AtomicUsize,
        timeout_count: &AtomicUsize,
    ) -> Result<EvaluationSuccess, ToolError> {
        let body = json!({
            "state": input.state,
            "model": input.model.as_ref().unwrap_or(&self.default_model),
            "questions": input.questions,
        });
        for attempt in 0..=settings.max_retries {
            attempts.store(attempt + 1, Ordering::Relaxed);
            match tokio::time::timeout(
                settings.per_attempt_timeout,
                self.attempt(input, key, &body),
            )
            .await
            {
                Err(_) => {
                    timeout_count.fetch_add(1, Ordering::Relaxed);
                    if settings.retry_timeouts && attempt < settings.max_retries {
                        continue;
                    }
                    return Err(ToolError::timeout(
                        "TypeSafe request exceeded the profile's per-attempt timeout",
                    ));
                }
                Ok(Err(error)) => {
                    if error.kind == crate::error::ErrorKind::Timeout {
                        timeout_count.fetch_add(1, Ordering::Relaxed);
                        if settings.retry_timeouts && attempt < settings.max_retries {
                            continue;
                        }
                    }
                    return Err(error);
                }
                Ok(Ok(AttemptOutcome::Success(success))) => return Ok(success),
                Ok(Ok(AttemptOutcome::Retry { status, delay })) => {
                    if attempt < settings.max_retries {
                        let fallback = settings.retry_delay * (1 << attempt);
                        tokio::time::sleep(delay.unwrap_or(fallback).min(settings.total_budget))
                            .await;
                        continue;
                    }
                    return Err(match status {
                        429 => ToolError::rate_limit(
                            "TypeSafe rate limit was exceeded after retries",
                            status,
                        ),
                        529 => ToolError::http(
                            "TypeSafe remained overloaded after retries",
                            status,
                            true,
                        ),
                        _ => unreachable!("only retryable statuses produce Retry"),
                    });
                }
            }
        }
        unreachable!("the final attempt always returns")
    }

    async fn attempt(
        &self,
        input: &BatchInput,
        key: &header::HeaderValue,
        body: &Value,
    ) -> Result<AttemptOutcome, ToolError> {
        let mut response = self
            .http
            .post(&self.endpoint)
            .header(header::AUTHORIZATION, key.clone())
            .json(body)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    ToolError::timeout("TypeSafe request timed out")
                } else {
                    ToolError::network("Could not connect to TypeSafe API", None)
                }
            })?;
        let status = response.status().as_u16();
        if matches!(status, 429 | 529) {
            let delay = response
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .map(Duration::from_secs);
            return Ok(AttemptOutcome::Retry { status, delay });
        }
        if !response.status().is_success() {
            // Do not echo remote bodies: they can contain credentials or submitted state.
            return Err(match status {
                401 => ToolError::authentication(
                    "TypeSafe authentication failed; check TYPESAFE_API_KEY",
                    Some(status),
                ),
                422 => ToolError::validation_response(
                    "TypeSafe rejected the request; check model and question definitions",
                    status,
                ),
                _ => ToolError::http(
                    "TypeSafe returned an unexpected HTTP response",
                    status,
                    status >= 500,
                ),
            });
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| ToolError::network("Could not read TypeSafe response", Some(status)))?
        {
            if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err(ToolError::invalid_response(
                    "TypeSafe response exceeds 8 MiB",
                    status,
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| ToolError::invalid_response("TypeSafe returned invalid JSON", status))?;
        let evaluation: Evaluation = serde_json::from_value(value.clone()).map_err(|_| {
            ToolError::invalid_response("TypeSafe returned an invalid answer structure", status)
        })?;
        if !evaluation.matches(&input.questions) {
            return Err(ToolError::invalid_response(
                "TypeSafe answers do not match the requested questions or value ranges",
                status,
            ));
        }
        Ok(AttemptOutcome::Success(EvaluationSuccess {
            value,
            evaluation,
        }))
    }

    #[cfg(test)]
    pub(crate) fn set_profile_settings(
        &mut self,
        profile: ExecutionProfile,
        settings: ProfileSettings,
    ) {
        self.profiles.set(profile, settings);
    }
}

enum AttemptOutcome {
    Success(EvaluationSuccess),
    Retry {
        status: u16,
        delay: Option<Duration>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

    fn input() -> BatchInput {
        serde_json::from_value(json!({
            "state":"evidence",
            "questions":{"q":{"type":"noul","instructions":"Relevant?"}}
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn total_budget_bounds_slow_responses_and_long_retry_delays() {
        for template in [
            ResponseTemplate::new(200).set_delay(Duration::from_secs(5)),
            ResponseTemplate::new(429).insert_header("Retry-After", u64::MAX.to_string()),
        ] {
            let mock = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(template)
                .expect(1)
                .mount(&mock)
                .await;
            let mut client =
                TypeSafeClient::new(mock.uri(), Some("test-secret".into()), "jev-latest".into())
                    .unwrap();
            client.set_profile_settings(
                ExecutionProfile::Reliable,
                ProfileSettings {
                    per_attempt_timeout: Duration::from_secs(5),
                    total_budget: Duration::from_millis(100),
                    max_retries: 2,
                    retry_delay: Duration::from_millis(1),
                    retry_timeouts: false,
                },
            );
            let error = client.evaluate(input()).await.unwrap_err();
            assert_eq!(error.kind, ErrorKind::Timeout);
            assert!(error.retryable);
            assert!(error.message.contains("total budget"));
            assert_eq!(error.status, None);
        }
    }

    #[tokio::test]
    async fn oversized_responses_are_rejected() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_bytes(vec![b' '; MAX_RESPONSE_BYTES + 1]),
            )
            .expect(1)
            .mount(&mock)
            .await;
        let client =
            TypeSafeClient::new(mock.uri(), Some("test-secret".into()), "jev-latest".into())
                .unwrap();
        let error = client.evaluate(input()).await.unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidResponse);
        assert_eq!(error.message, "TypeSafe response exceeds 8 MiB");
        assert_eq!(error.status, Some(200));
        assert!(!error.retryable);
    }

    #[tokio::test]
    async fn connection_failures_are_retryable_network_errors() {
        let client = TypeSafeClient::new(
            "not a URL".into(),
            Some("test-secret".into()),
            "jev-latest".into(),
        )
        .unwrap();
        let error = client.evaluate(input()).await.unwrap_err();
        assert_eq!(error.kind, ErrorKind::Network);
        assert!(error.retryable);
        assert_eq!(error.status, None);
    }

    #[test]
    fn rejects_invalid_configuration_without_echoing_credentials() {
        let error = TypeSafeClient::new(
            ENDPOINT.into(),
            Some("secret\r\nvalue".into()),
            "jev-latest".into(),
        )
        .err()
        .unwrap();
        assert!(!error.contains("secret"));
        assert!(TypeSafeClient::new(ENDPOINT.into(), None, " ".into()).is_err());
    }
}
