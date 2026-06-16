use super::*;

#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum McpToolError {
    #[error("tool arguments must be a JSON object")]
    ArgumentsMustBeObject,
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("field `{field}` does not accept null")]
    UnexpectedNull { field: String },
    #[error("field `{field}` was provided more than once")]
    DuplicateField { field: String },
    #[error("failed to decode field `{field}`: {message}")]
    DecodeField { field: String, message: String },
    #[error("unknown field `{field}`")]
    UnknownField { field: String },
    #[error("unknown value `{value}` for field `{field}`")]
    InvalidFieldValue { field: String, value: String },
    #[error("validation failed: {message}")]
    Validation {
        message: String,
        details: Vec<Value>,
    },
    #[error("tool `{name}` returned invalid structured content: {message}")]
    InvalidToolOutput { name: String, message: String },
    #[error("conversion failed: {message}")]
    Conversion { message: String },
    #[error("handler failed: {message}")]
    Handler { message: String },
    #[error("invalid {label}: {message}")]
    InvalidSchema { label: String, message: String },
    #[error("tool `{name}` is already registered")]
    DuplicateTool { name: String },
    #[error("unknown tool `{name}`")]
    UnknownTool { name: String },
    #[error("resource `{uri}` is already registered")]
    DuplicateResource { uri: String },
    #[error("unknown resource `{uri}`")]
    UnknownResource { uri: String },
    #[error("prompt `{name}` is already registered")]
    DuplicatePrompt { name: String },
    #[error("unknown prompt `{name}`")]
    UnknownPrompt { name: String },
}

impl McpToolError {
    /// Stable machine-readable error kind used in MCP structured error content.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::ArgumentsMustBeObject => "arguments_must_be_object",
            Self::MissingField { .. } => "missing_field",
            Self::UnexpectedNull { .. } => "unexpected_null",
            Self::DuplicateField { .. } => "duplicate_field",
            Self::DecodeField { .. } => "decode_field",
            Self::UnknownField { .. } => "unknown_field",
            Self::InvalidFieldValue { .. } => "invalid_field_value",
            Self::Validation { .. } => "validation",
            Self::InvalidToolOutput { .. } => "invalid_tool_output",
            Self::Conversion { .. } => "conversion",
            Self::Handler { .. } => "handler",
            Self::InvalidSchema { .. } => "invalid_schema",
            Self::DuplicateTool { .. } => "duplicate_tool",
            Self::UnknownTool { .. } => "unknown_tool",
            Self::DuplicateResource { .. } => "duplicate_resource",
            Self::UnknownResource { .. } => "unknown_resource",
            Self::DuplicatePrompt { .. } => "duplicate_prompt",
            Self::UnknownPrompt { .. } => "unknown_prompt",
        }
    }

    /// Build the `structured_content.error` object for this typed MCP error.
    pub fn to_structured_value(&self) -> Value {
        let mut object = Map::new();
        object.insert("kind".to_string(), json!(self.kind()));
        object.insert("message".to_string(), json!(self.to_string()));

        match self {
            Self::ArgumentsMustBeObject => {},
            Self::MissingField { field }
            | Self::UnexpectedNull { field }
            | Self::DuplicateField { field }
            | Self::UnknownField { field } => {
                object.insert("field".to_string(), json!(field));
            },
            Self::DecodeField { field, message } => {
                object.insert("field".to_string(), json!(field));
                object.insert("detail".to_string(), json!(message));
            },
            Self::InvalidFieldValue { field, value } => {
                object.insert("field".to_string(), json!(field));
                object.insert("value".to_string(), json!(value));
            },
            Self::Validation { message, details } => {
                object.insert("detail".to_string(), json!(message));
                if !details.is_empty() {
                    object.insert("details".to_string(), json!(details));
                }
            },
            Self::InvalidToolOutput { name, message } => {
                object.insert("name".to_string(), json!(name));
                object.insert("detail".to_string(), json!(message));
            },
            Self::Conversion { message } | Self::Handler { message } => {
                object.insert("detail".to_string(), json!(message));
            },
            Self::InvalidSchema { label, message } => {
                object.insert("label".to_string(), json!(label));
                object.insert("detail".to_string(), json!(message));
            },
            Self::DuplicateTool { name } | Self::UnknownTool { name } => {
                object.insert("name".to_string(), json!(name));
            },
            Self::DuplicateResource { uri } | Self::UnknownResource { uri } => {
                object.insert("uri".to_string(), json!(uri));
            },
            Self::DuplicatePrompt { name } | Self::UnknownPrompt { name } => {
                object.insert("name".to_string(), json!(name));
            },
        }

        Value::Object(object)
    }

    pub fn missing_field(field: impl Into<String>) -> Self {
        Self::MissingField {
            field: field.into(),
        }
    }

    pub fn decode(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::DecodeField {
            field: field.into(),
            message: message.into(),
        }
    }

    pub fn invalid_field_value(field: impl Into<String>, value: impl Into<String>) -> Self {
        Self::InvalidFieldValue {
            field: field.into(),
            value: value.into(),
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
            details: Vec::new(),
        }
    }

    pub fn validation_details(details: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let details = details.into_iter().map(Into::into).collect::<Vec<_>>();
        Self::Validation {
            message: details.join("; "),
            details: details.into_iter().map(Value::String).collect(),
        }
    }

    /// Build a validation error with machine-readable structured details.
    pub fn validation_structured_details(
        message: impl Into<String>,
        details: impl IntoIterator<Item = Value>,
    ) -> Self {
        Self::Validation {
            message: message.into(),
            details: details.into_iter().collect(),
        }
    }

    pub fn conversion(message: impl Into<String>) -> Self {
        Self::Conversion {
            message: message.into(),
        }
    }

    pub fn invalid_tool_output(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidToolOutput {
            name: name.into(),
            message: message.into(),
        }
    }

    pub fn handler(message: impl Into<String>) -> Self {
        Self::Handler {
            message: message.into(),
        }
    }

    pub fn invalid_schema(label: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidSchema {
            label: label.into(),
            message: message.into(),
        }
    }

    pub fn duplicate_tool(name: impl Into<String>) -> Self {
        Self::DuplicateTool { name: name.into() }
    }

    pub fn duplicate_resource(uri: impl Into<String>) -> Self {
        Self::DuplicateResource { uri: uri.into() }
    }

    pub fn unknown_resource(uri: impl Into<String>) -> Self {
        Self::UnknownResource { uri: uri.into() }
    }

    pub fn duplicate_prompt(name: impl Into<String>) -> Self {
        Self::DuplicatePrompt { name: name.into() }
    }

    pub fn unknown_prompt(name: impl Into<String>) -> Self {
        Self::UnknownPrompt { name: name.into() }
    }
}

