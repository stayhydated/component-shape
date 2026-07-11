use super::*;

/// Where a generated MCP validation issue applies.
#[derive(Clone, Copy, Debug, Eq, IntoStaticStr, PartialEq)]
#[strum(serialize_all = "snake_case", const_into_str)]
pub enum McpValidationScope {
    /// The whole form failed validation without a precise field.
    Form,
    /// A field-level validator failed.
    Field,
    /// A validator for one item inside a collection field failed.
    Element,
    /// A table filter argument failed validation.
    Filter,
}

impl McpValidationScope {
    pub const fn as_str(self) -> &'static str {
        self.into_str()
    }
}

/// Koruma target selector recorded for an MCP validation rule.
#[derive(Clone, Copy, Debug, Eq, IntoStaticStr, PartialEq)]
#[strum(serialize_all = "snake_case", const_into_str)]
pub enum McpValidationTarget {
    /// Koruma selected the default target for the field type.
    Default,
    /// The validator targets the full field value, such as an `Option<T>`.
    Full,
    /// The validator targets the unwrapped field value.
    Unwrapped,
}

impl McpValidationTarget {
    pub const fn as_str(self) -> &'static str {
        self.into_str()
    }
}

/// Type argument syntax used by a generated validator descriptor.
#[derive(Clone, Copy, Debug, Eq, IntoStaticStr, PartialEq)]
#[strum(serialize_all = "snake_case", const_into_str)]
pub enum McpValidationTypeArgMode {
    /// The validator path did not supply a type argument.
    None,
    /// The validator used `::<_>`.
    Infer,
    /// The validator supplied an explicit type argument.
    Explicit,
}

impl McpValidationTypeArgMode {
    pub const fn as_str(self) -> &'static str {
        self.into_str()
    }
}

/// One builder argument captured from a generated validator chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McpValidationParam {
    name: &'static str,
    literal: Option<&'static str>,
    expr: Option<&'static str>,
}

impl McpValidationParam {
    /// Record a literal argument value that can be reflected into schemas.
    pub const fn literal(name: &'static str, literal: &'static str) -> Self {
        Self {
            name,
            literal: Some(literal),
            expr: None,
        }
    }

    /// Record a non-literal expression for tool clients to display or inspect.
    pub const fn expr(name: &'static str, expr: &'static str) -> Self {
        Self {
            name,
            literal: None,
            expr: Some(expr),
        }
    }

    /// Builder method or argument name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Literal argument value, when the derive could statically identify one.
    pub const fn literal_value(self) -> Option<&'static str> {
        self.literal
    }

    /// Non-literal argument expression, when no literal value is available.
    pub const fn expr_value(self) -> Option<&'static str> {
        self.expr
    }

    pub fn to_value(self) -> Value {
        let mut object = Map::new();
        object.insert("name".to_string(), Value::String(self.name.to_string()));
        if let Some(literal) = self.literal {
            object.insert("value".to_string(), Value::String(literal.to_string()));
        }
        if let Some(expr) = self.expr {
            object.insert("expr".to_string(), Value::String(expr.to_string()));
        }
        Value::Object(object)
    }
}

/// Shared empty validation parameter slice for generated descriptors.
pub const MCP_VALIDATION_PARAMS_NONE: &[McpValidationParam] = &[];

/// Static validator metadata attached to an MCP-visible field or filter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McpValidationRule {
    scope: McpValidationScope,
    validator: &'static str,
    path: &'static str,
    label: Option<&'static str>,
    target: Option<McpValidationTarget>,
    type_arg_mode: McpValidationTypeArgMode,
    params: &'static [McpValidationParam],
}

impl McpValidationRule {
    /// Create a validation rule descriptor for generated MCP metadata.
    pub const fn new(
        scope: McpValidationScope,
        validator: &'static str,
        path: &'static str,
        label: Option<&'static str>,
        type_arg_mode: McpValidationTypeArgMode,
        params: &'static [McpValidationParam],
    ) -> Self {
        Self {
            scope,
            validator,
            path,
            label,
            target: None,
            type_arg_mode,
            params,
        }
    }

    /// Attach a Koruma target selector to a rule descriptor.
    pub const fn with_target(mut self, target: McpValidationTarget) -> Self {
        self.target = Some(target);
        self
    }

    /// Scope where this validator runs.
    pub const fn scope(self) -> McpValidationScope {
        self.scope
    }

    /// Terminal validator type name.
    pub const fn validator(self) -> &'static str {
        self.validator
    }

    /// Validator path as written in the source attribute.
    pub const fn path(self) -> &'static str {
        self.path
    }

    /// Optional source label assigned to this validator.
    pub const fn label(self) -> Option<&'static str> {
        self.label
    }

    /// Optional Koruma target selector used by this validator.
    pub const fn target(self) -> Option<McpValidationTarget> {
        self.target
    }

    /// Type argument syntax used by this validator.
    pub const fn type_arg_mode(self) -> McpValidationTypeArgMode {
        self.type_arg_mode
    }

    /// Captured builder parameters for this validator.
    pub const fn params(self) -> &'static [McpValidationParam] {
        self.params
    }

    pub fn to_value(self) -> Value {
        let mut object = Map::new();
        object.insert(
            "scope".to_string(),
            Value::String(self.scope.as_str().to_string()),
        );
        object.insert(
            "validator".to_string(),
            Value::String(self.validator.to_string()),
        );
        object.insert("path".to_string(), Value::String(self.path.to_string()));
        if let Some(label) = self.label {
            object.insert("label".to_string(), Value::String(label.to_string()));
        }
        if let Some(target) = self.target {
            object.insert(
                "target".to_string(),
                Value::String(target.as_str().to_string()),
            );
        }
        object.insert(
            "type_arg_mode".to_string(),
            Value::String(self.type_arg_mode.as_str().to_string()),
        );
        if !self.params.is_empty() {
            object.insert(
                "params".to_string(),
                Value::Array(self.params.iter().map(|param| param.to_value()).collect()),
            );
        }
        Value::Object(object)
    }
}

