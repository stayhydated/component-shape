use super::*;

#[test]
fn output_schema_checks_json_schema_assertions() {
    let cases = [
        ("string", json!({"type": "string"}), json!("ok"), json!(42)),
        (
            "nonnull",
            json!({"type": "string"}),
            json!("ok"),
            Value::Null,
        ),
        (
            "boolean",
            json!({"type": "boolean"}),
            json!(false),
            json!("false"),
        ),
        (
            "integer",
            json!({"type": "integer"}),
            json!(1.0),
            json!(1.5),
        ),
        (
            "number",
            json!({"type": "number"}),
            json!(1.5),
            json!("1.5"),
        ),
        ("null", json!({"type": "null"}), Value::Null, json!(0)),
        (
            "nullable",
            json!({"type": ["integer", "null"]}),
            Value::Null,
            json!("no"),
        ),
        (
            "enum",
            json!({"enum": ["one", "two"]}),
            json!("one"),
            json!("three"),
        ),
        (
            "const",
            json!({"const": {"version": 2}}),
            json!({"version": 2}),
            json!({"version": 3}),
        ),
        ("minimum", json!({"minimum": 2}), json!(2), json!(1)),
        ("maximum", json!({"maximum": 2}), json!(2), json!(3)),
        (
            "exclusive",
            json!({"exclusiveMinimum": 0, "exclusiveMaximum": 2}),
            json!(1),
            json!(2),
        ),
        ("multiple", json!({"multipleOf": 2}), json!(4), json!(3)),
        (
            "length",
            json!({"minLength": 2, "maxLength": 3}),
            json!("éé"),
            json!("é"),
        ),
        (
            "pattern",
            json!({"pattern": "^ok$"}),
            json!("ok"),
            json!("other"),
        ),
        (
            "items",
            json!({"type": "array", "items": {"type": "integer"}}),
            json!([1, 2]),
            json!([1, "bad"]),
        ),
        (
            "item_count",
            json!({"minItems": 1, "maxItems": 2}),
            json!([1]),
            json!([]),
        ),
        (
            "unique",
            json!({"uniqueItems": true}),
            json!([1, 2]),
            json!([1, 1]),
        ),
        (
            "tuple",
            json!({"prefixItems": [{"type": "integer"}, {"type": "string"}], "items": false}),
            json!([1, "ok"]),
            json!([1, "ok", 3]),
        ),
        (
            "required",
            json!({"type": "object", "required": ["value"]}),
            json!({"value": 1}),
            json!({}),
        ),
        (
            "property",
            json!({"properties": {"value": {"type": "integer"}}}),
            json!({"value": 1}),
            json!({"value": "bad"}),
        ),
        (
            "closed",
            json!({"properties": {"value": {}}, "additionalProperties": false}),
            json!({"value": 1}),
            json!({"extra": 1}),
        ),
        (
            "map",
            json!({"additionalProperties": {"type": "integer"}}),
            json!({"a": 1}),
            json!({"a": "bad"}),
        ),
        (
            "any_of",
            json!({"anyOf": [{"type": "integer"}, {"type": "null"}]}),
            json!(1),
            json!(true),
        ),
        (
            "one_of",
            json!({"oneOf": [{"type": "integer"}, {"type": "number"}]}),
            json!(1.5),
            json!(1),
        ),
        (
            "all_of",
            json!({"allOf": [{"minimum": 1}, {"maximum": 3}]}),
            json!(2),
            json!(4),
        ),
        (
            "not",
            json!({"not": {"type": "string"}}),
            json!(1),
            json!("bad"),
        ),
        (
            "conditional",
            json!({"if": {"type": "integer"}, "then": {"minimum": 1}, "else": {"const": null}}),
            json!(1),
            json!(0),
        ),
        (
            "local_ref",
            json!({"$defs": {"value": {"type": "string"}}, "$ref": "#/$defs/value"}),
            json!("ok"),
            json!(false),
        ),
        (
            "false_schema",
            json!({"properties": {"forbidden": false}}),
            json!({}),
            json!({"forbidden": null}),
        ),
    ];

    for (name, output_schema, valid, invalid) in cases {
        let mut server = McpServer::new("schema-tests", "0");
        let definition = crate::tool_definition(
            name,
            None,
            None,
            crate::McpSchema::object(),
            Some(schema(output_schema)),
        )
        .unwrap();
        server
            .add_tool(definition, |call| {
                let value = call.into_arguments().into_inner().remove("value").unwrap();
                crate::tool_structured_result(value)
            })
            .unwrap();
        let result = server.call_tool(name, Some(json!({"value": valid})));
        assert_eq!(result.is_error, Some(false), "valid {name}: {result:?}");
        let result = server.call_tool(name, Some(json!({"value": invalid})));
        assert_eq!(result.is_error, Some(true), "invalid {name}: {result:?}");
        let error = &result.structured_content.as_ref().unwrap()["error"];
        assert_eq!(error["kind"], "invalid_tool_output");
        assert_eq!(error["name"], name);
    }
}

