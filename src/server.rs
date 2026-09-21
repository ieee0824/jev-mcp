use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
use schemars::{JsonSchema, schema_for};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::time::Instant;

use crate::{
    client::{EvaluationSuccess, TypeSafeClient},
    error::ToolError,
    policy::PolicyConfig,
    profile::ExecutionProfile,
    telemetry::{CallStatus, CallTelemetry, Telemetry, TelemetryError},
    types::*,
};

pub struct JevServer {
    client: TypeSafeClient,
    telemetry: Telemetry,
    policy: PolicyConfig,
}

struct ErrorCall<'a> {
    profile: Option<ExecutionProfile>,
    attempts: usize,
    timeout_count: usize,
    requested_model: Option<&'a str>,
}

impl JevServer {
    #[cfg(test)]
    pub fn new(client: TypeSafeClient) -> Self {
        Self::with_config(client, Telemetry::disabled(), PolicyConfig::default())
    }

    #[cfg(test)]
    pub fn with_telemetry(client: TypeSafeClient, telemetry: Telemetry) -> Self {
        Self::with_config(client, telemetry, PolicyConfig::default())
    }

    pub fn with_config(client: TypeSafeClient, telemetry: Telemetry, policy: PolicyConfig) -> Self {
        Self {
            client,
            telemetry,
            policy,
        }
    }

