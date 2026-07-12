use std::{
    io::{BufRead as _, BufReader, Read as _, Write as _},
    process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use thiserror::Error;

use crate::MCP_PROTOCOL_VERSION;

const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

/// Error returned by the newline-delimited JSON-RPC stdio smoke client.
#[derive(Debug, Error)]
pub enum McpStdioSmokeError {
    #[error("failed to spawn MCP stdio server: {source}")]
    Spawn {
        #[source]
        source: std::io::Error,
    },
    #[error("spawned MCP stdio server has no {0}")]
    MissingPipe(&'static str),
    #[error("failed to write MCP request: {source}")]
    Write {
        #[source]
        source: std::io::Error,
    },
    #[error("failed to flush MCP request: {source}")]
    Flush {
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read MCP response: {source}")]
    Read {
        #[source]
        source: std::io::Error,
    },
    #[error("MCP stdio server closed stdout before responding to `{method}`{status}{stderr}")]
    Eof {
        method: String,
        status: ProcessStatus,
        stderr: StderrSnapshot,
    },
    #[error("MCP stdio server returned invalid JSON for `{method}`: {source}; line: {line}")]
    InvalidJson {
        method: String,
        line: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("MCP stdio server returned JSON-RPC error for `{method}`: {error}")]
    Rpc { method: String, error: Value },
    #[error("MCP stdio server response for `{method}` did not contain `result`: {response}")]
    MissingResult { method: String, response: Value },
    #[error(
        "MCP stdio server response for `{method}` did not contain the expected id `{id}`: {response}"
    )]
    UnexpectedResponse {
        method: String,
        id: u64,
        response: Value,
    },
    #[error("failed to wait for MCP stdio server shutdown: {source}")]
    Wait {
        #[source]
        source: std::io::Error,
    },
    #[error("failed to kill MCP stdio server after shutdown timeout: {source}")]
    Kill {
        #[source]
        source: std::io::Error,
    },
}

/// Process status attached to stdio smoke failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessStatus(Option<String>);

impl ProcessStatus {
    fn running() -> Self {
        Self(None)
    }

    fn exited(status: ExitStatus) -> Self {
        Self(Some(status.to_string()))
    }
}

impl std::fmt::Display for ProcessStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(status) => write!(formatter, " (process status: {status})"),
            None => Ok(()),
        }
    }
}

/// Captured stderr attached to stdio smoke failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StderrSnapshot(String);

impl StderrSnapshot {
    fn empty() -> Self {
        Self(String::new())
    }

    fn from_stderr(stderr: &Arc<Mutex<String>>) -> Self {
        Self(
            stderr
                .lock()
                .map(|stderr| stderr.clone())
                .unwrap_or_else(|error| format!("stderr capture lock failed: {error}")),
        )
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for StderrSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.trim().is_empty() {
            return Ok(());
        }
        write!(formatter, "\nstderr:\n{}", self.0.trim_end())
    }
}

/// Minimal newline-delimited JSON-RPC MCP client for subprocess smoke tests.
///
/// This intentionally exposes raw JSON protocol results so application-level
/// smoke flows can assert the same field names seen by external MCP clients.
pub struct McpStdioSmokeClient {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    stderr: Arc<Mutex<String>>,
    next_request_id: u64,
}

