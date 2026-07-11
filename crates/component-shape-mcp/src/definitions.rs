use super::*;

/// Build and validate an MCP tool definition.
///
/// # Errors
///
/// Returns [`McpToolError`] when the tool metadata is invalid or when the input
/// or output schema is not an object-shaped MCP tool schema.
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

/// Build and validate a typed MCP tool definition from its input type.
///
/// # Errors
///
/// Returns [`McpToolError`] when the tool metadata is invalid or when the input
/// type's generated schema or output schema is not accepted.
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

/// Build and validate an MCP tool definition with tool annotations.
///
/// # Errors
///
/// Returns [`McpToolError`] when the annotations, metadata, input schema, or
/// output schema are invalid.
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

/// Build and validate a typed MCP tool definition with tool annotations.
///
/// # Errors
///
/// Returns [`McpToolError`] when the annotations, metadata, input schema, or
/// output schema are invalid.
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

/// Build and validate an MCP tool definition from generated metadata.
///
/// # Errors
///
/// Returns [`McpToolError`] when the generated metadata, input schema, or output
/// schema is invalid.
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

/// Build and validate a typed MCP tool definition from generated metadata.
///
/// # Errors
///
/// Returns [`McpToolError`] when the generated metadata, typed input schema, or
/// output schema is invalid.
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
///
/// # Errors
///
/// Returns [`McpToolError`] when the resource URI, name, title, description, or
/// MIME type is invalid.
pub fn resource_definition(
    uri: impl Into<String>,
    name: impl Into<String>,
    title: Option<String>,
    description: Option<String>,
    mime_type: Option<String>,
) -> Result<ResourceDefinition, McpToolError> {
    let mut resource = Resource::new(uri, name);
    resource.title = title;
    resource.description = description;
    resource.mime_type = mime_type;
    validate_resource_definition(&resource)?;
    Ok(resource)
}

/// Build and validate an MCP resource template definition.
///
/// # Errors
///
/// Returns [`McpToolError`] when the template URI, name, title, description, or
/// MIME type is invalid.
pub fn resource_template_definition(
    uri_template: impl Into<String>,
    name: impl Into<String>,
    title: Option<String>,
    description: Option<String>,
    mime_type: Option<String>,
) -> Result<ResourceTemplateDefinition, McpToolError> {
    let mut template = ResourceTemplate::new(uri_template, name);
    template.title = title;
    template.description = description;
    template.mime_type = mime_type;
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
///
/// # Errors
///
/// Returns [`McpToolError`] when `value` cannot be serialized as JSON.
pub fn json_resource_result(
    uri: impl Into<String>,
    value: &Value,
) -> Result<ReadResourceResult, McpToolError> {
    let text = serde_json::to_string_pretty(value).map_err(|error| {
        McpToolError::conversion(format!("failed to encode JSON resource: {error}"))
    })?;
    Ok(text_resource_result(uri, text, "application/json"))
}

/// Validated static JSON resource ready to register on an [`McpServer`].
#[derive(Clone, Debug)]
pub struct McpJsonResourceSpec {
    uri: String,
    definition: ResourceDefinition,
    value: Arc<Value>,
}

impl McpJsonResourceSpec {
    /// Build and validate an `application/json` resource backed by `value`.
    ///
    /// # Errors
    ///
    /// Returns [`McpToolError`] when the resource URI, name, title, description,
    /// or MIME type is invalid.
    pub fn new(
        uri: impl Into<String>,
        name: impl Into<String>,
        title: Option<String>,
        description: Option<String>,
        value: Value,
    ) -> Result<Self, McpToolError> {
        let uri = uri.into();
        let definition = resource_definition(
            uri.clone(),
            name,
            title,
            description,
            Some("application/json".to_string()),
        )?;
        Ok(Self {
            uri,
            definition,
            value: Arc::new(value),
        })
    }

    /// Concrete resource URI.
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Resource definition advertised by `resources/list`.
    pub fn definition(&self) -> &ResourceDefinition {
        &self.definition
    }

    /// Consume the spec and return its resource definition.
    pub fn into_definition(self) -> ResourceDefinition {
        self.definition
    }
}

/// Return resource definitions for a distinct set of generated JSON resources.
///
/// # Errors
///
/// Returns [`McpToolError::DuplicateResource`] when `specs` contains the same
/// URI more than once.
pub fn json_resource_definitions(
    specs: &[McpJsonResourceSpec],
) -> Result<Vec<ResourceDefinition>, McpToolError> {
    ensure_json_resource_specs_distinct(specs)?;
    Ok(specs.iter().map(|spec| spec.definition.clone()).collect())
}

/// Register generated JSON resources, failing if any URI is duplicated.
///
/// # Errors
///
/// Returns [`McpToolError`] when any spec URI is duplicated within the batch,
/// is already registered on `server`, or cannot be registered by the server.
pub fn register_json_resource_specs(
    server: &mut McpServer,
    specs: Vec<McpJsonResourceSpec>,
) -> Result<(), McpToolError> {
    ensure_json_resource_specs_available(server, &specs)?;
    for spec in specs {
        let uri = spec.uri.clone();
        let value = Arc::clone(&spec.value);
        server.add_resource(spec.definition, move || {
            json_resource_result(uri.clone(), value.as_ref())
                .expect("generated JSON resource should encode")
        })?;
    }
    Ok(())
}

/// Register generated JSON resources unless every URI is already present.
///
/// If only some resources are present, this fails with a duplicate-resource
/// setup error instead of silently publishing a partial set.
///
/// # Errors
///
/// Returns [`McpToolError`] when the resource set is partially registered or
/// contains duplicate URIs.
pub fn register_json_resource_specs_if_missing(
    server: &mut McpServer,
    specs: Vec<McpJsonResourceSpec>,
) -> Result<(), McpToolError> {
    if specs
        .iter()
        .all(|spec| server.contains_resource(spec.uri()))
    {
        return Ok(());
    }
    register_json_resource_specs(server, specs)
}

/// Ensure generated JSON resource URIs are unique and not already registered.
///
/// # Errors
///
/// Returns [`McpToolError::DuplicateResource`] when a spec URI is duplicated
/// within the batch or is already registered on `server`.
pub fn ensure_json_resource_specs_available(
    server: &McpServer,
    specs: &[McpJsonResourceSpec],
) -> Result<(), McpToolError> {
    ensure_json_resource_specs_distinct(specs)?;
    for spec in specs {
        if server.contains_resource(spec.uri()) {
            return Err(McpToolError::duplicate_resource(spec.uri().to_string()));
        }
    }
    Ok(())
}

/// Ensure generated JSON resource URIs are unique within one batch.
///
/// # Errors
///
/// Returns [`McpToolError::DuplicateResource`] when a spec URI appears more
/// than once.
pub fn ensure_json_resource_specs_distinct(
    specs: &[McpJsonResourceSpec],
) -> Result<(), McpToolError> {
    let mut seen = BTreeSet::new();
    for spec in specs {
        if !seen.insert(spec.uri()) {
            return Err(McpToolError::duplicate_resource(spec.uri().to_string()));
        }
    }
    Ok(())
}

/// Build and validate an MCP prompt definition.
///
/// # Errors
///
/// Returns [`McpToolError`] when the prompt name, title, description, or
/// argument metadata is invalid.
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
    let mut result = GetPromptResult::new(vec![PromptMessage::new_text(Role::User, text)]);
    result.description = description;
    result
}

