use super::*;

#[test]
fn server_handles_direct_tools_call() {
    let mut server = McpServer::new("test-server", "0.0.0");
    server
        .add_tool(
            crate::tool_definition(
                "echo",
                None,
                None,
                schema(json!({ "type": "object" })),
                None,
            )
            .expect("tool definition should build"),
            |call| crate::tool_structured_result(Value::Object(call.into_arguments().into_inner())),
        )
        .expect("tool should register");

    assert!(server.contains_tool("echo"));
    assert_eq!(server.tool_count(), 1);

    let result = server.call_tool("echo", Some(json!({ "value": 42 })));

    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.structured_content.expect("structured")["value"], 42);
}

#[test]
fn tool_registry_supports_direct_and_server_calls() {
    let mut tools = crate::McpToolRegistry::new();
    tools
        .add_tool(
            crate::tool_definition(
                "echo",
                None,
                None,
                schema(json!({ "type": "object" })),
                None,
            )
            .expect("tool definition should build"),
            |call| crate::tool_structured_result(Value::Object(call.into_arguments().into_inner())),
        )
        .expect("tool should register");

    let direct = tools.call_tool("echo", Some(json!({ "source": "registry" })));
    let server = McpServer::from_tool_registry("test-server", "0.0.0", tools.clone());
    let protocol = server.call_tool("echo", Some(json!({ "source": "server" })));

    assert_eq!(tools.tool_count(), 1);
    assert_eq!(server.tool_registry().tool_count(), 1);
    assert_eq!(
        direct.structured_content.expect("structured")["source"],
        "registry"
    );
    assert_eq!(
        protocol.structured_content.expect("structured")["source"],
        "server"
    );
    assert_eq!(server.into_tool_registry().tool_count(), 1);
}

#[test]
fn server_handles_async_tools_call() {
    let mut server = McpServer::new("test-server", "0.0.0");
    server
        .add_tool_async(
            crate::tool_definition(
                "echo",
                None,
                None,
                schema(json!({ "type": "object" })),
                None,
            )
            .expect("tool definition should build"),
            |call| async move {
                crate::tool_structured_result(Value::Object(call.into_arguments().into_inner()))
            },
        )
        .expect("tool should register");

    let result = server.call_tool("echo", Some(json!({ "value": 42 })));

    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.structured_content.expect("structured")["value"], 42);
}

#[test]
fn server_handles_typed_tools_call() {
    #[derive(crate::McpToolInput)]
    #[allow(dead_code)]
    struct EchoInput {
        value: String,
    }

    let mut server = McpServer::new("test-server", "0.0.0");
    server
        .add_typed_tool::<EchoInput, _>(
            crate::tool_definition_for_input::<EchoInput>("echo", None, None, None)
                .expect("tool definition should build"),
            |input| crate::tool_structured_result(json!({ "value": input.value })),
        )
        .expect("tool should register");

    let result = server.call_tool("echo", Some(json!({ "value": "typed" })));
    assert_eq!(result.is_error, Some(false));
    assert_eq!(
        result.structured_content.expect("structured")["value"],
        "typed"
    );

    let result = server.call_tool("echo", Some(json!({ "value": "typed", "extra": true })));
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.expect("structured error")["error"],
        json!({
            "kind": "unknown_field",
            "message": "unknown field `extra`",
            "field": "extra"
        })
    );
}

