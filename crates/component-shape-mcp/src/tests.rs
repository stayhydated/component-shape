use super::{
    McpAny, McpInput, McpJsonSchema as _, McpRange, McpServer, McpToolInput as _,
    McpValidationParam, McpValidationRule, McpValidationScope, McpValidationTarget,
    McpValidationTypeArgMode, ToolDefinition,
};
use serde_json::{Value, json};

fn schema(value: Value) -> super::McpSchema {
    super::McpSchema::new(value)
}

#[test]
fn schema_description_helpers_only_mutate_object_schemas() {
    let described = schema(json!({ "type": "string" })).with_description("Search text");

    assert_eq!(described["description"], "Search text");

    let boolean_schema = schema(json!(true)).with_description("Ignored");

    assert_eq!(boolean_schema.as_value(), &json!(true));
}

#[test]
fn schema_for_input_maps_range_dates() {
    let schema = super::schema_for_input(McpInput::date_range());

    assert_eq!(schema["properties"]["min"]["anyOf"][0]["format"], "date");
    assert_eq!(schema["properties"]["max"]["anyOf"][1]["type"], "null");
}

#[test]
fn validation_schema_metadata_attaches_rules_and_hints() {
    const PARAMS: &[McpValidationParam] = &[
        McpValidationParam::literal("min", "2"),
        McpValidationParam::literal("max", "8"),
    ];
    const RULES: &[McpValidationRule] = &[McpValidationRule::new(
        McpValidationScope::Field,
        "LenValidation",
        "koruma_collection::collection::LenValidation",
        Some("Title"),
        McpValidationTypeArgMode::Infer,
        PARAMS,
    )
    .with_target(McpValidationTarget::Default)];
    let mut object = json!({ "type": "string" })
        .as_object()
        .expect("schema should be an object")
        .clone();

    super::apply_validation_schema_metadata(&mut object, "x-testValidation", RULES);

    assert_eq!(object["minLength"], json!(2));
    assert_eq!(object["maxLength"], json!(8));
    assert_eq!(object["x-testValidation"][0]["scope"], json!("field"));
    assert_eq!(
        object["x-testValidation"][0]["path"],
        json!("koruma_collection::collection::LenValidation")
    );
    assert_eq!(object["x-testValidation"][0]["target"], json!("default"));
    assert_eq!(object["x-testValidation"][0]["label"], json!("Title"));
}

#[test]
fn validation_issues_error_preserves_field_filter_and_rule_details() {
    const PARAMS: &[McpValidationParam] = &[McpValidationParam::literal("min", "1")];
    const RULE: McpValidationRule = McpValidationRule::new(
        McpValidationScope::Filter,
        "LenValidation",
        "LenValidation",
        None,
        McpValidationTypeArgMode::Infer,
        PARAMS,
    );
    let error = super::validation_issues_error(vec![
        super::McpValidationIssue::required("title"),
        super::McpValidationIssue::for_filter_rule("name", RULE, "name validation failed"),
    ]);
    let structured = error.to_structured_value();

    assert_eq!(structured["kind"], json!("validation"));
    assert_eq!(structured["details"][0]["field"], json!("title"));
    assert_eq!(structured["details"][0]["validator"], json!("required"));
    assert_eq!(structured["details"][1]["scope"], json!("filter"));
    assert_eq!(structured["details"][1]["filter"], json!("name"));
    assert_eq!(structured["details"][1]["params"][0]["value"], json!("1"));
}

#[test]
fn schema_for_input_distinguishes_any_from_unsupported() {
    assert_eq!(
        super::schema_for_input(McpInput::any()).as_value(),
        &json!({})
    );
    assert_eq!(
        super::schema_for_input(McpInput::unsupported()).as_value(),
        &json!({ "not": {} })
    );
}

#[test]
fn schema_for_input_distinguishes_lists_from_sets() {
    let list_schema = super::schema_for_input(McpInput::string_list());
    let set_schema = super::schema_for_input(McpInput::string_set());

    assert_eq!(list_schema["type"], "array");
    assert!(list_schema["uniqueItems"].is_null());
    assert_eq!(set_schema["type"], "array");
    assert_eq!(set_schema["uniqueItems"], true);
}

#[test]
fn mcp_input_descriptor_value_describes_supported_shapes() {
    assert_eq!(
        super::mcp_input_descriptor_value(McpInput::unsupported()),
        json!({
            "supported": false,
            "shape": "unsupported",
        })
    );
    assert_eq!(
        super::mcp_input_descriptor_value(McpInput::string_set()),
        json!({
            "supported": true,
            "shape": "set",
            "items": "string",
        })
    );
    assert_eq!(
        super::mcp_input_descriptor_value(McpInput::date_range()),
        json!({
            "supported": true,
            "shape": "range",
            "bound": "date",
        })
    );
}

