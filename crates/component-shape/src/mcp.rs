use strum::IntoStaticStr;

/// Primitive value kinds a component shape can expose to model-controlled tools.
#[derive(Clone, Copy, Debug, Eq, IntoStaticStr, PartialEq)]
#[strum(serialize_all = "snake_case", const_into_str)]
pub enum McpPrimitiveKind {
    Any,
    Boolean,
    Integer,
    Number,
    Decimal,
    String,
    Date,
    DateTime,
}

impl McpPrimitiveKind {
    /// Returns the stable schema label for this primitive kind.
    pub const fn as_str(self) -> &'static str {
        self.into_str()
    }
}

/// Primitive kinds that can be used as `{ "min": ..., "max": ... }` MCP range bounds.
#[derive(Clone, Copy, Debug, Eq, IntoStaticStr, PartialEq)]
#[strum(serialize_all = "snake_case", const_into_str)]
pub enum McpRangeBoundKind {
    Integer,
    Number,
    Decimal,
    Date,
    DateTime,
}

impl McpRangeBoundKind {
    /// Returns the stable schema label for this range-bound kind.
    pub const fn as_str(self) -> &'static str {
        self.into_str()
    }

    /// Returns the primitive kind used by this range-bound kind.
    pub const fn primitive_kind(self) -> McpPrimitiveKind {
        match self {
            Self::Integer => McpPrimitiveKind::Integer,
            Self::Number => McpPrimitiveKind::Number,
            Self::Decimal => McpPrimitiveKind::Decimal,
            Self::Date => McpPrimitiveKind::Date,
            Self::DateTime => McpPrimitiveKind::DateTime,
        }
    }
}

impl From<McpRangeBoundKind> for McpPrimitiveKind {
    fn from(kind: McpRangeBoundKind) -> Self {
        kind.primitive_kind()
    }
}

/// Framework-neutral shape of structured MCP input accepted by a component.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McpInputShape {
    /// The component does not expose model-controlled MCP input.
    Unsupported,
    /// A single primitive value.
    Scalar(McpPrimitiveKind),
    /// An ordered array of primitive values.
    List(McpPrimitiveKind),
    /// A unique array of primitive values.
    Set(McpPrimitiveKind),
    /// A range object with `min` and `max` bounds.
    Range(McpRangeBoundKind),
    /// An object with integration-defined structure.
    Object,
}

/// Validation error for generated MCP tool metadata.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum McpToolMetadataError {
    /// The tool name is empty or contains only whitespace.
    #[error("tool name cannot be empty")]
    EmptyName,
    /// The tool name starts with an unsupported character.
    #[error("tool name must start with an ASCII letter or number")]
    InvalidNameStart,
    /// The tool name contains an unsupported character.
    #[error("tool name may only contain ASCII letters, digits, '_' '-' '.'")]
    InvalidNameCharacter,
    /// A human-readable metadata field is empty or contains only whitespace.
    #[error("tool {label} cannot be empty")]
    EmptyText { label: String },
}

/// Validate the MCP tool-name subset used by generated integrations.
///
/// # Errors
///
/// Returns [`McpToolMetadataError`] when `name` is empty, starts with an
/// unsupported character, or contains a character outside the generated tool
/// name subset.
pub fn validate_mcp_tool_name(name: &str) -> Result<(), McpToolMetadataError> {
    if name.trim().is_empty() {
        return Err(McpToolMetadataError::EmptyName);
    }

    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err(McpToolMetadataError::EmptyName);
    };

    if !first.is_ascii_alphanumeric() {
        return Err(McpToolMetadataError::InvalidNameStart);
    }

    if chars.any(|ch| !is_mcp_tool_name_char(ch)) {
        return Err(McpToolMetadataError::InvalidNameCharacter);
    }

    Ok(())
}

fn is_mcp_tool_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.'
}

/// Validate human-readable MCP tool metadata text.
///
/// # Errors
///
/// Returns [`McpToolMetadataError`] when `value` is empty or contains only
/// whitespace.
pub fn validate_mcp_tool_metadata_text(
    label: &str,
    value: &str,
) -> Result<(), McpToolMetadataError> {
    if value.trim().is_empty() {
        Err(McpToolMetadataError::EmptyText {
            label: label.to_string(),
        })
    } else {
        Ok(())
    }
}

