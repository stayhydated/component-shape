# component-shape-mcp

`component-shape-mcp` contains shared MCP server and JSON Schema helpers for
crates that consume `component-shape` metadata. Protocol lifecycle and stdio
transport are delegated to the official `rmcp` SDK.

Most component authors should start with `component-shape` or
`component-shape-gpui`. Use this crate when building an integration layer that
needs MCP tool definitions, a dynamic `rmcp` tool server, structured tool
results, sync or async tool executors, a blocking stdio server entry point, or
JSON Schema from `McpInput` metadata. Generated integration crates can use
`McpToolMetadata` to carry optional application-owned tool names, titles, and
descriptions beside their domain descriptors, and can call
`validate_tool_name`, `validate_tool_metadata_text`, or
`McpToolMetadata::validate()` to share the same validation as final tool
construction.

```rust
use component_shape::McpInput;

let schema = component_shape_mcp::schema_for_input(McpInput::date_time_range());
```

Use `McpJsonSchema` when a Rust type should publish a more precise schema than
the coarse `McpInput::object()` marker. Built-in implementations cover common
primitive, `Option<T>`, `Vec<T>`, slices, arrays, sets, string-keyed maps,
references, `Cow<T>`, and boxed values, so type aliases inherit the underlying
schema. `McpRange<T>` covers typed `{ "min": ..., "max": ... }` range
arguments. App-owned named structs, tuple or named transparent newtypes, and
fieldless enums can derive it. The derive follows serde deserialize names, includes
deserialize aliases for enum variants, skips deserialization-skipped fields,
rejects flattened fields that cannot be inferred safely, and treats
serde-defaulted fields as not required:

```rust
#[derive(component_shape_mcp::McpJsonSchema)]
#[mcp(crate = component_shape_mcp)]
struct SearchArgs {
    #[mcp(rename = "q", description = "Search text")]
    query: String,
    page: Option<u32>,
}

#[derive(component_shape_mcp::McpJsonSchema)]
#[serde(rename_all = "kebab-case")]
enum IssueState {
    Open,
    InReview,
    Closed,
}
```

`McpInput::unsupported()` maps to an impossible schema. Use
`McpInput::any()` when a tool should accept unconstrained JSON. Tool
definitions reject non-object input or output schemas during registration
instead of silently publishing an empty schema. Use
`schema_object(label, value)?` when custom integration code needs the same
strict object validation before constructing an `rmcp` tool definition.
`McpInput::*_list()` maps to an ordered JSON array; `McpInput::*_set()` maps
to an array with `uniqueItems: true`.

This crate does not decode application values or authorize tool calls. Domain
integrations such as form submit or table query crates own those policies.
Those integrations can register into the same `McpServer`, so an
application can serve form submit tools, table query tools, and custom tools
from one MCP server. Use `McpServer::builder(name, version)` to chain generated
registrars from integration crates with `.register(...)`, or add custom tools
with `.tool(...)` and `.tool_async(...)`. If you already have a mutable server,
call `server.add_tool(...)` or `server.add_tool_async(...)` directly. Call
`.build()?`, `.serve_stdio().await`, or `.serve_stdio_blocking()` when
registration is complete. Registration returns an error for duplicate tool
names so composed server construction can fail explicitly.

Custom tool executors receive a typed `McpToolCall`, not raw JSON. The shared
server has already normalized missing arguments to an empty object and rejected
non-object arguments before dispatch. Use `call.into_arguments()` to get an
`McpArguments` decoder, consume expected fields with helpers such as
`take_required`, `take_present`, `take_nullable`, or `take_optional_usize`, and
finish with `arguments.finish()?` to reject unknown fields. Use
`into_inner()` only when an integration intentionally needs raw JSON ownership.
