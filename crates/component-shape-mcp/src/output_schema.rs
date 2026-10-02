use super::*;

/// Compile a self-contained output schema without any network or filesystem access.
pub(crate) fn compile_output_schema(
    schema: &JsonObject,
) -> Result<jsonschema::Validator, McpToolError> {
    // Set a retriever even with default features disabled: another dependency can
    // enable jsonschema's HTTP or file features through Cargo feature unification.
    jsonschema::options()
        .with_retriever(DenyExternalSchemas)
        .should_validate_formats(false)
        .build(&Value::Object(schema.clone()))
        .map_err(|error| McpToolError::InvalidSchema {
            label: "output_schema".into(),
            message: error.to_string(),
        })
}

struct DenyExternalSchemas;

impl jsonschema::Retrieve for DenyExternalSchemas {
    fn retrieve(
        &self,
        _uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err(
            "external schema retrieval is disabled; bundle referenced schemas in output_schema"
                .into(),
        )
    }
}
