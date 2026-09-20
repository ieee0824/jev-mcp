use std::{process::Stdio, time::Duration};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

#[tokio::test]
async fn executable_supports_legacy_stdio_handshake_tools_ping_and_eof() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut process = Command::new(env!("CARGO_BIN_EXE_jev-mcp"))
            .env_remove("TYPESAFE_API_KEY").env_remove("TYPESAFE_MODEL")
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .kill_on_drop(true).spawn().unwrap();
        let mut stdin = process.stdin.take().unwrap();
        let mut lines = BufReader::new(process.stdout.take().unwrap()).lines();
        let requests = [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"stdio-test","version":"1"}}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"jev.noul","arguments":{"state":"s","instructions":"q"}}}),
            json!({"jsonrpc":"2.0","id":4,"method":"ping"}),
        ];
        for request in requests {
            stdin.write_all(format!("{request}\n").as_bytes()).await.unwrap();
            stdin.flush().await.unwrap();
            if request.get("id").is_none() { continue; }
            let line = lines.next_line().await.unwrap().expect("server stdout closed early");
            let reply: Value = serde_json::from_str(&line).expect("stdout must contain only MCP JSON");
            assert_eq!(reply["id"], request["id"]);
            assert!(reply.get("error").is_none(), "{reply}");
            match request["id"].as_u64().unwrap() {
                1 => {
                    assert_eq!(reply["result"]["protocolVersion"], "2025-11-25");
                    assert!(reply["result"]["capabilities"].get("tools").is_some());
                    assert_eq!(reply["result"]["serverInfo"]["name"], "jev-mcp");
                }
                2 => assert_eq!(reply["result"]["tools"].as_array().unwrap().len(), 4),
                3 => {
                    assert_eq!(reply["result"]["isError"], true);
                    assert!(reply["result"]["content"][0]["text"].as_str().unwrap().contains("TYPESAFE_API_KEY"));
                    assert!(reply["result"].get("resultType").is_none());
                }
                _ => {}
            }
        }
        drop(stdin);
        assert!(process.wait().await.unwrap().success());
    }).await.expect("stdio test timed out");
}
