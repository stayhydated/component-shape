use super::*;

#[test]
fn schema_description_helpers_only_mutate_object_schemas() {
    let described = schema(json!({ "type": "string" })).with_description("Search text");

    assert_eq!(described["description"], "Search text");

    let boolean_schema = schema(json!(true)).with_description("Ignored");

    assert_eq!(boolean_schema.as_value(), &json!(true));
}

#[test]
fn schema_builders_emit_common_json_schema_keywords() {
    let schema = crate::McpSchema::object()
        .with_properties(crate::McpSchemaProperties::from([
            (
                "state".to_string(),
                crate::McpSchema::string().with_enum_values(["open", "closed"]),
            ),
            (
                "limit".to_string(),
                crate::McpSchema::integer()
                    .with_minimum(0_u64)
                    .with_default(25_u64),
            ),
            (
                "tags".to_string(),
                crate::McpSchema::array(crate::McpSchema::string()).with_unique_items(true),
            ),
        ]))
        .with_required(["state"])
        .with_additional_properties(false);

    assert_eq!(schema["type"], "object");
    assert_eq!(
        schema["properties"]["state"]["enum"],
        json!(["open", "closed"])
    );
    assert_eq!(schema["properties"]["limit"]["minimum"], 0);
    assert_eq!(schema["properties"]["limit"]["default"], 25);
    assert_eq!(schema["properties"]["tags"]["items"]["type"], "string");
    assert_eq!(schema["properties"]["tags"]["uniqueItems"], true);
    assert_eq!(schema["required"], json!(["state"]));
    assert_eq!(schema["additionalProperties"], false);
}

#[test]
fn schema_for_input_maps_range_dates() {
    let schema = crate::schema_for_input(McpInput::date_range());

    assert_eq!(schema["properties"]["min"]["anyOf"][0]["format"], "date");
    assert_eq!(schema["properties"]["max"]["anyOf"][1]["type"], "null");
}

#[test]
fn schema_for_input_distinguishes_any_from_unsupported() {
    assert_eq!(
        crate::schema_for_input(McpInput::any()).as_value(),
        &json!({})
    );
    assert_eq!(
        crate::schema_for_input(McpInput::unsupported()).as_value(),
        &json!({ "not": {} })
    );
}

#[test]
fn schema_for_input_distinguishes_lists_from_sets() {
    let list_schema = crate::schema_for_input(McpInput::string_list());
    let set_schema = crate::schema_for_input(McpInput::string_set());

    assert_eq!(list_schema["type"], "array");
    assert!(list_schema["uniqueItems"].is_null());
    assert_eq!(set_schema["type"], "array");
    assert_eq!(set_schema["uniqueItems"], true);
}

