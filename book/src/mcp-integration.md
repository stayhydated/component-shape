# MCP integration

Use `component-shape-mcp` when an application must expose component metadata
or typed Rust inputs through MCP. The crate keeps each published schema paired
with the decoder and handler contract that accepts it.

Add the dependency:

```toml
[dependencies]
component-shape-mcp = "0.1"
```

The default features include the schema and tool-input derives.

## Choose the input contract

- Use `McpInput` and `schema_for_input` for coarse metadata published by a
  component shape.
- Use `McpJsonSchema` for a nested Rust value with a precise JSON Schema.
- Use `McpToolValue` when one value needs both schema and strict decoding.
- Use `McpToolInput` for a named top-level tool argument struct.
- Use `McpRange<T>` for a typed `{ "min": ..., "max": ... }` range.
- Use `McpAny` only when unconstrained JSON is intentional.

## Follow the integration path

1. Define schemas and strict arguments in
   [Schemas and decoding](mcp-schemas.md).
2. Pair arguments with handlers and structured results in
   [Tools and results](mcp-tools.md).
3. Compose registrations and choose a transport in
   [Servers and stdio](mcp-server.md).

Keep authorization, domain validation execution, and handler policy in the
application. `component-shape-mcp` provides protocol contracts and shared
validation metadata, not application policy.
