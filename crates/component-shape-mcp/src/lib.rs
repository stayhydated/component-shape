//! Shared MCP schema helpers and `rmcp` server glue for component-shape integrations.
//!
//! This crate owns protocol-level building blocks and schema-paired value
//! decoding. Domain integrations still own validation, authorization, and
//! handler contracts.

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
        CallToolRequestParams, Implementation, JsonObject, ListToolsResult, PaginatedRequestParams,
        ProtocolVersion, ServerCapabilities, ServerInfo, Tool,
    },
    service::{MaybeSendFuture, RequestContext},
    transport::stdio,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};

pub use rmcp;
pub use rmcp::model::{
    CallToolResult as ToolCallResult, Content as ContentBlock, Tool as ToolDefinition,
};
pub use serde;
pub use serde_json;

pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

pub type ServeStdioResult = Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>;
pub type McpToolArguments = Map<String, Value>;
pub type McpSchemaProperties = BTreeMap<String, McpSchema>;
pub type McpSchemaFn = fn() -> McpSchema;
type ToolFuture = Pin<Box<dyn Future<Output = ToolCallResult> + Send + 'static>>;

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

/// Optional application-facing metadata for a generated MCP tool.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McpToolMetadata {
    name: Option<&'static str>,
    title: Option<&'static str>,
    description: Option<&'static str>,
}

impl McpToolMetadata {
    pub const fn new() -> Self {
        Self {
            name: None,
            title: None,
            description: None,
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

    pub const fn name(self) -> Option<&'static str> {
        self.name
    }

    pub const fn title(self) -> Option<&'static str> {
        self.title
    }

    pub const fn description(self) -> Option<&'static str> {
        self.description
    }

    pub fn validate(self) -> Result<(), McpToolError> {
        if let Some(name) = self.name {
            validate_tool_name(name)?;
        }
        if let Some(title) = self.title {
            validate_tool_metadata_text("title", title)?;
        }
        if let Some(description) = self.description {
            validate_tool_metadata_text("description", description)?;
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

/// JSON Schema metadata for typed MCP argument values.
///
/// Runtime integrations use this trait when a Rust type can describe its
/// structured JSON shape more precisely than [`McpInput`]'s coarse object
/// marker. Type aliases inherit the implementation of their target type, and
/// app-owned newtypes or structs can derive this trait with
/// `#[derive(component_shape_mcp::McpJsonSchema)]`.
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
    Validation { message: String },
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
}

impl McpToolError {
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
        }
    }

    pub fn conversion(message: impl Into<String>) -> Self {
        Self::Conversion {
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
                Err(error) => return tool_error_result(error.to_string()),
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
                    return Box::pin(std::future::ready(tool_error_result(error.to_string())))
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

    pub fn call_tool(&self, name: &str, arguments: Option<Value>) -> ToolCallResult {
        match self.tools.get(name) {
            Some(executor) => {
                let call = match McpToolCall::from_value(arguments) {
                    Ok(call) => call,
                    Err(error) => return tool_error_result(error.to_string()),
                };
                block_on_tool_future(executor.call(call))
            },
            None => tool_error_result(
                McpToolError::UnknownTool {
                    name: name.to_string(),
                }
                .to_string(),
            ),
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
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
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
            Some(executor) => executor.call(call),
            None => {
                let result = tool_error_result(
                    McpToolError::UnknownTool {
                        name: name.to_string(),
                    }
                    .to_string(),
                );
                Box::pin(std::future::ready(result))
            },
        };
        async move { Ok(call.await) }
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
        Ok(Err(error)) => tool_error_result(
            McpToolError::handler(format!("failed to run async tool handler: {error}")).to_string(),
        ),
        Err(_) => tool_error_result(
            McpToolError::handler("async tool handler runtime panicked").to_string(),
        ),
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
    tool.input_schema = Arc::new(schema_object("input_schema", input_schema)?);
    tool.output_schema = output_schema
        .map(|schema| schema_object("output_schema", schema))
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

pub fn tool_definition_with_metadata(
    default_name: impl Into<String>,
    metadata: McpToolMetadata,
    input_schema: McpSchema,
    output_schema: Option<McpSchema>,
) -> Result<ToolDefinition, McpToolError> {
    metadata.validate()?;
    let default_name = default_name.into();
    let name = metadata.name().map(str::to_string).unwrap_or(default_name);
    tool_definition(
        name,
        metadata.title().map(str::to_string),
        metadata.description().map(str::to_string),
        input_schema,
        output_schema,
    )
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

pub fn validate_tool_name(name: &str) -> Result<(), McpToolError> {
    component_shape::validate_mcp_tool_name(name).map_err(|error| McpToolError::Validation {
        message: error.to_string(),
    })
}

pub fn validate_tool_metadata_text(label: &str, value: &str) -> Result<(), McpToolError> {
    component_shape::validate_mcp_tool_metadata_text(label, value).map_err(|error| {
        McpToolError::Validation {
            message: error.to_string(),
        }
    })
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

pub fn tool_structured_result(value: Value) -> ToolCallResult {
    let text = match &value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    };
    let mut result = ToolCallResult::success(vec![ContentBlock::text(text)]);
    result.structured_content = Some(value);
    result
}

pub fn tool_error_result(message: impl Into<String>) -> ToolCallResult {
    ToolCallResult::error(vec![ContentBlock::text(message.into())])
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
        Err(error) => tool_error_result(McpToolError::handler(error.to_string()).to_string()),
    }
}

pub fn serialize_response_value<Response>(response: Response) -> ToolCallResult
where
    Response: Serialize,
{
    match serde_json::to_value(response) {
        Ok(value) => tool_structured_result(value),
        Err(error) => tool_error_result(
            McpToolError::handler(format!("failed to serialize response: {error}")).to_string(),
        ),
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
        ToolDefinition,
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
        let metadata = super::McpToolMetadata::new()
            .with_name("custom_tool")
            .with_title("Custom tool")
            .with_description("Runs a custom tool.");

        assert_eq!(metadata.name(), Some("custom_tool"));
        assert_eq!(metadata.title(), Some("Custom tool"));
        assert_eq!(metadata.description(), Some("Runs a custom tool."));
    }

    #[test]
    fn tool_metadata_validates_optional_overrides() {
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
                schema(json!({ "type": "object" })),
                Some(schema(json!(false))),
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

        let metadata = super::McpToolMetadata::new()
            .with_name("custom_echo")
            .with_title("Custom echo")
            .with_description("Echoes a value.");
        let tool =
            super::tool_definition_for_input_with_metadata::<EchoInput>("echo", metadata, None)
                .expect("tool definition should build");

        assert_eq!(tool.name.as_ref(), "custom_echo");
        assert_eq!(tool.title.as_deref(), Some("Custom echo"));
        assert_eq!(tool.description.as_deref(), Some("Echoes a value."));
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
                        None,
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
    fn server_accepts_owned_metadata() {
        let server = McpServer::new("owned-server".to_string(), "1.2.3".to_string());

        assert_eq!(server.server_name, "owned-server");
        assert_eq!(server.server_version, "1.2.3");
    }
}
