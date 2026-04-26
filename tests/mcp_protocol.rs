use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdout, Command};
use tokio::time::timeout;

async fn write_json_line(
    stdin: &mut tokio::process::ChildStdin,
    payload: &Value,
) -> std::io::Result<()> {
    stdin.write_all(payload.to_string().as_bytes()).await?;
    stdin.write_all(b"\n").await?;
    stdin.flush().await
}

async fn read_message(stdout: &mut BufReader<ChildStdout>) -> Value {
    loop {
        let mut line = String::new();
        let read = timeout(Duration::from_secs(20), stdout.read_line(&mut line))
            .await
            .expect("timed out waiting for MCP message")
            .expect("failed reading MCP message");
        assert!(read > 0, "MCP server closed stdio unexpectedly");
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        return serde_json::from_str(line).expect("MCP server emitted non-JSON stdout");
    }
}

fn id_matches(message: &Value, expected: i64) -> bool {
    message
        .get("id")
        .and_then(Value::as_i64)
        .is_some_and(|id| id == expected)
        || message
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id == expected.to_string())
}

async fn read_response(stdout: &mut BufReader<ChildStdout>, request_id: i64) -> Value {
    loop {
        let message = read_message(stdout).await;
        if id_matches(&message, request_id) {
            return message;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_stdio_initializes_and_lists_weather_tools() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_weather-signal"));
    let config_dir = tempfile::tempdir().expect("temp config dir");
    let config_path = config_dir.path().join("config.toml");

    let mut child = Command::new(&binary)
        .arg("--config")
        .arg(&config_path)
        .arg("server")
        .arg("start")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn weather-signal MCP server");

    let mut stdin = child.stdin.take().expect("child stdin unavailable");
    let stdout = child.stdout.take().expect("child stdout unavailable");
    let mut stdout = BufReader::new(stdout);

    write_json_line(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "weather-signal-test", "version": "1.0.0"}
            }
        }),
    )
    .await
    .expect("failed to write initialize request");

    let initialize_response = read_response(&mut stdout, 1).await;
    assert!(
        initialize_response.get("result").is_some(),
        "initialize response should include result, got: {initialize_response}"
    );

    write_json_line(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }),
    )
    .await
    .expect("failed to write initialized notification");

    write_json_line(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
    )
    .await
    .expect("failed to write tools/list request");

    let tools_response = read_response(&mut stdout, 2).await;
    let tools = tools_response
        .get("result")
        .and_then(|result| result.get("tools"))
        .and_then(Value::as_array)
        .expect("tools/list response missing result.tools");
    let names = tools
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(names.contains(&"weather_summary"), "tools: {names:?}");
    assert!(names.contains(&"demand_signal"), "tools: {names:?}");
    assert!(names.contains(&"threshold_days"), "tools: {names:?}");
    assert!(names.contains(&"historical_weather"), "tools: {names:?}");

    write_json_line(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "cache_status",
                "arguments": {}
            }
        }),
    )
    .await
    .expect("failed to write tools/call request");

    let call_response = read_response(&mut stdout, 3).await;
    let content = call_response
        .get("result")
        .and_then(|result| result.get("content"))
        .and_then(Value::as_array)
        .expect("tools/call response missing content");
    let text = content
        .first()
        .and_then(|item| item.get("text"))
        .and_then(Value::as_str)
        .expect("tools/call response missing text content");
    let payload: Value = serde_json::from_str(text).expect("cache_status text should be JSON");
    assert!(
        payload.get("path").is_some(),
        "cache_status payload should include cache path: {payload}"
    );

    child.start_kill().expect("failed to terminate MCP child");
    let _ = child.wait().await;
}
