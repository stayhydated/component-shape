# AGENTS.md

This is the working guide for contributors and coding agents in the
`component-shape` workspace.

Use it to decide:

1. which crate or surface owns a change,
2. whether that surface is user-facing, public integration, reusable guidance,
   or validation,
3. which docs, skills, macro tests, and diagnostic expectations must change
   together,
4. which validation command should run before handoff.

Start here:

- Framework-neutral component metadata: `crates/component-shape`.
- GPUI component contracts and public macros: `crates/component-shape-gpui`.
- MCP schema, typed tool input, server, resource, prompt, and stdio helpers:
  `crates/component-shape-mcp`.
- Repository command index: `justfile`; run `just --list` to inspect recipes.

## Project Summary

`component-shape` is a Rust workspace for framework-neutral component shape
metadata, GPUI-specific component shape contracts, shared code generation
helpers, and MCP integration helpers.

Keep shared shape naming, capability, syntax, MCP input, and value-change
concepts independent of GPUI. GPUI crates may depend on `component-shape`;
`component-shape` must not depend on GPUI.

## Quick Decision Flow

Before editing, classify the change:

1. **Find the surface in the workspace map.** Use its audience label to decide
   the docs, tests, and validation that must move with the change.
2. **Keep framework-neutral behavior in `crates/component-shape`.** Shared
   metadata, suffix validation, syntax wrappers, `McpInput`, and value-change
   primitives belong there.
3. **Route GPUI public workflows through `crates/component-shape-gpui`.**
   Macro implementation internals belong in `crates/component-shape-gpui-macros`.
4. **Route MCP public workflows through `crates/component-shape-mcp`.** Schema
   derives are implemented in `crates/component-shape-mcp-macros`; coarse
   shape metadata still starts in `crates/component-shape`.
5. **Sync public contracts.** When public API shape, macro syntax, generated
   impls, diagnostics, MCP schema/server behavior, value binding, render
   metadata, or documented usage changes, update the owning crate and any
   rustdoc, README, reusable skill, or test surface that names or locks the
   changed behavior.
6. **Validate narrowly.** Run the smallest evidenced command that proves the
   edited crate, macro, fixture, docs, or workflow surface.

## Audience Labels

- **User-facing**: normal entry points for application, framework, or
  integration developers.
- **Public integration**: crates meant for code generation, proc macros, or
  deeper customization.
- **Reusable guidance**: checked-in Codex skills that document public
  component-shape workflows.
- **Validation**: tests, compile fixtures, and diagnostic expectations.

## Documentation And Sync

Treat these surfaces as user-facing when they name public behavior:

- `crates/component-shape-mcp/README.md`,
- rustdoc on public traits, types, functions, and macros,
- checked-in reusable skills under `skills/`,
- public examples or integration snippets inside tests.

Keep implementation details close to the code, tests, fixtures, or rustdoc that
prove the behavior. Use README and skill files for entry points, supported
workflows, and public examples.

When public component metadata, GPUI macro input syntax, generated output, trait
contracts, value-binding behavior, render capability behavior, MCP tool
metadata, schema or decoding behavior, server/resource/prompt behavior, or
diagnostic text changes:

1. Update the owning crate implementation.
2. Update rustdoc, README, or `skills/` guidance when they name the changed
   behavior.
3. Update `crates/component-shape-codegen` when shared token generation, shape
   path normalization, suffix derivation, `_`-type substitution, imports, or
   MCP metadata token output changes.
4. Update `trybuild` pass or compile-fail tests when GPUI macro behavior is
   involved.
5. Update `.stderr` fixtures only when the new diagnostic output is intentional.
6. Update `AGENTS.md` when ownership, synchronization, or validation guidance
   changes.

## Workspace Map

### User-Facing Crates

- `crates/component-shape`
  Audience: **User-facing**
  Role: framework-neutral component shape metadata, capability flags,
  component suffix validation, Rust syntax wrappers, `McpInput`, and normalized
  value-change primitives. This crate owns shared contracts such as
  `ComponentShapeMetadata`, `ComponentCapabilities`, `ComponentPrototyping`,
  `ComponentSuffix`, `ComponentShapeFor`, and `ValueChange`.

- `crates/component-shape-gpui`
  Audience: **User-facing**
  Role: GPUI runtime contracts and public macro re-exports. This is the normal
  GPUI entry point for `GpuiComponentShape`, `GpuiComponentRender`,
  `GpuiComponentShapeFor`, `GpuiComponentValueBinding`,
  `GpuiComponentShapeBuilder`, the `GpuiComponentShape` derive, and the
  `component_shape!` macro.

- `crates/component-shape-mcp`
  Audience: **User-facing**
  Role: MCP schema, typed decoding, tool metadata, validation metadata,
  structured result, server, resource, prompt, stdio serving, and smoke-client
  helpers for integrations that consume `McpInput` metadata. Its README is the
  current user-facing guide for this surface.

### Public Integration Crates

- `crates/component-shape-codegen`
  Audience: **Public integration**
  Role: shared code generation helpers for component-shape consumers. It owns
  token span rewriting, shape path normalization, suffix derivation,
  `_`-type substitution, import helpers, documentation extraction, and MCP
  metadata token helpers used by macro or generator crates.

- `crates/component-shape-gpui-macros`
  Audience: **Public integration**
  Role: proc-macro implementation for GPUI shape declarations. Most users
  should use the re-exports from `component-shape-gpui` instead of depending on
  this crate directly.

- `crates/component-shape-mcp-macros`
  Audience: **Public integration**
  Role: proc-macro implementation for `component-shape-mcp` schema and typed
  input derives. Most users should use the `McpJsonSchema` and `McpToolInput`
  derive re-exports from `component-shape-mcp` with the `derive` feature.

### Validation And Reusable Guidance

- `crates/component-shape-gpui/tests/ui`
  Audience: **Validation**
  Role: `trybuild` pass and compile-fail fixtures that lock GPUI macro
  expansion behavior and diagnostic output.

- `skills/use-component-shape`
  Audience: **Reusable guidance**
  Role: framework-neutral shape metadata guidance for Codex tasks involving
  `ComponentShapeMetadata`, capabilities, suffixes, `McpInput`, value changes,
  and generator-facing contracts.

- `skills/use-component-shape-gpui`
  Audience: **Reusable guidance**
  Role: GPUI component shape declaration guidance for Codex tasks involving
  `GpuiComponentShape`, `component_shape!`, render contracts, value binding,
  and GPUI macro syntax.

## Validation And Editing Rules

- Run the narrowest command that proves the edited behavior for the affected
  crate, macro, fixture, docs, skill, or workspace surface.
- Use `just check`, `just clippy`, `just test`, or a matching focused `cargo`
  command when the change spans code surfaces.
- Run `just test-publish` before handoff for public crate layout or
  publishability-sensitive Cargo metadata changes.
- If validation cannot run, state why and what remains unvalidated.
- Do not claim a change works unless it was validated or the remaining risk is
  explicitly documented.

### When Editing Proc Macros, Codegen, Or Fixtures

- Keep macro parsing errors specific and close to the offending syntax.
- Preserve spans when generating diagnostics or transformed tokens.
- Keep generated identifiers stable unless the public helper naming rule is
  intentionally changing.
- Add focused `trybuild` pass or compile-fail fixtures for new GPUI macro
  success cases, failure cases, or diagnostics.
- Use `TRYBUILD=overwrite cargo test -p component-shape-gpui --test trybuild`
  only when intentionally regenerating `.stderr` expectations, then inspect the
  `.stderr` diff before handoff.
