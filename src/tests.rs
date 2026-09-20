use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use rmcp::{ServiceExt, model::CallToolRequestParams};
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, header, method, path},
};

use crate::{client::TypeSafeClient, error::ErrorKind, server::JevServer};

fn client(mock: &MockServer, key: Option<&str>) -> TypeSafeClient {
    TypeSafeClient::new(
        format!("{}/v1/systemone", mock.uri()),
        key.map(str::to_string),
        "jev-latest".into(),
    )
    .unwrap()
}

fn response(answers: Value) -> Value {
    json!({"model": "jev-test", "answers": answers, "usage": {"input_tokens": 21, "output_tokens": 5}})
}

fn noul_args() -> Value {
    json!({"state": "test evidence", "instructions": "Is this relevant?"})
}

fn assert_tool_error(
    result: &rmcp::model::CallToolResult,
    kind: ErrorKind,
    retryable: bool,
    status: Option<u16>,
) {
    assert_eq!(result.is_error, Some(true));
    let error = &result.structured_content.as_ref().unwrap()["error"];
    assert_eq!(error["kind"], serde_json::to_value(kind).unwrap());
    assert_eq!(error["retryable"], retryable);
    match status {
        Some(status) => assert_eq!(error["status"], status),
        None => assert!(error.get("status").is_none()),
    }
    let content = serde_json::to_value(&result.content).unwrap();
    assert_eq!(content[0]["text"], error["message"]);
}

#[tokio::test]
async fn single_tools_send_typed_requests_and_preserve_results() {
    let cases = [
        (
            "jev.noul",
            json!({"type":"noul","instructions":"Relevant?","criteria":{"true":"relevant","false":"unrelated"}}),
            json!({"type":"noul","noul":0.95}),
        ),
        (
            "jev.choice",
            json!({"type":"choice","instructions":{"question":"Subsystem?"},"criteria":{"api":null,"worker":{"description":"background jobs"}}}),
            json!({"type":"choice","choice":"worker","probabilities":{"api":0.1,"worker":0.9},"confidence":0.75}),
        ),
        (
            "jev.score",
            json!({"type":"score","instructions":"Severity?","criteria":["cosmetic","workaround exists","blocking"]}),
            json!({"type":"score","score":1.8,"legend":{"0":"cosmetic","1":"workaround exists","2":"blocking"},"probabilities":{"0":0.0,"1":0.2,"2":0.8},"confidence":0.6}),
        ),
    ];
    for (name, question, answer) in cases {
        let mock = MockServer::start().await;
        let result = response(json!({"result": answer}));
        let state = json!({"diff":"evidence", "tests":["failed"]});
        Mock::given(method("POST"))
            .and(path("/v1/systemone"))
            .and(header("authorization", "Bearer test-secret"))
            .and(header("content-type", "application/json"))
            .and(body_json(
                json!({"state":state,"model":"custom-model","questions":{"result":question}}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(&result))
            .expect(1)
            .mount(&mock)
            .await;
        let mut arguments = question.as_object().unwrap().clone();
        arguments.remove("type");
        arguments.insert("state".into(), state);
        arguments.insert("model".into(), json!("custom-model"));
        let reply = JevServer::new(client(&mock, Some("test-secret")))
            .dispatch(name, Value::Object(arguments))
            .await
            .unwrap();
        assert_eq!(reply.is_error, Some(false));
        assert_eq!(reply.structured_content, Some(result.clone()));
        let content = serde_json::to_value(reply.content).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(content[0]["text"].as_str().unwrap()).unwrap(),
            result
        );
    }
}

#[tokio::test]
async fn batch_round_trip_through_mcp_client() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mock = MockServer::start().await;
        let questions = json!({
            "relevant":{"type":"noul","instructions":"Relevant?"},
            "owner":{"type":"choice","instructions":"Owner?","criteria":{"api":null,"worker":null}},
            "risk":{"type":"score","instructions":"Risk?","criteria":["low","high"]}
        });
        let expected = response(json!({
            "relevant":{"type":"noul","noul":0.01},
            "owner":{"type":"choice","choice":"api","probabilities":{"api":0.9,"worker":0.1},"confidence":0.75},
            "risk":{"type":"score","score":0.2,"legend":{"0":"low","1":"high"},"probabilities":{"0":0.8,"1":0.2},"confidence":0.6}
        }));
        Mock::given(method("POST"))
            .and(body_json(json!({"state":["evidence"],"model":"jev-latest","questions":questions})))
            .respond_with(ResponseTemplate::new(200).set_body_json(&expected)).expect(1).mount(&mock).await;
        let server = JevServer::new(client(&mock, Some("test-secret")));
        let (server_io, client_io) = tokio::io::duplex(65536);
        let task = tokio::spawn(async move {
            server.serve(server_io).await.unwrap().waiting().await.unwrap();
        });
        let peer = ().serve(client_io).await.unwrap();
        let tools = peer.list_all_tools().await.unwrap();
        assert_eq!(tools.iter().map(|t| t.name.as_ref()).collect::<Vec<_>>(), ["jev.noul", "jev.choice", "jev.score", "jev.batch"]);
        for tool in &tools {
            assert_eq!(tool.input_schema["type"], "object");
            assert_eq!(tool.input_schema["additionalProperties"], false);
        }
        let result = peer.call_tool(CallToolRequestParams::new("jev.batch").with_arguments(
            json!({"state":["evidence"], "questions":questions}).as_object().unwrap().clone()
        )).await.unwrap();
        assert_eq!(result.structured_content, Some(expected));
        peer.cancel().await.unwrap();
        task.await.unwrap();
    }).await.unwrap();
}