impl McpInputShape {
    /// Returns whether this shape accepts model-controlled MCP input.
    pub const fn supported(self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

/// Shape-owned metadata for model-controlled MCP input.
///
/// This is metadata only. Protocol handling, JSON decoding, validation, and
/// application authorization stay in MCP integration crates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McpInput {
    input_shape: McpInputShape,
}

impl McpInput {
    /// No model-controlled MCP input is supported.
    pub const fn unsupported() -> Self {
        Self {
            input_shape: McpInputShape::Unsupported,
        }
    }

    /// Accept any JSON value.
    pub const fn any() -> Self {
        Self::scalar(McpPrimitiveKind::Any)
    }

    /// Accept a boolean scalar.
    pub const fn boolean() -> Self {
        Self::scalar(McpPrimitiveKind::Boolean)
    }

    /// Accept an integer scalar.
    pub const fn integer() -> Self {
        Self::scalar(McpPrimitiveKind::Integer)
    }

    /// Accept a number scalar.
    pub const fn number() -> Self {
        Self::scalar(McpPrimitiveKind::Number)
    }

    /// Accept a decimal scalar encoded as a JSON number or string.
    pub const fn decimal() -> Self {
        Self::scalar(McpPrimitiveKind::Decimal)
    }

    /// Accept a string scalar.
    pub const fn string() -> Self {
        Self::scalar(McpPrimitiveKind::String)
    }

    /// Accept an RFC 3339 full-date string.
    pub const fn date() -> Self {
        Self::scalar(McpPrimitiveKind::Date)
    }

    /// Accept an RFC 3339 date-time string.
    pub const fn date_time() -> Self {
        Self::scalar(McpPrimitiveKind::DateTime)
    }

    /// Accept a scalar of the given primitive kind.
    pub const fn scalar(kind: McpPrimitiveKind) -> Self {
        Self {
            input_shape: McpInputShape::Scalar(kind),
        }
    }

    /// Accept an ordered array of strings.
    pub const fn string_list() -> Self {
        Self::list(McpPrimitiveKind::String)
    }

    /// Accept an ordered array of booleans.
    pub const fn boolean_list() -> Self {
        Self::list(McpPrimitiveKind::Boolean)
    }

    /// Accept an ordered array of integers.
    pub const fn integer_list() -> Self {
        Self::list(McpPrimitiveKind::Integer)
    }

    /// Accept an ordered array of numbers.
    pub const fn number_list() -> Self {
        Self::list(McpPrimitiveKind::Number)
    }

    /// Accept an ordered array of decimals encoded as JSON numbers or strings.
    pub const fn decimal_list() -> Self {
        Self::list(McpPrimitiveKind::Decimal)
    }

    /// Accept an ordered array of RFC 3339 full-date strings.
    pub const fn date_list() -> Self {
        Self::list(McpPrimitiveKind::Date)
    }

    /// Accept an ordered array of RFC 3339 date-time strings.
    pub const fn date_time_list() -> Self {
        Self::list(McpPrimitiveKind::DateTime)
    }

    /// Accept an ordered array of primitive values.
    pub const fn list(items: McpPrimitiveKind) -> Self {
        Self {
            input_shape: McpInputShape::List(items),
        }
    }

    /// Accept a unique array of strings.
    pub const fn string_set() -> Self {
        Self::set(McpPrimitiveKind::String)
    }

    /// Accept a unique array of booleans.
    pub const fn boolean_set() -> Self {
        Self::set(McpPrimitiveKind::Boolean)
    }

    /// Accept a unique array of integers.
    pub const fn integer_set() -> Self {
        Self::set(McpPrimitiveKind::Integer)
    }

    /// Accept a unique array of numbers.
    pub const fn number_set() -> Self {
        Self::set(McpPrimitiveKind::Number)
    }

    /// Accept a unique array of decimals encoded as JSON numbers or strings.
    pub const fn decimal_set() -> Self {
        Self::set(McpPrimitiveKind::Decimal)
    }

    /// Accept a unique array of RFC 3339 full-date strings.
    pub const fn date_set() -> Self {
        Self::set(McpPrimitiveKind::Date)
    }

    /// Accept a unique array of RFC 3339 date-time strings.
    pub const fn date_time_set() -> Self {
        Self::set(McpPrimitiveKind::DateTime)
    }

    /// Accept a unique array of primitive values.
    pub const fn set(items: McpPrimitiveKind) -> Self {
        Self {
            input_shape: McpInputShape::Set(items),
        }
    }

    /// Accept a decimal `{ "min": ..., "max": ... }` range object.
    pub const fn decimal_range() -> Self {
        Self::range(McpRangeBoundKind::Decimal)
    }

