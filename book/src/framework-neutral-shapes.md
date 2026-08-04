# Framework-neutral shapes

Use a framework-neutral shape when a generator or adapter needs component
metadata without depending on GPUI or another UI runtime. A shape can advertise
value compatibility, capabilities, stable generated-name suffixes, and coarse
MCP input metadata.

## Declare metadata and value compatibility

```rust
use component_shape::{ComponentShapeFor, ComponentShapeMetadata, McpInput};

struct TextInputShape;

impl ComponentShapeMetadata for TextInputShape {
    const MCP_INPUT: McpInput = McpInput::string();
}

impl ComponentShapeFor<String> for TextInputShape {}
```

`ComponentShapeFor<Value>::MCP_INPUT` inherits the shape-level value. Override
the associated constant on a specific shape/value pair when one shape supports
values with different model-facing forms.

## Choose the contract

- Put shape-owned prototyping, capabilities, and coarse MCP input in
  `ComponentShapeMetadata`.
- Use `ComponentShapeFor<Value>` to advertise a supported value and any
  value-specific MCP input.
- Require `DeclaredComponentShape` only when a backend accepts shapes produced
  by its trusted declaration surface.
- Record a selected source field and shape path with `ComponentShapeUse`.
- Normalize component events into `ValueChange::Unchanged`,
  `ValueChange::Set`, or `ValueChange::Clear`.

Use `ComponentSuffix` for non-empty ASCII suffixes that become part of
generated identifiers. Use `RustPath`, `RustType`, and `RustExpr` when a
generator must preserve validated Rust syntax.

## Keep MCP metadata coarse

Use `McpInput` for common scalar, collection, object, and range shapes. Use
`McpInput::any()` only when arbitrary JSON is intentional. Move precise JSON
Schema, typed decoding, tool registration, and transport behavior to
[`component-shape-mcp`](mcp-integration.md).
