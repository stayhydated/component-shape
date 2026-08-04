# Schemas and decoding

Define a named argument struct with `McpToolInput` when a tool should publish
an object schema and decode the same contract into Rust.

```rust
#[derive(component_shape_mcp::McpToolInput)]
#[serde(rename_all = "camelCase")]
struct SearchArgs {
    #[serde(alias = "q")]
    query: String,
    page_size: Option<u32>,
}
```

The derive follows serde deserialize names and aliases, rejects unknown input
fields, and decodes each field through `McpToolValue`. It also implements
`McpJsonSchema`, so the argument type can be nested in another schema.

## Describe nested values

Derive `McpJsonSchema` for application-owned named structs, transparent
newtypes, and fieldless enums. Built-in implementations cover common
primitives, options, collections, arrays, tuples, sets, string-keyed maps,
references, `Cow<T>`, boxed values, `McpRange<T>`, and `McpAny`.

The derive honors deserialize-facing serde behavior:

- Renames and aliases become accepted input names.
- Deserialization-skipped fields are omitted.
- Defaulted and optional fields are not required.
- Flattened fields are rejected when a safe schema cannot be inferred.
- Rust doc comments become descriptions unless an `mcp` description is set.

Use `#[mcp(crate = path::to::mcp)]` only when the dependency is renamed or
multiple MCP facade re-exports make the path ambiguous.

## Build custom schemas

Use typed builders such as `McpSchema::string()`,
`McpSchema::integer().with_minimum(0_u64)`, and
`McpSchema::object().with_properties(...)`. Reserve `McpSchema::new(...)`
for JSON Schema keywords that the typed builders do not cover.

Tool input schemas must describe a top-level object. Output schemas may
describe any JSON value. Use `McpInput::unsupported()` for a coarse shape
that should not advertise model input and `McpInput::any()` or `McpAny`
only for intentionally arbitrary JSON.

## Decode untyped calls strictly

Custom untyped executors receive `McpToolCall`. Convert the call with
`into_arguments()`, consume known fields with
`take_required_tool_value::<T>` or `take_present_tool_value::<T>`, and call
`finish()?` to reject unknown fields. Prefer these schema-paired helpers over
taking raw JSON.
