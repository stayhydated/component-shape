//! Shared MCP schema helpers and `rmcp` server glue for component-shape integrations.
//!
//! This crate owns protocol-level building blocks, schema-paired value
//! decoding, and common validation metadata/error helpers. Domain integrations
//! still own validation execution, authorization, and handler contracts.

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt,
    future::Future,
    hash::Hash,
    marker::PhantomData,
    ops::Index,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};

pub use component_shape::{
    ComponentShapeFor, ComponentShapeMetadata, McpInput, McpInputShape, McpPrimitiveKind,
    McpRangeBoundKind,
};
#[cfg(feature = "derive")]
pub use component_shape_mcp_macros::{McpJsonSchema, McpToolInput};
use rmcp::{
    ErrorData, RoleServer, ServerHandler, ServiceExt as _,
    model::{
        AnnotateAble, CallToolRequestParams, GetPromptRequestParams, GetPromptResult,
        Implementation, JsonObject, ListPromptsResult, ListResourceTemplatesResult,
        ListResourcesResult, ListToolsResult, PaginatedRequestParams, Prompt, PromptMessage,
        PromptMessageRole, ProtocolVersion, RawResource, RawResourceTemplate,
        ReadResourceRequestParams, ReadResourceResult, ResourceContents, ResourceTemplate,
        ServerCapabilities, ServerInfo, Tool,
    },
    service::{MaybeSendFuture, RequestContext},
    transport::stdio,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};

pub use rmcp;
pub use rmcp::model::{
    CallToolResult as ToolCallResult, Content as ContentBlock, GetPromptResult as McpPromptResult,
    Icon as McpIcon, IconTheme as McpIconTheme, Prompt as PromptDefinition,
    PromptArgument as McpPromptArgument, PromptMessage as McpPromptMessage,
    PromptMessageContent as McpPromptMessageContent, PromptMessageRole as McpPromptMessageRole,
    RawResource as RawResourceDefinition, RawResourceTemplate as RawResourceTemplateDefinition,
    ReadResourceResult as McpResourceResult, Resource as ResourceDefinition,
    ResourceContents as McpResourceContents, ResourceTemplate as ResourceTemplateDefinition,
    TaskSupport as McpToolTaskSupport, Tool as ToolDefinition,
    ToolAnnotations as McpToolAnnotations, ToolExecution as McpToolExecution,
};
pub use serde;
pub use serde_json;

pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

pub type ServeStdioResult = Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>;
pub type McpToolArguments = Map<String, Value>;
pub type McpSchemaProperties = BTreeMap<String, McpSchema>;
pub type McpSchemaFn = fn() -> McpSchema;
type ToolFuture = Pin<Box<dyn Future<Output = ToolCallResult> + Send + 'static>>;
type ResourceFuture =
    Pin<Box<dyn Future<Output = Result<ReadResourceResult, ErrorData>> + Send + 'static>>;
type PromptFuture =
    Pin<Box<dyn Future<Output = Result<GetPromptResult, ErrorData>> + Send + 'static>>;

/// Where a generated MCP validation issue applies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
        match self {
            Self::Form => "form",
            Self::Field => "field",
            Self::Element => "element",
            Self::Filter => "filter",
        }
    }
}

/// Koruma target selector recorded for an MCP validation rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
        match self {
            Self::Default => "default",
            Self::Full => "full",
            Self::Unwrapped => "unwrapped",
        }
    }
}

/// Type argument syntax used by a generated validator descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
        match self {
            Self::None => "none",
            Self::Infer => "infer",
            Self::Explicit => "explicit",
        }
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
    pub fn required(field: impl Into<String>) -> Self {
        let field = field.into();
        Self::custom(
            McpValidationScope::Field,
            format!("missing required field `{field}`"),
        )
        .with_field(field)
        .with_validator("required")
    }

    /// Build an issue from a static validation rule descriptor.
    pub fn for_rule(
        field: impl Into<String>,
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
    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
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

/// Typed MCP tool call payload passed to registered handlers.
///
/// The shared server normalizes protocol-level arguments into this object
/// before dispatch. Domain integrations can then decode fields without
/// repeatedly accepting arbitrary JSON values at every handler boundary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct McpToolCall {
    arguments: McpToolArguments,
}

impl McpToolCall {
    pub fn new(arguments: McpToolArguments) -> Self {
        Self { arguments }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_value(arguments: Option<Value>) -> Result<Self, McpToolError> {
        match arguments {
            None => Ok(Self::empty()),
            Some(Value::Object(arguments)) => Ok(Self::new(arguments)),
            Some(_) => Err(McpToolError::ArgumentsMustBeObject),
        }
    }

    pub fn arguments(&self) -> &McpToolArguments {
        &self.arguments
    }

    pub fn into_arguments(self) -> McpArguments {
        McpArguments::new(self.arguments)
    }
}

/// Owning decoder for MCP tool argument objects.
///
/// Generated form and table integrations consume arguments through this type
/// instead of open-coding JSON map removal. Manual tool integrations can use
/// the same helpers and call [`McpArguments::finish`] when every expected
/// argument has been consumed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct McpArguments {
    arguments: McpToolArguments,
}

impl McpArguments {
    pub fn new(arguments: McpToolArguments) -> Self {
        Self { arguments }
    }

    pub fn is_empty(&self) -> bool {
        self.arguments.is_empty()
    }

    pub fn as_inner(&self) -> &McpToolArguments {
        &self.arguments
    }

    pub fn into_inner(self) -> McpToolArguments {
        self.arguments
    }

    pub fn take_raw(&mut self, field: &str) -> Option<Value> {
        self.arguments.remove(field)
    }

    pub fn take_raw_one_of(
        &mut self,
        field: &str,
        aliases: &[&str],
    ) -> Result<Option<Value>, McpToolError> {
        let mut found = self.take_raw(field);
        for alias in aliases {
            if let Some(alias_value) = self.take_raw(alias) {
                if found.is_some() {
                    return Err(McpToolError::DuplicateField {
                        field: field.to_string(),
                    });
                }
                found = Some(alias_value);
            }
        }
        Ok(found)
    }

    pub fn take_required_tool_value<T>(
        &mut self,
        field: impl Into<String>,
    ) -> Result<T, McpToolError>
    where
        T: McpToolValue,
    {
        let field = field.into();
        let value = self
            .take_raw(&field)
            .ok_or_else(|| McpToolError::missing_field(field.clone()))?;
        T::from_tool_value(&field, value)
    }

    pub fn take_required_tool_value_from<T>(
        &mut self,
        field: &'static str,
        aliases: &'static [&'static str],
    ) -> Result<T, McpToolError>
    where
        T: McpToolValue,
    {
        let value = self
            .take_raw_one_of(field, aliases)?
            .ok_or_else(|| McpToolError::missing_field(field))?;
        T::from_tool_value(field, value)
    }

    pub fn take_present_tool_value<T>(
        &mut self,
        field: impl Into<String>,
    ) -> Result<Option<T>, McpToolError>
    where
        T: McpToolValue,
    {
        let field = field.into();
        self.take_raw(&field)
            .map(|value| T::from_tool_value(&field, value))
            .transpose()
    }

    pub fn take_present_tool_value_from<T>(
        &mut self,
        field: &'static str,
        aliases: &'static [&'static str],
    ) -> Result<Option<T>, McpToolError>
    where
        T: McpToolValue,
    {
        self.take_raw_one_of(field, aliases)?
            .map(|value| T::from_tool_value(field, value))
            .transpose()
    }

    pub fn finish(self) -> Result<(), McpToolError> {
        reject_unknown_arguments(self.arguments)
    }
}

impl From<McpToolCall> for McpArguments {
    fn from(call: McpToolCall) -> Self {
        call.into_arguments()
    }
}

impl From<McpToolArguments> for McpArguments {
    fn from(arguments: McpToolArguments) -> Self {
        Self::new(arguments)
    }
}

/// Static icon metadata for an MCP tool definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McpToolIcon {
    src: &'static str,
    mime_type: Option<&'static str>,
    sizes: &'static [&'static str],
    theme: Option<McpIconTheme>,
}