#[test]
fn asynchronous_output_validation_matches_synchronous_calls() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let mut registry = crate::McpToolRegistry::new();
    let definition = crate::tool_definition(
        "invalid",
        None,
        None,
        crate::McpSchema::object(),
        Some(crate::McpSchema::string()),
    )
    .unwrap();
    registry
        .add_tool_async(definition, |_| async {
            crate::tool_structured_result(json!(42))
        })
        .unwrap();
    let cloned = registry.clone();
    let direct = registry.call_tool("invalid", None);
    let asynchronous = runtime.block_on(cloned.call_tool_async("invalid", None));
    assert_eq!(direct.is_error, Some(true));
    assert_eq!(direct.structured_content, asynchronous.structured_content);
}

fn definition_with_output(output: Value) -> Result<ToolDefinition, crate::McpToolError> {
    crate::tool_definition(
        "output",
        None,
        None,
        crate::McpSchema::object(),
        Some(schema(output)),
    )
}

fn assert_invalid_schema(error: crate::McpToolError) -> String {
    let crate::McpToolError::InvalidSchema { label, message } = error else {
        panic!("expected InvalidSchema, got {error:?}");
    };
    assert_eq!(label, "output_schema");
    message
}

#[test]
fn malformed_output_schemas_fail_before_registration() {
    for output in [
        json!({"type": "not-a-type"}),
        json!({"properties": {"value": 42}}),
        json!({"required": "value"}),
        json!({"minimum": "zero"}),
        json!({"pattern": "["}),
        json!({"$schema": 42}),
        json!({"$schema": "https://example.invalid/unknown-dialect"}),
        json!({"$ref": "#/$defs/missing"}),
    ] {
        assert_invalid_schema(definition_with_output(output.clone()).unwrap_err());
        // Definitions can also be assembled directly through rmcp's public model.
        let mut definition = definition_with_output(json!({})).unwrap();
        definition.output_schema = Some(std::sync::Arc::new(output.as_object().unwrap().clone()));
        assert_invalid_schema(crate::validate_tool_definition(&definition).unwrap_err());
        let mut registry = crate::McpToolRegistry::new();
        assert_invalid_schema(
            registry
                .add_tool(definition.clone(), |_| unreachable!())
                .unwrap_err(),
        );
        assert_invalid_schema(
            registry
                .add_tool_async(definition, |_| async { unreachable!() })
                .unwrap_err(),
        );
        assert_eq!(registry.tool_count(), 0);
    }
}

#[test]
fn output_schemas_honor_known_dialects_and_default_to_2020_12() {
    for dialect in [
        "http://json-schema.org/draft-04/schema#",
        "http://json-schema.org/draft-06/schema#",
        "http://json-schema.org/draft-07/schema#",
        "https://json-schema.org/draft/2019-09/schema",
        "https://json-schema.org/draft/2020-12/schema",
    ] {
        definition_with_output(json!({"$schema": dialect, "type": "string"})).unwrap();
    }

    let mut registry = crate::McpToolRegistry::new();
    // Draft 7 permits an array-valued items keyword, which Draft 2020-12 rejects.
    let draft7 = json!({"$schema": "http://json-schema.org/draft-07/schema#", "type": "array", "items": [{"type": "string"}], "additionalItems": false});
    registry
        .add_tool(definition_with_output(draft7).unwrap(), |_| {
            crate::tool_structured_result(json!(["ok", "extra"]))
        })
        .unwrap();
    assert_eq!(registry.call_tool("output", None).is_error, Some(true));
    assert_invalid_schema(
        definition_with_output(json!({"items": [{"type": "string"}]})).unwrap_err(),
    );
    // The default dialect honors siblings of $ref, unlike Draft 7.
    let default =
        json!({"$defs": {"number": {"type": "integer"}}, "$ref": "#/$defs/number", "maximum": 1});
    let mut registry = crate::McpToolRegistry::new();
    registry
        .add_tool(definition_with_output(default).unwrap(), |_| {
            crate::tool_structured_result(json!(2))
        })
        .unwrap();
    assert_eq!(registry.call_tool("output", None).is_error, Some(true));
}

