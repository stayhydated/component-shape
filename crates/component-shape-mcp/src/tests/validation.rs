use super::*;

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

    crate::apply_validation_schema_metadata(&mut object, "x-testValidation", RULES);

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
    let error = crate::validation_issues_error(vec![
        crate::McpValidationIssue::required("title"),
        crate::McpValidationIssue::for_filter_rule("name", RULE, "name validation failed"),
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
fn validation_schema_hints_cover_ranges_non_empty_values_and_arrays() {
    static RANGE_PARAMS: &[crate::McpValidationParam] = &[
        crate::McpValidationParam::literal("min", "-2.5"),
        crate::McpValidationParam::literal("max", "18446744073709551615"),
        crate::McpValidationParam::literal("exclusive_min", "true"),
        crate::McpValidationParam::literal("exclusive_max", "false"),
    ];
    static LEN_PARAMS: &[crate::McpValidationParam] = &[
        crate::McpValidationParam::literal("min", "2"),
        crate::McpValidationParam::literal("max", "4"),
    ];
    let range = crate::McpValidationRule::new(
        crate::McpValidationScope::Field,
        "RangeValidation",
        "validators::RangeValidation",
        None,
        crate::McpValidationTypeArgMode::None,
        RANGE_PARAMS,
    );
    let len = crate::McpValidationRule::new(
        crate::McpValidationScope::Field,
        "LenValidation",
        "validators::LenValidation",
        None,
        crate::McpValidationTypeArgMode::None,
        LEN_PARAMS,
    );
    let non_empty = crate::McpValidationRule::new(
        crate::McpValidationScope::Field,
        "NonEmptyValidation",
        "validators::NonEmptyValidation",
        None,
        crate::McpValidationTypeArgMode::None,
        crate::MCP_VALIDATION_PARAMS_NONE,
    );

    let mut number = crate::McpSchema::number().into_value();
    crate::apply_validation_schema_hints(
        number
            .as_object_mut()
            .expect("number schema should be object"),
        &[range],
    );
    assert_eq!(number["exclusiveMinimum"], json!(-2.5));
    assert_eq!(number["maximum"], json!(18446744073709551615_u64));

    let mut array = crate::McpSchema::array(crate::McpSchema::string()).into_value();
    crate::apply_validation_schema_hints(
        array
            .as_object_mut()
            .expect("array schema should be object"),
        &[len, non_empty],
    );
    assert_eq!(array["minItems"], 2);
    assert_eq!(array["maxItems"], 4);

    let mut string = crate::McpSchema::string().into_value();
    crate::apply_validation_schema_hints(
        string
            .as_object_mut()
            .expect("string schema should be object"),
        &[non_empty],
    );
    assert_eq!(string["minLength"], 1);

    let mut any_of = json!({ "anyOf": [false, { "type": "string" }] });
    crate::apply_validation_schema_hints(
        any_of.as_object_mut().expect("schema should be object"),
        &[non_empty],
    );
    assert_eq!(any_of["minLength"], 1);

    let mut object = json!({ "type": "object" });
    crate::apply_validation_schema_hints(
        object.as_object_mut().expect("schema should be object"),
        &[range, len, non_empty],
    );
    assert_eq!(object, json!({ "type": "object" }));
}

#[test]
fn validation_metadata_accessors_and_issue_builders_are_structured() {
    static PARAMS: &[crate::McpValidationParam] = &[
        crate::McpValidationParam::literal("min", "1"),
        crate::McpValidationParam::expr("max", "limits.max"),
    ];
    let rule = crate::McpValidationRule::new(
        crate::McpValidationScope::Element,
        "LenValidation",
        "validators::LenValidation::<String>",
        Some("tag length"),
        crate::McpValidationTypeArgMode::Explicit,
        PARAMS,
    )
    .with_target(crate::McpValidationTarget::Unwrapped);

    assert_eq!(PARAMS[1].expr_value(), Some("limits.max"));

    let literal = std::hint::black_box(crate::McpValidationParam::literal("min", "3"));
    assert_eq!(literal.name(), "min");
    assert_eq!(literal.literal_value(), Some("3"));
    assert_eq!(literal.expr_value(), None);
    let expr = std::hint::black_box(crate::McpValidationParam::expr("max", "limits.max"));
    assert_eq!(expr.name(), "max");
    assert_eq!(expr.literal_value(), None);
    assert_eq!(expr.expr_value(), Some("limits.max"));
    assert_eq!(
        rule.type_arg_mode(),
        crate::McpValidationTypeArgMode::Explicit
    );
    assert_eq!(rule.to_value()["target"], "unwrapped");
    assert_eq!(rule.to_value()["params"][1]["expr"], "limits.max");

    let issue = crate::McpValidationIssue::for_rule("tags", rule, "too long").with_element_index(3);
    assert_eq!(issue.field(), Some("tags"));
    assert_eq!(issue.filter(), None);
    assert_eq!(issue.to_value()["label"], "tag length");
    assert_eq!(issue.to_value()["target"], "unwrapped");
    assert_eq!(issue.to_value()["element_index"], 3);

    let issue = crate::McpValidationIssue::form("invalid form").with_label("form");
    assert_eq!(issue.to_value()["scope"], "form");
    assert_eq!(issue.to_value()["label"], "form");
}

#[test]
fn input_descriptors_and_schema_hints_cover_remaining_shape_boundaries() {
    assert_eq!(
        crate::schema_for_input(crate::McpInput::object())["type"],
        "object"
    );
    assert_eq!(
        crate::mcp_input_descriptor_value(crate::McpInput::number()),
        json!({ "supported": true, "shape": "scalar", "primitive": "number" })
    );
    assert_eq!(
        crate::mcp_input_descriptor_value(crate::McpInput::boolean_list()),
        json!({ "supported": true, "shape": "list", "items": "boolean" })
    );
    assert_eq!(
        crate::mcp_input_descriptor_value(crate::McpInput::object()),
        json!({ "supported": true, "shape": "object" })
    );

    static INCLUSIVE_PARAMS: &[crate::McpValidationParam] = &[
        crate::McpValidationParam::literal("min", "1"),
        crate::McpValidationParam::literal("max", "2.5"),
        crate::McpValidationParam::literal("exclusive_min", "not-a-bool"),
        crate::McpValidationParam::literal("exclusive_max", "true"),
    ];
    let range = crate::McpValidationRule::new(
        crate::McpValidationScope::Field,
        "RangeValidation",
        "RangeValidation",
        None,
        crate::McpValidationTypeArgMode::None,
        INCLUSIVE_PARAMS,
    );
    let unknown = crate::McpValidationRule::new(
        crate::McpValidationScope::Field,
        "CustomValidation",
        "CustomValidation",
        None,
        crate::McpValidationTypeArgMode::None,
        crate::MCP_VALIDATION_PARAMS_NONE,
    );
    let mut schema = json!({ "type": "integer" });
    crate::apply_validation_schema_metadata(
        schema.as_object_mut().expect("schema object"),
        "x-rules",
        &[range, unknown],
    );
    assert_eq!(schema["minimum"], 1);
    assert_eq!(schema["exclusiveMaximum"], 2.5);
    assert_eq!(schema["x-rules"].as_array().map(Vec::len), Some(2));

    let mut unconstrained = serde_json::Map::new();
    crate::apply_validation_schema_metadata(&mut unconstrained, "x-rules", &[]);
    assert!(unconstrained.is_empty());

    assert!(crate::schema_allows_null(&crate::McpSchema::new(json!(
        true
    ))));
    assert!(!crate::schema_allows_null(&crate::McpSchema::new(json!(
        false
    ))));
    assert!(!crate::schema_allows_null(&crate::McpSchema::new(json!(
        "invalid"
    ))));
    assert!(!crate::schema_allows_null(&crate::McpSchema::new(
        json!({ "type": true })
    )));
}

#[test]
fn definition_validation_covers_icons_prompt_arguments_and_resource_specs() {
    let mut tool = crate::tool_definition(
        "validated",
        None,
        Some("Description".to_string()),
        crate::McpSchema::new(json!({ "type": ["null", "object"] })),
        None,
    )
    .expect("object type union should be accepted");
    tool.annotations = Some(crate::McpToolAnnotations::with_title("Annotation").read_only(true));
    tool.icons = Some(vec![
        crate::McpIcon::new("data:image/svg+xml,x")
            .with_mime_type("image/svg+xml")
            .with_sizes(vec!["any".to_string()]),
    ]);
    crate::validate_tool_definition(&tool).expect("complete definition should validate");

    let mut invalid = tool.clone();
    invalid.description = Some(" ".into());
    assert!(crate::validate_tool_definition(&invalid).is_err());
    let mut invalid = tool.clone();
    invalid.annotations = Some(crate::McpToolAnnotations::with_title(" "));
    assert!(crate::validate_tool_definition(&invalid).is_err());
    for icon in [
        crate::McpIcon::new(" "),
        crate::McpIcon::new("icon").with_mime_type(" "),
        crate::McpIcon::new("icon").with_sizes(vec![" ".to_string()]),
    ] {
        let mut invalid = tool.clone();
        invalid.icons = Some(vec![icon]);
        assert!(crate::validate_tool_definition(&invalid).is_err());
    }

    let arguments = vec![
        crate::McpPromptArgument::new("topic")
            .with_title("Topic")
            .with_description("Prompt topic"),
    ];
    crate::prompt_definition(
        "draft",
        Some("Draft".to_string()),
        Some("Draft content".to_string()),
        Some(arguments),
    )
    .expect("prompt arguments should validate");
    for argument in [
        crate::McpPromptArgument::new("bad name"),
        crate::McpPromptArgument::new("topic").with_title(" "),
        crate::McpPromptArgument::new("topic").with_description(" "),
    ] {
        assert!(crate::prompt_definition("draft", None, None, Some(vec![argument])).is_err());
    }

    let spec = crate::McpJsonResourceSpec::new(
        "shape://definition",
        "definition",
        None,
        None,
        json!({ "ok": true }),
    )
    .expect("spec should build");
    assert_eq!(spec.definition().uri, "shape://definition");
    assert_eq!(spec.clone().into_definition().uri, "shape://definition");

    let mut server = crate::McpServer::new("resources", "0.0.0");
    crate::register_json_resource_specs(&mut server, vec![spec.clone()])
        .expect("resource should register");
    assert!(crate::ensure_json_resource_specs_available(&server, &[spec]).is_err());

    crate::resource_definition(
        "shape://resource",
        "resource",
        Some("Resource".to_string()),
        Some("Resource description".to_string()),
        Some("application/json".to_string()),
    )
    .expect("complete resource definition should validate");
    crate::resource_template_definition(
        "shape://resource/{id}",
        "resource_template",
        Some("Resource template".to_string()),
        Some("Resource template description".to_string()),
        Some("application/json".to_string()),
    )
    .expect("complete resource template should validate");

    assert!(crate::tool_definition("missing_type", None, None, schema(json!({})), None,).is_err());
    assert!(
        crate::tool_definition(
            "invalid_type_union",
            None,
            None,
            schema(json!({ "type": [1, "null"] })),
            None,
        )
        .is_err()
    );
}

#[test]
fn runtime_icon_metadata_and_untyped_validation_rules_are_exercised() {
    static LEN_PARAMS: &[crate::McpValidationParam] =
        &[crate::McpValidationParam::literal("min", "1")];
    let icon = std::hint::black_box(
        crate::McpToolIcon::new("data:image/svg+xml,x")
            .with_mime_type("image/svg+xml")
            .with_sizes(&["48x48", "any"])
            .with_theme(crate::McpIconTheme::Dark),
    );
    assert_eq!(icon.src(), "data:image/svg+xml,x");
    assert_eq!(icon.mime_type(), Some("image/svg+xml"));
    assert_eq!(icon.sizes(), &["48x48", "any"]);
    assert_eq!(icon.theme(), Some(crate::McpIconTheme::Dark));

    let len = crate::McpValidationRule::new(
        crate::McpValidationScope::Field,
        "LenValidation",
        "LenValidation",
        None,
        crate::McpValidationTypeArgMode::None,
        LEN_PARAMS,
    );
    let mut schema = serde_json::Map::new();
    crate::apply_validation_schema_hints(&mut schema, &[len]);
    assert!(schema.is_empty());
}