#[test]
fn mcp_input_descriptor_value_describes_supported_shapes() {
    assert_eq!(
        crate::mcp_input_descriptor_value(McpInput::unsupported()),
        json!({
            "supported": false,
            "shape": "unsupported",
        })
    );
    assert_eq!(
        crate::mcp_input_descriptor_value(McpInput::string_set()),
        json!({
            "supported": true,
            "shape": "set",
            "items": "string",
        })
    );
    assert_eq!(
        crate::mcp_input_descriptor_value(McpInput::date_range()),
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
        <UserId as crate::McpJsonSchema>::json_schema()["type"],
        "integer"
    );
    assert_eq!(
        <Option<Vec<String>> as crate::McpJsonSchema>::json_schema()["anyOf"][0]["items"]["type"],
        "string"
    );
    assert_eq!(
        <std::collections::HashSet<String> as crate::McpJsonSchema>::json_schema()["uniqueItems"],
        true
    );
    assert_eq!(
        <std::collections::BTreeMap<String, u32> as crate::McpJsonSchema>::json_schema()["additionalProperties"]
            ["type"],
        "integer"
    );
    assert_eq!(
        <serde_json::Map<String, u32> as crate::McpJsonSchema>::json_schema()["additionalProperties"]
            ["type"],
        "integer"
    );
    assert_eq!(
        <McpAny as crate::McpJsonSchema>::json_schema().as_value(),
        &json!({})
    );
    assert_eq!(
        <serde_json::Value as crate::McpJsonSchema>::json_schema().as_value(),
        &json!({})
    );
    assert_eq!(
        <&str as crate::McpJsonSchema>::json_schema()["type"],
        "string"
    );
    assert_eq!(
        <std::borrow::Cow<'static, str> as crate::McpJsonSchema>::json_schema()["type"],
        "string"
    );
    assert_eq!(
        <[String] as crate::McpJsonSchema>::json_schema()["items"]["type"],
        "string"
    );
    assert_eq!(
        <McpRange<u32> as crate::McpJsonSchema>::json_schema()["properties"]["min"]["anyOf"][0]["type"],
        "integer"
    );
    assert_eq!(
        <(u32, String) as crate::McpJsonSchema>::json_schema()["prefixItems"][0]["type"],
        "integer"
    );
    assert_eq!(
        <(u32, String) as crate::McpJsonSchema>::json_schema()["prefixItems"][1]["type"],
        "string"
    );
    assert_eq!(
        <(u32, String) as crate::McpJsonSchema>::json_schema()["minItems"],
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
fn schema_allows_null_matches_common_schema_forms() {
    assert!(crate::schema_allows_null(&schema(json!({}))));
    assert!(crate::schema_allows_null(&schema(json!({
        "anyOf": [
            { "type": "string" },
            { "type": "null" }
        ]
    }))));
    assert!(crate::schema_allows_null(&schema(json!({
        "type": ["string", "null"]
    }))));
    assert!(!crate::schema_allows_null(&schema(json!({
        "type": "string"
    }))));
}

#[test]
fn schema_object_rejects_non_object_values() {
    let object = crate::schema_object("input_schema", schema(json!({ "type": "object" })))
        .expect("schema should be accepted");
    assert_eq!(object["type"], "object");

    let error = crate::schema_object("input_schema", schema(json!(null)))
        .expect_err("schema should be rejected");
    assert!(error.to_string().contains("input_schema"));
}

#[test]
fn schema_conversions_and_remaining_json_schema_impls_are_exercised() {
    use std::{
        borrow::Cow,
        collections::{BTreeMap, BTreeSet, HashMap},
        path::{Path, PathBuf},
    };

    assert_eq!(crate::McpSchemaType::Number.as_str(), "number");
    assert_eq!(crate::McpStringFormat::DateTime.as_str(), "date-time");
    assert_eq!(
        crate::McpSchemaNumber::from_f64(1.5)
            .expect("finite number should convert")
            .into_value(),
        json!(1.5)
    );
    assert!(crate::McpSchemaNumber::from_f64(f64::NAN).is_none());
    assert_eq!(
        crate::McpSchemaNumber::from(-2_isize).into_value(),
        json!(-2)
    );
    assert_eq!(crate::McpSchemaNumber::from(2_usize).into_value(), json!(2));

    let schema = crate::McpSchema::one_of([crate::McpSchema::string(), crate::McpSchema::number()])
        .with_extension("x-test", json!(true))
        .with_const("fixed");
    assert_eq!(schema["oneOf"].as_array().map(Vec::len), Some(2));
    assert_eq!(schema["x-test"], true);
    assert_eq!(schema["const"], "fixed");

    let any = crate::McpAny::from(json!({ "value": 1 }));
    assert_eq!(any.as_value()["value"], 1);
    assert_eq!((*any)["value"], 1);
    let value: Value = any.into();
    assert_eq!(value["value"], 1);

    let range = crate::McpRange::from((Some(1_i32), Some(3_i32)));
    let tuple: (Option<i32>, Option<i32>) = range.into();
    assert_eq!(tuple, (Some(1), Some(3)));

    let schemas = [
        <f32 as crate::McpJsonSchema>::json_schema(),
        <f64 as crate::McpJsonSchema>::json_schema(),
        <rust_decimal::Decimal as crate::McpJsonSchema>::json_schema(),
        <char as crate::McpJsonSchema>::json_schema(),
        <PathBuf as crate::McpJsonSchema>::json_schema(),
        <Path as crate::McpJsonSchema>::json_schema(),
        <chrono::NaiveDate as crate::McpJsonSchema>::json_schema(),
        <chrono::NaiveDateTime as crate::McpJsonSchema>::json_schema(),
        <chrono::DateTime<chrono::Utc> as crate::McpJsonSchema>::json_schema(),
        <[String; 2] as crate::McpJsonSchema>::json_schema(),
        <BTreeSet<String> as crate::McpJsonSchema>::json_schema(),
        <HashMap<String, i32> as crate::McpJsonSchema>::json_schema(),
        <BTreeMap<String, i32> as crate::McpJsonSchema>::json_schema(),
        <Box<String> as crate::McpJsonSchema>::json_schema(),
        <() as crate::McpJsonSchema>::json_schema(),
        <&str as crate::McpJsonSchema>::json_schema(),
        <Cow<'static, str> as crate::McpJsonSchema>::json_schema(),
    ];
    assert_eq!(schemas[0]["type"], "number");
    assert_eq!(schemas[3]["type"], "string");
    assert_eq!(schemas[8]["format"], "date-time");
    assert_eq!(schemas[9]["minItems"], 2);
    assert_eq!(schemas[10]["uniqueItems"], true);
    assert_eq!(schemas[11]["additionalProperties"]["type"], "integer");
    assert_eq!(schemas[14]["type"], "null");
}

#[test]
fn schema_normalization_covers_composites_aliases_and_collection_items() {
    let enum_alias = json!({
        "type": "string",
        "enum": ["ready"],
        "x-mcpEnumDecodeAliases": { "prepared": "ready" }
    });
    let any_of = json!({
        "anyOf": [
            { "type": "integer" },
            enum_alias
        ]
    });
    assert_eq!(
        crate::normalize_value_against_schema(&any_of, json!("prepared")),
        json!("ready")
    );
    assert_eq!(
        crate::normalize_value_against_schema(
            &json!({ "oneOf": [false, enum_alias] }),
            json!("prepared")
        ),
        json!("ready")
    );

    let object_schema = json!({
        "type": "object",
        "properties": {
            "wire": {
                "type": "string",
                "x-mcpAliases": ["alias"],
                "x-mcpDecodeName": "rust_name",
                "x-mcpEnumDecodeAliases": { "prepared": "ready" }
            }
        },
        "additionalProperties": enum_alias
    });
    assert_eq!(
        crate::normalize_value_against_schema(
            &object_schema,
            json!({ "alias": "prepared", "extra": "prepared" })
        ),
        json!({ "rust_name": "ready", "extra": "ready" })
    );

    let all_of = json!({ "allOf": [true, object_schema] });
    assert_eq!(
        crate::normalize_value_against_schema(&all_of, json!({ "wire": "prepared" })),
        json!({ "rust_name": "ready" })
    );
    assert_eq!(
        crate::normalize_value_against_schema(&json!({ "items": enum_alias }), json!(["prepared"])),
        json!(["ready"])
    );
    assert_eq!(
        crate::normalize_value_against_schema(
            &json!({ "prefixItems": [enum_alias] }),
            json!(["prepared", "unchanged"])
        ),
        json!(["ready", "unchanged"])
    );

    for value in [
        json!(null),
        json!(true),
        json!(1),
        json!(1.5),
        json!("ready"),
        json!([]),
    ] {
        let schema = json!({ "anyOf": [true] });
        assert_eq!(
            crate::normalize_value_against_schema(&schema, value.clone()),
            value
        );
    }
}
