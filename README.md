# component-shape

[![Build Status](https://github.com/stayhydated/component-shape/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/component-shape/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/github/stayhydated/component-shape/graph/badge.svg?token=34CV04UOU1)](https://codecov.io/github/stayhydated/component-shape)
[![Book](https://img.shields.io/badge/docs-book-black)](https://stayhydated.github.io/component-shape/book/)
[![Crates.io](https://img.shields.io/crates/v/component-shape.svg)](https://crates.io/crates/component-shape)

Rust contracts for describing UI components to frameworks, code generators,
and MCP integrations.

## Choose a crate

- [`component-shape`](https://docs.rs/component-shape/) provides
  framework-neutral metadata, value compatibility, naming, and coarse MCP input
  descriptions.
- [`component-shape-gpui`](https://docs.rs/component-shape-gpui/) adds GPUI
  declaration macros, render contracts, configured builders, and value binding.
- [`component-shape-mcp`](https://docs.rs/component-shape-mcp/) adds typed JSON
  Schema, strict argument decoding, shared tool registries, composed MCP
  servers, and stdio support.

Follow the [user guide](https://stayhydated.github.io/component-shape/book/) to
choose and combine these contracts.

## Codex skills

Install the repository skills with:

```sh
npx skills add stayhydated/component-shape
```
