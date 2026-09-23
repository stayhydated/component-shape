---
name: use-component-shape-mcp
description: "Build, review, or document component-shape-mcp integrations: paired schemas and argument decoding, typed tools, registries, structured results, composed servers, and stdio. Use the framework-neutral skill for coarse McpInput metadata alone."
---

# Use component shape MCP

## Route the task

Use this skill for protocol-facing code built with `component-shape-mcp`.
Coarse `McpInput` metadata belongs in `component-shape`, and GPUI declarations
belong in `component-shape-gpui`; use their dedicated skills when available.
Keep authorization, domain validation execution, and handler policy in the
consumer.

Inside this repository, read `AGENTS.md` first. Keep public contracts and
rustdoc in `crates/component-shape-mcp`, derive implementation in
`crates/component-shape-mcp-macros`, and user workflows in the MCP chapters
under `book/src`.

## Choose the contract

- Use `McpInput` and `schema_for_input` for coarse component metadata.
- Use `McpJsonSchema` for precise nested Rust schemas.
- Use `McpToolValue` when a value needs paired schema and strict decoding.
- Use `McpToolInput` for a named top-level argument struct.
- Use `McpRange<T>` for typed minimum/maximum objects.
- Use `McpAny` only for intentionally unconstrained JSON.

The default crate features include the derives.

## Register a typed tool

```rust
#[derive(component_shape_mcp::McpToolInput)]
struct SearchArgs {
    query: String,
}

fn main() -> Result<(), component_shape_mcp::McpToolError> {
    let mut tools = component_shape_mcp::McpToolRegistry::new();
    let tool = component_shape_mcp::tool_definition_for_input::<SearchArgs>(
        "search",
        Some("Search".to_owned()),
        None,
        None,
    )?;

    tools.add_typed_tool(tool, |args: SearchArgs| {
        component_shape_mcp::tool_structured_result(
            component_shape_mcp::serde_json::json!({ "query": args.query }),
        )
    })?;

    let server = component_shape_mcp::McpServer::from_tool_registry(
        "search-server",
        "1.0.0",
        tools,
    );
    assert_eq!(server.tool_count(), 1);
    Ok(())
}
```

Follow serde deserialize names, aliases, skips, and defaults. The derives
reject `#[serde(flatten)]`; use explicit nested fields or a custom schema and
decoder. Use `#[mcp(crate = facade::mcp)]` only for a renamed dependency or
ambiguous facades.

For an untyped handler, convert `McpToolCall` with `into_arguments()`,
consume fields through the typed `take_*` helpers, and call `finish()?` to
reject unknown fields.

## Enforce schema and result rules

Publish object-shaped tool inputs; outputs may describe any JSON value. Prefer
typed `McpSchema` builders and reserve `McpSchema::new(...)` for unsupported
keywords.

Keep application-owned names, text, icons, and annotation hints in
`McpToolMetadata`. Let registration validate typed and raw definitions.

Return `structured_content` matching a declared output schema on success. Use
`McpToolError` or `validation_issues_error` for structured failures that
clients can inspect without parsing display text.

## Compose and serve

Use `McpToolRegistry` when tool definitions and handlers are assembled
independently from a server. Clone the registry to share handler allocations
and install it in `McpServer::from_tool_registry`. Keep resources, prompts,
server identity, and transport lifecycle in `McpServer`.

Use `McpServer::builder(name, version)` to combine generated registrars,
custom tools, resources, templates, and prompts. Propagate duplicate-name and
duplicate-URI errors.

Call `build()?` on a builder when the caller owns transport. Call
`serve_stdio().await` in an async application or `serve_stdio_blocking()` at a
synchronous binary boundary. The blocking method creates a Tokio runtime.
Use `McpStdioSmokeClient` only for process-level tests over real
stdin and stdout.

## Coordinate changes

When editing this repository, align public rustdoc, focused schema or server
tests, derive tests, MCP book chapters, the crate README, and this skill when
user-visible behavior changes. Prefer the narrowest test that proves schema, decoding,
registration, result, or stdio behavior. In a consumer, update its affected
integration and tests. For review requests, report needed changes without
applying them.