impl McpToolIcon {
    /// Create icon metadata from an icon resource URI or data URI.
    pub const fn new(src: &'static str) -> Self {
        Self {
            src,
            mime_type: None,
            sizes: &[],
            theme: None,
        }
    }

    /// Override the icon MIME type.
    pub const fn with_mime_type(mut self, mime_type: &'static str) -> Self {
        self.mime_type = Some(mime_type);
        self
    }

    /// Declare supported icon sizes such as `"48x48"` or `"any"`.
    pub const fn with_sizes(mut self, sizes: &'static [&'static str]) -> Self {
        self.sizes = sizes;
        self
    }

    /// Declare the icon's intended theme.
    pub const fn with_theme(mut self, theme: McpIconTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    pub const fn src(self) -> &'static str {
        self.src
    }

    pub const fn mime_type(self) -> Option<&'static str> {
        self.mime_type
    }

    pub const fn sizes(self) -> &'static [&'static str] {
        self.sizes
    }

    pub const fn theme(self) -> Option<McpIconTheme> {
        self.theme
    }

    fn into_definition_icon(self) -> McpIcon {
        let mut icon = McpIcon::new(self.src);
        if let Some(mime_type) = self.mime_type {
            icon = icon.with_mime_type(mime_type);
        }
        if !self.sizes.is_empty() {
            icon = icon.with_sizes(self.sizes.iter().map(|size| (*size).to_string()).collect());
        }
        if let Some(theme) = self.theme {
            icon = icon.with_theme(theme);
        }
        icon
    }

    fn validate(self) -> Result<(), McpToolError> {
        validate_required_metadata_text("icon src", self.src)?;
        if let Some(mime_type) = self.mime_type {
            validate_required_metadata_text("icon mime_type", mime_type)?;
        }
        for size in self.sizes {
            validate_required_metadata_text("icon size", size)?;
        }
        Ok(())
    }
}

/// Optional application-facing metadata for a generated MCP tool.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McpToolMetadata {
    name: Option<&'static str>,
    title: Option<&'static str>,
    description: Option<&'static str>,
    read_only_hint: Option<bool>,
    destructive_hint: Option<bool>,
    idempotent_hint: Option<bool>,
    open_world_hint: Option<bool>,
    icons: &'static [McpToolIcon],
    task_support: Option<McpToolTaskSupport>,
}

impl McpToolMetadata {
    pub const fn new() -> Self {
        Self {
            name: None,
            title: None,
            description: None,
            read_only_hint: None,
            destructive_hint: None,
            idempotent_hint: None,
            open_world_hint: None,
            icons: &[],
            task_support: None,
        }
    }

    pub const fn with_name(mut self, name: &'static str) -> Self {
        self.name = Some(name);
        self
    }

    pub const fn with_title(mut self, title: &'static str) -> Self {
        self.title = Some(title);
        self
    }

    pub const fn with_description(mut self, description: &'static str) -> Self {
        self.description = Some(description);
        self
    }

    pub const fn with_read_only_hint(mut self, read_only: bool) -> Self {
        self.read_only_hint = Some(read_only);
        self
    }

    pub const fn with_destructive_hint(mut self, destructive: bool) -> Self {
        self.destructive_hint = Some(destructive);
        self
    }

    pub const fn with_idempotent_hint(mut self, idempotent: bool) -> Self {
        self.idempotent_hint = Some(idempotent);
        self
    }

    pub const fn with_open_world_hint(mut self, open_world: bool) -> Self {
        self.open_world_hint = Some(open_world);
        self
    }

    pub const fn with_icons(mut self, icons: &'static [McpToolIcon]) -> Self {
        self.icons = icons;
        self
    }

    pub const fn with_task_support(mut self, task_support: McpToolTaskSupport) -> Self {
        self.task_support = Some(task_support);
        self
    }

    pub const fn name(self) -> Option<&'static str> {
        self.name
    }

    pub const fn title(self) -> Option<&'static str> {
        self.title
    }

    pub const fn description(self) -> Option<&'static str> {
        self.description
    }

    pub const fn read_only_hint(self) -> Option<bool> {
        self.read_only_hint
    }

    pub const fn destructive_hint(self) -> Option<bool> {
        self.destructive_hint
    }

    pub const fn idempotent_hint(self) -> Option<bool> {
        self.idempotent_hint
    }

    pub const fn open_world_hint(self) -> Option<bool> {
        self.open_world_hint
    }

    pub const fn icons(self) -> &'static [McpToolIcon] {
        self.icons
    }

    pub const fn task_support(self) -> Option<McpToolTaskSupport> {
        self.task_support
    }

    pub fn tool_annotations(self) -> Option<McpToolAnnotations> {
        if self.read_only_hint.is_none()
            && self.destructive_hint.is_none()
            && self.idempotent_hint.is_none()
            && self.open_world_hint.is_none()
        {
            return None;
        }

        Some(McpToolAnnotations::from_raw(
            self.title.map(str::to_string),
            self.read_only_hint,
            self.destructive_hint,
            self.idempotent_hint,
            self.open_world_hint,
        ))
    }

    pub fn tool_icons(self) -> Option<Vec<McpIcon>> {
        (!self.icons.is_empty()).then(|| {
            self.icons
                .iter()
                .map(|icon| icon.into_definition_icon())
                .collect()
        })
    }

    pub fn tool_execution(self) -> Option<McpToolExecution> {
        self.task_support
            .map(|task_support| McpToolExecution::from_raw(Some(task_support)))
    }

    pub fn validate(self) -> Result<(), McpToolError> {
        validate_tool_annotation_hints(self.read_only_hint, self.destructive_hint)?;
        if let Some(name) = self.name {
            validate_tool_name(name)?;
        }
        if let Some(title) = self.title {
            validate_tool_metadata_text("title", title)?;
        }
        if let Some(description) = self.description {
            validate_tool_metadata_text("description", description)?;
        }
        for icon in self.icons {
            icon.validate()?;
        }
        Ok(())
    }
}

/// Typed JSON Schema value used by generated MCP tool metadata.
///
/// This wrapper keeps schema-producing APIs distinct from arbitrary tool-call
/// JSON while still exposing the underlying [`serde_json::Value`] for tests,
/// protocol handoff, and last-mile schema extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpSchema(Value);

impl McpSchema {
    pub fn new(schema: Value) -> Self {
        Self(schema)
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.set_description(description);
        self
    }

    pub fn with_extension(mut self, key: impl Into<String>, value: Value) -> Self {
        self.set_extension(key, value);
        self
    }

    pub fn set_description(&mut self, description: impl Into<String>) {
        self.set_extension("description", Value::String(description.into()));
    }

    pub fn set_extension(&mut self, key: impl Into<String>, value: Value) {
        if let Some(object) = self.as_object_mut() {
            object.insert(key.into(), value);
        }
    }

    pub fn as_value(&self) -> &Value {
        &self.0
    }

    pub fn as_object_mut(&mut self) -> Option<&mut Map<String, Value>> {
        self.0.as_object_mut()
    }

    pub fn into_value(self) -> Value {
        self.0
    }
}

impl<I> Index<I> for McpSchema
where
    Value: Index<I>,
{
    type Output = <Value as Index<I>>::Output;

    fn index(&self, index: I) -> &Self::Output {
        &self.0[index]
    }
}

/// JSON Schema metadata for typed MCP argument and result values.
///
/// Runtime integrations use this trait when a Rust type can describe its
/// structured JSON shape more precisely than [`McpInput`]'s coarse object
/// marker. Type aliases inherit the implementation of their target type, and
/// app-owned newtypes or structs can derive this trait with
/// `#[derive(component_shape_mcp::McpJsonSchema)]`. `serde_json::Value`
/// publishes an unconstrained schema for dynamic argument fields; tool output
/// schemas must still declare an object root.
pub trait McpJsonSchema {
    fn json_schema() -> McpSchema;
}

/// Explicit unconstrained JSON value for typed MCP inputs.
///
/// Use this instead of `serde_json::Value` in `McpToolInput` or
/// `McpToolValue` positions when a field intentionally accepts any JSON. Raw
/// protocol and server internals still use `serde_json::Value` directly.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct McpAny(Value);

