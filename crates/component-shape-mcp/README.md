# component-shape-mcp

Typed JSON Schema, strict argument decoding, shared tool registries,
structured results, composed servers, and stdio support for Rust MCP
integrations.

Most component authors should start with
[`component-shape`](https://docs.rs/component-shape/) or
[`component-shape-gpui`](https://docs.rs/component-shape-gpui/). Add this
crate when an application needs to expose component metadata or typed Rust
inputs through MCP.

## Add the dependency

```toml
[dependencies]
component-shape-mcp = "0.2"
```

The default features include the `McpJsonSchema` and `McpToolInput` derives.
Disable default features when an integration only consumes coarse `McpInput`
metadata or implements its schema contracts manually.

Register reusable definitions and handlers in `McpToolRegistry`. Install that
registry in `McpServer::from_tool_registry` when the application builds its MCP
tool surface independently. Registry clones share handler allocations. MCP
servers retain the registry for their host lifetime. Individual tool
completion never implies shutdown; a host that wants one request terminates
explicitly after receiving its result.

## Learn more

- [MCP integration guide](https://stayhydated.github.io/component-shape/book/mcp-integration.html)
- [Schemas and decoding](https://stayhydated.github.io/component-shape/book/mcp-schemas.html)
- [Tools and results](https://stayhydated.github.io/component-shape/book/mcp-tools.html)
- [Servers and stdio](https://stayhydated.github.io/component-shape/book/mcp-server.html)
- [API documentation](https://docs.rs/component-shape-mcp/)