/// Structured validation failure returned in MCP error details and snapshots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpValidationIssue {
    field: Option<String>,
    filter: Option<String>,
    scope: McpValidationScope,
    validator: Option<String>,
    path: Option<String>,
    label: Option<String>,
    target: Option<McpValidationTarget>,
    element_index: Option<usize>,
    message: String,
    params: Vec<McpValidationParam>,
}

impl McpValidationIssue {
    /// Build a form-scoped issue when no field-specific metadata is available.
    pub fn form(message: impl Into<String>) -> Self {
        Self::custom(McpValidationScope::Form, message)
    }

    /// Build a generic issue for integrations that already have structured metadata.
    pub fn custom(scope: McpValidationScope, message: impl Into<String>) -> Self {
        Self {
            field: None,
            filter: None,
            scope,
            validator: None,
            path: None,
            label: None,
            target: None,
            element_index: None,
            message: message.into(),
            params: Vec::new(),
        }
    }

    /// Build a required-field issue.
    pub fn required(field: impl AsRef<str>) -> Self {
        let field = field.as_ref();
        Self::custom(
            McpValidationScope::Field,
            format!("missing required field `{field}`"),
        )
        .with_field(field)
        .with_validator("required")
    }

    /// Build an issue from a static validation rule descriptor.
    pub fn for_rule(
        field: impl AsRef<str>,
        rule: McpValidationRule,
        message: impl Into<String>,
    ) -> Self {
        Self::custom(rule.scope(), message)
            .with_field(field)
            .with_rule(rule)
    }

    /// Build a table-filter issue from a static validation rule descriptor.
    pub fn for_filter_rule(
        filter: impl Into<String>,
        rule: McpValidationRule,
        message: impl Into<String>,
    ) -> Self {
        Self::custom(rule.scope(), message)
            .with_filter(filter)
            .with_rule(rule)
    }

    fn with_rule(mut self, rule: McpValidationRule) -> Self {
        self.validator = Some(rule.validator().to_string());
        self.path = Some(rule.path().to_string());
        self.label = rule.label().map(str::to_string);
        self.target = rule.target();
        self.params = rule.params().to_vec();
        self
    }

    /// Attach the failing collection element index to an element issue.
    pub fn with_element_index(mut self, element_index: usize) -> Self {
        self.element_index = Some(element_index);
        self
    }

    /// Attach a field name to an issue.
    pub fn with_field(mut self, field: impl AsRef<str>) -> Self {
        self.field = Some(field.as_ref().to_string());
        self
    }

    /// Attach a table filter name to an issue.
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Attach a validator name to a generic issue.
    pub fn with_validator(mut self, validator: impl Into<String>) -> Self {
        self.validator = Some(validator.into());
        self
    }

    /// Attach a source label to a generic issue.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Field name for field or element issues.
    pub fn field(&self) -> Option<&str> {
        self.field.as_deref()
    }

    /// Table filter name for filter issues.
    pub fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }

    /// Human-readable validation message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Convert the issue to JSON for MCP structured content.
    pub fn to_value(&self) -> Value {
        let mut object = Map::new();
        object.insert(
            "scope".to_string(),
            Value::String(self.scope.as_str().to_string()),
        );
        object.insert("message".to_string(), Value::String(self.message.clone()));
        if let Some(field) = &self.field {
            object.insert("field".to_string(), Value::String(field.clone()));
        }
        if let Some(filter) = &self.filter {
            object.insert("filter".to_string(), Value::String(filter.clone()));
        }
        if let Some(validator) = &self.validator {
            object.insert("validator".to_string(), Value::String(validator.clone()));
        }
        if let Some(path) = &self.path {
            object.insert("path".to_string(), Value::String(path.clone()));
        }
        if let Some(label) = &self.label {
            object.insert("label".to_string(), Value::String(label.clone()));
        }
        if let Some(target) = self.target {
            object.insert(
                "target".to_string(),
                Value::String(target.as_str().to_string()),
            );
        }
        if let Some(element_index) = self.element_index {
            object.insert(
                "element_index".to_string(),
                Value::Number((element_index as u64).into()),
            );
        }
        if !self.params.is_empty() {
            object.insert(
                "params".to_string(),
                Value::Array(self.params.iter().map(|param| param.to_value()).collect()),
            );
        }
        Value::Object(object)
    }
}

/// Convert validation issues into a structured MCP validation error.
pub fn validation_issues_error(issues: Vec<McpValidationIssue>) -> McpToolError {
    let message = issues
        .iter()
        .map(McpValidationIssue::message)
        .collect::<Vec<_>>()
        .join("; ");
    McpToolError::validation_structured_details(
        message,
        issues.into_iter().map(|issue| issue.to_value()),
    )
}
