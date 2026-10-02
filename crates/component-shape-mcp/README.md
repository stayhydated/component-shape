# component-shape-mcp

[![Codecov: component-shape-mcp][codecov-badge]][codecov]
[![crates.io: component-shape-mcp][crate-badge]][crate]

component-shape-mcp provides typed JSON Schema, strict argument decoding, shared
tool registries, structured results, composed servers, and stdio support for
Rust applications that expose component contracts through MCP. It is the
protocol-facing crate in the [component-shape project][project]. Koruma is the
ecosystem's canonical domain validator; applications choose its rules and own
authorization and handler policy.

## Overview

- Derive `McpToolInput` for named tool arguments with strict field decoding.
- Use `McpJsonSchema` for nested schemas and `McpToolValue` for values that
  pair a schema with a decoder.
- Follow Serde's deserialization names or choose explicit MCP wire-name overrides.
- Reject duplicate field spellings and unknown arguments with strict tool-input decoding.
- Register typed handlers in `McpToolRegistry` and return structured results.
- Use `add_koruma_tool` or `add_koruma_tool_async` to execute Koruma rules on
  decoded inputs before their handlers run. Server builders expose `koruma_tool`
  and `koruma_tool_async` for the same contract.
- Validate successful output against compiled JSON Schemas, with bundled
  references and no external retrieval. Formats remain annotations; output
  uses the exact advertised names.
- Install a reusable registry with `McpServer::from_tool_registry`, or compose
  tools, resources, and prompts directly in `McpServer`.

## Validate domain input

Derive `McpToolInput` and `koruma::Koruma` on arguments with domain rules, then
register through `add_koruma_tool`. It requires Koruma's `ValidateExt` and
`ValidationIssues` contracts. The runtime integration uses `koruma-core`; add the
`koruma` facade and `koruma-collection` in applications that derive or use built-in
validators.

Validation failures return structured form, field, or element issues, preserving
the validator, source field, label, index, and runtime parameters. Parameter values
keep their JSON types. Opaque values use `{ "kind": "opaque", "type_name": ... }`;
non-finite floats use `{ "kind": "f64", "value": ... }`. A custom error with no
issues still fails with a form-level issue. Use `McpValidationIssue::from(&issue)`
or `koruma_validation_error(&error)` when an integration needs shared conversion,
and `validate_koruma(&input)` after constructing an untyped tool's domain value.

Schemas and strict decoding enforce protocol structure. Koruma executes domain
rules; schema hints describe constraints to clients. Successful results retain
the output-schema checks described above.

[codecov-badge]: https://codecov.io/gh/stayhydated/component-shape/branch/master/graph/badge.svg?component=component-shape-mcp
[codecov]: https://codecov.io/gh/stayhydated/component-shape
[crate-badge]: https://img.shields.io/crates/v/component-shape-mcp.svg?label=component-shape-mcp
[crate]: https://crates.io/crates/component-shape-mcp
[project]: https://github.com/stayhydated/component-shape