#[test]
fn json_schema_trait_supports_aliases_and_containers() {
    type UserId = u64;

    assert_eq!(
        <UserId as super::McpJsonSchema>::json_schema()["type"],
        "integer"
    );
    assert_eq!(
        <Option<Vec<String>> as super::McpJsonSchema>::json_schema()["anyOf"][0]["items"]["type"],
        "string"
    );
    assert_eq!(
        <std::collections::HashSet<String> as super::McpJsonSchema>::json_schema()["uniqueItems"],
        true
    );
    assert_eq!(
        <std::collections::BTreeMap<String, u32> as super::McpJsonSchema>::json_schema()["additionalProperties"]
            ["type"],
        "integer"
    );
    assert_eq!(
        <serde_json::Map<String, u32> as super::McpJsonSchema>::json_schema()["additionalProperties"]
            ["type"],
        "integer"
    );
    assert_eq!(
        <McpAny as super::McpJsonSchema>::json_schema().as_value(),
        &json!({})
    );
    assert_eq!(
        <serde_json::Value as super::McpJsonSchema>::json_schema().as_value(),
        &json!({})
    );
    assert_eq!(
        <&str as super::McpJsonSchema>::json_schema()["type"],
        "string"
    );
    assert_eq!(
        <std::borrow::Cow<'static, str> as super::McpJsonSchema>::json_schema()["type"],
        "string"
    );
    assert_eq!(
        <[String] as super::McpJsonSchema>::json_schema()["items"]["type"],
        "string"
    );
    assert_eq!(
        <McpRange<u32> as super::McpJsonSchema>::json_schema()["properties"]["min"]["anyOf"][0]["type"],
        "integer"
    );
    assert_eq!(
        <(u32, String) as super::McpJsonSchema>::json_schema()["prefixItems"][0]["type"],
        "integer"
    );
    assert_eq!(
        <(u32, String) as super::McpJsonSchema>::json_schema()["prefixItems"][1]["type"],
        "string"
    );
    assert_eq!(
        <(u32, String) as super::McpJsonSchema>::json_schema()["minItems"],
        2
    );
    assert_eq!(
        serde_json::from_value::<McpRange<u32>>(json!({
            "min": 1,
            "max": null
        }))
        .expect("range object should decode"),
        McpRange::new(Some(1), None)
    );
    assert!(
        serde_json::from_value::<McpRange<u32>>(json!({
            "min": 1,
            "step": 2
        }))
        .is_err()
    );
}

#[test]
fn explicit_any_value_accepts_unconstrained_json() {
    for raw in [
        Value::Null,
        json!(true),
        json!(42),
        json!("text"),
        json!(["nested", null]),
        json!({ "nested": { "value": true } }),
    ] {
        let decoded = <McpAny as super::McpToolValue>::from_tool_value("payload", raw.clone())
            .expect("McpAny should accept any JSON value");

        assert_eq!(decoded.into_value(), raw);
    }
}

#[test]
fn tool_value_trait_pairs_schema_and_strict_decode() {
    let schema = <McpRange<u32> as super::McpToolValue>::tool_value_schema();
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["properties"]["min"]["anyOf"][0]["type"], "integer");

    let value = <McpRange<u32> as super::McpToolValue>::from_tool_value(
        "window",
        json!({ "min": 1, "max": null }),
    )
    .expect("range value should decode");
    assert_eq!(value, McpRange::new(Some(1), None));

    let error = <String as super::McpToolValue>::from_tool_value("title", Value::Null)
        .expect_err("null should not decode as a present string");
    assert_eq!(
        error,
        super::McpToolError::UnexpectedNull {
            field: "title".to_string(),
        }
    );

    let value = <Option<String> as super::McpToolValue>::from_tool_value("title", Value::Null)
        .expect("nullable schema should decode null");
    assert_eq!(value, None);
}

#[test]
fn tool_value_trait_enforces_closed_object_schema_before_serde() {
    #[derive(Debug, serde::Deserialize, crate::McpJsonSchema, PartialEq)]
    struct Preferences {
        #[mcp(rename = "email", alias = "emailUpdates")]
        email_updates: bool,
        #[serde(default)]
        topics: Vec<String>,
    }

    let value = <Preferences as super::McpToolValue>::from_tool_value(
        "preferences",
        json!({
            "email": true,
            "topics": ["rust"]
        }),
    )
    .expect("alias should decode through serde after schema validation");
    assert_eq!(
        value,
        Preferences {
            email_updates: true,
            topics: vec!["rust".to_string()],
        }
    );

    let error = <Preferences as super::McpToolValue>::from_tool_value(
        "preferences",
        json!({
            "email": true,
            "emailUpdates": false
        }),
    )
    .expect_err("primary and alias should be rejected as duplicate input");
    assert_eq!(
        error,
        super::McpToolError::DuplicateField {
            field: "preferences.email".to_string(),
        }
    );

    let error = <Preferences as super::McpToolValue>::from_tool_value(
        "preferences",
        json!({
            "email": true,
            "unexpected": true
        }),
    )
    .expect_err("unknown nested field should be rejected before serde");
    assert_eq!(
        error,
        super::McpToolError::UnknownField {
            field: "preferences.unexpected".to_string(),
        }
    );

    let error = <Preferences as super::McpToolValue>::from_tool_value("preferences", json!({}))
        .expect_err("missing required nested field should be rejected before serde");
    assert_eq!(
        error,
        super::McpToolError::MissingField {
            field: "preferences.email".to_string(),
        }
    );
}

#[test]
fn tool_value_trait_normalizes_mcp_enum_aliases_before_serde() {
    #[derive(Debug, serde::Deserialize, crate::McpJsonSchema, PartialEq)]
    enum IssueState {
        Open,
        #[mcp(rename = "in-review", alias = "reviewing")]
        InReview,
    }

    let value = <IssueState as super::McpToolValue>::from_tool_value("state", json!("reviewing"))
        .expect("mcp enum alias should decode");

    assert_eq!(value, IssueState::InReview);
}