#[test]
fn server_validates_success_output_against_declared_schema() {
    fn echo_output_schema() -> crate::McpSchema {
        schema(json!({
            "type": "object",
            "properties": {
                "value": { "type": "integer" }
            },
            "required": ["value"],
            "additionalProperties": false
        }))
    }

    let mut server = McpServer::new("test-server", "0.0.0");
    server
        .add_tool(
            crate::tool_definition(
                "missing_structured_content",
                None,
                None,
                schema(json!({ "type": "object" })),
                Some(echo_output_schema()),
            )
            .expect("tool definition should build"),
            |_| crate::ToolCallResult::success(vec![crate::ContentBlock::text("ok")]),
        )
        .expect("tool should register");
    server
        .add_tool(
            crate::tool_definition(
                "scalar_structured_content",
                None,
                None,
                schema(json!({ "type": "object" })),
                Some(schema(json!({ "type": "string" }))),
            )
            .expect("tool definition should build"),
            |_| crate::tool_structured_result(json!("scalar output")),
        )
        .expect("tool should register");
    server
        .add_tool(
            crate::tool_definition(
                "schema_mismatch",
                None,
                None,
                schema(json!({ "type": "object" })),
                Some(echo_output_schema()),
            )
            .expect("tool definition should build"),
            |_| crate::tool_structured_result(json!({})),
        )
        .expect("tool should register");
    server
        .add_tool(
            crate::tool_definition(
                "handler_error",
                None,
                None,
                schema(json!({ "type": "object" })),
                Some(echo_output_schema()),
            )
            .expect("tool definition should build"),
            |_| crate::tool_error_result_for(crate::McpToolError::handler("boom")),
        )
        .expect("tool should register");

    let result = server.call_tool("missing_structured_content", Some(json!({})));
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.expect("structured error")["error"],
        json!({
            "kind": "invalid_tool_output",
            "message": "tool `missing_structured_content` returned invalid structured content: tool declares output_schema but returned no structured_content",
            "name": "missing_structured_content",
            "detail": "tool declares output_schema but returned no structured_content"
        })
    );

    let result = server.call_tool("scalar_structured_content", Some(json!({})));
    assert_eq!(result.is_error, Some(false));
    assert_eq!(
        result.structured_content.expect("structured"),
        json!("scalar output")
    );

    let result = server.call_tool("schema_mismatch", Some(json!({})));
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.expect("structured error")["error"],
        json!({
            "kind": "invalid_tool_output",
            "message": "tool `schema_mismatch` returned invalid structured content: missing required field `structured_content.value`",
            "name": "schema_mismatch",
            "detail": "missing required field `structured_content.value`"
        })
    );

    let result = server.call_tool("handler_error", Some(json!({})));
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.expect("structured error")["error"]["kind"],
        json!("handler")
    );
}

#[test]
fn server_composes_registrars() {
    fn register_echo(server: &mut McpServer) -> Result<(), crate::McpToolError> {
        server.add_tool(
            crate::tool_definition(
                "echo",
                None,
                None,
                schema(json!({ "type": "object" })),
                None,
            )
            .expect("tool definition should build"),
            |call| crate::tool_structured_result(Value::Object(call.into_arguments().into_inner())),
        )
    }

    let server = McpServer::builder("test-server", "0.0.0")
        .register(register_echo)
        .build()
        .expect("registrar should compose");

    assert!(server.contains_tool("echo"));
}

