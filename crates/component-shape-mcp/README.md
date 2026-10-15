# component-shape-mcp

[![Codecov: component-shape-mcp][codecov-badge]][codecov]
[![crates.io: component-shape-mcp][crate-badge]][crate]

`component-shape-mcp` provides typed JSON Schema, strict argument decoding, tool
registries, structured results, composed servers, and stdio transport for Rust
applications exposing MCP tools. Depend on this protocol-facing crate directly;
this crate re-exports shared `McpInput` metadata from the
[component-shape project][project].
Applications choose Koruma domain rules and own authorization and handler policy.

Requires Rust 1.99 or newer. The transport uses RMCP 3.5.

## Overview

- Pair nested schemas with decoding through `McpJsonSchema` and `McpToolValue`;
  derive `McpToolInput` for named top-level argument structs.
- Register typed handlers, or execute Koruma rules before dispatch with
  `add_koruma_tool` and `add_koruma_tool_async`.
- Validate successful structured output against compiled schemas with bundled
  references and no external retrieval. Formats remain annotations.
- Compose tools, resources, and prompts through `McpServer`; serve stdio at the
  application's synchronous or asynchronous boundary.

See the [API reference][api] for schema semantics, domain-issue conversion, and
server lifecycle details.

## Features

The default feature set enables `derive`.

| Feature | Default | Purpose |
| --- | --- | --- |
| `chrono` | No | Implements `McpJsonSchema` for `NaiveDate`, `NaiveDateTime`, and `DateTime<Tz>`. |
| `derive` | Yes | Re-exports `McpJsonSchema` and `McpToolInput` from `component-shape-mcp-macros`. |
| `rust_decimal` | No | Implements decimal schema metadata for `rust_decimal::Decimal`. |

## Example

With the default `derive` feature, register and call a [typed echo tool][example]
in process.

```rust
use component_shape_mcp::{
    McpServer, McpToolError, McpToolInput, serde_json::json,
    tool_definition_for_input, tool_structured_result,
};

#[derive(McpToolInput)]
struct EchoInput {
    value: String,
}

fn main() -> Result<(), McpToolError> {
    let mut server = McpServer::new("echo-server", "1.0.0");
    server.add_typed_tool(
        tool_definition_for_input::<EchoInput>("echo", None, None, None)?,
        |input: EchoInput| tool_structured_result(json!({ "value": input.value })),
    )?;

    let result = server.call_tool("echo", Some(json!({ "value": "typed" })));
    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.structured_content, Some(json!({ "value": "typed" })));
    Ok(())
}
```

The call returns the echoed value in structured content. Unknown arguments are
rejected before the handler runs.

[codecov-badge]: https://codecov.io/gh/stayhydated/component-shape/branch/master/graph/badge.svg?component=component-shape-mcp
[codecov]: https://codecov.io/gh/stayhydated/component-shape
[crate-badge]: https://img.shields.io/crates/v/component-shape-mcp.svg?label=component-shape-mcp
[crate]: https://crates.io/crates/component-shape-mcp
[project]: https://github.com/stayhydated/component-shape
[api]: https://docs.rs/component-shape-mcp
[example]: https://github.com/stayhydated/component-shape/blob/master/crates/component-shape-mcp/src/tests/registry_server.rs