#[test]
fn tool_value_trait_applies_string_keyed_object_value_schemas() {
    #[derive(Debug, serde::Deserialize, crate::McpJsonSchema, PartialEq)]
    enum IssueState {
        Open,
        #[mcp(rename = "in-review", alias = "reviewing")]
        InReview,
    }

    let states =
        <std::collections::BTreeMap<String, IssueState> as super::McpToolValue>::from_tool_value(
            "states",
            json!({
                "issue": "reviewing"
            }),
        )
        .expect("additional property values should be normalized through their schema");
    let expected = std::collections::BTreeMap::from([("issue".to_string(), IssueState::InReview)]);
    assert_eq!(states, expected);

    #[derive(Debug, serde::Deserialize, crate::McpJsonSchema, PartialEq)]
    struct Preferences {
        #[mcp(rename = "email", alias = "emailUpdates")]
        email_updates: bool,
    }

    let error =
        <std::collections::BTreeMap<String, Preferences> as super::McpToolValue>::from_tool_value(
            "preferences",
            json!({
                "team": {
                    "email": true,
                    "unexpected": true
                }
            }),
        )
        .expect_err("nested unknown fields should be rejected through additionalProperties schema");
    assert_eq!(
        error,
        super::McpToolError::UnknownField {
            field: "preferences.team.unexpected".to_string(),
        }
    );

    let error =
        <std::collections::BTreeMap<String, Preferences> as super::McpToolValue>::from_tool_value(
            "preferences",
            json!({
                "team": {}
            }),
        )
        .expect_err(
            "nested required fields should be enforced through additionalProperties schema",
        );
    assert_eq!(
        error,
        super::McpToolError::MissingField {
            field: "preferences.team.email".to_string(),
        }
    );
}

#[test]
fn schema_allows_null_matches_common_schema_forms() {
    assert!(super::schema_allows_null(&schema(json!({}))));
    assert!(super::schema_allows_null(&schema(json!({
        "anyOf": [
            { "type": "string" },
            { "type": "null" }
        ]
    }))));
    assert!(super::schema_allows_null(&schema(json!({
        "type": ["string", "null"]
    }))));
    assert!(!super::schema_allows_null(&schema(json!({
        "type": "string"
    }))));
}

#[test]
fn json_schema_derive_builds_object_schema() {
    #[derive(crate::McpJsonSchema)]
    #[allow(dead_code)]
    struct SearchArgs {
        #[mcp(rename = "q", alias = "query", description = "Search text")]
        query: String,
        page: Option<u32>,
        #[serde(skip)]
        internal: String,
    }

    let schema = SearchArgs::json_schema();

    assert_eq!(schema["type"], "object");
    assert_eq!(schema["properties"]["q"]["type"], "string");
    assert_eq!(schema["properties"]["q"]["description"], "Search text");
    assert_eq!(schema["properties"]["q"]["x-mcpAliases"], json!(["query"]));
    assert_eq!(schema["properties"]["q"]["x-mcpDecodeName"], "query");
    assert_eq!(schema["properties"]["page"]["anyOf"][0]["type"], "integer");
    assert_eq!(schema["required"], json!(["q"]));
    assert!(schema["properties"].get("internal").is_none());
}

#[test]
fn json_schema_derive_infers_doc_descriptions() {
    /// Search arguments sent to the tool.
    #[derive(crate::McpJsonSchema)]
    #[allow(dead_code)]
    struct SearchArgs {
        /// Full text query.
        query: String,
    }

    let schema = SearchArgs::json_schema();

    assert_eq!(schema["description"], "Search arguments sent to the tool.");
    assert_eq!(
        schema["properties"]["query"]["description"],
        "Full text query."
    );
}

#[test]
fn json_schema_derive_builds_enum_schema() {
    /// Issue state.
    #[derive(crate::McpJsonSchema)]
    #[allow(dead_code)]
    #[serde(rename_all = "kebab-case")]
    enum IssueState {
        Open,
        #[serde(alias = "reviewing")]
        InReview,
        #[mcp(rename = "done", alias = "resolved")]
        Closed,
        #[serde(other)]
        Unknown,
    }

    let schema = IssueState::json_schema();

    assert_eq!(schema["type"], "string");
    assert_eq!(schema["description"], "Issue state.");
    assert_eq!(
        schema["enum"],
        json!(["open", "in-review", "reviewing", "done", "resolved"])
    );
    assert_eq!(schema["x-mcpEnumDecodeAliases"]["done"], "closed");
    assert_eq!(schema["x-mcpEnumDecodeAliases"]["resolved"], "closed");
}