#[tokio::test]
async fn invalid_input_never_reaches_api() {
    let mock = MockServer::start().await;
    let server = JevServer::new(client(&mock, Some("test-secret")));
    let cases = [
        ("jev.noul", json!({"state":true,"instructions":"q"})),
        ("jev.noul", json!({"state":"s","instructions":null})),
        (
            "jev.noul",
            json!({"state":"s","instructions":"q","unexpected":1}),
        ),
        (
            "jev.noul",
            json!({"state":"s","instructions":"q","model":" "}),
        ),
        (
            "jev.choice",
            json!({"state":"s","instructions":"q","criteria":{}}),
        ),
        (
            "jev.choice",
            json!({"state":"s","instructions":"q","criteria":{"a":42}}),
        ),
        (
            "jev.score",
            json!({"state":"s","instructions":"q","criteria":["one"]}),
        ),
        (
            "jev.score",
            json!({"state":"s","instructions":"q","criteria":vec!["level";11]}),
        ),
        ("jev.batch", json!({"state":"s","questions":{}})),
        (
            "jev.batch",
            json!({"state":"s","questions":{"q":{"type":"score","instructions":"q","criteria":["one"]}}}),
        ),
        (
            "jev.batch",
            json!({"state":"s","questions":{"q":{"type":"other","instructions":"q"}}}),
        ),
    ];
    for (name, args) in cases {
        let result = server.dispatch(name, args).await.unwrap();
        assert_tool_error(&result, ErrorKind::Validation, false, None);
    }
    let too_many: serde_json::Map<String, Value> =
        (0..256).map(|i| (i.to_string(), Value::Null)).collect();
    let result = server
        .dispatch(
            "jev.choice",
            json!({"state":"s","instructions":"q","criteria":too_many}),
        )
        .await
        .unwrap();
    assert_tool_error(&result, ErrorKind::Validation, false, None);
    assert!(server.dispatch("unknown", json!({})).await.is_err());
    assert!(mock.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn missing_key_is_tool_error_without_http() {
    let mock = MockServer::start().await;
    let result = JevServer::new(client(&mock, None))
        .dispatch("jev.noul", noul_args())
        .await
        .unwrap();
    assert_tool_error(&result, ErrorKind::Authentication, false, None);
    assert!(
        serde_json::to_string(&result)
            .unwrap()
            .contains("TYPESAFE_API_KEY")
    );
    assert!(mock.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn network_and_timeout_failures_are_structured_tool_errors() {
    let network_client = TypeSafeClient::new(
        "not a URL".into(),
        Some("test-secret".into()),
        "jev-latest".into(),
    )
    .unwrap();
    let network = JevServer::new(network_client)
        .dispatch("jev.noul", noul_args())
        .await
        .unwrap();
    assert_tool_error(&network, ErrorKind::Network, true, None);

    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .expect(1)
        .mount(&mock)
        .await;
    let mut timeout_client = client(&mock, Some("test-secret"));
    timeout_client.set_total_timeout(Duration::from_millis(50));
    let timeout = JevServer::new(timeout_client)
        .dispatch("jev.noul", noul_args())
        .await
        .unwrap();
    assert_tool_error(&timeout, ErrorKind::Timeout, true, None);
}

#[tokio::test]
async fn validation_errors_do_not_echo_arguments() {
    let mock = MockServer::start().await;
    let result = JevServer::new(client(&mock, Some("test-secret")))
        .dispatch(
            "jev.noul",
            json!({"state":"private-state","instructions":"private-question","unexpected":"private-value"}),
        )
        .await
        .unwrap();
    assert_tool_error(&result, ErrorKind::Validation, false, None);
    let serialized = serde_json::to_string(&result).unwrap();
    assert!(!serialized.contains("private-state"));
    assert!(!serialized.contains("private-question"));
    assert!(!serialized.contains("private-value"));
}

#[tokio::test]
async fn retries_rate_limit_and_overload_then_succeeds() {
    let mock = MockServer::start().await;
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    Mock::given(method("POST"))
        .respond_with(
            move |_: &wiremock::Request| match calls.fetch_add(1, Ordering::SeqCst) {
                0 => ResponseTemplate::new(429).insert_header("Retry-After", "0"),
                1 => ResponseTemplate::new(529).insert_header("Retry-After", "0"),
                _ => ResponseTemplate::new(200)
                    .set_body_json(response(json!({"result":{"type":"noul","noul":0.7}}))),
            },
        )
        .expect(3)
        .mount(&mock)
        .await;
    let result = JevServer::new(client(&mock, Some("test-secret")))
        .dispatch("jev.noul", noul_args())
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(false));
    assert_eq!(count.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn http_errors_are_bounded_and_do_not_echo_response_bodies() {
    for status in [401, 422, 429, 529, 500, 302] {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(status)
                    .insert_header("Retry-After", "0")
                    .insert_header("Location", "/redirect")
                    .set_body_string("secret-and-private-state"),
            )
            .expect(if matches!(status, 429 | 529) { 3 } else { 1 })
            .mount(&mock)
            .await;
        let result = JevServer::new(client(&mock, Some("test-secret")))
            .dispatch("jev.noul", noul_args())
            .await
            .unwrap();
        let (kind, retryable) = match status {
            401 => (ErrorKind::Authentication, false),
            422 => (ErrorKind::Validation, false),
            429 => (ErrorKind::RateLimit, true),
            529 | 500 => (ErrorKind::Http, true),
            _ => (ErrorKind::Http, false),
        };
        assert_tool_error(&result, kind, retryable, Some(status));
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(serialized.contains(&status.to_string()));
        assert!(!serialized.contains("secret-and-private-state"));
        assert!(!serialized.contains("test-secret"));
    }
}

#[tokio::test]
async fn rejects_malformed_or_mismatched_upstream_answers() {
    let cases = [
        "not JSON".into(),
        "{}".into(),
        response(json!({})).to_string(),
        response(json!({"wrong_id":{"type":"noul","noul":0.5}})).to_string(),
        response(json!({"result":{"type":"noul","noul":1.2}})).to_string(),
        response(json!({"result":{"type":"choice","choice":"a","probabilities":{"a":1.0},"confidence":1.0}})).to_string(),
    ];
    for body in cases {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .expect(1)
            .mount(&mock)
            .await;
        let result = JevServer::new(client(&mock, Some("test-secret")))
            .dispatch("jev.noul", noul_args())
            .await
            .unwrap();
        assert_tool_error(&result, ErrorKind::InvalidResponse, false, Some(200));
    }
}
