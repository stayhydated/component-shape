# component-shape

[![Codecov: component-shape][codecov-badge]][codecov]
[![crates.io: component-shape][crate-badge]][crate]

`component-shape` provides framework-neutral metadata, value compatibility,
validated Rust syntax, and naming contracts for library and generator authors in
the [component-shape project][project]. Depend on it directly for shared contracts
that do not need a UI or protocol runtime. GPUI consumers can use the same
metadata through [`component-shape-gpui`][gpui]; typed schemas and decoding belong
in [`component-shape-mcp`][mcp].

Requires Rust 1.99 or newer.

## Example

A shape advertises the value it supports and a coarse input description without
constructing a widget or starting an MCP server:

```rust
use component_shape::{ComponentShapeFor, ComponentShapeMetadata, McpInput};

struct TextInputShape;

impl ComponentShapeMetadata for TextInputShape {
    const MCP_INPUT: McpInput = McpInput::string();
}

impl ComponentShapeFor<String> for TextInputShape {}

fn main() {
    let input = <TextInputShape as ComponentShapeFor<String>>::MCP_INPUT;
    assert_eq!(input, McpInput::string());
}
```

`ComponentShapeFor<Value>` can override this input description for a particular
shape/value pair. See the [API reference][api] for the metadata and syntax contracts.

[codecov-badge]: https://codecov.io/gh/stayhydated/component-shape/branch/master/graph/badge.svg?component=component-shape
[codecov]: https://codecov.io/gh/stayhydated/component-shape
[crate-badge]: https://img.shields.io/crates/v/component-shape.svg?label=component-shape
[crate]: https://crates.io/crates/component-shape
[project]: https://github.com/stayhydated/component-shape
[gpui]: https://crates.io/crates/component-shape-gpui
[mcp]: https://crates.io/crates/component-shape-mcp
[api]: https://docs.rs/component-shape
