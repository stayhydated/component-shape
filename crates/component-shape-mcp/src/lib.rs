//! Shared MCP schema helpers and `rmcp` server glue for component-shape integrations.
//!
//! This crate owns protocol-level building blocks only. Domain integrations
//! still own typed decoding, validation, authorization, and handler contracts.

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt,
    future::Future,
    hash::Hash,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};

pub use component_shape::{
    ComponentShapeFor, ComponentShapeMetadata, McpInput, McpInputShape, McpPrimitiveKind,
};
#[cfg(feature = "derive")]
pub use component_shape_mcp_macros::McpJsonSchema;
use rmcp::{
    ErrorData, RoleServer, ServerHandler, ServiceExt,
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
pub use serde_json;

pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

pub type ServeStdioResult = Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>;
pub type McpToolArguments = Map<String, Value>;
type ToolFuture = Pin<Box<dyn Future<Output = ToolCallResult> + Send + 'static>>;

/// Typed MCP tool call payload passed to registered handlers.
///
/// The shared server normalizes protocol-level arguments into this object
/// before dispatch. Domain integrations can then decode fields without
/// repeatedly accepting arbitrary JSON values at every handler boundary.
#[derive(Clone, Debug, Default, PartialEq)]
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
#[derive(Clone, Debug, Default, PartialEq)]
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

    pub fn take_required<T>(&mut self, field: impl Into<String>) -> Result<T, McpToolError>
    where
        T: DeserializeOwned,
    {
        let field = field.into();
        let value = self
            .take_raw(&field)
            .ok_or_else(|| McpToolError::missing_field(field.clone()))?;
        decode_present_field(field, value)
    }

    /// Decode a present non-null field.
    ///
    /// Returns `Ok(None)` when the field is absent. A present JSON `null`
    /// remains an error.
    pub fn take_present<T>(&mut self, field: impl Into<String>) -> Result<Option<T>, McpToolError>
    where
        T: DeserializeOwned,
    {
        let field = field.into();
        self.take_raw(&field)
            .map(|value| decode_present_field(field, value))
            .transpose()
    }

    /// Decode a present nullable field.
    ///
    /// The outer `Option` represents presence in the argument object; the inner
    /// `Option` represents JSON null versus a decoded value.
    pub fn take_nullable<T>(
        &mut self,
        field: impl Into<String>,
    ) -> Result<Option<Option<T>>, McpToolError>
    where
        T: DeserializeOwned,
    {
        let field = field.into();
        self.take_raw(&field)
            .map(|value| decode_optional_field(field, value))
            .transpose()
    }

    /// Decode an optional field where both absence and JSON null mean `None`.
    pub fn take_optional<T>(&mut self, field: impl Into<String>) -> Result<Option<T>, McpToolError>
    where
        T: DeserializeOwned,
    {
        Ok(self.take_nullable(field)?.flatten())
    }

    pub fn take_required_usize(&mut self, field: impl Into<String>) -> Result<usize, McpToolError> {
        let field = field.into();
        let value = self
            .take_raw(&field)
            .ok_or_else(|| McpToolError::missing_field(field.clone()))?;
        decode_usize_field(field, value)
    }

    /// Decode a present non-null `usize` field.
    ///
    /// Returns `Ok(None)` when the field is absent. A present JSON `null`
    /// remains an error.
    pub fn take_usize(&mut self, field: impl Into<String>) -> Result<Option<usize>, McpToolError> {
        let field = field.into();
        self.take_raw(&field)
            .map(|value| decode_usize_field(field, value))
            .transpose()
    }

    /// Decode an optional `usize` field where both absence and JSON null mean
    /// `None`.
    pub fn take_optional_usize(
        &mut self,
        field: impl Into<String>,
    ) -> Result<Option<usize>, McpToolError> {
        let field = field.into();
        self.take_raw(&field)
            .map(|value| decode_optional_usize_field(field, value))
            .transpose()
            .map(Option::flatten)
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

/// JSON Schema metadata for typed MCP argument values.
///
/// Runtime integrations use this trait when a Rust type can describe its
/// structured JSON shape more precisely than [`McpInput`]'s coarse object
/// marker. Type aliases inherit the implementation of their target type, and
/// app-owned newtypes or structs can derive this trait with
/// `#[derive(component_shape_mcp::McpJsonSchema)]`.
pub trait McpJsonSchema {
    fn json_schema() -> Value;
}

/// Typed `{ "min": ..., "max": ... }` range argument used by MCP tools.
///
/// Custom table filter shapes can use this as their `RawValue` when they want
/// derive-driven MCP decoding for range-shaped arguments.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
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
    fn json_schema() -> Value {
        range_schema(T::json_schema())
    }
}

impl McpJsonSchema for bool {
    fn json_schema() -> Value {
        schema_for_primitive(McpPrimitiveKind::Boolean)
    }
}

macro_rules! impl_integer_schema {
    ($($ty:ty),* $(,)?) => {
        $(
            impl McpJsonSchema for $ty {
                fn json_schema() -> Value {
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
                fn json_schema() -> Value {
                    schema_for_primitive(McpPrimitiveKind::Number)
                }
            }
        )*
    };
}

impl_number_schema!(f32, f64);

#[cfg(feature = "rust_decimal")]
impl McpJsonSchema for rust_decimal::Decimal {
    fn json_schema() -> Value {
        schema_for_primitive(McpPrimitiveKind::Decimal)
    }
}

impl McpJsonSchema for String {
    fn json_schema() -> Value {
        schema_for_primitive(McpPrimitiveKind::String)
    }
}

impl McpJsonSchema for str {
    fn json_schema() -> Value {
        schema_for_primitive(McpPrimitiveKind::String)
    }
}

impl McpJsonSchema for char {
    fn json_schema() -> Value {
        schema_for_primitive(McpPrimitiveKind::String)
    }
}

impl McpJsonSchema for PathBuf {
    fn json_schema() -> Value {
        schema_for_primitive(McpPrimitiveKind::String)
    }
}

impl McpJsonSchema for Path {
    fn json_schema() -> Value {
        schema_for_primitive(McpPrimitiveKind::String)
    }
}

impl McpJsonSchema for Value {
    fn json_schema() -> Value {
        json!({})
    }
}

#[cfg(feature = "chrono")]
impl McpJsonSchema for chrono::NaiveDate {
    fn json_schema() -> Value {
        schema_for_primitive(McpPrimitiveKind::Date)
    }
}

#[cfg(feature = "chrono")]
impl McpJsonSchema for chrono::NaiveDateTime {
    fn json_schema() -> Value {
        schema_for_primitive(McpPrimitiveKind::DateTime)
    }
}

#[cfg(feature = "chrono")]
impl<Tz> McpJsonSchema for chrono::DateTime<Tz>
where
    Tz: chrono::TimeZone,
{
    fn json_schema() -> Value {
        schema_for_primitive(McpPrimitiveKind::DateTime)
    }
}

impl<T> McpJsonSchema for Option<T>
where
    T: McpJsonSchema,
{
    fn json_schema() -> Value {
        nullable_schema(T::json_schema())
    }
}

impl<T> McpJsonSchema for Vec<T>
where
    T: McpJsonSchema,
{
    fn json_schema() -> Value {
        array_schema(T::json_schema())
    }
}

impl<T> McpJsonSchema for [T]
where
    T: McpJsonSchema,
{
    fn json_schema() -> Value {
        array_schema(T::json_schema())
    }
}

impl<T, const N: usize> McpJsonSchema for [T; N]
where
    T: McpJsonSchema,
{
    fn json_schema() -> Value {
        let mut schema = array_schema(T::json_schema());
        if let Some(object) = schema.as_object_mut() {
            object.insert("minItems".to_string(), json!(N));
            object.insert("maxItems".to_string(), json!(N));
        }
        schema
    }
}

impl<T> McpJsonSchema for BTreeSet<T>
where
    T: McpJsonSchema,
{
    fn json_schema() -> Value {
        unique_array_schema(T::json_schema())
    }
}

impl<T> McpJsonSchema for HashSet<T>
where
    T: Eq + Hash + McpJsonSchema,
{
    fn json_schema() -> Value {
        unique_array_schema(T::json_schema())
    }
}

impl<T> McpJsonSchema for BTreeMap<String, T>
where
    T: McpJsonSchema,
{
    fn json_schema() -> Value {
        string_keyed_object_schema(T::json_schema())
    }
}

impl<T> McpJsonSchema for HashMap<String, T>
where
    T: McpJsonSchema,
{
    fn json_schema() -> Value {
        string_keyed_object_schema(T::json_schema())
    }
}

impl<T> McpJsonSchema for &T
where
    T: ?Sized + McpJsonSchema,
{
    fn json_schema() -> Value {
        T::json_schema()
    }
}

impl<'a, T> McpJsonSchema for Cow<'a, T>
where
    T: ?Sized + ToOwned + McpJsonSchema,
{
    fn json_schema() -> Value {
        T::json_schema()
    }
}

pub fn array_schema(item_schema: Value) -> Value {
    json!({
        "type": "array",
        "items": item_schema
    })
}

pub fn unique_array_schema(item_schema: Value) -> Value {
    let mut schema = array_schema(item_schema);
    if let Some(object) = schema.as_object_mut() {
        object.insert("uniqueItems".to_string(), Value::Bool(true));
    }
    schema
}

pub fn string_keyed_object_schema(value_schema: Value) -> Value {
    json!({
        "type": "object",
        "additionalProperties": value_schema
    })
}

pub fn object_schema<I, S>(properties: Map<String, Value>, required: I) -> Value
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let required = required
        .into_iter()
        .map(|field| Value::String(field.into()))
        .collect::<Vec<_>>();

    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

impl<T> McpJsonSchema for Box<T>
where
    T: McpJsonSchema,
{
    fn json_schema() -> Value {
        T::json_schema()
    }
}

impl McpJsonSchema for () {
    fn json_schema() -> Value {
        json!({ "type": "null" })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum McpToolError {
    #[error("tool arguments must be a JSON object")]
    ArgumentsMustBeObject,
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("field `{field}` does not accept null")]
    UnexpectedNull { field: String },
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

pub fn decode_present_field<T>(field: impl Into<String>, value: Value) -> Result<T, McpToolError>
where
    T: DeserializeOwned,
{
    let field = field.into();
    if value.is_null() {
        return Err(McpToolError::UnexpectedNull { field });
    }
    serde_json::from_value(value).map_err(|error| McpToolError::DecodeField {
        field,
        message: error.to_string(),
    })
}

pub fn decode_optional_field<T>(
    field: impl Into<String>,
    value: Value,
) -> Result<Option<T>, McpToolError>
where
    T: DeserializeOwned,
{
    let field = field.into();
    if value.is_null() {
        return Ok(None);
    }
    decode_present_field(field, value).map(Some)
}

pub fn decode_usize_field(field: impl Into<String>, value: Value) -> Result<usize, McpToolError> {
    let field = field.into();
    if value.is_null() {
        return Err(McpToolError::UnexpectedNull { field });
    }

    let Some(raw) = value.as_u64() else {
        return Err(McpToolError::decode(
            field,
            "expected a non-negative integer",
        ));
    };

    usize::try_from(raw)
        .map_err(|_| McpToolError::decode(field, "integer is too large for this platform's usize"))
}

pub fn decode_optional_usize_field(
    field: impl Into<String>,
    value: Value,
) -> Result<Option<usize>, McpToolError> {
    if value.is_null() {
        return Ok(None);
    }
    decode_usize_field(field, value).map(Some)
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

    pub fn tool_async<Call, Fut>(self, definition: ToolDefinition, call: Call) -> Self
    where
        Call: Fn(McpToolCall) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToolCallResult> + Send + 'static,
    {
        self.register(move |server| server.add_tool_async(definition, call))
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
    input_schema: Value,
    output_schema: Option<Value>,
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

pub fn schema_object(label: impl Into<String>, schema: Value) -> Result<JsonObject, McpToolError> {
    match schema {
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

pub fn schema_for_input(input: McpInput) -> Value {
    match input.input_shape() {
        McpInputShape::Unsupported => json!({ "not": {} }),
        McpInputShape::Scalar(kind) => schema_for_primitive(kind),
        McpInputShape::List(kind) => array_schema(schema_for_primitive(kind)),
        McpInputShape::Set(kind) => unique_array_schema(schema_for_primitive(kind)),
        McpInputShape::Range(kind) => range_schema(schema_for_primitive(kind)),
        McpInputShape::Object => json!({ "type": "object" }),
    }
}

pub fn range_schema(bound_schema: Value) -> Value {
    json!({
        "type": "object",
        "properties": {
            "min": nullable_schema(bound_schema.clone()),
            "max": nullable_schema(bound_schema)
        },
        "additionalProperties": false
    })
}

pub fn schema_for_primitive(kind: McpPrimitiveKind) -> Value {
    match kind {
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
    }
}

pub fn nullable_schema(schema: Value) -> Value {
    json!({
        "anyOf": [
            schema,
            { "type": "null" }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::{McpInput, McpJsonSchema as _, McpRange, McpServer, ToolDefinition};
    use serde_json::{Value, json};

    #[test]
    fn schema_for_input_maps_range_dates() {
        let schema = super::schema_for_input(McpInput::date_range());

        assert_eq!(schema["properties"]["min"]["anyOf"][0]["format"], "date");
        assert_eq!(schema["properties"]["max"]["anyOf"][1]["type"], "null");
    }

    #[test]
    fn schema_for_input_distinguishes_any_from_unsupported() {
        assert_eq!(super::schema_for_input(McpInput::any()), json!({}));
        assert_eq!(
            super::schema_for_input(McpInput::unsupported()),
            json!({ "not": {} })
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
            serde_json::from_value::<McpRange<u32>>(json!({
                "min": 1,
                "max": null
            }))
            .expect("range object should decode"),
            McpRange::new(Some(1), None)
        );
    }

    #[test]
    fn json_schema_derive_builds_object_schema() {
        #[derive(crate::McpJsonSchema)]
        #[allow(dead_code)]
        struct SearchArgs {
            #[mcp(rename = "q", description = "Search text")]
            query: String,
            page: Option<u32>,
            #[serde(skip)]
            internal: String,
        }

        let schema = SearchArgs::json_schema();

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["q"]["type"], "string");
        assert_eq!(schema["properties"]["q"]["description"], "Search text");
        assert_eq!(schema["properties"]["page"]["anyOf"][0]["type"], "integer");
        assert_eq!(schema["required"], json!(["q"]));
        assert!(schema["properties"].get("internal").is_none());
    }

    #[test]
    fn json_schema_derive_builds_enum_schema() {
        #[derive(crate::McpJsonSchema)]
        #[allow(dead_code)]
        #[serde(rename_all = "kebab-case")]
        enum IssueState {
            Open,
            #[serde(alias = "reviewing")]
            InReview,
            #[mcp(rename = "done")]
            Closed,
            #[serde(other)]
            Unknown,
        }

        let schema = IssueState::json_schema();

        assert_eq!(schema["type"], "string");
        assert_eq!(
            schema["enum"],
            json!(["open", "in-review", "reviewing", "done"])
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
        assert!(super::tool_definition("", None, None, json!({ "type": "object" }), None).is_err());
        assert!(
            super::tool_definition("bad name", None, None, json!({ "type": "object" }), None)
                .is_err()
        );
        assert!(
            super::tool_definition(
                "valid_name",
                Some("".to_string()),
                None,
                json!({ "type": "object" }),
                None,
            )
            .is_err()
        );
        assert!(super::tool_definition("valid", None, None, json!("bad"), None).is_err());
        assert!(
            super::tool_definition(
                "valid",
                None,
                None,
                json!({ "type": "object" }),
                Some(json!(false)),
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
            json!({ "type": "object" }),
            None,
        )
        .expect("tool definition should build");

        assert_eq!(name.to_string(), "good-name");
    }

    #[test]
    fn schema_object_rejects_non_object_values() {
        let object = super::schema_object("input_schema", json!({ "type": "object" }))
            .expect("schema should be accepted");
        assert_eq!(object["type"], "object");

        let error = super::schema_object("input_schema", json!(null))
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
    fn mcp_arguments_decodes_and_rejects_unknown_fields() {
        let call = super::McpToolCall::from_value(Some(json!({
            "name": "Ada",
            "nickname": null,
            "limit": 2,
            "unused": true
        })))
        .expect("object arguments are accepted");
        let mut arguments = call.into_arguments();

        let name = arguments
            .take_required::<String>("name")
            .expect("name should decode");
        let nickname = arguments
            .take_nullable::<String>("nickname")
            .expect("nickname should decode");
        let limit = arguments.take_usize("limit").expect("limit should decode");

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
    fn decode_usize_field_rejects_negative_values() {
        assert_eq!(
            super::decode_usize_field("limit", json!(10)).expect("usize should decode"),
            10
        );

        let error =
            super::decode_usize_field("limit", json!(-1)).expect_err("negative should fail");
        assert_eq!(
            error,
            super::McpToolError::DecodeField {
                field: "limit".to_string(),
                message: "expected a non-negative integer".to_string(),
            }
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
                super::tool_definition("echo", None, None, json!({ "type": "object" }), None)
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
                super::tool_definition("echo", None, None, json!({ "type": "object" }), None)
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
                        json!({ "type": "object" }),
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
                super::tool_definition("echo", None, None, json!({ "type": "object" }), None)
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
                super::tool_definition("echo", None, None, json!({ "type": "object" }), None)
                    .expect("tool definition should build"),
                |call| {
                    super::tool_structured_result(Value::Object(call.into_arguments().into_inner()))
                },
            )
            .expect("tool should register");

        let error = match server.add_tool(
            super::tool_definition("echo", None, None, json!({ "type": "object" }), None)
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
