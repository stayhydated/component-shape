use super::*;

#[test]
fn tool_metadata_records_optional_overrides() {
    static ICONS: &[crate::McpToolIcon] =
        &[crate::McpToolIcon::new("https://example.com/tool.png")
            .with_mime_type("image/png")
            .with_sizes(&["48x48"])
            .with_theme(crate::McpIconTheme::Light)];

    let metadata = crate::McpToolMetadata::new()
        .with_name("custom_tool")
        .with_title("Custom tool")
        .with_description("Runs a custom tool.")
        .with_read_only_hint(true)
        .with_destructive_hint(false)
        .with_idempotent_hint(true)
        .with_open_world_hint(false)
        .with_icons(ICONS);

    assert_eq!(metadata.name(), Some("custom_tool"));
    assert_eq!(metadata.title(), Some("Custom tool"));
    assert_eq!(metadata.description(), Some("Runs a custom tool."));
    assert_eq!(metadata.read_only_hint(), Some(true));
    assert_eq!(metadata.destructive_hint(), Some(false));
    assert_eq!(metadata.idempotent_hint(), Some(true));
    assert_eq!(metadata.open_world_hint(), Some(false));
    assert_eq!(metadata.icons()[0].src(), "https://example.com/tool.png");
    assert_eq!(metadata.icons()[0].mime_type(), Some("image/png"));
    assert_eq!(metadata.icons()[0].sizes(), &["48x48"]);
    assert_eq!(
        metadata.icons()[0].theme(),
        Some(crate::McpIconTheme::Light)
    );
    let annotations = metadata
        .tool_annotations()
        .expect("metadata should publish tool annotations");
    assert_eq!(annotations.title.as_deref(), Some("Custom tool"));
    assert_eq!(annotations.read_only_hint, Some(true));
    assert_eq!(annotations.destructive_hint, Some(false));
    assert_eq!(annotations.idempotent_hint, Some(true));
    assert_eq!(annotations.open_world_hint, Some(false));
}

#[test]
fn tool_metadata_validates_optional_overrides() {
    static EMPTY_SRC_ICONS: &[crate::McpToolIcon] = &[crate::McpToolIcon::new("")];

    assert!(
        crate::McpToolMetadata::new()
            .with_name("custom_tool")
            .with_title("Custom tool")
            .with_description("Runs a custom tool.")
            .validate()
            .is_ok()
    );
    assert!(
        crate::McpToolMetadata::new()
            .with_icons(EMPTY_SRC_ICONS)
            .validate()
            .is_err()
    );
    assert!(
        crate::McpToolMetadata::new()
            .with_name("bad name")
            .validate()
            .is_err()
    );
    assert!(
        crate::McpToolMetadata::new()
            .with_description(" ")
            .validate()
            .is_err()
    );
    assert!(
        crate::McpToolMetadata::new()
            .with_read_only_hint(true)
            .with_destructive_hint(true)
            .validate()
            .is_err()
    );
}

#[test]
fn tool_definition_validates_name_and_metadata() {
    assert!(
        crate::tool_definition("", None, None, schema(json!({ "type": "object" })), None).is_err()
    );
    assert!(
        crate::tool_definition(
            "bad name",
            None,
            None,
            schema(json!({ "type": "object" })),
            None
        )
        .is_err()
    );
    assert!(
        crate::tool_definition(
            "valid_name",
            Some("".to_string()),
            None,
            schema(json!({ "type": "object" })),
            None,
        )
        .is_err()
    );
    assert!(crate::tool_definition("valid", None, None, schema(json!("bad")), None).is_err());
    assert!(
        crate::tool_definition(
            "valid",
            None,
            None,
            schema(json!({ "type": "string" })),
            None
        )
        .is_err()
    );
    assert!(
        crate::tool_definition(
            "valid",
            None,
            None,
            schema(json!({ "type": "object" })),
            Some(schema(json!({}))),
        )
        .is_ok()
    );
    assert!(
        crate::tool_definition(
            "valid",
            None,
            None,
            schema(json!({ "type": "object" })),
            Some(schema(json!({ "type": "string" }))),
        )
        .is_ok()
    );
    assert!(
        crate::tool_definition(
            "valid",
            None,
            None,
            schema(json!({ "type": "object" })),
            Some(schema(json!({ "type": "object" }))),
        )
        .is_ok()
    );
    assert!(
        crate::tool_definition(
            "valid",
            None,
            None,
            schema(json!({ "type": "object" })),
            Some(schema(json!(false))),
        )
        .is_err()
    );
    assert!(
        crate::tool_definition_with_annotations(
            "valid",
            None,
            None,
            schema(json!({ "type": "object" })),
            None,
            Some(
                crate::McpToolAnnotations::new()
                    .read_only(true)
                    .destructive(true),
            ),
        )
        .is_err()
    );
}

#[test]
fn tool_definition_accepts_valid_name() {
    let ToolDefinition { name, .. } = crate::tool_definition(
        "good-name",
        Some("Good name".to_string()),
        None,
        schema(json!({ "type": "object" })),
        None,
    )
    .expect("tool definition should build");

    assert_eq!(name.to_string(), "good-name");
}