impl McpAny {
    pub fn new(value: Value) -> Self {
        Self(value)
    }

    pub fn as_value(&self) -> &Value {
        &self.0
    }

    pub fn into_value(self) -> Value {
        self.0
    }
}

impl From<Value> for McpAny {
    fn from(value: Value) -> Self {
        Self::new(value)
    }
}

impl From<McpAny> for Value {
    fn from(value: McpAny) -> Self {
        value.into_value()
    }
}

impl std::ops::Deref for McpAny {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        self.as_value()
    }
}

impl McpJsonSchema for McpAny {
    fn json_schema() -> McpSchema {
        McpSchema::new(json!({}))
    }
}

impl McpJsonSchema for Value {
    fn json_schema() -> McpSchema {
        McpSchema::new(json!({}))
    }
}

/// Typed MCP tool input decoded from a protocol argument object.
///
/// Derive this on app-owned form submit or table query argument structs when a
/// generated integration should publish the JSON schema and pass a typed value
/// to the tool handler. Field schemas and decoders are paired through
/// [`McpToolValue`]. The derive also implements [`McpJsonSchema`] for the input
/// struct, so object-shaped tool inputs can be reused as nested field or filter
/// values.
pub trait McpToolInput: Sized {
    fn input_schema() -> McpSchema;

    fn from_tool_call(call: McpToolCall) -> Result<Self, McpToolError>;
}

impl McpToolInput for () {
    fn input_schema() -> McpSchema {
        object_schema(McpSchemaProperties::new(), std::iter::empty::<&str>())
    }

    fn from_tool_call(call: McpToolCall) -> Result<Self, McpToolError> {
        call.into_arguments().finish()
    }
}

/// Tool definition paired with the typed MCP input it was built from.
///
/// `McpServer::add_typed_tool` consumes this wrapper so a typed handler cannot
/// accidentally be registered with another input type's schema.
#[derive(Clone)]
pub struct McpTypedTool<Input> {
    definition: ToolDefinition,
    _input: PhantomData<fn() -> Input>,
}

impl<Input> McpTypedTool<Input> {
    fn from_definition_unchecked(definition: ToolDefinition) -> Self {
        Self {
            definition,
            _input: PhantomData,
        }
    }

    pub fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    pub fn definition_mut(&mut self) -> &mut ToolDefinition {
        &mut self.definition
    }

    pub fn into_definition(self) -> ToolDefinition {
        self.definition
    }
}

impl<Input> AsRef<ToolDefinition> for McpTypedTool<Input> {
    fn as_ref(&self) -> &ToolDefinition {
        self.definition()
    }
}

impl<Input> std::ops::Deref for McpTypedTool<Input> {
    type Target = ToolDefinition;

    fn deref(&self) -> &Self::Target {
        self.definition()
    }
}

/// Typed value used as one field inside an MCP tool argument object.
///
/// This is the value-level companion to [`McpToolInput`]. Generated form and
/// table integrations use it when a single field or filter value must provide
/// both JSON Schema metadata and strict serde-based decoding. Any type that
/// implements [`McpJsonSchema`] and [`serde::de::DeserializeOwned`] gets the
/// default implementation; JSON null is accepted only when the generated schema
/// allows null.
pub trait McpToolValue: Sized {
    fn tool_value_schema() -> McpSchema;

    fn from_tool_value(field: &str, value: Value) -> Result<Self, McpToolError>;
}

impl<T> McpToolValue for T
where
    T: McpJsonSchema + DeserializeOwned,
{
    fn tool_value_schema() -> McpSchema {
        T::json_schema()
    }

    fn from_tool_value(field: &str, value: Value) -> Result<Self, McpToolError> {
        let schema = T::json_schema();
        if value.is_null() && !schema_allows_null(&schema) {
            return Err(McpToolError::UnexpectedNull {
                field: field.to_string(),
            });
        }
        validate_value_against_closed_schema(field, schema.as_value(), &value)?;
        let value = normalize_value_against_schema(schema.as_value(), value);

        serde_json::from_value(value).map_err(|error| McpToolError::DecodeField {
            field: field.to_string(),
            message: error.to_string(),
        })
    }
}

/// Typed `{ "min": ..., "max": ... }` range argument used by MCP tools.
///
/// Custom table filter shapes can use this as their `RawValue` when they want
/// derive-driven MCP decoding for range-shaped arguments.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct McpRange<T> {
    pub min: Option<T>,
    pub max: Option<T>,
}

impl<T> McpRange<T> {
    pub fn new(min: Option<T>, max: Option<T>) -> Self {
        Self { min, max }
    }

    pub fn into_tuple(self) -> (Option<T>, Option<T>) {
        (self.min, self.max)
    }
}

impl<T> From<(Option<T>, Option<T>)> for McpRange<T> {
    fn from((min, max): (Option<T>, Option<T>)) -> Self {
        Self::new(min, max)
    }
}

impl<T> From<McpRange<T>> for (Option<T>, Option<T>) {
    fn from(value: McpRange<T>) -> Self {
        value.into_tuple()
    }
}

impl<T> McpJsonSchema for McpRange<T>
where
    T: McpJsonSchema,
{
    fn json_schema() -> McpSchema {
        range_schema(T::json_schema())
    }
}

impl McpJsonSchema for bool {
    fn json_schema() -> McpSchema {
        schema_for_primitive(McpPrimitiveKind::Boolean)
    }
}

macro_rules! impl_integer_schema {
    ($($ty:ty),* $(,)?) => {
        $(
            impl McpJsonSchema for $ty {
                fn json_schema() -> McpSchema {
                    schema_for_primitive(McpPrimitiveKind::Integer)
                }
            }
        )*
    };
}

impl_integer_schema!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize,
);

macro_rules! impl_number_schema {
    ($($ty:ty),* $(,)?) => {
        $(
            impl McpJsonSchema for $ty {
                fn json_schema() -> McpSchema {
                    schema_for_primitive(McpPrimitiveKind::Number)
                }
            }
        )*
    };
}

impl_number_schema!(f32, f64);

#[cfg(feature = "rust_decimal")]
impl McpJsonSchema for rust_decimal::Decimal {
    fn json_schema() -> McpSchema {
        schema_for_primitive(McpPrimitiveKind::Decimal)
    }
}

impl McpJsonSchema for String {
    fn json_schema() -> McpSchema {
        schema_for_primitive(McpPrimitiveKind::String)
    }
}

impl McpJsonSchema for str {
    fn json_schema() -> McpSchema {
        schema_for_primitive(McpPrimitiveKind::String)
    }
}

impl McpJsonSchema for char {
    fn json_schema() -> McpSchema {
        schema_for_primitive(McpPrimitiveKind::String)
    }
}

impl McpJsonSchema for PathBuf {
    fn json_schema() -> McpSchema {
        schema_for_primitive(McpPrimitiveKind::String)
    }
}

impl McpJsonSchema for Path {
    fn json_schema() -> McpSchema {
        schema_for_primitive(McpPrimitiveKind::String)
    }
}

#[cfg(feature = "chrono")]
impl McpJsonSchema for chrono::NaiveDate {
    fn json_schema() -> McpSchema {
        schema_for_primitive(McpPrimitiveKind::Date)
    }
}

#[cfg(feature = "chrono")]
impl McpJsonSchema for chrono::NaiveDateTime {
    fn json_schema() -> McpSchema {
        schema_for_primitive(McpPrimitiveKind::DateTime)
    }
}

#[cfg(feature = "chrono")]
impl<Tz> McpJsonSchema for chrono::DateTime<Tz>
where
    Tz: chrono::TimeZone,
{
    fn json_schema() -> McpSchema {
        schema_for_primitive(McpPrimitiveKind::DateTime)
    }
}

impl<T> McpJsonSchema for Option<T>
where
    T: McpJsonSchema,
{
    fn json_schema() -> McpSchema {
        nullable_schema(T::json_schema())
    }
}

impl<T> McpJsonSchema for Vec<T>
where
    T: McpJsonSchema,
{
    fn json_schema() -> McpSchema {
        array_schema(T::json_schema())
    }
}

