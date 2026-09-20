---
name: use-component-shape
description: "Add, review, or document framework-neutral component-shape metadata and code-generation contracts. Covers shape/value compatibility, capabilities, suffixes, Rust syntax, coarse McpInput, and ValueChange; use dedicated skills for GPUI declarations or typed MCP integration."
---

# Use component shape

## Route the task

Keep a contract in `component-shape` when it does not require a UI framework
or protocol runtime. GPUI declarations, rendering, builders, and value binding
belong in `component-shape-gpui`; typed JSON Schema, decoding, and servers
belong in `component-shape-mcp`. Use their dedicated skills when available.

Inside this repository, read `AGENTS.md` before editing. The owning surfaces
are:

- `crates/component-shape` for public framework-neutral contracts.
- `crates/component-shape-codegen` for shared token generation and path,
  suffix, syntax, or MCP metadata normalization.
- `book/src/framework-neutral-shapes.md` for user guidance.
- `crates/component-shape/src/lib.rs` and public rustdoc for the API contract.

## Choose the public contract

- Implement `ComponentShapeMetadata` for shape-owned `PROTOTYPING`,
  `CAPABILITIES`, and coarse `MCP_INPUT`.
- Implement `ComponentShapeFor<Value>` for each supported value. Override its
  `MCP_INPUT` only when the pair differs from the shape-level metadata.
- Require `DeclaredComponentShape` only for shapes emitted by a
  backend-approved declaration surface.
- Use `ComponentFieldName` and `ComponentShapeUse` to record a selected
  source field and shape path for generators.
- Use `ComponentSuffix` for ASCII identifier suffixes. They must start with a
  letter or underscore; empty strings and `_` are rejected.
- Use `RustPath`, `RustType`, and `RustExpr` to preserve validated Rust
  syntax.
- Normalize framework events into `ValueChange::Unchanged`,
  `ValueChange::Set`, or `ValueChange::Clear`.

Keep framework types out of this crate. Prefer generic capability metadata over
framework-specific branching.

## Handle coarse MCP metadata

Use `McpInput` for common scalar, collection, object, and range shapes. Leave
`McpInput::unsupported()` when a shape should not advertise model input. Use
`McpInput::any()` only for intentionally arbitrary JSON.

Keep precise schemas, strict JSON decoding, and transport in
`component-shape-mcp`. The consuming application owns authorization, domain
validation, and handler policy.

## Coordinate changes

When editing this repository's shared semantics, coordinate the affected
surfaces below. In a consumer, update its integration and focused checks.
For review requests, report needed changes without applying them.

1. Update the framework-neutral definition and rustdoc.
2. Update `component-shape-codegen` only when token generation,
   normalization, imports, suffixes, or emitted metadata change.
3. Update framework macro fixtures only when their public syntax, generated
   contract, or diagnostics change.
4. Update the framework-neutral book chapter and affected downstream skills
   when the user workflow changes.

Keep parser internals and design rationale in focused code documentation or
tests rather than user-facing READMEs.