    /// Accept an integer `{ "min": ..., "max": ... }` range object.
    pub const fn integer_range() -> Self {
        Self::range(McpRangeBoundKind::Integer)
    }

    /// Accept a number `{ "min": ..., "max": ... }` range object.
    pub const fn number_range() -> Self {
        Self::range(McpRangeBoundKind::Number)
    }

    /// Accept a date `{ "min": ..., "max": ... }` range object.
    pub const fn date_range() -> Self {
        Self::range(McpRangeBoundKind::Date)
    }

    /// Accept a date-time `{ "min": ..., "max": ... }` range object.
    pub const fn date_time_range() -> Self {
        Self::range(McpRangeBoundKind::DateTime)
    }

    /// Accept a `{ "min": ..., "max": ... }` range object of the given kind.
    pub const fn range(bound: McpRangeBoundKind) -> Self {
        Self {
            input_shape: McpInputShape::Range(bound),
        }
    }

    /// Accept an object with integration-defined structure.
    pub const fn object() -> Self {
        Self {
            input_shape: McpInputShape::Object,
        }
    }

    /// Return the structured input shape exposed to MCP integrations.
    pub const fn input_shape(self) -> McpInputShape {
        self.input_shape
    }

    /// Whether this metadata describes any supported input shape.
    pub const fn supported(self) -> bool {
        self.input_shape.supported()
    }
}

impl Default for McpInput {
    fn default() -> Self {
        Self::unsupported()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        McpInput, McpInputShape, McpPrimitiveKind, McpRangeBoundKind,
        validate_mcp_tool_metadata_text, validate_mcp_tool_name,
    };

    #[test]
    fn mcp_input_defaults_to_unsupported() {
        let input = McpInput::default();

        assert!(!input.supported());
        assert_eq!(input.input_shape(), McpInputShape::Unsupported);
    }

    #[test]
    fn mcp_input_records_shape() {
        let input = McpInput::range(McpRangeBoundKind::Date);

        assert!(input.supported());
        assert_eq!(
            input.input_shape(),
            McpInputShape::Range(McpRangeBoundKind::Date)
        );
    }

    #[test]
    fn mcp_input_convenience_constructors_match_common_shapes() {
        assert_eq!(
            McpInput::string().input_shape(),
            McpInputShape::Scalar(McpPrimitiveKind::String)
        );
        assert_eq!(
            McpInput::string_list().input_shape(),
            McpInputShape::List(McpPrimitiveKind::String)
        );
        assert_eq!(
            McpInput::string_set().input_shape(),
            McpInputShape::Set(McpPrimitiveKind::String)
        );
        assert_eq!(
            McpInput::date_range().input_shape(),
            McpInputShape::Range(McpRangeBoundKind::Date)
        );
        assert_eq!(
            McpInput::decimal_range().input_shape(),
            McpInputShape::Range(McpRangeBoundKind::Decimal)
        );
        assert_eq!(
            McpInput::integer_range().input_shape(),
            McpInputShape::Range(McpRangeBoundKind::Integer)
        );
        assert_eq!(
            McpInput::date_time_range().input_shape(),
            McpInputShape::Range(McpRangeBoundKind::DateTime)
        );
        assert_eq!(
            McpInput::decimal_list().input_shape(),
            McpInputShape::List(McpPrimitiveKind::Decimal)
        );
        assert_eq!(
            McpInput::decimal_set().input_shape(),
            McpInputShape::Set(McpPrimitiveKind::Decimal)
        );
    }