impl<T> McpJsonSchema for [T]
where
    T: McpJsonSchema,
{
    fn json_schema() -> McpSchema {
        array_schema(T::json_schema())
    }
}

impl<T, const N: usize> McpJsonSchema for [T; N]
where
    T: McpJsonSchema,
{
    fn json_schema() -> McpSchema {
        let mut schema = array_schema(T::json_schema());
        if let Some(object) = schema.as_object_mut() {
            object.insert("minItems".to_string(), json!(N));
            object.insert("maxItems".to_string(), json!(N));
        }
        schema
    }
}

macro_rules! impl_tuple_schema {
    ($($name:ident),+ $(,)?) => {
        impl<$($name),+> McpJsonSchema for ($($name,)+)
        where
            $($name: McpJsonSchema,)+
        {
            fn json_schema() -> McpSchema {
                tuple_schema([
                    $(<$name as McpJsonSchema>::json_schema(),)+
                ])
            }
        }
    };
}

impl_tuple_schema!(A);
impl_tuple_schema!(A, B);
impl_tuple_schema!(A, B, C);
impl_tuple_schema!(A, B, C, D);

impl<T> McpJsonSchema for BTreeSet<T>
where
    T: McpJsonSchema,
{
    fn json_schema() -> McpSchema {
        unique_array_schema(T::json_schema())
    }
}

impl<T> McpJsonSchema for HashSet<T>
where
    T: Eq + Hash + McpJsonSchema,
{
    fn json_schema() -> McpSchema {
        unique_array_schema(T::json_schema())
    }
}

impl<T> McpJsonSchema for BTreeMap<String, T>
where
    T: McpJsonSchema,
{
    fn json_schema() -> McpSchema {
        string_keyed_object_schema(T::json_schema())
    }
}

impl<T> McpJsonSchema for HashMap<String, T>
where
    T: McpJsonSchema,
{
    fn json_schema() -> McpSchema {
        string_keyed_object_schema(T::json_schema())
    }
}

impl<T> McpJsonSchema for Map<String, T>
where
    T: McpJsonSchema,
{
    fn json_schema() -> McpSchema {
        string_keyed_object_schema(T::json_schema())
    }
}

impl<T> McpJsonSchema for &T
where
    T: ?Sized + McpJsonSchema,
{
    fn json_schema() -> McpSchema {
        T::json_schema()
    }
}

impl<'a, T> McpJsonSchema for Cow<'a, T>
where
    T: ?Sized + ToOwned + McpJsonSchema,
{
    fn json_schema() -> McpSchema {
        T::json_schema()
    }
}

pub fn array_schema(item_schema: McpSchema) -> McpSchema {
    let item_schema = item_schema.into_value();
    McpSchema::new(json!({
        "type": "array",
        "items": item_schema
    }))
}

pub fn unique_array_schema(item_schema: McpSchema) -> McpSchema {
    let mut schema = array_schema(item_schema);
    if let Some(object) = schema.as_object_mut() {
        object.insert("uniqueItems".to_string(), Value::Bool(true));
    }
    schema
}

pub fn tuple_schema<I>(item_schemas: I) -> McpSchema
where
    I: IntoIterator<Item = McpSchema>,
{
    let prefix_items = item_schemas
        .into_iter()
        .map(McpSchema::into_value)
        .collect::<Vec<_>>();
    let len = prefix_items.len();

    McpSchema::new(json!({
        "type": "array",
        "prefixItems": prefix_items,
        "minItems": len,
        "maxItems": len
    }))
}

pub fn string_keyed_object_schema(value_schema: McpSchema) -> McpSchema {
    let value_schema = value_schema.into_value();
    McpSchema::new(json!({
        "type": "object",
        "additionalProperties": value_schema
    }))
}

pub fn object_schema<I, S>(properties: McpSchemaProperties, required: I) -> McpSchema
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let properties = properties
        .into_iter()
        .map(|(name, schema)| (name, schema.into_value()))
        .collect::<Map<String, Value>>();
    let required = required
        .into_iter()
        .map(|field| Value::String(field.into()))
        .collect::<Vec<_>>();

    McpSchema::new(json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    }))
}

impl<T> McpJsonSchema for Box<T>
where
    T: McpJsonSchema,
{
    fn json_schema() -> McpSchema {
        T::json_schema()
    }
}

impl McpJsonSchema for () {
    fn json_schema() -> McpSchema {
        McpSchema::new(json!({ "type": "null" }))
    }
}

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

fn reject_unknown_arguments(arguments: McpToolArguments) -> Result<(), McpToolError> {
    if let Some(field) = arguments.keys().next() {
        return Err(McpToolError::UnknownField {
            field: field.clone(),
        });
    }
    Ok(())
}

