# component-shape-mcp-macros

[![Codecov: component-shape-mcp-macros][codecov-badge]][codecov]
[![crates.io: component-shape-mcp-macros][crate-badge]][crate]

`component-shape-mcp-macros` implements the `McpJsonSchema` and `McpToolInput`
derives for the [component-shape project][project]. Applications should use
[`component-shape-mcp`][mcp] with its default `derive` feature; that facade supplies
the schemas, strict decoders, and runtime traits used by generated code. Depend
on this crate directly only when forwarding those derives through another facade.

Requires Rust 1.99 or newer.

The derives follow Serde's deserialization names, aliases, skips, and defaults;
`#[serde(flatten)]` is rejected. Use explicit nested fields or a custom schema and
decoder instead. See the [macro API reference][api] and the
[MCP package example][example].

[codecov-badge]: https://codecov.io/gh/stayhydated/component-shape/branch/master/graph/badge.svg?component=component-shape-mcp-macros
[codecov]: https://codecov.io/gh/stayhydated/component-shape
[crate-badge]: https://img.shields.io/crates/v/component-shape-mcp-macros.svg?label=component-shape-mcp-macros
[crate]: https://crates.io/crates/component-shape-mcp-macros
[project]: https://github.com/stayhydated/component-shape
[mcp]: https://crates.io/crates/component-shape-mcp
[api]: https://docs.rs/component-shape-mcp-macros
[example]: https://github.com/stayhydated/component-shape/blob/master/crates/component-shape-mcp/README.md
