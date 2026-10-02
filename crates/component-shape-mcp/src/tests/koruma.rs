use super::*;
use koruma_collection::numeric::RangeValidation;

#[derive(crate::McpToolInput, ::koruma::Koruma)]
struct BoundedArgs {
    #[koruma(RangeValidation::<_>.min(1).max(5))]
    count: u32,
}

fn definition(name: &str) -> crate::McpTypedTool<BoundedArgs> {
    crate::tool_definition_for_input::<BoundedArgs>(
        name,
        None,
        None,
        Some(crate::McpSchema::integer()),
    )
    .expect("paired tool definition")
}

fn response(args: BoundedArgs) -> crate::ToolCallResult {
    assert!(
        (1..=5).contains(&args.count),
        "invalid input reached handler"
    );
    crate::tool_structured_result(json!(args.count))
}

fn assert_validation(result: crate::ToolCallResult) {
    assert_eq!(result.is_error, Some(true));
    let error = result.structured_content.expect("structured failure");
    assert_eq!(error["error"]["kind"], "validation");
    assert_eq!(error["error"]["details"][0]["field"], "count");
    assert_eq!(error["error"]["details"][0]["scope"], "field");
}

#[test]
fn koruma_registry_decodes_then_validates_before_sync_dispatch() {
    let mut registry = crate::McpToolRegistry::new();
    registry
        .add_koruma_tool(definition("bounded"), response)
        .expect("register");
    assert_validation(registry.call_tool("bounded", Some(json!({"count": 0}))));
    let valid = registry.call_tool("bounded", Some(json!({"count": 3})));
    assert_eq!(valid.is_error, Some(false));
    assert_eq!(valid.structured_content, Some(json!(3)));

    for input in [
        json!({"count": "bad"}),
        json!({"count": 3, "unknown": true}),
    ] {
        let result = registry.call_tool("bounded", Some(input));
        assert_eq!(result.is_error, Some(true));
        assert_ne!(
            result.structured_content.expect("decode failure")["error"]["kind"],
            "validation"
        );
    }
    assert!(
        registry
            .add_koruma_tool(definition("bounded"), response)
            .is_err()
    );
}

#[test]
fn koruma_server_builder_validates_before_async_future_creation_and_checks_outputs() {
    let server = crate::McpServer::builder("bounded", "1")
        .koruma_tool(definition("sync"), response)
        .koruma_tool_async(definition("async"), |args| {
            assert!(
                (1..=5).contains(&args.count),
                "invalid input created handler future"
            );
            async move { response(args) }
        })
        .koruma_tool(definition("bad_output"), |_args| {
            crate::tool_structured_result(json!("bad"))
        })
        .build()
        .expect("build server");
    assert_validation(server.call_tool("sync", Some(json!({"count": 0}))));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        assert_validation(
            server
                .call_tool_async("async", Some(json!({"count": 6})))
                .await,
        );
        let valid = server
            .call_tool_async("async", Some(json!({"count": 4})))
            .await;
        assert_eq!(valid.structured_content, Some(json!(4)));
        assert_eq!(valid.is_error, Some(false));
    });
    let output = server.call_tool("bad_output", Some(json!({"count": 3})));
    assert_eq!(output.is_error, Some(true));
    assert_eq!(
        output.structured_content.expect("output failure")["error"]["kind"],
        "invalid_tool_output"
    );
}

#[test]
fn koruma_validation_is_enforced_over_the_mcp_protocol() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        use rmcp::model::{CallToolRequestParams, ProtocolVersion};
        use rmcp::{ClientLifecycleMode, ClientServiceExt as _, ServiceExt as _};

        let server = crate::McpServer::builder("bounded", "1")
            .koruma_tool_async(definition("bounded"), |args| {
                assert!(
                    (1..=5).contains(&args.count),
                    "invalid protocol input reached handler"
                );
                async move { response(args) }
            })
            .build()
            .expect("server");
        let (server_transport, client_transport) = tokio::io::duplex(4096);
        let server_task = tokio::spawn(async move {
            server
                .serve(server_transport)
                .await
                .expect("serve")
                .waiting()
                .await
                .expect("join service");
        });
        let client = ()
            .serve_with_lifecycle(
                client_transport,
                ClientLifecycleMode::Discover {
                    preferred_versions: vec![ProtocolVersion::V_2026_07_28],
                },
            )
            .await
            .expect("client");
        for count in [0, 3] {
            let result = client
                .peer()
                .call_tool(
                    CallToolRequestParams::new("bounded").with_arguments(
                        json!({"count": count})
                            .as_object()
                            .expect("arguments")
                            .clone(),
                    ),
                )
                .await
                .expect("tool call");
            if count == 0 {
                assert_validation(result);
            } else {
                assert_eq!(result.structured_content, Some(json!(3)));
                assert_eq!(result.is_error, Some(false));
            }
        }
        client.cancel().await.expect("cancel client");
        server_task.await.expect("server task");
    });
}

#[test]
fn koruma_failure_with_no_issues_still_stops_validation() {
    #[derive(Default)]
    struct SilentError;
    impl koruma_core::ValidationError for SilentError {
        fn is_empty(&self) -> bool {
            false
        }
    }
    impl koruma_core::ValidationIssues for SilentError {
        fn issues(&self) -> Vec<koruma_core::ValidationIssue> {
            Vec::new()
        }
    }
    struct Rejected;
    impl koruma_core::ValidateExt for Rejected {
        type Error = SilentError;
        fn validate(&self) -> Result<(), Self::Error> {
            Err(SilentError)
        }
    }
    let error = crate::validate_koruma(&Rejected)
        .expect_err("validation must fail")
        .to_structured_value();
    assert_eq!(error["kind"], "validation");
    assert_eq!(
        error["details"],
        json!([{"scope": "form", "message": "domain validation failed"}])
    );
}

#[test]
fn canonical_issue_adapter_preserves_scopes_and_typed_runtime_parameters() {
    use koruma_core::{ValidationIssue, ValidatorParam, ValidatorParamValue as Param};
    let params = vec![
        ValidatorParam::new("flag", Param::Bool(true)),
        ValidatorParam::new("min", Param::I64(-2)),
        ValidatorParam::new("max", Param::U64(u64::MAX)),
        ValidatorParam::new("fraction", Param::F64(1.5)),
        ValidatorParam::new("text", Param::String("value".into())),
        ValidatorParam::new("optional", Param::None),
        ValidatorParam::new(
            "opaque",
            Param::Opaque {
                type_name: "DomainLimit",
            },
        ),
        ValidatorParam::new("nonfinite", Param::F64(f64::INFINITY)),
    ];
    let issue = ValidationIssue::element(
        "items",
        2,
        "DomainRule",
        Some("limit"),
        "invalid item",
        params,
    );
    let converted = crate::McpValidationIssue::from(&issue).to_value();
    assert_eq!(
        converted,
        json!({
            "scope": "element", "field": "items", "element_index": 2,
            "validator": "DomainRule", "label": "limit", "message": "invalid item",
            "params": [
                {"name": "flag", "value": true}, {"name": "min", "value": -2},
                {"name": "max", "value": u64::MAX}, {"name": "fraction", "value": 1.5},
                {"name": "text", "value": "value"}, {"name": "optional", "value": null},
                {"name": "opaque", "value": {"kind": "opaque", "type_name": "DomainLimit"}},
                {"name": "nonfinite", "value": {"kind": "f64", "value": "inf"}}
            ]
        })
    );
    assert_eq!(
        crate::McpValidationIssue::from(&ValidationIssue::form("invalid form")).to_value(),
        json!({"scope": "form", "message": "invalid form"})
    );
}
