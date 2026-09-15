# component-shape-mcp

[![CI](https://github.com/stayhydated/component-shape/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/stayhydated/component-shape/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/gh/stayhydated/component-shape/branch/master/graph/badge.svg)](https://codecov.io/gh/stayhydated/component-shape)
[![Book](https://img.shields.io/badge/book-online-blue)](https://stayhydated.github.io/component-shape/book/)
[![crates.io](https://img.shields.io/crates/v/component-shape-mcp.svg)](https://crates.io/crates/component-shape-mcp)

Typed JSON Schema, strict argument decoding, shared tool registries,
structured results, composed servers, and stdio support for Rust MCP
integrations.

Use this crate to expose component metadata or typed Rust inputs through the
Model Context Protocol (MCP). Applications own authorization, domain validation,
and handler behavior.

## Pair schemas with handlers

- Derive `McpToolInput` for named tool arguments with strict field decoding.
- Use `McpJsonSchema` for nested schemas and `McpToolValue` for values that
  pair a schema with a decoder.
- Register typed handlers in `McpToolRegistry` and return structured results.
- Compose tools, resources, and prompts in `McpServer`.

## Reuse a tool registry

Register reusable definitions and handlers in `McpToolRegistry`. Install that
registry in `McpServer::from_tool_registry` when the application builds its MCP
tool surface independently. Registry clones share handler allocations. MCP
servers retain the registry across calls until the host shuts down.
