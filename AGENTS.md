# AGENTS.md

This is the working guide for contributors and coding agents in the
`component-shape` workspace.

Use it to decide:

1. which crate or surface owns a change,
2. whether that surface is user-facing, public integration, or internal,
3. which related docs, macro tests, and generated expectations must change together,
4. which validation command should run before handoff.

For framework-neutral component metadata, start with `crates/component-shape`.

For GPUI component contracts and user-facing macros, start with
`crates/component-shape-gpui`.

## Project Summary

`component-shape` is a Rust workspace for describing reusable component shape
metadata and GPUI-specific component contracts.

Its priorities are:

1. **Framework-neutral metadata**: keep shared shape naming, capability, syntax,
   and value-change concepts independent of GPUI.
2. **GPUI ergonomics**: make GPUI component shape declarations concise through
   runtime contracts and macros.
3. **Generator support**: keep parsing, suffix derivation, and code generation
   helpers consistent for downstream consumers.
4. **Diagnostic quality**: preserve clear compile-time errors for invalid shape
   declarations and incompatible value bindings.

## Quick Decision Flow

Before editing, classify the change:

1. **Find the surface in the workspace map.** Use its audience label to decide
   how much public explanation the change needs.
2. **Keep framework-neutral behavior out of GPUI crates.** Shared metadata,
   suffix validation, syntax wrappers, and value-change primitives belong in
   `crates/component-shape`.
3. **Keep GPUI runtime contracts and GPUI macro exports together.** Public GPUI
   workflows should enter through `crates/component-shape-gpui` unless the
   change is specifically in macro implementation internals.
4. **Sync macro behavior and compile tests.** If macro input syntax, generated
   impls, diagnostics, suffix rules, render metadata, or value-binding behavior
   changes, update the relevant `trybuild` UI tests and stderr fixtures in the
   same change.
5. **Validate narrowly.** Run the smallest command that proves the edited
   behavior or documentation surface is still sound.

## Audience Labels

These labels describe the crate or surface itself, not the documentation file
being edited:

- **User-facing**: normal entry points for application or framework developers.
- **Public integration**: public crates meant for code generation, macros,
  tooling, or deeper customization. These are usually not the default starting
  point.
- **Internal**: implementation details, compile-test fixtures, workspace
  maintenance, and contributor-only workflow surfaces.

## Documentation Placement

### User-Facing Documentation

Treat these surfaces as user-facing when they exist:

- every `README.md` in the workspace,
- rustdoc on public traits, types, functions, and macros,
- public examples or integration snippets.

Even README files for public-integration or internal crates should explain:

- who the crate is for,
- what it does,
- what most users should use instead.

Keep user-facing documentation example-first. Prefer Rust snippets over
prose-only explanations when showing behavior changes.

### Internal Documentation

Use rustdoc, focused tests, compile fixtures, or topic-specific crate-local
docs for internal documentation when deeper design notes are needed.

Keep these topics out of READMEs:

- implementation details,
- proc-macro parsing and expansion details,
- code generation data flow,
- subsystem boundaries,
- design rationale,
- internal relationships between macro, codegen, and runtime crates.

Do not add `ARCHITECTURE.md` files. If implementation notes are needed, use a
specific filename that names the subsystem or keep the guidance next to the code
or tests it explains.

### Test Expectations

The `trybuild` tests under `crates/component-shape-gpui/tests/ui` are public
contract documentation for macro behavior and diagnostics.

Update stderr fixtures when diagnostics intentionally change. Prefer adding a
small focused UI test over broadening an existing one when a new macro rule or
failure mode is introduced.

## Synchronization Rules

When a substantive change modifies public component metadata, GPUI macro input
syntax, generated output, trait contracts, value-binding behavior, render
capability behavior, or diagnostic text:

1. Update the owning crate implementation.
2. Update rustdoc or README guidance when the behavior is user-facing.
3. Update `trybuild` pass or compile-fail tests when macro behavior is involved.
4. Update `.stderr` fixtures only when the new diagnostic output is intentional.
5. Keep these surfaces aligned in the same change unless there is a documented
   reason not to.

Keep `component-shape` behavior independent of GPUI-specific APIs. GPUI crates
may depend on `component-shape`; `component-shape` must not depend on GPUI.

## Workspace Map

### Main User-Facing Entry Points

- `crates/component-shape`
  Audience: **User-facing**
  Role: framework-neutral component shape metadata, capability flags,
  component suffix validation, Rust syntax wrappers, and normalized value-change
  primitives. This crate owns shared contracts such as `ComponentShapeMetadata`,
  `ComponentCapabilities`, `ComponentPrototyping`, `McpInput`,
  `ComponentSuffix`, and `ValueChange`.

