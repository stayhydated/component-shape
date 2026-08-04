# Tools and results

Register a typed tool so its published input schema and Rust handler argument
remain paired by type.

```rust
#[derive(component_shape_mcp::McpToolInput)]
#[serde(rename_all = "camelCase")]
struct SearchArgs {
    #[serde(alias = "q")]
    query: String,
    page_size: Option<u32>,
}

let mut server = component_shape_mcp::McpServer::builder(
    "search-server",
    env!("CARGO_PKG_VERSION"),
)
.build()?;

let tool = component_shape_mcp::tool_definition_for_input::<SearchArgs>(
    "search",
    Some("Search".to_owned()),
    None,
    None,
)?;

server.add_typed_tool(tool, |args: SearchArgs| {
    component_shape_mcp::tool_structured_result(
        component_shape_mcp::serde_json::json!({ "query": args.query }),
    )
})?;
```

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

Handler failures remain error results and may include a structured `error`
object. `McpToolError` supplies stable error kinds and relevant fields so
clients can branch on failures without parsing display text. Use
`validation_issues_error` when one call must report multiple domain
validation issues.