/// Validate an MCP tool name accepted by generated integrations.
///
/// # Errors
///
/// Returns [`McpToolError`] when `name` is outside the generated tool-name
/// subset.
pub fn validate_tool_name(name: &str) -> Result<(), McpToolError> {
    component_shape::validate_mcp_tool_name(name)
        .map_err(|error| McpToolError::validation(error.to_string()))
}

/// Validate human-readable MCP tool metadata text.
///
/// # Errors
///
/// Returns [`McpToolError`] when `value` is empty or contains only whitespace.
pub fn validate_tool_metadata_text(label: &str, value: &str) -> Result<(), McpToolError> {
    component_shape::validate_mcp_tool_metadata_text(label, value)
        .map_err(|error| McpToolError::validation(error.to_string()))
}

/// Validate MCP tool annotations accepted by generated integrations.
///
/// # Errors
///
/// Returns [`McpToolError`] when annotation hints conflict or annotation text is
/// invalid.
pub fn validate_tool_annotations(annotations: &McpToolAnnotations) -> Result<(), McpToolError> {
    validate_tool_annotation_hints(annotations.read_only_hint, annotations.destructive_hint)?;
    if let Some(title) = annotations.title.as_deref() {
        validate_tool_metadata_text("annotation title", title)?;
    }
    Ok(())
}

/// Validate an MCP tool definition accepted by this shared server.
///
/// # Errors
///
/// Returns [`McpToolError`] when the name, schemas, title, description,
/// annotations, or icons are invalid.
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
///
/// # Errors
///
/// Returns [`McpToolError`] when the URI, name, title, description, or MIME type
/// is invalid.
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
///
/// # Errors
///
/// Returns [`McpToolError`] when the URI template, name, title, description, or
/// MIME type is invalid.
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
///
/// # Errors
///
/// Returns [`McpToolError`] when the prompt name, title, description, or
/// argument metadata is invalid.
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

pub(crate) fn validate_required_metadata_text(
    label: &str,
    value: &str,
) -> Result<(), McpToolError> {
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

pub(crate) fn validate_tool_annotation_hints(
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

/// Converts an MCP schema wrapper into a JSON object.
///
/// # Errors
///
/// Returns [`McpToolError::InvalidSchema`] when `schema` is not a JSON object.
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
