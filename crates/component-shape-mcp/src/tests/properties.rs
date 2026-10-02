use super::*;
use crate::{McpArguments, McpToolError, McpToolRegistry};
use proptest::prelude::*;
use serde_json::Map;

const SPELLINGS: [&str; 3] = ["value", "old_value", "legacy_value"];
const ALIASES: [&str; 2] = ["old_value", "legacy_value"];

// Schema and witnesses shrink together; invalid witnesses remain invalid without
// rejection sampling. Expected outcomes do not call the production validator.
fn output_witness() -> impl Strategy<Value = (Value, Value, Value)> {
    prop_oneof![
        (-1024_i64..1024, 1_u16..64, any::<u8>()).prop_map(|(min, width, offset)| {
            let max = min + i64::from(width);
            let valid = min + i64::from(offset) % (i64::from(width) + 1);
            (
                json!({"type": "object", "properties": {"value": {
                    "type": "integer", "minimum": min, "maximum": max
                }}, "required": ["value"], "additionalProperties": false}),
                json!({"value": valid}),
                json!({"value": max + 1}),
            )
        }),
        (1_usize..16, 0_usize..16).prop_map(|(min, extra)| {
            let max = min + extra;
            (
                json!({"type": "object", "properties": {"value": {
                    "type": "string", "minLength": min, "maxLength": max
                }}, "required": ["value"], "additionalProperties": false}),
                json!({"value": "é".repeat(max)}),
                json!({"value": "é".repeat(min - 1)}),
            )
        }),
        prop::collection::vec(-32_i64..33, 0..8).prop_map(|values| {
            let rows = values
                .into_iter()
                .map(|value| json!({"value": value}))
                .collect::<Vec<_>>();
            let mut invalid_rows = rows.clone();
            invalid_rows.push(json!({"value": "wrong type"}));
            (
                json!({"type": "object", "properties": {"rows": {
                    "type": "array", "items": {
                        "type": "object", "properties": {"value": {"type": "integer"}},
                        "required": ["value"], "additionalProperties": false
                    }
                }}, "required": ["rows"], "additionalProperties": false}),
                json!({"rows": rows}),
                json!({"rows": invalid_rows}),
            )
        }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn accepted_spellings_decode_once_and_leave_unknown_fields(
        mask in 0_u8..8,
        values in prop::array::uniform3(any::<i64>()),
        extra in prop::collection::btree_map("[a-z]{1,8}", any::<bool>(), 0..8),
    ) {
        let extra = extra.into_iter()
            .map(|(key, value)| (format!("extra_{key}"), json!(value)))
            .collect::<Map<_, _>>();
        let mut input = extra.clone();
        for (index, spelling) in SPELLINGS.iter().enumerate() {
            if mask & (1 << index) != 0 {
                input.insert((*spelling).into(), json!(values[index]));
            }
        }
        let mut arguments = McpArguments::new(input);
        let decoded = arguments.take_required_tool_value_from::<i64>(
            SPELLINGS[0], &ALIASES,
        );
        match mask.count_ones() {
            0 => prop_assert_eq!(decoded, Err(McpToolError::missing_field("value"))),
            1 => {
                let index = mask.trailing_zeros() as usize;
                prop_assert_eq!(decoded, Ok(values[index]));
                prop_assert_eq!(arguments.as_inner(), &extra);
                let finished = arguments.finish();
                match extra.keys().next() {
                    None => prop_assert_eq!(finished, Ok(())),
                    Some(field) => prop_assert_eq!(finished, Err(McpToolError::UnknownField {
                        field: field.clone(),
                    })),
                }
            },
            _ => prop_assert_eq!(decoded, Err(McpToolError::DuplicateField {
                field: "value".into(),
            })),
        }
    }

    #[test]
    fn cached_output_validation_preserves_valid_results_and_rejects_invalid_witnesses(
        (output_schema, valid, invalid) in output_witness(),
    ) {
        let mut registry = McpToolRegistry::new();
        let definition = crate::tool_definition(
            "witness", None, None, crate::McpSchema::object(), Some(schema(output_schema)),
        ).unwrap();
        registry.add_tool(definition, |call| {
            crate::tool_structured_result(Value::Object(call.into_arguments().into_inner()))
        }).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let result = runtime.block_on(registry.call_tool_async("witness", Some(valid.clone())));
        prop_assert_eq!(result.is_error, Some(false));
        prop_assert_eq!(result.structured_content, Some(valid));

        let cloned = registry.clone();
        let result = runtime.block_on(cloned.call_tool_async("witness", Some(invalid)));
        prop_assert_eq!(result.is_error, Some(true));
        let error = &result.structured_content.as_ref().unwrap()["error"];
        prop_assert_eq!(&error["kind"], &json!("invalid_tool_output"));
        prop_assert_eq!(&error["name"], &json!("witness"));
    }
}

#[test]
fn aliases_do_not_make_required_integer_null_nullable() {
    for spelling in SPELLINGS {
        let mut arguments = McpArguments::new(Map::from_iter([(spelling.into(), Value::Null)]));
        assert_eq!(
            arguments.take_required_tool_value_from::<i64>("value", &ALIASES),
            Err(McpToolError::UnexpectedNull {
                field: "value".into()
            }),
        );
    }
}
