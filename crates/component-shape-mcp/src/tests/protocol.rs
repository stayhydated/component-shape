use super::*;

#[test]
fn tool_errors_include_structured_content() {
    let result = crate::tool_error_result("plain failure");
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.expect("structured error")["error"],
        json!({
            "kind": "error",
            "message": "plain failure"
        })
    );

    let result = crate::serialize_handler_response::<(), _>(Err("No client"));
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.expect("structured error")["error"],
        json!({
            "kind": "handler",
            "message": "handler failed: No client",
            "detail": "No client"
        })
    );

    let result = crate::tool_error_result_for(crate::McpToolError::validation_details([
        "missing required field `title`",
        "name must not be empty",
    ]));
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.expect("structured error")["error"],
        json!({
            "kind": "validation",
            "message": "validation failed: missing required field `title`; name must not be empty",
            "detail": "missing required field `title`; name must not be empty",
            "details": [
                "missing required field `title`",
                "name must not be empty"
            ]
        })
    );
}

#[test]
fn server_exposes_tools_through_rmcp_protocol() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime should start");

    runtime.block_on(async {
        use rmcp::{
            ClientLifecycleMode, ClientServiceExt as _, ServiceExt as _,
            model::{CacheScope, CallToolRequestParams, ProtocolVersion, ResultType},
        };

        let mut server = McpServer::new("test-server", "0.0.0");
        server
            .add_tool(
                crate::tool_definition(
                    "echo",
                    Some("Echo".to_string()),
                    None,
                    schema(json!({ "type": "object" })),
                    Some(schema(json!({
                        "type": "object",
                        "properties": {
                            "value": { "type": "integer" }
                        },
                        "required": ["value"],
                        "additionalProperties": false
                    }))),
                )
                .expect("tool definition should build"),
                |call| {
                    crate::tool_structured_result(Value::Object(call.into_arguments().into_inner()))
                },
            )
            .expect("tool should register");

        let (server_transport, client_transport) = tokio::io::duplex(4096);
        let server_handle = tokio::spawn(async move {
            let service = server
                .serve(server_transport)
                .await
                .expect("server should start");
            service.waiting().await.expect("server task should join");
        });

        let client = ()
            .serve_with_lifecycle(
                client_transport,
                ClientLifecycleMode::Discover {
                    preferred_versions: vec![ProtocolVersion::V_2026_07_28],
                },
            )
            .await
            .expect("client should start");
        assert_eq!(
            client
                .peer()
                .peer_info()
                .expect("server discovery info")
                .protocol_version,
            ProtocolVersion::V_2026_07_28
        );

        let tools = client
            .peer()
            .list_tools(Default::default())
            .await
            .expect("tools/list should succeed");
        assert_eq!(tools.tools.len(), 1);
        assert_eq!(tools.tools[0].name, "echo");
        assert_eq!(tools.tools[0].title.as_deref(), Some("Echo"));
        assert_eq!(tools.result_type, Some(ResultType::COMPLETE));
        assert_eq!(tools.ttl_ms, Some(0));
        assert_eq!(tools.cache_scope, Some(CacheScope::Private));
        assert_eq!(
            tools
                .meta
                .as_ref()
                .expect("tools/list result metadata")
                .0["io.modelcontextprotocol/serverInfo"]["name"],
            "test-server"
        );

        let result = client
            .peer()
            .call_tool(
                CallToolRequestParams::new("echo").with_arguments(
                    json!({ "value": 42 })
                        .as_object()
                        .expect("arguments should be an object")
                        .clone(),
                ),
            )
            .await
            .expect("tools/call should succeed");

        assert_eq!(result.is_error, Some(false));
        assert_eq!(result.result_type, Some(ResultType::COMPLETE));
        assert_eq!(
            result
                .meta
                .as_ref()
                .expect("tools/call result metadata")
                .0["io.modelcontextprotocol/serverInfo"]["name"],
            "test-server"
        );
        assert_eq!(result.structured_content.expect("structured")["value"], 42);

        client.cancel().await.expect("client should close");
        server_handle.await.expect("server should finish");
    });
}

#[test]
fn every_typed_tool_error_has_stable_structured_content() {
    let errors = vec![
        crate::McpToolError::ArgumentsMustBeObject,
        crate::McpToolError::missing_field("name"),
        crate::McpToolError::UnexpectedNull {
            field: "name".to_string(),
        },
        crate::McpToolError::DuplicateField {
            field: "name".to_string(),
        },
        crate::McpToolError::decode("name", "bad string"),
        crate::McpToolError::UnknownField {
            field: "extra".to_string(),
        },
        crate::McpToolError::invalid_field_value("state", "missing"),
        crate::McpToolError::validation("invalid"),
        crate::McpToolError::invalid_tool_output("tool", "bad output"),
        crate::McpToolError::conversion("bad conversion"),
        crate::McpToolError::handler("handler failed"),
        crate::McpToolError::invalid_schema("input", "bad schema"),
        crate::McpToolError::duplicate_tool("tool"),
        crate::McpToolError::UnknownTool {
            name: "tool".to_string(),
        },
        crate::McpToolError::duplicate_resource("shape://one"),
        crate::McpToolError::unknown_resource("shape://one"),
        crate::McpToolError::duplicate_prompt("prompt"),
        crate::McpToolError::unknown_prompt("prompt"),
    ];

    for error in errors {
        let structured = error.to_structured_value();
        assert_eq!(structured["kind"], error.kind());
        assert_eq!(structured["message"], error.to_string());
    }
}

#[test]
fn response_serialization_covers_success_and_failure() {
    struct FailsToSerialize;

    impl serde::Serialize for FailsToSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom("expected failure"))
        }
    }

    let success = crate::serialize_handler_response::<_, &str>(Ok(json!({ "ok": true })));
    assert_eq!(success.is_error, Some(false));
    assert_eq!(success.structured_content.expect("structured")["ok"], true);

    let failure = crate::serialize_response_value(FailsToSerialize);
    assert_eq!(failure.is_error, Some(true));
    assert_eq!(
        failure.structured_content.expect("structured error")["error"]["kind"],
        "handler"
    );
}

#[test]
fn response_serialization_reports_serializer_failures() {
    struct FailingSerialize;

    impl serde::Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom(
                "intentional serialization failure",
            ))
        }
    }

    let result = crate::serialize_response_value(FailingSerialize);
    assert_eq!(result.is_error, Some(true));
    assert!(
        result.structured_content.expect("structured")["error"]["message"]
            .as_str()
            .expect("message should be text")
            .contains("intentional serialization failure")
    );
}
