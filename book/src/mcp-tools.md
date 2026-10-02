# Tools and results

Register a typed tool so its published input schema and Rust handler argument
remain paired by type.

```rust
# extern crate component_shape_mcp;
# fn main() -> Result<(), component_shape_mcp::McpToolError> {
#[derive(component_shape_mcp::McpToolInput)]
#[serde(rename_all = "camelCase")]
# #[mcp(crate = component_shape_mcp)]
struct SearchArgs {
    #[serde(alias = "q")]
    query: String,
    page_size: Option<u32>,
}

let mut tools = component_shape_mcp::McpToolRegistry::new();

let tool = component_shape_mcp::tool_definition_for_input::<SearchArgs>(
    "search",
    Some("Search".to_owned()),
    None,
    None,
)?;

tools.add_typed_tool(tool, |args: SearchArgs| {
    component_shape_mcp::tool_structured_result(
        component_shape_mcp::serde_json::json!({ "query": args.query }),
    )
})?;

let server = component_shape_mcp::McpServer::from_tool_registry(
    "search-server",
    "1.0.0",
    tools,
);
# Ok(())
# }
```

This creates a server with one registered tool. It can now serve requests
using the transport described in [Servers and stdio](mcp-server.md).

Use the async registration methods when the handler returns a future. Use an
untyped tool only when the integration must decode a dynamic argument set.

## Add metadata

Store application-owned names, titles, descriptions, icons, and MCP annotation
hints in `McpToolMetadata`. Use
`tool_definition_for_input_with_metadata` when the metadata and typed input
should be constructed together.

Registration validates names, required text, icons, schemas, duplicates, and
incompatible annotation hints. Raw custom tool definitions registered through
`add_tool` or `add_tool_async` receive the same checks.

## Return structured results

Use `tool_structured_result` for a successful structured response. If the tool
publishes an output schema, its successful `structured_content` must match
that schema.

The registry compiles each output schema at registration and shares the
validator across calls and registry clones. Direct calls, async calls, and MCP
protocol calls all validate successful results. Invalid schemas fail definition
construction or registration with `InvalidSchema`; missing or invalid structured
content produces `InvalidToolOutput`. Handler error results bypass output
validation. Error kinds and structured fields are stable; diagnostic text may
change with the validator.

Output schemas use JSON Schema Draft 2020-12 when `$schema` is absent. Explicit
Draft 4, 6, 7, 2019-09, and 2020-12 dialects are supported; unknown dialects are
rejected. `format` remains annotation-only, including application formats such
as `language-tag`. References must resolve within the supplied schema, including
bundled `$defs` and identifiers. External retrieval is disabled for every URI
scheme, including HTTP and local files, even if another dependency enables the
validator's retrieval features.

Return the exact advertised property and enum names. The input decoder's aliases
and `x-mcp*` metadata do not rename or normalize output. If a type serializes with
different names from its deserialize-facing `McpJsonSchema`, supply an output
schema that describes the serialized representation. Application data containing
keys such as `$ref` or `$schema` remains data and is never retrieved as a schema.

Handler failures remain error results and may include a structured `error`
object. `McpToolError` supplies stable error kinds and relevant fields so
clients can branch on failures without parsing display text. Use
`validation_issues_error` when one call must report multiple domain
validation issues.
