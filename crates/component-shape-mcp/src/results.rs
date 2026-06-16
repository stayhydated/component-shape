use super::*;

pub fn tool_structured_result(value: Value) -> ToolCallResult {
    let text = match &value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    };
    let mut result = ToolCallResult::success(vec![ContentBlock::text(text)]);
    result.structured_content = Some(value);
    result
}

/// Build an MCP error result for a plain message.
///
/// Prefer [`tool_error_result_for`] when the failure is a typed
/// [`McpToolError`], so clients receive a specific `error.kind`.
pub fn tool_error_result(message: impl Into<String>) -> ToolCallResult {
    let message = message.into();
    tool_error_result_with_structured_content(
        message.clone(),
        json!({
            "error": {
                "kind": "error",
                "message": message,
            }
        }),
    )
}

/// Build an MCP error result with machine-readable `structured_content.error`.
pub fn tool_error_result_for(error: McpToolError) -> ToolCallResult {
    let message = error.to_string();
    tool_error_result_with_structured_content(
        message,
        json!({
            "error": error.to_structured_value(),
        }),
    )
}

fn tool_error_result_with_structured_content(
    message: String,
    structured_content: Value,
) -> ToolCallResult {
    let mut result = ToolCallResult::error(vec![ContentBlock::text(message)]);
    result.structured_content = Some(structured_content);
    result
}

pub fn serialize_handler_response<Response, Error>(
    result: Result<Response, Error>,
) -> ToolCallResult
where
    Response: Serialize,
    Error: fmt::Display,
{
    match result {
        Ok(response) => serialize_response_value(response),
        Err(error) => tool_error_result_for(McpToolError::handler(error.to_string())),
    }
}

pub fn serialize_response_value<Response>(response: Response) -> ToolCallResult
where
    Response: Serialize,
{
    match serde_json::to_value(response) {
        Ok(value) => tool_structured_result(value),
        Err(error) => tool_error_result_for(McpToolError::handler(format!(
            "failed to serialize response: {error}"
        ))),
    }
}