pub(crate) fn reject_unknown_arguments(arguments: McpToolArguments) -> Result<(), McpToolError> {
    if let Some(field) = arguments.keys().next() {
        return Err(McpToolError::UnknownField {
            field: field.clone(),
        });
    }
    Ok(())
}

pub(crate) fn validate_value_against_closed_schema(
    field: &str,
    schema: &Value,
    value: &Value,
) -> Result<(), McpToolError> {
    if value.is_null() {
        return Ok(());
    }

    let Value::Object(schema) = schema else {
        return Ok(());
    };

    if let Some(object_value) = value.as_object() {
        if let Some(schemas) = schema.get("anyOf").and_then(Value::as_array) {
            validate_value_against_any_closed_schema(field, schemas, value)?;
        }
        if let Some(schemas) = schema.get("oneOf").and_then(Value::as_array) {
            validate_value_against_any_closed_schema(field, schemas, value)?;
        }
        if let Some(schemas) = schema.get("allOf").and_then(Value::as_array) {
            for schema in applicable_closed_schemas(schemas, value) {
                validate_value_against_closed_schema(field, schema, value)?;
            }
        }
        validate_closed_object_fields(field, schema, object_value)?;
    } else if let Some(array_value) = value.as_array() {
        if let Some(schemas) = schema.get("anyOf").and_then(Value::as_array) {
            validate_value_against_any_closed_schema(field, schemas, value)?;
        }
        if let Some(items) = schema.get("items") {
            for (index, item) in array_value.iter().enumerate() {
                validate_value_against_closed_schema(&format!("{field}[{index}]"), items, item)?;
            }
        }
        if let Some(items) = schema.get("prefixItems").and_then(Value::as_array) {
            for (index, (schema, item)) in items.iter().zip(array_value).enumerate() {
                validate_value_against_closed_schema(&format!("{field}[{index}]"), schema, item)?;
            }
        }
    }

    Ok(())
}

fn validate_value_against_any_closed_schema(
    field: &str,
    schemas: &[Value],
    value: &Value,
) -> Result<(), McpToolError> {
    let mut first_error = None;
    let mut saw_applicable_schema = false;
    for schema in applicable_closed_schemas(schemas, value) {
        saw_applicable_schema = true;
        match validate_value_against_closed_schema(field, schema, value) {
            Ok(()) => return Ok(()),
            Err(error) if first_error.is_none() => first_error = Some(error),
            Err(_) => {},
        }
    }

    if saw_applicable_schema {
        Err(first_error.expect("applicable schema should record validation result"))
    } else {
        Ok(())
    }
}