fn validate_value_against_closed_schema(
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

fn type_includes(schema: &Map<String, Value>, expected: &str) -> bool {
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

fn normalize_value_against_schema(schema: &Value, value: Value) -> Value {
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

#[derive(Clone)]
pub struct McpServer {
    server_name: Cow<'static, str>,
    server_version: Cow<'static, str>,
    tools: BTreeMap<String, Arc<dyn ToolExecutor>>,
    resources: BTreeMap<String, Arc<dyn ResourceReader>>,
    resource_templates: Vec<ResourceTemplate>,
    prompts: BTreeMap<String, Arc<dyn PromptExecutor>>,
}

impl McpServer {
    /// Create a dynamic MCP tool server with the advertised metadata.
    pub fn new(
        server_name: impl Into<Cow<'static, str>>,
        server_version: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            server_name: server_name.into(),
            server_version: server_version.into(),
            tools: BTreeMap::new(),
            resources: BTreeMap::new(),
            resource_templates: Vec::new(),
            prompts: BTreeMap::new(),
        }
    }

    /// Start building a dynamic MCP tool server with generated registrars.
    pub fn builder(
        server_name: impl Into<Cow<'static, str>>,
        server_version: impl Into<Cow<'static, str>>,
    ) -> McpServerBuilder {
        McpServerBuilder::new(server_name, server_version)
    }

    pub fn add_tool<Call>(
        &mut self,
        definition: ToolDefinition,
        call: Call,
    ) -> Result<(), McpToolError>
    where
        Call: Fn(McpToolCall) -> ToolCallResult + Send + Sync + 'static,
    {
        let name = definition.name.to_string();
        validate_tool_definition(&definition)?;
        if self.tools.contains_key(&name) {
            return Err(McpToolError::duplicate_tool(name));
        }

        self.tools.insert(
            name,
            Arc::new(RegisteredTool {
                definition,
                call: Arc::new(move |arguments| Box::pin(std::future::ready(call(arguments)))),
            }),
        );
        Ok(())
    }

    pub fn add_typed_tool<Input, Call>(
        &mut self,
        definition: McpTypedTool<Input>,
        call: Call,
    ) -> Result<(), McpToolError>
    where
        Input: McpToolInput,
        Call: Fn(Input) -> ToolCallResult + Send + Sync + 'static,
    {
        self.add_tool(definition.into_definition(), move |tool_call| {
            let input = match Input::from_tool_call(tool_call) {
                Ok(input) => input,
                Err(error) => return tool_error_result_for(error),
            };
            call(input)
        })
    }

    pub fn add_tool_async<Call, Fut>(
        &mut self,
        definition: ToolDefinition,
        call: Call,
    ) -> Result<(), McpToolError>
    where
        Call: Fn(McpToolCall) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToolCallResult> + Send + 'static,
    {
        let name = definition.name.to_string();
        validate_tool_definition(&definition)?;
        if self.tools.contains_key(&name) {
            return Err(McpToolError::duplicate_tool(name));
        }

        self.tools.insert(
            name,
            Arc::new(RegisteredTool {
                definition,
                call: Arc::new(move |arguments| Box::pin(call(arguments))),
            }),
        );
        Ok(())
    }

    pub fn add_typed_tool_async<Input, Call, Fut>(
        &mut self,
        definition: McpTypedTool<Input>,
        call: Call,
    ) -> Result<(), McpToolError>
    where
        Input: McpToolInput,
        Call: Fn(Input) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToolCallResult> + Send + 'static,
    {
        self.add_tool_async(definition.into_definition(), move |tool_call| {
            let input = Input::from_tool_call(tool_call);
            let future = match input {
                Ok(input) => call(input),
                Err(error) => {
                    return Box::pin(std::future::ready(tool_error_result_for(error)))
                        as ToolFuture;
                },
            };
            Box::pin(future) as ToolFuture
        })
    }

    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|executor| executor.definition())
            .collect()
    }

    pub fn contains_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Register a static MCP resource reader.
    pub fn add_resource<Read>(
        &mut self,
        definition: ResourceDefinition,
        read: Read,
    ) -> Result<(), McpToolError>
    where
        Read: Fn() -> ReadResourceResult + Send + Sync + 'static,
    {
        self.add_resource_async(definition, move || {
            let result = read();
            std::future::ready(Ok(result))
        })
    }

    /// Register an async MCP resource reader.
    pub fn add_resource_async<Read, Fut>(
        &mut self,
        definition: ResourceDefinition,
        read: Read,
    ) -> Result<(), McpToolError>
    where
        Read: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ReadResourceResult, ErrorData>> + Send + 'static,
    {
        let uri = definition.raw.uri.clone();
        validate_resource_definition(&definition)?;
        if self.resources.contains_key(&uri) {
            return Err(McpToolError::duplicate_resource(uri));
        }

        self.resources.insert(
            uri,
            Arc::new(RegisteredResource {
                definition,
                read: Arc::new(move || Box::pin(read())),
            }),
        );
        Ok(())
    }

    /// Register an MCP resource template advertised by `resources/templates/list`.
    pub fn add_resource_template(
        &mut self,
        definition: ResourceTemplateDefinition,
    ) -> Result<(), McpToolError> {
        validate_resource_template(&definition)?;
        self.resource_templates.push(definition);
        Ok(())
    }

    /// Return registered MCP resource definitions.
    pub fn list_resources(&self) -> Vec<ResourceDefinition> {
        self.resources
            .values()
            .map(|resource| resource.definition())
            .collect()
    }

    /// Return registered MCP resource template definitions.
    pub fn list_resource_templates(&self) -> Vec<ResourceTemplateDefinition> {
        self.resource_templates.clone()
    }

    /// Whether a resource URI is already registered.
    pub fn contains_resource(&self, uri: &str) -> bool {
        self.resources.contains_key(uri)
    }

    /// Number of registered concrete resources.
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    /// Register a static MCP prompt.
    pub fn add_prompt<Get>(
        &mut self,
        definition: PromptDefinition,
        get: Get,
    ) -> Result<(), McpToolError>
    where
        Get: Fn(Option<JsonObject>) -> GetPromptResult + Send + Sync + 'static,
    {
        self.add_prompt_async(definition, move |arguments| {
            let result = get(arguments);
            std::future::ready(Ok(result))
        })
    }

    /// Register an async MCP prompt.
    pub fn add_prompt_async<Get, Fut>(
        &mut self,
        definition: PromptDefinition,
        get: Get,
    ) -> Result<(), McpToolError>
    where
        Get: Fn(Option<JsonObject>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<GetPromptResult, ErrorData>> + Send + 'static,
    {
        let name = definition.name.clone();
        validate_prompt_definition(&definition)?;
        if self.prompts.contains_key(&name) {
            return Err(McpToolError::duplicate_prompt(name));
        }

        self.prompts.insert(
            name,
            Arc::new(RegisteredPrompt {
                definition,
                get: Arc::new(move |arguments| Box::pin(get(arguments))),
            }),
        );
        Ok(())
    }

    /// Return registered MCP prompt definitions.
    pub fn list_prompts(&self) -> Vec<PromptDefinition> {
        self.prompts
            .values()
            .map(|prompt| prompt.definition())
            .collect()
    }

    /// Whether a prompt name is already registered.
    pub fn contains_prompt(&self, name: &str) -> bool {
        self.prompts.contains_key(name)
    }

    /// Number of registered prompts.
    pub fn prompt_count(&self) -> usize {
        self.prompts.len()
    }

    pub fn call_tool(&self, name: &str, arguments: Option<Value>) -> ToolCallResult {
        match self.tools.get(name) {
            Some(executor) => {
                let call = match McpToolCall::from_value(arguments) {
                    Ok(call) => call,
                    Err(error) => return tool_error_result_for(error),
                };
                let output_schema = executor.definition().output_schema;
                validate_tool_call_result(
                    name,
                    output_schema.as_deref(),
                    block_on_tool_future(executor.call(call)),
                )
            },
            None => tool_error_result_for(McpToolError::UnknownTool {
                name: name.to_string(),
            }),
        }
    }

    pub async fn serve_stdio(self) -> ServeStdioResult {
        let service = self.serve(stdio()).await?;
        service.waiting().await?;
        Ok(())
    }

    pub fn serve_stdio_blocking(self) -> ServeStdioResult {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        runtime.block_on(self.serve_stdio())
    }
}

#[derive(Clone)]
pub struct McpServerBuilder {
    server: Result<McpServer, McpToolError>,
}

impl McpServerBuilder {
    pub fn new(
        server_name: impl Into<Cow<'static, str>>,
        server_version: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            server: Ok(McpServer::new(server_name, server_version)),
        }
    }

    pub fn register<Register>(mut self, register: Register) -> Self
    where
        Register: FnOnce(&mut McpServer) -> Result<(), McpToolError>,
    {
        if let Ok(server) = self.server.as_mut()
            && let Err(error) = register(server)
        {
            self.server = Err(error);
        }
        self
    }

    pub fn tool<Call>(self, definition: ToolDefinition, call: Call) -> Self
    where
        Call: Fn(McpToolCall) -> ToolCallResult + Send + Sync + 'static,
    {
        self.register(move |server| server.add_tool(definition, call))
    }

    pub fn typed_tool<Input, Call>(self, definition: McpTypedTool<Input>, call: Call) -> Self
    where
        Input: McpToolInput,
        Call: Fn(Input) -> ToolCallResult + Send + Sync + 'static,
    {
        self.register(move |server| server.add_typed_tool(definition, call))
    }

    pub fn tool_async<Call, Fut>(self, definition: ToolDefinition, call: Call) -> Self
    where
        Call: Fn(McpToolCall) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToolCallResult> + Send + 'static,
    {
        self.register(move |server| server.add_tool_async(definition, call))
    }

    pub fn typed_tool_async<Input, Call, Fut>(
        self,
        definition: McpTypedTool<Input>,
        call: Call,
    ) -> Self
    where
        Input: McpToolInput,
        Call: Fn(Input) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToolCallResult> + Send + 'static,
    {
        self.register(move |server| server.add_typed_tool_async(definition, call))
    }

    /// Add a static MCP resource to the server being built.
    pub fn resource<Read>(self, definition: ResourceDefinition, read: Read) -> Self
    where
        Read: Fn() -> ReadResourceResult + Send + Sync + 'static,
    {
        self.register(move |server| server.add_resource(definition, read))
    }

    /// Add an async MCP resource to the server being built.
    pub fn resource_async<Read, Fut>(self, definition: ResourceDefinition, read: Read) -> Self
    where
        Read: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ReadResourceResult, ErrorData>> + Send + 'static,
    {
        self.register(move |server| server.add_resource_async(definition, read))
    }

    /// Add an MCP resource template to the server being built.
    pub fn resource_template(self, definition: ResourceTemplateDefinition) -> Self {
        self.register(move |server| server.add_resource_template(definition))
    }

    /// Add a static MCP prompt to the server being built.
    pub fn prompt<Get>(self, definition: PromptDefinition, get: Get) -> Self
    where
        Get: Fn(Option<JsonObject>) -> GetPromptResult + Send + Sync + 'static,
    {
        self.register(move |server| server.add_prompt(definition, get))
    }

    /// Add an async MCP prompt to the server being built.
    pub fn prompt_async<Get, Fut>(self, definition: PromptDefinition, get: Get) -> Self
    where
        Get: Fn(Option<JsonObject>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<GetPromptResult, ErrorData>> + Send + 'static,
    {
        self.register(move |server| server.add_prompt_async(definition, get))
    }

    pub fn build(self) -> Result<McpServer, McpToolError> {
        self.server
    }

    pub async fn serve_stdio(self) -> ServeStdioResult {
        self.build()?.serve_stdio().await
    }

    pub fn serve_stdio_blocking(self) -> ServeStdioResult {
        self.build()?.serve_stdio_blocking()
    }
}

impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        let mut capabilities = ServerCapabilities::builder().enable_tools().build();
        capabilities.resources = (!self.resources.is_empty()
            || !self.resource_templates.is_empty())
        .then(Default::default);
        capabilities.prompts = (!self.prompts.is_empty()).then(Default::default);
        ServerInfo::new(capabilities)
            .with_protocol_version(ProtocolVersion::V_2025_11_25)
            .with_server_info(Implementation::new(
                self.server_name.clone(),
                self.server_version.clone(),
            ))
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(Ok(ListToolsResult {
            tools: self.list_tools(),
            next_cursor: None,
            meta: None,
        }))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools.get(name).map(|executor| executor.definition())
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ToolCallResult, ErrorData>> + MaybeSendFuture + '_ {
        let name = request.name.to_string();
        let call = McpToolCall::new(request.arguments.unwrap_or_default());
        let call = match self.tools.get(&name) {
            Some(executor) => {
                let output_schema = executor.definition().output_schema;
                let name = name.clone();
                let call = executor.call(call);
                Box::pin(async move {
                    validate_tool_call_result(&name, output_schema.as_deref(), call.await)
                }) as ToolFuture
            },
            None => {
                let result = tool_error_result_for(McpToolError::UnknownTool {
                    name: name.to_string(),
                });
                Box::pin(std::future::ready(result))
            },
        };
        async move { Ok(call.await) }
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(Ok(ListResourcesResult {
            resources: self.list_resources(),
            next_cursor: None,
            meta: None,
        }))
    }

    fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourceTemplatesResult, ErrorData>> + MaybeSendFuture + '_
    {
        std::future::ready(Ok(ListResourceTemplatesResult {
            resource_templates: self.list_resource_templates(),
            next_cursor: None,
            meta: None,
        }))
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ReadResourceResult, ErrorData>> + MaybeSendFuture + '_ {
        let uri = request.uri;
        let read = self.resources.get(&uri).map(|resource| resource.read());
        async move {
            match read {
                Some(read) => read.await,
                None => Err(ErrorData::resource_not_found(
                    format!("resource `{uri}` not found"),
                    Some(McpToolError::unknown_resource(uri).to_structured_value()),
                )),
            }
        }
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(Ok(ListPromptsResult {
            prompts: self.list_prompts(),
            next_cursor: None,
            meta: None,
        }))
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<GetPromptResult, ErrorData>> + MaybeSendFuture + '_ {
        let name = request.name;
        let get = self
            .prompts
            .get(&name)
            .map(|prompt| prompt.get(request.arguments));
        async move {
            match get {
                Some(get) => get.await,
                None => Err(ErrorData::invalid_params(
                    format!("prompt `{name}` not found"),
                    Some(McpToolError::unknown_prompt(name).to_structured_value()),
                )),
            }
        }
    }
}

