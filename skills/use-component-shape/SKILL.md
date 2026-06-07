---
name: use-component-shape
description: "Use when Codex needs to work with framework-neutral component-shape metadata, including ComponentShapeMetadata, ComponentCapabilities, ComponentPrototyping, ComponentSuffix, Rust syntax wrappers, value-change metadata, or generator-facing shape contracts."
---

# Use Component Shape

## Scope Boundary

Use this skill for framework-neutral `component-shape` concepts and contracts.
It covers shape metadata, naming, suffix validation, component capabilities,
Rust syntax wrappers, normalized value-change metadata, and generator-facing
shape contracts.

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

- Shape identity and display metadata.
- Capability flags that describe what a component can do.
- Component suffix validation and derived suffix behavior.
- Syntax wrappers used by macro and generator crates.
- Normalized value-change concepts.
- Metadata consumed by downstream generators.

Do not add GPUI dependencies, GPUI types, or GPUI-specific assumptions to the
framework-neutral crate. GPUI-specific crates may depend on `component-shape`;
`component-shape` must remain independent.

## Metadata Guidance

Use `ComponentShapeMetadata` as the normalized description of a component shape.
Keep it stable enough for macro output, documentation, and downstream
generators to agree on the same shape identity and behavior.

Use `ComponentCapabilities` for behavior flags. Prefer capability metadata over
framework-specific branching when the behavior can be described generically.

Use `ComponentPrototyping` for generator-facing naming details such as stable
field suffixes. Suffixes should be valid non-empty ASCII identifier suffixes so
generated identifiers are deterministic and portable.

Use `ValueChange` metadata for normalized value-change behavior. Keep this
separate from any framework-specific event type; downstream integrations can
map generic value-change metadata onto their own event systems.

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

Keep implementation details, parser internals, and design rationale in
`docs/` or crate-local architecture notes instead of user-facing READMEs.
