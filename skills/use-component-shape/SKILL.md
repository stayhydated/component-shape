---
name: use-component-shape
description: "Use when Codex needs to add, review, or refactor framework-neutral component-shape metadata, including ComponentShapeMetadata, ComponentShapeFor, DeclaredComponentShape, ComponentShapeUse, capabilities, prototyping suffixes, Rust syntax wrappers, MCP input metadata, value changes, or generator-facing contracts."
---

# Use Component Shape

## Scope Boundary

Use this skill for framework-neutral `component-shape` concepts and contracts.
It covers shape metadata, naming, suffix validation, component capabilities,
field/component use records, Rust syntax wrappers, normalized value changes,
MCP input metadata, and generator-facing shape contracts.

Do not use this skill for GPUI-specific component contracts or macros. Use
`use-component-shape-gpui` for `component_shape_gpui::GpuiComponentShape`,
`component_shape_gpui::component_shape!`, GPUI render contracts, and GPUI value
binding.

Do not use this skill for downstream `gpui-form` integration. Use
`use-gpui-form-component-shapes` when the task is about
`#[gpui_form(component(...))]`, form value storage, generated fields, or
prototyping output in `gpui-form`.

## Ownership Rule

Keep shared behavior in `component-shape` when it does not require a framework:

- Shape-owned capability and prototyping metadata.
- Component suffix validation and derived suffix behavior.
- Field/component use records for downstream generators.
- Syntax wrappers used by macro and generator crates.
- Normalized value-change concepts.
- MCP input metadata that describes structured model-controlled input.

Do not add GPUI dependencies, GPUI types, or GPUI-specific assumptions to the
framework-neutral crate. GPUI-specific crates may depend on `component-shape`;
`component-shape` must remain independent.

## Metadata Guidance

Use `ComponentShapeMetadata` to publish type-owned `PROTOTYPING`,
`CAPABILITIES`, and coarse `MCP_INPUT` constants. Keep those constants aligned
with macro output and downstream generators.

Use `DeclaredComponentShape` only as the trusted-declaration marker emitted by
backend declaration macros or other backend-approved declaration APIs. Do not
make every hand-written `ComponentShapeMetadata` implementation declared by
default.

Use `ComponentShapeFor<Value>` to advertise value-specific compatibility and
MCP input metadata. Its `MCP_INPUT` inherits the shape-level value unless the
shape/value pair needs a more precise override.

Use `ComponentCapabilities` for behavior flags. Prefer capability metadata over
framework-specific branching when the behavior can be described generically.

Use `ComponentPrototyping` for generator-facing naming details such as stable
field suffixes. Suffixes should be valid non-empty ASCII identifier suffixes so
generated identifiers are deterministic and portable.

Use `ComponentFieldName` and `ComponentShapeUse` for an erased source-field to
shape-path record. Add the field type when known, call
`with_shape_metadata::<Shape>()` to copy type-owned metadata, and call
`with_value_mcp_input::<Shape, Value>()` when the selected value type needs its
value-specific MCP input.

Use `ValueChange` for normalized `Unchanged`, `Set`, and `Clear` outcomes. Keep
it separate from any framework-specific event type; downstream integrations
can map their events onto these generic outcomes.

Use `McpInput` for declarative structured input metadata such as text values,
primitive lists, primitive sets, decimal ranges, date ranges, and date-time
ranges.
Use `McpInput::any()` for coarse unconstrained JSON metadata and
`component_shape_mcp::McpAny` for typed tool fields that intentionally accept
any JSON. Leave the default
`McpInput::unsupported()` for shapes that should not advertise structured MCP
input. Keep protocol execution, JSON decoding, authorization, and handler
policy in downstream MCP integration crates or `component-shape-mcp`.
GPUI shape declarations infer common MCP metadata from unambiguous declared
value types. The generated `ComponentShapeFor<Value>` impl carries the
value-specific metadata; custom or ambiguous wire schemas should be handled by
the downstream MCP integration's typed schema or a manual decode/schema impl.
Manual `ComponentShapeFor<Value>` impls inherit the shape-level
`ComponentShapeMetadata::MCP_INPUT` by default; override the value-specific
`MCP_INPUT` only when that value should publish a different coarse MCP shape.
Use `component_shape_mcp::McpToolValue` when an integration needs one value to
provide both JSON Schema and strict MCP decoding. The blanket implementation
covers `Deserialize` types that implement `McpJsonSchema`; arbitrary JSON input
should use `McpAny` explicitly.
Use `component_shape_mcp::McpJsonSchema` when an integration has a concrete
Rust argument type and needs richer JSON Schema than `McpInput::object()`.
Aliases inherit the underlying type schema, and app-owned named structs,
single-field transparent newtypes, or fieldless enums can derive it. Facade
re-exports such as `gpui_form::mcp` and `gpui_table::mcp` are inferred when
unambiguous; use `#[mcp(crate = facade::mcp)]` only for renamed crates or
ambiguous facade paths. The derive follows serde deserialize names, includes
enum deserialize aliases, skips deserialization-skipped fields, rejects
flattened fields, and treats serde-defaulted fields as not required.
Use `component_shape_mcp::McpToolInput` for top-level object-shaped tool
argument structs that should derive schema and strict decoding together.
Use `component_shape_mcp::McpRange<T>` for typed object-shaped range arguments
with nullable `min` and `max` fields.

## Generator and Macro Coordination

When changing shared metadata semantics, update the consumers that normalize or
emit those semantics:

- `crates/component-shape` for framework-neutral contracts.
- `crates/component-shape-codegen` for token helpers, suffix derivation, path
  normalization, `_`-type substitution, or import helpers.
- Framework-specific macro crates only when the shared contract changes their
  public syntax, generated output, or diagnostics.

Prefer small focused tests or fixtures for new metadata rules. For macro-facing
behavior, update the relevant framework-specific compile tests rather than only
testing the lowest-level helper.

## Documentation Sync

When changing user-visible framework-neutral behavior, keep public docs and
contracts aligned:

- crate README or rustdoc for public concepts,
- `AGENTS.md` workspace guidance if ownership boundaries change,
- `component-shape-codegen` docs when generator-facing behavior changes,
- downstream framework guidance only when their public workflow changes.

Keep implementation details, parser internals, and design rationale in focused
rustdoc, tests, fixtures, or topic-specific crate-local docs instead of
user-facing READMEs.
