# Getting started

Use Rust 1.96 or newer, then add the narrowest crate that owns your integration.
All three crates use edition 2024.

## Framework-neutral metadata

```toml
[dependencies]
component-shape = "0.1"
```

Implement `ComponentShapeMetadata` on a shape type. Add
`ComponentShapeFor<Value>` for each value the shape supports. Continue with
[Framework-neutral shapes](framework-neutral-shapes.md).

## GPUI declarations

```toml
[dependencies]
component-shape-gpui = "0.1"
```

The GPUI crate re-exports the shared framework-neutral traits. Align the
application's `gpui` source and version with `component-shape-gpui`, then
choose the derive for an owned component or `component_shape!` for a local
wrapper around external types. Continue with [GPUI shapes](gpui-shapes.md).

## MCP integrations

```toml
[dependencies]
component-shape-mcp = "0.1"
```

The default features export the `McpJsonSchema` and `McpToolInput` derives.
Use `default-features = false` when an integration only consumes coarse
`McpInput` metadata or implements schema contracts manually. Continue with
[MCP integration](mcp-integration.md).

## Check the integration

Run Cargo against the package that owns the new dependency:

```sh
cargo check -p <package-name>
```

A successful check confirms that the selected crate and its public contracts
resolve in that package.