impl McpStdioSmokeClient {
    /// Spawn an MCP stdio server process and connect to its stdin/stdout.
    pub fn spawn(command: &mut Command) -> Result<Self, McpStdioSmokeError> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| McpStdioSmokeError::Spawn { source })?;

        let stdin = child
            .stdin
            .take()
            .ok_or(McpStdioSmokeError::MissingPipe("stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or(McpStdioSmokeError::MissingPipe("stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(McpStdioSmokeError::MissingPipe("stderr"))?;
        let stderr = capture_stderr(stderr);

        Ok(Self {
            child,
            stdin: Some(stdin),
            stdout: BufReader::new(stdout),
            stderr,
            next_request_id: 1,
        })
    }

    /// Perform the MCP initialize request and initialized notification.
    pub fn initialize(&mut self) -> Result<Value, McpStdioSmokeError> {
        let result = self.request(
            "initialize",
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "component-shape-mcp-stdio-smoke",
                    "version": env!("CARGO_PKG_VERSION"),
                },
            }),
        )?;
        self.notify_initialized()?;
        Ok(result)
    }

    /// Send the MCP initialized notification.
    pub fn notify_initialized(&mut self) -> Result<(), McpStdioSmokeError> {
        self.write_message(json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
        }))
    }

    /// Call `tools/list`.
    pub fn list_tools(&mut self) -> Result<Value, McpStdioSmokeError> {
        self.request("tools/list", json!({}))
    }

    /// Call `resources/list`.
    pub fn list_resources(&mut self) -> Result<Value, McpStdioSmokeError> {
        self.request("resources/list", json!({}))
    }

    /// Call `resources/templates/list`.
    pub fn list_resource_templates(&mut self) -> Result<Value, McpStdioSmokeError> {
        self.request("resources/templates/list", json!({}))
    }

    /// Call `resources/read` for one concrete URI.
    pub fn read_resource(&mut self, uri: &str) -> Result<Value, McpStdioSmokeError> {
        self.request("resources/read", json!({ "uri": uri }))
    }

    /// Call `tools/call` for one tool with JSON object arguments.
    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value, McpStdioSmokeError> {
        self.request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments,
            }),
        )
    }

    /// Send a raw JSON-RPC request and return the response `result`.
    pub fn request(&mut self, method: &str, params: Value) -> Result<Value, McpStdioSmokeError> {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        self.write_message(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))?;
        self.read_response(method, id)
    }

    /// Close stdin, wait briefly for the server, and kill it if it remains alive.
    pub fn shutdown(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<ExitStatus>, McpStdioSmokeError> {
        self.stdin.take();
        let deadline = Instant::now() + timeout;
        loop {
            match self
                .child
                .try_wait()
                .map_err(|source| McpStdioSmokeError::Wait { source })?
            {
                Some(status) => return Ok(Some(status)),
                None if Instant::now() >= deadline => {
                    self.child
                        .kill()
                        .map_err(|source| McpStdioSmokeError::Kill { source })?;
                    return self
                        .child
                        .wait()
                        .map(Some)
                        .map_err(|source| McpStdioSmokeError::Wait { source });
                },
                None => thread::sleep(Duration::from_millis(20)),
            }
        }
    }

    /// Return currently captured stderr text.
    pub fn stderr(&self) -> StderrSnapshot {
        StderrSnapshot::from_stderr(&self.stderr)
    }

    fn write_message(&mut self, message: Value) -> Result<(), McpStdioSmokeError> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or(McpStdioSmokeError::MissingPipe("stdin"))?;
        serde_json::to_writer(&mut *stdin, &message).map_err(|source| {
            McpStdioSmokeError::Write {
                source: std::io::Error::other(source),
            }
        })?;
        stdin
            .write_all(b"\n")
            .map_err(|source| McpStdioSmokeError::Write { source })?;
        stdin
            .flush()
            .map_err(|source| McpStdioSmokeError::Flush { source })
    }

    fn read_response(&mut self, method: &str, id: u64) -> Result<Value, McpStdioSmokeError> {
        let method = method.to_string();
        loop {
            let mut line = String::new();
            let read = self
                .stdout
                .read_line(&mut line)
                .map_err(|source| McpStdioSmokeError::Read { source })?;
            if read == 0 {
                let status = match self.child.try_wait() {
                    Ok(Some(status)) => ProcessStatus::exited(status),
                    Ok(None) | Err(_) => ProcessStatus::running(),
                };
                return Err(McpStdioSmokeError::Eof {
                    method,
                    status,
                    stderr: self.stderr(),
                });
            }

            let response = serde_json::from_str::<Value>(&line).map_err(|source| {
                McpStdioSmokeError::InvalidJson {
                    method: method.clone(),
                    line: line.trim_end().to_string(),
                    source,
                }
            })?;

            if response.get("id").and_then(Value::as_u64) != Some(id) {
                if response.get("id").is_none() {
                    continue;
                }
                return Err(McpStdioSmokeError::UnexpectedResponse {
                    method,
                    id,
                    response,
                });
            }

            if let Some(error) = response.get("error") {
                return Err(McpStdioSmokeError::Rpc {
                    method,
                    error: error.clone(),
                });
            }

            return response
                .get("result")
                .cloned()
                .ok_or(McpStdioSmokeError::MissingResult { method, response });
        }
    }
}

impl Drop for McpStdioSmokeClient {
    fn drop(&mut self) {
        let _ = self.shutdown(DEFAULT_SHUTDOWN_TIMEOUT);
    }
}

/// Return a tool call's protocol-level `structuredContent`, if present.
pub fn tool_call_structured_content(result: &Value) -> Option<&Value> {
    result
        .get("structuredContent")
        .or_else(|| result.get("structured_content"))
}

fn capture_stderr(stderr: impl std::io::Read + Send + 'static) -> Arc<Mutex<String>> {
    let output = Arc::new(Mutex::new(String::new()));
    let output_for_thread = Arc::clone(&output);
    thread::spawn(move || {
        let mut stderr = BufReader::new(stderr);
        let mut captured = String::new();
        if stderr.read_to_string(&mut captured).is_ok()
            && let Ok(mut output) = output_for_thread.lock()
        {
            *output = captured;
        }
    });
    output
}

impl Default for StderrSnapshot {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{process::Command, time::Duration};

    use serde_json::json;

    use super::{
        McpStdioSmokeClient, McpStdioSmokeError, ProcessStatus, StderrSnapshot,
        tool_call_structured_content,
    };

