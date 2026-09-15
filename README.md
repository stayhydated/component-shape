# component-shape

[![CI](https://github.com/stayhydated/component-shape/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/stayhydated/component-shape/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/gh/stayhydated/component-shape/branch/master/graph/badge.svg)](https://codecov.io/gh/stayhydated/component-shape)
[![Book](https://img.shields.io/badge/book-online-blue)](https://stayhydated.github.io/component-shape/book/)
[![crates.io](https://img.shields.io/crates/v/component-shape.svg)](https://crates.io/crates/component-shape)

Rust contracts for describing UI components to frameworks, code generators,
and Model Context Protocol (MCP) integrations.

## Choose a crate

- `component-shape` provides framework-neutral metadata, value compatibility,
  naming, and coarse MCP input descriptions.
- `component-shape-gpui` adds GPUI declaration macros, render contracts,
  configured builders, and value binding.
- `component-shape-mcp` adds typed JSON Schema, strict argument decoding,
  shared tool registries, composed MCP servers, and stdio support.

## Describe a component

A shape publishes metadata independently of its UI runtime. A generator can
check which value types it supports and choose the corresponding input format.

```rust
use component_shape::{ComponentShapeFor, ComponentShapeMetadata, McpInput};

struct TextInputShape;

impl ComponentShapeMetadata for TextInputShape {
    const MCP_INPUT: McpInput = McpInput::string();
}

impl ComponentShapeFor<String> for TextInputShape {}
```