trait ToolExecutor: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    fn call(&self, call: McpToolCall) -> ToolFuture;
}

struct RegisteredTool {
    definition: ToolDefinition,
    call: Arc<dyn Fn(McpToolCall) -> ToolFuture + Send + Sync>,
}

impl ToolExecutor for RegisteredTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    fn call(&self, call: McpToolCall) -> ToolFuture {
        (self.call)(call)
    }
}

trait ResourceReader: Send + Sync {
    fn definition(&self) -> ResourceDefinition;
    fn read(&self) -> ResourceFuture;
}

struct RegisteredResource {
    definition: ResourceDefinition,
    read: Arc<dyn Fn() -> ResourceFuture + Send + Sync>,
}

impl ResourceReader for RegisteredResource {
    fn definition(&self) -> ResourceDefinition {
        self.definition.clone()
    }

    fn read(&self) -> ResourceFuture {
        (self.read)()
    }
}

trait PromptExecutor: Send + Sync {
    fn definition(&self) -> PromptDefinition;
    fn get(&self, arguments: Option<JsonObject>) -> PromptFuture;
}

struct RegisteredPrompt {
    definition: PromptDefinition,
    get: Arc<dyn Fn(Option<JsonObject>) -> PromptFuture + Send + Sync>,
}

impl PromptExecutor for RegisteredPrompt {
    fn definition(&self) -> PromptDefinition {
        self.definition.clone()
    }

    fn get(&self, arguments: Option<JsonObject>) -> PromptFuture {
        (self.get)(arguments)
    }
}

fn validate_tool_call_result(
    tool_name: &str,
    output_schema: Option<&JsonObject>,
    result: ToolCallResult,
) -> ToolCallResult {
    let Some(output_schema) = output_schema else {
        return result;
    };
    if result.is_error == Some(true) {
        return result;
    }

    let Some(structured_content) = result.structured_content.as_ref() else {
        return tool_error_result_for(McpToolError::invalid_tool_output(
            tool_name,
            "tool declares output_schema but returned no structured_content",
        ));
    };

    if !structured_content.is_object() {
        return tool_error_result_for(McpToolError::invalid_tool_output(
            tool_name,
            "tool declares output_schema with object root but returned non-object structured_content",
        ));
    }

    let output_schema = Value::Object(output_schema.clone());
    if let Err(error) = validate_value_against_closed_schema(
        "structured_content",
        &output_schema,
        structured_content,
    ) {
        return tool_error_result_for(McpToolError::invalid_tool_output(
            tool_name,
            error.to_string(),
        ));
    }

    result
}

fn block_on_tool_future(future: ToolFuture) -> ToolCallResult {
    let join = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(runtime.block_on(future))
    })
    .join();

    match join {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => tool_error_result_for(McpToolError::handler(format!(
            "failed to run async tool handler: {error}"
        ))),
        Err(_) => {
            tool_error_result_for(McpToolError::handler("async tool handler runtime panicked"))
        },
    }
}

pub fn tool_definition(
    name: impl Into<String>,
    title: Option<String>,
    description: Option<String>,
    input_schema: McpSchema,
    output_schema: Option<McpSchema>,
) -> Result<ToolDefinition, McpToolError> {
    let name = name.into();
    validate_tool_name(&name)?;
    if let Some(title) = &title {
        validate_tool_metadata_text("title", title)?;
    }
    if let Some(description) = &description {
        validate_tool_metadata_text("description", description)?;
    }

    let mut tool = ToolDefinition::default();
    tool.name = Cow::Owned(name);
    tool.title = title;
    tool.description = description.map(Cow::Owned);
    tool.input_schema = Arc::new(input_schema_object("input_schema", input_schema)?);
    tool.output_schema = output_schema
        .map(|schema| output_schema_object("output_schema", schema))
        .transpose()?
        .map(Arc::new);
    Ok(tool)
}

pub fn tool_definition_for_input<Input>(
    name: impl Into<String>,
    title: Option<String>,
    description: Option<String>,
    output_schema: Option<McpSchema>,
) -> Result<McpTypedTool<Input>, McpToolError>
where
    Input: McpToolInput,
{
    tool_definition(
        name,
        title,
        description,
        Input::input_schema(),
        output_schema,
    )
    .map(McpTypedTool::from_definition_unchecked)
}

pub fn tool_definition_with_annotations(
    name: impl Into<String>,
    title: Option<String>,
    description: Option<String>,
    input_schema: McpSchema,
    output_schema: Option<McpSchema>,
    annotations: Option<McpToolAnnotations>,
) -> Result<ToolDefinition, McpToolError> {
    if let Some(annotations) = annotations.as_ref() {
        validate_tool_annotations(annotations)?;
    }
    let mut tool = tool_definition(name, title, description, input_schema, output_schema)?;
    tool.annotations = annotations;
    Ok(tool)
}

pub fn tool_definition_for_input_with_annotations<Input>(
    name: impl Into<String>,
    title: Option<String>,
    description: Option<String>,
    output_schema: Option<McpSchema>,
    annotations: Option<McpToolAnnotations>,
) -> Result<McpTypedTool<Input>, McpToolError>
where
    Input: McpToolInput,
{
    tool_definition_with_annotations(
        name,
        title,
        description,
        Input::input_schema(),
        output_schema,
        annotations,
    )
    .map(McpTypedTool::from_definition_unchecked)
}