    pub async fn dispatch(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<CallToolResult, ErrorData> {
        if !matches!(name, "jev.noul" | "jev.choice" | "jev.score" | "jev.batch") {
            return Err(ErrorData::invalid_params("Unknown tool", None));
        }
        let started = Instant::now();
        let question_count = question_count(name, &arguments);
        let input = match decode(name, arguments) {
            Ok(input) => input,
            Err(error) => {
                self.record_error(
                    name,
                    question_count,
                    started,
                    ErrorCall {
                        profile: None,
                        attempts: 0,
                        timeout_count: 0,
                        requested_model: None,
                    },
                    &error,
                );
                return Ok(error_result(error));
            }
        };
        let report = self.client.evaluate_detailed(input).await;
        match report.result {
            Ok(success) => {
                self.telemetry.record(&CallTelemetry {
                    tool: name,
                    question_count,
                    elapsed_ms: elapsed_ms(started),
                    attempts: report.attempts,
                    retry_count: report.attempts.saturating_sub(1),
                    timeout_count: report.timeout_count,
                    profile: Some(report.profile),
                    status: CallStatus::Success,
                    requested_model: Some(&report.requested_model),
                    resolved_model: report.resolved_model.as_deref(),
                    input_tokens: report.input_tokens,
                    output_tokens: report.output_tokens,
                    error: None,
                });
                Ok(CallToolResult::structured(self.with_policy(success)))
            }
            Err(error) => {
                self.record_error(
                    name,
                    question_count,
                    started,
                    ErrorCall {
                        profile: Some(report.profile),
                        attempts: report.attempts,
                        timeout_count: report.timeout_count,
                        requested_model: Some(&report.requested_model),
                    },
                    &error,
                );
                Ok(error_result(error))
            }
        }
    }

    fn with_policy(&self, success: EvaluationSuccess) -> Value {
        let mut value = success.value;
        let policy = serde_json::to_value(self.policy.assess(&success.evaluation))
            .expect("policy assessment is serializable");
        value
            .as_object_mut()
            .expect("validated evaluation is a JSON object")
            .insert("policy".into(), policy);
        value
    }

    fn record_error(
        &self,
        tool: &str,
        question_count: usize,
        started: Instant,
        call: ErrorCall<'_>,
        error: &ToolError,
    ) {
        self.telemetry.record(&CallTelemetry {
            tool,
            question_count,
            elapsed_ms: elapsed_ms(started),
            attempts: call.attempts,
            retry_count: call.attempts.saturating_sub(1),
            timeout_count: call.timeout_count,
            profile: call.profile,
            status: CallStatus::Error,
            requested_model: call.requested_model,
            resolved_model: None,
            input_tokens: None,
            output_tokens: None,
            error: Some(TelemetryError { kind: error.kind }),
        });
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn question_count(name: &str, arguments: &Value) -> usize {
    if name == "jev.batch" {
        arguments
            .get("questions")
            .and_then(Value::as_object)
            .map_or(0, serde_json::Map::len)
    } else {
        1
    }
}

fn error_result(error: ToolError) -> CallToolResult {
    let message = error.message.clone();
    let mut result = CallToolResult::structured_error(serde_json::json!({
        "error": error,
        "policy": PolicyConfig::failure(),
    }));
    result.content = vec![ContentBlock::text(message)];
    result
}

fn decode(name: &str, args: Value) -> Result<BatchInput, ToolError> {
    fn parse<T: DeserializeOwned>(args: Value) -> Result<T, ToolError> {
        serde_json::from_value(args).map_err(|_| ToolError::validation("Invalid tool arguments"))
    }
    Ok(match name {
        "jev.noul" => {
            let a: NoulInput = parse(args)?;
            BatchInput::single(
                a.state,
                Question::Noul {
                    instructions: a.instructions,
                    criteria: a.criteria,
                },
                a.model,
                a.profile,
            )
        }
        "jev.choice" => {
            let a: ChoiceInput = parse(args)?;
            BatchInput::single(
                a.state,
                Question::Choice {
                    instructions: a.instructions,
                    criteria: a.criteria,
                },
                a.model,
                a.profile,
            )
        }
        "jev.score" => {
            let a: ScoreInput = parse(args)?;
            BatchInput::single(
                a.state,
                Question::Score {
                    instructions: a.instructions,
                    criteria: a.criteria,
                },
                a.model,
                a.profile,
            )
        }
        "jev.batch" => parse(args)?,
        _ => unreachable!("tool name checked before decoding"),
    })
}

pub fn tools() -> Vec<Tool> {
    fn tool<T: JsonSchema>(name: &'static str, description: &'static str) -> Tool {
        let schema = schema_for!(T).to_value().as_object().unwrap().clone();
        Tool::new(name, description, schema).with_annotations(
            ToolAnnotations::new()
                .read_only(true)
                .destructive(false)
                .open_world(true),
        )
    }
    vec![
        tool::<NoulInput>(
            "jev.noul",
            "Evaluate one yes/no statement with Jev. Returns the raw Yes probability plus deterministic policy guidance. Near 0.5 is uncertain. Optional profile overrides reliable/interactive latency behavior. Sends supplied state to TypeSafe AI.",
        ),
        tool::<ChoiceInput>(
            "jev.choice",
            "Choose one host-defined option with Jev. Returns choice, the complete probability distribution, confidence, and deterministic policy guidance. Include unknown/other when appropriate. Jev does not execute the choice.",
        ),
        tool::<ScoreInput>(
            "jev.score",
            "Score one dimension using 2..10 ordered descriptive levels. Returns the raw score, legend, probability distribution, confidence, and deterministic policy guidance. Score is a weighted level index, not a probability.",
        ),
        tool::<BatchInput>(
            "jev.batch",
            "Evaluate multiple independent closed decisions against shared state in one TypeSafe API request. Questions cannot depend on other answers. Returns raw answers and per-answer deterministic policy guidance. Optional profile overrides reliable/interactive latency behavior.",
        ),
    ]
}

impl ServerHandler for JevServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("jev-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions("Use Jev for focused, closed decisions grounded in supplied evidence. The host investigates and executes actions; Jev only returns typed judgments. Prefer jev.batch for independent questions sharing state. Raw probabilities are preserved and policy is deterministic guidance, not a correctness guarantee. On an explicit tool error, continue without Jev only when the host can do so safely.")
    }

    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(tools()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        self.dispatch(
            &request.name,
            Value::Object(request.arguments.unwrap_or_default()),
        )
        .await
        .map(Into::into)
    }
}