    #[test]
    fn all_mcp_input_convenience_constructors_match_their_shapes() {
        let scalar_cases: [(fn() -> McpInput, McpPrimitiveKind); 8] = [
            (McpInput::any, McpPrimitiveKind::Any),
            (McpInput::boolean, McpPrimitiveKind::Boolean),
            (McpInput::integer, McpPrimitiveKind::Integer),
            (McpInput::number, McpPrimitiveKind::Number),
            (McpInput::decimal, McpPrimitiveKind::Decimal),
            (McpInput::string, McpPrimitiveKind::String),
            (McpInput::date, McpPrimitiveKind::Date),
            (McpInput::date_time, McpPrimitiveKind::DateTime),
        ];
        for (constructor, kind) in scalar_cases {
            assert_eq!(constructor().input_shape(), McpInputShape::Scalar(kind));
        }

        let collection_cases: [(fn() -> McpInput, McpInputShape); 14] = [
            (
                McpInput::string_list,
                McpInputShape::List(McpPrimitiveKind::String),
            ),
            (
                McpInput::boolean_list,
                McpInputShape::List(McpPrimitiveKind::Boolean),
            ),
            (
                McpInput::integer_list,
                McpInputShape::List(McpPrimitiveKind::Integer),
            ),
            (
                McpInput::number_list,
                McpInputShape::List(McpPrimitiveKind::Number),
            ),
            (
                McpInput::decimal_list,
                McpInputShape::List(McpPrimitiveKind::Decimal),
            ),
            (
                McpInput::date_list,
                McpInputShape::List(McpPrimitiveKind::Date),
            ),
            (
                McpInput::date_time_list,
                McpInputShape::List(McpPrimitiveKind::DateTime),
            ),
            (
                McpInput::string_set,
                McpInputShape::Set(McpPrimitiveKind::String),
            ),
            (
                McpInput::boolean_set,
                McpInputShape::Set(McpPrimitiveKind::Boolean),
            ),
            (
                McpInput::integer_set,
                McpInputShape::Set(McpPrimitiveKind::Integer),
            ),
            (
                McpInput::number_set,
                McpInputShape::Set(McpPrimitiveKind::Number),
            ),
            (
                McpInput::decimal_set,
                McpInputShape::Set(McpPrimitiveKind::Decimal),
            ),
            (
                McpInput::date_set,
                McpInputShape::Set(McpPrimitiveKind::Date),
            ),
            (
                McpInput::date_time_set,
                McpInputShape::Set(McpPrimitiveKind::DateTime),
            ),
        ];
        for (constructor, shape) in collection_cases {
            assert_eq!(constructor().input_shape(), shape);
        }

        let range_cases: [(fn() -> McpInput, McpRangeBoundKind); 5] = [
            (McpInput::integer_range, McpRangeBoundKind::Integer),
            (McpInput::number_range, McpRangeBoundKind::Number),
            (McpInput::decimal_range, McpRangeBoundKind::Decimal),
            (McpInput::date_range, McpRangeBoundKind::Date),
            (McpInput::date_time_range, McpRangeBoundKind::DateTime),
        ];
        for (constructor, kind) in range_cases {
            assert_eq!(constructor().input_shape(), McpInputShape::Range(kind));
            assert_eq!(McpPrimitiveKind::from(kind), kind.primitive_kind());
        }

        assert_eq!(McpInput::object().input_shape(), McpInputShape::Object);
    }

    #[test]
    fn mcp_kind_names_are_stable_metadata() {
        assert_eq!(McpPrimitiveKind::Any.as_str(), "any");
        assert_eq!(McpPrimitiveKind::Boolean.as_str(), "boolean");
        assert_eq!(McpPrimitiveKind::Integer.as_str(), "integer");
        assert_eq!(McpPrimitiveKind::Number.as_str(), "number");
        assert_eq!(McpPrimitiveKind::Decimal.as_str(), "decimal");
        assert_eq!(McpPrimitiveKind::String.as_str(), "string");
        assert_eq!(McpPrimitiveKind::Date.as_str(), "date");
        assert_eq!(McpPrimitiveKind::DateTime.as_str(), "date_time");

        assert_eq!(McpRangeBoundKind::Integer.as_str(), "integer");
        assert_eq!(McpRangeBoundKind::Number.as_str(), "number");
        assert_eq!(McpRangeBoundKind::Decimal.as_str(), "decimal");
        assert_eq!(McpRangeBoundKind::Date.as_str(), "date");
        assert_eq!(McpRangeBoundKind::DateTime.as_str(), "date_time");
    }

    #[test]
    fn mcp_tool_name_validation_matches_generated_tool_contract() {
        assert!(validate_mcp_tool_name("query.users-1").is_ok());
        assert_eq!(
            validate_mcp_tool_name(""),
            Err(super::McpToolMetadataError::EmptyName)
        );
        assert_eq!(
            validate_mcp_tool_name("_query"),
            Err(super::McpToolMetadataError::InvalidNameStart)
        );
        assert_eq!(
            validate_mcp_tool_name("query users"),
            Err(super::McpToolMetadataError::InvalidNameCharacter)
        );
    }

    #[test]
    fn mcp_tool_metadata_text_validation_rejects_blank_text() {
        let error =
            validate_mcp_tool_metadata_text("title", "  ").expect_err("blank title should fail");

        assert_eq!(error.to_string(), "tool title cannot be empty");
        assert!(validate_mcp_tool_metadata_text("title", "Readable title").is_ok());
    }
}