pub fn tool_definition_with_metadata(
    default_name: impl Into<String>,
    metadata: McpToolMetadata,
    input_schema: McpSchema,
    output_schema: Option<McpSchema>,
) -> Result<ToolDefinition, McpToolError> {
    metadata.validate()?;
    let default_name = default_name.into();
    let name = metadata.name().map(str::to_string).unwrap_or(default_name);
    let mut tool = tool_definition(
        name,
        metadata.title().map(str::to_string),
        metadata.description().map(str::to_string),
        input_schema,
        output_schema,
    )?;
    tool.annotations = metadata.tool_annotations();
    tool.icons = metadata.tool_icons();
    tool.execution = metadata.tool_execution();
    Ok(tool)
}

pub fn tool_definition_for_input_with_metadata<Input>(
    default_name: impl Into<String>,
    metadata: McpToolMetadata,
    output_schema: Option<McpSchema>,
) -> Result<McpTypedTool<Input>, McpToolError>
where
    Input: McpToolInput,
{
    tool_definition_with_metadata(default_name, metadata, Input::input_schema(), output_schema)
        .map(McpTypedTool::from_definition_unchecked)
}

/// Build and validate a concrete MCP resource definition.
pub fn resource_definition(
    uri: impl Into<String>,
    name: impl Into<String>,
    title: Option<String>,
    description: Option<String>,
    mime_type: Option<String>,
) -> Result<ResourceDefinition, McpToolError> {
    let mut resource = RawResource::new(uri, name);
    resource.title = title;
    resource.description = description;
    resource.mime_type = mime_type;
    let resource = resource.no_annotation();
    validate_resource_definition(&resource)?;
    Ok(resource)
}

/// Build and validate an MCP resource template definition.
pub fn resource_template_definition(
    uri_template: impl Into<String>,
    name: impl Into<String>,
    title: Option<String>,
    description: Option<String>,
    mime_type: Option<String>,
) -> Result<ResourceTemplateDefinition, McpToolError> {
    let mut template = RawResourceTemplate::new(uri_template, name);
    template.title = title;
    template.description = description;
    template.mime_type = mime_type;
    let template = template.no_annotation();
    validate_resource_template(&template)?;
    Ok(template)
}

/// Build a text resource result with an explicit MIME type.
pub fn text_resource_result(
    uri: impl Into<String>,
    text: impl Into<String>,
    mime_type: impl Into<String>,
) -> ReadResourceResult {
    ReadResourceResult::new(vec![
        ResourceContents::text(text, uri).with_mime_type(mime_type),
    ])
}

/// Encode a JSON value as a pretty-printed `application/json` resource result.
pub fn json_resource_result(
    uri: impl Into<String>,
    value: &Value,
) -> Result<ReadResourceResult, McpToolError> {
    let text = serde_json::to_string_pretty(value).map_err(|error| {
        McpToolError::conversion(format!("failed to encode JSON resource: {error}"))
    })?;
    Ok(text_resource_result(uri, text, "application/json"))
}

/// Build and validate an MCP prompt definition.
pub fn prompt_definition(
    name: impl Into<String>,
    title: Option<String>,
    description: Option<String>,
    arguments: Option<Vec<McpPromptArgument>>,
) -> Result<PromptDefinition, McpToolError> {
    let mut prompt = Prompt::new(name, description, arguments);
    prompt.title = title;
    validate_prompt_definition(&prompt)?;
    Ok(prompt)
}

/// Build a prompt result containing one user text message.
pub fn text_prompt_result(description: Option<String>, text: impl Into<String>) -> GetPromptResult {
    let mut result =
        GetPromptResult::new(vec![PromptMessage::new_text(PromptMessageRole::User, text)]);
    result.description = description;
    result
}

pub fn validate_tool_name(name: &str) -> Result<(), McpToolError> {
    component_shape::validate_mcp_tool_name(name)
        .map_err(|error| McpToolError::validation(error.to_string()))
}

pub fn validate_tool_metadata_text(label: &str, value: &str) -> Result<(), McpToolError> {
    component_shape::validate_mcp_tool_metadata_text(label, value)
        .map_err(|error| McpToolError::validation(error.to_string()))
}

pub fn validate_tool_annotations(annotations: &McpToolAnnotations) -> Result<(), McpToolError> {
    validate_tool_annotation_hints(annotations.read_only_hint, annotations.destructive_hint)?;
    if let Some(title) = annotations.title.as_deref() {
        validate_tool_metadata_text("annotation title", title)?;
    }
    Ok(())
}

pub fn validate_tool_definition(definition: &ToolDefinition) -> Result<(), McpToolError> {
    validate_tool_name(definition.name.as_ref())?;
    validate_tool_input_schema("input_schema", definition.input_schema.as_ref())?;
    if let Some(output_schema) = definition.output_schema.as_ref() {
        validate_tool_output_schema("output_schema", output_schema.as_ref())?;
    }
    if let Some(title) = definition.title.as_deref() {
        validate_tool_metadata_text("title", title)?;
    }
    if let Some(description) = definition.description.as_deref() {
        validate_tool_metadata_text("description", description)?;
    }
    if let Some(annotations) = definition.annotations.as_ref() {
        validate_tool_annotations(annotations)?;
    }
    if let Some(icons) = definition.icons.as_ref() {
        for icon in icons {
            validate_icon_definition(icon)?;
        }
    }
    Ok(())
}

/// Validate a concrete MCP resource definition accepted by this shared server.
pub fn validate_resource_definition(definition: &ResourceDefinition) -> Result<(), McpToolError> {
    validate_required_metadata_text("resource uri", &definition.uri)?;
    validate_required_metadata_text("resource name", &definition.name)?;
    if let Some(title) = definition.title.as_deref() {
        validate_tool_metadata_text("resource title", title)?;
    }
    if let Some(description) = definition.description.as_deref() {
        validate_tool_metadata_text("resource description", description)?;
    }
    if let Some(mime_type) = definition.mime_type.as_deref() {
        validate_required_metadata_text("resource mime type", mime_type)?;
    }
    Ok(())
}

/// Validate an MCP resource template definition accepted by this shared server.
pub fn validate_resource_template(
    definition: &ResourceTemplateDefinition,
) -> Result<(), McpToolError> {
    validate_required_metadata_text("resource uri template", &definition.uri_template)?;
    validate_required_metadata_text("resource template name", &definition.name)?;
    if let Some(title) = definition.title.as_deref() {
        validate_tool_metadata_text("resource template title", title)?;
    }
    if let Some(description) = definition.description.as_deref() {
        validate_tool_metadata_text("resource template description", description)?;
    }
    if let Some(mime_type) = definition.mime_type.as_deref() {
        validate_required_metadata_text("resource template mime type", mime_type)?;
    }
    Ok(())
}

/// Validate an MCP prompt definition accepted by this shared server.
pub fn validate_prompt_definition(definition: &PromptDefinition) -> Result<(), McpToolError> {
    validate_tool_name(&definition.name)?;
    if let Some(title) = definition.title.as_deref() {
        validate_tool_metadata_text("prompt title", title)?;
    }
    if let Some(description) = definition.description.as_deref() {
        validate_tool_metadata_text("prompt description", description)?;
    }
    if let Some(arguments) = definition.arguments.as_deref() {
        for argument in arguments {
            validate_tool_name(&argument.name)?;
            if let Some(title) = argument.title.as_deref() {
                validate_tool_metadata_text("prompt argument title", title)?;
            }
            if let Some(description) = argument.description.as_deref() {
                validate_tool_metadata_text("prompt argument description", description)?;
            }
        }
    }
    Ok(())
}

fn validate_required_metadata_text(label: &str, value: &str) -> Result<(), McpToolError> {
    if value.trim().is_empty() {
        return Err(McpToolError::invalid_schema(label, "must not be empty"));
    }
    validate_tool_metadata_text(label, value)
}

