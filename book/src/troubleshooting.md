# Troubleshooting

## A GPUI shape is not compatible with a value

Declare `value = T` or `values(...)`, publish compatibility through value
binding, or implement both `ComponentShapeFor<T>` and
`GpuiComponentShapeFor<T>`.

## A consumer rejects a hand-written GPUI shape

If the error names `DeclaredGpuiComponentShape`, the consumer requires a
macro-declared shape. Use the derive or `component_shape!` so that marker is
generated, then rerun the consumer's Cargo check.

## A derive cannot find component-shape-mcp

Use the normal crate name or an unambiguous facade re-export. Add
`#[mcp(crate = path::to::mcp)]` for a renamed crate or when multiple facades
make inference ambiguous.

## A tool input schema is rejected

Tool inputs need a top-level object schema. Derive `McpToolInput` for a named
argument struct. Use scalar, list, or other schemas only for nested fields and
tool outputs.

## An argument is reported as unknown

Match the deserialize-facing serde name or one of its aliases. In a custom
untyped handler, consume every supported field before calling
`McpArguments::finish()`.

## A successful output fails validation

Return `structured_content` that matches the declared output schema. Keep
handler failures as error results with a structured `error` object.