- `crates/component-shape-gpui`
  Audience: **User-facing**
  Role: GPUI runtime contracts and public macro re-exports. This is the normal
  GPUI entry point for `GpuiComponentShape`, `GpuiComponentRender`,
  `GpuiComponentShapeFor`, `GpuiComponentValueBinding`, and the
  `GpuiComponentShape` derive or `component_shape!` macro.

### Public Integration Crates

- `crates/component-shape-codegen`
  Audience: **Public integration**
  Role: shared code generation helpers for component-shape consumers. It owns
  token span rewriting, shape path normalization, suffix derivation,
  `_`-type substitution, and import helpers used by macro or generator crates.

- `crates/component-shape-mcp`
  Audience: **Public integration**
  Role: shared MCP tool server, `rmcp` stdio serving, and JSON Schema helpers
  for crates that consume `McpInput` metadata. It does not own downstream
  form or table decoding policy.

- `crates/component-shape-mcp-macros`
  Audience: **Public integration**
  Role: proc-macro implementation for `component-shape-mcp` schema derives.
  Most users should use the `McpJsonSchema` derive re-export from
  `component-shape-mcp` with the `derive` feature instead of depending on this
  crate directly.

- `crates/component-shape-gpui-macros`
  Audience: **Public integration**
  Role: proc-macro implementation for GPUI shape declarations. Most users
  should use the re-exports from `component-shape-gpui` instead of depending on
  this crate directly.

### Internal Tests and Tooling

- `crates/component-shape-gpui/tests/ui`
  Audience: **Internal**
  Role: `trybuild` pass and compile-fail fixtures that lock macro expansion
  behavior and diagnostic output.

- `justfile`
  Audience: **Internal**
  Role: workspace formatting, linting, checking, testing, and publish dry-run
  recipes.

  Key commands:
  - `fmt`: run Rust, TOML, and Markdown formatting.
  - `check`: run workspace `cargo check`.
  - `clippy`: run workspace Clippy.
  - `test`: run workspace tests, including `trybuild`.
  - `test-publish`: run a publish dry run for the workspace.

## Validation and Editing Rules

### Validation After Changes

- Validation is the default after code or workflow changes.
- Run the narrowest command that proves the edited behavior works for the
  affected crate, macro, test fixture, docs, or workspace surface.
- Prefer targeted crate checks before full-workspace validation.
- Use `just check`, `just test`, or a more specific `cargo` command when the
  change spans multiple surfaces.
- If validation cannot be run, state why and what remains unvalidated.
- Do not claim a change works unless it was validated, generated from a source
  of truth, or the remaining risk is explicitly documented.

### When Editing Docs

- Keep READMEs and rustdoc user-facing.
- Move proc-macro expansion details, codegen internals, and subsystem design
  into focused rustdoc, tests, fixtures, or topic-specific crate-local docs.
- Prefer examples over prose-only explanations.
- Keep docs for `component-shape-gpui` aligned with macro tests when macro
  syntax, generated output, or diagnostics change.

### When Editing Rust Crates

- Use `cargo` for build, test, and run tasks.
- Keep dependency versions in the workspace root `Cargo.toml`.
- Use `workspace = true` in member crates.
- Let each crate choose its own dependency features in its own `Cargo.toml`.
- Use `path` dependencies only in the root `Cargo.toml`.
- Non-example crates should reference workspace crates with `workspace = true`,
  not explicit paths.
- Keep `component-shape` framework-neutral. Do not add GPUI dependencies or
  GPUI-specific types there.

### When Editing Proc Macros or Codegen

- Keep macro parsing errors specific and close to the offending syntax.
- Preserve spans when generating diagnostics or transformed tokens.
- Keep generated identifiers stable unless the public helper naming rule is
  intentionally changing.
- Update `trybuild` tests for new success cases, failure cases, or diagnostics.
- Use `TRYBUILD=overwrite cargo test -p component-shape-gpui --test trybuild`
  only when stderr fixture changes are intentional, then inspect the diff.

### When Writing Tests

- Prefer focused unit tests for framework-neutral behavior in
  `crates/component-shape`.
- Prefer `trybuild` tests for macro behavior, generated trait requirements, and
  compile-time diagnostics.
- Keep compile-fail fixture names specific to the rule being tested.
- Prefer raw multiline strings, or `quote! { ... }` in macro contexts, over
  escaped single-line literals for embedded Rust code.

### When Editing Dependency or Release Metadata

- Keep package metadata in `[workspace.package]` unless a crate has a deliberate
  reason to diverge.
- Keep dependency versions in `[workspace.dependencies]`.
- Run `just test-publish` before handoff when publishability, package metadata,
  dependency features, or public crate layout changes.