fn applicable_closed_schemas<'a>(
    schemas: &'a [Value],
    value: &Value,
) -> impl Iterator<Item = &'a Value> {
    schemas
        .iter()
        .filter(move |schema| closed_schema_applies_to_value(schema, value))
}

fn closed_schema_applies_to_value(schema: &Value, value: &Value) -> bool {
    let Value::Object(schema) = schema else {
        return false;
    };

    if schema.contains_key("anyOf") || schema.contains_key("oneOf") || schema.contains_key("allOf")
    {
        return true;
    }

    match value {
        Value::Object(_) => {
            type_includes(schema, "object")
                || schema.contains_key("properties")
                || schema.contains_key("required")
                || schema.contains_key("additionalProperties")
        },
        Value::Array(_) => {
            type_includes(schema, "array")
                || schema.contains_key("items")
                || schema.contains_key("prefixItems")
        },
        _ => false,
    }
}

pub(crate) fn type_includes(schema: &Map<String, Value>, expected: &str) -> bool {
    match schema.get("type") {
        Some(Value::String(value)) => value == expected,
        Some(Value::Array(values)) => values
            .iter()
            .any(|value| matches!(value, Value::String(value) if value == expected)),
        _ => false,
    }
}

fn validate_closed_object_fields(
    field: &str,
    schema: &Map<String, Value>,
    value: &Map<String, Value>,
) -> Result<(), McpToolError> {
    let mut wire_names = BTreeMap::new();
    let properties = schema.get("properties").and_then(Value::as_object);
    if let Some(properties) = properties {
        for (property, property_schema) in properties {
            wire_names.insert(property.as_str(), property.as_str());
            if let Some(aliases) = property_schema
                .get("x-mcpAliases")
                .and_then(Value::as_array)
            {
                for alias in aliases.iter().filter_map(Value::as_str) {
                    wire_names.insert(alias, property.as_str());
                }
            }
        }
    }

    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for required in required.iter().filter_map(Value::as_str) {
            let present = if wire_names.is_empty() {
                value.contains_key(required)
            } else {
                value.keys().any(|name| {
                    wire_names
                        .get(name.as_str())
                        .is_some_and(|property| *property == required)
                })
            };
            if !present {
                return Err(McpToolError::MissingField {
                    field: nested_field_name(field, required),
                });
            }
        }
    }

    let rejects_additional = rejects_additional_properties(schema);
    let additional_schema = additional_properties_schema(schema);
    let mut seen = BTreeSet::new();
    for (name, nested_value) in value {
        let Some(property) = wire_names.get(name.as_str()).copied() else {
            if rejects_additional {
                return Err(McpToolError::UnknownField {
                    field: nested_field_name(field, name),
                });
            }
            if let Some(additional_schema) = additional_schema {
                validate_value_against_closed_schema(
                    &nested_field_name(field, name),
                    additional_schema,
                    nested_value,
                )?;
            }
            continue;
        };
        if !seen.insert(property) {
            return Err(McpToolError::DuplicateField {
                field: nested_field_name(field, property),
            });
        }
        if let Some(property_schema) = properties.and_then(|properties| properties.get(property)) {
            validate_value_against_closed_schema(
                &nested_field_name(field, property),
                property_schema,
                nested_value,
            )?;
        }
    }

    Ok(())
}

fn rejects_additional_properties(schema: &Map<String, Value>) -> bool {
    schema
        .get("additionalProperties")
        .is_some_and(|value| matches!(value, Value::Bool(false)))
}

fn additional_properties_schema(schema: &Map<String, Value>) -> Option<&Value> {
    match schema.get("additionalProperties") {
        Some(Value::Bool(_)) | None => None,
        Some(schema) => Some(schema),
    }
}

fn nested_field_name(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_string()
    } else {
        format!("{parent}.{child}")
    }
}

pub(crate) fn normalize_value_against_schema(schema: &Value, value: Value) -> Value {
    if schema_has_composite_keywords(schema) {
        return normalize_with_composite_schema(schema, value);
    }

    let Value::Object(schema) = schema else {
        return value;
    };

    match value {
        Value::Object(value) => normalize_object_value_against_schema(schema, value),
        Value::Array(value) => normalize_array_value_against_schema(schema, value),
        Value::String(value) => normalize_string_value_against_schema(schema, value),
        value => value,
    }
}

fn schema_has_composite_keywords(schema: &Value) -> bool {
    let Value::Object(schema) = schema else {
        return false;
    };
    schema.contains_key("anyOf") || schema.contains_key("oneOf") || schema.contains_key("allOf")
}