#[test]
fn tool_input_derive_builds_schema_and_decodes_strict_arguments() {
    fn default_limit() -> usize {
        25
    }

    #[derive(Debug, crate::McpToolInput, PartialEq)]
    #[allow(dead_code)]
    #[serde(rename_all = "camelCase")]
    struct SearchInput {
        #[serde(rename(deserialize = "q"), alias = "queryText")]
        query: String,
        page_size: Option<u32>,
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(skip)]
        internal: String,
    }

    let schema = SearchInput::input_schema();
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["properties"]["q"]["type"], "string");
    assert_eq!(
        schema["properties"]["q"]["x-mcpAliases"],
        json!(["queryText"])
    );
    assert_eq!(
        schema["properties"]["pageSize"]["anyOf"][0]["type"],
        "integer"
    );
    assert_eq!(schema["required"], json!(["q"]));
    assert!(schema["properties"].get("internal").is_none());
    assert_eq!(SearchInput::json_schema(), schema);

    let input = SearchInput::from_tool_call(
        super::McpToolCall::from_value(Some(json!({
            "queryText": "rust",
            "pageSize": 2
        })))
        .expect("tool call should normalize"),
    )
    .expect("input should decode");

    assert_eq!(
        input,
        SearchInput {
            query: "rust".to_string(),
            page_size: Some(2),
            limit: 25,
            internal: String::new(),
        }
    );
}

#[test]
fn tool_input_derive_uses_custom_tool_value_decoders() {
    #[derive(Debug, PartialEq)]
    struct SlashSeparatedTags(Vec<String>);

    impl super::McpToolValue for SlashSeparatedTags {
        fn tool_value_schema() -> super::McpSchema {
            schema(json!({ "type": "string" }))
        }

        fn from_tool_value(field: &str, value: Value) -> Result<Self, super::McpToolError> {
            let raw = value.as_str().ok_or_else(|| {
                super::McpToolError::decode(field, "expected slash-separated tags")
            })?;
            Ok(Self(
                raw.split('/')
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(ToString::to_string)
                    .collect(),
            ))
        }
    }

    #[derive(Debug, crate::McpToolInput, PartialEq)]
    struct TagInput {
        tags: SlashSeparatedTags,
    }

    assert_eq!(
        TagInput::input_schema()["properties"]["tags"]["type"],
        "string"
    );

    let input = TagInput::from_tool_call(
        super::McpToolCall::from_value(Some(json!({
            "tags": "alpha / beta"
        })))
        .expect("tool call should normalize"),
    )
    .expect("custom tool value should decode");

    assert_eq!(
        input,
        TagInput {
            tags: SlashSeparatedTags(vec!["alpha".to_string(), "beta".to_string()])
        }
    );

    let error = TagInput::from_tool_call(
        super::McpToolCall::from_value(Some(json!({
            "tags": ["alpha"]
        })))
        .expect("tool call should normalize"),
    )
    .expect_err("custom tool value should reject invalid JSON");

    assert_eq!(
        error,
        super::McpToolError::DecodeField {
            field: "tags".to_string(),
            message: "expected slash-separated tags".to_string(),
        }
    );
}

#[test]
fn tool_input_derive_rejects_missing_duplicate_and_unknown_fields() {
    #[derive(Debug, crate::McpToolInput, PartialEq)]
    #[allow(dead_code)]
    struct SearchInput {
        #[serde(alias = "queryText")]
        query: String,
    }

    let missing = SearchInput::from_tool_call(
        super::McpToolCall::from_value(Some(json!({}))).expect("tool call should normalize"),
    )
    .expect_err("required field should be enforced");
    assert_eq!(
        missing,
        super::McpToolError::MissingField {
            field: "query".to_string()
        }
    );

    let duplicate = SearchInput::from_tool_call(
        super::McpToolCall::from_value(Some(json!({
            "query": "rust",
            "queryText": "go"
        })))
        .expect("tool call should normalize"),
    )
    .expect_err("duplicate aliases should be rejected");
    assert_eq!(
        duplicate,
        super::McpToolError::DuplicateField {
            field: "query".to_string()
        }
    );

    let unknown = SearchInput::from_tool_call(
        super::McpToolCall::from_value(Some(json!({
            "query": "rust",
            "extra": true
        })))
        .expect("tool call should normalize"),
    )
    .expect_err("unknown fields should be rejected");
    assert_eq!(
        unknown,
        super::McpToolError::UnknownField {
            field: "extra".to_string()
        }
    );
}

#[test]
fn tool_metadata_records_optional_overrides() {
    static ICONS: &[super::McpToolIcon] =
        &[super::McpToolIcon::new("https://example.com/tool.png")
            .with_mime_type("image/png")
            .with_sizes(&["48x48"])
            .with_theme(super::McpIconTheme::Light)];

    let metadata = super::McpToolMetadata::new()
        .with_name("custom_tool")
        .with_title("Custom tool")
        .with_description("Runs a custom tool.")
        .with_read_only_hint(true)
        .with_destructive_hint(false)
        .with_idempotent_hint(true)
        .with_open_world_hint(false)
        .with_icons(ICONS)
        .with_task_support(super::McpToolTaskSupport::Optional);

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
        Some(super::McpIconTheme::Light)
    );
    assert_eq!(
        metadata.task_support(),
        Some(super::McpToolTaskSupport::Optional)
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
    static EMPTY_SRC_ICONS: &[super::McpToolIcon] = &[super::McpToolIcon::new("")];

    assert!(
        super::McpToolMetadata::new()
            .with_name("custom_tool")
            .with_title("Custom tool")
            .with_description("Runs a custom tool.")
            .validate()
            .is_ok()
    );
    assert!(
        super::McpToolMetadata::new()
            .with_icons(EMPTY_SRC_ICONS)
            .validate()
            .is_err()
    );
    assert!(
        super::McpToolMetadata::new()
            .with_name("bad name")
            .validate()
            .is_err()
    );
    assert!(
        super::McpToolMetadata::new()
            .with_description(" ")
            .validate()
            .is_err()
    );
    assert!(
        super::McpToolMetadata::new()
            .with_read_only_hint(true)
            .with_destructive_hint(true)
            .validate()
            .is_err()
    );
}

