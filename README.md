# component-shape

[![CI][ci-badge]][ci]
[![Codecov][codecov-badge]][codecov]
[![Book][book-badge]][book]
[![crates.io: component-shape][component-shape-badge]][component-shape-crate]
[![crates.io: component-shape-codegen][component-shape-codegen-badge]][component-shape-codegen-crate]
[![crates.io: component-shape-gpui][component-shape-gpui-badge]][component-shape-gpui-crate]
[![crates.io: component-shape-mcp][component-shape-mcp-badge]][component-shape-mcp-crate]

component-shape provides Rust contracts for library authors to describe UI
components once and share their metadata with frameworks, code generators, and
Model Context Protocol (MCP) integrations.

Requires Rust 1.99 or newer across the published crates. The GPUI integration
uses GPUI Kit 0.7; MCP transport uses RMCP 3.5.

## Crates

Choose `component-shape` for framework-neutral contracts, `component-shape-gpui`
for GPUI declarations and runtime integration, or `component-shape-mcp` for typed
protocol tools. Macro packages are normally consumed through those facades.

| Crate | Purpose |
| --- | --- |
| [`component-shape`][component-shape-readme] | Framework-neutral metadata, value compatibility, naming, and coarse MCP input descriptions. |
| [`component-shape-codegen`][component-shape-codegen-readme] | Shared token, path, suffix, and metadata generation for macro authors. |
| [`component-shape-gpui`][component-shape-gpui-readme] | GPUI declarations, render contracts, configured builders, and value binding. |
| [`component-shape-gpui-macros`][component-shape-gpui-macros-readme] | Declaration macro implementation, re-exported by the GPUI facade. |
| [`component-shape-mcp`][component-shape-mcp-readme] | Typed schemas, strict decoding, validated tools, composed servers, and stdio. |
| [`component-shape-mcp-macros`][component-shape-mcp-macros-readme] | Schema and input derives, re-exported by the MCP facade's default `derive` feature. |

[ci-badge]: https://github.com/stayhydated/component-shape/actions/workflows/ci.yml/badge.svg?branch=master
[ci]: https://github.com/stayhydated/component-shape/actions/workflows/ci.yml
[codecov-badge]: https://codecov.io/gh/stayhydated/component-shape/branch/master/graph/badge.svg
[codecov]: https://codecov.io/gh/stayhydated/component-shape
[book-badge]: https://img.shields.io/badge/Book-mdBook-blue
[book]: https://stayhydated.github.io/component-shape/book/
[component-shape-badge]: https://img.shields.io/crates/v/component-shape.svg?label=component-shape
[component-shape-crate]: https://crates.io/crates/component-shape
[component-shape-gpui-badge]: https://img.shields.io/crates/v/component-shape-gpui.svg?label=component-shape-gpui
[component-shape-gpui-crate]: https://crates.io/crates/component-shape-gpui
[component-shape-mcp-badge]: https://img.shields.io/crates/v/component-shape-mcp.svg?label=component-shape-mcp
[component-shape-mcp-crate]: https://crates.io/crates/component-shape-mcp
[component-shape-mcp-readme]: https://github.com/stayhydated/component-shape/blob/master/crates/component-shape-mcp/README.md
[component-shape-readme]: https://github.com/stayhydated/component-shape/blob/master/crates/component-shape/README.md
[component-shape-codegen-readme]: https://github.com/stayhydated/component-shape/blob/master/crates/component-shape-codegen/README.md
[component-shape-gpui-readme]: https://github.com/stayhydated/component-shape/blob/master/crates/component-shape-gpui/README.md
[component-shape-gpui-macros-readme]: https://github.com/stayhydated/component-shape/blob/master/crates/component-shape-gpui-macros/README.md
[component-shape-mcp-macros-readme]: https://github.com/stayhydated/component-shape/blob/master/crates/component-shape-mcp-macros/README.md
[component-shape-codegen-badge]: https://img.shields.io/crates/v/component-shape-codegen.svg?label=component-shape-codegen
[component-shape-codegen-crate]: https://crates.io/crates/component-shape-codegen