fn normalize_with_composite_schema(schema: &Value, value: Value) -> Value {
    let Value::Object(schema) = schema else {
        return value;
    };

    for keyword in ["anyOf", "oneOf"] {
        let Some(schemas) = schema.get(keyword).and_then(Value::as_array) else {
            continue;
        };
        let Some(schema) = schemas
            .iter()
            .find(|schema| schema_applies_to_value_for_normalization(schema, &value))
        else {
            continue;
        };
        return normalize_value_against_schema(schema, value);
    }

    let Some(schemas) = schema.get("allOf").and_then(Value::as_array) else {
        return value;
    };
    let mut value = value;
    for schema in schemas {
        if schema_applies_to_value_for_normalization(schema, &value) {
            value = normalize_value_against_schema(schema, value);
        }
    }
    value
}

fn schema_applies_to_value_for_normalization(schema: &Value, value: &Value) -> bool {
    let Value::Object(schema_object) = schema else {
        return matches!(schema, Value::Bool(true));
    };

    if schema_object.is_empty() {
        return true;
    }
    if schema_object.contains_key("anyOf")
        || schema_object.contains_key("oneOf")
        || schema_object.contains_key("allOf")
    {
        return true;
    }

    match value {
        Value::Null => value_schema_allows_null(schema),
        Value::Bool(_) => type_includes(schema_object, "boolean"),
        Value::Number(number) if number.is_i64() || number.is_u64() => {
            type_includes(schema_object, "integer") || type_includes(schema_object, "number")
        },
        Value::Number(_) => type_includes(schema_object, "number"),
        Value::String(value) => {
            type_includes(schema_object, "string")
                || schema_object
                    .get("enum")
                    .and_then(Value::as_array)
                    .is_some_and(|values| {
                        values
                            .iter()
                            .any(|candidate| matches!(candidate, Value::String(candidate) if candidate == value))
                    })
                || enum_decode_alias_target(schema_object, value).is_some()
        },
        Value::Array(_) | Value::Object(_) => closed_schema_applies_to_value(schema, value),
    }
}

fn normalize_object_value_against_schema(
    schema: &Map<String, Value>,
    value: Map<String, Value>,
) -> Value {
    let mut wire_names = BTreeMap::new();
    let properties = schema.get("properties").and_then(Value::as_object);
    if let Some(properties) = properties {
        for (property, property_schema) in properties {
            wire_names.insert(property.as_str(), property.as_str());
            if let Some(aliases) = property_schema
                .get("x-mcpAliases")
                .and_then(Value::as_array)
            {
                for alias in aliases.iter().filter_map(Value::as_str) {
                    wire_names.insert(alias, property.as_str());
                }
            }
        }
    }

    let additional_schema = additional_properties_schema(schema);
    let mut normalized = Map::new();
    for (name, nested_value) in value {
        let Some(property) = wire_names.get(name.as_str()).copied() else {
            let nested_value = match additional_schema {
                Some(schema) => normalize_value_against_schema(schema, nested_value),
                None => nested_value,
            };
            normalized.insert(name, nested_value);
            continue;
        };

        let property_schema = properties
            .and_then(|properties| properties.get(property))
            .expect("wire name should refer to an existing schema property");
        let decode_name = property_schema
            .get("x-mcpDecodeName")
            .and_then(Value::as_str)
            .unwrap_or(property);
        normalized.insert(
            decode_name.to_string(),
            normalize_value_against_schema(property_schema, nested_value),
        );
    }

    Value::Object(normalized)
}

fn normalize_array_value_against_schema(schema: &Map<String, Value>, value: Vec<Value>) -> Value {
    if let Some(items) = schema.get("items") {
        return Value::Array(
            value
                .into_iter()
                .map(|item| normalize_value_against_schema(items, item))
                .collect(),
        );
    }

    let Some(prefix_items) = schema.get("prefixItems").and_then(Value::as_array) else {
        return Value::Array(value);
    };

    Value::Array(
        value
            .into_iter()
            .enumerate()
            .map(|(index, item)| match prefix_items.get(index) {
                Some(schema) => normalize_value_against_schema(schema, item),
                None => item,
            })
            .collect(),
    )
}

fn normalize_string_value_against_schema(schema: &Map<String, Value>, value: String) -> Value {
    match enum_decode_alias_target(schema, &value) {
        Some(target) => Value::String(target.to_string()),
        None => Value::String(value),
    }
}

fn enum_decode_alias_target<'a>(schema: &'a Map<String, Value>, value: &str) -> Option<&'a str> {
    schema
        .get("x-mcpEnumDecodeAliases")
        .and_then(Value::as_object)
        .and_then(|aliases| aliases.get(value))
        .and_then(Value::as_str)
}