#[test]
fn tool_definition_validates_name_and_metadata() {
    assert!(
        super::tool_definition("", None, None, schema(json!({ "type": "object" })), None).is_err()
    );
    assert!(
        super::tool_definition(
            "bad name",
            None,
            None,
            schema(json!({ "type": "object" })),
            None
        )
        .is_err()
    );
    assert!(
        super::tool_definition(
            "valid_name",
            Some("".to_string()),
            None,
            schema(json!({ "type": "object" })),
            None,
        )
        .is_err()
    );
    assert!(super::tool_definition("valid", None, None, schema(json!("bad")), None).is_err());
    assert!(
        super::tool_definition(
            "valid",
            None,
            None,
            schema(json!({ "type": "string" })),
            None
        )
        .is_err()
    );
    assert!(
        super::tool_definition(
            "valid",
            None,
            None,
            schema(json!({ "type": "object" })),
            Some(schema(json!({}))),
        )
        .is_err()
    );
    assert!(
        super::tool_definition(
            "valid",
            None,
            None,
            schema(json!({ "type": "object" })),
            Some(schema(json!({ "type": "string" }))),
        )
        .is_err()
    );
    assert!(
        super::tool_definition(
            "valid",
            None,
            None,
            schema(json!({ "type": "object" })),
            Some(schema(json!({ "type": "object" }))),
        )
        .is_ok()
    );
    assert!(
        super::tool_definition(
            "valid",
            None,
            None,
            schema(json!({ "type": "object" })),
            Some(schema(json!(false))),
        )
        .is_err()
    );
    assert!(
        super::tool_definition_with_annotations(
            "valid",
            None,
            None,
            schema(json!({ "type": "object" })),
            None,
            Some(
                super::McpToolAnnotations::new()
                    .read_only(true)
                    .destructive(true),
            ),
        )
        .is_err()
    );
}

#[test]
fn tool_definition_accepts_valid_name() {
    let ToolDefinition { name, .. } = super::tool_definition(
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
        super::tool_definition_for_input::<EchoInput>("echo", Some("Echo".to_string()), None, None)
            .expect("tool definition should build");
    fn assert_typed_tool(_tool: &super::McpTypedTool<EchoInput>) {}
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

    static ICONS: &[super::McpToolIcon] =
        &[super::McpToolIcon::new("https://example.com/echo.svg")
            .with_mime_type("image/svg+xml")
            .with_sizes(&["any"])
            .with_theme(super::McpIconTheme::Dark)];

    let metadata = super::McpToolMetadata::new()
        .with_name("custom_echo")
        .with_title("Custom echo")
        .with_description("Echoes a value.")
        .with_read_only_hint(true)
        .with_destructive_hint(false)
        .with_open_world_hint(false)
        .with_icons(ICONS)
        .with_task_support(super::McpToolTaskSupport::Required);
    let tool = super::tool_definition_for_input_with_metadata::<EchoInput>("echo", metadata, None)
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
    assert_eq!(icons[0].theme, Some(super::McpIconTheme::Dark));
    assert_eq!(
        tool.execution
            .as_ref()
            .and_then(|execution| execution.task_support),
        Some(super::McpToolTaskSupport::Required)
    );
    assert_eq!(tool.input_schema["properties"]["value"]["type"], "string");
}

#[test]
fn unit_tool_input_accepts_only_empty_arguments() {
    assert_eq!(
        <() as super::McpToolInput>::input_schema()["properties"],
        json!({})
    );
    assert_eq!(
        <() as super::McpToolInput>::from_tool_call(super::McpToolCall::empty()),
        Ok(())
    );
    assert_eq!(
        <() as super::McpToolInput>::from_tool_call(
            super::McpToolCall::from_value(Some(json!({ "extra": true })))
                .expect("tool call should normalize")
        ),
        Err(super::McpToolError::UnknownField {
            field: "extra".to_string()
        })
    );
}

#[test]
fn schema_object_rejects_non_object_values() {
    let object = super::schema_object("input_schema", schema(json!({ "type": "object" })))
        .expect("schema should be accepted");
    assert_eq!(object["type"], "object");

    let error = super::schema_object("input_schema", schema(json!(null)))
        .expect_err("schema should be rejected");
    assert!(error.to_string().contains("input_schema"));
}

#[test]
fn mcp_tool_call_normalizes_arguments() {
    let call = super::McpToolCall::from_value(None).expect("missing arguments are empty");
    assert!(call.arguments().is_empty());

    let call = super::McpToolCall::from_value(Some(json!({ "value": 42 })))
        .expect("object arguments are accepted");
    assert_eq!(call.arguments()["value"], 42);

    let error = super::McpToolCall::from_value(Some(json!(42)))
        .expect_err("non-object arguments should fail");
    assert_eq!(error, super::McpToolError::ArgumentsMustBeObject);
}

#[test]
fn mcp_arguments_decodes_tool_values_and_rejects_unknown_fields() {
    let call = super::McpToolCall::from_value(Some(json!({
        "name": "Ada",
        "nickname": null,
        "limit": 2,
        "unused": true
    })))
    .expect("object arguments are accepted");
    let mut arguments = call.into_arguments();

    let name = arguments
        .take_required_tool_value::<String>("name")
        .expect("name should decode");
    let nickname = arguments
        .take_present_tool_value::<Option<String>>("nickname")
        .expect("nickname should decode");
    let limit = arguments
        .take_present_tool_value::<usize>("limit")
        .expect("limit should decode");

    assert_eq!(name, "Ada");
    assert_eq!(nickname, Some(None));
    assert_eq!(limit, Some(2));
    assert_eq!(
        arguments.finish(),
        Err(super::McpToolError::UnknownField {
            field: "unused".to_string()
        })
    );
}

#[test]
fn tool_value_usize_rejects_negative_values() {
    assert_eq!(
        <usize as super::McpToolValue>::from_tool_value("limit", json!(10))
            .expect("usize should decode"),
        10
    );

    let error = <usize as super::McpToolValue>::from_tool_value("limit", json!(-1))
        .expect_err("negative should fail");
    assert!(matches!(error, super::McpToolError::DecodeField { field, .. } if field == "limit"));
}

#[test]
fn tool_names_are_stable_and_mcp_friendly() {
    assert_eq!(
        super::tool_name("example::tools", "Contact", "tool_"),
        "example_tools_contact"
    );
    assert_eq!(
        super::tool_name("example::forms", "ContactRequest", "tool_"),
        "example_forms_contact_request"
    );
    assert_eq!(
        super::tool_name("example::api", "HTTPServer2", "tool_"),
        "example_api_http_server2"
    );
    assert_eq!(super::tool_name("123", "", "tool_"), "tool_123");
}

#[test]
fn server_handles_direct_tools_call() {
    let mut server = McpServer::new("test-server", "0.0.0");
    server
        .add_tool(
            super::tool_definition(
                "echo",
                None,
                None,
                schema(json!({ "type": "object" })),
                None,
            )
            .expect("tool definition should build"),
            |call| super::tool_structured_result(Value::Object(call.into_arguments().into_inner())),
        )
        .expect("tool should register");

    assert!(server.contains_tool("echo"));
    assert_eq!(server.tool_count(), 1);

    let result = server.call_tool("echo", Some(json!({ "value": 42 })));

    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.structured_content.expect("structured")["value"], 42);
}

