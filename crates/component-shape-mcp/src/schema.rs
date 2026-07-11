use super::*;

/// Primitive JSON Schema root types supported by typed schema builders.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McpSchemaType {
    Null,
    Boolean,
    Integer,
    Number,
    String,
    Array,
    Object,
}

impl McpSchemaType {
    /// Returns the JSON Schema type keyword value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Number => "number",
            Self::String => "string",
            Self::Array => "array",
            Self::Object => "object",
        }
    }
}

/// Standard string formats emitted by component-shape MCP schemas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McpStringFormat {
    Date,
    DateTime,
}

impl McpStringFormat {
    /// Returns the JSON Schema string format keyword value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::DateTime => "date-time",
        }
    }
}

/// JSON number used by numeric JSON Schema keywords.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpSchemaNumber(Number);

impl McpSchemaNumber {
    /// Converts a finite `f64` into a JSON Schema number.
    pub fn from_f64(value: f64) -> Option<Self> {
        Number::from_f64(value).map(Self)
    }

    /// Converts this number into a JSON value.
    pub fn into_value(self) -> Value {
        Value::Number(self.0)
    }
}

macro_rules! impl_schema_number {
    ($($ty:ty),* $(,)?) => {
        $(
            impl From<$ty> for McpSchemaNumber {
                fn from(value: $ty) -> Self {
                    Self(Number::from(value))
                }
            }
        )*
    };
}

impl_schema_number!(i8, i16, i32, i64, u8, u16, u32, u64);

impl From<isize> for McpSchemaNumber {
    fn from(value: isize) -> Self {
        Self(Number::from(value as i64))
    }
}

impl From<usize> for McpSchemaNumber {
    fn from(value: usize) -> Self {
        Self(Number::from(value as u64))
    }
}

/// Value accepted by the JSON Schema `additionalProperties` keyword.
pub enum McpAdditionalProperties {
    /// Whether additional properties are allowed.
    Allowed(bool),
    /// Schema applied to additional property values.
    Schema(McpSchema),
}

impl From<bool> for McpAdditionalProperties {
    fn from(value: bool) -> Self {
        Self::Allowed(value)
    }
}

