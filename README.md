# component-shape

[![CI][ci-badge]][ci]
[![Codecov][codecov-badge]][codecov]
[![Book][book-badge]][book]
[![crates.io: component-shape][component-shape-badge]][component-shape-crate]
[![crates.io: component-shape-gpui][component-shape-gpui-badge]][component-shape-gpui-crate]
[![crates.io: component-shape-mcp][component-shape-mcp-badge]][component-shape-mcp-crate]

component-shape provides Rust contracts for library authors to describe UI
components once and share their metadata with frameworks, code generators, and
Model Context Protocol (MCP) integrations.

## Crates

| Package | Purpose | Source |
| --- | --- | --- |
| `component-shape` | Framework-neutral metadata, value compatibility, naming, and coarse MCP input descriptions | [source][component-shape-source] |
| `component-shape-gpui` | GPUI declaration macros, render contracts, configured builders, and value binding | [source][component-shape-gpui-source] |
| `component-shape-mcp` | Typed JSON Schema, strict argument decoding, tool registries, composed MCP servers, and stdio support | [README][component-shape-mcp-readme] |

## Example

A shape publishes metadata independently of its UI runtime. A generator can
check which value types it supports and choose the corresponding input format.

```rust
use component_shape::{ComponentShapeFor, ComponentShapeMetadata, McpInput};

struct TextInputShape;

impl ComponentShapeMetadata for TextInputShape {
    const MCP_INPUT: McpInput = McpInput::string();
}

impl ComponentShapeFor<String> for TextInputShape {}

let input = <TextInputShape as ComponentShapeFor<String>>::MCP_INPUT;
assert_eq!(input, McpInput::string());
```

[ci-badge]: https://github.com/stayhydated/component-shape/actions/workflows/ci.yml/badge.svg?branch=master
[ci]: https://github.com/stayhydated/component-shape/actions/workflows/ci.yml
[codecov-badge]: https://codecov.io/gh/stayhydated/component-shape/branch/master/graph/badge.svg
[codecov]: https://codecov.io/gh/stayhydated/component-shape
[book-badge]: https://img.shields.io/badge/Book-mdBook-blue
[book]: https://stayhydated.github.io/component-shape/book/
[component-shape-badge]: https://img.shields.io/crates/v/component-shape.svg?label=component-shape
[component-shape-crate]: https://crates.io/crates/component-shape
[component-shape-gpui-badge]: https://img.shields.io/crates/v/component-shape-gpui.svg?label=component-shape-gpui
[component-shape-gpui-crate]: https://crates.io/crates/component-shape-gpui
[component-shape-mcp-badge]: https://img.shields.io/crates/v/component-shape-mcp.svg?label=component-shape-mcp
[component-shape-mcp-crate]: https://crates.io/crates/component-shape-mcp
[component-shape-source]: https://github.com/stayhydated/component-shape/tree/master/crates/component-shape
[component-shape-gpui-source]: https://github.com/stayhydated/component-shape/tree/master/crates/component-shape-gpui
[component-shape-mcp-readme]: https://github.com/stayhydated/component-shape/blob/master/crates/component-shape-mcp/README.md
