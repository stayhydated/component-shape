use super::*;

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
        let decoded = <McpAny as crate::McpToolValue>::from_tool_value("payload", raw.clone())
            .expect("McpAny should accept any JSON value");

        assert_eq!(decoded.into_value(), raw);
    }
}

#[test]
fn tool_value_trait_pairs_schema_and_strict_decode() {
    let schema = <McpRange<u32> as crate::McpToolValue>::tool_value_schema();
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["properties"]["min"]["anyOf"][0]["type"], "integer");

    let value = <McpRange<u32> as crate::McpToolValue>::from_tool_value(
        "window",
        json!({ "min": 1, "max": null }),
    )
    .expect("range value should decode");
    assert_eq!(value, McpRange::new(Some(1), None));

    let error = <String as crate::McpToolValue>::from_tool_value("title", Value::Null)
        .expect_err("null should not decode as a present string");
    assert_eq!(
        error,
        crate::McpToolError::UnexpectedNull {
            field: "title".to_string(),
        }
    );

    let value = <Option<String> as crate::McpToolValue>::from_tool_value("title", Value::Null)
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

    let value = <Preferences as crate::McpToolValue>::from_tool_value(
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

    let error = <Preferences as crate::McpToolValue>::from_tool_value(
        "preferences",
        json!({
            "email": true,
            "emailUpdates": false
        }),
    )
    .expect_err("primary and alias should be rejected as duplicate input");
    assert_eq!(
        error,
        crate::McpToolError::DuplicateField {
            field: "preferences.email".to_string(),
        }
    );

    let error = <Preferences as crate::McpToolValue>::from_tool_value(
        "preferences",
        json!({
            "email": true,
            "unexpected": true
        }),
    )
    .expect_err("unknown nested field should be rejected before serde");
    assert_eq!(
        error,
        crate::McpToolError::UnknownField {
            field: "preferences.unexpected".to_string(),
        }
    );

    let error = <Preferences as crate::McpToolValue>::from_tool_value("preferences", json!({}))
        .expect_err("missing required nested field should be rejected before serde");
    assert_eq!(
        error,
        crate::McpToolError::MissingField {
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

    let value = <IssueState as crate::McpToolValue>::from_tool_value("state", json!("reviewing"))
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
        <std::collections::BTreeMap<String, IssueState> as crate::McpToolValue>::from_tool_value(
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
        <std::collections::BTreeMap<String, Preferences> as crate::McpToolValue>::from_tool_value(
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
        crate::McpToolError::UnknownField {
            field: "preferences.team.unexpected".to_string(),
        }
    );

    let error =
        <std::collections::BTreeMap<String, Preferences> as crate::McpToolValue>::from_tool_value(
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
        crate::McpToolError::MissingField {
            field: "preferences.team.email".to_string(),
        }
    );
}

#[test]
fn mcp_tool_call_normalizes_arguments() {
    let call = crate::McpToolCall::from_value(None).expect("missing arguments are empty");
    assert!(call.arguments().is_empty());

    let call = crate::McpToolCall::from_value(Some(json!({ "value": 42 })))
        .expect("object arguments are accepted");
    assert_eq!(call.arguments()["value"], 42);

    let error = crate::McpToolCall::from_value(Some(json!(42)))
        .expect_err("non-object arguments should fail");
    assert_eq!(error, crate::McpToolError::ArgumentsMustBeObject);
}

#[test]
fn mcp_arguments_decodes_tool_values_and_rejects_unknown_fields() {
    let call = crate::McpToolCall::from_value(Some(json!({
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
        Err(crate::McpToolError::UnknownField {
            field: "unused".to_string()
        })
    );
}

#[test]
fn tool_value_usize_rejects_negative_values() {
    assert_eq!(
        <usize as crate::McpToolValue>::from_tool_value("limit", json!(10))
            .expect("usize should decode"),
        10
    );

    let error = <usize as crate::McpToolValue>::from_tool_value("limit", json!(-1))
        .expect_err("negative should fail");
    assert!(matches!(error, crate::McpToolError::DecodeField { field, .. } if field == "limit"));
}

#[test]
fn closed_schema_validation_covers_composites_arrays_and_nested_objects() {
    let object_a = json!({
        "type": "object",
        "required": ["a"],
        "additionalProperties": false,
        "properties": { "a": { "type": "integer" } }
    });
    let object_b = json!({
        "type": ["null", "object"],
        "required": ["b"],
        "additionalProperties": false,
        "properties": { "b": { "type": "integer" } }
    });

    for schema in [
        json!({ "anyOf": [false, object_a, object_b] }),
        json!({ "oneOf": [object_a, object_b] }),
    ] {
        crate::validate_value_against_closed_schema("input", &schema, &json!({ "b": 1 }))
            .expect("one applicable object branch should accept the value");
        assert!(
            crate::validate_value_against_closed_schema("input", &schema, &json!({ "c": 1 }))
                .is_err()
        );
    }

    let all_of = json!({
        "allOf": [
            { "type": "object", "required": ["a"] },
            { "type": "object", "required": ["b"] }
        ]
    });
    crate::validate_value_against_closed_schema("", &all_of, &json!({ "a": 1, "b": 2 }))
        .expect("all object branches should accept the value");

    let array = json!({
        "anyOf": [{ "type": "array" }],
        "items": {
            "type": "object",
            "required": ["value"],
            "additionalProperties": false,
            "properties": { "value": { "type": "string" } }
        },
        "prefixItems": [
            {
                "type": "object",
                "required": ["value"],
                "additionalProperties": false,
                "properties": { "value": { "type": "string" } }
            }
        ]
    });
    crate::validate_value_against_closed_schema("items", &array, &json!([{ "value": "ok" }]))
        .expect("nested array objects should validate");
    assert!(
        crate::validate_value_against_closed_schema("items", &array, &json!([{ "extra": true }]))
            .is_err()
    );

    assert!(crate::validate_value_against_closed_schema("value", &json!(true), &json!(1)).is_ok());
    assert!(crate::validate_value_against_closed_schema("value", &json!({}), &Value::Null).is_ok());
}