fn validate_icon_definition(icon: &McpIcon) -> Result<(), McpToolError> {
    validate_required_metadata_text("icon src", &icon.src)?;
    if let Some(mime_type) = icon.mime_type.as_deref() {
        validate_required_metadata_text("icon mime_type", mime_type)?;
    }
    if let Some(sizes) = icon.sizes.as_ref() {
        for size in sizes {
            validate_required_metadata_text("icon size", size)?;
        }
    }
    Ok(())
}

fn validate_tool_annotation_hints(
    read_only: Option<bool>,
    destructive: Option<bool>,
) -> Result<(), McpToolError> {
    if read_only == Some(true) && destructive == Some(true) {
        return Err(McpToolError::validation(
            "MCP tool annotation hints cannot be both read-only and destructive",
        ));
    }
    Ok(())
}

pub fn schema_object(
    label: impl Into<String>,
    schema: McpSchema,
) -> Result<JsonObject, McpToolError> {
    match schema.into_value() {
        Value::Object(object) => Ok(object),
        _ => Err(McpToolError::invalid_schema(
            label,
            "MCP tool schemas must be JSON objects",
        )),
    }
}

fn input_schema_object(
    label: impl Into<String>,
    schema: McpSchema,
) -> Result<JsonObject, McpToolError> {
    let label = label.into();
    let object = schema_object(label.clone(), schema)?;
    validate_tool_input_schema(&label, &object)?;
    Ok(object)
}

fn output_schema_object(
    label: impl Into<String>,
    schema: McpSchema,
) -> Result<JsonObject, McpToolError> {
    let label = label.into();
    let object = schema_object(label.clone(), schema)?;
    validate_tool_output_schema(&label, &object)?;
    Ok(object)
}

fn validate_tool_input_schema(
    label: impl Into<String>,
    schema: &JsonObject,
) -> Result<(), McpToolError> {
    let label = label.into();
    let type_value = schema.get("type").ok_or_else(|| {
        McpToolError::invalid_schema(
            label.clone(),
            "MCP tool input schemas must declare `type: \"object\"`",
        )
    })?;

    let object_type = match type_value {
        Value::String(value) => value == "object",
        Value::Array(values) => values
            .iter()
            .any(|value| matches!(value, Value::String(value) if value == "object")),
        _ => false,
    };

    if object_type {
        Ok(())
    } else {
        Err(McpToolError::invalid_schema(
            label,
            "MCP tool input schemas must declare `type: \"object\"`",
        ))
    }
}

fn validate_tool_output_schema(
    label: impl Into<String>,
    schema: &JsonObject,
) -> Result<(), McpToolError> {
    if type_includes(schema, "object") {
        Ok(())
    } else {
        Err(McpToolError::invalid_schema(
            label,
            "MCP tool output schemas must declare `type: \"object\"`",
        ))
    }
}

pub fn tool_structured_result(value: Value) -> ToolCallResult {
    let text = match &value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    };
    let mut result = ToolCallResult::success(vec![ContentBlock::text(text)]);
    result.structured_content = Some(value);
    result
}

/// Build an MCP error result for a plain message.
///
/// Prefer [`tool_error_result_for`] when the failure is a typed
/// [`McpToolError`], so clients receive a specific `error.kind`.
pub fn tool_error_result(message: impl Into<String>) -> ToolCallResult {
    let message = message.into();
    tool_error_result_with_structured_content(
        message.clone(),
        json!({
            "error": {
                "kind": "error",
                "message": message,
            }
        }),
    )
}

/// Build an MCP error result with machine-readable `structured_content.error`.
pub fn tool_error_result_for(error: McpToolError) -> ToolCallResult {
    let message = error.to_string();
    tool_error_result_with_structured_content(
        message,
        json!({
            "error": error.to_structured_value(),
        }),
    )
}

fn tool_error_result_with_structured_content(
    message: String,
    structured_content: Value,
) -> ToolCallResult {
    let mut result = ToolCallResult::error(vec![ContentBlock::text(message)]);
    result.structured_content = Some(structured_content);
    result
}

pub fn serialize_handler_response<Response, Error>(
    result: Result<Response, Error>,
) -> ToolCallResult
where
    Response: Serialize,
    Error: fmt::Display,
{
    match result {
        Ok(response) => serialize_response_value(response),
        Err(error) => tool_error_result_for(McpToolError::handler(error.to_string())),
    }
}

pub fn serialize_response_value<Response>(response: Response) -> ToolCallResult
where
    Response: Serialize,
{
    match serde_json::to_value(response) {
        Ok(value) => tool_structured_result(value),
        Err(error) => tool_error_result_for(McpToolError::handler(format!(
            "failed to serialize response: {error}"
        ))),
    }
}

pub fn tool_name(source_module_path: &str, subject_id: &str, fallback_prefix: &str) -> String {
    #[derive(Clone, Copy, Eq, PartialEq)]
    enum CharKind {
        Upper,
        Lower,
        Digit,
    }

    let input: Vec<char> = source_module_path
        .chars()
        .chain(['_'])
        .chain(subject_id.chars())
        .collect();
    let mut output = String::new();
    let mut last_was_separator = false;
    let mut last_kind = None;

    for (index, ch) in input.iter().copied().enumerate() {
        let kind = if ch.is_ascii_uppercase() {
            CharKind::Upper
        } else if ch.is_ascii_lowercase() {
            CharKind::Lower
        } else if ch.is_ascii_digit() {
            CharKind::Digit
        } else if !last_was_separator && !output.is_empty() {
            output.push('_');
            last_was_separator = true;
            last_kind = None;
            continue;
        } else {
            last_kind = None;
            continue;
        };

        if kind == CharKind::Upper && !last_was_separator && !output.is_empty() {
            let next_is_lower = input
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_lowercase());
            if matches!(last_kind, Some(CharKind::Lower | CharKind::Digit))
                || (matches!(last_kind, Some(CharKind::Upper)) && next_is_lower)
            {
                output.push('_');
            }
        }

        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
            last_was_separator = false;
            last_kind = Some(kind);
        }
    }

    while output.ends_with('_') {
        output.pop();
    }

    if output.is_empty() || output.starts_with(|ch: char| ch.is_ascii_digit()) {
        output.insert_str(0, fallback_prefix);
    }

    output
}

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

fn value_schema_allows_null(schema: &Value) -> bool {
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

#[cfg(test)]
mod tests {
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
            <McpRange<u32> as super::McpJsonSchema>::json_schema()["properties"]["min"]["anyOf"][0]
                ["type"],
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

        let value =
            <IssueState as super::McpToolValue>::from_tool_value("state", json!("reviewing"))
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
        let expected =
            std::collections::BTreeMap::from([("issue".to_string(), IssueState::InReview)]);
        assert_eq!(states, expected);

        #[derive(Debug, serde::Deserialize, crate::McpJsonSchema, PartialEq)]
        struct Preferences {
            #[mcp(rename = "email", alias = "emailUpdates")]
            email_updates: bool,
        }

        let error = <std::collections::BTreeMap<String, Preferences> as super::McpToolValue>::from_tool_value(
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

        let error = <std::collections::BTreeMap<String, Preferences> as super::McpToolValue>::from_tool_value(
            "preferences",
            json!({
                "team": {}
            }),
        )
        .expect_err("nested required fields should be enforced through additionalProperties schema");
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
            super::tool_definition("", None, None, schema(json!({ "type": "object" })), None)
                .is_err()
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

        let tool = super::tool_definition_for_input::<EchoInput>(
            "echo",
            Some("Echo".to_string()),
            None,
            None,
        )
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
        let tool =
            super::tool_definition_for_input_with_metadata::<EchoInput>("echo", metadata, None)
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
        assert!(
            matches!(error, super::McpToolError::DecodeField { field, .. } if field == "limit")
        );
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
                |call| {
                    super::tool_structured_result(Value::Object(call.into_arguments().into_inner()))
                },
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
                        super::tool_structured_result(Value::Object(
                            call.into_arguments().into_inner(),
                        ))
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
                |call| {
                    super::tool_structured_result(Value::Object(call.into_arguments().into_inner()))
                },
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
                |call| {
                    super::tool_structured_result(Value::Object(call.into_arguments().into_inner()))
                },
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
}