impl From<McpSchema> for McpAdditionalProperties {
    fn from(schema: McpSchema) -> Self {
        Self::Schema(schema)
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

    pub fn any() -> Self {
        Self(Value::Object(Map::new()))
    }

    pub fn impossible() -> Self {
        Self::any().with_not(Self::any())
    }

    pub fn typed(schema_type: McpSchemaType) -> Self {
        let mut object = Map::new();
        object.insert(
            "type".to_string(),
            Value::String(schema_type.as_str().to_string()),
        );
        Self(Value::Object(object))
    }

    pub fn null() -> Self {
        Self::typed(McpSchemaType::Null)
    }

    pub fn boolean() -> Self {
        Self::typed(McpSchemaType::Boolean)
    }

    pub fn integer() -> Self {
        Self::typed(McpSchemaType::Integer)
    }

    pub fn number() -> Self {
        Self::typed(McpSchemaType::Number)
    }

    pub fn string() -> Self {
        Self::typed(McpSchemaType::String)
    }

    pub fn array(item_schema: McpSchema) -> Self {
        Self::typed(McpSchemaType::Array).with_items(item_schema)
    }

    pub fn object() -> Self {
        Self::typed(McpSchemaType::Object)
    }

    pub fn any_of<I>(schemas: I) -> Self
    where
        I: IntoIterator<Item = McpSchema>,
    {
        let mut schema = Self::any();
        schema.set_any_of(schemas);
        schema
    }

    pub fn one_of<I>(schemas: I) -> Self
    where
        I: IntoIterator<Item = McpSchema>,
    {
        let mut schema = Self::any();
        schema.set_one_of(schemas);
        schema
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.set_description(description);
        self
    }

    pub fn with_extension(mut self, key: impl Into<String>, value: Value) -> Self {
        self.set_extension(key, value);
        self
    }

    pub fn with_format(mut self, format: McpStringFormat) -> Self {
        self.set_format(format);
        self
    }

    pub fn with_minimum(mut self, minimum: impl Into<McpSchemaNumber>) -> Self {
        self.set_minimum(minimum);
        self
    }

    pub fn with_min_items(mut self, min_items: usize) -> Self {
        self.set_min_items(min_items);
        self
    }

    pub fn with_max_items(mut self, max_items: usize) -> Self {
        self.set_max_items(max_items);
        self
    }

    pub fn with_unique_items(mut self, unique_items: bool) -> Self {
        self.set_unique_items(unique_items);
        self
    }

    pub fn with_default(mut self, value: impl Into<Value>) -> Self {
        self.set_default(value);
        self
    }

    pub fn with_const(mut self, value: impl Into<Value>) -> Self {
        self.set_const(value);
        self
    }

    pub fn with_enum_values<I, V>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<Value>,
    {
        self.set_enum_values(values);
        self
    }

    pub fn with_items(mut self, item_schema: McpSchema) -> Self {
        self.set_items(item_schema);
        self
    }

    pub fn with_prefix_items<I>(mut self, item_schemas: I) -> Self
    where
        I: IntoIterator<Item = McpSchema>,
    {
        self.set_prefix_items(item_schemas);
        self
    }

    pub fn with_properties(mut self, properties: McpSchemaProperties) -> Self {
        self.set_properties(properties);
        self
    }

    pub fn with_required<I, S>(mut self, required: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.set_required(required);
        self
    }

    pub fn with_additional_properties(
        mut self,
        additional_properties: impl Into<McpAdditionalProperties>,
    ) -> Self {
        self.set_additional_properties(additional_properties);
        self
    }

    pub fn with_not(mut self, schema: McpSchema) -> Self {
        self.set_not(schema);
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

    pub fn set_format(&mut self, format: McpStringFormat) {
        self.set_extension("format", Value::String(format.as_str().to_string()));
    }

    pub fn set_minimum(&mut self, minimum: impl Into<McpSchemaNumber>) {
        self.set_extension("minimum", minimum.into().into_value());
    }

    pub fn set_min_items(&mut self, min_items: usize) {
        self.set_extension("minItems", Value::Number(Number::from(min_items as u64)));
    }

    pub fn set_max_items(&mut self, max_items: usize) {
        self.set_extension("maxItems", Value::Number(Number::from(max_items as u64)));
    }

    pub fn set_unique_items(&mut self, unique_items: bool) {
        self.set_extension("uniqueItems", Value::Bool(unique_items));
    }

    pub fn set_default(&mut self, value: impl Into<Value>) {
        self.set_extension("default", value.into());
    }

    pub fn set_const(&mut self, value: impl Into<Value>) {
        self.set_extension("const", value.into());
    }

    pub fn set_enum_values<I, V>(&mut self, values: I)
    where
        I: IntoIterator<Item = V>,
        V: Into<Value>,
    {
        self.set_extension(
            "enum",
            Value::Array(values.into_iter().map(Into::into).collect()),
        );
    }

    pub fn set_items(&mut self, item_schema: McpSchema) {
        self.set_extension("items", item_schema.into_value());
    }

    pub fn set_prefix_items<I>(&mut self, item_schemas: I)
    where
        I: IntoIterator<Item = McpSchema>,
    {
        self.set_extension(
            "prefixItems",
            Value::Array(
                item_schemas
                    .into_iter()
                    .map(McpSchema::into_value)
                    .collect(),
            ),
        );
    }

    pub fn set_properties(&mut self, properties: McpSchemaProperties) {
        self.set_extension(
            "properties",
            Value::Object(
                properties
                    .into_iter()
                    .map(|(name, schema)| (name, schema.into_value()))
                    .collect(),
            ),
        );
    }

    pub fn set_required<I, S>(&mut self, required: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.set_extension(
            "required",
            Value::Array(
                required
                    .into_iter()
                    .map(|field| Value::String(field.into()))
                    .collect(),
            ),
        );
    }

    pub fn set_additional_properties(
        &mut self,
        additional_properties: impl Into<McpAdditionalProperties>,
    ) {
        let value = match additional_properties.into() {
            McpAdditionalProperties::Allowed(value) => Value::Bool(value),
            McpAdditionalProperties::Schema(schema) => schema.into_value(),
        };
        self.set_extension("additionalProperties", value);
    }

    pub fn set_any_of<I>(&mut self, schemas: I)
    where
        I: IntoIterator<Item = McpSchema>,
    {
        self.set_extension(
            "anyOf",
            Value::Array(schemas.into_iter().map(McpSchema::into_value).collect()),
        );
    }

    pub fn set_one_of<I>(&mut self, schemas: I)
    where
        I: IntoIterator<Item = McpSchema>,
    {
        self.set_extension(
            "oneOf",
            Value::Array(schemas.into_iter().map(McpSchema::into_value).collect()),
        );
    }

    pub fn set_not(&mut self, schema: McpSchema) {
        self.set_extension("not", schema.into_value());
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
    /// Returns the JSON Schema for this type.
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
    /// Creates an unconstrained MCP value from raw JSON.
    pub fn new(value: Value) -> Self {
        Self(value)
    }

    /// Returns the raw JSON value.
    pub fn as_value(&self) -> &Value {
        &self.0
    }

    /// Returns the raw JSON value.
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
        McpSchema::any()
    }
}

impl McpJsonSchema for Value {
    fn json_schema() -> McpSchema {
        McpSchema::any()
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
    /// Returns the input object's JSON Schema.
    fn input_schema() -> McpSchema;

    /// Decodes a normalized tool call into this input type.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError`] when required fields are missing, unknown fields
    /// remain, a field value fails schema validation, or serde rejects a field.
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
    pub(crate) fn from_definition_unchecked(definition: ToolDefinition) -> Self {
        Self {
            definition,
            _input: PhantomData,
        }
    }

    /// Returns the wrapped MCP tool definition.
    pub fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    /// Returns the wrapped MCP tool definition mutably.
    pub fn definition_mut(&mut self) -> &mut ToolDefinition {
        &mut self.definition
    }

    /// Returns the wrapped MCP tool definition.
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
    /// Returns the schema used for this value inside an MCP argument object.
    fn tool_value_schema() -> McpSchema;

    /// Decodes one JSON field value into this type.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError`] when the value is unexpectedly null, fails
    /// schema validation, or serde rejects the normalized JSON value.
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
    /// Inclusive lower range bound.
    pub min: Option<T>,
    /// Inclusive upper range bound.
    pub max: Option<T>,
}

impl<T> McpRange<T> {
    /// Creates a range argument from optional bounds.
    pub fn new(min: Option<T>, max: Option<T>) -> Self {
        Self { min, max }
    }

    /// Returns this range as `(min, max)`.
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
            object.insert("minItems".to_string(), Value::Number((N as u64).into()));
            object.insert("maxItems".to_string(), Value::Number((N as u64).into()));
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

/// Build an array schema for items of `item_schema`.
pub fn array_schema(item_schema: McpSchema) -> McpSchema {
    McpSchema::array(item_schema)
}

/// Build a unique-array schema for items of `item_schema`.
pub fn unique_array_schema(item_schema: McpSchema) -> McpSchema {
    array_schema(item_schema).with_unique_items(true)
}

/// Build a fixed-length tuple schema.
pub fn tuple_schema<I>(item_schemas: I) -> McpSchema
where
    I: IntoIterator<Item = McpSchema>,
{
    let prefix_items = item_schemas.into_iter().collect::<Vec<_>>();
    let len = prefix_items.len();

    McpSchema::typed(McpSchemaType::Array)
        .with_prefix_items(prefix_items)
        .with_min_items(len)
        .with_max_items(len)
}

/// Build an object schema whose string keys all use `value_schema`.
pub fn string_keyed_object_schema(value_schema: McpSchema) -> McpSchema {
    McpSchema::object().with_additional_properties(value_schema)
}

/// Build a closed object schema from properties and required field names.
pub fn object_schema<I, S>(properties: McpSchemaProperties, required: I) -> McpSchema
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    McpSchema::object()
        .with_properties(properties)
        .with_required(required)
        .with_additional_properties(false)
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
        McpSchema::null()
    }
}
