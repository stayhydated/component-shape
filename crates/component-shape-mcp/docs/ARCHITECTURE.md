# component-shape-mcp Architecture

`component-shape-mcp` is a public integration crate for MCP server and schema
helpers that are useful across downstream component-shape consumers.

## Boundary

This crate owns:

1. MCP protocol version constant matching the `rmcp` protocol version in use.
1. Re-exports for `rmcp` tool definitions, content blocks, and tool call
   results.
1. `McpToolCall`, the normalized typed argument object passed to registered
   tool executors.
1. `McpArguments`, the owning field decoder used by generated and manual
   integrations to consume expected arguments and reject unknown fields.
1. `McpToolMetadata`, a small shared descriptor fragment for optional
   application-owned tool name, title, and description overrides.
1. A generic in-process sync/async MCP server implemented as an
   `rmcp::ServerHandler`.
1. Async and blocking stdio serving through `rmcp::transport::stdio`.
1. JSON Schema helpers for framework-neutral `McpInput` metadata.
1. `McpJsonSchema`, a typed schema trait and derive for aliases, tuple or named
   transparent newtypes, app-owned object argument structs, and fieldless enums.
1. `McpRange<T>`, a typed helper for object-shaped `min`/`max` range arguments.

It does not own:

- GPUI runtime contracts.
- Form holder decoding.
- Table filter decoding.
- Application authorization, confirmation UI, or sensitive-action policy.
- Long-running task policy.
- Low-level JSON-RPC parsing, protocol errors, version negotiation, shutdown,
  or transport framing; those are delegated to `rmcp`.

## Metadata Flow

`component-shape` publishes `McpInput` on `ComponentShapeMetadata`.
Downstream crates can copy that metadata into their own descriptors and call
`schema_for_input(...)` when building MCP tool schemas.
`McpInput::unsupported()` emits an impossible schema, while `McpInput::any()`
emits an unconstrained schema.
List metadata emits ordered JSON arrays; set metadata emits arrays with
`uniqueItems: true`.
When a downstream integration has access to an actual Rust type rather than
only `McpInput` metadata, it can use `McpJsonSchema::json_schema()` instead.
Range-shaped integrations can use `McpRange<T>` when the wire shape should be
an object with nullable `min` and `max` fields instead of a serde tuple array.
The derive emits object schemas for named structs, delegates tuple or named
transparent newtypes to their inner value schema, and emits string enum schemas for
fieldless enums; `#[mcp(crate = ...)]` lets facade crates re-export the derive
while keeping generated paths pointed at their own MCP module. For named
structs, the derive follows serde deserialize-facing `rename`, `rename_all`,
`skip_deserializing`, and `default` metadata. For enums, it follows
deserialize-facing `rename`, `rename_all`, `alias`, `skip_deserializing`, and
`other` metadata. It rejects `serde(flatten)` because the generated schema
cannot infer that merged object shape without a manual implementation.
`tool_definition(...)` validates that the final input and output schemas are
JSON objects, returning `McpToolError::InvalidSchema` for invalid schema values
so setup fails explicitly. The public `schema_object(label, value)?` helper
uses the same validation for custom integration code that builds tool
definitions in steps.
Downstream crates can also carry `McpToolMetadata` in their descriptors when
their derive or builder surfaces allow applications to override generated tool
names, titles, or descriptions. `validate_tool_name`,
`validate_tool_metadata_text`, and `McpToolMetadata::validate()` expose the
same validation for derive-time or descriptor-time checks.

Runtime decoding remains domain-specific. For example, a table range filter may
decode a decimal range into `RangeValue<Decimal>`, while a form field may
deserialize directly into a holder field through `serde`.

The `McpServer` is intentionally integration-neutral. Downstream crates
register domain-specific executors into the same server instead of wrapping
the server in mutually incompatible newtypes, which lets applications serve
form submit tools, table query tools, and custom tools from one MCP server. It
rejects duplicate tool names during registration so composed servers fail
clearly instead of silently replacing a tool executor.
At dispatch time the server converts protocol arguments into `McpToolCall`.
Missing arguments become an empty object, non-object arguments become tool
execution errors, and domain integrations decode from the normalized
`McpArguments` cursor returned by `McpToolCall::into_arguments()`. The cursor
owns the JSON object, decodes required, optional, nullable, and `usize` fields,
and reports leftover fields through `finish()`. Integrations that intentionally
need raw JSON can unwrap the cursor with `into_inner()`.
`McpServer::builder(name, version)` is the consuming builder form for chaining
generated registrars from multiple integration crates and serving or building
the final server once registration is complete. Direct integrations that own a
mutable server use `add_tool(...)` or `add_tool_async(...)`.
