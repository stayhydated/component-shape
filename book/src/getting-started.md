# Getting started

Use Rust 1.98 or newer, then add the narrowest crate that owns your integration.
All crates use edition 2024.

## Framework-neutral metadata

```toml
[dependencies]
component-shape = "0.2"
```

Implement `ComponentShapeMetadata` on a shape type. Add
`ComponentShapeFor<Value>` for each value the shape supports. Continue with
[Framework-neutral shapes](framework-neutral-shapes.md).

## GPUI declarations

```toml
[dependencies]
component-shape-gpui = "0.2"
gpui-kit = "0.6.0"
```

The GPUI crate re-exports the shared framework-neutral traits and uses the
`gpui-kit` facade for GPUI types and components. Choose the derive for an owned
component or `component_shape!` for a local wrapper around external types.
Continue with [GPUI shapes](gpui-shapes.md).

## MCP integrations

```toml
[dependencies]
component-shape-mcp = "0.2"
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