#[test]
fn tool_definition_for_input_uses_typed_schema() {
    #[derive(crate::McpToolInput)]
    #[allow(dead_code)]
    struct EchoInput {
        value: String,
    }

    let tool =
        crate::tool_definition_for_input::<EchoInput>("echo", Some("Echo".to_string()), None, None)
            .expect("tool definition should build");
    fn assert_typed_tool(_tool: &crate::McpTypedTool<EchoInput>) {}
    assert_typed_tool(&tool);

    assert_eq!(tool.input_schema["properties"]["value"]["type"], "string");
}

#[test]
fn tool_definition_for_input_accepts_typed_metadata() {
    #[derive(crate::McpToolInput)]
    #[allow(dead_code)]
    struct EchoInput {
        value: String,
    }

    static ICONS: &[crate::McpToolIcon] =
        &[crate::McpToolIcon::new("https://example.com/echo.svg")
            .with_mime_type("image/svg+xml")
            .with_sizes(&["any"])
            .with_theme(crate::McpIconTheme::Dark)];

    let metadata = crate::McpToolMetadata::new()
        .with_name("custom_echo")
        .with_title("Custom echo")
        .with_description("Echoes a value.")
        .with_read_only_hint(true)
        .with_destructive_hint(false)
        .with_open_world_hint(false)
        .with_icons(ICONS);
    let tool = crate::tool_definition_for_input_with_metadata::<EchoInput>("echo", metadata, None)
        .expect("tool definition should build");

    assert_eq!(tool.name.as_ref(), "custom_echo");
    assert_eq!(tool.title.as_deref(), Some("Custom echo"));
    assert_eq!(tool.description.as_deref(), Some("Echoes a value."));
    let annotations = tool
        .annotations
        .as_ref()
        .expect("metadata annotations should be applied");
    assert_eq!(annotations.title.as_deref(), Some("Custom echo"));
    assert_eq!(annotations.read_only_hint, Some(true));
    assert_eq!(annotations.destructive_hint, Some(false));
    assert_eq!(annotations.open_world_hint, Some(false));
    let icons = tool
        .icons
        .as_ref()
        .expect("metadata icons should be applied");
    assert_eq!(icons[0].src, "https://example.com/echo.svg");
    assert_eq!(icons[0].mime_type.as_deref(), Some("image/svg+xml"));
    assert_eq!(
        icons[0].sizes.as_ref().expect("sizes should be set")[0],
        "any"
    );
    assert_eq!(icons[0].theme, Some(crate::McpIconTheme::Dark));
    assert_eq!(tool.input_schema["properties"]["value"]["type"], "string");
}

#[test]
fn tool_names_are_stable_and_mcp_friendly() {
    assert_eq!(
        crate::tool_name("example::tools", "Contact", "tool_"),
        "example_tools_contact"
    );
    assert_eq!(
        crate::tool_name("example::forms", "ContactRequest", "tool_"),
        "example_forms_contact_request"
    );
    assert_eq!(
        crate::tool_name("example::api", "HTTPServer2", "tool_"),
        "example_api_http_server2"
    );
    assert_eq!(crate::tool_name("123", "", "tool_"), "tool_123");
}

#[test]
fn server_accepts_owned_metadata() {
    let server = McpServer::new("owned-server".to_string(), "1.2.3".to_string());

    assert_eq!(server.server_name, "owned-server");
    assert_eq!(server.server_version, "1.2.3");
}

#[test]
fn typed_tool_metadata_arguments_and_ranges_expose_adapters() {
    #[derive(crate::McpToolInput)]
    #[allow(dead_code)]
    struct Input {
        value: String,
    }

    let mut tool = crate::tool_definition_for_input::<Input>("adapter", None, None, None)
        .expect("tool should build");
    assert_eq!(tool.as_ref().name.as_ref(), "adapter");
    tool.definition_mut().title = Some("Adapter".to_string());
    assert_eq!(tool.definition().title.as_deref(), Some("Adapter"));

    let call = crate::McpToolCall::from_value(Some(json!({ "value": "x" })))
        .expect("call should normalize");
    let arguments: crate::McpArguments = call.into();
    assert!(!arguments.is_empty());
    assert_eq!(arguments.as_inner()["value"], "x");
    let raw = arguments.into_inner();
    let arguments: crate::McpArguments = raw.into();
    assert_eq!(arguments.as_inner()["value"], "x");

    let icon = crate::McpToolIcon::new("data:image/svg+xml,x")
        .with_mime_type("image/svg+xml")
        .with_sizes(&["16x16"])
        .with_theme(crate::McpIconTheme::Light);
    assert_eq!(icon.src(), "data:image/svg+xml,x");
    assert_eq!(icon.mime_type(), Some("image/svg+xml"));
    assert_eq!(icon.sizes(), &["16x16"]);
    assert_eq!(icon.theme(), Some(crate::McpIconTheme::Light));
    assert_eq!(crate::McpToolMetadata::new().tool_annotations(), None);
}
