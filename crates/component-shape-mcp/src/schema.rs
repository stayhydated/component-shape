use super::*;

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
    pub(crate) fn from_definition_unchecked(definition: ToolDefinition) -> Self {
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
