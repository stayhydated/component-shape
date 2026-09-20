# component-shape-mcp

[![Codecov: component-shape-mcp][codecov-badge]][codecov]
[![crates.io: component-shape-mcp][crate-badge]][crate]

component-shape-mcp provides typed JSON Schema, strict argument decoding, shared
tool registries, structured results, composed servers, and stdio support for
Rust applications that expose component contracts through MCP. It is the
protocol-facing crate in the [component-shape project][project]; applications
own authorization, domain validation, and handler policy.

## Overview

- Derive `McpToolInput` for named tool arguments with strict field decoding.
- Use `McpJsonSchema` for nested schemas and `McpToolValue` for values that
  pair a schema with a decoder.
- Register typed handlers in `McpToolRegistry` and return structured results.
- Install a reusable registry with `McpServer::from_tool_registry`, or compose
  tools, resources, and prompts directly in `McpServer`.

[codecov-badge]: https://codecov.io/gh/stayhydated/component-shape/branch/master/graph/badge.svg?component=component-shape-mcp
[codecov]: https://codecov.io/gh/stayhydated/component-shape
[crate-badge]: https://img.shields.io/crates/v/component-shape-mcp.svg?label=component-shape-mcp
[crate]: https://crates.io/crates/component-shape-mcp
[project]: https://github.com/stayhydated/component-shape
