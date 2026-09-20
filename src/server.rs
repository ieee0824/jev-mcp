use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
use schemars::{JsonSchema, schema_for};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::time::Instant;

use crate::{
    client::TypeSafeClient,
    error::ToolError,
    telemetry::{CallStatus, CallTelemetry, Telemetry, TelemetryError},
    types::*,
};

pub struct JevServer {
    client: TypeSafeClient,
    telemetry: Telemetry,
}

impl JevServer {
    #[cfg(test)]
    pub fn new(client: TypeSafeClient) -> Self {
        Self::with_telemetry(client, Telemetry::disabled())
    }

    pub fn with_telemetry(client: TypeSafeClient, telemetry: Telemetry) -> Self {
        Self { client, telemetry }
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
                self.record_error(name, question_count, started, 0, &error);
                return Ok(error_result(error));
            }
        };
        let report = self.client.evaluate_detailed(input).await;
        match report.result {
            Ok(value) => {
                self.telemetry.record(&CallTelemetry {
                    tool: name,
                    question_count,
                    elapsed_ms: elapsed_ms(started),
                    attempts: report.attempts,
                    status: CallStatus::Success,
                    requested_model: Some(&report.requested_model),
                    resolved_model: report.resolved_model.as_deref(),
                    input_tokens: report.input_tokens,
                    output_tokens: report.output_tokens,
                    error: None,
                });
                Ok(CallToolResult::structured(value))
            }
            Err(error) => {
                self.record_error(name, question_count, started, report.attempts, &error);
                Ok(error_result(error))
            }
        }
    }

    fn record_error(
        &self,
        tool: &str,
        question_count: usize,
        started: Instant,
        attempts: usize,
        error: &ToolError,
    ) {
        self.telemetry.record(&CallTelemetry {
            tool,
            question_count,
            elapsed_ms: elapsed_ms(started),
            attempts,
            status: CallStatus::Error,
            requested_model: None,
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
    let mut result = CallToolResult::structured_error(serde_json::json!({ "error": error }));
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
            "Evaluate one yes/no statement with Jev. Returns answers.result.noul: probability of yes (0..1); near 0 is strong no, near 0.5 uncertain. No separate confidence. Sends supplied state to TypeSafe AI. Prefer jev.batch for multiple questions sharing state.",
        ),
        tool::<ChoiceInput>(
            "jev.choice",
            "Choose among named criteria with Jev. Returns answers.result with choice, probabilities and confidence. Include an unknown/other option when appropriate. Sends supplied state to TypeSafe AI. Prefer jev.batch for multiple questions sharing state.",
        ),
        tool::<ScoreInput>(
            "jev.score",
            "Score one dimension using 2..10 ordered descriptive levels. Returns answers.result with score (0..levels-1), legend, probabilities and confidence. Score is a weighted level index, not a probability. Sends supplied state to TypeSafe AI. Prefer jev.batch for multiple questions sharing state.",
        ),
        tool::<BatchInput>(
            "jev.batch",
            "Preferred for multiple independent decisions sharing state: mix noul, choice and score questions in one TypeSafe API request. Answers use your question IDs. Each question sees only the supplied state, not other answers. Ask one focused judgment per question. Returns model, answers and usage. Sends supplied state to TypeSafe AI.",
        ),
    ]
}

impl ServerHandler for JevServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("jev-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions("Use Jev for focused decisions grounded in supplied evidence. Prefer jev.batch for independent questions sharing state. Results are judgments, not guarantees. Choice/Score confidence describes the probability distribution; Noul has no separate confidence. All results include model, answers and token usage; single-tool answers are under result.")
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