#[test]
fn output_schemas_resolve_bundled_identifiers_and_recursive_references() {
    let output = json!({
        "$id": "https://example.invalid/root",
        "$defs": {"node": {
            "$id": "node",
            "type": "object",
            "properties": {"value": {"type": "integer"}, "next": {"$ref": "node"}},
            "required": ["value"],
            "additionalProperties": false
        }},
        "$ref": "node"
    });
    let mut registry = crate::McpToolRegistry::new();
    registry
        .add_tool(definition_with_output(output).unwrap(), |call| {
            crate::tool_structured_result(
                call.into_arguments().into_inner().remove("value").unwrap(),
            )
        })
        .unwrap();
    let valid = json!({"value": {"value": 1, "next": {"value": 2}}});
    assert_eq!(
        registry.call_tool("output", Some(valid)).is_error,
        Some(false)
    );
    let invalid = json!({"value": {"value": 1, "next": {"value": "wrong"}}});
    let result = registry.call_tool("output", Some(invalid));
    assert_eq!(result.is_error, Some(true));
    let detail = result.structured_content.as_ref().unwrap()["error"]["detail"]
        .as_str()
        .unwrap();
    assert!(detail.contains("structured_content/next/value"), "{detail}");
    assert!(
        !detail.contains("wrong"),
        "output values should be masked: {detail}"
    );
}

#[test]
fn output_schemas_deny_external_retrieval_including_readable_files() {
    let path = std::env::temp_dir().join(format!(
        "component-shape-schema-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, r#"{"type":"string"}"#).unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        r#"{"type":"string"}"#
    );
    let path_uri = path
        .to_string_lossy()
        .replace('\\', "/")
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect::<String>();
    let file_uri = if path_uri.starts_with('/') {
        format!("file://{path_uri}")
    } else {
        format!("file:///{path_uri}")
    };
    let references = [
        "http://127.0.0.1:9/schema.json".to_owned(),
        "https://example.invalid/schema.json".to_owned(),
        "urn:external:schema".to_owned(),
        file_uri,
    ];
    let errors =
        references.map(|reference| definition_with_output(json!({"$ref": reference})).unwrap_err());
    std::fs::remove_file(path).unwrap();
    for error in errors {
        let message = assert_invalid_schema(error);
        assert!(
            message.contains("external schema retrieval is disabled"),
            "{message}"
        );
    }
}

#[test]
fn output_formats_and_mcp_extensions_are_annotations_not_normalization() {
    for format in ["date", "language-tag"] {
        let output = json!({"type": "string", "format": format});
        let mut registry = crate::McpToolRegistry::new();
        registry
            .add_tool(definition_with_output(output).unwrap(), |_| {
                crate::tool_structured_result(json!("not a date or language tag"))
            })
            .unwrap();
        assert_eq!(registry.call_tool("output", None).is_error, Some(false));
    }
    let output = json!({
        "type": "object", "properties": {"wireName": {"type": "string", "x-mcpAliases": ["alias"], "x-mcpDecodeName": "serde_name"}},
        "required": ["wireName"], "additionalProperties": false
    });
    let mut registry = crate::McpToolRegistry::new();
    registry
        .add_tool(definition_with_output(output).unwrap(), |call| {
            crate::tool_structured_result(Value::Object(call.into_arguments().into_inner()))
        })
        .unwrap();
    let valid = json!({"wireName": "ok"});
    let result = registry.call_tool("output", Some(valid.clone()));
    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.structured_content, Some(valid));
    for invalid in [json!({"alias": "ok"}), json!({"serde_name": "ok"})] {
        assert_eq!(
            registry.call_tool("output", Some(invalid)).is_error,
            Some(true)
        );
    }

    let mut registry = crate::McpToolRegistry::new();
    let output =
        json!({"enum": ["wire"], "x-mcpEnumDecodeAliases": {"wire": "Rust", "alias": "Rust"}});
    registry
        .add_tool(definition_with_output(output).unwrap(), |call| {
            crate::tool_structured_result(
                call.into_arguments().into_inner().remove("value").unwrap(),
            )
        })
        .unwrap();
    let result = registry.call_tool("output", Some(json!({"value": "wire"})));
    assert_eq!(result.structured_content, Some(json!("wire")));
    for value in ["Rust", "alias"] {
        assert_eq!(
            registry
                .call_tool("output", Some(json!({"value": value})))
                .is_error,
            Some(true)
        );
    }
}
