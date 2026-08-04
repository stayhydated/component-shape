# Introduction

`component-shape` lets Rust libraries describe a UI component once and expose
the public contracts that frameworks, code generators, and MCP integrations
need. Those contracts cover stable naming, value compatibility, runtime
capabilities, render behavior, and structured model input.

Choose the entry point that owns your task:

- Use `component-shape` to publish framework-neutral metadata or build a
  generator or framework adapter.
- Use `component-shape-gpui` to declare GPUI components, construction,
  rendering, and value binding.
- Use `component-shape-mcp` to turn coarse component metadata or typed Rust
  inputs into JSON Schema, MCP tools, composed servers, and stdio applications.

Start with [Getting started](getting-started.md) to add a dependency. The
remaining chapters explain how to combine the contracts without coupling the
framework-neutral crate to GPUI or MCP protocol behavior.

For complete signatures and trait definitions, use the
[`component-shape` API documentation](https://docs.rs/component-shape/),
[`component-shape-gpui` API documentation](https://docs.rs/component-shape-gpui/),
and
[`component-shape-mcp` API documentation](https://docs.rs/component-shape-mcp/).