#[test]
fn server_handles_async_tools_call() {
    let mut server = McpServer::new("test-server", "0.0.0");
    server
        .add_tool_async(
            super::tool_definition(
                "echo",
                None,
                None,
                schema(json!({ "type": "object" })),
                None,
            )
            .expect("tool definition should build"),
            |call| async move {
                super::tool_structured_result(Value::Object(call.into_arguments().into_inner()))
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
            super::tool_definition_for_input::<EchoInput>("echo", None, None, None)
                .expect("tool definition should build"),
            |input| super::tool_structured_result(json!({ "value": input.value })),
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
    fn echo_output_schema() -> super::McpSchema {
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
            super::tool_definition(
                "missing_structured_content",
                None,
                None,
                schema(json!({ "type": "object" })),
                Some(echo_output_schema()),
            )
            .expect("tool definition should build"),
            |_| super::ToolCallResult::success(vec![super::ContentBlock::text("ok")]),
        )
        .expect("tool should register");
    server
        .add_tool(
            super::tool_definition(
                "non_object_structured_content",
                None,
                None,
                schema(json!({ "type": "object" })),
                Some(echo_output_schema()),
            )
            .expect("tool definition should build"),
            |_| super::tool_structured_result(json!("not an object")),
        )
        .expect("tool should register");
    server
        .add_tool(
            super::tool_definition(
                "schema_mismatch",
                None,
                None,
                schema(json!({ "type": "object" })),
                Some(echo_output_schema()),
            )
            .expect("tool definition should build"),
            |_| super::tool_structured_result(json!({})),
        )
        .expect("tool should register");
    server
        .add_tool(
            super::tool_definition(
                "handler_error",
                None,
                None,
                schema(json!({ "type": "object" })),
                Some(echo_output_schema()),
            )
            .expect("tool definition should build"),
            |_| super::tool_error_result_for(super::McpToolError::handler("boom")),
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

    let result = server.call_tool("non_object_structured_content", Some(json!({})));
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.expect("structured error")["error"],
        json!({
            "kind": "invalid_tool_output",
            "message": "tool `non_object_structured_content` returned invalid structured content: tool declares output_schema with object root but returned non-object structured_content",
            "name": "non_object_structured_content",
            "detail": "tool declares output_schema with object root but returned non-object structured_content"
        })
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
fn tool_errors_include_structured_content() {
    let result = super::tool_error_result("plain failure");
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.expect("structured error")["error"],
        json!({
            "kind": "error",
            "message": "plain failure"
        })
    );

    let result = super::serialize_handler_response::<(), _>(Err("No client"));
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.expect("structured error")["error"],
        json!({
            "kind": "handler",
            "message": "handler failed: No client",
            "detail": "No client"
        })
    );

    let result = super::tool_error_result_for(super::McpToolError::validation_details([
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
        use rmcp::{ServiceExt as _, model::CallToolRequestParams};

        let mut server = McpServer::new("test-server", "0.0.0");
        server
            .add_tool(
                super::tool_definition(
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
                    super::tool_structured_result(Value::Object(call.into_arguments().into_inner()))
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

        let client = ().serve(client_transport).await.expect("client should start");

        let tools = client
            .peer()
            .list_tools(Default::default())
            .await
            .expect("tools/list should succeed");
        assert_eq!(tools.tools.len(), 1);
        assert_eq!(tools.tools[0].name, "echo");
        assert_eq!(tools.tools[0].title.as_deref(), Some("Echo"));

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
        assert_eq!(result.structured_content.expect("structured")["value"], 42);

        client.cancel().await.expect("client should close");
        server_handle.await.expect("server should finish");
    });
}

#[test]
fn json_resource_specs_register_and_reuse_generated_resources() {
    let specs = vec![
        super::McpJsonResourceSpec::new(
            "gpui-form://forms/contact/descriptor",
            "contact_descriptor",
            Some("Contact descriptor".to_string()),
            Some("Descriptor for the contact form.".to_string()),
            json!({ "fields": ["name", "email"] }),
        )
        .expect("resource spec should build"),
    ];
    let definitions = super::json_resource_definitions(&specs).expect("definitions should build");
    assert_eq!(definitions[0].uri, "gpui-form://forms/contact/descriptor");
    assert_eq!(
        definitions[0].mime_type.as_deref(),
        Some("application/json")
    );

    let mut server = super::McpServer::new("test", "0.0.0");
    super::register_json_resource_specs_if_missing(&mut server, specs.clone())
        .expect("resources should register");
    super::register_json_resource_specs_if_missing(&mut server, specs)
        .expect("existing complete resource set should be reused");

    assert_eq!(server.resource_count(), 1);
    assert!(server.contains_resource("gpui-form://forms/contact/descriptor"));
}

#[test]
fn json_resource_specs_reject_duplicate_uris() {
    let specs = vec![
        super::McpJsonResourceSpec::new(
            "gpui-form://forms/contact/descriptor",
            "contact_descriptor",
            None,
            None,
            json!({}),
        )
        .expect("resource spec should build"),
        super::McpJsonResourceSpec::new(
            "gpui-form://forms/contact/descriptor",
            "duplicate_contact_descriptor",
            None,
            None,
            json!({}),
        )
        .expect("resource spec should build"),
    ];

    assert_eq!(
        super::ensure_json_resource_specs_distinct(&specs).expect_err("duplicate URI should fail"),
        super::McpToolError::duplicate_resource("gpui-form://forms/contact/descriptor")
    );
}

#[test]
fn server_exposes_resources_and_prompts_through_rmcp_protocol() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime should start");

    runtime.block_on(async {
        use rmcp::{
            ServerHandler as _, ServiceExt as _,
            model::{GetPromptRequestParams, ReadResourceRequestParams},
        };

        let descriptor_uri = "gpui-form://forms/contact/descriptor";
        let descriptor = json!({
            "tool": "form_contact",
            "fields": ["name", "email"]
        });
        let descriptor_resource = descriptor.clone();
        let mut server = McpServer::new("test-server", "0.0.0");
        server
            .add_resource(
                super::resource_definition(
                    descriptor_uri,
                    "contact_descriptor",
                    Some("Contact descriptor".to_string()),
                    Some("Descriptor for the contact form.".to_string()),
                    Some("application/json".to_string()),
                )
                .expect("resource definition should build"),
                move || {
                    super::json_resource_result(descriptor_uri, &descriptor_resource)
                        .expect("descriptor resource should encode")
                },
            )
            .expect("resource should register");
        server
            .add_resource_template(
                super::resource_template_definition(
                    "gpui-form://forms/{form}/descriptor",
                    "gpui_form_descriptor",
                    Some("GPUI form descriptor".to_string()),
                    Some("Descriptor for a generated GPUI form.".to_string()),
                    Some("application/json".to_string()),
                )
                .expect("resource template should build"),
            )
            .expect("resource template should register");
        server
            .add_prompt(
                super::prompt_definition(
                    "draft_contact",
                    Some("Draft contact".to_string()),
                    Some("Draft values for the contact form.".to_string()),
                    None,
                )
                .expect("prompt definition should build"),
                |_| {
                    super::text_prompt_result(
                        Some("Draft values for the contact form.".to_string()),
                        "Use the contact descriptor resource and return valid form fields.",
                    )
                },
            )
            .expect("prompt should register");

        let info = server.get_info();
        assert!(info.capabilities.resources.is_some());
        assert!(info.capabilities.prompts.is_some());

        let (server_transport, client_transport) = tokio::io::duplex(4096);
        let server_handle = tokio::spawn(async move {
            let service = server
                .serve(server_transport)
                .await
                .expect("server should start");
            service.waiting().await.expect("server task should join");
        });

        let client = ().serve(client_transport).await.expect("client should start");

        let resources = client
            .peer()
            .list_resources(Default::default())
            .await
            .expect("resources/list should succeed");
        assert_eq!(resources.resources.len(), 1);
        assert_eq!(resources.resources[0].uri, descriptor_uri);
        assert_eq!(
            resources.resources[0].title.as_deref(),
            Some("Contact descriptor")
        );

        let templates = client
            .peer()
            .list_resource_templates(Default::default())
            .await
            .expect("resources/templates/list should succeed");
        assert_eq!(templates.resource_templates.len(), 1);
        assert_eq!(
            templates.resource_templates[0].uri_template,
            "gpui-form://forms/{form}/descriptor"
        );

        let resource = client
            .peer()
            .read_resource(ReadResourceRequestParams::new(descriptor_uri))
            .await
            .expect("resources/read should succeed");
        assert_eq!(resource.contents.len(), 1);
        match &resource.contents[0] {
            super::McpResourceContents::TextResourceContents {
                uri,
                mime_type,
                text,
                ..
            } => {
                assert_eq!(uri, descriptor_uri);
                assert_eq!(mime_type.as_deref(), Some("application/json"));
                let value: Value =
                    serde_json::from_str(text).expect("resource should contain JSON");
                assert_eq!(value, descriptor);
            },
            other => panic!("expected text resource contents, got {other:?}"),
        }

        let prompts = client
            .peer()
            .list_prompts(Default::default())
            .await
            .expect("prompts/list should succeed");
        assert_eq!(prompts.prompts.len(), 1);
        assert_eq!(prompts.prompts[0].name, "draft_contact");
        assert_eq!(prompts.prompts[0].title.as_deref(), Some("Draft contact"));

        let prompt = client
            .peer()
            .get_prompt(GetPromptRequestParams::new("draft_contact"))
            .await
            .expect("prompts/get should succeed");
        assert_eq!(
            prompt.description.as_deref(),
            Some("Draft values for the contact form.")
        );
        assert_eq!(prompt.messages.len(), 1);
        assert_eq!(prompt.messages[0].role, super::McpPromptMessageRole::User);
        match &prompt.messages[0].content {
            super::McpPromptMessageContent::Text { text } => {
                assert!(text.contains("Use the contact descriptor resource"))
            },
            other => panic!("expected text prompt message, got {other:?}"),
        }

        client.cancel().await.expect("client should close");
        server_handle.await.expect("server should finish");
    });
}

#[test]
fn server_composes_registrars() {
    fn register_echo(server: &mut McpServer) -> Result<(), super::McpToolError> {
        server.add_tool(
            super::tool_definition(
                "echo",
                None,
                None,
                schema(json!({ "type": "object" })),
                None,
            )
            .expect("tool definition should build"),
            |call| super::tool_structured_result(Value::Object(call.into_arguments().into_inner())),
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
            super::tool_definition(
                "echo",
                None,
                None,
                schema(json!({ "type": "object" })),
                None,
            )
            .expect("tool definition should build"),
            |call| super::tool_structured_result(Value::Object(call.into_arguments().into_inner())),
        )
        .expect("tool should register");

    let error = match server.add_tool(
        super::tool_definition(
            "echo",
            None,
            None,
            schema(json!({ "type": "object" })),
            None,
        )
        .expect("tool definition should build"),
        |call| super::tool_structured_result(Value::Object(call.into_arguments().into_inner())),
    ) {
        Ok(_) => panic!("duplicate tool should be rejected"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        super::McpToolError::DuplicateTool {
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
        .add_tool(invalid_name, |_| super::tool_structured_result(json!(null)))
        .expect_err("invalid raw tool name should fail");

    assert_eq!(error.kind(), "validation");
    assert_eq!(server.tool_count(), 0);

    let mut invalid_input_schema = super::tool_definition(
        "valid",
        None,
        None,
        schema(json!({ "type": "object" })),
        None,
    )
    .expect("tool definition should build");
    invalid_input_schema.input_schema = std::sync::Arc::new(
        super::schema_object("input_schema", schema(json!({ "type": "string" })))
            .expect("raw schema object should build"),
    );

    let error = server
        .add_tool(invalid_input_schema, |_| {
            super::tool_structured_result(json!(null))
        })
        .expect_err("raw tool input schema should describe object arguments");

    assert_eq!(error.kind(), "invalid_schema");
    assert_eq!(server.tool_count(), 0);

    let mut invalid_output_schema = super::tool_definition(
        "valid",
        None,
        None,
        schema(json!({ "type": "object" })),
        Some(schema(json!({ "type": "object" }))),
    )
    .expect("tool definition should build");
    invalid_output_schema.output_schema = Some(std::sync::Arc::new(
        super::schema_object("output_schema", schema(json!({ "type": "string" })))
            .expect("raw output schema object should build"),
    ));

    let error = server
        .add_tool(invalid_output_schema, |_| {
            super::tool_structured_result(json!({}))
        })
        .expect_err("raw tool output schema should describe object content");

    assert_eq!(error.kind(), "invalid_schema");
    assert_eq!(server.tool_count(), 0);

    let mut conflicting_annotations = super::tool_definition(
        "valid",
        None,
        None,
        schema(json!({ "type": "object" })),
        None,
    )
    .expect("tool definition should build");
    conflicting_annotations.annotations = Some(
        super::McpToolAnnotations::new()
            .read_only(true)
            .destructive(true),
    );

    let error = server
        .add_tool(conflicting_annotations, |_| {
            super::tool_structured_result(json!(null))
        })
        .expect_err("conflicting raw annotations should fail");

    assert_eq!(error.kind(), "validation");
    assert_eq!(server.tool_count(), 0);
}

#[test]
fn server_accepts_owned_metadata() {
    let server = McpServer::new("owned-server".to_string(), "1.2.3".to_string());

    assert_eq!(server.server_name, "owned-server");
    assert_eq!(server.server_version, "1.2.3");
}