#[test]
fn server_rejects_duplicate_tool_names() {
    let mut server = McpServer::new("test-server", "0.0.0");
    server
        .add_tool(
            crate::tool_definition(
                "echo",
                None,
                None,
                schema(json!({ "type": "object" })),
                None,
            )
            .expect("tool definition should build"),
            |call| crate::tool_structured_result(Value::Object(call.into_arguments().into_inner())),
        )
        .expect("tool should register");

    let error = match server.add_tool(
        crate::tool_definition(
            "echo",
            None,
            None,
            schema(json!({ "type": "object" })),
            None,
        )
        .expect("tool definition should build"),
        |call| crate::tool_structured_result(Value::Object(call.into_arguments().into_inner())),
    ) {
        Ok(_) => panic!("duplicate tool should be rejected"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        crate::McpToolError::DuplicateTool {
            name: "echo".to_string()
        }
    );
    assert_eq!(server.tool_count(), 1);
}

#[test]
fn server_validates_raw_tool_definitions() {
    let mut server = McpServer::new("test-server", "0.0.0");
    let invalid_name = ToolDefinition::default();

    let error = server
        .add_tool(invalid_name, |_| crate::tool_structured_result(json!(null)))
        .expect_err("invalid raw tool name should fail");

    assert_eq!(error.kind(), "validation");
    assert_eq!(server.tool_count(), 0);

    let mut invalid_input_schema = crate::tool_definition(
        "valid",
        None,
        None,
        schema(json!({ "type": "object" })),
        None,
    )
    .expect("tool definition should build");
    invalid_input_schema.input_schema = std::sync::Arc::new(
        crate::schema_object("input_schema", schema(json!({ "type": "string" })))
            .expect("raw schema object should build"),
    );

    let error = server
        .add_tool(invalid_input_schema, |_| {
            crate::tool_structured_result(json!(null))
        })
        .expect_err("raw tool input schema should describe object arguments");

    assert_eq!(error.kind(), "invalid_schema");
    assert_eq!(server.tool_count(), 0);

    let scalar_output = crate::tool_definition(
        "scalar_output",
        None,
        None,
        schema(json!({ "type": "object" })),
        Some(schema(json!({ "type": "string" }))),
    )
    .expect("tool definition should build");

    server
        .add_tool(scalar_output, |_| {
            crate::tool_structured_result(json!("scalar output"))
        })
        .expect("scalar output schema should register");

    assert_eq!(server.tool_count(), 1);

    let mut conflicting_annotations = crate::tool_definition(
        "valid",
        None,
        None,
        schema(json!({ "type": "object" })),
        None,
    )
    .expect("tool definition should build");
    conflicting_annotations.annotations = Some(
        crate::McpToolAnnotations::new()
            .read_only(true)
            .destructive(true),
    );

    let error = server
        .add_tool(conflicting_annotations, |_| {
            crate::tool_structured_result(json!(null))
        })
        .expect_err("conflicting raw annotations should fail");

    assert_eq!(error.kind(), "validation");
    assert_eq!(server.tool_count(), 1);
}

#[test]
fn server_builder_registers_sync_async_raw_and_typed_tools() {
    #[derive(crate::McpToolInput)]
    struct Input {
        value: String,
    }

    fn raw_tool(name: &str) -> crate::ToolDefinition {
        crate::tool_definition(name, None, None, crate::McpSchema::object(), None)
            .expect("tool should build")
    }

    let typed_sync = crate::tool_definition_for_input_with_annotations::<Input>(
        "typed_sync",
        None,
        None,
        None,
        Some(crate::McpToolAnnotations::new().read_only(true)),
    )
    .expect("typed tool should build");
    let typed_async = crate::tool_definition_for_input::<Input>("typed_async", None, None, None)
        .expect("typed tool should build");

    let server = crate::McpServer::builder("builder", "0.0.0")
        .tool(raw_tool("raw_sync"), |call| {
            crate::tool_structured_result(Value::Object(call.into_arguments().into_inner()))
        })
        .typed_tool(typed_sync, |input| {
            crate::tool_structured_result(json!({ "value": input.value }))
        })
        .tool_async(raw_tool("raw_async"), |call| async move {
            crate::tool_structured_result(Value::Object(call.into_arguments().into_inner()))
        })
        .typed_tool_async(typed_async, |input| async move {
            crate::tool_structured_result(json!({ "value": input.value }))
        })
        .build()
        .expect("builder should register tools");

    for name in ["raw_sync", "typed_sync", "raw_async", "typed_async"] {
        let result = server.call_tool(name, Some(json!({ "value": name })));
        assert_eq!(result.is_error, Some(false), "tool: {name}");
        assert_eq!(
            result.structured_content.expect("structured")["value"],
            name
        );
    }

    let invalid = server.call_tool("typed_async", Some(json!({ "extra": true })));
    assert_eq!(invalid.is_error, Some(true));
    let unknown = server.call_tool("missing", Some(json!({})));
    assert_eq!(unknown.is_error, Some(true));

    let duplicate = crate::McpServer::builder("builder", "0.0.0")
        .tool(raw_tool("duplicate"), |_| {
            crate::tool_structured_result(json!({}))
        })
        .tool(raw_tool("duplicate"), |_| {
            crate::tool_structured_result(json!({}))
        })
        .build();
    assert!(matches!(
        duplicate,
        Err(crate::McpToolError::DuplicateTool { .. })
    ));
}

#[test]
fn direct_server_calls_cover_argument_errors_async_duplicates_and_panics() {
    let definition = crate::tool_definition(
        "async_failure",
        None,
        None,
        crate::McpSchema::object(),
        None,
    )
    .expect("tool definition should build");
    let mut server = crate::McpServer::new("test", "0.0.0");
    server
        .add_tool_async(definition.clone(), |_| async {
            panic!("intentional async handler panic")
        })
        .expect("async tool should register");
    assert!(
        server
            .add_tool_async(definition, |_| async {
                crate::tool_structured_result(json!(null))
            })
            .is_err()
    );

    let invalid_arguments = server.call_tool("async_failure", Some(json!(1)));
    assert_eq!(invalid_arguments.is_error, Some(true));
    assert_eq!(
        invalid_arguments.structured_content.expect("structured")["error"]["kind"],
        "arguments_must_be_object"
    );

    let panic_result = server.call_tool("async_failure", None);
    assert_eq!(panic_result.is_error, Some(true));
    assert!(
        panic_result.structured_content.expect("structured")["error"]["message"]
            .as_str()
            .expect("message should be text")
            .contains("runtime panicked")
    );
}
