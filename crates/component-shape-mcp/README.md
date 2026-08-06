# component-shape-mcp

Typed JSON Schema, strict argument decoding, tool definitions, structured
results, composed servers, and stdio support for Rust MCP integrations.

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

## Learn more

- [MCP integration guide](https://stayhydated.github.io/component-shape/book/mcp-integration.html)
- [Schemas and decoding](https://stayhydated.github.io/component-shape/book/mcp-schemas.html)
- [Tools and results](https://stayhydated.github.io/component-shape/book/mcp-tools.html)
- [Servers and stdio](https://stayhydated.github.io/component-shape/book/mcp-server.html)
- [API documentation](https://docs.rs/component-shape-mcp/)
