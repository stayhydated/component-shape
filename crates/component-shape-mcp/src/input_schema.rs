use super::*;

pub fn schema_for_input(input: McpInput) -> McpSchema {
    match input.input_shape() {
        McpInputShape::Unsupported => McpSchema::new(json!({ "not": {} })),
        McpInputShape::Scalar(kind) => schema_for_primitive(kind),
        McpInputShape::List(kind) => array_schema(schema_for_primitive(kind)),
        McpInputShape::Set(kind) => unique_array_schema(schema_for_primitive(kind)),
        McpInputShape::Range(kind) => range_schema(schema_for_range_bound(kind)),
        McpInputShape::Object => McpSchema::new(json!({ "type": "object" })),
    }
}

/// Build a compact JSON descriptor for component-shape MCP input metadata.
///
/// Descriptor resources use this alongside full JSON Schema so clients can
/// display the intended component/control shape without reverse-engineering
/// schema details.
pub fn mcp_input_descriptor_value(input: McpInput) -> Value {
    match input.input_shape() {
        McpInputShape::Unsupported => json!({
            "supported": false,
            "shape": "unsupported",
        }),
        McpInputShape::Scalar(kind) => json!({
            "supported": true,
            "shape": "scalar",
            "primitive": kind.as_str(),
        }),
        McpInputShape::List(kind) => json!({
            "supported": true,
            "shape": "list",
            "items": kind.as_str(),
        }),
        McpInputShape::Set(kind) => json!({
            "supported": true,
            "shape": "set",
            "items": kind.as_str(),
        }),
        McpInputShape::Range(kind) => json!({
            "supported": true,
            "shape": "range",
            "bound": kind.as_str(),
        }),
        McpInputShape::Object => json!({
            "supported": true,
            "shape": "object",
        }),
    }
}

/// Attach generated validation metadata and supported JSON Schema hints.
pub fn apply_validation_schema_metadata(
    object: &mut Map<String, Value>,
    extension_key: &str,
    rules: &[McpValidationRule],
) {
    if !rules.is_empty() {
        object.insert(
            extension_key.to_string(),
            Value::Array(rules.iter().map(|rule| rule.to_value()).collect()),
        );
    }
    apply_validation_schema_hints(object, rules);
}

/// Reflect supported literal validation parameters into a JSON Schema object.
pub fn apply_validation_schema_hints(object: &mut Map<String, Value>, rules: &[McpValidationRule]) {
    for rule in rules {
        match rule.validator() {
            "LenValidation" => apply_len_validation_schema_hint(*rule, object),
            "RangeValidation" => apply_range_validation_schema_hint(*rule, object),
            "NonEmptyValidation" => apply_non_empty_validation_schema_hint(object),
            _ => {},
        }
    }
}

fn apply_len_validation_schema_hint(rule: McpValidationRule, object: &mut Map<String, Value>) {
    let Some(schema_type) = primary_schema_type(object) else {
        return;
    };
    let (min_keyword, max_keyword) = match schema_type {
        "string" => ("minLength", "maxLength"),
        "array" => ("minItems", "maxItems"),
        _ => return,
    };
    if let Some(min) = rule
        .params()
        .iter()
        .find(|param| param.name() == "min")
        .and_then(|param| param.literal_value())
        .and_then(literal_to_u64)
    {
        object.insert(min_keyword.to_string(), Value::Number(min.into()));
    }
    if let Some(max) = rule
        .params()
        .iter()
        .find(|param| param.name() == "max")
        .and_then(|param| param.literal_value())
        .and_then(literal_to_u64)
    {
        object.insert(max_keyword.to_string(), Value::Number(max.into()));
    }
}

fn apply_range_validation_schema_hint(rule: McpValidationRule, object: &mut Map<String, Value>) {
    if !matches!(primary_schema_type(object), Some("integer" | "number")) {
        return;
    }
    let exclusive_min = rule
        .params()
        .iter()
        .find(|param| param.name() == "exclusive_min")
        .and_then(|param| param.literal_value())
        .and_then(literal_to_bool)
        .unwrap_or(false);
    let exclusive_max = rule
        .params()
        .iter()
        .find(|param| param.name() == "exclusive_max")
        .and_then(|param| param.literal_value())
        .and_then(literal_to_bool)
        .unwrap_or(false);
    if let Some(min) = rule
        .params()
        .iter()
        .find(|param| param.name() == "min")
        .and_then(|param| param.literal_value())
        .and_then(literal_to_number_value)
    {
        let keyword = if exclusive_min {
            "exclusiveMinimum"
        } else {
            "minimum"
        };
        object.insert(keyword.to_string(), min);
    }
    if let Some(max) = rule
        .params()
        .iter()
        .find(|param| param.name() == "max")
        .and_then(|param| param.literal_value())
        .and_then(literal_to_number_value)
    {
        let keyword = if exclusive_max {
            "exclusiveMaximum"
        } else {
            "maximum"
        };
        object.insert(keyword.to_string(), max);
    }
}