    fn spawn_shell(script: &str) -> McpStdioSmokeClient {
        McpStdioSmokeClient::spawn(Command::new("sh").arg("-c").arg(script))
            .expect("shell smoke server should spawn")
    }

    #[test]
    fn stdio_client_exercises_the_public_protocol_helpers() {
        let mut client = spawn_shell(
            r#"
read initialize
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"ok"}}'
read initialized
read tools
printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[]}}'
read resources
printf '%s\n' '{"jsonrpc":"2.0","id":3,"result":{"resources":[]}}'
read templates
printf '%s\n' '{"jsonrpc":"2.0","id":4,"result":{"resourceTemplates":[]}}'
read resource
printf '%s\n' '{"jsonrpc":"2.0","id":5,"result":{"contents":[]}}'
read tool
printf '%s\n' '{"jsonrpc":"2.0","id":6,"result":{"structuredContent":{"ok":true}}}'
"#,
        );

        assert_eq!(
            client.initialize().expect("initialize should succeed"),
            json!({ "protocolVersion": "ok" })
        );
        assert_eq!(
            client.list_tools().expect("tools/list should succeed"),
            json!({ "tools": [] })
        );
        assert_eq!(
            client
                .list_resources()
                .expect("resources/list should succeed"),
            json!({ "resources": [] })
        );
        assert_eq!(
            client
                .list_resource_templates()
                .expect("resources/templates/list should succeed"),
            json!({ "resourceTemplates": [] })
        );
        assert_eq!(
            client
                .read_resource("shape://example")
                .expect("resources/read should succeed"),
            json!({ "contents": [] })
        );
        let result = client
            .call_tool("shape_example", json!({ "value": 1 }))
            .expect("tools/call should succeed");
        assert_eq!(
            tool_call_structured_content(&result),
            Some(&json!({ "ok": true }))
        );
        assert!(
            client
                .shutdown(Duration::from_secs(1))
                .expect("server should shut down")
                .is_some()
        );
    }

    #[test]
    fn response_reader_skips_notifications_and_reports_protocol_errors() {
        let cases = [
            (
                "read request; printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"notice\"}' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":7}'",
                None,
            ),
            (
                "read request; printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":9,\"result\":7}'",
                Some("expected id `1`"),
            ),
            (
                "read request; printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"error\":{\"code\":-1}}'",
                Some("JSON-RPC error"),
            ),
            (
                "read request; printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1}'",
                Some("did not contain `result`"),
            ),
            (
                "read request; printf '%s\\n' 'not-json'",
                Some("invalid JSON"),
            ),
        ];

        for (script, expected_error) in cases {
            let mut client = spawn_shell(script);
            let result = client.request("example", json!({}));
            match expected_error {
                Some(expected_error) => assert!(
                    result
                        .expect_err("response should fail")
                        .to_string()
                        .contains(expected_error),
                    "expected error containing `{expected_error}`"
                ),
                None => assert_eq!(result.expect("response should succeed"), json!(7)),
            }
        }
    }

    #[test]
    fn client_reports_spawn_eof_and_closed_stdin_failures() {
        let spawn_error =
            match McpStdioSmokeClient::spawn(&mut Command::new("/definitely/not/a/program")) {
                Ok(_) => panic!("invalid executable should fail"),
                Err(error) => error,
            };
        assert!(matches!(spawn_error, McpStdioSmokeError::Spawn { .. }));

        let mut exited = spawn_shell("printf 'server detail\\n' >&2");
        let eof = exited
            .request("exited", json!({}))
            .expect_err("closed stdout should fail");
        assert!(matches!(eof, McpStdioSmokeError::Eof { .. }));
        assert!(eof.to_string().contains("closed stdout"));

        let mut closed = spawn_shell("read ignored");
        closed
            .shutdown(Duration::from_secs(1))
            .expect("server should shut down when stdin closes");
        assert!(matches!(
            closed.notify_initialized(),
            Err(McpStdioSmokeError::MissingPipe("stdin"))
        ));
    }

    #[test]
    fn shutdown_kills_a_server_that_does_not_exit_after_stdin_closes() {
        let mut client = spawn_shell("while :; do :; done");
        let status = client
            .shutdown(Duration::ZERO)
            .expect("timed-out server should be killed")
            .expect("killed server should return a status");

        assert!(!status.success());
    }

    #[test]
    fn diagnostics_format_status_stderr_and_structured_content() {
        assert_eq!(ProcessStatus::running().to_string(), "");
        assert_eq!(StderrSnapshot::default().to_string(), "");

        let stderr = StderrSnapshot(" detail \n".to_string());
        assert_eq!(stderr.as_str(), " detail \n");
        assert_eq!(stderr.to_string(), "\nstderr:\n detail");

        assert_eq!(
            tool_call_structured_content(&json!({ "structured_content": 3 })),
            Some(&json!(3))
        );
        assert_eq!(tool_call_structured_content(&json!({})), None);
    }
}