fn apply_non_empty_validation_schema_hint(object: &mut Map<String, Value>) {
    match primary_schema_type(object) {
        Some("string") => {
            object
                .entry("minLength")
                .or_insert(Value::Number(1_u64.into()));
        },
        Some("array") => {
            object
                .entry("minItems")
                .or_insert(Value::Number(1_u64.into()));
        },
        _ => {},
    }
}

fn primary_schema_type(object: &Map<String, Value>) -> Option<&str> {
    object.get("type").and_then(Value::as_str).or_else(|| {
        object
            .get("anyOf")
            .and_then(Value::as_array)
            .and_then(|schemas| schemas.iter().find_map(schema_type_from_value))
    })
}

fn schema_type_from_value(value: &Value) -> Option<&str> {
    match value {
        Value::Object(object) => object.get("type").and_then(Value::as_str),
        _ => None,
    }
}

fn literal_to_u64(literal: &str) -> Option<u64> {
    literal.parse::<u64>().ok()
}

fn literal_to_bool(literal: &str) -> Option<bool> {
    match literal {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn literal_to_number_value(literal: &str) -> Option<Value> {
    if let Ok(value) = literal.parse::<i64>() {
        return Some(Value::Number(value.into()));
    }
    if let Ok(value) = literal.parse::<u64>() {
        return Some(Value::Number(value.into()));
    }
    literal
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .map(Value::Number)
}

pub fn range_schema(bound_schema: McpSchema) -> McpSchema {
    let min_schema = nullable_schema(bound_schema.clone()).into_value();
    let max_schema = nullable_schema(bound_schema).into_value();
    McpSchema::new(json!({
        "type": "object",
        "properties": {
            "min": min_schema,
            "max": max_schema
        },
        "additionalProperties": false
    }))
}

pub fn schema_for_primitive(kind: McpPrimitiveKind) -> McpSchema {
    let schema = match kind {
        McpPrimitiveKind::Any => json!({}),
        McpPrimitiveKind::Boolean => json!({ "type": "boolean" }),
        McpPrimitiveKind::Integer => json!({ "type": "integer" }),
        McpPrimitiveKind::Number => json!({ "type": "number" }),
        McpPrimitiveKind::Decimal => json!({
            "anyOf": [
                { "type": "number" },
                { "type": "string" }
            ]
        }),
        McpPrimitiveKind::String => json!({ "type": "string" }),
        McpPrimitiveKind::Date => json!({ "type": "string", "format": "date" }),
        McpPrimitiveKind::DateTime => json!({ "type": "string", "format": "date-time" }),
    };
    McpSchema::new(schema)
}

pub fn schema_for_range_bound(kind: McpRangeBoundKind) -> McpSchema {
    schema_for_primitive(kind.primitive_kind())
}

pub fn nullable_schema(schema: McpSchema) -> McpSchema {
    let schema = schema.into_value();
    McpSchema::new(json!({
        "anyOf": [
            schema,
            { "type": "null" }
        ]
    }))
}

pub fn schema_allows_null(schema: &McpSchema) -> bool {
    value_schema_allows_null(schema.as_value())
}

pub(crate) fn value_schema_allows_null(schema: &Value) -> bool {
    match schema {
        Value::Bool(value) => *value,
        Value::Object(object) if object.is_empty() => true,
        Value::Object(object) => {
            object.get("type").is_some_and(type_allows_null)
                || object.get("const").is_some_and(Value::is_null)
                || object
                    .get("enum")
                    .and_then(Value::as_array)
                    .is_some_and(|values| values.iter().any(Value::is_null))
                || object
                    .get("anyOf")
                    .and_then(Value::as_array)
                    .is_some_and(|schemas| schemas.iter().any(value_schema_allows_null))
                || object
                    .get("oneOf")
                    .and_then(Value::as_array)
                    .is_some_and(|schemas| schemas.iter().any(value_schema_allows_null))
                || object
                    .get("allOf")
                    .and_then(Value::as_array)
                    .is_some_and(|schemas| schemas.iter().all(value_schema_allows_null))
        },
        _ => false,
    }
}

fn type_allows_null(value: &Value) -> bool {
    match value {
        Value::String(value) => value == "null",
        Value::Array(values) => values
            .iter()
            .any(|value| matches!(value, Value::String(value) if value == "null")),
        _ => false,
    }
}
